//! SCRAM-SHA-256 (RFC 5802, RFC 7677), the password authentication of
//! PostgreSQL servers, with HMAC (RFC 2104) and PBKDF2 (RFC 8018) over
//! SHA-256, and base64 (RFC 4648).

use sha2::{Digest, Sha256};

const BLOCK: usize = 64;

pub fn hmac(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k = [0u8; BLOCK];
    if key.len() > BLOCK {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(k.map(|b| b ^ 0x36));
    inner.update(data);
    let mut outer = Sha256::new();
    outer.update(k.map(|b| b ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

/// PBKDF2-HMAC-SHA256 with one block of output (SCRAM's Hi).
pub fn hi(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut s = salt.to_vec();
    s.extend_from_slice(&1u32.to_be_bytes());
    let mut u = hmac(password, &s);
    let mut out = u;
    for _ in 1..iterations {
        u = hmac(password, &u);
        for (o, x) in out.iter_mut().zip(u) {
            *o ^= x;
        }
    }
    out
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64(d: &[u8]) -> String {
    let mut out = String::new();
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16
            | (c.get(1).copied().unwrap_or(0) as u32) << 8
            | c.get(2).copied().unwrap_or(0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(B64[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

pub fn unbase64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0;
    for c in s.bytes().filter(|&c| c != b'=') {
        let v = B64.iter().position(|&b| b == c)? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

/// A SCRAM-SHA-256 exchange from the client's side.
pub struct Scram {
    password: String,
    nonce: String,
    first_bare: String,
    auth_message: String,
    salted: [u8; 32],
}

impl Scram {
    pub fn new(password: &str, nonce: &str) -> Scram {
        Scram {
            password: password.to_string(),
            nonce: nonce.to_string(),
            // the user name is the one of the startup message
            first_bare: format!("n=,r={nonce}"),
            auth_message: String::new(),
            salted: [0; 32],
        }
    }

    /// client-first-message (no channel binding).
    pub fn client_first(&self) -> String {
        format!("n,,{}", self.first_bare)
    }

    /// client-final-message for the server-first-message.
    pub fn client_final(&mut self, server_first: &str) -> Result<String, String> {
        let attr = |k: &str| {
            server_first
                .split(',')
                .find_map(|a| a.strip_prefix(k).and_then(|v| v.strip_prefix('=')))
        };
        let nonce = attr("r").ok_or("SCRAM: no nonce")?;
        if !nonce.starts_with(&self.nonce) {
            return Err("SCRAM: the server's nonce does not extend ours".into());
        }
        let salt = attr("s").and_then(unbase64).ok_or("SCRAM: no salt")?;
        let iterations: u32 = attr("i")
            .and_then(|i| i.parse().ok())
            .ok_or("SCRAM: no iteration count")?;
        self.salted = hi(self.password.as_bytes(), &salt, iterations);
        let client_key = hmac(&self.salted, b"Client Key");
        let stored_key: [u8; 32] = Sha256::digest(client_key).into();
        let without_proof = format!("c=biws,r={nonce}");
        self.auth_message = format!("{},{server_first},{without_proof}", self.first_bare);
        let signature = hmac(&stored_key, self.auth_message.as_bytes());
        let proof: Vec<u8> = client_key
            .iter()
            .zip(signature)
            .map(|(a, b)| a ^ b)
            .collect();
        Ok(format!("{without_proof},p={}", base64(&proof)))
    }

    /// Checks the server-final-message (the server knows the password).
    pub fn verify(&self, server_final: &str) -> Result<(), String> {
        let server_key = hmac(&self.salted, b"Server Key");
        let expected = base64(&hmac(&server_key, self.auth_message.as_bytes()));
        match server_final.strip_prefix("v=") {
            Some(v) if v == expected => Ok(()),
            _ => Err("SCRAM: the server's signature does not match".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7677_example() {
        // RFC 7677 section 3 (user "user", password "pencil")
        let mut s = Scram::new("pencil", "rOprNGfwEbeRWgbNEkqO");
        s.first_bare = "n=user,r=rOprNGfwEbeRWgbNEkqO".into();
        let server_first = "r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,\
                            s=W22ZaJ0SNY7soEsUEjb6gQ==,i=4096";
        assert_eq!(
            s.client_final(server_first).unwrap(),
            "c=biws,r=rOprNGfwEbeRWgbNEkqO%hvYDpWUa2RaTCAfuxFIlj)hNlF$k0,\
             p=dHzbZapWIk4jUhN+Ute9ytag9zjfMHgsqmmiz7AndVQ="
        );
        s.verify("v=6rriTRBi23WpRR/wtup+mMhUZUn/dB5nLTJRsjl95G4=")
            .unwrap();
    }

    #[test]
    fn base64_round_trip() {
        for d in [&b""[..], b"f", b"fo", b"foo", b"foob"] {
            assert_eq!(unbase64(&base64(d)).unwrap(), d);
        }
        assert_eq!(base64(b"foob"), "Zm9vYg==");
    }
}

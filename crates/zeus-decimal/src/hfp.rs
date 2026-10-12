//! IBM hexadecimal floating point (short, long and extended): COBOL
//! COMP-1 / COMP-2 and PL/I FLOAT data.

/// Decodes IBM hexadecimal floating point (4, 8 or 16 bytes).
pub fn decode(b: &[u8]) -> f64 {
    if b.iter().all(|&x| x & 0x7F == 0 || x == 0) && b.iter().skip(1).all(|&x| x == 0) {
        return 0.0;
    }
    let neg = b[0] & 0x80 != 0;
    let exp = i32::from(b[0] & 0x7F) - 64;
    let frac_bytes = if b.len() >= 8 {
        &b[1..8]
    } else {
        &b[1..b.len()]
    };
    let mut frac: f64 = 0.0;
    let mut scale = 1.0 / 256.0;
    for &x in frac_bytes {
        frac += f64::from(x) * scale;
        scale /= 256.0;
    }
    let v = frac * 16f64.powi(exp);
    if neg { -v } else { v }
}

/// Encodes IBM hexadecimal floating point (4, 8 or 16 bytes, truncated).
pub fn encode(f: f64, out: &mut [u8]) {
    out.iter_mut().for_each(|x| *x = 0);
    if f == 0.0 || !f.is_finite() {
        return;
    }
    let neg = f < 0.0;
    let mut a = f.abs();
    let mut e: i32 = 0;
    while a >= 1.0 {
        a /= 16.0;
        e += 1;
    }
    while a < 1.0 / 16.0 {
        a *= 16.0;
        e -= 1;
    }
    let exp = (e + 64).clamp(0, 127) as u8;
    out[0] = exp | if neg { 0x80 } else { 0 };
    let n = if out.len() >= 8 { 7 } else { out.len() - 1 };
    for x in out.iter_mut().skip(1).take(n) {
        a *= 256.0;
        let d = a.floor();
        *x = d as u8;
        a -= d;
    }
    if out.len() == 16 {
        // extended: low-order part exponent = high - 14
        out[8] = ((i32::from(exp) - 14).clamp(0, 127) as u8) | if neg { 0x80 } else { 0 };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut b = [0u8; 8];
        encode(1.0, &mut b);
        assert_eq!(b, [0x41, 0x10, 0, 0, 0, 0, 0, 0]);
        assert_eq!(decode(&b), 1.0);
        encode(-0.15625, &mut b);
        assert_eq!(decode(&b), -0.15625);
        let mut s = [0u8; 4];
        encode(100.0, &mut s);
        assert_eq!(s, [0x42, 0x64, 0, 0]);
    }
}

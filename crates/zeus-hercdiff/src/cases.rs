//! Generation of decimal-instruction test cases.
//!
//! Each case is one SS-format instruction with two operand fields. Operands
//! are mostly valid packed-decimal numbers of random length, digits and sign,
//! with a small share of invalid digits/signs and invalid length combinations
//! so that data and specification exceptions are exercised too.

use serde::{Deserialize, Serialize};

/// Packed-decimal and related SS instructions (ESA/390 and z/Architecture).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Op {
    Ap,
    Sp,
    Zap,
    Cp,
    Mp,
    Dp,
    Srp,
    Mvo,
    Pack,
    Unpk,
}

impl Op {
    pub const ALL: [Op; 10] = [
        Op::Ap,
        Op::Sp,
        Op::Zap,
        Op::Cp,
        Op::Mp,
        Op::Dp,
        Op::Srp,
        Op::Mvo,
        Op::Pack,
        Op::Unpk,
    ];

    pub fn opcode(self) -> u8 {
        match self {
            Op::Ap => 0xFA,
            Op::Sp => 0xFB,
            Op::Zap => 0xF8,
            Op::Cp => 0xF9,
            Op::Mp => 0xFC,
            Op::Dp => 0xFD,
            Op::Srp => 0xF0,
            Op::Mvo => 0xF1,
            Op::Pack => 0xF2,
            Op::Unpk => 0xF3,
        }
    }

    pub fn mnemonic(self) -> &'static str {
        match self {
            Op::Ap => "AP",
            Op::Sp => "SP",
            Op::Zap => "ZAP",
            Op::Cp => "CP",
            Op::Mp => "MP",
            Op::Dp => "DP",
            Op::Srp => "SRP",
            Op::Mvo => "MVO",
            Op::Pack => "PACK",
            Op::Unpk => "UNPK",
        }
    }
}

/// One test case. Operand fields are at most 16 bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case {
    pub op: Op,
    #[serde(with = "hex")]
    pub op1: Vec<u8>,
    /// Second operand; for SRP it is unused (empty).
    #[serde(with = "hex")]
    pub op2: Vec<u8>,
    /// SRP only: shift amount (six-bit signed, as in the second-operand
    /// address) and rounding digit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub srp: Option<(u8, u8)>,
}

pub mod hex {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02X}")).collect()
    }

    pub fn decode(s: &str) -> Result<Vec<u8>, String> {
        if !s.len().is_multiple_of(2) {
            return Err(format!("odd-length hex string {s:?}"));
        }
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
            .collect()
    }

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        decode(&s).map_err(serde::de::Error::custom)
    }
}

/// xorshift64* - small deterministic generator, so a seed reproduces a run.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.max(1))
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n
    }

    pub fn chance(&mut self, percent: u64) -> bool {
        self.below(100) < percent
    }

    /// 1..=max, biased towards short lengths.
    fn len(&mut self, max: usize) -> usize {
        if self.chance(60) {
            1 + self.below(max.min(4) as u64) as usize
        } else {
            1 + self.below(max as u64) as usize
        }
    }
}

const SIGNS: [u8; 6] = [0xC, 0xD, 0xF, 0xA, 0xB, 0xE];

/// A packed-decimal field of `len` bytes; `significant` digits at most are
/// non-zero (the rest are leading zeros). Occasionally invalid.
fn packed(rng: &mut Rng, len: usize, significant: usize) -> Vec<u8> {
    let ndigits = 2 * len - 1;
    let mut digits = vec![0u8; ndigits];
    let sig = significant.min(ndigits);
    for d in &mut digits[ndigits - sig..] {
        *d = rng.below(10) as u8;
    }
    if rng.chance(15) {
        // all nines or zero - boundary values
        let v = if rng.chance(50) { 9 } else { 0 };
        for d in &mut digits[ndigits - sig..] {
            *d = v;
        }
    }
    let mut sign = if rng.chance(85) {
        SIGNS[rng.below(2) as usize]
    } else {
        SIGNS[rng.below(6) as usize]
    };
    if rng.chance(4) {
        let i = rng.below(ndigits as u64) as usize;
        digits[i] = 0xA + rng.below(6) as u8;
    }
    if rng.chance(3) {
        sign = rng.below(10) as u8;
    }
    let mut out = Vec::with_capacity(len);
    let mut nibbles = digits;
    nibbles.push(sign);
    for pair in nibbles.chunks(2) {
        out.push((pair[0] << 4) | pair[1]);
    }
    out
}

fn bytes(rng: &mut Rng, len: usize) -> Vec<u8> {
    (0..len).map(|_| rng.next_u64() as u8).collect()
}

fn zoned(rng: &mut Rng, len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| {
            let zone = if i + 1 == len && rng.chance(50) {
                0xC0
            } else {
                0xF0
            };
            zone | rng.below(10) as u8
        })
        .collect()
}

pub fn generate(rng: &mut Rng, op: Op) -> Case {
    let (op1, op2, srp) = match op {
        Op::Ap | Op::Sp | Op::Zap | Op::Cp => {
            let l1 = rng.len(16);
            let l2 = rng.len(16);
            let s1 = rng.len(2 * l1);
            let s2 = rng.len(2 * l2);
            (packed(rng, l1, s1), packed(rng, l2, s2), None)
        }
        Op::Mp | Op::Dp => {
            // Valid cases need L2 <= 8 and L2 < L1; keep a few invalid ones.
            let l2 = if rng.chance(95) {
                1 + rng.below(8) as usize
            } else {
                rng.len(16)
            };
            let l1 = if rng.chance(95) {
                (l2 + 1 + rng.below(8) as usize).min(16)
            } else {
                rng.len(16)
            };
            let sig1 = if op == Op::Mp {
                // multiplicand must leave room for the product
                (2 * l1).saturating_sub(2 * l2 + 1).max(1)
            } else {
                rng.len(2 * l1)
            };
            let s1 = rng.len(sig1);
            let s2 = rng.len(2 * l2);
            (packed(rng, l1, s1), packed(rng, l2, s2), None)
        }
        Op::Srp => {
            let l1 = rng.len(16);
            let s1 = rng.len(2 * l1);
            // shift: -32..=31 encoded as six-bit two's complement
            let shift = rng.below(64) as u8;
            let round = rng.below(10) as u8;
            (packed(rng, l1, s1), Vec::new(), Some((shift, round)))
        }
        Op::Mvo => {
            let l1 = rng.len(16);
            let l2 = rng.len(16);
            (bytes(rng, l1), bytes(rng, l2), None)
        }
        Op::Pack => {
            let l1 = rng.len(16);
            let l2 = rng.len(16);
            (bytes(rng, l1), zoned(rng, l2), None)
        }
        Op::Unpk => {
            let l1 = rng.len(16);
            let l2 = rng.len(16);
            let s2 = rng.len(2 * l2);
            (bytes(rng, l1), packed(rng, l2, s2), None)
        }
    };
    Case { op, op1, op2, srp }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic() {
        let a: Vec<Case> = (0..50)
            .map(|i| generate(&mut Rng::new(7 + i), Op::Ap))
            .collect();
        let b: Vec<Case> = (0..50)
            .map(|i| generate(&mut Rng::new(7 + i), Op::Ap))
            .collect();
        assert_eq!(a, b);
    }

    #[test]
    fn field_lengths_in_range() {
        let mut rng = Rng::new(42);
        for _ in 0..2000 {
            for op in Op::ALL {
                let c = generate(&mut rng, op);
                assert!((1..=16).contains(&c.op1.len()));
                assert!(c.op2.len() <= 16);
                assert_eq!(c.op2.is_empty(), op == Op::Srp);
            }
        }
    }

    #[test]
    fn packed_layout() {
        let mut rng = Rng::new(3);
        let p = packed(&mut rng, 3, 5);
        assert_eq!(p.len(), 3);
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(hex::encode(&[0x12, 0x3C]), "123C");
        assert_eq!(hex::decode("123C").unwrap(), [0x12, 0x3C]);
        assert!(hex::decode("123").is_err());
    }
}

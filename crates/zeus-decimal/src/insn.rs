//! Semantics of the z/Architecture decimal instructions, written from the
//! Principles of Operation and checked against Hercules by `zeus-hercdiff`.
//!
//! Each function works on the operand fields in storage. On a program
//! exception the operation is suppressed (operands unchanged) and the
//! exception is returned. Decimal overflow is reported as condition code 3;
//! whether it also causes an interruption depends on the PSW program mask
//! and is left to the caller.

use crate::packed;
use crate::{SIGN_MINUS, SIGN_PLUS, sign_is_negative};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exception {
    Specification,
    Data,
    DecimalDivide,
}

impl Exception {
    /// Program interruption code.
    pub fn code(self) -> u16 {
        match self {
            Exception::Specification => 0x0006,
            Exception::Data => 0x0007,
            Exception::DecimalDivide => 0x000B,
        }
    }
}

type Result<T> = std::result::Result<T, Exception>;

fn valid(field: &[u8]) -> Result<(u128, bool)> {
    packed::decode(field).ok_or(Exception::Data)
}

fn sign_code(negative: bool) -> u8 {
    if negative { SIGN_MINUS } else { SIGN_PLUS }
}

/// Stores a signed result in `op1` and returns the condition code of the
/// add-type instructions (0 zero, 1 < 0, 2 > 0, 3 overflow).
fn store_result(op1: &mut [u8], magnitude: u128, negative: bool) -> u8 {
    let overflow = packed::encode(op1, magnitude, SIGN_PLUS);
    let (stored, _) = packed::decode_unchecked(op1);
    // A zero result is positive, unless it is the truncated result of an
    // overflow, which keeps the sign of the true result.
    let neg = negative && (stored != 0 || overflow);
    packed::encode(op1, magnitude, sign_code(neg));
    if overflow {
        3
    } else if stored == 0 {
        0
    } else if neg {
        1
    } else {
        2
    }
}

fn signed(v: (u128, bool)) -> i128 {
    if v.1 { -(v.0 as i128) } else { v.0 as i128 }
}

fn add_signed(op1: &mut [u8], a: i128, b: i128) -> u8 {
    let r = a + b;
    store_result(op1, r.unsigned_abs(), r < 0)
}

/// ADD DECIMAL.
pub fn ap(op1: &mut [u8], op2: &[u8]) -> Result<u8> {
    let a = valid(op1)?;
    let b = valid(op2)?;
    Ok(add_signed(op1, signed(a), signed(b)))
}

/// SUBTRACT DECIMAL.
pub fn sp(op1: &mut [u8], op2: &[u8]) -> Result<u8> {
    let a = valid(op1)?;
    let b = valid(op2)?;
    Ok(add_signed(op1, signed(a), -signed(b)))
}

/// ZERO AND ADD (the first operand is not checked).
pub fn zap(op1: &mut [u8], op2: &[u8]) -> Result<u8> {
    let b = valid(op2)?;
    Ok(store_result(op1, b.0, b.1))
}

/// COMPARE DECIMAL: 0 equal, 1 first low, 2 first high.
pub fn cp(op1: &[u8], op2: &[u8]) -> Result<u8> {
    let a = signed(valid(op1)?);
    let b = signed(valid(op2)?);
    Ok(match a.cmp(&b) {
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Less => 1,
        std::cmp::Ordering::Greater => 2,
    })
}

fn check_mul_div_lengths(op1: &[u8], op2: &[u8]) -> Result<()> {
    if op2.len() > 8 || op2.len() >= op1.len() {
        return Err(Exception::Specification);
    }
    Ok(())
}

/// MULTIPLY DECIMAL (condition code unchanged).
pub fn mp(op1: &mut [u8], op2: &[u8]) -> Result<()> {
    check_mul_div_lengths(op1, op2)?;
    let a = valid(op1)?;
    let b = valid(op2)?;
    // The multiplicand needs as many leftmost zero bytes as op2 has bytes.
    if op1[..op2.len()].iter().any(|&x| x != 0) {
        return Err(Exception::Data);
    }
    // The sign follows the rules of algebra even for a zero product.
    packed::encode(op1, a.0 * b.0, sign_code(a.1 != b.1));
    Ok(())
}

/// DIVIDE DECIMAL (condition code unchanged): quotient in the leftmost
/// L1-L2 bytes, remainder in the rightmost L2 bytes.
pub fn dp(op1: &mut [u8], op2: &[u8]) -> Result<()> {
    check_mul_div_lengths(op1, op2)?;
    let a = valid(op1)?;
    let b = valid(op2)?;
    if b.0 == 0 {
        return Err(Exception::DecimalDivide);
    }
    let q = a.0 / b.0;
    let r = a.0 % b.0;
    let split = op1.len() - op2.len();
    let qdigits = packed::capacity(split) as u32;
    if q >= 10u128.pow(qdigits) {
        return Err(Exception::DecimalDivide);
    }
    let (quot, rem) = op1.split_at_mut(split);
    packed::encode(quot, q, sign_code(a.1 != b.1));
    packed::encode(rem, r, sign_code(a.1));
    Ok(())
}

/// SHIFT AND ROUND DECIMAL. `shift` is the six-bit shift amount from the
/// second-operand address (two's complement, positive = left), `round` the
/// rounding digit (I3).
pub fn srp(op1: &mut [u8], shift: u8, round: u8) -> Result<u8> {
    let (v, neg) = valid(op1)?;
    let n = (shift & 0x3F) as i32;
    let n = if n >= 32 { n - 64 } else { n };
    let cap = packed::capacity(op1.len()) as u32;
    if n >= 0 {
        let n = n as u32;
        // Digits that stay in the field after shifting left by n.
        let (kept, overflow) = if n >= cap {
            (0, v != 0)
        } else {
            let room = 10u128.pow(cap - n);
            ((v % room) * 10u128.pow(n), v >= room)
        };
        let neg = neg && (kept != 0 || overflow);
        packed::encode(op1, kept, sign_code(neg));
        return Ok(if overflow {
            3
        } else if kept == 0 {
            0
        } else if neg {
            1
        } else {
            2
        });
    }
    let k = (-n) as u32;
    let r = if k > 38 {
        0
    } else {
        let div = 10u128.pow(k);
        (v + u128::from(round) * div / 10) / div
    };
    let neg = neg && r != 0;
    packed::encode(op1, r, sign_code(neg));
    Ok(if r == 0 {
        0
    } else if neg {
        1
    } else {
        2
    })
}

/// MOVE WITH OFFSET: the second operand is placed to the left of the
/// rightmost four bits of the first operand.
pub fn mvo(op1: &mut [u8], op2: &[u8]) {
    let keep = op1[op1.len() - 1] & 0xF;
    let mut nibbles = op2.iter().rev().flat_map(|b| [b & 0xF, b >> 4]);
    let mut out = vec![keep];
    for _ in 1..2 * op1.len() {
        out.push(nibbles.next().unwrap_or(0));
    }
    for (i, b) in op1.iter_mut().rev().enumerate() {
        *b = (out[2 * i + 1] << 4) | out[2 * i];
    }
}

/// PACK: zoned (or any) bytes to packed, right to left.
pub fn pack(op1: &mut [u8], op2: &[u8]) {
    let last = op2[op2.len() - 1];
    // Digits of op2 other than the last, right to left.
    let mut digits = op2[..op2.len() - 1].iter().rev().map(|b| b & 0xF);
    let n = op1.len();
    op1[n - 1] = last.rotate_right(4);
    for i in (0..n - 1).rev() {
        let lo = digits.next().unwrap_or(0);
        let hi = digits.next().unwrap_or(0);
        op1[i] = (hi << 4) | lo;
    }
}

/// UNPACK: packed (or any) bytes to zoned with zone F, right to left.
pub fn unpk(op1: &mut [u8], op2: &[u8]) {
    let last = op2[op2.len() - 1];
    let mut digits = op2[..op2.len() - 1]
        .iter()
        .rev()
        .flat_map(|b| [b & 0xF, b >> 4]);
    let n = op1.len();
    op1[n - 1] = last.rotate_right(4);
    for i in (0..n - 1).rev() {
        op1[i] = 0xF0 | digits.next().unwrap_or(0);
    }
}

/// Whether a packed sign denotes minus; re-exported for callers.
pub fn is_minus(sign: u8) -> bool {
    sign_is_negative(sign)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_overflow() {
        let mut a = [0x12, 0x3C];
        assert_eq!(ap(&mut a, &[0x45, 0x6C]), Ok(2));
        assert_eq!(a, [0x57, 0x9C]);
        let mut b = [0x99, 0x9C];
        assert_eq!(ap(&mut b, &[0x1C]), Ok(3));
        assert_eq!(b, [0x00, 0x0C]);
        let mut c = [0x1A, 0x3C];
        assert_eq!(ap(&mut c, &[0x1C]), Err(Exception::Data));
        assert_eq!(c, [0x1A, 0x3C]);
    }

    #[test]
    fn zap_and_compare() {
        let mut a = [0xFF, 0xFF, 0xFF];
        assert_eq!(zap(&mut a, &[0x5D]), Ok(1));
        assert_eq!(a, [0x00, 0x00, 0x5D]);
        assert_eq!(cp(&[0x0D], &[0x0C]), Ok(0));
        assert_eq!(cp(&[0x1D], &[0x0C]), Ok(1));
    }

    #[test]
    fn multiply_divide() {
        let mut a = [0x00, 0x01, 0x2C];
        assert_eq!(mp(&mut a, &[0x2C]), Ok(()));
        assert_eq!(a, [0x00, 0x02, 0x4C]);
        let mut b = [0x01, 0x2C];
        assert_eq!(mp(&mut b, &[0x2C]), Err(Exception::Data));
        assert_eq!(mp(&mut [0x0C], &[0x1C]), Err(Exception::Specification));
        // 125 / 4 = 31 remainder 1
        let mut c = [0x00, 0x12, 0x5C];
        assert_eq!(dp(&mut c, &[0x4D]), Ok(()));
        assert_eq!(c, [0x03, 0x1D, 0x1C]);
        assert_eq!(
            dp(&mut [0x12, 0x5C], &[0x0C]),
            Err(Exception::DecimalDivide)
        );
    }

    #[test]
    fn shift_and_round() {
        let mut a = [0x01, 0x23, 0x4C];
        assert_eq!(srp(&mut a, 0x3E, 5), Ok(2)); // right 2, round: 1234 -> 12
        assert_eq!(a, [0x00, 0x01, 0x2C]);
        let mut b = [0x01, 0x25, 0x0C];
        assert_eq!(srp(&mut b, 0x3F, 5), Ok(2)); // 1250 -> 125
        assert_eq!(b, [0x00, 0x12, 0x5C]);
        let mut c = [0x12, 0x3C];
        assert_eq!(srp(&mut c, 0x01, 0), Ok(3)); // left 1: 1230 overflows 3 digits
        assert_eq!(c, [0x23, 0x0C]);
        // Shift beyond the field width: zero with the original sign, CC3.
        let mut d = [
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x23,
            0x2D,
        ];
        assert_eq!(srp(&mut d, 0x1D, 9), Ok(3));
        assert_eq!(d[14], 0x0D);
        assert!(d[..14].iter().all(|&b| b == 0));
    }

    #[test]
    fn moves() {
        let mut a = [0x77, 0x88, 0x9C];
        mvo(&mut a, &[0x12, 0x34]);
        assert_eq!(a, [0x01, 0x23, 0x4C]);
        let mut p = [0u8; 3];
        pack(&mut p, &[0xF1, 0xF2, 0xF3, 0xC4]);
        assert_eq!(p, [0x01, 0x23, 0x4C]);
        let mut u = [0u8; 4];
        unpk(&mut u, &[0x12, 0x3C]);
        assert_eq!(u, [0xF0, 0xF1, 0xF2, 0xC3]);
    }
}

//! Packed decimal (USAGE PACKED-DECIMAL / COMP-3): two digits per byte, the
//! rightmost nibble is the sign.

use crate::{Dec, SIGN_MINUS, SIGN_PLUS, SIGN_UNSIGNED, sign_is_negative, sign_is_valid};

/// Digit capacity of a packed field of `len` bytes.
pub fn capacity(len: usize) -> usize {
    2 * len - 1
}

/// Field length in bytes for `digits` digits.
pub fn len_for_digits(digits: usize) -> usize {
    digits / 2 + 1
}

/// Nibbles of a packed field, digits first (left to right), sign last.
pub fn nibbles(field: &[u8]) -> impl Iterator<Item = u8> + '_ {
    field.iter().flat_map(|b| [b >> 4, b & 0xF])
}

/// Whether every digit nibble is 0-9 and the sign nibble is A-F.
pub fn is_valid(field: &[u8]) -> bool {
    let n = 2 * field.len();
    nibbles(field)
        .enumerate()
        .all(|(i, x)| if i + 1 == n { sign_is_valid(x) } else { x <= 9 })
}

/// Decodes the digits and sign. `None` if a nibble is invalid.
pub fn decode(field: &[u8]) -> Option<(u128, bool)> {
    if !is_valid(field) {
        return None;
    }
    Some(decode_unchecked(field))
}

/// Decodes without validity checks: invalid digit nibbles are taken
/// modulo 10 (as storage holds them), any sign nibble other than B/D is plus.
pub fn decode_unchecked(field: &[u8]) -> (u128, bool) {
    let n = 2 * field.len();
    let mut v: u128 = 0;
    let mut neg = false;
    for (i, x) in nibbles(field).enumerate() {
        if i + 1 == n {
            neg = sign_is_negative(x);
        } else {
            v = v * 10 + u128::from(x % 10);
        }
    }
    (v, neg)
}

/// Value of the field with an implied decimal scale.
pub fn to_dec(field: &[u8], scale: u32) -> Option<Dec> {
    let (v, neg) = decode(field)?;
    let c = v as i128;
    Some(Dec::new(if neg { -c } else { c }, scale))
}

/// Encodes `magnitude` with the given sign nibble, keeping the low-order
/// digits that fit. Returns whether nonzero digits were lost.
pub fn encode(field: &mut [u8], magnitude: u128, sign: u8) -> bool {
    let mut v = magnitude;
    let n = field.len();
    for i in (0..n).rev() {
        let (hi, lo) = if i == n - 1 {
            let d = (v % 10) as u8;
            v /= 10;
            (d, sign)
        } else {
            let lo = (v % 10) as u8;
            v /= 10;
            let hi = (v % 10) as u8;
            v /= 10;
            (hi, lo)
        };
        field[i] = (hi << 4) | lo;
    }
    v != 0
}

/// Stores a value (already at the field's scale). `signed` selects C/D
/// signs, otherwise F. Returns whether high-order digits were lost.
pub fn store(field: &mut [u8], value: Dec, signed: bool) -> bool {
    let sign = if !signed {
        SIGN_UNSIGNED
    } else if value.coef < 0 {
        SIGN_MINUS
    } else {
        SIGN_PLUS
    };
    encode(field, value.coef.unsigned_abs(), sign)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut f = [0u8; 3];
        assert!(!store(&mut f, Dec::new(-12345, 2), true));
        assert_eq!(f, [0x12, 0x34, 0x5D]);
        assert_eq!(to_dec(&f, 2).unwrap(), Dec::new(-12345, 2));
        assert!(store(&mut f, Dec::new(123456, 0), false));
        assert_eq!(f, [0x23, 0x45, 0x6F]);
    }

    #[test]
    fn validity() {
        assert!(is_valid(&[0x12, 0x3C]));
        assert!(!is_valid(&[0x1A, 0x3C]));
        assert!(!is_valid(&[0x12, 0x34]));
        assert_eq!(capacity(3), 5);
        assert_eq!(len_for_digits(5), 3);
        assert_eq!(len_for_digits(4), 3);
    }
}

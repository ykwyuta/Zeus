//! Binary integers (USAGE BINARY / COMP / COMP-4 / COMP-5): big-endian two's
//! complement (signed) or unsigned, 2, 4 or 8 bytes.

use crate::Dec;

/// Storage length for a binary item of `digits` digits (Enterprise COBOL:
/// 1-4 digits halfword, 5-9 fullword, 10-18 doubleword).
pub fn len_for_digits(digits: u32) -> usize {
    match digits {
        0..=4 => 2,
        5..=9 => 4,
        _ => 8,
    }
}

pub fn read(field: &[u8], signed: bool) -> i128 {
    let mut v: u128 = 0;
    for &b in field {
        v = (v << 8) | u128::from(b);
    }
    let bits = 8 * field.len() as u32;
    if signed && bits > 0 && bits < 128 && v >> (bits - 1) & 1 == 1 {
        (v as i128) - (1i128 << bits)
    } else {
        v as i128
    }
}

pub fn to_dec(field: &[u8], signed: bool, scale: u32) -> Dec {
    Dec::new(read(field, signed), scale)
}

/// Stores the low-order bytes of `v` (two's complement).
pub fn write(field: &mut [u8], v: i128) {
    let mut x = v as u128;
    for b in field.iter_mut().rev() {
        *b = x as u8;
        x >>= 8;
    }
}

/// Range of a binary field of `len` bytes.
pub fn range(len: usize, signed: bool) -> (i128, i128) {
    let bits = 8 * len as u32;
    if signed {
        (-(1i128 << (bits - 1)), (1i128 << (bits - 1)) - 1)
    } else {
        (0, (1i128 << bits) - 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut f = [0u8; 2];
        write(&mut f, -2);
        assert_eq!(f, [0xFF, 0xFE]);
        assert_eq!(read(&f, true), -2);
        assert_eq!(read(&f, false), 65534);
        let mut g = [0u8; 8];
        write(&mut g, 999_999_999_999_999_999);
        assert_eq!(read(&g, true), 999_999_999_999_999_999);
        assert_eq!(len_for_digits(4), 2);
        assert_eq!(len_for_digits(9), 4);
        assert_eq!(len_for_digits(18), 8);
        assert_eq!(range(2, true), (-32768, 32767));
    }
}

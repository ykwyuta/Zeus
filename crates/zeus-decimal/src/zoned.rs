//! Zoned decimal (USAGE DISPLAY numeric) in EBCDIC: one digit per byte with
//! zone F; the sign is carried in the zone of the last (or first) digit, or
//! as a separate `+`/`-` character.

use crate::{Dec, SIGN_MINUS, SIGN_PLUS, SIGN_UNSIGNED, sign_is_negative, sign_is_valid};

/// EBCDIC `+` and `-`.
pub const PLUS: u8 = 0x4E;
pub const MINUS: u8 = 0x60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignPosition {
    /// No sign: all zones F.
    Unsigned,
    /// Sign in the zone of the last digit (the default).
    TrailingOverpunch,
    LeadingOverpunch,
    TrailingSeparate,
    LeadingSeparate,
}

impl SignPosition {
    pub fn is_separate(self) -> bool {
        matches!(
            self,
            SignPosition::TrailingSeparate | SignPosition::LeadingSeparate
        )
    }
}

/// The digit bytes and the sign of a zoned field.
fn split(field: &[u8], pos: SignPosition) -> (&[u8], Option<u8>) {
    match pos {
        SignPosition::LeadingSeparate if !field.is_empty() => (&field[1..], Some(field[0])),
        SignPosition::TrailingSeparate if !field.is_empty() => {
            (&field[..field.len() - 1], Some(field[field.len() - 1]))
        }
        _ => (field, None),
    }
}

/// Class test NUMERIC for a zoned field.
pub fn is_valid(field: &[u8], pos: SignPosition) -> bool {
    let (digits, sep) = split(field, pos);
    if let Some(s) = sep
        && s != PLUS
        && s != MINUS
    {
        return false;
    }
    let n = digits.len();
    digits.iter().enumerate().all(|(i, &b)| {
        let zone = b >> 4;
        let digit_ok = (b & 0xF) <= 9;
        let overpunch = match pos {
            SignPosition::TrailingOverpunch => i + 1 == n,
            SignPosition::LeadingOverpunch => i == 0,
            _ => false,
        };
        // An unsigned field must have zone F; a signed position accepts the
        // sign zones C, D and F.
        let zone_ok = if overpunch {
            zone == 0xC || zone == 0xD || zone == 0xF
        } else {
            zone == 0xF
        };
        digit_ok && zone_ok
    })
}

/// Decodes the magnitude and sign without validity checks (the low nibble
/// of each digit byte, taken modulo 10).
pub fn decode_unchecked(field: &[u8], pos: SignPosition) -> (u128, bool) {
    let (digits, sep) = split(field, pos);
    let mut v: u128 = 0;
    for &b in digits {
        v = v * 10 + u128::from((b & 0xF) % 10);
    }
    let neg = match pos {
        SignPosition::Unsigned => false,
        SignPosition::TrailingOverpunch => digits.last().is_some_and(|b| zone_negative(*b)),
        SignPosition::LeadingOverpunch => digits.first().is_some_and(|b| zone_negative(*b)),
        _ => sep == Some(MINUS),
    };
    (v, neg)
}

fn zone_negative(b: u8) -> bool {
    let z = b >> 4;
    sign_is_valid(z) && sign_is_negative(z)
}

pub fn to_dec(field: &[u8], pos: SignPosition, scale: u32) -> Dec {
    let (v, neg) = decode_unchecked(field, pos);
    let c = v as i128;
    Dec::new(if neg { -c } else { c }, scale)
}

/// Stores a value (already at the field's scale). Returns whether nonzero
/// high-order digits were lost.
pub fn store(field: &mut [u8], value: Dec, pos: SignPosition) -> bool {
    let neg = value.coef < 0;
    let mut v = value.coef.unsigned_abs();
    let (start, end) = match pos {
        SignPosition::LeadingSeparate => (1, field.len()),
        SignPosition::TrailingSeparate => (0, field.len().saturating_sub(1)),
        _ => (0, field.len()),
    };
    for b in field[start..end].iter_mut().rev() {
        *b = 0xF0 | (v % 10) as u8;
        v /= 10;
    }
    let sign_zone = if neg { SIGN_MINUS } else { SIGN_PLUS };
    match pos {
        SignPosition::Unsigned => {}
        SignPosition::TrailingOverpunch => {
            if let Some(b) = field.last_mut() {
                *b = (sign_zone << 4) | (*b & 0xF);
            }
        }
        SignPosition::LeadingOverpunch => {
            if let Some(b) = field.first_mut() {
                *b = (sign_zone << 4) | (*b & 0xF);
            }
        }
        SignPosition::LeadingSeparate => field[0] = if neg { MINUS } else { PLUS },
        SignPosition::TrailingSeparate => {
            let last = field.len() - 1;
            field[last] = if neg { MINUS } else { PLUS };
        }
    }
    let _ = SIGN_UNSIGNED;
    v != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overpunch() {
        let mut f = [0u8; 4];
        store(&mut f, Dec::new(-123, 0), SignPosition::TrailingOverpunch);
        assert_eq!(f, [0xF0, 0xF1, 0xF2, 0xD3]);
        assert!(is_valid(&f, SignPosition::TrailingOverpunch));
        assert!(!is_valid(&f, SignPosition::Unsigned));
        assert_eq!(
            to_dec(&f, SignPosition::TrailingOverpunch, 1),
            Dec::new(-123, 1)
        );
        store(&mut f, Dec::new(5, 0), SignPosition::LeadingOverpunch);
        assert_eq!(f, [0xC0, 0xF0, 0xF0, 0xF5]);
    }

    #[test]
    fn separate() {
        let mut f = [0u8; 3];
        assert!(store(
            &mut f,
            Dec::new(-123, 0),
            SignPosition::LeadingSeparate
        ));
        assert_eq!(f, [MINUS, 0xF2, 0xF3]);
        assert_eq!(
            to_dec(&f, SignPosition::LeadingSeparate, 0),
            Dec::new(-23, 0)
        );
        store(&mut f, Dec::new(7, 0), SignPosition::TrailingSeparate);
        assert_eq!(f, [0xF0, 0xF7, PLUS]);
        assert!(is_valid(&f, SignPosition::TrailingSeparate));
        assert!(!is_valid(&[0xF1, 0x40], SignPosition::Unsigned));
    }
}

use std::cmp::Ordering;
use std::fmt;

/// An exact decimal value `coef * 10^-scale`.
///
/// The coefficient holds up to 38 decimal digits, enough for COBOL
/// intermediate results (31-digit operands under ARITH(EXTEND)).
#[derive(Debug, Clone, Copy, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dec {
    pub coef: i128,
    pub scale: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rounding {
    /// Truncate towards zero (COBOL default).
    Truncate,
    /// Round half away from zero (COBOL ROUNDED).
    HalfAwayFromZero,
}

pub const MAX_DIGITS: u32 = 38;

pub fn pow10(n: u32) -> i128 {
    10i128.pow(n)
}

impl Dec {
    pub const ZERO: Dec = Dec { coef: 0, scale: 0 };

    pub fn new(coef: i128, scale: u32) -> Self {
        Dec { coef, scale }
    }

    pub fn from_i64(v: i64) -> Self {
        Dec::new(v as i128, 0)
    }

    pub fn is_zero(&self) -> bool {
        self.coef == 0
    }

    pub fn is_negative(&self) -> bool {
        self.coef < 0
    }

    /// Number of decimal digits in the coefficient (0 for zero).
    pub fn digits(&self) -> u32 {
        let mut n = self.coef.unsigned_abs();
        let mut d = 0;
        while n > 0 {
            n /= 10;
            d += 1;
        }
        d
    }

    /// Number of integer digits (digits left of the decimal point).
    pub fn integer_digits(&self) -> u32 {
        self.digits().saturating_sub(self.scale)
    }

    /// Value with the given scale; extra fraction digits are dropped or
    /// rounded. `None` on coefficient overflow.
    pub fn rescale(&self, scale: u32, rounding: Rounding) -> Option<Dec> {
        match scale.cmp(&self.scale) {
            Ordering::Equal => Some(*self),
            Ordering::Greater => {
                let f = checked_pow10(scale - self.scale)?;
                Some(Dec::new(self.coef.checked_mul(f)?, scale))
            }
            Ordering::Less => {
                let drop = self.scale - scale;
                if drop > MAX_DIGITS {
                    return Some(Dec::new(0, scale));
                }
                let f = pow10(drop);
                let mut q = self.coef / f;
                if rounding == Rounding::HalfAwayFromZero {
                    let r = (self.coef % f).abs();
                    if r * 2 >= f {
                        q += self.coef.signum();
                    }
                }
                Some(Dec::new(q, scale))
            }
        }
    }

    /// Both operands brought to the larger scale.
    fn align(a: Dec, b: Dec) -> Option<(i128, i128, u32)> {
        let s = a.scale.max(b.scale);
        Some((
            a.rescale(s, Rounding::Truncate)?.coef,
            b.rescale(s, Rounding::Truncate)?.coef,
            s,
        ))
    }

    pub fn checked_add(self, o: Dec) -> Option<Dec> {
        let (a, b, s) = Dec::align(self, o)?;
        Some(Dec::new(a.checked_add(b)?, s))
    }

    pub fn checked_sub(self, o: Dec) -> Option<Dec> {
        let (a, b, s) = Dec::align(self, o)?;
        Some(Dec::new(a.checked_sub(b)?, s))
    }

    pub fn checked_mul(self, o: Dec) -> Option<Dec> {
        Some(Dec::new(
            self.coef.checked_mul(o.coef)?,
            self.scale + o.scale,
        ))
    }

    /// Quotient with `scale` fraction digits, truncated (or rounded).
    /// `None` on division by zero or overflow.
    pub fn checked_div(self, o: Dec, scale: u32, rounding: Rounding) -> Option<Dec> {
        if o.coef == 0 {
            return None;
        }
        // self/o = (a * 10^-sa) / (b * 10^-sb); scale the dividend so that
        // the integer quotient has `scale` (+1 for rounding) fraction digits.
        let extra = u32::from(rounding == Rounding::HalfAwayFromZero);
        let target = scale + extra;
        let shift = target as i64 + o.scale as i64 - self.scale as i64;
        let (num, den) = if shift >= 0 {
            (self.coef.checked_mul(checked_pow10(shift as u32)?)?, o.coef)
        } else {
            (
                self.coef,
                o.coef.checked_mul(checked_pow10((-shift) as u32)?)?,
            )
        };
        let q = Dec::new(num / den, target);
        q.rescale(scale, rounding)
    }

    /// Remainder of the truncated division `self / o` whose quotient has
    /// `scale` fraction digits: `self - q * o`.
    pub fn checked_rem(self, o: Dec, scale: u32) -> Option<Dec> {
        let q = self.checked_div(o, scale, Rounding::Truncate)?;
        self.checked_sub(q.checked_mul(o)?)
    }

    pub fn abs(self) -> Dec {
        Dec::new(self.coef.abs(), self.scale)
    }

    /// Integer power; `None` when not representable exactly.
    pub fn checked_powi(self, mut e: u32) -> Option<Dec> {
        let mut base = self;
        let mut acc = Dec::new(1, 0);
        while e > 0 {
            if e & 1 == 1 {
                acc = acc.checked_mul(base)?;
            }
            e >>= 1;
            if e > 0 {
                base = base.checked_mul(base)?;
            }
        }
        Some(acc)
    }

    /// Removes trailing fraction zeros.
    pub fn normalize(self) -> Dec {
        let mut d = self;
        while d.scale > 0 && d.coef % 10 == 0 {
            d.coef /= 10;
            d.scale -= 1;
        }
        d
    }

    /// Truncates the value to at most `int_digits` integer digits; returns
    /// the truncated value and whether significant digits were lost.
    pub fn truncate_integer(self, int_digits: u32) -> (Dec, bool) {
        let limit = pow10((int_digits + self.scale).min(MAX_DIGITS));
        if int_digits + self.scale >= MAX_DIGITS {
            return (self, false);
        }
        let r = self.coef % limit;
        (Dec::new(r, self.scale), r != self.coef)
    }

    pub fn to_f64(self) -> f64 {
        self.coef as f64 / 10f64.powi(self.scale as i32)
    }

    /// Closest decimal with `scale` fraction digits.
    pub fn from_f64(v: f64, scale: u32) -> Option<Dec> {
        let x = (v * 10f64.powi(scale as i32)).round();
        if !x.is_finite() || x.abs() >= 1e37 {
            return None;
        }
        Some(Dec::new(x as i128, scale))
    }

    /// Parses `[+-]digits[.digits]`.
    pub fn parse(s: &str) -> Option<Dec> {
        let s = s.trim();
        let (neg, s) = match s.as_bytes().first()? {
            b'-' => (true, &s[1..]),
            b'+' => (false, &s[1..]),
            _ => (false, s),
        };
        let (int, frac) = match s.split_once('.') {
            Some((i, f)) => (i, f),
            None => (s, ""),
        };
        if int.is_empty() && frac.is_empty() {
            return None;
        }
        let mut coef: i128 = 0;
        for c in int.chars().chain(frac.chars()) {
            coef = coef.checked_mul(10)?.checked_add(c.to_digit(10)? as i128)?;
        }
        Some(Dec::new(if neg { -coef } else { coef }, frac.len() as u32))
    }
}

fn checked_pow10(n: u32) -> Option<i128> {
    10i128.checked_pow(n)
}

impl std::ops::Neg for Dec {
    type Output = Dec;

    fn neg(self) -> Dec {
        Dec::new(-self.coef, self.scale)
    }
}

impl PartialEq for Dec {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Dec {}

impl PartialOrd for Dec {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Dec {
    fn cmp(&self, other: &Self) -> Ordering {
        match Dec::align(*self, *other) {
            Some((a, b, _)) => a.cmp(&b),
            // Alignment overflow: the operand with more integer digits wins.
            None => {
                let sign = self.coef.signum().cmp(&other.coef.signum());
                if sign != Ordering::Equal {
                    return sign;
                }
                let mag = self.integer_digits().cmp(&other.integer_digits());
                if self.coef < 0 { mag.reverse() } else { mag }
            }
        }
    }
}

impl fmt::Display for Dec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let digits = self.coef.unsigned_abs().to_string();
        let sign = if self.coef < 0 { "-" } else { "" };
        if self.scale == 0 {
            return write!(f, "{sign}{digits}");
        }
        let s = self.scale as usize;
        let padded = format!("{digits:0>width$}", width = s + 1);
        let (i, fr) = padded.split_at(padded.len() - s);
        write!(f, "{sign}{i}.{fr}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> Dec {
        Dec::parse(s).unwrap()
    }

    #[test]
    fn parse_and_display() {
        assert_eq!(d("-12.340").to_string(), "-12.340");
        assert_eq!(d(".5").to_string(), "0.5");
        assert_eq!(d("7").to_string(), "7");
        assert!(Dec::parse("1.2.3").is_none());
        assert!(Dec::parse("").is_none());
    }

    #[test]
    fn arithmetic() {
        assert_eq!(d("1.5").checked_add(d("2.25")).unwrap(), d("3.75"));
        assert_eq!(d("1.5").checked_sub(d("2.25")).unwrap(), d("-0.75"));
        assert_eq!(d("1.5").checked_mul(d("-2.2")).unwrap(), d("-3.3"));
        assert_eq!(
            d("10").checked_div(d("3"), 4, Rounding::Truncate).unwrap(),
            d("3.3333")
        );
        assert_eq!(
            d("2")
                .checked_div(d("3"), 2, Rounding::HalfAwayFromZero)
                .unwrap(),
            d("0.67")
        );
        assert_eq!(
            d("-2")
                .checked_div(d("3"), 2, Rounding::HalfAwayFromZero)
                .unwrap(),
            d("-0.67")
        );
        assert!(d("1").checked_div(d("0"), 2, Rounding::Truncate).is_none());
        assert_eq!(d("10").checked_rem(d("3"), 0).unwrap(), d("1"));
        assert_eq!(d("1.1").checked_powi(3).unwrap(), d("1.331"));
    }

    #[test]
    fn rescale_and_truncate() {
        assert_eq!(
            d("1.255").rescale(2, Rounding::HalfAwayFromZero).unwrap(),
            d("1.26")
        );
        assert_eq!(
            d("1.255").rescale(2, Rounding::Truncate).unwrap(),
            d("1.25")
        );
        let (v, lost) = d("1234.5").truncate_integer(2);
        assert_eq!(v, d("34.5"));
        assert!(lost);
        assert!(!d("34.5").truncate_integer(2).1);
    }

    #[test]
    fn ordering() {
        assert!(d("1.10") == d("1.1"));
        assert!(d("-1") < d("0.5"));
        assert_eq!(d("0").digits(), 0);
        assert_eq!(d("120.5").integer_digits(), 3);
    }
}

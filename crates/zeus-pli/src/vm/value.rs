//! Run-time values and their representations: arithmetic formatting and
//! parsing, IBM hexadecimal floating point.

use crate::ir::{FileId, LabelId, ProcId};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// FIXED value m × base^-q (base 10 or 2).
    Fixed {
        m: i128,
        q: i16,
        dec: bool,
    },
    Float(f64),
    /// Character string (EBCDIC bytes); also the image of a picture.
    Char(Vec<u8>),
    Bit(Vec<bool>),
    Ptr(u32),
    Label(LabelId, usize),
    Entry(ProcId, usize),
    File(FileId),
}

pub const NULL: u32 = 0xFF00_0000;
pub const SPACE: u8 = 0x40;

impl Value {
    pub fn fixed(m: i128, q: i16, dec: bool) -> Value {
        Value::Fixed { m, q, dec }
    }

    pub fn int(v: i64) -> Value {
        Value::Fixed {
            m: i128::from(v),
            q: 0,
            dec: false,
        }
    }

    pub fn bit1(b: bool) -> Value {
        Value::Bit(vec![b])
    }

    /// Numeric value as f64.
    pub fn as_f64(&self) -> f64 {
        match self {
            Value::Fixed { m, q, dec } => {
                let b: f64 = if *dec { 10.0 } else { 2.0 };
                *m as f64 / b.powi(i32::from(*q))
            }
            Value::Float(f) => *f,
            Value::Bit(b) => bits_to_u128(b) as f64,
            _ => 0.0,
        }
    }

    /// Integer value (truncated toward zero).
    pub fn as_i64(&self) -> i64 {
        match self {
            Value::Fixed { m, q, dec } => {
                if *q <= 0 {
                    let b: i128 = if *dec { 10 } else { 2 };
                    (m.saturating_mul(b.saturating_pow((-*q) as u32))) as i64
                } else {
                    let b: i128 = if *dec { 10 } else { 2 };
                    (m / b.saturating_pow(*q as u32)) as i64
                }
            }
            Value::Float(f) => f.trunc() as i64,
            Value::Bit(b) => bits_to_u128(b) as i64,
            Value::Ptr(p) => i64::from(*p),
            _ => 0,
        }
    }

    pub fn truth(&self) -> bool {
        match self {
            Value::Bit(b) => b.iter().any(|&x| x),
            Value::Fixed { m, .. } => *m != 0,
            Value::Float(f) => *f != 0.0,
            _ => false,
        }
    }
}

pub fn bits_to_u128(b: &[bool]) -> u128 {
    b.iter().fold(0u128, |a, &x| (a << 1) | u128::from(x))
}

pub fn pow(base: i128, e: u32) -> Option<i128> {
    base.checked_pow(e)
}

/// Rescales m × b^-q to scale `to`, truncating toward zero.
pub fn rescale(m: i128, q: i16, to: i16, dec: bool) -> Option<i128> {
    let b: i128 = if dec { 10 } else { 2 };
    if to >= q {
        m.checked_mul(pow(b, (to - q) as u32)?)
    } else {
        let d = pow(b, (q - to) as u32);
        Some(match d {
            Some(d) => m / d,
            None => 0,
        })
    }
}

/// Converts between bases exactly where possible: the value m × 10^-q
/// (dec) as a FIXED BINARY with scale `to` (or the reverse), truncated.
pub fn convert_base(m: i128, q: i16, from_dec: bool, to: i16, to_dec: bool) -> Option<i128> {
    if from_dec == to_dec {
        return rescale(m, q, to, to_dec);
    }
    // value = m / fb^q; result = trunc(value * tb^to)
    let fb: i128 = if from_dec { 10 } else { 2 };
    let tb: i128 = if to_dec { 10 } else { 2 };
    let mut num = m;
    let mut den: i128 = 1;
    if q >= 0 {
        den = pow(fb, q as u32)?;
    } else {
        num = num.checked_mul(pow(fb, (-q) as u32)?)?;
    }
    if to >= 0 {
        num = num.checked_mul(pow(tb, to as u32)?)?;
    } else {
        den = den.checked_mul(pow(tb, (-to) as u32)?)?;
    }
    Some(num / den)
}

/// Number of decimal digits of |m|.
pub fn digits(m: i128) -> u32 {
    let mut n = m.unsigned_abs();
    let mut d = 0;
    while n > 0 {
        n /= 10;
        d += 1;
    }
    d
}

/// FIXED DECIMAL(p,q) to its character form (host text), length p+3 (or
/// p+k+3 with a scale factor F).
pub fn fixed_dec_text(m: i128, p: u8, q: i16) -> String {
    let neg = m < 0;
    let a = m.unsigned_abs();
    let p = u32::from(p);
    if q < 0 || i32::from(q) > p as i32 {
        let k = (i32::from(q).unsigned_abs()).to_string().len() as u32;
        let s = format!(
            "{}{}F{}{}",
            if neg { "-" } else { "" },
            a,
            if q < 0 { '+' } else { '-' },
            i32::from(q).unsigned_abs()
        );
        return format!("{s:>w$}", w = (p + k + 3) as usize);
    }
    let q = q as u32;
    let s = a.to_string();
    let body = if q == 0 {
        s
    } else {
        let s = format!("{s:0>w$}", w = q as usize + 1);
        let (i, f) = s.split_at(s.len() - q as usize);
        format!("{i}.{f}")
    };
    let body = if neg { format!("-{body}") } else { body };
    format!("{body:>w$}", w = (p + 3) as usize)
}

/// FLOAT DECIMAL(p) character form: E(p+8, p-1, p) with a 4-digit
/// exponent.
pub fn float_text(f: f64, p: u8) -> String {
    let p = usize::from(p.max(1));
    let w = p + 8;
    if f == 0.0 {
        let frac = "0".repeat(p - 1);
        let s = if p > 1 {
            format!("0.{frac}E+0000")
        } else {
            "0E+0000".to_string()
        };
        return format!("{s:>w$}");
    }
    let neg = f < 0.0;
    let s = format!("{:.*e}", p - 1, f.abs());
    // s like 1.7350e8
    let (mant, exp) = s.split_once('e').unwrap_or((&s, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let e = format!("{}{:04}", if exp < 0 { '-' } else { '+' }, exp.abs());
    let body = format!("{}{mant}E{e}", if neg { "-" } else { "" });
    format!("{body:>w$}")
}

/// Parses an arithmetic constant in a character string (blanks around;
/// sign allowed). Returns the value and its attributes (dec, p, q) or
/// float.
pub fn parse_number(s: &str) -> Option<Value> {
    let t = s.trim_matches(' ');
    if t.is_empty() {
        return Some(Value::fixed(0, 0, true));
    }
    let (neg, body) = match t.as_bytes()[0] {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    if body.is_empty() || body.starts_with(' ') {
        return None;
    }
    let upper = body.to_ascii_uppercase();
    // binary constant
    if let Some(b) = upper.strip_suffix('B')
        && !b.is_empty()
        && b.chars().all(|c| c == '0' || c == '1' || c == '.')
    {
        let (i, f) = b.split_once('.').unwrap_or((b, ""));
        let digits = format!("{i}{f}");
        let m = i128::from_str_radix(&digits, 2).ok()?;
        return Some(Value::fixed(
            if neg { -m } else { m },
            f.len() as i16,
            false,
        ));
    }
    if upper.contains('E') {
        let v: f64 = upper.parse().ok()?;
        return Some(Value::Float(if neg { -v } else { v }));
    }
    if !upper.chars().all(|c| c.is_ascii_digit() || c == '.')
        || upper.matches('.').count() > 1
        || upper == "."
    {
        return None;
    }
    let (i, f) = upper.split_once('.').unwrap_or((&upper, ""));
    let digits = format!("{i}{f}");
    let m: i128 = if digits.is_empty() {
        0
    } else {
        digits.parse().ok()?
    };
    Some(Value::fixed(if neg { -m } else { m }, f.len() as i16, true))
}

/// Decodes IBM hexadecimal floating point (4, 8 or 16 bytes).
pub fn hfp_decode(b: &[u8]) -> f64 {
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
pub fn hfp_encode(f: f64, out: &mut [u8]) {
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
    fn formats() {
        assert_eq!(fixed_dec_text(2947, 5, 0), "    2947");
        assert_eq!(fixed_dec_text(-1217, 4, 1), " -121.7");
        assert_eq!(fixed_dec_text(0, 1, 0), "   0");
        assert_eq!(fixed_dec_text(5, 3, 2), "  0.05");
        assert_eq!(fixed_dec_text(-3279, 4, -3), "-3279F+3");
        assert_eq!(float_text(1735e5, 5), " 1.7350E+0008");
        assert_eq!(float_text(-0.001663, 5), "-1.6630E-0003");
        assert_eq!(float_text(1.0, 3), " 1.00E+0000");
        assert_eq!(parse_number(" 12.345 "), Some(Value::fixed(12345, 3, true)));
        assert_eq!(parse_number("-3"), Some(Value::fixed(-3, 0, true)));
        assert_eq!(parse_number("   "), Some(Value::fixed(0, 0, true)));
        assert_eq!(parse_number("1 2"), None);
        let mut b = [0u8; 8];
        hfp_encode(1.0, &mut b);
        assert_eq!(b, [0x41, 0x10, 0, 0, 0, 0, 0, 0]);
        assert_eq!(hfp_decode(&b), 1.0);
        hfp_encode(-0.15625, &mut b);
        assert_eq!(hfp_decode(&b), -0.15625);
        assert_eq!(convert_base(1, 1, true, 4, false), Some(1)); // .1 -> 1/16
        assert_eq!(convert_base(10, 2, true, 7, false), Some(12)); // .10 -> 12/128
    }
}

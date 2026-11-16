//! IEEE decimal floating point (DFP) for FLOAT DECIMAL under the
//! FLOAT(DFP) compiler option (LRM, HEXADEC and IEEE attributes: "Under
//! FLOAT(DFP) all computations that would yield a FLOAT DEC result are done
//! using the IEEE decimal floating-point instructions").
//!
//! Values are a sign, a kind (finite, infinity, quiet or signaling NaN), an
//! integer coefficient and a power-of-ten exponent. Storage is the IEEE
//! 754-2008 interchange format with the densely packed decimal (DPD)
//! coefficient, as z/Architecture uses (Principles of Operation, "Decimal
//! Floating-Point Data Formats"): decimal32 for FLOAT DEC(1-7), decimal64
//! for (8-16), decimal128 for (17-34). Results are rounded to the digits of
//! the format, to nearest with ties to even (the default DFP rounding
//! mode). Zeus computes the coefficients in 128-bit integers, so products
//! and quotients of 34-digit (extended) operands are refused.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Finite,
    Inf,
    QNan,
    SNan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dfp {
    pub neg: bool,
    pub kind: Kind,
    pub coef: u128,
    pub exp: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct Fmt {
    pub digits: u32,
    pub emax: i32,
    pub emin: i32,
    pub bias: i32,
    pub ecbits: u32,
    pub declets: u32,
    pub bytes: usize,
}

pub const D32: Fmt = Fmt { digits: 7, emax: 96, emin: -95, bias: 101, ecbits: 6, declets: 2, bytes: 4 };
pub const D64: Fmt = Fmt { digits: 16, emax: 384, emin: -383, bias: 398, ecbits: 8, declets: 5, bytes: 8 };
pub const D128: Fmt = Fmt { digits: 34, emax: 6144, emin: -6143, bias: 6176, ecbits: 12, declets: 11, bytes: 16 };

/// The format of FLOAT DECIMAL(p).
pub fn fmt_for(p: u8) -> Fmt {
    match p {
        0..=7 => D32,
        8..=16 => D64,
        _ => D128,
    }
}

fn ndigits(c: u128) -> u32 {
    if c == 0 { 1 } else { c.ilog10() + 1 }
}

fn pow10(n: u32) -> u128 {
    10u128.pow(n)
}

impl Dfp {
    pub fn finite(neg: bool, coef: u128, exp: i32) -> Dfp {
        Dfp { neg, kind: Kind::Finite, coef, exp }
    }

    pub fn is_finite(&self) -> bool {
        self.kind == Kind::Finite
    }

    pub fn is_zero(&self) -> bool {
        self.is_finite() && self.coef == 0
    }

    /// Finite, nonzero and not below the smallest normal number.
    pub fn is_normal(&self, f: Fmt) -> bool {
        self.is_finite() && self.coef != 0 && self.exp + ndigits(self.coef) as i32 - 1 >= f.emin
    }

    /// From a decimal string `[-]digits[.digits][E[+-]n]`.
    pub fn parse(s: &str) -> Option<Dfp> {
        let s = s.trim();
        let (neg, s) = match s.as_bytes().first()? {
            b'-' => (true, &s[1..]),
            b'+' => (false, &s[1..]),
            _ => (false, s),
        };
        let (mant, e) = match s.find(['E', 'e']) {
            Some(i) => (&s[..i], s[i + 1..].trim().parse::<i32>().ok()?),
            None => (s, 0),
        };
        let (i, f) = mant.split_once('.').unwrap_or((mant, ""));
        let digits = format!("{i}{f}");
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let t = digits.trim_start_matches('0');
        // more digits than fit: keep 38 and round later through the exponent
        let (t, extra) = if t.len() > 38 { (&t[..38], (t.len() - 38) as i32) } else { (t, 0) };
        let coef: u128 = if t.is_empty() { 0 } else { t.parse().ok()? };
        Some(Dfp::finite(neg, coef, e - f.len() as i32 + extra))
    }

    /// From a binary double: its shortest decimal form.
    pub fn from_f64(x: f64) -> Dfp {
        if x.is_nan() {
            return Dfp { neg: false, kind: Kind::QNan, coef: 0, exp: 0 };
        }
        if x.is_infinite() {
            return Dfp { neg: x < 0.0, kind: Kind::Inf, coef: 0, exp: 0 };
        }
        Dfp::parse(&format!("{x:e}")).unwrap_or(Dfp::finite(false, 0, 0))
    }

    pub fn to_f64(&self) -> f64 {
        let v = match self.kind {
            Kind::Finite => format!("{}e{}", self.coef, self.exp).parse().unwrap_or(0.0),
            Kind::Inf => f64::INFINITY,
            _ => f64::NAN,
        };
        if self.neg { -v } else { v }
    }

    /// Rounded to the format (ties to even); the second value tells that
    /// the result overflowed to infinity.
    pub fn round(self, f: Fmt) -> (Dfp, bool) {
        if !self.is_finite() {
            return (self, false);
        }
        let Dfp { neg, mut coef, mut exp, .. } = self;
        let qmin = f.emin - (f.digits as i32 - 1);
        let qmax = f.emax - (f.digits as i32 - 1);
        // digits to drop: beyond the format, or below the smallest exponent
        let mut drop = ndigits(coef).saturating_sub(f.digits) as i32;
        if exp + drop < qmin {
            drop = qmin - exp;
        }
        if drop > 0 {
            let (q, r, half) = if drop as u32 > 38 {
                (0, if coef == 0 { 0 } else { 1 }, u128::MAX)
            } else {
                let d = pow10(drop as u32);
                (coef / d, coef % d, d / 2)
            };
            let up = r > half || (r == half && r != 0 && q % 2 == 1);
            coef = q + u128::from(up);
            exp += drop;
            if ndigits(coef) > f.digits {
                coef /= 10;
                exp += 1;
            }
        }
        if coef == 0 {
            return (Dfp::finite(neg, 0, exp.clamp(qmin, qmax)), false);
        }
        if exp > qmax {
            // fold down: pad the coefficient with zeros if it fits
            let pad = (exp - qmax) as u32;
            if ndigits(coef) + pad <= f.digits {
                return (Dfp::finite(neg, coef * pow10(pad), qmax), false);
            }
            return (Dfp { neg, kind: Kind::Inf, coef: 0, exp: 0 }, true);
        }
        (Dfp::finite(neg, coef, exp), false)
    }

    fn signed(&self) -> Option<i128> {
        i128::try_from(self.coef).ok().map(|c| if self.neg { -c } else { c })
    }

    fn from_signed(v: i128, exp: i32) -> Dfp {
        Dfp::finite(v < 0, v.unsigned_abs(), exp)
    }

    /// x + y (exactly, before rounding); None when the coefficients do not
    /// fit 128 bits.
    pub fn add(self, y: Dfp) -> Option<Dfp> {
        let (a, b) = (self, y);
        if !a.is_finite() || !b.is_finite() {
            return Some(special(a, b, |x, y| x.neg == y.neg));
        }
        if a.coef == 0 && b.coef == 0 {
            return Some(Dfp::finite(a.neg && b.neg, 0, a.exp.min(b.exp)));
        }
        let (hi, lo) = if a.exp >= b.exp { (a, b) } else { (b, a) };
        let shift = (hi.exp - lo.exp) as u32;
        if shift + ndigits(hi.coef) > 37 {
            // the smaller operand is far below the rounding point of the
            // larger: it cannot change the rounded result
            if lo.coef != 0 && hi.coef != 0 {
                return Some(hi);
            }
            return Some(if hi.coef == 0 { lo } else { hi });
        }
        let h = hi.signed()?.checked_mul(i128::try_from(pow10(shift)).ok()?)?;
        let r = h.checked_add(lo.signed()?)?;
        Some(Dfp::from_signed(r, lo.exp))
    }

    pub fn neg(self) -> Dfp {
        Dfp { neg: !self.neg, ..self }
    }

    pub fn mul(self, y: Dfp) -> Option<Dfp> {
        if !self.is_finite() || !y.is_finite() {
            return Some(special(self, y, |_, _| true));
        }
        Some(Dfp::finite(self.neg != y.neg, self.coef.checked_mul(y.coef)?, self.exp + y.exp))
    }

    /// x / y for y not zero, to `digits` digits plus a sticky digit, with
    /// the preferred exponent when exact.
    pub fn div(self, y: Dfp, digits: u32) -> Option<Dfp> {
        if !self.is_finite() || !y.is_finite() {
            return Some(special(self, y, |_, _| true));
        }
        let neg = self.neg != y.neg;
        if self.coef == 0 {
            return Some(Dfp::finite(neg, 0, self.exp - y.exp));
        }
        let k = (digits + 2 + ndigits(y.coef)).saturating_sub(ndigits(self.coef));
        let n = self.coef.checked_mul(pow10(k))?;
        let (mut q, r) = (n / y.coef, n % y.coef);
        let mut exp = self.exp - y.exp - k as i32;
        if r != 0 {
            q = q * 10 + 1;
            exp -= 1;
        } else {
            let ideal = self.exp - y.exp;
            while exp < ideal && q % 10 == 0 {
                q /= 10;
                exp += 1;
            }
        }
        Some(Dfp::finite(neg, q, exp))
    }

    pub fn cmp(&self, y: &Dfp) -> std::cmp::Ordering {
        let key = |d: &Dfp| -> i32 {
            match d.kind {
                Kind::Inf => if d.neg { -2 } else { 2 },
                _ if d.coef == 0 => 0,
                _ => if d.neg { -1 } else { 1 },
            }
        };
        let (ka, kb) = (key(self), key(y));
        if ka != kb || ka.abs() != 1 {
            return ka.cmp(&kb);
        }
        // both finite nonzero, same sign
        let ae = self.exp + ndigits(self.coef) as i32;
        let be = y.exp + ndigits(y.coef) as i32;
        let mag = if ae != be {
            ae.cmp(&be)
        } else {
            // the same number of integer digits: compare the digits
            let (sa, sb) = (self.coef.to_string(), y.coef.to_string());
            let n = sa.len().max(sb.len());
            format!("{sa:0<n$}").cmp(&format!("{sb:0<n$}"))
        };
        if self.neg { mag.reverse() } else { mag }
    }

    /// FLOAT DECIMAL(p) character form (as for hexadecimal floating
    /// point): p digits, E(p+8, p-1, p), the last digit rounded to even.
    pub fn text(&self, p: u8) -> String {
        let p = u32::from(p.max(1));
        let w = (p + 8) as usize;
        let body = match self.kind {
            Kind::Inf => "INF".to_string(),
            Kind::QNan | Kind::SNan => "NAN".to_string(),
            Kind::Finite if self.coef == 0 => {
                if p > 1 {
                    format!("0.{}E+0000", "0".repeat(p as usize - 1))
                } else {
                    "0E+0000".to_string()
                }
            }
            Kind::Finite => {
                let f = Fmt { digits: p, emax: i32::MAX / 2, emin: i32::MIN / 2, ..D128 };
                let (r, _) = Dfp::finite(false, self.coef, self.exp).round(f);
                let digits = format!("{:0<w$}", r.coef.to_string(), w = p as usize);
                let digits = &digits[..p as usize];
                let adj = r.exp + ndigits(r.coef) as i32 - 1;
                let mant = if p > 1 { format!("{}.{}", &digits[..1], &digits[1..]) } else { digits.to_string() };
                format!("{mant}E{}{:04}", if adj < 0 { '-' } else { '+' }, adj.abs())
            }
        };
        let body = if self.neg { format!("-{body}") } else { body };
        format!("{body:>w$}")
    }
}

/// Infinity and NaN in an operation: a NaN gives a quiet NaN; infinities
/// combine by `same` (for addition, the signs agree).
fn special(a: Dfp, b: Dfp, same: impl Fn(&Dfp, &Dfp) -> bool) -> Dfp {
    let nan = Dfp { neg: false, kind: Kind::QNan, coef: 0, exp: 0 };
    if matches!(a.kind, Kind::QNan | Kind::SNan) || matches!(b.kind, Kind::QNan | Kind::SNan) {
        return nan;
    }
    match (a.kind, b.kind) {
        (Kind::Inf, Kind::Inf) if !same(&a, &b) => nan,
        (Kind::Inf, _) => a,
        _ => b,
    }
}

// ---- densely packed decimal ------------------------------------------------

/// Three digits as a 10-bit declet (IEEE 754-2008, Table 3.3).
fn declet(d: u32) -> u32 {
    let (d2, d1, d0) = (d / 100, d / 10 % 10, d % 10);
    let (a, e, i) = (d2 >= 8, d1 >= 8, d0 >= 8);
    let bit = |x: u32, n: u32| (x >> n) & 1;
    let (b, c, dd) = (bit(d2, 2), bit(d2, 1), bit(d2, 0));
    let (f, g, h) = (bit(d1, 2), bit(d1, 1), bit(d1, 0));
    let (j, k, m) = (bit(d0, 2), bit(d0, 1), bit(d0, 0));
    let pack = |v: [u32; 10]| v.iter().fold(0, |acc, x| acc << 1 | x);
    match (a, e, i) {
        (false, false, false) => pack([b, c, dd, f, g, h, 0, j, k, m]),
        (false, false, true) => pack([b, c, dd, f, g, h, 1, 0, 0, m]),
        (false, true, false) => pack([b, c, dd, j, k, h, 1, 0, 1, m]),
        (true, false, false) => pack([j, k, dd, f, g, h, 1, 1, 0, m]),
        (true, true, false) => pack([j, k, dd, 0, 0, h, 1, 1, 1, m]),
        (true, false, true) => pack([f, g, dd, 0, 1, h, 1, 1, 1, m]),
        (false, true, true) => pack([b, c, dd, 1, 0, h, 1, 1, 1, m]),
        (true, true, true) => pack([0, 0, dd, 1, 1, h, 1, 1, 1, m]),
    }
}

fn undeclet(v: u32) -> u32 {
    use std::sync::OnceLock;
    static TABLE: OnceLock<Vec<u16>> = OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = vec![0u16; 1024];
        for d in 0..1000 {
            t[declet(d) as usize] = d as u16;
        }
        // the 24 non-canonical declets are not produced by Zeus; they read
        // as 0 here
        t
    });
    u32::from(t[(v & 1023) as usize])
}

/// The bytes of a value in format `f`.
pub fn encode(d: &Dfp, f: Fmt) -> Vec<u8> {
    let bits = 8 * f.bytes as u32;
    let mut v: u128;
    let sign = u128::from(d.neg) << (bits - 1);
    match d.kind {
        Kind::Inf => v = 0b11110u128 << (bits - 6),
        Kind::QNan => v = 0b11111u128 << (bits - 6),
        Kind::SNan => v = (0b111111u128) << (bits - 7),
        Kind::Finite => {
            let e = (d.exp + f.bias) as u128;
            let digits = format!("{:0>w$}", d.coef, w = f.digits as usize);
            let msd = u128::from(digits.as_bytes()[0] - b'0');
            let mut cc: u128 = 0;
            for chunk in digits.as_bytes()[1..].chunks(3) {
                let n: u32 = std::str::from_utf8(chunk).unwrap_or("0").parse().unwrap_or(0);
                cc = cc << 10 | u128::from(declet(n));
            }
            let etop = e >> f.ecbits;
            let combo = if msd < 8 { etop << 3 | msd } else { 0b11000 | etop << 1 | (msd & 1) };
            let ec = e & ((1 << f.ecbits) - 1);
            v = combo << (bits - 6) | ec << (10 * f.declets) | cc;
        }
    }
    v |= sign;
    v.to_be_bytes()[16 - f.bytes..].to_vec()
}

/// The value of the bytes of format `f`.
pub fn decode(b: &[u8], f: Fmt) -> Dfp {
    let bits = 8 * f.bytes as u32;
    let mut x = [0u8; 16];
    x[16 - f.bytes..].copy_from_slice(&b[..f.bytes]);
    let v = u128::from_be_bytes(x);
    let neg = v >> (bits - 1) & 1 == 1;
    let combo = (v >> (bits - 6)) & 0b11111;
    if combo == 0b11110 {
        return Dfp { neg, kind: Kind::Inf, coef: 0, exp: 0 };
    }
    if combo == 0b11111 {
        let snan = (v >> (bits - 7)) & 1 == 1;
        return Dfp { neg, kind: if snan { Kind::SNan } else { Kind::QNan }, coef: 0, exp: 0 };
    }
    let (etop, msd) = if combo >> 3 == 0b11 {
        ((combo >> 1) & 3, 8 + (combo & 1))
    } else {
        (combo >> 3, combo & 7)
    };
    let ec = (v >> (10 * f.declets)) & ((1 << f.ecbits) - 1);
    let e = (etop << f.ecbits | ec) as i32 - f.bias;
    let mut coef = msd;
    for k in (0..f.declets).rev() {
        let dl = ((v >> (10 * k)) & 1023) as u32;
        coef = coef * 1000 + u128::from(undeclet(dl));
    }
    Dfp::finite(neg, coef, e)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02X}")).collect()
    }

    #[test]
    fn known_encodings() {
        let one = Dfp::finite(false, 1, 0);
        assert_eq!(hex(&encode(&one, D64)), "2238000000000001");
        assert_eq!(hex(&encode(&one, D32)), "22500001");
        assert_eq!(hex(&encode(&Dfp::finite(true, 0, 0), D64)), "A238000000000000");
        assert_eq!(hex(&encode(&Dfp { neg: false, kind: Kind::Inf, coef: 0, exp: 0 }, D64)), "7800000000000000");
        assert_eq!(hex(&encode(&Dfp { neg: false, kind: Kind::QNan, coef: 0, exp: 0 }, D64)), "7C00000000000000");
        // 9999999999999999E369, the largest decimal64
        let max = Dfp::finite(false, 9_999_999_999_999_999, 369);
        assert_eq!(hex(&encode(&max, D64)), "77FCFF3FCFF3FCFF");
    }

    #[test]
    fn round_trip() {
        for c in [0u128, 1, 7, 8, 9, 10, 99, 888, 999, 1000, 1234567, 9_999_999_999_999_999, 8_000_000_000_000_001] {
            for e in [-398, -10, 0, 5, 369] {
                let d = Dfp::finite(c % 3 == 0, c, e);
                assert_eq!(decode(&encode(&d, D64), D64), d, "{c} {e}");
            }
        }
        for n in 0..1000 {
            assert_eq!(undeclet(declet(n)), n);
        }
    }

    #[test]
    fn arithmetic() {
        let p = |s: &str| Dfp::parse(s).unwrap();
        let (s, _) = p("0.1").add(p("0.2")).unwrap().round(D64);
        assert_eq!(s.cmp(&p("0.3")), std::cmp::Ordering::Equal);
        let (q, _) = p("1").div(p("3"), 16).unwrap().round(D64);
        assert_eq!(q, Dfp::finite(false, 3_333_333_333_333_333, -16));
        let (q, _) = p("2").div(p("3"), 16).unwrap().round(D64);
        assert_eq!(q.coef, 6_666_666_666_666_667);
        let (q, _) = p("6.0").div(p("2"), 16).unwrap().round(D64);
        assert_eq!(q, Dfp::finite(false, 30, -1));
        assert_eq!(p("1.50").text(3), " 1.50E+0000");
    }
}

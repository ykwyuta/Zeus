//! Built-in functions.

use super::conv::cmp_num;
use super::datefmt;
use super::value::{self, SPACE, Value};
use super::*;
use crate::builtin::Bif;

fn chars(v: &Value) -> Vec<u8> {
    match v {
        Value::Char(c) => c.clone(),
        Value::Bit(b) => b.iter().map(|&x| if x { 0xF1 } else { 0xF0 }).collect(),
        _ => Vec::new(),
    }
}

fn now_parts() -> (i64, u32, u32, u32, u32, u32, u32) {
    // local time from the system clock (UTC; TZ not applied)
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs() as i64;
    let ms = d.subsec_millis();
    let days = secs.div_euclid(86400);
    let rem = secs.rem_euclid(86400);
    let (y, m, dd) = civil(days);
    (
        y,
        m,
        dd,
        (rem / 3600) as u32,
        (rem / 60 % 60) as u32,
        (rem % 60) as u32,
        ms,
    )
}

/// Days since 1970-01-01 to (year, month, day).
fn civil(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Lilian day number (1 = 15 October 1582) of a date.
fn lilian(y: i64, m: u32, d: u32) -> i64 {
    let (y, m) = if m <= 2 {
        (y - 1, i64::from(m) + 9)
    } else {
        (y, i64::from(m) - 3)
    };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days1970 = era * 146_097 + doe - 719_468;
    days1970 + 141_428
}

/// VALIDDATE: whether `text` is a valid date in `pattern` (YYYY, MM, DD,
/// DDD and any other characters taken literally).
fn valid_date(text: &str, pattern: &str) -> bool {
    let text = text.trim_end();
    if text.len() != pattern.len() {
        return false;
    }
    let field = |name: &str| -> Option<Option<i64>> {
        let at = pattern.find(name)?;
        Some(text.get(at..at + name.len()).and_then(|s| s.parse().ok()))
    };
    let year = match field("YYYY") {
        Some(Some(y)) if (1..=9999).contains(&y) => y,
        Some(_) => return false,
        None => 2000,
    };
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    if let Some(d) = field("DDD") {
        return matches!(d, Some(d) if (1..=if leap { 366 } else { 365 }).contains(&d));
    }
    let month = match field("MM") {
        Some(Some(m)) if (1..=12).contains(&m) => m,
        Some(_) => return false,
        None => 1,
    };
    let days = match month {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };
    match field("DD") {
        Some(Some(d)) => (1..=days).contains(&d),
        Some(None) => false,
        None => true,
    }
}

/// Hexadecimal digits of the HFP form of FLOAT DEC(p) / FLOAT BIN(p).
fn hex_digits(dec: bool, p: u8) -> i32 {
    match (dec, p) {
        (true, 0..=6) | (false, 0..=21) => 6,
        (true, 7..=16) | (false, 22..=53) => 14,
        _ => 28,
    }
}

/// Storage bits of a FIXED BINARY integer type: 8, 16, 32 or 64.
fn bin_width(t: &Ty) -> u32 {
    match t.arith() {
        Some(Ty::Fixed {
            dec: false,
            p,
            unsigned,
            ..
        }) => {
            let p = u32::from(p) + u32::from(!unsigned);
            match p {
                0..=8 => 8,
                9..=16 => 16,
                17..=32 => 32,
                _ => 64,
            }
        }
        _ => 32,
    }
}

/// The error function (Abramowitz-Stegun 7.1.5 series for small x, the
/// continued fraction of erfc for large x); within 1E-15.
fn erf(x: f64) -> f64 {
    if x.abs() > 2.5 {
        return x.signum() * (1.0 - erfc_cf(x.abs()));
    }
    let mut sum = x;
    let mut term = x;
    let x2 = x * x;
    for n in 1..200 {
        term *= -x2 / n as f64;
        let t = term / (2 * n + 1) as f64;
        sum += t;
        if t.abs() < 1e-17 * sum.abs() {
            break;
        }
    }
    sum * 2.0 / std::f64::consts::PI.sqrt()
}

fn erfc(x: f64) -> f64 {
    if x > 2.5 { erfc_cf(x) } else { 1.0 - erf(x) }
}

/// erfc for x > 0 by its continued fraction (Lentz).
fn erfc_cf(x: f64) -> f64 {
    let tiny = 1e-300;
    let mut f = x;
    let mut c = x;
    let mut d = 0.0;
    for n in 1..300 {
        let a = n as f64 / 2.0;
        d = x + a * d;
        d = if d.abs() < tiny { tiny } else { d };
        c = x + a / c;
        c = if c.abs() < tiny { tiny } else { c };
        d = 1.0 / d;
        let delta = c * d;
        f *= delta;
        if (delta - 1.0).abs() < 1e-16 {
            break;
        }
    }
    (-x * x).exp() / (f * std::f64::consts::PI.sqrt())
}

/// ln(GAMMA(x)) for x > 0 (Lanczos, g = 7).
fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        let pi = std::f64::consts::PI;
        return (pi / (pi * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = G[0];
    let t = x + 7.5;
    for (i, g) in G.iter().enumerate().skip(1) {
        a += g / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The next HFP value above (up) or below x with h hexadecimal digits.
fn hfp_step(x: f64, h: i32, up: bool) -> f64 {
    if x == 0.0 {
        return if up {
            16f64.powi(-65)
        } else {
            -16f64.powi(-65)
        };
    }
    if x < 0.0 {
        return -hfp_step(-x, h, !up);
    }
    // x = f * 16**e, 1/16 <= f < 1
    let mut e = 0;
    let mut a = x;
    while a >= 1.0 {
        a /= 16.0;
        e += 1;
    }
    while a < 1.0 / 16.0 {
        a *= 16.0;
        e -= 1;
    }
    if up {
        x + 16f64.powi(e - h)
    } else if a == 1.0 / 16.0 {
        // the smallest fraction: the step below is one digit finer
        x - 16f64.powi(e - 1 - h)
    } else {
        x - 16f64.powi(e - h)
    }
}

/// Y4DATE / Y4JULIAN / Y4YEAR: the two leading year digits widened by the
/// window w (a year, or a negative offset from the current year).
fn widen_year(d: &str, w: i64) -> String {
    let w = if w < 0 {
        let (y, ..) = now_parts();
        y + w
    } else {
        w
    };
    let yy: i64 = d.get(0..2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let century = if yy < w % 100 { w / 100 + 1 } else { w / 100 };
    format!("{:04}{}", century * 100 + yy, d.get(2..).unwrap_or(""))
}

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64_encode(src: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for chunk in src.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for k in 0..4 {
            out.push(if k <= chunk.len() {
                B64[(n >> (18 - 6 * k) & 63) as usize]
            } else {
                b'='
            });
        }
    }
    out
}

fn base64_decode(src: &[u8]) -> Option<Vec<u8>> {
    let mut bits = 0u32;
    let mut n = 0;
    let mut out = Vec::new();
    for &c in src {
        if c == b'=' {
            break;
        }
        let v = B64.iter().position(|&d| d == c)? as u32;
        bits = (bits << 6) | v;
        n += 6;
        if n >= 8 {
            n -= 8;
            out.push((bits >> n) as u8);
        }
    }
    Some(out)
}

fn hex_decode(src: &[u8]) -> Option<Vec<u8>> {
    if !src.len().is_multiple_of(2) {
        return None;
    }
    src.chunks(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).ok()?, 16).ok())
        .collect()
}

/// COLLAPSE and SQUEEZE: runs of y become one y from position n on; COLLAPSE
/// also trims leading and trailing y (of that part).
fn squeeze(x: &[u8], y: u8, n: usize, trim: bool) -> Vec<u8> {
    let start = n.max(1) - 1;
    if start >= x.len() {
        return x.to_vec();
    }
    let mut out = x[..start].to_vec();
    let mut part: Vec<u8> = Vec::new();
    for &c in &x[start..] {
        if c == y && part.last() == Some(&y) {
            continue;
        }
        part.push(c);
    }
    if trim {
        while part.first() == Some(&y) {
            part.remove(0);
        }
        while part.last() == Some(&y) {
            part.pop();
        }
    }
    out.extend(part);
    out
}

fn ebcdic(s: &str) -> Vec<u8> {
    zeus_cobol::ebcdic::encode(s)
}

impl Machine {
    pub fn builtin(&mut self, b: Bif, args: &[Expr], rt: &Ty, act: usize) -> R<Value> {
        use Bif::*;
        // IFTHENELSE evaluates only the operand it returns
        if b == IfThenElse {
            let c = self.eval(&args[0], act)?;
            let truth = match &c {
                Value::Bit(bits) => bits.iter().any(|x| *x),
                Value::Char(_) => {
                    let bits =
                        self.convert(c.clone(), &args[0].ty, &crate::sema::expr::bit_any(), act)?;
                    matches!(bits, Value::Bit(b) if b.iter().any(|x| *x))
                }
                x => x.as_f64() != 0.0,
            };
            let e = &args[if truth { 1 } else { 2 }];
            let v = self.eval(e, act)?;
            return self.convert(v, &e.ty, rt, act);
        }
        let mut v = Vec::new();
        for a in args {
            v.push(self.eval(a, act)?);
        }
        let int = |i: usize| v.get(i).map(|x| x.as_i64());
        Ok(match b {
            Abs => match &v[0] {
                Value::Fixed { m, q, dec } => Value::fixed(m.abs(), *q, *dec),
                Value::Cplx(c) => Value::Float(c.0.as_f64().hypot(c.1.as_f64())),
                x => Value::Float(x.as_f64().abs()),
            },
            Sign => {
                let f = cmp_num(&v[0], &Value::int(0));
                Value::int(match f {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                })
            }
            Max | Min => {
                let mut best = 0;
                for i in 1..v.len() {
                    let o = cmp_num(&v[i], &v[best]);
                    if (b == Max && o == std::cmp::Ordering::Greater)
                        || (b == Min && o == std::cmp::Ordering::Less)
                    {
                        best = i;
                    }
                }
                self.convert(v[best].clone(), &args[best].ty, rt, act)?
            }
            Mod | Rem => {
                let (x, y) = (&v[0], &v[1]);
                if y.as_f64() == 0.0 {
                    if self.enabled_now(act, cond::ZERODIVIDE) {
                        self.raise(Cond::ZeroDivide, 320, act)?;
                    }
                    return Ok(Value::fixed(0, 0, true));
                }
                match (x, y, rt) {
                    (
                        Value::Fixed { m: m1, q: q1, dec },
                        Value::Fixed { m: m2, q: q2, .. },
                        Ty::Fixed { q, .. },
                    ) => {
                        let s = (*q1).max(*q2).max(*q);
                        let a = value::rescale(*m1, *q1, s, *dec).unwrap_or(0);
                        let c = value::rescale(*m2, *q2, s, *dec).unwrap_or(1);
                        let r = if b == Mod {
                            a.rem_euclid(c.abs())
                        } else {
                            a % c
                        };
                        let r = value::rescale(r, s, *q, *dec).unwrap_or(0);
                        Value::fixed(r, *q, *dec)
                    }
                    _ => {
                        let (a, c) = (x.as_f64(), y.as_f64());
                        Value::Float(if b == Mod {
                            a.rem_euclid(c.abs())
                        } else {
                            a % c
                        })
                    }
                }
            }
            Round | RoundAwayFromZero | RoundToEven => {
                let n = int(1).unwrap_or(0);
                match (&v[0], rt) {
                    (Value::Fixed { m, q, dec }, Ty::Fixed { q: rq, .. }) => {
                        let base: i128 = if *dec { 10 } else { 2 };
                        let drop = i64::from(*q) - n;
                        let r = if drop > 0 {
                            let d = base.pow(drop as u32);
                            let half = d / 2;
                            let a = m.abs();
                            let mut r = (a + half) / d;
                            // ROUNDTOEVEN: a tie goes to the even neighbour
                            if b == RoundToEven && a % d == half && d % 2 == 0 && r % 2 == 1 {
                                r -= 1;
                            }
                            if *m < 0 { -r } else { r }
                        } else {
                            m * base.pow((-drop) as u32)
                        };
                        let r = value::rescale(r, n as i16, *rq, *dec).unwrap_or(r);
                        Value::fixed(r, *rq, *dec)
                    }
                    (x, _) => Value::Float(x.as_f64()),
                }
            }
            Trunc | Ceil | Floor => match (&v[0], rt) {
                (Value::Fixed { m, q, dec }, _) => {
                    let base: i128 = if *dec { 10 } else { 2 };
                    let d = if *q > 0 { base.pow(*q as u32) } else { 1 };
                    let t = m / d;
                    let r = match b {
                        Ceil if *m > 0 && m % d != 0 => t + 1,
                        Floor if *m < 0 && m % d != 0 => t - 1,
                        _ => t,
                    };
                    let r = if *q < 0 {
                        m * base.pow((-*q) as u32)
                    } else {
                        r
                    };
                    Value::fixed(r, 0, *dec)
                }
                (x, _) => {
                    let f = x.as_f64();
                    Value::Float(match b {
                        Ceil => f.ceil(),
                        Floor => f.floor(),
                        _ => f.trunc(),
                    })
                }
            },
            Sqrt | Exp | Log | Log2 | Log10 | Sin | Cos | Tan | Sinh | Cosh | Tanh | Asin
            | Acos | Atan | Atanh | Sind | Cosd | Tand | Atand => {
                let x = v[0].as_f64();
                let deg = std::f64::consts::PI / 180.0;
                let r = match b {
                    Sqrt => {
                        if x < 0.0 {
                            self.raise(Cond::Error, 1500, act)?;
                        }
                        x.sqrt()
                    }
                    Exp => x.exp(),
                    Log => x.ln(),
                    Log2 => x.log2(),
                    Log10 => x.log10(),
                    Sin => x.sin(),
                    Cos => x.cos(),
                    Tan => x.tan(),
                    Sinh => x.sinh(),
                    Cosh => x.cosh(),
                    Tanh => x.tanh(),
                    Asin => x.asin(),
                    Acos => x.acos(),
                    Atan => match v.get(1) {
                        Some(y) => x.atan2(y.as_f64()),
                        None => x.atan(),
                    },
                    Atanh => x.atanh(),
                    Sind => (x * deg).sin(),
                    Cosd => (x * deg).cos(),
                    Tand => (x * deg).tan(),
                    _ => x.atan() / deg,
                };
                Value::Float(r)
            }
            Random => {
                self.random ^= self.random << 13;
                self.random ^= self.random >> 7;
                self.random ^= self.random << 17;
                Value::Float((self.random >> 11) as f64 / (1u64 << 53) as f64)
            }
            Substr => {
                let s = int(1).unwrap_or(1);
                let n = int(2);
                match &v[0] {
                    Value::Bit(x) => Value::Bit(self.substr(x, s, n, act)?),
                    x => {
                        let c = chars(x);
                        Value::Char(self.substr(&c, s, n, act)?)
                    }
                }
            }
            Index | Search | Verify | Tally => {
                let (x, y) = (chars(&v[0]), chars(&v[1]));
                let start = int(2).unwrap_or(1);
                if start < 1 || start as usize > x.len() + 1 {
                    if v.len() > 2 && self.enabled_now(act, cond::STRINGRANGE) {
                        self.raise(Cond::StringRange, 350, act)?;
                    }
                    return Ok(Value::int(0));
                }
                let from = (start - 1) as usize;
                let r = match b {
                    Index => {
                        if y.is_empty() || x.is_empty() {
                            0
                        } else {
                            x[from..]
                                .windows(y.len())
                                .position(|w| w == y.as_slice())
                                .map_or(0, |p| p + from + 1)
                        }
                    }
                    Search => x[from..]
                        .iter()
                        .position(|c| y.contains(c))
                        .map_or(0, |p| p + from + 1),
                    Verify => {
                        if x.is_empty() {
                            0
                        } else {
                            x[from..]
                                .iter()
                                .position(|c| !y.contains(c))
                                .map_or(0, |p| p + from + 1)
                        }
                    }
                    _ => {
                        if y.is_empty() {
                            0
                        } else {
                            let mut n = 0;
                            let mut i = 0;
                            while i + y.len() <= x.len() {
                                if x[i..i + y.len()] == y[..] {
                                    n += 1;
                                    i += y.len();
                                } else {
                                    i += 1;
                                }
                            }
                            n
                        }
                    }
                };
                Value::int(r as i64)
            }
            Length => Value::int(match &v[0] {
                Value::Char(c) => c.len(),
                Value::Bit(b) => b.len(),
                _ => 0,
            } as i64),
            MaxLength => Value::int(match args[0].ty.str_len() {
                Some(Ext::Const(n)) => i64::from(n),
                _ => match &v[0] {
                    Value::Char(c) => c.len() as i64,
                    Value::Bit(b) => b.len() as i64,
                    _ => 0,
                },
            }),
            Copy | Repeat => {
                let n = int(1).unwrap_or(0);
                let times = if b == Repeat {
                    (n.max(0) + 1) as usize
                } else {
                    n.max(0) as usize
                };
                match &v[0] {
                    Value::Bit(x) => Value::Bit(x.repeat(times)),
                    x => Value::Char(chars(x).repeat(times)),
                }
            }
            Reverse => match &v[0] {
                Value::Bit(x) => Value::Bit(x.iter().rev().copied().collect()),
                x => Value::Char(chars(x).into_iter().rev().collect()),
            },
            Left | Right | Center | CenterRight => {
                let s = chars(&v[0]);
                let n = int(1).unwrap_or(0).max(0) as usize;
                let pad = v
                    .get(2)
                    .map(|p| chars(p).first().copied().unwrap_or(SPACE))
                    .unwrap_or(SPACE);
                Value::Char(match b {
                    Left => {
                        let mut s = s;
                        s.resize(n, pad);
                        s
                    }
                    Right => {
                        if s.len() >= n {
                            s[s.len() - n..].to_vec()
                        } else {
                            let mut out = vec![pad; n - s.len()];
                            out.extend(s);
                            out
                        }
                    }
                    _ => {
                        if s.len() >= n {
                            let cut = (s.len() - n) / 2;
                            s[cut..cut + n].to_vec()
                        } else {
                            let total = n - s.len();
                            let left = if b == Center {
                                total / 2
                            } else {
                                total.div_ceil(2)
                            };
                            let mut out = vec![pad; left];
                            out.extend(&s);
                            out.resize(n, pad);
                            out
                        }
                    }
                })
            }
            Trim => {
                let s = chars(&v[0]);
                let lead = v.get(1).map(chars).unwrap_or_else(|| vec![SPACE]);
                let trail = v.get(2).map(chars).unwrap_or_else(|| vec![SPACE]);
                let start = s.iter().position(|c| !lead.contains(c)).unwrap_or(s.len());
                let end = s
                    .iter()
                    .rposition(|c| !trail.contains(c))
                    .map_or(start, |p| p + 1)
                    .max(start);
                Value::Char(s[start..end].to_vec())
            }
            Translate => {
                let s = chars(&v[0]);
                let to = chars(&v[1]);
                let from = match v.get(2) {
                    Some(f) => chars(f),
                    None => (0..=255u8).collect(),
                };
                let mut to = to;
                to.resize(from.len(), SPACE);
                Value::Char(
                    s.iter()
                        .map(|c| match from.iter().position(|f| f == c) {
                            Some(p) => to[p],
                            None => *c,
                        })
                        .collect(),
                )
            }
            Lowercase | Uppercase => {
                let s = chars(&v[0]);
                Value::Char(
                    s.into_iter()
                        .map(|c| match (b, c) {
                            (Lowercase, 0xC1..=0xC9 | 0xD1..=0xD9 | 0xE2..=0xE9) => c - 0x40,
                            (Uppercase, 0x81..=0x89 | 0x91..=0x99 | 0xA2..=0xA9) => c + 0x40,
                            _ => c,
                        })
                        .collect(),
                )
            }
            Collate => Value::Char((0..=255u8).collect()),
            High | Low => Value::Char(vec![
                if b == High { 0xFF } else { 0x00 };
                int(0).unwrap_or(0).max(0) as usize
            ]),
            Real | Imag | Conjg if matches!(v.first(), Some(Value::Cplx(_))) => {
                let Some(Value::Cplx(c)) = v.into_iter().next() else {
                    unreachable!()
                };
                let (re, im) = *c;
                match b {
                    Real => re,
                    Imag => im,
                    Conjg => Value::cplx(
                        re,
                        match im {
                            Value::Fixed { m, q, dec } => Value::fixed(-m, q, dec),
                            Value::Float(f) => Value::Float(-f),
                            v => v,
                        },
                    ),
                    _ => unreachable!(),
                }
            }
            Complex => {
                let mut it = v.into_iter();
                let re = it.next().unwrap_or(Value::int(0));
                let im = it.next().unwrap_or(Value::int(0));
                Value::cplx(re, im)
            }
            Hex => {
                let bytes = match (&v[0], &args[0].ty) {
                    (Value::Char(c), Ty::Char { .. } | Ty::Wchar { .. }) => c.clone(),
                    (Value::Bit(bits), _) => bits
                        .chunks(8)
                        .map(|c| {
                            let mut x = 0u8;
                            for (k, &on) in c.iter().enumerate() {
                                x |= u8::from(on) << (7 - k);
                            }
                            x
                        })
                        .collect(),
                    (x, t) => {
                        let (n, _) = t.storage(true);
                        let mut b = vec![0u8; n as usize];
                        let loc_ty = t.clone();
                        let addr = self.stack_alloc(n.max(1));
                        let loc = Loc {
                            bit: u64::from(addr) * 8,
                            ty: loc_ty,
                        };
                        if self.store(&loc, x).is_ok() {
                            b = self.read(addr, n as usize)?;
                        }
                        b
                    }
                };
                let sep = v.get(1).map(chars);
                let mut s = std::string::String::new();
                for (i, x) in bytes.iter().enumerate() {
                    if i > 0
                        && i % 4 == 0
                        && let Some(sep) = &sep
                    {
                        s.push_str(&zeus_cobol::ebcdic::decode(sep));
                    }
                    s.push_str(&format!("{x:02X}"));
                }
                Value::Char(ebcdic(&s))
            }
            Bool => {
                let (Value::Bit(mut x), Value::Bit(mut y), Value::Bit(z)) = (
                    v[0].clone(),
                    v[1].clone(),
                    self.convert(v[2].clone(), &args[2].ty, &Ty::bit(4), act)?,
                ) else {
                    return err("BOOL arguments");
                };
                let n = x.len().max(y.len());
                x.resize(n, false);
                y.resize(n, false);
                Value::Bit(
                    x.iter()
                        .zip(&y)
                        .map(|(a, c)| z[usize::from(*a) * 2 + usize::from(*c)])
                        .collect(),
                )
            }
            Unspec => {
                let (n, _) = args[0].ty.storage(true);
                let addr = self.stack_alloc(n.max(1));
                let loc = Loc {
                    bit: u64::from(addr) * 8,
                    ty: args[0].ty.clone(),
                };
                self.store(&loc, &v[0])?;
                Value::Bit(self.read_bits(loc.bit, u64::from(n) * 8)?)
            }
            Date => {
                let (y, m, d, ..) = now_parts();
                Value::Char(ebcdic(&format!("{:02}{m:02}{d:02}", y % 100)))
            }
            Time => {
                let (_, _, _, h, mi, s, ms) = now_parts();
                Value::Char(ebcdic(&format!("{h:02}{mi:02}{s:02}{ms:03}")))
            }
            Datetime => {
                let (y, m, d, h, mi, s, ms) = now_parts();
                Value::Char(ebcdic(&format!(
                    "{y:04}{m:02}{d:02}{h:02}{mi:02}{s:02}{ms:03}"
                )))
            }
            Days => match v.first() {
                Some(x) => {
                    let t = zeus_cobol::ebcdic::decode(&chars(x));
                    let p = pattern(&v, 1, "YYYYMMDDHHMISS999");
                    match datefmt::parse(&t, &p, window(int(2))) {
                        Some(st) => Value::int(datefmt::lilian_day(st.y, st.m, st.d)),
                        None => {
                            self.raise(Cond::Error, 2512, act)?;
                            Value::int(0)
                        }
                    }
                }
                None => {
                    let st = now_stamp();
                    Value::int(datefmt::lilian_day(st.y, st.m, st.d))
                }
            },
            DaysToDate => {
                let p = pattern(&v, 1, "YYYYMMDDHHMISS999");
                let st = datefmt::Stamp::from_micros(int(0).unwrap_or(1) * 86_400_000_000);
                Value::Char(ebcdic(&datefmt::format(&st, &p)))
            }
            Iand | Ior | Ieor => {
                let mut acc = int(0).unwrap_or(0);
                for i in 1..v.len() {
                    let x = int(i).unwrap_or(0);
                    acc = match b {
                        Iand => acc & x,
                        Ior => acc | x,
                        _ => acc ^ x,
                    };
                }
                Value::int(acc)
            }
            // INOT, ISLL and ISRL work on the bits of the result precision:
            // 32 bits for FIXED BIN(31), 64 for FIXED BIN(63)
            Inot | Isll | Isrl => {
                let wide = matches!(rt, Ty::Fixed { p, .. } if *p > 31);
                let x = int(0).unwrap_or(0);
                let n = int(1).unwrap_or(0).clamp(0, 64) as u32;
                Value::int(if wide {
                    let u = x as u64;
                    (match b {
                        Inot => !u,
                        Isll => u.checked_shl(n).unwrap_or(0),
                        _ => u.checked_shr(n).unwrap_or(0),
                    }) as i64
                } else {
                    let u = x as u32;
                    i64::from(
                        (match b {
                            Inot => !u,
                            Isll => u.checked_shl(n).unwrap_or(0),
                            _ => u.checked_shr(n).unwrap_or(0),
                        }) as i32,
                    )
                })
            }
            Raise2 => Value::int(1i64.checked_shl(int(0).unwrap_or(0) as u32).unwrap_or(0)),
            // SEARCHR / VERIFYR scan from position n (default the last) toward
            // the start and return the position found, 0 if none
            SearchR | VerifyR => {
                let (x, y) = (chars(&v[0]), chars(&v[1]));
                let start = int(2).unwrap_or(x.len() as i64);
                if x.is_empty() || start < 1 {
                    return Ok(Value::int(0));
                }
                let end = (start as usize).min(x.len());
                Value::int(
                    x[..end]
                        .iter()
                        .rposition(|c| y.contains(c) == (b == SearchR))
                        .map_or(0, |p| p as i64 + 1),
                )
            }
            CharVal => Value::Char(vec![int(0).unwrap_or(0).clamp(0, 255) as u8]),
            DaysToSecs => Value::Float(int(0).unwrap_or(0) as f64 * 86_400.0),
            SecsToDays => Value::int(v[0].as_f64().div_euclid(86_400.0) as i64),
            ValidDate => {
                let text = zeus_cobol::ebcdic::decode(&chars(&v[0]));
                let pattern = v
                    .get(1)
                    .map(|p| zeus_cobol::ebcdic::decode(&chars(p)))
                    .unwrap_or_else(|| "YYYYMMDD".into());
                Value::Bit(vec![valid_date(&text, &pattern)])
            }
            // the hexadecimal floating-point model: radix 16, fraction digits h
            // (6 short, 14 long, 28 extended), exponents -64..63
            Huge | Tiny | Epsilon | MaxExp | MinExp | Radix => {
                let ty = args[0].ty.arith().unwrap_or_else(|| args[0].ty.clone());
                let h = match &ty {
                    Ty::Float { dec, p } => hex_digits(*dec, *p),
                    _ => 28,
                };
                match b {
                    Huge => Value::Float((1.0 - 16f64.powi(-h)) * 16f64.powi(63)),
                    Tiny => Value::Float(16f64.powi(-65)),
                    Epsilon => Value::Float(16f64.powi(1 - h)),
                    MaxExp => Value::int(63),
                    MinExp => Value::int(-64),
                    _ => Value::int(16),
                }
            }
            PointerSubtract | OffsetAdd | OffsetSubtract => match (&v[0], int(1)) {
                (Value::Ptr(p), Some(n)) => Value::Ptr(if b == OffsetAdd {
                    (i64::from(*p) + n) as u32
                } else {
                    (i64::from(*p) - n) as u32
                }),
                _ => return err(format!("{b:?} arguments")),
            },
            OffsetValue => Value::Ptr(int(0).unwrap_or(0) as u32),
            OffsetDiff => match (&v[0], &v[1]) {
                (Value::Ptr(p), Value::Ptr(q)) => Value::int(i64::from(*p) - i64::from(*q)),
                _ => return err("OFFSETDIFF requires two offsets"),
            },
            // the position of a character in the collating sequence (its code)
            Rank => match &v[0] {
                Value::Char(c) if c.len() == 1 => Value::int(i64::from(c[0])),
                _ => return err("RANK requires a single character"),
            },
            // the hexadecimal model: x = f * 16**e with 1/16 <= |f| < 1
            Exponent | Scale => {
                let x = v[0].as_f64();
                if b == Scale {
                    Value::Float(x * 16f64.powi(int(1).unwrap_or(0) as i32))
                } else if x == 0.0 {
                    Value::int(0)
                } else {
                    let mut e = 0;
                    let mut a = x.abs();
                    while a >= 1.0 {
                        a /= 16.0;
                        e += 1;
                    }
                    while a < 1.0 / 16.0 {
                        a *= 16.0;
                        e -= 1;
                    }
                    Value::int(e)
                }
            }
            // seconds since 00:00:00 14 October 1582 (DAYSTOSECS(1) = 86400)
            SecsToDate => {
                let p = pattern(&v, 1, "YYYYMMDDHHMISS999");
                let us = (v[0].as_f64() * 1e6).round() as i64;
                Value::Char(ebcdic(&datefmt::format(
                    &datefmt::Stamp::from_micros(us),
                    &p,
                )))
            }
            // COMPARE AND SWAP: the word (CS) or doubleword (CDS) at p against
            // the one at q; equal: store x at p, 0; otherwise copy p to q, 1
            Cs | Cds => {
                let n = if b == Cs { 4 } else { 8 };
                let (Value::Ptr(p), Value::Ptr(q)) = (&v[0], &v[1]) else {
                    return err(format!("{b:?} requires two pointers"));
                };
                let (p, q) = (*p, *q);
                let old = self.read(p, n)?.to_vec();
                let cmp = self.read(q, n)?.to_vec();
                if old == cmp {
                    let x = v[2].as_i64();
                    let bytes = x.to_be_bytes();
                    self.write(p, &bytes[8 - n..])?;
                    Value::int(0)
                } else {
                    self.write(q, &old)?;
                    Value::int(1)
                }
            }
            Between | BetweenExclusive | BetweenLeftExclusive | BetweenRightExclusive => {
                let (x, a, c) = (&v[0], &v[1], &v[2]);
                let lo = self.compare(a, &args[1].ty, x, &args[0].ty)?;
                let hi = self.compare(x, &args[0].ty, c, &args[2].ty)?;
                use std::cmp::Ordering::*;
                let left_ok = match b {
                    BetweenExclusive | BetweenLeftExclusive => lo == Less,
                    _ => lo != Greater,
                };
                let right_ok = match b {
                    BetweenExclusive | BetweenRightExclusive => hi == Less,
                    _ => hi != Greater,
                };
                Value::bit1(left_ok && right_ok)
            }
            InList => {
                let mut hit = false;
                for i in 1..v.len() {
                    if self.compare(&v[0], &args[0].ty, &v[i], &args[i].ty)?
                        == std::cmp::Ordering::Equal
                    {
                        hit = true;
                        break;
                    }
                }
                Value::bit1(hit)
            }
            Iclz => {
                let w = bin_width(&args[0].ty);
                let x = (v[0].as_i64() as u64) & (u64::MAX >> (64 - w));
                Value::int(i64::from(x.leading_zeros()) - i64::from(64 - w))
            }
            Irll | Irrl | Lower2 | Popcnt => {
                let w = bin_width(&args[0].ty);
                let mask = u64::MAX >> (64 - w);
                let x = (v[0].as_i64() as u64) & mask;
                let n = int(1).unwrap_or(0).clamp(0, 64) as u32;
                let r = match b {
                    Irll => (x << (n % w) | x >> ((w - n % w) % w)) & mask,
                    Irrl => (x >> (n % w) | x << ((w - n % w) % w)) & mask,
                    // LOWER2 is the arithmetic shift right (SRA)
                    Lower2 => (v[0].as_i64() >> n.min(63)) as u64,
                    // POPCNT counts the one bits of each byte into that byte
                    _ => {
                        let mut out = 0u64;
                        for k in 0..w / 8 {
                            out |= u64::from(((x >> (8 * k)) & 0xff).count_ones()) << (8 * k);
                        }
                        out
                    }
                };
                // back to the signed value of the result width
                let unsigned = matches!(rt, Ty::Fixed { unsigned: true, .. });
                Value::int(if unsigned || w == 64 || b == Lower2 {
                    r as i64
                } else if r >> (w - 1) & 1 == 1 {
                    (r as i64) - (1i64 << w)
                } else {
                    r as i64
                })
            }
            ISigned | IUnsigned => {
                let w = bin_width(&args[0].ty);
                let x = (v[0].as_i64() as u64) & (u64::MAX >> (64 - w));
                Value::int(if b == IUnsigned || w == 64 || x >> (w - 1) & 1 == 0 {
                    x as i64
                } else {
                    (x as i64) - (1i64 << w)
                })
            }
            MaxVal | MinVal => match args[0].ty.arith() {
                Some(Ty::Fixed {
                    dec,
                    p,
                    q,
                    unsigned,
                }) => {
                    let m: i128 = if dec {
                        10i128.pow(u32::from(p)) - 1
                    } else {
                        (1i128 << p) - 1
                    };
                    let m = match (b, unsigned) {
                        (MaxVal, _) => m,
                        (MinVal, true) => 0,
                        (MinVal, false) if dec => -m,
                        _ => -m - 1,
                    };
                    Value::fixed(m, q, dec)
                }
                Some(Ty::Float { dec, p }) => {
                    let h = hex_digits(dec, p);
                    let huge = (1.0 - 16f64.powi(-h)) * 16f64.powi(63);
                    Value::Float(if b == MaxVal { huge } else { -huge })
                }
                _ => return err(format!("{b:?} requires a REAL arithmetic argument")),
            },
            Pred | Succ => {
                let h = match rt {
                    Ty::Float { dec, p } => hex_digits(*dec, *p),
                    _ => 6,
                };
                Value::Float(hfp_step(v[0].as_f64(), h, b == Succ))
            }
            Places => match args[0].ty.arith() {
                Some(Ty::Float { dec, p }) => Value::int(i64::from(hex_digits(dec, p))),
                _ => return err("PLACES requires a REAL FLOAT argument"),
            },
            PrecVal | ScaleVal => match args[0].ty.arith() {
                Some(Ty::Fixed { p, q, .. }) => Value::int(if b == PrecVal {
                    i64::from(p)
                } else {
                    i64::from(q)
                }),
                Some(Ty::Float { p, .. }) => {
                    Value::int(if b == PrecVal { i64::from(p) } else { 0 })
                }
                _ => return err(format!("{b:?} requires a numeric argument")),
            },
            // date/time built-ins on the Lilian form (Language Reference,
            // "Date/time built-in functions"; patterns of Table 65)
            Microsecs | Secs => {
                let default = if b == Microsecs {
                    "YYYY-MM-DD-HH.MI.SS.999999"
                } else {
                    "YYYYMMDDHHMISS999"
                };
                let us = match v.first() {
                    Some(d) => {
                        let text = zeus_cobol::ebcdic::decode(&chars(d));
                        let p = pattern(&v, 1, default);
                        match datefmt::parse(&text, &p, window(int(2))) {
                            Some(t) => t.micros(),
                            None => {
                                self.raise(Cond::Error, 2512, act)?;
                                0
                            }
                        }
                    }
                    None => now_stamp().micros(),
                };
                if b == Microsecs {
                    Value::int(us)
                } else {
                    Value::Float(us as f64 / 1e6)
                }
            }
            MicrosecsToDate => {
                let p = pattern(&v, 1, "YYYY-MM-DD-HH.MI.SS.999999");
                let t = datefmt::Stamp::from_micros(int(0).unwrap_or(0));
                Value::Char(ebcdic(&datefmt::format(&t, &p)))
            }
            DaysToMicrosecs => Value::int(int(0).unwrap_or(0) * 86_400_000_000),
            MicrosecsToDays => Value::int(int(0).unwrap_or(0).div_euclid(86_400_000_000)),
            Timestamp => Value::Char(ebcdic(&datefmt::format(
                &now_stamp(),
                "YYYY-MM-DD-HH.MI.SS.999999",
            ))),
            // Zeus keeps the system clock in UTC, so local time and UTC agree
            UtcDatetime => Value::Char(ebcdic(&datefmt::format(&now_stamp(), "YYYYMMDDHHMISS999"))),
            UtcSecs => Value::Float(now_stamp().micros() as f64 / 1e6),
            UtcMicrosecs => Value::int(now_stamp().micros()),
            // the latest and the earliest value of the pattern (NONULLDATE:
            // the earliest date is 14 October 1582, Lilian day 0)
            MaxDate | MinDate => {
                let p = pattern(&v, 0, "YYYY-MM-DD-HH.MI.SS.999999");
                let t = if b == MaxDate {
                    datefmt::Stamp {
                        y: 9999,
                        m: 12,
                        d: 31,
                        h: 23,
                        mi: 59,
                        s: 59,
                        micro: 999_999,
                    }
                } else {
                    datefmt::Stamp::from_micros(0)
                };
                Value::Char(ebcdic(&datefmt::format(&t, &p)))
            }
            // a STCK value counts 2**-12 microseconds since 1 January 1900;
            // a STCKE value has an epoch byte and then the same bits
            StckToDate | StckeToDate => {
                let tod = if b == StckToDate {
                    match &v[0] {
                        Value::Fixed { m, q: 0, .. } => *m as u64,
                        x => x.as_i64() as u64,
                    }
                } else {
                    let c = chars(&v[0]);
                    let mut w = [0u8; 8];
                    for (i, x) in c.iter().skip(1).take(8).enumerate() {
                        w[i] = *x;
                    }
                    u64::from_be_bytes(w)
                };
                let p = pattern(&v, 1, "YYYY-MM-DD-HH.MI.SS.999999");
                let us = (tod >> 12) as i64 + datefmt::lilian_day(1900, 1, 1) * 86_400_000_000;
                Value::Char(ebcdic(&datefmt::format(
                    &datefmt::Stamp::from_micros(us),
                    &p,
                )))
            }
            // the SMF date X'0cyydddF': c is the century (0 = 19xx, 1 = 20xx)
            JulianToSmf => {
                let d = zeus_cobol::ebcdic::decode(&chars(&v[0]));
                let y: u32 = d.get(0..4).and_then(|x| x.parse().ok()).unwrap_or(1900);
                let ddd: u32 = d.get(4..7).and_then(|x| x.parse().ok()).unwrap_or(1);
                let c = y.saturating_sub(1900) / 100;
                let digits = format!("0{c}{:02}{ddd:03}F", y % 100);
                Value::Char(hex_decode(digits.as_bytes()).unwrap_or_default())
            }
            SmfToJulian => {
                let c = chars(&v[0]);
                let h: std::string::String = c.iter().map(|b| format!("{b:02X}")).collect();
                let cent: u32 = h.get(1..2).and_then(|x| x.parse().ok()).unwrap_or(0);
                let yy = h.get(2..4).unwrap_or("00");
                let ddd = h.get(4..7).unwrap_or("001");
                Value::Char(ebcdic(&format!("{}{yy}{ddd}", 19 + cent)))
            }
            // WIDECHAR strings of n highest (FFFF) or lowest (0000) characters
            WHigh | WLow => {
                let n = int(0).unwrap_or(0).max(0) as usize;
                Value::Char(vec![if b == WHigh { 0xFF } else { 0 }; 2 * n])
            }
            // SUBTO(x,y,z) is SUBSTR(x,y,z-y+1)
            SubTo => {
                let s = int(1).unwrap_or(1);
                let n = int(2).map(|z| z - s + 1);
                match &v[0] {
                    Value::Bit(x) => Value::Bit(self.substr(x, s, n, act)?),
                    x => {
                        let c = chars(x);
                        Value::Char(self.substr(&c, s, n, act)?)
                    }
                }
            }
            // PUTENV values are kept by the run unit and read first by GETENV
            // COUNT: the data items of the last GET or PUT on the file
            Count => match &v[0] {
                Value::File(f) => {
                    let st = &self.files[*f as usize];
                    Value::int(if st.open { st.count as i64 } else { 0 })
                }
                _ => return err("COUNT requires a file"),
            },
            GetEnv => {
                let name = zeus_cobol::ebcdic::decode(&chars(&v[0]));
                let name = name.trim();
                let val = ENV
                    .with(|e| e.borrow().get(name).cloned())
                    .or_else(|| std::env::var(name).ok())
                    .unwrap_or_default();
                Value::Char(ebcdic(&val))
            }
            PutEnv => {
                let text = zeus_cobol::ebcdic::decode(&chars(&v[0]));
                match text.split_once('=') {
                    Some((k, val)) if !k.trim().is_empty() => {
                        ENV.with(|e| e.borrow_mut().insert(k.trim().to_string(), val.to_string()));
                        Value::bit1(true)
                    }
                    _ => Value::bit1(false),
                }
            }
            // a version 4 UUID: random bits, version nibble 4, variant 10
            Uuid4 => {
                let mut r = [0u8; 16];
                for chunk in r.chunks_mut(8) {
                    self.random ^= self.random << 13;
                    self.random ^= self.random >> 7;
                    self.random ^= self.random << 17;
                    chunk.copy_from_slice(&self.random.to_be_bytes()[..chunk.len()]);
                }
                r[6] = (r[6] & 0x0f) | 0x40;
                r[8] = (r[8] & 0x3f) | 0x80;
                let h: std::string::String = r.iter().map(|c| format!("{c:02x}")).collect();
                Value::Char(ebcdic(&format!(
                    "{}-{}-{}-{}-{}",
                    &h[0..8],
                    &h[8..12],
                    &h[12..16],
                    &h[16..20],
                    &h[20..32]
                )))
            }
            // buffer functions on UTF-16: whitespace \t \f \v \n \r, base 64
            // digits as UTF-16 characters
            WsCollapse16 | WsReplace16 | Base64Encode16 | Base64Decode16 => {
                let (Value::Ptr(p), Value::Ptr(q)) = (&v[0], &v[2]) else {
                    return err(format!("{b:?} requires locators"));
                };
                let (p, q) = (*p, *q);
                let m = int(1).unwrap_or(0).max(0) as usize;
                let n = int(3).unwrap_or(0).max(0) as usize;
                let src = self.read(q, n)?.to_vec();
                let units: Vec<u16> = src
                    .chunks(2)
                    .map(|c| u16::from_be_bytes([c[0], *c.get(1).unwrap_or(&0)]))
                    .collect();
                let utf16 = |u: &[u16]| u.iter().flat_map(|x| x.to_be_bytes()).collect::<Vec<u8>>();
                let out: Option<Vec<u8>> = match b {
                    WsCollapse16 | WsReplace16 => {
                        let mut u: Vec<u16> = units
                            .iter()
                            .map(|c| if matches!(c, 9..=13) { 0x20 } else { *c })
                            .collect();
                        if b == WsCollapse16 {
                            let mut c = Vec::new();
                            for x in u {
                                if !(x == 0x20 && (c.is_empty() || c.last() == Some(&0x20))) {
                                    c.push(x);
                                }
                            }
                            while c.last() == Some(&0x20) {
                                c.pop();
                            }
                            u = c;
                        }
                        Some(utf16(&u))
                    }
                    Base64Encode16 => {
                        let e = base64_encode(&src);
                        Some(utf16(&e.iter().map(|c| u16::from(*c)).collect::<Vec<_>>()))
                    }
                    _ => {
                        let ascii: Option<Vec<u8>> =
                            units.iter().map(|u| u8::try_from(*u).ok()).collect();
                        ascii.and_then(|a| base64_decode(&a))
                    }
                };
                let Some(out) = out else {
                    return Ok(Value::int(-1));
                };
                if p == 0 {
                    Value::int(out.len() as i64)
                } else if out.len() > m {
                    Value::int(-1)
                } else {
                    self.write(p, &out)?;
                    Value::int(out.len() as i64)
                }
            }
            Y4Date | Y4Julian | Y4Year => {
                let d = zeus_cobol::ebcdic::decode(&chars(&v[0]));
                let w = int(1).unwrap_or(1950);
                Value::Char(ebcdic(&widen_year(d.trim_start(), w)))
            }
            ReplaceBy2 => {
                let (x, y, z) = (chars(&v[0]), chars(&v[1]), chars(&v[2]));
                if y.len() != 2 * z.len() {
                    return err(
                        "REPLACEBY2: the second argument must be twice as long as the third",
                    );
                }
                let mut out = Vec::new();
                for c in x {
                    match z.iter().position(|k| *k == c) {
                        Some(i) => out.extend_from_slice(&y[2 * i..2 * i + 2]),
                        None => out.push(c),
                    }
                }
                Value::Char(out)
            }
            HexImage => {
                let Value::Ptr(p) = &v[0] else {
                    return err("HEXIMAGE requires a locator");
                };
                let n = int(1).unwrap_or(0).max(0) as usize;
                let bytes = self.read(*p, n)?.to_vec();
                let sep = v.get(2).map(chars).and_then(|c| c.first().copied());
                let mut out = Vec::new();
                for (i, byte) in bytes.iter().enumerate() {
                    if i > 0
                        && i % 4 == 0
                        && let Some(s) = sep
                    {
                        out.push(s);
                    }
                    out.extend(ebcdic(&format!("{byte:02X}")));
                }
                Value::Char(out)
            }
            Compare => {
                let (Value::Ptr(x), Value::Ptr(y)) = (&v[0], &v[1]) else {
                    return err("COMPARE requires two locators");
                };
                let n = int(2).unwrap_or(0).max(0) as usize;
                let a = self.read(*x, n)?.to_vec();
                let c = self.read(*y, n)?.to_vec();
                Value::int(match a.cmp(&c) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                })
            }
            Collapse | Squeeze => {
                let (x, y) = (chars(&v[0]), chars(&v[1]));
                let n = int(2).unwrap_or(1).max(1) as usize;
                Value::Char(squeeze(
                    &x,
                    y.first().copied().unwrap_or(b' '),
                    n,
                    b == Collapse,
                ))
            }
            IndexR => {
                let (x, y) = (chars(&v[0]), chars(&v[1]));
                let n = int(2).unwrap_or(x.len() as i64);
                if n < 0 || n as usize > x.len() {
                    if self.enabled_now(act, cond::STRINGRANGE) {
                        self.raise(Cond::StringRange, 350, act)?;
                    }
                    return Ok(Value::int(0));
                }
                // the rightmost match starting at or before position n
                let found = if y.is_empty() || y.len() > x.len() {
                    None
                } else {
                    (0..=(x.len() - y.len()).min((n.max(1) - 1) as usize))
                        .rev()
                        .find(|&i| x[i..i + y.len()] == y[..])
                };
                Value::int(found.map_or(0, |i| i as i64 + 1))
            }
            Replace => {
                let (x, f, t) = (chars(&v[0]), chars(&v[1]), chars(&v[2]));
                let n = int(3).unwrap_or(1);
                // at most i replacements; i = 0 replaces all, the default is 1
                let limit = int(4).unwrap_or(1);
                if n < 1 || n as usize > x.len() {
                    if self.enabled_now(act, cond::STRINGRANGE) {
                        self.raise(Cond::StringRange, 350, act)?;
                    }
                    return Ok(Value::Char(Vec::new()));
                }
                let mut out = x[..(n - 1) as usize].to_vec();
                let mut i = (n - 1) as usize;
                let mut done = 0;
                while i < x.len() {
                    if !f.is_empty() && (limit == 0 || done < limit) && x[i..].starts_with(&f) {
                        out.extend_from_slice(&t);
                        i += f.len();
                        done += 1;
                    } else {
                        out.push(x[i]);
                        i += 1;
                    }
                }
                Value::Char(out)
            }
            Scrubout => {
                let (x, f) = (chars(&v[0]), chars(&v[1]));
                let n = (int(2).unwrap_or(1).max(1) as usize - 1).min(x.len());
                let mut out = x[..n].to_vec();
                out.extend(x.iter().skip(n).filter(|c| !f.contains(c)));
                Value::Char(out)
            }
            Unhex => {
                let mut x = chars(&v[0]);
                if let Some(c) = v.get(1).map(chars).and_then(|c| c.first().copied()) {
                    x.retain(|b| *b != c);
                }
                let text = zeus_cobol::ebcdic::decode(&x);
                match hex_decode(text.as_bytes()) {
                    Some(b) => Value::Char(b),
                    None => {
                        self.raise(Cond::Conversion, 600, act)?;
                        Value::Char(Vec::new())
                    }
                }
            }
            // HEX8 / HEXIMAGE8: the HEX / HEXIMAGE digits as UTF-8 (ASCII)
            Hex8 | HexImage8 => {
                let base = if b == Hex8 { Hex } else { HexImage };
                let h = self.builtin(base, args, &Ty::char_var(65_535), act)?;
                let Value::Char(c) = h else {
                    return err(format!("{b:?}: no hexadecimal image"));
                };
                Value::Char(zeus_cobol::ebcdic::decode(&c).into_bytes())
            }
            MemIndex | MemSearch | MemSearchR | MemVerify | MemVerifyR => {
                let Value::Ptr(p) = &v[0] else {
                    return err(format!("{b:?} requires a locator"));
                };
                let n = int(1).unwrap_or(0).max(0) as usize;
                let buf = self.read(*p, n)?.to_vec();
                let target = match (b, v.get(3)) {
                    (MemIndex, Some(m)) => {
                        let Value::Ptr(q) = &v[2] else {
                            return err("MEMINDEX requires a locator");
                        };
                        self.read(*q, m.as_i64().max(0) as usize)?.to_vec()
                    }
                    _ => chars(&v[2]),
                };
                let pos = if n == 0 || target.is_empty() {
                    None
                } else {
                    match b {
                        MemIndex => buf
                            .windows(target.len())
                            .position(|w| w == target.as_slice()),
                        MemSearch => buf.iter().position(|c| target.contains(c)),
                        MemSearchR => buf.iter().rposition(|c| target.contains(c)),
                        MemVerify => buf.iter().position(|c| !target.contains(c)),
                        _ => buf.iter().rposition(|c| !target.contains(c)),
                    }
                };
                Value::int(pos.map_or(0, |i| i as i64 + 1))
            }
            MemCollapse | MemSqueeze | MemReplace | WsCollapse | WsReplace | HexEncode
            | HexDecode | HexEncode8 | HexDecode8 | Base64Encode | Base64Decode | Base64Encode8
            | Base64Decode8 => {
                let (Value::Ptr(p), Value::Ptr(q)) = (&v[0], &v[2]) else {
                    return err(format!("{b:?} requires locators"));
                };
                let (p, q) = (*p, *q);
                let m = int(1).unwrap_or(0).max(0) as usize;
                let n = int(3).unwrap_or(0).max(0) as usize;
                let src = self.read(q, n)?.to_vec();
                // the forms without 8 use the program's (EBCDIC) characters
                let enc = |s: &[u8]| ebcdic(std::str::from_utf8(s).unwrap_or(""));
                let dec = |s: &[u8]| zeus_cobol::ebcdic::decode(s).into_bytes();
                let out: Option<Vec<u8>> = match b {
                    MemCollapse | MemSqueeze => {
                        let z = chars(&v[4]).first().copied().unwrap_or(b' ');
                        let i = int(5).unwrap_or(1).max(1) as usize;
                        Some(squeeze(&src, z, i, b == MemCollapse))
                    }
                    MemReplace => {
                        let (Value::Ptr(f), Value::Ptr(t)) = (&v[4], &v[6]) else {
                            return err("MEMREPLACE requires locators");
                        };
                        let x = int(5).unwrap_or(0).max(0) as usize;
                        let y = int(7).unwrap_or(0).max(0) as usize;
                        let fb = self.read(*f, x)?.to_vec();
                        let tb = self.read(*t, y)?.to_vec();
                        let s = int(8).unwrap_or(1);
                        let limit = int(9).unwrap_or(1);
                        if s < 1 || s as usize > n + 1 {
                            Some(Vec::new())
                        } else {
                            let mut out = src[..(s - 1) as usize].to_vec();
                            let mut i = (s - 1) as usize;
                            let mut done = 0;
                            while i < src.len() {
                                if !fb.is_empty()
                                    && (limit == 0 || done < limit)
                                    && src[i..].starts_with(&fb)
                                {
                                    out.extend_from_slice(&tb);
                                    i += fb.len();
                                    done += 1;
                                } else {
                                    out.push(src[i]);
                                    i += 1;
                                }
                            }
                            Some(out)
                        }
                    }
                    WsCollapse | WsReplace => {
                        // \t \f \v \n \r in EBCDIC: 05 0C 0B 15 (or 25) 0D
                        let ws = [0x05u8, 0x0c, 0x0b, 0x15, 0x25, 0x0d];
                        let blank = ebcdic(" ")[0];
                        let mut s: Vec<u8> = src
                            .iter()
                            .map(|c| if ws.contains(c) { blank } else { *c })
                            .collect();
                        if b == WsCollapse {
                            s = squeeze(&s, blank, 1, true);
                        }
                        Some(s)
                    }
                    HexEncode | HexEncode8 => {
                        let h: std::string::String =
                            src.iter().map(|c| format!("{c:02X}")).collect();
                        Some(if b == HexEncode {
                            enc(h.as_bytes())
                        } else {
                            h.into_bytes()
                        })
                    }
                    HexDecode => hex_decode(&dec(&src)),
                    HexDecode8 => hex_decode(&src),
                    Base64Encode => Some(enc(&base64_encode(&src))),
                    Base64Encode8 => Some(base64_encode(&src)),
                    Base64Decode => base64_decode(&dec(&src)),
                    _ => base64_decode(&src),
                };
                let Some(out) = out else {
                    return Ok(Value::int(-1));
                };
                // a zero target only measures; a short target gives -1
                if p == 0 {
                    Value::int(out.len() as i64)
                } else if out.len() > m {
                    Value::int(-1)
                } else {
                    self.write(p, &out)?;
                    Value::int(out.len() as i64)
                }
            }
            WhereDiff => {
                let (Value::Ptr(x), Value::Ptr(y)) = (&v[0], &v[1]) else {
                    return err("WHEREDIFF requires two locators");
                };
                let n = int(2).unwrap_or(0).max(0) as usize;
                let a = self.read(*x, n)?.to_vec();
                let c = self.read(*y, n)?.to_vec();
                Value::int(
                    a.iter()
                        .zip(&c)
                        .position(|(l, r)| l != r)
                        .map_or(0, |i| i as i64 + 1),
                )
            }
            Erf => Value::Float(erf(v[0].as_f64())),
            Erfc => Value::Float(erfc(v[0].as_f64())),
            Gamma | LogGamma => {
                let x = v[0].as_f64();
                if x <= 0.0 {
                    return err(format!("{b:?} requires an argument greater than zero"));
                }
                let lg = ln_gamma(x);
                Value::Float(if b == Gamma { lg.exp() } else { lg })
            }
            PointerDiff => match (&v[0], &v[1]) {
                (Value::Ptr(p), Value::Ptr(q)) => Value::int(i64::from(*p) - i64::from(*q)),
                _ => return err("POINTERDIFF requires two pointers"),
            },
            Weekday => {
                let n = match v.first() {
                    Some(x) => x.as_i64(),
                    None => {
                        let (y, m, d, ..) = now_parts();
                        lilian(y, m, d)
                    }
                };
                // Lilian day 1 (15 Oct 1582) was a Friday
                Value::int((n + 4).rem_euclid(7) + 1)
            }
            PliRetV => Value::int(i64::from(self.rc)),
            LineNo | PageNo => {
                let file = if let Some(Value::File(file)) = v.first() {
                    *file
                } else {
                    self.file_id(&None, "SYSPRINT", act)?
                };
                let state = &self.files[file as usize];
                Value::int(if b == LineNo {
                    state.lineno
                } else {
                    state.pageno
                } as i64)
            }
            Null => Value::Ptr(value::NULL),
            // ORDINAL members: the table the compiler gives (value, name)
            OrdinalName | OrdinalSucc | OrdinalPred => {
                let x = v[0].as_i64();
                let Value::Char(t) = &v[1] else {
                    return Err(Intr::Fatal("ordinal table".into()));
                };
                let mut members = Vec::new();
                let mut at = 0;
                while at + 9 <= t.len() {
                    let val = i64::from_be_bytes(t[at..at + 8].try_into().unwrap());
                    let n = t[at + 8] as usize;
                    let name =
                        std::string::String::from_utf8_lossy(&t[at + 9..(at + 9 + n).min(t.len())])
                            .into_owned();
                    members.push((val, name));
                    at += 9 + n;
                }
                let k = members.iter().position(|m| m.0 == x);
                match b {
                    OrdinalName => Value::Char(zeus_cobol::ebcdic::encode(
                        &k.map(|k| members[k].1.clone()).unwrap_or_default(),
                    )),
                    OrdinalSucc => {
                        Value::int(k.and_then(|k| members.get(k + 1)).map_or(x + 1, |m| m.0))
                    }
                    _ => Value::int(
                        k.and_then(|k| k.checked_sub(1))
                            .map_or(x - 1, |k| members[k].0),
                    ),
                }
            }
            // an empty area
            Empty => Value::Char(vec![0u8; crate::types::AREA_HEADER as usize]),
            SysNull => Value::Ptr(0),
            PointerAdd => match (&v[0], int(1)) {
                (Value::Ptr(p), Some(n)) => Value::Ptr((i64::from(*p) + n) as u32),
                _ => return err("POINTERADD arguments"),
            },
            PointerValue => Value::Ptr(int(0).unwrap_or(0) as u32),
            BinaryValue => match &v[0] {
                Value::Ptr(p) => Value::int(i64::from(*p)),
                x => Value::int(x.as_i64()),
            },
            other => return err(format!("built-in {other:?} is not supported")),
        })
    }

    /// SUBSTR with STRINGRANGE checking.
    fn substr<T: Clone>(&mut self, x: &[T], s: i64, n: Option<i64>, act: usize) -> R<Vec<T>> {
        let k = x.len() as i64;
        let bad = match n {
            Some(n) => s < 1 || n < 0 || s - 1 + n > k,
            None => s < 1 || s > k + 1,
        };
        if bad {
            if self.enabled_now(act, cond::STRINGRANGE) {
                self.raise(Cond::StringRange, 350, act)?;
            }
            if s > k {
                return Ok(Vec::new());
            }
            let m = s.max(1);
            let len = match n {
                Some(j) => (j + s.min(1) - 1).min(k - m + 1).max(0),
                None => k - m + 1,
            };
            return Ok(x[(m - 1) as usize..(m - 1 + len) as usize].to_vec());
        }
        let n = n.unwrap_or(k - s + 1);
        Ok(x[(s - 1) as usize..(s - 1 + n) as usize].to_vec())
    }

    pub fn ref_builtin(&mut self, b: Bif, r: &Ref, args: &[Expr], rt: &Ty, act: usize) -> R<Value> {
        use Bif::*;
        let dimno = match args.first() {
            Some(e) => self.eval(e, act)?.as_i64(),
            None => 1,
        };
        Ok(match b {
            // POLY(x,y): x(m) + x(m+1)*y + ... + x(n)*y**(n-m)
            Poly => {
                let y = args.first().map(|e| self.eval(e, act)).transpose()?;
                let y = y.map_or(0.0, |v| v.as_f64());
                let mut sum = 0.0;
                let mut pow = 1.0;
                for l in self.elements(r, act)? {
                    sum += self.load(&l)?.as_f64() * pow;
                    pow *= y;
                }
                Value::Float(sum)
            }
            // VALID: a PICTURE image valid for its specification, a FIXED
            // DECIMAL item holding valid packed decimal data
            Valid => {
                let locs = self.elements(r, act)?;
                let mut ok = true;
                for l in locs {
                    match &l.ty {
                        Ty::Pic(p) => {
                            let Value::Char(img) = self.load(&l)? else {
                                return err("VALID: picture value");
                            };
                            let text = super::conv::ebcdic_text(&img);
                            ok &= if p.float.is_some() {
                                p.value_float(&text).is_some()
                            } else if p.numeric {
                                p.value(&text).is_some()
                            } else {
                                p.valid_char(&text)
                            };
                        }
                        Ty::Fixed { dec: true, p, .. } => {
                            let n = usize::from(*p) / 2 + 1;
                            let bytes = self.read((l.bit / 8) as u32, n)?.to_vec();
                            ok &= packed_valid(&bytes);
                        }
                        _ => return err("VALID requires a PICTURE or FIXED DECIMAL reference"),
                    }
                }
                Value::bit1(ok)
            }
            // the store-clock instructions: set the reference and return the
            // condition code 0 (clock in the set state)
            PliStck | PliStckE | PliStckF | PliStckLocal | PliStckUtc | PliStckELocal
            | PliStckEUtc | PliStckP | PliStckPLocal | PliStckPUtc => {
                let us = now_stamp().micros() - datefmt::lilian_day(1900, 1, 1) * 86_400_000_000;
                let tod = (us as u64) << 12;
                let value = if matches!(b, PliStckE | PliStckELocal | PliStckEUtc) {
                    let mut c = vec![0u8; 16];
                    c[1..9].copy_from_slice(&tod.to_be_bytes());
                    Value::Char(c)
                } else {
                    Value::fixed(i128::from(tod), 0, false)
                };
                let locs = self.elements(r, act)?;
                for l in locs {
                    let v = self.convert(value.clone(), &arg_ty(b), &l.ty, act)?;
                    self.store(&l, &v)?;
                }
                Value::int(0)
            }
            New => {
                let size = self.var_size(r.var, act, None)?;
                Value::Ptr(self.heap_alloc(size as u32))
            }
            // ADDRDATA: the address of the data (Zeus passes no descriptors, so
            // it is ADDR)
            Addr | AddrData => {
                let (bit, _) = self.element_or_start(r, act)?;
                Value::Ptr((bit / 8) as u32)
            }
            // the byte location of a member in its level-1 structure
            Location => Value::int((self.module.vars[r.var as usize].bit_offset / 8) as i64),
            // the bit offset of the reference within its byte
            BitLocation => {
                let (bit, _) = self.element_or_start(r, act)?;
                Value::int((bit % 8) as i64)
            }
            PointerOf | OffsetOf => {
                let base = self.area_base(r.var, act)?;
                let Value::Ptr(x) = self.eval(&args[0], act)? else {
                    return Err(Intr::Fatal(format!("{b:?}: not a locator")));
                };
                // offset 0 is the first allocation; pointer 0 is SYSNULL
                Value::Ptr(if x == value::NULL || (b == OffsetOf && x == 0) {
                    value::NULL
                } else if b == PointerOf {
                    base.wrapping_add(x)
                } else {
                    x.wrapping_sub(base)
                })
            }
            Dim | Lbound | Hbound => {
                let (_, dims) = self.base_of(r, act)?;
                let free: Vec<(i64, i64, u64)> = if r.subs.iter().all(|s| s.is_none()) {
                    dims
                } else {
                    r.subs
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| s.is_none())
                        .map(|(k, _)| dims[k])
                        .collect()
                };
                let (lb, ub, _) = free
                    .get((dimno - 1).max(0) as usize)
                    .copied()
                    .unwrap_or((1, 0, 0));
                Value::int(match b {
                    Dim => ub - lb + 1,
                    Lbound => lb,
                    _ => ub,
                })
            }
            Size | CurrentSize => {
                let bytes = self.aggregate_bytes(r, act)?;
                let mut n = bytes.len() as i64;
                if b == CurrentSize
                    && let Some(Ty::Char {
                        var: crate::types::Var::Varying,
                        ..
                    }) = &self.module.vars[r.var as usize].ty
                    && r.subs.iter().all(|s| s.is_some())
                {
                    n = 2 + i64::from(u16::from_be_bytes([bytes[0], bytes[1]]));
                }
                Value::int(n)
            }
            Allocation => Value::int(self.allocation(r.var, act)?),
            // a parameter whose argument was written as * has no binding
            // (Language Reference, PRESENT and OMITTED)
            Present | Omitted => {
                let var = &self.module.vars[r.var as usize];
                let root = &self.module.vars[var.root as usize];
                let present = match &root.storage {
                    crate::ir::Storage::Param(k) => {
                        let k = *k as usize;
                        let a = self.act_of(act, root.block)?;
                        self.acts[a].params.get(k).is_some_and(|b| b.is_some())
                    }
                    _ => true,
                };
                Value::bit1(present == (b == Present))
            }
            String => {
                let locs = self.elements(r, act)?;
                let mut out: Vec<u8> = Vec::new();
                let mut bits: Vec<bool> = Vec::new();
                let bit = rt.is_bit();
                for l in locs {
                    match self.load(&l)? {
                        Value::Char(c) => out.extend(c),
                        Value::Bit(x) => bits.extend(x),
                        other => {
                            let t = l.ty.clone();
                            let c = self.convert(other, &t, &crate::sema::expr::char_any(), act)?;
                            if let Value::Char(c) = c {
                                out.extend(c);
                            }
                        }
                    }
                }
                if bit {
                    Value::Bit(bits)
                } else {
                    Value::Char(out)
                }
            }
            Unspec => {
                let (bit, _) = self.element_or_start(r, act)?;
                let n = self.aggregate_bytes(r, act)?.len() as u64 * 8;
                let n = match &self.module.vars[r.var as usize].ty {
                    Some(Ty::Bit {
                        len: Ext::Const(k),
                        var: crate::types::Var::Fixed,
                        aligned: false,
                    }) => u64::from(*k),
                    _ => n,
                };
                Value::Bit(self.read_bits(bit, n)?)
            }
            Sum | Prod => {
                let locs = self.elements(r, act)?;
                let mut acc: Option<Value> = None;
                let ety = self.module.vars[r.var as usize]
                    .ty
                    .clone()
                    .unwrap_or(Ty::fixed_dec(15, 0));
                for l in locs {
                    let x = self.load(&l)?;
                    let x = self.convert(x, &l.ty, rt, act)?;
                    acc = Some(match acc {
                        None => x,
                        Some(a) => {
                            let op = if b == Sum { ArOp::Add } else { ArOp::Mul };
                            self.arith(op, a, rt, x, rt, rt, act)?
                        }
                    });
                }
                let _ = ety;
                acc.unwrap_or(Value::fixed(if b == Sum { 0 } else { 1 }, 0, true))
            }
            All | Any => {
                let locs = self.elements(r, act)?;
                let mut acc: Option<Vec<bool>> = None;
                for l in locs {
                    let x = self.load(&l)?;
                    let Value::Bit(x) =
                        self.convert(x, &l.ty, &crate::sema::expr::bit_any(), act)?
                    else {
                        continue;
                    };
                    acc = Some(match acc {
                        None => x,
                        Some(a) => {
                            let n = a.len().max(x.len());
                            (0..n)
                                .map(|i| {
                                    let p = a.get(i).copied().unwrap_or(false);
                                    let q = x.get(i).copied().unwrap_or(false);
                                    if b == All { p && q } else { p || q }
                                })
                                .collect()
                        }
                    });
                }
                Value::Bit(acc.unwrap_or_default())
            }
            other => {
                return err(format!(
                    "built-in {other:?} on a reference is not supported"
                ));
            }
        })
    }
}

thread_local! {
    /// Environment variables set by PUTENV.
    static ENV: std::cell::RefCell<std::collections::HashMap<std::string::String, std::string::String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Valid packed decimal data: digits 0-9 and a sign nibble A-F.
fn packed_valid(b: &[u8]) -> bool {
    let Some((last, rest)) = b.split_last() else {
        return false;
    };
    rest.iter().all(|x| x >> 4 <= 9 && x & 15 <= 9) && last >> 4 <= 9 && last & 15 >= 10
}

/// The pattern argument `i`, or the default pattern.
fn pattern(v: &[Value], i: usize, default: &str) -> String {
    v.get(i)
        .map(|p| zeus_cobol::ebcdic::decode(&chars(p)).trim_end().to_string())
        .unwrap_or_else(|| default.to_string())
}

/// The century window: a year such as 1950, or (zero or negative) an offset
/// from the current year; WINDOW(1950) when omitted.
fn window(w: Option<i64>) -> i64 {
    match w {
        Some(w) if w > 0 => w,
        Some(w) => now_stamp().y + w,
        None => 1950,
    }
}

/// The current date and time (UTC; see now_parts).
fn now_stamp() -> datefmt::Stamp {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let us = d.as_micros() as i64 + datefmt::LILIAN_1970 * 86_400_000_000;
    datefmt::Stamp::from_micros(us)
}

/// The attributes of the value a PLISTCK built-in stores.
fn arg_ty(b: Bif) -> Ty {
    match b {
        Bif::PliStckE | Bif::PliStckELocal | Bif::PliStckEUtc => Ty::char(16),
        _ => Ty::Fixed {
            dec: false,
            p: 64,
            q: 0,
            unsigned: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(lilian(1582, 10, 15), 1);
        assert_eq!(lilian(1988, 5, 16), 148_138);
        assert_eq!(civil(148_138 - 141_428), (1988, 5, 16));
    }
}

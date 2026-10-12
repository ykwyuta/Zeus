//! Built-in functions.

use super::conv::cmp_num;
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

fn ebcdic(s: &str) -> Vec<u8> {
    zeus_cobol::ebcdic::encode(s)
}

impl Machine {
    pub fn builtin(&mut self, b: Bif, args: &[Expr], rt: &Ty, act: usize) -> R<Value> {
        use Bif::*;
        let mut v = Vec::new();
        for a in args {
            v.push(self.eval(a, act)?);
        }
        let int = |i: usize| v.get(i).map(|x| x.as_i64());
        Ok(match b {
            Abs => match &v[0] {
                Value::Fixed { m, q, dec } => Value::fixed(m.abs(), *q, *dec),
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
            Round => {
                let n = int(1).unwrap_or(0);
                match (&v[0], rt) {
                    (Value::Fixed { m, q, dec }, Ty::Fixed { q: rq, .. }) => {
                        let base: i128 = if *dec { 10 } else { 2 };
                        let drop = i64::from(*q) - n;
                        let r = if drop > 0 {
                            let d = base.pow(drop as u32);
                            let half = d / 2;
                            let a = m.abs();
                            let r = (a + half) / d;
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
            Hex => {
                let bytes = match (&v[0], &args[0].ty) {
                    (Value::Char(c), Ty::Char { .. }) => c.clone(),
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
                    let p = v
                        .get(1)
                        .map(|p| zeus_cobol::ebcdic::decode(&chars(p)))
                        .unwrap_or_else(|| "YYYYMMDDHHMISS999".into());
                    let (y, m, d) = parse_date(&t, &p);
                    Value::int(lilian(y, m, d))
                }
                None => {
                    let (y, m, d, ..) = now_parts();
                    Value::int(lilian(y, m, d))
                }
            },
            DaysToDate => {
                let n = int(0).unwrap_or(1);
                let (y, m, d) = civil(n - 141_428);
                let p = v
                    .get(1)
                    .map(|p| zeus_cobol::ebcdic::decode(&chars(p)))
                    .unwrap_or_else(|| "YYYYMMDDHHMISS999".into());
                Value::Char(ebcdic(&format_date(y, m, d, &p)))
            }
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
            Null => Value::Ptr(value::NULL),
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
            Addr => {
                let (bit, _) = self.element_or_start(r, act)?;
                Value::Ptr((bit / 8) as u32)
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
            Present => Value::bit1(true),
            Omitted => Value::bit1(false),
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

fn parse_date(t: &str, p: &str) -> (i64, u32, u32) {
    let get = |key: &str| -> Option<i64> {
        let i = p.find(key)?;
        t.get(i..i + key.len())?.trim().parse().ok()
    };
    let y = get("YYYY")
        .or_else(|| get("YY").map(|y| if y < 50 { 2000 + y } else { 1900 + y }))
        .unwrap_or(1970);
    let m = get("MM").unwrap_or(1) as u32;
    let d = get("DD").unwrap_or(1) as u32;
    (y, m, d)
}

fn format_date(y: i64, m: u32, d: u32, p: &str) -> String {
    let mut out = p.to_string();
    for (k, v) in [
        ("YYYY", format!("{y:04}")),
        ("YY", format!("{:02}", y % 100)),
        ("MM", format!("{m:02}")),
        ("DD", format!("{d:02}")),
        ("HH", "00".into()),
        ("MI", "00".into()),
        ("SS", "00".into()),
        ("999", "000".into()),
    ] {
        out = out.replacen(k, &v, 1);
    }
    out
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

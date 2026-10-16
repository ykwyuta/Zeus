//! Intrinsic functions.

use zeus_decimal::{Dec, Rounding};

use super::eval::{Val, civil_from_days, days_from_civil};
use super::{Machine, RResult, err};
use crate::ebcdic;
use crate::zir::{Arg, DataType, Ex, Func, Loc};

pub(crate) fn timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = now.as_secs() as i64;
    let (y, m, d) = civil_from_days(seconds / 86400);
    format!(
        "{y:04}{m:02}{d:02}{:02}{:02}{:02}{:02}+0000",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60,
        now.subsec_millis() / 10
    )
}

/// Integer date (day 1 = 1601-01-01) to days since 1970-01-01.
fn int_to_unix_days(n: i64) -> i64 {
    n - 1 + days_from_civil(1601, 1, 1)
}

fn unix_days_to_int(d: i64) -> i64 {
    d - days_from_civil(1601, 1, 1) + 1
}

/// The 100-year interval ends at current_year + window (IBM LR YEAR-TO-YYYY).
fn expand_year(yy: i64, window: i64, current_year: i64) -> RResult<i64> {
    let Some(end) = current_year
        .checked_add(window)
        .filter(|y| (1700..10000).contains(y))
    else {
        return err("year window must end between 1700 and 9999");
    };
    if !(0..100).contains(&yy) {
        return err("windowed year must be between 0 and 99");
    }
    let year = end / 100 * 100 + yy;
    Ok(if year > end { year - 100 } else { year })
}

fn f(v: f64) -> RResult<Dec> {
    Dec::from_f64(v, 18)
        .map(|d| d.normalize())
        .ok_or_else(|| super::RunError("function result out of range".into()))
}

impl Machine {
    /// All values of the arguments (ALL subscripts expanded).
    fn arg_values(&mut self, cur: usize, args: &[Arg]) -> RResult<Vec<Val>> {
        let mut out = Vec::new();
        for a in args {
            match a {
                Arg::Num(e) => out.push(Val::Num(self.eval_ex(cur, e)?)),
                Arg::Src(s) => out.push(self.eval_src(cur, s)?),
                Arg::All(loc) => {
                    let dims = self.instances[cur].code.descs[loc.desc as usize]
                        .dims
                        .clone();
                    let counts: Vec<u32> = dims.iter().map(|d| d.max).collect();
                    let mut idx = vec![1u32; counts.len()];
                    loop {
                        let l = Loc {
                            desc: loc.desc,
                            subs: idx
                                .iter()
                                .map(|&i| Ex::Lit(Dec::from_i64(i as i64)))
                                .collect(),
                            refmod: None,
                        };
                        out.push(self.eval_src(cur, &crate::zir::Src::Loc(l))?);
                        let mut k = counts.len();
                        let mut done = true;
                        while k > 0 {
                            k -= 1;
                            idx[k] += 1;
                            if idx[k] <= counts[k] {
                                done = false;
                                break;
                            }
                            idx[k] = 1;
                        }
                        if done || counts.is_empty() {
                            break;
                        }
                    }
                }
            }
        }
        Ok(out)
    }

    fn nums(&mut self, cur: usize, args: &[Arg]) -> RResult<Vec<Dec>> {
        let vals = self.arg_values(cur, args)?;
        Ok(vals.iter().map(|v| self.val_num(v)).collect())
    }

    pub(super) fn func_num(&mut self, cur: usize, func: Func, args: &[Arg]) -> RResult<Dec> {
        let need = |v: &Vec<Dec>, n: usize| -> RResult<()> {
            if v.len() < n {
                return err(format!("function {func:?} needs {n} argument(s)"));
            }
            Ok(())
        };
        Ok(match func {
            Func::Length | Func::ByteLength => {
                let vals = self.arg_values(cur, args)?;
                let n = match vals.first() {
                    // national data: characters (two bytes each)
                    Some(Val::Item(ty, b)) if ty.is_national() && func == Func::Length => {
                        b.len() / 2
                    }
                    Some(Val::Item(DataType::Numeric(_), b) | Val::Item(_, b)) => b.len(),
                    Some(Val::Alnum(b) | Val::All(b)) => b.len(),
                    Some(Val::NumLit(_, d)) => d.len(),
                    Some(Val::Fig(_)) => 1,
                    Some(Val::Num(_)) | None => 0,
                };
                Dec::from_i64(n as i64)
            }
            Func::Ord => {
                let vals = self.arg_values(cur, args)?;
                let b = match vals.first() {
                    Some(v) => self.alnum_bytes(cur, v, 1)?,
                    None => vec![0],
                };
                let coll = self.instances[cur].code.collating.clone();
                let w = match coll {
                    Some(c) => c[b[0] as usize] as i64,
                    None => b[0] as i64,
                };
                Dec::from_i64(w + 1)
            }
            Func::OrdMax | Func::OrdMin => {
                let vals = self.arg_values(cur, args)?;
                let numeric = vals.iter().all(|v| {
                    matches!(
                        v,
                        Val::Num(_) | Val::NumLit(..) | Val::Item(DataType::Numeric(_), _)
                    )
                });
                let mut best = 0usize;
                for i in 1..vals.len() {
                    let ord = if numeric {
                        self.val_num(&vals[i]).cmp(&self.val_num(&vals[best]))
                    } else {
                        let a = self.alnum_bytes(cur, &vals[i], 0)?;
                        let b = self.alnum_bytes(cur, &vals[best], 0)?;
                        let coll = self.instances[cur].code.collating.clone();
                        cmp_padded(&a, &b, coll.as_deref())
                    };
                    let better = if func == Func::OrdMax {
                        ord == std::cmp::Ordering::Greater
                    } else {
                        ord == std::cmp::Ordering::Less
                    };
                    if better {
                        best = i;
                    }
                }
                Dec::from_i64(best as i64 + 1)
            }
            Func::TestNumVal | Func::TestNumValC => {
                let vals = self.arg_values(cur, args)?;
                let b = match vals.first() {
                    Some(v) => self.alnum_bytes(cur, v, 0)?,
                    None => vec![],
                };
                let s: String = ebcdic::decode_latin1(&b)
                    .iter()
                    .map(|&c| c as char)
                    .collect();
                Dec::from_i64(test_numval(&s, func == Func::TestNumValC) as i64)
            }
            Func::NumVal | Func::NumValC => {
                let vals = self.arg_values(cur, args)?;
                let b = match vals.first() {
                    Some(v) => self.alnum_bytes(cur, v, 0)?,
                    None => vec![],
                };
                let s: String = ebcdic::decode_latin1(&b)
                    .iter()
                    .map(|&c| c as char)
                    .collect();
                numval(&s, func == Func::NumValC)
            }
            Func::Random => {
                // a linear congruential generator: RANDOM(seed) starts a
                // sequence, RANDOM continues it
                let v = self.nums(cur, args)?;
                if let Some(seed) = v.first() {
                    let seed = seed
                        .rescale(0, zeus_decimal::Rounding::Truncate)
                        .map_or(0, |d| d.coef) as u64;
                    self.random = seed ^ 0x0005_DEEC_E66D;
                }
                self.random = self
                    .random
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                let x = (self.random >> 11) as f64 / (1u64 << 53) as f64;
                Dec::from_f64(x, 18).unwrap_or(Dec::ZERO)
            }
            Func::Max | Func::Min => {
                let v = self.nums(cur, args)?;
                need(&v, 1)?;
                let it = v.iter().copied();
                if func == Func::Max {
                    it.max().unwrap()
                } else {
                    it.min().unwrap()
                }
            }
            _ => {
                let v = self.nums(cur, args)?;
                numeric_function(func, &v)?
            }
        })
    }

    pub(super) fn func_alnum(&mut self, cur: usize, func: Func, args: &[Arg]) -> RResult<Vec<u8>> {
        let vals = self.arg_values(cur, args)?;
        Ok(match func {
            // the code page: the second argument (a CCSID), else the
            // program's
            Func::NationalOf | Func::DisplayOf => {
                let cp = match vals.get(1) {
                    Some(v) => {
                        let c = super::data::to_raw(self.val_num(v), 0, false);
                        crate::codepage::get(c as u32)
                            .ok_or_else(|| super::RunError(format!("CCSID {c} is not supported")))?
                    }
                    None => crate::codepage::active(),
                };
                match (func, vals.first()) {
                    (Func::NationalOf, Some(Val::Item(ty, b))) if ty.is_national() => b.clone(),
                    (Func::NationalOf, Some(Val::Item(DataType::Dbcs, b))) => {
                        super::data::utf16(&cp.decode_dbcs(b))
                    }
                    (Func::NationalOf, Some(v)) => {
                        let b = self.alnum_bytes(cur, v, 0)?;
                        super::data::utf16(&cp.decode(&b))
                    }
                    (Func::DisplayOf, Some(v)) => {
                        let b = self.national_bytes(cur, v)?;
                        cp.encode(&super::data::from_utf16(&b))
                    }
                    _ => vec![],
                }
            }
            Func::Char => {
                let n = vals.first().map(|v| self.val_num(v)).unwrap_or(Dec::ZERO);
                let n = super::data::to_raw(n, 0, false);
                let w = (n - 1).clamp(0, 255) as u16;
                // the character at ordinal position n of the program
                // collating sequence
                let b = match &self.instances[cur].code.collating {
                    Some(c) => (0..=255u8).find(|&b| c[b as usize] == w).unwrap_or(w as u8),
                    None => w as u8,
                };
                vec![b]
            }
            Func::Trim => {
                let b = match vals.first() {
                    Some(v) => self.alnum_bytes(cur, v, 0)?,
                    None => vec![],
                };
                let mode = match vals.get(1) {
                    Some(v) => self.alnum_bytes(cur, v, 0)?.first().copied(),
                    None => None,
                };
                let (lead, trail) = match mode {
                    Some(0xD3) => (true, false), // L
                    Some(0xE3) => (false, true), // T
                    _ => (true, true),
                };
                let sp = ebcdic::SPACE;
                let s = if lead {
                    b.iter().position(|&c| c != sp).unwrap_or(b.len())
                } else {
                    0
                };
                let e = if trail {
                    b.iter().rposition(|&c| c != sp).map_or(s, |p| p + 1)
                } else {
                    b.len()
                };
                // all spaces: a zero-length value (CardDemo tests
                // FUNCTION LENGTH(FUNCTION TRIM(x)) = 0 for blank fields)
                if s >= e { Vec::new() } else { b[s..e].to_vec() }
            }
            Func::UpperCase | Func::LowerCase | Func::Reverse => {
                let mut b = match vals.first() {
                    Some(v) => self.alnum_bytes(cur, v, 0)?,
                    None => vec![],
                };
                match func {
                    Func::Reverse => b.reverse(),
                    Func::UpperCase => {
                        for x in &mut b {
                            if matches!(*x, 0x81..=0x89 | 0x91..=0x99 | 0xA2..=0xA9) {
                                *x += 0x40;
                            }
                        }
                    }
                    _ => {
                        for x in &mut b {
                            if matches!(*x, 0xC1..=0xC9 | 0xD1..=0xD9 | 0xE2..=0xE9) {
                                *x -= 0x40;
                            }
                        }
                    }
                }
                b
            }
            Func::WhenCompiled => ebcdic::encode(self.instances[cur].module.compiled_at.as_deref()
                .ok_or_else(|| super::RunError("WHEN-COMPILED is unavailable in this legacy module; recompile the source".into()))?),
            Func::WhenCompiledRegister => {
                let t = self.instances[cur].module.compiled_at.as_deref().filter(|s| s.len() == 21 && s.is_ascii())
                    .ok_or_else(|| super::RunError("WHEN-COMPILED timestamp unavailable; recompile the source".into()))?;
                ebcdic::encode(&format!("{}/{}/{}{}.{}.{}", &t[4..6], &t[6..8], &t[2..4], &t[8..10], &t[10..12], &t[12..14]))
            },
            Func::CurrentDate => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs_f64())
                    .unwrap_or(0.0);
                let days = (now / 86400.0).floor() as i64;
                let secs = now - days as f64 * 86400.0;
                let (y, m, d) = civil_from_days(days);
                let h = (secs / 3600.0) as i64;
                let mi = ((secs % 3600.0) / 60.0) as i64;
                let s = (secs % 60.0) as i64;
                let hs = ((secs * 100.0) as i64) % 100;
                let text = format!("{y:04}{m:02}{d:02}{h:02}{mi:02}{s:02}{hs:02}+0000");
                ebcdic::encode(&text)
            }
            Func::Max | Func::Min => {
                let mut best: Option<Vec<u8>> = None;
                for v in &vals {
                    let b = self.alnum_bytes(cur, v, 0)?;
                    best = Some(match best {
                        None => b,
                        Some(p) => {
                            let coll = self.instances[cur].code.collating.clone();
                            let o = cmp_padded(&b, &p, coll.as_deref());
                            if (func == Func::Max && o == std::cmp::Ordering::Greater)
                                || (func == Func::Min && o == std::cmp::Ordering::Less)
                            {
                                b
                            } else {
                                p
                            }
                        }
                    });
                }
                best.unwrap_or_default()
            }
            other => {
                let n = self.func_num(cur, other, args)?;
                ebcdic::encode(&n.to_string())
            }
        })
    }
}

/// Compares two alphanumeric values (the shorter padded with spaces) in
/// the program collating sequence.
fn cmp_padded(a: &[u8], b: &[u8], coll: Option<&[u16]>) -> std::cmp::Ordering {
    let w = |c: u8| coll.map_or(c as u16, |t| t[c as usize]);
    let n = a.len().max(b.len());
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(ebcdic::SPACE);
        let y = b.get(i).copied().unwrap_or(ebcdic::SPACE);
        if w(x) != w(y) {
            return w(x).cmp(&w(y));
        }
    }
    std::cmp::Ordering::Equal
}

/// TEST-NUMVAL / TEST-NUMVAL-C: 0 when `s` is a valid argument of
/// NUMVAL (NUMVAL-C), otherwise the position of the first character in
/// error (the length plus one when the string ends too early).
fn test_numval(s: &str, currency: bool) -> usize {
    let c: Vec<char> = s.chars().collect();
    let n = c.len();
    let mut i = 0;
    let skip = |i: &mut usize| {
        while *i < n && c[*i] == ' ' {
            *i += 1;
        }
    };
    skip(&mut i);
    let mut sign = false;
    if i < n && (c[i] == '+' || c[i] == '-') {
        sign = true;
        i += 1;
        skip(&mut i);
    }
    if currency && i < n && c[i] == '$' {
        i += 1;
        skip(&mut i);
        if !sign && i < n && (c[i] == '+' || c[i] == '-') {
            sign = true;
            i += 1;
            skip(&mut i);
        }
    }
    let mut digits = 0;
    let mut point = false;
    while i < n {
        match c[i] {
            '0'..='9' => digits += 1,
            '.' if !point => point = true,
            ',' if currency && !point => {}
            _ => break,
        }
        i += 1;
    }
    if digits == 0 {
        return if i >= n { n + 1 } else { i + 1 };
    }
    skip(&mut i);
    if !sign && i < n {
        let rest: String = c[i..].iter().collect::<String>().to_ascii_uppercase();
        if c[i] == '+' || c[i] == '-' {
            i += 1;
        } else if rest.starts_with("CR") || rest.starts_with("DB") {
            i += 2;
        }
        skip(&mut i);
    }
    if i < n { i + 1 } else { 0 }
}

fn numval(s: &str, currency: bool) -> Dec {
    let t = s.trim();
    let mut neg = false;
    let mut digits = String::new();
    for c in t.chars() {
        match c {
            '-' => neg = true,
            '0'..='9' | '.' => digits.push(c),
            'C' | 'D' | 'R' | 'B' => {
                if t.ends_with("CR") || t.ends_with("DB") {
                    neg = true;
                }
            }
            ',' if currency => {}
            '$' if currency => {}
            _ => {}
        }
    }
    let v = Dec::parse(&digits).unwrap_or(Dec::ZERO);
    if neg { -v } else { v }
}

fn numeric_function(func: Func, v: &[Dec]) -> RResult<Dec> {
    let a = |i: usize| -> RResult<Dec> {
        v.get(i)
            .copied()
            .ok_or_else(|| super::RunError(format!("function {func:?}: missing argument")))
    };
    let int = |d: Dec| {
        d.rescale(0, Rounding::Truncate)
            .map(|x| x.coef as i64)
            .unwrap_or(0)
    };
    Ok(match func {
        Func::Abs => a(0)?.abs(),
        Func::Sqrt => f(a(0)?.to_f64().sqrt())?,
        Func::Sin => f(a(0)?.to_f64().sin())?,
        Func::Cos => f(a(0)?.to_f64().cos())?,
        Func::Tan => f(a(0)?.to_f64().tan())?,
        Func::Asin => f(a(0)?.to_f64().asin())?,
        Func::Acos => f(a(0)?.to_f64().acos())?,
        Func::Atan => f(a(0)?.to_f64().atan())?,
        Func::Log => f(a(0)?.to_f64().ln())?,
        Func::Log10 => f(a(0)?.to_f64().log10())?,
        Func::Factorial => {
            let n = int(a(0)?);
            let mut r = Dec::new(1, 0);
            for i in 2..=n {
                r = r
                    .checked_mul(Dec::from_i64(i))
                    .ok_or_else(|| super::RunError("FACTORIAL overflow".into()))?;
            }
            r
        }
        Func::Integer => {
            let x = a(0)?;
            let t = x.rescale(0, Rounding::Truncate).unwrap_or(x);
            if x < t {
                t.checked_sub(Dec::new(1, 0)).unwrap()
            } else {
                t
            }
        }
        Func::IntegerPart => a(0)?.rescale(0, Rounding::Truncate).unwrap_or(Dec::ZERO),
        Func::Mod => {
            let (x, y) = (a(0)?, a(1)?);
            if y.is_zero() {
                return err("MOD by zero");
            }
            let q = numeric_function(
                Func::Integer,
                &[x.checked_div(y, 20, Rounding::Truncate).unwrap()],
            )?;
            x.checked_sub(y.checked_mul(q).unwrap()).unwrap()
        }
        Func::Rem => {
            let (x, y) = (a(0)?, a(1)?);
            if y.is_zero() {
                return err("REM by zero");
            }
            let q = x.checked_div(y, 0, Rounding::Truncate).unwrap();
            x.checked_sub(y.checked_mul(q).unwrap()).unwrap()
        }
        Func::Sum => v
            .iter()
            .try_fold(Dec::ZERO, |acc, x| acc.checked_add(*x))
            .unwrap_or(Dec::ZERO),
        Func::Mean => {
            let s = numeric_function(Func::Sum, v)?;
            s.checked_div(Dec::from_i64(v.len().max(1) as i64), 20, Rounding::Truncate)
                .unwrap()
                .normalize()
        }
        Func::Midrange => {
            let mx = *v.iter().max().unwrap_or(&Dec::ZERO);
            let mn = *v.iter().min().unwrap_or(&Dec::ZERO);
            mx.checked_add(mn)
                .unwrap()
                .checked_div(Dec::from_i64(2), 20, Rounding::Truncate)
                .unwrap()
                .normalize()
        }
        Func::Range => {
            let mx = *v.iter().max().unwrap_or(&Dec::ZERO);
            let mn = *v.iter().min().unwrap_or(&Dec::ZERO);
            mx.checked_sub(mn).unwrap()
        }
        Func::Median => {
            let mut s = v.to_vec();
            s.sort();
            let n = s.len();
            if n == 0 {
                Dec::ZERO
            } else if n % 2 == 1 {
                s[n / 2]
            } else {
                s[n / 2 - 1]
                    .checked_add(s[n / 2])
                    .unwrap()
                    .checked_div(Dec::from_i64(2), 20, Rounding::Truncate)
                    .unwrap()
                    .normalize()
            }
        }
        Func::Variance | Func::StandardDeviation => {
            let n = v.len().max(1) as f64;
            let mean = v.iter().map(|x| x.to_f64()).sum::<f64>() / n;
            let var = v.iter().map(|x| (x.to_f64() - mean).powi(2)).sum::<f64>() / n;
            if func == Func::Variance {
                f(var)?
            } else {
                f(var.sqrt())?
            }
        }
        Func::Annuity => {
            let (r, n) = (a(0)?.to_f64(), a(1)?.to_f64());
            if r == 0.0 {
                f(1.0 / n)?
            } else {
                f(r / (1.0 - (1.0 + r).powf(-n)))?
            }
        }
        Func::PresentValue => {
            let r = a(0)?.to_f64();
            let mut pv = 0.0;
            for (i, x) in v[1..].iter().enumerate() {
                pv += x.to_f64() / (1.0 + r).powi(i as i32 + 1);
            }
            f(pv)?
        }
        Func::Random => {
            // deterministic linear congruential generator
            let seed = v.first().map(|x| int(*x)).unwrap_or(0) as u64;
            let x = (seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407)
                >> 11) as f64
                / (1u64 << 53) as f64;
            f(x)?
        }
        Func::IntegerOfDate => {
            let n = int(a(0)?);
            let (y, m, d) = (n / 10000, n / 100 % 100, n % 100);
            Dec::from_i64(unix_days_to_int(days_from_civil(y, m, d)))
        }
        Func::IntegerOfDay => {
            let n = int(a(0)?);
            let (y, doy) = (n / 1000, n % 1000);
            Dec::from_i64(unix_days_to_int(days_from_civil(y, 1, 1)) + doy - 1)
        }
        Func::DateOfInteger => {
            let (y, m, d) = civil_from_days(int_to_unix_days(int(a(0)?)));
            Dec::from_i64(y * 10000 + m * 100 + d)
        }
        Func::DayOfInteger => {
            let days = int_to_unix_days(int(a(0)?));
            let (y, _, _) = civil_from_days(days);
            Dec::from_i64(y * 1000 + days - days_from_civil(y, 1, 1) + 1)
        }
        Func::DateToYyyymmdd | Func::DayToYyyyddd | Func::YearToYyyy => {
            let x = int(a(0)?);
            let window = v.get(1).map(|w| int(*w)).unwrap_or(50);
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| super::RunError(e.to_string()))?;
            let cur_year = civil_from_days((now.as_secs() / 86400) as i64).0;
            let (yy, rest, mult) = match func {
                Func::DateToYyyymmdd => (x / 10000, x % 10000, 10000),
                Func::DayToYyyyddd => (x / 1000, x % 1000, 1000),
                _ => (x, 0, 1),
            };
            let y = expand_year(yy, window, cur_year)?;
            Dec::from_i64(y * mult + rest)
        }
        _ => return err(format!("function {func:?} is not supported yet")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ibm_year_to_yyyy_reference_examples_and_window_boundaries() {
        assert_eq!(expand_year(4, 23, 1995).unwrap(), 2004);
        assert_eq!(expand_year(4, -15, 1995).unwrap(), 1904);
        assert_eq!(expand_year(98, 23, 2008).unwrap(), 1998);
        assert_eq!(expand_year(98, -15, 2008).unwrap(), 1898);
        assert_eq!(expand_year(76, 50, 2026).unwrap(), 2076);
        assert_eq!(expand_year(77, 50, 2026).unwrap(), 1977);
        assert_eq!(expand_year(76, 50, 2027).unwrap(), 2076);
        assert_eq!(expand_year(77, 50, 2027).unwrap(), 2077);
        assert!(expand_year(0, -301, 2000).is_err());
        assert_eq!(expand_year(0, -300, 2000).unwrap(), 1700);
        assert_eq!(expand_year(99, 7999, 2000).unwrap(), 9999);
        assert!(expand_year(0, 8000, 2000).is_err());
        assert!(expand_year(-1, 50, 2000).is_err());
        assert!(expand_year(100, 50, 2000).is_err());
        assert!(expand_year(0, i64::MAX, 2000).is_err());
    }

    #[test]
    fn test_numval_positions() {
        assert_eq!(test_numval(" -12.5 ", false), 0);
        assert_eq!(test_numval("12.5CR", false), 0);
        assert_eq!(test_numval("12A", false), 3);
        assert_eq!(test_numval("   ", false), 4);
        assert_eq!(test_numval("$1,234.50", true), 0);
        assert_eq!(test_numval("$1,234.50", false), 1);
    }
}

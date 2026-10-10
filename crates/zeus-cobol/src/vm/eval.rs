//! Addressing, expression and condition evaluation, MOVE and arithmetic
//! stores.

use std::cmp::Ordering;
use std::io::BufRead;

use zeus_decimal::{Dec, Rounding, binary, packed, zoned};

use super::data as d;
use super::{Machine, RResult, SIZE_ERROR, err};
use crate::ebcdic;
use crate::zir::{
    AcceptFrom, Base, CallArgMode, ClassKind, CmpMode, Cx, DataType, Ex, Fig, Loc, NumUsage, Op,
    Rel, SignKind, Src,
};

/// An evaluated operand.
#[derive(Debug, Clone)]
pub enum Val {
    /// Contents of a data item (copied) and its type.
    Item(DataType, Vec<u8>),
    Alnum(Vec<u8>),
    NumLit(Dec, Vec<u8>),
    Fig(Fig),
    All(Vec<u8>),
    Num(Dec),
}

fn fig_byte(f: Fig) -> u8 {
    match f {
        Fig::Zero => ebcdic::ZERO,
        Fig::Space => ebcdic::SPACE,
        Fig::High => 0xFF,
        Fig::Low => 0x00,
        Fig::Quote => ebcdic::QUOTE,
    }
}

/// Integer part of a value as display digits (unsigned).
fn int_digits(v: Dec) -> Vec<u8> {
    let i = v
        .rescale(0, Rounding::Truncate)
        .map(|x| x.coef)
        .unwrap_or(0);
    let s = i.unsigned_abs().to_string();
    s.bytes().map(|c| 0xF0 + (c - b'0')).collect()
}

impl Machine {
    // ---- addressing ----------------------------------------------------

    fn int_of(&mut self, cur: usize, e: &Ex) -> RResult<i64> {
        let v = self.eval_ex(cur, e)?;
        Ok(v.rescale(0, Rounding::Truncate)
            .map(|x| x.coef as i64)
            .unwrap_or(0))
    }

    /// Absolute address and length of a location.
    pub(super) fn addr(&mut self, cur: usize, loc: &Loc) -> RResult<(usize, usize)> {
        let code = self.instances[cur].code.clone();
        let desc = &code.descs[loc.desc as usize];
        let base = match desc.base {
            Base::Static => self.instances[cur].static_base,
            Base::Linkage(n) => match self.instances[cur].linkage[n as usize] {
                Some(a) => a,
                None => {
                    return err(format!(
                        "{}: linkage item {} is not addressable",
                        code.name, desc.name
                    ));
                }
            },
        };
        let mut a = base + desc.offset as usize;
        let mut len = desc.len as usize;
        for (k, dim) in desc.dims.iter().enumerate() {
            let s = match loc.subs.get(k) {
                Some(e) => self.int_of(cur, e)?,
                None => 1,
            };
            let s = s.max(1) as usize;
            a += (s - 1) * dim.stride as usize;
        }
        if let Some(odo) = &desc.odo {
            let n = {
                let l = (*odo.depending).clone();
                let (oa, ol) = self.addr(cur, &l)?;
                let ty = code.descs[l.desc as usize].ty.clone();
                d::value_of(&ty, &self.mem[oa..oa + ol])
                    .rescale(0, Rounding::Truncate)
                    .map(|x| x.coef.max(0) as usize)
                    .unwrap_or(0)
            };
            len = odo.fixed as usize + n * odo.stride as usize;
        }
        if let Some((start, l)) = &loc.refmod {
            let s = self.int_of(cur, start)?.max(1) as usize;
            let total = len;
            let l = match l {
                Some(l) => self.int_of(cur, l)?.max(0) as usize,
                None => total.saturating_sub(s - 1),
            };
            a += s - 1;
            len = l;
        }
        if a + len > self.mem.len() {
            return err(format!(
                "{}: reference to {} out of storage",
                code.name, desc.name
            ));
        }
        Ok((a, len))
    }

    fn loc_type(&self, cur: usize, loc: &Loc) -> DataType {
        let ty = &self.instances[cur].code.descs[loc.desc as usize].ty;
        if loc.refmod.is_some() {
            DataType::Alphanumeric { just: false }
        } else {
            ty.clone()
        }
    }

    // ---- evaluation ----------------------------------------------------

    pub(super) fn eval_ex(&mut self, cur: usize, e: &Ex) -> RResult<Dec> {
        Ok(match e {
            Ex::Lit(v) => *v,
            Ex::Temp(t) => self.instances[cur].temps[*t as usize],
            Ex::Load(loc) => {
                let (a, l) = self.addr(cur, loc)?;
                let ty = self.loc_type(cur, loc);
                d::value_of(&ty, &self.mem[a..a + l])
            }
            Ex::LengthOf(loc) => {
                let (_, l) = self.addr(cur, loc)?;
                Dec::from_i64(l as i64)
            }
            Ex::Neg(x) => -self.eval_ex(cur, x)?,
            Ex::Bin(op, a, b) => {
                let x = self.eval_ex(cur, a)?;
                let y = self.eval_ex(cur, b)?;
                arith(*op, x, y)?
            }
            Ex::Func(f, args) => self.func_num(cur, *f, args)?,
        })
    }

    pub(super) fn eval_src(&mut self, cur: usize, s: &Src) -> RResult<Val> {
        Ok(match s {
            Src::Loc(loc) => {
                let (a, l) = self.addr(cur, loc)?;
                Val::Item(self.loc_type(cur, loc), self.mem[a..a + l].to_vec())
            }
            Src::Alnum(b) => Val::Alnum(b.clone()),
            Src::NumLit(v, digits) => Val::NumLit(*v, digits.clone()),
            // With a PROGRAM COLLATING SEQUENCE, HIGH-VALUE and LOW-VALUE
            // are its highest and lowest characters.
            Src::Fig(Fig::High) if self.instances[cur].code.high_value != 0xFF => {
                Val::All(vec![self.instances[cur].code.high_value])
            }
            Src::Fig(Fig::Low) if self.instances[cur].code.low_value != 0x00 => {
                Val::All(vec![self.instances[cur].code.low_value])
            }
            Src::Fig(f) => Val::Fig(*f),
            Src::All(p) => Val::All(p.clone()),
            Src::Num(e) => Val::Num(self.eval_ex(cur, e)?),
            Src::Func(f, args, refmod) => {
                let mut b = self.func_alnum(cur, *f, args)?;
                if let Some((s, l)) = refmod {
                    let s = self.int_of(cur, s)?.max(1) as usize;
                    let l = match l {
                        Some(l) => self.int_of(cur, l)?.max(0) as usize,
                        None => b.len().saturating_sub(s - 1),
                    };
                    b = b
                        .get(s - 1..(s - 1 + l).min(b.len()))
                        .unwrap_or(&[])
                        .to_vec();
                }
                Val::Alnum(b)
            }
        })
    }

    /// Numeric value of an operand.
    pub(super) fn val_num(&self, v: &Val) -> Dec {
        match v {
            Val::Item(ty, b) => d::value_of(ty, b),
            Val::NumLit(x, _) | Val::Num(x) => *x,
            Val::Fig(_) => Dec::ZERO,
            Val::Alnum(b) | Val::All(b) => d::alnum_as_number(b),
        }
    }

    /// Bytes of an operand as alphanumeric data; figurative constants and
    /// ALL literals are expanded to `len` (at least one character).
    pub(super) fn alnum_bytes(&self, _cur: usize, v: &Val, len: usize) -> RResult<Vec<u8>> {
        Ok(match v {
            Val::Item(DataType::Numeric(t), b) => {
                // A numeric item as an unsigned display integer.
                let raw = d::value_of(&DataType::Numeric(*t), b);
                let raw = d::to_raw(raw, t.scale, false);
                d::display_digits(raw, t.digits as usize)
            }
            Val::Item(DataType::Index, b) => int_digits(Dec::new(binary::read(b, true), 0)),
            Val::Item(_, b) | Val::Alnum(b) => b.clone(),
            Val::NumLit(_, digits) => digits.clone(),
            Val::Num(x) => int_digits(*x),
            Val::Fig(f) => vec![fig_byte(*f); len.max(1)],
            Val::All(p) => {
                let mut out = vec![0; len.max(p.len())];
                d::fill_pattern(&mut out, p);
                out
            }
        })
    }

    /// Bytes shown by DISPLAY.
    pub(super) fn display_bytes(&self, _cur: usize, v: &Val) -> RResult<Vec<u8>> {
        Ok(match v {
            Val::Item(DataType::Numeric(t), b) => match t.usage {
                NumUsage::Display => b.clone(),
                _ => {
                    let x = d::value_of(&DataType::Numeric(*t), b);
                    let raw = d::to_raw(x, t.scale, false);
                    let mut digits = d::display_digits(raw, t.digits as usize);
                    if t.signed
                        && let Some(last) = digits.last_mut()
                    {
                        *last = (if raw < 0 { 0xD0 } else { 0xC0 }) | (*last & 0xF);
                    }
                    digits
                }
            },
            Val::Item(DataType::Index, b) => int_digits(Dec::new(binary::read(b, true), 0)),
            Val::Item(_, b) | Val::Alnum(b) => b.clone(),
            Val::NumLit(x, _) | Val::Num(x) => ebcdic::encode(&x.to_string()),
            Val::Fig(f) => vec![fig_byte(*f)],
            Val::All(p) => p.clone(),
        })
    }

    // ---- MOVE ----------------------------------------------------------

    pub(super) fn move_val(&mut self, cur: usize, v: &Val, dst: &Loc) -> RResult<()> {
        let (a, l) = self.addr(cur, dst)?;
        let ty = self.loc_type(cur, dst);
        let mut out = self.mem[a..a + l].to_vec();
        move_into(v, &ty, &mut out);
        self.mem[a..a + l].copy_from_slice(&out);
        Ok(())
    }

    /// Stores an arithmetic result.
    pub(super) fn store(
        &mut self,
        cur: usize,
        v: Dec,
        dst: &Loc,
        rounded: bool,
        check: bool,
    ) -> RResult<()> {
        let (a, l) = self.addr(cur, dst)?;
        let ty = self.loc_type(cur, dst);
        let field = &mut self.mem[a..a + l];
        match &ty {
            DataType::Numeric(t) => {
                if d::would_overflow(t, l, v, rounded) {
                    self.size_error = true;
                    if check {
                        return Ok(());
                    }
                }
                d::store_num(t, field, v, rounded);
            }
            DataType::NumericEdited {
                edit,
                digits,
                scale,
                blank_zero,
                currency,
                point,
            } => {
                let raw = d::to_raw(v, *scale, rounded);
                if raw.unsigned_abs() >= 10u128.pow((*digits).min(38)) {
                    self.size_error = true;
                    if check {
                        return Ok(());
                    }
                }
                let vv = d::scaled(raw, *scale);
                d::edit_num(
                    edit,
                    *digits,
                    *scale,
                    *blank_zero,
                    *currency,
                    *point,
                    vv,
                    field,
                );
            }
            DataType::Index => binary::write(field, d::to_raw(v, 0, rounded)),
            _ => {
                let mut out = field.to_vec();
                move_into(&Val::Num(v), &ty, &mut out);
                field.copy_from_slice(&out);
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn div_rem(
        &mut self,
        cur: usize,
        a: Dec,
        b: Dec,
        quot: &Loc,
        rounded: bool,
        rem: &Loc,
        check: bool,
    ) -> RResult<()> {
        if b.is_zero() {
            self.size_error = true;
            return Ok(());
        }
        let qty = self.loc_type(cur, quot);
        let qscale = match &qty {
            DataType::Numeric(t) => t.scale,
            DataType::NumericEdited { scale, .. } => *scale,
            _ => 0,
        };
        // The remainder uses the truncated quotient as stored.
        let q_trunc = div_scaled(a, b, qscale.max(0) as u32)?;
        let q_store = if rounded {
            div_scaled(a, b, qscale.max(0) as u32 + 1)?
        } else {
            q_trunc
        };
        let before = self.size_error;
        self.size_error = false;
        self.store(cur, q_store, quot, rounded, check)?;
        let q_err = self.size_error;
        self.size_error = before || q_err;
        if q_err && check {
            return Ok(());
        }
        let r = a
            .checked_sub(
                q_trunc
                    .checked_mul(b)
                    .ok_or_else(|| super::RunError(SIZE_ERROR.into()))?,
            )
            .ok_or_else(|| super::RunError(SIZE_ERROR.into()))?;
        self.store(cur, r, rem, false, check)
    }

    /// SET target TO value (index, integer item).
    pub(super) fn set_value(&mut self, cur: usize, loc: &Loc, v: Dec) -> RResult<()> {
        let ty = self.loc_type(cur, loc);
        let (a, l) = self.addr(cur, loc)?;
        let field = &mut self.mem[a..a + l];
        match &ty {
            DataType::Index | DataType::Pointer => {
                binary::write(field, d::to_raw(v, 0, false));
            }
            DataType::Numeric(t) => {
                d::store_num(t, field, v, false);
            }
            _ => {
                let mut out = field.to_vec();
                move_into(&Val::Num(v), &ty, &mut out);
                field.copy_from_slice(&out);
            }
        }
        Ok(())
    }

    // ---- conditions ----------------------------------------------------

    pub(super) fn eval_cx(&mut self, cur: usize, c: &Cx) -> RResult<bool> {
        Ok(match c {
            Cx::True => true,
            Cx::Not(x) => !self.eval_cx(cur, x)?,
            Cx::And(a, b) => self.eval_cx(cur, a)? && self.eval_cx(cur, b)?,
            Cx::Or(a, b) => self.eval_cx(cur, a)? || self.eval_cx(cur, b)?,
            Cx::SizeError => self.size_error,
            Cx::IoCond => self.io_cond,
            Cx::CallException => self.call_exception,
            Cx::Overflow => self.overflow,
            Cx::Switch(idx, on) => {
                let name = &self.instances[cur].code.switches[*idx as usize];
                self.switches.get(name).copied().unwrap_or(false) == *on
            }
            Cx::Sign(e, k) => {
                let v = self.eval_ex(cur, e)?;
                match k {
                    SignKind::Positive => v > Dec::ZERO,
                    SignKind::Negative => v < Dec::ZERO,
                    SignKind::Zero => v.is_zero(),
                }
            }
            Cx::Class(s, k) => {
                let v = self.eval_src(cur, s)?;
                class_test(&v, k)
            }
            Cx::Cmp(a, op, b, mode) => {
                let va = self.eval_src(cur, a)?;
                let vb = self.eval_src(cur, b)?;
                let ord = match mode {
                    CmpMode::Numeric => self.val_num(&va).cmp(&self.val_num(&vb)),
                    CmpMode::Alnum => {
                        let la = self.alnum_len(&va);
                        let lb = self.alnum_len(&vb);
                        let n = la.max(lb).max(1);
                        let x = self.alnum_bytes(cur, &va, n)?;
                        let y = self.alnum_bytes(cur, &vb, n)?;
                        let coll = self.instances[cur].code.collating.clone();
                        compare_alnum(&x, &y, coll.as_deref())
                    }
                };
                match op {
                    Rel::Eq => ord == Ordering::Equal,
                    Rel::Ne => ord != Ordering::Equal,
                    Rel::Lt => ord == Ordering::Less,
                    Rel::Le => ord != Ordering::Greater,
                    Rel::Gt => ord == Ordering::Greater,
                    Rel::Ge => ord != Ordering::Less,
                }
            }
        })
    }

    /// Length an operand contributes to an alphanumeric comparison
    /// (0 for figurative constants, which adapt to the other operand).
    fn alnum_len(&self, v: &Val) -> usize {
        match v {
            Val::Fig(_) => 0,
            Val::All(p) => p.len(),
            Val::Item(DataType::Numeric(t), _) => t.digits as usize,
            Val::Item(_, b) | Val::Alnum(b) => b.len(),
            Val::NumLit(_, digits) => digits.len(),
            Val::Num(x) => int_digits(*x).len(),
        }
    }

    // ---- ACCEPT and CALL arguments -------------------------------------

    pub(super) fn accept(&mut self, cur: usize, dst: &Loc, from: AcceptFrom) -> RResult<()> {
        let v = match from {
            AcceptFrom::Stdin => {
                let _ = self.stdout.flush();
                if self.stdin_lines.is_none() {
                    self.stdin_lines = Some(std::io::stdin().lock().lines());
                }
                let line = self
                    .stdin_lines
                    .as_mut()
                    .and_then(|l| l.next())
                    .and_then(|r| r.ok())
                    .unwrap_or_default();
                let bytes: Vec<u8> = line
                    .chars()
                    .map(|c| ebcdic::from_latin1(u32::from(c).min(255) as u8))
                    .collect();
                Val::Alnum(bytes)
            }
            other => {
                let n = date_value(other);
                let width = match other {
                    AcceptFrom::Date | AcceptFrom::Time => {
                        if other == AcceptFrom::Time {
                            8
                        } else {
                            6
                        }
                    }
                    AcceptFrom::DateYyyymmdd => 8,
                    AcceptFrom::Day => 5,
                    AcceptFrom::DayYyyyddd => 7,
                    AcceptFrom::DayOfWeek => 1,
                    AcceptFrom::Stdin => 0,
                };
                Val::NumLit(Dec::new(n, 0), d::display_digits(n, width))
            }
        };
        self.move_val(cur, &v, dst)
    }

    pub(super) fn call_args(
        &mut self,
        cur: usize,
        args: &[CallArgMode],
    ) -> RResult<Vec<Option<usize>>> {
        let mut out = Vec::new();
        for a in args {
            out.push(match a {
                CallArgMode::Reference(loc) => Some(self.addr(cur, loc)?.0),
                CallArgMode::Content(src, len) => {
                    let v = self.eval_src(cur, src)?;
                    let mut bytes = vec![ebcdic::SPACE; *len as usize];
                    match &v {
                        Val::Item(_, b) => bytes = b.clone(),
                        other => {
                            let b = self.alnum_bytes(cur, other, *len as usize)?;
                            d::move_alnum(&b, &mut bytes, false);
                        }
                    }
                    Some(self.alloc(&bytes))
                }
                CallArgMode::Value(e) => {
                    let v = self.eval_ex(cur, e)?;
                    let mut b = [0u8; 4];
                    binary::write(&mut b, d::to_raw(v, 0, false));
                    Some(self.alloc(&b))
                }
                CallArgMode::Omitted => None,
            });
        }
        Ok(out)
    }
}

/// Current date/time values for ACCEPT ... FROM.
fn date_value(from: AcceptFrom) -> i128 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let days = (now / 86400.0).floor() as i64;
    let secs = now - (days as f64) * 86400.0;
    let (y, m, dd) = civil_from_days(days);
    let doy = days - days_from_civil(y, 1, 1) + 1;
    match from {
        AcceptFrom::Date => i128::from((y % 100) * 10000 + m * 100 + dd),
        AcceptFrom::DateYyyymmdd => i128::from(y * 10000 + m * 100 + dd),
        AcceptFrom::Day => i128::from((y % 100) * 1000 + doy),
        AcceptFrom::DayYyyyddd => i128::from(y * 1000 + doy),
        AcceptFrom::DayOfWeek => i128::from((days + 3).rem_euclid(7) + 1),
        AcceptFrom::Time => {
            let h = (secs / 3600.0) as i64;
            let mi = ((secs - h as f64 * 3600.0) / 60.0) as i64;
            let s = secs - h as f64 * 3600.0 - mi as f64 * 60.0;
            let hs = (s * 100.0) as i64;
            i128::from(h * 1_000_000 + mi * 10_000 + hs)
        }
        AcceptFrom::Stdin => 0,
    }
}

/// Days since 1970-01-01 -> (year, month, day). (H. Hinnant's algorithm)
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Division producing an intermediate with generous precision.
fn div_scaled(a: Dec, b: Dec, scale: u32) -> RResult<Dec> {
    a.checked_div(b, scale, Rounding::Truncate)
        .ok_or_else(|| super::RunError(SIZE_ERROR.into()))
}

pub fn arith(op: Op, x: Dec, y: Dec) -> RResult<Dec> {
    let overflow = || super::RunError(SIZE_ERROR.into());
    Ok(match op {
        Op::Add => x.checked_add(y).ok_or_else(overflow)?,
        Op::Sub => x.checked_sub(y).ok_or_else(overflow)?,
        Op::Mul => {
            let r = x.checked_mul(y).ok_or_else(overflow)?;
            // keep intermediate scale bounded
            if r.scale > 30 {
                r.rescale(30, Rounding::Truncate).ok_or_else(overflow)?
            } else {
                r
            }
        }
        Op::Div => {
            if y.is_zero() {
                return Err(overflow());
            }
            let mut scale = 20u32.max(x.scale).max(y.scale);
            loop {
                if let Some(q) = x.checked_div(y, scale, Rounding::Truncate) {
                    break q;
                }
                if scale == 0 {
                    return Err(overflow());
                }
                scale -= 1;
            }
        }
        Op::Pow => pow(x, y).ok_or_else(overflow)?,
    })
}

fn pow(x: Dec, y: Dec) -> Option<Dec> {
    let yn = y.normalize();
    if yn.scale == 0 && yn.coef.abs() <= 1000 {
        let e = yn.coef;
        if x.is_zero() {
            return if e > 0 { Some(Dec::ZERO) } else { None };
        }
        let p = x.checked_powi(e.unsigned_abs() as u32);
        return match p {
            Some(p) if e >= 0 => Some(if p.scale > 30 {
                p.rescale(30, Rounding::Truncate)?
            } else {
                p
            }),
            Some(p) => Dec::new(1, 0).checked_div(p, 20, Rounding::Truncate),
            None => Dec::from_f64(x.to_f64().powf(y.to_f64()), 10),
        };
    }
    let r = x.to_f64().powf(y.to_f64());
    if !r.is_finite() {
        return None;
    }
    Dec::from_f64(r, 15)
}

fn compare_alnum(a: &[u8], b: &[u8], coll: Option<&[u16]>) -> Ordering {
    let n = a.len().max(b.len());
    for i in 0..n {
        let x = a.get(i).copied().unwrap_or(ebcdic::SPACE);
        let y = b.get(i).copied().unwrap_or(ebcdic::SPACE);
        let (wx, wy) = match coll {
            Some(c) => (c[x as usize], c[y as usize]),
            None => (u16::from(x), u16::from(y)),
        };
        match wx.cmp(&wy) {
            Ordering::Equal => {}
            o => return o,
        }
    }
    Ordering::Equal
}

fn is_upper(b: u8) -> bool {
    matches!(b, 0xC1..=0xC9 | 0xD1..=0xD9 | 0xE2..=0xE9)
}

fn is_lower(b: u8) -> bool {
    matches!(b, 0x81..=0x89 | 0x91..=0x99 | 0xA2..=0xA9)
}

fn class_test(v: &Val, k: &ClassKind) -> bool {
    let bytes: &[u8] = match v {
        Val::Item(_, b) | Val::Alnum(b) => b,
        _ => return false,
    };
    match k {
        ClassKind::Numeric => match v {
            Val::Item(DataType::Numeric(t), b) => match t.usage {
                NumUsage::Display => {
                    let pos = if !t.signed {
                        zoned::SignPosition::Unsigned
                    } else {
                        match t.sign {
                            crate::zir::SignPos::None | crate::zir::SignPos::Trailing => {
                                zoned::SignPosition::TrailingOverpunch
                            }
                            crate::zir::SignPos::Leading => zoned::SignPosition::LeadingOverpunch,
                            crate::zir::SignPos::TrailingSeparate => {
                                zoned::SignPosition::TrailingSeparate
                            }
                            crate::zir::SignPos::LeadingSeparate => {
                                zoned::SignPosition::LeadingSeparate
                            }
                        }
                    };
                    zoned::is_valid(b, pos)
                }
                NumUsage::Packed => {
                    packed::is_valid(b) && (t.signed || b.last().is_some_and(|x| x & 0xF == 0xF))
                }
                _ => true,
            },
            _ => bytes.iter().all(|b| (0xF0..=0xF9).contains(b)),
        },
        ClassKind::Alphabetic => bytes
            .iter()
            .all(|&b| b == ebcdic::SPACE || is_upper(b) || is_lower(b)),
        ClassKind::Upper => bytes.iter().all(|&b| b == ebcdic::SPACE || is_upper(b)),
        ClassKind::Lower => bytes.iter().all(|&b| b == ebcdic::SPACE || is_lower(b)),
        ClassKind::Set(set) => bytes.iter().all(|b| set.contains(b)),
    }
}

/// The MOVE statement's data conversion into a receiving field.
pub fn move_into(v: &Val, ty: &DataType, out: &mut [u8]) {
    // Into an alphanumeric-edited item, a figurative constant or ALL
    // literal is the sending item of the receiver's size, then edited.
    if let DataType::AlphanumericEdited { edit } = ty
        && matches!(v, Val::Fig(_) | Val::All(_))
    {
        let mut src = vec![0u8; out.len()];
        match v {
            Val::Fig(f) => src.fill(fig_byte(*f)),
            Val::All(p) => d::fill_pattern(&mut src, p),
            _ => {}
        }
        d::edit_alnum(edit, &src, out);
        return;
    }
    // Figurative constants and ALL literals.
    match v {
        Val::Fig(Fig::Zero) => match ty {
            DataType::Numeric(t) => {
                d::store_num(t, out, Dec::ZERO, false);
                return;
            }
            DataType::NumericEdited { .. } => {
                move_into(&Val::Num(Dec::ZERO), ty, out);
                return;
            }
            _ => {
                out.fill(ebcdic::ZERO);
                return;
            }
        },
        Val::Fig(f) => {
            out.fill(fig_byte(*f));
            return;
        }
        Val::All(p) => {
            if let DataType::Numeric(_) | DataType::NumericEdited { .. } = ty
                && p.iter().all(|b| (0xF0..=0xF9).contains(b))
            {
                let mut tmp = vec![0; out.len().max(1)];
                d::fill_pattern(&mut tmp, p);
                move_into(&Val::Alnum(tmp), ty, out);
            } else {
                d::fill_pattern(out, p);
            }
            return;
        }
        _ => {}
    }
    let src_is_group = matches!(v, Val::Item(DataType::Group, _));
    // Group moves are alphanumeric moves without conversion.
    if src_is_group || matches!(ty, DataType::Group) {
        let bytes: Vec<u8> = match v {
            Val::Item(_, b) | Val::Alnum(b) => b.clone(),
            Val::NumLit(_, digits) => digits.clone(),
            Val::Num(x) => int_digits(*x),
            _ => Vec::new(),
        };
        let just = matches!(
            ty,
            DataType::Alphanumeric { just: true } | DataType::Alphabetic { just: true }
        );
        d::move_alnum(&bytes, out, just);
        return;
    }
    // Numeric value of the source, when the source is numeric.
    let numeric_src: Option<Dec> = match v {
        Val::Item(DataType::Numeric(_) | DataType::Index, _) => Some(numeric_val(v)),
        Val::Item(DataType::NumericEdited { .. }, _) => {
            if matches!(ty, DataType::Numeric(_) | DataType::NumericEdited { .. }) {
                Some(numeric_val(v))
            } else {
                None
            }
        }
        Val::NumLit(x, _) | Val::Num(x) => Some(*x),
        _ => None,
    };
    match ty {
        DataType::Numeric(t) => {
            let val = numeric_src.unwrap_or_else(|| numeric_val(v));
            d::store_num(t, out, val, false);
        }
        DataType::Index | DataType::Pointer => {
            let val = numeric_src.unwrap_or_else(|| numeric_val(v));
            binary::write(out, d::to_raw(val, 0, false));
        }
        DataType::NumericEdited {
            edit,
            digits,
            scale,
            blank_zero,
            currency,
            point,
        } => {
            let val = numeric_src.unwrap_or_else(|| numeric_val(v));
            d::edit_num(
                edit,
                *digits,
                *scale,
                *blank_zero,
                *currency,
                *point,
                val,
                out,
            );
        }
        DataType::Alphanumeric { just } | DataType::Alphabetic { just } => {
            let bytes = alnum_source(v);
            d::move_alnum(&bytes, out, *just);
        }
        DataType::AlphanumericEdited { edit } => {
            let bytes = alnum_source(v);
            d::edit_alnum(edit, &bytes, out);
        }
        DataType::Group => unreachable!(),
    }
}

fn numeric_val(v: &Val) -> Dec {
    match v {
        Val::Item(ty, b) => d::value_of(ty, b),
        Val::NumLit(x, _) | Val::Num(x) => *x,
        Val::Alnum(b) | Val::All(b) => d::alnum_as_number(b),
        Val::Fig(_) => Dec::ZERO,
    }
}

/// Source bytes for an alphanumeric receiving item.
fn alnum_source(v: &Val) -> Vec<u8> {
    match v {
        Val::Item(DataType::Numeric(t), b) => {
            // A numeric integer moves as unsigned display digits.
            let x = d::value_of(&DataType::Numeric(*t), b);
            let raw = d::to_raw(x, t.scale.min(0).max(t.scale), false);
            let n = if t.scale < 0 {
                (t.digits as i32 - t.scale) as usize
            } else {
                t.digits as usize
            };
            let raw = if t.scale < 0 { x.coef } else { raw };
            d::display_digits(raw, n)
        }
        Val::Item(DataType::Index, b) => int_digits(Dec::new(binary::read(b, true), 0)),
        Val::Item(_, b) | Val::Alnum(b) => b.clone(),
        Val::NumLit(_, digits) => digits.clone(),
        Val::Num(x) => int_digits(*x),
        Val::Fig(f) => vec![fig_byte(*f)],
        Val::All(p) => p.clone(),
    }
}

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
        Fig::Apostrophe => ebcdic::encode("'")[0],
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

    pub(super) fn range_checks(&self, cur: usize) -> bool {
        self.range_checking && self.instances[cur].code.ssrange
    }

    fn reference_slice(
        &mut self,
        cur: usize,
        total: usize,
        start: &Ex,
        length: Option<&Ex>,
    ) -> RResult<(usize, usize)> {
        let first = self.int_of(cur, start)?;
        let count = match length {
            Some(ex) => self.int_of(cur, ex)?,
            None => (total as i64).saturating_sub(first.saturating_sub(1)),
        };
        if self.range_checks(cur)
            && (first < 1
                || first > total as i64
                || count < 1
                || first
                    .checked_sub(1)
                    .and_then(|n| n.checked_add(count))
                    .is_none_or(|n| n > total as i64))
        {
            return err("SSRANGE: reference modification outside the data item");
        }
        Ok((first.max(1) as usize, count.max(0) as usize))
    }

    /// Whether a location has storage (a linkage record whose address
    /// was not set has none).
    pub(super) fn addressable(&self, cur: usize, loc: &Loc) -> bool {
        let desc = &self.instances[cur].code.descs[loc.desc as usize];
        match desc.base {
            Base::Linkage(n) => self.instances[cur].linkage[n as usize].is_some(),
            _ => true,
        }
    }

    /// Absolute address and length of a location.
    pub(super) fn addr(&mut self, cur: usize, loc: &Loc) -> RResult<(usize, usize)> {
        let code = self.instances[cur].code.clone();
        let checking = self.range_checks(cur);
        let desc = &code.descs[loc.desc as usize];
        let invocation_base = self.instances[cur].invocation_bases.get(&loc.desc).copied();
        let base = if let Some(base) = invocation_base {
            base
        } else {
            match desc.base {
                Base::Static => {
                    let ci = &self.instances[cur];
                    match (ci.local_base, ci.code.local_storage) {
                        (Some(base), Some((offset, len)))
                            if desc.offset >= offset && desc.offset < offset + len =>
                        {
                            base - offset as usize
                        }
                        _ => ci.static_base,
                    }
                }
                Base::Linkage(n) => match self.instances[cur].linkage[n as usize] {
                    Some(a) => a,
                    None => {
                        return err(format!(
                            "{}: linkage item {} is not addressable",
                            code.name, desc.name
                        ));
                    }
                },
                Base::External(k) => self.instances[cur].ext_bases[k as usize],
                Base::Outer(p) => {
                    let module = self.instances[cur].module.clone();
                    let o = self.instantiate(&module, p);
                    self.instances[o].static_base
                }
                Base::OuterLinkage(p, n) => {
                    let module = self.instances[cur].module.clone();
                    let o = self.instantiate(&module, p);
                    match self.instances[o].linkage[n as usize] {
                        Some(a) => a,
                        None => {
                            return err(format!(
                                "{}: linkage item {} is not addressable",
                                code.name, desc.name
                            ));
                        }
                    }
                }
            }
        };
        let owner = match desc.base {
            Base::Outer(p) | Base::OuterLinkage(p, _) => {
                let module = self.instances[cur].module.clone();
                self.instantiate(&module, p)
            }
            _ => cur,
        };
        let offset = if invocation_base.is_some() {
            0
        } else {
            match &desc.dynamic_offset {
                Some(e) => self.int_of(owner, e)?,
                None => desc.offset as i64,
            }
        };
        let length = match &desc.dynamic_len {
            Some(e) => self.int_of(owner, e)?,
            None => desc.len as i64,
        };
        if checking
            && (offset < 0
                || length < 0
                || length > desc.len as i64
                || (length == 0 && desc.dynamic_len.is_some()))
        {
            return err(format!(
                "SSRANGE: variable-length item {} exceeds its defined storage",
                desc.name
            ));
        }
        let mut address = base as i128 + offset.max(0) as i128;
        let mut len = length.max(0) as usize;
        for (k, dim) in desc.dims.iter().enumerate() {
            let s = match loc.subs.get(k) {
                Some(e) => self.int_of(cur, e)?,
                None => 1,
            };
            let s = if checking {
                s as i128
            } else {
                s.max(1) as i128
            };
            let stride = match &dim.dynamic_stride {
                Some(e) => self.int_of(owner, e)?.max(0) as usize,
                None => dim.stride as usize,
            };
            address += (s - 1) * stride as i128;
        }
        if checking && let Some(bounds) = &desc.table_bounds {
            let offset = match &bounds.dynamic_offset {
                Some(ex) => self.int_of(owner, ex)? as i128,
                None => bounds.offset as i128,
            };
            let start = base as i128 + offset;
            if address < start || address + len as i128 > start + bounds.len as i128 {
                return err(format!(
                    "SSRANGE: table reference {} outside its defined region",
                    desc.name
                ));
            }
        }
        let mut a = usize::try_from(address)
            .map_err(|_| super::RunError("reference outside storage".into()))?;
        if let Some(odo) = &desc.odo
            && desc.dynamic_len.is_none()
        {
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
        if checking && (len > desc.len as usize || (len == 0 && desc.odo.is_some())) {
            return err(format!(
                "SSRANGE: variable-length item {} exceeds its defined storage",
                desc.name
            ));
        }
        if let Some((start, l)) = &loc.refmod {
            // national items are positioned in characters (two bytes)
            let w = if desc.ty.is_national() { 2 } else { 1 };
            let total = len / w;
            let (s, l) = self.reference_slice(cur, total, start, l.as_deref())?;
            a += (s - 1) * w;
            len = l * w;
        }
        if a.checked_add(len).is_none_or(|end| end > self.mem.len()) {
            return err(format!(
                "{}: reference to {} out of storage",
                code.name, desc.name
            ));
        }
        Ok((a, len))
    }

    pub(super) fn loc_type(&self, cur: usize, loc: &Loc) -> DataType {
        let ty = &self.instances[cur].code.descs[loc.desc as usize].ty;
        if loc.refmod.is_some() {
            if ty.is_national() {
                DataType::National
            } else {
                DataType::Alphanumeric { just: false }
            }
        } else {
            ty.clone()
        }
    }

    // ---- evaluation ----------------------------------------------------

    pub(super) fn eval_ex(&mut self, cur: usize, e: &Ex) -> RResult<Dec> {
        if self.instances[cur].code.numeric.is_some()
            && matches!(e, Ex::Bin(..) | Ex::Neg(_) | Ex::Temp(_))
        {
            let number = self.wide_expression(
                cur,
                e,
                self.expression_decimals(cur, e),
                self.float_expression(cur, e),
            )?;
            return number
                .to_dec()
                .ok_or_else(|| super::RunError(SIZE_ERROR.into()));
        }
        Ok(match e {
            Ex::Lit(v) => *v,
            Ex::Float(text) => crate::numeric::floating_literal(text)
                .map_err(super::RunError)?
                .to_dec()
                .ok_or_else(|| {
                    super::RunError(
                        "floating value cannot be converted to this fixed-point context".into(),
                    )
                })?,
            Ex::Temp(t) => self.instances[cur].temps[*t as usize],
            Ex::Load(loc) => {
                let (a, l) = self.addr(cur, loc)?;
                let ty = self.loc_type(cur, loc);
                d::value_of(&ty, &self.mem[a..a + l])
            }
            Ex::LengthOf(loc) => {
                // LENGTH OF does not need the item to be addressable
                let l = if self.addressable(cur, loc) {
                    self.addr(cur, loc)?.1
                } else {
                    self.instances[cur].code.descs[loc.desc as usize].len as usize
                };
                Dec::from_i64(l as i64)
            }
            Ex::AddressOf(loc) => {
                let a = if self.addressable(cur, loc) {
                    self.addr(cur, loc)?.0
                } else {
                    0
                };
                Dec::from_i64(a as i64)
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
            Src::National(b) => Val::Item(DataType::National, b.clone()),
            Src::NumLit(v, digits) => Val::NumLit(*v, digits.clone()),
            Src::Float(text) => Val::Item(
                DataType::Float { long: true },
                crate::numeric::floating_literal(text)
                    .and_then(|n| {
                        n.hfp(
                            self.instances[cur]
                                .code
                                .numeric
                                .unwrap_or_default()
                                .float_width(),
                            true,
                        )
                    })
                    .map_err(super::RunError)?,
            ),
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
            Src::Num(e) => {
                if matches!(e, Ex::LengthOf(_) | Ex::AddressOf(_)) {
                    let lp64 = self.instances[cur].code.lp64;
                    let mut bytes = vec![0; if lp64 { 8 } else { 4 }];
                    binary::write(&mut bytes, self.eval_ex(cur, e)?.coef);
                    let ty = if matches!(e, Ex::AddressOf(_)) {
                        DataType::Pointer
                    } else {
                        DataType::Numeric(crate::zir::NumType {
                            usage: NumUsage::Binary,
                            digits: if lp64 { 18 } else { 9 },
                            scale: 0,
                            signed: false,
                            sign: crate::zir::SignPos::None,
                        })
                    };
                    Val::Item(ty, bytes)
                } else if self.float_expression(cur, e) {
                    let number =
                        self.wide_expression(cur, e, self.expression_decimals(cur, e), true)?;
                    Val::Item(
                        DataType::Float { long: true },
                        number
                            .hfp(
                                self.instances[cur]
                                    .code
                                    .numeric
                                    .unwrap_or_default()
                                    .float_width(),
                                false,
                            )
                            .map_err(super::RunError)?,
                    )
                } else {
                    Val::Num(self.eval_ex(cur, e)?)
                }
            }
            Src::Func(f, args, refmod) => {
                let national = self.function_national(cur, *f, args);
                let mut b = self.func_alnum(cur, *f, args)?;
                if let Some((s, l)) = refmod {
                    let (s, l) = self.reference_slice(
                        cur,
                        b.len() / if national { 2 } else { 1 },
                        s,
                        l.as_deref(),
                    )?;
                    // national results are positioned in characters
                    let (s, l) = if national { (2 * s - 1, 2 * l) } else { (s, l) };
                    b = b
                        .get(s - 1..(s - 1 + l).min(b.len()))
                        .unwrap_or(&[])
                        .to_vec();
                }
                if national {
                    Val::Item(DataType::National, b)
                } else {
                    Val::Alnum(b)
                }
            }
        })
    }

    /// An operand as national data (UTF-16): national as it is, DBCS and
    /// alphanumeric data converted from the code page.
    pub(super) fn national_bytes(&self, cur: usize, v: &Val) -> RResult<Vec<u8>> {
        Ok(match v {
            Val::Item(ty, b) if ty.is_national() => b.clone(),
            Val::Item(DataType::Dbcs, b) => d::utf16(&crate::codepage::active().decode_dbcs(b)),
            Val::Fig(f) => d::national_fig(fig_byte(*f)),
            Val::All(p) => d::national_of(p),
            _ => d::national_of(&self.alnum_bytes(cur, v, 0)?),
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
            Val::Item(DataType::Float { long }, b) => {
                let picture = if *long {
                    "-9.9999999999999999E+99"
                } else {
                    "-9.99999999E+99"
                };
                let mut out = vec![0; picture.len()];
                crate::numeric::external::encode(
                    &crate::numeric::number::Number::from_hfp(b),
                    picture,
                    false,
                    false,
                    &mut out,
                )
                .map_err(super::RunError)?;
                out
            }
            Val::Item(_, b) | Val::Alnum(b) => b.clone(),
            Val::NumLit(_, digits) => digits.clone(),
            Val::Num(x) => int_digits(*x),
            Val::Fig(f) => vec![fig_byte(*f); len.max(1)],
            Val::All(p) => {
                // An ALL literal takes the size of the other operand.
                let mut out = vec![0; if len == 0 { p.len() } else { len }];
                d::fill_pattern(&mut out, p);
                out
            }
        })
    }

    /// Bytes shown by DISPLAY.
    pub(super) fn display_bytes(&self, _cur: usize, v: &Val) -> RResult<Vec<u8>> {
        Ok(match v {
            Val::Item(ty, b) if ty.is_national() => d::display_of(b),
            Val::Item(DataType::Numeric(t), b) => match t.usage {
                NumUsage::Display => b.clone(),
                _ => {
                    let x = d::value_of(&DataType::Numeric(*t), b);
                    let raw = d::to_raw(x, t.scale, false);
                    let width = if t.usage == NumUsage::Native {
                        match b.len() {
                            2 => 5,
                            4 => 10,
                            _ => 20,
                        }
                    } else {
                        t.digits as usize
                    };
                    let mut digits = d::display_digits(raw, width);
                    if t.signed
                        && raw < 0
                        && let Some(last) = digits.last_mut()
                    {
                        *last = 0xD0 | (*last & 0xF);
                    }
                    digits
                }
            },
            Val::Item(DataType::Index, b) => int_digits(Dec::new(binary::read(b, true), 0)),
            Val::Item(DataType::Float { .. }, _) => self.alnum_bytes(_cur, v, 0)?,
            // national data is displayed in the code page
            Val::Item(DataType::National, b) => d::display_of(b),
            // DBCS data is displayed between SO and SI
            Val::Item(DataType::Dbcs, b) => {
                let mut out = vec![crate::codepage::SO];
                out.extend_from_slice(b);
                out.push(crate::codepage::SI);
                out
            }
            Val::Item(_, b) | Val::Alnum(b) => b.clone(),
            Val::NumLit(x, _) | Val::Num(x) => ebcdic::encode(&x.to_string()),
            Val::Fig(f) => vec![fig_byte(*f)],
            Val::All(p) => p.clone(),
        })
    }

    // ---- MOVE ----------------------------------------------------------

    pub(super) fn move_val(&mut self, cur: usize, v: &Val, dst: &Loc) -> RResult<()> {
        let (a, mut l) = self.addr(cur, dst)?;
        // A receiving group that contains its own OCCURS DEPENDING ON
        // object is used with its maximum length.
        let desc = self.instances[cur].code.descs[dst.desc as usize].clone();
        if let Some(odo) = &desc.odo
            && dst.refmod.is_none()
        {
            let (da, _) = self.addr(cur, &odo.depending)?;
            let max = desc.len as usize;
            if da >= a && da < a + max && a + max <= self.mem.len() {
                l = max;
            }
        }
        if let Some(length) = &desc.dynamic_len
            && dst.refmod.is_none()
        {
            fn objects(ex: &Ex, out: &mut Vec<Loc>) {
                match ex {
                    Ex::Load(loc) => out.push(loc.clone()),
                    Ex::Bin(_, a, b) => {
                        objects(a, out);
                        objects(b, out);
                    }
                    Ex::Func(_, args) => {
                        for arg in args {
                            if let crate::zir::Arg::Num(ex) = arg {
                                objects(ex, out);
                            }
                        }
                    }
                    _ => (),
                }
            }
            let mut counts = vec![];
            objects(length, &mut counts);
            let owner = match desc.base {
                Base::Outer(p) | Base::OuterLinkage(p, _) => {
                    let module = self.instances[cur].module.clone();
                    self.instantiate(&module, p)
                }
                _ => cur,
            };
            for count in counts {
                let (da, _) = self.addr(owner, &count)?;
                let max = desc.len as usize;
                if da >= a && da < a + max && a + max <= self.mem.len() {
                    l = max;
                    break;
                }
            }
        }
        let ty = self.loc_type(cur, dst);
        let mut out = self.mem[a..a + l].to_vec();
        if desc.right_justify
            && dst.refmod.is_none()
            && matches!(ty, DataType::National | DataType::Dbcs)
            && !matches!(v, Val::Fig(_) | Val::All(_))
        {
            let bytes = if ty == DataType::National {
                match v {
                    Val::Item(t, bytes) if t.is_national() => bytes.clone(),
                    Val::Item(DataType::Dbcs, bytes) => {
                        d::utf16(&crate::codepage::active().decode_dbcs(bytes))
                    }
                    _ => d::national_of(&alnum_source(v)),
                }
            } else {
                alnum_source(v)
            };
            let count = bytes.len().min(l) & !1;
            let pad = if ty == DataType::National {
                [0, 32]
            } else {
                [0x40, 0x40]
            };
            d::fill_pattern(&mut out, &pad);
            out[l - count..].copy_from_slice(&bytes[bytes.len() - count..]);
            self.mem[a..a + l].copy_from_slice(&out);
            return Ok(());
        }
        if let Val::Item(source_ty, bytes) = v
            && source_ty.is_floating()
        {
            let value = crate::numeric::number::Number::from_item(source_ty, bytes);
            value.store(&ty, &mut out, false).map_err(super::RunError)?;
            self.mem[a..a + l].copy_from_slice(&out);
            return Ok(());
        }
        if ty.is_floating() {
            let value = match v {
                Val::Item(source_ty, bytes) => {
                    crate::numeric::number::Number::from_item(source_ty, bytes)
                }
                _ => {
                    let d = self.val_num(v);
                    crate::numeric::number::Number::decimal(
                        d,
                        d.integer_digits() as i32,
                        d.scale as i32,
                    )
                }
            };
            value.store(&ty, &mut out, false).map_err(super::RunError)?;
            self.mem[a..a + l].copy_from_slice(&out);
            return Ok(());
        }
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
            DataType::NumericEdited { digits, scale, .. } => {
                let raw = d::to_raw(v, *scale, rounded);
                if raw.unsigned_abs() >= 10u128.pow((*digits).min(38)) {
                    self.size_error = true;
                    if check {
                        return Ok(());
                    }
                }
                let vv = d::scaled(raw, *scale);
                move_into(&Val::Num(vv), &ty, field);
            }
            DataType::Index => binary::write(field, d::to_raw(v, 0, rounded)),
            DataType::Float { .. } => d::store_float(field, v),
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
            DataType::Index
            | DataType::Pointer
            | DataType::ProcedurePointer
            | DataType::FunctionPointer => {
                binary::write(field, d::to_raw(v, 0, false));
            }
            DataType::Numeric(t) => {
                d::store_num(t, field, v, false);
            }
            DataType::Float { .. } => d::store_float(field, v),
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
                class_test(&v, k, self.instances[cur].code.numeric.unwrap_or_default())
            }
            Cx::Cmp(a, op, b, mode) => {
                if *mode == CmpMode::Numeric {
                    let decimals = self
                        .source_decimals(cur, a)
                        .max(self.source_decimals(cur, b));
                    let mut a = self.numeric_source(cur, a, decimals)?;
                    let mut b = self.numeric_source(cur, b, decimals)?;
                    if a.floating || b.floating {
                        let width = self.instances[cur]
                            .code
                            .numeric
                            .unwrap_or_default()
                            .float_width();
                        a = a.as_float(width, true).map_err(super::RunError)?;
                        b = b.as_float(width, true).map_err(super::RunError)?;
                    }
                    let order = a.compare(&b);
                    return Ok(match op {
                        Rel::Eq => order == Ordering::Equal,
                        Rel::Ne => order != Ordering::Equal,
                        Rel::Lt => order == Ordering::Less,
                        Rel::Le => order != Ordering::Greater,
                        Rel::Gt => order == Ordering::Greater,
                        Rel::Ge => order != Ordering::Less,
                    });
                }
                let va = self.eval_src(cur, a)?;
                let vb = self.eval_src(cur, b)?;
                let ord = match mode {
                    CmpMode::Numeric => unreachable!(),
                    // national comparison: both operands as national
                    // data, the shorter padded with national spaces
                    CmpMode::Alnum
                        if matches!(&va, Val::Item(t, _) if t.is_national())
                            || matches!(&vb, Val::Item(t, _) if t.is_national()) =>
                    {
                        let x = self.national_bytes(cur, &va)?;
                        let y = self.national_bytes(cur, &vb)?;
                        let n = x.len().max(y.len()).max(2);
                        let fit = |v: &Val, b: Vec<u8>| match v {
                            Val::Fig(_) | Val::All(_) => {
                                b.iter().copied().cycle().take(n).collect::<Vec<u8>>()
                            }
                            _ => b,
                        };
                        let mut x = fit(&va, x);
                        let mut y = fit(&vb, y);
                        for z in [&mut x, &mut y] {
                            while z.len() < n {
                                z.extend_from_slice(&d::NATIONAL_SPACE);
                            }
                        }
                        x.cmp(&y)
                    }
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
            Val::Fig(_) | Val::All(_) => 0,
            Val::Item(DataType::Numeric(t), _) => t.digits as usize,
            Val::Item(_, b) | Val::Alnum(b) => b.len(),
            Val::NumLit(_, digits) => digits.len(),
            Val::Num(x) => int_digits(*x).len(),
        }
    }

    // ---- ACCEPT and CALL arguments -------------------------------------

    /// Next line of standard input (the operator console), None at end.
    pub(super) fn stdin_line(&mut self) -> Option<String> {
        let _ = self.stdout.flush();
        if let Some(lines) = &mut self.stdin_buffer {
            return lines.pop_front();
        }
        if self.stdin_lines.is_none() {
            self.stdin_lines = Some(Box::new(std::io::stdin().lock().lines()));
        }
        self.stdin_lines
            .as_mut()
            .and_then(|l| l.next())
            .and_then(|r| r.ok())
    }

    pub(super) fn accept(&mut self, cur: usize, dst: &Loc, from: AcceptFrom) -> RResult<()> {
        let v = match from {
            AcceptFrom::Stdin => {
                let line = self.stdin_line().unwrap_or_default();
                let bytes: Vec<u8> = if crate::codepage::active().ccsid == 37 {
                    line.chars()
                        .map(|c| ebcdic::from_latin1(u32::from(c).min(255) as u8))
                        .collect()
                } else {
                    ebcdic::encode(&line)
                };
                // The data is transferred as alphanumeric data, whatever
                // the receiving item's class (national: converted).
                let national = self.instances[cur].code.descs[dst.desc as usize]
                    .ty
                    .is_national();
                let (a, l) = self.addr(cur, dst)?;
                if national {
                    d::move_national(&d::utf16(&line), &mut self.mem[a..a + l]);
                } else {
                    d::move_alnum(&bytes, &mut self.mem[a..a + l], false);
                }
                return Ok(());
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
        Ok(self
            .call_args_sized(cur, args)?
            .into_iter()
            .map(|a| a.map(|(a, _)| a))
            .collect())
    }

    /// Addresses and lengths of CALL arguments.
    pub(super) fn call_args_sized(
        &mut self,
        cur: usize,
        args: &[CallArgMode],
    ) -> RResult<Vec<Option<(usize, usize)>>> {
        let mut out = Vec::new();
        for a in args {
            out.push(match a {
                CallArgMode::Reference(loc) => Some(self.addr(cur, loc)?),
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
                    Some((self.alloc(&bytes), bytes.len()))
                }
                CallArgMode::Value(e) => {
                    if let Ex::Float(text) = e {
                        let value =
                            crate::numeric::floating_literal(text).map_err(super::RunError)?;
                        let bytes = value.hfp(8, true).map_err(super::RunError)?;
                        out.push(Some((self.alloc(&bytes), bytes.len())));
                        continue;
                    }
                    let v = self.eval_ex(cur, e)?;
                    let width = if self.instances[cur].code.lp64
                        && matches!(e, Ex::AddressOf(_) | Ex::LengthOf(_))
                    {
                        8
                    } else {
                        4
                    };
                    let mut b = vec![0u8; width];
                    binary::write(&mut b, d::to_raw(v, 0, false));
                    Some((self.alloc(&b), width))
                }
                CallArgMode::ValueData(src) => {
                    let value = self.eval_src(cur, src)?;
                    if let Val::Item(ty, bytes) = &value {
                        if !ty.by_value_allowed(bytes.len() as u32) {
                            return err("CALL BY VALUE data category or computed width is invalid");
                        }
                    } else if let Val::Alnum(bytes) = &value
                        && bytes.len() != 1
                    {
                        return err("CALL BY VALUE alphanumeric argument must have length one");
                    }
                    let bytes = match value {
                        Val::Item(_, bytes) | Val::Alnum(bytes) => bytes,
                        other => self.alnum_bytes(cur, &other, 1)?,
                    };
                    Some((self.alloc(&bytes), bytes.len()))
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

fn class_test(v: &Val, k: &ClassKind, settings: crate::numeric::Settings) -> bool {
    let bytes: &[u8] = match v {
        Val::Item(_, b) | Val::Alnum(b) => b,
        _ => return false,
    };
    match k {
        ClassKind::Numeric => match v {
            Val::Item(
                DataType::ExternalFloat {
                    picture,
                    national,
                    comma,
                },
                b,
            ) => crate::numeric::external::decode(picture, *national, *comma, b).is_ok(),
            Val::Item(DataType::Numeric(t), b) => match t.usage {
                NumUsage::National => {
                    let mut t = *t;
                    t.usage = NumUsage::Display;
                    class_test(
                        &Val::Item(DataType::Numeric(t), d::display_of(b)),
                        k,
                        settings,
                    )
                }
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
                    let mut valid = b.clone();
                    if t.signed && !pos.is_separate() && !b.is_empty() {
                        let index = if matches!(pos, zoned::SignPosition::LeadingOverpunch) {
                            0
                        } else {
                            b.len() - 1
                        };
                        let zone = b[index] >> 4;
                        let allowed = if settings.numproc == crate::numeric::Numproc::Pfd {
                            matches!(zone, 0xC | 0xD)
                                || (zone == 0xF && b.iter().all(|x| *x == 0xF0))
                        } else if settings.numcls == crate::numeric::Numcls::Alternate {
                            zone >= 0xA
                        } else {
                            matches!(zone, 0xC | 0xD | 0xF)
                        };
                        if !allowed {
                            return false;
                        }
                        valid[index] = 0xC0 | (valid[index] & 0xF);
                    }
                    zoned::is_valid(&valid, pos)
                }
                NumUsage::Packed => {
                    let sign = b.last().map_or(0, |x| x & 0xF);
                    let allowed = if !t.signed {
                        sign == 0xF
                    } else if settings.numproc == crate::numeric::Numproc::Pfd {
                        matches!(sign, 0xC | 0xD)
                            || (sign == 0xF && packed::decode_unchecked(b).0 == 0)
                    } else if settings.numcls == crate::numeric::Numcls::Alternate {
                        sign >= 0xA
                    } else {
                        matches!(sign, 0xC | 0xD | 0xF)
                    };
                    packed::is_valid(b) && allowed
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
    if matches!(ty, DataType::ExternalFloat { .. }) {
        let number = match v {
            Val::Item(source, bytes) if source.is_numeric() => {
                crate::numeric::number::Number::from_item(source, bytes)
            }
            _ => {
                let value = numeric_val(v);
                crate::numeric::number::Number::decimal(
                    value,
                    value.integer_digits() as i32,
                    value.scale as i32,
                )
            }
        };
        let _ = number.store(ty, out, false);
        return;
    }
    if let DataType::NumericEdited { national: true, .. } = ty {
        let mut display_type = ty.clone();
        if let DataType::NumericEdited { national, .. } = &mut display_type {
            *national = false;
        }
        let mut display = vec![0; out.len() / 2];
        move_into(v, &display_type, &mut display);
        out.copy_from_slice(&d::national_of(&display));
        return;
    }
    if let DataType::AlphanumericEdited {
        national: true,
        edit,
    } = ty
    {
        let src = match v {
            Val::Item(t, b) if t.is_national() => b.clone(),
            Val::Fig(f) => {
                let mut src = vec![0; out.len()];
                d::fill_pattern(&mut src, &d::national_fig(fig_byte(*f)));
                src
            }
            Val::All(b) => {
                let mut src = vec![0; out.len()];
                d::fill_pattern(&mut src, &d::national_of(b));
                src
            }
            _ => d::national_of(&alnum_source(v)),
        };
        let mut source = src.as_chunks::<2>().0.iter();
        for (sym, target) in edit.iter().zip(out.as_chunks_mut::<2>().0) {
            match sym {
                crate::zir::EditSym::Char => {
                    target.copy_from_slice(source.next().unwrap_or(&[0, 32]))
                }
                crate::zir::EditSym::Insert(b) => target.copy_from_slice(&[0, *b]),
                _ => target.copy_from_slice(&[0, 32]),
            }
        }
        return;
    }
    // Into an alphanumeric-edited item, a figurative constant or ALL
    // literal is the sending item of the receiver's size, then edited.
    if let DataType::AlphanumericEdited { edit, .. } = ty
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
    // Figurative constants and ALL literals (national receivers take
    // them as national characters).
    if *ty == DataType::National {
        match v {
            Val::Fig(f) => return d::fill_pattern(out, &d::national_fig(fig_byte(*f))),
            Val::All(p) => return d::fill_pattern(out, &d::national_of(p)),
            _ => {}
        }
    }
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
        Val::Item(
            DataType::Numeric(_)
            | DataType::Index
            | DataType::Float { .. }
            | DataType::ExternalFloat { .. },
            _,
        ) => Some(numeric_val(v)),
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
        DataType::ExternalFloat { .. } => {
            let value = numeric_src.unwrap_or_else(|| numeric_val(v));
            let number = crate::numeric::number::Number::decimal(
                value,
                value.integer_digits() as i32,
                value.scale as i32,
            );
            let _ = number.store(ty, out, false);
        }
        DataType::Numeric(t) => {
            let val = numeric_src.unwrap_or_else(|| numeric_val(v));
            d::store_num(t, out, val, false);
        }
        DataType::Index
        | DataType::Pointer
        | DataType::ProcedurePointer
        | DataType::FunctionPointer => {
            let val = numeric_src.unwrap_or_else(|| numeric_val(v));
            binary::write(out, d::to_raw(val, 0, false));
        }
        DataType::Float { .. } => {
            let val = numeric_src.unwrap_or_else(|| numeric_val(v));
            d::store_float(out, val);
        }
        DataType::NumericEdited {
            edit,
            digits,
            scale,
            blank_zero,
            currency,
            point,
            ..
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
        // padded with DBCS spaces (X'4040')
        DataType::Dbcs => {
            let bytes = alnum_source(v);
            d::move_alnum(&bytes, out, false);
        }
        // national: converted from the code page (DBCS and mixed data),
        // padded with national spaces
        DataType::National => match v {
            Val::Item(ty, b) if ty.is_national() => d::move_national(b, out),
            Val::Item(DataType::Dbcs, b) => {
                d::move_national(&d::utf16(&crate::codepage::active().decode_dbcs(b)), out)
            }
            Val::Fig(f) => d::fill_pattern(out, &d::national_fig(fig_byte(*f))),
            Val::All(p) => d::fill_pattern(out, &d::national_of(p)),
            _ => d::move_national(&d::national_of(&alnum_source(v)), out),
        },
        DataType::AlphanumericEdited { edit, .. } => {
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
        // national data moves to alphanumeric data in the code page
        Val::Item(ty, b) if ty.is_national() => d::display_of(b),
        Val::Item(_, b) | Val::Alnum(b) => b.clone(),
        Val::NumLit(_, digits) => digits.clone(),
        Val::Num(x) => int_digits(*x),
        Val::Fig(f) => vec![fig_byte(*f)],
        Val::All(p) => p.clone(),
    }
}

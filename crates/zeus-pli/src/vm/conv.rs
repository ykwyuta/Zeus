//! Conversions between data types, arithmetic and comparison.

use std::cmp::Ordering;

use super::value::{self, SPACE, Value};
use super::*;
use crate::types::{MAX_DEC, arith_bit_len, operand_arith};

fn ebcdic_text(b: &[u8]) -> String {
    zeus_cobol::ebcdic::decode(b)
}

fn ebcdic(s: &str) -> Vec<u8> {
    zeus_cobol::ebcdic::encode(s)
}

impl Machine {
    /// The arithmetic value of a value of type `from` (pictures: their
    /// numeric value; strings: the constant they hold).
    fn arith_value(&mut self, v: Value, from: &Ty, act: usize) -> R<Value> {
        match (&v, from) {
            (Value::Char(img), Ty::Pic(p)) if p.numeric => {
                let text = ebcdic_text(img);
                match p.value(&text) {
                    Some(d) => Ok(Value::fixed(d.coef, d.scale as i16, true)),
                    None => {
                        self.conversion(img, act)?;
                        Ok(Value::fixed(0, 0, true))
                    }
                }
            }
            (Value::Char(s), _) => {
                let text = ebcdic_text(s);
                match value::parse_number(&text) {
                    Some(v) => Ok(v),
                    None => {
                        self.conversion(s, act)?;
                        Ok(Value::fixed(0, 0, true))
                    }
                }
            }
            (Value::Bit(b), _) => {
                // unsigned FIXED BIN(M,0): leftmost bits beyond M dropped
                let keep = b.len().saturating_sub(64);
                Ok(Value::fixed(
                    value::bits_to_u128(&b[keep..]) as i128,
                    0,
                    false,
                ))
            }
            _ => Ok(v),
        }
    }

    /// CONVERSION: raised for an invalid character string.
    fn conversion(&mut self, src: &[u8], act: usize) -> R<()> {
        let bad = src
            .iter()
            .copied()
            .find(|c| !matches!(*c, 0xF0..=0xF9 | 0x40 | 0x4B | 0x60 | 0x4E | 0xC5))
            .unwrap_or(SPACE);
        PENDING_CONV.with(|p| *p.borrow_mut() = Some((src.to_vec(), bad)));
        let r = if self.enabled_now(act, cond::CONVERSION) {
            self.raise(Cond::Conversion, 600, act)
        } else {
            Ok(())
        };
        PENDING_CONV.with(|p| *p.borrow_mut() = None);
        r
    }

    /// High-order digits lost on assignment: SIZE when enabled.
    fn size_check(&mut self, act: usize) -> R<()> {
        if self.enabled_now(act, cond::SIZE) {
            self.raise(Cond::Size, 340, act)?;
        }
        Ok(())
    }

    /// Converts a value of type `from` to type `to` (assignment rules).
    pub fn convert(&mut self, v: Value, from: &Ty, to: &Ty, act: usize) -> R<Value> {
        match to {
            Ty::Fixed {
                dec,
                p,
                q,
                unsigned,
            } => {
                let v = self.arith_value(v, from, act)?;
                let m = match &v {
                    Value::Fixed { m, q: fq, dec: fd } => {
                        value::convert_base(*m, *fq, *fd, *q, *dec).unwrap_or(0)
                    }
                    Value::Float(f) => {
                        let b: f64 = if *dec { 10.0 } else { 2.0 };
                        let x = f * b.powi(i32::from(*q));
                        if !x.is_finite() || x.abs() > 1e37 {
                            0
                        } else {
                            x.trunc() as i128
                        }
                    }
                    Value::Ptr(p) => i128::from(*p),
                    other => return err(format!("cannot convert {other:?} to FIXED")),
                };
                let limit: i128 = if *dec {
                    10i128.pow(u32::from(*p))
                } else {
                    1i128 << (u32::from(*p) + u32::from(*unsigned))
                };
                let m = if m.abs() >= limit || (*unsigned && m < 0) {
                    self.size_check(act)?;
                    if *dec { m % limit } else { m }
                } else {
                    m
                };
                Ok(Value::fixed(m, *q, *dec))
            }
            Ty::Float { .. } => {
                let v = self.arith_value(v, from, act)?;
                Ok(Value::Float(v.as_f64()))
            }
            Ty::Char { len, var } => {
                let s: Vec<u8> = match (&v, from) {
                    (Value::Char(s), _) => s.clone(),
                    (Value::Bit(b), _) => b.iter().map(|&x| if x { 0xF1 } else { 0xF0 }).collect(),
                    (Value::Fixed { .. } | Value::Float(_), _) => {
                        ebcdic(&self.arith_text(&v, from))
                    }
                    (Value::Ptr(p), _) => ebcdic(&format!("{p:08X}")),
                    (other, _) => return err(format!("cannot convert {other:?} to CHARACTER")),
                };
                Ok(Value::Char(fit_chars(s, *len, *var)))
            }
            Ty::Bit { len, var, .. } => {
                let b: Vec<bool> = match (&v, from) {
                    (Value::Bit(b), _) => b.clone(),
                    (Value::Char(s), Ty::Pic(p)) if p.numeric => {
                        let a = self.arith_value(v.clone(), from, act)?;
                        int_bits(&a, arith_bit_len(&p.arith()))
                    }
                    (Value::Char(s), _) => {
                        let mut out = Vec::new();
                        for &c in s {
                            match c {
                                0xF0 => out.push(false),
                                0xF1 => out.push(true),
                                _ => {
                                    self.conversion(s, act)?;
                                    return Ok(Value::Bit(Vec::new()));
                                }
                            }
                        }
                        out
                    }
                    (Value::Fixed { .. } | Value::Float(_), t) => {
                        int_bits(&v, arith_bit_len(&operand_arith(t)))
                    }
                    (other, _) => return err(format!("cannot convert {other:?} to BIT")),
                };
                Ok(Value::Bit(fit_bits(b, *len, *var)))
            }
            Ty::Pic(p) => {
                if !p.numeric {
                    let s = match self.convert(v, from, &crate::sema::expr::char_any(), act)? {
                        Value::Char(s) => s,
                        _ => Vec::new(),
                    };
                    let s = fit_chars(s, Ext::Const(p.len as u32), Varying::Fixed);
                    if !p.valid_char(&ebcdic_text(&s)) {
                        self.conversion(&s, act)?;
                    }
                    return Ok(Value::Char(s));
                }
                let a = p.arith();
                let v = self.convert(v, from, &a, act)?;
                let Value::Fixed { m, q, .. } = v else {
                    return err("picture value");
                };
                let (text, lost) = p.edit(zeus_decimal::Dec::new(m, q.max(0) as u32));
                if lost {
                    self.size_check(act)?;
                }
                Ok(Value::Char(ebcdic(&text)))
            }
            // an offset from or to a pointer: through the base of its area
            Ty::Offset(Some(a)) if matches!(from, Ty::Pointer) => match v {
                Value::Ptr(p) if p == NULL || p == 0 => Ok(Value::Ptr(NULL)),
                Value::Ptr(p) => Ok(Value::Ptr(p.wrapping_sub(self.area_base(*a, act)?))),
                other => err(format!("cannot convert {other:?} to OFFSET")),
            },
            Ty::Pointer if matches!(from, Ty::Offset(Some(_))) => {
                let Ty::Offset(Some(a)) = from else {
                    unreachable!()
                };
                match v {
                    Value::Ptr(o) if o == NULL => Ok(Value::Ptr(NULL)),
                    Value::Ptr(o) => Ok(Value::Ptr(self.area_base(*a, act)?.wrapping_add(o))),
                    other => err(format!("cannot convert {other:?} to POINTER")),
                }
            }
            Ty::Area(_) => match v {
                Value::Char(b) => Ok(Value::Char(b)),
                other => err(format!("cannot convert {other:?} to AREA")),
            },
            Ty::Pointer | Ty::Offset(_) => match v {
                Value::Ptr(p) => Ok(Value::Ptr(p)),
                Value::Char(s) if s.is_empty() => Ok(Value::Ptr(NULL)),
                Value::Bit(b) if b.is_empty() => Ok(Value::Ptr(NULL)),
                other => err(format!("cannot convert {other:?} to POINTER")),
            },
            _ => Ok(v),
        }
    }

    /// Character form of an arithmetic value.
    pub fn arith_text(&self, v: &Value, from: &Ty) -> String {
        let a = operand_arith(from);
        match (v, &a) {
            (Value::Fixed { m, q, dec: true }, Ty::Fixed { p, .. }) => {
                value::fixed_dec_text(*m, *p, *q)
            }
            (Value::Fixed { m, q, dec: false }, Ty::Fixed { p, q: tq, .. }) => {
                let (dp, dq) = crate::types::bin_as_dec(*p, *tq);
                let dm = value::convert_base(*m, *q, false, dq as i16, true).unwrap_or(0);
                value::fixed_dec_text(dm, dp.clamp(1, 255) as u8, dq as i16)
            }
            (Value::Float(f), Ty::Float { dec: true, p }) => value::float_text(*f, *p),
            (Value::Float(f), Ty::Float { dec: false, p }) => value::float_text(
                *f,
                crate::types::bin_to_dec_digits(i32::from(*p)).max(1) as u8,
            ),
            (Value::Fixed { m, q, dec }, _) => {
                let (p, _) = a.precision();
                if *dec {
                    value::fixed_dec_text(*m, p.max(value::digits(*m) as u8), *q)
                } else {
                    value::fixed_dec_text(*m, 11, 0)
                }
            }
            (Value::Float(f), _) => value::float_text(*f, 6),
            _ => String::new(),
        }
    }

    /// An infix arithmetic operation with result type `rt`.
    #[allow(clippy::too_many_arguments)]
    pub fn arith(
        &mut self,
        op: ArOp,
        x: Value,
        xt: &Ty,
        y: Value,
        yt: &Ty,
        rt: &Ty,
        act: usize,
    ) -> R<Value> {
        let x = self.arith_value(x, xt, act)?;
        let y = self.arith_value(y, yt, act)?;
        match rt {
            Ty::Float { .. } => {
                let (a, b) = (x.as_f64(), y.as_f64());
                let r = match op {
                    ArOp::Add => a + b,
                    ArOp::Sub => a - b,
                    ArOp::Mul => a * b,
                    ArOp::Div => {
                        if b == 0.0 {
                            self.zero_divide(act)?;
                            return Ok(Value::Float(0.0));
                        }
                        a / b
                    }
                    ArOp::Pow => {
                        if a == 0.0 && b <= 0.0 {
                            self.raise(Cond::Error, 1550, act)?;
                            return Ok(Value::Float(0.0));
                        }
                        if matches!(y, Value::Fixed { q: 0, .. }) || b.fract() == 0.0 {
                            a.powi(b as i32)
                        } else {
                            a.powf(b)
                        }
                    }
                };
                if !r.is_finite() && self.enabled_now(act, cond::OVERFLOW) {
                    self.raise(Cond::Overflow, 300, act)?;
                }
                Ok(Value::Float(r))
            }
            Ty::Fixed { dec, p, q, .. } => {
                let (Value::Fixed { m: m1, q: q1, .. }, Value::Fixed { m: m2, q: q2, .. }) =
                    (&x, &y)
                else {
                    // operands not fixed: compute in float then convert
                    let f = Value::Float(match op {
                        ArOp::Add => x.as_f64() + y.as_f64(),
                        ArOp::Sub => x.as_f64() - y.as_f64(),
                        ArOp::Mul => x.as_f64() * y.as_f64(),
                        ArOp::Div => x.as_f64() / y.as_f64(),
                        ArOp::Pow => x.as_f64().powf(y.as_f64()),
                    });
                    return self.convert(f, &Ty::Float { dec: true, p: 16 }, rt, act);
                };
                let b: i128 = if *dec { 10 } else { 2 };
                let (m, mq): (Option<i128>, i16) = match op {
                    ArOp::Add | ArOp::Sub => {
                        let s = (*q1).max(*q2);
                        let a = value::rescale(*m1, *q1, s, *dec);
                        let c = value::rescale(*m2, *q2, s, *dec);
                        let r = match (a, c) {
                            (Some(a), Some(c)) => {
                                if op == ArOp::Add {
                                    a.checked_add(c)
                                } else {
                                    a.checked_sub(c)
                                }
                            }
                            _ => None,
                        };
                        (r, s)
                    }
                    ArOp::Mul => (m1.checked_mul(*m2), q1 + q2),
                    ArOp::Div => {
                        if *m2 == 0 {
                            self.zero_divide(act)?;
                            return Ok(Value::fixed(0, *q, *dec));
                        }
                        let e = i32::from(*q) - i32::from(*q1) + i32::from(*q2);
                        let r = if e >= 0 {
                            b.checked_pow(e as u32)
                                .and_then(|f| m1.checked_mul(f))
                                .map(|n| n / m2)
                        } else {
                            b.checked_pow((-e) as u32)
                                .and_then(|f| m2.checked_mul(f))
                                .map(|d| m1 / d)
                        };
                        (r, *q)
                    }
                    ArOp::Pow => {
                        let n = y.as_i64();
                        if n < 0 {
                            let f = x.as_f64().powi(n as i32);
                            return self.convert(
                                Value::Float(f),
                                &Ty::Float { dec: true, p: 16 },
                                rt,
                                act,
                            );
                        }
                        (m1.checked_pow(n as u32), (i64::from(*q1) * n) as i16)
                    }
                };
                let Some(m) = m else {
                    if *dec {
                        self.fofl(act)?;
                    }
                    return Ok(Value::fixed(0, *q, *dec));
                };
                let m = value::rescale(m, mq, *q, *dec).unwrap_or(0);
                // FIXEDOVERFLOW: a decimal result beyond the maximum
                // precision; SIZE: beyond the result precision
                let limit = if *dec {
                    10i128.pow(u32::from(*p))
                } else {
                    1i128 << u32::from((*p).min(126))
                };
                if m.abs() >= limit {
                    if *dec {
                        if *p >= MAX_DEC || value::digits(m) > u32::from(MAX_DEC) {
                            self.fofl(act)?;
                        } else {
                            self.size_check(act)?;
                        }
                        return Ok(Value::fixed(m % limit, *q, *dec));
                    }
                    self.size_check(act)?;
                }
                Ok(Value::fixed(m, *q, *dec))
            }
            _ => err("arithmetic with a non-arithmetic result"),
        }
    }

    fn zero_divide(&mut self, act: usize) -> R<()> {
        if self.enabled_now(act, cond::ZERODIVIDE) {
            self.raise(Cond::ZeroDivide, 320, act)?;
        }
        Ok(())
    }

    fn fofl(&mut self, act: usize) -> R<()> {
        if self.enabled_now(act, cond::FIXEDOVERFLOW) {
            self.raise(Cond::FixedOverflow, 310, act)?;
        }
        Ok(())
    }

    /// Adds two arithmetic values with the language's result attributes
    /// (DO increments).
    pub fn add_values(
        &mut self,
        a: &Value,
        at: &Ty,
        b: &Value,
        bt: &Ty,
        act: usize,
    ) -> R<(Value, Ty)> {
        let ta = operand_arith(at);
        let tb = operand_arith(bt);
        let rt = crate::types::arith_result("+", &ta, &tb);
        let v = self.arith(ArOp::Add, a.clone(), at, b.clone(), bt, &rt, act)?;
        Ok((v, rt))
    }

    pub fn cmp_arith(&mut self, a: &Value, at: &Ty, b: &Value, bt: &Ty) -> R<Ordering> {
        let x = self.arith_value(a.clone(), at, 0)?;
        let y = self.arith_value(b.clone(), bt, 0)?;
        Ok(cmp_num(&x, &y))
    }

    /// Compares two values converted to a common class by the compiler.
    pub fn compare(&mut self, a: &Value, at: &Ty, b: &Value, bt: &Ty) -> R<Ordering> {
        Ok(match (a, b) {
            (Value::Char(x), Value::Char(y)) if !(at.arith().is_some() && bt.arith().is_some()) => {
                let n = x.len().max(y.len());
                let mut x = x.clone();
                let mut y = y.clone();
                x.resize(n, SPACE);
                y.resize(n, SPACE);
                x.cmp(&y)
            }
            (Value::Bit(x), Value::Bit(y)) => {
                let n = x.len().max(y.len());
                let mut x = x.clone();
                let mut y = y.clone();
                x.resize(n, false);
                y.resize(n, false);
                x.cmp(&y)
            }
            (Value::Ptr(x), Value::Ptr(y)) => x.cmp(y),
            (Value::Label(x, _), Value::Label(y, _)) => x.cmp(y),
            (Value::Entry(x, _), Value::Entry(y, _)) => x.cmp(y),
            (Value::File(x), Value::File(y)) => x.cmp(y),
            _ => return self.cmp_arith(a, at, b, bt),
        })
    }
}

/// Exact comparison of two arithmetic values.
pub fn cmp_num(x: &Value, y: &Value) -> Ordering {
    match (x, y) {
        (
            Value::Fixed {
                m: m1,
                q: q1,
                dec: d1,
            },
            Value::Fixed {
                m: m2,
                q: q2,
                dec: d2,
            },
        ) if d1 == d2 => {
            let s = (*q1).max(*q2);
            match (
                value::rescale(*m1, *q1, s, *d1),
                value::rescale(*m2, *q2, s, *d2),
            ) {
                (Some(a), Some(b)) => a.cmp(&b),
                _ => x
                    .as_f64()
                    .partial_cmp(&y.as_f64())
                    .unwrap_or(Ordering::Equal),
            }
        }
        _ => x
            .as_f64()
            .partial_cmp(&y.as_f64())
            .unwrap_or(Ordering::Equal),
    }
}

/// The integer magnitude of an arithmetic value as `n` bits.
fn int_bits(v: &Value, n: u32) -> Vec<bool> {
    let i = v.as_f64().abs().trunc();
    let mut m: u128 = if i >= 3.4e38 { u128::MAX } else { i as u128 };
    if let Value::Fixed { m: mm, q, dec } = v {
        m = (value::rescale(*mm, *q, 0, *dec).unwrap_or(0)).unsigned_abs();
    }
    (0..n)
        .rev()
        .map(|k| if k >= 128 { false } else { m >> k & 1 == 1 })
        .collect()
}

/// A character string fitted to a length: padded with blanks or
/// truncated (VARYING: truncated to the maximum).
pub fn fit_chars(mut s: Vec<u8>, len: Ext, var: Varying) -> Vec<u8> {
    match (len, var) {
        (Ext::Const(n), Varying::Fixed) => {
            s.resize(n as usize, SPACE);
            s
        }
        (Ext::Const(n), _) => {
            s.truncate(n as usize);
            s
        }
        _ => s,
    }
}

pub fn fit_bits(mut b: Vec<bool>, len: Ext, var: Varying) -> Vec<bool> {
    match (len, var) {
        (Ext::Const(n), Varying::Fixed) => {
            b.resize(n as usize, false);
            b
        }
        (Ext::Const(n), _) => {
            b.truncate(n as usize);
            b
        }
        _ => b,
    }
}

//! DFP finite arithmetic uses exact integer ratios and DPD encoding. Quantum
//! selection is kept separate from numeric rounding, including exact division.
use super::{
    State, Value, arithmetic, cc, compare, exceptions, nan, quiet, read, signaling, write,
};
use crate::hlasm::{
    Instruction,
    float::{
        self, Ratio, Rounding,
        operations::{Decimal, Kind, check_register},
    },
};
use num_bigint::BigUint;
use num_traits::Zero;
use std::cmp::Ordering;

mod memory;
mod packed;
pub(super) use memory::execute as packed_memory;
pub(super) use packed::execute as packed;

fn power(n: u32) -> BigUint {
    BigUint::from(10u8).pow(n)
}

/// DPD steering bits choose which BCD high bits were compressed. For the 24
/// redundant encodings of three large digits, the unused p/q bits are ignored.
fn undeclet(code: u16) -> u16 {
    let b = |shift: u32| (code >> shift) & 1u16;
    let (p, q, r, s, t, u, v, w, x, y) =
        (b(9), b(8), b(7), b(6), b(5), b(4), b(3), b(2), b(1), b(0));
    let (a, d, e) = if v == 0 {
        (
            (p << 2) | (q << 1) | r,
            (s << 2) | (t << 1) | u,
            (w << 2) | (x << 1) | y,
        )
    } else {
        match (w, x) {
            (0, 0) => ((p << 2) | (q << 1) | r, (s << 2) | (t << 1) | u, 8 | y),
            (0, 1) => ((p << 2) | (q << 1) | r, 8 | u, (s << 2) | (t << 1) | y),
            (1, 0) => (8 | r, (s << 2) | (t << 1) | u, (p << 2) | (q << 1) | y),
            _ => match (s, t) {
                (1, 0) => ((p << 2) | (q << 1) | r, 8 | u, 8 | y),
                (0, 1) => (8 | r, (p << 2) | (q << 1) | u, 8 | y),
                (0, 0) => (8 | r, 8 | u, (p << 2) | (q << 1) | y),
                _ => (8 | r, 8 | u, 8 | y),
            },
        }
    };
    a * 100 + d * 10 + e
}

fn canonical_payload(raw: u128, trailing: usize) -> u128 {
    let mut result = 0;
    for at in (0..trailing / 10).rev() {
        result = (result << 10)
            | u128::from(float::declet(undeclet(((raw >> (at * 10)) & 1023) as u16)));
    }
    result
}
pub(super) fn decode(raw: u128, width: usize) -> (Value, i32) {
    let (precision, bias, expbits) = float::decimal_shape(width);
    let trailing = (precision - 1) / 3 * 10;
    let combination = (raw >> (width * 8 - 6)) & 31;
    let negative = raw >> (width * 8 - 1) != 0;
    if combination == 30 {
        return (Value::Infinity(negative), 0);
    }
    if combination == 31 {
        return (
            Value::Nan {
                negative,
                payload: canonical_payload(raw, trailing),
                fraction: trailing as u32,
                signaling: raw & (1 << (width * 8 - 7)) != 0,
            },
            0,
        );
    }
    let (high, leading) = if combination < 24 {
        (combination >> 3, combination & 7)
    } else {
        ((combination >> 1) & 3, 8 | (combination & 1))
    };
    let exponent = (((high << expbits) | ((raw >> trailing) & ((1 << expbits) - 1))) as i32) - bias;
    let mut coefficient = BigUint::from(leading);
    for at in (0..trailing / 10).rev() {
        coefficient = coefficient * 1000u16 + undeclet(((raw >> (at * 10)) & 1023) as u16);
    }
    let (n, d) = if exponent >= 0 {
        (coefficient * power(exponent as u32), BigUint::from(1u8))
    } else {
        (coefficient, power(exponent.unsigned_abs()))
    };
    (Value::Finite(Ratio { negative, n, d }), exponent)
}
fn special(value: &Value, width: usize) -> u128 {
    let sign = u128::from(super::negative(value)) << (width * 8 - 1);
    match value {
        Value::Infinity(_) => sign | (30 << (width * 8 - 6)),
        Value::Nan {
            payload, signaling, ..
        } => sign | (31 << (width * 8 - 6)) | (u128::from(*signaling) << (width * 8 - 7)) | payload,
        _ => unreachable!(),
    }
}
use crate::hlasm::float::convert::decimal_rounding as rounding;

pub(super) fn quantize(v: &Ratio, exponent: i32, mode: Rounding) -> (BigUint, bool) {
    let (n, d) = if exponent >= 0 {
        (v.n.clone(), &v.d * power(exponent as u32))
    } else {
        (&v.n * power(exponent.unsigned_abs()), v.d.clone())
    };
    float::round_quotient(&n, &d, v.negative, mode, 10)
}
pub(super) fn floor_log10(v: &Ratio) -> i32 {
    let mut e = v.n.to_str_radix(10).len() as i32 - v.d.to_str_radix(10).len() as i32;
    let order = if e >= 0 {
        v.n.cmp(&(&v.d * power(e as u32)))
    } else {
        (&v.n * power(e.unsigned_abs())).cmp(&v.d)
    };
    if order == Ordering::Less {
        e -= 1;
    }
    e
}

pub(super) fn encode(
    v: &Ratio,
    preferred: i32,
    width: usize,
    mode: Rounding,
    masks: u8,
    division: bool,
    rounded_source: Option<usize>,
) -> (u128, usize, u8, bool) {
    let (precision, bias, expbits) = float::decimal_shape(width);
    let max_exp = (3i32 << expbits) - 1 - bias;
    if v.n.is_zero() {
        let e = preferred.clamp(-bias, max_exp);
        return (
            float::pack_decimal(v.negative, &BigUint::zero(), e, width),
            width,
            0,
            e != preferred,
        );
    }
    let order = floor_log10(v);
    let tiny = order < -bias + precision as i32 - 1;
    let mut e = order - (precision as i32 - 1);
    if masks & 0x10 == 0 {
        e = e.max(-bias);
    }
    let (mut q, inexact) = quantize(v, e, mode);
    if q.to_str_radix(10).len() > precision {
        q /= 10u8;
        e += 1;
    }
    let overflow = e > max_exp;
    let mut flags = (if inexact { 8 } else { 0 })
        | (if overflow { 0x20 } else { 0 })
        | (if tiny && (inexact || masks & 0x10 != 0) {
            0x10
        } else {
            0
        });
    let increment = if inexact {
        if e >= 0 {
            &q * power(e as u32) * &v.d > v.n
        } else {
            &q * &v.d > &v.n * power(e.unsigned_abs())
        }
    } else {
        false
    };
    if increment {
        flags |= 4;
    }
    let trapped_range = flags & masks & 0x30 != 0;
    let result_width = if trapped_range {
        rounded_source.unwrap_or(width)
    } else {
        width
    };
    let mut delivered_preferred = preferred;
    let mut delivered_max = max_exp;
    if trapped_range {
        let alpha = if let Some(source) = rounded_source {
            if source == 8 { 192 } else { 3072 }
        } else if width == 8 {
            576
        } else {
            9216
        };
        let shift = if overflow { -alpha } else { alpha };
        e += shift;
        delivered_preferred += shift;
        let (_, result_bias, result_expbits) = float::decimal_shape(result_width);
        delivered_max = (3i32 << result_expbits) - 1 - result_bias;
    } else if overflow {
        flags |= 8;
    }
    if !inexact {
        while e < delivered_preferred && e < delivered_max && (&q % 10u8).is_zero() && !q.is_zero()
        {
            q /= 10u8;
            e += 1;
        }
    }
    let raw = float::encode_decimal(v.negative, &q.to_str_radix(10), e, result_width, mode);
    if flags & 8 != 0 && !trapped_range {
        let increased = match decode(raw, result_width).0 {
            Value::Infinity(_) => true,
            Value::Finite(delivered) => &delivered.n * &v.d > &v.n * &delivered.d,
            _ => false,
        };
        flags = (flags & !4) | if increased { 4 } else { 0 };
    }
    let quantum_changed = if division {
        (e == max_exp && e != preferred && (&q % 10u8).is_zero()) || (e == -bias && e > preferred)
    } else {
        e != preferred
    };
    (raw, result_width, flags, inexact || quantum_changed)
}

pub(super) fn execute(
    ins: &Instruction,
    spec: Decimal,
    state: &mut State,
    condition: &mut u8,
) -> Result<(), String> {
    if spec.kind == Kind::RoundInteger {
        return round_integer(ins, spec, state);
    }
    if spec.kind == Kind::Convert {
        return conversion(ins, spec, state);
    }
    let reg = |n: usize| -> Result<usize, String> {
        let r = ins
            .registers
            .get(n)
            .copied()
            .ok_or("missing DFP register")?;
        check_register(r, spec.width)?;
        Ok(usize::from(r))
    };
    let r = reg(0)?;
    let (b, eb) = decode(read(state, reg(1)?, spec.width), spec.width);
    if spec.kind == Kind::CompareExponent {
        let (a, ea) = decode(read(state, r, spec.width), spec.width);
        *condition = match (&a, &b) {
            (Value::Finite(_), Value::Finite(_)) => match ea.cmp(&eb) {
                Ordering::Equal => 0,
                Ordering::Less => 1,
                Ordering::Greater => 2,
            },
            (Value::Infinity(_), Value::Infinity(_)) | (Value::Nan { .. }, Value::Nan { .. }) => 0,
            _ => 3,
        };
        return Ok(());
    }
    let mode = rounding(ins.immediate & 15, state.control);
    let mut special_quantum = false;
    let (mut value, preferred, flags) = if spec.arithmetic {
        let (c, ec) = decode(read(state, reg(2)?, spec.width), spec.width);
        let preferred = match spec.kind {
            Kind::Add | Kind::Subtract => eb.min(ec),
            Kind::Multiply => eb + ec,
            _ if matches!((&b, &c), (Value::Finite(_), Value::Infinity(_))) => {
                special_quantum = true;
                -float::decimal_shape(spec.width).1
            }
            _ => eb - ec,
        };
        let (value, flags) = arithmetic(&b, &c, spec.kind, mode);
        if flags & 0x80 != 0 && !nan(&b) && !nan(&c) {
            (
                Value::Nan {
                    negative: false,
                    payload: 0,
                    fraction: 0,
                    signaling: false,
                },
                preferred,
                flags,
            )
        } else {
            (value, preferred, flags)
        }
    } else if matches!(spec.kind, Kind::Compare | Kind::SignalCompare) {
        let (a, _) = decode(read(state, r, spec.width), spec.width);
        let flags = if signaling(&a)
            || signaling(&b)
            || (spec.kind == Kind::SignalCompare && (nan(&a) || nan(&b)))
        {
            0x80
        } else {
            0
        };
        exceptions(state, flags)?;
        *condition = compare(&a, &b);
        return Ok(());
    } else {
        (quiet(&b), eb, if signaling(&b) { 0x80 } else { 0 })
    };
    if signaling(&value) {
        value = quiet(&value);
    }
    let (raw, _, encoding_flags, quantum_changed) = if let Value::Finite(v) = &value {
        encode(
            v,
            preferred,
            spec.width,
            mode,
            (state.control >> 24) as u8,
            spec.kind == Kind::Divide,
            None,
        )
    } else {
        (special(&value, spec.width), spec.width, 0, false)
    };
    let flags = flags | encoding_flags;
    if flags & ((state.control >> 24) as u8) & 0xc0 != 0 {
        return exceptions(state, flags);
    }
    write(state, r, spec.width, raw);
    if matches!(spec.kind, Kind::Add | Kind::Subtract | Kind::Test) {
        *condition = cc(&decode(raw, spec.width).0);
    }
    exceptions(state, flags)?;
    quantum_exception(
        state,
        spec.arithmetic && (quantum_changed || special_quantum),
        ins.immediate & 15,
    )
}

fn round_integer(ins: &Instruction, spec: Decimal, state: &mut State) -> Result<(), String> {
    let r = *ins.registers.first().ok_or("missing DFP destination")?;
    let source = *ins.registers.get(1).ok_or("missing DFP source")?;
    check_register(r, spec.width)?;
    check_register(source, spec.width)?;
    let (value, exponent) = decode(read(state, usize::from(source), spec.width), spec.width);
    let mode = rounding(ins.immediate & 15, state.control);
    let (raw, mut flags, changed) = match &value {
        Value::Finite(v) => {
            let (q, lost) = quantize(v, 0, mode);
            let increment = &q * &v.d > v.n;
            let rounded = Ratio {
                negative: v.negative,
                n: q,
                d: BigUint::from(1u8),
            };
            let preferred = if lost { 0 } else { exponent.max(0) };
            let (raw, _, _, _) = encode(
                &rounded,
                preferred,
                spec.width,
                Rounding::Zero,
                0,
                false,
                None,
            );
            (
                raw,
                if lost {
                    8 | if increment { 4 } else { 0 }
                } else {
                    0
                },
                preferred != exponent,
            )
        }
        _ => (
            special(&quiet(&value), spec.width),
            if signaling(&value) { 0x80 } else { 0 },
            false,
        ),
    };
    if ins.immediate & 0x40 != 0 {
        flags &= !12;
    }
    if flags & ((state.control >> 24) as u8) & 0x80 == 0 {
        write(state, usize::from(r), spec.width, raw);
    }
    exceptions(state, flags)?;
    quantum_exception(state, changed, ins.immediate & 15)
}

pub(super) fn quantum_exception(state: &mut State, changed: bool, mask: u32) -> Result<(), String> {
    if changed && (1..=7).contains(&mask) {
        if state.control & 0x04000000 != 0 {
            state.control = (state.control & !0xff00) | 0x400;
            return Err("DFP quantum exception DXC=04".into());
        }
        state.control |= 0x00040000;
    }
    Ok(())
}

pub(super) fn integer(
    value: &Ratio,
    width: usize,
    mode: Rounding,
    state: &State,
) -> (u128, u8, bool) {
    let (raw, _, flags, quantum) = encode(
        value,
        0,
        width,
        mode,
        (state.control >> 24) as u8,
        false,
        None,
    );
    (raw, flags, quantum)
}

fn conversion(ins: &Instruction, spec: Decimal, state: &mut State) -> Result<(), String> {
    let r = usize::from(*ins.registers.first().ok_or("missing DFP destination")?);
    let source = usize::from(*ins.registers.get(1).ok_or("missing DFP source")?);
    check_register(
        r as u8,
        spec.destination.max(if spec.mask { spec.width } else { 0 }),
    )?;
    check_register(source as u8, spec.width)?;
    let raw_source = read(state, source, spec.width);
    let (value, exponent) = decode(raw_source, spec.width);
    let mask = (ins.immediate >> 4) & 15;
    let xi_suppressed = mask & 8 != 0;
    let mode = rounding(ins.immediate & 15, state.control);
    let (raw, width, mut flags, quantum) = match &value {
        Value::Finite(v) => encode(
            v,
            exponent,
            spec.destination,
            mode,
            (state.control >> 24) as u8,
            false,
            spec.mask.then_some(spec.width),
        ),
        _ => {
            let trailing = (float::decimal_shape(spec.destination).0 - 1) / 3 * 10;
            let payload_mask = (1u128 << trailing) - 1;
            let converted = match &value {
                Value::Infinity(sign) => {
                    special(&value, spec.destination)
                        | if xi_suppressed {
                            canonical_payload(
                                raw_source,
                                (float::decimal_shape(spec.width).0 - 1) / 3 * 10,
                            ) & payload_mask
                        } else {
                            0
                        }
                        | (u128::from(*sign) << (spec.destination * 8 - 1))
                }
                Value::Nan {
                    negative,
                    payload,
                    signaling,
                    ..
                } => special(
                    &Value::Nan {
                        negative: *negative,
                        payload: payload & payload_mask,
                        fraction: trailing as u32,
                        signaling: *signaling && xi_suppressed,
                    },
                    spec.destination,
                ),
                _ => unreachable!(),
            };
            (
                converted,
                spec.destination,
                if signaling(&value) && !xi_suppressed {
                    0x80
                } else {
                    0
                },
                false,
            )
        }
    };
    if spec.mask && mask & 4 != 0 {
        flags &= !12;
    }
    if flags & ((state.control >> 24) as u8) & 0xc0 == 0 {
        write(state, r, width, raw);
    }
    exceptions(state, flags)?;
    quantum_exception(state, spec.mask && quantum, ins.immediate & 15)
}

pub(super) fn coefficient(raw: u128, width: usize) -> BigUint {
    let (precision, _, _) = float::decimal_shape(width);
    let trailing = (precision - 1) / 3 * 10;
    let combination = (raw >> (width * 8 - 6)) & 31;
    let leading = if combination < 24 {
        combination & 7
    } else if combination < 30 {
        8 | (combination & 1)
    } else {
        0
    };
    let mut result = BigUint::from(leading);
    for at in (0..trailing / 10).rev() {
        result = result * 1000u16 + undeclet(((raw >> (at * 10)) & 1023) as u16);
    }
    result
}

pub(super) fn bits(
    ins: &Instruction,
    spec: float::operations::DecimalBits,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    state: &mut State,
) -> Result<(), String> {
    use float::operations::DecimalBitKind;
    let reg = |n: usize| {
        ins.registers
            .get(n)
            .copied()
            .filter(|r| *r < 16)
            .map(usize::from)
            .ok_or_else(|| "invalid DFP register".to_string())
    };
    if matches!(
        spec.kind,
        DecimalBitKind::Quantize | DecimalBitKind::Reround
    ) {
        return requantize(ins, spec, low, high, state);
    }
    let r = reg(0)?;
    let s = reg(1)?;
    check_register(s as u8, spec.width)?;
    let raw = read(state, s, spec.width);
    let combination = (raw >> (spec.width * 8 - 6)) & 31;
    let (precision, bias, expbits) = float::decimal_shape(spec.width);
    let trailing = (precision - 1) / 3 * 10;
    if spec.kind != DecimalBitKind::Insert {
        let value: i64 = if combination == 30 {
            -1
        } else if combination == 31 {
            if raw & (1 << (spec.width * 8 - 7)) != 0 {
                -3
            } else {
                -2
            }
        } else if spec.kind == DecimalBitKind::Exponent {
            let high = if combination < 24 {
                combination >> 3
            } else {
                (combination >> 1) & 3
            };
            ((high << expbits) | ((raw >> trailing) & ((1 << expbits) - 1))) as i64
        } else {
            let digits = coefficient(raw, spec.width);
            if digits.is_zero() {
                0
            } else {
                digits.to_str_radix(10).len() as i64
            }
        };
        super::super::wide::write(low, high, r, value as u64);
        return Ok(());
    }
    check_register(r as u8, spec.width)?;
    let exponent = super::super::wide::read(low, high, reg(2)?) as i64;
    let sign = raw & (1 << (spec.width * 8 - 1));
    let result = if (0..(3i64 << expbits)).contains(&exponent) {
        float::pack_decimal(
            sign != 0,
            &coefficient(raw, spec.width),
            exponent as i32 - bias,
            spec.width,
        )
    } else {
        let special = if exponent == -1 {
            30 << (spec.width * 8 - 6)
        } else {
            (31 << (spec.width * 8 - 6))
                | if exponent == -3 {
                    1 << (spec.width * 8 - 7)
                } else {
                    0
                }
        };
        sign | special | canonical_payload(raw, trailing)
    };
    write(state, r, spec.width, result);
    Ok(())
}

fn requantize(
    ins: &Instruction,
    spec: float::operations::DecimalBits,
    low: &[u32; 16],
    high: &[u32; 16],
    state: &mut State,
) -> Result<(), String> {
    use float::operations::DecimalBitKind;
    let reg = |n: usize| {
        ins.registers
            .get(n)
            .copied()
            .filter(|r| *r < 16)
            .map(usize::from)
            .ok_or_else(|| "invalid DFP register".to_string())
    };
    let r = reg(0)?;
    let s = reg(1)?;
    let request = reg(2)?;
    check_register(r as u8, spec.width)?;
    check_register(s as u8, spec.width)?;
    let source = read(state, s, spec.width);
    let (c, ec) = decode(source, spec.width);
    let mode = rounding(ins.immediate & 15, state.control);
    let (precision, bias, expbits) = float::decimal_shape(spec.width);
    let max_exp = (3i32 << expbits) - 1 - bias;
    let mut invalid = false;
    let mut flags = 0;
    let mut changed = false;
    let mut requested = ec;
    let special_value = if spec.kind == DecimalBitKind::Quantize {
        check_register(request as u8, spec.width)?;
        let (b, eb) = decode(read(state, request, spec.width), spec.width);
        requested = eb;
        if nan(&b) || nan(&c) {
            let (v, f) = arithmetic(&b, &c, Kind::Add, mode);
            flags = f;
            Some(v)
        } else if matches!((&b, &c), (Value::Infinity(_), Value::Infinity(_))) {
            Some(c.clone())
        } else if matches!(&b, Value::Infinity(_)) || matches!(&c, Value::Infinity(_)) {
            invalid = true;
            None
        } else {
            None
        }
    } else if !matches!(&c, Value::Finite(_)) {
        flags = if signaling(&c) { 0x80 } else { 0 };
        Some(quiet(&c))
    } else {
        let significance = (super::super::wide::read(low, high, request) & 63) as usize;
        let digits = coefficient(source, spec.width);
        let count = if digits.is_zero() {
            0
        } else {
            digits.to_str_radix(10).len()
        };
        if significance > 0 && significance < count {
            requested = ec + (count - significance) as i32;
        }
        None
    };
    let mut raw = if let Some(value) = special_value {
        special(&value, spec.width)
    } else if let Value::Finite(value) = &c {
        let (mut q, lost) = quantize(value, requested, mode);
        if spec.kind == DecimalBitKind::Reround {
            let significance = (super::super::wide::read(low, high, request) & 63) as usize;
            if significance > 0 && q.to_str_radix(10).len() > significance && requested != ec {
                q /= 10u8;
                requested += 1;
            }
        }
        if q.to_str_radix(10).len() > precision || requested > max_exp {
            invalid = true;
        }
        if lost {
            let (n, d) = if requested >= 0 {
                (&q * power(requested as u32), BigUint::from(1u8))
            } else {
                (q.clone(), power(requested.unsigned_abs()))
            };
            flags |= 8 | if &n * &value.d > &value.n * &d { 4 } else { 0 };
        }
        changed = requested != ec;
        if invalid {
            0
        } else {
            float::pack_decimal(value.negative, &q, requested, spec.width)
        }
    } else {
        0
    };
    if invalid {
        raw = special(
            &Value::Nan {
                negative: false,
                payload: 0,
                fraction: 0,
                signaling: false,
            },
            spec.width,
        );
        flags = 0x80;
        changed = false;
    }
    if flags & ((state.control >> 24) as u8) & 0x80 == 0 {
        write(state, r, spec.width, raw);
    }
    exceptions(state, flags)?;
    quantum_exception(state, changed, ins.immediate & 15)
}

pub(super) fn classification(raw: u128, width: usize, group: bool) -> usize {
    let (precision, bias, expbits) = float::decimal_shape(width);
    let trailing = (precision - 1) / 3 * 10;
    let combination = (raw >> (width * 8 - 6)) & 31;
    if combination >= 30 {
        return if group {
            5
        } else if combination == 30 {
            3
        } else if raw & (1 << (width * 8 - 7)) != 0 {
            5
        } else {
            4
        };
    }
    let high = if combination < 24 {
        combination >> 3
    } else {
        (combination >> 1) & 3
    };
    let exponent = (((high << expbits) | ((raw >> trailing) & ((1 << expbits) - 1))) as i32) - bias;
    let coefficient = coefficient(raw, width);
    if group {
        let extreme = exponent == -bias || exponent == (3i32 << expbits) - 1 - bias;
        if coefficient.is_zero() {
            if extreme { 1 } else { 0 }
        } else if extreme {
            2
        } else if coefficient.to_str_radix(10).len() < precision {
            3
        } else {
            4
        }
    } else if coefficient.is_zero() {
        0
    } else if coefficient.to_str_radix(10).len() as i32 - 1 + exponent
        < -bias + precision as i32 - 1
    {
        1
    } else {
        2
    }
}

pub(super) fn shift(
    ins: &Instruction,
    spec: float::operations::DecimalShift,
    low: &[u32; 16],
    state: &mut State,
) -> Result<(), String> {
    let reg = |n: usize| {
        ins.registers
            .get(n)
            .copied()
            .filter(|r| *r < 16)
            .ok_or_else(|| "invalid DFP shift register".to_string())
    };
    let r = reg(0)?;
    let s = reg(1)?;
    check_register(r, spec.width)?;
    check_register(s, spec.width)?;
    let raw = read(state, usize::from(s), spec.width);
    let count =
        super::super::address(ins.address.as_ref().ok_or("missing DFP shift count")?, low)? & 63;
    let (precision, bias, expbits) = float::decimal_shape(spec.width);
    let trailing = (precision - 1) / 3 * 10;
    let combination = (raw >> (spec.width * 8 - 6)) & 31;
    let capacity = if combination >= 30 {
        precision - 1
    } else {
        precision
    };
    let mut digits = coefficient(raw, spec.width);
    if spec.left {
        digits = (digits * power(count as u32)) % power(capacity as u32);
    } else {
        digits /= power(count as u32);
    }
    let sign = raw & (1 << (spec.width * 8 - 1));
    let result = if combination >= 30 {
        let payload =
            float::pack_decimal(false, &digits, 0, spec.width) & ((1u128 << trailing) - 1);
        sign | (combination << (spec.width * 8 - 6))
            | if combination == 31 {
                raw & (1 << (spec.width * 8 - 7))
            } else {
                0
            }
            | payload
    } else {
        let high = if combination < 24 {
            combination >> 3
        } else {
            (combination >> 1) & 3
        };
        let exponent =
            (((high << expbits) | ((raw >> trailing) & ((1 << expbits) - 1))) as i32) - bias;
        float::pack_decimal(sign != 0, &digits, exponent, spec.width)
    };
    write(state, usize::from(r), spec.width, result);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dpd_canonical_cases_and_redundant_encodings() {
        let mut codes = std::collections::HashSet::new();
        for number in 0..1000 {
            let encoded = float::declet(number);
            assert!(codes.insert(encoded));
            assert_eq!(undeclet(encoded), number);
        }
        for code in 0..1024 {
            assert!(undeclet(code) < 1000);
            assert_eq!(undeclet(float::declet(undeclet(code))), undeclet(code));
        }
    }

    fn step(state: &mut State, op: &str, mask: u32) -> Result<(), String> {
        let ins:Instruction=serde_json::from_value(serde_json::json!({"offset":0,"op":op,"registers":[0,2,4],"address":null,"source":null,"length":0,"immediate":mask,"file":"test","line":1})).unwrap();
        execute(&ins, float::operations::decimal(op).unwrap(), state, &mut 0)
    }
    #[test]
    fn decimal_range_traps_store_architecture_scaled_results() {
        let mut state = State {
            control: 0x20000000,
            ..Default::default()
        };
        state.registers[2] =
            float::encode_decimal(false, "9999999999999999", 369, 8, Rounding::NearestEven) as u64;
        state.registers[4] = float::encode_decimal(false, "1", 1, 8, Rounding::NearestEven) as u64;
        assert!(step(&mut state, "MDTR", 0).unwrap_err().contains("DXC=20"));
        assert_eq!(
            state.registers[0],
            float::encode_decimal(false, "9999999999999999", -206, 8, Rounding::NearestEven) as u64
        );
        state.control = 0x10000000;
        state.registers[2] = 1;
        assert!(step(&mut state, "DDTR", 0).unwrap_err().contains("DXC=10"));
        let (value, _) = decode(state.registers[0].into(), 8);
        let Value::Finite(value) = value else {
            panic!("scaled result must be finite")
        };
        assert_eq!(value.n / value.d, power(177));
    }
    #[test]
    fn quantum_trap_completes_result_and_retains_inexact_status() {
        let mut state = State {
            control: 0x04000000,
            ..Default::default()
        };
        state.registers[2] = float::encode_decimal(false, "1", 0, 8, Rounding::NearestEven) as u64;
        state.registers[4] = float::encode_decimal(false, "3", 0, 8, Rounding::NearestEven) as u64;
        assert!(step(&mut state, "DDTRA", 4).unwrap_err().contains("DXC=04"));
        assert_eq!(
            state.registers[0],
            float::encode_decimal(false, "3333333333333333", -16, 8, Rounding::NearestEven) as u64
        );
        assert_eq!(state.control, 0x04080400);
    }
    #[test]
    fn rounded_decimal_range_traps_use_source_format() {
        let mut state = State {
            control: 0x20000000,
            ..Default::default()
        };
        state.registers[2] =
            float::encode_decimal(false, "1", 100, 8, Rounding::NearestEven) as u64;
        assert!(step(&mut state, "LEDTR", 8).unwrap_err().contains("DXC=20"));
        assert_eq!(
            state.registers[0],
            float::encode_decimal(false, "1", -92, 8, Rounding::NearestEven) as u64
        );
        state.control = 0x10000000;
        state.registers[2] =
            float::encode_decimal(false, "1", -101, 8, Rounding::NearestEven) as u64;
        assert!(step(&mut state, "LEDTR", 8).unwrap_err().contains("DXC=10"));
        assert_eq!(
            state.registers[0],
            float::encode_decimal(false, "1", 91, 8, Rounding::NearestEven) as u64
        );
    }
    #[test]
    fn decimal_conversion_invalid_suppression_and_nan_payload() {
        let mut state = State {
            control: 0x80000000,
            ..Default::default()
        };
        state.registers[0] = 0x123456789abcdef0;
        state.registers[2] = 0x7e00000000000001;
        assert!(step(&mut state, "LEDTR", 8).unwrap_err().contains("DXC=80"));
        assert_eq!(state.registers[0], 0x123456789abcdef0);
        state.control = 0x80000000;
        step(&mut state, "LEDTR", 8 | (8 << 4)).unwrap();
        assert_eq!(state.registers[0], 0x7e0000019abcdef0);
        assert_eq!(state.control, 0x80000000);
        // Lengthening preserves infinity trailing digits only when XiC=1.
        state.registers[2] = 0x7800000100000000;
        step(&mut state, "LDETR", 8 << 4).unwrap();
        assert_eq!(state.registers[0], 0x7800000000000001);
        step(&mut state, "LDETR", 0).unwrap();
        assert_eq!(state.registers[0], 0x7800000000000000);
    }
    #[test]
    fn quantize_traps_suppress_invalid_but_complete_inexact_and_quantum() {
        let ins: Instruction = serde_json::from_value(serde_json::json!({"offset":0,"op":"QADTR","registers":[0,2,4],"address":null,"source":null,"length":0,"immediate":8,"file":"test","line":1})).unwrap();
        let run = |state: &mut State, ins: &Instruction| {
            bits(
                ins,
                float::operations::decimal_bits("QADTR").unwrap(),
                &mut [0; 16],
                &mut [0; 16],
                state,
            )
        };
        let mut state = State {
            control: 0x08000000,
            ..Default::default()
        };
        state.registers[2] =
            float::encode_decimal(false, "1235", -3, 8, Rounding::NearestEven) as u64;
        state.registers[4] =
            float::encode_decimal(false, "100", -2, 8, Rounding::NearestEven) as u64;
        assert!(run(&mut state, &ins).unwrap_err().contains("DXC=0C"));
        assert_eq!(
            state.registers[0],
            float::encode_decimal(false, "124", -2, 8, Rounding::NearestEven) as u64
        );
        state.control = 0x04000000;
        state.registers[2] =
            float::encode_decimal(false, "2000", -3, 8, Rounding::NearestEven) as u64;
        let mut ins = ins;
        ins.immediate = 4;
        assert!(run(&mut state, &ins).unwrap_err().contains("DXC=04"));
        let saved = state.registers[0];
        state.control = 0x80000000;
        state.registers[4] =
            float::encode_decimal(false, "1", -30, 8, Rounding::NearestEven) as u64;
        assert!(run(&mut state, &ins).unwrap_err().contains("DXC=80"));
        assert_eq!(state.registers[0], saved);
    }

    #[test]
    fn load_integer_traps_complete_results_but_snan_trap_suppresses() {
        let mut state = State {
            control: 0x08000000,
            ..Default::default()
        };
        state.registers[2] =
            float::encode_decimal(false, "25", -1, 8, Rounding::NearestEven) as u64;
        assert!(
            step(&mut state, "FIDTR", 12)
                .unwrap_err()
                .contains("DXC=0C")
        );
        assert_eq!(
            state.registers[0],
            float::encode_decimal(false, "3", 0, 8, Rounding::NearestEven) as u64
        );
        state.control = 0x04000000;
        state.registers[2] =
            float::encode_decimal(false, "200", -2, 8, Rounding::NearestEven) as u64;
        assert!(
            step(&mut state, "FIDTR", 4 | (4 << 4))
                .unwrap_err()
                .contains("DXC=04")
        );
        assert_eq!(
            state.registers[0],
            float::encode_decimal(false, "2", 0, 8, Rounding::NearestEven) as u64
        );
        state.control = 0x80000000;
        state.registers[2] = 0x7e00000000000001;
        let saved = state.registers[0];
        assert!(
            step(&mut state, "FIDTR", 8 | (8 << 4))
                .unwrap_err()
                .contains("DXC=80")
        );
        assert_eq!(state.registers[0], saved); // M4 bit 0 is ignored by FIDTR.
    }
}

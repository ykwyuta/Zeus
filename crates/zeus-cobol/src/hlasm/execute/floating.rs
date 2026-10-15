//! Integer-based floating-point execution with shared architectural FPR/FPC state.
mod classify;
mod convert;
mod decimal;
mod divide_integer;
mod hex;
mod pfpo;
mod support;
use super::{address, range, word};
use crate::hlasm::{
    DynamicStorage, Instruction,
    float::{
        self, Ratio, Rounding,
        operations::{self, Kind},
    },
};
use num_traits::{ToPrimitive, Zero};
use std::cmp::Ordering;

#[derive(Clone, Default)]
pub(crate) struct State {
    pub registers: [u64; 16],
    pub control: u32,
    pub program_mask: u8,
}

#[derive(Clone)]
enum Value {
    Finite(Ratio),
    Infinity(bool),
    Nan {
        negative: bool,
        payload: u128,
        fraction: u32,
        signaling: bool,
    },
}
fn decode(raw: u128, width: usize) -> Value {
    let (fraction, bias) = float::binary_shape(width);
    let negative = raw >> (width * 8 - 1) != 0;
    let exponent = (raw >> fraction) & ((2 * bias + 1) as u128);
    let significand = raw & ((1u128 << fraction) - 1);
    if exponent == (2 * bias + 1) as u128 {
        if significand == 0 {
            Value::Infinity(negative)
        } else {
            Value::Nan {
                negative,
                payload: significand,
                fraction,
                signaling: significand & (1 << (fraction - 1)) == 0,
            }
        }
    } else {
        let (sig, e) = if exponent == 0 {
            (significand, 1 - bias)
        } else {
            (significand | (1 << fraction), exponent as i32 - bias)
        };
        Value::Finite(Ratio::binary(negative, sig, e - fraction as i32))
    }
}
fn cc(value: &Value) -> u8 {
    match value {
        Value::Nan { .. } => 3,
        Value::Infinity(negative) => {
            if *negative {
                1
            } else {
                2
            }
        }
        Value::Finite(v) => {
            if v.n.is_zero() {
                0
            } else if v.negative {
                1
            } else {
                2
            }
        }
    }
}
fn read(state: &State, r: usize, width: usize) -> u128 {
    match width {
        4 => u128::from(state.registers[r] >> 32),
        8 => state.registers[r].into(),
        _ => u128::from(state.registers[r]) << 64 | u128::from(state.registers[r + 2]),
    }
}
fn write(state: &mut State, r: usize, width: usize, raw: u128) {
    match width {
        4 => state.registers[r] = (state.registers[r] & 0xffff_ffff) | ((raw as u64) << 32),
        8 => state.registers[r] = raw as u64,
        _ => {
            state.registers[r] = (raw >> 64) as u64;
            state.registers[r + 2] = raw as u64;
        }
    }
}
fn mode(control: u32) -> Result<Rounding, String> {
    Ok(match control & 7 {
        0 => Rounding::NearestEven,
        1 => Rounding::Zero,
        2 => Rounding::Positive,
        3 => Rounding::Negative,
        7 => Rounding::Prepare,
        _ => return Err("invalid BFP rounding mode".into()),
    })
}
fn valid_control(value: u32) -> Result<(), String> {
    if value & !0xfcfc_ff77 != 0 || matches!(value & 7, 4..=6) {
        Err("invalid or reserved floating-point-control bits".into())
    } else {
        Ok(())
    }
}

/// Invalid/divide-by-zero traps suppress the destination. Flags are sticky only
/// for exceptions whose trap masks are clear. A trapped exception records DXC.
fn exceptions(state: &mut State, flags: u8) -> Result<(), String> {
    let masks = (state.control >> 24) as u8;
    let trapped = flags & masks & 0xf8;
    state.control |= u32::from(flags & !masks & 0xf0) << 16;
    if trapped != 0 {
        let dxc = if trapped & 0x80 != 0 {
            0x80
        } else if trapped & 0x40 != 0 {
            0x40
        } else if trapped & 0x20 != 0 {
            0x20
        } else if trapped & 0x10 != 0 {
            0x10
        } else {
            0x08
        } | u32::from(flags & 0x0c);
        state.control = (state.control & !0xff00) | (dxc << 8);
        return Err(format!("IEEE floating-point data exception DXC={dxc:02X}"));
    }
    state.control |= u32::from(flags & 0xf8) << 16;
    Ok(())
}
fn default_nan() -> Value {
    Value::Nan {
        negative: false,
        payload: 1,
        fraction: 1,
        signaling: false,
    }
}
fn nan(value: &Value) -> bool {
    matches!(value, Value::Nan { .. })
}
fn zero(value: &Value) -> bool {
    matches!(value,Value::Finite(v) if v.n.is_zero())
}
fn negative(value: &Value) -> bool {
    match value {
        Value::Finite(v) => v.negative,
        Value::Infinity(n) => *n,
        Value::Nan { negative, .. } => *negative,
    }
}
fn negate(value: &mut Value) {
    match value {
        Value::Finite(v) => v.negative = !v.negative,
        Value::Infinity(n) => *n = !*n,
        Value::Nan { negative, .. } => *negative = !*negative,
    }
}
fn quiet(value: &Value) -> Value {
    let mut value = value.clone();
    if let Value::Nan { signaling, .. } = &mut value {
        *signaling = false;
    }
    value
}
fn signaling(value: &Value) -> bool {
    matches!(
        value,
        Value::Nan {
            signaling: true,
            ..
        }
    )
}
fn encode(value: &Value, width: usize, rounding: Rounding) -> (u128, u8) {
    let (fraction, bias) = float::binary_shape(width);
    let sign = u128::from(negative(value)) << (width * 8 - 1);
    match value {
        Value::Infinity(_) => (sign | ((2 * bias + 1) as u128) << fraction, 0),
        Value::Nan {
            payload,
            fraction: source,
            signaling,
            ..
        } => {
            let payload = if *source > fraction {
                payload >> (source - fraction)
            } else {
                payload << (fraction - source)
            };
            let payload = if *signaling {
                (payload & !(1 << (fraction - 1))).max(1)
            } else {
                payload | (1 << (fraction - 1))
            };
            (sign | ((2 * bias + 1) as u128) << fraction | payload, 0)
        }
        Value::Finite(v) => {
            let (raw, flags) = float::encode_binary(v, width, fraction, rounding);
            (
                raw,
                (u8::from(flags.overflow) * 0x20)
                    | (u8::from(flags.underflow) * 0x10)
                    | (u8::from(flags.inexact) * 8),
            )
        }
    }
}
fn compare(a: &Value, b: &Value) -> u8 {
    if nan(a) || nan(b) {
        return 3;
    }
    if zero(a) && zero(b) {
        return 0;
    }
    if negative(a) != negative(b) {
        return if negative(a) { 1 } else { 2 };
    }
    let order = match (a, b) {
        (Value::Infinity(_), Value::Infinity(_)) => Ordering::Equal,
        (Value::Infinity(_), _) => Ordering::Greater,
        (_, Value::Infinity(_)) => Ordering::Less,
        (Value::Finite(a), Value::Finite(b)) => (&a.n * &b.d).cmp(&(&b.n * &a.d)),
        _ => unreachable!(),
    };
    let order = if negative(a) { order.reverse() } else { order };
    match order {
        Ordering::Equal => 0,
        Ordering::Less => 1,
        Ordering::Greater => 2,
    }
}
fn arithmetic(a: &Value, b: &Value, kind: Kind, rounding: Rounding) -> (Value, u8) {
    if signaling(a) || signaling(b) {
        return (quiet(if signaling(a) { a } else { b }), 0x80);
    }
    if nan(a) || nan(b) {
        return (quiet(if nan(a) { a } else { b }), 0);
    }
    let mut b = b.clone();
    if kind == Kind::Subtract {
        negate(&mut b);
    }
    let sign = negative(a) ^ negative(&b);
    match kind {
        Kind::Add | Kind::Subtract => match (a, &b) {
            (Value::Infinity(x), Value::Infinity(y)) if x != y => (default_nan(), 0x80),
            (Value::Infinity(_), _) => (a.clone(), 0),
            (_, Value::Infinity(_)) => (b, 0),
            (Value::Finite(a), Value::Finite(b)) => (
                Value::Finite(a.add(b, matches!(rounding, Rounding::Negative))),
                0,
            ),
            _ => unreachable!(),
        },
        Kind::Multiply => {
            if (matches!(a, Value::Infinity(_)) && zero(&b))
                || (matches!(b, Value::Infinity(_)) && zero(a))
            {
                return (default_nan(), 0x80);
            }
            if matches!(a, Value::Infinity(_)) || matches!(b, Value::Infinity(_)) {
                return (Value::Infinity(sign), 0);
            }
            if let (Value::Finite(a), Value::Finite(b)) = (a, b) {
                (Value::Finite(a.multiply(&b)), 0)
            } else {
                unreachable!()
            }
        }
        Kind::Divide => {
            if (matches!(a, Value::Infinity(_)) && matches!(b, Value::Infinity(_)))
                || (zero(a) && zero(&b))
            {
                return (default_nan(), 0x80);
            }
            if matches!(a, Value::Infinity(_)) {
                return (Value::Infinity(sign), 0);
            }
            if matches!(b, Value::Infinity(_)) {
                return (Value::Finite(Ratio::binary(sign, 0, 0)), 0);
            }
            if zero(&b) {
                return (Value::Infinity(sign), 0x40);
            }
            if let (Value::Finite(a), Value::Finite(b)) = (a, b) {
                (Value::Finite(a.divide(&b)), 0)
            } else {
                unreachable!()
            }
        }
        _ => unreachable!(),
    }
}
fn sqrt(value: &Value, width: usize, rounding: Rounding) -> (u128, u8) {
    if signaling(value) {
        let (r, _) = encode(&quiet(value), width, rounding);
        return (r, 0x80);
    }
    if nan(value) || zero(value) || matches!(value, Value::Infinity(false)) {
        return encode(value, width, rounding);
    }
    if negative(value) {
        return (encode(&default_nan(), width, rounding).0, 0x80);
    }
    let Value::Finite(v) = value else {
        unreachable!()
    };
    let (precision, bias) = float::binary_shape(width);
    let e = v.floor_log2().div_euclid(2).max(1 - bias);
    let quantum = e - precision as i32;
    let (n, d) = if quantum < 0 {
        (&v.n << (-2 * quantum) as usize, v.d.clone())
    } else {
        (v.n.clone(), &v.d << (2 * quantum) as usize)
    };
    let mut q = (&n / &d).sqrt();
    let inexact = &q * &q * &d != n;
    let mid = ((&q << 1usize) + 1u8).pow(2) * &d;
    let half = (&n << 2usize).cmp(&mid);
    let increase = inexact
        && match rounding {
            Rounding::NearestEven => {
                half == Ordering::Greater || (half == Ordering::Equal && q.bit(0))
            }
            Rounding::NearestAway => half != Ordering::Less,
            Rounding::Positive | Rounding::Away => true,
            Rounding::NearestZero => half == Ordering::Greater,
            Rounding::Prepare => !q.bit(0),
            _ => false,
        };
    if increase {
        q += 1u8;
    }
    let ratio = Ratio::binary(false, q.to_u128().unwrap(), quantum);
    (
        float::encode_binary(&ratio, width, precision, Rounding::Zero).0,
        if inexact {
            8 | if increase { 4 } else { 0 }
        } else {
            0
        },
    )
}

pub(super) fn execute(
    ins: &Instruction,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    mem: &mut [u8],
    storage: &DynamicStorage,
    state: &mut State,
    condition: &mut u8,
) -> Result<bool, String> {
    let op = ins.op.as_str();
    if op == "PFPO" {
        pfpo::execute(low, state, condition)?;
        return Ok(true);
    }
    if operations::size(op).is_none() {
        return Ok(false);
    }
    if let Some((_, width)) = operations::divide_integer(op) {
        divide_integer::execute(ins, width, state, condition)?;
        return Ok(true);
    }
    if let Some(spec) = operations::hex_unnormalized(op) {
        hex::unnormalized(ins, spec, low, mem, state)?;
        return Ok(true);
    }
    if let Some(spec) = operations::classify(op) {
        classify::execute(ins, spec, low, state, condition)?;
        return Ok(true);
    }
    if let Some(spec) = operations::decimal_shift(op) {
        decimal::shift(ins, spec, low, state)?;
        return Ok(true);
    }
    if let Some(spec) = operations::support(op) {
        support::execute(ins, spec, state, condition)?;
        return Ok(true);
    }
    if let Some(spec) = operations::decimal_memory(op) {
        decimal::packed_memory(ins, spec, low, mem, storage, state, condition)?;
        return Ok(true);
    }
    if let Some(spec) = operations::decimal_packed(op) {
        decimal::packed(ins, spec, low, high, state)?;
        return Ok(true);
    }
    if let Some(spec) = operations::decimal_bits(op) {
        decimal::bits(ins, spec, low, high, state)?;
        return Ok(true);
    }
    if let Some(spec) = operations::decimal(op) {
        decimal::execute(ins, spec, state, condition)?;
        return Ok(true);
    }
    if let Some(spec) = float::convert::instruction(op) {
        convert::execute(ins, spec, low, high, state, condition)?;
        return Ok(true);
    }
    if let Some(spec) = operations::hex(op) {
        hex::execute(ins, spec, low, mem, storage, state, condition)?;
        return Ok(true);
    }
    let reg = |i: usize| -> Result<usize, String> {
        ins.registers
            .get(i)
            .copied()
            .filter(|r| *r < 16)
            .map(usize::from)
            .ok_or_else(|| "invalid floating-point register".into())
    };
    let ea = || {
        address(
            ins.address
                .as_ref()
                .ok_or("missing floating-point address")?,
            low,
        )
    };
    match op {
        "LPDFR" | "LNDFR" | "LCDFR" => {
            let (r, s) = (reg(0)?, reg(1)?);
            let value = state.registers[s];
            state.registers[r] = match op {
                "LPDFR" => value & !(1 << 63),
                "LNDFR" => value | (1 << 63),
                _ => value ^ (1 << 63),
            };
            return Ok(true);
        }
        "LDGR" => {
            state.registers[reg(0)?] = super::wide::read(low, high, reg(1)?);
            return Ok(true);
        }
        "LGDR" => {
            super::wide::write(low, high, reg(0)?, state.registers[reg(1)?]);
            return Ok(true);
        }
        "SRNMT" => {
            let mode = ea()? as u32 & 7;
            state.control = (state.control & !0x70) | (mode << 4);
            return Ok(true);
        }
        "SFPC" | "LFPC" => {
            let value = if op == "SFPC" {
                low[reg(0)?]
            } else {
                word(mem, ea()?)?
            };
            valid_control(value)?;
            state.control = value;
            return Ok(true);
        }
        "EFPC" => {
            low[reg(0)?] = state.control;
            return Ok(true);
        }
        "STFPC" => {
            let at = ea()?;
            let area = range(mem, at, 4)?;
            storage.check_write(at, 4)?;
            mem[area].copy_from_slice(&state.control.to_be_bytes());
            return Ok(true);
        }
        "SRNM" | "SRNMB" => {
            let mode = (ea()? as u32) & if op == "SRNM" { 3 } else { 7 };
            if matches!(mode, 4..=6) {
                return Err("invalid BFP rounding mode".into());
            }
            state.control = (state.control & !7) | mode;
            return Ok(true);
        }
        "LER" | "LDR" | "LXR" => {
            let width = if op == "LER" {
                4
            } else if op == "LDR" {
                8
            } else {
                16
            };
            let (r, s) = (reg(0)?, reg(1)?);
            operations::check_register(r as u8, width)?;
            operations::check_register(s as u8, width)?;
            let raw = read(state, s, width);
            write(state, r, width, raw);
            return Ok(true);
        }
        "LE" | "LD" | "STE" | "STD" => {
            let width = if matches!(op, "LE" | "STE") { 4 } else { 8 };
            let at = ea()?;
            let area = range(mem, at, width)?;
            let r = reg(0)?;
            if op.starts_with('S') {
                storage.check_write(at, width)?;
                let raw = read(state, r, width).to_be_bytes();
                mem[area].copy_from_slice(&raw[16 - width..]);
            } else {
                let mut raw = [0u8; 16];
                raw[16 - width..].copy_from_slice(&mem[area]);
                write(state, r, width, u128::from_be_bytes(raw));
            }
            return Ok(true);
        }
        _ => {}
    }
    let (dest_width, src_width, kind, memory) = if let Some((_, d, s, k)) = operations::register(op)
    {
        (d, s, k, false)
    } else {
        let (_, w, k) = operations::memory(op).unwrap();
        (operations::memory_destination(op).unwrap(), w, k, true)
    };
    let r = reg(0)?;
    operations::check_register(
        r as u8,
        if kind == Kind::Convert {
            dest_width.max(src_width)
        } else {
            dest_width
        },
    )?;
    if kind == Kind::Zero {
        write(state, r, dest_width, 0);
        return Ok(true);
    }
    let source = if memory {
        let area = range(mem, ea()?, src_width)?;
        let mut bytes = [0u8; 16];
        bytes[16 - src_width..].copy_from_slice(&mem[area]);
        u128::from_be_bytes(bytes)
    } else {
        let s = reg(1)?;
        operations::check_register(s as u8, src_width)?;
        read(state, s, src_width)
    };
    let first_width = if kind == Kind::Multiply && dest_width > src_width {
        src_width
    } else {
        dest_width
    };
    let a = decode(read(state, r, first_width), first_width);
    let mut b = decode(source, src_width);
    let rounding = if operations::binary_controlled(op) {
        float::convert::validate_binary_masks(ins.immediate)?;
        float::convert::rounding(ins.immediate & 15, state.control, float::Format::Binary)?
    } else {
        mode(state.control)?
    };
    if matches!(kind, Kind::Positive | Kind::Negative | Kind::Complement) {
        let sign = 1u128 << (dest_width * 8 - 1);
        let raw = match kind {
            Kind::Positive => source & !sign,
            Kind::Negative => source | sign,
            _ => source ^ sign,
        };
        write(state, r, dest_width, raw);
        *condition = cc(&decode(raw, dest_width));
        return Ok(true);
    }
    if matches!(kind, Kind::Compare | Kind::SignalCompare) {
        let invalid =
            signaling(&a) || signaling(&b) || (kind == Kind::SignalCompare && (nan(&a) || nan(&b)));
        exceptions(state, if invalid { 0x80 } else { 0 })?;
        *condition = compare(&a, &b);
        return Ok(true);
    }
    let mut result_width = dest_width;
    let (result, mut flags) = if kind == Kind::Sqrt {
        sqrt(&b, dest_width, rounding)
    } else {
        let (value, flags) = if matches!(kind, Kind::FusedAdd | Kind::FusedSubtract) {
            let operand = if memory {
                decode(read(state, reg(1)?, src_width), src_width)
            } else {
                decode(read(state, reg(2)?, src_width), src_width)
            };
            let (product, multiply_flags) = arithmetic(&b, &operand, Kind::Multiply, rounding);
            let (sum, add_flags) = arithmetic(
                &product,
                &a,
                if kind == Kind::FusedSubtract {
                    Kind::Subtract
                } else {
                    Kind::Add
                },
                rounding,
            );
            (sum, multiply_flags | add_flags)
        } else if kind == Kind::RoundInteger {
            if let Value::Finite(v) = &b {
                let (q, lost) = v.quantize(0, rounding);
                let increased = &q * &v.d > v.n;
                (
                    Value::Finite(Ratio {
                        negative: v.negative,
                        n: q,
                        d: num_bigint::BigUint::from(1u8),
                    }),
                    if lost {
                        8 | if increased { 4 } else { 0 }
                    } else {
                        0
                    },
                )
            } else {
                (quiet(&b), if signaling(&b) { 0x80 } else { 0 })
            }
        } else if matches!(kind, Kind::Convert | Kind::Test) {
            let flags = if signaling(&b) { 0x80 } else { 0 };
            b = quiet(&b);
            (b, flags)
        } else {
            arithmetic(&a, &b, kind, rounding)
        };
        let (mut raw, mut encoding_flags) = encode(&value, dest_width, rounding);
        if let Value::Finite(v) = &value
            && !v.n.is_zero()
        {
            let (precision, bias) = float::binary_shape(dest_width);
            let e = v.floor_log2();
            let mask = (state.control >> 24) as u8;
            if e < 1 - bias && mask & 0x10 != 0 {
                encoding_flags |= 0x10;
            }
            if encoding_flags & mask & 0x30 != 0 {
                let rounded = kind == Kind::Convert && src_width > dest_width;
                let alpha = if rounded {
                    if src_width == 8 { 512 } else { 8192 }
                } else {
                    match dest_width {
                        4 => 192,
                        8 => 1536,
                        _ => 24576,
                    }
                };
                let exponent = e - precision as i32;
                let (q, inexact) = v.quantize(exponent, rounding);
                let mut scaled = Ratio::binary(v.negative, q.to_u128().unwrap(), exponent);
                if encoding_flags & 0x20 != 0 {
                    scaled.d <<= alpha as usize;
                } else {
                    scaled.n <<= alpha as usize;
                }
                result_width = if rounded { src_width } else { dest_width };
                raw = float::encode_binary(&scaled, result_width, precision, Rounding::Zero).0;
                encoding_flags = (encoding_flags & 0x30) | if inexact { 8 } else { 0 };
            }
            if encoding_flags & 8 != 0 {
                // DXC distinguishes a truncated magnitude from an incremented
                // one; the inexact status flag itself has no such distinction.
                let q = if encoding_flags & mask & 0x30 != 0 {
                    let exponent = e - precision as i32;
                    let (q, _) = v.quantize(exponent, rounding);
                    Ratio::binary(false, q.to_u128().unwrap(), exponent)
                } else if let Value::Finite(q) = decode(raw, dest_width) {
                    q
                } else {
                    Ratio::binary(false, 1, 2 * bias)
                };
                if &q.n * &v.d > &v.n * &q.d {
                    encoding_flags |= 4;
                }
            }
        }
        (raw, flags | encoding_flags)
    };
    if operations::binary_controlled(op) && ins.immediate & 0x40 != 0 {
        flags &= !12;
    }
    let suppress = flags & ((state.control >> 24) as u8) & 0xc0 != 0;
    if !suppress {
        operations::check_register(r as u8, result_width)?;
        write(state, r, result_width, result);
        if matches!(kind, Kind::Add | Kind::Subtract | Kind::Test) {
            *condition = cc(&decode(result, result_width));
        }
    }
    exceptions(state, flags)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn step(op: &str, state: &mut State) -> Result<bool, String> {
        let ins: Instruction = serde_json::from_value(serde_json::json!({
            "offset":0,"op":op,"registers":[0,1],"address":null,"source":null,
            "length":0,"immediate":0,"file":"test","line":1
        }))
        .unwrap();
        execute(
            &ins,
            &mut [0; 16],
            &mut [0; 16],
            &mut [],
            &DynamicStorage::default(),
            state,
            &mut 0,
        )
    }
    #[test]
    fn overflow_underflow_traps_store_scaled_results_and_dxc() {
        let mut state = State {
            control: 0x20000000,
            ..Default::default()
        };
        state.registers[0] = 0x7f00000000000000;
        state.registers[1] = 0x4000000000000000;
        assert!(step("MEEBR", &mut state).unwrap_err().contains("DXC=20"));
        assert_eq!(state.registers[0], 0x1f80000000000000);
        assert_eq!(state.control, 0x20002000);
        state.control = 0x10000000;
        state.registers[0] = 0x0080000000000000;
        assert!(step("DEBR", &mut state).unwrap_err().contains("DXC=10"));
        assert_eq!(state.registers[0], 0x6000000000000000);
        assert_eq!(state.control, 0x10001000);
    }
    #[test]
    fn inexact_trap_completes_result_and_distinguishes_increment() {
        let mut state = State {
            control: 0x08000000,
            ..Default::default()
        };
        state.registers[0] = 0x3f80000000000000;
        state.registers[1] = 0x4040000000000000;
        assert!(step("DEBR", &mut state).unwrap_err().contains("DXC=0C"));
        assert_eq!(state.registers[0], 0x3eaaaaab00000000);
        assert_eq!(state.control, 0x08000c00);
    }
    #[test]
    fn binary_square_root_reports_incremented_inexact_dxc() {
        let mut state = State {
            control: 0x08000000,
            ..Default::default()
        };
        state.registers[1] = 0x40a0000000000000;
        assert!(step("SQEBR", &mut state).unwrap_err().contains("DXC=0C"));
        assert_eq!(state.registers[0], 0x400f1bbd00000000);
    }
    #[test]
    fn load_integer_inexact_completes_and_snan_invalid_suppresses() {
        let ins:Instruction=serde_json::from_value(serde_json::json!({"offset":0,"op":"FIEBR","registers":[0,1],"address":null,"source":null,"length":0,"immediate":4,"file":"test","line":1})).unwrap();
        let run = |state: &mut State| {
            execute(
                &ins,
                &mut [0; 16],
                &mut [0; 16],
                &mut [],
                &DynamicStorage::default(),
                state,
                &mut 0,
            )
        };
        let mut state = State {
            control: 0x08000000,
            ..Default::default()
        };
        state.registers[1] = 0x3fc0000000000000;
        assert!(run(&mut state).unwrap_err().contains("DXC=0C"));
        assert_eq!(state.registers[0], 0x4000000000000000);
        state.control = 0x80000000;
        state.registers[1] = 0x7fa0000000000000;
        assert!(run(&mut state).unwrap_err().contains("DXC=80"));
        assert_eq!(state.registers[0], 0x4000000000000000);
    }
    #[test]
    fn rounded_overflow_uses_source_width_for_scaled_trap_result() {
        let mut state = State {
            control: 0x20000000,
            ..Default::default()
        };
        state.registers[1] = 0x47f0000000000000;
        assert!(step("LEDBR", &mut state).unwrap_err().contains("DXC=20"));
        assert_eq!(state.registers[0], 0x27f0000000000000);
    }
}

#[cfg(test)]
mod hfp_mask_tests {
    use super::*;
    #[test]
    fn hfp_interrupts_complete_register_and_condition_code() {
        let instruction = |op: &str| -> Instruction {
            serde_json::from_value(serde_json::json!({
                "offset":0,"op":op,"registers":[0,1],"address":null,"source":null,
                "length":0,"immediate":0,"file":"test","line":1
            }))
            .unwrap()
        };
        let mut state = State {
            program_mask: 2,
            ..Default::default()
        };
        state.registers[0] = 0x0010000000000000;
        state.registers[1] = 0x4210000000000000;
        let mut cc = 2;
        assert!(
            execute(
                &instruction("DER"),
                &mut [0; 16],
                &mut [0; 16],
                &mut [],
                &DynamicStorage::default(),
                &mut state,
                &mut cc
            )
            .unwrap_err()
            .contains("exponent-underflow")
        );
        assert_eq!(state.registers[0], 0x7f10000000000000);
        assert_eq!(cc, 2); // Divide leaves CC unchanged.
        state.program_mask = 1;
        state.registers[0] = 0x4110000000000000;
        state.registers[1] = state.registers[0];
        assert!(
            execute(
                &instruction("SER"),
                &mut [0; 16],
                &mut [0; 16],
                &mut [],
                &DynamicStorage::default(),
                &mut state,
                &mut cc
            )
            .unwrap_err()
            .contains("significance")
        );
        assert_eq!(state.registers[0], 0x4100000000000000);
        assert_eq!(cc, 0);
    }
}

#[cfg(test)]
mod binary_modifier_tests {
    use super::*;
    #[test]
    fn reserved_bfp_modifiers_are_rejected_before_any_architectural_state_changes() {
        for op in ["FIEBR", "LEDBR", "CFDBR", "CDFBR"] {
            for immediate in [2, 8, 0x10, 0x80, 0x100] {
                let ins = serde_json::from_value(serde_json::json!({
                    "offset":0,"op":op,"registers":[0,1],"address":null,"source":null,
                    "length":0,"immediate":immediate,"file":"own-test","line":1
                }))
                .unwrap();
                let mut state = State {
                    control: 0x08000000,
                    ..Default::default()
                };
                state.registers[0] = 0x4000000011223344;
                state.registers[1] = 0x3ff8000055667788;
                let before = state.registers;
                let mut cc = 2;
                let mut low = [17; 16];
                let mut high = [19; 16];
                assert!(
                    execute(
                        &ins,
                        &mut low,
                        &mut high,
                        &mut [],
                        &DynamicStorage::default(),
                        &mut state,
                        &mut cc
                    )
                    .is_err(),
                    "{op} {immediate}"
                );
                assert_eq!(state.registers, before);
                assert_eq!(state.control, 0x08000000);
                assert_eq!(low, [17; 16]);
                assert_eq!(high, [19; 16]);
                assert_eq!(cc, 2);
            }
        }
    }
}

use super::{State, Value, cc, decimal, decode, encode, exceptions, hex, read, write};
use crate::hlasm::{
    Instruction,
    float::{
        Format, Ratio, Rounding,
        convert::{Conversion, rounding},
        operations::check_register,
    },
};
use num_bigint::BigUint;
use num_traits::ToPrimitive;

pub(super) fn execute(
    ins: &Instruction,
    spec: Conversion,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    state: &mut State,
    condition: &mut u8,
) -> Result<(), String> {
    let reg = |n: usize| -> Result<usize, String> {
        ins.registers
            .get(n)
            .copied()
            .filter(|r| *r < 16)
            .map(usize::from)
            .ok_or_else(|| "invalid conversion register".into())
    };
    let (r, s) = (reg(0)?, reg(1)?);
    check_register(if spec.to_integer { s } else { r } as u8, spec.width)?;
    if spec.format == Format::Binary {
        crate::hlasm::float::convert::validate_binary_masks(ins.immediate)?;
    }
    let rounding = rounding(ins.immediate & 15, state.control, spec.format)?;
    let suppress_inexact = ins.immediate & 0x40 != 0;
    if !spec.to_integer {
        let raw = if spec.bits == 32 {
            u64::from(low[s])
        } else {
            super::super::wide::read(low, high, s)
        };
        let (negative, magnitude) = if spec.logical {
            (false, raw)
        } else {
            let value = if spec.bits == 32 {
                i64::from(raw as i32)
            } else {
                raw as i64
            };
            (value < 0, value.unsigned_abs())
        };
        let value = Ratio::binary(negative, u128::from(magnitude), 0);
        let mut quantum = false;
        let (raw, mut flags) = if spec.format == Format::Decimal {
            let (raw, flags, changed) = decimal::integer(&value, spec.width, rounding, state);
            quantum = changed;
            (raw, flags)
        } else if spec.format == Format::Hex {
            (hex::pack(&value, spec.width, None, Rounding::Zero, 0).0, 0)
        } else {
            encode(&Value::Finite(value.clone()), spec.width, rounding)
        };
        if spec.format == Format::Binary
            && flags & 8 != 0
            && let Value::Finite(q) = decode(raw, spec.width)
            && &q.n * &value.d > &value.n * &q.d
        {
            flags |= 4;
        }
        if suppress_inexact {
            flags &= !12;
        }
        write(state, r, spec.width, raw);
        exceptions(state, flags)?;
        if spec.format == Format::Decimal {
            return decimal::quantum_exception(state, quantum, ins.immediate & 15);
        }
        return Ok(());
    }
    let raw = read(state, s, spec.width);
    let value = if spec.format == Format::Decimal {
        decimal::decode(raw, spec.width).0
    } else if spec.format == Format::Hex {
        Value::Finite(hex::decode(raw, spec.width).0)
    } else {
        decode(raw, spec.width)
    };
    let limit = if spec.logical {
        (1u128 << spec.bits) - 1
    } else {
        (1u128 << (spec.bits - 1)) - 1
    };
    let min = if spec.logical {
        0
    } else {
        1u128 << (spec.bits - 1)
    };
    let mut inexact = false;
    let mut increment = false;
    let (magnitude, negative, invalid) = match &value {
        Value::Finite(v) => {
            let (q, lost) = if spec.format == Format::Decimal {
                decimal::quantize(v, 0, rounding)
            } else {
                v.quantize(0, rounding)
            };
            inexact = lost;
            increment = &q * &v.d > v.n;
            let bound = if v.negative { min } else { limit };
            let invalid = q > BigUint::from(bound);
            let q = if invalid { bound } else { q.to_u128().unwrap() };
            (q, v.negative, invalid)
        }
        Value::Infinity(negative) => (if *negative { min } else { limit }, *negative, true),
        Value::Nan { .. } => (min, !spec.logical, true),
    };
    if invalid && spec.format != Format::Hex && state.control & 0x80000000 != 0 {
        return exceptions(state, 0x80);
    }
    let value_bits = if negative {
        (magnitude as u64).wrapping_neg()
    } else {
        magnitude as u64
    };
    if spec.bits == 32 {
        low[r] = value_bits as u32;
    } else {
        super::super::wide::write(low, high, r, value_bits);
    }
    *condition = if invalid { 3 } else { cc(&value) };
    if spec.format == Format::Hex {
        return Ok(());
    }
    if invalid {
        state.control |= 0x00800000;
        // DFP functionally constrained invalid conversions recognize only
        // invalid, unlike BFP's additional inexact action.
        inexact = spec.format == Format::Binary;
        increment = false;
    }
    let flags = if inexact && !suppress_inexact {
        8 | if increment { 4 } else { 0 }
    } else {
        0
    };
    exceptions(state, flags)
}

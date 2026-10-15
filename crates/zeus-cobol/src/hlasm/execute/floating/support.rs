//! Radix conversion and radix-independent sign copies do not use IEEE masks.
use super::{State, Value, decode, hex, read, write};
use crate::hlasm::{
    Instruction,
    float::{
        self, Rounding,
        operations::{Support, SupportKind},
    },
};
use num_traits::Zero;

pub(super) fn execute(
    ins: &Instruction,
    spec: Support,
    state: &mut State,
    cc: &mut u8,
) -> Result<(), String> {
    let reg = |n: usize| {
        ins.registers
            .get(n)
            .copied()
            .filter(|r| *r < 16)
            .map(usize::from)
            .ok_or_else(|| "invalid floating-point support register".to_string())
    };
    let r = reg(0)?;
    let s = reg(1)?;
    if spec.kind == SupportKind::CopySign {
        let sign = read(state, s, 8) & (1 << 63);
        let magnitude = read(state, reg(2)?, 8) & !(1 << 63);
        write(state, r, 8, sign | magnitude);
        return Ok(());
    }
    let source = read(state, s, spec.source);
    if spec.kind == SupportKind::BinaryToHex {
        let (result, condition) = match decode(source, spec.source) {
            Value::Nan { .. } => (0x7fff_ffff_ffff_ffff, 3),
            Value::Infinity(negative) => ((u128::from(negative) << 63) | 0x7fff_ffff_ffff_ffff, 3),
            Value::Finite(value) => {
                let sign = u128::from(value.negative) << 63;
                if value.n.is_zero() {
                    (sign, 0)
                } else {
                    let exponent = value.floor_log2().div_euclid(4) + 1;
                    if exponent > 63 {
                        (sign | 0x7fff_ffff_ffff_ffff, 3)
                    } else if exponent < -64 {
                        (sign, if value.negative { 1 } else { 2 })
                    } else {
                        (
                            hex::pack(&value, 8, None, Rounding::Zero, 0).0,
                            if value.negative { 1 } else { 2 },
                        )
                    }
                }
            }
        };
        write(state, r, spec.destination, result);
        *cc = condition;
    } else {
        let value = hex::decode(source, 8).0;
        let mode = float::convert::radix_rounding(ins.immediate & 15)?;
        let (precision, _) = float::binary_shape(spec.destination);
        let (result, flags) = float::encode_binary(&value, spec.destination, precision, mode);
        write(state, r, spec.destination, result);
        *cc = if flags.overflow {
            3
        } else if value.n.is_zero() {
            0
        } else if value.negative {
            1
        } else {
            2
        };
    }
    Ok(())
}

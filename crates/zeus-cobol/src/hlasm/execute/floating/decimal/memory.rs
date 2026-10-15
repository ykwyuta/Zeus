//! Packed/zoned-memory DFP conversions and completed decimal-overflow results.
use super::{State, coefficient, float, read, write};
use crate::hlasm::{
    DynamicStorage, Instruction,
    float::operations::{DecimalMemory, check_register},
};
use num_bigint::BigUint;
use num_traits::Zero;

pub(in crate::hlasm::execute::floating) fn execute(
    ins: &Instruction,
    spec: DecimalMemory,
    low: &[u32; 16],
    memory: &mut [u8],
    storage: &DynamicStorage,
    state: &mut State,
    cc: &mut u8,
) -> Result<(), String> {
    let r = *ins.registers.first().ok_or("missing DFP register")?;
    check_register(r, spec.width)?;
    let length = ins.length as usize;
    if !(1..=spec.max_length() as usize).contains(&length) {
        return Err("invalid DFP memory conversion field length".into());
    }
    let at = super::super::super::address(
        ins.address.as_ref().ok_or("missing DFP packed address")?,
        low,
    )?;
    let range = super::super::super::range(memory, at, length)?;
    let signed = ins.immediate & 8 != 0;
    let digit_count = if spec.zoned {
        length
    } else {
        length * 2 - usize::from(signed)
    };
    if spec.to_decimal {
        let data = &memory[range];
        let sign = if spec.zoned {
            data[length - 1] >> 4
        } else {
            data[length - 1] & 15
        };
        let negative = if signed && (spec.zoned || ins.immediate & 1 == 0) {
            if sign < 10 {
                return Err("general-operand data exception: invalid decimal sign".into());
            }
            matches!(sign, 11 | 13)
        } else {
            false
        };
        let mut digits = BigUint::zero();
        for position in 0..digit_count {
            let digit = if spec.zoned {
                data[position] & 15
            } else {
                (data[position / 2] >> if position % 2 == 0 { 4 } else { 0 }) & 15
            };
            if digit > 9 {
                return Err("general-operand data exception: invalid decimal digit".into());
            }
            digits = digits * 10u8 + digit;
        }
        if digits.to_str_radix(10).len() > float::decimal_shape(spec.width).0 {
            return Err("general-operand data exception: nonzero unused packed digit".into());
        }
        write(
            state,
            usize::from(r),
            spec.width,
            float::pack_decimal(negative, &digits, 0, spec.width),
        );
        return Ok(());
    }
    let source = read(state, usize::from(r), spec.width);
    let original = coefficient(source, spec.width);
    let limit = BigUint::from(10u8).pow(digit_count as u32);
    let lost = original >= limit;
    let digits = &original % limit;
    let negative = source >> (spec.width * 8 - 1) != 0;
    let special = (source >> (spec.width * 8 - 6)) & 31 >= 30;
    let mut data = vec![0u8; length];
    let text = format!("{:0>digit_count$}", digits.to_str_radix(10));
    for (position, digit) in text.bytes().enumerate() {
        if spec.zoned {
            data[position] = (if ins.immediate & 4 != 0 { 0x30 } else { 0xf0 }) | (digit - b'0');
        } else {
            data[position / 2] |= (digit - b'0') << if position % 2 == 0 { 4 } else { 0 };
        }
    }
    if signed {
        let minus = negative && !(ins.immediate & 1 != 0 && digits.is_zero());
        let sign = if minus {
            13
        } else if ins.immediate & 2 != 0 {
            15
        } else {
            12
        };
        if spec.zoned {
            data[length - 1] = (data[length - 1] & 15) | (sign << 4);
        } else {
            data[length - 1] |= sign;
        }
    }
    storage.check_write(at, length)?;
    memory[range].copy_from_slice(&data);
    *cc = if special || lost {
        3
    } else if original.is_zero() {
        0
    } else if negative {
        1
    } else {
        2
    };
    if lost && state.program_mask & 4 != 0 {
        return Err("decimal-overflow exception".into());
    }
    Ok(())
}

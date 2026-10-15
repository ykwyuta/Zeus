//! Register packed-decimal conversions operate on significand digits, without
//! applying the floating-point exponent or recognizing IEEE exceptions.
use super::{State, coefficient, float, read, write};
use crate::hlasm::{
    Instruction,
    float::operations::{DecimalPacked, check_register},
};
use num_bigint::BigUint;
use num_traits::Zero;

pub(in crate::hlasm::execute::floating) fn execute(
    ins: &Instruction,
    spec: DecimalPacked,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    state: &mut State,
) -> Result<(), String> {
    let reg = |n: usize| {
        ins.registers
            .get(n)
            .copied()
            .filter(|r| *r < 16)
            .map(usize::from)
            .ok_or_else(|| "invalid packed-conversion register".to_string())
    };
    let r = reg(0)?;
    let s = reg(1)?;
    let (fp, gr) = if spec.to_decimal { (r, s) } else { (s, r) };
    check_register(fp as u8, spec.width)?;
    if spec.width == 16 && !gr.is_multiple_of(2) {
        return Err("packed general-register pair must begin with an even register".into());
    }
    if spec.to_decimal {
        let mut raw = u128::from(super::super::super::wide::read(low, high, gr));
        if spec.width == 16 {
            raw = (raw << 64) | u128::from(super::super::super::wide::read(low, high, gr + 1));
        }
        let negative = if spec.signed {
            let sign = raw & 15;
            if sign < 10 {
                return Err("general-operand data exception: invalid packed sign".into());
            }
            matches!(sign, 11 | 13)
        } else {
            false
        };
        let count = spec.width * 2 - usize::from(spec.signed);
        let mut digits = BigUint::zero();
        for index in (usize::from(spec.signed)..count + usize::from(spec.signed)).rev() {
            let digit = (raw >> (index * 4)) & 15;
            if digit > 9 {
                return Err("general-operand data exception: invalid packed digit".into());
            }
            digits = digits * 10u8 + digit as u8;
        }
        write(
            state,
            fp,
            spec.width,
            float::pack_decimal(negative, &digits, 0, spec.width),
        );
    } else {
        let source = read(state, fp, spec.width);
        let digits = coefficient(source, spec.width)
            % BigUint::from(10u8).pow((spec.width * 2 - usize::from(spec.signed)) as u32);
        let mut raw = 0u128;
        for digit in digits.to_str_radix(10).bytes() {
            raw = (raw << 4) | u128::from(digit - b'0');
        }
        if spec.signed {
            let sign = if source >> (spec.width * 8 - 1) != 0 {
                13
            } else if ins.immediate & 0x10 != 0 {
                15
            } else {
                12
            };
            raw = (raw << 4) | sign;
        }
        if spec.width == 16 {
            super::super::super::wide::write(low, high, gr, (raw >> 64) as u64);
            super::super::super::wide::write(low, high, gr + 1, raw as u64);
        } else {
            super::super::super::wide::write(low, high, gr, raw as u64);
        }
    }
    Ok(())
}

//! Mask addresses are bit selectors, never memory accesses.
use super::{State, decimal, read};
use crate::hlasm::{
    Instruction,
    float::{
        self,
        operations::{Classify, check_register},
    },
};
pub(super) fn execute(
    ins: &Instruction,
    spec: Classify,
    low: &[u32; 16],
    state: &State,
    cc: &mut u8,
) -> Result<(), String> {
    let r = *ins
        .registers
        .first()
        .ok_or("missing floating classification register")?;
    check_register(r, spec.width)?;
    let raw = read(state, usize::from(r), spec.width);
    let group = if spec.decimal {
        decimal::classification(raw, spec.width, spec.group)
    } else {
        let (fraction, bias) = float::binary_shape(spec.width);
        let exponent = (raw >> fraction) & ((2 * bias + 1) as u128);
        let payload = raw & ((1u128 << fraction) - 1);
        if exponent == 0 {
            if payload == 0 { 0 } else { 2 }
        } else if exponent == (2 * bias + 1) as u128 {
            if payload == 0 {
                3
            } else if payload & (1 << (fraction - 1)) != 0 {
                4
            } else {
                5
            }
        } else {
            1
        }
    };
    let negative = (raw >> (spec.width * 8 - 1)) as usize;
    let bit = 1usize << (11 - (group * 2 + negative));
    let mask = super::super::address(
        ins.address
            .as_ref()
            .ok_or("missing floating classification mask")?,
        low,
    )?;
    *cc = u8::from(mask & bit != 0);
    Ok(())
}

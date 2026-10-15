//! Register logical operations and conditional selection.
use super::wide::{read, write};
use crate::hlasm::{
    Instruction,
    operation::{self, Logic},
};
pub(super) fn execute(
    ins: &Instruction,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    cc: &mut u8,
) -> Result<bool, String> {
    let logic = operation::register_logic(&ins.op);
    let select = operation::select(&ins.op);
    if logic.is_none() && select.is_none() {
        return Ok(false);
    }
    let reg = |n| {
        ins.registers
            .get(n)
            .copied()
            .filter(|r| *r < 16)
            .map(usize::from)
            .ok_or("invalid logical/select register")
    };
    let (r1, r2, r3) = (reg(0)?, reg(1)?, reg(2)?);
    // Fetch both sources before updating aliased destination registers.
    let (a, b) = (read(low, high, r2), read(low, high, r3));
    if let Some((kind, wide)) = logic {
        let result = match kind {
            Logic::And => a & b,
            Logic::Or => a | b,
            Logic::Xor => a ^ b,
            Logic::Nand => !(a & b),
            Logic::Nor => !(a | b),
            Logic::Nxor => !(a ^ b),
            Logic::AndComplement => a & !b,
            Logic::OrComplement => a | !b,
        };
        if wide {
            write(low, high, r1, result);
            *cc = u8::from(result != 0);
        } else {
            low[r1] = result as u32;
            *cc = u8::from(low[r1] != 0);
        }
    } else {
        let (root, mask) = select.unwrap();
        let mask = mask.map_or(ins.immediate, u32::from);
        if mask > 15 {
            return Err("invalid SELECT condition mask".into());
        }
        let value = if mask & (8 >> *cc) != 0 { a } else { b };
        match root {
            "SELGR" => write(low, high, r1, value),
            "SELFHR" => high[r1] = (value >> 32) as u32,
            _ => low[r1] = value as u32,
        };
    }
    Ok(true)
}

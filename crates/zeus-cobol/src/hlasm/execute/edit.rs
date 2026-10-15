use super::{address, range};
use crate::hlasm::Instruction;

pub(super) fn execute(
    ins: &Instruction,
    registers: &mut [u32; 16],
    high: &mut [u32; 16],
    memory: &mut [u8],
    storage: &crate::hlasm::DynamicStorage,
    cc: &mut u8,
    mode: u8,
) -> Result<bool, String> {
    if !matches!(ins.op.as_str(), "ED" | "EDMK") {
        return Ok(false);
    }
    let first = address(
        ins.address.as_ref().ok_or("missing edit pattern")?,
        registers,
    )?;
    let mut source = address(
        ins.source.as_ref().ok_or("missing packed edit source")?,
        registers,
    )?;
    let length = ins.length as usize;
    if !(1..=256).contains(&length) {
        return Err("invalid edit length".into());
    }
    range(memory, first, length)?;
    storage.check_write(first, length)?;
    let fill = memory[first];
    let (mut significant, mut nonzero, mut lower, mut current) = (false, false, false, 0u8);
    for at in first + 1..first + length {
        let pattern = memory[at];
        match pattern {
            0x20 | 0x21 => {
                let (digit, plus) = if lower {
                    lower = false;
                    source += 1;
                    (current & 15, false)
                } else {
                    range(memory, source, 1)?;
                    current = memory[source];
                    let digit = current >> 4;
                    if digit > 9 {
                        return Err("invalid packed decimal edit digit".into());
                    }
                    let sign = current & 15;
                    if sign > 9 {
                        source += 1;
                        (digit, matches!(sign, 0xa | 0xc | 0xe | 0xf))
                    } else {
                        lower = true;
                        (digit, false)
                    }
                };
                if !significant && digit != 0 && ins.op == "EDMK" {
                    if mode == 64 {
                        super::wide::write(registers, high, 1, at as u64);
                    } else {
                        let mask = crate::hlasm::abi::mask(mode)? as u32;
                        registers[1] = (registers[1] & !mask) | (at as u32 & mask);
                    }
                }
                memory[at] = if significant || digit != 0 {
                    0xf0 | digit
                } else {
                    fill
                };
                nonzero |= digit != 0;
                significant = !plus && (significant || pattern == 0x21 || digit != 0);
            }
            0x22 => {
                memory[at] = fill;
                significant = false;
                nonzero = false;
            }
            _ if !significant => memory[at] = fill,
            _ => {}
        }
    }
    *cc = if !nonzero {
        0
    } else if significant {
        1
    } else {
        2
    };
    Ok(true)
}

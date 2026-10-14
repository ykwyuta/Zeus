use super::wide::{doubleword, read, write};
use super::{address, range, word};
use crate::hlasm::Instruction;

/// Model byte-order conversions without relying on the host's endianness.
/// These operations preserve the condition code.
pub(super) fn execute(
    ins: &Instruction,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    mem: &mut [u8],
    storage: &crate::hlasm::DynamicStorage,
) -> Result<bool, String> {
    let op = ins.op.as_str();
    if !matches!(
        op,
        "LRV"
            | "LRVG"
            | "LRVH"
            | "LRVR"
            | "LRVGR"
            | "STRV"
            | "STRVG"
            | "STRVH"
            | "LB"
            | "LGB"
            | "LBR"
            | "LGBR"
    ) {
        return Ok(false);
    }
    let reg = |n: usize| -> Result<usize, String> {
        ins.registers
            .get(n)
            .copied()
            .filter(|&r| r < 16)
            .map(usize::from)
            .ok_or_else(|| "invalid register operand".into())
    };
    let r = reg(0)?;
    if matches!(op, "LRVR" | "LRVGR" | "LBR" | "LGBR") {
        let source = reg(1)?;
        match op {
            "LRVR" => low[r] = low[source].swap_bytes(),
            "LRVGR" => write(low, high, r, read(low, high, source).swap_bytes()),
            "LBR" => low[r] = low[source] as u8 as i8 as i32 as u32,
            "LGBR" => write(low, high, r, low[source] as u8 as i8 as i64 as u64),
            _ => unreachable!(),
        }
        return Ok(true);
    }
    let addr = address(ins.address.as_ref().ok_or("missing address operand")?, low)?;
    match op {
        "STRV" | "STRVG" | "STRVH" => {
            let len = match op {
                "STRVG" => 8,
                "STRV" => 4,
                _ => 2,
            };
            let dest = range(mem, addr, len)?;
            storage.check_write(addr, len)?;
            mem[dest].copy_from_slice(&read(low, high, r).to_le_bytes()[..len]);
        }
        "LRV" => low[r] = word(mem, addr)?.swap_bytes(),
        "LRVG" => write(low, high, r, doubleword(mem, addr)?.swap_bytes()),
        "LRVH" => {
            let bytes = &mem[range(mem, addr, 2)?];
            low[r] = (low[r] & 0xffff_0000) | u32::from(u16::from_le_bytes([bytes[0], bytes[1]]));
        }
        "LB" | "LGB" => {
            let byte = mem[range(mem, addr, 1)?][0] as i8;
            if op == "LB" {
                low[r] = byte as i32 as u32;
            } else {
                write(low, high, r, byte as i64 as u64);
            }
        }
        _ => unreachable!(),
    }
    Ok(true)
}

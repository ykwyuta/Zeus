use super::{
    address, range,
    wide::{read, write},
};
use crate::hlasm::{
    Instruction,
    operation::{ConditionalKind, conditional},
};

pub(super) fn execute(
    ins: &Instruction,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    mem: &mut [u8],
    cc: u8,
    storage: &crate::hlasm::DynamicStorage,
) -> Result<bool, String> {
    let Some(spec) = conditional(&ins.op) else {
        return Ok(false);
    };
    let mask = if spec.kind == ConditionalKind::Immediate {
        ins.length
    } else {
        ins.immediate
    };
    if mask > 15 || cc > 3 {
        return Err("invalid conditional mask or CC".into());
    }
    let r = *ins
        .registers
        .first()
        .filter(|r| **r < 16)
        .ok_or("invalid conditional register")? as usize;
    let selected = mask & (8 >> cc) != 0;
    let value = match spec.kind {
        ConditionalKind::Register => {
            let source = *ins
                .registers
                .get(1)
                .filter(|r| **r < 16)
                .ok_or("invalid conditional source register")? as usize;
            if spec.wide {
                read(low, high, source)
            } else if spec.high {
                u64::from(high[source])
            } else {
                u64::from(low[source])
            }
        }
        ConditionalKind::Immediate => {
            if i16::try_from(ins.immediate as i32).is_err() {
                return Err("invalid signed conditional immediate".into());
            }
            ins.immediate as i32 as i64 as u64
        }
        _ => {
            let operand = ins.address.as_ref().ok_or("missing conditional address")?;
            if operand.index != 0 {
                return Err("conditional address cannot have an index".into());
            }
            let at = address(operand, low)?;
            // This engine chooses the permitted model that validates storage
            // accesses even when the condition is false.
            let area = range(mem, at, if spec.wide { 8 } else { 4 })?;
            if spec.kind == ConditionalKind::MemoryStore {
                storage.check_write(at, if spec.wide { 8 } else { 4 })?;
                if selected {
                    if spec.wide {
                        mem[area].copy_from_slice(&read(low, high, r).to_be_bytes());
                    } else {
                        mem[area].copy_from_slice(
                            &(if spec.high { high[r] } else { low[r] }).to_be_bytes(),
                        );
                    }
                }
                return Ok(true);
            }
            if spec.wide {
                u64::from_be_bytes(mem[area].try_into().unwrap())
            } else {
                u64::from(u32::from_be_bytes(mem[area].try_into().unwrap()))
            }
        }
    };
    if selected {
        if spec.wide {
            write(low, high, r, value);
        } else if spec.high {
            high[r] = value as u32;
        } else {
            low[r] = value as u32;
        }
    }
    Ok(true)
}

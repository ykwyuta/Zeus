use super::{
    address, range,
    wide::{read, write},
};
use crate::hlasm::{Instruction, operation};

pub(super) fn execute(
    ins: &Instruction,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    mem: &mut [u8],
    cc: &mut u8,
    storage: &crate::hlasm::DynamicStorage,
    mode: u8,
) -> Result<bool, String> {
    let op = ins.op.as_str();
    if op == "CSST" {
        csst(ins, low, high, mem, cc, storage, mode)?;
        return Ok(true);
    }
    if matches!(op, "LPD" | "LPDG") {
        let r = ins
            .registers
            .first()
            .copied()
            .filter(|r| *r < 16 && r % 2 == 0)
            .map(usize::from)
            .ok_or("specification exception: LPD/LPDG require an even register pair")?;
        let width = if op == "LPD" { 4 } else { 8 };
        let operand = |operand: Option<&crate::hlasm::Address>| -> Result<usize, String> {
            let operand = operand.ok_or("missing LPD/LPDG operand")?;
            if operand.index != 0 {
                return Err("LPD/LPDG do not accept index registers".into());
            }
            let at = address(operand, low)?;
            if !at.is_multiple_of(width) {
                return Err("specification exception: LPD/LPDG operand alignment".into());
            }
            range(mem, at, width)?;
            Ok(at)
        };
        let (first, second) = (
            operand(ins.address.as_ref())?,
            operand(ins.source.as_ref())?,
        );
        // Both fetches complete under the same exclusive run-unit memory loan.
        // Capture addresses and bytes before either destination register changes.
        if width == 4 {
            let a = super::word(mem, first)?;
            let b = super::word(mem, second)?;
            low[r] = a;
            low[r + 1] = b;
        } else {
            let a = super::wide::doubleword(mem, first)?;
            let b = super::wide::doubleword(mem, second)?;
            write(low, high, r, a);
            write(low, high, r + 1, b);
        }
        *cc = 0;
        return Ok(true);
    }
    let Some((_, width)) = operation::interlocked(op) else {
        return Ok(false);
    };
    let operand = ins.address.as_ref().ok_or("missing interlocked operand")?;
    if operand.index != 0 {
        return Err("interlocked operand cannot have an index".into());
    }
    let addr = address(operand, low)?;
    if !addr.is_multiple_of(width) {
        return Err("specification exception: interlocked operand alignment".into());
    }
    let target = range(mem, addr, width)?;
    storage.check_write(addr, width)?;
    if op == "TS" {
        *cc = mem[addr] >> 7;
        mem[addr] = 0xff;
        return Ok(true);
    }
    let reg = |n| {
        ins.registers
            .get(n)
            .copied()
            .filter(|r| *r < 16)
            .map(usize::from)
            .ok_or("invalid interlocked register")
    };
    let (r1, r3) = (reg(0)?, reg(1)?);
    if op.starts_with('C') {
        let pair = op.starts_with("CDS");
        if pair && (r1 % 2 != 0 || r3 % 2 != 0) {
            return Err("specification exception: compare double register pair".into());
        }
        let register_bytes = |r: usize| -> Vec<u8> {
            if op == "CDSG" {
                [
                    read(low, high, r).to_be_bytes(),
                    read(low, high, r + 1).to_be_bytes(),
                ]
                .concat()
            } else if pair {
                [low[r].to_be_bytes(), low[r + 1].to_be_bytes()].concat()
            } else if width == 8 {
                read(low, high, r).to_be_bytes().to_vec()
            } else {
                low[r].to_be_bytes().to_vec()
            }
        };
        let compare = register_bytes(r1);
        // Capture the replacement before updating any overlapping registers.
        let replacement = register_bytes(r3);
        if mem[target.clone()] == compare {
            mem[target].copy_from_slice(&replacement);
            *cc = 0;
        } else {
            if op == "CDSG" {
                let first = u64::from_be_bytes(mem[addr..addr + 8].try_into().unwrap());
                let second = u64::from_be_bytes(mem[addr + 8..addr + 16].try_into().unwrap());
                write(low, high, r1, first);
                write(low, high, r1 + 1, second);
            } else if pair {
                low[r1] = u32::from_be_bytes(mem[addr..addr + 4].try_into().unwrap());
                low[r1 + 1] = u32::from_be_bytes(mem[addr + 4..addr + 8].try_into().unwrap());
            } else if width == 8 {
                write(
                    low,
                    high,
                    r1,
                    u64::from_be_bytes(mem[target].try_into().unwrap()),
                );
            } else {
                low[r1] = u32::from_be_bytes(mem[target].try_into().unwrap());
            }
            *cc = 1;
        }
        return Ok(true);
    }
    let old = if width == 8 {
        u64::from_be_bytes(mem[target.clone()].try_into().unwrap())
    } else {
        u64::from(u32::from_be_bytes(mem[target.clone()].try_into().unwrap()))
    };
    let arg = if width == 8 {
        read(low, high, r3)
    } else {
        u64::from(low[r3])
    };
    let (sum, carry, overflow) = if width == 8 {
        let (sum, carry) = old.overflowing_add(arg);
        (sum, carry, (old as i64).overflowing_add(arg as i64).1)
    } else {
        let (sum, carry) = (old as u32).overflowing_add(arg as u32);
        (
            u64::from(sum),
            carry,
            (old as i32).overflowing_add(arg as i32).1,
        )
    };
    let value = match op {
        "LAA" | "LAAG" => {
            *cc = if overflow {
                3
            } else if sum == 0 {
                0
            } else if (width == 8 && (sum as i64) < 0) || (width == 4 && (sum as i32) < 0) {
                1
            } else {
                2
            };
            sum
        }
        "LAAL" | "LAALG" => {
            *cc = u8::from(sum != 0) + 2 * u8::from(carry);
            sum
        }
        _ => {
            let value = if matches!(op, "LAN" | "LANG") {
                old & arg
            } else if matches!(op, "LAO" | "LAOG") {
                old | arg
            } else {
                old ^ arg
            };
            *cc = u8::from(value != 0);
            value
        }
    };
    if width == 8 {
        mem[target].copy_from_slice(&value.to_be_bytes());
        write(low, high, r1, old);
    } else {
        mem[target].copy_from_slice(&(value as u32).to_be_bytes());
        low[r1] = old as u32;
    }
    Ok(true)
}

fn csst(
    ins: &Instruction,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    memory: &mut [u8],
    cc: &mut u8,
    storage: &crate::hlasm::DynamicStorage,
    mode: u8,
) -> Result<(), String> {
    let function = low[0] & 255;
    let characteristic = (low[0] >> 8) & 255;
    if function > 2 || characteristic > 4 {
        return Err(
            "specification exception: unassigned CSST function/store characteristic".into(),
        );
    }
    let width = 4usize << function;
    let count = 1usize << characteristic;
    let r = ins
        .registers
        .first()
        .copied()
        .filter(|r| *r < 16)
        .map(usize::from)
        .ok_or("invalid CSST compare register")?;
    if function == 2 && r % 2 != 0 {
        return Err("specification exception: CSST quadword requires an even pair".into());
    }
    let operand = |value: Option<&crate::hlasm::Address>, alignment| -> Result<usize, String> {
        let value = value.ok_or("missing CSST operand")?;
        if value.index != 0 {
            return Err("CSST does not accept index registers".into());
        }
        let at = address(value, low)?;
        if !at.is_multiple_of(alignment) {
            return Err("specification exception: CSST operand alignment".into());
        }
        Ok(at)
    };
    let first = operand(ins.address.as_ref(), width)?;
    let second = operand(ins.source.as_ref(), count)?;
    let first_range = range(memory, first, width)?;
    let second_range = range(memory, second, count)?;
    storage.check_write(first, width)?;
    storage.check_write(second, count)?;
    // The parameter address ignores its low four bits in AMODE 31.
    let parameters = crate::hlasm::abi::register(low, high, 1, mode)? & !15;
    let replacement = memory[range(memory, parameters, width)?].to_vec();
    let footprint = memory[range(memory, parameters + 16, count)?].to_vec();
    let original = memory[first_range.clone()].to_vec();
    let compare = if function == 0 {
        low[r].to_be_bytes().to_vec()
    } else if function == 1 {
        read(low, high, r).to_be_bytes().to_vec()
    } else {
        [
            read(low, high, r).to_be_bytes(),
            read(low, high, r + 1).to_be_bytes(),
        ]
        .concat()
    };
    if original == compare {
        // Fetch both parameter values first, then commit operand 1 followed by
        // operand 2. This also defines the architecturally ordered overlap case.
        memory[first_range].copy_from_slice(&replacement);
        memory[second_range].copy_from_slice(&footprint);
        *cc = 0;
    } else {
        if function == 0 {
            low[r] = u32::from_be_bytes(original.try_into().unwrap());
        } else {
            write(
                low,
                high,
                r,
                u64::from_be_bytes(original[..8].try_into().unwrap()),
            );
            if function == 2 {
                write(
                    low,
                    high,
                    r + 1,
                    u64::from_be_bytes(original[8..].try_into().unwrap()),
                );
            }
        }
        *cc = 1;
    }
    Ok(())
}

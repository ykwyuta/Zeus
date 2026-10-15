use super::{address, ordering_cc, range};
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
    if matches!(ins.op.as_str(), "MVST" | "CLST" | "SRST") {
        return string(ins, registers, high, memory, storage, cc, mode);
    }
    if !matches!(ins.op.as_str(), "MVCL" | "CLCL" | "MVCLE" | "CLCLE") {
        return Ok(false);
    }
    let r1 = *ins.registers.first().ok_or("missing first register pair")? as usize;
    let r2 = *ins.registers.get(1).ok_or("missing second register pair")? as usize;
    if r1 >= 15 || r2 >= 15 || !r1.is_multiple_of(2) || !r2.is_multiple_of(2) {
        return Err("long string instructions require even register pairs".into());
    }
    let (mut a, mut b) = (
        crate::hlasm::abi::register(registers, high, r1, mode)?,
        crate::hlasm::abi::register(registers, high, r2, mode)?,
    );
    let extended = ins.op.ends_with('E');
    let mask = if extended { u32::MAX } else { 0x00ff_ffff };
    let (mut n1, mut n2) = if extended && mode == 64 {
        (
            usize::try_from(crate::hlasm::abi::read(registers, high, r1 + 1))
                .map_err(|_| "long string length exceeds host offset range")?,
            usize::try_from(crate::hlasm::abi::read(registers, high, r2 + 1))
                .map_err(|_| "long string length exceeds host offset range")?,
        )
    } else {
        (
            (registers[r1 + 1] & mask) as usize,
            (registers[r2 + 1] & mask) as usize,
        )
    };
    let (top1, top2) = (registers[r1 + 1] & !mask, registers[r2 + 1] & !mask);
    let pad = if extended {
        address(
            ins.address.as_ref().ok_or("missing padding operand")?,
            registers,
        )? as u8
    } else {
        (top2 >> 24) as u8
    };
    if matches!(ins.op.as_str(), "MVCL" | "MVCLE") {
        let count = if extended { n1.min(4096) } else { n1 };
        let copied = count.min(n2);
        if !extended && copied != 0 && a > b && a - b < copied {
            *cc = 3;
            return Ok(true);
        }
        if count != 0 {
            range(memory, a, count)?;
        }
        if copied != 0 {
            range(memory, b, copied)?;
        }
        *cc = ordering_cc(n1.cmp(&n2));
        storage.check_write(a, count)?;
        for i in 0..count {
            memory[a + i] = if i < copied { memory[b + i] } else { pad };
        }
        a += count;
        b += copied;
        n2 -= copied;
        n1 -= count;
        if n1 != 0 {
            *cc = 3;
        }
    } else {
        *cc = 0;
        let mut processed = 0;
        while n1 != 0 || n2 != 0 {
            if extended && processed == 4096 {
                *cc = 3;
                break;
            }
            processed += 1;
            let left = if n1 == 0 {
                pad
            } else {
                memory[range(memory, a, 1)?.start]
            };
            let right = if n2 == 0 {
                pad
            } else {
                memory[range(memory, b, 1)?.start]
            };
            if left != right {
                *cc = ordering_cc(left.cmp(&right));
                break;
            }
            if n1 != 0 {
                n1 -= 1;
                a += 1;
            }
            if n2 != 0 {
                n2 -= 1;
                b += 1;
            }
        }
    }
    put(registers, high, r1, a, mode)?;
    put(registers, high, r2, b, mode)?;
    if extended && mode == 64 {
        super::wide::write(registers, high, r1 + 1, n1 as u64);
        super::wide::write(registers, high, r2 + 1, n2 as u64);
    } else {
        registers[r1 + 1] = top1 | n1 as u32;
        registers[r2 + 1] = top2 | n2 as u32;
    }
    Ok(true)
}

fn string(
    ins: &Instruction,
    registers: &mut [u32; 16],
    high: &mut [u32; 16],
    memory: &mut [u8],
    storage: &crate::hlasm::DynamicStorage,
    cc: &mut u8,
    mode: u8,
) -> Result<bool, String> {
    let r1 = *ins.registers.first().ok_or("missing string register")? as usize;
    let r2 = *ins.registers.get(1).ok_or("missing string register")? as usize;
    if r1 > 15 || r2 > 15 {
        return Err("invalid string register".into());
    }
    if registers[0] & 0xffff_ff00 != 0 {
        return Err("string ending-character register has nonzero reserved bits".into());
    }
    let stop = registers[0] as u8;
    let (mut a, mut b) = (
        crate::hlasm::abi::register(registers, high, r1, mode)?,
        crate::hlasm::abi::register(registers, high, r2, mode)?,
    );
    for _ in 0..4096 {
        if ins.op == "SRST" && a == b {
            *cc = 2;
            return Ok(true);
        }
        let right = memory[range(memory, b, 1)?.start];
        if ins.op == "SRST" {
            if right == stop {
                put(registers, high, r1, b, mode)?;
                *cc = 1;
                return Ok(true);
            }
        } else if ins.op == "MVST" {
            let target = range(memory, a, 1)?.start;
            storage.check_write(target, 1)?;
            memory[target] = right;
            if right == stop {
                put(registers, high, r1, a, mode)?;
                *cc = 1;
                return Ok(true);
            }
        } else {
            let left = memory[range(memory, a, 1)?.start];
            if left == stop && right == stop {
                *cc = 0;
                return Ok(true);
            }
            if left != right {
                *cc = if left == stop {
                    1
                } else if right == stop {
                    2
                } else {
                    ordering_cc(left.cmp(&right))
                };
                put(registers, high, r1, a, mode)?;
                put(registers, high, r2, b, mode)?;
                return Ok(true);
            }
        }
        if ins.op != "SRST" {
            a = (a + 1) & crate::hlasm::abi::mask(mode)? as usize;
        }
        b = (b + 1) & crate::hlasm::abi::mask(mode)? as usize;
    }
    *cc = 3;
    if ins.op != "SRST" {
        put(registers, high, r1, a, mode)?;
    }
    put(registers, high, r2, b, mode)?;
    Ok(true)
}

fn put(
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    r: usize,
    address: usize,
    mode: u8,
) -> Result<(), String> {
    let value = address as u64 & crate::hlasm::abi::mask(mode)?;
    if mode == 64 {
        super::wide::write(low, high, r, value);
    } else {
        low[r] = value as u32;
    }
    Ok(())
}

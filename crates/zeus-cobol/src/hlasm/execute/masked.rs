use super::wide::{read, write};
use super::{address, ordering_cc, range};
use crate::hlasm::Instruction;
use crate::hlasm::operation::{MaskedKind, masked_bytes};

pub(super) fn execute(
    ins: &Instruction,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    mem: &mut [u8],
    storage: &crate::hlasm::DynamicStorage,
    cc: &mut u8,
) -> Result<bool, String> {
    let Some(spec) = masked_bytes(&ins.op) else {
        return Ok(false);
    };
    let r = ins
        .registers
        .first()
        .copied()
        .filter(|&r| r < 16)
        .map(usize::from)
        .ok_or("invalid register operand")?;
    let mask = u8::try_from(ins.immediate)
        .ok()
        .filter(|&mask| mask <= 15)
        .ok_or("invalid four-bit byte mask")?;
    let operand = ins.address.as_ref().ok_or("missing address operand")?;
    if operand.index != 0 {
        return Err("index not permitted for masked-byte instruction".into());
    }
    // Capture the address before modifying a register that may be its base.
    let addr = address(operand, low)?;
    let count = mask.count_ones() as usize;
    // ICM/CLM require a one-byte access check even for mask zero. STCM
    // permits this model-dependent check; Zeus chooses to perform it too.
    range(mem, addr, count.max(1))?;
    if matches!(spec.kind, MaskedKind::Store) {
        storage.check_write(addr, count)?;
    }
    let mut bytes = read(low, high, r).to_be_bytes();
    let start = if spec.high { 0 } else { 4 };
    let mut selected = [0u8; 4];
    let mut n = 0;
    for i in 0..4 {
        if mask & (8 >> i) == 0 {
            continue;
        }
        match spec.kind {
            MaskedKind::Insert => bytes[start + i] = mem[addr + n],
            MaskedKind::Store => mem[addr + n] = bytes[start + i],
            MaskedKind::Compare => selected[n] = bytes[start + i],
        }
        n += 1;
    }
    match spec.kind {
        MaskedKind::Insert => {
            *cc = if mem[addr..addr + count].iter().all(|&byte| byte == 0) {
                0
            } else if mem[addr] & 0x80 != 0 {
                1
            } else {
                2
            };
            write(low, high, r, u64::from_be_bytes(bytes));
        }
        MaskedKind::Compare => *cc = ordering_cc(selected[..count].cmp(&mem[addr..addr + count])),
        MaskedKind::Store => {}
    }
    Ok(true)
}

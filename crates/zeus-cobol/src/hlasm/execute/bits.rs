use super::wide::{doubleword, read, write};
use super::{address, word};
use crate::hlasm::Instruction;
use crate::hlasm::operation::{ImmediateKind, partial_immediate};

pub(super) fn execute(
    ins: &Instruction,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    mem: &[u8],
    cc: &mut u8,
) -> Result<bool, String> {
    let op = ins.op.as_str();
    let partial = partial_immediate(op);
    let carry = matches!(
        op,
        "ALC" | "ALCR" | "ALCG" | "ALCGR" | "SLB" | "SLBR" | "SLBG" | "SLBGR"
    );
    if partial.is_none() && !carry {
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
    if let Some(spec) = partial {
        let mask = (1u64 << spec.width) - 1;
        let immediate = u64::from(ins.immediate);
        if immediate > mask {
            return Err("invalid unsigned immediate".into());
        }
        let original = read(low, high, r);
        let field = (original >> spec.shift) & mask;
        let result = match spec.kind {
            ImmediateKind::Insert | ImmediateKind::Load => immediate,
            ImmediateKind::And => field & immediate,
            ImmediateKind::Or => field | immediate,
            ImmediateKind::Xor => field ^ immediate,
            ImmediateKind::Test => {
                let selected = field & immediate;
                *cc = if selected == 0 {
                    0
                } else if selected == immediate {
                    3
                } else if selected & (1u64 << (63 - immediate.leading_zeros())) != 0 {
                    2
                } else {
                    1
                };
                return Ok(true);
            }
        };
        if matches!(
            spec.kind,
            ImmediateKind::And | ImmediateKind::Or | ImmediateKind::Xor
        ) {
            // Only the field replaced by this instruction determines the CC.
            *cc = u8::from(result != 0);
        }
        let value = if matches!(spec.kind, ImmediateKind::Load) {
            result << spec.shift
        } else {
            (original & !(mask << spec.shift)) | (result << spec.shift)
        };
        write(low, high, r, value);
        return Ok(true);
    }

    let wide = op.contains('G');
    let width = if wide { 64 } else { 32 };
    let mask = if wide { u64::MAX } else { u64::from(u32::MAX) };
    let a = read(low, high, r) & mask;
    // Fetch both inputs before writing the result, including aliased operands.
    let b = if op.ends_with('R') {
        read(low, high, reg(1)?) & mask
    } else {
        let addr = address(ins.address.as_ref().ok_or("missing address operand")?, low)?;
        if wide {
            doubleword(mem, addr)?
        } else {
            u64::from(word(mem, addr)?)
        }
    };
    let b = if op.starts_with('S') { b ^ mask } else { b };
    // For subtraction, the CC high bit represents the absence of a borrow.
    let sum = u128::from(a) + u128::from(b) + u128::from((*cc >> 1) & 1);
    let value = sum as u64 & mask;
    *cc = u8::from(value != 0) + 2 * u8::from((sum >> width) != 0);
    if wide {
        write(low, high, r, value);
    } else {
        low[r] = value as u32;
    }
    Ok(true)
}

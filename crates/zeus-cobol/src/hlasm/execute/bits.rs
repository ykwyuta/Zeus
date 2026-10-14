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
    if super::super::operation::rotate_selected(op).is_some() {
        return rotate_selected(ins, low, high, cc).map(|()| true);
    }
    if matches!(op, "FLOGR" | "POPCNT") {
        let r = |n| {
            ins.registers
                .get(n)
                .copied()
                .filter(|r| *r < 16)
                .map(usize::from)
                .ok_or("invalid bit-count register")
        };
        let (r1, r2) = (r(0)?, r(1)?);
        let source = read(low, high, r2);
        if op == "FLOGR" {
            if r1 % 2 != 0 {
                return Err("FLOGR requires an even register pair".into());
            }
            let count = source.leading_zeros();
            let remaining = if source == 0 {
                0
            } else {
                source & !(1u64 << (63 - count))
            };
            write(low, high, r1, u64::from(count));
            write(low, high, r1 + 1, remaining);
            *cc = if source == 0 { 0 } else { 2 };
        } else {
            if ![0, 8].contains(&ins.immediate) {
                return Err("invalid POPCNT mask: reserved bits".into());
            }
            let result = if ins.immediate == 8 {
                u64::from(source.count_ones())
            } else {
                u64::from_be_bytes(source.to_be_bytes().map(|b| b.count_ones() as u8))
            };
            write(low, high, r1, result);
            *cc = u8::from(result != 0);
        }
        return Ok(true);
    }
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

fn rotate_selected(
    ins: &Instruction,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    cc: &mut u8,
) -> Result<(), String> {
    let r = |n| {
        ins.registers
            .get(n)
            .copied()
            .filter(|r| *r < 16)
            .map(usize::from)
            .ok_or("invalid rotate/select register")
    };
    let (r1, r2) = (r(0)?, r(1)?);
    if ins.immediate > 0xffffff {
        return Err("invalid rotate/select immediates".into());
    }
    let op = ins.op.as_str();
    let first = (ins.immediate >> 16) as u8;
    let last = (ins.immediate >> 8) as u8;
    let rotation = ins.immediate & 63;
    let half = matches!(op, "RISBHG" | "RISBLG");
    let width = if half { 32 } else { 64 };
    let start = usize::from(first) % width;
    let end = usize::from(last) % width;
    let offset = if op == "RISBLG" { 32 } else { 0 };
    // Number bits from the architectural leftmost bit, and wrap only within
    // the selected word for HIGH/LOW forms.
    let mut mask = 0u64;
    for position in 0..width {
        let included = if start <= end {
            (start..=end).contains(&position)
        } else {
            position >= start || position <= end
        };
        if included {
            mask |= 1u64 << (63 - position - offset);
        }
    }
    let original = read(low, high, r1);
    let rotated = read(low, high, r2).rotate_left(rotation);
    let logical = matches!(op, "RNSBG" | "ROSBG" | "RXSBG");
    let selected = match op {
        "RNSBG" => original & rotated,
        "ROSBG" => original | rotated,
        "RXSBG" => original ^ rotated,
        _ => rotated,
    } & mask;
    let remaining = if !logical && last & 0x80 != 0 {
        if op == "RISBHG" {
            original & 0xffffffff
        } else if op == "RISBLG" {
            original & 0xffffffff00000000
        } else {
            0
        }
    } else {
        original & !mask
    };
    let result = remaining | selected;
    if logical {
        *cc = u8::from(selected != 0);
    } else if op == "RISBG" {
        *cc = super::ordering_cc((result as i64).cmp(&0));
    }
    if !logical || first & 0x80 == 0 {
        write(low, high, r1, result);
    }
    Ok(())
}

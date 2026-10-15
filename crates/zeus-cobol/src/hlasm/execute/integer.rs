use super::wide::{doubleword, read, write};
use super::{address, ordering_cc, word};
use crate::hlasm::Instruction;
use crate::hlasm::assemble::{pair_operation, shift_operation};

pub(super) fn execute(
    ins: &Instruction,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    mem: &[u8],
    cc: &mut u8,
) -> Result<bool, String> {
    let op = ins.op.as_str();
    if !pair_operation(op) && !shift_operation(op) {
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
    if pair_operation(op) && !r.is_multiple_of(2) {
        return Err("instruction requires an even register pair".into());
    }
    let pair = || (u64::from(low[r]) << 32) | u64::from(low[r + 1]);
    if shift_operation(op) {
        let addr = ins.address.as_ref().ok_or("missing shift count")?;
        if addr.index != 0 || addr.base >= 16 {
            return Err("invalid shift/rotate base register".into());
        }
        // This operand supplies a count, not a storage access. Negative
        // sums and addresses outside storage are valid; only six bits matter.
        let base = if addr.base == 0 {
            0
        } else {
            i64::from(low[usize::from(addr.base)])
        };
        let count = (base.wrapping_add(addr.displacement) as u64 & 63) as u32;
        let paired = pair_operation(op);
        let src = if paired {
            pair()
        } else {
            read(low, high, reg(1)?)
        };
        let value = match op {
            "RLL" => {
                low[r] = (src as u32).rotate_left(count);
                return Ok(true);
            }
            "RLLG" => src.rotate_left(count),
            "SLLG" | "SLDL" => src << count,
            "SRLG" | "SRDL" => src >> count,
            "SRAG" | "SRDA" => {
                let v = (src as i64) >> count;
                *cc = ordering_cc(v.cmp(&0));
                v as u64
            }
            "SLAG" | "SLDA" => {
                let exact = i128::from(src as i64) * (1i128 << count);
                let overflow = i64::try_from(exact).is_err();
                let value = (src & (1u64 << 63)) | ((src << count) & i64::MAX as u64);
                *cc = if overflow {
                    3
                } else {
                    ordering_cc((value as i64).cmp(&0))
                };
                value
            }
            _ => unreachable!(),
        };
        if paired {
            low[r] = (value >> 32) as u32;
            low[r + 1] = value as u32;
        } else {
            write(low, high, r, value);
        }
        return Ok(true);
    }

    let narrow = matches!(op, "ML" | "MLR" | "DL" | "DLR");
    let signed = op.starts_with("DSG");
    let fullword = narrow || matches!(op, "DSGF" | "DSGFR");
    // Fetch before writing either destination, allowing overlapping registers.
    let b = if op.ends_with('R') {
        let source = reg(1)?;
        if fullword {
            u64::from(low[source])
        } else {
            read(low, high, source)
        }
    } else {
        let addr = address(ins.address.as_ref().ok_or("missing address operand")?, low)?;
        if fullword {
            u64::from(word(mem, addr)?)
        } else {
            doubleword(mem, addr)?
        }
    };
    let odd = if narrow {
        u64::from(low[r + 1])
    } else {
        read(low, high, r + 1)
    };
    let (even, odd) = if op.starts_with('M') {
        if narrow {
            let product = odd * b;
            (product >> 32, product & u64::from(u32::MAX))
        } else {
            let product = u128::from(odd) * u128::from(b);
            ((product >> 64) as u64, product as u64)
        }
    } else if signed {
        let dividend = odd as i64;
        let divisor = if fullword { b as i32 as i64 } else { b as i64 };
        let quotient = dividend
            .checked_div(divisor)
            .ok_or("fixed-point divide exception")?;
        ((dividend % divisor) as u64, quotient as u64)
    } else {
        let dividend = if narrow {
            u128::from(pair())
        } else {
            (u128::from(read(low, high, r)) << 64) | u128::from(odd)
        };
        let quotient = dividend
            .checked_div(u128::from(b))
            .filter(|&q| {
                q <= if narrow {
                    u128::from(u32::MAX)
                } else {
                    u128::from(u64::MAX)
                }
            })
            .ok_or("fixed-point divide exception")?;
        ((dividend % u128::from(b)) as u64, quotient as u64)
    };
    if narrow {
        low[r] = even as u32;
        low[r + 1] = odd as u32;
    } else {
        write(low, high, r, even);
        write(low, high, r + 1, odd);
    }
    // Multiply and divide leave the incoming condition code unchanged.
    Ok(true)
}

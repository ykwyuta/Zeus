use super::{address, ordering_cc, range, word};
use crate::hlasm::Instruction;

pub(super) fn read(low: &[u32; 16], high: &[u32; 16], r: usize) -> u64 {
    (u64::from(high[r]) << 32) | u64::from(low[r])
}

pub(super) fn write(low: &mut [u32; 16], high: &mut [u32; 16], r: usize, value: u64) {
    low[r] = value as u32;
    high[r] = (value >> 32) as u32;
}

pub(super) fn doubleword(mem: &[u8], addr: usize) -> Result<u64, String> {
    Ok(u64::from_be_bytes(
        mem[range(mem, addr, 8)?].try_into().unwrap(),
    ))
}

/// Execute integer operations on all 64 bits, retaining the AMODE 31 address ABI.
pub(super) fn execute(
    ins: &Instruction,
    low: &mut [u32; 16],
    high: &mut [u32; 16],
    mem: &mut [u8],
    cc: &mut u8,
) -> Result<bool, String> {
    let op = ins.op.as_str();
    if !matches!(
        op,
        "LG" | "STG"
            | "LGF"
            | "LLGF"
            | "LGH"
            | "LLGH"
            | "LLGC"
            | "LGR"
            | "LGFR"
            | "LLGFR"
            | "LTGR"
            | "LCGR"
            | "LPGR"
            | "LNGR"
            | "AG"
            | "SG"
            | "ALG"
            | "SLG"
            | "CG"
            | "CLG"
            | "NG"
            | "OG"
            | "XG"
            | "AGR"
            | "SGR"
            | "ALGR"
            | "SLGR"
            | "CGR"
            | "CLGR"
            | "NGR"
            | "OGR"
            | "XGR"
            | "AGF"
            | "SGF"
            | "CGF"
            | "AGFR"
            | "SGFR"
            | "CGFR"
            | "MSG"
            | "MSGR"
            | "MGHI"
            | "LGHI"
            | "AGHI"
            | "CGHI"
            | "LGFI"
            | "AGFI"
            | "CGFI"
            | "LMG"
            | "STMG"
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
    let a = read(low, high, r);
    let ea = || address(ins.address.as_ref().ok_or("missing address operand")?, low);
    if matches!(op, "LMG" | "STMG") {
        let last = reg(1)?;
        let count = (last + 16 - r) % 16 + 1;
        let addr = ea()?;
        range(mem, addr, count * 8)?;
        // Compute the complete address before changing any base register.
        for n in 0..count {
            let target = (r + n) % 16;
            if op == "LMG" {
                write(low, high, target, doubleword(mem, addr + n * 8)?);
            } else {
                mem[addr + n * 8..addr + (n + 1) * 8]
                    .copy_from_slice(&read(low, high, target).to_be_bytes());
            }
        }
        return Ok(true);
    }
    if op == "STG" {
        let dest = range(mem, ea()?, 8)?;
        mem[dest].copy_from_slice(&a.to_be_bytes());
        return Ok(true);
    }
    let b = if matches!(
        op,
        "LGHI" | "AGHI" | "CGHI" | "MGHI" | "LGFI" | "AGFI" | "CGFI"
    ) {
        ins.immediate as i32 as i64 as u64
    } else if op.ends_with('R') {
        let src = reg(1)?;
        if matches!(op, "LGFR" | "AGFR" | "SGFR" | "CGFR") {
            low[src] as i32 as i64 as u64
        } else if op == "LLGFR" {
            u64::from(low[src])
        } else {
            read(low, high, src)
        }
    } else {
        let addr = ea()?;
        match op {
            "LGF" | "AGF" | "SGF" | "CGF" => word(mem, addr)? as i32 as i64 as u64,
            "LLGF" => u64::from(word(mem, addr)?),
            "LGH" | "LLGH" => {
                let bytes = &mem[range(mem, addr, 2)?];
                let n = u16::from_be_bytes([bytes[0], bytes[1]]);
                if op == "LGH" {
                    n as i16 as i64 as u64
                } else {
                    u64::from(n)
                }
            }
            "LLGC" => u64::from(mem[range(mem, addr, 1)?][0]),
            _ => doubleword(mem, addr)?,
        }
    };
    let signed_cc = |v: u64| ordering_cc((v as i64).cmp(&0));
    let result = match op {
        "LG" | "LGF" | "LLGF" | "LGH" | "LLGH" | "LLGC" | "LGR" | "LGFR" | "LLGFR" | "LGHI"
        | "LGFI" => b,
        "LTGR" => {
            *cc = signed_cc(b);
            b
        }
        "LCGR" | "LPGR" | "LNGR" => {
            let (v, overflow) = match op {
                "LCGR" => (b as i64).overflowing_neg(),
                "LPGR" => (b as i64).overflowing_abs(),
                _ => (
                    if (b as i64) > 0 {
                        -(b as i64)
                    } else {
                        b as i64
                    },
                    false,
                ),
            };
            *cc = if overflow { 3 } else { signed_cc(v as u64) };
            v as u64
        }
        "CG" | "CGR" | "CGF" | "CGFR" | "CGHI" | "CGFI" => {
            *cc = ordering_cc((a as i64).cmp(&(b as i64)));
            return Ok(true);
        }
        "CLG" | "CLGR" => {
            *cc = ordering_cc(a.cmp(&b));
            return Ok(true);
        }
        "AG" | "AGR" | "AGF" | "AGFR" | "AGHI" | "AGFI" | "SG" | "SGR" | "SGF" | "SGFR" => {
            let (v, overflow) = if op.starts_with('A') {
                (a as i64).overflowing_add(b as i64)
            } else {
                (a as i64).overflowing_sub(b as i64)
            };
            *cc = if overflow { 3 } else { signed_cc(v as u64) };
            v as u64
        }
        "ALG" | "ALGR" | "SLG" | "SLGR" => {
            let (v, carry) = if op.starts_with('A') {
                a.overflowing_add(b)
            } else {
                let (v, borrow) = a.overflowing_sub(b);
                (v, !borrow)
            };
            *cc = u8::from(v != 0) + 2 * u8::from(carry);
            v
        }
        "NG" | "NGR" | "OG" | "OGR" | "XG" | "XGR" => {
            let v = match op.as_bytes()[0] {
                b'N' => a & b,
                b'O' => a | b,
                _ => a ^ b,
            };
            *cc = u8::from(v != 0);
            v
        }
        "MSG" | "MSGR" | "MGHI" => a.wrapping_mul(b),
        _ => unreachable!(),
    };
    write(low, high, r, result);
    Ok(true)
}

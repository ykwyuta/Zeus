//! Builds an IPL-able ESA/390 storage image running a list of cases, and
//! decodes the results from a storage dump.
//!
//! Layout (real addresses):
//!
//! ```text
//! 0x0000  IPL PSW -> 0x1000            0x0068  program new PSW -> 0x0F00
//! 0x0F00  program-check handler: MVC 4(2,R11),X'8E'(R0); BR R12
//! 0x1000  case blocks, 0x80 bytes each, then the final block
//! ```
//!
//! A case block (R13 = block + 2 after BASR):
//!
//! ```text
//! +0x00 BASR 13,0                 +0x02 LA 11,slot(13)   +0x06 LA 12,next(13)
//! +0x0A SR 1,1                    +0x0C instruction (6 bytes)
//! +0x12 IPM 1                     +0x16 ST 1,0(11)       +0x1A BR 12
//! +0x40 operand 1 (16 bytes)      +0x50 operand 2 (16 bytes)
//! +0x60 slot: condition-code word (X'FFFFFFFF' if not reached),
//!             program interruption code (0 if none)
//! ```
//!
//! The final block stores a marker byte and loads a disabled-wait PSW, which
//! stops the CPU.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::cases::{Case, Op};

pub const START: usize = 0x1000;
pub const BLOCK: usize = 0x80;
const OP1: usize = 0x40;
const OP2: usize = 0x50;
const SLOT: usize = 0x60;
/// R13 points 2 bytes into the block.
const BASE: usize = 2;
const MARKER: u8 = 0xEE;

/// Outcome of one case as observed on the oracle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Observed {
    #[serde(with = "crate::cases::hex")]
    pub op1: Vec<u8>,
    #[serde(with = "crate::cases::hex")]
    pub op2: Vec<u8>,
    /// Condition code; `None` when a program interruption occurred.
    pub cc: Option<u8>,
    /// Program interruption code (e.g. 7 = data exception).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interruption: Option<u16>,
}

pub fn instruction(case: &Case) -> [u8; 6] {
    let d1 = (OP1 - BASE) as u16;
    let d2 = (OP2 - BASE) as u16;
    let l1 = case.op1.len() as u8 - 1;
    let (lens, b2d2) = match case.op {
        Op::Srp => {
            let (shift, round) = case.srp.unwrap_or((0, 0));
            // second-operand address = shift amount (base register 0)
            ((l1 << 4) | (round & 0xF), shift as u16 & 0x3F)
        }
        _ => {
            let l2 = case.op2.len() as u8 - 1;
            ((l1 << 4) | l2, 0xD000 | d2)
        }
    };
    let b1d1 = 0xD000 | d1;
    [
        case.op.opcode(),
        lens,
        (b1d1 >> 8) as u8,
        b1d1 as u8,
        (b2d2 >> 8) as u8,
        b2d2 as u8,
    ]
}

fn la(r: u8, disp: usize) -> [u8; 4] {
    let d = disp - BASE;
    [0x41, r << 4, 0xD0 | (d >> 8) as u8, d as u8]
}

/// PSW mask byte 2: decimal-overflow mask bit (bit 21).
fn program_mask(decimal_overflow_interrupt: bool) -> u8 {
    if decimal_overflow_interrupt {
        0x04
    } else {
        0x00
    }
}

pub fn build(cases: &[Case], decimal_overflow_interrupt: bool) -> Vec<u8> {
    let end = START + (cases.len() + 1) * BLOCK;
    let mut core = vec![0u8; end.next_multiple_of(0x1000)];
    let mask = program_mask(decimal_overflow_interrupt);
    // IPL PSW: EC mode (bit 12), 31-bit addressing, start at 0x1000.
    core[0..8].copy_from_slice(&[0x00, 0x08, mask, 0x00, 0x80, 0x00, 0x10, 0x00]);
    // Program new PSW -> handler at 0xF00 (same program mask).
    core[0x68..0x70].copy_from_slice(&[0x00, 0x08, mask, 0x00, 0x80, 0x00, 0x0F, 0x00]);
    // Handler: MVC 4(2,R11),X'8E'(R0) ; BR R12
    core[0xF00..0xF08].copy_from_slice(&[0xD2, 0x01, 0xB0, 0x04, 0x00, 0x8E, 0x07, 0xFC]);

    for (i, case) in cases.iter().enumerate() {
        let b = START + i * BLOCK;
        let mut code = vec![0x0D, 0xD0];
        code.extend_from_slice(&la(11, SLOT));
        code.extend_from_slice(&la(12, BLOCK));
        code.extend_from_slice(&[0x1B, 0x11]);
        code.extend_from_slice(&instruction(case));
        code.extend_from_slice(&[0xB2, 0x22, 0x00, 0x10]); // IPM 1
        code.extend_from_slice(&[0x50, 0x10, 0xB0, 0x00]); // ST 1,0(11)
        code.extend_from_slice(&[0x07, 0xFC]); // BR 12
        core[b..b + code.len()].copy_from_slice(&code);
        core[b + OP1..b + OP1 + case.op1.len()].copy_from_slice(&case.op1);
        core[b + OP2..b + OP2 + case.op2.len()].copy_from_slice(&case.op2);
        core[b + SLOT..b + SLOT + 4].copy_from_slice(&[0xFF; 4]);
    }

    let f = START + cases.len() * BLOCK;
    let mvi_disp = (0x3E - BASE) as u16;
    let psw_disp = (0x48 - BASE) as u16;
    let code = [
        0x0D,
        0xD0,
        0x92,
        MARKER,
        0xD0 | (mvi_disp >> 8) as u8,
        mvi_disp as u8,
        0x82,
        0x00,
        0xD0 | (psw_disp >> 8) as u8,
        psw_disp as u8,
    ];
    core[f..f + code.len()].copy_from_slice(&code);
    // Disabled wait PSW (wait bit 14).
    core[f + 0x48..f + 0x50].copy_from_slice(&[0x00, 0x0A, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00]);
    core
}

/// Last address that must be saved to read all results.
pub fn dump_end(count: usize) -> usize {
    START + (count + 1) * BLOCK - 1
}

pub fn decode(cases: &[Case], dump: &[u8]) -> Result<Vec<Observed>> {
    let f = START + cases.len() * BLOCK;
    if dump.len() <= f + 0x3E || dump[f + 0x3E] != MARKER {
        bail!("end marker not found: the test program did not run to completion");
    }
    Ok(cases
        .iter()
        .enumerate()
        .map(|(i, case)| {
            let b = START + i * BLOCK;
            let word = u32::from_be_bytes(dump[b + SLOT..b + SLOT + 4].try_into().unwrap());
            let pic = u16::from_be_bytes([dump[b + SLOT + 4], dump[b + SLOT + 5]]);
            Observed {
                op1: dump[b + OP1..b + OP1 + case.op1.len()].to_vec(),
                op2: dump[b + OP2..b + OP2 + case.op2.len()].to_vec(),
                cc: (word != u32::MAX).then_some(((word >> 28) & 3) as u8),
                interruption: (pic != 0).then_some(pic),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ap() -> Case {
        Case {
            op: Op::Ap,
            op1: vec![0x12, 0x3C],
            op2: vec![0x45, 0x6C],
            srp: None,
        }
    }

    #[test]
    fn encodes_ss_instruction() {
        assert_eq!(instruction(&ap()), [0xFA, 0x11, 0xD0, 0x3E, 0xD0, 0x4E]);
        let srp = Case {
            op: Op::Srp,
            op1: vec![0x00, 0x12, 0x3C],
            op2: vec![],
            srp: Some((0x3F, 5)),
        };
        assert_eq!(instruction(&srp), [0xF0, 0x25, 0xD0, 0x3E, 0x00, 0x3F]);
    }

    #[test]
    fn image_layout_and_decode() {
        let cases = [ap()];
        let core = build(&cases, false);
        assert_eq!(&core[0..8], &[0, 8, 0, 0, 0x80, 0, 0x10, 0]);
        assert_eq!(&core[START + OP1..START + OP1 + 2], &[0x12, 0x3C]);
        // A dump without the marker is rejected.
        assert!(decode(&cases, &core).is_err());
        // Simulate the oracle: CC2, result 579C, marker set.
        let mut dump = core.clone();
        dump[START + OP1..START + OP1 + 2].copy_from_slice(&[0x57, 0x9C]);
        dump[START + SLOT..START + SLOT + 4].copy_from_slice(&[0x20, 0, 0, 0]);
        dump[START + BLOCK + 0x3E] = MARKER;
        let obs = decode(&cases, &dump).unwrap();
        assert_eq!(obs[0].op1, [0x57, 0x9C]);
        assert_eq!(obs[0].cc, Some(2));
        assert_eq!(obs[0].interruption, None);
    }
}

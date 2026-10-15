//! RSY-a/b and SIY share the signed twenty-bit displacement but have
//! distinct register, mask and immediate fields. No index register is present.
use super::{Address, Instruction};
use crate::hlasm::operation;

pub(super) fn decode(i: &mut Instruction, b: &[u8]) -> Option<&'static str> {
    let op = match b[5] {
        0x04 => "LMG",
        0x0a => "SRAG",
        0x0b => "SLAG",
        0x0c => "SRLG",
        0x0d => "SLLG",
        0x14 => "CSY",
        0x1c => "RLLG",
        0x1d => "RLL",
        0x20 => "CLMH",
        0x21 => "CLMY",
        0x24 => "STMG",
        0x2c => "STCMH",
        0x2d => "STCMY",
        0x30 => "CSG",
        0x31 => "CDSY",
        0x3e => "CDSG",
        0x51 => "TMY",
        0x52 => "MVIY",
        0x54 => "NIY",
        0x55 => "CLIY",
        0x56 => "OIY",
        0x57 => "XIY",
        0x80 => "ICMH",
        0x81 => "ICMY",
        0x90 => "STMY",
        0x98 => "LMY",
        0xe0 => "LOCFH",
        0xe1 => "STOCFH",
        0xe2 => "LOCG",
        0xe3 => "STOCG",
        0xe4 => "LANG",
        0xe6 => "LAOG",
        0xe7 => "LAXG",
        0xe8 => "LAAG",
        0xea => "LAALG",
        0xf2 => "LOC",
        0xf3 => "STOC",
        0xf4 => "LAN",
        0xf6 => "LAO",
        0xf7 => "LAX",
        0xf8 => "LAA",
        0xfa => "LAAL",
        _ => return None,
    };
    let displacement =
        ((i32::from(b[4]) << 12) | (i32::from(b[2] & 15) << 8) | i32::from(b[3])) << 12 >> 12;
    i.address = Some(Address {
        base: b[2] >> 4,
        index: 0,
        displacement: displacement.into(),
    });
    if (0x51..=0x57).contains(&b[5]) {
        i.immediate = u32::from(b[1]);
    } else if operation::masked_bytes(op).is_some() || operation::conditional(op).is_some() {
        i.registers = vec![b[1] >> 4];
        i.immediate = u32::from(b[1] & 15);
    } else {
        i.registers = vec![b[1] >> 4, b[1] & 15];
    }
    Some(op)
}

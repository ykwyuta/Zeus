//! RXE, RXF and RSL-b layouts from the architectural instruction-format tables.
use super::{Instruction, address};
use crate::hlasm::float::operations as fp;

pub(super) fn decode(i: &mut Instruction, b: &[u8]) -> Option<&'static str> {
    let full = u16::from_be_bytes([b[0], b[5]]);
    let op = match b[5] {
        0x04 => "LDEB",
        0x05 => "LXDB",
        0x06 => "LXEB",
        0x07 => "MXDB",
        0x08 => "KEB",
        0x09 => "CEB",
        0x0a => "AEB",
        0x0b => "SEB",
        0x0c => "MDEB",
        0x0d => "DEB",
        0x0e => "MAEB",
        0x0f => "MSEB",
        0x10 => "TCEB",
        0x11 => "TCDB",
        0x12 => "TCXB",
        0x14 => "SQEB",
        0x15 => "SQDB",
        0x17 => "MEEB",
        0x18 => "KDB",
        0x19 => "CDB",
        0x1a => "ADB",
        0x1b => "SDB",
        0x1c => "MDB",
        0x1d => "DDB",
        0x1e => "MADB",
        0x1f => "MSDB",
        0x40 => "SLDT",
        0x41 => "SRDT",
        0x48 => "SLXT",
        0x49 => "SRXT",
        0x50 => "TDCET",
        0x51 => "TDGET",
        0x54 => "TDCDT",
        0x55 => "TDGDT",
        0x58 => "TDCXT",
        0x59 => "TDGXT",
        0xa8 => "CZDT",
        0xa9 => "CZXT",
        0xaa => "CDZT",
        0xab => "CXZT",
        0xac => "CPDT",
        0xad => "CPXT",
        0xae => "CDPT",
        0xaf => "CXPT",
        _ => fp::hex_name(full).or_else(|| fp::hex_unnormalized_name(full))?,
    };
    if fp::decimal_memory(op).is_some() {
        // RSL-b has an eight-bit length, no index, and R1/M3 in byte four.
        i.registers = vec![b[4] >> 4];
        i.immediate = u32::from(b[4] & 15);
        i.length = u32::from(b[1]) + 1;
        i.address = Some(address(&b[2..4], 0));
    } else {
        i.address = Some(address(&b[2..4], b[1] & 15));
        let fused = fp::memory(op).is_some_and(|(_, _, kind)| {
            matches!(kind, fp::Kind::FusedAdd | fp::Kind::FusedSubtract)
        }) || fp::hex(op)
            .is_some_and(|s| matches!(s.kind, fp::Kind::FusedAdd | fp::Kind::FusedSubtract))
            || fp::hex_unnormalized(op).is_some()
            || fp::decimal_shift(op).is_some();
        i.registers = if fused {
            vec![b[4] >> 4, b[1] >> 4]
        } else {
            vec![b[1] >> 4]
        };
    }
    Some(op)
}

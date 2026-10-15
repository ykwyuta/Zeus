//! RRE, RRF and RRD register fields for architectural instruction fetch.
use crate::hlasm::{
    Instruction,
    float::{convert, operations as fp},
    operation, template,
};

pub(super) fn decode(i: &mut Instruction, b: &[u8]) -> Option<&'static str> {
    let full = u16::from_be_bytes([b[0], b[1]]);
    let op = template::RRE
        .iter()
        .find(|s| s.1 == b[0] && s.2 == b[1])
        .map(|s| s.0)
        .or_else(|| fp::hex_name(full))
        .or_else(|| fp::hex_unnormalized_name(full))
        .or_else(|| convert::name(full))
        .or_else(|| {
            if b[0] == 0xb3 {
                fp::RRE
                    .iter()
                    .find(|s| s.1 == b[1])
                    .map(|s| s.0)
                    .or_else(|| fp::COMMON.iter().find(|s| s.1 == b[1]).map(|s| s.0))
                    .or_else(|| fp::support_name(b[1]))
                    .or_else(|| fp::divide_integer_name(b[1]))
                    .or_else(|| fp::decimal_name(b[1]))
                    .or_else(|| fp::decimal_packed_name(b[1]))
                    .or_else(|| fp::decimal_bits_name(b[1]))
                    .or(match b[1] {
                        0x65 => Some("LXR"),
                        0x84 => Some("SFPC"),
                        0x8c => Some("EFPC"),
                        _ => None,
                    })
            } else {
                None
            }
        })?;
    i.registers = vec![b[3] >> 4, b[3] & 15];
    if fp::divide_integer(op).is_some() {
        i.registers = vec![b[3] >> 4, b[2] >> 4, b[3] & 15];
        i.immediate = u32::from(b[2] & 15);
    } else if operation::register_logic(op).is_some() || operation::select(op).is_some() {
        i.registers.push(b[2] >> 4);
        if operation::select(op).is_some() {
            i.immediate = u32::from(b[2] & 15);
        }
    } else if operation::conditional(op).is_some() || op == "POPCNT" {
        i.immediate = u32::from(b[2] >> 4);
    } else if fp::support(op).is_some_and(|s| s.kind == fp::SupportKind::CopySign)
        || fp::decimal_bits(op).is_some_and(|s| {
            matches!(
                s.kind,
                fp::DecimalBitKind::Insert
                    | fp::DecimalBitKind::Quantize
                    | fp::DecimalBitKind::Reround
            )
        })
    {
        i.registers = vec![b[3] >> 4, b[2] >> 4, b[3] & 15];
        i.immediate = u32::from(b[2] & 15);
    } else if fp::hex_unnormalized(op).is_some()
        || fp::hex(op)
            .is_some_and(|s| matches!(s.kind, fp::Kind::FusedAdd | fp::Kind::FusedSubtract))
        || fp::register(op)
            .is_some_and(|s| matches!(s.3, fp::Kind::FusedAdd | fp::Kind::FusedSubtract))
    {
        i.registers = vec![b[2] >> 4, b[3] >> 4, b[3] & 15];
    } else if fp::decimal(op).is_some_and(|s| s.arithmetic) {
        i.registers.push(b[2] >> 4);
        i.immediate = u32::from(b[2] & 15);
    } else if convert::instruction(op).is_some()
        || fp::support(op).is_some_and(|s| s.kind == fp::SupportKind::HexToBinary)
        || fp::binary_controlled(op)
        || fp::decimal_packed(op).is_some_and(|s| s.mask())
        || fp::decimal(op)
            .is_some_and(|s| matches!(s.kind, fp::Kind::Convert | fp::Kind::RoundInteger))
    {
        i.immediate = u32::from(b[2] >> 4) | (u32::from(b[2] & 15) << 4);
    }
    Some(op)
}

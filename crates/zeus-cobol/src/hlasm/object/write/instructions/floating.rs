//! Architectural RR/RRE/RRF/RRD/RXE/RXF/RSL-b floating-point output.
//! This writes instruction fields from decoded operands, never template bytes.
use super::{address, nibble};
use crate::hlasm::{
    Instruction,
    float::{convert, operations as fp},
};

pub(super) fn encode(i: &Instruction) -> Result<Option<Vec<u8>>, String> {
    let op = i.op.as_str();
    let r = |n: usize| {
        i.registers
            .get(n)
            .copied()
            .ok_or_else(|| format!("{op}: missing register operand {}", n + 1))
            .and_then(|r| nibble(u32::from(r)))
    };
    let fused = |k| matches!(k, fp::Kind::FusedAdd | fp::Kind::FusedSubtract);
    let memory_opcode = fp::memory(op)
        .map(|s| s.0)
        .or_else(|| {
            fp::hex(op)
                .filter(|s| s.memory && s.opcode > 255)
                .map(|s| s.opcode as u8)
        })
        .or_else(|| {
            fp::hex_unnormalized(op)
                .filter(|s| s.memory)
                .map(|s| s.opcode as u8)
        })
        .or(match op {
            "TCEB" => Some(0x10),
            "TCDB" => Some(0x11),
            "TCXB" => Some(0x12),
            "SLDT" => Some(0x40),
            "SRDT" => Some(0x41),
            "SLXT" => Some(0x48),
            "SRXT" => Some(0x49),
            "TDCET" => Some(0x50),
            "TDGET" => Some(0x51),
            "TDCDT" => Some(0x54),
            "TDGDT" => Some(0x55),
            "TDCXT" => Some(0x58),
            "TDGXT" => Some(0x59),
            _ => None,
        });
    if let Some(opcode) = memory_opcode {
        let a = i
            .address
            .as_ref()
            .ok_or("missing floating-point storage operand")?;
        let pair = address(Some(a), true)?;
        let extra = fp::memory(op).is_some_and(|s| fused(s.2))
            || fp::hex(op).is_some_and(|s| fused(s.kind))
            || fp::hex_unnormalized(op).is_some()
            || fp::decimal_shift(op).is_some();
        return Ok(Some(if extra {
            vec![
                0xed,
                r(1)? << 4 | a.index,
                pair[0],
                pair[1],
                r(0)? << 4,
                opcode,
            ]
        } else {
            vec![0xed, r(0)? << 4 | a.index, pair[0], pair[1], 0, opcode]
        }));
    }
    if let Some(spec) = fp::decimal_memory(op) {
        let opcode = match op {
            "CZDT" => 0xa8,
            "CZXT" => 0xa9,
            "CDZT" => 0xaa,
            "CXZT" => 0xab,
            "CPDT" => 0xac,
            "CPXT" => 0xad,
            "CDPT" => 0xae,
            _ => 0xaf,
        };
        if i.length == 0 || i.length > spec.max_length() {
            return Err("DFP packed/zoned length out of range".into());
        }
        let a = address(i.address.as_ref(), false)?;
        return Ok(Some(vec![
            0xed,
            (i.length - 1) as u8,
            a[0],
            a[1],
            r(0)? << 4 | nibble(i.immediate)?,
            opcode,
        ]));
    }
    if let Some(opcode) = match op {
        "LFPC" => Some(0x9d),
        "STFPC" => Some(0x9c),
        "SRNM" => Some(0x99),
        "SRNMB" => Some(0xb8),
        "SRNMT" => Some(0xb9),
        _ => None,
    } {
        let a = address(i.address.as_ref(), false)?;
        return Ok(Some(vec![0xb2, opcode, a[0], a[1]]));
    }
    let opcode = convert::instruction(op)
        .map(|s| s.opcode)
        .or_else(|| fp::register(op).map(|s| 0xb300 | u16::from(s.0)))
        .or_else(|| fp::common(op).map(|s| 0xb300 | u16::from(s)))
        .or_else(|| fp::hex(op).filter(|s| !s.memory).map(|s| s.opcode))
        .or_else(|| {
            fp::hex_unnormalized(op)
                .filter(|s| !s.memory)
                .map(|s| s.opcode)
        })
        .or_else(|| fp::support(op).map(|s| 0xb300 | u16::from(s.opcode)))
        .or_else(|| fp::divide_integer(op).map(|s| 0xb300 | u16::from(s.0)))
        .or_else(|| fp::decimal(op).map(|s| 0xb300 | u16::from(s.opcode)))
        .or_else(|| fp::decimal_packed(op).map(|s| 0xb300 | u16::from(s.opcode)))
        .or_else(|| fp::decimal_bits(op).map(|s| 0xb300 | u16::from(s.opcode)))
        .or(match op {
            "IPM" => Some(0xb222),
            "LXR" => Some(0xb365),
            "SFPC" => Some(0xb384),
            "EFPC" => Some(0xb38c),
            _ => None,
        });
    let Some(opcode) = opcode else {
        return Ok(None);
    };
    if opcode < 256 {
        return Ok(Some(vec![opcode as u8, r(0)? << 4 | r(1)?]));
    }
    let (byte2, byte3) = if fp::divide_integer(op).is_some()
        || fp::support(op).is_some_and(|s| s.kind == fp::SupportKind::CopySign)
        || fp::decimal_bits(op).is_some_and(|s| {
            matches!(
                s.kind,
                fp::DecimalBitKind::Insert
                    | fp::DecimalBitKind::Quantize
                    | fp::DecimalBitKind::Reround
            )
        }) {
        (r(1)? << 4 | nibble(i.immediate)?, r(0)? << 4 | r(2)?)
    } else if fp::hex_unnormalized(op).is_some()
        || fp::hex(op).is_some_and(|s| fused(s.kind))
        || fp::register(op).is_some_and(|s| fused(s.3))
    {
        (r(0)? << 4, r(1)? << 4 | r(2)?)
    } else if fp::decimal(op).is_some_and(|s| s.arithmetic) {
        (r(2)? << 4 | nibble(i.immediate)?, r(0)? << 4 | r(1)?)
    } else if convert::instruction(op).is_some()
        || fp::support(op).is_some_and(|s| s.kind == fp::SupportKind::HexToBinary)
        || fp::binary_controlled(op)
        || fp::decimal_packed(op).is_some_and(|s| s.mask())
        || fp::decimal(op)
            .is_some_and(|s| matches!(s.kind, fp::Kind::Convert | fp::Kind::RoundInteger))
    {
        if i.immediate > 255 {
            return Err("floating-point mask exceeds two nibbles".into());
        }
        (
            ((i.immediate as u8 & 15) << 4) | (i.immediate >> 4) as u8,
            r(0)? << 4 | r(1)?,
        )
    } else {
        let second = if matches!(op, "IPM" | "SFPC" | "EFPC" | "LZER" | "LZDR" | "LZXR") {
            0
        } else {
            r(1)?
        };
        (0, r(0)? << 4 | second)
    };
    Ok(Some(vec![(opcode >> 8) as u8, opcode as u8, byte2, byte3]))
}

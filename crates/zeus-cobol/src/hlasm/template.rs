//! EXECUTE overlays for source-assembled Zeus instruction templates. These
//! modules are not IBM object decks: unrepresented byte changes are rejected.
use super::{Instruction, assemble};

enum Format {
    Length,
    Decimal,
    Srp,
    Immediate,
    Mask,
    TwoRegisters,
    RegisterIndex,
    FusedMemory,
    AddressMode(u8),
    ProgramMask,
    RegisterOpcode(u8),
    Relative(u8),
    Rre(u8, u8),
    Opaque,
    Ignored,
    ConditionalImmediate,
}

// Fixed opcode bytes in RRE occupy the second byte, while the registers
// remain in the fourth byte. EXECUTE must not reinterpret them as RR fields.
pub(super) const RRE: &[(&str, u8, u8)] = &[
    ("IPM", 0xb2, 0x22),
    ("NRK", 0xB9, 0xF4),
    ("NGRK", 0xB9, 0xE4),
    ("ORK", 0xB9, 0xF6),
    ("OGRK", 0xB9, 0xE6),
    ("XRK", 0xB9, 0xF7),
    ("XGRK", 0xB9, 0xE7),
    ("NNRK", 0xB9, 0x74),
    ("NNGRK", 0xB9, 0x64),
    ("NORK", 0xB9, 0x76),
    ("NOGRK", 0xB9, 0x66),
    ("NXRK", 0xB9, 0x77),
    ("NXGRK", 0xB9, 0x67),
    ("NCRK", 0xB9, 0xF5),
    ("NCGRK", 0xB9, 0xE5),
    ("OCRK", 0xB9, 0x75),
    ("OCGRK", 0xB9, 0x65),
    ("SELR", 0xB9, 0xF0),
    ("SELGR", 0xB9, 0xE3),
    ("SELFHR", 0xB9, 0xC0),
    ("FLOGR", 0xB9, 0x83),
    ("POPCNT", 0xB9, 0xE1),
    ("LOCR", 0xB9, 0xF2),
    ("LOCGR", 0xB9, 0xE2),
    ("LOCFHR", 0xB9, 0xE0),
    ("LGR", 0xB9, 0x04),
    ("LGFR", 0xB9, 0x14),
    ("LLGFR", 0xB9, 0x16),
    ("LTGR", 0xB9, 0x02),
    ("LCGR", 0xB9, 0x03),
    ("LPGR", 0xB9, 0x00),
    ("LNGR", 0xB9, 0x01),
    ("AGR", 0xB9, 0x08),
    ("SGR", 0xB9, 0x09),
    ("ALGR", 0xB9, 0x0A),
    ("SLGR", 0xB9, 0x0B),
    ("CGR", 0xB9, 0x20),
    ("CLGR", 0xB9, 0x21),
    ("NGR", 0xB9, 0x80),
    ("OGR", 0xB9, 0x81),
    ("XGR", 0xB9, 0x82),
    ("AGFR", 0xB9, 0x18),
    ("SGFR", 0xB9, 0x19),
    ("CGFR", 0xB9, 0x30),
    ("MSGR", 0xB9, 0x0C),
    ("MLGR", 0xB9, 0x86),
    ("DLGR", 0xB9, 0x87),
    ("MLR", 0xB9, 0x96),
    ("DLR", 0xB9, 0x97),
    ("DSGR", 0xB9, 0x0D),
    ("DSGFR", 0xB9, 0x1D),
    ("BCTGR", 0xB9, 0x46),
    ("ALCR", 0xB9, 0x98),
    ("ALCGR", 0xB9, 0x88),
    ("SLBR", 0xB9, 0x99),
    ("SLBGR", 0xB9, 0x89),
    ("LRVR", 0xB9, 0x1F),
    ("LRVGR", 0xB9, 0x0F),
    ("LBR", 0xB9, 0x26),
    ("LGBR", 0xB9, 0x06),
    ("MVST", 0xB2, 0x55),
    ("CLST", 0xB2, 0x5D),
    ("SRST", 0xB2, 0x5E),
];

fn format(ins: &Instruction) -> Format {
    use Format::*;
    let op = assemble::canonical_operation(&ins.op);
    if let Some(byte) = super::float::operations::common(op) {
        return Rre(0xb3, byte);
    }
    if op == "PFPO" {
        return AddressMode(10);
    }
    if matches!(op, "SAM24" | "SAM31" | "SAM64") {
        return AddressMode(if op == "SAM24" {
            12
        } else if op == "SAM31" {
            13
        } else {
            14
        });
    }
    if super::float::operations::classify(op).is_some() {
        return RegisterIndex;
    }
    if super::float::operations::decimal_shift(op).is_some() {
        return FusedMemory;
    }
    if let Some((opcode, _)) = super::float::operations::divide_integer(op) {
        return Rre(0xb3, opcode);
    }
    if let Some(spec) = super::float::operations::support(op) {
        return Rre(0xb3, spec.opcode);
    }
    if super::float::operations::decimal_memory(op).is_some() {
        return Length;
    }
    if let Some(spec) = super::float::operations::hex_unnormalized(op) {
        return if spec.memory {
            FusedMemory
        } else {
            Rre((spec.opcode >> 8) as u8, spec.opcode as u8)
        };
    }
    if let Some(spec) = super::float::operations::decimal_packed(op) {
        return Rre(0xb3, spec.opcode);
    }
    if let Some(spec) = super::float::operations::decimal_bits(op) {
        return Rre(0xb3, spec.opcode);
    }
    if let Some(spec) = super::float::operations::decimal(op) {
        return Rre(0xb3, spec.opcode);
    }
    if let Some(spec) = super::float::convert::instruction(op) {
        return Rre((spec.opcode >> 8) as u8, spec.opcode as u8);
    }
    if let Some(spec) = super::float::operations::hex(op) {
        if !spec.memory && spec.size == 4 {
            return Rre((spec.opcode >> 8) as u8, spec.opcode as u8);
        }
        if spec.size == 2 {
            return TwoRegisters;
        }
    }
    if let Some((byte, _, _, _)) = super::float::operations::register(op) {
        return Rre(0xb3, byte);
    }
    if op == "LXR" {
        return Rre(0xb3, 0x65);
    }
    if op == "SFPC" {
        return Rre(0xb3, 0x84);
    }
    if op == "EFPC" {
        return Rre(0xb3, 0x8c);
    }
    if matches!(op, "LFPC" | "STFPC" | "SRNM" | "SRNMB" | "SRNMT") {
        return Ignored;
    }

    if super::float::operations::hex(op).is_some_and(|s| {
        s.memory
            && matches!(
                s.kind,
                super::float::operations::Kind::FusedAdd
                    | super::float::operations::Kind::FusedSubtract
            )
    }) || super::float::operations::memory(op).is_some_and(|(_, _, kind)| {
        matches!(
            kind,
            super::float::operations::Kind::FusedAdd
                | super::float::operations::Kind::FusedSubtract
        )
    }) {
        return FusedMemory;
    }
    if op == "SPM" {
        return ProgramMask;
    }
    let condition = super::operation::conditional(op);
    let opcode = condition.as_ref().map_or(op, |spec| spec.root);
    if super::operation::rotate_selected(op).is_some() {
        return TwoRegisters;
    }
    if op == "TS" {
        return Ignored;
    }
    if super::operation::interlocked(op).is_some() {
        return TwoRegisters;
    }
    if let Some((_, first, second)) = RRE.iter().find(|(name, _, _)| *name == opcode) {
        return Rre(*first, *second);
    }
    if let Some(spec) = condition {
        return if spec.kind == super::operation::ConditionalKind::Immediate {
            ConditionalImmediate
        } else {
            Mask
        };
    }
    if let Some(relative) = super::operation::relative(op) {
        use super::operation::RelativeKind::*;
        return Relative(match relative.kind {
            Condition(_) => 4,
            Save => 5,
            Count32 => 6,
            Count64 => 7,
            CountHigh => 6,
            LoadAddress | Execute => 0,
        });
    }
    match op {
        "MVC" | "CLC" | "XC" | "NC" | "OC" | "MVN" | "MVZ" | "TR" | "TRT" | "TRTR" | "ED"
        | "EDMK" => Length,
        "AP" | "SP" | "CP" | "ZAP" | "MP" | "DP" | "PACK" | "UNPK" => Decimal,
        "SRP" => Srp,
        "MVI" | "CLI" | "NI" | "OI" | "XI" | "TM" => Immediate,
        "BC" | "BCR" | "BR" => Mask,
        _ if super::operation::masked_bytes(&ins.op).is_some() => Mask,
        "MVCLE" | "CLCLE" | "STM" | "LM" | "LMG" | "STMG" | "SLLG" | "SRLG" | "SLAG" | "SRAG"
        | "RLL" | "RLLG" => TwoRegisters,
        "SLL" | "SRL" | "SLA" | "SRA" | "SLDL" | "SRDL" | "SLDA" | "SRDA" => RegisterOpcode(0),
        "CSST" => RegisterOpcode(2),
        "LPD" => RegisterOpcode(4),
        "LPDG" => RegisterOpcode(5),
        "LHI" => RegisterOpcode(8),
        "MHI" => RegisterOpcode(12),
        "CHI" => RegisterOpcode(14),
        "AHI" => RegisterOpcode(10),
        "LGHI" => RegisterOpcode(9),
        "AGHI" => RegisterOpcode(11),
        "MGHI" => RegisterOpcode(13),
        "CGHI" => RegisterOpcode(15),
        "LGFI" => RegisterOpcode(1),
        "AGFI" => RegisterOpcode(8),
        "CGFI" => RegisterOpcode(12),
        "IIHH" | "IIHL" | "IILH" | "IILL" | "NIHH" | "NIHL" | "NILH" | "NILL" | "OIHH" | "OIHL"
        | "OILH" | "OILL" | "LLIHH" | "LLIHL" | "LLILH" | "LLILL" => {
            const OPS: [&str; 16] = [
                "IIHH", "IIHL", "IILH", "IILL", "NIHH", "NIHL", "NILH", "NILL", "OIHH", "OIHL",
                "OILH", "OILL", "LLIHH", "LLIHL", "LLILH", "LLILL",
            ];
            RegisterOpcode(OPS.iter().position(|s| *s == op).unwrap() as u8)
        }
        "IIHF" => RegisterOpcode(8),
        "IILF" => RegisterOpcode(9),
        "LLIHF" => RegisterOpcode(10),
        "LLILF" => RegisterOpcode(11),
        "NIHF" => RegisterOpcode(12),
        "NILF" => RegisterOpcode(13),
        "OIHF" => RegisterOpcode(14),
        "OILF" => RegisterOpcode(15),
        "XIHF" => RegisterOpcode(6),
        "XILF" => RegisterOpcode(7),
        "TMHH" => RegisterOpcode(2),
        "TMHL" => RegisterOpcode(3),
        "TMLH" => RegisterOpcode(0),
        "TMLL" => RegisterOpcode(1),
        _ if assemble::instruction_size(&ins.op) == Some(2) => TwoRegisters,
        _ if ins.address.is_some()
            && !ins.registers.is_empty()
            && !matches!(
                op,
                "STIMER" | "GETMAIN" | "FREEMAIN" | "STORAGE" | "LINK" | "XCTL"
            )
            && super::operation::relative(&ins.op).is_none() =>
        {
            RegisterIndex
        }
        _ => Opaque,
    }
}

pub(super) fn byte(ins: &Instruction) -> u8 {
    use Format::*;
    let r = |n| ins.registers.get(n).copied().unwrap_or(0);
    match format(ins) {
        Length => ins.length.saturating_sub(1) as u8,
        Decimal => {
            (ins.length.saturating_sub(1) as u8) << 4 | ins.source_length.saturating_sub(1) as u8
        }
        Srp => (ins.length.saturating_sub(1) as u8) << 4 | ins.immediate as u8,
        Immediate => ins.immediate as u8,
        Mask if ins.op == "BC" => {
            (ins.immediate as u8) << 4 | ins.address.as_ref().map_or(0, |a| a.index)
        }
        Mask if ins.op == "BCR" || ins.op == "BR" => {
            (if ins.op == "BR" {
                15
            } else {
                ins.immediate as u8
            }) << 4
                | r(0)
        }
        Mask => r(0) << 4 | ins.immediate as u8,
        TwoRegisters => r(0) << 4 | r(1),
        RegisterIndex => r(0) << 4 | ins.address.as_ref().map_or(0, |a| a.index),
        FusedMemory => r(1) << 4 | ins.address.as_ref().map_or(0, |a| a.index),
        RegisterOpcode(op) => r(0) << 4 | op,
        Relative(op) => {
            let high = if matches!(
                super::operation::relative(&ins.op).unwrap().kind,
                super::operation::RelativeKind::Condition(_)
            ) {
                ins.immediate as u8
            } else {
                r(0)
            };
            high << 4 | op
        }
        Opaque | Ignored => 0,
        AddressMode(byte) => byte,
        ProgramMask => r(0) << 4,
        Rre(_, byte) => byte,
        ConditionalImmediate => r(0) << 4 | ins.length as u8,
    }
}

pub(super) fn apply(ins: &mut Instruction, byte: u8) -> Result<(), String> {
    use Format::*;
    let (high, low) = (byte >> 4, byte & 15);
    match format(ins) {
        ConditionalImmediate => {
            ins.registers[0] = high;
            ins.length = low.into();
        }
        Length => ins.length = u32::from(byte) + 1,
        Decimal => {
            ins.length = u32::from(high) + 1;
            ins.source_length = u32::from(low) + 1;
        }
        Srp => {
            ins.length = u32::from(high) + 1;
            ins.immediate = u32::from(low);
        }
        Immediate => ins.immediate = u32::from(byte),
        Mask if ins.op == "BC" => {
            ins.immediate = u32::from(high);
            ins.address.as_mut().ok_or("missing branch address")?.index = low;
        }
        Mask if matches!(ins.op.as_str(), "BCR" | "BR") => {
            ins.immediate = u32::from(high);
            ins.registers = vec![low];
            ins.op = "BCR".into();
        }
        Mask => {
            ins.registers[0] = high;
            ins.immediate = u32::from(low);
        }
        TwoRegisters => {
            ins.registers = vec![high, low];
        }
        FusedMemory => {
            ins.registers[1] = high;
            ins.address
                .as_mut()
                .ok_or("missing fused template address")?
                .index = low;
        }
        RegisterIndex => {
            ins.registers[0] = high;
            ins.address
                .as_mut()
                .ok_or("missing template address")?
                .index = low;
        }
        RegisterOpcode(_) if matches!(ins.op.as_str(), "LPD" | "LPDG") => {
            ins.op = match low {
                4 => "LPD",
                5 => "LPDG",
                _ => return Err("EX modification selects unsupported SSF opcode".into()),
            }
            .into();
            ins.registers[0] = high;
        }
        RegisterOpcode(op) => {
            if low != op {
                return Err("EX modification changes an unsupported opcode/reserved field".into());
            }
            ins.registers[0] = high;
        }
        Relative(op) => {
            if low != op {
                return Err("EX modification changes an unsupported opcode field".into());
            }
            if matches!(
                super::operation::relative(&ins.op).unwrap().kind,
                super::operation::RelativeKind::Condition(_)
            ) {
                ins.immediate = u32::from(high);
            } else {
                ins.registers = vec![high];
            }
        }
        AddressMode(_) => {
            ins.op = match byte {
                10 => "PFPO",
                12 => "SAM24",
                13 => "SAM31",
                14 => "SAM64",
                _ => return Err("EX selects an unsupported addressing-mode opcode".into()),
            }
            .into();
        }
        ProgramMask => ins.registers = vec![high],
        Ignored => {}
        Opaque if byte == 0 => {}
        Opaque => return Err("EX modification of this instruction format is unsupported".into()),
        Rre(first, _) => {
            let current_hex = super::float::operations::hex(&ins.op);
            if let Some(spec) = current_hex
                && spec.opcode == (u16::from(first) << 8) | u16::from(byte)
            {
                return Ok(());
            }
            let operation = RRE
                .iter()
                .find(|(_, primary, secondary)| *primary == first && *secondary == byte)
                .map(|s| s.0)
                .or_else(|| {
                    super::float::operations::hex_name(u16::from(first) << 8 | u16::from(byte))
                        .or_else(|| {
                            super::float::operations::hex_unnormalized_name(
                                u16::from(first) << 8 | u16::from(byte),
                            )
                        })
                })
                .or_else(|| {
                    (first == 0xb3)
                        .then(|| {
                            super::float::operations::RRE
                                .iter()
                                .find(|s| s.1 == byte)
                                .map(|s| s.0)
                        })
                        .flatten()
                })
                .or_else(|| {
                    if first == 0xb3 {
                        super::float::operations::COMMON
                            .iter()
                            .find(|s| s.1 == byte)
                            .map(|s| s.0)
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    if first == 0xb3 {
                        super::float::operations::support_name(byte)
                            .or_else(|| super::float::operations::divide_integer_name(byte))
                            .or_else(|| super::float::operations::decimal_name(byte))
                            .or_else(|| super::float::operations::decimal_packed_name(byte))
                            .or_else(|| super::float::operations::decimal_bits_name(byte))
                    } else {
                        None
                    }
                })
                .or_else(|| super::float::convert::name(u16::from(first) << 8 | u16::from(byte)))
                .or({
                    if first == 0xb3 {
                        match byte {
                            0x65 => Some("LXR"),
                            0x84 => Some("SFPC"),
                            0x8c => Some("EFPC"),
                            _ => None,
                        }
                    } else {
                        None
                    }
                })
                .ok_or("EX modification selects an unsupported RRE opcode")?;
            if first == 0xb3
                || super::float::convert::instruction(operation).is_some()
                || super::float::operations::decimal_packed(operation).is_some_and(|s| s.mask())
                || super::float::convert::instruction(&ins.op).is_some()
                || super::float::operations::decimal_packed(&ins.op).is_some_and(|s| s.mask())
            {
                let fused = |op: &str| {
                    super::float::operations::hex_unnormalized(op).is_some()
                        || super::float::operations::hex(op).is_some_and(|s| {
                            matches!(
                                s.kind,
                                super::float::operations::Kind::FusedAdd
                                    | super::float::operations::Kind::FusedSubtract
                            )
                        })
                        || super::float::operations::register(op).is_some_and(|(_, _, _, kind)| {
                            matches!(
                                kind,
                                super::float::operations::Kind::FusedAdd
                                    | super::float::operations::Kind::FusedSubtract
                            )
                        })
                };
                let r = |n| ins.registers.get(n).copied().unwrap_or(0);
                let insert = |op: &str| {
                    super::float::operations::divide_integer(op).is_some()
                        || super::float::operations::support(op).is_some_and(|s| {
                            s.kind == super::float::operations::SupportKind::CopySign
                        })
                        || super::float::operations::decimal_bits(op).is_some_and(|s| {
                            matches!(
                                s.kind,
                                super::float::operations::DecimalBitKind::Insert
                                    | super::float::operations::DecimalBitKind::Quantize
                                    | super::float::operations::DecimalBitKind::Reround
                            )
                        })
                };
                let (byte2, byte3) = if insert(&ins.op) {
                    (r(1) << 4 | (ins.immediate as u8 & 15), r(0) << 4 | r(2))
                } else if fused(&ins.op) {
                    (r(0) << 4, r(1) << 4 | r(2))
                } else if super::float::operations::decimal(&ins.op).is_some_and(|s| s.arithmetic) {
                    (r(2) << 4 | ins.immediate as u8, r(0) << 4 | r(1))
                } else if super::float::convert::instruction(&ins.op).is_some()
                    || super::float::operations::support(&ins.op).is_some_and(|s| {
                        s.kind == super::float::operations::SupportKind::HexToBinary
                    })
                    || super::float::operations::binary_controlled(&ins.op)
                    || super::float::operations::decimal_packed(&ins.op).is_some_and(|s| s.mask())
                    || super::float::operations::decimal(&ins.op).is_some_and(|s| {
                        matches!(
                            s.kind,
                            super::float::operations::Kind::Convert
                                | super::float::operations::Kind::RoundInteger
                        )
                    })
                {
                    (
                        (ins.immediate as u8 & 15) << 4 | (ins.immediate >> 4) as u8,
                        r(0) << 4 | r(1),
                    )
                } else {
                    (0, r(0) << 4 | r(1))
                };
                if insert(operation) {
                    ins.registers = vec![byte3 >> 4, byte2 >> 4, byte3 & 15];
                    ins.immediate = u32::from(byte2 & 15);
                } else if fused(operation) {
                    ins.registers = vec![byte2 >> 4, byte3 >> 4, byte3 & 15];
                    ins.immediate = 0;
                } else if super::float::operations::decimal(operation).is_some_and(|s| s.arithmetic)
                {
                    ins.registers = vec![byte3 >> 4, byte3 & 15, byte2 >> 4];
                    ins.immediate = u32::from(byte2 & 15);
                } else {
                    ins.registers = vec![byte3 >> 4, byte3 & 15];
                    ins.immediate = if super::float::convert::instruction(operation).is_some()
                        || super::float::operations::support(operation).is_some_and(|s| {
                            s.kind == super::float::operations::SupportKind::HexToBinary
                        })
                        || super::float::operations::binary_controlled(operation)
                        || super::float::operations::decimal_packed(operation)
                            .is_some_and(|s| s.mask())
                        || super::float::operations::decimal(operation).is_some_and(|s| {
                            matches!(
                                s.kind,
                                super::float::operations::Kind::Convert
                                    | super::float::operations::Kind::RoundInteger
                            )
                        }) {
                        u32::from(byte2 >> 4) | u32::from(byte2 & 15) << 4
                    } else {
                        0
                    };
                }
            }
            ins.op = operation.into();
        }
    }
    Ok(())
}

pub(super) fn initialize(code: &[Instruction], image: &mut [u8]) -> Result<(), String> {
    for ins in code {
        let at = ins.offset as usize + 1;
        *image
            .get_mut(at)
            .ok_or("instruction template outside module storage")? = byte(ins);
    }
    Ok(())
}

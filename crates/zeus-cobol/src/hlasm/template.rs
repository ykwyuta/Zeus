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
    RegisterOpcode(u8),
    Relative(u8),
    Rre(u8, u8),
    Opaque,
}

// Fixed opcode bytes in RRE occupy the second byte, while the registers
// remain in the fourth byte. EXECUTE must not reinterpret them as RR fields.
const RRE: &[(&str, u8, u8)] = &[
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
    if let Some((_, first, second)) = RRE.iter().find(|(name, _, _)| *name == op) {
        return Rre(*first, *second);
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
            && !matches!(op, "STIMER" | "GETMAIN" | "FREEMAIN" | "STORAGE")
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
        Opaque => 0,
        Rre(_, byte) => byte,
    }
}

pub(super) fn apply(ins: &mut Instruction, byte: u8) -> Result<(), String> {
    use Format::*;
    let (high, low) = (byte >> 4, byte & 15);
    match format(ins) {
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
        RegisterIndex => {
            ins.registers[0] = high;
            ins.address
                .as_mut()
                .ok_or("missing template address")?
                .index = low;
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
        Opaque if byte == 0 => {}
        Opaque => return Err("EX modification of this instruction format is unsupported".into()),
        Rre(first, _) => {
            let (op, _, _) = RRE
                .iter()
                .find(|(_, primary, secondary)| *primary == first && *secondary == byte)
                .ok_or("EX modification selects an unsupported RRE opcode")?;
            ins.op = (*op).into();
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

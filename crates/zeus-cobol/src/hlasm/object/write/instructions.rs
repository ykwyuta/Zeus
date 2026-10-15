//! Architectural scalar encodings, independent of Zeus EXECUTE templates.
use super::super::super::{Address, Instruction, operation};
mod floating;

fn nibble(value: u32) -> Result<u8, String> {
    u8::try_from(value)
        .ok()
        .filter(|n| *n < 16)
        .ok_or_else(|| "register/mask exceeds four bits".into())
}
fn address(value: Option<&Address>, indexed: bool) -> Result<[u8; 2], String> {
    let a = value.ok_or("missing storage operand")?;
    if !(0..=4095).contains(&a.displacement) || (!indexed && a.index != 0) {
        return Err("storage operand cannot be encoded as a 12-bit displacement".into());
    }
    nibble(u32::from(a.index))?;
    let b = nibble(u32::from(a.base))?;
    let d = a.displacement as u16;
    Ok([(b << 4) | (d >> 8) as u8, d as u8])
}
pub(in crate::hlasm::object) fn encode(i: &Instruction) -> Result<Vec<u8>, String> {
    if i.storage_service.is_some()
        || i.module_service.is_some()
        || i.system_service.is_some()
        || i.data_service.is_some()
    {
        return Err("runtime services require IBM macro expansion before OBJ output".into());
    }
    if i.relative_external.is_some() || i.relative_expression.is_some() {
        return Err("external relative instructions must be linked before OBJ output".into());
    }
    let r = |n: usize| {
        i.registers
            .get(n)
            .copied()
            .ok_or_else(|| "missing register operand".to_string())
            .and_then(|n| nibble(u32::from(n)))
    };
    let op = i.op.as_str();
    if let Some(bytes) = floating::encode(i)? {
        return Ok(bytes);
    }
    if let Some(spec) = operation::partial_immediate(op) {
        // Architectural RI-a/RIL-a opcode nibbles; EXECUTE templates use a
        // private representation and cannot supply these bytes.
        let (prefix, sub) = match op {
            "IIHH" => (0xa5, 0),
            "IIHL" => (0xa5, 1),
            "IILH" => (0xa5, 2),
            "IILL" => (0xa5, 3),
            "NIHH" => (0xa5, 4),
            "NIHL" => (0xa5, 5),
            "NILH" => (0xa5, 6),
            "NILL" => (0xa5, 7),
            "OIHH" => (0xa5, 8),
            "OIHL" => (0xa5, 9),
            "OILH" => (0xa5, 10),
            "OILL" => (0xa5, 11),
            "LLIHH" => (0xa5, 12),
            "LLIHL" => (0xa5, 13),
            "LLILH" => (0xa5, 14),
            "LLILL" => (0xa5, 15),
            "TMLH" => (0xa7, 0),
            "TMLL" => (0xa7, 1),
            "TMHH" => (0xa7, 2),
            "TMHL" => (0xa7, 3),
            "XIHF" => (0xc0, 6),
            "XILF" => (0xc0, 7),
            "IIHF" => (0xc0, 8),
            "IILF" => (0xc0, 9),
            "NIHF" => (0xc0, 10),
            "NILF" => (0xc0, 11),
            "OIHF" => (0xc0, 12),
            "OILF" => (0xc0, 13),
            "LLIHF" => (0xc0, 14),
            "LLILF" => (0xc0, 15),
            _ => unreachable!(),
        };
        let mut bytes = vec![prefix, r(0)? << 4 | sub];
        if spec.width == 16 {
            bytes.extend(
                u16::try_from(i.immediate)
                    .map_err(|_| "unsigned halfword immediate out of range")?
                    .to_be_bytes(),
            );
        } else {
            bytes.extend(i.immediate.to_be_bytes());
        }
        return Ok(bytes);
    }
    if let Some((root, flags)) = operation::rotate_selected(op) {
        let sub = match root {
            "RISBG" => 0x55,
            "RISBGN" => 0x59,
            "RISBHG" => 0x5d,
            "RISBLG" => 0x51,
            "RNSBG" => 0x54,
            "ROSBG" => 0x56,
            "RXSBG" => 0x57,
            _ => unreachable!(),
        };
        let n = i.immediate | flags;
        return Ok(vec![
            0xec,
            r(0)? << 4 | r(1)?,
            (n >> 16) as u8,
            (n >> 8) as u8,
            n as u8,
            sub,
        ]);
    }
    if let Some(spec) = operation::conditional(op)
        && matches!(spec.kind, operation::ConditionalKind::Immediate)
    {
        let sub = match spec.root {
            "LOCHI" => 0x42,
            "LOCGHI" => 0x46,
            "LOCHHI" => 0x4e,
            _ => unreachable!(),
        };
        let n = i16::try_from(i.immediate as i32)
            .map_err(|_| "signed conditional halfword immediate out of range")?
            .to_be_bytes();
        return Ok(vec![
            0xec,
            r(0)? << 4 | nibble(i.length)?,
            n[0],
            n[1],
            0,
            sub,
        ]);
    }
    if let Some(spec) = operation::relative(op) {
        use operation::RelativeKind::*;
        let (opcode, sub, first) = match spec.kind {
            Condition(mask) => (
                if spec.bits == 16 { 0xa7 } else { 0xc0 },
                4,
                mask.map_or_else(|| nibble(i.immediate), Ok)?,
            ),
            Save => (if spec.bits == 16 { 0xa7 } else { 0xc0 }, 5, r(0)?),
            Count32 => (0xa7, 6, r(0)?),
            Count64 => (0xa7, 7, r(0)?),
            CountHigh => (0xcc, 6, r(0)?),
            LoadAddress => (0xc0, 0, r(0)?),
            Execute => (0xc6, 0, r(0)?),
        };
        let a = i.address.as_ref().ok_or("missing relative operand")?;
        if a.base != 0 || a.index != 0 {
            return Err("relative operand cannot use base/index registers".into());
        }
        operation::check_relative(a.displacement, spec.bits)?;
        let mut b = vec![opcode, first << 4 | sub];
        let n = (a.displacement / 2).to_be_bytes();
        b.extend_from_slice(&n[8 - spec.bits as usize / 8..]);
        return Ok(b);
    }
    if matches!(op, "SAM24" | "SAM31" | "SAM64" | "PFPO") {
        return Ok(vec![
            1,
            match op {
                "SAM24" => 12,
                "SAM31" => 13,
                "SAM64" => 14,
                _ => 10,
            },
        ]);
    }
    if matches!(op, "BR" | "BCR" | "SPM") {
        return Ok(vec![
            if op == "SPM" { 4 } else { 7 },
            if op == "BR" {
                0xf0 | r(0)?
            } else if op == "SPM" {
                r(0)? << 4
            } else {
                nibble(i.immediate)? << 4 | r(0)?
            },
        ]);
    }
    let rr = [
        ("BALR", 5),
        ("BCTR", 6),
        ("BSM", 11),
        ("BASSM", 12),
        ("BASR", 13),
        ("MVCL", 14),
        ("CLCL", 15),
        ("LPR", 16),
        ("LNR", 17),
        ("LTR", 18),
        ("LCR", 19),
        ("NR", 20),
        ("CLR", 21),
        ("OR", 22),
        ("XR", 23),
        ("LR", 24),
        ("CR", 25),
        ("AR", 26),
        ("SR", 27),
        ("MR", 28),
        ("DR", 29),
        ("ALR", 30),
        ("SLR", 31),
        ("LPDR", 0x20),
        ("LNDR", 0x21),
        ("LTDR", 0x22),
        ("LCDR", 0x23),
        ("HDR", 0x24),
        ("LRDR", 0x25),
        ("MXR", 0x26),
        ("MXDR", 0x27),
        ("LDR", 0x28),
        ("CDR", 0x29),
        ("ADR", 0x2a),
        ("SDR", 0x2b),
        ("MDR", 0x2c),
        ("DDR", 0x2d),
        ("AWR", 0x2e),
        ("SWR", 0x2f),
        ("LPER", 0x30),
        ("LNER", 0x31),
        ("LTER", 0x32),
        ("LCER", 0x33),
        ("HER", 0x34),
        ("LEDR", 0x35),
        ("AXR", 0x36),
        ("SXR", 0x37),
        ("LER", 0x38),
        ("CER", 0x39),
        ("AER", 0x3a),
        ("SER", 0x3b),
        ("MER", 0x3c),
        ("DER", 0x3d),
        ("AUR", 0x3e),
        ("SUR", 0x3f),
    ];
    if let Some((_, opcode)) = rr.iter().find(|(name, _)| *name == op) {
        return Ok(vec![*opcode, r(0)? << 4 | r(1)?]);
    }
    if operation::register_logic(op).is_some() || operation::select(op).is_some() {
        let sub = match op {
            "NRK" => 0xf4,
            "NGRK" => 0xe4,
            "ORK" => 0xf6,
            "OGRK" => 0xe6,
            "XRK" => 0xf7,
            "XGRK" => 0xe7,
            "NNRK" => 0x74,
            "NNGRK" => 0x64,
            "NORK" => 0x76,
            "NOGRK" => 0x66,
            "NXRK" => 0x77,
            "NXGRK" => 0x67,
            "NCRK" => 0xf5,
            "NCGRK" => 0xe5,
            "OCRK" => 0x75,
            "OCGRK" => 0x65,
            "SELR" => 0xf0,
            "SELGR" => 0xe3,
            "SELFHR" => 0xc0,
            _ => return Err("noncanonical register logic/select operation".into()),
        };
        let mask = if operation::select(op).is_some() {
            nibble(i.immediate)?
        } else {
            0
        };
        return Ok(vec![0xb9, sub, r(2)? << 4 | mask, r(0)? << 4 | r(1)?]);
    }
    if matches!(op, "LOCR" | "LOCGR" | "LOCFHR" | "POPCNT") {
        let sub = match op {
            "LOCR" => 0xf2,
            "LOCGR" => 0xe2,
            "LOCFHR" => 0xe0,
            _ => 0xe1,
        };
        return Ok(vec![
            0xb9,
            sub,
            nibble(i.immediate)? << 4,
            r(0)? << 4 | r(1)?,
        ]);
    }
    let rre = [
        ("LGR", 0x04),
        ("LGFR", 0x14),
        ("LLGFR", 0x16),
        ("LTGR", 0x02),
        ("LCGR", 0x03),
        ("LPGR", 0x00),
        ("LNGR", 0x01),
        ("AGR", 0x08),
        ("SGR", 0x09),
        ("ALGR", 0x0a),
        ("SLGR", 0x0b),
        ("CGR", 0x20),
        ("CLGR", 0x21),
        ("NGR", 0x80),
        ("OGR", 0x81),
        ("XGR", 0x82),
        ("AGFR", 0x18),
        ("SGFR", 0x19),
        ("CGFR", 0x30),
        ("MSGR", 0x0c),
        ("MLGR", 0x86),
        ("DLGR", 0x87),
        ("MLR", 0x96),
        ("DLR", 0x97),
        ("DSGR", 0x0d),
        ("DSGFR", 0x1d),
        ("BCTGR", 0x46),
        ("FLOGR", 0x83),
        ("ALCR", 0x98),
        ("ALCGR", 0x88),
        ("SLBR", 0x99),
        ("SLBGR", 0x89),
        ("LRVR", 0x1f),
        ("LRVGR", 0x0f),
        ("LBR", 0x26),
        ("LGBR", 0x06),
    ];
    if let Some((_, opcode)) = rre.iter().find(|(name, _)| *name == op) {
        return Ok(vec![0xb9, *opcode, 0, r(0)? << 4 | r(1)?]);
    }
    let ri = [
        ("LHI", 8),
        ("LGHI", 9),
        ("AHI", 10),
        ("AGHI", 11),
        ("MHI", 12),
        ("MGHI", 13),
        ("CHI", 14),
        ("CGHI", 15),
    ];
    if let Some((_, sub)) = ri.iter().find(|(name, _)| *name == op) {
        let n = i16::try_from(i.immediate as i32)
            .map_err(|_| "signed halfword immediate out of range")?;
        let mut b = vec![0xa7, r(0)? << 4 | sub];
        b.extend(n.to_be_bytes());
        return Ok(b);
    }
    if matches!(op, "LGFI" | "AGFI" | "CGFI") {
        let mut b = vec![
            if op == "LGFI" { 0xc0 } else { 0xc2 },
            r(0)? << 4
                | match op {
                    "LGFI" => 1,
                    "AGFI" => 8,
                    _ => 12,
                },
        ];
        b.extend(i.immediate.to_be_bytes());
        return Ok(b);
    }
    let rx = [
        ("STH", 0x40),
        ("LA", 0x41),
        ("STC", 0x42),
        ("IC", 0x43),
        ("EX", 0x44),
        ("BAL", 0x45),
        ("BCT", 0x46),
        ("BC", 0x47),
        ("LH", 0x48),
        ("CH", 0x49),
        ("AH", 0x4a),
        ("SH", 0x4b),
        ("MH", 0x4c),
        ("BAS", 0x4d),
        ("CVD", 0x4e),
        ("CVB", 0x4f),
        ("ST", 0x50),
        ("N", 0x54),
        ("CL", 0x55),
        ("O", 0x56),
        ("X", 0x57),
        ("L", 0x58),
        ("C", 0x59),
        ("A", 0x5a),
        ("S", 0x5b),
        ("M", 0x5c),
        ("D", 0x5d),
        ("AL", 0x5e),
        ("SL", 0x5f),
        ("STD", 0x60),
        ("MXD", 0x67),
        ("LD", 0x68),
        ("CD", 0x69),
        ("AD", 0x6a),
        ("SD", 0x6b),
        ("MD", 0x6c),
        ("DD", 0x6d),
        ("AW", 0x6e),
        ("SW", 0x6f),
        ("STE", 0x70),
        ("LE", 0x78),
        ("CE", 0x79),
        ("AE", 0x7a),
        ("SE", 0x7b),
        ("ME", 0x7c),
        ("DE", 0x7d),
        ("AU", 0x7e),
        ("SU", 0x7f),
    ];
    if let Some((_, opcode)) = rx.iter().find(|(name, _)| *name == op) {
        let a = i.address.as_ref().ok_or("missing storage operand")?;
        let mut b = vec![
            *opcode,
            (if op == "BC" {
                nibble(i.immediate)?
            } else {
                r(0)?
            }) << 4
                | a.index,
        ];
        b.extend(address(Some(a), true)?);
        return Ok(b);
    }
    let rxy = [
        ("CVBY", 0x06),
        ("CVDY", 0x26),
        ("LRVG", 0x0f),
        ("DSGF", 0x1d),
        ("LRV", 0x1e),
        ("LRVH", 0x1f),
        ("STRVG", 0x2f),
        ("STRV", 0x3e),
        ("STRVH", 0x3f),
        ("NY", 0x54),
        ("CLY", 0x55),
        ("OY", 0x56),
        ("XY", 0x57),
        ("ALY", 0x5e),
        ("SLY", 0x5f),
        ("STHY", 0x70),
        ("STCY", 0x72),
        ("ICY", 0x73),
        ("LB", 0x76),
        ("LGB", 0x77),
        ("LHY", 0x78),
        ("CHY", 0x79),
        ("AHY", 0x7a),
        ("SHY", 0x7b),
        ("MLG", 0x86),
        ("DLG", 0x87),
        ("ALCG", 0x88),
        ("SLBG", 0x89),
        ("LLGC", 0x90),
        ("LLGH", 0x91),
        ("LLC", 0x94),
        ("LLH", 0x95),
        ("ML", 0x96),
        ("DL", 0x97),
        ("ALC", 0x98),
        ("SLB", 0x99),
        ("LG", 0x04),
        ("AG", 0x08),
        ("SG", 0x09),
        ("ALG", 0x0a),
        ("SLG", 0x0b),
        ("MSG", 0x0c),
        ("DSG", 0x0d),
        ("LGF", 0x14),
        ("LGH", 0x15),
        ("LLGF", 0x16),
        ("AGF", 0x18),
        ("SGF", 0x19),
        ("CG", 0x20),
        ("CLG", 0x21),
        ("STG", 0x24),
        ("CGF", 0x30),
        ("BCTG", 0x46),
        ("STY", 0x50),
        ("LY", 0x58),
        ("CY", 0x59),
        ("AY", 0x5a),
        ("SY", 0x5b),
        ("LAY", 0x71),
        ("NG", 0x80),
        ("OG", 0x81),
        ("XG", 0x82),
    ];
    if let Some((_, opcode)) = rxy.iter().find(|(name, _)| *name == op) {
        let a = i.address.as_ref().ok_or("missing storage operand")?;
        if !(-(1 << 19)..(1 << 19)).contains(&a.displacement) {
            return Err("signed 20-bit displacement out of range".into());
        }
        let d = a.displacement as u32;
        return Ok(vec![
            0xe3,
            r(0)? << 4 | nibble(u32::from(a.index))?,
            nibble(u32::from(a.base))? << 4 | ((d >> 8) & 15) as u8,
            d as u8,
            (d >> 12) as u8,
            *opcode,
        ]);
    }
    let rsy = [
        ("LMG", 0x04),
        ("SRAG", 0x0a),
        ("SLAG", 0x0b),
        ("SRLG", 0x0c),
        ("SLLG", 0x0d),
        ("CSY", 0x14),
        ("RLLG", 0x1c),
        ("RLL", 0x1d),
        ("CLMH", 0x20),
        ("CLMY", 0x21),
        ("STMG", 0x24),
        ("STCMH", 0x2c),
        ("STCMY", 0x2d),
        ("CSG", 0x30),
        ("CDSY", 0x31),
        ("CDSG", 0x3e),
        ("TMY", 0x51),
        ("MVIY", 0x52),
        ("NIY", 0x54),
        ("CLIY", 0x55),
        ("OIY", 0x56),
        ("XIY", 0x57),
        ("ICMH", 0x80),
        ("ICMY", 0x81),
        ("STMY", 0x90),
        ("LMY", 0x98),
        ("LOCFH", 0xe0),
        ("STOCFH", 0xe1),
        ("LOCG", 0xe2),
        ("STOCG", 0xe3),
        ("LANG", 0xe4),
        ("LAOG", 0xe6),
        ("LAXG", 0xe7),
        ("LAAG", 0xe8),
        ("LAALG", 0xea),
        ("LOC", 0xf2),
        ("STOC", 0xf3),
        ("LAN", 0xf4),
        ("LAO", 0xf6),
        ("LAX", 0xf7),
        ("LAA", 0xf8),
        ("LAAL", 0xfa),
    ];
    if let Some((_, opcode)) = rsy.iter().find(|(name, _)| *name == op) {
        let a = i.address.as_ref().ok_or("missing storage operand")?;
        if a.index != 0 || !(-(1 << 19)..(1 << 19)).contains(&a.displacement) {
            return Err("RSY/SIY requires no index and a signed 20-bit displacement".into());
        }
        let first = if (0x51..=0x57).contains(opcode) {
            u8::try_from(i.immediate).map_err(|_| "byte immediate out of range")?
        } else {
            r(0)? << 4
                | if operation::masked_bytes(op).is_some() || operation::conditional(op).is_some() {
                    nibble(i.immediate)?
                } else {
                    r(1)?
                }
        };
        let d = a.displacement as u32;
        return Ok(vec![
            0xeb,
            first,
            nibble(u32::from(a.base))? << 4 | ((d >> 8) & 15) as u8,
            d as u8,
            (d >> 12) as u8,
            *opcode,
        ]);
    }
    let rs = [
        ("SRL", 0x88),
        ("SLL", 0x89),
        ("SRA", 0x8a),
        ("SLA", 0x8b),
        ("SRDL", 0x8c),
        ("SLDL", 0x8d),
        ("SRDA", 0x8e),
        ("SLDA", 0x8f),
        ("STM", 0x90),
        ("LM", 0x98),
    ];
    if let Some((_, opcode)) = rs.iter().find(|(name, _)| *name == op) {
        let second = if matches!(op, "STM" | "LM") { r(1)? } else { 0 };
        let mut b = vec![*opcode, r(0)? << 4 | second];
        b.extend(address(i.address.as_ref(), false)?);
        return Ok(b);
    }
    let si = [
        ("TM", 0x91),
        ("MVI", 0x92),
        ("NI", 0x94),
        ("CLI", 0x95),
        ("OI", 0x96),
        ("XI", 0x97),
    ];
    if let Some((_, opcode)) = si.iter().find(|(name, _)| *name == op) {
        let mut b = vec![
            *opcode,
            u8::try_from(i.immediate).map_err(|_| "byte immediate out of range")?,
        ];
        b.extend(address(i.address.as_ref(), false)?);
        return Ok(b);
    }
    let ss = [
        ("MVN", 0xd1),
        ("MVC", 0xd2),
        ("MVZ", 0xd3),
        ("NC", 0xd4),
        ("CLC", 0xd5),
        ("OC", 0xd6),
        ("XC", 0xd7),
        ("TR", 0xdc),
        ("TRT", 0xdd),
        ("ED", 0xde),
        ("EDMK", 0xdf),
        ("PACK", 0xf2),
        ("UNPK", 0xf3),
        ("ZAP", 0xf8),
        ("CP", 0xf9),
        ("AP", 0xfa),
        ("SP", 0xfb),
        ("MP", 0xfc),
        ("DP", 0xfd),
    ];
    if let Some((_, opcode)) = ss.iter().find(|(name, _)| *name == op) {
        let length = i.length.checked_sub(1).ok_or("zero operand length")?;
        let field = if *opcode >= 0xf0 {
            nibble(length)? << 4
                | nibble(i.source_length.checked_sub(1).ok_or("zero source length")?)?
        } else {
            u8::try_from(length).map_err(|_| "string operand exceeds 256 bytes")?
        };
        let mut b = vec![*opcode, field];
        b.extend(address(i.address.as_ref(), false)?);
        b.extend(address(i.source.as_ref(), false)?);
        return Ok(b);
    }
    Err("architectural encoding is not implemented for this instruction".into())
}

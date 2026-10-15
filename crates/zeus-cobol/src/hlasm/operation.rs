#[derive(Clone, Copy)]
pub(super) enum ImmediateKind {
    Insert,
    Load,
    And,
    Or,
    Xor,
    Test,
}

pub(super) struct PartialImmediate {
    pub width: u32,
    pub shift: u32,
    pub kind: ImmediateKind,
}

#[derive(Clone, Copy)]
pub(super) enum RelativeKind {
    Condition(Option<u8>),
    Save,
    Count32,
    Count64,
    CountHigh,
    LoadAddress,
    Execute,
}

pub(super) struct Relative {
    pub kind: RelativeKind,
    pub bits: u32,
}

impl Relative {
    pub fn size(&self) -> u32 {
        if self.bits == 16 { 4 } else { 6 }
    }
}

pub(super) fn relative(op: &str) -> Option<Relative> {
    let (kind, bits) = match op {
        "BRC" => (RelativeKind::Condition(None), 16),
        "BRCL" => (RelativeKind::Condition(None), 32),
        "BRAS" => (RelativeKind::Save, 16),
        "BRASL" => (RelativeKind::Save, 32),
        "BRCT" => (RelativeKind::Count32, 16),
        "BRCTG" => (RelativeKind::Count64, 16),
        "BRCTH" => (RelativeKind::CountHigh, 32),
        "LARL" => (RelativeKind::LoadAddress, 32),
        "EXRL" => (RelativeKind::Execute, 32),
        _ => {
            let (suffix, bits) = if let Some(suffix) = op.strip_prefix("JG") {
                (suffix, 32)
            } else {
                (op.strip_prefix('J')?, 16)
            };
            let mask = match suffix {
                "" => 15,
                "E" | "Z" => 8,
                "NE" | "NZ" => 7,
                "H" | "P" => 2,
                "L" | "M" => 4,
                "NH" | "NP" => 13,
                "NL" | "NM" => 11,
                "O" => 1,
                "NO" => 14,
                "NOP" => 0,
                _ => return None,
            };
            (RelativeKind::Condition(Some(mask)), bits)
        }
    };
    Some(Relative { kind, bits })
}

pub(super) fn check_relative(displacement: i64, bits: u32) -> Result<(), String> {
    if displacement % 2 != 0 {
        return Err("relative target must be halfword aligned".into());
    }
    let halfwords = displacement / 2;
    if (bits == 16 && i16::try_from(halfwords).is_err()) || i32::try_from(halfwords).is_err() {
        return Err("relative halfword displacement out of range".into());
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub(super) enum MaskedKind {
    Insert,
    Store,
    Compare,
}

pub(super) struct MaskedBytes {
    pub kind: MaskedKind,
    pub high: bool,
    pub long: bool,
}

pub(super) fn masked_bytes(op: &str) -> Option<MaskedBytes> {
    let kind = match op {
        "ICM" | "ICMY" | "ICMH" => MaskedKind::Insert,
        "STCM" | "STCMY" | "STCMH" => MaskedKind::Store,
        "CLM" | "CLMY" | "CLMH" => MaskedKind::Compare,
        _ => return None,
    };
    Some(MaskedBytes {
        kind,
        high: op.ends_with('H'),
        long: op.ends_with(['Y', 'H']),
    })
}

/// The suffix identifies a field of a 64-bit GPR, independent of AMODE.
pub(super) fn partial_immediate(op: &str) -> Option<PartialImmediate> {
    let kind = match op {
        "IIHH" | "IIHL" | "IILH" | "IILL" | "IIHF" | "IILF" => ImmediateKind::Insert,
        "LLIHH" | "LLIHL" | "LLILH" | "LLILL" | "LLIHF" | "LLILF" => ImmediateKind::Load,
        "NIHH" | "NIHL" | "NILH" | "NILL" | "NIHF" | "NILF" => ImmediateKind::And,
        "OIHH" | "OIHL" | "OILH" | "OILL" | "OIHF" | "OILF" => ImmediateKind::Or,
        "XIHF" | "XILF" => ImmediateKind::Xor,
        "TMHH" | "TMHL" | "TMLH" | "TMLL" => ImmediateKind::Test,
        _ => return None,
    };
    let suffix = &op[op.len() - 2..];
    let (width, shift) = match suffix {
        "HH" => (16, 48),
        "HL" => (16, 32),
        "LH" => (16, 16),
        "LL" => (16, 0),
        "HF" => (32, 32),
        "LF" => (32, 0),
        _ => unreachable!(),
    };
    Some(PartialImmediate { width, shift, kind })
}
/// Interlocked operations run to completion while the engine exclusively owns
/// the run-unit storage. No host thread can observe an intermediate update.
pub(super) fn interlocked(op: &str) -> Option<(u32, usize)> {
    Some(match op {
        "TS" => (4, 1),
        "CS" => (4, 4),
        "CDS" => (4, 8),
        "CSY" => (6, 4),
        "CDSY" | "CSG" => (6, 8),
        "CDSG" => (6, 16),
        "LAA" | "LAAL" | "LAN" | "LAO" | "LAX" => (6, 4),
        "LAAG" | "LAALG" | "LANG" | "LAOG" | "LAXG" => (6, 8),
        _ => return None,
    })
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum ConditionalKind {
    MemoryLoad,
    MemoryStore,
    Register,
    Immediate,
}

pub(super) struct Conditional {
    pub root: &'static str,
    pub kind: ConditionalKind,
    pub wide: bool,
    pub high: bool,
    pub mask: Option<u8>,
}

pub(super) fn conditional(op: &str) -> Option<Conditional> {
    for root in [
        "LOCFHR", "LOCGHI", "LOCHHI", "STOCFH", "LOCGR", "LOCFH", "LOCHI", "STOCG", "LOCR", "LOCG",
        "STOC", "LOC",
    ] {
        if let Some(suffix) = op.strip_prefix(root) {
            let mask = if suffix.is_empty() {
                None
            } else {
                Some(match suffix {
                    "O" => 1,
                    "NO" => 14,
                    "H" | "P" => 2,
                    "NH" | "NP" => 13,
                    "L" | "M" => 4,
                    "NL" | "NM" => 11,
                    "E" | "Z" => 8,
                    "NE" | "NZ" => 7,
                    _ => continue,
                })
            };
            return Some(Conditional {
                root,
                kind: if root.starts_with("STOC") {
                    ConditionalKind::MemoryStore
                } else if root.ends_with('R') {
                    ConditionalKind::Register
                } else if root.ends_with("HI") {
                    ConditionalKind::Immediate
                } else {
                    ConditionalKind::MemoryLoad
                },
                wide: matches!(root, "LOCG" | "LOCGR" | "LOCGHI" | "STOCG"),
                high: matches!(root, "LOCFH" | "LOCFHR" | "LOCHHI" | "STOCFH"),
                mask,
            });
        }
    }
    None
}

/// RIE-f rotate/select operations; suffix controls are expanded at assembly.
pub(super) fn rotate_selected(op: &str) -> Option<(&'static str, u32)> {
    Some(match op {
        "RISBG" => ("RISBG", 0),
        "RISBGZ" => ("RISBG", 0x8000),
        "RISBGN" => ("RISBGN", 0),
        "RISBGNZ" => ("RISBGN", 0x8000),
        "RISBHG" => ("RISBHG", 0),
        "RISBHGZ" => ("RISBHG", 0x8000),
        "RISBLG" => ("RISBLG", 0),
        "RISBLGZ" => ("RISBLG", 0x8000),
        "RNSBG" => ("RNSBG", 0),
        "RNSBGT" => ("RNSBG", 0x800000),
        "ROSBG" => ("ROSBG", 0),
        "ROSBGT" => ("ROSBG", 0x800000),
        "RXSBG" => ("RXSBG", 0),
        "RXSBGT" => ("RXSBG", 0x800000),
        _ => return None,
    })
}

#[derive(Clone, Copy)]
pub(super) enum Logic {
    And,
    Or,
    Xor,
    Nand,
    Nor,
    Nxor,
    AndComplement,
    OrComplement,
}
pub(super) fn register_logic(op: &str) -> Option<(Logic, bool)> {
    use Logic::*;
    Some(match op {
        "NRK" => (And, false),
        "NGRK" => (And, true),
        "ORK" => (Or, false),
        "OGRK" => (Or, true),
        "XRK" => (Xor, false),
        "XGRK" => (Xor, true),
        "NNRK" => (Nand, false),
        "NNGRK" => (Nand, true),
        "NORK" | "NOTR" => (Nor, false),
        "NOGRK" | "NOTGR" => (Nor, true),
        "NXRK" => (Nxor, false),
        "NXGRK" => (Nxor, true),
        "NCRK" => (AndComplement, false),
        "NCGRK" => (AndComplement, true),
        "OCRK" => (OrComplement, false),
        "OCGRK" => (OrComplement, true),
        _ => return None,
    })
}
pub(super) fn select(op: &str) -> Option<(&'static str, Option<u8>)> {
    for root in ["SELFHR", "SELGR", "SELR"] {
        if let Some(suffix) = op.strip_prefix(root) {
            let mask = if suffix.is_empty() {
                None
            } else {
                Some(match suffix {
                    "O" => 1,
                    "NO" => 14,
                    "H" | "P" => 2,
                    "NH" | "NP" => 13,
                    "L" | "M" => 4,
                    "NL" | "NM" => 11,
                    "E" | "Z" => 8,
                    "NE" | "NZ" => 7,
                    _ => continue,
                })
            };
            return Some((root, mask));
        }
    }
    None
}

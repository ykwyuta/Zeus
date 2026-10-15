//! Architectural instruction decoding for imported IBM objects. Fetch only
//! at executed addresses: text can interleave instructions, literals and data.
//! Opcode/layout definitions follow z/Architecture Principles of Operation.
mod floating;
mod long_storage;
mod registers;
use super::{Address, Instruction, Module};

pub(super) fn contains(module: &Module, offset: u32) -> bool {
    module.machine_code.iter().any(|r| r.contains(&offset))
}

pub(super) fn validate(module: &Module) -> Result<(), String> {
    if !module.machine_code.is_empty() && module.zeus_hlasm < 11 {
        return Err("IBM machine-code ranges require HLASM module version 11".into());
    }
    let mut end = 0;
    for r in &module.machine_code {
        if r.start >= r.end || r.start < end || r.end as usize > module.storage.len() {
            return Err("invalid or overlapping IBM machine-code ranges".into());
        }
        if module.code.iter().any(|i| r.contains(&i.offset)) {
            return Err("IBM machine code overlaps a Zeus source instruction".into());
        }
        end = r.end;
    }
    Ok(())
}

fn address(bytes: &[u8], index: u8) -> Address {
    Address {
        base: bytes[0] >> 4,
        index,
        displacement: i64::from(u16::from_be_bytes([bytes[0] & 15, bytes[1]])),
    }
}

pub(super) fn fetch(
    module: &Module,
    memory: &[u8],
    base: usize,
    at: usize,
    overlay: u8,
) -> Result<Instruction, String> {
    let offset = at
        .checked_sub(base)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or("IBM instruction address outside module")?;
    let region = module
        .machine_code
        .iter()
        .find(|r| r.contains(&offset))
        .ok_or("IBM instruction address outside machine-code ranges")?;
    if offset % 2 != 0 {
        return Err("IBM instruction address must be halfword aligned".into());
    }
    let prefix = *memory.get(at).ok_or("truncated IBM instruction")?;
    let size = match prefix >> 6 {
        0 => 2,
        1 | 2 => 4,
        _ => 6,
    };
    if offset.checked_add(size).is_none_or(|end| end > region.end) {
        return Err("IBM instruction crosses its section boundary".into());
    }
    let mut bytes = memory
        .get(at..at + size as usize)
        .ok_or("truncated IBM instruction storage")?
        .to_vec();
    bytes[1] |= overlay;
    decode(&bytes, offset)
}

fn decode(b: &[u8], offset: u32) -> Result<Instruction, String> {
    let mut i = Instruction {
        offset,
        op: String::new(),
        storage_service: None,
        module_service: None,
        system_service: None,
        data_service: None,
        registers: Vec::new(),
        address: None,
        source: None,
        length: 1,
        source_length: 1,
        immediate: 0,
        relative_external: None,
        relative_expression: None,
        file: "<IBM object>".into(),
        line: offset as usize,
    };
    let r1 = b[1] >> 4;
    let r2 = b[1] & 15;
    let op = match b[0] {
        0x01 => match b[1] {
            0x0a => "PFPO",
            0x0c => "SAM24",
            0x0d => "SAM31",
            0x0e => "SAM64",
            _ => "",
        },
        0x04..=0x3f => {
            let op = match b[0] {
                0x04 => "SPM",
                0x05 => "BALR",
                0x06 => "BCTR",
                0x07 => "BCR",
                0x0b => "BSM",
                0x0c => "BASSM",
                0x0d => "BASR",
                0x0e => "MVCL",
                0x0f => "CLCL",
                0x10 => "LPR",
                0x11 => "LNR",
                0x12 => "LTR",
                0x13 => "LCR",
                0x14 => "NR",
                0x15 => "CLR",
                0x16 => "OR",
                0x17 => "XR",
                0x18 => "LR",
                0x19 => "CR",
                0x1a => "AR",
                0x1b => "SR",
                0x1c => "MR",
                0x1d => "DR",
                0x1e => "ALR",
                0x1f => "SLR",
                0x20 => "LPDR",
                0x21 => "LNDR",
                0x22 => "LTDR",
                0x23 => "LCDR",
                0x24 => "HDR",
                0x25 => "LRDR",
                0x26 => "MXR",
                0x27 => "MXDR",
                0x28 => "LDR",
                0x29 => "CDR",
                0x2a => "ADR",
                0x2b => "SDR",
                0x2c => "MDR",
                0x2d => "DDR",
                0x2e => "AWR",
                0x2f => "SWR",
                0x30 => "LPER",
                0x31 => "LNER",
                0x32 => "LTER",
                0x33 => "LCER",
                0x34 => "HER",
                0x35 => "LEDR",
                0x36 => "AXR",
                0x37 => "SXR",
                0x38 => "LER",
                0x39 => "CER",
                0x3a => "AER",
                0x3b => "SER",
                0x3c => "MER",
                0x3d => "DER",
                0x3e => "AUR",
                0x3f => "SUR",
                _ => "",
            };
            i.registers = vec![r1, r2];
            if b[0] == 0x07 {
                i.registers = vec![r2];
                i.immediate = u32::from(r1);
            }
            if b[0] == 0x04 {
                i.registers = vec![r1];
            }
            op
        }
        0x40..=0x7f => {
            let op = match b[0] {
                0x40 => "STH",
                0x41 => "LA",
                0x42 => "STC",
                0x43 => "IC",
                0x44 => "EX",
                0x45 => "BAL",
                0x46 => "BCT",
                0x47 => "BC",
                0x48 => "LH",
                0x49 => "CH",
                0x4a => "AH",
                0x4b => "SH",
                0x4c => "MH",
                0x4d => "BAS",
                0x4e => "CVD",
                0x4f => "CVB",
                0x50 => "ST",
                0x54 => "N",
                0x55 => "CL",
                0x56 => "O",
                0x57 => "X",
                0x58 => "L",
                0x59 => "C",
                0x5a => "A",
                0x5b => "S",
                0x5c => "M",
                0x5d => "D",
                0x5e => "AL",
                0x5f => "SL",
                0x60 => "STD",
                0x67 => "MXD",
                0x68 => "LD",
                0x69 => "CD",
                0x6a => "AD",
                0x6b => "SD",
                0x6c => "MD",
                0x6d => "DD",
                0x6e => "AW",
                0x6f => "SW",
                0x70 => "STE",
                0x78 => "LE",
                0x79 => "CE",
                0x7a => "AE",
                0x7b => "SE",
                0x7c => "ME",
                0x7d => "DE",
                0x7e => "AU",
                0x7f => "SU",
                _ => "",
            };
            i.registers = vec![r1];
            i.address = Some(address(&b[2..4], r2));
            if b[0] == 0x47 {
                i.registers.clear();
                i.immediate = u32::from(r1);
            }
            op
        }
        0x88..=0x8f | 0x90 | 0x98 | 0x9a | 0x9b | 0xba | 0xbb | 0xbe | 0xbf => {
            let op = match b[0] {
                0x88 => "SRL",
                0x89 => "SLL",
                0x8a => "SRA",
                0x8b => "SLA",
                0x8c => "SRDL",
                0x8d => "SLDL",
                0x8e => "SRDA",
                0x8f => "SLDA",
                0x90 => "STM",
                0x98 => "LM",
                0x9a => "LAM",
                0x9b => "STAM",
                0xba => "CS",
                0xbb => "CDS",
                0xbe => "STCM",
                0xbf => "ICM",
                _ => "",
            };
            i.registers = vec![r1, r2];
            i.address = Some(address(&b[2..4], 0));
            if matches!(b[0], 0xbe | 0xbf) {
                i.registers = vec![r1];
                i.immediate = u32::from(r2);
            }
            op
        }
        0x91..=0x97 => {
            i.immediate = u32::from(b[1]);
            i.address = Some(address(&b[2..4], 0));
            match b[0] {
                0x91 => "TM",
                0x92 => "MVI",
                0x94 => "NI",
                0x95 => "CLI",
                0x96 => "OI",
                0x97 => "XI",
                _ => "",
            }
        }
        0xa5 => {
            i.registers = vec![r1];
            i.immediate = u32::from(u16::from_be_bytes([b[2], b[3]]));
            [
                "IIHH", "IIHL", "IILH", "IILL", "NIHH", "NIHL", "NILH", "NILL", "OIHH", "OIHL",
                "OILH", "OILL", "LLIHH", "LLIHL", "LLILH", "LLILL",
            ][r2 as usize]
        }
        0xa7 => {
            let op = match r2 {
                0 => "TMLH",
                1 => "TMLL",
                2 => "TMHH",
                3 => "TMHL",
                4 => "BRC",
                5 => "BRAS",
                6 => "BRCT",
                7 => "BRCTG",
                8 => "LHI",
                9 => "LGHI",
                10 => "AHI",
                11 => "AGHI",
                12 => "MHI",
                13 => "MGHI",
                14 => "CHI",
                15 => "CGHI",
                _ => "",
            };
            i.registers = vec![r1];
            let n = i16::from_be_bytes([b[2], b[3]]);
            if (4..=7).contains(&r2) {
                i.address = Some(Address {
                    base: 0,
                    index: 0,
                    displacement: i64::from(n) * 2,
                });
                if r2 == 4 {
                    i.registers.clear();
                    i.immediate = u32::from(r1);
                }
            } else {
                i.immediate = if r2 < 4 {
                    u32::from(n as u16)
                } else {
                    n as i32 as u32
                };
            }
            op
        }
        0xb2 if matches!(b[1], 0x99 | 0x9c | 0x9d | 0xb8 | 0xb9) => {
            i.address = Some(address(&b[2..4], 0));
            match b[1] {
                0x99 => "SRNM",
                0x9c => "STFPC",
                0x9d => "LFPC",
                0xb8 => "SRNMB",
                _ => "SRNMT",
            }
        }
        0xb2 | 0xb3 | 0xb9 => registers::decode(&mut i, b).unwrap_or(""),
        0xec => match b[5] {
            0x42 | 0x46 | 0x4e => {
                i.registers = vec![r1];
                i.length = u32::from(r2);
                i.immediate = i16::from_be_bytes([b[2], b[3]]) as i32 as u32;
                match b[5] {
                    0x42 => "LOCHI",
                    0x46 => "LOCGHI",
                    _ => "LOCHHI",
                }
            }
            0x51 | 0x54..=0x57 | 0x59 | 0x5d => {
                i.registers = vec![r1, r2];
                i.immediate = u32::from_be_bytes([0, b[2], b[3], b[4]]);
                match b[5] {
                    0x51 => "RISBLG",
                    0x54 => "RNSBG",
                    0x55 => "RISBG",
                    0x56 => "ROSBG",
                    0x57 => "RXSBG",
                    0x59 => "RISBGN",
                    _ => "RISBHG",
                }
            }
            _ => "",
        },
        0xed => floating::decode(&mut i, b).unwrap_or(""),
        0xeb => long_storage::decode(&mut i, b).unwrap_or(""),
        0xe3 => {
            let op = match b[5] {
                0x04 => "LG",
                0x06 => "CVBY",
                0x08 => "AG",
                0x09 => "SG",
                0x0a => "ALG",
                0x0b => "SLG",
                0x0c => "MSG",
                0x0d => "DSG",
                0x0e => "CVBG",
                0x0f => "LRVG",
                0x14 => "LGF",
                0x15 => "LGH",
                0x16 => "LLGF",
                0x18 => "AGF",
                0x19 => "SGF",
                0x1d => "DSGF",
                0x1e => "LRV",
                0x1f => "LRVH",
                0x20 => "CG",
                0x21 => "CLG",
                0x24 => "STG",
                0x26 => "CVDY",
                0x2e => "CVDG",
                0x2f => "STRVG",
                0x30 => "CGF",
                0x3e => "STRV",
                0x3f => "STRVH",
                0x46 => "BCTG",
                0x50 => "STY",
                0x51 => "MSY",
                0x54 => "NY",
                0x55 => "CLY",
                0x56 => "OY",
                0x57 => "XY",
                0x58 => "LY",
                0x59 => "CY",
                0x5a => "AY",
                0x5b => "SY",
                0x5e => "ALY",
                0x5f => "SLY",
                0x70 => "STHY",
                0x71 => "LAY",
                0x72 => "STCY",
                0x73 => "ICY",
                0x76 => "LB",
                0x77 => "LGB",
                0x78 => "LHY",
                0x79 => "CHY",
                0x7a => "AHY",
                0x7b => "SHY",
                0x80 => "NG",
                0x81 => "OG",
                0x82 => "XG",
                0x86 => "MLG",
                0x87 => "DLG",
                0x88 => "ALCG",
                0x89 => "SLBG",
                0x8e => "STPQ",
                0x8f => "LPQ",
                0x90 => "LLGC",
                0x91 => "LLGH",
                0x94 => "LLC",
                0x95 => "LLH",
                0x96 => "ML",
                0x97 => "DL",
                0x98 => "ALC",
                0x99 => "SLB",
                _ => "",
            };
            let displacement =
                ((i32::from(b[4]) << 12) | (i32::from(b[2] & 15) << 8) | i32::from(b[3])) << 12
                    >> 12;
            i.registers = vec![r1];
            i.address = Some(Address {
                base: b[2] >> 4,
                index: r2,
                displacement: i64::from(displacement),
            });
            op
        }
        0xc2 | 0xc6 | 0xcc => {
            let op = match (b[0], r2) {
                (0xc2, 8) => "AGFI",
                (0xc2, 12) => "CGFI",
                (0xc6, 0) => "EXRL",
                (0xcc, 6) => "BRCTH",
                _ => "",
            };
            i.registers = vec![r1];
            let n = i32::from_be_bytes(b[2..6].try_into().unwrap());
            if b[0] == 0xc2 {
                i.immediate = n as u32;
            } else {
                i.address = Some(Address {
                    base: 0,
                    index: 0,
                    displacement: i64::from(n) * 2,
                });
            }
            op
        }
        0xc0 => {
            let op = match r2 {
                0 => "LARL",
                1 => "LGFI",
                4 => "BRCL",
                5 => "BRASL",
                6 => "XIHF",
                7 => "XILF",
                8 => "IIHF",
                9 => "IILF",
                10 => "NIHF",
                11 => "NILF",
                12 => "OIHF",
                13 => "OILF",
                14 => "LLIHF",
                15 => "LLILF",
                _ => "",
            };
            i.registers = vec![r1];
            let n = i32::from_be_bytes(b[2..6].try_into().unwrap());
            if r2 == 1 || r2 >= 6 {
                i.immediate = n as u32;
            } else {
                i.address = Some(Address {
                    base: 0,
                    index: 0,
                    displacement: i64::from(n) * 2,
                });
                if r2 == 4 {
                    i.registers.clear();
                    i.immediate = u32::from(r1);
                }
            }
            op
        }
        0xd1..=0xd7 | 0xdc..=0xdf | 0xf0..=0xf3 | 0xf8..=0xfd => {
            let op = match b[0] {
                0xd1 => "MVN",
                0xd2 => "MVC",
                0xd3 => "MVZ",
                0xd4 => "NC",
                0xd5 => "CLC",
                0xd6 => "OC",
                0xd7 => "XC",
                0xdc => "TR",
                0xdd => "TRT",
                0xde => "ED",
                0xdf => "EDMK",
                0xf0 => "SRP",
                0xf1 => "MVO",
                0xf2 => "PACK",
                0xf3 => "UNPK",
                0xf8 => "ZAP",
                0xf9 => "CP",
                0xfa => "AP",
                0xfb => "SP",
                0xfc => "MP",
                0xfd => "DP",
                _ => "",
            };
            i.address = Some(address(&b[2..4], 0));
            i.source = Some(address(&b[4..6], 0));
            if b[0] >= 0xf0 {
                i.length = u32::from(r1) + 1;
                i.source_length = u32::from(r2) + 1;
                if b[0] == 0xf0 {
                    i.immediate = u32::from(r2);
                }
            } else {
                i.length = u32::from(b[1]) + 1;
            }
            op
        }
        _ => "",
    };
    if op.is_empty() || super::assemble::instruction_size(op) != Some(b.len() as u32) {
        return Err(format!(
            "unsupported IBM instruction {:02X}{:02X} at offset {offset:#x}",
            b[0], b[1]
        ));
    }
    i.op = op.into();
    Ok(i)
}

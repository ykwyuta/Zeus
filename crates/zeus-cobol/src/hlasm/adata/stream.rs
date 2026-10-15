//! Lossless record transport, plus explicitly selected analysis record families.
use crate::hlasm::{AssemblyOptions, AssemblyReport};
mod semantic;
use serde::{Deserialize, Serialize};

const LIMIT: usize = 64 << 20;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdataRecord {
    pub record_type: u16,
    pub edition: u8,
    pub continued: bool,
    /// Byte order of the common-header length; payload is preserved verbatim.
    pub little_endian: bool,
    pub data: Vec<u8>,
}
impl AdataRecord {
    pub fn new(record_type: u16, edition: u8, data: Vec<u8>) -> Self {
        Self {
            record_type,
            edition,
            continued: false,
            little_endian: false,
            data,
        }
    }
}
/// Concatenated physical records, with optional four-byte variable-record RDWs.
/// All ADATA families and future record types retain their uninterpreted payload.
pub fn encode_adata_records(records: &[AdataRecord], rdw: bool) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for (index, r) in records.iter().enumerate() {
        if r.continued
            && !records.get(index + 1).is_some_and(|n| {
                n.record_type == r.record_type
                    && n.edition == r.edition
                    && n.little_endian == r.little_endian
            })
        {
            return Err(
                "ADATA continuation must be followed by the same record type/edition/byte order"
                    .into(),
            );
        }
        let length =
            u16::try_from(r.data.len()).map_err(|_| "ADATA payload exceeds 65535 bytes")?;
        let size = 12 + r.data.len() + if rdw { 4 } else { 0 };
        if out.len().checked_add(size).is_none_or(|n| n > LIMIT) {
            return Err("ADATA stream exceeds 64 MiB".into());
        }
        if rdw {
            let size = u16::try_from(size).map_err(|_| "ADATA record exceeds RDW length")?;
            out.extend(size.to_be_bytes());
            out.extend([0, 0]);
        }
        out.push(16);
        out.extend(r.record_type.to_be_bytes());
        out.push(3);
        out.push(u8::from(r.continued) | u8::from(r.little_endian) << 1);
        out.push(r.edition);
        out.extend([0; 4]);
        out.extend(if r.little_endian {
            length.to_le_bytes()
        } else {
            length.to_be_bytes()
        });
        out.extend(&r.data);
    }
    Ok(out)
}
/// Read physical records without conflating continuation payloads or interpreting
/// an opaque record family's fields. Truncation, bad flags and RDWs are errors.
pub fn decode_adata_records(bytes: &[u8], rdw: bool) -> Result<Vec<AdataRecord>, String> {
    if bytes.len() > LIMIT {
        return Err("ADATA stream exceeds 64 MiB".into());
    }
    let mut records = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let remaining = &bytes[at..];
        let (physical, header) = if rdw {
            if remaining.len() < 4 || remaining[2..4] != [0, 0] {
                return Err("invalid ADATA RDW".into());
            }
            let size = usize::from(u16::from_be_bytes(remaining[..2].try_into().unwrap()));
            if size < 16 || size > remaining.len() {
                return Err("truncated ADATA RDW record".into());
            }
            (Some(size), &remaining[4..size])
        } else {
            (None, remaining)
        };
        if header.len() < 12 {
            return Err("truncated ADATA common header".into());
        }
        if header[0] != 16 || header[3] != 3 || header[4] & !3 != 0 || header[6..10] != [0; 4] {
            return Err(
                "unsupported ADATA language/architecture/flags or nonzero reserved header".into(),
            );
        }
        let length = usize::from(if header[4] & 2 != 0 {
            u16::from_le_bytes(header[10..12].try_into().unwrap())
        } else {
            u16::from_be_bytes(header[10..12].try_into().unwrap())
        });
        let size = 12 + length;
        if size > header.len() || physical.is_some_and(|n| n != size + 4) {
            return Err("inconsistent or truncated ADATA record length".into());
        }
        records.push(AdataRecord {
            record_type: u16::from_be_bytes(header[1..3].try_into().unwrap()),
            edition: header[5],
            continued: header[4] & 1 != 0,
            little_endian: header[4] & 2 != 0,
            data: header[12..size].to_vec(),
        });
        at += physical.unwrap_or(size);
    }
    // Reuse continuation validation after the complete input is framed.
    encode_adata_records(&records, rdw)?;
    Ok(records)
}
pub(super) fn word(data: &mut [u8], at: usize, value: u32) {
    data[at..at + 4].copy_from_slice(&value.to_be_bytes());
}
pub(super) fn text(data: &mut Vec<u8>, at: usize, value: &str, ccsid: u16) -> Result<(), String> {
    let cp = crate::codepage::get(u32::from(ccsid)).ok_or("unsupported ADATA CCSID")?;
    let encoded = cp.encode(value);
    if cp.decode(&encoded) != value {
        return Err(format!("ADATA text cannot be represented in CCSID {ccsid}"));
    }
    if encoded.is_empty() {
        return Ok(());
    }
    let offset = (12 + data.len()) as u32;
    word(data, at, offset);
    word(data, at + 4, encoded.len() as u32);
    data.extend(encoded);
    Ok(())
}
fn unit(end: bool, count: u32) -> AdataRecord {
    let mut data = vec![0; 8];
    data[1] = u8::from(end);
    word(&mut data, 4, count);
    AdataRecord::new(2, 0, data)
}
/// Produce a compilation unit with identification, source analysis, diagnostics,
/// independently encoded machine instructions, user data and symbol references.
/// `universal_time` is the caller's raw eight-byte ADATA UT value, not Unix time.
/// This analysis subset does not yet produce all IBM assembler record families.
pub fn adata_bytes(
    report: &AssemblyReport,
    options: &AssemblyOptions,
    ccsid: u16,
    universal_time: u64,
    rdw: bool,
) -> Result<Vec<u8>, String> {
    crate::codepage::get(u32::from(ccsid)).ok_or("unsupported ADATA CCSID")?;
    let listing = report
        .listing
        .as_ref()
        .ok_or("ADATA output requires collect_listing")?;
    let mut records = vec![unit(false, 0)];
    let mut identification = universal_time.to_be_bytes().to_vec();
    identification.extend(ccsid.to_be_bytes());
    records.push(AdataRecord::new(1, 0, identification));
    // Original product identity and file names; never impersonate an IBM release.
    let mut job = vec![0; 94];
    let (date, _, time, _) = super::super::context::clock(options.started);
    let cp = crate::codepage::get(u32::from(ccsid)).unwrap();
    for (at, width, value) in [
        (0, 8, date.as_str()),
        (8, 4, &time.replace('.', "")),
        (12, 8, "ZEUS"),
        (20, 8, env!("CARGO_PKG_VERSION")),
        (30, 8, ""),
        (38, 24, "ZEUS"),
        (62, 8, options.job.as_deref().unwrap_or("")),
        (70, 8, options.step.as_deref().unwrap_or("")),
        (78, 8, ""),
    ] {
        let mut bytes = cp.encode(value);
        if bytes.len() > width || cp.decode(&bytes) != value {
            return Err("ADATA job metadata is not representable".into());
        }
        bytes.resize(width, 0x40);
        job[at..at + width].copy_from_slice(&bytes);
    }
    let primary: Vec<_> = listing.source_files.iter().filter(|f| !f.library).collect();
    word(&mut job, 86, primary.len() as u32);
    for (index, file) in primary.iter().enumerate() {
        let at = job.len();
        job.resize(at + 32, 0);
        if index == 0 {
            word(&mut job, 90, (at + 12) as u32);
        }
        word(&mut job, at + 4, (index + 1) as u32);
        text(&mut job, at + 8, &file.file, ccsid)?;
        if index + 1 < primary.len() {
            let next = (job.len() + 12) as u32;
            word(&mut job, at, next);
        }
    }
    records.push(AdataRecord::new(0, 1, job));
    let mut output = vec![0; 40];
    for (index, name) in ["SYSLIN", "SYSPUNCH", "SYSPRINT", "SYSTERM", "SYSADATA"]
        .iter()
        .enumerate()
    {
        if let Some(file) = options.data_sets.get(*name) {
            let at = output.len();
            output.resize(at + 32, 0);
            word(&mut output, index * 8, 1);
            word(&mut output, index * 8 + 4, (at + 12) as u32);
            word(&mut output, at + 4, 1);
            text(&mut output, at + 8, &file.name, ccsid)?;
            text(&mut output, at + 16, &file.volume, ccsid)?;
            text(&mut output, at + 24, &file.member, ccsid)?;
        }
    }
    records.push(AdataRecord::new(0x0a, 1, output));
    // Library concatenation IDs are independent of the primary SYSIN IDs.
    // In-stream macros use the documented PRIMARY INPUT library identity.
    for (index, file) in listing.source_files.iter().enumerate() {
        let mut members: std::collections::BTreeSet<String> = listing
            .macro_calls
            .iter()
            .filter(|call| call.definition_file == file.file)
            .map(|call| call.operation.clone())
            .collect();
        if file.library {
            members.insert(
                std::path::Path::new(&file.file)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        if members.is_empty() {
            continue;
        }
        let mut data = vec![0; 36];
        word(&mut data, 0, index as u32 + 1);
        let directory = if file.library {
            std::path::Path::new(&file.file)
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_string_lossy()
                .into_owned()
        } else {
            "PRIMARY INPUT".into()
        };
        text(&mut data, 4, &directory, ccsid)?;
        text(&mut data, 20, "SYSLIB", ccsid)?;
        word(&mut data, 28, members.len() as u32);
        let mut previous = None;
        for member in members {
            let at = data.len();
            data.resize(at + 12, 0);
            if let Some(previous) = previous {
                word(&mut data, previous, (at + 12) as u32);
            } else {
                word(&mut data, 32, (at + 12) as u32);
            }
            text(&mut data, at + 4, &member, ccsid)?;
            previous = Some(at);
        }
        records.push(AdataRecord::new(0x60, 1, data));
    }
    semantic::options(&mut records, report, options, ccsid)?;
    let module = report.module.as_ref();
    if let Some(module) = module {
        // The Zeus image is normalized into one SD; all offsets refer to it.
        let mut esd = vec![0; 52];
        esd[1] = match module.amode {
            24 => 1,
            64 => 0x10,
            _ => 2,
        };
        if module.rmode == 31 {
            esd[1] |= 4;
        }
        word(&mut esd, 4, 1);
        word(&mut esd, 20, module.storage.len() as u32);
        text(&mut esd, 36, &module.name, ccsid)?;
        records.push(AdataRecord::new(0x20, 1, esd));
    }
    for row in &listing.statements {
        let mut data = vec![0; 124];
        word(
            &mut data,
            0,
            u32::from(row.section.is_empty() && module.is_some()),
        );
        word(&mut data, 4, row.location.statement as u32);
        word(
            &mut data,
            8,
            if row.macro_depth > 0 {
                0
            } else {
                row.location.line as u32
            },
        );
        word(&mut data, 12, row.location.line as u32);
        let parent_primary = primary
            .iter()
            .position(|f| f.file == row.location.file)
            .map(|n| n as u32 + 1);
        let parent_library = listing
            .source_files
            .iter()
            .position(|f| f.file == row.location.file)
            .map_or(0, |n| n as u32 + 1);
        let input_file = row
            .definition
            .as_ref()
            .filter(|_| row.macro_depth > 0)
            .map_or(row.location.file.as_str(), |d| d.file.as_str());
        let input_primary = primary
            .iter()
            .position(|f| f.file == input_file)
            .map(|n| n as u32 + 1);
        let input_library = listing
            .source_files
            .iter()
            .position(|f| f.file == input_file)
            .map_or(0, |n| n as u32 + 1);
        let generated = row.macro_depth > 0;
        word(
            &mut data,
            16,
            if generated {
                input_library
            } else {
                input_primary.unwrap_or(input_library)
            },
        );
        word(&mut data, 20, parent_primary.unwrap_or(parent_library));
        word(&mut data, 24, row.offset);
        data[28] = if generated {
            2
        } else if input_primary.is_some() {
            1
        } else {
            3
        };
        data[29] = if parent_primary.is_some() { 1 } else { 3 };
        if generated {
            if let Some(call) = listing.macro_calls.iter().find(|call| {
                call.definition_file == input_file
                    && row
                        .definition
                        .as_ref()
                        .is_some_and(|d| d.line == call.definition_line)
            }) {
                text(&mut data, 92, &call.operation, ccsid)?;
            }
        } else if input_primary.is_none() {
            text(
                &mut data,
                92,
                &std::path::Path::new(input_file)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy(),
                ccsid,
            )?;
        }
        if parent_primary.is_none() {
            text(
                &mut data,
                100,
                &std::path::Path::new(&row.location.file)
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy(),
                ccsid,
            )?;
        }
        for (key, default, bit) in [
            ("GEN", true, 0x80),
            ("DATA", true, 0x40),
            ("ON", true, 0x20),
            ("UHEAD", false, 8),
            ("MCALL", false, 4),
        ] {
            if row.print.get(key).copied().unwrap_or(default) {
                data[30] |= bit;
            }
        }
        if !row.print.get("MSOURCE").copied().unwrap_or(true) {
            data[30] |= 0x10;
        }
        let instruction = module.and_then(|m| {
            m.code.iter().find(|i| {
                i.offset == row.offset && i.file == row.location.file && i.line == row.location.line
            })
        });
        data[33] = if instruction.is_some() { 2 } else { 3 };
        let opcodes = [
            "GBLA", "GBLB", "GBLC", "LCLA", "LCLB", "LCLC", "SETA", "SETB", "SETC", "AIF", "AGO",
            "ANOP", "COPY", "MACRO", "MNOTE", "MEXIT", "MEND", "ICTL", "ISEQ", "PRINT", "SPACE",
            "EJECT", "PUNCH", "REPRO", "TITLE", "ENTRY", "EXTRN", "START", "CSECT", "DSECT", "COM",
            "EQU", "ORG", "END", "LTORG", "USING", "DROP", "ACTR", "DC", "DS", "CCW", "CNOP",
            "LOCTR", "DXD", "CXD",
        ];
        data[34] = opcodes
            .iter()
            .position(|op| *op == row.operation)
            .map_or_else(
                || match row.operation.as_str() {
                    "OPSYN" => 0x2e,
                    "PUSH" => 0x2f,
                    "POP" => 0x30,
                    "MHELP" => 0x37,
                    "AREAD" => 0x38,
                    "WXTRN" => 0x3b,
                    "AMODE" => 0x3d,
                    "RMODE" => 0x3e,
                    "RSECT" => 0x3f,
                    "CCW0" => 0x40,
                    "CCW1" => 0x41,
                    "EXITCTL" => 0x42,
                    "ASPACE" => 0x43,
                    "AEJECT" => 0x44,
                    "ALIAS" => 0x45,
                    "CEJECT" => 0x46,
                    "ADATA" => 0x47,
                    "SETAF" => 0x48,
                    "SETCF" => 0x49,
                    "CATTR" => 0x4a,
                    "ACONTROL" => 0x4b,
                    "XATTR" => 0x4c,
                    "AINSERT" => 0x4d,
                    _ => 0,
                },
                |n| n as u8,
            );
        let source = format!("{} {} {}", row.label, row.operation, row.operands);
        text(&mut data, 108, &source, ccsid)?;
        for (at, prefix, value) in [
            (52, String::new(), row.label.as_str()),
            (60, format!("{} ", row.label), row.operation.as_str()),
            (
                68,
                format!("{} {} ", row.label, row.operation),
                row.operands.as_str(),
            ),
        ] {
            if !value.is_empty() {
                word(&mut data, at, cp.encode(&prefix).len() as u32);
                word(&mut data, at + 4, cp.encode(value).len() as u32);
            }
        }
        records.push(AdataRecord::new(0x30, 1, data));
        semantic::data(&mut records, row, module, ccsid)?;
        if let Some(i) = instruction {
            let bytes = super::super::object::instruction_bytes(i).map_err(|e| {
                format!(
                    "{}:{}: {}: ADATA instruction output: {e}",
                    i.file, i.line, i.op
                )
            })?;
            let mut data = vec![0; 28];
            word(&mut data, 0, 1);
            word(&mut data, 4, row.location.statement as u32);
            word(&mut data, 8, i.offset);
            word(&mut data, 20, 40);
            word(&mut data, 24, bytes.len() as u32);
            data.extend(bytes);
            records.push(AdataRecord::new(0x36, 1, data));
        }
        for user in listing
            .user_data
            .iter()
            .filter(|u| u.location.statement == row.location.statement)
        {
            let bytes = user.data.record(u32::from(ccsid))?;
            records.push(AdataRecord::new(0x70, 1, bytes[12..].to_vec()));
        }
    }
    for diagnostic in &report.diagnostics {
        let mut data = vec![0; 39];
        let statement = listing
            .statements
            .iter()
            .find(|s| s.location.file == diagnostic.file && s.location.line == diagnostic.line)
            .map_or(0, |s| s.location.statement as u32);
        word(&mut data, 0, statement);
        let mut identifier = cp.encode("ZEUS");
        identifier.resize(16, 0x40);
        data[4..20].copy_from_slice(&identifier);
        data[20..22].copy_from_slice(&u16::from(diagnostic.severity).to_be_bytes());
        text(&mut data, 23, &diagnostic.message, ccsid)?;
        records.push(AdataRecord::new(0x32, 1, data));
    }
    for (name, symbol) in &listing.symbols {
        let definition = symbol.definitions.first().map_or(0, |d| d.statement as u32);
        let mut data = vec![0; 50];
        word(
            &mut data,
            0,
            u32::from(symbol.section.as_deref() == Some("") && module.is_some()),
        );
        word(&mut data, 4, definition);
        word(&mut data, 8, symbol.value as u32);
        let source = listing
            .statements
            .iter()
            .find(|r| r.location.statement as u32 == definition);
        data[12] = if definition == 0 {
            0
        } else {
            match source.map(|r| r.operation.as_str()) {
                Some("CSECT" | "RSECT" | "START") => 1,
                Some("DSECT") => 2,
                Some("COM") => 3,
                Some("DXD") => 4,
                Some("EXTRN" | "WXTRN") => 7,
                Some("LOCTR") => 8,
                Some("EQU") => 0x0c,
                _ => 0x0d,
            }
        };
        if let Some(field) = source.and_then(|r| r.data_operands.first()) {
            word(&mut data, 13, field.duplication);
        }
        data[17] = symbol.type_attribute;
        data[18..22].fill(0x40);
        let assembler = cp.encode(&symbol.assembler_type);
        if assembler.len() > 4 {
            return Err("ADATA assembler type exceeds four bytes".into());
        }
        data[18..18 + assembler.len()].copy_from_slice(&assembler);
        word(&mut data, 22, symbol.program_type);
        word(&mut data, 26, symbol.length);
        data[30..32].copy_from_slice(&(symbol.integer_attribute as i16).to_be_bytes());
        data[32..34].copy_from_slice(&(symbol.scale_attribute as i16).to_be_bytes());
        data[34] = if !symbol.relocations.is_empty() {
            0xc0
        } else if symbol.section.is_some() {
            0x80
        } else {
            0
        };
        text(&mut data, 42, name, ccsid)?;
        records.push(AdataRecord::new(0x42, 1, data));
        let refs = &symbol.references;
        for (part, chunk) in refs.chunks(4096).enumerate() {
            let mut data = vec![0; 32];
            word(&mut data, 0, if part == 0 { definition } else { 0 });
            data[4] = if part > 0 {
                0x40
            } else if !symbol.relocations.is_empty() {
                0xc3
            } else if symbol.section.is_some() {
                0x40
            } else {
                0xc1
            };
            if part == 0 {
                text(&mut data, 12, name, ccsid)?;
                word(&mut data, 20, refs.len() as u32);
            }
            word(&mut data, 24, chunk.len() as u32);
            let at = data.len() + 12;
            word(&mut data, 28, at as u32);
            for reference in chunk {
                data.extend((reference.statement as u32).to_be_bytes());
                data.push(0x40);
            }
            let mut record = AdataRecord::new(0x44, 1, data);
            record.continued = (part + 1) * 4096 < refs.len();
            records.push(record);
        }
    }
    semantic::references(&mut records, report, ccsid)?;
    semantic::statistics(&mut records, report, options)?;
    // Common-header framing and Compilation Unit End include every physical
    // record, including both unit delimiters. RDWs are access-method framing.
    let count = u32::try_from(records.len() + 1).map_err(|_| "too many ADATA records")?;
    records.push(unit(true, count));
    encode_adata_records(&records, rdw)
}

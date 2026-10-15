//! Semantic ADATA records from retained assembly-time layout and references.
use super::{AdataRecord, text, word};
use crate::hlasm::{AssemblyOptions, AssemblyReport, ListingStatement, Module};
use std::collections::BTreeMap;
mod registers;
mod relocations;

pub(super) fn options(
    out: &mut Vec<AdataRecord>,
    report: &AssemblyReport,
    options: &AssemblyOptions,
    ccsid: u16,
) -> Result<(), String> {
    // No ASMAOPT reader is configured: the file-information count is zero.
    out.push(AdataRecord::new(0x0b, 1, vec![0; 8]));
    let listing = report.listing.as_ref().unwrap();
    let cp = crate::codepage::get(u32::from(ccsid)).unwrap();
    let mut data = vec![0; 197];
    data[0] = 0x21
        | if options.data_sets.contains_key("SYSLIN") {
            0x40
        } else {
            0
        };
    data[5] = 0x04
        | if !options.sysparm.is_empty() { 0x80 } else { 0 }
        | if listing.optable.is_some() { 8 } else { 0 };
    data[4] = 0x10; // USING maps are collected by this producer.
    for (key, onbyte, onbit, offbyte, offbit) in [
        ("ON", 3, 0x80, 8, 0x20),
        ("GEN", 3, 0x40, 8, 8),
        ("DATA", 3, 0x20, 8, 0x10),
        ("UHEAD", 3, 0x10, 8, 4),
        ("MSOURCE", 3, 8, 8, 2),
        ("MCALL", 8, 0x40, 8, 1),
    ] {
        if let Some(value) = listing.pcontrol.get(key) {
            if *value {
                data[onbyte] |= onbit;
            } else {
                data[offbyte] |= offbit;
            }
        }
    }
    if !listing.pcontrol.is_empty() {
        data[18] |= 0x20;
    }
    if listing.macrocase.is_some() {
        data[18] |= 0x80;
    }
    if listing.macrocase == Some(true) {
        data[10] |= 1;
    }
    data[9] |= 4; // CODEPAGE is explicit in this generated stream.
    data[23..27].copy_from_slice(&cp.encode(&format!("{ccsid:04}")));
    data[28..31].copy_from_slice(&cp.encode("ENG"));
    data[31..33].copy_from_slice(&60u16.to_be_bytes());
    data[33..36].copy_from_slice(&cp.encode("UNI"));
    data[36..44].fill(0x40);
    data[48..50].fill(0x40);
    text(&mut data, 93, &options.sysparm, ccsid)?;
    out.push(AdataRecord::new(0x10, 3, data));
    Ok(())
}
pub(super) fn data(
    out: &mut Vec<AdataRecord>,
    row: &ListingStatement,
    module: Option<&Module>,
    ccsid: u16,
) -> Result<(), String> {
    if let Some(error) = &row.data_layout_error {
        return Err(format!(
            "{}:{}: ADATA data layout: {error}",
            row.location.file, row.location.line
        ));
    }
    let count = row.data_operands.len();
    for (index, operand) in row.data_operands.iter().enumerate() {
        let mut data = vec![0; 22];
        if index == 0 {
            word(&mut data, 0, u32::from(row.section.is_empty()));
            word(&mut data, 10, row.location.statement as u32);
        }
        if index == 0 {
            if row.operation == "DC" {
                data[4] |= 0x80;
            }
            if row.data_operands.iter().any(|o| o.location_dependent) {
                data[4] |= 0x10;
            }
        }
        word(&mut data, 14, 1);
        word(&mut data, 18, 34);
        let at = data.len();
        data.resize(at + 31, 0);
        word(&mut data, at + 4, operand.offset);
        word(&mut data, at + 8, operand.duplication);
        data[at + 12] = operand.bit_offset;
        data[at + 13] = operand.type_attribute;
        data[at + 14] = if operand.type_extension.is_empty() {
            0x40
        } else {
            crate::codepage::get(u32::from(ccsid))
                .unwrap()
                .encode(&operand.type_extension)[0]
        };
        word(&mut data, at + 15, operand.program_type);
        word(&mut data, at + 23, operand.values.len() as u32);
        if !operand.values.is_empty() {
            let start = data.len() + 12;
            word(&mut data, at + 27, start as u32);
        }
        let mut previous = None;
        for value in &operand.values {
            let at = data.len();
            data.resize(at + 16, 0);
            if let Some(previous) = previous {
                word(&mut data, previous, (at + 12) as u32);
            }
            word(&mut data, at + 8, value.byte_length);
            word(&mut data, at + 12, value.bit_length);
            if !value.data.is_empty() {
                let start = data.len() + 12;
                word(&mut data, at + 4, start as u32);
                data.extend(&value.data);
            }
            previous = Some(at);
        }
        let mut record = AdataRecord::new(0x34, 1, data);
        record.continued = index + 1 < count;
        out.push(record);
    }
    if row.data_operands.iter().any(|o| o.location_dependent) {
        let image = module.ok_or("ADATA DC extension requires assembled data")?;
        let bytes = image
            .storage
            .get(row.offset as usize..row.offset as usize + row.length as usize)
            .ok_or("ADATA DC extension outside image")?;
        for (index, chunk) in bytes.chunks(60_000).enumerate() {
            let mut data = vec![0; 28];
            if index == 0 {
                word(&mut data, 0, 1);
                word(&mut data, 4, row.location.statement as u32);
            }
            word(&mut data, 8, row.offset + (index * 60_000) as u32);
            word(&mut data, 20, 40);
            word(&mut data, 24, chunk.len() as u32);
            data.extend(chunk);
            let mut record = AdataRecord::new(0x35, 1, data);
            record.continued = (index + 1) * 60_000 < bytes.len();
            out.push(record);
        }
    }
    Ok(())
}
fn reference_records(
    out: &mut Vec<AdataRecord>,
    kind: u16,
    head: Vec<u8>,
    count_at: usize,
    offset_at: usize,
    refs: &[(u32, u8)],
) {
    for (index, chunk) in refs.chunks(4096).enumerate() {
        let mut data = if index == 0 {
            head.clone()
        } else {
            vec![0; head.len()]
        };
        word(&mut data, count_at, chunk.len() as u32);
        let at = data.len() + 12;
        word(&mut data, offset_at, at as u32);
        for (statement, flag) in chunk {
            data.extend(statement.to_be_bytes());
            data.push(*flag);
        }
        let mut record = AdataRecord::new(kind, 1, data);
        record.continued = (index + 1) * 4096 < refs.len();
        out.push(record);
    }
}
pub(super) fn references(
    out: &mut Vec<AdataRecord>,
    report: &AssemblyReport,
    ccsid: u16,
) -> Result<(), String> {
    let listing = report.listing.as_ref().unwrap();
    if let Some(module) = &report.module {
        for (register, refs) in registers::collect(module, listing) {
            let mut data = vec![0; 16];
            data[0] = register;
            data[1] = 0xc7;
            word(&mut data, 4, refs.len() as u32);
            reference_records(out, 0x45, data, 8, 12, &refs);
        }
    }
    let mut macros: BTreeMap<(usize, String, String), Vec<(u32, u8)>> = BTreeMap::new();
    for call in &listing.macro_calls {
        let library = listing
            .source_files
            .iter()
            .position(|f| f.file == call.definition_file)
            .unwrap_or(0);
        let parent = listing
            .macro_calls
            .iter()
            .rev()
            .find(|p| {
                p.depth + 1 == call.depth
                    && p.statement <= call.statement
                    && p.statement + p.generated >= call.statement
            })
            .map_or("PRIMARY INPUT", |p| p.operation.as_str())
            .to_string();
        let parent = if call.parent.is_empty() {
            parent
        } else {
            call.parent.clone()
        };
        macros
            .entry((library, call.operation.clone(), parent))
            .or_default()
            .push((call.statement as u32, 0x40));
    }
    for call in &listing.copy_calls {
        let library = listing
            .source_files
            .iter()
            .position(|f| f.file == call.definition_file)
            .ok_or("ADATA COPY member has no source-file identity")?;
        macros
            .entry((library, call.member.clone(), call.parent.clone()))
            .or_default()
            .push((call.location.statement as u32, 0xc3));
    }
    let mut totals: BTreeMap<(usize, String), usize> = BTreeMap::new();
    for ((library, name, _), refs) in &macros {
        *totals.entry((*library, name.clone())).or_default() += refs.len();
    }
    for ((library, name, parent), refs) in macros {
        let mut data = vec![0; 46];
        word(&mut data, 0, library as u32 + 1);
        data[8] = if listing.source_files.get(library).is_some_and(|f| f.library) {
            0xd3
        } else {
            0xd7
        };
        data[9] = 0x40;
        text(&mut data, 18, &name, ccsid)?;
        text(&mut data, 26, &parent, ccsid)?;
        word(&mut data, 34, totals[&(library, name.clone())] as u32);
        reference_records(out, 0x62, data, 38, 42, &refs);
    }
    using(out, report, ccsid)?;
    relocations::emit(out, report, ccsid)?;
    Ok(())
}
fn using(out: &mut Vec<AdataRecord>, report: &AssemblyReport, ccsid: u16) -> Result<(), String> {
    let listing = report.listing.as_ref().unwrap();
    for (index, row) in listing.statements.iter().enumerate() {
        let after = listing
            .statements
            .get(index + 1)
            .map_or(&row.usings, |next| &next.usings);
        let kind = match row.operation.as_str() {
            "USING" => 0,
            "DROP" => 0x80,
            "PUSH" | "POP"
                if crate::hlasm::assemble::operands(&row.operands)
                    .iter()
                    .any(|s| s == "USING") =>
            {
                if row.operation == "PUSH" {
                    0x40
                } else {
                    0x20
                }
            }
            _ => continue,
        };
        let requested: std::collections::BTreeSet<u8> =
            crate::hlasm::assemble::operands(&row.operands)
                .iter()
                .skip(1)
                .filter_map(|s| {
                    s.parse::<u8>().ok().or_else(|| {
                        listing
                            .symbols
                            .get(s)
                            .and_then(|v| u8::try_from(v.value).ok())
                    })
                })
                .collect();
        let selected: Vec<_> = if kind == 0 {
            after
                .iter()
                .filter(|u| {
                    u.label == row.label
                        && (requested.is_empty() || requested.contains(&u.register))
                })
                .map(Some)
                .collect()
        } else {
            vec![None]
        };
        let drop_registers: Vec<u8> = if kind == 0x80 {
            if row.operands.is_empty() || row.operands == "ALL" {
                vec![0xff]
            } else {
                row.usings
                    .iter()
                    .filter(|u| {
                        !after.iter().any(|v| {
                            v.register == u.register && v.label == u.label && v.origin == u.origin
                        })
                    })
                    .map(|u| u.register)
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect()
            }
        } else {
            vec![]
        };
        let selected: Vec<_> = if kind == 0x80 {
            drop_registers.iter().map(|r| (None, Some(*r))).collect()
        } else {
            selected.into_iter().map(|u| (u, None)).collect()
        };
        for (current, drop_register) in selected {
            let mut data = vec![0; 44];
            data[0] = kind;
            word(&mut data, 2, u32::from(row.section.is_empty()));
            word(&mut data, 6, row.location.statement as u32);
            word(&mut data, 10, row.offset);
            if let Some(current) = current {
                data[1] = (u8::from(!current.label.is_empty()) * 0x10)
                    | (u8::from(current.dependent) * 0x20);
                word(&mut data, 14, current.origin as u32);
                word(
                    &mut data,
                    22,
                    u32::from(current.section.as_deref() == Some("")),
                );
                data[26] = current.register;
                let mut last = 0;
                let mut displacement = 0i64;
                for future in listing.statements.iter().skip(index + 1) {
                    if !future.usings.iter().any(|u| {
                        u.register == current.register
                            && u.label == current.label
                            && u.origin == current.origin
                    }) {
                        break;
                    }
                    if let Some(module) = &report.module {
                        for instruction in module.code.iter().filter(|i| {
                            i.offset == future.offset
                                && i.file == future.location.file
                                && i.line == future.location.line
                        }) {
                            for a in [&instruction.address, &instruction.source]
                                .into_iter()
                                .flatten()
                                .filter(|a| a.base == current.register)
                            {
                                last = future.location.statement as u32;
                                displacement = displacement.max(a.displacement);
                            }
                        }
                    }
                }
                word(&mut data, 18, last);
                let displacement = u16::try_from(displacement)
                    .map_err(|_| "ADATA USING displacement exceeds 16-bit field")?;
                data[27..29].copy_from_slice(&displacement.to_be_bytes());
                let range = current
                    .upper
                    .zip(current.lower)
                    .map_or(4096, |(hi, lo)| hi.saturating_sub(lo));
                word(&mut data, 30, range as u32);
            } else if kind == 0x80 {
                data[26] = drop_register.unwrap_or(0xff);
            }
            if kind == 0 || kind == 0x80 {
                let source = format!("{} {} {}", row.label, row.operation, row.operands);
                text(&mut data, 36, &source, ccsid)?;
                let length = (data.len() - 44).next_multiple_of(8);
                data.resize(44 + length, 0x40);
                word(&mut data, 40, length as u32);
            }
            out.push(AdataRecord::new(0x80, 1, data));
        }
    }
    Ok(())
}
fn packed_time(time: std::time::SystemTime) -> u32 {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let seconds = duration.as_secs();
    let digits = format!(
        "{:02}{:02}{:02}{:02}",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60,
        duration.subsec_millis() / 10
    );
    u32::from_str_radix(&digits, 16).unwrap()
}
pub(super) fn statistics(
    out: &mut Vec<AdataRecord>,
    report: &AssemblyReport,
    options: &AssemblyOptions,
) -> Result<(), String> {
    let listing = report.listing.as_ref().unwrap();
    let mut data = vec![0; 176];
    for file in &listing.source_files {
        let at = if file.library { 12 } else { 8 };
        let old = u32::from_be_bytes(data[at..at + 4].try_into().unwrap());
        word(&mut data, at, old.saturating_add(file.records.len() as u32));
    }
    word(&mut data, 32, (out.len() + 2) as u32);
    word(&mut data, 148, packed_time(options.started));
    word(
        &mut data,
        152,
        packed_time(
            options
                .started
                .checked_add(std::time::Duration::from_micros(listing.elapsed_micros))
                .ok_or("ADATA assembly end time exceeds host clock range")?,
        ),
    );
    // No access-method buffer pool, work files or assembler user exits are used.
    // Host CPU time is not available through the portable safe runtime API.
    out.push(AdataRecord::new(0x90, 2, data));
    Ok(())
}

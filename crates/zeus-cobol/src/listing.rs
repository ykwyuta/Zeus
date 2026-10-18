//! Human-readable Zeus compilation listings. Locations are ZIR instruction
//! indices and virtual data offsets, never IBM machine-code addresses.

#[derive(Debug, Clone)]
pub struct ListingOptions {
    pub flag: Option<crate::diag::Severity>,
    pub embedded_flag: Option<crate::diag::Severity>,
    pub explicit_embedded_flag: bool,
    pub source: bool,
    pub source_hex: bool,
    pub map: bool,
    pub list: bool,
    pub offset: bool,
    /// None: NOXREF; Some(false): SHORT; Some(true): FULL.
    pub xref: Option<bool>,
    pub vbref: bool,
}
impl Default for ListingOptions {
    fn default() -> Self {
        Self {
            flag: Some(crate::diag::Severity::Info),
            embedded_flag: Some(crate::diag::Severity::Info),
            explicit_embedded_flag: false,
            source: true,
            source_hex: false,
            map: false,
            list: false,
            offset: false,
            xref: Some(true),
            vbref: false,
        }
    }
}

impl ListingOptions {
    fn shows(&self, diagnostic: &Diag) -> bool {
        diagnostic.phase == crate::diag::Phase::Options
            || self.flag.is_some_and(|level| diagnostic.severity >= level)
    }
    fn embeds(&self, diagnostic: &Diag) -> bool {
        diagnostic.phase == crate::diag::Phase::Source
            && diagnostic.severity != crate::diag::Severity::Unrecoverable
            && self
                .embedded_flag
                .is_some_and(|level| diagnostic.severity >= level)
    }
}

fn diagnostic_summary(out: &mut String, diagnostics: &[Diag], options: &ListingOptions) {
    if diagnostics.iter().any(|d| options.shows(d)) {
        out.push_str("\nDIAGNOSTICS\n");
        for diagnostic in diagnostics.iter().filter(|d| options.shows(d)) {
            writeln!(out, "{diagnostic}").unwrap();
        }
    }
}

use crate::{
    diag::Diag,
    front::{
        FileTable,
        lexer::{Tok, Token},
    },
    zir::{Instr, Module, ProgramCode, Src},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
    path::Path,
};

fn position(program: &ProgramCode, pc: usize) -> Option<(u32, u32)> {
    program
        .instr_stmt
        .get(pc)
        .and_then(|index| program.stmt_pos.get(*index as usize))
        .copied()
}
fn location(pos: (u32, u32)) -> String {
    if pos.1 == 0 {
        "generated".into()
    } else {
        format!("{}:{}", pos.0 + 1, pos.1)
    }
}
fn descriptors(value: &serde_json::Value, out: &mut BTreeSet<u32>) {
    match value {
        serde_json::Value::Object(fields) => {
            if let Some(desc) = fields.get("desc").and_then(serde_json::Value::as_u64) {
                out.insert(desc as u32);
            }
            for value in fields.values() {
                descriptors(value, out);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                descriptors(value, out);
            }
        }
        _ => {}
    }
}

pub(crate) fn render(
    path: &Path,
    files: &FileTable,
    tokens: &[Token],
    result: &Result<Module, Diag>,
    options: &ListingOptions,
) -> String {
    let mut out = format!(
        "ZEUS COBOL {} COMPILATION LISTING\nSOURCE {}\nLocations: FILE:LINE; PC = ZIR instruction index. Data offsets are virtual bytes.\n",
        env!("CARGO_PKG_VERSION"),
        path.display()
    );
    writeln!(
        out,
        "OPTIONS SOURCE={} MAP={} LIST={} OFFSET={} XREF={} VBREF={}",
        options.source,
        options.map,
        options.list,
        options.offset,
        match options.xref {
            Some(true) => "FULL",
            Some(false) => "SHORT",
            None => "OFF",
        },
        options.vbref
    )
    .unwrap();
    let names = if files.names.is_empty() {
        vec![path.display().to_string()]
    } else {
        files.names.clone()
    };
    let diagnostics = match result {
        Ok(module) => module.diagnostics.as_slice(),
        Err(diagnostic) => std::slice::from_ref(diagnostic),
    };
    out.push_str("\nSOURCE/COPY FILE DIRECTORY\n");
    for (index, file) in names.iter().enumerate() {
        writeln!(out, "FILE {} {file}", index + 1).unwrap();
        if options.source {
            match files
                .text
                .get(file)
                .cloned()
                .map(Ok)
                .unwrap_or_else(|| std::fs::read_to_string(file))
            {
                Ok(source) => {
                    for (line, text) in source.lines().enumerate() {
                        if options.source_hex {
                            writeln!(out, "{:04X}:{:06X} {text}", index + 1, line + 1).unwrap();
                        } else {
                            writeln!(out, "{:04}:{:06} {text}", index + 1, line + 1).unwrap();
                        }
                        for diagnostic in diagnostics.iter().filter(|d| {
                            d.pos.file as usize == index
                                && d.pos.line as usize == line + 1
                                && options.embeds(d)
                        }) {
                            writeln!(out, "=={:04}:{:06}==> {diagnostic}", index + 1, line + 1)
                                .unwrap();
                        }
                    }
                }
                Err(_) => {
                    out.push_str(if file.starts_with("<system>") {
                        "  Generated source; references retain FILE:LINE positions.\n"
                    } else {
                        "  Source text is unavailable for this listing.\n"
                    });
                }
            }
        }
    }
    if options.source && !tokens.is_empty() {
        out.push_str("\nEXPANDED ACTIVE TOKENS (COPY/REPLACE applied; canonical spelling)\n");
        let mut previous = None;
        for token in tokens {
            let pos = (token.pos.file, token.pos.line);
            if previous != Some(pos) {
                if previous.is_some() {
                    out.push('\n');
                }
                if options.source_hex {
                    write!(out, "{:04X}:{:06X}", pos.0 + 1, pos.1).unwrap();
                } else {
                    write!(out, "{:04}:{:06}", pos.0 + 1, pos.1).unwrap();
                }
                previous = Some(pos);
            }
            if let Tok::Exec(kind, body) = &token.tok {
                let body: String = body.iter().map(|(c, _)| *c).collect();
                write!(
                    out,
                    " EXEC {kind} {} END-EXEC",
                    body.split_whitespace().collect::<Vec<_>>().join(" ")
                )
                .unwrap();
            } else if let Tok::Str(text) = &token.tok {
                write!(out, " \"{}\"", text.replace('"', "\"\"")).unwrap();
            } else {
                write!(out, " {}", token.text()).unwrap();
            }
        }
        out.push('\n');
    }
    match result {
        Err(diagnostic) => {
            diagnostic_summary(&mut out, diagnostics, options);
            writeln!(
                out,
                "END OF COMPILATION; RC={}",
                diagnostic.severity.return_code()
            )
            .unwrap();
        }
        Ok(module) => {
            writeln!(
                out,
                "\nCOMPILED {} CCSID={} OPTIMIZE={}\n",
                module.compiled_at.as_deref().unwrap_or("unknown"),
                module.ccsid,
                module.optimize
            )
            .unwrap();
            for program in &module.programs {
                writeln!(
                    out,
                    "PROGRAM {} DYNAM={} SSRANGE={}",
                    program.name, program.dynam, program.ssrange
                )
                .unwrap();
                if options.map {
                    out.push_str("DATA MAP: DESC NAME BASE OFFSET MAX-BYTES TYPE DEFINITION\n");
                    for (index, descriptor) in program.descs.iter().enumerate() {
                        writeln!(
                            out,
                            "#{index} {} {:?} {:08X} {} {:?} {}",
                            descriptor.name,
                            descriptor.base,
                            descriptor.offset,
                            descriptor.len,
                            descriptor.ty,
                            location(descriptor.definition)
                        )
                        .unwrap();
                        if descriptor.dynamic_offset.is_some() || descriptor.dynamic_len.is_some() {
                            writeln!(
                                out,
                                "  dynamic offset={:?} length={:?}",
                                descriptor.dynamic_offset, descriptor.dynamic_len
                            )
                            .unwrap();
                        }
                    }
                    out.push_str("PROCEDURE MAP: NAME PC DEFINITION\n");
                    for paragraph in &program.paragraphs {
                        writeln!(
                            out,
                            "{} {:08X} {}",
                            paragraph.name,
                            paragraph.start,
                            location(paragraph.pos)
                        )
                        .unwrap();
                    }
                    out.push_str("ENTRY MAP\n");
                    for entry in &program.entries {
                        writeln!(
                            out,
                            "{} PC={:08X} USING={:?}",
                            entry.name, entry.pc, entry.using
                        )
                        .unwrap();
                    }
                }
                if options.list || options.offset {
                    out.push_str(if options.list {
                        "ZIR CODE LIST (Zeus virtual instructions)\n"
                    } else {
                        "OFFSET TABLE (ZIR PC; not IBM byte offsets)\n"
                    });
                    let mut last = None;
                    for (pc, instruction) in program.code.iter().enumerate() {
                        let pos = position(program, pc);
                        if options.list {
                            writeln!(
                                out,
                                "{pc:08X} {} {instruction:?}",
                                pos.map(location).unwrap_or_else(|| "generated".into())
                            )
                            .unwrap();
                        } else if pos != last
                            && let Some(pos) = pos
                        {
                            writeln!(out, "{} PC={pc:08X}", location(pos)).unwrap();
                        }
                        last = pos;
                    }
                    if options.list {
                        out.push_str("INITIAL PRIVATE STORAGE (hex bytes)\n");
                        for (line, bytes) in program.static_init.chunks(32).enumerate() {
                            write!(out, "{:08X}", line * 32).unwrap();
                            for byte in bytes {
                                write!(out, " {byte:02X}").unwrap();
                            }
                            out.push('\n');
                        }
                    }
                }
                if let Some(full) = options.xref {
                    let mut references: BTreeMap<u32, BTreeSet<(u32, u32)>> = BTreeMap::new();
                    let mut calls: BTreeMap<String, BTreeSet<(u32, u32)>> = BTreeMap::new();
                    let mut procedures: BTreeMap<u32, BTreeSet<(u32, u32)>> = BTreeMap::new();
                    let mut file_refs: BTreeMap<u32, BTreeSet<(u32, u32)>> = BTreeMap::new();
                    for (pc, instruction) in program.code.iter().enumerate() {
                        let Some(pos) = position(program, pc) else {
                            continue;
                        };
                        let mut ids = BTreeSet::new();
                        match instruction {
                            Instr::Perform { start, end, .. } => {
                                procedures.entry(*start).or_default().insert(pos);
                                if let Some(paragraph) = program.paragraphs.get(*end as usize) {
                                    procedures.entry(paragraph.start).or_default().insert(pos);
                                }
                            }
                            Instr::Jump(target)
                            | Instr::JumpIf(_, target)
                            | Instr::JumpUnless(_, target)
                            | Instr::Alter(_, target) => {
                                procedures.entry(*target).or_default().insert(pos);
                            }
                            Instr::GoDepending(_, targets) => {
                                for target in targets {
                                    procedures.entry(*target).or_default().insert(pos);
                                }
                            }
                            _ => {}
                        }
                        let file = match instruction {
                            Instr::Open(file, _)
                            | Instr::Close(file, _)
                            | Instr::CloseReel(file)
                            | Instr::Delete(file)
                            | Instr::SortBegin(file)
                            | Instr::SortRead(file)
                            | Instr::SortWrite(file)
                            | Instr::OpenTape { file, .. }
                            | Instr::CloseTape { file, .. }
                            | Instr::Write { file, .. }
                            | Instr::Rewrite { file, .. }
                            | Instr::Read { file, .. }
                            | Instr::Start { file, .. }
                            | Instr::SortOrder { file, .. } => Some(*file),
                            _ => None,
                        };
                        if let Some(file) = file {
                            file_refs.entry(file).or_default().insert(pos);
                        }
                        descriptors(
                            &serde_json::to_value(instruction).expect("serializable ZIR"),
                            &mut ids,
                        );
                        match instruction {
                            Instr::Write { record, .. } | Instr::Rewrite { record, .. } => {
                                ids.insert(*record);
                            }
                            Instr::Read { file, .. } => {
                                ids.insert(program.files[*file as usize].record);
                            }
                            _ => {}
                        }
                        for id in ids {
                            references.entry(id).or_default().insert(pos);
                        }
                        if let Instr::Call {
                            program: Src::Alnum(bytes),
                            ..
                        }
                        | Instr::SetEntry(_, Src::Alnum(bytes))
                        | Instr::SetEntryWithInterface(_, Src::Alnum(bytes), _) = instruction
                        {
                            let name = crate::codepage::get(module.ccsid).unwrap().decode(bytes);
                            calls.entry(name).or_default().insert(pos);
                        }
                    }
                    out.push_str("DATA CROSS-REFERENCE: DESC NAME DEFINITION REFERENCES (resolved operands)\n");
                    for (index, descriptor) in program.descs.iter().enumerate() {
                        let positions = references.get(&(index as u32));
                        if !full && positions.is_none() {
                            continue;
                        }
                        write!(
                            out,
                            "#{index} {} DEF={}",
                            descriptor.name,
                            location(descriptor.definition)
                        )
                        .unwrap();
                        if let Some(positions) = positions {
                            for &pos in positions {
                                write!(out, " {}", location(pos)).unwrap();
                            }
                        }
                        out.push('\n');
                    }
                    out.push_str("PROGRAM/ENTRY CROSS-REFERENCE (literal targets)\n");
                    for (name, positions) in calls {
                        write!(out, "{name}").unwrap();
                        for pos in positions {
                            write!(out, " {}", location(pos)).unwrap();
                        }
                        out.push('\n');
                    }
                    out.push_str("PROCEDURE CROSS-REFERENCE (generated branch/PERFORM targets)\n");
                    for paragraph in &program.paragraphs {
                        let positions = procedures.get(&paragraph.start);
                        if !full && positions.is_none() {
                            continue;
                        }
                        write!(
                            out,
                            "{} PC={:08X} DEF={}",
                            paragraph.name,
                            paragraph.start,
                            location(paragraph.pos)
                        )
                        .unwrap();
                        if let Some(positions) = positions {
                            for &pos in positions {
                                write!(out, " {}", location(pos)).unwrap();
                            }
                        }
                        out.push('\n');
                    }
                    out.push_str("FILE CROSS-REFERENCE\n");
                    for (index, file) in program.files.iter().enumerate() {
                        let positions = file_refs.get(&(index as u32));
                        if !full && positions.is_none() {
                            continue;
                        }
                        write!(out, "{} ASSIGN={}", file.name, file.assign).unwrap();
                        if let Some(positions) = positions {
                            for &pos in positions {
                                write!(out, " {}", location(pos)).unwrap();
                            }
                        }
                        out.push('\n');
                    }
                }
            }
            if options.vbref {
                let statements: BTreeSet<_> = module
                    .programs
                    .iter()
                    .flat_map(|p| p.stmt_pos.iter().copied())
                    .collect();
                let mut verbs: BTreeMap<&str, BTreeSet<(u32, u32)>> = BTreeMap::new();
                for token in tokens {
                    let pos = (token.pos.file, token.pos.line);
                    if statements.contains(&pos)
                        && let Tok::Word(word) = &token.tok
                        && crate::front::parser::VERBS.contains(&word.as_str())
                    {
                        verbs.entry(word).or_default().insert(pos);
                    }
                }
                out.push_str("\nVERB CROSS-REFERENCE (active source)\n");
                for (verb, positions) in verbs {
                    write!(out, "{verb}").unwrap();
                    for pos in positions {
                        write!(out, " {}", location(pos)).unwrap();
                    }
                    out.push('\n');
                }
            }
            diagnostic_summary(&mut out, diagnostics, options);
            writeln!(
                out,
                "END OF COMPILATION; RC={}",
                module.compilation_return_code()
            )
            .unwrap();
        }
    }
    out
}

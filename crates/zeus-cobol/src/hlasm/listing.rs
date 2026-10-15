//! Assembly-time source locations and symbols. Instruction slots in Zeus
//! modules are decoded templates, so they are never presented as IBM bytes.
mod data;
use super::{
    Module,
    assemble::{Statement, Symbols},
};
pub use data::{DataOperand, DataValue};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyCall {
    pub location: SourceLocation,
    pub member: String,
    pub definition_file: String,
    pub parent: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroCall {
    #[serde(default)]
    pub parent: String,
    /// The next expanded statement, including calls which produce no statements.
    #[serde(default)]
    pub statement: usize,
    #[serde(default)]
    pub generated: usize,
    #[serde(default)]
    pub print: BTreeMap<String, bool>,
    pub file: String,
    pub line: usize,
    pub label: String,
    pub operation: String,
    pub operands: String,
    pub depth: usize,
    pub definition_file: String,
    #[serde(default)]
    pub definition_line: usize,
}
#[derive(Clone, Default)]
pub(super) struct Attributes {
    pub print: BTreeMap<String, bool>,
    pub macro_depth: usize,
    pub calls: Vec<MacroCall>,
    pub definition: Option<SourceLocation>,
    pub model: Option<ModelStatement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStatement {
    pub label: String,
    pub operation: String,
    pub operands: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsingSummary {
    pub register: u8,
    pub origin: i64,
    pub section: Option<String>,
    pub label: String,
    pub dependent: bool,
    pub lower: Option<i64>,
    pub upper: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssemblyListing {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub pcontrol: BTreeMap<String, bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub process_statements: Vec<super::ProcessStatement>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub optable: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macrocase: Option<bool>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macro_calls: Vec<MacroCall>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub copy_calls: Vec<CopyCall>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_files: Vec<SourceFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub literals: Vec<ListingLiteral>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub user_data: Vec<ListingUserData>,
    #[serde(default)]
    pub elapsed_micros: u64,
    pub statements: Vec<ListingStatement>,
    pub symbols: BTreeMap<String, ListingSymbol>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingUserData {
    pub location: SourceLocation,
    pub data: super::UserData,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingLiteral {
    pub name: String,
    pub operand: String,
    pub offset: u32,
    pub length: u32,
    pub data: Vec<u8>,
    pub pool: SourceLocation,
    pub references: Vec<SourceLocation>,
}

pub(super) fn build_literals(
    module: &Module,
    original: &[Statement],
    symbols: &Symbols,
    emitted: &[super::assemble::EmittedLiteral],
) -> Vec<ListingLiteral> {
    let location = |index: usize| {
        let index = index.min(original.len().saturating_sub(1));
        SourceLocation {
            statement: index + 1,
            file: original[index].file.clone(),
            line: original[index].line,
        }
    };
    emitted
        .iter()
        .map(|literal| {
            let offset = symbols[&literal.name].value as u32;
            ListingLiteral {
                name: literal.name.clone(),
                operand: format!("={}", literal.spec),
                offset,
                length: literal.length as u32,
                data: module.storage[offset as usize..offset as usize + literal.length].to_vec(),
                pool: location(literal.pool),
                references: literal
                    .references
                    .iter()
                    .map(|index| location(*index))
                    .collect(),
            }
        })
        .collect()
}

/// Decoded physical input captured when read, before input and macro controls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFile {
    pub file: String,
    pub encoding: String,
    pub format: String,
    pub library: bool,
    pub records: Vec<SourceRecord>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRecord {
    pub line: usize,
    /// Unicode columns for UTF-8, original byte columns for EBCDIC.
    pub columns: usize,
    pub text: String,
    /// One-based original column for each Unicode scalar, only when nonidentity.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub positions: Vec<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceLocation {
    pub statement: usize,
    pub file: String,
    pub line: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingStatement {
    #[serde(default)]
    pub print: BTreeMap<String, bool>,
    #[serde(default)]
    pub macro_depth: usize,
    #[serde(default)]
    pub macro_calls: Vec<MacroCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub definition: Option<SourceLocation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<ModelStatement>,
    #[serde(default)]
    pub space_lines: u64,
    #[serde(default)]
    pub usings: Vec<UsingSummary>,
    pub location: SourceLocation,
    pub label: String,
    pub operation: String,
    pub operands: String,
    /// Empty denotes executable image offsets; named sections are dummy areas.
    pub section: String,
    pub offset: u32,
    pub length: u32,
    /// Initialized data only, never synthetic instruction template bytes.
    pub data: Vec<u8>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub data_operands: Vec<DataOperand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_layout_error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListingSymbol {
    pub value: i64,
    pub section: Option<String>,
    pub length: u32,
    pub type_attribute: u8,
    #[serde(default)]
    pub integer_attribute: i64,
    #[serde(default)]
    pub scale_attribute: i64,
    #[serde(default)]
    pub program_type: u32,
    #[serde(default)]
    pub assembler_type: String,
    pub relocations: BTreeMap<String, i64>,
    pub definitions: Vec<SourceLocation>,
    pub references: Vec<SourceLocation>,
}

fn references(statement: &Statement, known: &Symbols) -> BTreeSet<String> {
    let text = statement.args.to_ascii_uppercase();
    let bytes = text.as_bytes();
    let mut type_positions = BTreeSet::new();
    if matches!(statement.op.as_str(), "DC" | "DS") {
        let mut from = 0;
        for item in super::assemble::operands(&text) {
            if let Some(relative) = text[from..].find(&item) {
                let origin = from + relative;
                if let Ok(position) = super::constants::type_start(&item) {
                    type_positions.insert(origin + position);
                }
                from = origin + item.len();
            }
        }
    }
    let mut found = BTreeSet::new();
    let mut at = 0;
    let mut quoted = false;
    while at < bytes.len() {
        if bytes[at] == b'\'' && (quoted || !super::assemble::attribute_quote(&text, at)) {
            quoted = !quoted;
            at += 1;
        } else if quoted {
            at += 1;
        } else if bytes[at] == b'=' {
            if let Ok(end) = super::assemble::literal_end(&text, at)
                && let Ok(position) = super::constants::type_start(&text[at + 1..end])
            {
                type_positions.insert(at + 1 + position);
            }
            at += 1;
        } else if bytes[at].is_ascii_alphabetic() || b"@#$_.".contains(&bytes[at]) {
            let start = at;
            at += 1;
            while at < bytes.len()
                && (bytes[at].is_ascii_alphanumeric() || b"@#$_.".contains(&bytes[at]))
            {
                at += 1;
            }
            let name = &text[start..at];
            let attribute = at < bytes.len() && bytes[at] == b'\'';
            if !attribute && !type_positions.contains(&start) && known.contains_key(name) {
                found.insert(name.into());
            }
        } else {
            at += 1;
        }
    }
    found
}

pub(super) fn build(
    module: &Module,
    original: &[Statement],
    lines: &[Statement],
    symbols: &Symbols,
    lengths: &HashMap<usize, u32>,
    usings: &HashMap<usize, Vec<UsingSummary>>,
) -> AssemblyListing {
    let mut listing = AssemblyListing::default();
    for (name, value) in symbols {
        listing.symbols.insert(
            name.clone(),
            ListingSymbol {
                value: value.value,
                section: value.section.clone(),
                length: value.length,
                type_attribute: value.type_attribute,
                scale_attribute: value.numeric_attributes.map_or(0, |v| v.0),
                integer_attribute: value.numeric_attributes.map_or(0, |v| v.1),
                program_type: value.program_type.unwrap_or(0),
                assembler_type: value.assembler_type.clone().unwrap_or_default(),
                relocations: value.complex.clone(),
                definitions: vec![],
                references: vec![],
            },
        );
    }
    for (index, (source, placed)) in original.iter().zip(lines).enumerate() {
        let location = SourceLocation {
            statement: index + 1,
            file: source.file.clone(),
            line: source.line,
        };
        if source.op == "ADATA" {
            listing.user_data.push(ListingUserData {
                location: location.clone(),
                data: super::adata::parse(&source.args)
                    .expect("ADATA syntax was validated during expansion"),
            });
        }
        if let Some(symbol) = listing.symbols.get_mut(&source.label) {
            symbol.definitions.push(location.clone());
        }
        for name in references(source, symbols) {
            if let Some(symbol) = listing.symbols.get_mut(&name) {
                if matches!(source.op.as_str(), "EXTRN" | "WXTRN") {
                    symbol.definitions.push(location.clone());
                } else {
                    symbol.references.push(location.clone());
                }
            }
        }
        let length = lengths
            .get(&index)
            .copied()
            .or_else(|| super::assemble::instruction_size(&source.op))
            .unwrap_or(0);
        let data =
            if placed.section.is_empty() && matches!(source.op.as_str(), "DC" | "DCB" | "CXD") {
                module
                    .storage
                    .get(placed.offset as usize..placed.offset as usize + length as usize)
                    .unwrap_or_default()
                    .to_vec()
            } else {
                vec![]
            };
        let attributes = source.listing.clone().unwrap_or_default();
        let space_lines = if matches!(source.op.as_str(), "SPACE" | "CEJECT") {
            if source.args.is_empty() {
                1
            } else {
                super::assemble::expression(&source.args, symbols, &placed.section, placed.offset)
                    .map_or(0, |v| v.value.max(0) as u64)
            }
        } else {
            0
        };
        let (data_operands, data_layout_error) = if matches!(source.op.as_str(), "DC" | "DS") {
            match data::layout(module, placed, symbols) {
                Ok(fields) => (fields, None),
                Err(error) => (vec![], Some(error)),
            }
        } else {
            (vec![], None)
        };
        listing.statements.push(ListingStatement {
            print: attributes.print,
            macro_depth: attributes.macro_depth,
            macro_calls: attributes.calls,
            definition: attributes.definition,
            model: attributes.model,
            space_lines,
            usings: usings.get(&index).cloned().unwrap_or_default(),
            location,
            label: source.label.clone(),
            operation: source.op.clone(),
            operands: source.args.clone(),
            section: placed.section.clone(),
            offset: placed.offset,
            length,
            data,
            data_operands,
            data_layout_error,
        });
    }
    listing
}

#[derive(Default)]
struct Page {
    lines: u64,
    title: String,
    header_pending: bool,
}
impl Page {
    fn eject(&mut self, out: &mut String, _row: &ListingStatement) {
        if self.lines != 0 {
            out.push('\u{c}');
        }
        self.lines = 0;
        self.header_pending = true;
    }
    fn header(&mut self, out: &mut String, row: &ListingStatement, print: &BTreeMap<String, bool>) {
        if !self.title.is_empty() {
            let _ = writeln!(out, "{}", self.title);
        }
        if print.get("UHEAD").copied().unwrap_or(false) {
            if row.usings.is_empty() {
                out.push_str("Active USINGs: none\n");
            } else {
                out.push_str("Active USINGs:\n");
                for using in &row.usings {
                    let section = using
                        .section
                        .as_deref()
                        .map_or("ABS", |s| if s.is_empty() { "IMAGE" } else { s });
                    let _ = writeln!(
                        out,
                        "  R{}={section}+{} {}{}",
                        using.register,
                        using.origin,
                        using.label,
                        if using.dependent { " (dependent)" } else { "" }
                    );
                }
            }
        }
        self.header_pending = false;
    }
    fn line(&mut self, out: &mut String, row: &ListingStatement, text: &str) {
        self.line_with_print(out, row, &row.print, text);
    }
    fn line_with_print(
        &mut self,
        out: &mut String,
        row: &ListingStatement,
        print: &BTreeMap<String, bool>,
        text: &str,
    ) {
        if self.lines >= 60 {
            self.eject(out, row);
        }
        if self.header_pending {
            self.header(out, row, print);
        }
        let _ = writeln!(out, "{text}");
        self.lines += 1;
    }
}
fn first_macro_output<'a>(
    rows: &'a [ListingStatement],
    next: &[Option<usize>],
    pools: &BTreeMap<usize, Vec<&'a ListingLiteral>>,
    call: &MacroCall,
    fallback: &'a ListingStatement,
) -> Option<&'a [u8]> {
    if call.statement == 0 {
        return Some(&fallback.data);
    } // Older listing JSON.
    let start = call.statement - 1;
    let end = start.checked_add(call.generated)?;
    let index = (*next.get(start)?)?;
    if index >= end {
        return None;
    }
    let first = &rows[index];
    // An instruction remains the first object statement even though Zeus
    // templates do not contain architectural bytes for display.
    if super::assemble::instruction_size(&first.operation).is_some() || !first.data.is_empty() {
        return Some(&first.data);
    }
    pools
        .get(&first.location.statement)?
        .first()
        .map(|literal| literal.data.as_slice())
}
fn render_macro_call(
    page: &mut Page,
    out: &mut String,
    row: &ListingStatement,
    call: &MacroCall,
    first: Option<&[u8]>,
) {
    let print = if call.print.is_empty() {
        &row.print
    } else {
        &call.print
    };
    let enabled = |key: &str, default| print.get(key).copied().unwrap_or(default);
    if !enabled("ON", true) || call.depth > 0 && !(enabled("GEN", true) && enabled("MCALL", false))
    {
        return;
    }
    page.line_with_print(
        out,
        row,
        print,
        &format!(
            "      MACRO {} {} {} [{}:{}; definition {}:{}]",
            call.label,
            call.operation,
            call.operands,
            call.file,
            call.line,
            call.definition_file,
            call.definition_line
        ),
    );
    if !enabled("GEN", true)
        && let Some(first) = first.filter(|bytes| !bytes.is_empty())
    {
        let bytes = first
            .iter()
            .take(8)
            .map(|b| format!("{b:02X}"))
            .collect::<String>();
        page.line_with_print(out, row, print, &format!("      {bytes}"));
    }
}

impl AssemblyListing {
    /// Original decoded records, independent of PRINT, including comments and
    /// macro definitions. Repeated reads of a member remain separate snapshots.
    pub fn original_source_text(&self) -> String {
        let mut text = String::from("Zeus HLASM original source records\n");
        for source in &self.source_files {
            let _ = writeln!(
                text,
                "\n{} [{}; {}; {}]",
                source.file,
                source.encoding,
                source.format,
                if source.library { "library" } else { "input" }
            );
            for record in &source.records {
                let _ = writeln!(text, "{:>6} |{}", record.line, record.text);
            }
        }
        text
    }

    pub fn text(&self) -> String {
        if self.pcontrol.get("ON") == Some(&false) {
            return String::new();
        }
        let mut out = String::from(
            "ZEUS EXPANDED SOURCE LISTING\nData bytes are initialized image bytes; instruction slots contain decoded Zeus templates.\n\n STMT OFFSET   LENGTH SECTION SOURCE\n",
        );
        for statement in &self.process_statements {
            let _ = writeln!(
                out,
                "*PROCESS {} [{}:{}]",
                statement.options, statement.file, statement.line
            );
        }
        let mut page = Page::default();
        let mut pools = BTreeMap::<usize, Vec<&ListingLiteral>>::new();
        for literal in &self.literals {
            pools
                .entry(literal.pool.statement)
                .or_default()
                .push(literal);
        }
        // Next generated object statement for every insertion position. A
        // control/directive prefix must not hide a later first DC, and a first
        // instruction must not claim the bytes of some later unrelated DC.
        let mut next = vec![None; self.statements.len() + 1];
        for index in (0..self.statements.len()).rev() {
            let row = &self.statements[index];
            let object = pools.contains_key(&row.location.statement)
                || super::assemble::instruction_size(&row.operation).is_some()
                || row.length > 0
                    && matches!(row.operation.as_str(), "DC" | "DCB" | "CXD" | "CNOP");
            next[index] = if object { Some(index) } else { next[index + 1] };
        }
        let mut calls = self.macro_calls.iter().peekable();
        for row in &self.statements {
            let enabled = |key: &str, default| row.print.get(key).copied().unwrap_or(default);
            let on = enabled("ON", true);
            let suppress_control =
                matches!(
                    row.operation.as_str(),
                    "PRINT" | "PUSH" | "POP" | "ACONTROL"
                ) && super::assemble::operands(&row.operands.to_ascii_uppercase())
                    .iter()
                    .any(|a| a == "NOPRINT");
            if self.macro_calls.is_empty() {
                for call in &row.macro_calls {
                    render_macro_call(
                        &mut page,
                        &mut out,
                        row,
                        call,
                        first_macro_output(&self.statements, &next, &pools, call, row),
                    );
                }
            } else {
                while calls
                    .peek()
                    .is_some_and(|call| call.statement <= row.location.statement)
                {
                    let call = calls.next().unwrap();
                    render_macro_call(
                        &mut page,
                        &mut out,
                        row,
                        call,
                        first_macro_output(&self.statements, &next, &pools, call, row),
                    );
                }
            }
            if let Some(model) = &row.model
                && on
                && !suppress_control
                && row.operation != "PRINT"
            {
                page.line(
                    &mut out,
                    row,
                    &format!(
                        "      MODEL {} {} {} [{}:{}]",
                        model.label,
                        model.operation,
                        model.operands,
                        row.location.file,
                        row.location.line
                    ),
                );
                if !enabled("GEN", true) {
                    let data = if !row.data.is_empty() {
                        row.data.as_slice()
                    } else {
                        &[]
                    };
                    if !data.is_empty() {
                        let bytes = data
                            .iter()
                            .take(8)
                            .map(|b| format!("{b:02X}"))
                            .collect::<String>();
                        page.line(&mut out, row, &format!("      {bytes}"));
                    }
                    continue;
                }
            }
            if suppress_control
                || !on && row.operation != "PRINT"
                || row.macro_depth > 0 && !enabled("GEN", true) && row.operation != "PRINT"
            {
                continue;
            }
            match row.operation.as_str() {
                "EJECT" => {
                    if page.lines > 0 {
                        page.eject(&mut out, row);
                    }
                    continue;
                }
                "CEJECT" => {
                    if page.lines > 0
                        && (row.operands.is_empty()
                            || row.space_lines == 0
                            || 60 - page.lines < row.space_lines)
                    {
                        page.eject(&mut out, row);
                    }
                    continue;
                }
                "TITLE" => {
                    page.title = row.operands.trim().trim_matches('\'').replace("''", "'");
                    page.eject(&mut out, row);
                    continue;
                }
                "SPACE" => {
                    if row.space_lines > 60 - page.lines {
                        page.eject(&mut out, row);
                    } else {
                        for _ in 0..row.space_lines {
                            page.line(&mut out, row, "");
                        }
                    }
                    continue;
                }
                _ => {}
            }
            let source =
                row.operation == "PRINT" || row.macro_depth == 0 || enabled("MSOURCE", true);
            page.line(
                &mut out,
                row,
                &format!(
                    "{:5} {:08X} {:6} {:8} {} {} {}",
                    row.location.statement,
                    row.offset,
                    row.length,
                    if row.section.is_empty() {
                        "IMAGE"
                    } else {
                        &row.section
                    },
                    if source { &row.label } else { "" },
                    if source { &row.operation } else { "" },
                    if source { &row.operands } else { "" }
                ),
            );
            page.line(
                &mut out,
                row,
                &format!("      {}:{}", row.location.file, row.location.line),
            );
            if source && let Some(definition) = &row.definition {
                page.line(
                    &mut out,
                    row,
                    &format!("      definition {}:{}", definition.file, definition.line),
                );
            }
            let count = if enabled("DATA", false) {
                row.data.len()
            } else {
                row.data.len().min(8)
            };
            for (index, chunk) in row.data[..count].chunks(16).enumerate() {
                let mut bytes = format!("      {:08X} ", row.offset as usize + index * 16);
                for byte in chunk {
                    let _ = write!(bytes, "{byte:02X}");
                }
                page.line(&mut out, row, &bytes);
            }
            for literal in pools.get(&row.location.statement).into_iter().flatten() {
                page.line(
                    &mut out,
                    row,
                    &format!(
                        "      {:08X} {:6} LITERAL {} {}",
                        literal.offset, literal.length, literal.name, literal.operand
                    ),
                );
                let count = if enabled("DATA", false) {
                    literal.data.len()
                } else {
                    literal.data.len().min(8)
                };
                for (index, chunk) in literal.data[..count].chunks(16).enumerate() {
                    let mut bytes = format!("      {:08X} ", literal.offset as usize + index * 16);
                    for byte in chunk {
                        let _ = write!(bytes, "{byte:02X}");
                    }
                    page.line(&mut out, row, &bytes);
                }
            }
        }
        if let Some(last) = self.statements.last() {
            for call in calls {
                render_macro_call(
                    &mut page,
                    &mut out,
                    last,
                    call,
                    first_macro_output(&self.statements, &next, &pools, call, last),
                );
            }
        }
        out
    }
    pub fn cross_reference(&self) -> String {
        let mut out = String::from(
            "ZEUS EXPANDED SOURCE SYMBOL CROSS-REFERENCE\nReferences give file:line#expanded-statement, including library members.\n\nSYMBOL VALUE LENGTH SECTION DEFINITIONS REFERENCES\n",
        );
        for (name, symbol) in &self.symbols {
            let positions = |items: &[SourceLocation]| {
                items
                    .iter()
                    .map(|p| format!("{}:{}#{}", p.file, p.line, p.statement))
                    .collect::<Vec<_>>()
                    .join(",")
            };
            let section = symbol
                .section
                .as_deref()
                .map_or("ABS", |s| if s.is_empty() { "IMAGE" } else { s });
            let _ = writeln!(
                out,
                "{name} {} {} {section} [{}] [{}]",
                symbol.value,
                symbol.length,
                positions(&symbol.definitions),
                positions(&symbol.references)
            );
        }
        if !self.literals.is_empty() {
            out.push_str("\nLITERAL OFFSET LENGTH POOL REFERENCES\n");
        }
        for literal in &self.literals {
            let references = literal
                .references
                .iter()
                .map(|p| format!("{}:{}#{}", p.file, p.line, p.statement))
                .collect::<Vec<_>>()
                .join(",");
            let _ = writeln!(
                out,
                "{} {:08X} {} [{}:{}#{}] [{}]",
                literal.operand,
                literal.offset,
                literal.length,
                literal.pool.file,
                literal.pool.line,
                literal.pool.statement,
                references
            );
        }
        out
    }
}

use super::{Address, Instruction, Module, Relocation};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub(super) struct Statement {
    pub(super) label: String,
    pub(super) op: String,
    pub(super) args: String,
    pub(super) file: String,
    pub(super) line: usize,
    pub(super) sequence: String,
    pub(super) section: String,
    pub(super) offset: u32,
}

impl Statement {
    pub(super) fn error(&self, message: impl std::fmt::Display) -> String {
        format!("{}:{}: {}: {message}", self.file, self.line, self.op)
    }
}

struct Literal {
    name: String,
    spec: String,
    bytes: Vec<u8>,
    length: u32,
    location: u32,
    location_section: String,
    numeric_attributes: Option<(i64, i64)>,
}

struct LiteralFixup {
    pool: usize,
    spec: String,
    pub(super) offset: u32,
    location: u32,
    location_section: String,
    section: String,
}

#[derive(Default)]
struct LiteralPool {
    pending: Vec<Literal>,
    names: HashMap<String, String>,
    serial: usize,
}

impl LiteralPool {
    fn add(
        &mut self,
        spec: &str,
        location: u32,
        location_section: &str,
        defined: &HashSet<String>,
        symbols: &Symbols,
    ) -> Result<String, String> {
        if super::constants::duplication(spec, symbols)?.1 == 0 {
            return Err("literal duplication factor must be positive".into());
        }
        let key = if literal_has_location(spec) {
            format!("{spec}@{location_section}:{location}")
        } else {
            spec.to_string()
        };
        if let Some(name) = self.names.get(&key) {
            return Ok(name.clone());
        }
        let name = loop {
            self.serial += 1;
            let name = format!("@ZL{:08}", self.serial);
            if !defined.contains(name.as_str()) {
                break name;
            }
        };
        let (bytes, _, length) = storage(spec, true, symbols)?;
        self.pending.push(Literal {
            name: name.clone(),
            spec: spec.into(),
            bytes,
            length,
            location,
            location_section: location_section.into(),
            numeric_attributes: storage_numeric_attributes(spec, length, symbols).ok(),
        });
        self.names.insert(key, name.clone());
        Ok(name)
    }

    fn flush(
        &mut self,
        offset: &mut u32,
        image: &mut Vec<u8>,
        symbols: &mut Symbols,
        fixups: &mut Vec<LiteralFixup>,
        section: &str,
        pool: usize,
    ) -> Result<(), String> {
        if self.pending.is_empty() {
            return Ok(());
        }
        *offset = offset.next_multiple_of(8);
        // Stable length-segment ordering: 16, 8, 4, 2, then odd lengths.
        self.pending.sort_by_key(|l| {
            [16, 8, 4, 2]
                .iter()
                .position(|n| l.bytes.len().is_multiple_of(*n))
                .unwrap_or(4)
        });
        for literal in self.pending.drain(..) {
            let end = (*offset as usize)
                .checked_add(literal.bytes.len())
                .filter(|n| *n <= 16 << 20)
                .ok_or("literal pool exceeds 16 MiB")?;
            image.resize(image.len().max(end), 0);
            image[*offset as usize..end].copy_from_slice(&literal.bytes);
            symbols.insert(
                literal.name,
                Symbol {
                    section: Some(section.into()),
                    value: i64::from(*offset),
                    length: literal.length,
                    type_attribute: storage_type(&literal.spec),
                    numeric_attributes: literal.numeric_attributes,
                    program_type: super::constants::annotations(&literal.spec, symbols)?.0,
                    assembler_type: Some(super::constants::annotations(&literal.spec, symbols)?.1),
                    complex: BTreeMap::new(),
                },
            );
            if address_constants(&literal.spec, symbols)?.is_some() {
                fixups.push(LiteralFixup {
                    pool,
                    spec: literal.spec,
                    offset: *offset,
                    location: literal.location,
                    location_section: literal.location_section,
                    section: section.into(),
                });
            }
            *offset = end as u32;
        }
        self.names.clear();
        Ok(())
    }
}

pub(super) fn statements(path: &Path) -> Result<Vec<Statement>, String> {
    super::source::statements(path, &super::AssemblyOptions::default(), false, &mut vec![])
}

/// Split operands while preserving commas in strings and address subfields.
pub(super) fn operands(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let (mut depth, mut quoted, mut start) = (0i32, false, 0);
    for (i, c) in text.char_indices() {
        match c {
            '\'' if quoted || !attribute_quote(text, i) => quoted = !quoted,
            '(' if !quoted => depth += 1,
            ')' if !quoted => depth -= 1,
            ',' if !quoted && depth == 0 => {
                result.push(text[start..i].to_string());
                start = i + 1;
            }
            _ => {}
        }
    }
    result.push(text[start..].to_string());
    result
}

pub(super) fn attribute_quote(text: &str, at: usize) -> bool {
    at > 0
        && b"LKNDTOSI".contains(&text.as_bytes()[at - 1].to_ascii_uppercase())
        && (at == 1
            || !text.as_bytes()[at - 2].is_ascii_alphanumeric()
                && !b"_@$#".contains(&text.as_bytes()[at - 2]))
}

pub(super) fn member(name: &str, source: &Path, dirs: &[PathBuf]) -> Result<PathBuf, String> {
    for dir in dirs
        .iter()
        .cloned()
        .chain(source.parent().map(Path::to_path_buf))
    {
        for name in [name.to_string(), name.to_ascii_lowercase()] {
            for ext in ["", ".mac", ".MAC", ".asm", ".ASM", ".cpy"] {
                let path = dir.join(format!("{name}{ext}"));
                if path.is_file() {
                    return Ok(path);
                }
            }
        }
    }
    Err(format!("COPY/macro member {name} not found"))
}

pub(super) fn directive(op: &str) -> bool {
    matches!(
        op,
        "CSECT"
            | "DCB"
            | "LOCTR"
            | "CNOP"
            | "PUSH"
            | "POP"
            | "RSECT"
            | "START"
            | "DSECT"
            | "DXD"
            | "CXD"
            | "XATTR"
            | "CATTR"
            | "USING"
            | "DROP"
            | "EQU"
            | "DS"
            | "DC"
            | "END"
            | "LTORG"
            | "AMODE"
            | "RMODE"
            | "TITLE"
            | "PRINT"
            | "ACONTROL"
            | "ENTRY"
            | "EXTRN"
            | "WXTRN"
            | "ORG"
    )
}

/// Long-displacement encodings share semantics and operand shapes with these
/// legacy operations, while retaining their own six-byte instruction size.
pub(super) fn canonical_operation(op: &str) -> &str {
    match op {
        "LY" => "L",
        "STY" => "ST",
        "LHY" => "LH",
        "STHY" => "STH",
        "ICY" => "IC",
        "STCY" => "STC",
        "AY" => "A",
        "SY" => "S",
        "CY" => "C",
        "CLY" => "CL",
        "ALY" => "AL",
        "SLY" => "SL",
        "NY" => "N",
        "OY" => "O",
        "XY" => "X",
        "AHY" => "AH",
        "SHY" => "SH",
        "CHY" => "CH",
        "LMY" => "LM",
        "STMY" => "STM",
        "CLIY" => "CLI",
        "MVIY" => "MVI",
        "NIY" => "NI",
        "OIY" => "OI",
        "XIY" => "XI",
        "TMY" => "TM",
        "CVBY" => "CVB",
        "CVDY" => "CVD",
        _ => op,
    }
}

pub(super) fn pair_operation(op: &str) -> bool {
    matches!(
        op,
        "ML" | "MLR"
            | "FLOGR"
            | "LPD"
            | "LPDG"
            | "MLG"
            | "MLGR"
            | "DL"
            | "DLR"
            | "DLG"
            | "DLGR"
            | "DSG"
            | "DSGR"
            | "DSGF"
            | "DSGFR"
            | "SLDL"
            | "SRDL"
            | "SLDA"
            | "SRDA"
    )
}

pub(super) fn shift_operation(op: &str) -> bool {
    matches!(
        op,
        "SLLG" | "SRLG" | "SLAG" | "SRAG" | "RLL" | "RLLG" | "SLDL" | "SRDL" | "SLDA" | "SRDA"
    )
}

fn wide_memory(op: &str) -> bool {
    matches!(
        op,
        "BCTG"
            | "ALC"
            | "ALCG"
            | "SLB"
            | "SLBG"
            | "LRV"
            | "LRVG"
            | "LRVH"
            | "STRV"
            | "STRVG"
            | "STRVH"
            | "LB"
            | "LGB"
            | "ML"
            | "MLG"
            | "DL"
            | "DLG"
            | "DSG"
            | "DSGF"
            | "LG"
            | "STG"
            | "LGF"
            | "LLGF"
            | "LGH"
            | "LLGH"
            | "LLGC"
            | "AG"
            | "SG"
            | "ALG"
            | "SLG"
            | "CG"
            | "CLG"
            | "NG"
            | "OG"
            | "XG"
            | "AGF"
            | "SGF"
            | "CGF"
            | "MSG"
            | "LMG"
            | "STMG"
    )
}

pub(super) fn instruction_size(op: &str) -> Option<u32> {
    if super::operation::register_logic(op).is_some() || super::operation::select(op).is_some() {
        return Some(4);
    }
    if super::operation::rotate_selected(op).is_some() {
        return Some(6);
    }
    if let Some(spec) = super::operation::conditional(op) {
        return Some(
            if spec.kind == super::operation::ConditionalKind::Register {
                4
            } else {
                6
            },
        );
    }
    if let Some((size, _)) = super::operation::interlocked(op) {
        return Some(size);
    }
    if let Some(spec) = super::operation::relative(op) {
        return Some(spec.size());
    }
    if let Some(spec) = super::operation::masked_bytes(op) {
        return Some(if spec.long { 6 } else { 4 });
    }
    if let Some(spec) = super::operation::partial_immediate(op) {
        return Some(if spec.width == 16 { 4 } else { 6 });
    }
    if wide_memory(op) || canonical_operation(op) != op {
        return Some(6);
    }
    Some(match op {
        "POPCNT" | "FLOGR" | "BCTGR" | "ALCR" | "ALCGR" | "SLBR" | "SLBGR" | "LRVR" | "LRVGR"
        | "LBR" | "LGBR" | "MLR" | "MLGR" | "DLR" | "DLGR" | "DSGR" | "DSGFR" => 4,
        "SLLG" | "SRLG" | "SLAG" | "SRAG" | "RLL" | "RLLG" | "LPD" | "LPDG" | "CSST" => 6,
        "SLDL" | "SRDL" | "SLDA" | "SRDA" => 4,
        "LGR" | "LGFR" | "LLGFR" | "LTGR" | "LCGR" | "LPGR" | "LNGR" | "AGR" | "SGR" | "ALGR"
        | "SLGR" | "CGR" | "CLGR" | "NGR" | "OGR" | "XGR" | "AGFR" | "SGFR" | "CGFR" | "MSGR"
        | "LGHI" | "AGHI" | "CGHI" | "MGHI" => 4,
        "LGFI" | "AGFI" | "CGFI" => 6,
        "MVCLE" | "CLCLE" | "MVST" | "CLST" | "SRST" => 4,
        "MVCL" | "CLCL" | "BCTR" | "LR" | "SR" | "AR" | "CR" | "LTR" | "BR" | "BCR" | "XR"
        | "NR" | "OR" | "CLR" | "ALR" | "SLR" | "LCR" | "LPR" | "LNR" | "MR" | "DR" | "BALR"
        | "BASR" | "NOPR" => 2,
        "EX" | "L" | "ST" | "LA" | "STM" | "LM" | "CLI" | "MVI" | "B" | "BE" | "BNE" | "BH"
        | "BL" | "BCT" | "AHI" | "A" | "S" | "C" | "CL" | "AL" | "SL" | "N" | "O" | "X" | "AH"
        | "SH" | "CH" | "MH" | "M" | "D" | "LH" | "STH" | "IC" | "STC" | "SLL" | "SRL" | "SLA"
        | "SRA" | "NI" | "OI" | "XI" | "TM" | "BC" | "BAL" | "BAS" | "NOP" | "BNH" | "BNL"
        | "BO" | "BNO" | "BP" | "BM" | "BZ" | "BNZ" | "BNP" | "BNM" | "CVB" | "CVD" => 4,
        "MVC" | "CLC" | "XC" | "NC" | "OC" | "MVN" | "MVZ" | "TR" | "ZAP" | "AP" | "SP" | "CP"
        | "MP" | "DP" | "PACK" | "UNPK" | "TRT" | "TRTR" | "SRP" | "ED" | "EDMK" => 6,
        // STIMER is implemented as an OS service, not an IBM macro expansion.
        "STIMER" | "GETMAIN" | "FREEMAIN" | "STORAGE" | "LINK" | "XCTL" | "LOAD" | "DELETE"
        | "WTO" | "WAIT" | "POST" | "OPEN" | "CLOSE" | "GET" | "PUT" | "FREEPOOL" => 4,
        _ => return None,
    })
}

#[derive(Clone)]
pub(super) struct Symbol {
    pub(super) section: Option<String>,
    pub(super) value: i64,
    pub(super) length: u32,
    pub(super) type_attribute: u8,
    pub(super) numeric_attributes: Option<(i64, i64)>,
    pub(super) program_type: Option<u32>,
    pub(super) assembler_type: Option<String>,
    /// Only complex EQU values use this map. Simple symbols retain section
    /// identity so consumers requiring an absolute/simple value can reject it.
    pub(super) complex: BTreeMap<String, i64>,
}
pub(super) type Symbols = HashMap<String, Symbol>;

pub(super) fn storage_type(spec: &str) -> u8 {
    let (clean, _, assembler) = super::constants::annotated(spec)
        .unwrap_or_else(|_| (std::borrow::Cow::Borrowed(spec), None, String::new()));
    let spec = clean.as_ref();
    let at = super::constants::type_start(spec).unwrap_or(0);
    let kind = spec
        .as_bytes()
        .get(at)
        .copied()
        .unwrap_or(b'U')
        .to_ascii_uppercase();
    let modifiers = spec.get(at + assembler.len()..).unwrap_or("");
    let kind = if modifiers.starts_with('L') {
        match kind {
            b'F' | b'H' => b'G',
            b'A' | b'S' | b'Q' | b'J' | b'R' | b'V' | b'Y' => b'R',
            b'E' | b'D' | b'L' => b'K',
            b'G' => b'@',
            _ => kind,
        }
    } else {
        match kind {
            b'J' => b'A',
            b'R' => b'V',
            b'G' => b'@',
            _ => kind,
        }
    };
    crate::ebcdic::encode(&(kind as char).to_string())[0]
}

pub(super) fn literal_end(text: &str, start: usize) -> Result<usize, String> {
    if text.as_bytes().get(start) != Some(&b'=') {
        return Err("literal must start with =".into());
    }
    let type_end = start + 1 + super::constants::type_start(&text[start + 1..])? + 1;
    let mut at = type_end;
    while let Some(&c) = text.as_bytes().get(at) {
        if c == b'('
            && at > type_end
            && matches!(
                text.as_bytes().get(at - 1),
                Some(b'L' | b'S' | b'E' | b'P' | b'.' | b'+' | b'-')
            )
        {
            at += super::constants::parenthesized_end(&text[at..])?;
            continue;
        }
        if c == b'\'' || c == b'(' {
            break;
        }
        if !c.is_ascii_alphanumeric() && !b".+-".contains(&c) {
            return Err("invalid literal specification".into());
        }
        at += 1;
    }
    match text.as_bytes().get(at) {
        Some(b'\'') => {
            at += 1;
            while let Some(&c) = text.as_bytes().get(at) {
                at += 1;
                if c == b'\'' {
                    if text.as_bytes().get(at) == Some(&b'\'') {
                        at += 1;
                    } else {
                        return Ok(at);
                    }
                }
            }
            Err("unterminated literal constant".into())
        }
        Some(b'(') => {
            let (mut depth, mut quoted) = (0u32, false);
            while let Some(&c) = text.as_bytes().get(at) {
                if c == b'\'' && (quoted || !attribute_quote(text, at)) {
                    quoted = !quoted;
                }
                if !quoted {
                    if c == b'(' {
                        depth += 1;
                    }
                    if c == b')' {
                        depth -= 1;
                        if depth == 0 {
                            return Ok(at + 1);
                        }
                    }
                }
                at += 1;
            }
            Err("unclosed literal address expression".into())
        }
        _ => Err("literal constant value required".into()),
    }
}

pub(super) fn literal_attributes(literal: &str, symbols: &Symbols) -> Result<(u32, u8), String> {
    if literal_end(literal, 0)? != literal.len() {
        return Err("unexpected literal suffix".into());
    }
    let spec = &literal[1..];
    if super::constants::duplication(spec, symbols)?.1 == 0 {
        return Err("literal duplication factor must be positive".into());
    }
    let (_, _, length) = storage(spec, true, symbols)?;
    Ok((length, storage_type(spec)))
}

/// Numeric attributes for the first constant: (scale, integer capacity).
pub(super) fn storage_numeric_attributes(
    spec: &str,
    length: u32,
    symbols: &Symbols,
) -> Result<(i64, i64), String> {
    let first = operands(spec)
        .into_iter()
        .next()
        .ok_or("constant required")?;
    let (first, _, _) = super::constants::annotated(&first)?;
    let at = super::constants::type_start(&first)?;
    let kind = first.as_bytes()[at].to_ascii_uppercase();
    let scale = if matches!(kind, b'P' | b'Z') {
        let nominal = first
            .split('\'')
            .nth(1)
            .unwrap_or("")
            .split(',')
            .next()
            .unwrap_or("");
        nominal
            .split_once('.')
            .map_or(0, |(_, fraction)| fraction.len() as i64)
    } else if matches!(kind, b'F' | b'H') {
        let tail = if let Some((_, after_length)) = super::bit_constants::length(&first, symbols)? {
            &first[after_length..]
        } else {
            &first[at + 1..]
        };
        let tail = if kind == b'F' {
            tail.strip_prefix('D').unwrap_or(tail)
        } else {
            tail
        };
        let modifiers = &tail[..super::constants::nominal_quote(tail)?.unwrap_or(tail.len())];
        i64::from(super::constants::fixed_modifiers(modifiers, symbols)?.1)
    } else {
        0
    };
    let length = i64::from(length);
    let integer = match kind {
        b'F' | b'H' => 8 * length - scale - 1,
        b'P' => 2 * length - scale - 1,
        b'Z' => length - scale,
        _ => {
            return Err(
                "scale/integer attributes require a supported fixed-point or decimal constant"
                    .into(),
            );
        }
    };
    Ok((scale, integer))
}

pub(super) fn literal_has_location(spec: &str) -> bool {
    let (mut quoted, mut previous) = (false, None);
    for (at, c) in spec.char_indices() {
        if c == '\'' && (quoted || !attribute_quote(spec, at)) {
            quoted = !quoted;
        }
        if !quoted
            && c == '*'
            && previous.is_none_or(|c| matches!(c, '(' | ',' | '+' | '-' | '*' | '/'))
        {
            return true;
        }
        if !quoted && !c.is_whitespace() {
            previous = Some(c);
        }
    }
    false
}

// The attribute view follows source order, independently of forward constants
// precomputed for layout. Pending EQU values become available once their
// dependencies have actually been defined.
fn resolve_attribute_equ(
    symbols: &mut Symbols,
    pending: &mut Vec<(Statement, super::expression::EquAttributes)>,
) {
    loop {
        let mut changed = false;
        pending.retain(|(s, attributes)| {
            if let Ok(mut value) =
                super::expression::evaluate_equ(&s.args, symbols, &s.section, s.offset)
            {
                attributes.apply(&mut value);
                symbols.insert(s.label.clone(), value);
                changed = true;
                false
            } else {
                true
            }
        });
        if !changed {
            break;
        }
    }
}

pub(super) fn expression(
    text: &str,
    symbols: &Symbols,
    section: &str,
    offset: u32,
) -> Result<Symbol, String> {
    super::expression::evaluate(text, symbols, section, offset)
}
/// (bytes, alignment, length attribute). Limits keep malformed declarations
/// from allocating arbitrary amounts of memory while compiling.
pub(super) fn storage(
    spec: &str,
    initialized: bool,
    symbols: &Symbols,
) -> Result<(Vec<u8>, u32, u32), String> {
    storage_with_origin(spec, initialized, symbols, 0)
}

fn storage_with_origin(
    spec: &str,
    initialized: bool,
    symbols: &Symbols,
    origin: u32,
) -> Result<(Vec<u8>, u32, u32), String> {
    let list = operands(spec);
    if list.len() == 1 {
        return storage_item(spec, initialized, symbols);
    }
    let (mut bytes, mut alignment, mut length) = (Vec::new(), 1, 1);
    let mut position = 0;
    let mut origin = origin as usize;
    for (i, part) in list.iter().enumerate() {
        if let Some(field) = super::bit_constants::assemble(part, initialized, symbols)? {
            if i == 0 {
                length = field.length;
            }
            if position + field.bits > 8 << 20 {
                return Err("storage declaration exceeds 1 MiB".into());
            }
            super::bit_constants::append(&mut bytes, &mut position, &field.bytes, field.bits);
            continue;
        }
        let (data, align, len) = storage_item(part, initialized, symbols)?;
        if i == 0 {
            alignment = align;
            length = len;
            origin = origin.next_multiple_of(align as usize);
        }
        bytes.resize(
            (origin + bytes.len()).next_multiple_of(align as usize) - origin,
            0,
        );
        if bytes.len().saturating_add(data.len()) > 1 << 20 {
            return Err("storage declaration exceeds 1 MiB".into());
        }
        bytes.extend(data);
        position = bytes.len() * 8;
    }
    Ok((bytes, alignment, length))
}

pub(super) fn storage_item(
    spec: &str,
    initialized: bool,
    symbols: &Symbols,
) -> Result<(Vec<u8>, u32, u32), String> {
    let (clean, _, _) = super::constants::annotated(spec)?;
    let spec = clean.as_ref();
    if let Some(field) = super::bit_constants::assemble(spec, initialized, symbols)? {
        return Ok((field.bytes, 1, field.length));
    }
    if let Some(adcon) = address_constants(spec, symbols)? {
        let AddressConstants {
            count,
            values,
            width,
            alignment,
            ..
        } = adcon;
        let size = count
            .checked_mul(values.len().max(1))
            .and_then(|n| n.checked_mul(width as usize))
            .filter(|n| *n <= 1 << 20)
            .ok_or("storage declaration exceeds 1 MiB")?;
        return Ok((vec![0; size], alignment, width.into()));
    }
    let (type_at, count) = super::constants::duplication(spec, symbols)?;
    let kind = spec.as_bytes()[type_at].to_ascii_uppercase();
    let tail = &spec[type_at + 1..];
    let doubleword = kind == b'F' && tail.starts_with('D');
    let character_extension = if kind == b'C' {
        tail.chars().next().filter(|c| matches!(c, 'A' | 'E' | 'U'))
    } else {
        None
    };
    let extension = doubleword || character_extension.is_some();
    let tail = if extension { &tail[1..] } else { tail };
    let quote = super::constants::nominal_quote(tail)?;
    let initialized = initialized && (count != 0 || quote.is_some());
    let modifiers = &tail[..quote.unwrap_or(tail.len())];
    let (explicit, scale, exponent) = if matches!(kind, b'F' | b'H') {
        super::constants::fixed_modifiers(modifiers, symbols)?
    } else {
        let explicit = super::constants::byte_length(modifiers, symbols)?;
        (explicit, 0, 0)
    };
    let literal = if let Some(q) = quote {
        tail[q + 1..]
            .strip_suffix('\'')
            .ok_or("unterminated constant")?
            .replace("''", "'")
    } else {
        String::new()
    };
    if matches!(kind, b'F' | b'H' | b'A' | b'P' | b'Z' | b'X' | b'B') && literal.contains(',') {
        let q = quote.unwrap();
        let mut result = Vec::new();
        let (mut alignment, mut length) = (1, 1);
        for (index, value) in literal.split(',').enumerate() {
            let item = format!(
                "{}'{}'",
                &spec[type_at..type_at + 1 + usize::from(doubleword) + q],
                value
            );
            let (bytes, align, len) = storage_item(&item, initialized, symbols)?;
            if index == 0 {
                alignment = align;
                length = len;
            }
            result.extend(bytes);
            if result.len().saturating_mul(count as usize) > 1 << 20 {
                return Err("storage declaration exceeds 1 MiB".into());
            }
        }
        return Ok((result.repeat(count as usize), alignment, length));
    }
    let character = kind == b'C' || kind == b'G';
    let unicode = character_extension == Some('U');
    let graphic = kind == b'G';
    let (mut bytes, align) = match kind {
        b'C' if character_extension == Some('A') => (
            literal
                .chars()
                .map(|c| {
                    u8::try_from(u32::from(c))
                        .map_err(|_| "CA constant character is outside CCSID 819")
                })
                .collect::<Result<Vec<_>, _>>()?,
            1,
        ),
        b'C' if unicode => (
            literal.encode_utf16().flat_map(u16::to_be_bytes).collect(),
            1,
        ),
        b'C' => (crate::ebcdic::encode(&literal), 1),
        b'G' => {
            let cp = crate::codepage::active();
            if !cp.is_mixed() {
                return Err("G constant requires a mixed DBCS ZEUS_CCSID".into());
            }
            let bytes = cp.encode_dbcs(&literal);
            if cp.decode_dbcs(&bytes) != literal {
                return Err("G constant contains an unmappable graphic character".into());
            }
            (bytes, 1)
        }
        b'X' => {
            if !literal.is_ascii() {
                return Err("invalid hex constant".into());
            }
            let literal = if literal.len().is_multiple_of(2) {
                literal.clone()
            } else {
                format!("0{literal}")
            };
            let bytes = (0..literal.len())
                .step_by(2)
                .map(|i| {
                    u8::from_str_radix(&literal[i..i + 2], 16).map_err(|_| "invalid hex constant")
                })
                .collect::<Result<Vec<_>, _>>()?;
            (bytes, 1)
        }
        b'B' => {
            if literal.chars().any(|c| c != '0' && c != '1') {
                return Err("invalid binary constant".into());
            }
            let padded = format!("{}{}", "0".repeat((8 - literal.len() % 8) % 8), literal);
            let bytes = padded
                .as_bytes()
                .chunks(8)
                .map(|chunk| chunk.iter().fold(0u8, |a, b| (a << 1) | (b - b'0')))
                .collect();
            (bytes, 1)
        }
        b'P' | b'Z' => {
            let nominal = literal.strip_prefix(['+', '-']).unwrap_or(&literal);
            let digits = nominal.replace('.', "");
            if initialized
                && (digits.is_empty()
                    || nominal.matches('.').count() > 1
                    || digits.chars().any(|c| !c.is_ascii_digit()))
            {
                return Err(
                    "decimal constant requires digits and at most one decimal point".into(),
                );
            }
            let negative = literal.starts_with('-');
            if kind == b'Z' {
                let mut bytes: Vec<u8> = digits.bytes().map(|c| 0xf0 | (c - b'0')).collect();
                if let Some(last) = bytes.last_mut() {
                    *last = (*last & 15) | if negative { 0xd0 } else { 0xc0 };
                }
                (bytes, 1)
            } else {
                let mut nibbles: Vec<u8> = digits.bytes().map(|c| c - b'0').collect();
                nibbles.push(if negative { 0xd } else { 0xc });
                if !nibbles.len().is_multiple_of(2) {
                    nibbles.insert(0, 0);
                }
                (
                    nibbles
                        .chunks_exact(2)
                        .map(|p| (p[0] << 4) | p[1])
                        .collect(),
                    1,
                )
            }
        }
        b'F' | b'H' => {
            let n = if initialized {
                super::constants::fixed_value(&literal, scale, exponent)?
            } else {
                0
            };
            let width = if doubleword {
                8
            } else if kind == b'H' {
                2
            } else {
                4
            };
            if explicit.is_none()
                && ((width == 2 && i16::try_from(n).is_err())
                    || (width == 4 && i32::try_from(n).is_err()))
            {
                return Err("fixed-point constant exceeds its implicit length".into());
            }
            let encoded = n.to_be_bytes();
            let bytes = if explicit.is_some() {
                encoded.to_vec()
            } else {
                encoded[8 - width..].to_vec()
            };
            (bytes, width as u32)
        }
        b'A' => {
            let n = if initialized {
                literal
                    .parse::<i32>()
                    .map_err(|_| "fullword constant must be an integer")?
            } else {
                0
            };
            (n.to_be_bytes().to_vec(), 4)
        }
        _ => return Err(format!("unsupported storage type {}", char::from(kind))),
    };
    let len = explicit.unwrap_or(if initialized {
        bytes.len() as u32
    } else {
        match kind {
            b'C' if unicode => 2,
            b'G' => 2,
            b'C' | b'X' | b'B' | b'P' | b'Z' => 1,
            b'H' => 2,
            b'F' if doubleword => 8,
            _ => 4,
        }
    });
    if len > 1 << 20 || count.checked_mul(len).is_none_or(|n| n > 1 << 20) {
        return Err("storage declaration exceeds 1 MiB".into());
    }
    if (unicode || graphic) && !len.is_multiple_of(2) {
        return Err("CU/G byte length must be a multiple of two".into());
    }
    if initialized && !character {
        let size = len as usize;
        if bytes.len() > size {
            bytes = bytes[bytes.len() - size..].to_vec();
        } else if bytes.len() < size {
            let fill = if kind == b'Z' {
                0xf0
            } else if matches!(kind, b'F' | b'H') && literal.starts_with('-') {
                0xff
            } else {
                0
            };
            let mut padded = vec![fill; size - bytes.len()];
            padded.extend(bytes);
            bytes = padded;
        }
    } else if initialized && character {
        if kind == b'C'
            && matches!(character_extension, None | Some('E'))
            && bytes.len() > len as usize
        {
            let cp = crate::codepage::active();
            let truncated = &bytes[..len as usize];
            if cp.is_mixed() && cp.encode(&cp.decode(truncated)) != truncated {
                return Err("truncation into DBCS character data is not permitted".into());
            }
        }
        bytes.truncate(len as usize);
        let padding: &[u8] = if unicode {
            &[0, 0x20]
        } else if graphic {
            &crate::codepage::DBCS_SPACE
        } else if character_extension == Some('A') {
            &[0x20]
        } else {
            &[0x40]
        };
        while bytes.len() < len as usize {
            bytes.extend_from_slice(padding);
        }
    } else {
        bytes.resize(
            len as usize,
            if initialized && kind == b'C' { 0x40 } else { 0 },
        );
        bytes.truncate(len as usize);
    }
    Ok((
        bytes.repeat(count as usize),
        if explicit.is_some() { 1 } else { align },
        len,
    ))
}

fn register(text: &str, symbols: &Symbols) -> Result<u8, String> {
    let upper = text.to_ascii_uppercase();
    let text = upper.as_str();
    let n = text
        .strip_prefix('R')
        .and_then(|s| s.parse().ok())
        .or_else(|| text.parse().ok())
        .or_else(|| {
            symbols
                .get(text)
                .filter(|s| s.section.is_none() && s.complex.is_empty())
                .map(|s| s.value)
        });
    n.filter(|&n| (0..16).contains(&n))
        .map(|n| n as u8)
        .ok_or_else(|| format!("invalid register {text}"))
}

#[derive(Clone)]
struct Using {
    origin: Symbol,
    register: u8,
    label: String,
    root: usize,
    dependent: bool,
    lower: Option<i64>,
    upper: Option<i64>,
}

fn drop_using(using: &mut Vec<Using>, predicate: impl Fn(&Using) -> bool) {
    let roots: HashSet<_> = using
        .iter()
        .filter(|u| !u.dependent && predicate(u))
        .map(|u| u.root)
        .collect();
    using.retain(|u| !predicate(u) && !roots.contains(&u.root));
}

fn qualified_expression(text: &str) -> Result<(String, String), String> {
    let mut output = String::new();
    let mut qualifier = String::new();
    let mut at = 0;
    while at < text.len() {
        let start = at;
        if text.as_bytes()[at].is_ascii_alphabetic() || b"_@$#".contains(&text.as_bytes()[at]) {
            while text
                .as_bytes()
                .get(at)
                .is_some_and(|b| b.is_ascii_alphanumeric() || b"_@$#".contains(b))
            {
                at += 1;
            }
            if text.as_bytes().get(at) == Some(&b'.') {
                let name = text[start..at].to_ascii_uppercase();
                if !qualifier.is_empty() && qualifier != name {
                    return Err("mixed USING qualifiers in one address".into());
                }
                qualifier = name;
                at += 1;
            } else {
                output.push_str(&text[start..at]);
            }
        } else {
            let c = text[at..].chars().next().unwrap();
            output.push(c);
            at += c.len_utf8();
            if c == '\'' {
                // Preserve self-defining terms, including dots in C constants.
                while at < text.len() {
                    let c = text[at..].chars().next().unwrap();
                    output.push(c);
                    at += c.len_utf8();
                    if c == '\'' {
                        break;
                    }
                }
            }
        }
    }
    Ok((output, qualifier))
}

fn address(
    text: &str,
    s: &Statement,
    symbols: &Symbols,
    using: &[Using],
    ss_length: bool,
) -> Result<(Address, u32), String> {
    let suffix_at = text.rfind('(').filter(|&at| {
        at > 0 && !text[..at].ends_with(['+', '-', '*', '/']) && !symbols.contains_key(text)
    });
    let (head, sub) = if let Some(at) = suffix_at.filter(|_| text.ends_with(')')) {
        (&text[..at], operands(&text[at + 1..text.len() - 1]))
    } else {
        (text, Vec::new())
    };
    let displacement_range = if super::operation::masked_bytes(&s.op).is_some_and(|spec| spec.long)
        || canonical_operation(&s.op) != s.op
        || wide_memory(&s.op)
        || super::operation::interlocked(&s.op).is_some_and(|(size, _)| size == 6)
        || super::operation::conditional(&s.op).is_some()
        || matches!(
            s.op.as_str(),
            "SLLG" | "SRLG" | "SLAG" | "SRAG" | "RLL" | "RLLG"
        ) {
        -524288..=524287
    } else {
        0..=4095
    };
    let (expression_text, qualifier) = qualified_expression(head)?;
    let mut value = expression(&expression_text, symbols, &s.section, s.offset)?;
    let mut len = value.length;
    let (mut base, mut index) = (0, 0);
    if ss_length {
        if let Some(n) = sub.first().filter(|n| !n.is_empty()) {
            len = u32::try_from(expression(n, symbols, &s.section, s.offset)?.value)
                .map_err(|_| "invalid length")?;
        }
        if let Some(b) = sub.get(1) {
            base = register(b, symbols)?;
        }
    } else if sub.len() == 1 {
        base = register(&sub[0], symbols)?;
    } else if sub.len() == 2 {
        if !sub[0].is_empty() {
            index = register(&sub[0], symbols)?;
        }
        if !sub[1].is_empty() {
            base = register(&sub[1], symbols)?;
        }
    } else if !sub.is_empty() {
        return Err("invalid address subfields".into());
    }
    if value.section.is_some()
        || !qualifier.is_empty()
        || !displacement_range.contains(&value.value)
    {
        let binding = using
            .iter()
            .rev()
            .filter(|u| {
                u.origin.section == value.section
                    && u.label == qualifier
                    && (base == 0 || base == u.register)
                    && u.lower.is_none_or(|lower| value.value >= lower)
                    && u.upper.is_none_or(|upper| value.value < upper)
            })
            .filter(|u| displacement_range.contains(&(value.value - u.origin.value)))
            .min_by_key(|u| {
                (
                    value.value < u.origin.value,
                    (value.value - u.origin.value).unsigned_abs(),
                    std::cmp::Reverse(u.register),
                )
            })
            .ok_or_else(|| {
                if value.section.is_none() && qualifier.is_empty() {
                    format!("displacement out of range: {}", value.value)
                } else {
                    format!("no USING for {head}")
                }
            })?;
        base = binding.register;
        value.value -= binding.origin.value;
    }
    if !displacement_range.contains(&value.value) {
        return Err(format!("displacement out of range: {}", value.value));
    }
    Ok((
        Address {
            displacement: value.value,
            base,
            index,
        },
        len,
    ))
}

pub fn compile_file(path: &Path, dirs: &[PathBuf]) -> Result<Module, String> {
    let report = super::compile_report(path, dirs);
    report.module.ok_or_else(|| {
        report
            .diagnostics
            .iter()
            .filter(|d| d.severity >= 8)
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    })
}

pub(super) fn compile_statements(lines: Vec<Statement>) -> Result<Module, String> {
    let mut phases = HashMap::new();
    for _ in 0..lines.len() + 3 {
        match compile_layout(lines.clone(), &phases)? {
            LayoutPass::Complete(module) => return Ok(*module),
            LayoutPass::Retry(next) => phases = next,
        }
    }
    Err("location-counter alignment did not converge".into())
}

enum LayoutPass {
    Complete(Box<Module>),
    Retry(HashMap<String, u32>),
}

// A counter begins at its final origin modulo the largest supported
// alignment (4096). These phase bytes are temporary prefix padding and are
// stripped when counters are concatenated. Repeat until preceding counter
// lengths no longer change later phases; never reorder source statements.
fn compile_layout(
    mut lines: Vec<Statement>,
    phases: &HashMap<String, u32>,
) -> Result<LayoutPass, String> {
    if let Some(end) = lines.iter().position(|s| s.op == "END") {
        lines.truncate(end + 1);
    }
    let mut symbols = Symbols::new();
    let mut externals = HashMap::new();
    for s in lines.iter().take_while(|s| s.op != "END") {
        if matches!(s.op.as_str(), "EXTRN" | "WXTRN") {
            for name in operands(&s.args) {
                let name = name.to_ascii_uppercase();
                if !symbol_name(&name) {
                    return Err(s.error("invalid external symbol"));
                }
                let weak = s.op == "WXTRN";
                if externals
                    .insert(name.clone(), weak)
                    .is_some_and(|old| old != weak)
                {
                    return Err(s.error(format!("conflicting external declarations for {name}")));
                }
            }
        }
    }
    let defined: HashSet<_> = lines
        .iter()
        .filter(|s| !s.label.is_empty())
        .map(|s| s.label.clone())
        .collect();
    for s in lines.iter().take_while(|s| s.op != "END") {
        for operand in operands(&s.args) {
            let spec = if s.op == "DC" {
                Some(operand.as_str())
            } else {
                operand.strip_prefix('=')
            };
            if let Some(spec) = spec
                && let Some(values) = external_address_names(spec).ok().flatten()
            {
                for name in values {
                    let name = name.to_ascii_uppercase();
                    if !symbol_name(&name) {
                        return Err(s.error("V constant requires an external symbol name"));
                    }
                    if !defined.contains(name.as_str()) {
                        externals.entry(name).or_insert(false);
                    }
                }
            }
        }
    }
    for name in externals.keys() {
        symbols.insert(
            name.clone(),
            Symbol {
                section: Some(format!("EXTERN:{name}")),
                value: 0,
                length: 1,
                type_attribute: crate::ebcdic::encode(if externals[name] { "$" } else { "T" })[0],
                numeric_attributes: None,
                program_type: None,
                assembler_type: None,
                complex: BTreeMap::new(),
            },
        );
    }
    // Resolve chains of forward absolute EQU definitions before layout.
    for _ in 0..lines.len() {
        let mut changed = false;
        for s in lines.iter().filter(|s| s.op == "EQU") {
            if !symbols.contains_key(&s.label)
                && let Ok(mut value) = expression(&s.args, &symbols, "", 0)
                && value.section.is_none()
            {
                value.numeric_attributes = Some((0, 0));
                symbols.insert(s.label.clone(), value);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let (mut section, mut name) = (String::new(), String::new());
    let mut current_control = String::new();
    let mut control_areas: Vec<(String, Vec<String>)> = Vec::new();
    let mut area_images: HashMap<String, Vec<u8>> = HashMap::new();
    let mut area_ranges: HashMap<String, Vec<std::ops::Range<u32>>> = HashMap::new();
    let mut layout_labels: HashMap<String, String> = HashMap::new();
    let mut section_types = HashMap::new();
    let mut rmode = 31;
    let mut last_areas: HashMap<String, String> = HashMap::new();
    let mut counters: HashMap<String, u32> = HashMap::new();
    let mut high_water: HashMap<String, u32> = HashMap::new();
    let mut instruction_ranges: Vec<std::ops::Range<u32>> = Vec::new();
    let mut cnop_ranges = HashMap::new();
    let has_cnop = lines.iter().any(|s| s.op == "CNOP");
    let mut image_alignment = 8;
    let mut image = Vec::new();
    let mut literals = LiteralPool::default();
    let mut literal_fixups = Vec::new();
    let mut bindings = super::Bindings::default();
    let mut area_alignment = HashMap::<String, u32>::new();
    let mut class_attributes = HashMap::<String, super::classes::Attributes>::new();
    let mut class_controls = HashMap::<String, String>::new();
    let mut part_attributes = HashMap::<String, super::classes::Attributes>::new();
    let mut parent_control = String::new();
    let mut entry = 0;
    let mut attribute_symbols = Symbols::new();
    let mut pending_attribute_equ = Vec::new();
    let mut equ_attributes = HashMap::new();
    for (statement_index, s) in lines.iter_mut().enumerate() {
        if s.op == "END" {
            break;
        }
        if !current_control.is_empty() {
            last_areas.insert(current_control.clone(), section.clone());
        }
        if s.op == "XATTR" {
            continue;
        }
        if s.op == "DXD" {
            if !symbol_name(&s.label) || symbols.contains_key(&s.label) {
                return Err(s.error("DXD requires a unique symbol"));
            }
            let (bytes, alignment, _) =
                storage(&s.args, false, &symbols).map_err(|e| s.error(e))?;
            bindings.sections.push(super::BindingSection {
                name: s.label.clone(),
                length: bytes.len() as u32,
                alignment,
                dummy: true,
                class: None,
                concatenate: false,
                priority: 0,
                image: None,
            });
            symbols.insert(
                s.label.clone(),
                Symbol {
                    section: Some(s.label.clone()),
                    value: 0,
                    length: 1,
                    type_attribute: crate::ebcdic::encode("J")[0],
                    numeric_attributes: None,
                    program_type: None,
                    assembler_type: None,
                    complex: BTreeMap::new(),
                },
            );
            continue;
        }
        if s.op == "AMODE"
            && !matches!(s.args.to_ascii_uppercase().as_str(), "31" | "ANY" | "ANY31")
        {
            return Err(s.error(
                "the Zeus caller uses AMODE 31; supported declarations are 31, ANY and ANY31",
            ));
        }
        if s.op == "RMODE" {
            match s.args.to_ascii_uppercase().as_str() {
                "ANY" => {}
                "24" => rmode = 24,
                _ => return Err(s.error("supported RMODE declarations are 24 and ANY")),
            }
        }
        if matches!(s.op.as_str(), "CSECT" | "RSECT" | "START") {
            if s.label.is_empty() {
                return Err(s.error("control section name required"));
            }
            if s.op == "START" && !s.args.is_empty() && s.args != "0" {
                return Err(s.error("only START 0 is supported"));
            }
            if name.is_empty() {
                name = s.label.clone();
            }
            current_control = format!("$C:{}", s.label);
            parent_control = current_control.clone();
            let kind = if s.op == "START" {
                "CSECT"
            } else {
                s.op.as_str()
            };
            if let Some(old) = section_types.insert(current_control.clone(), kind.to_string())
                && old != kind
            {
                return Err(s.error("control section cannot reopen with a different section type"));
            }
            let area = format!("{current_control}:");
            if !control_areas
                .iter()
                .any(|(control, _)| control == &current_control)
            {
                control_areas.push((current_control.clone(), vec![area.clone()]));
            }
            area_images.insert(section.clone(), std::mem::take(&mut image));
            area_ranges.insert(section.clone(), std::mem::take(&mut instruction_ranges));
            section = last_areas.get(&current_control).cloned().unwrap_or(area);
            image = area_images.remove(&section).unwrap_or_default();
            instruction_ranges = area_ranges.remove(&section).unwrap_or_default();
        } else if s.op == "CATTR" {
            if parent_control.is_empty()
                || (!current_control.starts_with("$C:")
                    && !class_controls.contains_key(&current_control))
            {
                return Err(s.error("CATTR requires an executable control section"));
            }
            if !symbol_name(&s.label) || s.label.len() > 16 {
                return Err(s.error("CATTR class name must be a symbol of 1..16 characters"));
            }
            let attributes = if let Some(first) = class_attributes.get(&s.label) {
                if first.part.is_none() {
                    if !s.args.is_empty() {
                        return Err(s.error("CATTR continuation must not repeat class attributes"));
                    }
                    first.clone()
                } else {
                    let mut attrs = first.clone();
                    attrs.part = None;
                    attrs.priority = 0;
                    attrs.fill = 0;
                    let parsed = super::classes::parse(&format!("NOLOAD,{}", s.args), &symbols)
                        .map_err(|e| s.error(e))?;
                    attrs.part = Some(
                        parsed
                            .part
                            .ok_or_else(|| s.error("CATTR continuation requires PART"))?,
                    );
                    attrs.priority = parsed.priority;
                    attrs.fill = parsed.fill;
                    attrs
                }
            } else {
                let parsed = super::classes::parse(&s.args, &symbols).map_err(|e| s.error(e))?;
                class_attributes.insert(s.label.clone(), parsed.clone());
                parsed
            };
            image_alignment = image_alignment.max(attributes.alignment);
            rmode = rmode.min(attributes.rmode);
            area_images.insert(section.clone(), std::mem::take(&mut image));
            area_ranges.insert(section.clone(), std::mem::take(&mut instruction_ranges));
            current_control = format!(
                "{}{}:{}",
                if attributes.no_load { "$D:" } else { "$C:" },
                &parent_control[3..],
                format_args!("@{}:@{}", s.label, attributes.part.as_deref().unwrap_or(""))
            );
            class_controls.insert(current_control.clone(), s.label.clone());
            part_attributes
                .entry(current_control.clone())
                .or_insert_with(|| attributes.clone());
            section_types.insert(
                current_control.clone(),
                if attributes.read_only {
                    "RSECT"
                } else {
                    "CSECT"
                }
                .into(),
            );
            section = format!("{current_control}:");
            if !control_areas
                .iter()
                .any(|(control, _)| control == &current_control)
            {
                control_areas.push((current_control.clone(), vec![section.clone()]));
            }
            section = last_areas.get(&current_control).cloned().unwrap_or(section);
            image = area_images.remove(&section).unwrap_or_default();
            instruction_ranges = area_ranges.remove(&section).unwrap_or_default();
        } else if s.op == "DSECT" {
            area_images.insert(section.clone(), std::mem::take(&mut image));
            area_ranges.insert(section.clone(), std::mem::take(&mut instruction_ranges));
            if s.label.is_empty() {
                return Err(s.error("DSECT name required"));
            }
            current_control = format!("$D:{}", s.label);
            section = format!("{current_control}:");
            if !control_areas
                .iter()
                .any(|(control, _)| control == &current_control)
            {
                control_areas.push((current_control.clone(), vec![section.clone()]));
            }
            section = last_areas.get(&current_control).cloned().unwrap_or(section);
        } else if s.op == "LOCTR" {
            let known_area = layout_labels.get(&s.label).cloned();
            if let Some(area) = &known_area {
                current_control = area.rsplit_once(':').unwrap().0.into();
            }
            let (_, areas) = control_areas
                .iter_mut()
                .find(|(control, _)| control == &current_control)
                .ok_or_else(|| s.error("LOCTR requires an active section"))?;
            let area = known_area.unwrap_or_else(|| format!("{current_control}:{}", s.label));
            if !areas.contains(&area) {
                areas.push(area.clone());
            }
            area_images.insert(section.clone(), std::mem::take(&mut image));
            area_ranges.insert(section.clone(), std::mem::take(&mut instruction_ranges));
            section = area;
            image = area_images.remove(&section).unwrap_or_default();
            instruction_ranges = area_ranges.remove(&section).unwrap_or_default();
        }
        let layout_statement = matches!(
            s.op.as_str(),
            "CSECT" | "RSECT" | "START" | "DSECT" | "LOCTR"
        );
        let reopening =
            (layout_statement && !s.label.is_empty() && layout_labels.contains_key(&s.label))
                || (s.op == "CATTR" && symbols.contains_key(&s.label));
        if layout_statement && !s.label.is_empty() {
            let label_area = if s.op == "LOCTR" {
                section.clone()
            } else {
                format!("{current_control}:")
            };
            if let Some(previous) = layout_labels.insert(s.label.clone(), label_area.clone()) {
                if previous != label_area {
                    return Err(s.error(
                        "section or location-counter name already belongs to another area",
                    ));
                }
            } else if symbols.contains_key(&s.label) {
                return Err(s.error(format!("duplicate symbol {}", s.label)));
            }
        }
        let executable = section.starts_with("$C:");
        let offset = counters
            .entry(section.clone())
            .or_insert_with(|| phases.get(&section).copied().unwrap_or(0));
        let data = if s.op == "CXD" {
            if !s.args.is_empty() {
                return Err(s.error("CXD takes no operands"));
            }
            Some((vec![0; 4], 4, 4))
        } else if s.op == "DCB" {
            Some((
                vec![0; super::data_management::LENGTH],
                4,
                super::data_management::LENGTH as u32,
            ))
        } else if matches!(s.op.as_str(), "DS" | "DC") {
            Some(
                storage_with_origin(&s.args, s.op == "DC", &symbols, *offset)
                    .map_err(|e| s.error(e))?,
            )
        } else {
            None
        };
        if matches!(s.op.as_str(), "DC" | "DS") {
            super::constants::annotations(&s.args, &symbols).map_err(|e| s.error(e))?;
        }
        if let Some((_, align, _)) = &data {
            area_alignment
                .entry(current_control.clone())
                .and_modify(|a| *a = (*a).max(*align))
                .or_insert(*align);
            *offset = offset.next_multiple_of(*align);
        }
        if instruction_size(&s.op).is_some() || s.op == "CNOP" {
            *offset = offset.next_multiple_of(2);
        }
        if s.op == "LTORG" {
            *offset = offset.next_multiple_of(8);
        }
        s.offset = *offset;
        s.section = section.clone();
        if matches!(s.op.as_str(), "EXTRN" | "WXTRN") {
            for name in operands(&s.args) {
                if let Some(value) = symbols.get(&name.to_ascii_uppercase()) {
                    attribute_symbols.insert(name.to_ascii_uppercase(), value.clone());
                }
            }
            resolve_attribute_equ(&mut attribute_symbols, &mut pending_attribute_equ);
        }
        if s.op == "EQU" {
            if s.label.is_empty() {
                return Err(s.error("EQU symbol required"));
            }
            let attributes =
                super::expression::equ_attributes(&s.args, &attribute_symbols, &section, *offset)
                    .map_err(|e| s.error(e))?;
            equ_attributes.insert(s.label.clone(), attributes.clone());
            pending_attribute_equ.push((s.clone(), attributes));
            resolve_attribute_equ(&mut attribute_symbols, &mut pending_attribute_equ);
        }
        if !s.label.is_empty()
            && !matches!(s.op.as_str(), "USING" | "AMODE" | "RMODE")
            && !reopening
        {
            let value = if s.op == "EQU" {
                let Ok(mut value) =
                    super::expression::evaluate_equ(&s.args, &symbols, &section, *offset)
                else {
                    continue;
                };
                equ_attributes[&s.label].apply(&mut value);
                value
            } else {
                Symbol {
                    section: Some(section.clone()),
                    value: i64::from(*offset),
                    length: data
                        .as_ref()
                        .map_or_else(|| instruction_size(&s.op).unwrap_or(1), |d| d.2),
                    type_attribute: if s.op == "CXD" {
                        crate::ebcdic::encode("A")[0]
                    } else if s.op == "DCB" {
                        crate::ebcdic::encode("C")[0]
                    } else if data.is_some() {
                        storage_type(&s.args)
                    } else {
                        crate::ebcdic::encode(if layout_statement {
                            "J"
                        } else if instruction_size(&s.op).is_some() {
                            "I"
                        } else {
                            "U"
                        })[0]
                    },
                    numeric_attributes: data
                        .as_ref()
                        .and_then(|d| storage_numeric_attributes(&s.args, d.2, &symbols).ok()),
                    program_type: if matches!(s.op.as_str(), "DC" | "DS") {
                        super::constants::annotations(&s.args, &symbols)
                            .map_err(|e| s.error(e))?
                            .0
                    } else {
                        None
                    },
                    assembler_type: if matches!(s.op.as_str(), "DC" | "DS") {
                        Some(
                            super::constants::annotations(&s.args, &symbols)
                                .map_err(|e| s.error(e))?
                                .1,
                        )
                    } else {
                        None
                    },
                    complex: BTreeMap::new(),
                }
            };
            if s.op != "EQU" {
                attribute_symbols.insert(s.label.clone(), value.clone());
                resolve_attribute_equ(&mut attribute_symbols, &mut pending_attribute_equ);
            }
            if let Some(old) = symbols.insert(s.label.clone(), value.clone())
                && (old.section != value.section
                    || old.value != value.value
                    || old.length != value.length
                    || old.type_attribute != value.type_attribute
                    || old.numeric_attributes != value.numeric_attributes
                    || old.program_type != value.program_type
                    || old.assembler_type != value.assembler_type
                    || old.complex != value.complex)
            {
                return Err(s.error(format!("duplicate symbol {}", s.label)));
            }
        }
        if let Some((bytes, _, _)) = data {
            *offset = offset
                .checked_add(bytes.len() as u32)
                .ok_or_else(|| s.error("section too large"))?;
            if executable {
                if *offset > 16 << 20 {
                    return Err(s.error("control section exceeds 16 MiB"));
                }
                image.resize(image.len().max(*offset as usize), 0);
                if matches!(s.op.as_str(), "DC" | "DCB" | "CXD") {
                    if instruction_ranges
                        .iter()
                        .any(|r| s.offset < r.end && r.start < *offset)
                    {
                        return Err(s.error("overwriting instructions with DC is not supported"));
                    }
                    image[s.offset as usize..*offset as usize].copy_from_slice(&bytes);
                }
            }
        } else if s.op == "CNOP" {
            let args = operands(&s.args);
            if args.len() != 2 {
                return Err(s.error("CNOP byte,boundary required"));
            }
            let byte =
                expression(&args[0], &symbols, &section, s.offset).map_err(|e| s.error(e))?;
            let boundary =
                expression(&args[1], &symbols, &section, s.offset).map_err(|e| s.error(e))?;
            if byte.section.is_some()
                || boundary.section.is_some()
                || !(4..=4096).contains(&boundary.value)
                || !(boundary.value as u32).is_power_of_two()
                || byte.value < 0
                || byte.value > boundary.value - 2
                || byte.value % 2 != 0
            {
                return Err(s.error("CNOP requires an even byte in 0..boundary-2 and power-of-two boundary in 4..4096"));
            }
            if !executable {
                return Err(s.error("CNOP instructions in DSECT are not supported"));
            }
            let boundary = boundary.value as u32;
            image_alignment = image_alignment.max(boundary);
            let mut target = *offset / boundary * boundary + byte.value as u32;
            if target < *offset {
                target += boundary;
            }
            if target > 16 << 20 {
                return Err(s.error("CNOP exceeds 16 MiB"));
            }
            if instruction_ranges
                .iter()
                .any(|r| *offset < r.end && r.start < target)
            {
                return Err(s.error("CNOP overlaps existing instructions"));
            }
            if target > *offset {
                instruction_ranges.push(*offset..target);
            }
            cnop_ranges.insert(statement_index, target);
            image.resize(image.len().max(target as usize), 0);
            for at in (*offset..target).step_by(2) {
                image[at as usize..at as usize + 2].copy_from_slice(&[7, 0]);
            }
            *offset = target;
        } else if let Some(size) = instruction_size(&s.op) {
            if class_controls
                .get(&current_control)
                .is_some_and(|name| !class_attributes[name].executable)
            {
                return Err(s.error("instruction inside NOTEXECUTABLE class"));
            }
            if !executable {
                return Err(s.error("instruction inside DSECT"));
            }
            *offset += size;
            if *offset > 16 << 20 {
                return Err(s.error("control section exceeds 16 MiB"));
            }
            if instruction_ranges
                .iter()
                .any(|r| s.offset < r.end && r.start < *offset)
            {
                return Err(s.error("overlapping instructions are not supported"));
            }
            instruction_ranges.push(s.offset..*offset);
            image.resize(image.len().max(*offset as usize), 0);
            let mut args = operands(&s.args);
            for operand in &mut args {
                if let Some(spec) = operand.strip_prefix('=') {
                    *operand = literals
                        .add(spec, s.offset, &section, &defined, &attribute_symbols)
                        .map_err(|e| s.error(e))?;
                }
            }
            s.args = args.join(",");
        } else if s.op == "LTORG" {
            if !executable {
                return Err(s.error("literal pools in DSECT are not supported"));
            }
            literals.flush(
                offset,
                &mut image,
                &mut symbols,
                &mut literal_fixups,
                &section,
                statement_index,
            )?;
            if *offset > s.offset
                && instruction_ranges
                    .iter()
                    .any(|r| s.offset < r.end && r.start < *offset)
            {
                return Err(s.error("literal pool overlaps existing instructions"));
            }
        } else if s.op == "ORG" {
            let args = operands(&s.args);
            if args.len() > 3 {
                return Err(s.error("ORG accepts expression,boundary,offset"));
            }
            let mut target = if args[0].is_empty() {
                i64::from(high_water.get(&section).copied().unwrap_or(*offset))
            } else {
                let value =
                    expression(&args[0], &symbols, &section, *offset).map_err(|e| s.error(e))?;
                if value.section.as_ref() != Some(&section) {
                    return Err(
                        s.error("ORG requires a relocatable expression in the current section")
                    );
                }
                value.value
            };
            if let Some(boundary) = args.get(1).filter(|a| !a.is_empty()) {
                let b =
                    expression(boundary, &symbols, &section, *offset).map_err(|e| s.error(e))?;
                if b.section.is_some()
                    || !(2..=4096).contains(&b.value)
                    || !(b.value as u32).is_power_of_two()
                {
                    return Err(
                        s.error("ORG boundary must be an absolute power of two from 2 to 4096")
                    );
                }
                target = target
                    .checked_add(b.value - 1)
                    .map(|n| n & !(b.value - 1))
                    .ok_or_else(|| s.error("ORG overflow"))?;
            }
            if let Some(adjust) = args.get(2).filter(|a| !a.is_empty()) {
                let n = expression(adjust, &symbols, &section, *offset).map_err(|e| s.error(e))?;
                if n.section.is_some() {
                    return Err(s.error("ORG offset must be absolute"));
                }
                target = target
                    .checked_add(n.value)
                    .ok_or_else(|| s.error("ORG overflow"))?;
            }
            if !(i64::from(phases.get(&section).copied().unwrap_or(0))..=16 << 20).contains(&target)
            {
                return Err(s.error("ORG outside current section or exceeds 16 MiB"));
            }
            *offset = target as u32;
        }
        let high = high_water.entry(section.clone()).or_default();
        *high = (*high).max(*offset);
        if image.len() > 16 << 20 {
            return Err(s.error("control section exceeds 16 MiB"));
        }
    }
    if name.is_empty() {
        return Err("CSECT or START required".into());
    }
    for s in lines
        .iter()
        .filter(|s| matches!(s.op.as_str(), "AMODE" | "RMODE") && !s.label.is_empty())
    {
        if !section_types.contains_key(&format!("$C:{}", s.label)) {
            return Err(s.error("AMODE/RMODE name must identify an executable control section"));
        }
    }
    // Each section/location counter is laid out independently. Concatenate
    // counters in first-use order only after their final lengths are known.
    // Source order remains unchanged for USING and attribute evaluation.
    {
        area_images.insert(section.clone(), std::mem::take(&mut image));
        let first_control = &control_areas
            .iter()
            .find(|(_, areas)| areas[0].starts_with("$C:"))
            .unwrap()
            .0;
        section = if &current_control == first_control {
            section
        } else {
            last_areas
                .get(first_control)
                .cloned()
                .unwrap_or_else(|| format!("{first_control}:"))
        };
        image = area_images.remove(&section).unwrap_or_default();
    }
    let offset = counters.entry(section.clone()).or_default();
    *offset = (*offset).max(high_water.get(&section).copied().unwrap_or(0));
    literals.flush(
        offset,
        &mut image,
        &mut symbols,
        &mut literal_fixups,
        &section,
        usize::MAX,
    )?;
    area_images.insert(section, image);
    let mut image = Vec::new();
    let mut area_bases = HashMap::new();
    let mut read_only = Vec::new();
    let mut next_phases = HashMap::new();
    let mut control_origins = HashMap::<String, u32>::new();
    let symbol_controls: HashMap<_, _> = symbols
        .iter()
        .filter_map(|(name, value)| {
            let area = value.section.as_deref()?;
            let control = area.rsplit_once(':')?.0;
            control
                .starts_with("$C:")
                .then(|| (name.clone(), control.to_string()))
        })
        .collect();
    let symbol_classes: HashMap<_, _> = symbols
        .iter()
        .filter_map(|(name, value)| {
            let area = value.section.as_deref()?;
            let control = area.rsplit_once(':')?.0;
            class_controls
                .contains_key(control)
                .then(|| (name.clone(), control.to_string()))
        })
        .collect();
    for (control, areas) in &control_areas {
        let mut dummy_offset = 0u32;
        let section_start = image.len() as u32;
        for (area_index, area) in areas.iter().enumerate() {
            let executable = area.starts_with("$C:");
            let base = if executable {
                image.len() as u32
            } else {
                dummy_offset
            }
            .next_multiple_of(if area_index == 0 {
                if executable && has_cnop {
                    4096
                } else if let Some(name) = class_controls.get(control) {
                    class_attributes[name].alignment
                } else {
                    8
                }
            } else {
                1
            });
            if area_index == 0 {
                control_origins.insert(control.clone(), base);
            }
            let phase = phases.get(area).copied().unwrap_or(0);
            next_phases.insert(area.clone(), base % 4096);
            let mut bytes = area_images.remove(area).unwrap_or_default();
            let extent = bytes
                .len()
                .max(high_water.get(area).copied().unwrap_or(phase) as usize)
                .saturating_sub(phase as usize);
            if base as usize + extent > 16 << 20 {
                return Err("combined control sections exceed 16 MiB".into());
            }
            if executable {
                bytes.resize(extent + phase as usize, 0);
                image.resize(base as usize, 0);
                image.extend(&bytes[phase as usize..]);
                if section_types
                    .get(control)
                    .is_some_and(|kind| kind == "RSECT")
                    && extent != 0
                {
                    read_only.push(base..base + extent as u32);
                }
            } else {
                dummy_offset = base + extent as u32;
            }
            area_bases.insert(
                area.clone(),
                (
                    base.saturating_sub(phase),
                    if executable {
                        String::new()
                    } else {
                        control.strip_prefix("$D:").unwrap().into()
                    },
                ),
            );
        }
        if control.starts_with("$C:") && !class_controls.contains_key(control) {
            continue;
        }
        bindings.sections.push(super::BindingSection {
            name: part_attributes
                .get(control)
                .and_then(|a| a.part.clone())
                .or_else(|| class_controls.get(control).cloned())
                .unwrap_or_else(|| control[3..].into()),
            length: if control.starts_with("$D:") {
                dummy_offset
            } else {
                (image.len() as u32).saturating_sub(section_start.next_multiple_of(if has_cnop {
                    4096
                } else if let Some(name) = class_controls.get(control) {
                    class_attributes[name].alignment
                } else {
                    8
                }))
            },
            alignment: area_alignment
                .get(control)
                .copied()
                .unwrap_or(1)
                .max(part_attributes.get(control).map_or(1, |a| a.alignment)),
            dummy: part_attributes
                .get(control)
                .is_some_and(|a| a.part.is_some()),
            class: part_attributes
                .get(control)
                .filter(|a| a.part.is_some())
                .map(|_| class_controls[control].clone()),
            concatenate: part_attributes
                .get(control)
                .is_some_and(|a| a.part.is_none()),
            priority: part_attributes.get(control).map_or(0, |a| a.priority),
            image: (class_controls.contains_key(control) && control.starts_with("$C:"))
                .then(|| control_origins[control]),
        });
    }
    if next_phases
        .iter()
        .any(|(area, phase)| phases.get(area).copied().unwrap_or(0) != *phase)
    {
        return Ok(LayoutPass::Retry(next_phases));
    }
    for value in symbols.values_mut() {
        let mut relocations = std::mem::take(&mut value.complex);
        if relocations.is_empty()
            && let Some(section) = value.section.take()
        {
            relocations.insert(section, 1);
        }
        let mut flattened = BTreeMap::new();
        for (section, coefficient) in relocations {
            let section = if let Some((base, target_section)) = area_bases.get(&section) {
                value.value = value
                    .value
                    .checked_add(
                        i64::from(*base)
                            .checked_mul(coefficient)
                            .ok_or("layout expression overflow")?,
                    )
                    .ok_or("layout expression overflow")?;
                target_section.clone()
            } else {
                section
            };
            *flattened.entry(section).or_default() += coefficient;
        }
        flattened.retain(|_, coefficient| *coefficient != 0);
        value.section = None;
        if flattened.len() == 1 && flattened.values().next() == Some(&1) {
            value.section = flattened.into_keys().next();
        } else {
            value.complex = flattened;
        }
    }
    for (index, s) in lines.iter_mut().enumerate() {
        if let Some((base, target_section)) = area_bases.get(&s.section) {
            s.offset += base;
            if let Some(end) = cnop_ranges.get_mut(&index) {
                *end += base;
            }
            s.section = target_section.clone();
        }
    }
    for literal in &mut literal_fixups {
        literal.offset += area_bases[&literal.section].0;
        literal.location += area_bases[&literal.location_section].0;
    }
    for _ in 0..lines.len() {
        let mut changed = false;
        for s in lines.iter().filter(|s| s.op == "EQU") {
            if !symbols.contains_key(&s.label)
                && let Ok(mut value) =
                    super::expression::evaluate_equ(&s.args, &symbols, &s.section, s.offset)
            {
                equ_attributes[&s.label].apply(&mut value);
                symbols.insert(s.label.clone(), value);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    if let Some(s) = lines
        .iter()
        .find(|s| s.op == "EQU" && !symbols.contains_key(&s.label))
    {
        let reason = super::expression::evaluate_equ(&s.args, &symbols, &s.section, s.offset)
            .err()
            .unwrap_or_else(|| "unresolved definition".into());
        return Err(s.error(format!("cannot resolve EQU {}: {reason}", s.label)));
    }
    // Literal pools and save areas are part of the mapped section, not host memory.
    image.resize(image.len() + 256, 0);
    let mut using = Vec::new();
    let mut using_serial = 0;
    let mut using_stack = Vec::new();
    let mut code = Vec::new();
    let mut dcbs = Vec::new();
    let mut relocations = Vec::new();
    for literal in &literal_fixups {
        section_quantities(&literal.spec, literal.offset, &symbols, &mut bindings)?;
        fix_address_constants(
            &literal.spec,
            literal.offset,
            Some(literal.location),
            &symbols,
            &externals,
            &mut image,
            &mut relocations,
        )?;
    }
    let mut entries = HashMap::new();
    entries.insert(name.clone(), entry);
    for (statement_index, s) in lines.iter().enumerate() {
        for literal in literal_fixups
            .iter()
            .filter(|l| l.pool == statement_index || (s.op == "END" && l.pool == usize::MAX))
        {
            fix_base_constants(
                &literal.spec,
                literal.offset,
                Some(literal.location),
                &symbols,
                &using,
                &mut image,
            )
            .map_err(|e| s.error(e))?;
        }
        if matches!(s.op.as_str(), "PUSH" | "POP") {
            if !operands(&s.args.to_ascii_uppercase())
                .iter()
                .any(|a| a == "USING")
            {
                continue;
            }
            if s.op == "PUSH" {
                using_stack.push(using.clone());
            } else {
                using = using_stack
                    .pop()
                    .ok_or_else(|| s.error("POP USING without PUSH USING"))?;
            }
            continue;
        }
        if s.op == "CNOP" {
            for offset in (s.offset..cnop_ranges[&statement_index]).step_by(2) {
                code.push(Instruction {
                    offset,
                    op: "BCR".into(),
                    storage_service: None,
                    module_service: None,
                    system_service: None,
                    data_service: None,
                    registers: vec![0],
                    address: None,
                    source: None,
                    length: 1,
                    source_length: 1,
                    immediate: 0,
                    relative_external: None,
                    relative_expression: None,
                    file: s.file.clone(),
                    line: s.line,
                });
            }
            continue;
        }
        if s.op == "END" {
            if !s.args.is_empty() {
                entry = u32::try_from(expression(&s.args, &symbols, "", 0)?.value)
                    .map_err(|_| s.error("invalid entry"))?;
            }
            break;
        }
        let args = operands(&s.args);
        if matches!(s.op.as_str(), "CSECT" | "RSECT" | "ENTRY") {
            for label in if s.op == "ENTRY" {
                args
            } else {
                vec![s.label.clone()]
            } {
                let label = label.to_ascii_uppercase();
                let value = symbols
                    .get(&label)
                    .ok_or_else(|| s.error(format!("undefined entry {label}")))?;
                if value.section.as_deref() != Some("") {
                    return Err(s.error("entry must be in executable storage"));
                }
                entries.insert(label, value.value as u32);
            }
            continue;
        }
        if s.op == "CATTR" {
            continue;
        }
        if s.op == "XATTR" {
            if !entries.contains_key(&s.label)
                && !lines
                    .iter()
                    .any(|l| l.op == "ENTRY" && operands(&l.args).contains(&s.label))
            {
                return Err(s.error("XATTR requires a defined section or ENTRY name"));
            }
            let mut target = None;
            for arg in &args {
                if let Some(name) = arg
                    .to_ascii_uppercase()
                    .strip_prefix("PSECT(")
                    .and_then(|v| v.strip_suffix(')'))
                {
                    let value = expression(name, &symbols, "", 0).map_err(|e| s.error(e))?;
                    if value
                        .section
                        .as_deref()
                        .is_some_and(|section| section.starts_with("EXTERN:"))
                    {
                        target = Some((0, None, Some(name.to_string())));
                    } else {
                        if value.section.as_deref() != Some("") {
                            return Err(s.error("PSECT must refer to mapped writable storage"));
                        }
                        let control = symbol_controls.get(name).ok_or_else(|| {
                            s.error("PSECT target must identify a mapped section or class")
                        })?;
                        target = Some((
                            control_origins[control],
                            class_controls.get(control).cloned(),
                            None,
                        ));
                    }
                } else if !matches!(
                    arg.to_ascii_uppercase().as_str(),
                    "LINKAGE(OS)" | "LINK(OS)"
                ) {
                    return Err(s.error(format!("unsupported XATTR attribute {arg}")));
                }
            }
            if let Some((offset, class, target)) = target {
                bindings.psects.push(super::Psect {
                    symbol: s.label.clone(),
                    offset,
                    class,
                    target,
                });
            }
            continue;
        }
        if s.op == "CXD" && s.section.is_empty() {
            bindings.quantities.push(super::Quantity {
                offset: s.offset,
                width: 4,
                kind: super::QuantityKind::Total,
                symbol: String::new(),
                element: None,
                addend: 0,
            });
            continue;
        }
        if s.op == "DCB" && s.section.is_empty() {
            dcbs.push(data_control_block(s, &symbols).map_err(|e| s.error(e))?);
            continue;
        }
        if s.op == "DC" && s.section.is_empty() {
            let mut position = s.offset as usize * 8;
            for item in operands(&s.args) {
                if let Some(field) =
                    super::bit_constants::assemble(&item, true, &symbols).map_err(|e| s.error(e))?
                {
                    position += field.bits;
                    continue;
                }
                let (bytes, align, _) =
                    storage_item(&item, true, &symbols).map_err(|e| s.error(e))?;
                let at = (position.div_ceil(8) as u32).next_multiple_of(align);
                if address_constants(&item, &symbols)
                    .map_err(|e| s.error(e))?
                    .is_some()
                {
                    section_quantities(&item, at, &symbols, &mut bindings)
                        .map_err(|e| s.error(e))?;
                    fix_base_constants(&item, at, None, &symbols, &using, &mut image)
                        .map_err(|e| s.error(e))?;
                    fix_address_constants(
                        &item,
                        at,
                        None,
                        &symbols,
                        &externals,
                        &mut image,
                        &mut relocations,
                    )
                    .map_err(|e| s.error(e))?;
                }
                position = (at as usize + bytes.len()) * 8;
            }
            continue;
        }
        if s.op == "USING" {
            if args.len() < 2 {
                return Err(s.error("USING origin,register required"));
            }
            let range = args[0]
                .strip_prefix('(')
                .and_then(|v| v.strip_suffix(')'))
                .map(operands)
                .unwrap_or_else(|| vec![args[0].clone()]);
            if !(1..=4).contains(&range.len()) {
                return Err(s.error("USING accepts base, (base,end), or (base,end,lower,upper)"));
            }
            let origin =
                expression(&range[0], &symbols, &s.section, s.offset).map_err(|e| s.error(e))?;
            let bound = |n: usize| -> Result<Option<i64>, String> {
                if let Some(text) = range.get(n).filter(|text| !text.is_empty()) {
                    let value = expression(text, &symbols, &s.section, s.offset)?;
                    if value.section != origin.section || !value.complex.is_empty() {
                        return Err("USING limit must have the base relocatability".into());
                    }
                    Ok(Some(value.value))
                } else {
                    Ok(None)
                }
            };
            let end = bound(1).map_err(|e| s.error(e))?;
            let lower = bound(2)
                .map_err(|e| s.error(e))?
                .or(end.map(|_| origin.value));
            let upper = bound(3).map_err(|e| s.error(e))?.or(end);
            if end.is_some_and(|end| end <= origin.value)
                || lower.zip(upper).is_some_and(|(low, high)| low >= high)
            {
                return Err(s.error("USING limits must form an increasing range"));
            }
            if !s.label.is_empty() {
                drop_using(&mut using, |u| u.label == s.label);
            }
            if args.len() == 2 && register(&args[1], &symbols).is_err() {
                let (text, qualifier) = qualified_expression(&args[1]).map_err(|e| s.error(e))?;
                let support =
                    expression(&text, &symbols, &s.section, s.offset).map_err(|e| s.error(e))?;
                if support.section.is_none() {
                    return Err(
                        s.error("dependent USING requires a relocatable supporting address")
                    );
                }
                let bindings: Vec<_> = using
                    .iter()
                    .filter(|u| u.label == qualifier && u.origin.section == support.section)
                    .filter(|u| (-524288..=524287).contains(&(support.value - u.origin.value)))
                    .filter(|u| {
                        u.lower.is_none_or(|low| support.value >= low)
                            && u.upper.is_none_or(|high| support.value < high)
                    })
                    .map(|u| {
                        let delta = origin.value - support.value;
                        let mut mapped = origin.clone();
                        mapped.value = u.origin.value + delta;
                        Using {
                            origin: mapped,
                            register: u.register,
                            label: s.label.clone(),
                            root: u.root,
                            dependent: true,
                            lower: lower.into_iter().chain(u.lower.map(|v| v + delta)).max(),
                            upper: upper.into_iter().chain(u.upper.map(|v| v + delta)).min(),
                        }
                    })
                    .collect();
                if bindings.is_empty() {
                    return Err(s.error("no active supporting USING for dependent USING"));
                }
                using.extend(bindings);
                continue;
            }
            for (index, arg) in args[1..].iter().enumerate() {
                let r = register(arg, &symbols).map_err(|e| s.error(e))?;
                let mut origin = origin.clone();
                origin.value += index as i64 * 4096;
                drop_using(&mut using, |u| {
                    !u.dependent && u.label == s.label && u.register == r
                });
                using_serial += 1;
                using.push(Using {
                    origin,
                    register: r,
                    label: s.label.clone(),
                    root: using_serial,
                    dependent: false,
                    lower,
                    upper,
                });
            }
            continue;
        }
        if s.op == "DROP" {
            for a in args {
                let a = a.to_ascii_uppercase();
                if let Ok(r) = register(&a, &symbols) {
                    drop_using(&mut using, |u| {
                        !u.dependent && u.label.is_empty() && u.register == r
                    });
                } else if using.iter().any(|u| u.label == a) {
                    drop_using(&mut using, |u| u.label == a);
                } else {
                    return Err(s.error(format!("unknown USING label or register {a}")));
                }
            }
            continue;
        }
        if instruction_size(&s.op).is_none() {
            continue;
        }
        let make = || -> Result<Instruction, String> {
            let mut i = Instruction {
                offset: s.offset,
                op: s.op.clone(),
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
                file: s.file.clone(),
                line: s.line,
            };
            let reg = |n: usize| register(args.get(n).ok_or("missing register")?, &symbols);
            let addr = |n: usize, ss| {
                address(
                    args.get(n).ok_or("missing address")?,
                    s,
                    &symbols,
                    &using,
                    ss,
                )
            };
            let expected = match canonical_operation(&s.op) {
                "OPEN" | "CLOSE" | "GET" | "PUT" | "FREEPOOL" => {
                    i.data_service = Some(data_service(s, &args, &symbols, &using)?);
                    args.len()
                }
                op if super::operation::conditional(op).is_some() => {
                    use super::operation::ConditionalKind;
                    let spec = super::operation::conditional(op).unwrap();
                    i.registers = vec![reg(0)?];
                    let mask = if let Some(mask) = spec.mask {
                        u32::from(mask)
                    } else {
                        let value = super::expression::evaluate_early(
                            args.get(2).ok_or("missing condition mask")?,
                            &symbols,
                        )?
                        .value;
                        u32::try_from(value)
                            .ok()
                            .filter(|n| *n <= 15)
                            .ok_or("condition mask must be 0..15")?
                    };
                    match spec.kind {
                        ConditionalKind::Register => {
                            i.registers.push(reg(1)?);
                            i.immediate = mask;
                        }
                        ConditionalKind::Immediate => {
                            let value = super::expression::evaluate_early(
                                args.get(1).ok_or("missing conditional immediate")?,
                                &symbols,
                            )?
                            .value;
                            i.immediate = i16::try_from(value)
                                .map_err(|_| "conditional immediate must fit signed 16 bits")?
                                as i32 as u32;
                            i.length = mask;
                        }
                        _ => {
                            let operand = addr(1, false)?.0;
                            if operand.index != 0 {
                                return Err("index not permitted for conditional load/store".into());
                            }
                            i.address = Some(operand);
                            i.immediate = mask;
                        }
                    }
                    if spec.mask.is_some() { 2 } else { 3 }
                }
                "WTO" | "WAIT" | "POST" => {
                    i.system_service = Some(system_service(s, &args, &symbols, &using)?);
                    args.len()
                }
                "LINK" | "XCTL" | "LOAD" | "DELETE" => {
                    i.module_service = Some(module_service(s, &args, &symbols, &using)?);
                    args.len()
                }
                op if super::operation::interlocked(op).is_some() => {
                    let operand = if op == "TS" { 0 } else { 2 };
                    if operand == 2 {
                        i.registers = vec![reg(0)?, reg(1)?];
                        if op.starts_with("CDS") && i.registers.iter().any(|r| r % 2 != 0) {
                            return Err(
                                "COMPARE DOUBLE AND SWAP requires even register pairs".into()
                            );
                        }
                    }
                    let (address, _) = addr(operand, false)?;
                    if address.index != 0 {
                        return Err(
                            "interlocked instructions do not accept an index register".into()
                        );
                    }
                    i.address = Some(address);
                    operand + 1
                }
                "MVST" | "CLST" | "SRST" => {
                    i.registers = vec![reg(0)?, reg(1)?];
                    2
                }
                "MVCL" | "CLCL" | "MVCLE" | "CLCLE" => {
                    i.registers = vec![reg(0)?, reg(1)?];
                    if i.registers.iter().any(|r| r % 2 != 0) {
                        return Err("long string instructions require even register pairs".into());
                    }
                    if matches!(s.op.as_str(), "MVCLE" | "CLCLE") {
                        let (operand, _) = addr(2, false)?;
                        if operand.index != 0 {
                            return Err(
                                "long string padding does not accept an index register".into()
                            );
                        }
                        i.address = Some(operand);
                        3
                    } else {
                        2
                    }
                }
                "GETMAIN" | "FREEMAIN" | "STORAGE" => {
                    i.storage_service = Some(storage_service(s, &args, &symbols, &using)?);
                    args.len()
                }
                op if super::operation::relative(op).is_some() => {
                    use super::operation::RelativeKind;
                    let spec = super::operation::relative(op).unwrap();
                    let target = match spec.kind {
                        RelativeKind::Condition(Some(mask)) => {
                            i.immediate = u32::from(mask);
                            0
                        }
                        RelativeKind::Condition(None) => {
                            let mask = expression(
                                args.first().ok_or("missing branch mask")?,
                                &symbols,
                                &s.section,
                                s.offset,
                            )?;
                            if mask.section.is_some() || !(0..=15).contains(&mask.value) {
                                return Err("invalid branch mask".into());
                            }
                            i.immediate = mask.value as u32;
                            1
                        }
                        _ => {
                            i.registers = vec![reg(0)?];
                            1
                        }
                    };
                    let operand = args.get(target).ok_or("missing relative target")?;
                    let value =
                        super::expression::evaluate_terms(operand, &symbols, &s.section, s.offset)?;
                    let mut local_coefficient = 0i64;
                    let mut coefficient = 0i64;
                    let mut terms = Vec::new();
                    for (section, n) in &value.relocations {
                        coefficient = coefficient.checked_add(*n).ok_or("expression overflow")?;
                        if let Some(symbol) = section.strip_prefix("EXTERN:") {
                            terms.push(super::RelativeTerm {
                                symbol: symbol.to_owned(),
                                coefficient: *n,
                                weak: externals.get(symbol).copied().unwrap_or(false),
                            });
                        } else if section == &s.section {
                            local_coefficient = *n;
                        } else {
                            return Err(
                                "relative target must belong to the assembled control sections"
                                    .into(),
                            );
                        }
                    }
                    if coefficient != 1 {
                        return Err(
                            "relative target must belong to the assembled control sections or external symbols, with net relocation coefficient one".into(),
                        );
                    }
                    if !s.offset.is_multiple_of(2) {
                        return Err("relative instruction must be halfword aligned".into());
                    }
                    let displacement = if terms.len() == 1
                        && terms[0].coefficient == 1
                        && local_coefficient == 0
                    {
                        let term = terms.pop().unwrap();
                        i.relative_external = Some(super::RelativeExternal {
                            symbol: term.symbol,
                            addend: value.number,
                            weak: term.weak,
                        });
                        // A symbolic operand is fixed only once all modules have
                        // their final locations. Zero is a placeholder here.
                        0
                    } else if !terms.is_empty() {
                        i.relative_expression = Some(super::RelativeExpression {
                            addend: value.number,
                            local_coefficient,
                            terms,
                        });
                        0
                    } else {
                        let displacement = value
                            .number
                            .checked_sub(i64::from(s.offset))
                            .ok_or("relative displacement overflow")?;
                        super::operation::check_relative(displacement, spec.bits)?;
                        displacement
                    };
                    i.address = Some(Address {
                        displacement,
                        base: 0,
                        index: 0,
                    });
                    target + 1
                }
                "BCTR" | "BCTGR" => {
                    i.registers = vec![reg(0)?, reg(1)?];
                    2
                }
                op if super::operation::masked_bytes(op).is_some() => {
                    i.registers = vec![reg(0)?];
                    let mask = expression(
                        args.get(1).ok_or("missing byte mask")?,
                        &symbols,
                        &s.section,
                        s.offset,
                    )?;
                    if mask.section.is_some() || !(0..=15).contains(&mask.value) {
                        return Err("invalid four-bit byte mask".into());
                    }
                    i.immediate = mask.value as u32;
                    i.address = Some(addr(2, false)?.0);
                    if i.address.as_ref().unwrap().index != 0 {
                        return Err("index not permitted for masked-byte instruction".into());
                    }
                    3
                }
                op if super::operation::register_logic(op).is_some() => {
                    i.registers = vec![reg(0)?, reg(1)?];
                    if matches!(op, "NOTR" | "NOTGR") {
                        i.registers.push(reg(1)?);
                        i.op = if op == "NOTR" { "NORK" } else { "NOGRK" }.into();
                        2
                    } else {
                        i.registers.push(reg(2)?);
                        3
                    }
                }
                op if super::operation::select(op).is_some() => {
                    let (root, mask) = super::operation::select(op).unwrap();
                    i.op = root.into();
                    i.registers = vec![reg(0)?, reg(1)?, reg(2)?];
                    i.immediate = if let Some(mask) = mask {
                        mask.into()
                    } else {
                        let value = expression(
                            args.get(3).ok_or("SELECT condition mask required")?,
                            &symbols,
                            &s.section,
                            s.offset,
                        )?;
                        if value.section.is_some() || !(0..=15).contains(&value.value) {
                            return Err("SELECT mask must be 0..15".into());
                        }
                        value.value as u32
                    };
                    if mask.is_some() { 3 } else { 4 }
                }
                "CSST" => {
                    i.address = Some(addr(0, false)?.0);
                    i.source = Some(addr(1, false)?.0);
                    i.registers = vec![reg(2)?];
                    if i.address.as_ref().unwrap().index != 0
                        || i.source.as_ref().unwrap().index != 0
                    {
                        return Err("CSST does not accept index registers".into());
                    }
                    3
                }
                "LPD" | "LPDG" => {
                    i.registers = vec![reg(0)?];
                    i.address = Some(addr(1, false)?.0);
                    i.source = Some(addr(2, false)?.0);
                    if i.address.as_ref().unwrap().index != 0
                        || i.source.as_ref().unwrap().index != 0
                    {
                        return Err("LPD/LPDG do not accept index registers".into());
                    }
                    3
                }
                "FLOGR" | "POPCNT" => {
                    i.registers = vec![reg(0)?, reg(1)?];
                    if s.op == "POPCNT" && args.len() == 3 {
                        let mask = expression(&args[2], &symbols, &s.section, s.offset)?;
                        if mask.section.is_some() || ![0, 8].contains(&mask.value) {
                            return Err(
                                "POPCNT mask must be 0 or 8; other bits are reserved".into()
                            );
                        }
                        i.immediate = mask.value as u32;
                        3
                    } else {
                        2
                    }
                }
                op if super::operation::rotate_selected(op).is_some() => {
                    let (root, flags) = super::operation::rotate_selected(op).unwrap();
                    if !(4..=5).contains(&args.len()) {
                        return Err("rotate/select requires r1,r2,start,end[,rotate]".into());
                    }
                    i.op = root.into();
                    i.registers = vec![reg(0)?, reg(1)?];
                    for (index, arg) in args.iter().enumerate().skip(2) {
                        let value = expression(arg, &symbols, &s.section, s.offset)?;
                        if value.section.is_some() || !(0..=255).contains(&value.value) {
                            return Err("rotate/select immediate must fit an unsigned byte".into());
                        }
                        i.immediate |= (value.value as u32) << ((4 - index) * 8);
                    }
                    i.immediate |= flags;
                    args.len()
                }
                op if super::operation::partial_immediate(op).is_some() => {
                    let spec = super::operation::partial_immediate(op).unwrap();
                    i.registers = vec![reg(0)?];
                    let value = expression(
                        args.get(1).ok_or("missing immediate")?,
                        &symbols,
                        &s.section,
                        s.offset,
                    )?;
                    if value.section.is_some() {
                        return Err("immediate must be absolute".into());
                    }
                    i.immediate = u32::try_from(value.value)
                        .ok()
                        .or_else(|| {
                            (spec.width == 32)
                                .then(|| i32::try_from(value.value).ok().map(|n| n as u32))
                                .flatten()
                        })
                        .filter(|&n| spec.width == 32 || n <= u32::from(u16::MAX))
                        .ok_or("invalid unsigned immediate")?;
                    2
                }
                "ALCR" | "ALCGR" | "SLBR" | "SLBGR" => {
                    i.registers = vec![reg(0)?, reg(1)?];
                    2
                }
                "SLLG" | "SRLG" | "SLAG" | "SRAG" | "RLL" | "RLLG" => {
                    i.registers = vec![reg(0)?, reg(1)?];
                    i.address = Some(addr(2, false)?.0);
                    3
                }
                "SLDL" | "SRDL" | "SLDA" | "SRDA" => {
                    i.registers = vec![reg(0)?];
                    i.address = Some(addr(1, false)?.0);
                    2
                }
                "LRVR" | "LRVGR" | "LBR" | "LGBR" | "MLR" | "MLGR" | "DLR" | "DLGR" | "DSGR"
                | "DSGFR" => {
                    i.registers = vec![reg(0)?, reg(1)?];
                    2
                }
                "LGR" | "LGFR" | "LLGFR" | "LTGR" | "LCGR" | "LPGR" | "LNGR" | "AGR" | "SGR"
                | "ALGR" | "SLGR" | "CGR" | "CLGR" | "NGR" | "OGR" | "XGR" | "AGFR" | "SGFR"
                | "CGFR" | "MSGR" => {
                    i.registers = vec![reg(0)?, reg(1)?];
                    2
                }
                "LMG" | "STMG" => {
                    i.registers = vec![reg(0)?, reg(1)?];
                    i.address = Some(addr(2, false)?.0);
                    if i.address.as_ref().unwrap().index != 0 {
                        return Err("index not permitted for multiple-register instruction".into());
                    }
                    3
                }
                op if wide_memory(op) => {
                    i.registers = vec![reg(0)?];
                    i.address = Some(addr(1, false)?.0);
                    2
                }
                "LGFI" | "AGFI" | "CGFI" => {
                    i.registers = vec![reg(0)?];
                    i.immediate = i32::try_from(
                        expression(
                            args.get(1).ok_or("missing immediate")?,
                            &symbols,
                            &s.section,
                            s.offset,
                        )?
                        .value,
                    )
                    .map_err(|_| "invalid signed immediate")?
                        as u32;
                    2
                }
                "LR" | "SR" | "AR" | "CR" | "LTR" | "XR" | "NR" | "OR" | "CLR" | "ALR" | "SLR"
                | "LCR" | "LPR" | "LNR" | "MR" | "DR" | "BALR" | "BASR" => {
                    i.registers = vec![reg(0)?, reg(1)?];
                    2
                }
                "BR" => {
                    i.registers = vec![reg(0)?];
                    1
                }
                "BCR" => {
                    i.immediate = reg(0)? as u32;
                    i.registers = vec![reg(1)?];
                    2
                }
                "NOPR" => {
                    i.op = "BCR".into();
                    i.immediate = 0;
                    i.registers = vec![reg(0)?];
                    1
                }
                "EX" | "L" | "ST" | "LA" | "BCT" | "A" | "S" | "C" | "CL" | "AL" | "SL" | "N"
                | "O" | "X" | "AH" | "SH" | "CH" | "MH" | "M" | "D" | "LH" | "STH" | "IC"
                | "STC" | "SLL" | "SRL" | "SLA" | "SRA" | "BAL" | "BAS" | "CVB" | "CVD" => {
                    i.registers = vec![reg(0)?];
                    i.address = Some(addr(1, false)?.0);
                    2
                }
                "STM" | "LM" => {
                    i.registers = vec![reg(0)?, reg(1)?];
                    i.address = Some(addr(2, false)?.0);
                    3
                }
                "AHI" | "LGHI" | "AGHI" | "CGHI" | "MGHI" => {
                    i.registers = vec![reg(0)?];
                    i.immediate = i16::try_from(
                        expression(
                            args.get(1).ok_or("missing immediate")?,
                            &symbols,
                            &s.section,
                            s.offset,
                        )?
                        .value,
                    )
                    .map_err(|_| "invalid signed immediate")?
                        as u32;
                    2
                }
                "B" | "BE" | "BNE" | "BH" | "BL" | "BNH" | "BNL" | "BO" | "BNO" | "BP" | "BM"
                | "BZ" | "BNZ" | "BNP" | "BNM" | "NOP" => {
                    i.immediate = match s.op.as_str() {
                        "B" => 15,
                        "BE" | "BZ" => 8,
                        "BNE" | "BNZ" => 7,
                        "BH" | "BP" => 2,
                        "BL" | "BM" => 4,
                        "BNH" | "BNP" => 13,
                        "BNL" | "BNM" => 11,
                        "BO" => 1,
                        "BNO" => 14,
                        _ => 0,
                    };
                    i.op = "BC".into();
                    i.address = Some(addr(0, false)?.0);
                    1
                }
                "BC" => {
                    i.immediate = reg(0)? as u32;
                    i.address = Some(addr(1, false)?.0);
                    2
                }
                "MVI" | "CLI" | "NI" | "OI" | "XI" | "TM" => {
                    i.address = Some(addr(0, false)?.0);
                    let spec = args.get(1).ok_or("missing immediate")?;
                    // Use original spelling for character immediates.
                    let original = operands(&s.args);
                    i.immediate = if spec.contains('\'') {
                        let bytes = storage(&original[1], true, &symbols)?.0;
                        if bytes.len() != 1 {
                            return Err("byte immediate required".into());
                        }
                        u32::from(bytes[0])
                    } else {
                        u8::try_from(expression(spec, &symbols, &s.section, s.offset)?.value)
                            .map(u32::from)
                            .map_err(|_| "invalid byte immediate")?
                    };
                    2
                }
                "MVC" | "CLC" | "XC" | "NC" | "OC" | "MVN" | "MVZ" | "TR" | "TRT" | "TRTR"
                | "ED" | "EDMK" => {
                    let (a, len) = addr(0, true)?;
                    if !(1..=256).contains(&len) {
                        return Err("SS length must be 1..256".into());
                    }
                    i.address = Some(a);
                    i.source = Some(addr(1, false)?.0);
                    i.length = len;
                    2
                }
                "SRP" => {
                    let (a, length) = addr(0, true)?;
                    if !(1..=16).contains(&length) {
                        return Err("SRP length must be 1..16".into());
                    }
                    let shift = addr(1, false)?.0;
                    if a.index != 0 || shift.index != 0 {
                        return Err("SRP does not accept index registers".into());
                    }
                    let rounding = expression(
                        args.get(2).ok_or("SRP rounding digit required")?,
                        &symbols,
                        &s.section,
                        s.offset,
                    )?;
                    if rounding.section.is_some() || !(0..=9).contains(&rounding.value) {
                        return Err("SRP rounding digit must be 0..9".into());
                    }
                    i.address = Some(a);
                    i.source = Some(shift);
                    i.length = length;
                    i.immediate = rounding.value as u32;
                    3
                }
                "ZAP" | "AP" | "SP" | "CP" | "MP" | "DP" | "PACK" | "UNPK" => {
                    let (a, len) = addr(0, true)?;
                    let (b, len2) = addr(1, true)?;
                    if !(1..=16).contains(&len) || !(1..=16).contains(&len2) {
                        return Err("decimal lengths must be 1..16".into());
                    }
                    i.address = Some(a);
                    i.source = Some(b);
                    i.length = len;
                    i.source_length = len2;
                    2
                }
                "STIMER" => {
                    if args.len() != 2 || args[0] != "WAIT" {
                        return Err("only STIMER WAIT,BINTVL=address is supported".into());
                    }
                    let operand = args[1].strip_prefix("BINTVL=").ok_or("BINTVL required")?;
                    i.address = Some(address(operand, s, &symbols, &using, false)?.0);
                    2
                }
                _ => return Err("unsupported instruction".into()),
            };
            if args.len() != expected {
                return Err("wrong operand count".into());
            }
            if matches!(
                canonical_operation(&s.op),
                "LM" | "STM" | "CLI" | "MVI" | "NI" | "OI" | "XI" | "TM"
            ) && i.address.as_ref().is_some_and(|a| a.index != 0)
            {
                return Err("index not permitted for RS/SI instruction".into());
            }
            if pair_operation(&s.op) && !i.registers[0].is_multiple_of(2) {
                return Err("instruction requires an even register pair".into());
            }
            if shift_operation(&s.op) && i.address.as_ref().is_some_and(|a| a.index != 0) {
                return Err("index not permitted for shift/rotate instruction".into());
            }
            Ok(i)
        };
        code.push(make().map_err(|e| s.error(e))?);
    }
    for literal in literal_fixups.iter().filter(|l| l.pool == usize::MAX) {
        fix_base_constants(
            &literal.spec,
            literal.offset,
            Some(literal.location),
            &symbols,
            &using,
            &mut image,
        )?;
    }
    if !code.iter().any(|i| i.offset == entry) {
        return Err("entry does not address an instruction".into());
    }
    super::template::initialize(&code, &mut image)?;
    for q in &bindings.quantities {
        if matches!(
            q.kind,
            super::QuantityKind::Offset | super::QuantityKind::LongOffset
        ) {
            for section in &mut bindings.sections {
                if section.name == q.symbol
                    && control_areas
                        .iter()
                        .any(|(control, _)| control == &format!("$D:{}", q.symbol))
                {
                    section.dummy = true;
                }
            }
        }
    }
    bindings.sections.retain(|section| {
        section.dummy
            || section.concatenate
            || bindings.quantities.iter().any(|q| q.symbol == section.name)
    });
    for q in &mut bindings.quantities {
        if matches!(
            q.kind,
            super::QuantityKind::Offset | super::QuantityKind::LongOffset
        ) && let Some(control) = symbol_classes.get(&q.symbol)
        {
            if q.symbol == class_controls[control] {
                continue;
            }
            let attributes = &part_attributes[control];
            let name = attributes.part.as_ref().unwrap_or(&class_controls[control]);
            let value = &symbols[&q.symbol];
            let origin = control_origins[control];
            q.addend = u32::try_from(value.value)
                .ok()
                .and_then(|value| value.checked_sub(origin))
                .ok_or("invalid class label offset")?;
            q.element = bindings
                .sections
                .iter()
                .position(|s| s.name == *name && (attributes.no_load || s.image == Some(origin)));
            if q.element.is_none() {
                return Err("class label has no binding element".into());
            }
        }
    }
    for (name, &offset) in &entries {
        if let Some(control) = symbol_controls.get(name) {
            bindings.labels.push(super::BindingLabel {
                symbol: name.clone(),
                offset,
                section: control_origins[control],
                class: class_controls.get(control).cloned(),
            });
        }
    }
    let module = Module {
        zeus_hlasm: super::format_version(&code, &relocations).max(
            if bindings.requires_extended() {
                9
            } else if !dcbs.is_empty() {
                8
            } else if !read_only.is_empty() || rmode == 24 {
                6
            } else if image_alignment > 8 {
                5
            } else {
                1
            },
        ),
        name,
        alignment: image_alignment,
        instruction_templates: true,
        read_only,
        rmode,
        storage: image,
        entry,
        code,
        relocations,
        entries,
        externals,
        dcbs,
        bindings,
    };
    super::data_management::validate(&module)?;
    module.bindings.validate(&module)?;
    Ok(LayoutPass::Complete(Box::new(module)))
}

fn data_service(
    s: &Statement,
    args: &[String],
    symbols: &Symbols,
    using: &[Using],
) -> Result<super::DataService, String> {
    use super::{DataService, StorageOperand};
    let pointer = |text: &str| -> Result<StorageOperand, String> {
        if let Some(r) = text.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            let r = register(r, symbols)?;
            if !(1..=12).contains(&r) {
                return Err("data service address register must be 1..12".into());
            }
            Ok(StorageOperand::Register(r))
        } else {
            Ok(StorageOperand::Address(
                address(text, s, symbols, using, false)?.0,
            ))
        }
    };
    match s.op.as_str() {
        "GET" | "PUT" => {
            if args.is_empty() || args.len() > 2 {
                return Err("GET/PUT dcb[,area] required".into());
            }
            let dcb = pointer(&args[0])?;
            let area = args
                .get(1)
                .filter(|s| !s.is_empty())
                .map(|s| pointer(s))
                .transpose()?;
            Ok(if s.op == "GET" {
                DataService::Get { dcb, area }
            } else {
                DataService::Put { dcb, area }
            })
        }
        "FREEPOOL" => {
            if args.len() != 1 {
                return Err("FREEPOOL dcb required".into());
            }
            Ok(DataService::FreePool(pointer(&args[0])?))
        }
        "OPEN" => {
            if args.len() != 1 {
                return Err("unsupported OPEN form; use OPEN (dcb,mode,...)".into());
            }
            let list = args[0]
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .ok_or("OPEN requires a parenthesized list")?;
            let list = operands(list);
            let mut result = vec![];
            let mut at = 0;
            while at < list.len() {
                let dcb = pointer(&list[at])?;
                let mode = list
                    .get(at + 1)
                    .map(|s| s.to_ascii_uppercase())
                    .unwrap_or_default();
                let (output, extend) = match mode.as_str() {
                    "" | "INPUT" => (false, false),
                    "OUTPUT" => (true, false),
                    "EXTEND" => (true, true),
                    _ => return Err(format!("unsupported OPEN mode {mode}")),
                };
                result.push((dcb, output, extend));
                at += 2;
            }
            if result.is_empty() {
                return Err("OPEN list is empty".into());
            }
            Ok(DataService::Open(result))
        }
        "CLOSE" => {
            if args.len() != 1 {
                return Err("unsupported CLOSE form; use CLOSE (dcb,...)".into());
            }
            let list = args[0]
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .ok_or("CLOSE requires a parenthesized list")?;
            let list = operands(list)
                .into_iter()
                .filter(|s| !s.is_empty())
                .map(|s| pointer(&s))
                .collect::<Result<Vec<_>, _>>()?;
            if list.is_empty() {
                return Err("CLOSE list is empty".into());
            }
            Ok(DataService::Close(list))
        }
        _ => unreachable!(),
    }
}

fn data_control_block(s: &Statement, symbols: &Symbols) -> Result<super::DataControlBlock, String> {
    use super::{DataControlBlock, DcbAccessMode as AccessMode, DcbRecordFormat as RecordFormat};
    if s.label.is_empty() {
        return Err("DCB requires a label".into());
    }
    let mut dcb = DataControlBlock {
        offset: s.offset,
        ddname: String::new(),
        record_format: RecordFormat::Fixed,
        blocked: false,
        record_length: 0,
        block_size: 0,
        get: None,
        put: None,
        end: None,
        error: None,
    };
    let mut seen = HashSet::new();
    for arg in operands(&s.args) {
        let (key, value) = arg.split_once('=').ok_or("DCB keyword=value required")?;
        let key = key.to_ascii_uppercase();
        if !seen.insert(key.clone()) {
            return Err(format!("duplicate DCB keyword {key}"));
        }
        let number = || -> Result<u16, String> {
            let n = super::expression::evaluate_early(value, symbols)?.value;
            u16::try_from(n)
                .ok()
                .filter(|n| *n <= 32760)
                .ok_or_else(|| "DCB length must be 0..32760".into())
        };
        match key.as_str() {
            "DDNAME" => {
                let name = value.trim_matches('\'').to_ascii_uppercase();
                if !symbol_name(&name) || name.len() > 8 {
                    return Err("DCB DDNAME must be a symbol of 1..8 characters".into());
                }
                dcb.ddname = name;
            }
            "DSORG" if value.eq_ignore_ascii_case("PS") => {}
            "DEVD" if value.eq_ignore_ascii_case("DA") => {}
            "RECFM" => {
                let value = value.to_ascii_uppercase();
                match value.as_str() {
                    "F" | "FB" => dcb.record_format = RecordFormat::Fixed,
                    "V" | "VB" => dcb.record_format = RecordFormat::Variable,
                    _ => return Err("supported QSAM RECFM forms are F/FB/V/VB".into()),
                }
                dcb.blocked = value.ends_with('B');
            }
            "LRECL" => dcb.record_length = number()?,
            "BLKSIZE" => dcb.block_size = number()?,
            "MACRF" => {
                let modes = value
                    .strip_prefix('(')
                    .and_then(|s| s.strip_suffix(')'))
                    .unwrap_or(value);
                for mode in operands(modes) {
                    let mode = mode.to_ascii_uppercase();
                    match mode.as_str() {
                        "GM" | "GL" => {
                            if dcb
                                .get
                                .replace(if mode.ends_with('M') {
                                    AccessMode::Move
                                } else {
                                    AccessMode::Locate
                                })
                                .is_some()
                            {
                                return Err("duplicate DCB GET mode".into());
                            }
                        }
                        "PM" | "PL" => {
                            if dcb
                                .put
                                .replace(if mode.ends_with('M') {
                                    AccessMode::Move
                                } else {
                                    AccessMode::Locate
                                })
                                .is_some()
                            {
                                return Err("duplicate DCB PUT mode".into());
                            }
                        }
                        _ => return Err("supported QSAM MACRF forms are GM/GL/PM/PL".into()),
                    }
                }
            }
            "EODAD" | "SYNAD" => {
                let target = expression(value, symbols, "", s.offset)?;
                if target.section.as_deref() != Some("") {
                    return Err("DCB exit must address executable storage".into());
                }
                let target = u32::try_from(target.value).map_err(|_| "invalid DCB exit")?;
                if key == "EODAD" {
                    dcb.end = Some(target);
                } else {
                    dcb.error = Some(target);
                }
            }
            _ => return Err(format!("unsupported DCB keyword/form {key}={value}")),
        }
    }
    if dcb.ddname.is_empty() || (dcb.get.is_none() && dcb.put.is_none()) {
        return Err("DCB requires DDNAME and MACRF".into());
    }
    if dcb.record_format == RecordFormat::Variable
        && dcb.record_length != 0
        && dcb.record_length < 4
    {
        return Err("variable DCB LRECL must include its four-byte RDW".into());
    }
    Ok(dcb)
}

fn system_service(
    s: &Statement,
    args: &[String],
    symbols: &Symbols,
    using: &[Using],
) -> Result<super::SystemService, String> {
    use super::{MessageText, StorageOperand, SystemService};
    let pointer = |text: &str| -> Result<StorageOperand, String> {
        if let Some(r) = text.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            let r = register(r, symbols)?;
            if !(1..=12).contains(&r) {
                return Err("service address register must be 1..12".into());
            }
            Ok(StorageOperand::Register(r))
        } else {
            Ok(StorageOperand::Address(
                address(text, s, symbols, using, false)?.0,
            ))
        }
    };
    let scalar = |text: &str, max: u32| -> Result<StorageOperand, String> {
        if let Some(r) = text.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            let r = register(r, symbols)?;
            if !(r == 0 || (2..=12).contains(&r)) {
                return Err("service value register must be 0 or 2..12".into());
            }
            return Ok(StorageOperand::Register(r));
        }
        let value = super::expression::evaluate_early(text, symbols)?.value;
        let value = u32::try_from(value)
            .ok()
            .filter(|v| *v <= max)
            .ok_or("service scalar value out of range")?;
        Ok(StorageOperand::Constant(value))
    };
    if s.op == "POST" {
        if args.is_empty() || args.len() > 2 {
            return Err("POST ecb[,completion] required".into());
        }
        return Ok(SystemService::Post {
            ecb: pointer(&args[0])?,
            code: scalar(
                args.get(1)
                    .map(String::as_str)
                    .filter(|s| !s.is_empty())
                    .unwrap_or("0"),
                0x3fff_ffff,
            )?,
        });
    }
    let mut seen = HashSet::new();
    if s.op == "WAIT" {
        let mut events = StorageOperand::Constant(1);
        let mut ecbs = None;
        let mut list = false;
        for (index, arg) in args.iter().enumerate() {
            if index == 0 && !arg.contains('=') {
                if !arg.is_empty() {
                    events = scalar(arg, 255)?;
                }
                continue;
            }
            let (key, value) = arg.split_once('=').ok_or("WAIT keyword=value required")?;
            let key = key.to_ascii_uppercase();
            if !seen.insert(key.clone()) {
                return Err(format!("duplicate WAIT keyword {key}"));
            }
            match key.as_str() {
                "ECB" | "ECBLIST" => {
                    if ecbs.replace(pointer(value)?).is_some() {
                        return Err("ECB and ECBLIST are mutually exclusive".into());
                    }
                    list = key == "ECBLIST";
                }
                "LINKAGE" if value == "SVC" => {}
                _ => return Err(format!("unsupported WAIT keyword/form {key}={value}")),
            }
        }
        if !list && matches!(events,StorageOperand::Constant(n) if n != 1) {
            return Err("WAIT ECB requires one event".into());
        }
        return Ok(SystemService::Wait {
            ecbs: ecbs.ok_or("WAIT requires ECB or ECBLIST")?,
            list,
            events,
        });
    }
    let (mut text, mut routes, mut descriptors) = (None, vec![], vec![]);
    for (index, arg) in args.iter().enumerate() {
        if index == 0 && arg.starts_with('\'') && arg.ends_with('\'') && arg.len() >= 2 {
            text = Some(MessageText::Literal(
                arg[1..arg.len() - 1].replace("''", "'"),
            ));
            continue;
        }
        let (key, value) = arg
            .split_once('=')
            .ok_or("WTO requires quoted text or TEXT=address")?;
        let key = key.to_ascii_uppercase();
        if !seen.insert(key.clone()) {
            return Err(format!("duplicate WTO keyword {key}"));
        }
        match key.as_str() {
            "TEXT" => {
                if text
                    .replace(MessageText::Address(pointer(value)?))
                    .is_some()
                {
                    return Err("WTO text forms are mutually exclusive".into());
                }
            }
            "ROUTCDE" | "DESC" => {
                let list = value
                    .strip_prefix('(')
                    .and_then(|s| s.strip_suffix(')'))
                    .unwrap_or(value);
                let codes = super::assemble::operands(list)
                    .iter()
                    .map(|v| {
                        let n = super::expression::evaluate_early(v, symbols)?.value;
                        if !(1..=if key == "ROUTCDE" { 128 } else { 16 }).contains(&n) {
                            return Err("WTO routing/descriptor code out of range".to_string());
                        }
                        Ok(n as u8)
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                if key == "ROUTCDE" {
                    routes = codes;
                } else {
                    descriptors = codes;
                }
            }
            "LINKAGE" if value == "SVC" => {}
            _ => return Err(format!("unsupported WTO keyword/form {key}={value}")),
        }
    }
    Ok(SystemService::Write {
        text: text.ok_or("WTO message text required")?,
        routes,
        descriptors,
    })
}

fn module_service(
    s: &Statement,
    args: &[String],
    symbols: &Symbols,
    using: &[Using],
) -> Result<super::ModuleService, String> {
    use super::{ModuleName, ModuleService, StorageOperand};
    let pointer = |text: &str| -> Result<StorageOperand, String> {
        if let Some(r) = text.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            let r = register(r, symbols)?;
            if !(2..=12).contains(&r) {
                return Err("module service address registers must be 2..12".into());
            }
            Ok(StorageOperand::Register(r))
        } else {
            Ok(StorageOperand::Address(
                address(text, s, symbols, using, false)?.0,
            ))
        }
    };
    let (mut name, mut parameters, mut variable_list) = (None, None, false);
    let mut seen = HashSet::new();
    for arg in args {
        let (key, value) = arg
            .split_once('=')
            .ok_or("module service keyword=value required")?;
        let key = key.to_ascii_uppercase();
        if !seen.insert(key.clone()) {
            return Err(format!("duplicate module service keyword {key}"));
        }
        match key.as_str() {
            "EP" => {
                let value = value.to_ascii_uppercase();
                if !symbol_name(&value) || value.len() > 8 {
                    return Err("EP must be a symbol of 1..8 characters".into());
                }
                if name.replace(ModuleName::Literal(value)).is_some() {
                    return Err("EP and EPLOC are mutually exclusive".into());
                }
            }
            "EPLOC" => {
                if name.replace(ModuleName::Address(pointer(value)?)).is_some() {
                    return Err("EP and EPLOC are mutually exclusive".into());
                }
            }
            "PARAM" if matches!(s.op.as_str(), "LINK" | "XCTL") => {
                let value = value
                    .strip_prefix('(')
                    .and_then(|s| s.strip_suffix(')'))
                    .unwrap_or(value);
                parameters = Some(if value.is_empty() {
                    vec![]
                } else {
                    operands(value)
                        .iter()
                        .map(|v| pointer(v))
                        .collect::<Result<Vec<_>, _>>()?
                });
            }
            "VL" if value == "1" && matches!(s.op.as_str(), "LINK" | "XCTL") => {
                variable_list = true
            }
            _ => return Err(format!("unsupported {} keyword/form {key}={value}", s.op)),
        }
    }
    if variable_list && parameters.is_none() {
        return Err("VL=1 requires PARAM".into());
    }
    if matches!(s.op.as_str(), "LOAD" | "DELETE") {
        parameters = Some(vec![]);
    }
    Ok(ModuleService {
        name: name.ok_or("module service requires EP or EPLOC")?,
        parameters,
        variable_list,
    })
}

fn storage_service(
    s: &Statement,
    args: &[String],
    symbols: &Symbols,
    using: &[Using],
) -> Result<super::StorageService, String> {
    use super::{StorageOperand, StorageService};
    let mode_text = args
        .first()
        .ok_or("storage service mode required")?
        .to_ascii_uppercase();
    let mode = mode_text.as_str();
    let storage = s.op == "STORAGE";
    let obtain = if storage {
        match mode {
            "OBTAIN" => true,
            "RELEASE" => false,
            _ => return Err("STORAGE requires OBTAIN or RELEASE".into()),
        }
    } else {
        if !matches!(mode, "R" | "RU" | "RC") {
            return Err("GETMAIN/FREEMAIN currently support R, RU and RC requests".into());
        }
        s.op == "GETMAIN"
    };
    let operand = |text: &str| -> Result<StorageOperand, String> {
        if let Some(r) = text.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            return Ok(StorageOperand::Register(register(r, symbols)?));
        }
        let n = expression(text, symbols, &s.section, s.offset)?;
        if n.section.is_some() {
            return Err("storage size/subpool must be absolute or register notation".into());
        }
        Ok(StorageOperand::Constant(u32::try_from(n.value).map_err(
            |_| "storage operand must fit unsigned 32 bits",
        )?))
    };
    let mut request = StorageService {
        obtain,
        conditional: mode == "RC",
        length: None,
        address: StorageOperand::Register(1),
        subpool: StorageOperand::Constant(0),
        below_16m: false,
        page: false,
        packed_length: false,
        return_code: None,
    };
    let mut seen = HashSet::new();
    for arg in &args[1..] {
        let (key, value) = arg
            .split_once('=')
            .ok_or("storage service keyword=value required")?;
        let key = key.to_ascii_uppercase();
        if !seen.insert(key.clone()) {
            return Err(format!("duplicate storage service keyword {key}"));
        }
        match key.as_str() {
            "LENGTH" if storage => request.length = Some(operand(value)?),
            "LV" if !storage => request.length = Some(operand(value)?),
            "SP" => request.subpool = operand(value)?,
            "ADDR" if storage => {
                request.address = if value.starts_with('(') {
                    operand(value)?
                } else if obtain {
                    StorageOperand::Memory(address(value, s, symbols, using, false)?.0)
                } else {
                    StorageOperand::Address(address(value, s, symbols, using, false)?.0)
                }
            }
            "A" if !storage && !obtain => {
                request.address = if value.starts_with('(') {
                    operand(value)?
                } else {
                    StorageOperand::Address(address(value, s, symbols, using, false)?.0)
                }
            }
            "COND" if storage => {
                request.conditional = match value.to_ascii_uppercase().as_str() {
                    "YES" => true,
                    "NO" => false,
                    _ => return Err("COND must be YES or NO".into()),
                }
            }
            "LOC" if obtain => {
                request.below_16m = match value.to_ascii_uppercase().as_str() {
                    "BELOW" | "24" => true,
                    "ANY" | "31" => false,
                    _ => return Err("LOC must be BELOW/24 or ANY/31".into()),
                }
            }
            "BNDRY" if obtain => {
                request.page = match value.to_ascii_uppercase().as_str() {
                    "PAGE" => true,
                    "DBLWD" => false,
                    _ => return Err("BNDRY must be PAGE or DBLWD".into()),
                }
            }
            "RTCD" if storage => {
                request.return_code = Some(if value.starts_with('(') {
                    operand(value)?
                } else {
                    StorageOperand::Memory(address(value, s, symbols, using, false)?.0)
                })
            }
            _ => return Err(format!("unsupported storage service keyword {key}")),
        }
    }
    if let Some(StorageOperand::Register(r)) = request.length
        && !(r == 0 || (2..=12).contains(&r))
    {
        return Err("storage length register must be 0 or 2..12".into());
    }
    if let StorageOperand::Register(r) = request.subpool
        && !((2..=12).contains(&r) || (storage && r == 15))
    {
        return Err("invalid storage subpool register".into());
    }
    if storage
        && let StorageOperand::Register(r) = request.address
        && !(1..=12).contains(&r)
    {
        return Err("STORAGE ADDR register must be 1..12".into());
    }
    if let Some(StorageOperand::Register(r)) = request.return_code
        && !((2..=12).contains(&r) || r == 15)
    {
        return Err("STORAGE RTCD register must be 2..12 or 15".into());
    }
    if request.length.is_none() {
        if mode == "R" {
            request.length = Some(StorageOperand::Register(0));
        } else if obtain
            || seen.contains("A")
            || seen.contains("ADDR")
            || (storage && !seen.contains("SP"))
        {
            return Err("storage length required".into());
        }
    }
    if storage && !obtain && request.length.is_some() && !seen.contains("ADDR") {
        return Err("STORAGE RELEASE with LENGTH requires ADDR".into());
    }
    if request.return_code.is_some() && !request.conditional {
        return Err("STORAGE RTCD requires COND=YES".into());
    }
    if !obtain && !storage && mode != "R" && request.length.is_some() && !seen.contains("A") {
        return Err("FREEMAIN with LV requires A".into());
    }
    request.packed_length = mode == "R"
        && !seen.contains("SP")
        && matches!(request.length, Some(StorageOperand::Register(0)));
    Ok(request)
}

struct AddressConstants {
    count: usize,
    values: Vec<String>,
    external: bool,
    width: u8,
    alignment: u32,
    wide: bool,
    kind: u8,
    long_offset: bool,
}

fn external_address_names(spec: &str) -> Result<Option<Vec<String>>, String> {
    let (clean, _, _) = super::constants::annotated(spec)?;
    let spec = clean.as_ref();
    let at = super::constants::type_start(spec)?;
    if !spec.as_bytes()[at].eq_ignore_ascii_case(&b'V') {
        return Ok(None);
    }
    let tail = &spec[at + 1..];
    let tail = tail.strip_prefix('D').unwrap_or(tail);
    let tail = if let Some(length) = tail.strip_prefix('L') {
        let end = if length.starts_with('(') {
            super::constants::parenthesized_end(length)?
        } else {
            length.bytes().take_while(u8::is_ascii_digit).count()
        };
        &length[end..]
    } else {
        tail
    };
    Ok(tail
        .strip_prefix('(')
        .and_then(|s| s.strip_suffix(')'))
        .map(operands))
}

fn address_constants(spec: &str, symbols: &Symbols) -> Result<Option<AddressConstants>, String> {
    let (clean, _, _) = super::constants::annotated(spec)?;
    let spec = clean.as_ref();
    let at = super::constants::type_start(spec)?;
    let kind = spec.as_bytes()[at].to_ascii_uppercase();
    if !matches!(kind, b'A' | b'V' | b'Y' | b'S' | b'Q' | b'J' | b'R') {
        return Ok(None);
    }
    let mut tail = &spec[at + 1..];
    let wide = (tail.starts_with('D') && !matches!(kind, b'Y' | b'S'))
        || (kind == b'S' && tail.starts_with('Y'));
    let long_offset = kind == b'Q' && tail.starts_with('Y');
    if wide || long_offset {
        tail = &tail[1..];
    }
    if tail.starts_with("L.") {
        return Ok(None);
    }
    let end = if let Some(modifier) = tail.strip_prefix('L') {
        1 + if modifier.starts_with('(') {
            super::constants::parenthesized_end(modifier)?
        } else {
            modifier.bytes().take_while(u8::is_ascii_digit).count()
        }
    } else {
        0
    };
    let explicit = super::constants::byte_length(&tail[..end], symbols)?;
    let implicit = if long_offset {
        3
    } else if kind == b'S' {
        if wide { 3 } else { 2 }
    } else if wide {
        8
    } else if kind == b'Y' {
        2
    } else {
        4
    };
    let width = explicit.unwrap_or(implicit);
    if (long_offset && width != 3)
        || (kind == b'Q' && !wide && !long_offset && width > 4)
        || (kind == b'J' && ![2, 3, 4, 8].contains(&width))
        || (kind == b'R' && !(width == 3 || width == 4 || (wide && width == 8)))
        || (kind == b'S' && width != implicit)
        || width == 0
        || width > if kind == b'Y' { 2 } else { 8 }
        || (kind == b'V' && !(width == 3 || width == 4 || (wide && width == 8)))
    {
        return Err("invalid address constant length".into());
    }
    let nominal = &tail[end..];
    if nominal.starts_with('\'') && kind == b'A' && !wide {
        return Ok(None);
    }
    let values = if nominal.is_empty() {
        vec![]
    } else {
        let inner = nominal
            .strip_prefix('(')
            .and_then(|s| s.strip_suffix(')'))
            .ok_or("address constant requires a parenthesized nominal")?;
        if inner.is_empty() {
            return Err("address constant nominal is empty".into());
        }
        operands(inner)
    };
    Ok(Some(AddressConstants {
        count: super::constants::duplication(spec, symbols)?.1 as usize,
        values,
        external: kind == b'V',
        width: width as u8,
        alignment: if explicit.is_some() {
            1
        } else if kind == b'S' || long_offset {
            2
        } else {
            implicit
        },
        wide,
        kind,
        long_offset,
    }))
}

pub(super) fn symbol_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 63
        && name
            .bytes()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || b"_@$#".contains(&c))
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_@$#".contains(&c))
}

/// Encode base/displacement constants using the bindings at the declaration or
/// at the literal-pool statement. A literal's location counter remains that of
/// the referring instruction for every duplicated nominal.
fn section_quantities(
    spec: &str,
    mut at: u32,
    symbols: &Symbols,
    bindings: &mut super::Bindings,
) -> Result<(), String> {
    let Some(c) = address_constants(spec, symbols)? else {
        return Ok(());
    };
    if !matches!(c.kind, b'Q' | b'J' | b'R') {
        return Ok(());
    }
    if c.count != 0 && c.values.is_empty() {
        return Err("DC Q/J/R constant requires a nominal value".into());
    }
    for _ in 0..c.count {
        for name in &c.values {
            if !symbol_name(name) {
                return Err("Q/J/R constant requires a section name".into());
            }
            bindings.quantities.push(super::Quantity {
                offset: at,
                width: c.width,
                kind: if c.kind == b'R' {
                    super::QuantityKind::Psect
                } else if c.kind == b'J' {
                    super::QuantityKind::Length
                } else if c.long_offset {
                    super::QuantityKind::LongOffset
                } else {
                    super::QuantityKind::Offset
                },
                symbol: name.to_ascii_uppercase(),
                element: None,
                addend: 0,
            });
            at += u32::from(c.width);
        }
    }
    Ok(())
}

fn fix_base_constants(
    spec: &str,
    mut at: u32,
    location: Option<u32>,
    symbols: &Symbols,
    using: &[Using],
    image: &mut [u8],
) -> Result<(), String> {
    let Some(c) = address_constants(spec, symbols)? else {
        return Ok(());
    };
    if c.kind != b'S' {
        return Ok(());
    }
    if c.count != 0 && c.values.is_empty() {
        return Err("DC S/SY constant requires a nominal value".into());
    }
    for _ in 0..c.count {
        for nominal in &c.values {
            let statement = Statement {
                label: String::new(),
                op: if c.wide { "LY" } else { "L" }.into(),
                args: String::new(),
                file: String::new(),
                line: 0,
                sequence: String::new(),
                section: String::new(),
                offset: location.unwrap_or(at),
            };
            let (a, _) = address(nominal, &statement, symbols, using, false)?;
            // An S nominal has only a base field, never an index field.
            if a.index != 0
                || nominal
                    .rfind('(')
                    .is_some_and(|i| operands(&nominal[i + 1..nominal.len() - 1]).len() > 1)
            {
                return Err("S/SY constant accepts displacement(base), without an index".into());
            }
            let d = a.displacement as u32;
            let end = at as usize + c.width as usize;
            let bytes = image
                .get_mut(at as usize..end)
                .ok_or("S/SY constant outside module storage")?;
            bytes[0] = a.base << 4 | ((d >> 8) & 15) as u8;
            bytes[1] = d as u8;
            if c.wide {
                bytes[2] = (d >> 12) as u8;
            }
            at += u32::from(c.width);
        }
    }
    Ok(())
}

fn fix_address_constants(
    spec: &str,
    mut at: u32,
    location: Option<u32>,
    symbols: &Symbols,
    externals: &HashMap<String, bool>,
    image: &mut [u8],
    relocations: &mut Vec<Relocation>,
) -> Result<(), String> {
    let Some(adcon) = address_constants(spec, symbols)? else {
        return Ok(());
    };
    let AddressConstants {
        count,
        values,
        external,
        width,
        wide,
        kind,
        ..
    } = adcon;
    if matches!(kind, b'S' | b'Q' | b'J' | b'R') {
        return Ok(());
    }
    if count != 0 && values.is_empty() {
        return Err("DC address constant requires a nominal value".into());
    }
    for _ in 0..count {
        for expr in &values {
            if external && !symbol_name(expr) {
                return Err("V constant requires an external symbol name".into());
            }
            let terms =
                super::expression::evaluate_terms(expr, symbols, "", location.unwrap_or(at))?;
            if !terms.relocations.is_empty()
                && !external
                && ((kind == b'Y' && width != 2)
                    || (kind == b'A' && !wide && !(2..=4).contains(&width))
                    || (wide && width < 2))
            {
                return Err("invalid relocatable address constant length".into());
            }
            let complex = !external
                && (terms.relocations.len() > 1 || terms.relocations.values().any(|n| *n != 1));
            if complex {
                let mut local_coefficient = 0;
                let mut external_terms = Vec::new();
                for (section, coefficient) in terms.relocations {
                    if section.is_empty() {
                        local_coefficient = coefficient;
                    } else if let Some(symbol) = section.strip_prefix("EXTERN:") {
                        external_terms.push(super::RelativeTerm {
                            symbol: symbol.into(),
                            coefficient,
                            weak: externals.get(symbol).copied().unwrap_or(false),
                        });
                    } else {
                        return Err("DSECT address constant is not relocatable storage".into());
                    }
                }
                relocations.push(Relocation {
                    offset: at,
                    width,
                    target: 0,
                    symbol: None,
                    addend: 0,
                    weak: false,
                    expression: Some(super::RelocationExpression {
                        addend: terms.number,
                        local_coefficient,
                        terms: external_terms,
                    }),
                });
                let end = (at as usize)
                    .checked_add(width as usize)
                    .ok_or("address constant offset overflow")?;
                image
                    .get_mut(at as usize..end)
                    .ok_or("address constant outside module storage")?
                    .fill(0);
                at += u32::from(width);
                continue;
            }
            let value = expression(expr, symbols, "", location.unwrap_or(at))?;
            let external_name = if external {
                Some(expr.to_ascii_uppercase())
            } else {
                value
                    .section
                    .as_deref()
                    .and_then(|s| s.strip_prefix("EXTERN:"))
                    .map(str::to_string)
            };
            let target = if let Some(symbol) = external_name {
                let weak = externals.get(&symbol).copied().unwrap_or(false);
                relocations.push(Relocation {
                    offset: at,
                    width,
                    target: 0,
                    symbol: Some(symbol),
                    addend: if external { 0 } else { value.value },
                    weak,
                    expression: None,
                });
                0
            } else {
                let target = if value.section.is_none() {
                    value.value as u32
                } else {
                    u32::try_from(value.value)
                        .or_else(|_| i32::try_from(value.value).map(|n| n as u32))
                        .map_err(|_| "address constant out of range")?
                };
                if value.section.as_deref() == Some("") {
                    relocations.push(Relocation {
                        offset: at,
                        width,
                        target: 0,
                        symbol: None,
                        addend: value.value,
                        weak: false,
                        expression: None,
                    });
                } else if value.section.is_some() {
                    return Err("DSECT address constant is not relocatable storage".into());
                }
                target
            };
            let end = (at as usize)
                .checked_add(width as usize)
                .ok_or("address constant offset overflow")?;
            image
                .get_mut(at as usize..end)
                .ok_or("address constant outside module storage")?
                .copy_from_slice(
                    &(if value.section.is_none() {
                        (if wide {
                            value.value
                        } else {
                            i64::from(value.value as i32)
                        }) as u64
                    } else if value.section.as_deref() == Some("") {
                        value.value as u64
                    } else {
                        u64::from(target)
                    })
                    .to_be_bytes()[8 - width as usize..],
                );
            at += u32::from(width);
        }
    }
    Ok(())
}

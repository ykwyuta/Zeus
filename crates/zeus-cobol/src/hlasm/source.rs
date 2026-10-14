//! Physical records and input controls. EBCDIC columns count original bytes,
//! including SO/SI and both bytes of a DBCS character.
use super::assemble::{Statement, attribute_quote, operands};
use super::{AssemblyOptions, Diagnostic};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub enum SourceEncoding {
    #[default]
    Utf8,
    /// Fixed-length EBCDIC records, without host newline translation.
    Ebcdic { ccsid: u32, record_length: usize },
}

impl std::str::FromStr for SourceEncoding {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        if value.eq_ignore_ascii_case("utf8") {
            return Ok(Self::Utf8);
        }
        let fields: Vec<_> = value.split(':').collect();
        if fields.len() == 3 && fields[0].eq_ignore_ascii_case("ebcdic") {
            let ccsid = fields[1].parse().map_err(|_| "invalid source CCSID")?;
            let record_length = fields[2]
                .parse()
                .map_err(|_| "invalid source record length")?;
            if crate::codepage::get(ccsid).is_none() {
                return Err(format!("unsupported source CCSID {ccsid}"));
            }
            if !(80..=32760).contains(&record_length) {
                return Err("source record length must be 80..32760".into());
            }
            return Ok(Self::Ebcdic {
                ccsid,
                record_length,
            });
        }
        Err("source encoding must be utf8 or ebcdic:CCSID:RECORD_LENGTH".into())
    }
}

/// Auto preserves host text with extended remarks; Fixed always observes the
/// physical continuation column. ICTL explicitly selects strict columns.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SourceFormat {
    #[default]
    Auto,
    Fixed,
}
impl std::str::FromStr for SourceFormat {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "auto" => Ok(Self::Auto),
            "fixed" => Ok(Self::Fixed),
            _ => Err("source format must be auto or fixed".into()),
        }
    }
}

fn extended_remark(record: &[char], begin: usize, end: usize) -> bool {
    // An alphabetic identification field is unambiguously fixed only when the
    // caller selects Fixed/ICTL. Auto treats a remark continuing through it as
    // an extended host comment, retaining the historic source acceptance.
    if !record
        .iter()
        .skip(end + 1)
        .take(80 - end - 1)
        .any(|c| !c.is_whitespace() && !c.is_ascii_digit())
    {
        return false;
    }
    let area = field(record, begin - 1, end);
    let mut rest = area.as_str();
    if !rest.starts_with(char::is_whitespace) {
        rest = rest.find(char::is_whitespace).map_or("", |at| &rest[at..]);
    }
    rest = rest.trim_start();
    rest = rest
        .find(char::is_whitespace)
        .map_or("", |at| rest[at..].trim_start());
    let mut quoted = false;
    let mut depth = 0;
    for (at, c) in rest.char_indices() {
        if c == '\'' && (quoted || !attribute_quote(rest, at)) {
            quoted = !quoted;
        }
        if !quoted {
            if c == '(' {
                depth += 1;
            } else if c == ')' {
                depth -= 1;
            }
            if c.is_whitespace() && depth == 0 {
                return !rest[at..].trim().is_empty();
            }
        }
    }
    false
}

fn records(path: &Path, encoding: &SourceEncoding) -> Result<Vec<Vec<char>>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    match encoding {
        SourceEncoding::Utf8 => {
            let text = std::str::from_utf8(&bytes)
                .map_err(|e| format!("{}: invalid UTF-8 source: {e}", path.display()))?;
            if text.contains('\0') {
                return Err(format!("{}: NUL in source", path.display()));
            }
            Ok(text
                .trim_start_matches('\u{feff}')
                .lines()
                .map(|s| s.chars().collect())
                .collect())
        }
        SourceEncoding::Ebcdic {
            ccsid,
            record_length,
        } => {
            let cp = crate::codepage::get(*ccsid)
                .ok_or_else(|| format!("unsupported source CCSID {ccsid}"))?;
            if !(80..=32760).contains(record_length) || !bytes.len().is_multiple_of(*record_length)
            {
                return Err(format!(
                    "{}: EBCDIC input must contain complete fixed records of length 80..32760",
                    path.display()
                ));
            }
            bytes
                .chunks(*record_length)
                .enumerate()
                .map(|(line, record)| {
                    let error = |s: &str| format!("{}:{}: {s}", path.display(), line + 1);
                    let mut columns = vec!['\0'; record.len()];
                    let (mut at, mut shifted) = (0, false);
                    while at < record.len() {
                        let b = record[at];
                        if cp.is_mixed() && b == crate::codepage::SO {
                            if shifted {
                                return Err(error("nested DBCS shift-out"));
                            }
                            shifted = true;
                            at += 1;
                        } else if cp.is_mixed() && b == crate::codepage::SI {
                            if !shifted {
                                return Err(error("DBCS shift-in without shift-out"));
                            }
                            shifted = false;
                            at += 1;
                        } else if shifted {
                            if at + 1 >= record.len() || matches!(record[at + 1], 0x0e | 0x0f) {
                                return Err(error("incomplete DBCS character"));
                            }
                            let decoded = cp.decode(&[0x0e, b, record[at + 1], 0x0f]);
                            let mut chars = decoded.chars();
                            let c = chars.next().ok_or_else(|| error("empty DBCS mapping"))?;
                            let second = chars.next();
                            if c == '\u{fffd}' || chars.next().is_some() {
                                return Err(error("unsupported DBCS source character mapping"));
                            }
                            columns[at] = c;
                            if let Some(second) = second {
                                columns[at + 1] = second;
                            }
                            at += 2;
                        } else {
                            let c = cp
                                .decode(&[b])
                                .chars()
                                .next()
                                .ok_or_else(|| error("empty source mapping"))?;
                            if matches!(c, '\0' | '\u{1a}' | '\u{fffd}' | '\n' | '\r') {
                                return Err(error("invalid character in fixed EBCDIC record"));
                            }
                            columns[at] = c;
                            at += 1;
                        }
                    }
                    if shifted {
                        return Err(error("DBCS shift-out crosses physical record boundary"));
                    }
                    Ok(columns)
                })
                .collect()
        }
    }
}

fn field(record: &[char], start: usize, end: usize) -> String {
    record
        .iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .filter(|c| **c != '\0')
        .collect()
}

fn parse(area: &str, path: &Path, line: usize, sequence: String) -> Statement {
    let mut rest = area;
    let label = if rest.starts_with(char::is_whitespace) {
        String::new()
    } else {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let label = rest[..end].to_ascii_uppercase();
        rest = &rest[end..];
        label
    };
    rest = rest.trim_start();
    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let op = rest[..end].to_ascii_uppercase();
    rest = rest[end..].trim_start();
    let (mut quoted, mut depth) = (false, 0);
    let end = rest
        .char_indices()
        .find_map(|(i, c)| {
            if c == '\'' && (quoted || !attribute_quote(rest, i)) {
                quoted = !quoted;
            }
            if !quoted {
                if c == '(' {
                    depth += 1;
                }
                if c == ')' {
                    depth -= 1;
                }
            }
            (c.is_whitespace() && !quoted && depth == 0).then_some(i)
        })
        .unwrap_or(rest.len());
    Statement {
        label,
        op,
        args: rest[..end].into(),
        file: path.display().to_string(),
        line,
        sequence,
        section: String::new(),
        offset: 0,
    }
}

#[derive(Default)]
struct Sequence {
    columns: Option<(usize, usize)>,
    previous: Option<Vec<u8>>,
}

impl Sequence {
    fn check(
        &mut self,
        record: &[char],
        path: &Path,
        line: usize,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let Some((left, right)) = self.columns else {
            return;
        };
        let mut value = field(record, left - 1, right);
        value.extend(std::iter::repeat_n(
            ' ',
            right - left + 1 - value.chars().count(),
        ));
        if value.chars().all(|c| c == ' ') {
            return;
        }
        let value = crate::ebcdic::encode(&value);
        if self
            .previous
            .as_ref()
            .is_some_and(|previous| value <= *previous)
        {
            diagnostics.push(Diagnostic {
                file: path.display().to_string(),
                line,
                operation: "ISEQ".into(),
                severity: 4,
                category: "SEQUENCE".into(),
                message: "source sequence field is not strictly increasing".into(),
            });
        }
        self.previous = Some(value);
    }
}

pub(super) fn statements(
    path: &Path,
    options: &AssemblyOptions,
    library: bool,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<Vec<Statement>, String> {
    let records = records(path, &options.source_encoding)?;
    let (mut begin, mut end, mut continuation) = (1, 71, Some(16));
    let (mut first, mut at) = (true, 0);
    let mut explicit_columns = false;
    let mut sequence = Sequence::default();
    let mut result = vec![];
    while let Some(raw) = records.get(at) {
        let line = at + 1;
        sequence.check(raw, path, line, diagnostics);
        let mut area = field(raw, begin - 1, end);
        at += 1;
        if area.trim().is_empty() || area.starts_with('*') || area.starts_with(".*") {
            continue;
        }
        let mut previous = raw;
        while end < 80
            && previous.get(end).is_some_and(|c| *c != ' ')
            && !(options.source_format == SourceFormat::Auto
                && !explicit_columns
                && (extended_remark(previous, begin, end)
                    || (matches!(options.source_encoding, SourceEncoding::Utf8)
                        && previous.len() > 80)))
        {
            let col = continuation.ok_or_else(|| {
                format!("{}:{line}: continuation disabled by ICTL", path.display())
            })?;
            let next = records
                .get(at)
                .ok_or_else(|| format!("{}:{line}: missing continuation record", path.display()))?;
            if next.len() < col - 1 || next.iter().take(col - 1).skip(begin - 1).any(|c| *c != ' ')
            {
                return Err(format!(
                    "{}:{}: continuation must start in column {col}",
                    path.display(),
                    at + 1
                ));
            }
            sequence.check(next, path, at + 1, diagnostics);
            if area
                .chars()
                .filter(|c| *c == '\'')
                .count()
                .is_multiple_of(2)
            {
                area.truncate(area.trim_end().len());
            }
            area.push_str(&field(next, col - 1, end));
            previous = next;
            at += 1;
        }
        let mut s = parse(&area, path, line, field(raw, 72, 80));
        if matches!(s.op.as_str(), "ICTL" | "ISEQ") && library {
            return Err(s.error("input controls are not allowed in COPY or library macro members"));
        }
        if s.op == "ICTL" {
            explicit_columns = true;
            if !first || !s.label.is_empty() {
                return Err(s.error("ICTL must be the first instruction and have no label"));
            }
            let args = operands(&s.args);
            if args.is_empty() || args.len() > 3 {
                return Err(s.error("ICTL begin[,end[,continue]] required"));
            }
            let number = |n: usize, default: usize| -> Result<usize, String> {
                match args.get(n).map(String::as_str) {
                    Some("") | None => Ok(default),
                    Some(value) if value.bytes().all(|c| c.is_ascii_digit()) => {
                        value.parse().map_err(|_| s.error("invalid ICTL column"))
                    }
                    _ => Err(s.error("ICTL columns must be decimal integers")),
                }
            };
            begin = number(0, 1)?;
            end = number(1, 71)?;
            let col = number(2, 0)?;
            if !(1..=40).contains(&begin)
                || !(41..=80).contains(&end)
                || end < begin + 5
                || (col != 0 && (!(2..=40).contains(&col) || col <= begin || col >= end))
            {
                return Err(s.error("invalid ICTL begin/end/continue range"));
            }
            continuation = (end != 80 && col != 0).then_some(col);
        } else if s.op == "ISEQ" {
            if !s.label.is_empty() && !s.label.starts_with('.') {
                return Err(s.error("ISEQ permits only a sequence-symbol label"));
            }
            sequence.previous = None;
            sequence.columns = if s.args.is_empty() {
                None
            } else {
                let args = operands(&s.args);
                let cols = args
                    .iter()
                    .map(|s| s.parse::<usize>())
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|_| s.error("ISEQ requires decimal columns"))?;
                if cols.len() != 2
                    || !(1..=80).contains(&cols[0])
                    || !(cols[0]..=80).contains(&cols[1])
                {
                    return Err(s.error("ISEQ left,right must be within 1..80"));
                }
                Some((cols[0], cols[1]))
            };
            if !s.label.is_empty() {
                s.op = "ANOP".into();
                s.args.clear();
                result.push(s);
            }
        } else if !s.op.is_empty() {
            result.push(s);
        }
        first = false;
    }
    Ok(result)
}

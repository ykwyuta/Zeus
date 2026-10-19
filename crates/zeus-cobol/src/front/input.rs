//! External source representation. Internal padding retains physical EBCDIC
//! byte columns until reference-format processing, and never reaches tokens.
use super::source::Pos;
use crate::diag::{Diag, Phase, Result};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SourceEncoding {
    #[default]
    Utf8,
    /// EBCDIC records separated by NL/LF or host LF/CRLF.
    Ebcdic,
    /// Concatenated EBCDIC records, exactly 80 bytes each.
    Ebcdic80,
}
impl SourceEncoding {
    /// The value of a `--source-encoding` option: utf8, ebcdic or ebcdic80.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "utf8" | "utf-8" => Some(Self::Utf8),
            "ebcdic" => Some(Self::Ebcdic),
            "ebcdic80" => Some(Self::Ebcdic80),
            _ => None,
        }
    }

    pub(crate) fn padded(self) -> bool {
        self != Self::Utf8
    }
}

pub(crate) const PAD: char = '\u{FDD0}';
pub(crate) fn clean(text: &str) -> String {
    text.chars().filter(|&ch| ch != PAD).collect()
}

pub(crate) fn decode(
    bytes: &[u8],
    encoding: SourceEncoding,
    ccsid: u32,
    file: u32,
    strict: bool,
) -> Result<String> {
    if encoding == SourceEncoding::Utf8 {
        return std::str::from_utf8(bytes)
            .map(|text| text.strip_prefix('\u{FEFF}').unwrap_or(text).to_owned())
            .map_err(|e| {
                Diag::error(
                    Pos { file, line: 1 },
                    format!("invalid UTF-8 source: {e}; select EBCDIC input explicitly"),
                )
                .during(Phase::Preprocess)
            });
    }
    let page = crate::codepage::get(ccsid).ok_or_else(|| {
        Diag::error(Pos { file, line: 1 }, "unsupported source CCSID").during(Phase::Options)
    })?;
    let records: Vec<_> = match encoding {
        SourceEncoding::Ebcdic80 => {
            if !bytes.len().is_multiple_of(80) {
                return Err(Diag::error(
                    Pos { file, line: 1 },
                    "EBCDIC80 source length must be a multiple of 80 bytes",
                ));
            }
            bytes.chunks(80).collect()
        }
        SourceEncoding::Ebcdic => {
            let mut records: Vec<_> = bytes.split(|b| matches!(b, 0x15 | 0x25 | 0x0A)).collect();
            if records.last().is_some_and(|r| r.is_empty()) {
                records.pop();
            }
            records
                .into_iter()
                .map(|r| r.strip_suffix(&[0x0D]).unwrap_or(r))
                .collect()
        }
        SourceEncoding::Utf8 => unreachable!(),
    };
    let mut text = String::new();
    for (line, record) in records.into_iter().enumerate() {
        let pos = Pos {
            file,
            line: line as u32 + 1,
        };
        // Fields are sliced as bytes, before decoding. A DBCS character
        // cannot straddle the sequence/indicator/program/id boundaries.
        for (start, end) in [(0, 6), (6, 7), (7, 72), (72, record.len())] {
            let part = &record[start.min(record.len())..end.min(record.len())];
            let chars = page
                .source_chars(part, strict && start == 7, start)
                .map_err(|message| {
                    Diag::error(
                        pos,
                        format!("invalid EBCDIC CCSID {ccsid} source: {message}"),
                    )
                    .during(Phase::Preprocess)
                })?;
            for (chars, width) in chars {
                let count = chars.len();
                if chars.contains(&PAD) || count > width {
                    return Err(Diag::error(pos, "unsupported source character mapping"));
                }
                text.extend(chars);
                text.extend(std::iter::repeat_n(PAD, width - count));
            }
        }
        text.push('\n');
    }
    Ok(text)
}

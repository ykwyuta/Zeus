//! Reference-format (fixed-form) source reading.
//!
//! Columns 1-6 hold a sequence number, column 7 the indicator, columns 8-72
//! the program text (area A: 8-11, area B: 12-72) and columns 73-80 an
//! identification field. Indicators: `*` and `/` comment lines, `-`
//! continuation, `D` debugging line (compiled only WITH DEBUGGING MODE).
//!
//! The result is a stream of characters, each tagged with its source
//! position, in which continuation lines are already joined.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    /// Index into the compilation's file table.
    pub file: u32,
    /// 1-based line number.
    pub line: u32,
}

impl fmt::Display for Pos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.line)
    }
}

/// Width of the program-text area (columns 8-72).
const AREA_WIDTH: usize = 65;

#[derive(Debug, Clone)]
struct Line {
    pos: Pos,
    indicator: char,
    /// Columns 8-72, padded with blanks to the full width.
    text: Vec<char>,
}

fn split_lines(text: &str, file: u32) -> Vec<Line> {
    text.lines()
        .enumerate()
        .map(|(i, raw)| {
            let chars: Vec<char> = raw.trim_end_matches('\r').chars().collect();
            let indicator = chars.get(6).copied().unwrap_or(' ');
            let mut area: Vec<char> = chars.iter().skip(7).take(AREA_WIDTH).copied().collect();
            // Tabs are not part of reference format; treat them as blanks.
            for c in &mut area {
                if *c == '\t' {
                    *c = ' ';
                }
            }
            area.resize(AREA_WIDTH, ' ');
            Line {
                pos: Pos {
                    file,
                    line: i as u32 + 1,
                },
                indicator,
                text: area,
            }
        })
        .collect()
}

/// Quote state after scanning `chars` starting in state `open`.
fn scan_quotes(mut open: Option<char>, chars: &[char]) -> Option<char> {
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match open {
            None => {
                if c == '"' || c == '\'' {
                    open = Some(c);
                }
            }
            Some(q) if c == q => {
                if chars.get(i + 1) == Some(&q) {
                    i += 1;
                } else {
                    open = None;
                }
            }
            Some(_) => {}
        }
        i += 1;
    }
    open
}

/// Identification-division paragraphs whose bodies are comment entries.
const COMMENT_PARAGRAPHS: [&str; 6] = [
    "AUTHOR",
    "INSTALLATION",
    "DATE-WRITTEN",
    "DATE-COMPILED",
    "SECURITY",
    "REMARKS",
];

/// If the line starts (in area A) with a comment-entry paragraph header,
/// returns the header length including its period.
fn comment_paragraph(text: &[char]) -> Option<usize> {
    if text[..4].iter().all(|c| *c == ' ') {
        return None;
    }
    let start = text.iter().position(|c| *c != ' ')?;
    let s: String = text[start..]
        .iter()
        .collect::<String>()
        .to_ascii_uppercase();
    COMMENT_PARAGRAPHS.iter().find_map(|p| {
        let rest = s.strip_prefix(p)?;
        let rest = rest.trim_start();
        rest.starts_with('.').then(|| {
            // header word + blanks + period
            let after_word = start + p.len();
            let dot = text[after_word..].iter().position(|c| *c == '.')? + after_word;
            Some(dot + 1)
        })?
    })
}

/// Joins the lines of a source file into a positioned character stream.
pub fn read(text: &str, file: u32, debugging_mode: bool) -> Vec<(char, Pos)> {
    let mut out: Vec<(char, Pos)> = Vec::new();
    let mut open: Option<char> = None;
    let mut in_comment_entry = false;
    let lines = split_lines(text, file);
    for (idx, line) in lines.iter().enumerate() {
        let ind = line.indicator;
        if ind == '*' || ind == '/' {
            continue;
        }
        if (ind == 'D' || ind == 'd') && !debugging_mode {
            continue;
        }
        let area_a_used = line.text[..4].iter().any(|c| *c != ' ');
        if in_comment_entry {
            if area_a_used && ind != '-' {
                in_comment_entry = false;
            } else {
                continue;
            }
        }
        let mut text = line.text.clone();
        let line = line.clone();
        if let Some(header) = comment_paragraph(&text) {
            for c in &mut text[header..] {
                *c = ' ';
            }
            in_comment_entry = true;
        }
        if ind == '-' {
            // Remove the line break added after the previous line.
            if out.last().is_some_and(|(c, _)| *c == '\n') {
                out.pop();
            }
            let first = text.iter().position(|c| *c != ' ').unwrap_or(text.len());
            let body: &[char] = if let Some(q) = open {
                // A continued literal resumes after the quote that starts
                // the continuation line.
                if text.get(first) == Some(&q) {
                    &text[first + 1..]
                } else {
                    &text[first..]
                }
            } else {
                // A continued word or number: join without blanks.
                while out.last().is_some_and(|(c, _)| *c == ' ') {
                    out.pop();
                }
                &text[first..]
            };
            let body = trim_unless_open(open, body);
            open = scan_quotes(open, body);
            out.extend(body.iter().map(|&c| (c, line.pos)));
        } else {
            let mut body = trim_unless_open(None, &text);
            open = scan_quotes(None, body);
            // A literal whose closing quote falls in column 72, followed by a
            // continuation line starting with a quote: the two quotes form a
            // doubled (embedded) quote and the literal goes on.
            if open.is_none()
                && let Some(&q) = text.last()
                && (q == '"' || q == '\'')
                && scan_quotes(None, &text[..text.len() - 1]) == Some(q)
                && continues_with_quote(&lines[idx + 1..], q)
            {
                body = &text;
                open = Some(q);
            }
            out.extend(body.iter().map(|&c| (c, line.pos)));
        }
        out.push(('\n', line.pos));
    }
    out
}

/// Whether the next non-comment line is a continuation line whose first
/// non-blank character is the quote `q`.
fn continues_with_quote(rest: &[Line], q: char) -> bool {
    rest.iter()
        .find(|l| l.indicator != '*' && l.indicator != '/')
        .is_some_and(|l| l.indicator == '-' && l.text.iter().find(|c| **c != ' ') == Some(&q))
}

/// Trailing blanks are significant only inside a literal continued on the
/// next line.
fn trim_unless_open(open: Option<char>, body: &[char]) -> &[char] {
    if scan_quotes(open, body).is_some() {
        body
    } else {
        let end = body.iter().rposition(|c| *c != ' ').map_or(0, |i| i + 1);
        &body[..end]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(stream: &[(char, Pos)]) -> String {
        stream.iter().map(|(c, _)| *c).collect()
    }

    fn card(ind: char, body: &str) -> String {
        format!("000100{ind}{body:<65}IDENT")
    }

    #[test]
    fn comments_and_debug_lines() {
        let src = [
            card(' ', "    MOVE A TO B."),
            card('*', "comment"),
            card('D', "    DISPLAY X."),
        ]
        .join("\n");
        assert_eq!(text(&read(&src, 0, false)), "    MOVE A TO B.\n");
        assert!(text(&read(&src, 0, true)).contains("DISPLAY X."));
    }

    #[test]
    fn continued_literal() {
        let src = [card(' ', "    VALUE \"ABC"), card('-', "    \"DEF\".")].join("\n");
        let t = text(&read(&src, 0, false));
        // The first line's literal runs to column 72.
        let expected = format!("    VALUE \"ABC{}DEF\".\n", " ".repeat(65 - 14));
        assert_eq!(t, expected);
    }

    #[test]
    fn continued_word() {
        let src = [card(' ', "    MOVE ABC"), card('-', "    DEF TO X.")].join("\n");
        assert_eq!(text(&read(&src, 0, false)), "    MOVE ABCDEF TO X.\n");
    }

    #[test]
    fn comment_entries() {
        let src = [
            card(' ', "AUTHOR. SOMEONE'S \"NAME"),
            card(' ', "    MORE TEXT."),
            card(' ', "ENVIRONMENT DIVISION."),
        ]
        .join("\n");
        assert_eq!(
            text(&read(&src, 0, false)),
            "AUTHOR.\nENVIRONMENT DIVISION.\n"
        );
    }

    #[test]
    fn quote_scanning() {
        let chars: Vec<char> = "\"A\"\"B".chars().collect();
        assert_eq!(scan_quotes(None, &chars), Some('"'));
        let chars: Vec<char> = "'X' Y".chars().collect();
        assert_eq!(scan_quotes(None, &chars), None);
    }
}

//! Parsing of the report files written by the CCVS85 test programs.
//!
//! Every test program prints a report (to the file assigned by X-card 55)
//! ending with summary lines such as
//!
//! ```text
//!     93 OF     93  TESTS WERE EXECUTED SUCCESSFULLY
//!  NO   TEST(S) FAILED
//!   3   TEST(S) DELETED
//! ```
//!
//! A few programs cannot check themselves and need their output inspected;
//! for those the report is examined in a program-specific way (`Style`).

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Counts {
    pub total: u32,
    pub pass: u32,
    pub fail: u32,
    pub deleted: u32,
    pub inspect: u32,
}

impl std::ops::AddAssign for Counts {
    fn add_assign(&mut self, o: Counts) {
        self.total += o.total;
        self.pass += o.pass;
        self.fail += o.fail;
        self.deleted += o.deleted;
        self.inspect += o.inspect;
    }
}

/// How a program's report is evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    /// Summary lines written by the program itself.
    Normal,
    /// NC107A: figurative constants are printed as `*** INFORMATION ***`
    /// lines whose 20-byte value must be checked.
    FigurativeConstants,
    /// NC113M: `MARGIN TESTING MAR-TEST-n` lines must appear in sequence.
    MarginSequence,
    /// NC121M / NC220M: the `*** INFORMATION ***` values must match the
    /// second and third lines the program DISPLAYed on standard output.
    DisplayMatch,
    /// NC135A: a table of numbers 1..300 must be printed in order.
    TableSequence,
}

/// Skips leading blanks.
fn ws(s: &[u8]) -> &[u8] {
    let n = s.iter().take_while(|&&c| c == b' ').count();
    &s[n..]
}

/// Takes a run of bytes satisfying `pred` (at least one).
fn token(s: &[u8], pred: impl Fn(u8) -> bool) -> Option<(&[u8], &[u8])> {
    let n = s.iter().take_while(|&&c| pred(c)).count();
    (n > 0).then(|| (&s[..n], &s[n..]))
}

fn number(s: &[u8]) -> Option<(u32, &[u8])> {
    let (digits, rest) = token(s, |c| c.is_ascii_digit())?;
    Some((std::str::from_utf8(digits).ok()?.parse().ok()?, rest))
}

/// `^ *n *OF *m *TESTS WERE`
fn tests_were(line: &[u8]) -> Option<(u32, u32)> {
    let (n, rest) = number(ws(line))?;
    let rest = ws(rest).strip_prefix(b"OF")?;
    let (m, rest) = number(ws(rest))?;
    ws(rest).starts_with(b"TESTS WERE").then_some((n, m))
}

/// `^ *(n|NO) *TEST(S) WORD`
fn tests_status(line: &[u8]) -> Option<(u32, &[u8])> {
    let (num, rest) = token(ws(line), |c| c.is_ascii_digit() || c == b'N' || c == b'O')?;
    let n = if num == b"NO" {
        0
    } else {
        std::str::from_utf8(num).ok()?.parse().ok()?
    };
    let rest = ws(rest).strip_prefix(b"TEST(S) ")?;
    let (word, _) = token(rest, |c| c.is_ascii_uppercase())?;
    Some((n, word))
}

/// `^ *n *TESTS REQUIRE VISUAL INSPECTION`
fn visual_inspection(line: &[u8]) -> Option<u32> {
    let (n, rest) = number(ws(line))?;
    ws(rest)
        .starts_with(b"TESTS REQUIRE VISUAL INSPECTION")
        .then_some(n)
}

const INFORMATION: &[u8] = b"*** INFORMATION ***";

fn lines(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    data.split(|&c| c == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
}

/// Applies the summary lines common to all styles. Returns true if the line
/// was a summary line.
fn summary_line(line: &[u8], c: &mut Counts, count_require: bool) -> bool {
    if let Some((n, m)) = tests_were(line) {
        c.total += m;
        c.pass += n;
    } else if let Some((n, word)) = tests_status(line) {
        match word {
            b"FAILED" => c.fail += n,
            b"DELETED" => c.deleted += n,
            b"REQUIRE" if count_require => c.inspect += n,
            _ => {}
        }
    } else {
        return false;
    }
    true
}

pub fn evaluate(style: Style, report: &[u8], stdout: &[u8]) -> Counts {
    let mut c = Counts::default();
    match style {
        Style::Normal => {
            for line in lines(report) {
                if !summary_line(line, &mut c, true)
                    && let Some(n) = visual_inspection(line)
                {
                    c.total += n;
                    c.inspect += n;
                }
            }
        }
        Style::FigurativeConstants => {
            for line in lines(report) {
                if summary_line(line, &mut c, false) {
                    continue;
                }
                // "*** INFORMATION ***" + 8 blanks + 20-byte value + 5 blanks + name
                let Some(rest) = line.strip_prefix(INFORMATION) else {
                    continue;
                };
                let Some(rest) = rest.strip_prefix(b"        ") else {
                    continue;
                };
                if rest.len() < 26 || &rest[20..25] != b"     " {
                    continue;
                }
                let value = &rest[..20];
                let Some((name, after)) =
                    token(&rest[25..], |c| c.is_ascii_uppercase() || c == b'-')
                else {
                    continue;
                };
                if !after.starts_with(b" ") {
                    continue;
                }
                let expected: [u8; 20] = match name {
                    b"ZERO" => *b" 000000000000000000 ",
                    b"SPACE" => [b' '; 20],
                    b"QUOTE" => [b'"'; 20],
                    b"HIGH-VALUE" => [0xff; 20],
                    b"LOW-VALUE" => [0x00; 20],
                    _ => {
                        c.fail += 1;
                        continue;
                    }
                };
                // An EBCDIC program prints HIGH-VALUE (X'FF') as X'9F' in
                // the ISO 8859-1 report.
                let ebcdic_high = name == b"HIGH-VALUE" && value.iter().all(|&b| b == 0x9f);
                if value == expected || ebcdic_high {
                    c.pass += 1;
                } else {
                    c.fail += 1;
                }
            }
        }
        Style::MarginSequence => {
            let mut seq = 0;
            for line in lines(report) {
                if let Some(rest) = line.strip_prefix(b" MARGIN TESTING") {
                    let rest = ws(rest);
                    if let Some(rest) = rest.strip_prefix(b"MAR-TEST-")
                        && let Some((n, after)) = number(rest)
                        && (after.is_empty() || after.starts_with(b" "))
                    {
                        seq += 1;
                        if seq == n {
                            c.pass += 1;
                        } else {
                            c.fail += 1;
                            seq = n;
                        }
                    }
                } else if let Some(n) = visual_inspection(line) {
                    c.total += n;
                }
            }
        }
        Style::DisplayMatch => {
            let mut shown = lines(stdout).skip(1).take(2);
            for line in lines(report) {
                if summary_line(line, &mut c, false) {
                    continue;
                }
                let Some(rest) = line.strip_prefix(INFORMATION) else {
                    continue;
                };
                let Some((value, after)) = token(ws(rest), |c| {
                    c.is_ascii_digit() || c.is_ascii_uppercase() || c == b'-'
                }) else {
                    continue;
                };
                if !after.starts_with(b" ") {
                    continue;
                }
                if shown.next() == Some(value) {
                    c.pass += 1;
                } else {
                    c.fail += 1;
                }
            }
        }
        Style::TableSequence => {
            let mut seq = 0;
            for line in lines(report) {
                if summary_line(line, &mut c, false) {
                    continue;
                }
                let Some(mut rest) = line.strip_prefix(b"   ") else {
                    continue;
                };
                if !rest.first().is_some_and(u8::is_ascii_digit) {
                    continue;
                }
                while let Some((n, after)) = number(ws(rest)) {
                    // Two blanks follow each number (trailing blanks may
                    // have been trimmed at the end of the line).
                    let after = if after.is_empty() {
                        after
                    } else if let Some(a) = after.strip_prefix(b"  ") {
                        a
                    } else {
                        break;
                    };
                    seq += 1;
                    if seq != n {
                        c.fail += 1;
                        break;
                    }
                    if seq == 300 {
                        c.pass += 1;
                    }
                    rest = after;
                }
            }
        }
    }
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_report() {
        let r = b"   93 OF    93  TESTS WERE EXECUTED SUCCESSFULLY\n NO   TEST(S) FAILED\n  3   TEST(S) DELETED\n  1   TEST(S) REQUIRE INSPECTION\n";
        let c = evaluate(Style::Normal, r, b"");
        assert_eq!(
            c,
            Counts {
                total: 93,
                pass: 93,
                fail: 0,
                deleted: 3,
                inspect: 1
            }
        );
    }

    #[test]
    fn visual_inspection_lines() {
        let c = evaluate(Style::Normal, b"  5 TESTS REQUIRE VISUAL INSPECTION\n", b"");
        assert_eq!((c.total, c.inspect), (5, 5));
    }

    #[test]
    fn figurative_constants() {
        let mut r = Vec::new();
        r.extend_from_slice(b"*** INFORMATION ***        ");
        r.extend_from_slice(&[0xff; 20]);
        r.extend_from_slice(b"     HIGH-VALUE X\n");
        r.extend_from_slice(b"*** INFORMATION ***        ");
        r.extend_from_slice(b"XXXXXXXXXXXXXXXXXXXX     SPACE X\n");
        let c = evaluate(Style::FigurativeConstants, &r, b"");
        assert_eq!((c.pass, c.fail), (1, 1));
    }

    #[test]
    fn display_match() {
        let r = b"*** INFORMATION *** ABC X\n*** INFORMATION *** DEF X\n";
        let c = evaluate(Style::DisplayMatch, r, b"header\nABC\nXYZ\n");
        assert_eq!((c.pass, c.fail), (1, 1));
    }

    #[test]
    fn table_sequence() {
        let mut r = String::new();
        for row in 0..30 {
            r.push_str("   ");
            for i in 1..=10 {
                r.push_str(&format!("{}  ", row * 10 + i));
            }
            r.push('\n');
        }
        let c = evaluate(Style::TableSequence, r.as_bytes(), b"");
        assert_eq!((c.pass, c.fail), (1, 0));
        let c = evaluate(Style::TableSequence, b"   1  3  \n", b"");
        assert_eq!((c.pass, c.fail), (0, 1));
    }

    #[test]
    fn margin_sequence() {
        let r = b" MARGIN TESTING   MAR-TEST-1 ok\n MARGIN TESTING   MAR-TEST-3 bad\n  2 TESTS REQUIRE VISUAL INSPECTION\n";
        let c = evaluate(Style::MarginSequence, r, b"");
        assert_eq!((c.total, c.pass, c.fail), (2, 1, 1));
    }
}

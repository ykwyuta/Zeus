//! Copybook text: the data description entries of a layout in fixed
//! format (indicator in column 7, entries in columns 8-72), with the
//! start position of each item in columns 73-80.

use zeus_cobol::front::ast::{DataEntry, Figurative, Literal, Operand, Usage, ValueClause};

/// Text of a literal or figurative constant.
pub fn operand(o: &Operand) -> String {
    match o {
        Operand::Lit(Literal::Num(n)) => n.clone(),
        Operand::Lit(Literal::Str(s)) => format!("'{}'", s.replace('\'', "''")),
        Operand::Lit(Literal::Hex(b)) => {
            format!(
                "X'{}'",
                b.iter().map(|x| format!("{x:02X}")).collect::<String>()
            )
        }
        Operand::Lit(Literal::National(b)) => {
            format!(
                "NX'{}'",
                b.iter().map(|x| format!("{x:02X}")).collect::<String>()
            )
        }
        Operand::Fig(f) => match f {
            Figurative::Zero => "ZERO".into(),
            Figurative::Space => "SPACE".into(),
            Figurative::HighValue => "HIGH-VALUE".into(),
            Figurative::LowValue => "LOW-VALUE".into(),
            Figurative::Quote => "QUOTE".into(),
            Figurative::Symbolic(s) => s.clone(),
        },
        Operand::All(o) => format!("ALL {}", operand(o)),
        Operand::Null => "NULL".into(),
        Operand::Ref(r) => r.name.clone(),
        Operand::LengthOf(r) => format!("LENGTH OF {}", r.name),
        Operand::AddressOf(r) => format!("ADDRESS OF {}", r.name),
        Operand::Func(_) => "?".into(),
    }
}

/// The clauses of an entry after its name (the REDEFINES clause only
/// when `redefines` is set; VALUE only when `values`).
pub fn clauses(e: &DataEntry, redefines: bool, values: bool) -> Vec<String> {
    let mut c = Vec::new();
    if redefines && let Some(r) = &e.redefines {
        c.push(format!("REDEFINES {r}"));
    }
    if let Some(p) = &e.picture {
        c.push(format!("PIC {p}"));
    }
    if e.group_national {
        c.push("GROUP-USAGE NATIONAL".into());
    }
    if let Some(u) = e.usage {
        let u = match u {
            Usage::Display => "",
            Usage::Display1 => "DISPLAY-1",
            Usage::National => "NATIONAL",
            Usage::Binary => "COMP",
            Usage::Packed => "COMP-3",
            Usage::NativeBinary => "COMP-5",
            Usage::Index => "INDEX",
            Usage::Pointer => "POINTER",
            Usage::ProcedurePointer => "PROCEDURE-POINTER",
            Usage::FunctionPointer => "FUNCTION-POINTER",
            Usage::Float => "COMP-1",
            Usage::Double => "COMP-2",
        };
        if !u.is_empty() {
            c.push(u.to_string());
        }
    }
    if let Some((leading, separate)) = e.sign {
        c.push(format!(
            "SIGN {}{}",
            if leading { "LEADING" } else { "TRAILING" },
            if separate { " SEPARATE" } else { "" }
        ));
    }
    if let Some(o) = &e.occurs {
        let mut s = if o.depending.is_some() || o.min != o.max {
            format!("OCCURS {} TO {} TIMES", o.min, o.max)
        } else {
            format!("OCCURS {} TIMES", o.max)
        };
        if let Some(d) = &o.depending {
            s.push_str(&format!(" DEPENDING ON {}", d.name));
        }
        for (asc, keys) in &o.keys {
            s.push_str(if *asc {
                " ASCENDING KEY"
            } else {
                " DESCENDING KEY"
            });
            for k in keys {
                s.push(' ');
                s.push_str(&k.name);
            }
        }
        if !o.indexed_by.is_empty() {
            s.push_str(" INDEXED BY ");
            s.push_str(&o.indexed_by.join(" "));
        }
        c.push(s);
    }
    if e.sync {
        c.push("SYNC".into());
    }
    if e.justified {
        c.push("JUST".into());
    }
    if e.blank_when_zero {
        c.push("BLANK WHEN ZERO".into());
    }
    if let Some(v) = &e.value {
        match v {
            ValueClause::Literal(o) if values => c.push(format!("VALUE {}", operand(o))),
            ValueClause::Condition(list) => {
                let vals: Vec<String> = list
                    .iter()
                    .map(|(a, b)| match b {
                        Some(b) => format!("{} THRU {}", operand(a), operand(b)),
                        None => operand(a),
                    })
                    .collect();
                c.push(format!("VALUE {}", vals.join(" ")));
            }
            _ => {}
        }
    }
    c
}

/// Lines of copybook text being built.
#[derive(Default)]
pub struct Text {
    pub lines: Vec<String>,
}

impl Text {
    /// A comment line.
    /// A comment line; a longer text goes on to more lines, indented as
    /// after the ` NAME      : ` heading.
    pub fn comment(&mut self, s: &str) {
        let mut rest = s.to_string();
        let mut first = true;
        while first || !rest.is_empty() {
            let prefix = if first {
                "      *"
            } else {
                "      *             "
            };
            let room = 72 - prefix.len();
            let mut cut = rest.chars().count().min(room);
            if cut < rest.chars().count() {
                // break at a space when there is one
                let head: String = rest.chars().take(room).collect();
                if let Some(sp) = head.rfind(' ').filter(|&i| i > room / 2) {
                    cut = head[..sp].chars().count();
                }
            }
            let head: String = rest.chars().take(cut).collect();
            rest = rest
                .chars()
                .skip(cut)
                .collect::<String>()
                .trim_start()
                .to_string();
            let mut l = format!("{prefix}{}", head.trim_end());
            truncate(&mut l, 72);
            self.lines.push(l);
            first = false;
        }
    }

    /// An entry: `level name clauses.` starting at column `col` (8 or
    /// more), wrapped at column 72; `pos` goes to columns 73-80 of its
    /// first line.
    pub fn entry(
        &mut self,
        col: usize,
        level: u8,
        name: &str,
        clauses: &[String],
        pos: Option<u32>,
    ) {
        let indent = " ".repeat(col - 1);
        let mut first = format!("{indent}{level:02}  {name}");
        let cont = " ".repeat((col + 3).max(12) - 1);
        let mut words: Vec<String> = Vec::new();
        for c in clauses {
            words.extend(split_words(c));
        }
        if let Some(w) = words.last_mut() {
            w.push('.');
        } else {
            first.push('.');
        }
        let mut lines = vec![first];
        let mut aligned = false;
        for w in words {
            // the clauses start at column 40 when the name leaves room
            if !aligned {
                aligned = true;
                let cur = lines.last_mut().unwrap();
                if cur.len() < 39 && 39 + w.len() <= 72 {
                    let pad = 39 - cur.len();
                    cur.push_str(&" ".repeat(pad));
                    cur.push_str(&w);
                    continue;
                }
            }
            let cur = lines.last_mut().unwrap();
            if cur.len() + 1 + w.len() <= 72 {
                cur.push(' ');
                cur.push_str(&w);
            } else if is_literal(&w) && cont.len() + w.len() > 72 {
                // a literal longer than a line: continuation lines
                continue_literal(&mut lines, &cont, &w);
            } else {
                lines.push(format!("{cont}{w}"));
            }
        }
        for (i, mut l) in lines.into_iter().enumerate() {
            if i == 0
                && let Some(p) = pos
            {
                l = format!("{l:<72}{}", seq(p));
            }
            self.lines.push(l);
        }
    }

    pub fn into_string(self) -> String {
        let mut s = self.lines.join("\n");
        s.push('\n');
        s
    }
}

/// Columns 73-80: the start position (1-based).
fn seq(p: u32) -> String {
    if p <= 99_999 {
        format!("{p:05}")
    } else {
        format!("{p:08}")
    }
}

fn truncate(s: &mut String, n: usize) {
    if s.chars().count() > n {
        *s = s.chars().take(n).collect();
    }
}

fn is_literal(w: &str) -> bool {
    let w = w.trim_end_matches('.');
    w.starts_with('\'') || w.starts_with("X'") || w.starts_with("NX'")
}

/// The words of a clause, keeping literals (with their spaces) whole.
fn split_words(c: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote = false;
    for ch in c.chars() {
        if ch == '\'' {
            quote = !quote;
        }
        if ch == ' ' && !quote {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Writes a long literal: the first part up to column 72, then lines
/// with `-` in column 7 resuming it after a quote in column 12. Each part
/// fills its line to column 72 (the literal's characters run to there).
fn continue_literal(lines: &mut Vec<String>, cont: &str, w: &str) {
    let (lit, suffix) = match w.strip_suffix('.') {
        Some(l) => (l, "."),
        None => (w, ""),
    };
    let chars: Vec<char> = lit.chars().collect();
    const PART: usize = 72 - 12;
    let mut used = lines.last().map_or(0, |l| l.chars().count());
    if 72usize.saturating_sub(used + 1) < 4 {
        lines.push(cont.to_string());
        used = cont.len();
    }
    let mut room = 72 - (used + 1);
    // a last part of only the closing quote would read as an escaped
    // quote: start the literal one column later
    if (chars.len() - room) % PART == 1 {
        room -= 1;
        lines.last_mut().unwrap().push(' ');
    }
    let cur = lines.last_mut().unwrap();
    cur.push(' ');
    cur.extend(&chars[..room]);
    let mut rest = &chars[room..];
    while !rest.is_empty() {
        let n = rest.len().min(PART);
        let mut l = String::from("      -    '");
        l.extend(&rest[..n]);
        rest = &rest[n..];
        lines.push(l);
    }
    if !suffix.is_empty() {
        let cur = lines.last_mut().unwrap();
        if cur.chars().count() < 72 {
            cur.push_str(suffix);
        } else {
            lines.push(format!("{cont}{suffix}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_and_positions() {
        let mut t = Text::default();
        t.entry(8, 1, "REC", &[], Some(1));
        t.entry(
            12,
            5,
            "REC-AMOUNT",
            &["PIC S9(7)V99".into(), "COMP-3".into()],
            Some(11),
        );
        let s = t.into_string();
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines[0], format!("{:<72}00001", "       01  REC."));
        assert_eq!(
            lines[1],
            format!(
                "{:<72}00011",
                format!("{:<39}PIC S9(7)V99 COMP-3.", "           05  REC-AMOUNT")
            )
        );
    }

    #[test]
    fn long_values_wrap() {
        let mut t = Text::default();
        let v = format!("VALUE '{}'", "A".repeat(90));
        t.entry(12, 5, "LONG-ITEM", &["PIC X(90)".into(), v], None);
        let s = t.into_string();
        assert!(s.lines().all(|l| l.len() <= 72), "{s}");
        assert!(s.lines().any(|l| l.starts_with("      -    'A")), "{s}");
        // every continued part fills its line; the literal ends with a
        // quote and the period
        let lines: Vec<&str> = s.lines().collect();
        let n = lines.len();
        for l in &lines[..n - 1] {
            assert_eq!(l.len(), 72, "{s}");
        }
        assert!(lines[n - 1].ends_with("'."), "{s}");
        let text: String = lines
            .iter()
            .enumerate()
            .map(|(k, l)| {
                if k == 0 {
                    l.split_once('\'').unwrap().1.to_string()
                } else {
                    l[12..].to_string()
                }
            })
            .collect();
        assert_eq!(text, format!("{}'.", "A".repeat(90)));
    }

    #[test]
    fn literal_continuation_at_every_length() {
        for n in 40..200 {
            let mut t = Text::default();
            let v = format!("VALUE '{}'", "B".repeat(n));
            t.entry(16, 88, "COND-NAME", &[v], None);
            let s = t.into_string();
            let lines: Vec<&str> = s.lines().collect();
            let last = lines.len() - 1;
            let mut text = String::new();
            for (k, l) in lines.iter().enumerate() {
                assert!(l.len() <= 72, "{n}: {s}");
                if k < last && lines[k + 1].starts_with("      -") {
                    assert_eq!(l.len(), 72, "{n}: {s}");
                }
                if l.starts_with("      -") {
                    // more than the closing quote on a continuation line
                    assert!(l[12..].trim_end_matches('.').len() >= 2, "{n}: {s}");
                    text.push_str(&l[12..]);
                } else if let Some((_, lit)) = l.split_once('\'') {
                    text.push_str(lit);
                }
            }
            let text = text.trim_end_matches(['.', ' ']);
            assert_eq!(text, format!("{}'", "B".repeat(n)), "{n}: {s}");
        }
    }
}

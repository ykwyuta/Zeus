//! COBOL sources (fixed format): COPY, CALL, EXEC CICS / SQL / DLI,
//! PROGRAM-ID and ENTRY, and the constants put in variables (VALUE and
//! MOVE literal TO) that name called programs.

use super::{Ref, Scanned, Tok, TokKind, Values, add_value, exec_refs, targets};

/// The tokens of the program text (columns 8-72; comment lines skipped).
pub fn tokens(text: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line: Vec<char> = raw.trim_end_matches('\r').chars().collect();
        if line.len() < 7 {
            continue;
        }
        if matches!(line[6], '*' | '/') {
            continue;
        }
        let end = line.len().min(72);
        let area = &line[7.min(end)..end];
        let mut k = 0;
        let mut picture = false;
        while k < area.len() {
            let c = area[k];
            let col = k + 8;
            if c.is_whitespace() {
                k += 1;
                continue;
            }
            // a PICTURE string: up to the next blank
            if picture {
                let start = k;
                while k < area.len() && !area[k].is_whitespace() {
                    k += 1;
                }
                let mut s: String = area[start..k].iter().collect();
                // PIC IS X(10)
                if s.eq_ignore_ascii_case("IS") {
                    out.push(Tok {
                        text: "IS".into(),
                        kind: TokKind::Word,
                        line: i + 1,
                        col,
                    });
                    continue;
                }
                let period = s.ends_with('.');
                if period {
                    s.pop();
                }
                out.push(Tok {
                    text: s.to_ascii_uppercase(),
                    kind: TokKind::Word,
                    line: i + 1,
                    col,
                });
                if period {
                    out.push(Tok {
                        text: ".".into(),
                        kind: TokKind::Punct,
                        line: i + 1,
                        col: col + k - start - 1,
                    });
                }
                picture = false;
                continue;
            }
            if c == '\'' || c == '"' {
                let mut s = String::new();
                k += 1;
                while k < area.len() {
                    if area[k] == c {
                        if area.get(k + 1) == Some(&c) {
                            s.push(c);
                            k += 2;
                            continue;
                        }
                        k += 1;
                        break;
                    }
                    s.push(area[k]);
                    k += 1;
                }
                // X'..' / N'..' prefixes were read as words before
                out.push(Tok {
                    text: s,
                    kind: TokKind::Lit,
                    line: i + 1,
                    col,
                });
                continue;
            }
            if c.is_alphanumeric() || c == '-' || c == '_' {
                let start = k;
                while k < area.len()
                    && (area[k].is_alphanumeric() || area[k] == '-' || area[k] == '_')
                {
                    k += 1;
                }
                let w: String = area[start..k]
                    .iter()
                    .collect::<String>()
                    .to_ascii_uppercase();
                // a literal prefix (X'...', N'...', G'...')
                if k < area.len()
                    && matches!(area[k], '\'' | '"')
                    && matches!(w.as_str(), "X" | "N" | "G" | "NX" | "Z")
                {
                    continue;
                }
                picture = matches!(w.as_str(), "PIC" | "PICTURE");
                out.push(Tok {
                    text: w,
                    kind: TokKind::Word,
                    line: i + 1,
                    col,
                });
                continue;
            }
            out.push(Tok {
                text: c.to_string(),
                kind: TokKind::Punct,
                line: i + 1,
                col,
            });
            k += 1;
        }
    }
    out
}

pub fn scan(text: &str) -> Scanned {
    let toks = tokens(text);
    let mut s = Scanned::default();
    // the constants of variables: VALUE clauses and MOVE literal TO
    let mut values = Values::new();
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        // level number, name ... VALUE literal
        if t.kind == TokKind::Word
            && t.text.len() <= 2
            && t.text.chars().all(|c| c.is_ascii_digit())
            && toks.get(i + 1).is_some_and(|n| n.kind == TokKind::Word)
        {
            let name = toks[i + 1].text.clone();
            let mut j = i + 2;
            while j < toks.len() && !toks[j].is_punct(".") {
                if (toks[j].is("VALUE") || toks[j].is("VALUES")) && j + 1 < toks.len() {
                    let mut v = j + 1;
                    if toks[v].is("IS") || toks[v].is("ARE") {
                        v += 1;
                    }
                    if toks.get(v).is_some_and(|x| x.kind == TokKind::Lit) {
                        add_value(&mut values, &name, &toks[v].text);
                    }
                    break;
                }
                j += 1;
            }
        }
        // MOVE variable TO variable
        if t.is("MOVE")
            && toks
                .get(i + 1)
                .is_some_and(|x| x.kind == TokKind::Word && !is_verb(&x.text))
            && toks.get(i + 2).is_some_and(|x| x.is("TO"))
            && let Some(dst) = toks.get(i + 3).filter(|x| x.kind == TokKind::Word)
        {
            s.moves.push((toks[i + 1].text.clone(), dst.text.clone()));
        }
        if t.is("MOVE")
            && toks.get(i + 1).is_some_and(|x| x.kind == TokKind::Lit)
            && toks.get(i + 2).is_some_and(|x| x.is("TO"))
        {
            let mut j = i + 3;
            while j < toks.len() && toks[j].kind == TokKind::Word && !is_verb(&toks[j].text) {
                add_value(&mut values, &toks[j].text, &toks[i + 1].text);
                // MOVE 'X' TO A OF B: only A
                if toks.get(j + 1).is_some_and(|x| x.is("OF") || x.is("IN")) {
                    j += 2;
                }
                j += 1;
                if toks.get(j).is_some_and(|x| x.is_punct(",")) {
                    j += 1;
                }
            }
        }
        i += 1;
    }
    let mut programs: Vec<String> = Vec::new();
    let mut entries: Vec<String> = Vec::new();
    let (mut cics, mut sql, mut dli) = (0, 0, 0);
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        if t.is("EXEC") || t.is("EXECUTE") {
            let kind = toks.get(i + 1).map(|k| k.text.clone()).unwrap_or_default();
            let start = i + 2;
            let mut j = start;
            while j < toks.len() && !toks[j].is("END-EXEC") {
                j += 1;
            }
            match kind.as_str() {
                "CICS" => cics += 1,
                "SQL" => sql += 1,
                "DLI" => dli += 1,
                _ => {}
            }
            exec_refs(&kind, &toks[start.min(j)..j], &values, &mut s.refs);
            i = j + 1;
            continue;
        }
        if t.is("COPY")
            && let Some(name) = toks.get(i + 1).filter(|n| n.kind != TokKind::Punct)
        {
            let member = std::path::Path::new(&name.text)
                .file_stem()
                .map(|x| x.to_string_lossy().to_ascii_uppercase())
                .unwrap_or_default();
            let mut r = Ref::new("COPY", &member, name.line, name.col);
            let mut j = i + 2;
            if toks.get(j).is_some_and(|x| x.is("OF") || x.is("IN"))
                && let Some(lib) = toks.get(j + 1)
            {
                r = r.attr("library", lib.text.clone());
                j += 2;
            }
            let mut replacing = false;
            while j < toks.len() && !toks[j].is_punct(".") {
                if toks[j].is("REPLACING") {
                    replacing = true;
                }
                j += 1;
            }
            r = r.attr("replacing", if replacing { "Y" } else { "N" });
            s.refs.push(r);
            i = j;
            continue;
        }
        if t.is("CALL")
            && let Some(target) = toks.get(i + 1)
        {
            targets("CALL", target, &values, 8, &mut s.refs, &[]);
        }
        if t.is("PROGRAM-ID") {
            let mut j = i + 1;
            if toks.get(j).is_some_and(|x| x.is_punct(".")) {
                j += 1;
            }
            if let Some(n) = toks.get(j).filter(|n| n.kind != TokKind::Punct) {
                programs.push(n.text.trim().to_ascii_uppercase());
            }
        }
        if t.is("ENTRY")
            && let Some(n) = toks.get(i + 1).filter(|n| n.kind == TokKind::Lit)
        {
            entries.push(n.text.trim().to_ascii_uppercase());
        }
        i += 1;
    }
    s.values = values;
    if let Some(p) = programs.first() {
        s.name = Some(p.clone());
        s.attrs.push(("program_id", p.clone()));
        s.attrs.push(("programs", programs.join(" ")));
    }
    if !entries.is_empty() {
        s.attrs.push(("entries", entries.join(" ")));
    }
    s.aliases = programs
        .iter()
        .skip(1)
        .chain(entries.iter())
        .cloned()
        .collect();
    s.attrs.push(("exec_cics", cics.to_string()));
    s.attrs.push(("exec_sql", sql.to_string()));
    s.attrs.push(("exec_dli", dli.to_string()));
    s
}

/// Words that start a statement (the end of a MOVE's receivers).
fn is_verb(w: &str) -> bool {
    matches!(
        w,
        "MOVE"
            | "IF"
            | "ELSE"
            | "END-IF"
            | "PERFORM"
            | "CALL"
            | "EXEC"
            | "GO"
            | "GOBACK"
            | "DISPLAY"
            | "ADD"
            | "SUBTRACT"
            | "COMPUTE"
            | "SET"
            | "EVALUATE"
            | "WHEN"
            | "END-EVALUATE"
            | "INITIALIZE"
            | "STRING"
            | "READ"
            | "WRITE"
            | "STOP"
            | "CONTINUE"
            | "OPEN"
            | "CLOSE"
            | "ACCEPT"
            | "INSPECT"
            | "UNSTRING"
            | "MULTIPLY"
            | "DIVIDE"
            | "EXIT"
            | "NOT"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_with_positions() {
        let src = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAINP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-PGM  PIC X(8) VALUE 'SUBA'.
       01 WS-NEXT PIC X(8).
      * COPY NOTME.
       COPY CUSTREC REPLACING ==:X:== BY ==Y==.
       PROCEDURE DIVISION.
           CALL 'SUBB' USING WS-PGM
           CALL WS-PGM
           MOVE 'COMEN01C' TO WS-NEXT
           EXEC CICS XCTL PROGRAM(WS-NEXT) END-EXEC
           EXEC CICS SEND MAP('M1') MAPSET('MS1') END-EXEC
           EXEC CICS RETURN TRANSID('CM00') END-EXEC
           EXEC SQL INCLUDE SQLCA END-EXEC
           CALL WS-UNKNOWN
           GOBACK.
";
        let s = scan(src);
        assert_eq!(s.name.as_deref(), Some("MAINP"));
        let refs: Vec<(&str, &str, usize, usize, bool)> = s
            .refs
            .iter()
            .map(|r| (r.edge, r.target.as_str(), r.line, r.col, r.dynamic))
            .collect();
        assert_eq!(
            refs,
            [
                ("COPY", "CUSTREC", 8, 13, false),
                ("CALL", "SUBB", 10, 17, false),
                ("CALL", "SUBA", 11, 17, false),
                ("CICS_XCTL", "COMEN01C", 13, 35, false),
                ("CICS_MAP", "MS1", 14, 44, false),
                ("CICS_TRANSID", "CM00", 15, 37, false),
                ("SQL_INCLUDE", "SQLCA", 16, 29, false),
                ("CALL", "WS-UNKNOWN", 17, 17, true),
            ]
        );
        assert!(s.refs[0].attrs.contains(&("replacing", "Y".into())));
        assert!(s.refs[2].attrs.contains(&("identifier", "WS-PGM".into())));
    }
}

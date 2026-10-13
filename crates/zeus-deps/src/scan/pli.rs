//! PL/I sources (margins 2-72): %INCLUDE, CALL and FETCH of external
//! procedures, EXEC CICS / SQL / DLI, the procedures, and the constants
//! put in variables (INITIAL and assignment) that name called programs.

use std::collections::HashSet;

use super::{Ref, Scanned, Tok, TokKind, Values, add_value, exec_refs};

pub fn tokens(text: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut comment = false;
    for (i, raw) in text.lines().enumerate() {
        let line: Vec<char> = raw.trim_end_matches('\r').chars().collect();
        // margins 2-72
        let end = line.len().min(72);
        let mut k = 1.min(end);
        while k < end {
            let c = line[k];
            let col = k + 1;
            if comment {
                if c == '*' && line.get(k + 1) == Some(&'/') && k + 1 < end {
                    comment = false;
                    k += 2;
                } else {
                    k += 1;
                }
                continue;
            }
            if c == '/' && line.get(k + 1) == Some(&'*') && k + 1 < end {
                comment = true;
                k += 2;
                continue;
            }
            if c.is_whitespace() {
                k += 1;
                continue;
            }
            if c == '\'' || c == '"' {
                let mut s = String::new();
                k += 1;
                while k < end {
                    if line[k] == c {
                        if line.get(k + 1) == Some(&c) && k + 1 < end {
                            s.push(c);
                            k += 2;
                            continue;
                        }
                        k += 1;
                        break;
                    }
                    s.push(line[k]);
                    k += 1;
                }
                out.push(Tok {
                    text: s,
                    kind: TokKind::Lit,
                    line: i + 1,
                    col,
                });
                continue;
            }
            if c.is_alphanumeric() || "_#@$".contains(c) {
                let start = k;
                while k < end && (line[k].is_alphanumeric() || "_#@$".contains(line[k])) {
                    k += 1;
                }
                out.push(Tok {
                    text: line[start..k]
                        .iter()
                        .collect::<String>()
                        .to_ascii_uppercase(),
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

/// The end of the statement starting at `i` (the index of its `;`).
fn stmt_end(toks: &[Tok], mut i: usize) -> usize {
    while i < toks.len() && !toks[i].is_punct(";") {
        i += 1;
    }
    i
}

pub fn scan(text: &str) -> Scanned {
    let toks = tokens(text);
    let mut s = Scanned::default();
    let mut values = Values::new();
    let mut procs: Vec<String> = Vec::new();
    let mut main = false;
    // procedures, INITIAL constants and assignments of constants
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        if t.kind == TokKind::Word
            && toks.get(i + 1).is_some_and(|x| x.is_punct(":"))
            && toks
                .get(i + 2)
                .is_some_and(|x| x.is("PROC") || x.is("PROCEDURE"))
        {
            procs.push(t.text.clone());
            let e = stmt_end(&toks, i);
            if toks[i..e].iter().any(|x| x.is("MAIN")) && procs.len() == 1 {
                main = true;
            }
        }
        if (t.is("DCL") || t.is("DECLARE"))
            && toks.get(i + 1).is_some_and(|x| x.kind == TokKind::Word)
        {
            let e = stmt_end(&toks, i);
            let name = toks[i + 1].text.clone();
            for j in i + 2..e {
                if (toks[j].is("INIT") || toks[j].is("INITIAL"))
                    && toks.get(j + 1).is_some_and(|x| x.is_punct("("))
                    && let Some(l) = toks.get(j + 2).filter(|x| x.kind == TokKind::Lit)
                {
                    add_value(&mut values, &name, &l.text);
                }
            }
        }
        // name = 'LITERAL';
        if t.kind == TokKind::Word
            && toks.get(i + 1).is_some_and(|x| x.is_punct("="))
            && toks.get(i + 2).is_some_and(|x| x.kind == TokKind::Lit)
            && toks.get(i + 3).is_some_and(|x| x.is_punct(";"))
        {
            add_value(&mut values, &t.text, &toks[i + 2].text);
        }
        // name = name;
        if t.kind == TokKind::Word
            && toks.get(i + 1).is_some_and(|x| x.is_punct("="))
            && toks.get(i + 2).is_some_and(|x| x.kind == TokKind::Word)
            && toks.get(i + 3).is_some_and(|x| x.is_punct(";"))
        {
            s.moves.push((toks[i + 2].text.clone(), t.text.clone()));
        }
        i += 1;
    }
    let internal: HashSet<&String> = procs.iter().collect();
    let (mut cics, mut sql) = (0, 0);
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        if t.is("EXEC") {
            let kind = toks.get(i + 1).map(|k| k.text.clone()).unwrap_or_default();
            let e = stmt_end(&toks, i);
            match kind.as_str() {
                "CICS" => cics += 1,
                "SQL" => sql += 1,
                _ => {}
            }
            exec_refs(&kind, &toks[(i + 2).min(e)..e], &values, &mut s.refs);
            i = e + 1;
            continue;
        }
        if t.is_punct("%")
            && toks
                .get(i + 1)
                .is_some_and(|x| x.is("INCLUDE") || x.is("XINCLUDE"))
        {
            let e = stmt_end(&toks, i);
            let mut j = i + 2;
            while j < e {
                let x = &toks[j];
                if x.kind != TokKind::Punct {
                    // DD(member) or member
                    let (name, at) = if toks.get(j + 1).is_some_and(|p| p.is_punct("("))
                        && let Some(m) = toks.get(j + 2)
                    {
                        j += 3;
                        (m.text.clone(), m)
                    } else {
                        (x.text.clone(), x)
                    };
                    let member = std::path::Path::new(&name)
                        .file_stem()
                        .map(|n| n.to_string_lossy().to_ascii_uppercase())
                        .unwrap_or_default();
                    s.refs
                        .push(Ref::new("PLI_INCLUDE", &member, at.line, at.col));
                }
                j += 1;
            }
            i = e + 1;
            continue;
        }
        if (t.is("CALL") || t.is("FETCH"))
            && let Some(target) = toks.get(i + 1)
            && target.kind == TokKind::Word
            && !internal.contains(&target.text)
        {
            let edge = if t.is("CALL") { "CALL" } else { "PLI_FETCH" };
            s.refs
                .push(Ref::new(edge, &target.text, target.line, target.col));
        }
        i += 1;
    }
    s.values = values;
    if let Some(p) = procs.first() {
        s.name = Some(p.clone());
        s.attrs.push(("procedure", p.clone()));
        s.attrs.push(("procedures", procs.join(" ")));
        s.attrs
            .push(("main", if main { "Y" } else { "N" }.to_string()));
    }
    s.attrs.push(("exec_cics", cics.to_string()));
    s.attrs.push(("exec_sql", sql.to_string()));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_with_positions() {
        let src = " MAINP: PROC OPTIONS(MAIN);
   %INCLUDE CUSTINC;
   %INCLUDE SYSLIB(OTHER);
   /* CALL NOTME; */
   DCL SUBX ENTRY EXTERNAL;
   DCL PGM CHAR(8) INIT('LINKED');
   CALL SUBX(1);
   CALL INNER;
   FETCH DYN1;
   EXEC CICS LINK PROGRAM(PGM);
   EXEC SQL INCLUDE SQLCA;
 INNER: PROC;
 END INNER;
 END MAINP;
";
        let s = scan(src);
        assert_eq!(s.name.as_deref(), Some("MAINP"));
        assert!(s.attrs.contains(&("main", "Y".into())));
        let refs: Vec<(&str, &str, usize, usize)> = s
            .refs
            .iter()
            .map(|r| (r.edge, r.target.as_str(), r.line, r.col))
            .collect();
        assert_eq!(
            refs,
            [
                ("PLI_INCLUDE", "CUSTINC", 2, 13),
                ("PLI_INCLUDE", "OTHER", 3, 20),
                ("CALL", "SUBX", 7, 9),
                ("PLI_FETCH", "DYN1", 9, 10),
                ("CICS_LINK", "LINKED", 10, 27),
                ("SQL_INCLUDE", "SQLCA", 11, 21),
            ]
        );
    }
}

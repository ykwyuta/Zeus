//! EXEC SQL statements: host variables, cursor declarations and WHENEVER.
//! The statements run on the Db2 interface of the COBOL run time.

use super::*;
use crate::sema::expr::mk;

fn name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '#' | '@' | '$')
}

/// A host variable name (qualified) and its indicator variable's.
pub type HostNames = Vec<(Vec<String>, Option<Vec<String>>)>;

/// SQL text with host variables (`:a.b`, with `:ind` or `INDICATOR :ind`)
/// replaced by `?`: the text and the variable names.
pub fn host_variables(text: &str) -> (String, HostNames) {
    let c: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut host = Vec::new();
    let mut quote = None;
    let mut i = 0;
    let name_at = |i: &mut usize| -> Option<Vec<String>> {
        let mut parts = Vec::new();
        loop {
            let s = *i;
            while *i < c.len() && name_char(c[*i]) {
                *i += 1;
            }
            if *i == s {
                return None;
            }
            parts.push(c[s..*i].iter().collect::<String>().to_ascii_uppercase());
            if *i + 1 < c.len() && c[*i] == '.' && name_char(c[*i + 1]) {
                *i += 1;
                continue;
            }
            return Some(parts);
        }
    };
    while i < c.len() {
        let ch = c[i];
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                }
                out.push(ch);
                i += 1;
            }
            None if ch == '\'' || ch == '"' => {
                quote = Some(ch);
                out.push(ch);
                i += 1;
            }
            None if ch == ':' && c.get(i + 1).is_some_and(|x| x.is_ascii_alphabetic()) => {
                i += 1;
                let Some(var) = name_at(&mut i) else {
                    out.push(':');
                    continue;
                };
                let mut k = i;
                while k < c.len() && c[k] == ' ' {
                    k += 1;
                }
                let rest: String = c[k..c.len().min(k + 9)].iter().collect();
                let mut ind = None;
                if c.get(k) == Some(&':') {
                    let mut m = k + 1;
                    ind = name_at(&mut m);
                    i = m;
                } else if rest.eq_ignore_ascii_case("INDICATOR") {
                    let mut m = k + 9;
                    while m < c.len() && c[m] == ' ' {
                        m += 1;
                    }
                    if c.get(m) == Some(&':') {
                        m += 1;
                        ind = name_at(&mut m);
                        i = m;
                    }
                }
                host.push((var, ind));
                out.push('?');
            }
            None => {
                out.push(ch);
                i += 1;
            }
        }
    }
    (out, host)
}

/// Replaces the k-th `?` marker (outside literals) by `counts[k]`
/// markers.
fn expand_markers(sql: &str, counts: &[usize]) -> String {
    if counts.iter().all(|&c| c == 1) {
        return sql.to_string();
    }
    let mut out = String::with_capacity(sql.len());
    let mut q = false;
    let mut k = 0;
    for c in sql.chars() {
        match c {
            '\'' => {
                q = !q;
                out.push(c);
            }
            '?' if !q => {
                let n = counts.get(k).copied().unwrap_or(1);
                out.push_str(&vec!["?"; n].join(", "));
                k += 1;
            }
            _ => out.push(c),
        }
    }
    out
}

impl Compiler {
    fn host_ref(&mut self, names: &[String], pos: Pos) -> Result<Ref> {
        let r = ast::Ref {
            locator: None,
            parts: names.iter().map(|n| (n.clone(), Vec::new())).collect(),
            pos,
        };
        if self.resolve_var(names).is_none() {
            return Err(self.err(
                pos,
                format!("host variable {} is not declared", names.join(".")),
            ));
        }
        self.reference(&r, true)
    }

    /// The scalar members of a structure, in order.
    fn leaves(&self, v: VarId, out: &mut Vec<VarId>) {
        for &m in &self.m.vars[v as usize].members {
            if self.m.vars[m as usize].ty.is_some() {
                out.push(m);
            } else {
                self.leaves(m, out);
            }
        }
    }

    fn sql_host(&mut self, text: &str, pos: Pos) -> Result<(String, HostVars)> {
        let (sql, names) = host_variables(text);
        // an SQLDA (DESCRIBE, PREPARE INTO, USING DESCRIPTOR) is passed as
        // a whole; other structures stand for their members
        let first = sql
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        let whole = matches!(first.as_str(), "DESCRIBE" | "PREPARE")
            || sql
                .split_whitespace()
                .any(|w| w.eq_ignore_ascii_case("DESCRIPTOR"));
        let mut host = Vec::new();
        let mut counts = Vec::new();
        for (v, i) in names {
            let r = self.host_ref(&v, pos)?;
            if whole || self.m.vars[r.var as usize].ty.is_some() {
                let i = match i {
                    Some(i) => Some(self.host_ref(&i, pos)?),
                    None => None,
                };
                host.push((r, i));
                counts.push(1);
                continue;
            }
            // a structure: its members, with the elements of an
            // indicator array
            let mut leaves = Vec::new();
            self.leaves(r.var, &mut leaves);
            for (k, &leaf) in leaves.iter().enumerate() {
                let mut path = Vec::new();
                let mut x = leaf;
                while x != r.var {
                    path.push(self.m.vars[x as usize].name.clone());
                    match self.m.vars[x as usize].parent {
                        Some(p) => x = p,
                        None => break,
                    }
                }
                path.reverse();
                let mut full = v.clone();
                full.extend(path);
                let m = self.host_ref(&full, pos)?;
                let ind = match &i {
                    Some(i) => {
                        let mut parts: Vec<(String, Vec<Vec<ast::Expr>>)> =
                            i.iter().map(|n| (n.clone(), Vec::new())).collect();
                        if let Some(last) = parts.last_mut() {
                            last.1.push(vec![ast::Expr::Num {
                                text: (k + 1).to_string(),
                                binary: false,
                                pos,
                            }]);
                        }
                        Some(self.reference(
                            &ast::Ref {
                                locator: None,
                                parts,
                                pos,
                            },
                            false,
                        )?)
                    }
                    None => None,
                };
                host.push((m, ind));
            }
            counts.push(leaves.len());
        }
        Ok((expand_markers(&sql, &counts), host))
    }

    /// An EXEC statement.
    pub fn exec_stmt(&mut self, kind: &str, text: &str, pos: Pos) -> Result<Stmt> {
        if kind == "CICS" {
            return self.exec_cics(text, pos);
        }
        if kind != "SQL" {
            return Err(self.err(
                pos,
                format!("EXEC {kind} is not supported in PL/I programs"),
            ));
        }
        let words: Vec<String> = text
            .split_whitespace()
            .map(|w| w.to_ascii_uppercase())
            .collect();
        let w = |k: usize| words.get(k).map(String::as_str).unwrap_or("");
        match w(0) {
            "DECLARE" if words.iter().any(|x| x == "CURSOR") => {
                let (sql, host) = self.sql_host(text, pos)?;
                self.sql_cursors.insert(w(1).to_string(), (sql, host));
                return Ok(Stmt::Null);
            }
            "DECLARE" | "INCLUDE" | "BEGIN" | "END" => return Ok(Stmt::Null),
            "WHENEVER" => {
                let (k, rest) = match (w(1), w(2)) {
                    ("SQLERROR", _) => (0, 2),
                    ("SQLWARNING", _) => (1, 2),
                    ("NOT", "FOUND") => (2, 3),
                    _ => {
                        return Err(
                            self.err(pos, "WHENEVER SQLERROR, SQLWARNING or NOT FOUND expected")
                        );
                    }
                };
                self.whenever[k] = match (w(rest), w(rest + 1)) {
                    ("CONTINUE", _) => None,
                    ("GO", "TO") => Some(w(rest + 2).to_string()),
                    ("GOTO", _) => Some(w(rest + 1).to_string()),
                    _ => return Err(self.err(pos, "WHENEVER ... CONTINUE or GO TO label expected")),
                };
                return Ok(Stmt::Null);
            }
            _ => {}
        }
        let (sql, mut host) = self.sql_host(text, pos)?;
        let mut query = None;
        if w(0) == "OPEN" {
            let Some((q, h)) = self.sql_cursors.get(w(1)).cloned() else {
                return Err(self.err(pos, format!("cursor {} is not declared", w(1))));
            };
            let words: Vec<&str> = q.split_whitespace().collect();
            let at = words
                .iter()
                .position(|x| x.eq_ignore_ascii_case("FOR"))
                .unwrap_or(words.len());
            query = Some(words[(at + 1).min(words.len())..].join(" "));
            // the cursor's host variables, then OPEN's (USING)
            let mut all = h;
            all.extend(host);
            host = all;
        }
        let sqlca = if self.resolve_var(&["SQLCA".to_string()]).is_some() {
            Some(self.host_ref(&["SQLCA".to_string()], pos)?)
        } else {
            None
        };
        let mut whenever = Vec::new();
        for k in 0..3u8 {
            if let Some(l) = self.whenever[k as usize].clone() {
                match self.sym(&l) {
                    Some(Sym::Label(id)) => whenever.push((k, mk(Ek::Label(id), Ty::Label))),
                    _ => return Err(self.err(pos, format!("WHENEVER: {l} is not a label"))),
                }
            }
        }
        if !whenever.is_empty() && sqlca.is_none() {
            return Err(self.err(pos, "WHENEVER needs the SQLCA (EXEC SQL INCLUDE SQLCA)"));
        }
        Ok(Stmt::Exec(Box::new(ExecStmt {
            kind: kind.to_string(),
            text: sql,
            host,
            query,
            sqlca,
            whenever,
            command: String::new(),
            options: Vec::new(),
        })))
    }
}

impl Compiler {
    /// EXEC CICS command options: `NAME` or `NAME(argument)`; an argument
    /// is a variable (its storage), a label, a value or a name.
    fn exec_cics(&mut self, text: &str, pos: Pos) -> Result<Stmt> {
        let mut toks = crate::lexer::tokenize_raw(text, pos)?;
        toks.push(crate::lexer::Token {
            tok: crate::lexer::Tok::Eof,
            pos,
        });
        let word = |t: &crate::lexer::Token| match &t.tok {
            crate::lexer::Tok::Ident(s) => Some(s.clone()),
            _ => None,
        };
        let command = word(&toks[0]).ok_or_else(|| self.err(pos, "EXEC CICS command expected"))?;
        let mut options = Vec::new();
        let mut map = None;
        let mut i = 1;
        while !matches!(toks[i].tok, crate::lexer::Tok::Eof) {
            let name = word(&toks[i]).ok_or_else(|| {
                self.err(
                    toks[i].pos,
                    format!("EXEC CICS option expected, found {}", toks[i].tok),
                )
            })?;
            i += 1;
            let mut arg = None;
            let mut arg_len = 0;
            if matches!(toks[i].tok, crate::lexer::Tok::Op("(")) {
                let mut depth = 0;
                let start = i + 1;
                let mut end = None;
                for (k, t) in toks.iter().enumerate().skip(i) {
                    match t.tok {
                        crate::lexer::Tok::Op("(") => depth += 1,
                        crate::lexer::Tok::Op(")") => {
                            depth -= 1;
                            if depth == 0 {
                                end = Some(k);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let end =
                    end.ok_or_else(|| self.err(pos, "unbalanced parentheses in EXEC CICS"))?;
                let mut a = toks[start..end].to_vec();
                a.push(crate::lexer::Token {
                    tok: crate::lexer::Tok::Eof,
                    pos,
                });
                arg = Some(self.cics_operand(a, pos)?);
                arg_len = end + 1 - i;
                i = end + 1;
            }
            if name == "MAP"
                && arg_len > 0
                && let [t] = &toks[i + 1 - arg_len..i - 1]
                && let crate::lexer::Tok::Str { text, .. } = &t.tok
            {
                map = Some(text.trim().to_ascii_uppercase());
            }
            options.push((name, arg));
        }
        // BMS: SEND MAP without FROM and RECEIVE MAP without INTO / SET use
        // the symbolic map named after the map (mapO / mapI)
        let has = |n: &str| options.iter().any(|o| o.0 == n);
        let default = match command.as_str() {
            "SEND" if !has("FROM") && !has("MAPONLY") => Some(("FROM", 'O')),
            "RECEIVE" if !has("INTO") && !has("SET") => Some(("INTO", 'I')),
            _ => None,
        };
        if let (Some(map), Some((opt, suffix))) = (map, default) {
            let names = vec![format!("{map}{suffix}")];
            if self.resolve_var(&names).is_some() {
                let r = ast::Ref {
                    parts: vec![(names[0].clone(), Vec::new())],
                    locator: None,
                    pos,
                };
                options.push((
                    opt.to_string(),
                    Some(ExecOperand::Ref(self.reference(&r, true)?)),
                ));
            }
        }
        Ok(Stmt::Exec(Box::new(ExecStmt {
            kind: "CICS".into(),
            text: text.to_string(),
            host: Vec::new(),
            query: None,
            sqlca: None,
            whenever: Vec::new(),
            command,
            options,
        })))
    }

    fn cics_operand(&mut self, toks: Vec<crate::lexer::Token>, pos: Pos) -> Result<ExecOperand> {
        let mut p = crate::parser::Parser::new(toks);
        let e = p.expr()?;
        if !p.at_end() {
            return Err(self.err(pos, "EXEC CICS: an option argument is an expression"));
        }
        if let ast::Expr::Ref(r) = &e {
            let names: Vec<String> = r.parts.iter().map(|p| p.0.clone()).collect();
            if r.locator.is_none() && names.len() == 1 && r.parts[0].1.is_empty() {
                match self.sym(&names[0]) {
                    Some(Sym::Label(l)) => return Ok(ExecOperand::Label(l)),
                    None if self.resolve_var(&names).is_none() => {
                        return Ok(ExecOperand::Name(names[0].clone()));
                    }
                    _ => {}
                }
            }
            if self.resolve_var(&names).is_some()
                && matches!(self.sym(&names[0]), Some(Sym::Var(_)) | None)
            {
                return Ok(ExecOperand::Ref(self.reference(r, true)?));
            }
        }
        Ok(ExecOperand::Value(self.expr(&e)?))
    }
}

#[cfg(test)]
mod tests {
    use super::host_variables;

    #[test]
    fn host_variable_markers() {
        let (t, h) =
            host_variables("SELECT A INTO :X.Y :XI FROM T WHERE B = ':Z' AND C = :K INDICATOR :KI");
        assert_eq!(t, "SELECT A INTO ? FROM T WHERE B = ':Z' AND C = ?");
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].0, ["X", "Y"]);
        assert_eq!(h[0].1.as_deref(), Some(&["XI".to_string()][..]));
        assert_eq!(h[1].1.as_deref(), Some(&["KI".to_string()][..]));
    }
}

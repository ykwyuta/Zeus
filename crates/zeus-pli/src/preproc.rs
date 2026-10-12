//! The macro preprocessor: preprocessor statements (`%` statements) are
//! executed as the token stream is scanned, and active identifiers in the
//! program text are replaced by their values (rescanned under RESCAN).
//!
//! Statements: `%DECLARE` (FIXED, CHARACTER, ENTRY, BUILTIN; SCAN, RESCAN,
//! NOSCAN; INITIAL), assignment, `%IF ... %THEN ... %ELSE`, `%DO` (all
//! forms, including `%DO SKIP`), `%SELECT / %WHEN / %OTHERWISE`, `%END`,
//! `%LEAVE`, `%ITERATE`, `%ACTIVATE`, `%DEACTIVATE`, `%REPLACE`, `%NOTE`,
//! `%INCLUDE` / `%XINCLUDE` and preprocessor procedures
//! (`%name: PROCEDURE(params) RETURNS(CHAR | FIXED); ... %END;`), which
//! are invoked from the text with their arguments as character strings.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crate::ast::{self, DoSpec, Expr, StmtKind};
use crate::lexer::{Diag, Files, LexOptions, Pos, Result, StrKind, Tok, Token, tokens_text};
use crate::parser::Parser;

#[derive(Debug, Clone, PartialEq)]
enum Val {
    Fixed(i64),
    Char(String),
    Bit(bool),
}

impl Val {
    fn num(&self) -> i64 {
        match self {
            Val::Fixed(n) => *n,
            Val::Bit(b) => i64::from(*b),
            Val::Char(s) => s.trim().parse().unwrap_or(0),
        }
    }

    fn text(&self) -> String {
        match self {
            // FIXED converts to an 8-character string
            Val::Fixed(n) => format!("{n:>8}"),
            Val::Char(s) => s.clone(),
            Val::Bit(b) => (if *b { "1" } else { "0" }).to_string(),
        }
    }

    fn truth(&self) -> bool {
        match self {
            Val::Bit(b) => *b,
            Val::Fixed(n) => *n != 0,
            Val::Char(s) => s.contains('1'),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scan {
    Rescan,
    Once,
}

#[derive(Debug, Clone)]
struct Proc {
    params: Vec<String>,
    returns_char: bool,
    body: Vec<Token>,
}

/// Control flow out of statement execution.
enum Flow {
    Next,
    Leave(Option<String>),
    Iterate(Option<String>),
    Return(Val),
}

struct Pp<'a> {
    vars: HashMap<String, Val>,
    chars: HashSet<String>,
    procs: HashMap<String, Proc>,
    active: HashMap<String, Scan>,
    opts: &'a LexOptions,
    files: &'a mut Files,
    out: Vec<Token>,
    diags: Vec<Diag>,
    included: HashSet<PathBuf>,
    depth: usize,
    counter: i64,
}

fn is_op(t: &Token, o: &str) -> bool {
    matches!(&t.tok, Tok::Op(x) if *x == o)
}

fn is_word(t: &Token, w: &str) -> bool {
    matches!(&t.tok, Tok::Ident(x) if x == w)
}

fn word(t: &Token) -> Option<&str> {
    match &t.tok {
        Tok::Ident(x) => Some(x),
        _ => None,
    }
}

/// Index of the ';' ending the statement starting at `i` (outside
/// parentheses).
fn stmt_end(t: &[Token], mut i: usize, end: usize) -> usize {
    let mut depth = 0i32;
    while i < end {
        match &t[i].tok {
            Tok::Op("(") => depth += 1,
            Tok::Op(")") => depth -= 1,
            Tok::Op(";") if depth <= 0 => return i,
            _ => {}
        }
        i += 1;
    }
    end
}

/// The statement keyword at `i` (after `%` and labels) and its index.
fn keyword(t: &[Token], mut i: usize, end: usize) -> (Option<String>, usize, Vec<String>) {
    let mut labels = Vec::new();
    while i + 1 < end && word(&t[i]).is_some() && is_op(&t[i + 1], ":") {
        labels.push(word(&t[i]).unwrap().to_string());
        i += 2;
    }
    (word(&t[i.min(end.saturating_sub(1))]).map(str::to_string).filter(|_| i < end), i, labels)
}

/// The index after the `%END ...;` that closes the group whose body
/// starts at `i`; and the index of that `%`.
fn group_end(t: &[Token], mut i: usize, end: usize) -> Result<(usize, usize)> {
    let mut depth = 1;
    while i < end {
        if is_op(&t[i], "%") {
            let (kw, k, _) = keyword(t, i + 1, end);
            match kw.as_deref() {
                Some("DO" | "SELECT" | "PROCEDURE" | "PROC") => depth += 1,
                Some("END") => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok((stmt_end(t, k, end) + 1, i));
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    Err(Diag::error(t.get(i.saturating_sub(1)).map_or(Pos::default(), |x| x.pos), "%END expected"))
}

/// The index after the unit (a % statement or group) starting at `i`
/// (the `%`).
fn unit_end(t: &[Token], i: usize, end: usize) -> Result<usize> {
    if !is_op(&t[i], "%") {
        return Err(Diag::error(t[i].pos, "a preprocessor statement must follow %THEN / %ELSE"));
    }
    let (kw, k, _) = keyword(t, i + 1, end);
    match kw.as_deref() {
        Some("DO" | "SELECT") => Ok(group_end(t, stmt_end(t, k, end) + 1, end)?.0),
        Some("IF") => {
            let then = find_pct(t, k + 1, end, "THEN").ok_or_else(|| Diag::error(t[i].pos, "%THEN expected"))?;
            let mut j = unit_end(t, then + 2, end)?;
            if j + 1 < end && is_op(&t[j], "%") && is_word(&t[j + 1], "ELSE") {
                j = unit_end(t, j + 2, end)?;
            }
            Ok(j)
        }
        _ => Ok(stmt_end(t, k, end) + 1),
    }
}

/// Index of the `%` of `%word` at nesting depth 0 from `i`.
fn find_pct(t: &[Token], mut i: usize, end: usize, w: &str) -> Option<usize> {
    let mut depth = 0i32;
    while i + 1 < end {
        match &t[i].tok {
            Tok::Op("(") => depth += 1,
            Tok::Op(")") => depth -= 1,
            Tok::Op("%") if depth == 0 && is_word(&t[i + 1], w) => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

impl Pp<'_> {
    fn err(&self, pos: Pos, m: impl Into<String>) -> Diag {
        Diag::error(pos, m)
    }

    /// Executes tokens from `i` to `end`: % statements and text.
    fn run(&mut self, t: &[Token], mut i: usize, end: usize) -> Result<Flow> {
        while i < end {
            if is_op(&t[i], "%") {
                let (next, flow) = self.statement(t, i + 1, end)?;
                if !matches!(flow, Flow::Next) {
                    return Ok(flow);
                }
                i = next;
                continue;
            }
            i = self.text(t, i, end)?;
        }
        Ok(Flow::Next)
    }

    /// Emits a text token (replacing an active identifier); the next
    /// index.
    fn text(&mut self, t: &[Token], i: usize, end: usize) -> Result<usize> {
        let tok = &t[i];
        let Some(name) = word(tok) else {
            self.out.push(tok.clone());
            return Ok(i + 1);
        };
        let Some(scan) = self.active.get(name).copied() else {
            self.out.push(tok.clone());
            return Ok(i + 1);
        };
        let name = name.to_string();
        let (value, next) = if let Some(proc) = self.procs.get(&name).cloned() {
            // a procedure reference: arguments in parentheses
            let mut args = Vec::new();
            let mut j = i + 1;
            if j < end && is_op(&t[j], "(") {
                let mut depth = 0;
                let mut cur = Vec::new();
                j += 1;
                while j < end {
                    match &t[j].tok {
                        Tok::Op("(") => depth += 1,
                        Tok::Op(")") if depth == 0 => break,
                        Tok::Op(")") => depth -= 1,
                        Tok::Op(",") if depth == 0 => {
                            args.push(tokens_text(&cur));
                            cur.clear();
                            j += 1;
                            continue;
                        }
                        _ => {}
                    }
                    cur.push(t[j].clone());
                    j += 1;
                }
                args.push(tokens_text(&cur));
                j += 1;
            }
            let v = self.call(&proc, args.into_iter().map(Val::Char).collect(), tok.pos)?;
            (v, j)
        } else {
            (self.vars.get(&name).cloned().unwrap_or(Val::Char(String::new())), i + 1)
        };
        let toks = crate::lexer::tokenize_raw(&value.text(), tok.pos)?;
        if scan == Scan::Rescan {
            self.depth += 1;
            if self.depth > 100 {
                return Err(self.err(tok.pos, format!("replacement of {name} does not end")));
            }
            let n = toks.len();
            // the replaced name is not replaced again within its own value
            let saved = self.active.remove(&name);
            let r = self.run_text(&toks, n);
            if let Some(s) = saved {
                self.active.insert(name, s);
            }
            self.depth -= 1;
            r?;
        } else {
            self.out.extend(toks);
        }
        Ok(next)
    }

    fn run_text(&mut self, t: &[Token], end: usize) -> Result<()> {
        let mut i = 0;
        while i < end {
            i = self.text(t, i, end)?;
        }
        Ok(())
    }

    /// Parses an expression from tokens.
    fn parse_expr(&self, t: &[Token], pos: Pos) -> Result<Expr> {
        let mut v = t.to_vec();
        v.push(Token { tok: Tok::Eof, pos });
        let mut p = Parser::new(v);
        let e = p.expr()?;
        if !p.at_end() {
            return Err(self.err(pos, "invalid preprocessor expression"));
        }
        Ok(e)
    }

    /// Executes the % statement starting at `i` (after `%`).
    fn statement(&mut self, t: &[Token], i: usize, end: usize) -> Result<(usize, Flow)> {
        let (kw, k, labels) = keyword(t, i, end);
        let pos = t[i.min(end - 1)].pos;
        let se = stmt_end(t, k, end);
        if i < end && is_op(&t[i], ";") {
            return Ok((i + 1, Flow::Next));
        }
        let Some(kw) = kw else {
            return Err(self.err(pos, "preprocessor statement expected"));
        };
        // assignment
        if se > k + 1 && t[k + 1..se].iter().position(|x| is_op(x, "=")).is_some() && !matches!(kw.as_str(), "IF" | "DO" | "SELECT" | "DECLARE" | "DCL") {
            let eq = k + 1 + t[k + 1..se].iter().position(|x| is_op(x, "=")).unwrap();
            if eq == k + 1 {
                let e = self.parse_expr(&t[eq + 1..se], pos)?;
                let v = self.eval(&e, &mut HashMap::new())?;
                self.assign(&kw, v, &mut HashMap::new());
                return Ok((se + 1, Flow::Next));
            }
        }
        match kw.as_str() {
            "DECLARE" | "DCL" => {
                self.declare(&t[k + 1..se], pos)?;
                Ok((se + 1, Flow::Next))
            }
            "ACTIVATE" | "ACT" | "DEACTIVATE" | "DEACT" => {
                let on = kw.starts_with("ACT");
                let mut j = k + 1;
                while j < se {
                    if let Some(n) = word(&t[j]) {
                        let n = n.to_string();
                        let mode = match t.get(j + 1).and_then(word) {
                            Some("SCAN" | "NORESCAN") => {
                                j += 1;
                                Scan::Once
                            }
                            Some("RESCAN") => {
                                j += 1;
                                Scan::Rescan
                            }
                            _ => Scan::Rescan,
                        };
                        if on {
                            self.active.insert(n, mode);
                        } else {
                            self.active.remove(&n);
                        }
                    }
                    j += 1;
                }
                Ok((se + 1, Flow::Next))
            }
            "REPLACE" => {
                let name = t.get(k + 1).and_then(word).ok_or_else(|| self.err(pos, "%REPLACE needs a name"))?.to_string();
                let v = match t.get(k + 3).map(|x| &x.tok) {
                    Some(Tok::Num { text, .. }) => Val::Fixed(text.parse().unwrap_or(0)),
                    Some(Tok::Str { text, .. }) => Val::Char(text.clone()),
                    Some(Tok::Ident(x)) => Val::Char(x.clone()),
                    _ => return Err(self.err(pos, "%REPLACE needs a constant")),
                };
                if matches!(v, Val::Char(_)) {
                    self.chars.insert(name.clone());
                }
                self.vars.insert(name.clone(), v);
                self.active.insert(name, Scan::Rescan);
                Ok((se + 1, Flow::Next))
            }
            "NOTE" => {
                let e = self.parse_expr(&t[k + 1..se], pos);
                let (msg, code) = match e {
                    Ok(Expr::Ref(_)) | Err(_) => {
                        // NOTE(msg, code): parse as a call-like list
                        let inner = &t[k + 2..se.saturating_sub(1)];
                        let parts: Vec<&[Token]> = inner.split(|x| is_op(x, ",")).collect();
                        let m = self.parse_expr(parts[0], pos)?;
                        let m = self.eval(&m, &mut HashMap::new())?.text();
                        let c = match parts.get(1) {
                            Some(p) => {
                                let e = self.parse_expr(p, pos)?;
                                self.eval(&e, &mut HashMap::new())?.num()
                            }
                            None => 0,
                        };
                        (m, c)
                    }
                    Ok(e) => (self.eval(&e, &mut HashMap::new())?.text(), 0),
                };
                let severity = match code {
                    0..=3 => 'I',
                    4..=7 => 'W',
                    8..=11 => 'E',
                    _ => 'S',
                };
                self.diags.push(Diag {
                    pos,
                    severity,
                    message: msg.clone(),
                });
                if code >= 16 {
                    return Err(Diag::error(pos, msg));
                }
                Ok((se + 1, Flow::Next))
            }
            "INCLUDE" | "XINCLUDE" => {
                let once = kw == "XINCLUDE";
                let items: Vec<&[Token]> = t[k + 1..se].split(|x| is_op(x, ",")).collect();
                for it in items {
                    let member = match it {
                        [Token { tok: Tok::Str { text, .. }, .. }] => text.clone(),
                        [Token { tok: Tok::Ident(n), .. }] => n.clone(),
                        [_, Token { tok: Tok::Op("("), .. }, Token { tok: Tok::Ident(n), .. }, Token { tok: Tok::Op(")"), .. }] => n.clone(),
                        [] => continue,
                        _ => return Err(self.err(pos, "invalid %INCLUDE item")),
                    };
                    let path = crate::lexer::find_include(&self.opts.include_dirs, &member)
                        .ok_or_else(|| self.err(pos, format!("%INCLUDE member {member} not found")))?;
                    if once && self.included.contains(&path) {
                        continue;
                    }
                    self.included.insert(path.clone());
                    if self.depth > 20 {
                        return Err(self.err(pos, "%INCLUDE nested too deeply"));
                    }
                    let mut toks = crate::lexer::tokenize(&path, self.opts, self.files)?;
                    toks.pop(); // end of file
                    let n = toks.len();
                    self.depth += 1;
                    let r = self.run(&toks, 0, n);
                    self.depth -= 1;
                    r?;
                }
                Ok((se + 1, Flow::Next))
            }
            "IF" => {
                let then = find_pct(t, k + 1, end, "THEN").ok_or_else(|| self.err(pos, "%THEN expected"))?;
                let e = self.parse_expr(&t[k + 1..then], pos)?;
                let c = self.eval(&e, &mut HashMap::new())?.truth();
                let u1 = then + 2;
                let u1e = unit_end(t, u1, end)?;
                let (u2, u2e) = if u1e + 1 < end && is_op(&t[u1e], "%") && is_word(&t[u1e + 1], "ELSE") {
                    let u2 = u1e + 2;
                    (Some(u2), unit_end(t, u2, end)?)
                } else {
                    (None, u1e)
                };
                let flow = if c {
                    self.run(t, u1, u1e)?
                } else if let Some(u2) = u2 {
                    self.run(t, u2, u2e)?
                } else {
                    Flow::Next
                };
                Ok((u2e, flow))
            }
            "DO" => {
                let body = se + 1;
                let (after, end_pct) = group_end(t, body, end)?;
                let spec_toks = &t[k + 1..se];
                if spec_toks.len() == 1 && is_word(&spec_toks[0], "SKIP") {
                    return Ok((after, Flow::Next));
                }
                let mut v = spec_toks.to_vec();
                v.push(Token { tok: Tok::Op(";"), pos });
                v.push(Token { tok: Tok::Eof, pos });
                let spec = Parser::new(v).do_spec()?;
                let flow = self.do_group(&spec, t, body, end_pct, &labels)?;
                Ok((after, flow))
            }
            "SELECT" => {
                let (after, end_pct) = group_end(t, se + 1, end)?;
                let subject = if se > k + 1 {
                    let e = self.parse_expr(&t[k + 1..se], pos)?;
                    Some(self.eval(&e, &mut HashMap::new())?)
                } else {
                    None
                };
                let mut j = se + 1;
                while j < end_pct {
                    if !is_op(&t[j], "%") {
                        return Err(self.err(t[j].pos, "%WHEN or %OTHERWISE expected"));
                    }
                    let (w, wk, _) = keyword(t, j + 1, end_pct);
                    match w.as_deref() {
                        Some("WHEN") => {
                            let close = stmt_end(t, wk, end_pct);
                            // WHEN(...) unit: the list ends at the matching ')'
                            let mut depth = 0;
                            let mut p = wk + 1;
                            while p < end_pct {
                                match &t[p].tok {
                                    Tok::Op("(") => depth += 1,
                                    Tok::Op(")") => {
                                        depth -= 1;
                                        if depth == 0 {
                                            break;
                                        }
                                    }
                                    _ => {}
                                }
                                p += 1;
                            }
                            let _ = close;
                            let list = &t[wk + 2..p];
                            let ue = unit_end(t, p + 1, end_pct)?;
                            let mut hit = false;
                            for item in list.split(|x| is_op(x, ",")) {
                                let e = self.parse_expr(item, pos)?;
                                let v = self.eval(&e, &mut HashMap::new())?;
                                hit |= match &subject {
                                    Some(s) => compare(s, &v) == std::cmp::Ordering::Equal,
                                    None => v.truth(),
                                };
                            }
                            if hit {
                                let f = self.run(t, p + 1, ue)?;
                                return Ok((after, f));
                            }
                            j = ue;
                        }
                        Some("OTHERWISE" | "OTHER") => {
                            let ue = unit_end(t, wk + 1, end_pct)?;
                            let f = self.run(t, wk + 1, ue)?;
                            return Ok((after, f));
                        }
                        _ => return Err(self.err(t[j].pos, "%WHEN or %OTHERWISE expected")),
                    }
                }
                Ok((after, Flow::Next))
            }
            "LEAVE" | "ITERATE" => {
                let l = t.get(k + 1).and_then(word).filter(|_| k + 1 < se).map(str::to_string);
                Ok((se + 1, if kw == "LEAVE" { Flow::Leave(l) } else { Flow::Iterate(l) }))
            }
            "PROCEDURE" | "PROC" => {
                let (after, end_pct) = group_end(t, se + 1, end)?;
                let name = labels.first().cloned().ok_or_else(|| self.err(pos, "a preprocessor procedure needs a name"))?;
                let mut params = Vec::new();
                let mut returns_char = false;
                let mut j = k + 1;
                while j < se {
                    if is_op(&t[j], "(") {
                        j += 1;
                        while j < se && !is_op(&t[j], ")") {
                            if let Some(p) = word(&t[j]) {
                                params.push(p.to_string());
                            }
                            j += 1;
                        }
                    } else if is_word(&t[j], "RETURNS") {
                        returns_char = t.get(j + 2).is_some_and(|x| is_word(x, "CHARACTER") || is_word(x, "CHAR"));
                        j += 3;
                    }
                    j += 1;
                }
                let body: Vec<Token> = t[se + 1..end_pct].iter().filter(|x| !is_op(x, "%")).cloned().collect();
                self.procs.insert(name, Proc { params, returns_char, body });
                Ok((after, Flow::Next))
            }
            "GO" | "GOTO" => Err(self.err(pos, "%GO TO is not supported")),
            "END" => Err(self.err(pos, "%END without a group")),
            other => Err(self.err(pos, format!("preprocessor statement %{other} is not supported"))),
        }
    }

    fn do_group(&mut self, spec: &DoSpec, t: &[Token], body: usize, end: usize, labels: &[String]) -> Result<Flow> {
        let mine = |l: &Option<String>| l.as_ref().is_none_or(|x| labels.contains(x));
        let mut guard = 0u32;
        let mut once = |me: &mut Pp| -> Result<Option<Flow>> {
            guard += 1;
            if guard > 100_000 {
                return Err(Diag::error(t[body.min(t.len() - 1)].pos, "%DO loop does not end"));
            }
            match me.run(t, body, end)? {
                Flow::Next => Ok(None),
                Flow::Iterate(l) if mine(&l) => Ok(None),
                Flow::Leave(l) if mine(&l) => Ok(Some(Flow::Next)),
                f => Ok(Some(f)),
            }
        };
        match spec {
            DoSpec::Group => Ok(once(self)?.unwrap_or(Flow::Next)),
            DoSpec::Forever => loop {
                if let Some(f) = once(self)? {
                    return Ok(f);
                }
            },
            DoSpec::Loop { while_, until } => loop {
                if let Some(w) = while_
                    && !self.eval(w, &mut HashMap::new())?.truth()
                {
                    return Ok(Flow::Next);
                }
                if let Some(f) = once(self)? {
                    return Ok(f);
                }
                if let Some(u) = until
                    && self.eval(u, &mut HashMap::new())?.truth()
                {
                    return Ok(Flow::Next);
                }
            },
            DoSpec::Iterative { var, specs } => {
                let name = var.name().to_string();
                for s in specs {
                    let start = self.eval(&s.start, &mut HashMap::new())?;
                    let to = match &s.to {
                        Some(e) => Some(self.eval(e, &mut HashMap::new())?.num()),
                        None => None,
                    };
                    let by = match &s.by {
                        Some(e) => self.eval(e, &mut HashMap::new())?.num(),
                        None => 1,
                    };
                    self.vars.insert(name.clone(), start);
                    loop {
                        let cur = self.vars.get(&name).map_or(0, Val::num);
                        if let Some(to) = to
                            && ((by >= 0 && cur > to) || (by < 0 && cur < to))
                        {
                            break;
                        }
                        if let Some(w) = &s.while_
                            && !self.eval(w, &mut HashMap::new())?.truth()
                        {
                            break;
                        }
                        if let Some(f) = once(self)? {
                            return Ok(f);
                        }
                        if let Some(u) = &s.until
                            && self.eval(u, &mut HashMap::new())?.truth()
                        {
                            break;
                        }
                        if let Some(r) = &s.repeat {
                            let v = self.eval(r, &mut HashMap::new())?;
                            self.vars.insert(name.clone(), v);
                        } else if to.is_none() && s.by.is_none() {
                            break;
                        } else {
                            let cur = self.vars.get(&name).map_or(0, Val::num);
                            self.vars.insert(name.clone(), Val::Fixed(cur + by));
                        }
                    }
                }
                Ok(Flow::Next)
            }
        }
    }

    fn declare(&mut self, t: &[Token], pos: Pos) -> Result<()> {
        // items: names (or a factored list) followed by attributes
        let mut i = 0;
        while i < t.len() {
            let mut names = Vec::new();
            if is_op(&t[i], "(") {
                i += 1;
                while i < t.len() && !is_op(&t[i], ")") {
                    if let Some(n) = word(&t[i]) {
                        names.push(n.to_string());
                    }
                    i += 1;
                }
                i += 1;
            } else if let Some(n) = word(&t[i]) {
                names.push(n.to_string());
                i += 1;
                // dimensions are not supported: skip them
                if i < t.len() && is_op(&t[i], "(") {
                    while i < t.len() && !is_op(&t[i], ")") {
                        i += 1;
                    }
                    i += 1;
                }
            } else {
                return Err(self.err(pos, "invalid %DECLARE"));
            }
            let mut ch = false;
            let mut scan = Some(Scan::Rescan);
            let mut entry = false;
            let mut init = None;
            while i < t.len() && !is_op(&t[i], ",") {
                match word(&t[i]) {
                    Some("CHARACTER" | "CHAR") => ch = true,
                    Some("FIXED") => ch = false,
                    Some("ENTRY") => entry = true,
                    Some("BUILTIN") => scan = None,
                    Some("NOSCAN") => scan = None,
                    Some("SCAN" | "NORESCAN") => scan = Some(Scan::Once),
                    Some("RESCAN") => scan = Some(Scan::Rescan),
                    Some("INITIAL" | "INIT") if t.get(i + 1).is_some_and(|x| is_op(x, "(")) => {
                        let start = i + 2;
                        let mut depth = 1;
                        let mut j = start;
                        while j < t.len() {
                            match &t[j].tok {
                                Tok::Op("(") => depth += 1,
                                Tok::Op(")") => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            j += 1;
                        }
                        let e = self.parse_expr(&t[start..j], pos)?;
                        init = Some(self.eval(&e, &mut HashMap::new())?);
                        i = j;
                    }
                    _ => {}
                }
                i += 1;
            }
            i += 1;
            for n in names {
                if !entry {
                    let v = init.clone().unwrap_or(if ch { Val::Char(String::new()) } else { Val::Fixed(0) });
                    if ch {
                        self.chars.insert(n.clone());
                    }
                    self.vars.insert(n.clone(), v);
                }
                match scan {
                    Some(s) => {
                        self.active.insert(n, s);
                    }
                    None => {
                        self.active.remove(&n);
                    }
                }
            }
        }
        Ok(())
    }

    fn assign(&mut self, name: &str, v: Val, locals: &mut HashMap<String, Val>) {
        let is_char = self.chars.contains(name);
        let v = if is_char {
            Val::Char(v.text())
        } else {
            match v {
                Val::Char(s) if !locals.contains_key(name) && !self.vars.contains_key(name) => Val::Char(s),
                other => Val::Fixed(other.num()),
            }
        };
        if let Some(x) = locals.get_mut(name) {
            *x = match (&*x, v) {
                (Val::Char(_), v) => Val::Char(v.text()),
                (_, v) => Val::Fixed(v.num()),
            };
            return;
        }
        self.vars.insert(name.to_string(), v);
    }

    fn eval(&mut self, e: &Expr, locals: &mut HashMap<String, Val>) -> Result<Val> {
        Ok(match e {
            Expr::Num { text, .. } => Val::Fixed(text.split('.').next().unwrap_or("0").parse().unwrap_or(0)),
            Expr::Str { text, kind, .. } => match kind {
                StrKind::Bit(_) => Val::Bit(text.contains('1')),
                _ => Val::Char(text.clone()),
            },
            Expr::Star(p) => return Err(self.err(*p, "'*' in a preprocessor expression")),
            Expr::Unary(op, x, _) => {
                let v = self.eval(x, locals)?;
                match *op {
                    "-" => Val::Fixed(-v.num()),
                    "+" => Val::Fixed(v.num()),
                    _ => Val::Bit(!v.truth()),
                }
            }
            Expr::Binary(op, a, b, _) => {
                let x = self.eval(a, locals)?;
                let y = self.eval(b, locals)?;
                match *op {
                    "+" => Val::Fixed(x.num() + y.num()),
                    "-" => Val::Fixed(x.num() - y.num()),
                    "*" => Val::Fixed(x.num() * y.num()),
                    "/" => Val::Fixed(if y.num() == 0 { 0 } else { x.num() / y.num() }),
                    "||" => Val::Char(format!("{}{}", x.text(), y.text())),
                    "&" => Val::Bit(x.truth() && y.truth()),
                    "|" => Val::Bit(x.truth() || y.truth()),
                    "¬" => Val::Bit(x.truth() != y.truth()),
                    o => {
                        let c = compare(&x, &y);
                        use std::cmp::Ordering::*;
                        Val::Bit(match o {
                            "=" => c == Equal,
                            "¬=" => c != Equal,
                            "<" => c == Less,
                            ">" => c == Greater,
                            "<=" | "¬>" => c != Greater,
                            _ => c != Less,
                        })
                    }
                }
            }
            Expr::Ref(r) => {
                let name = r.name().to_string();
                let args: Vec<Val> = match r.parts.last().and_then(|p| p.1.first()) {
                    Some(list) => {
                        let mut v = Vec::new();
                        for a in list {
                            v.push(self.eval(a, locals)?);
                        }
                        v
                    }
                    None => Vec::new(),
                };
                if let Some(v) = locals.get(&name) {
                    return Ok(v.clone());
                }
                if let Some(p) = self.procs.get(&name).cloned() {
                    return self.call(&p, args, r.pos);
                }
                if let Some(v) = builtin(&name, &args, self) {
                    return Ok(v);
                }
                self.vars.get(&name).cloned().unwrap_or(Val::Char(String::new()))
            }
        })
    }

    /// Invokes a preprocessor procedure.
    fn call(&mut self, p: &Proc, args: Vec<Val>, pos: Pos) -> Result<Val> {
        self.depth += 1;
        if self.depth > 100 {
            return Err(self.err(pos, "preprocessor procedures nested too deeply"));
        }
        let mut locals: HashMap<String, Val> = HashMap::new();
        for (i, name) in p.params.iter().enumerate() {
            locals.insert(name.clone(), args.get(i).cloned().unwrap_or(Val::Char(String::new())));
        }
        let mut toks = p.body.clone();
        toks.push(Token { tok: Tok::Eof, pos });
        let mut parser = Parser::new(toks);
        let mut stmts = Vec::new();
        while !parser.at_end() {
            stmts.push(parser.statement()?);
        }
        let r = self.exec(&stmts, &mut locals);
        self.depth -= 1;
        let v = match r? {
            Flow::Return(v) => v,
            _ => Val::Char(String::new()),
        };
        Ok(if p.returns_char { Val::Char(v.text()) } else { Val::Fixed(v.num()) })
    }

    /// Executes statements of a preprocessor procedure.
    fn exec(&mut self, stmts: &[ast::Stmt], locals: &mut HashMap<String, Val>) -> Result<Flow> {
        for s in stmts {
            let f = self.exec_one(s, locals)?;
            if !matches!(f, Flow::Next) {
                return Ok(f);
            }
        }
        Ok(Flow::Next)
    }

    fn exec_one(&mut self, s: &ast::Stmt, locals: &mut HashMap<String, Val>) -> Result<Flow> {
        match &s.kind {
            StmtKind::Declare(items) => {
                for it in items {
                    let ch = it.attrs.iter().any(|a| matches!(a, ast::Attr::Character(_)));
                    // a parameter keeps its argument, converted to the declared type
                    let v = match locals.get(&it.name) {
                        Some(a) if ch => Val::Char(a.text()),
                        Some(a) => Val::Fixed(a.num()),
                        None if ch => Val::Char(String::new()),
                        None => Val::Fixed(0),
                    };
                    locals.insert(it.name.clone(), v);
                }
                Ok(Flow::Next)
            }
            StmtKind::Assign { targets, value, .. } => {
                let v = self.eval(value, locals)?;
                for t in targets {
                    if let Expr::Ref(r) = t {
                        self.assign(r.name(), v.clone(), locals);
                    }
                }
                Ok(Flow::Next)
            }
            StmtKind::If { cond, then, els } => {
                if self.eval(cond, locals)?.truth() {
                    self.exec_one(then, locals)
                } else if let Some(e) = els {
                    self.exec_one(e, locals)
                } else {
                    Ok(Flow::Next)
                }
            }
            StmtKind::Do { spec, body } => {
                let mut guard = 0;
                loop {
                    guard += 1;
                    if guard > 100_000 {
                        return Err(self.err(s.pos, "DO loop in a preprocessor procedure does not end"));
                    }
                    match spec {
                        DoSpec::Group => return self.exec(body, locals),
                        DoSpec::Loop { while_, until } => {
                            if let Some(w) = while_
                                && !self.eval(w, locals)?.truth()
                            {
                                return Ok(Flow::Next);
                            }
                            match self.exec(body, locals)? {
                                Flow::Leave(_) => return Ok(Flow::Next),
                                Flow::Return(v) => return Ok(Flow::Return(v)),
                                _ => {}
                            }
                            if let Some(u) = until
                                && self.eval(u, locals)?.truth()
                            {
                                return Ok(Flow::Next);
                            }
                        }
                        DoSpec::Forever => match self.exec(body, locals)? {
                            Flow::Leave(_) => return Ok(Flow::Next),
                            Flow::Return(v) => return Ok(Flow::Return(v)),
                            _ => {}
                        },
                        DoSpec::Iterative { var, specs } => {
                            let name = var.name().to_string();
                            for sp in specs {
                                let start = self.eval(&sp.start, locals)?;
                                self.assign(&name, start, locals);
                                let to = match &sp.to {
                                    Some(e) => Some(self.eval(e, locals)?.num()),
                                    None => None,
                                };
                                let by = match &sp.by {
                                    Some(e) => self.eval(e, locals)?.num(),
                                    None => 1,
                                };
                                loop {
                                    let cur = locals.get(&name).or_else(|| self.vars.get(&name)).map_or(0, Val::num);
                                    if let Some(to) = to
                                        && ((by >= 0 && cur > to) || (by < 0 && cur < to))
                                    {
                                        break;
                                    }
                                    if let Some(w) = &sp.while_
                                        && !self.eval(w, locals)?.truth()
                                    {
                                        break;
                                    }
                                    match self.exec(body, locals)? {
                                        Flow::Leave(_) => return Ok(Flow::Next),
                                        Flow::Return(v) => return Ok(Flow::Return(v)),
                                        _ => {}
                                    }
                                    if let Some(u) = &sp.until
                                        && self.eval(u, locals)?.truth()
                                    {
                                        break;
                                    }
                                    if to.is_none() && sp.by.is_none() {
                                        break;
                                    }
                                    let cur = locals.get(&name).or_else(|| self.vars.get(&name)).map_or(0, Val::num);
                                    self.assign(&name, Val::Fixed(cur + by), locals);
                                }
                            }
                            return Ok(Flow::Next);
                        }
                    }
                }
            }
            StmtKind::Select { subject, whens, other } => {
                let subj = match subject {
                    Some(e) => Some(self.eval(e, locals)?),
                    None => None,
                };
                for (vals, unit) in whens {
                    for v in vals {
                        let x = self.eval(v, locals)?;
                        let hit = match &subj {
                            Some(sv) => compare(sv, &x) == std::cmp::Ordering::Equal,
                            None => x.truth(),
                        };
                        if hit {
                            return self.exec_one(unit, locals);
                        }
                    }
                }
                match other {
                    Some(o) => self.exec_one(o, locals),
                    None => Ok(Flow::Next),
                }
            }
            StmtKind::Return(v) => Ok(Flow::Return(match v {
                Some(e) => self.eval(e, locals)?,
                None => Val::Char(String::new()),
            })),
            StmtKind::Leave(l) => Ok(Flow::Leave(l.clone())),
            StmtKind::Iterate(l) => Ok(Flow::Iterate(l.clone())),
            StmtKind::Null => Ok(Flow::Next),
            _ => Err(self.err(s.pos, "statement not supported in a preprocessor procedure")),
        }
    }
}

fn compare(x: &Val, y: &Val) -> std::cmp::Ordering {
    match (x, y) {
        (Val::Char(a), Val::Char(b)) => {
            let n = a.len().max(b.len());
            format!("{a:<n$}").cmp(&format!("{b:<n$}"))
        }
        _ => x.num().cmp(&y.num()),
    }
}

/// Preprocessor built-in functions.
fn builtin(name: &str, a: &[Val], pp: &mut Pp) -> Option<Val> {
    let s = |i: usize| a.get(i).map(Val::text).unwrap_or_default();
    let n = |i: usize| a.get(i).map(Val::num);
    Some(match name {
        "LENGTH" => Val::Fixed(s(0).chars().count() as i64),
        "SUBSTR" => {
            let text: Vec<char> = s(0).chars().collect();
            let start = (n(1).unwrap_or(1).max(1) - 1) as usize;
            let len = n(2).map_or(text.len().saturating_sub(start), |l| l.max(0) as usize);
            Val::Char(text.iter().skip(start).take(len).collect())
        }
        "INDEX" => Val::Fixed(s(0).find(&s(1)).map_or(0, |p| p as i64 + 1)),
        "VERIFY" => {
            let set = s(1);
            Val::Fixed(s(0).chars().position(|c| !set.contains(c)).map_or(0, |p| p as i64 + 1))
        }
        "TRIM" => Val::Char(s(0).trim().to_string()),
        "UPPERCASE" => Val::Char(s(0).to_ascii_uppercase()),
        "LOWERCASE" => Val::Char(s(0).to_ascii_lowercase()),
        "COPY" => Val::Char(s(0).repeat(n(1).unwrap_or(0).max(0) as usize)),
        "REPEAT" => Val::Char(s(0).repeat(n(1).unwrap_or(0).max(0) as usize + 1)),
        "TRANSLATE" => {
            let (to, from): (Vec<char>, Vec<char>) = (s(1).chars().collect(), s(2).chars().collect());
            Val::Char(
                s(0).chars()
                    .map(|c| match from.iter().position(|f| *f == c) {
                        Some(p) => to.get(p).copied().unwrap_or(' '),
                        None => c,
                    })
                    .collect(),
            )
        }
        "MAX" => Val::Fixed(a.iter().map(Val::num).max().unwrap_or(0)),
        "MIN" => Val::Fixed(a.iter().map(Val::num).min().unwrap_or(0)),
        "QUOTE" => Val::Char(format!("'{}'", s(0).replace('\'', "''"))),
        "COUNTER" => {
            pp.counter += 1;
            Val::Char(format!("{:05}", pp.counter))
        }
        "SYSPARM" => Val::Char(std::env::var("ZEUS_SYSPARM").unwrap_or_default()),
        "SYSVERSION" => Val::Char("ZEUS PL/I".into()),
        "SYSTEM" => Val::Char("MVS".into()),
        "COMPILEDATE" => Val::Char(String::new()),
        "PARMSET" => Val::Bit(a.first().is_some_and(|v| !v.text().is_empty())),
        _ => return None,
    })
}

/// Runs the preprocessor over a token stream (ending with Eof); returns
/// the tokens for the parser and the messages.
pub fn preprocess(toks: Vec<Token>, opts: &LexOptions, files: &mut Files) -> Result<(Vec<Token>, Vec<Diag>)> {
    if !toks.iter().any(|t| is_op(t, "%")) {
        return Ok((toks, Vec::new()));
    }
    let eof = toks.last().cloned().unwrap_or(Token {
        tok: Tok::Eof,
        pos: Pos::default(),
    });
    let n = toks.len().saturating_sub(1);
    let mut pp = Pp {
        vars: HashMap::new(),
        chars: HashSet::new(),
        procs: HashMap::new(),
        active: HashMap::new(),
        opts,
        files,
        out: Vec::new(),
        diags: Vec::new(),
        included: HashSet::new(),
        depth: 0,
        counter: 0,
    };
    match pp.run(&toks, 0, n)? {
        Flow::Next => {}
        _ => return Err(Diag::error(eof.pos, "%LEAVE / %ITERATE outside a %DO group")),
    }
    let mut out = pp.out;
    out.push(eof);
    Ok((out, pp.diags))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize_text;

    fn pp(src: &str) -> String {
        let mut files = Files::default();
        let opts = LexOptions::default();
        let toks = tokenize_text(src, "t", &opts, &mut files).unwrap();
        let (out, _) = preprocess(toks, &opts, &mut files).unwrap();
        tokens_text(&out[..out.len() - 1])
    }

    #[test]
    fn statements() {
        assert_eq!(pp(" %DCL N FIXED; %N = 3; X = N;"), "X = 3 ;");
        assert_eq!(pp(" %REPLACE MAXN BY 10; DCL A(MAXN);"), "DCL A ( 10 ) ;");
        assert_eq!(
            pp(" %DCL T CHAR; %T = 'PROD';\n %IF T = 'PROD' %THEN %DO; A=1; %END;\n %ELSE %DO; A=2; %END;"),
            "A = 1 ;"
        );
        assert_eq!(pp(" %DCL I FIXED; %DO I = 1 TO 3; X(I)=I; %END;"), "X ( 1 ) = 1 ; X ( 2 ) = 2 ; X ( 3 ) = 3 ;");
        assert_eq!(
            pp(" %SQ: PROC(V) RETURNS(CHAR); DCL V CHAR;\n RETURN('(' || V || '*' || V || ')'); %END;\n %ACT SQ; Y = SQ(A+1);"),
            "Y = ( A + 1 * A + 1 ) ;"
        );
        assert_eq!(pp(" %DO SKIP; NOT HERE; %END; Z = 1;"), "Z = 1 ;");
        assert_eq!(pp(" %DCL A CHAR; %A = 'B'; %DEACTIVATE A; A = 1;"), "A = 1 ;");
    }
}

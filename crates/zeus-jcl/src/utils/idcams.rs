//! IDCAMS: access method services commands.
//!
//! Supported: DEFINE CLUSTER / ALTERNATEINDEX / PATH /
//! GENERATIONDATAGROUP, DELETE, REPRO, PRINT, BLDINDEX, VERIFY, LISTCAT,
//! and the modal commands IF ... THEN ... ELSE and SET. Commands continue
//! on the next line after a trailing `-` or `+`; `/* ... */` are comments.
//!
//! Condition codes follow IDCAMS: 0 success, 4 warning, 8 a requested
//! entry not found, 12 a logical error (entry already defined, bad
//! parameters), 16 severe error. MAXCC is the step's return code.

use std::io::Write;

use zeus_cobol::ebcdic;
use zeus_ds::catalog::{Entry, EntryKind, VsamOrg, WriteMode};

use crate::job::{Outcome, ProgramResult, StepCtx};

/// Command operand: a word, or a word with a parenthesised argument list.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Node {
    Word(String),
    Group(Vec<Node>),
}

fn tokenize(s: &str) -> Vec<Node> {
    fn group(chars: &[char], i: &mut usize) -> Vec<Node> {
        let mut out = Vec::new();
        while *i < chars.len() {
            let c = chars[*i];
            match c {
                '(' => {
                    *i += 1;
                    out.push(Node::Group(group(chars, i)));
                }
                ')' => {
                    *i += 1;
                    return out;
                }
                ' ' | ',' | '\t' => *i += 1,
                '\'' => {
                    let start = *i + 1;
                    *i += 1;
                    while *i < chars.len() && chars[*i] != '\'' {
                        *i += 1;
                    }
                    out.push(Node::Word(chars[start..*i].iter().collect()));
                    *i += 1;
                }
                _ => {
                    let start = *i;
                    while *i < chars.len() && !" ,()\t".contains(chars[*i]) {
                        *i += 1;
                    }
                    out.push(Node::Word(
                        chars[start..*i]
                            .iter()
                            .collect::<String>()
                            .to_ascii_uppercase(),
                    ));
                }
            }
        }
        out
    }
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    group(&chars, &mut i)
}

/// Keyword arguments of a parameter list: `KEY(args)` or `KEY`.
struct Params(Vec<Node>);

impl Params {
    fn has(&self, names: &[&str]) -> bool {
        self.0
            .iter()
            .any(|n| matches!(n, Node::Word(w) if names.contains(&w.as_str())))
    }

    fn args(&self, names: &[&str]) -> Option<Vec<String>> {
        let mut it = self.0.iter().peekable();
        while let Some(n) = it.next() {
            if let Node::Word(w) = n
                && names.contains(&w.as_str())
            {
                if let Some(Node::Group(g)) = it.peek() {
                    return Some(
                        g.iter()
                            .filter_map(|x| match x {
                                Node::Word(w) => Some(w.clone()),
                                Node::Group(_) => None,
                            })
                            .collect(),
                    );
                }
                return Some(Vec::new());
            }
        }
        None
    }

    fn arg(&self, names: &[&str]) -> Option<String> {
        self.args(names)?.into_iter().next()
    }

    fn pair(&self, names: &[&str]) -> Option<(u32, u32)> {
        let a = self.args(names)?;
        Some((
            a.first()?.parse().ok()?,
            a.get(1).and_then(|s| s.parse().ok()).unwrap_or(0),
        ))
    }

    /// The group following keyword `name` (CLUSTER(...)).
    fn sub(&self, names: &[&str]) -> Option<Params> {
        let mut it = self.0.iter().peekable();
        while let Some(n) = it.next() {
            if let Node::Word(w) = n
                && names.contains(&w.as_str())
                && let Some(Node::Group(g)) = it.peek()
            {
                return Some(Params(g.clone()));
            }
        }
        None
    }
}

/// Joins continuation lines and removes comments; returns the commands.
fn commands(lines: &[String]) -> Vec<String> {
    let mut text = String::new();
    for l in lines {
        let l: String = l.chars().take(72).collect();
        text.push_str(&l);
        text.push('\n');
    }
    // comments
    let mut clean = String::new();
    let mut rest = text.as_str();
    while let Some(i) = rest.find("/*") {
        clean.push_str(&rest[..i]);
        rest = match rest[i + 2..].find("*/") {
            Some(j) => &rest[i + 2 + j + 2..],
            None => "",
        };
    }
    clean.push_str(rest);
    let mut out = Vec::new();
    let mut cur = String::new();
    for line in clean.lines() {
        let t = line.trim_end();
        if let Some(c) = t.strip_suffix('-').or_else(|| t.strip_suffix('+')) {
            cur.push_str(c);
            cur.push(' ');
        } else {
            cur.push_str(t);
            if !cur.trim().is_empty() {
                out.push(cur.trim().to_string());
            }
            cur.clear();
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

struct Idcams<'a, 'b> {
    ctx: &'a mut StepCtx<'b>,
    print: std::fs::File,
    lastcc: i32,
    maxcc: i32,
}

impl Idcams<'_, '_> {
    fn msg(&mut self, s: &str) {
        let _ = writeln!(self.print, "{s}");
    }

    fn run_command(&mut self, cmd: &str) {
        let nodes = tokenize(cmd);
        let Some(Node::Word(verb)) = nodes.first() else {
            return;
        };
        let verb = verb.clone();
        let p = Params(nodes[1..].to_vec());
        // modal commands
        match verb.as_str() {
            "IF" => return self.if_command(cmd),
            "SET" => {
                let words: Vec<String> = cmd
                    .split(|c: char| c.is_whitespace() || c == '=')
                    .filter(|w| !w.is_empty())
                    .map(str::to_ascii_uppercase)
                    .collect();
                if let (Some(var), Some(v)) =
                    (words.get(1), words.get(2).and_then(|v| v.parse().ok()))
                {
                    match var.as_str() {
                        "MAXCC" => self.maxcc = v,
                        "LASTCC" => self.lastcc = v,
                        _ => {}
                    }
                }
                return;
            }
            _ => {}
        }
        self.msg("");
        self.msg(&format!("  {cmd}"));
        let cc = match verb.as_str() {
            "DEFINE" | "DEF" => self.define(&p),
            "DELETE" | "DEL" => self.delete(&nodes[1..]),
            "REPRO" => self.repro(&p),
            "PRINT" => self.print(&p),
            "BLDINDEX" | "BIX" => {
                // alternate indexes are built when the base cluster is opened
                self.msg("IDC0652I ALTERNATE INDEX BUILT");
                0
            }
            "VERIFY" | "VFY" => 0,
            "LISTCAT" | "LISTC" => self.listcat(&p),
            other => {
                self.msg(&format!("IDC3211I KEYWORD {other} IS IMPROPER"));
                12
            }
        };
        self.lastcc = cc;
        self.maxcc = self.maxcc.max(cc);
        self.msg(&format!(
            "IDC0001I FUNCTION COMPLETED, HIGHEST CONDITION CODE WAS {cc}"
        ));
    }

    fn if_command(&mut self, cmd: &str) {
        // IF LASTCC|MAXCC op n THEN [command] [ELSE command]
        let u = cmd.to_ascii_uppercase();
        let Some(then) = u.find(" THEN") else { return };
        let cond: Vec<String> = u[2..then]
            .replace("^=", " NE ")
            .replace("¬=", " NE ")
            .replace(">=", " GE ")
            .replace("<=", " LE ")
            .replace('=', " EQ ")
            .replace('>', " GT ")
            .replace('<', " LT ")
            .split_whitespace()
            .map(str::to_string)
            .collect();
        let (Some(var), Some(op), Some(n)) = (
            cond.first(),
            cond.get(1),
            cond.get(2).and_then(|n| n.parse::<i32>().ok()),
        ) else {
            return;
        };
        let v = if var == "MAXCC" {
            self.maxcc
        } else {
            self.lastcc
        };
        let ok = match op.as_str() {
            "EQ" => v == n,
            "NE" => v != n,
            "GT" => v > n,
            "LT" => v < n,
            "GE" => v >= n,
            "LE" => v <= n,
            _ => false,
        };
        let rest = &cmd[then + 5..];
        let (t, e) = match rest.to_ascii_uppercase().find(" ELSE") {
            Some(i) => (&rest[..i], Some(&rest[i + 5..])),
            None => (rest, None),
        };
        let chosen = if ok { Some(t) } else { e };
        if let Some(c) = chosen.map(str::trim).filter(|c| !c.is_empty()) {
            self.run_command(c);
        }
    }

    fn define(&mut self, p: &Params) -> i32 {
        let cat = &mut self.ctx.env.catalog;
        let (name, kind) = if let Some(c) = p.sub(&["CLUSTER", "CL"]) {
            let Some(name) = c.arg(&["NAME"]) else {
                return 12;
            };
            let org = if c.has(&["NONINDEXED", "NIXD"]) {
                VsamOrg::Esds
            } else if c.has(&["NUMBERED", "NUMD"]) {
                VsamOrg::Rrds
            } else {
                VsamOrg::Ksds
            };
            let data = p.sub(&["DATA"]);
            let keys = c
                .pair(&["KEYS"])
                .or_else(|| data.as_ref().and_then(|d| d.pair(&["KEYS"])));
            let rs = c
                .pair(&["RECORDSIZE", "RECSZ"])
                .or_else(|| data.as_ref().and_then(|d| d.pair(&["RECORDSIZE", "RECSZ"])))
                .unwrap_or((4089, 4089));
            let keys = match org {
                VsamOrg::Ksds => Some(keys.unwrap_or((64, 0))),
                _ => None,
            };
            (
                name,
                EntryKind::Cluster {
                    org,
                    keys,
                    record_size: rs,
                    aixs: Vec::new(),
                },
            )
        } else if let Some(a) = p.sub(&["ALTERNATEINDEX", "AIX"]) {
            let (Some(name), Some(relate), Some(keys)) = (
                a.arg(&["NAME"]),
                a.arg(&["RELATE", "REL"]),
                a.pair(&["KEYS"]),
            ) else {
                return 12;
            };
            (
                name,
                EntryKind::AlternateIndex {
                    relate,
                    keys,
                    unique: !a.has(&["NONUNIQUEKEY", "NUNQK"]),
                    upgrade: !a.has(&["NOUPGRADE", "NUPG"]),
                },
            )
        } else if let Some(a) = p.sub(&["PATH"]) {
            let (Some(name), Some(entry)) = (a.arg(&["NAME"]), a.arg(&["PATHENTRY", "PENT"]))
            else {
                return 12;
            };
            (name, EntryKind::Path { entry })
        } else if let Some(g) = p.sub(&["GENERATIONDATAGROUP", "GDG"]) {
            let Some(name) = g.arg(&["NAME"]) else {
                return 12;
            };
            let limit = g
                .arg(&["LIMIT"])
                .and_then(|l| l.parse().ok())
                .unwrap_or(255);
            (
                name,
                EntryKind::Gdg {
                    limit,
                    scratch: g.has(&["SCRATCH"]),
                    generations: Vec::new(),
                },
            )
        } else {
            self.msg("IDC3203I ITEM TYPE NOT SUPPORTED");
            return 12;
        };
        let r = cat.define(Entry {
            name: name.clone(),
            kind: kind.clone(),
        });
        let _ = cat.save();
        match r {
            Ok(()) => {
                if let EntryKind::Cluster { .. } = kind {
                    let _ = std::fs::write(cat.path(&name), b"");
                }
                self.msg(&format!(
                    "IDC0508I DATA ALLOCATION STATUS FOR VOLUME ZEUS IS 0 ({name})"
                ));
                0
            }
            Err(zeus_ds::DsError::DuplicateKey) => {
                self.msg(&format!("IDC3013I DUPLICATE DATA SET NAME {name}"));
                12
            }
            Err(e) => {
                self.msg(&format!("IDC3009I {name}: {e}"));
                12
            }
        }
    }

    fn delete(&mut self, nodes: &[Node]) -> i32 {
        let mut names = Vec::new();
        match nodes.first() {
            Some(Node::Group(g)) => {
                for n in g {
                    if let Node::Word(w) = n {
                        names.push(w.clone());
                    }
                }
            }
            Some(Node::Word(w)) => names.push(w.clone()),
            None => return 12,
        }
        let mut cc = 0;
        for name in names {
            match self.ctx.env.catalog.delete(&name) {
                Ok(_) => self.msg(&format!("IDC0550I ENTRY {name} DELETED")),
                Err(_) => {
                    self.msg(&format!("IDC3012I ENTRY {name} NOT FOUND"));
                    cc = 8;
                }
            }
        }
        let _ = self.ctx.env.catalog.save();
        cc
    }

    /// Input of REPRO / PRINT: a DD name or a data set name.
    fn input(&mut self, p: &Params) -> Result<Vec<Vec<u8>>, String> {
        if let Some(dd) = p.arg(&["INFILE", "IFILE"]) {
            return self.ctx.read(&dd);
        }
        if let Some(dsn) = p.arg(&["INDATASET", "IDS"]) {
            return self
                .ctx
                .env
                .catalog
                .read_records(&dsn)
                .map_err(|e| format!("{dsn}: {e}"));
        }
        Err("no input data set".into())
    }

    fn repro(&mut self, p: &Params) -> i32 {
        let recs = match self.input(p) {
            Ok(r) => r,
            Err(e) => {
                self.msg(&format!("IDC3300I ERROR OPENING INPUT: {e}"));
                return 12;
            }
        };
        let skip = p.arg(&["SKIP"]).and_then(|s| s.parse().ok()).unwrap_or(0);
        let count = p
            .arg(&["COUNT"])
            .and_then(|s| s.parse().ok())
            .unwrap_or(usize::MAX);
        let recs: Vec<Vec<u8>> = recs.into_iter().skip(skip).take(count).collect();
        // VSAM output keeps its records (REPRO adds); non-VSAM is rewritten
        let (out_dd, out_dsn) = (p.arg(&["OUTFILE", "OFILE"]), p.arg(&["OUTDATASET", "ODS"]));
        let vsam =
            |e: Option<&Entry>| matches!(e.map(|e| &e.kind), Some(EntryKind::Cluster { .. }));
        let result = match (&out_dd, &out_dsn) {
            (Some(dd), _) => {
                let mode = if vsam(
                    self.ctx
                        .entry(dd)
                        .and_then(|e| self.ctx.env.catalog.base_of(&e.name)),
                ) {
                    WriteMode::Append
                } else {
                    WriteMode::Replace
                };
                self.ctx.write(dd, &recs, mode)
            }
            (None, Some(dsn)) => {
                let mode = if vsam(self.ctx.env.catalog.base_of(dsn)) {
                    WriteMode::Append
                } else {
                    WriteMode::Replace
                };
                self.ctx
                    .env
                    .catalog
                    .write_records(dsn, &recs, mode)
                    .map_err(|e| format!("{dsn}: {e}"))
            }
            _ => Err("no output data set".into()),
        };
        match result {
            Ok(0) => {
                self.msg(&format!(
                    "IDC0005I NUMBER OF RECORDS PROCESSED WAS {}",
                    recs.len()
                ));
                0
            }
            Ok(dups) => {
                self.msg(&format!(
                    "IDC3314I {dups} RECORD(S) WITH DUPLICATE KEYS NOT COPIED"
                ));
                self.msg(&format!(
                    "IDC0005I NUMBER OF RECORDS PROCESSED WAS {}",
                    recs.len() - dups
                ));
                8
            }
            Err(e) => {
                self.msg(&format!("IDC3302I ACTION ERROR: {e}"));
                12
            }
        }
    }

    fn print(&mut self, p: &Params) -> i32 {
        let recs = match self.input(p) {
            Ok(r) => r,
            Err(e) => {
                self.msg(&format!("IDC3300I ERROR OPENING INPUT: {e}"));
                return 12;
            }
        };
        let count = p
            .arg(&["COUNT"])
            .and_then(|s| s.parse().ok())
            .unwrap_or(usize::MAX);
        let hex = p.has(&["HEX", "DUMP"]);
        let mut n = 0;
        for r in recs.iter().take(count) {
            n += 1;
            if hex {
                let h: String = r.iter().map(|b| format!("{b:02X}")).collect();
                self.msg(&format!("RECORD {n}: {h}"));
            } else {
                let t: String = ebcdic::decode_latin1(r)
                    .iter()
                    .map(|&b| b as char)
                    .collect();
                self.msg(&format!("RECORD {n}: {}", t.trim_end()));
            }
        }
        self.msg(&format!("IDC0005I NUMBER OF RECORDS PROCESSED WAS {n}"));
        0
    }

    fn listcat(&mut self, p: &Params) -> i32 {
        let filter = p.arg(&["ENTRIES", "ENT", "LEVEL", "LVL"]);
        let lines: Vec<String> = self
            .ctx
            .env
            .catalog
            .entries()
            .filter(|e| {
                filter
                    .as_ref()
                    .is_none_or(|f| e.name.starts_with(f.as_str()))
            })
            .map(|e| format!("{:<10} ------- {}", kind_name(&e.kind), e.name))
            .collect();
        for l in lines {
            self.msg(&l);
        }
        0
    }
}

fn kind_name(k: &EntryKind) -> &'static str {
    match k {
        EntryKind::Sequential { .. } | EntryKind::Partitioned { .. } => "NONVSAM",
        EntryKind::Cluster { .. } => "CLUSTER",
        EntryKind::AlternateIndex { .. } => "AIX",
        EntryKind::Path { .. } => "PATH",
        EntryKind::Gdg { .. } => "GDG BASE",
    }
}

pub fn run(ctx: &mut StepCtx) -> ProgramResult {
    let print = ctx.sysout("SYSPRINT").map_err(Outcome::JclError)?;
    let lines = ctx.read_text("SYSIN").unwrap_or_default();
    let mut a = Idcams {
        ctx,
        print,
        lastcc: 0,
        maxcc: 0,
    };
    a.msg("IDCAMS  SYSTEM SERVICES");
    for c in commands(&lines) {
        a.run_command(&c);
    }
    let cc = a.maxcc;
    a.msg("");
    a.msg(&format!(
        "IDC0002I IDCAMS PROCESSING COMPLETE. MAXIMUM CONDITION CODE WAS {cc}"
    ));
    Ok(cc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commands_and_tokens() {
        let lines: Vec<String> = [
            "   DEFINE CLUSTER (NAME(A.KSDS) -   /* comment */",
            "          KEYS(11 0) -",
            "          RECORDSIZE(300 300) INDEXED) -",
            "          DATA (NAME(A.KSDS.DATA))",
            "   IF MAXCC LE 08 THEN SET MAXCC = 0",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let c = commands(&lines);
        assert_eq!(c.len(), 2);
        let n = tokenize(&c[0]);
        let p = Params(n[1..].to_vec());
        let cl = p.sub(&["CLUSTER"]).unwrap();
        assert_eq!(cl.arg(&["NAME"]).as_deref(), Some("A.KSDS"));
        assert_eq!(cl.pair(&["KEYS"]), Some((11, 0)));
        assert!(cl.has(&["INDEXED"]));
    }
}

//! Scanning a source file for what it is and what it references.
//!
//! The scanners read the text the way the compilers do (COBOL columns
//! 8-72, PL/I margins 2-72 and comments, JCL statements through the Zeus
//! JCL parser) and report each reference with the line and column of the
//! name in the file. Program names held in variables are followed to the
//! constants the program puts in them (VALUE, MOVE, INITIAL, assignment).

pub mod cobol;
pub mod jcl;
pub mod misc;
pub mod pli;

use std::collections::HashMap;

/// A token with its position (1-based line and column).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tok {
    /// Words in capitals; literals without their quotes; punctuation.
    pub text: String,
    pub kind: TokKind,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokKind {
    Word,
    Lit,
    Punct,
}

impl Tok {
    pub fn is(&self, w: &str) -> bool {
        self.kind == TokKind::Word && self.text == w
    }

    pub fn is_punct(&self, p: &str) -> bool {
        self.kind == TokKind::Punct && self.text == p
    }
}

/// A reference to another asset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ref {
    /// Edge type (see `master::EDGE_TYPES`).
    pub edge: &'static str,
    /// The name referenced (in capitals); for a variable, a constant it
    /// may hold, or the variable when none is known (`dynamic`).
    pub target: String,
    /// The variable that names the target, if any.
    pub identifier: Option<String>,
    /// The target could not be determined from the source.
    pub dynamic: bool,
    pub line: usize,
    pub col: usize,
    /// Edge attributes (see `master::EDGE_ATTRS`).
    pub attrs: Vec<(&'static str, String)>,
}

impl Ref {
    pub fn new(edge: &'static str, target: &str, line: usize, col: usize) -> Ref {
        Ref {
            edge,
            target: target.trim().to_ascii_uppercase(),
            identifier: None,
            dynamic: false,
            line,
            col,
            attrs: Vec::new(),
        }
    }

    pub fn attr(mut self, k: &'static str, v: impl Into<String>) -> Ref {
        self.attrs.push((k, v.into()));
        self
    }
}

/// What a scanner found in a file.
#[derive(Debug, Clone, Default)]
pub struct Scanned {
    /// Node type (see `master::NODE_TYPES`).
    pub node_type: &'static str,
    /// The asset's own name (program id, mapset, ...), if it has one.
    pub name: Option<String>,
    /// Other names it can be referenced by (nested program ids, entry
    /// points, PL/I procedures).
    pub aliases: Vec<String>,
    pub attrs: Vec<(&'static str, String)>,
    pub refs: Vec<Ref>,
    /// CSD: transaction id -> program.
    pub transactions: Vec<(String, String)>,
    /// The constants put in variables, and the moves from one variable
    /// to another (for targets named by variables of other files: a COPY
    /// member using a constant of the program that copies it).
    pub values: Values,
    pub moves: Vec<(String, String)>,
}

/// A name a program, mapset or transaction can have: 1 to 8 characters
/// of letters, digits, #, @, $ and -.
pub fn is_name(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "#@$-_".contains(c))
}

/// The constants a program puts in each variable.
pub type Values = HashMap<String, Vec<String>>;

pub fn add_value(values: &mut Values, var: &str, lit: &str) {
    let v = values.entry(var.to_ascii_uppercase()).or_default();
    let lit = lit.trim().to_ascii_uppercase();
    if !lit.is_empty() && !v.contains(&lit) {
        v.push(lit);
    }
}

/// References to a target named by a literal or a variable: the literal,
/// or the constants the variable may hold, or one dynamic reference.
pub fn targets(
    edge: &'static str,
    arg: &Tok,
    values: &Values,
    max: usize,
    out: &mut Vec<Ref>,
    attrs: &[(&'static str, String)],
) {
    let mk = |target: &str| {
        let mut r = Ref::new(edge, target, arg.line, arg.col);
        r.attrs.extend(attrs.iter().cloned());
        r
    };
    match arg.kind {
        TokKind::Lit => {
            if is_name(arg.text.trim(), max) {
                out.push(mk(&arg.text));
            }
        }
        TokKind::Word => {
            let mk = |target: &str| mk(target).attr("identifier", arg.text.clone());
            let known: Vec<&String> = values
                .get(&arg.text)
                .map(|v| v.iter().filter(|l| is_name(l, max)).collect())
                .unwrap_or_default();
            if known.is_empty() {
                let mut r = mk(&arg.text);
                r.identifier = Some(arg.text.clone());
                r.dynamic = true;
                out.push(r);
            }
            for l in known {
                let mut r = mk(l);
                r.identifier = Some(arg.text.clone());
                out.push(r);
            }
        }
        TokKind::Punct => {}
    }
}

/// The options of an EXEC CICS / DLI command: keyword -> first token of
/// its argument (None for an option without one).
fn exec_options(toks: &[Tok]) -> Vec<(String, Option<Tok>)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < toks.len() {
        let t = &toks[i];
        if t.kind != TokKind::Word {
            i += 1;
            continue;
        }
        if toks.get(i + 1).is_some_and(|x| x.is_punct("(")) {
            let arg = toks.get(i + 2).filter(|x| !x.is_punct(")")).cloned();
            // skip to the matching ')'
            let mut depth = 0;
            let mut j = i + 1;
            while j < toks.len() {
                if toks[j].is_punct("(") {
                    depth += 1;
                } else if toks[j].is_punct(")") {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                j += 1;
            }
            out.push((t.text.clone(), arg));
            i = j + 1;
        } else {
            out.push((t.text.clone(), None));
            i += 1;
        }
    }
    out
}

/// The references of an EXEC block: `kind` is CICS, SQL or DLI and
/// `toks` the tokens after it.
pub fn exec_refs(kind: &str, toks: &[Tok], values: &Values, out: &mut Vec<Ref>) {
    let opts = exec_options(toks);
    let has = |k: &str| opts.iter().find(|(o, _)| o == k).map(|(_, a)| a.clone());
    let first = opts.first().map(|(o, _)| o.as_str()).unwrap_or("");
    match kind {
        "CICS" => match first {
            "LINK" | "XCTL" | "LOAD" => {
                if let Some(Some(a)) = has("PROGRAM") {
                    let edge = match first {
                        "LINK" => "CICS_LINK",
                        "XCTL" => "CICS_XCTL",
                        _ => "CICS_LOAD",
                    };
                    targets(edge, &a, values, 8, out, &[]);
                }
            }
            "START" | "RETURN" => {
                if let Some(Some(a)) = has("TRANSID") {
                    targets(
                        "CICS_TRANSID",
                        &a,
                        values,
                        4,
                        out,
                        &[("commands", first.to_string())],
                    );
                }
            }
            "SEND" | "RECEIVE" => {
                let map = has("MAP").flatten();
                let mapset = has("MAPSET").flatten();
                if let Some(arg) = mapset.or(map.clone()) {
                    let mut attrs = vec![("commands", first.to_string())];
                    if let Some(m) = &map {
                        attrs.push(("maps", m.text.clone()));
                    }
                    targets("CICS_MAP", &arg, values, 8, out, &attrs);
                }
            }
            _ => {}
        },
        "SQL" => {
            if let [inc, name, ..] = toks
                && inc.is("INCLUDE")
                && name.kind != TokKind::Punct
            {
                out.push(Ref::new("SQL_INCLUDE", &name.text, name.line, name.col));
            }
        }
        "DLI" => {
            if matches!(first, "SCHD" | "SCHEDULE")
                && let Some(Some(a)) = has("PSB")
            {
                targets("DLI_PSB", &a, values, 8, out, &[]);
            }
        }
        _ => {}
    }
}

/// The node type of a file from its name and text (None: not a source
/// member the analysis knows).
pub fn classify(path: &std::path::Path, text: &str) -> Option<&'static str> {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let upper = text.to_ascii_uppercase();
    let jcl = |upper: &str| {
        let ops: Vec<&str> = upper
            .lines()
            .filter(|l| l.starts_with("//") && !l.starts_with("//*"))
            .filter_map(|l| {
                let mut w = l[2..].split_whitespace();
                if l[2..].starts_with(' ') {
                    w.next()
                } else {
                    w.nth(1)
                }
            })
            .collect();
        if ops.contains(&"JOB") {
            Some("JCL_JOB")
        } else if ops.first() == Some(&"PROC") {
            Some("JCL_PROC")
        } else if !ops.is_empty() {
            Some("JCL_INCLUDE")
        } else {
            None
        }
    };
    let cobol_program = upper.contains("PROCEDURE DIVISION")
        || (upper.contains("IDENTIFICATION DIVISION") || upper.contains("ID DIVISION"))
            && upper.contains("PROGRAM-ID");
    let pli_program = || {
        upper
            .split(';')
            .any(|s| s.contains(": PROC") || s.contains(":PROC") || s.contains(": PROCEDURE"))
    };
    Some(match ext.as_str() {
        "jcl" | "job" | "cntl" => return jcl(&upper),
        "proc" | "prc" => "JCL_PROC",
        "cbl" | "cob" | "cobol" => {
            if cobol_program {
                "COBOL_PROGRAM"
            } else {
                "COBOL_COPYBOOK"
            }
        }
        "cpy" | "copy" | "dcl" => "COBOL_COPYBOOK",
        "pli" | "pl1" => {
            if pli_program() {
                "PLI_PROGRAM"
            } else {
                "PLI_INCLUDE"
            }
        }
        "inc" => "PLI_INCLUDE",
        "bms" => "BMS_MAPSET",
        "dbd" => "IMS_DBD",
        "psb" => "IMS_PSB",
        "asm" | "hlasm" | "mac" => {
            if upper.contains("DFHMSD") {
                "BMS_MAPSET"
            } else if upper.contains("PSBGEN") {
                "IMS_PSB"
            } else if upper.contains("DBDGEN") {
                "IMS_DBD"
            } else if upper.lines().filter(|l| !l.starts_with('*')).any(|l| {
                matches!(
                    l.split_whitespace().nth(1),
                    Some("CSECT" | "RSECT" | "START")
                )
            }) {
                "HLASM_PROGRAM"
            } else {
                "HLASM_COPYBOOK"
            }
        }
        "csd" => "CICS_CSD",
        "" | "txt" | "src" => {
            if upper.contains("DFHMSD") {
                "BMS_MAPSET"
            } else if let Some(t) = jcl(&upper) {
                t
            } else if cobol_program {
                "COBOL_PROGRAM"
            } else if upper.contains("PSBGEN") {
                "IMS_PSB"
            } else if upper.contains("DBDGEN") {
                "IMS_DBD"
            } else if upper.contains("DEFINE PROGRAM(") || upper.contains("DEFINE TRANSACTION(") {
                "CICS_CSD"
            } else if pli_program() {
                "PLI_PROGRAM"
            } else {
                return None;
            }
        }
        _ => return None,
    })
}

/// Scans a file of a known node type.
pub fn scan(node_type: &'static str, path: &std::path::Path, text: &str) -> Scanned {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_uppercase())
        .unwrap_or_default();
    let mut s = match node_type {
        "COBOL_PROGRAM" | "COBOL_COPYBOOK" => cobol::scan(text),
        "PLI_PROGRAM" | "PLI_INCLUDE" => pli::scan(text),
        "JCL_JOB" | "JCL_PROC" | "JCL_INCLUDE" => jcl::scan(text, &stem),
        "BMS_MAPSET" => misc::bms(text),
        "IMS_DBD" => misc::dbd(text),
        "IMS_PSB" => misc::psb(text),
        "CICS_CSD" => misc::csd(text),
        "HLASM_PROGRAM" | "HLASM_COPYBOOK" => misc::hlasm(text),
        _ => Scanned::default(),
    };
    s.node_type = node_type;
    // copy members and includes are named by their file
    if s.name.is_none() || matches!(node_type, "COBOL_COPYBOOK" | "PLI_INCLUDE" | "JCL_INCLUDE") {
        s.name = Some(stem);
    }
    s
}

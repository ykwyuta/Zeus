//! Language usage of programs: which statements, intrinsic functions, data
//! description clauses and EXEC commands / options they use. Used to
//! build the syntax usage inventory of the target applications.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::front::lexer::{Tok, Token};
use crate::front::parser::VERBS;
use crate::front::{self, FileTable, FrontOptions};
use crate::zir::{ExecKind, Module};

/// Data description words counted in the data division.
const DATA_WORDS: &[&str] = &[
    "BINARY",
    "BLANK",
    "COMP",
    "COMP-1",
    "COMP-2",
    "COMP-3",
    "COMP-4",
    "COMP-5",
    "COMPUTATIONAL",
    "COMPUTATIONAL-3",
    "COMPUTATIONAL-5",
    "DEPENDING",
    "DISPLAY-1",
    "EXTERNAL",
    "GLOBAL",
    "INDEX",
    "INDEXED",
    "JUST",
    "JUSTIFIED",
    "LOCAL-STORAGE",
    "NATIONAL",
    "OCCURS",
    "PACKED-DECIMAL",
    "POINTER",
    "PROCEDURE-POINTER",
    "FUNCTION-POINTER",
    "REDEFINES",
    "RENAMES",
    "SEPARATE",
    "SYNC",
    "SYNCHRONIZED",
];

/// Usage counts: item -> (programs using it, occurrences).
#[derive(Debug, Default)]
pub struct Inventory {
    pub items: BTreeMap<(String, String), (BTreeSet<String>, usize)>,
}

impl Inventory {
    fn add(&mut self, kind: &str, item: String, program: &str) {
        let e = self.items.entry((kind.to_string(), item)).or_default();
        e.0.insert(program.to_string());
        e.1 += 1;
    }

    /// Adds the usage of a source file and its compiled module.
    pub fn add_program(&mut self, name: &str, tokens: &[Token], module: Option<&Module>) {
        let mut in_proc = false;
        let mut in_data = false;
        for (i, t) in tokens.iter().enumerate() {
            let next = |k: usize| tokens.get(i + k);
            if t.is_word("PROCEDURE") && next(1).is_some_and(|t| t.is_word("DIVISION")) {
                in_proc = true;
                in_data = false;
            } else if t.is_word("DATA") && next(1).is_some_and(|t| t.is_word("DIVISION")) {
                in_data = true;
                in_proc = false;
            } else if t.is_word("IDENTIFICATION") {
                in_proc = false;
                in_data = false;
            }
            match &t.tok {
                Tok::Word(w) if in_proc && VERBS.contains(&w.as_str()) => {
                    self.add("statement", w.clone(), name)
                }
                Tok::Word(w) if w == "FUNCTION" => {
                    if let Some(Tok::Word(f)) = next(1).map(|t| &t.tok) {
                        self.add("function", f.clone(), name);
                    }
                }
                Tok::Word(w) if in_data && DATA_WORDS.contains(&w.as_str()) => {
                    self.add("data", w.clone(), name)
                }
                _ => {}
            }
        }
        let Some(module) = module else { return };
        for p in &module.programs {
            for e in p.execs.iter().chain(&p.data_execs) {
                let kind = match e.kind {
                    ExecKind::Cics => "EXEC CICS",
                    ExecKind::Sql => "EXEC SQL",
                    ExecKind::SqlIms => "EXEC SQLIMS",
                    ExecKind::Dli => "EXEC DLI",
                };
                self.add(kind, e.command.clone(), name);
                for o in &e.options {
                    self.add(
                        &format!("{kind} option"),
                        format!("{} {}", e.command, o.name),
                        name,
                    );
                }
            }
        }
    }

    /// Tab-separated lines: kind, item, programs, occurrences, program names.
    pub fn report(&self) -> String {
        let mut out = String::new();
        for ((kind, item), (progs, n)) in &self.items {
            let names: Vec<&str> = progs.iter().map(String::as_str).collect();
            out.push_str(&format!(
                "{kind}\t{item}\t{}\t{n}\t{}\n",
                progs.len(),
                names.join(" ")
            ));
        }
        out
    }
}

/// Inventory of source files; also returns the files that do not compile
/// with their first diagnostic.
pub fn inventory(paths: &[&Path], opts: &FrontOptions) -> (Inventory, Vec<(String, String)>) {
    let mut inv = Inventory::default();
    let mut failed = Vec::new();
    for p in paths {
        let name = p
            .file_stem()
            .map(|s| s.to_string_lossy().to_ascii_uppercase())
            .unwrap_or_default();
        // the programs of a batch compilation
        let mut tokens = Vec::new();
        let mut unit = 0;
        let read = loop {
            let mut files = FileTable::default();
            match front::source_unit_tokens(p, opts, &mut files, unit) {
                Ok((t, _, units)) => {
                    tokens.extend(t);
                    unit += 1;
                    if unit >= units {
                        break Ok(());
                    }
                }
                Err(d) => break Err(d),
            }
        };
        if let Err(d) = read {
            failed.push((name, d.message));
            continue;
        }
        let module = match crate::compile_file(p, opts) {
            Ok(m) => Some(m),
            Err(d) => {
                failed.push((name.clone(), d.message));
                None
            }
        };
        inv.add_program(&name, &tokens, module.as_ref());
    }
    (inv, failed)
}

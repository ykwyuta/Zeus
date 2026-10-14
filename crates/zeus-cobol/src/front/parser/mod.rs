//! Recursive-descent parser producing the AST.

mod data;
mod exec;
mod expr;
mod stmt;

use std::collections::HashSet;

use super::ast::*;
use super::lexer::{Tok, Token};
use super::source::Pos;
use crate::diag::{Diag, Result};

/// Words that begin a statement.
pub const VERBS: &[&str] = &[
    "ACCEPT",
    "ADD",
    "ALTER",
    "CALL",
    "CANCEL",
    "CLOSE",
    "COMPUTE",
    "CONTINUE",
    "DELETE",
    "DISPLAY",
    "DISABLE",
    "DIVIDE",
    "ENABLE",
    "ENTRY",
    "EVALUATE",
    "EXIT",
    "GENERATE",
    "GO",
    "GOBACK",
    "IF",
    "INITIALIZE",
    "INITIATE",
    "INSPECT",
    "MERGE",
    "MOVE",
    "MULTIPLY",
    "OPEN",
    "PERFORM",
    "PURGE",
    "READ",
    "RECEIVE",
    "RELEASE",
    "RETURN",
    "REWRITE",
    "SEARCH",
    "SEND",
    "SET",
    "SORT",
    "START",
    "STOP",
    "STRING",
    "SUBTRACT",
    "SUPPRESS",
    "TERMINATE",
    "UNSTRING",
    "WRITE",
];

pub struct Parser {
    toks: Vec<Token>,
    i: usize,
    /// User-defined class names (from SPECIAL-NAMES) for class conditions.
    class_names: HashSet<String>,
    /// Mnemonic and switch-status condition names.
    pub(crate) mnemonics: HashSet<String>,
    /// Names of data items that are tables or within tables (only these
    /// take subscripts, which resolves `MAX(A B (C + 1))`, where the
    /// parenthesis is a separate argument).
    pub(crate) tables: HashSet<String>,
    /// `tables` is known for the program being parsed.
    pub(crate) in_procedure: bool,
}

pub fn parse(toks: Vec<Token>) -> Result<Vec<Program>> {
    let mut p = Parser {
        toks,
        i: 0,
        class_names: HashSet::new(),
        mnemonics: HashSet::new(),
        tables: HashSet::new(),
        in_procedure: false,
    };
    let mut programs = Vec::new();
    while !p.eof() {
        programs.push(p.program()?);
    }
    if programs.is_empty() {
        return Err(Diag::error(Pos::default(), "no program found"));
    }
    Ok(programs)
}

/// Data description entries from free-form text (system-supplied
/// records such as the EXEC interface block).
pub fn data_entries_from(text: &str) -> Result<Vec<DataEntry>> {
    let stream: Vec<(char, Pos)> = text.chars().map(|c| (c, Pos::default())).collect();
    let mut p = Parser {
        toks: super::lexer::lex(&stream, false)?,
        i: 0,
        class_names: HashSet::new(),
        mnemonics: HashSet::new(),
        tables: HashSet::new(),
        in_procedure: false,
    };
    p.data_entries()
}

impl Parser {
    // ---- token helpers -------------------------------------------------

    fn eof(&self) -> bool {
        self.i >= self.toks.len()
    }

    fn peek(&self) -> Option<&Token> {
        self.toks.get(self.i)
    }

    fn peek_at(&self, k: usize) -> Option<&Token> {
        self.toks.get(self.i + k)
    }

    fn pos(&self) -> Pos {
        self.peek()
            .or(self.toks.last())
            .map(|t| t.pos)
            .unwrap_or_default()
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.toks.get(self.i).cloned();
        self.i += 1;
        t
    }

    fn is(&self, w: &str) -> bool {
        self.peek().is_some_and(|t| t.is_word(w))
    }

    fn is_at(&self, k: usize, w: &str) -> bool {
        self.peek_at(k).is_some_and(|t| t.is_word(w))
    }

    fn is_tok(&self, tok: &Tok) -> bool {
        self.peek().is_some_and(|t| &t.tok == tok)
    }

    fn is_any(&self, words: &[&str]) -> bool {
        words.iter().any(|w| self.is(w))
    }

    fn accept(&mut self, w: &str) -> bool {
        if self.is(w) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn accept_any(&mut self, words: &[&str]) -> Option<String> {
        for w in words {
            if self.accept(w) {
                return Some((*w).to_string());
            }
        }
        None
    }

    fn accept_tok(&mut self, tok: &Tok) -> bool {
        if self.is_tok(tok) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn err<T>(&self, msg: impl Into<String>) -> Result<T> {
        let found = self
            .peek()
            .map(|t| t.text())
            .unwrap_or_else(|| "end of source".into());
        Err(Diag::error(
            self.pos(),
            format!("{} (found {found})", msg.into()),
        ))
    }

    fn expect(&mut self, w: &str) -> Result<()> {
        if self.accept(w) {
            Ok(())
        } else {
            self.err(format!("{w} expected"))
        }
    }

    fn expect_tok(&mut self, tok: &Tok, what: &str) -> Result<()> {
        if self.accept_tok(tok) {
            Ok(())
        } else {
            self.err(format!("{what} expected"))
        }
    }

    fn expect_period(&mut self) -> Result<()> {
        self.expect_tok(&Tok::Period, "period")
    }

    /// Accepts an optional period.
    fn opt_period(&mut self) {
        self.accept_tok(&Tok::Period);
    }

    fn is_period(&self) -> bool {
        self.is_tok(&Tok::Period)
    }

    /// A user-defined word (not checked against reserved words).
    fn name(&mut self) -> Result<String> {
        match self.peek().map(|t| &t.tok) {
            Some(Tok::Word(w)) => {
                let w = w.clone();
                self.i += 1;
                Ok(w)
            }
            // paragraph and section names may be all digits
            Some(Tok::Num(n)) if n.chars().all(|c| c.is_ascii_digit()) => {
                let n = n.clone();
                self.i += 1;
                Ok(n)
            }
            _ => self.err("name expected"),
        }
    }

    fn integer(&mut self) -> Result<u32> {
        match self.peek().map(|t| &t.tok) {
            Some(Tok::Num(n)) => match n.trim_start_matches('+').parse::<u32>() {
                Ok(v) => {
                    self.i += 1;
                    Ok(v)
                }
                Err(_) => self.err("unsigned integer expected"),
            },
            _ => self.err("integer expected"),
        }
    }

    fn is_integer(&self) -> bool {
        matches!(self.peek().map(|t| &t.tok), Some(Tok::Num(n)) if n.chars().all(|c| c.is_ascii_digit()))
    }

    /// Skips tokens up to and including the next period.
    fn skip_sentence(&mut self) {
        while let Some(t) = self.next() {
            if t.tok == Tok::Period {
                break;
            }
        }
    }

    fn is_division(&self, name: &str) -> bool {
        self.is(name) && self.is_at(1, "DIVISION")
    }

    // ---- program structure ---------------------------------------------

    fn program(&mut self) -> Result<Program> {
        let pos = self.pos();
        self.in_procedure = false;
        if !(self.is_division("IDENTIFICATION") || self.is_division("ID")) {
            return self.err("IDENTIFICATION DIVISION expected");
        }
        self.i += 2;
        self.expect_period()?;
        self.expect("PROGRAM-ID")?;
        self.opt_period();
        let name = match self.next().map(|t| t.tok) {
            Some(Tok::Word(w)) => w,
            Some(Tok::Str(s)) => s.to_ascii_uppercase(),
            _ => return self.err("program-name expected"),
        };
        let mut prog = Program {
            name,
            pos,
            ..Program::default()
        };
        self.accept("IS");
        loop {
            if self.accept("INITIAL") {
                prog.initial = true;
            } else if self.accept("COMMON") {
                prog.common = true;
            } else if self.accept("RECURSIVE") {
                prog.recursive = true;
            } else {
                break;
            }
        }
        self.accept("PROGRAM");
        self.opt_period();
        // Other identification paragraphs (comment entries are removed by
        // the source reader).
        while self.is_any(&[
            "AUTHOR",
            "INSTALLATION",
            "DATE-WRITTEN",
            "DATE-COMPILED",
            "SECURITY",
            "REMARKS",
        ]) {
            self.skip_sentence();
        }
        if self.is_division("ENVIRONMENT") {
            self.i += 2;
            self.expect_period()?;
            prog.env = self.environment()?;
        }
        if self.is_division("DATA") {
            self.i += 2;
            self.expect_period()?;
            prog.data = self.data_division()?;
            self.note_tables(&prog.data);
            self.in_procedure = true;
        }
        if self.is_division("PROCEDURE") {
            prog.procedure = self.procedure_division()?;
        }
        // Nested programs, then END PROGRAM.
        while self.is_division("IDENTIFICATION") || self.is_division("ID") {
            prog.nested.push(self.program()?);
        }
        if self.is("END") && self.is_at(1, "PROGRAM") {
            self.i += 2;
            self.name()?;
            self.opt_period();
        }
        Ok(prog)
    }

    // ---- environment division ------------------------------------------

    fn environment(&mut self) -> Result<Environment> {
        let mut env = Environment::default();
        loop {
            if (self.is("CONFIGURATION") || self.is("INPUT-OUTPUT")) && self.is_at(1, "SECTION") {
                self.i += 2;
                self.expect_period()?;
            } else if self.accept("SOURCE-COMPUTER") || self.accept("OBJECT-COMPUTER") {
                self.opt_period();
                self.computer_paragraph(&mut env)?;
            } else if self.accept("SPECIAL-NAMES") {
                self.expect_period()?;
                self.special_names(&mut env)?;
            } else if self.accept("FILE-CONTROL") {
                self.expect_period()?;
                while self.is("SELECT") {
                    env.files.push(self.select()?);
                }
            } else if self.accept("I-O-CONTROL") {
                self.expect_period()?;
                self.io_control(&mut env)?;
            } else {
                break;
            }
        }
        Ok(env)
    }

    fn computer_paragraph(&mut self, env: &mut Environment) -> Result<()> {
        // computer-name [WITH DEBUGGING MODE] [MEMORY SIZE ...]
        // [PROGRAM COLLATING SEQUENCE IS alphabet] [SEGMENT-LIMIT IS n] .
        while !self.is_period() && !self.eof() {
            if self.is("PROGRAM") || self.is("COLLATING") {
                self.accept("PROGRAM");
                self.accept("COLLATING");
                self.expect("SEQUENCE")?;
                self.accept("IS");
                env.program_collating = Some(self.name()?);
            } else if self.is_any(&["SPECIAL-NAMES", "FILE-CONTROL", "I-O-CONTROL"])
                || self.is_at(1, "SECTION")
                || self.is_at(1, "DIVISION")
            {
                return Ok(());
            } else if self.is("DEBUGGING") && self.is_at(1, "MODE") {
                env.debugging_mode = true;
                self.i += 2;
            } else {
                self.i += 1;
            }
        }
        self.opt_period();
        Ok(())
    }

    fn char_spec(&mut self) -> Result<CharSpec> {
        match self.peek().map(|t| t.tok.clone()) {
            Some(Tok::Str(s)) => {
                self.i += 1;
                Ok(CharSpec::Text(s))
            }
            Some(Tok::Num(_)) => Ok(CharSpec::Ordinal(self.integer()?)),
            Some(Tok::Word(w))
                if matches!(
                    w.as_str(),
                    "SPACE" | "SPACES" | "ZERO" | "ZEROS" | "ZEROES" | "QUOTE" | "QUOTES"
                ) =>
            {
                self.i += 1;
                Ok(CharSpec::Text(
                    match w.as_str() {
                        "SPACE" | "SPACES" => " ",
                        "QUOTE" | "QUOTES" => "\"",
                        _ => "0",
                    }
                    .to_string(),
                ))
            }
            Some(Tok::Word(w))
                if matches!(
                    w.as_str(),
                    "HIGH-VALUE" | "HIGH-VALUES" | "LOW-VALUE" | "LOW-VALUES"
                ) =>
            {
                self.i += 1;
                Ok(CharSpec::Ordinal(if w.starts_with("HIGH") {
                    256
                } else {
                    1
                }))
            }
            _ => self.err("literal expected"),
        }
    }

    fn alphabet_items(&mut self) -> Result<Vec<AlphabetItem>> {
        let first = self.char_spec()?;
        if self.accept("THRU") || self.accept("THROUGH") {
            let last = self.char_spec()?;
            Ok(vec![AlphabetItem::Range(first, last)])
        } else {
            Ok(vec![AlphabetItem::Char(first)])
        }
    }

    fn is_char_spec_start(&self) -> bool {
        matches!(
            self.peek().map(|t| &t.tok),
            Some(Tok::Str(_)) | Some(Tok::Num(_))
        ) || self.is_any(&[
            "SPACE",
            "SPACES",
            "ZERO",
            "ZEROS",
            "ZEROES",
            "QUOTE",
            "QUOTES",
            "HIGH-VALUE",
            "HIGH-VALUES",
            "LOW-VALUE",
            "LOW-VALUES",
        ])
    }

    fn special_names(&mut self, env: &mut Environment) -> Result<()> {
        loop {
            if self.is_period() {
                self.i += 1;
                break;
            }
            if self.eof()
                || self.is_any(&["FILE-CONTROL", "I-O-CONTROL"])
                || self.is_at(1, "SECTION")
                || self.is_at(1, "DIVISION")
            {
                break;
            }
            if self.accept("ALPHABET") {
                let name = self.name()?;
                self.accept("FOR");
                self.accept("ALPHANUMERIC");
                self.accept("IS");
                let kind = if self.accept("NATIVE") || self.accept("EBCDIC") {
                    AlphabetKind::Native
                } else if self.accept("STANDARD-1") {
                    AlphabetKind::StandardOne
                } else if self.accept("STANDARD-2") {
                    AlphabetKind::StandardTwo
                } else {
                    let mut groups = Vec::new();
                    while self.is_char_spec_start() {
                        let mut group = self.alphabet_items()?;
                        while self.accept("ALSO") {
                            group.extend(self.alphabet_items()?);
                        }
                        groups.push(group);
                    }
                    AlphabetKind::Literals(groups)
                };
                self.class_names.remove(&name);
                env.alphabets.push(Alphabet { name, kind });
            } else if self.accept("SYMBOLIC") {
                self.accept("CHARACTERS");
                let mut names = Vec::new();
                loop {
                    // name {name} ARE|IS int {int}
                    let mut group = Vec::new();
                    while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                        && !self.is_any(&["IS", "ARE", "IN"])
                    {
                        group.push(self.name()?);
                    }
                    if group.is_empty() {
                        break;
                    }
                    if !(self.accept("ARE") || self.accept("IS")) {
                        return self.err("ARE expected");
                    }
                    let mut ords = Vec::new();
                    while self.is_integer() {
                        ords.push(self.integer()?);
                    }
                    for (n, o) in group.into_iter().zip(ords) {
                        names.push((n, vec![o]));
                    }
                }
                let alphabet = if self.accept("IN") {
                    Some(self.name()?)
                } else {
                    None
                };
                for (n, o) in names {
                    env.symbolic_chars.push((n, o, alphabet.clone()));
                }
            } else if self.accept("CLASS") {
                let name = self.name()?;
                self.accept("IS");
                let mut items = Vec::new();
                while self.is_char_spec_start() {
                    items.extend(self.alphabet_items()?);
                }
                self.class_names.insert(name.clone());
                env.classes.push(ClassDef { name, items });
            } else if self.accept("CURRENCY") {
                self.accept("SIGN");
                self.accept("IS");
                match self.next().map(|t| t.tok) {
                    Some(Tok::Str(s)) => env.currency_sign = Some(s),
                    _ => return self.err("currency literal expected"),
                }
            } else if self.accept("DECIMAL-POINT") {
                self.accept("IS");
                self.expect("COMMA")?;
                env.decimal_point_comma = true;
            } else {
                // implementor-name IS mnemonic-name [ON STATUS IS c] [OFF STATUS IS c]
                let implementor = self.name()?;
                if self.accept("IS") || !self.is_any(&["ON", "OFF"]) {
                    let mnemonic = self.name()?;
                    self.mnemonics.insert(mnemonic.clone());
                    env.mnemonics.push((implementor.clone(), mnemonic));
                }
                loop {
                    let on = if self.accept("ON") {
                        true
                    } else if self.accept("OFF") {
                        false
                    } else {
                        break;
                    };
                    self.accept("STATUS");
                    self.accept("IS");
                    let cond = self.name()?;
                    self.mnemonics.insert(cond.clone());
                    env.switch_conditions.push((implementor.clone(), cond, on));
                }
            }
        }
        Ok(())
    }

    fn select(&mut self) -> Result<FileControl> {
        let pos = self.pos();
        self.expect("SELECT")?;
        let mut fc = FileControl {
            pos,
            optional: self.accept("OPTIONAL"),
            ..FileControl::default()
        };
        fc.name = self.name()?;
        while !self.is_period() && !self.eof() {
            if self.accept("ASSIGN") {
                self.accept("TO");
                self.accept("USING");
                fc.assign = match self.next().map(|t| t.tok) {
                    Some(Tok::Str(s)) => s,
                    Some(Tok::Word(w)) => w,
                    _ => return self.err("assignment name expected"),
                };
                // IBM allows several names; keep the first.
                while matches!(self.peek().map(|t| &t.tok), Some(Tok::Str(_)))
                    || (matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                        && !self.is_select_clause())
                {
                    self.i += 1;
                }
            } else if self.accept("ORGANIZATION") {
                self.accept("IS");
                fc.organization = self.organization()?;
            } else if self.is_any(&["SEQUENTIAL", "RELATIVE", "INDEXED", "LINE"]) {
                fc.organization = self.organization()?;
            } else if self.accept("ACCESS") {
                self.accept("MODE");
                self.accept("IS");
                fc.access = if self.accept("SEQUENTIAL") {
                    Access::Sequential
                } else if self.accept("RANDOM") {
                    Access::Random
                } else if self.accept("DYNAMIC") {
                    Access::Dynamic
                } else {
                    return self.err("access mode expected");
                };
                if self.accept("RELATIVE") {
                    self.accept("KEY");
                    self.accept("IS");
                    fc.relative_key = Some(self.data_ref()?);
                }
            } else if self.accept("RELATIVE") {
                self.accept("KEY");
                self.accept("IS");
                fc.relative_key = Some(self.data_ref()?);
            } else if self.accept("RECORD") {
                if self.accept("DELIMITER") {
                    self.accept("IS");
                    self.name()?;
                    fc.record_delimiter = true;
                } else {
                    self.accept("KEY");
                    self.accept("IS");
                    fc.record_key = Some(self.data_ref()?);
                }
            } else if self.accept("ALTERNATE") {
                self.accept("RECORD");
                self.accept("KEY");
                self.accept("IS");
                let key = self.data_ref()?;
                let dup = if self.accept("WITH") || self.is("DUPLICATES") {
                    self.expect("DUPLICATES")?;
                    true
                } else {
                    false
                };
                fc.alternate_keys.push((key, dup));
            } else if self.accept("FILE") || self.is("STATUS") {
                self.expect("STATUS")?;
                self.accept("IS");
                fc.file_status = Some(self.data_ref()?);
            } else if self.accept("RESERVE") {
                fc.reserve = Some(self.integer()?);
                self.accept("AREA");
                self.accept("AREAS");
            } else if self.accept("PADDING") {
                self.accept("CHARACTER");
                self.accept("IS");
                fc.padding = Some(self.operand()?);
            } else {
                return self.err("SELECT clause expected");
            }
        }
        self.expect_period()?;
        Ok(fc)
    }

    fn is_select_clause(&self) -> bool {
        self.is_any(&[
            "ORGANIZATION",
            "ACCESS",
            "SEQUENTIAL",
            "RELATIVE",
            "INDEXED",
            "LINE",
            "RECORD",
            "ALTERNATE",
            "FILE",
            "STATUS",
            "RESERVE",
            "PADDING",
        ])
    }

    fn organization(&mut self) -> Result<Organization> {
        if self.accept("LINE") {
            self.expect("SEQUENTIAL")?;
            Ok(Organization::LineSequential)
        } else if self.accept("RECORD") {
            self.expect("SEQUENTIAL")?;
            Ok(Organization::Sequential)
        } else if self.accept("SEQUENTIAL") {
            Ok(Organization::Sequential)
        } else if self.accept("RELATIVE") {
            Ok(Organization::Relative)
        } else if self.accept("INDEXED") {
            Ok(Organization::Indexed)
        } else {
            self.err("organization expected")
        }
    }

    fn io_control(&mut self, env: &mut Environment) -> Result<()> {
        loop {
            if self.is_period() {
                self.i += 1;
                if !self.is_any(&["SAME", "MULTIPLE", "RERUN", "APPLY"]) {
                    break;
                }
                continue;
            }
            if self.eof() || self.is_at(1, "SECTION") || self.is_at(1, "DIVISION") {
                break;
            }
            if self.accept("SAME") {
                let record = self.accept("RECORD");
                self.accept("SORT");
                self.accept("SORT-MERGE");
                self.accept("AREA");
                self.accept("FOR");
                let mut files = Vec::new();
                while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                    && !self.is_any(&["SAME", "MULTIPLE", "RERUN", "APPLY"])
                {
                    files.push(self.name()?);
                }
                if record {
                    env.same_record_area.push(files);
                }
            } else if self.accept("MULTIPLE") {
                env.multiple_file_tape = true;
                self.accept("FILE");
                self.accept("TAPE");
                self.accept("CONTAINS");
                while matches!(
                    self.peek().map(|t| &t.tok),
                    Some(Tok::Word(_)) | Some(Tok::Num(_))
                ) && !self.is_any(&["SAME", "MULTIPLE", "RERUN", "APPLY"])
                {
                    self.i += 1;
                }
            } else {
                self.i += 1;
            }
        }
        Ok(())
    }

    // ---- procedure division --------------------------------------------

    fn note_tables(&mut self, data: &DataDivision) {
        let sections = data.files.iter().map(|f| &f.records).chain([
            &data.working,
            &data.local,
            &data.linkage,
        ]);
        for entries in sections {
            // (level, within a table)
            let mut stack: Vec<(u8, bool)> = Vec::new();
            for e in entries {
                let level = if e.level == 88 || e.level == 66 {
                    99
                } else if e.level == 77 {
                    1
                } else {
                    e.level
                };
                if level != 99 {
                    while stack.last().is_some_and(|&(l, _)| l >= level) {
                        stack.pop();
                    }
                }
                let outer = stack.last().is_some_and(|&(_, t)| t);
                let table = outer || (level != 99 && e.occurs.is_some());
                if table && let Some(n) = &e.name {
                    self.tables.insert(n.clone());
                }
                if level != 99 {
                    stack.push((level, table));
                }
            }
        }
    }

    fn procedure_division(&mut self) -> Result<Procedure> {
        let pos = self.pos();
        self.i += 2;
        let mut proc = Procedure {
            pos,
            ..Procedure::default()
        };
        if self.accept("USING") {
            while !self.is_period() && !self.is_any(&["GIVING", "RETURNING"]) {
                self.accept("BY");
                self.accept("REFERENCE");
                if self.eof() {
                    break;
                }
                proc.using.push(self.data_ref()?);
            }
        }
        if self.accept("GIVING") || self.accept("RETURNING") {
            proc.giving = Some(self.data_ref()?);
        }
        self.expect_period()?;
        if self.accept("DECLARATIVES") {
            self.expect_period()?;
            while !(self.is("END") && self.is_at(1, "DECLARATIVES")) {
                if self.eof() {
                    return self.err("END DECLARATIVES expected");
                }
                proc.declaratives.push(self.section(true)?);
            }
            self.i += 2;
            self.expect_period()?;
        }
        while !self.eof() && !self.at_program_end() {
            proc.sections.push(self.section(false)?);
        }
        Ok(proc)
    }

    fn at_program_end(&self) -> bool {
        (self.is("END") && self.is_at(1, "PROGRAM"))
            || self.is_division("IDENTIFICATION")
            || self.is_division("ID")
    }

    fn is_section_header(&self) -> bool {
        matches!(
            self.peek().map(|t| &t.tok),
            Some(Tok::Word(_)) | Some(Tok::Num(_))
        ) && self.is_at(1, "SECTION")
    }

    fn is_paragraph_header(&self) -> bool {
        match self.peek().map(|t| &t.tok) {
            Some(Tok::Word(w)) => {
                !VERBS.contains(&w.as_str())
                    && self.peek_at(1).is_some_and(|t| t.tok == Tok::Period)
            }
            Some(Tok::Num(n)) => {
                n.chars().all(|c| c.is_ascii_digit())
                    && self.peek_at(1).is_some_and(|t| t.tok == Tok::Period)
            }
            _ => false,
        }
    }

    fn section(&mut self, declarative: bool) -> Result<Section> {
        let pos = self.pos();
        let mut sec = Section {
            pos,
            ..Section::default()
        };
        if self.is_section_header() {
            sec.name = Some(self.name()?);
            self.i += 1;
            if self.is_integer() {
                sec.segment = Some(self.integer()?);
            }
            self.expect_period()?;
            if declarative || self.is("USE") {
                sec.use_stmt = Some(self.use_statement()?);
            }
        }
        // Statements before the first paragraph form an unnamed paragraph.
        loop {
            if self.eof()
                || self.at_program_end()
                || (self.is("END") && self.is_at(1, "DECLARATIVES"))
            {
                break;
            }
            if self.is_section_header() {
                if sec.name.is_none() && sec.paragraphs.is_empty() {
                    // procedure division starts with a section
                    return self.section(declarative);
                }
                break;
            }
            let ppos = self.pos();
            let name = if self.is_paragraph_header() {
                let n = self.name()?;
                self.i += 1;
                Some(n)
            } else {
                None
            };
            let mut para = Paragraph {
                name,
                pos: ppos,
                sentences: Vec::new(),
            };
            while !(self.eof()
                || self.is_paragraph_header()
                || self.is_section_header()
                || self.at_program_end()
                || (self.is("END") && self.is_at(1, "DECLARATIVES")))
            {
                para.sentences.push(self.sentence()?);
            }
            sec.paragraphs.push(para);
        }
        Ok(sec)
    }

    fn use_statement(&mut self) -> Result<UseStmt> {
        self.expect("USE")?;
        let stmt = if self.accept("FOR") {
            self.expect("DEBUGGING")?;
            self.accept("ON");
            let mut names = Vec::new();
            while !self.is_period() && !self.eof() {
                if self.accept("ALL") {
                    if self.accept("PROCEDURES") {
                        names.push(DebugTarget::AllProcedures);
                        continue;
                    }
                    self.accept("REFERENCES");
                    self.accept("OF");
                    let name = self.data_ref()?;
                    names.push(DebugTarget::Name {
                        name,
                        all_refs: true,
                    });
                } else {
                    let name = self.data_ref()?;
                    names.push(DebugTarget::Name {
                        name,
                        all_refs: false,
                    });
                }
            }
            UseStmt::Debugging(names)
        } else if self.accept("BEFORE") {
            self.expect("REPORTING")?;
            UseStmt::BeforeReporting(self.name()?)
        } else {
            let global = self.accept("GLOBAL");
            self.accept("AFTER");
            self.accept("STANDARD");
            if !(self.accept("EXCEPTION") || self.accept("ERROR")) {
                return self.err("EXCEPTION or ERROR expected");
            }
            self.accept("PROCEDURE");
            self.accept("ON");
            let target = if self.accept("INPUT") {
                UseTarget::Input
            } else if self.accept("OUTPUT") {
                UseTarget::Output
            } else if self.accept("I-O") {
                UseTarget::InputOutput
            } else if self.accept("EXTEND") {
                UseTarget::Extend
            } else {
                let mut files = Vec::new();
                while !self.is_period() && !self.eof() {
                    files.push(self.name()?);
                }
                UseTarget::Files(files)
            };
            UseStmt::Error(target, global)
        };
        self.expect_period()?;
        Ok(stmt)
    }

    /// Statements up to and including a period.
    fn sentence(&mut self) -> Result<Vec<Stmt>> {
        let stmts = self.statements()?;
        if stmts.is_empty() && !self.is_period() {
            return self.err("statement expected");
        }
        self.expect_period()?;
        Ok(stmts)
    }
}

#[cfg(test)]
mod tests;

//! Procedure division statements.

use super::{Parser, VERBS};
use crate::diag::Result;
use crate::front::ast::*;
use crate::front::lexer::Tok;

/// Exception phrases recognised after I/O and arithmetic statements.
#[derive(Clone, Copy)]
enum Phrase {
    SizeError,
    AtEnd,
    InvalidKey,
    Overflow,
    Exception,
    EndOfPage,
}

impl Parser {
    fn is_verb(&self) -> bool {
        matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(w)) if VERBS.contains(&w.as_str()))
            || (self.is("NEXT") && self.is_at(1, "SENTENCE"))
    }

    /// Statements while the next token starts a statement.
    pub(super) fn statements(&mut self) -> Result<Vec<Stmt>> {
        let mut out = Vec::new();
        while self.is_verb() {
            out.push(self.statement()?);
        }
        Ok(out)
    }

    fn end(&mut self, terminator: &str) {
        self.accept(terminator);
    }

    /// Recognises the start of an exception phrase; returns (negated,
    /// tokens to skip).
    fn phrase_start(&self, p: Phrase) -> Option<(bool, usize)> {
        let mut k = 0;
        let not = self.is_at(0, "NOT");
        if not {
            k += 1;
        }
        let matched = match p {
            Phrase::SizeError => {
                if self.is_at(k, "ON") {
                    k += 1;
                }
                if self.is_at(k, "SIZE") && self.is_at(k + 1, "ERROR") {
                    k += 2;
                    true
                } else {
                    false
                }
            }
            Phrase::AtEnd => {
                if self.is_at(k, "AT") {
                    k += 1;
                }
                if self.is_at(k, "END") {
                    k += 1;
                    true
                } else {
                    false
                }
            }
            Phrase::InvalidKey => {
                if self.is_at(k, "INVALID") {
                    k += 1;
                    if self.is_at(k, "KEY") {
                        k += 1;
                    }
                    true
                } else {
                    false
                }
            }
            Phrase::Overflow => {
                if self.is_at(k, "ON") {
                    k += 1;
                }
                if self.is_at(k, "OVERFLOW") {
                    k += 1;
                    true
                } else {
                    false
                }
            }
            Phrase::Exception => {
                if self.is_at(k, "ON") {
                    k += 1;
                }
                if self.is_at(k, "EXCEPTION") {
                    k += 1;
                    true
                } else {
                    false
                }
            }
            Phrase::EndOfPage => {
                if self.is_at(k, "AT") {
                    k += 1;
                }
                if self.is_at(k, "END-OF-PAGE") || self.is_at(k, "EOP") {
                    k += 1;
                    true
                } else {
                    false
                }
            }
        };
        matched.then_some((not, k))
    }

    fn handlers(&mut self, phrases: &[Phrase]) -> Result<Handlers> {
        let mut h = Handlers::default();
        loop {
            let found = phrases.iter().find_map(|p| self.phrase_start(*p));
            let Some((not, skip)) = found else {
                break;
            };
            self.i += skip;
            let body = self.statements()?;
            if not {
                h.not_on = Some(body);
            } else {
                h.on = Some(body);
            }
        }
        Ok(h)
    }

    /// Receivers with optional ROUNDED.
    fn receivers(&mut self) -> Result<Vec<Receiver>> {
        let mut out = Vec::new();
        while self.is_operand_start() {
            let target = self.data_ref()?;
            let rounded = self.accept("ROUNDED");
            out.push(Receiver { target, rounded });
        }
        Ok(out)
    }

    fn operand_exprs(&mut self) -> Result<Vec<Expr>> {
        let mut out = Vec::new();
        while self.is_operand_start() {
            out.push(Expr::Operand(self.operand()?));
        }
        Ok(out)
    }

    fn proc_name(&mut self) -> Result<ProcName> {
        let pos = self.pos();
        let name = self.name()?;
        let section = if (self.is("OF") || self.is("IN"))
            && matches!(
                self.peek_at(1).map(|t| &t.tok),
                Some(Tok::Word(_)) | Some(Tok::Num(_))
            ) {
            self.i += 1;
            Some(self.name()?)
        } else {
            None
        };
        Ok(ProcName { name, section, pos })
    }

    fn proc_range(&mut self) -> Result<(ProcName, Option<ProcName>)> {
        let a = self.proc_name()?;
        let b = if self.accept("THRU") || self.accept("THROUGH") {
            Some(self.proc_name()?)
        } else {
            None
        };
        Ok((a, b))
    }

    fn file_names(&mut self, stop: &[&str]) -> Result<Vec<String>> {
        let mut out = Vec::new();
        while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
            && !self.is_verb()
            && !self.is_any(stop)
        {
            out.push(self.name()?);
        }
        Ok(out)
    }

    fn statement(&mut self) -> Result<Stmt> {
        let pos = self.pos();
        if self.is("NEXT") && self.is_at(1, "SENTENCE") {
            self.i += 2;
            return Ok(Stmt::NextSentence);
        }
        let verb = self
            .next()
            .and_then(|t| t.word().map(str::to_string))
            .unwrap();
        Ok(match verb.as_str() {
            // Communication statements (module CM, not supported by
            // Enterprise COBOL): accepted, fail if executed.
            "ENABLE" | "DISABLE" | "SEND" | "RECEIVE" | "PURGE" => {
                while !self.eof() && !self.is_period() && !self.is_verb() {
                    if self.is("END-RECEIVE") {
                        self.i += 1;
                        break;
                    }
                    self.i += 1;
                }
                Stmt::Unsupported(verb, pos)
            }
            "INITIATE" | "TERMINATE" => {
                let mut names = Vec::new();
                while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                    && !self.is_verb()
                    && !self.is_period()
                {
                    names.push(self.name()?);
                }
                if verb == "INITIATE" {
                    Stmt::Initiate(names)
                } else {
                    Stmt::Terminate(names)
                }
            }
            "GENERATE" => Stmt::Generate(self.data_ref()?),
            "SUPPRESS" => {
                self.accept("PRINTING");
                Stmt::SuppressPrinting
            }
            "ACCEPT" if self.is_at(1, "MESSAGE") => {
                let target = self.data_ref()?;
                self.expect("MESSAGE")?;
                self.expect("COUNT")?;
                let _ = target;
                Stmt::Unsupported("ACCEPT MESSAGE COUNT".to_string(), pos)
            }
            "ACCEPT" => {
                let target = self.data_ref()?;
                let from = if self.accept("FROM") {
                    let mut src = self.name()?;
                    if let Some(fmt) = self.accept_any(&["YYYYMMDD", "YYYYDDD"]) {
                        src = format!("{src} {fmt}");
                    }
                    Some(src)
                } else {
                    None
                };
                self.end("END-ACCEPT");
                Stmt::Accept { target, from, pos }
            }
            "ADD" => self.add(pos)?,
            "ALTER" => {
                let mut pairs = Vec::new();
                while matches!(
                    self.peek().map(|t| &t.tok),
                    Some(Tok::Word(_)) | Some(Tok::Num(_))
                ) && !self.is_verb()
                {
                    let from = self.proc_name()?;
                    self.expect("TO")?;
                    self.accept("PROCEED");
                    self.accept("TO");
                    let to = self.proc_name()?;
                    pairs.push((from, to));
                }
                Stmt::Alter(pairs)
            }
            "CALL" => {
                let program = self.operand()?;
                let mut using = Vec::new();
                if self.accept("USING") {
                    let mut mode = 0;
                    loop {
                        if self.accept("BY") || self.is_any(&["REFERENCE", "CONTENT", "VALUE"]) {
                            mode = if self.accept("REFERENCE") {
                                0
                            } else if self.accept("CONTENT") {
                                1
                            } else if self.accept("VALUE") {
                                2
                            } else {
                                return self.err("REFERENCE, CONTENT or VALUE expected");
                            };
                        }
                        if self.accept("OMITTED") {
                            using.push(CallArg::Omitted);
                            continue;
                        }
                        if !self.is_operand_start() {
                            break;
                        }
                        let op = self.operand()?;
                        using.push(match mode {
                            0 => CallArg::Reference(op),
                            1 => CallArg::Content(op),
                            _ => CallArg::Value(op),
                        });
                    }
                }
                let returning = if self.accept("RETURNING") || self.accept("GIVING") {
                    Some(self.data_ref()?)
                } else {
                    None
                };
                let exception = self.handlers(&[Phrase::Overflow, Phrase::Exception])?;
                self.end("END-CALL");
                Stmt::Call {
                    program,
                    using,
                    returning,
                    exception,
                    pos,
                }
            }
            "CANCEL" => {
                let mut progs = Vec::new();
                while self.is_operand_start() {
                    progs.push(self.operand()?);
                }
                Stmt::Cancel(progs)
            }
            "CLOSE" => {
                let mut files = Vec::new();
                while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_))) && !self.is_verb() {
                    if self.is_any(&["END-CLOSE"]) {
                        break;
                    }
                    let f = self.name()?;
                    let mut opt = None;
                    if self.accept("REEL") || self.accept("UNIT") {
                        opt = Some("REEL".to_string());
                        if self.accept("FOR") {
                            self.expect("REMOVAL")?;
                        }
                    }
                    if self.accept("WITH") || self.is("NO") || self.is("LOCK") {
                        if self.accept("NO") {
                            self.expect("REWIND")?;
                            opt = Some("NO REWIND".to_string());
                        } else {
                            self.expect("LOCK")?;
                            opt = Some("LOCK".to_string());
                        }
                    }
                    files.push((f, opt));
                }
                Stmt::Close(files)
            }
            "COMPUTE" => {
                let targets = self.receivers()?;
                if !(self.accept_tok(&Tok::Eq) || self.accept("EQUAL")) {
                    return self.err("= expected");
                }
                let expr = self.expr()?;
                let size_error = self.handlers(&[Phrase::SizeError])?;
                self.end("END-COMPUTE");
                Stmt::Compute {
                    targets,
                    expr,
                    size_error,
                    pos,
                }
            }
            "CONTINUE" => Stmt::Continue,
            "DELETE" => {
                let file = self.name()?;
                self.accept("RECORD");
                let invalid_key = self.handlers(&[Phrase::InvalidKey])?;
                self.end("END-DELETE");
                Stmt::Delete { file, invalid_key }
            }
            "DISPLAY" => {
                let mut items = Vec::new();
                while self.is_operand_start() && !self.is("UPON") {
                    items.push(self.operand()?);
                }
                let upon = if self.accept("UPON") {
                    Some(self.name()?)
                } else {
                    None
                };
                let mut no_advancing = false;
                if self.accept("WITH") || self.is("NO") {
                    self.expect("NO")?;
                    self.expect("ADVANCING")?;
                    no_advancing = true;
                }
                self.end("END-DISPLAY");
                Stmt::Display {
                    items,
                    upon,
                    no_advancing,
                }
            }
            "DIVIDE" => self.divide(pos)?,
            "EVALUATE" => self.evaluate()?,
            "EXIT" => {
                if self.accept("PROGRAM") {
                    Stmt::ExitProgram
                } else if self.accept("PARAGRAPH") {
                    Stmt::ExitParagraph
                } else if self.accept("SECTION") {
                    Stmt::ExitSection
                } else {
                    Stmt::Exit
                }
            }
            "GO" => {
                self.accept("TO");
                // A list of targets only with DEPENDING ON; otherwise one.
                let mut k = 0;
                let mut depending = false;
                while let Some(t) = self.peek_at(k) {
                    match &t.tok {
                        Tok::Word(w) if w == "DEPENDING" => {
                            depending = true;
                            break;
                        }
                        Tok::Word(w) if VERBS.contains(&w.as_str()) => break,
                        Tok::Word(_) | Tok::Num(_) => k += 1,
                        _ => break,
                    }
                }
                let mut targets = Vec::new();
                if depending {
                    while !self.is("DEPENDING") {
                        targets.push(self.proc_name()?);
                    }
                } else if matches!(
                    self.peek().map(|t| &t.tok),
                    Some(Tok::Word(_)) | Some(Tok::Num(_))
                ) && !self.is_verb()
                    && !self.is_any(&[
                        "ELSE",
                        "WHEN",
                        "END-IF",
                        "END-PERFORM",
                        "END-EVALUATE",
                        "NOT",
                    ])
                {
                    targets.push(self.proc_name()?);
                }
                let depending = if self.accept("DEPENDING") {
                    self.accept("ON");
                    Some(self.data_ref()?)
                } else {
                    None
                };
                Stmt::GoTo {
                    targets,
                    depending,
                    pos,
                }
            }
            "GOBACK" => Stmt::GoBack,
            "IF" => {
                let cond = self.cond()?;
                self.accept("THEN");
                let then = self.statements()?;
                let els = if self.accept("ELSE") {
                    Some(self.statements()?)
                } else {
                    None
                };
                self.end("END-IF");
                Stmt::If { cond, then, els }
            }
            "INITIALIZE" => {
                let mut targets = Vec::new();
                while self.is_operand_start() {
                    targets.push(self.data_ref()?);
                }
                let mut replacing = Vec::new();
                if self.accept("REPLACING") {
                    while let Some(cat) = self.accept_any(&[
                        "ALPHANUMERIC-EDITED",
                        "NUMERIC-EDITED",
                        "ALPHABETIC",
                        "ALPHANUMERIC",
                        "NUMERIC",
                    ]) {
                        self.accept("DATA");
                        self.expect("BY")?;
                        replacing.push((cat, self.operand()?));
                    }
                }
                Stmt::Initialize { targets, replacing }
            }
            "INSPECT" => self.inspect()?,
            "MERGE" => {
                let file = self.name()?;
                let keys = self.sort_keys()?;
                let collating = self.collating()?;
                self.expect("USING")?;
                let using = self.file_names(&["GIVING", "OUTPUT"])?;
                let output = self.sort_output()?;
                Stmt::Merge {
                    file,
                    keys,
                    collating,
                    using,
                    output,
                }
            }
            "MOVE" => {
                let corresponding = self.accept("CORRESPONDING") || self.accept("CORR");
                let from = self.operand()?;
                self.expect("TO")?;
                let mut to = Vec::new();
                while self.is_operand_start() {
                    to.push(self.data_ref()?);
                }
                if to.is_empty() {
                    return self.err("receiving item expected");
                }
                Stmt::Move {
                    from,
                    to,
                    corresponding,
                    pos,
                }
            }
            "MULTIPLY" => {
                let by_ = Expr::Operand(self.operand()?);
                self.expect("BY")?;
                let (into, giving, operand) = if self.literal_operand_peek() {
                    let op = Expr::Operand(self.operand()?);
                    self.expect("GIVING")?;
                    (Vec::new(), self.receivers()?, op)
                } else {
                    let rest = self.receivers()?;
                    if self.accept("GIVING") {
                        let operand = match rest.as_slice() {
                            [r] if !r.rounded => Expr::Operand(Operand::Ref(r.target.clone())),
                            _ => return self.err("one operand expected before GIVING"),
                        };
                        (Vec::new(), self.receivers()?, operand)
                    } else {
                        (rest, Vec::new(), by_.clone())
                    }
                };
                let size_error = self.handlers(&[Phrase::SizeError])?;
                self.end("END-MULTIPLY");
                Stmt::Multiply {
                    by: by_,
                    operand,
                    into,
                    giving,
                    size_error,
                    pos,
                }
            }
            "OPEN" => {
                let mut groups = Vec::new();
                loop {
                    let mode = if self.accept("INPUT") {
                        OpenMode::Input
                    } else if self.accept("OUTPUT") {
                        OpenMode::Output
                    } else if self.accept("I-O") {
                        OpenMode::InputOutput
                    } else if self.accept("EXTEND") {
                        OpenMode::Extend
                    } else {
                        break;
                    };
                    let mut files = Vec::new();
                    while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                        && !self.is_verb()
                        && !self.is_any(&["INPUT", "OUTPUT", "I-O", "EXTEND"])
                    {
                        if self.accept("REVERSED") || self.accept("WITH") {
                            continue;
                        }
                        if self.accept("NO") {
                            self.expect("REWIND")?;
                            continue;
                        }
                        files.push(self.name()?);
                    }
                    groups.push((mode, files));
                }
                Stmt::Open(groups)
            }
            "PERFORM" => self.perform(pos)?,
            "READ" => {
                let file = self.name()?;
                let next = self.accept("NEXT");
                self.accept("RECORD");
                let into = if self.accept("INTO") {
                    Some(self.data_ref()?)
                } else {
                    None
                };
                let key = if self.accept("KEY") {
                    self.accept("IS");
                    Some(self.data_ref()?)
                } else {
                    None
                };
                let h = self.handlers(&[Phrase::AtEnd, Phrase::InvalidKey])?;
                self.end("END-READ");
                // AT END and INVALID KEY never appear together.
                Stmt::Read {
                    file,
                    next,
                    into,
                    key,
                    at_end: h.clone(),
                    invalid_key: h,
                }
            }
            "RELEASE" => {
                let record = self.data_ref()?;
                let from = if self.accept("FROM") {
                    Some(self.operand()?)
                } else {
                    None
                };
                Stmt::Release { record, from }
            }
            "RETURN" => {
                let file = self.name()?;
                self.accept("RECORD");
                let into = if self.accept("INTO") {
                    Some(self.data_ref()?)
                } else {
                    None
                };
                let at_end = self.handlers(&[Phrase::AtEnd])?;
                self.end("END-RETURN");
                Stmt::Return { file, into, at_end }
            }
            "REWRITE" => {
                let record = self.data_ref()?;
                let from = if self.accept("FROM") {
                    Some(self.operand()?)
                } else {
                    None
                };
                let invalid_key = self.handlers(&[Phrase::InvalidKey])?;
                self.end("END-REWRITE");
                Stmt::Rewrite {
                    record,
                    from,
                    invalid_key,
                }
            }
            "SEARCH" => {
                let all = self.accept("ALL");
                let table = self.data_ref()?;
                let varying = if self.accept("VARYING") {
                    Some(self.data_ref()?)
                } else {
                    None
                };
                let at_end = if self.phrase_start(Phrase::AtEnd).is_some_and(|(n, _)| !n) {
                    let (_, k) = self.phrase_start(Phrase::AtEnd).unwrap();
                    self.i += k;
                    Some(self.statements()?)
                } else {
                    None
                };
                let mut whens = Vec::new();
                while self.accept("WHEN") {
                    let c = self.cond()?;
                    let body = self.statements()?;
                    whens.push((c, body));
                }
                self.end("END-SEARCH");
                Stmt::Search {
                    table,
                    all,
                    varying,
                    at_end,
                    whens,
                }
            }
            "SET" => self.set()?,
            "SORT" => {
                let file = self.name()?;
                let keys = self.sort_keys()?;
                let mut duplicates = false;
                if self.accept("WITH") || self.is("DUPLICATES") {
                    self.expect("DUPLICATES")?;
                    self.accept("IN");
                    self.accept("ORDER");
                    duplicates = true;
                }
                let collating = self.collating()?;
                let input = if self.accept("USING") {
                    SortIo::Using(self.file_names(&["GIVING", "OUTPUT"])?)
                } else {
                    self.expect("INPUT")?;
                    self.expect("PROCEDURE")?;
                    self.accept("IS");
                    let (a, b) = self.proc_range()?;
                    SortIo::Procedure(a, b)
                };
                let output = self.sort_output()?;
                Stmt::Sort {
                    file,
                    keys,
                    duplicates,
                    collating,
                    input,
                    output,
                }
            }
            "START" => {
                let file = self.name()?;
                let key = if self.accept("KEY") {
                    let op = match self.relop() {
                        Some(op) => op,
                        None => return self.err("relational operator expected"),
                    };
                    Some((op, self.data_ref()?))
                } else {
                    None
                };
                let invalid_key = self.handlers(&[Phrase::InvalidKey])?;
                self.end("END-START");
                Stmt::Start {
                    file,
                    key,
                    invalid_key,
                }
            }
            "STOP" => {
                if self.accept("RUN") {
                    Stmt::StopRun
                } else {
                    Stmt::StopLiteral(self.operand()?)
                }
            }
            "STRING" => {
                let mut sources = Vec::new();
                loop {
                    let mut ops = Vec::new();
                    while self.is_operand_start() && !self.is("DELIMITED") {
                        ops.push(self.operand()?);
                    }
                    if ops.is_empty() {
                        break;
                    }
                    let delim = if self.accept("DELIMITED") {
                        self.accept("BY");
                        if self.accept("SIZE") {
                            None
                        } else {
                            Some(self.operand()?)
                        }
                    } else {
                        None
                    };
                    for op in ops {
                        sources.push((op, delim.clone()));
                    }
                }
                self.expect("INTO")?;
                let into = self.data_ref()?;
                let pointer = if self.accept("WITH") || self.is("POINTER") {
                    self.expect("POINTER")?;
                    Some(self.data_ref()?)
                } else {
                    None
                };
                let overflow = self.handlers(&[Phrase::Overflow])?;
                self.end("END-STRING");
                Stmt::String {
                    sources,
                    into,
                    pointer,
                    overflow,
                }
            }
            "SUBTRACT" => self.subtract(pos)?,
            "UNSTRING" => {
                let source = self.data_ref()?;
                let mut delimiters = Vec::new();
                if self.accept("DELIMITED") {
                    self.accept("BY");
                    loop {
                        let all = self.accept("ALL");
                        delimiters.push((all, self.operand()?));
                        if !self.accept("OR") {
                            break;
                        }
                    }
                }
                self.expect("INTO")?;
                let mut into = Vec::new();
                while self.is_operand_start() {
                    let target = self.data_ref()?;
                    let delim = if self.accept("DELIMITER") {
                        self.accept("IN");
                        Some(self.data_ref()?)
                    } else {
                        None
                    };
                    let count = if self.accept("COUNT") {
                        self.accept("IN");
                        Some(self.data_ref()?)
                    } else {
                        None
                    };
                    into.push((target, delim, count));
                }
                let pointer = if self.accept("WITH") || self.is("POINTER") {
                    self.expect("POINTER")?;
                    Some(self.data_ref()?)
                } else {
                    None
                };
                let tallying = if self.accept("TALLYING") {
                    self.accept("IN");
                    Some(self.data_ref()?)
                } else {
                    None
                };
                let overflow = self.handlers(&[Phrase::Overflow])?;
                self.end("END-UNSTRING");
                Stmt::Unstring {
                    source,
                    delimiters,
                    into,
                    pointer,
                    tallying,
                    overflow,
                }
            }
            "WRITE" => {
                let record = self.data_ref()?;
                let from = if self.accept("FROM") {
                    Some(self.operand()?)
                } else {
                    None
                };
                let advancing = if self.is("BEFORE") || self.is("AFTER") {
                    let before = self.accept("BEFORE");
                    if !before {
                        self.i += 1;
                    }
                    self.accept("ADVANCING");
                    let adv = if self.accept("PAGE") {
                        Advancing::Page
                    } else if let Some(Tok::Word(w)) = self.peek().map(|t| &t.tok)
                        && self.mnemonics.contains(w)
                    {
                        let w = w.clone();
                        self.i += 1;
                        Advancing::Mnemonic(w)
                    } else {
                        let n = self.operand()?;
                        self.accept("LINES");
                        self.accept("LINE");
                        Advancing::Lines(n)
                    };
                    Some((before, adv))
                } else {
                    None
                };
                let h = self.handlers(&[Phrase::EndOfPage, Phrase::InvalidKey])?;
                self.end("END-WRITE");
                Stmt::Write {
                    record,
                    from,
                    advancing,
                    end_of_page: h.clone(),
                    invalid_key: h,
                }
            }
            other => Stmt::Unsupported(other.to_string(), pos),
        })
    }

    fn literal_operand_peek(&self) -> bool {
        matches!(
            self.peek().map(|t| &t.tok),
            Some(Tok::Num(_)) | Some(Tok::Str(_))
        )
    }

    fn add(&mut self, pos: crate::front::source::Pos) -> Result<Stmt> {
        if self.accept("CORRESPONDING") || self.accept("CORR") {
            let from = Expr::Operand(Operand::Ref(self.data_ref()?));
            self.expect("TO")?;
            let to = self.receivers()?;
            let size_error = self.handlers(&[Phrase::SizeError])?;
            self.end("END-ADD");
            return Ok(Stmt::Add {
                from: vec![from],
                to,
                giving: Vec::new(),
                corresponding: true,
                size_error,
                pos,
            });
        }
        let mut from = self.operand_exprs()?;
        let mut to = Vec::new();
        let mut giving = Vec::new();
        if self.accept("TO") {
            // operands after TO are receivers unless GIVING follows
            let save = self.i;
            let rcv = self.receivers_or_literals()?;
            if self.is("GIVING") {
                self.i = save;
                from.extend(self.operand_exprs()?);
            } else {
                to = rcv;
            }
        }
        if self.accept("GIVING") {
            giving = self.receivers()?;
        }
        let size_error = self.handlers(&[Phrase::SizeError])?;
        self.end("END-ADD");
        Ok(Stmt::Add {
            from,
            to,
            giving,
            corresponding: false,
            size_error,
            pos,
        })
    }

    /// Receivers, tolerating literals (which are only valid before GIVING).
    fn receivers_or_literals(&mut self) -> Result<Vec<Receiver>> {
        let mut out = Vec::new();
        loop {
            if self.literal_operand_peek() {
                self.i += 1;
                continue;
            }
            if !self.is_operand_start() {
                break;
            }
            let target = self.data_ref()?;
            let rounded = self.accept("ROUNDED");
            out.push(Receiver { target, rounded });
        }
        Ok(out)
    }

    fn subtract(&mut self, pos: crate::front::source::Pos) -> Result<Stmt> {
        if self.accept("CORRESPONDING") || self.accept("CORR") {
            let from = Expr::Operand(Operand::Ref(self.data_ref()?));
            self.expect("FROM")?;
            let to = self.receivers()?;
            let size_error = self.handlers(&[Phrase::SizeError])?;
            self.end("END-SUBTRACT");
            return Ok(Stmt::Subtract {
                from: vec![from],
                subtrahend_of: to,
                giving: Vec::new(),
                corresponding: true,
                size_error,
                pos,
            });
        }
        let mut from = self.operand_exprs()?;
        self.expect("FROM")?;
        let save = self.i;
        let mut rcv = self.receivers_or_literals()?;
        let mut giving = Vec::new();
        if self.is("GIVING") {
            self.i = save;
            // the minuend: a single operand
            let minuend = Expr::Operand(self.operand()?);
            from.insert(0, minuend);
            self.expect("GIVING")?;
            giving = self.receivers()?;
            rcv = Vec::new();
        }
        let size_error = self.handlers(&[Phrase::SizeError])?;
        self.end("END-SUBTRACT");
        Ok(Stmt::Subtract {
            from,
            subtrahend_of: rcv,
            giving,
            corresponding: false,
            size_error,
            pos,
        })
    }

    fn divide(&mut self, pos: crate::front::source::Pos) -> Result<Stmt> {
        let first = Expr::Operand(self.operand()?);
        let stmt_divisor;
        let mut dividend = None;
        let mut into = Vec::new();
        let mut giving = Vec::new();
        if self.accept("INTO") {
            stmt_divisor = first;
            let save = self.i;
            let rcv = self.receivers_or_literals()?;
            if self.is("GIVING") {
                self.i = save;
                dividend = Some(Expr::Operand(self.operand()?));
                self.expect("GIVING")?;
                giving = self.receivers()?;
            } else {
                into = rcv;
            }
        } else {
            self.expect("BY")?;
            dividend = Some(first);
            stmt_divisor = Expr::Operand(self.operand()?);
            self.expect("GIVING")?;
            giving = self.receivers()?;
        }
        let remainder = if self.accept("REMAINDER") {
            Some(self.data_ref()?)
        } else {
            None
        };
        let size_error = self.handlers(&[Phrase::SizeError])?;
        self.end("END-DIVIDE");
        Ok(Stmt::Divide {
            divisor: stmt_divisor,
            dividend,
            into,
            giving,
            remainder,
            size_error,
            pos,
        })
    }

    fn eval_subject(&mut self) -> Result<EvalSubject> {
        if self.accept("TRUE") {
            return Ok(EvalSubject::True);
        }
        if self.accept("FALSE") {
            return Ok(EvalSubject::False);
        }
        let save = self.i;
        if let Ok(e) = self.expr()
            && (self.is("ALSO") || self.is("WHEN"))
        {
            return Ok(EvalSubject::Expr(e));
        }
        self.i = save;
        Ok(EvalSubject::Cond(self.cond()?))
    }

    fn eval_object(&mut self) -> Result<EvalObject> {
        if self.accept("ANY") {
            return Ok(EvalObject::Any);
        }
        if self.accept("TRUE") {
            return Ok(EvalObject::True);
        }
        if self.accept("FALSE") {
            return Ok(EvalObject::False);
        }
        let save = self.i;
        let not = self.accept("NOT");
        if let Ok(from) = self.expr() {
            let thru = if self.accept("THRU") || self.accept("THROUGH") {
                Some(self.expr()?)
            } else {
                None
            };
            if self.is("ALSO")
                || self.is("WHEN")
                || self.is_verb()
                || self.is_period()
                || self.is("END-EVALUATE")
            {
                return Ok(EvalObject::Value { not, from, thru });
            }
        }
        self.i = save;
        Ok(EvalObject::Cond(self.cond()?))
    }

    fn evaluate(&mut self) -> Result<Stmt> {
        let mut subjects = vec![self.eval_subject()?];
        while self.accept("ALSO") {
            subjects.push(self.eval_subject()?);
        }
        let mut whens = Vec::new();
        let mut other = None;
        while self.is("WHEN") {
            if self.is_at(1, "OTHER") {
                self.i += 2;
                other = Some(self.statements()?);
                break;
            }
            let mut clauses = Vec::new();
            while self.is("WHEN") && !self.is_at(1, "OTHER") {
                self.i += 1;
                let mut objects = vec![self.eval_object()?];
                while self.accept("ALSO") {
                    objects.push(self.eval_object()?);
                }
                clauses.push(WhenClause { objects });
            }
            let body = self.statements()?;
            whens.push((clauses, body));
        }
        self.end("END-EVALUATE");
        Ok(Stmt::Evaluate {
            subjects,
            whens,
            other,
        })
    }

    fn delimiters(&mut self) -> Result<Vec<(bool, Operand)>> {
        let mut out = Vec::new();
        while self.is("BEFORE") || self.is("AFTER") {
            let before = self.accept("BEFORE");
            if !before {
                self.i += 1;
            }
            self.accept("INITIAL");
            out.push((before, self.operand()?));
        }
        Ok(out)
    }

    fn inspect(&mut self) -> Result<Stmt> {
        let target = self.data_ref()?;
        let mut ops = Vec::new();
        loop {
            if self.accept("TALLYING") {
                let mut clauses = Vec::new();
                while self.is_operand_start() && self.is_at(1, "FOR")
                    || (self.is_operand_start() && self.tally_counter_follows())
                {
                    let counter = self.data_ref()?;
                    self.expect("FOR")?;
                    let mut items = Vec::new();
                    loop {
                        if self.accept("CHARACTERS") {
                            items.push(TallyItem::Characters(self.delimiters()?));
                        } else if self.is("ALL") || self.is("LEADING") {
                            let leading = self.accept("LEADING");
                            if !leading {
                                self.i += 1;
                            }
                            // one or more patterns, each with delimiters
                            loop {
                                if !self.is_inspect_operand() {
                                    break;
                                }
                                let pat = self.operand()?;
                                let d = self.delimiters()?;
                                items.push(TallyItem::Pattern(leading, pat, d));
                                if self.is_operand_start() && self.is_at(1, "FOR") {
                                    break;
                                }
                            }
                        } else {
                            break;
                        }
                    }
                    clauses.push(TallyClause { counter, items });
                }
                ops.push(InspectOp::Tallying(clauses));
            } else if self.accept("REPLACING") {
                let mut clauses = Vec::new();
                loop {
                    if self.accept("CHARACTERS") {
                        self.expect("BY")?;
                        let by = self.operand()?;
                        let d = self.delimiters()?;
                        clauses.push(ReplaceClause::Characters(by, d));
                    } else if let Some(m) = self.accept_any(&["ALL", "LEADING", "FIRST"]) {
                        let mode = match m.as_str() {
                            "ALL" => ReplaceMode::All,
                            "LEADING" => ReplaceMode::Leading,
                            _ => ReplaceMode::First,
                        };
                        loop {
                            if !self.is_inspect_operand() {
                                break;
                            }
                            let pat = self.operand()?;
                            self.expect("BY")?;
                            let by = self.operand()?;
                            let d = self.delimiters()?;
                            clauses.push(ReplaceClause::Pattern(mode, pat, by, d));
                        }
                    } else {
                        break;
                    }
                }
                ops.push(InspectOp::Replacing(clauses));
            } else if self.accept("CONVERTING") {
                let from = self.operand()?;
                self.expect("TO")?;
                let to = self.operand()?;
                let delims = self.delimiters()?;
                ops.push(InspectOp::Converting { from, to, delims });
            } else {
                break;
            }
        }
        Ok(Stmt::Inspect { target, ops })
    }

    /// An operand inside INSPECT that is not the start of another phrase.
    fn is_inspect_operand(&self) -> bool {
        (self.is_operand_start()
            || self.is_any(&[
                "ZERO",
                "ZEROS",
                "ZEROES",
                "SPACE",
                "SPACES",
                "QUOTE",
                "QUOTES",
                "HIGH-VALUE",
                "HIGH-VALUES",
                "LOW-VALUE",
                "LOW-VALUES",
            ]))
            && !self.is_any(&[
                "ALL",
                "LEADING",
                "FIRST",
                "CHARACTERS",
                "REPLACING",
                "TALLYING",
            ])
    }

    /// Whether `name [qualifiers/subscripts] FOR` starts a tally clause.
    fn tally_counter_follows(&self) -> bool {
        let mut k = 1;
        let mut depth = 0;
        while let Some(t) = self.peek_at(k) {
            match &t.tok {
                Tok::LParen => depth += 1,
                Tok::RParen => depth -= 1,
                Tok::Word(w) if depth == 0 => {
                    if w == "FOR" {
                        return true;
                    }
                    if w != "OF" && w != "IN" {
                        // qualifier name follows OF/IN
                        let prev = self.peek_at(k - 1).and_then(|p| p.word());
                        if prev != Some("OF") && prev != Some("IN") {
                            return false;
                        }
                    }
                }
                _ if depth == 0 => return false,
                _ => {}
            }
            k += 1;
        }
        false
    }

    fn perform_kind(&mut self) -> Result<PerformKind> {
        let mut test_after = false;
        if self.accept("WITH") || self.is("TEST") {
            self.expect("TEST")?;
            test_after = if self.accept("AFTER") {
                true
            } else {
                self.expect("BEFORE")?;
                false
            };
        }
        if self.accept("UNTIL") {
            return Ok(PerformKind::Until {
                test_after,
                cond: self.cond()?,
            });
        }
        if self.accept("VARYING") {
            let mut loops = vec![self.varying_clause()?];
            while self.accept("AFTER") {
                loops.push(self.varying_clause()?);
            }
            return Ok(PerformKind::Varying { test_after, loops });
        }
        if self.operand_then_times() {
            let n = self.operand()?;
            self.expect("TIMES")?;
            return Ok(PerformKind::Times(n));
        }
        Ok(PerformKind::Once)
    }

    /// Whether an operand (possibly qualified and subscripted) followed by
    /// TIMES is at the cursor.
    fn operand_then_times(&self) -> bool {
        if !self.is_operand_start() {
            return false;
        }
        let mut k = 1;
        loop {
            match self.peek_at(k).map(|t| &t.tok) {
                Some(Tok::Word(w)) if w == "OF" || w == "IN" => k += 2,
                Some(Tok::LParen) => {
                    let mut depth = 0;
                    while let Some(t) = self.peek_at(k) {
                        match t.tok {
                            Tok::LParen => depth += 1,
                            Tok::RParen => {
                                depth -= 1;
                                if depth == 0 {
                                    break;
                                }
                            }
                            _ => {}
                        }
                        k += 1;
                    }
                    k += 1;
                }
                _ => break,
            }
        }
        self.is_at(k, "TIMES")
    }

    fn varying_clause(&mut self) -> Result<VaryingClause> {
        let var = self.data_ref()?;
        self.expect("FROM")?;
        let from = self.expr()?;
        self.expect("BY")?;
        let by = self.expr()?;
        self.expect("UNTIL")?;
        let until = self.cond()?;
        Ok(VaryingClause {
            var,
            from,
            by,
            until,
        })
    }

    fn perform(&mut self, pos: crate::front::source::Pos) -> Result<Stmt> {
        let inline = self.is_any(&["UNTIL", "VARYING", "WITH", "TEST", "END-PERFORM"])
            || self.is_verb()
            || self.operand_then_times();
        if inline {
            let kind = self.perform_kind()?;
            let body = self.statements()?;
            self.expect("END-PERFORM")?;
            return Ok(Stmt::Perform {
                body: PerformBody::Inline(body),
                kind,
                pos,
            });
        }
        let (a, b) = self.proc_range()?;
        let kind = self.perform_kind()?;
        Ok(Stmt::Perform {
            body: PerformBody::Procedures(a, b),
            kind,
            pos,
        })
    }

    fn set(&mut self) -> Result<Stmt> {
        // SET mnemonic... TO ON/OFF [mnemonic... TO ON/OFF ...]
        if let Some(Tok::Word(w)) = self.peek().map(|t| &t.tok)
            && self.mnemonics.contains(w)
        {
            let mut out = Vec::new();
            loop {
                let mut names = Vec::new();
                while let Some(Tok::Word(w)) = self.peek().map(|t| &t.tok) {
                    if !self.mnemonics.contains(w) {
                        break;
                    }
                    names.push(w.clone());
                    self.i += 1;
                }
                if names.is_empty() {
                    break;
                }
                self.expect("TO")?;
                let on = if self.accept("ON") {
                    true
                } else {
                    self.expect("OFF")?;
                    false
                };
                out.extend(names.into_iter().map(|n| (n, on)));
            }
            return Ok(Stmt::Set(SetStmt::Switch(out)));
        }
        if self.is("ADDRESS") && self.is_at(1, "OF") {
            let mut targets = Vec::new();
            while self.is("ADDRESS") && self.is_at(1, "OF") {
                self.i += 2;
                targets.push(self.data_ref()?);
            }
            self.expect("TO")?;
            let v = self.operand()?;
            return Ok(Stmt::Set(SetStmt::Address(targets, v)));
        }
        let mut targets = Vec::new();
        while self.is_operand_start() {
            targets.push(self.data_ref()?);
        }
        if self.accept("TO") {
            if self.accept("TRUE") {
                return Ok(Stmt::Set(SetStmt::CondTrue(targets)));
            }
            let v = self.operand()?;
            return Ok(Stmt::Set(SetStmt::To(targets, v)));
        }
        let up = if self.accept("UP") {
            true
        } else {
            self.expect("DOWN")?;
            false
        };
        self.expect("BY")?;
        let by = self.expr()?;
        Ok(Stmt::Set(SetStmt::UpDown(targets, up, by)))
    }

    fn sort_keys(&mut self) -> Result<Vec<(bool, Vec<DataRef>)>> {
        let mut keys = Vec::new();
        loop {
            self.accept("ON");
            let asc = if self.accept("ASCENDING") {
                true
            } else if self.accept("DESCENDING") {
                false
            } else {
                break;
            };
            self.accept("KEY");
            self.accept("IS");
            let mut names = Vec::new();
            while self.is_operand_start()
                && !self.is_any(&[
                    "ON",
                    "ASCENDING",
                    "DESCENDING",
                    "WITH",
                    "DUPLICATES",
                    "INPUT",
                    "OUTPUT",
                    "USING",
                    "GIVING",
                    "COLLATING",
                    "SEQUENCE",
                ])
            {
                names.push(self.data_ref()?);
            }
            keys.push((asc, names));
        }
        Ok(keys)
    }

    fn collating(&mut self) -> Result<Option<String>> {
        if self.accept("COLLATING") || self.is("SEQUENCE") {
            self.expect("SEQUENCE")?;
            self.accept("IS");
            return Ok(Some(self.name()?));
        }
        Ok(None)
    }

    fn sort_output(&mut self) -> Result<SortIo> {
        if self.accept("GIVING") {
            Ok(SortIo::Giving(self.file_names(&[])?))
        } else {
            self.expect("OUTPUT")?;
            self.expect("PROCEDURE")?;
            self.accept("IS");
            let (a, b) = self.proc_range()?;
            Ok(SortIo::Procedure(a, b))
        }
    }
}

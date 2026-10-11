//! Data division.

use super::Parser;
use crate::diag::Result;
use crate::front::ast::*;
use crate::front::lexer::{Tok, Token};

impl Parser {
    pub(super) fn data_division(&mut self) -> Result<DataDivision> {
        let mut dd = DataDivision::default();
        loop {
            if self.data_exec(&mut dd)? {
                continue;
            }
            if self.is("FILE") && self.is_at(1, "SECTION") {
                self.i += 2;
                self.expect_period()?;
                while self.is("FD") || self.is("SD") {
                    dd.files.push(self.file_description()?);
                }
            } else if self.is("WORKING-STORAGE") && self.is_at(1, "SECTION") {
                self.i += 2;
                self.expect_period()?;
                loop {
                    dd.working.extend(self.data_entries()?);
                    if !self.data_exec(&mut dd)? {
                        break;
                    }
                }
            } else if self.is("LOCAL-STORAGE") && self.is_at(1, "SECTION") {
                self.i += 2;
                self.expect_period()?;
                loop {
                    dd.local.extend(self.data_entries()?);
                    if !self.data_exec(&mut dd)? {
                        break;
                    }
                }
            } else if self.is("LINKAGE") && self.is_at(1, "SECTION") {
                self.i += 2;
                self.expect_period()?;
                loop {
                    dd.linkage.extend(self.data_entries()?);
                    if !self.data_exec(&mut dd)? {
                        break;
                    }
                }
            } else if self.is("COMMUNICATION") && self.is_at(1, "SECTION") {
                // Communication descriptions (module CM, not supported by
                // Enterprise COBOL): the CD area and its records are kept
                // as working-storage so that programs compile.
                self.i += 2;
                self.expect_period()?;
                while self.is("CD") {
                    let pos = self.pos();
                    self.i += 1;
                    let name = self.name()?;
                    let mut len = 87;
                    while !self.eof() && !self.is_period() {
                        if self.is("OUTPUT") {
                            len = 23;
                        } else if self.is("I-O") {
                            len = 33;
                        }
                        self.i += 1;
                    }
                    self.expect_period()?;
                    dd.working.push(DataEntry {
                        level: 1,
                        name: Some(name.clone()),
                        pos,
                        picture: Some(format!("X({len})")),
                        ..DataEntry::default()
                    });
                    for mut e in self.data_entries()? {
                        if e.level == 1 {
                            e.redefines = Some(name.clone());
                        }
                        dd.working.push(e);
                    }
                }
            } else if self.is("REPORT") && self.is_at(1, "SECTION") {
                self.i += 2;
                self.expect_period()?;
                while self.is("RD") {
                    dd.reports.push(self.report_description()?);
                }
            } else if self.is("SCREEN") && self.is_at(1, "SECTION") {
                // Not supported: skip to the next section or division.
                self.i += 2;
                while !self.eof() && !self.is_at(1, "SECTION") && !self.is_at(1, "DIVISION") {
                    self.i += 1;
                }
            } else {
                break;
            }
        }
        Ok(dd)
    }

    fn is_level(&self) -> bool {
        self.is_integer()
    }

    /// An EXEC block between data entries (EXEC SQL DECLARE, BEGIN /
    /// END DECLARE SECTION): kept in `execs`.
    fn data_exec(&mut self, dd: &mut DataDivision) -> Result<bool> {
        let Some(Token {
            tok: Tok::Exec(kind, text),
            pos,
        }) = self.peek().cloned()
        else {
            return Ok(false);
        };
        self.i += 1;
        if self.is_period() {
            self.i += 1;
        }
        dd.execs.push(self.exec_block(&kind, &text, pos)?);
        Ok(true)
    }

    pub(super) fn data_entries(&mut self) -> Result<Vec<DataEntry>> {
        let mut out = Vec::new();
        while self.is_level() {
            out.push(self.data_entry()?);
        }
        Ok(out)
    }

    fn file_description(&mut self) -> Result<FileDescription> {
        let pos = self.pos();
        let sort = self.is("SD");
        self.i += 1;
        let mut fd = FileDescription {
            name: self.name()?,
            pos,
            sort,
            ..FileDescription::default()
        };
        while !self.is_period() && !self.eof() {
            if self.accept("IS") {
                continue;
            } else if self.accept("EXTERNAL") {
                fd.external = true;
            } else if self.accept("GLOBAL") {
                fd.global = true;
            } else if self.accept("BLOCK") {
                self.accept("CONTAINS");
                let a = self.integer()?;
                let b = if self.accept("TO") {
                    self.integer()?
                } else {
                    a
                };
                let records = self.accept("RECORDS");
                self.accept("CHARACTERS");
                fd.block_contains = Some((a, b, records));
            } else if self.accept("RECORD") {
                if self.accept("IS") || self.is("VARYING") {
                    self.expect("VARYING")?;
                    self.accept("IN");
                    self.accept("SIZE");
                    if self.accept("FROM") || self.is_integer() {
                        fd.record_min = Some(self.integer()?);
                    }
                    if self.accept("TO") {
                        fd.record_max = Some(self.integer()?);
                    }
                    self.accept("CHARACTERS");
                    if self.accept("DEPENDING") {
                        self.accept("ON");
                        fd.record_depending = Some(self.data_ref()?);
                    }
                } else {
                    self.accept("CONTAINS");
                    let a = self.integer()?;
                    if self.accept("TO") {
                        fd.record_min = Some(a);
                        fd.record_max = Some(self.integer()?);
                    } else {
                        fd.record_min = Some(a);
                        fd.record_max = Some(a);
                    }
                    self.accept("CHARACTERS");
                }
            } else if self.accept("LABEL") {
                if !(self.accept("RECORDS") || self.accept("RECORD")) {
                    return self.err("RECORD expected");
                }
                self.accept("ARE");
                self.accept("IS");
                if self.accept("STANDARD") {
                    fd.label_standard = true;
                } else if !self.accept("OMITTED") {
                    // data-names
                    while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                        && !self.is_fd_clause()
                    {
                        self.i += 1;
                    }
                }
            } else if self.accept("VALUE") {
                self.expect("OF")?;
                while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                    && !self.is_fd_clause()
                {
                    let name = self.name()?;
                    self.accept("IS");
                    let value = self.operand()?;
                    fd.value_of.push((name, value));
                }
            } else if self.accept("DATA") {
                if !(self.accept("RECORDS") || self.accept("RECORD")) {
                    return self.err("RECORD expected");
                }
                self.accept("ARE");
                self.accept("IS");
                while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                    && !self.is_fd_clause()
                {
                    let n = self.name()?;
                    fd.data_records.push(n);
                }
            } else if self.accept("LINAGE") {
                self.accept("IS");
                let lines = self.operand()?;
                self.accept("LINES");
                let mut linage = Linage {
                    lines,
                    footing: None,
                    top: None,
                    bottom: None,
                };
                loop {
                    if self.accept("WITH") || self.is("FOOTING") {
                        self.expect("FOOTING")?;
                        self.accept("AT");
                        linage.footing = Some(self.operand()?);
                    } else if self.is("LINES")
                        || self.is("AT")
                        || self.is("TOP")
                        || self.is("BOTTOM")
                    {
                        self.accept("LINES");
                        self.accept("AT");
                        if self.accept("TOP") {
                            linage.top = Some(self.operand()?);
                        } else if self.accept("BOTTOM") {
                            linage.bottom = Some(self.operand()?);
                        } else {
                            return self.err("TOP or BOTTOM expected");
                        }
                    } else {
                        break;
                    }
                }
                fd.linage = Some(linage);
            } else if self.accept("CODE-SET") {
                self.accept("IS");
                fd.code_set = Some(self.name()?);
            } else if self.accept("REPORT") || self.accept("REPORTS") {
                self.accept("IS");
                self.accept("ARE");
                while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                    && !self.is_fd_clause()
                {
                    let n = self.name()?;
                    fd.reports.push(n);
                }
            } else if self.accept("RECORDING") {
                self.accept("MODE");
                self.accept("IS");
                self.name()?;
            } else {
                return self.err("FD clause expected");
            }
        }
        self.expect_period()?;
        fd.records = self.data_entries()?;
        Ok(fd)
    }

    fn is_fd_clause(&self) -> bool {
        self.is_any(&[
            "BLOCK",
            "RECORD",
            "LABEL",
            "VALUE",
            "DATA",
            "LINAGE",
            "CODE-SET",
            "RECORDING",
            "EXTERNAL",
            "GLOBAL",
            "REPORT",
            "REPORTS",
        ])
    }

    /// RD entry followed by its report group descriptions.
    fn report_description(&mut self) -> Result<ReportDescription> {
        let pos = self.pos();
        self.expect("RD")?;
        let mut rd = ReportDescription {
            name: self.name()?,
            pos,
            ..ReportDescription::default()
        };
        while !self.is_period() && !self.eof() {
            if self.accept("IS") {
            } else if self.accept("GLOBAL") {
                rd.global = true;
            } else if self.accept("CODE") {
                self.accept("IS");
                rd.code = Some(self.operand()?);
            } else if self.accept("CONTROL") || self.accept("CONTROLS") {
                self.accept("IS");
                self.accept("ARE");
                while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                    && !self.is_any(&["PAGE", "CODE", "GLOBAL", "IS"])
                {
                    if self.accept("FINAL") {
                        rd.controls.push(None);
                    } else {
                        rd.controls.push(Some(self.data_ref()?));
                    }
                }
            } else if self.accept("PAGE") {
                let page = rd.page.get_or_insert_with(PageClause::default);
                self.accept("LIMIT");
                self.accept("LIMITS");
                self.accept("IS");
                self.accept("ARE");
                if self.is_integer() {
                    page.limit = Some(self.integer()?);
                    self.accept("LINES");
                    self.accept("LINE");
                }
            } else if self.accept("HEADING") {
                self.accept("IS");
                let n = self.integer()?;
                rd.page.get_or_insert_with(PageClause::default).heading = Some(n);
            } else if self.accept("FIRST") {
                self.expect("DETAIL")?;
                self.accept("IS");
                let n = self.integer()?;
                rd.page.get_or_insert_with(PageClause::default).first_detail = Some(n);
            } else if self.accept("LAST") {
                self.expect("DETAIL")?;
                self.accept("IS");
                let n = self.integer()?;
                rd.page.get_or_insert_with(PageClause::default).last_detail = Some(n);
            } else if self.accept("FOOTING") {
                self.accept("IS");
                let n = self.integer()?;
                rd.page.get_or_insert_with(PageClause::default).footing = Some(n);
            } else {
                return self.err("RD clause expected");
            }
        }
        self.expect_period()?;
        rd.entries = self.data_entries()?;
        Ok(rd)
    }

    /// LINE / NEXT GROUP operand: integer, PLUS integer, NEXT PAGE,
    /// integer ON NEXT PAGE.
    fn line_clause(&mut self) -> Result<LineClause> {
        self.accept("NUMBER");
        self.accept("IS");
        if self.accept("PLUS") {
            return Ok(LineClause::Plus(self.integer()?));
        }
        if self.accept("NEXT") {
            self.expect("PAGE")?;
            return Ok(LineClause::NextPage);
        }
        let n = self.integer()?;
        if self.is("ON") && self.is_at(1, "NEXT") || self.is("NEXT") && self.is_at(1, "PAGE") {
            self.accept("ON");
            self.expect("NEXT")?;
            self.expect("PAGE")?;
            return Ok(LineClause::AbsoluteNextPage(n));
        }
        Ok(LineClause::Absolute(n))
    }

    /// Report group description clauses; returns false when the cursor is
    /// not at one.
    fn report_clause(&mut self, e: &mut DataEntry) -> Result<bool> {
        if self.accept("LINE") {
            let l = self.line_clause()?;
            rc(e).line = Some(l);
        } else if self.is("NEXT") && self.is_at(1, "GROUP") {
            self.i += 2;
            let l = self.line_clause()?;
            rc(e).next_group = Some(l);
        } else if self.accept("TYPE") {
            self.accept("IS");
            let control = |p: &mut Parser| -> Result<Option<String>> {
                if p.accept("FINAL") {
                    Ok(None)
                } else if matches!(p.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                    && !p.is_report_clause()
                {
                    Ok(Some(p.name()?))
                } else {
                    Ok(None)
                }
            };
            let ty = if self.accept("RH") {
                GroupType::ReportHeading
            } else if self.accept("PH") {
                GroupType::PageHeading
            } else if self.accept("CH") {
                GroupType::ControlHeading(control(self)?)
            } else if self.accept("DE") || self.accept("DETAIL") {
                GroupType::Detail
            } else if self.accept("CF") {
                GroupType::ControlFooting(control(self)?)
            } else if self.accept("PF") {
                GroupType::PageFooting
            } else if self.accept("RF") {
                GroupType::ReportFooting
            } else if self.accept("REPORT") {
                if self.accept("HEADING") {
                    GroupType::ReportHeading
                } else {
                    self.expect("FOOTING")?;
                    GroupType::ReportFooting
                }
            } else if self.accept("PAGE") {
                if self.accept("HEADING") {
                    GroupType::PageHeading
                } else {
                    self.expect("FOOTING")?;
                    GroupType::PageFooting
                }
            } else if self.accept("CONTROL") {
                if self.accept("HEADING") {
                    GroupType::ControlHeading(control(self)?)
                } else {
                    self.expect("FOOTING")?;
                    GroupType::ControlFooting(control(self)?)
                }
            } else {
                return self.err("report group type expected");
            };
            rc(e).ty = Some(ty);
        } else if self.accept("COLUMN") {
            self.accept("NUMBER");
            self.accept("IS");
            let n = self.integer()?;
            rc(e).column = Some(n);
        } else if self.accept("SOURCE") {
            self.accept("IS");
            let s = self.operand()?;
            rc(e).source = Some(s);
        } else if self.accept("SUM") {
            let mut items = Vec::new();
            while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                && !self.is_any(&["UPON", "RESET"])
                && !self.is_report_clause()
                && !self.is_entry_clause()
            {
                items.push(self.data_ref()?);
            }
            let mut upon = Vec::new();
            if self.accept("UPON") {
                while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                    && !self.is_any(&["RESET"])
                    && !self.is_report_clause()
                    && !self.is_entry_clause()
                {
                    upon.push(self.name()?);
                }
            }
            let reset = if self.accept("RESET") {
                self.accept("ON");
                Some(if self.accept("FINAL") {
                    None
                } else {
                    Some(self.name()?)
                })
            } else {
                None
            };
            rc(e).sum.push(SumClause { items, upon, reset });
        } else if self.is("GROUP") && self.is_at(1, "INDICATE") {
            self.i += 2;
            rc(e).group_indicate = true;
        } else {
            return Ok(false);
        }
        Ok(true)
    }
}

fn rc(e: &mut DataEntry) -> &mut ReportClauses {
    e.report.get_or_insert_with(Box::default)
}

impl Parser {
    fn is_report_clause(&self) -> bool {
        self.is_any(&["LINE", "TYPE", "COLUMN", "SOURCE", "SUM"])
            || (self.is("NEXT") && self.is_at(1, "GROUP"))
            || (self.is("GROUP") && self.is_at(1, "INDICATE"))
    }

    fn usage_word(&mut self) -> Option<Usage> {
        let u = match self.peek()?.word()? {
            // DISPLAY-1 (DBCS) items are DISPLAY items of PICTURE G / N
            "DISPLAY" | "DISPLAY-1" => Usage::Display,
            "COMP" | "COMPUTATIONAL" | "COMP-4" | "COMPUTATIONAL-4" | "BINARY" => Usage::Binary,
            "COMP-3" | "COMPUTATIONAL-3" | "PACKED-DECIMAL" => Usage::Packed,
            "COMP-5" | "COMPUTATIONAL-5" => Usage::NativeBinary,
            "COMP-1" | "COMPUTATIONAL-1" => Usage::Float,
            "COMP-2" | "COMPUTATIONAL-2" => Usage::Double,
            "INDEX" => Usage::Index,
            "POINTER" => Usage::Pointer,
            _ => return None,
        };
        self.i += 1;
        Some(u)
    }

    fn data_entry(&mut self) -> Result<DataEntry> {
        let pos = self.pos();
        let level = self.integer()? as u8;
        let mut e = DataEntry {
            level,
            pos,
            ..DataEntry::default()
        };
        if self.accept("FILLER") {
        } else if let Some(Tok::Word(w)) = self.peek().map(|t| &t.tok)
            && !self.is_entry_clause()
            && !self.is_report_clause()
        {
            e.name = Some(w.clone());
            self.i += 1;
        }
        while !self.is_period() && !self.eof() {
            if self.report_clause(&mut e)? {
            } else if self.accept("REDEFINES") {
                e.redefines = Some(self.name()?);
            } else if self.accept("IS") {
                // IS EXTERNAL / IS GLOBAL
            } else if self.accept("EXTERNAL") {
                e.external = true;
            } else if self.accept("GLOBAL") {
                e.global = true;
            } else if self.accept("PIC") || self.accept("PICTURE") {
                self.accept("IS");
                match self.next().map(|t| t.tok) {
                    Some(Tok::Pic(p)) => e.picture = Some(p),
                    _ => return self.err("picture string expected"),
                }
            } else if self.accept("USAGE") {
                self.accept("IS");
                e.usage = Some(match self.usage_word() {
                    Some(u) => u,
                    None => return self.err("usage expected"),
                });
            } else if let Some(u) = self.usage_word() {
                e.usage = Some(u);
            } else if self.accept("SIGN") {
                self.accept("IS");
                let leading = if self.accept("LEADING") {
                    true
                } else {
                    self.expect("TRAILING")?;
                    false
                };
                let separate = self.accept("SEPARATE");
                self.accept("CHARACTER");
                e.sign = Some((leading, separate));
            } else if self.is("LEADING") || self.is("TRAILING") {
                let leading = self.accept("LEADING");
                if !leading {
                    self.i += 1;
                }
                let separate = self.accept("SEPARATE");
                self.accept("CHARACTER");
                e.sign = Some((leading, separate));
            } else if self.accept("OCCURS") {
                e.occurs = Some(self.occurs()?);
            } else if self.accept("SYNC") || self.accept("SYNCHRONIZED") {
                self.accept("LEFT");
                self.accept("RIGHT");
                e.sync = true;
            } else if self.accept("JUST") || self.accept("JUSTIFIED") {
                self.accept("RIGHT");
                e.justified = true;
            } else if self.accept("BLANK") {
                self.accept("WHEN");
                if !(self.accept("ZERO") || self.accept("ZEROS") || self.accept("ZEROES")) {
                    return self.err("ZERO expected");
                }
                e.blank_when_zero = true;
            } else if self.accept("VALUE") || self.accept("VALUES") {
                self.accept("IS");
                self.accept("ARE");
                if level == 88 {
                    let mut vals = Vec::new();
                    while !self.is_period() && !self.eof() {
                        let from = self.operand()?;
                        let thru = if self.accept("THRU") || self.accept("THROUGH") {
                            Some(self.operand()?)
                        } else {
                            None
                        };
                        vals.push((from, thru));
                    }
                    e.value = Some(ValueClause::Condition(vals));
                } else {
                    e.value = Some(ValueClause::Literal(self.operand()?));
                }
            } else if self.accept("RENAMES") {
                let a = self.data_ref()?;
                let b = if self.accept("THRU") || self.accept("THROUGH") {
                    Some(self.data_ref()?)
                } else {
                    None
                };
                e.renames = Some((a, b));
            } else {
                return self.err("data description clause expected");
            }
        }
        self.expect_period()?;
        Ok(e)
    }

    fn is_entry_clause(&self) -> bool {
        self.is_any(&[
            "REDEFINES",
            "IS",
            "EXTERNAL",
            "GLOBAL",
            "PIC",
            "PICTURE",
            "USAGE",
            "SIGN",
            "OCCURS",
            "SYNC",
            "SYNCHRONIZED",
            "JUST",
            "JUSTIFIED",
            "BLANK",
            "VALUE",
            "VALUES",
            "RENAMES",
            "COMP",
            "COMPUTATIONAL",
            "COMP-3",
            "COMPUTATIONAL-3",
            "COMP-4",
            "COMPUTATIONAL-4",
            "COMP-5",
            "COMPUTATIONAL-5",
            "BINARY",
            "PACKED-DECIMAL",
            "DISPLAY",
            "INDEX",
            "POINTER",
            "LEADING",
            "TRAILING",
        ])
    }

    fn occurs(&mut self) -> Result<Occurs> {
        let a = self.integer()?;
        let (min, max) = if self.accept("TO") {
            (a, self.integer()?)
        } else {
            (a, a)
        };
        self.accept("TIMES");
        let mut occ = Occurs {
            min,
            max,
            depending: None,
            keys: Vec::new(),
            indexed_by: Vec::new(),
        };
        loop {
            if self.accept("DEPENDING") {
                self.accept("ON");
                occ.depending = Some(self.data_ref()?);
            } else if self.is("ASCENDING") || self.is("DESCENDING") {
                let asc = self.accept("ASCENDING");
                if !asc {
                    self.i += 1;
                }
                self.accept("KEY");
                self.accept("IS");
                let mut keys = Vec::new();
                while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                    && !self.is_any(&["ASCENDING", "DESCENDING", "INDEXED", "DEPENDING"])
                    && !self.is_entry_clause()
                {
                    keys.push(self.data_ref()?);
                }
                occ.keys.push((asc, keys));
            } else if self.accept("INDEXED") {
                self.accept("BY");
                while matches!(self.peek().map(|t| &t.tok), Some(Tok::Word(_)))
                    && !self.is_entry_clause()
                    && !self.is_any(&["ASCENDING", "DESCENDING", "DEPENDING"])
                {
                    let n = self.name()?;
                    occ.indexed_by.push(n);
                }
            } else {
                break;
            }
        }
        Ok(occ)
    }
}

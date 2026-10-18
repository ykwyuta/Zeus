//! Embedded EXEC CICS / EXEC SQL / EXEC DLI blocks.
//!
//! CICS and DL/I commands are a command word followed by options, each
//! `NAME` or `NAME(argument)`; arguments are parsed as COBOL expressions
//! (data references, literals, LENGTH OF / ADDRESS OF, arithmetic).
//! SQL statements keep their text, with host variables (`:name`,
//! `:group.name`, `:name:indicator`) taken out as data references.

use super::Parser;
use crate::diag::{Diag, Result};
use crate::front::ast::*;
use crate::front::lexer::{self, Tok, Token};
use crate::front::source::Pos;

impl Parser {
    pub(super) fn exec_block(
        &mut self,
        kind: &str,
        text: &[(char, Pos)],
        pos: Pos,
    ) -> Result<ExecBlock> {
        let kind = match kind {
            "CICS" => ExecKind::Cics,
            "SQL" => ExecKind::Sql,
            "SQLIMS" => ExecKind::SqlIms,
            _ => ExecKind::Dli,
        };
        if matches!(kind, ExecKind::Sql | ExecKind::SqlIms) {
            let sql = sql_text(text, pos)?;
            let command = sql
                .text
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .to_ascii_uppercase();
            if kind == ExecKind::SqlIms
                && !matches!(
                    command.as_str(),
                    "PREPARE"
                        | "DECLARE"
                        | "OPEN"
                        | "FETCH"
                        | "CLOSE"
                        | "EXECUTE"
                        | "DESCRIBE"
                        | "WHENEVER"
                        | "BEGIN"
                        | "END"
                        | "INCLUDE"
                )
            {
                return Err(Diag::error(
                    pos,
                    "SQLIMS supports dynamic SQL; use PREPARE/EXECUTE or OPEN/FETCH, and DL/I system services for commit/rollback",
                ));
            }
            return Ok(ExecBlock {
                kind,
                pos,
                command,
                options: Vec::new(),
                sql: Some(sql),
            });
        }
        let toks = crate::front::cics::translate(lexer::lex(text, false)?)?;
        let mut j = 0;
        let word = |j: usize| toks.get(j).and_then(|t| t.word().map(str::to_string));
        let command = word(0).ok_or_else(|| Diag::error(pos, "EXEC command expected"))?;
        j += 1;
        let mut options = Vec::new();
        while j < toks.len() {
            let opos = toks[j].pos;
            let name = word(j).ok_or_else(|| {
                Diag::error(opos, format!("option expected, found {}", toks[j].text()))
            })?;
            j += 1;
            let mut arg = None;
            if toks.get(j).is_some_and(|t| t.tok == Tok::LParen) {
                let start = j + 1;
                let mut depth = 0;
                let mut end = None;
                for (k, t) in toks.iter().enumerate().skip(j) {
                    match t.tok {
                        Tok::LParen => depth += 1,
                        Tok::RParen => {
                            depth -= 1;
                            if depth == 0 {
                                end = Some(k);
                                break;
                            }
                        }
                        _ => {}
                    }
                }
                let end = end.ok_or_else(|| Diag::error(opos, "unbalanced parentheses"))?;
                arg = Some(if kind == ExecKind::Dli && name == "WHERE" {
                    // DL/I qualification: field, operator and operand
                    ExecArg::Raw(
                        toks[start..end]
                            .iter()
                            .map(Token::text)
                            .collect::<Vec<_>>()
                            .join(" "),
                    )
                } else {
                    self.exec_arg(&toks[start..end])
                });
                j = end + 1;
            }
            options.push(ExecOption {
                name,
                arg,
                pos: opos,
            });
        }
        Ok(ExecBlock {
            kind,
            pos,
            command,
            options,
            sql: None,
        })
    }

    /// An option argument: a COBOL expression when it parses as one.
    fn exec_arg(&mut self, toks: &[Token]) -> ExecArg {
        let saved = std::mem::replace(&mut self.toks, toks.to_vec());
        let saved_i = std::mem::replace(&mut self.i, 0);
        let r = self.expr();
        let ok = self.eof();
        self.toks = saved;
        self.i = saved_i;
        match r {
            Ok(e) if ok => ExecArg::Expr(e),
            _ => ExecArg::Raw(toks.iter().map(Token::text).collect::<Vec<_>>().join(" ")),
        }
    }
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// SQL text with host variables replaced by `?`.
fn sql_text(text: &[(char, Pos)], pos: Pos) -> Result<SqlText> {
    let chars: Vec<char> = text.iter().map(|(c, _)| *c).collect();
    let mut out = String::new();
    let mut host = Vec::new();
    let mut quote: Option<char> = None;
    let mut i = 0;
    // a host variable reference: name or qualifier.name
    let name_at = |i: &mut usize| -> Option<DataRef> {
        let start = *i;
        while *i < chars.len() && is_name_char(chars[*i]) {
            *i += 1;
        }
        if *i == start {
            return None;
        }
        let mut name: String = chars[start..*i]
            .iter()
            .collect::<String>()
            .to_ascii_uppercase();
        let mut qualifiers = Vec::new();
        if *i + 1 < chars.len() && chars[*i] == '.' && is_name_char(chars[*i + 1]) {
            *i += 1;
            let s = *i;
            while *i < chars.len() && is_name_char(chars[*i]) {
                *i += 1;
            }
            qualifiers.push(name);
            name = chars[s..*i].iter().collect::<String>().to_ascii_uppercase();
        }
        Some(DataRef {
            name,
            qualifiers,
            subscripts: Vec::new(),
            refmod: None,
            pos,
        })
    };
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
                out.push(c);
                i += 1;
            }
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                out.push(c);
                i += 1;
            }
            None if c == ':' && chars.get(i + 1).is_some_and(|c| c.is_ascii_alphabetic()) => {
                i += 1;
                let var =
                    name_at(&mut i).ok_or_else(|| Diag::error(pos, "host variable expected"))?;
                let mut indicator = None;
                let mut k = i;
                while k < chars.len() && chars[k] == ' ' {
                    k += 1;
                }
                let rest: String = chars[k..chars.len().min(k + 9)].iter().collect();
                if chars.get(i) == Some(&':') {
                    i += 1;
                    indicator = name_at(&mut i);
                } else if rest.eq_ignore_ascii_case("INDICATOR")
                    && chars.get(k + 9).is_some_and(|c| *c == ' ')
                {
                    let mut m = k + 9;
                    while m < chars.len() && chars[m] == ' ' {
                        m += 1;
                    }
                    if chars.get(m) == Some(&':') {
                        m += 1;
                        indicator = name_at(&mut m);
                        i = m;
                    }
                }
                host.push(SqlHostVar { var, indicator });
                out.push('?');
            }
            None if c == '\n' || c == '\t' => {
                out.push(' ');
                i += 1;
            }
            None => {
                out.push(c);
                i += 1;
            }
        }
    }
    let text = out.split_whitespace().collect::<Vec<_>>().join(" ");
    Ok(SqlText { text, host })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<(char, Pos)> {
        s.chars().map(|c| (c, Pos::default())).collect()
    }

    #[test]
    fn sql_host_variables() {
        let s = sql_text(
            &chars(" SELECT A, B INTO :WS-A, :GRP.B:IND-B\n FROM T WHERE K = :K AND C = 'X:Y' "),
            Pos::default(),
        )
        .unwrap();
        assert_eq!(
            s.text,
            "SELECT A, B INTO ?, ? FROM T WHERE K = ? AND C = 'X:Y'"
        );
        assert_eq!(s.host.len(), 3);
        assert_eq!(s.host[1].var.name, "B");
        assert_eq!(s.host[1].var.qualifiers, vec!["GRP".to_string()]);
        assert_eq!(s.host[1].indicator.as_ref().unwrap().name, "IND-B");
    }
}

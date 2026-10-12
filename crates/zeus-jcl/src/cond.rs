//! Relational expressions of the IF/THEN/ELSE/ENDIF statement construct.
//!
//! ```text
//! expr     := term { (OR | '|') term }
//! term     := factor { (AND | '&') factor }
//! factor   := (NOT | '¬') factor | '(' expr ')' | relation
//! relation := keyword [ op value ]
//! keyword  := [stepname[.procstepname].] (RC | ABEND | ABENDCC | RUN)
//!           | ¬ABEND | ¬RUN
//! op       := GT LT NG NL EQ NE GE LE  >  <  ¬>  ¬<  =  ¬=  >=  <=
//! ```
//!
//! `RC` is the highest return code of the steps that ran; `ABEND` is
//! true when a step abended; `ABENDCC=Sxxx` / `Uxxxx` tests the abend
//! code; `step.RUN` whether the step ran. A test of a step that did not
//! run is false.

use crate::job::{Outcome, StepResult};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    Word(String),
    Op(&'static str),
    Not,
    And,
    Or,
    Open,
    Close,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    /// keyword with its step qualifier, operator and value
    Rel {
        step: Option<String>,
        keyword: String,
        op: &'static str,
        value: String,
    },
}

fn tokens(s: &str) -> Result<Vec<Tok>, String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        let next = c.get(i + 1).copied();
        match ch {
            ' ' => i += 1,
            '(' => {
                out.push(Tok::Open);
                i += 1;
            }
            ')' => {
                out.push(Tok::Close);
                i += 1;
            }
            '&' => {
                out.push(Tok::And);
                i += 1;
            }
            '|' => {
                out.push(Tok::Or);
                i += 1;
            }
            '¬' | '^' => match next {
                Some('=') => {
                    out.push(Tok::Op("NE"));
                    i += 2;
                }
                Some('>') => {
                    out.push(Tok::Op("LE"));
                    i += 2;
                }
                Some('<') => {
                    out.push(Tok::Op("GE"));
                    i += 2;
                }
                _ => {
                    out.push(Tok::Not);
                    i += 1;
                }
            },
            '>' | '<' | '=' => {
                let op = match (ch, next) {
                    ('>', Some('=')) => "GE",
                    ('<', Some('=')) => "LE",
                    ('>', _) => "GT",
                    ('<', _) => "LT",
                    _ => "EQ",
                };
                out.push(Tok::Op(op));
                i += if matches!((ch, next), ('>' | '<', Some('='))) {
                    2
                } else {
                    1
                };
            }
            _ => {
                let start = i;
                while i < c.len() && (c[i].is_alphanumeric() || "@#$.".contains(c[i])) {
                    i += 1;
                }
                if i == start {
                    return Err(format!("invalid character '{ch}' in IF expression"));
                }
                let w: String = c[start..i].iter().collect::<String>().to_ascii_uppercase();
                out.push(match w.as_str() {
                    "AND" => Tok::And,
                    "OR" => Tok::Or,
                    "NOT" => Tok::Not,
                    "GT" => Tok::Op("GT"),
                    "LT" => Tok::Op("LT"),
                    "GE" | "NL" => Tok::Op("GE"),
                    "LE" | "NG" => Tok::Op("LE"),
                    "EQ" => Tok::Op("EQ"),
                    "NE" => Tok::Op("NE"),
                    _ => Tok::Word(w),
                });
            }
        }
    }
    Ok(out)
}

struct Parser {
    t: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    fn expr(&mut self) -> Result<Expr, String> {
        let mut e = self.term()?;
        while self.peek() == Some(&Tok::Or) {
            self.i += 1;
            e = Expr::Or(Box::new(e), Box::new(self.term()?));
        }
        Ok(e)
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut e = self.factor()?;
        while self.peek() == Some(&Tok::And) {
            self.i += 1;
            e = Expr::And(Box::new(e), Box::new(self.factor()?));
        }
        Ok(e)
    }

    fn factor(&mut self) -> Result<Expr, String> {
        match self.t.get(self.i).cloned() {
            Some(Tok::Not) => {
                self.i += 1;
                Ok(Expr::Not(Box::new(self.factor()?)))
            }
            Some(Tok::Open) => {
                self.i += 1;
                let e = self.expr()?;
                if self.peek() != Some(&Tok::Close) {
                    return Err("')' expected in IF expression".into());
                }
                self.i += 1;
                Ok(e)
            }
            Some(Tok::Word(w)) => {
                self.i += 1;
                let mut parts: Vec<&str> = w.split('.').collect();
                let keyword = parts.pop().unwrap_or("").to_string();
                if !matches!(keyword.as_str(), "RC" | "ABEND" | "ABENDCC" | "RUN") {
                    return Err(format!("invalid keyword {keyword} in IF expression"));
                }
                let step = (!parts.is_empty()).then(|| parts.join("."));
                let (op, value) = match self.t.get(self.i).cloned() {
                    Some(Tok::Op(op)) => {
                        self.i += 1;
                        match self.t.get(self.i).cloned() {
                            Some(Tok::Word(v)) => {
                                self.i += 1;
                                (op, v)
                            }
                            _ => return Err(format!("value expected after {keyword}")),
                        }
                    }
                    // ABEND / RUN alone: = TRUE
                    _ if keyword == "ABEND" || keyword == "RUN" => ("EQ", "TRUE".to_string()),
                    _ => return Err(format!("operator expected after {keyword}")),
                };
                Ok(Expr::Rel {
                    step,
                    keyword,
                    op,
                    value,
                })
            }
            _ => Err("relational expression expected".into()),
        }
    }
}

/// Parses the relational expression of an IF statement.
pub fn parse(s: &str) -> Result<Expr, String> {
    let mut p = Parser {
        t: tokens(s)?,
        i: 0,
    };
    let e = p.expr()?;
    if p.i != p.t.len() {
        return Err("unexpected text in IF expression".into());
    }
    Ok(e)
}

impl Expr {
    /// Whether the expression tests for an abend (such an IF lets its
    /// steps run after an abend).
    pub fn tests_abend(&self) -> bool {
        match self {
            Expr::Not(e) => e.tests_abend(),
            Expr::And(a, b) | Expr::Or(a, b) => a.tests_abend() || b.tests_abend(),
            Expr::Rel { keyword, .. } => keyword == "ABEND" || keyword == "ABENDCC",
        }
    }

    pub fn eval(&self, results: &[StepResult]) -> bool {
        match self {
            Expr::Not(e) => !e.eval(results),
            Expr::And(a, b) => a.eval(results) && b.eval(results),
            Expr::Or(a, b) => a.eval(results) || b.eval(results),
            Expr::Rel {
                step,
                keyword,
                op,
                value,
            } => {
                let of_step = |r: &&StepResult| match step {
                    None => true,
                    Some(s) => match s.split_once('.') {
                        Some((js, ps)) => r.step == js && r.proc_step.as_deref() == Some(ps),
                        None => r.step == *s || r.proc_step.as_deref() == Some(s.as_str()),
                    },
                };
                let selected: Vec<&StepResult> = results.iter().filter(of_step).collect();
                let ran: Vec<&&StepResult> = selected
                    .iter()
                    .filter(|r| matches!(r.outcome, Outcome::Rc(_) | Outcome::Abend(_)))
                    .collect();
                if step.is_some() && ran.is_empty() && keyword != "RUN" {
                    return false;
                }
                let truth = |b: bool| {
                    let v = value == "TRUE";
                    match *op {
                        "EQ" => b == v,
                        "NE" => b != v,
                        _ => false,
                    }
                };
                match keyword.as_str() {
                    "RC" => {
                        let Ok(n) = value.parse::<i32>() else {
                            return false;
                        };
                        let rcs: Vec<i32> = ran
                            .iter()
                            .filter_map(|r| match r.outcome {
                                Outcome::Rc(rc) => Some(rc),
                                _ => None,
                            })
                            .collect();
                        let Some(rc) = rcs.into_iter().max().or(step.is_none().then_some(0)) else {
                            return false;
                        };
                        match *op {
                            "GT" => rc > n,
                            "LT" => rc < n,
                            "GE" => rc >= n,
                            "LE" => rc <= n,
                            "EQ" => rc == n,
                            "NE" => rc != n,
                            _ => false,
                        }
                    }
                    "ABEND" => truth(ran.iter().any(|r| matches!(r.outcome, Outcome::Abend(_)))),
                    "RUN" => truth(!ran.is_empty()),
                    _ => {
                        // ABENDCC=Sxxx / Uxxxx (user codes compared as numbers)
                        let norm = |c: &str| match c.strip_prefix('U') {
                            Some(n) => format!("U{}", n.parse::<u32>().unwrap_or(u32::MAX)),
                            None => c.to_string(),
                        };
                        let hit = ran.iter().any(
                            |r| matches!(&r.outcome, Outcome::Abend(c) if norm(c) == norm(value)),
                        );
                        match *op {
                            "EQ" => hit,
                            "NE" => !hit,
                            _ => false,
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn res(step: &str, outcome: Outcome) -> StepResult {
        StepResult {
            step: step.into(),
            proc_step: None,
            program: "X".into(),
            outcome,
        }
    }

    #[test]
    fn expressions() {
        let r = vec![
            res("S1", Outcome::Rc(4)),
            res("S2", Outcome::Bypassed),
            res("S3", Outcome::Abend("U0100".into())),
        ];
        let t = |s: &str| parse(s).unwrap().eval(&r);
        assert!(t("RC = 4"));
        assert!(t("(RC GE 4 & S1.RC < 8)"));
        assert!(!t("RC > 4"));
        assert!(!t("S2.RC = 0"));
        assert!(t("¬S2.RUN"));
        assert!(t("S2.RUN = FALSE"));
        assert!(t("ABEND"));
        assert!(t("S3.ABENDCC=U100"));
        assert!(t("NOT S1.ABEND OR RC=99"));
        assert!(parse("ABEND").unwrap().tests_abend());
        assert!(!parse("RC=0").unwrap().tests_abend());
        assert!(parse("RC = ").is_err());
    }
}

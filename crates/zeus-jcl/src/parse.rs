//! JCL statements.
//!
//! A statement starts with `//` in columns 1-2: an optional name from
//! column 3, the operation, the operand field and comments, each separated
//! by blanks; columns 73-80 are not part of the statement. An operand
//! field ending with a comma continues on the next `//` card. `//*` is a
//! comment, `/*` ends in-stream data, `//` alone ends the job. Cards that
//! follow a `DD *` or `DD DATA` statement are its in-stream data; data
//! cards appearing anywhere else belong to an implicit `//SYSIN DD *`.

use std::fmt;

/// Operand value: a string (quotes removed) or a parenthesised list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    List(Vec<Param>),
}

/// Positional (no key) or keyword operand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub key: Option<String>,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stmt {
    pub name: Option<String>,
    pub op: String,
    pub params: Vec<Param>,
    /// The operand field as written (continuations joined).
    pub operands: String,
    /// In-stream data cards (DD * / DD DATA).
    pub data: Option<Vec<String>>,
    /// Line number of the first card (1-based).
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JclError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for JclError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for JclError {}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            Value::List(_) => None,
        }
    }

    /// Positional elements of a list (a plain string is a one-element
    /// list).
    pub fn items(&self) -> Vec<&Value> {
        match self {
            Value::Str(_) => vec![self],
            Value::List(l) => l.iter().map(|p| &p.value).collect(),
        }
    }
}

impl Stmt {
    /// Value of keyword operand `key`.
    pub fn get(&self, key: &str) -> Option<&Value> {
        get(&self.params, key)
    }

    pub fn get_str(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Value::as_str)
    }

    /// The n-th positional operand.
    pub fn positional(&self, n: usize) -> Option<&Value> {
        self.params
            .iter()
            .filter(|p| p.key.is_none())
            .nth(n)
            .map(|p| &p.value)
    }
}

/// Keyword `key` in a parameter list.
pub fn get<'a>(params: &'a [Param], key: &str) -> Option<&'a Value> {
    params
        .iter()
        .find(|p| {
            p.key
                .as_deref()
                .is_some_and(|k| k.eq_ignore_ascii_case(key))
        })
        .map(|p| &p.value)
}

fn err<T>(line: usize, message: impl Into<String>) -> Result<T, JclError> {
    Err(JclError {
        line,
        message: message.into(),
    })
}

/// Splits the operand field off a statement's text: up to the first blank
/// outside quotes.
fn operand_field(s: &str) -> &str {
    let mut quoted = false;
    for (i, c) in s.char_indices() {
        match c {
            '\'' => quoted = !quoted,
            ' ' if !quoted => return &s[..i],
            _ => {}
        }
    }
    s
}

/// Position of the word THEN in the text of an IF statement.
fn then_at(s: &str) -> Option<usize> {
    let u = s.to_ascii_uppercase();
    let b = u.as_bytes();
    let word = |c: Option<&u8>| c.is_some_and(|c| c.is_ascii_alphanumeric() || b"@#$.".contains(c));
    u.match_indices("THEN")
        .map(|(p, _)| p)
        .find(|&p| !word(p.checked_sub(1).and_then(|q| b.get(q))) && !word(b.get(p + 4)))
}

struct OperandParser<'a> {
    s: &'a [u8],
    i: usize,
    line: usize,
}

impl OperandParser<'_> {
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn list(&mut self, close: Option<u8>) -> Result<Vec<Param>, JclError> {
        let mut out = Vec::new();
        loop {
            if self.peek() == close {
                break;
            }
            out.push(self.param()?);
            match self.peek() {
                Some(b',') => self.i += 1,
                c if c == close => break,
                Some(c) => {
                    return err(self.line, format!("unexpected '{}' in operands", c as char));
                }
                None => return err(self.line, "unbalanced parentheses"),
            }
        }
        if close.is_some() {
            self.i += 1;
        }
        Ok(out)
    }

    fn param(&mut self) -> Result<Param, JclError> {
        // keyword: a name (letters, digits, national characters, dots)
        // followed by '='
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || b"@#$.&_-".contains(&c) {
                self.i += 1;
            } else {
                break;
            }
        }
        if self.peek() == Some(b'=') && self.i > start {
            let key = String::from_utf8_lossy(&self.s[start..self.i]).to_ascii_uppercase();
            self.i += 1;
            let value = self.value()?;
            return Ok(Param {
                key: Some(key),
                value,
            });
        }
        self.i = start;
        Ok(Param {
            key: None,
            value: self.value()?,
        })
    }

    fn value(&mut self) -> Result<Value, JclError> {
        match self.peek() {
            Some(b'(') => {
                self.i += 1;
                Ok(Value::List(self.list(Some(b')'))?))
            }
            Some(b'\'') => {
                self.i += 1;
                let mut v = Vec::new();
                loop {
                    match self.peek() {
                        Some(b'\'') if self.s.get(self.i + 1) == Some(&b'\'') => {
                            v.push(b'\'');
                            self.i += 2;
                        }
                        Some(b'\'') => {
                            self.i += 1;
                            break;
                        }
                        Some(c) => {
                            v.push(c);
                            self.i += 1;
                        }
                        None => return err(self.line, "unterminated quoted string"),
                    }
                }
                // a quoted value may continue unquoted (DSN='A'.B is rare)
                Ok(Value::Str(String::from_utf8_lossy(&v).into_owned()))
            }
            _ => {
                let start = self.i;
                let mut depth = 0;
                while let Some(c) = self.peek() {
                    match c {
                        b',' if depth == 0 => break,
                        b')' if depth == 0 => break,
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        _ => {}
                    }
                    self.i += 1;
                }
                Ok(Value::Str(
                    String::from_utf8_lossy(&self.s[start..self.i]).into_owned(),
                ))
            }
        }
    }
}

/// Parses an operand field.
pub fn parse_operands(s: &str, line: usize) -> Result<Vec<Param>, JclError> {
    if s.is_empty() {
        return Ok(Vec::new());
    }
    let mut p = OperandParser {
        s: s.as_bytes(),
        i: 0,
        line,
    };
    p.list(None)
}

fn is_data_dd(params: &[Param]) -> Option<Option<String>> {
    let star = params
        .iter()
        .any(|p| p.key.is_none() && matches!(&p.value, Value::Str(s) if s == "*" || s == "DATA"));
    star.then(|| {
        get(params, "DLM")
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

/// Parses JCL text into statements.
pub fn parse(text: &str) -> Result<Vec<Stmt>, JclError> {
    let lines: Vec<&str> = text.lines().map(|l| l.trim_end_matches('\r')).collect();
    let mut out: Vec<Stmt> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let raw = lines[i];
        let lineno = i + 1;
        let card: String = raw.chars().take(72).collect();
        if card.starts_with("//*") {
            i += 1;
            continue;
        }
        if card.trim() == "//" {
            break;
        }
        if !card.starts_with("//") {
            if card.starts_with("/*") || card.trim().is_empty() {
                i += 1;
                continue;
            }
            // data cards outside DD *: implicit SYSIN DD *
            let mut data = Vec::new();
            while i < lines.len() && !lines[i].starts_with("//") && !lines[i].starts_with("/*") {
                data.push(lines[i].chars().take(80).collect());
                i += 1;
            }
            out.push(Stmt {
                name: Some("SYSIN".into()),
                op: "DD".into(),
                params: vec![Param {
                    key: None,
                    value: Value::Str("*".into()),
                }],
                operands: "*".into(),
                data: Some(data),
                line: lineno,
            });
            continue;
        }
        let body = &card[2..];
        let (name, rest) = if body.starts_with(' ') {
            (None, body)
        } else {
            let n = body.find(' ').unwrap_or(body.len());
            (Some(body[..n].to_ascii_uppercase()), &body[n..])
        };
        let rest = rest.trim_start();
        let n = rest.find(' ').unwrap_or(rest.len());
        let op = rest[..n].to_ascii_uppercase();
        i += 1;
        // IF: the relational expression (blanks allowed) up to THEN,
        // possibly over several cards; ELSE and ENDIF have no operands
        if op == "IF" || op == "ELSE" || op == "ENDIF" {
            let mut text = rest[n..].to_string();
            if op == "IF" {
                while then_at(&text).is_none() && i < lines.len() && lines[i].starts_with("// ") {
                    text.push(' ');
                    text.push_str(&lines[i].chars().take(72).collect::<String>()[2..]);
                    i += 1;
                }
                match then_at(&text) {
                    Some(p) => text.truncate(p),
                    None => return err(lineno, "THEN expected in IF statement"),
                }
            } else {
                text.clear();
            }
            out.push(Stmt {
                name,
                op,
                params: Vec::new(),
                operands: text.trim().to_string(),
                data: None,
                line: lineno,
            });
            continue;
        }
        let mut operands = operand_field(rest[n..].trim_start()).to_string();
        // continuation cards
        while operands.ends_with(',') && i < lines.len() {
            let next: String = lines[i].chars().take(72).collect();
            if next.starts_with("//*") {
                i += 1;
                continue;
            }
            if !next.starts_with("// ") {
                break;
            }
            operands.push_str(operand_field(next[2..].trim_start()));
            i += 1;
        }
        // PROC/EXEC/IF statements may have operands with blanks inside
        // parentheses or quotes only; the above handles both.
        let params = parse_operands(&operands, lineno)?;
        let mut stmt = Stmt {
            name,
            op,
            params,
            operands,
            data: None,
            line: lineno,
        };
        if stmt.op == "DD"
            && let Some(dlm) = is_data_dd(&stmt.params)
        {
            let star = stmt
                .params
                .iter()
                .any(|p| matches!(&p.value, Value::Str(s) if s == "*"));
            let dlm = dlm.unwrap_or_else(|| "/*".to_string());
            let mut data = Vec::new();
            while i < lines.len() {
                let l = lines[i];
                if l.starts_with(&dlm) || (star && l.starts_with("//")) {
                    break;
                }
                data.push(l.chars().take(80).collect());
                i += 1;
            }
            if i < lines.len() && lines[i].starts_with(&dlm) {
                i += 1;
            }
            stmt.data = Some(data);
        }
        out.push(stmt);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statements_and_continuation() {
        let jcl = "\
//JOB1     JOB 'ACCT',CLASS=A,MSGCLASS=0,                                     00010000
// NOTIFY=&SYSUID
//* comment
//STEP1    EXEC PGM=CBACT04C,PARM='2022071800'                 a comment
//IN       DD DISP=SHR,
//         DSN=AWS.M2.CARDDEMO.ACCTDATA.PS
//OUT  DD DISP=(NEW,CATLG,DELETE),DCB=(LRECL=350,RECFM=FB,BLKSIZE=0),
//         SPACE=(CYL,(1,1),RLSE)
//SYSIN    DD *
   REPRO INFILE(IN) OUTFILE(OUT)
/*
";
        let s = parse(jcl).unwrap();
        assert_eq!(s.len(), 5, "{s:#?}");
        assert_eq!(s[0].op, "JOB");
        assert_eq!(s[0].get_str("NOTIFY"), Some("&SYSUID"));
        assert_eq!(s[1].get_str("PARM"), Some("2022071800"));
        assert_eq!(s[2].get_str("DSN"), Some("AWS.M2.CARDDEMO.ACCTDATA.PS"));
        let disp = s[3].get("DISP").unwrap().items();
        assert_eq!(disp[1].as_str(), Some("CATLG"));
        let dcb = s[3].get("DCB").unwrap();
        let Value::List(l) = dcb else { panic!() };
        assert_eq!(get(l, "RECFM").unwrap().as_str(), Some("FB"));
        let space = s[3].get("SPACE").unwrap().items();
        assert!(matches!(space[1], Value::List(_)));
        assert_eq!(
            s[4].data.as_ref().unwrap()[0].trim(),
            "REPRO INFILE(IN) OUTFILE(OUT)"
        );
    }

    #[test]
    fn implicit_sysin_and_dlm() {
        let jcl = "//S EXEC PGM=X\nDATA1\n//A DD DATA,DLM=$$\n//not jcl\n$$\n";
        let s = parse(jcl).unwrap();
        assert_eq!(s[1].name.as_deref(), Some("SYSIN"));
        assert_eq!(s[1].data.as_ref().unwrap(), &vec!["DATA1".to_string()]);
        assert_eq!(s[2].data.as_ref().unwrap(), &vec!["//not jcl".to_string()]);
    }

    #[test]
    fn quoted_operands() {
        let p = parse_operands("PARM='IT''S,OK',X=(A,'B C')", 1).unwrap();
        assert_eq!(p[0].value.as_str(), Some("IT'S,OK"));
        assert_eq!(p[1].value.items()[1].as_str(), Some("B C"));
    }
}

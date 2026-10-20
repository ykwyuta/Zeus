//! Conditional compilation, evaluated in source/COPY order before parsing.
use super::{
    lexer::{self, Tok},
    source::Pos,
};
use crate::{
    diag::{Diag, Result},
    numeric::number::Number,
    zir::Op,
};
use std::collections::HashMap;

#[derive(Clone, Debug)]
enum Value {
    Number(Number),
    Text(Vec<u8>),
    Boolean(bool),
}
impl Value {
    fn compare(&self, other: &Self) -> Result<std::cmp::Ordering> {
        Ok(match (self, other) {
            (Self::Number(a), Self::Number(b)) => a.compare(b),
            (Self::Boolean(a), Self::Boolean(b)) => a.cmp(b),
            (Self::Text(a), Self::Text(b)) => {
                let mut a = a.clone();
                let mut b = b.clone();
                let len = a.len().max(b.len());
                a.resize(len, 0x40);
                b.resize(len, 0x40);
                a.cmp(&b)
            }
            _ => {
                return Err(Diag::error(
                    Pos::default(),
                    "conditional operands must have the same category",
                ));
            }
        })
    }
    fn boolean(&self) -> Result<bool> {
        match self {
            Self::Boolean(b) => Ok(*b),
            _ => Err(Diag::error(
                Pos::default(),
                "conditional expression requires a boolean",
            )),
        }
    }
}
#[derive(Clone, Default)]
pub(super) struct Variables {
    values: HashMap<String, Value>,
    parameters: HashMap<String, Value>,
}
impl Variables {
    pub fn parameters(options: &super::FrontOptions) -> Result<Self> {
        let mut variables = Self::default();
        let version = [
            env!("CARGO_PKG_VERSION_MAJOR"),
            env!("CARGO_PKG_VERSION_MINOR"),
            env!("CARGO_PKG_VERSION_PATCH"),
        ]
        .iter()
        .fold(0u32, |version, part| {
            version * 100 + part.parse::<u32>().expect("numeric package version")
        });
        for (name, value) in [
            (
                "ARCH",
                Value::Number(Number::parse(&options.arch.unwrap_or(0).to_string()).unwrap()),
            ),
            ("CICS", Value::Boolean(options.cics)),
            ("DLL", Value::Boolean(options.dll)),
            ("DYNAM", Value::Boolean(options.dynam)),
            ("SQL", Value::Boolean(options.sql == Some(true))),
            ("SQLIMS", Value::Boolean(options.sqlims)),
            ("THREAD", Value::Boolean(options.thread)),
            (
                "LP",
                Value::Number(Number::parse(if options.lp64 { "64" } else { "32" }).unwrap()),
            ),
            (
                "OPTIMIZE",
                Value::Number(Number::parse(&options.optimize.to_string()).unwrap()),
            ),
            (
                "COMPILER-VRM",
                Value::Number(Number::parse(&version.to_string()).unwrap()),
            ),
        ] {
            variables.values.insert(name.into(), value.clone());
            variables.values.insert(format!("IGY-{name}"), value);
        }
        for (name, value) in &options.defines {
            variables.option(&format!("{name}={value}"))?;
        }
        Ok(variables)
    }
    pub fn option(&mut self, option: &str) -> Result<()> {
        let (name, text) = option
            .split_once('=')
            .ok_or_else(|| Diag::error(Pos::default(), "DEFINE option requires name=literal"))?;
        if reserved(name.trim()) {
            return Err(Diag::error(
                Pos::default(),
                "predefined compilation variable is read-only; IGY- names are reserved",
            ));
        }
        let value = Parser::new(text, self)?.finish()?;
        self.parameters
            .insert(name.trim().to_ascii_uppercase(), value);
        Ok(())
    }
}

fn reserved(name: &str) -> bool {
    let name = name.to_ascii_uppercase();
    name.starts_with("IGY-")
        || matches!(
            name.as_str(),
            "ARCH"
                | "CICS"
                | "COMPILER-VRM"
                | "DLL"
                | "DYNAM"
                | "LP"
                | "OPTIMIZE"
                | "SQL"
                | "SQLIMS"
                | "THREAD"
        )
}

struct Parser<'a> {
    tokens: Vec<Tok>,
    index: usize,
    variables: &'a Variables,
}
impl<'a> Parser<'a> {
    fn new(text: &str, variables: &'a Variables) -> Result<Self> {
        let stream: Vec<_> = text.chars().map(|c| (c, Pos::default())).collect();
        let tokens = lexer::join_suffixes(lexer::lex(&stream, false)?)
            .into_iter()
            .map(|t| t.tok)
            .collect();
        Ok(Self {
            tokens,
            index: 0,
            variables,
        })
    }
    fn word(&mut self, word: &str) -> bool {
        if matches!(self.tokens.get(self.index), Some(Tok::Word(w)) if w == word) {
            self.index += 1;
            true
        } else {
            false
        }
    }
    fn take(&mut self, token: Tok) -> bool {
        if self.tokens.get(self.index) == Some(&token) {
            self.index += 1;
            true
        } else {
            false
        }
    }
    fn finish(mut self) -> Result<Value> {
        let result = self.or()?;
        self.word("THEN");
        if self.index != self.tokens.len() {
            return Err(Diag::error(
                Pos::default(),
                "unexpected token in conditional expression",
            ));
        }
        Ok(result)
    }
    fn or(&mut self) -> Result<Value> {
        let mut left = self.and()?;
        while self.word("OR") {
            let right = self.and()?;
            left = Value::Boolean(left.boolean()? | right.boolean()?);
        }
        Ok(left)
    }
    fn and(&mut self) -> Result<Value> {
        let mut left = self.condition()?;
        while self.word("AND") {
            let right = self.condition()?;
            left = Value::Boolean(left.boolean()? & right.boolean()?);
        }
        Ok(left)
    }
    fn condition(&mut self) -> Result<Value> {
        if self.word("NOT") {
            return Ok(Value::Boolean(!self.condition()?.boolean()?));
        }
        if let Some(Tok::Word(name)) = self.tokens.get(self.index).cloned() {
            let mark = self.index;
            self.index += 1;
            self.word("IS");
            let not = self.word("NOT");
            if self.word("DEFINED") {
                return Ok(Value::Boolean(
                    self.variables.values.contains_key(&name) ^ not,
                ));
            }
            self.index = mark;
        }
        let left = self.sum()?;
        let mark = self.index;
        self.word("IS");
        let not = self.word("NOT");
        let op = if self.take(Tok::Eq) || self.word("EQUAL") {
            self.word("TO");
            Some(0)
        } else if self.take(Tok::Le) {
            Some(-2)
        } else if self.take(Tok::Ge) {
            Some(2)
        } else if self.take(Tok::Lt) || self.word("LESS") {
            self.word("THAN");
            if self.word("OR") {
                if !self.word("EQUAL") {
                    return Err(Diag::error(Pos::default(), "EQUAL expected"));
                }
                self.word("TO");
                Some(-2)
            } else {
                Some(-1)
            }
        } else if self.take(Tok::Gt) || self.word("GREATER") {
            self.word("THAN");
            if self.word("OR") {
                if !self.word("EQUAL") {
                    return Err(Diag::error(Pos::default(), "EQUAL expected"));
                }
                self.word("TO");
                Some(2)
            } else {
                Some(1)
            }
        } else {
            None
        };
        if let Some(op) = op {
            let right = self.sum()?;
            let order = left.compare(&right)?;
            let result = match op {
                0 => order.is_eq(),
                -1 => order.is_lt(),
                -2 => !order.is_gt(),
                1 => order.is_gt(),
                _ => !order.is_lt(),
            };
            Ok(Value::Boolean(result ^ not))
        } else {
            self.index = mark;
            Ok(left)
        }
    }
    fn sum(&mut self) -> Result<Value> {
        let mut left = self.product()?;
        loop {
            let op = if self.take(Tok::Plus) {
                Op::Add
            } else if self.take(Tok::Minus) {
                Op::Sub
            } else {
                break;
            };
            left = Self::arithmetic(left, op, self.product()?)?;
        }
        Ok(left)
    }
    fn product(&mut self) -> Result<Value> {
        let mut left = self.atom()?;
        loop {
            let op = if self.take(Tok::Star) {
                Op::Mul
            } else if self.take(Tok::Slash) {
                Op::Div
            } else {
                break;
            };
            left = Self::arithmetic(left, op, self.atom()?)?;
        }
        Ok(left)
    }
    fn arithmetic(left: Value, op: Op, right: Value) -> Result<Value> {
        match (left, right) {
            (Value::Number(a), Value::Number(b)) => a
                .binary(op, b, 0, false, super::numeric())
                .map(Value::Number)
                .map_err(|m| Diag::error(Pos::default(), m)),
            _ => Err(Diag::error(
                Pos::default(),
                "compile-time arithmetic requires integers",
            )),
        }
    }
    fn atom(&mut self) -> Result<Value> {
        if self.take(Tok::LParen) {
            let value = self.or()?;
            if !self.take(Tok::RParen) {
                return Err(Diag::error(
                    Pos::default(),
                    "missing conditional parenthesis",
                ));
            }
            return Ok(value);
        }
        if self.take(Tok::Plus) {
            return self.atom();
        }
        if self.take(Tok::Minus) {
            return match self.atom()? {
                Value::Number(n) => Ok(Value::Number(n.negate())),
                _ => Err(Diag::error(Pos::default(), "numeric unary minus required")),
            };
        }
        let token = self.tokens.get(self.index).cloned();
        self.index += 1;
        match token {
            Some(Tok::Num(text)) if !text.contains(['.', 'E']) => Number::parse(&text)
                .map(Value::Number)
                .map_err(|m| Diag::error(Pos::default(), m)),
            Some(Tok::Str(text)) => Ok(Value::Text(crate::ebcdic::encode(&text))),
            Some(Tok::Hex(bytes)) => Ok(Value::Text(bytes)),
            Some(Tok::Word(word)) if word == "B" => match self.tokens.get(self.index) {
                Some(Tok::Str(value)) if value == "0" || value == "1" => {
                    self.index += 1;
                    Ok(Value::Boolean(value == "1"))
                }
                _ => Err(Diag::error(
                    Pos::default(),
                    "boolean literal must be B'0' or B'1'",
                )),
            },
            Some(Tok::Word(word)) if word == "TRUE" || word == "FALSE" => {
                Ok(Value::Boolean(word == "TRUE"))
            }
            Some(Tok::Word(word)) => self.variables.values.get(&word).cloned().ok_or_else(|| {
                Diag::error(
                    Pos::default(),
                    format!("undefined compilation variable {word}"),
                )
            }),
            _ => Err(Diag::error(
                Pos::default(),
                "conditional literal or variable expected",
            )),
        }
    }
}

enum Branch {
    If {
        parent: bool,
        condition: bool,
        alternate: bool,
    },
    Evaluate {
        parent: bool,
        subject: Value,
        chosen: bool,
        active: bool,
        other: bool,
        body: bool,
    },
}
impl Branch {
    fn active(&self) -> bool {
        match self {
            Self::If {
                parent,
                condition,
                alternate,
            } => *parent && (*condition ^ *alternate),
            Self::Evaluate { parent, active, .. } => *parent && *active,
        }
    }
}

pub(super) fn expand<T>(
    text: &str,
    file: u32,
    padded: bool,
    variables: &mut Variables,
    process: &mut impl FnMut(&str, u32, &mut Variables) -> Result<Vec<T>>,
) -> Result<Vec<T>> {
    let mut out = Vec::new();
    let mut buffer = String::new();
    let mut first_line = 1;
    let mut stack: Vec<Branch> = Vec::new();
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let number = index as u32 + 1;
        let active = stack.last().is_none_or(Branch::active);
        let area: String = line.chars().skip(7).take(65).collect();
        let directive = (line.chars().nth(6) == Some(' '))
            .then(|| area.trim_start().strip_prefix(">>"))
            .flatten();
        let Some(directive) = directive else {
            if active {
                buffer.push_str(line);
            } else {
                buffer.push('\n');
            }
            if !area.trim().is_empty()
                && !matches!(line.chars().nth(6), Some('*' | '/'))
                && let Some(Branch::Evaluate { body, .. }) = stack.last_mut()
            {
                *body = true;
            }
            continue;
        };
        // CALLINTERFACE belongs to the parser, after COPY/REPLACE. Keep
        // its physical line in the ordinary chunk, without flushing a
        // partial COBOL statement or losing conditional branch selection.
        if directive.split_whitespace().next().is_some_and(|name| {
            name.eq_ignore_ascii_case("CALLINTERFACE") || name.eq_ignore_ascii_case("CALLINT")
        }) {
            if let Some(Branch::Evaluate { body, .. }) = stack.last_mut() {
                *body = true;
            }
            if active {
                buffer.push_str(line);
            } else {
                buffer.push('\n');
            }
            continue;
        }
        out.extend(process(&buffer, first_line, variables)?);
        buffer.clear();
        first_line = number + 1;
        let (name, rest) = directive
            .trim()
            .split_once(char::is_whitespace)
            .unwrap_or((directive.trim(), ""));
        let unpadded;
        let rest = if padded {
            unpadded = super::input::clean(rest);
            unpadded.as_str()
        } else {
            rest
        };
        let error = |message: &str| Diag::error(Pos { file, line: number }, message);
        let parsed = |text: &str, vars: &Variables| Parser::new(text.trim(), vars)?.finish();
        let upper = name.to_ascii_uppercase();
        if !matches!(upper.as_str(), "WHEN" | "END-EVALUATE")
            && let Some(Branch::Evaluate { body, .. }) = stack.last_mut()
        {
            *body = true;
        }
        if matches!(upper.as_str(), "ELSE" | "END-IF" | "END-EVALUATE") && !rest.trim().is_empty() {
            return Err(error("unexpected conditional directive operands"));
        }
        match upper.as_str() {
            "DEFINE" if active => {
                let mut parser = Parser::new(rest, variables)?;
                let Some(Tok::Word(name)) = parser.tokens.first().cloned() else {
                    return Err(error("DEFINE name required"));
                };
                if reserved(&name) {
                    return Err(error(
                        "predefined compilation variable is read-only; IGY- names are reserved",
                    ));
                }
                if matches!(
                    name.as_str(),
                    "DEFINE"
                        | "AS"
                        | "PARAMETER"
                        | "OVERRIDE"
                        | "OFF"
                        | "IF"
                        | "ELSE"
                        | "END-IF"
                        | "EVALUATE"
                        | "WHEN"
                        | "END-EVALUATE"
                        | "TRUE"
                        | "FALSE"
                        | "AND"
                        | "OR"
                        | "NOT"
                        | "DEFINED"
                ) {
                    return Err(error("reserved conditional compilation variable name"));
                }
                parser.index = 1;
                parser.word("AS");
                if parser.word("OFF") {
                    if parser.index != parser.tokens.len() {
                        return Err(error("unexpected DEFINE OFF operands"));
                    }
                    variables.values.remove(&name);
                    continue;
                }
                let parameter = parser.word("PARAMETER");
                let override_value = parser.tokens.last() == Some(&Tok::Word("OVERRIDE".into()));
                if override_value {
                    parser.tokens.pop();
                }
                let value = if parameter {
                    parser.variables.parameters.get(&name).cloned()
                } else {
                    Some(parser.or()?)
                };
                if parser.index != parser.tokens.len() {
                    return Err(error("unexpected DEFINE operands"));
                }
                if let Some(value) = value {
                    if !override_value
                        && variables
                            .values
                            .get(&name)
                            .is_some_and(|old| !old.compare(&value).is_ok_and(|c| c.is_eq()))
                    {
                        return Err(error("DEFINE requires OVERRIDE to change a defined value"));
                    }
                    variables.values.insert(name, value);
                } else {
                    variables.values.remove(&name);
                }
            }
            "DEFINE" => {}
            "IF" => {
                let condition = if active {
                    parsed(rest, variables)?.boolean()?
                } else {
                    false
                };
                stack.push(Branch::If {
                    parent: active,
                    condition,
                    alternate: false,
                });
            }
            "ELSE" => match stack.last_mut() {
                Some(Branch::If { alternate, .. }) if !*alternate => *alternate = true,
                _ => return Err(error("unexpected or duplicate ELSE")),
            },
            "END-IF" => {
                if !matches!(stack.pop(), Some(Branch::If { .. })) {
                    return Err(error("unmatched END-IF"));
                }
            }
            "EVALUATE" => {
                let subject = if active {
                    parsed(rest, variables)?
                } else {
                    Value::Boolean(false)
                };
                stack.push(Branch::Evaluate {
                    parent: active,
                    subject,
                    chosen: false,
                    active: false,
                    other: false,
                    body: false,
                });
            }
            "WHEN" => {
                let Some(Branch::Evaluate {
                    parent,
                    subject,
                    chosen,
                    active,
                    other,
                    body,
                }) = stack.last_mut()
                else {
                    return Err(error("WHEN without EVALUATE"));
                };
                if *other {
                    return Err(error("WHEN after WHEN OTHER"));
                }
                let text = rest.trim();
                let matches = if text.eq_ignore_ascii_case("OTHER") {
                    *other = true;
                    !*chosen
                } else if !*parent || (*chosen && *body) {
                    false
                } else {
                    let mut parser = Parser::new(text, variables)?;
                    let low = parser.or()?;
                    if parser.word("THRU") || parser.word("THROUGH") {
                        let high = parser.or()?;
                        if parser.index != parser.tokens.len() {
                            return Err(error("unexpected WHEN operands"));
                        }
                        !subject.compare(&low)?.is_lt() && !subject.compare(&high)?.is_gt()
                    } else {
                        if parser.index != parser.tokens.len() {
                            return Err(error("unexpected WHEN operands"));
                        }
                        subject.compare(&low)?.is_eq()
                    }
                };
                *active = matches || (!*body && *active);
                *chosen |= *active;
                *body = false;
            }
            "END-EVALUATE" => {
                if !matches!(stack.pop(), Some(Branch::Evaluate { .. })) {
                    return Err(error("unmatched END-EVALUATE"));
                }
            }
            _ => return Err(error("unknown compiler directive")),
        }
    }
    if !stack.is_empty() {
        return Err(Diag::error(
            Pos {
                file,
                line: text.lines().count() as u32,
            },
            "unterminated conditional compilation directive",
        ));
    }
    out.extend(process(&buffer, first_line, variables)?);
    Ok(out)
}

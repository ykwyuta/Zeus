//! The expression language of the API mappings: a subset of JSONata
//! (paths, literals, arrays, comparisons, `and` / `or`, `&`, arithmetic,
//! the conditional `?:`, lambdas and the functions the mappings use).
//!
//! A value is a JSON value or undefined (`None`). A path step on an array
//! applies to each element and flattens the results; a field that does not
//! exist is undefined, and an operator with an undefined operand gives
//! undefined (comparisons give false).

use std::collections::HashMap;
use std::fmt;

use serde_json::{Map, Number, Value};

#[derive(Debug, Clone, PartialEq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

type R<T> = Result<T, Error>;

fn err<T>(m: impl Into<String>) -> R<T> {
    Err(Error(m.into()))
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    /// A field name (or the keywords and, or, true, false, null, function).
    Name(String),
    /// `$name` (`$` alone: the context).
    Var(String),
    Regex(String),
    Op(&'static str),
}

const OPS: &[&str] = &[
    "!=", "<=", ">=", "..", ":=", ".", "(", ")", "[", "]", "{", "}", ",", "?", ":", "=", "<", ">",
    "+", "-", "*", "/", "%", "&", ";",
];

fn lex(s: &str) -> R<Vec<Tok>> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        if ch == '"' || ch == '\'' || ch == '`' {
            let q = ch;
            let mut t = String::new();
            i += 1;
            while i < c.len() && c[i] != q {
                if c[i] == '\\' && i + 1 < c.len() {
                    i += 1;
                    t.push(match c[i] {
                        'n' => '\n',
                        't' => '\t',
                        o => o,
                    });
                } else {
                    t.push(c[i]);
                }
                i += 1;
            }
            if i >= c.len() {
                return err("unterminated string");
            }
            i += 1;
            out.push(if q == '`' { Tok::Name(t) } else { Tok::Str(t) });
            continue;
        }
        if ch.is_ascii_digit() {
            let st = i;
            while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
                // `1..3` is a range, not a number
                if c[i] == '.' && c.get(i + 1) == Some(&'.') {
                    break;
                }
                i += 1;
            }
            if i < c.len() && (c[i] == 'e' || c[i] == 'E') {
                i += 1;
                if i < c.len() && (c[i] == '+' || c[i] == '-') {
                    i += 1;
                }
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let t: String = c[st..i].iter().collect();
            out.push(Tok::Num(
                t.parse().map_err(|_| Error(format!("bad number {t}")))?,
            ));
            continue;
        }
        if ch == '/' && regex_allowed(out.last()) {
            let mut t = String::new();
            i += 1;
            while i < c.len() && c[i] != '/' {
                if c[i] == '\\' && i + 1 < c.len() {
                    t.push('\\');
                    i += 1;
                }
                t.push(c[i]);
                i += 1;
            }
            i += 1;
            // flags
            while i < c.len() && c[i].is_ascii_alphabetic() {
                if c[i] == 'i' {
                    t.insert_str(0, "(?i)");
                }
                i += 1;
            }
            out.push(Tok::Regex(t));
            continue;
        }
        if ch == '$' {
            i += 1;
            let st = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            out.push(Tok::Var(c[st..i].iter().collect()));
            continue;
        }
        if ch.is_alphabetic() || ch == '_' {
            let st = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            out.push(Tok::Name(c[st..i].iter().collect()));
            continue;
        }
        let rest: String = c[i..c.len().min(i + 2)].iter().collect();
        match OPS.iter().find(|o| rest.starts_with(**o)) {
            Some(o) => {
                out.push(Tok::Op(o));
                i += o.len();
            }
            None => return err(format!("unexpected character {ch:?}")),
        }
    }
    Ok(out)
}

/// A `/` starts a regular expression where an operand is expected.
fn regex_allowed(prev: Option<&Tok>) -> bool {
    match prev {
        None => true,
        Some(Tok::Op(o)) => !matches!(*o, ")" | "]" | "}"),
        Some(Tok::Name(n)) => matches!(n.as_str(), "and" | "or"),
        _ => false,
    }
}

#[derive(Debug, Clone)]
pub enum Expr {
    Lit(Value),
    Regex(String),
    Var(String),
    /// A field of the context.
    Name(String),
    Path(Box<Expr>, Box<Expr>),
    /// `e[pred]`: a filter or an index.
    Filter(Box<Expr>, Box<Expr>),
    Array(Vec<Expr>),
    Object(Vec<(Expr, Expr)>),
    Unary(&'static str, Box<Expr>),
    Bin(&'static str, Box<Expr>, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    Call(String, Vec<Expr>),
    Lambda(Vec<String>, Box<Expr>),
    Range(Box<Expr>, Box<Expr>),
}

struct Parser {
    t: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    fn is_op(&self, o: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(x)) if *x == o)
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Name(x)) if x == w)
    }

    fn expect(&mut self, o: &str) -> R<()> {
        if self.is_op(o) {
            self.i += 1;
            Ok(())
        } else {
            err(format!("{o} expected, found {:?}", self.peek()))
        }
    }

    fn expr(&mut self) -> R<Expr> {
        let c = self.or()?;
        if self.is_op("?") {
            self.i += 1;
            let a = self.expr()?;
            let b = if self.is_op(":") {
                self.i += 1;
                Some(Box::new(self.expr()?))
            } else {
                None
            };
            return Ok(Expr::Cond(Box::new(c), Box::new(a), b));
        }
        Ok(c)
    }

    fn or(&mut self) -> R<Expr> {
        let mut l = self.and()?;
        while self.is_word("or") {
            self.i += 1;
            l = Expr::Bin("or", Box::new(l), Box::new(self.and()?));
        }
        Ok(l)
    }

    fn and(&mut self) -> R<Expr> {
        let mut l = self.cmp()?;
        while self.is_word("and") {
            self.i += 1;
            l = Expr::Bin("and", Box::new(l), Box::new(self.cmp()?));
        }
        Ok(l)
    }

    fn cmp(&mut self) -> R<Expr> {
        let l = self.concat()?;
        for o in ["=", "!=", "<", "<=", ">", ">="] {
            if self.is_op(o) {
                self.i += 1;
                let r = self.concat()?;
                let o = OPS.iter().find(|x| **x == o).copied().unwrap_or("=");
                return Ok(Expr::Bin(o, Box::new(l), Box::new(r)));
            }
        }
        if self.is_word("in") {
            self.i += 1;
            let r = self.concat()?;
            return Ok(Expr::Bin("in", Box::new(l), Box::new(r)));
        }
        Ok(l)
    }

    fn concat(&mut self) -> R<Expr> {
        let mut l = self.add()?;
        while self.is_op("&") {
            self.i += 1;
            l = Expr::Bin("&", Box::new(l), Box::new(self.add()?));
        }
        Ok(l)
    }

    fn add(&mut self) -> R<Expr> {
        let mut l = self.mul()?;
        loop {
            let o = if self.is_op("+") {
                "+"
            } else if self.is_op("-") {
                "-"
            } else {
                return Ok(l);
            };
            self.i += 1;
            l = Expr::Bin(o, Box::new(l), Box::new(self.mul()?));
        }
    }

    fn mul(&mut self) -> R<Expr> {
        let mut l = self.unary()?;
        loop {
            let o = if self.is_op("*") {
                "*"
            } else if self.is_op("/") {
                "/"
            } else if self.is_op("%") {
                "%"
            } else {
                return Ok(l);
            };
            self.i += 1;
            l = Expr::Bin(o, Box::new(l), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> R<Expr> {
        if self.is_op("-") {
            self.i += 1;
            return Ok(Expr::Unary("-", Box::new(self.unary()?)));
        }
        self.path()
    }

    fn path(&mut self) -> R<Expr> {
        let mut e = self.primary()?;
        loop {
            if self.is_op(".") {
                self.i += 1;
                let step = match self.peek().cloned() {
                    Some(Tok::Name(n)) => {
                        self.i += 1;
                        Expr::Name(n)
                    }
                    // a quoted field name
                    Some(Tok::Str(s)) => {
                        self.i += 1;
                        Expr::Name(s)
                    }
                    _ => self.primary()?,
                };
                e = Expr::Path(Box::new(e), Box::new(step));
            } else if self.is_op("[") {
                self.i += 1;
                if self.is_op("]") {
                    // e[]: keep as an array
                    self.i += 1;
                    e = Expr::Array(vec![e]);
                    continue;
                }
                let p = self.expr()?;
                self.expect("]")?;
                e = Expr::Filter(Box::new(e), Box::new(p));
            } else {
                return Ok(e);
            }
        }
    }

    fn primary(&mut self) -> R<Expr> {
        let Some(t) = self.peek().cloned() else {
            return err("unexpected end of expression");
        };
        self.i += 1;
        Ok(match t {
            Tok::Num(n) => Expr::Lit(num(n)),
            Tok::Str(s) => Expr::Lit(Value::String(s)),
            Tok::Regex(r) => Expr::Regex(r),
            Tok::Var(v) => {
                if self.is_op("(") {
                    self.i += 1;
                    let mut args = Vec::new();
                    while !self.is_op(")") {
                        args.push(self.expr()?);
                        if self.is_op(",") {
                            self.i += 1;
                        }
                    }
                    self.i += 1;
                    Expr::Call(v, args)
                } else {
                    Expr::Var(v)
                }
            }
            Tok::Name(n) => match n.as_str() {
                "true" => Expr::Lit(Value::Bool(true)),
                "false" => Expr::Lit(Value::Bool(false)),
                "null" => Expr::Lit(Value::Null),
                "function" => {
                    self.expect("(")?;
                    let mut ps = Vec::new();
                    while !self.is_op(")") {
                        match self.peek().cloned() {
                            Some(Tok::Var(v)) => ps.push(v),
                            o => return err(format!("parameter expected, found {o:?}")),
                        }
                        self.i += 1;
                        if self.is_op(",") {
                            self.i += 1;
                        }
                    }
                    self.i += 1;
                    self.expect("{")?;
                    let body = self.expr()?;
                    self.expect("}")?;
                    Expr::Lambda(ps, Box::new(body))
                }
                _ => Expr::Name(n),
            },
            Tok::Op("(") => {
                let e = self.expr()?;
                self.expect(")")?;
                e
            }
            Tok::Op("[") => {
                let mut items = Vec::new();
                while !self.is_op("]") {
                    let a = self.expr()?;
                    if self.is_op("..") {
                        self.i += 1;
                        let b = self.expr()?;
                        items.push(Expr::Range(Box::new(a), Box::new(b)));
                    } else {
                        items.push(a);
                    }
                    if self.is_op(",") {
                        self.i += 1;
                    } else if !self.is_op("]") {
                        return err("] expected");
                    }
                }
                self.i += 1;
                Expr::Array(items)
            }
            Tok::Op("{") => {
                let mut pairs = Vec::new();
                while !self.is_op("}") {
                    let k = self.expr()?;
                    self.expect(":")?;
                    let v = self.expr()?;
                    pairs.push((k, v));
                    if self.is_op(",") {
                        self.i += 1;
                    }
                }
                self.i += 1;
                Expr::Object(pairs)
            }
            o => return err(format!("unexpected {o:?}")),
        })
    }
}

/// Parses an expression.
pub fn parse(s: &str) -> R<Expr> {
    let mut p = Parser { t: lex(s)?, i: 0 };
    let e = p.expr()?;
    if p.i < p.t.len() {
        return err(format!("unexpected {:?} in {s:?}", p.t[p.i]));
    }
    Ok(e)
}

fn num(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.0e15 {
        Value::Number(Number::from(n as i64))
    } else {
        Number::from_f64(n).map_or(Value::Null, Value::Number)
    }
}

/// The variables of an evaluation.
#[derive(Clone, Default)]
pub struct Env {
    pub vars: HashMap<String, Value>,
    /// What bare names refer to.
    pub context: Value,
}

type V = Option<Value>;

fn truthy(v: &V) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().unwrap_or(0.0) != 0.0,
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => a.iter().any(|x| truthy(&Some(x.clone()))),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        _ => None,
    }
}

/// The string form of a value (`$string`).
pub fn string_of(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            let f = n.as_f64().unwrap_or(0.0);
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", f as i64)
            } else {
                n.to_string()
            }
        }
        Value::Null => "null".into(),
        o => o.to_string(),
    }
}

/// A field of a value: of each element of an array (flattened).
fn field(v: &Value, name: &str) -> V {
    match v {
        Value::Object(o) => o.get(name).cloned(),
        Value::Array(a) => {
            let mut out = Vec::new();
            for x in a {
                match field(x, name) {
                    Some(Value::Array(y)) => out.extend(y),
                    Some(y) => out.push(y),
                    None => {}
                }
            }
            match out.len() {
                0 => None,
                1 => out.pop(),
                _ => Some(Value::Array(out)),
            }
        }
        _ => None,
    }
}

impl Env {
    pub fn eval(&self, e: &Expr) -> R<V> {
        self.eval_in(e, &Some(self.context.clone()))
    }

    fn eval_in(&self, e: &Expr, ctx: &V) -> R<V> {
        Ok(match e {
            Expr::Lit(v) => Some(v.clone()),
            Expr::Regex(r) => Some(Value::String(format!("/{r}/"))),
            Expr::Var(n) if n.is_empty() => ctx.clone(),
            Expr::Var(n) => self.vars.get(n).cloned(),
            Expr::Name(n) => ctx.as_ref().and_then(|c| field(c, n)),
            Expr::Path(a, b) => {
                let base = self.eval_in(a, ctx)?;
                match (base, b.as_ref()) {
                    (None, _) => None,
                    (Some(v), Expr::Name(n)) => field(&v, n),
                    (Some(Value::Array(items)), step) => {
                        let mut out = Vec::new();
                        for x in items {
                            match self.eval_in(step, &Some(x))? {
                                Some(Value::Array(y)) => out.extend(y),
                                Some(y) => out.push(y),
                                None => {}
                            }
                        }
                        (!out.is_empty()).then_some(Value::Array(out))
                    }
                    (Some(v), step) => self.eval_in(step, &Some(v))?,
                }
            }
            Expr::Filter(a, p) => {
                let Some(base) = self.eval_in(a, ctx)? else {
                    return Ok(None);
                };
                let items = match base {
                    Value::Array(a) => a,
                    o => vec![o],
                };
                // a number: an index (negative from the end)
                if let Expr::Lit(Value::Number(n)) = p.as_ref() {
                    let k = n.as_f64().unwrap_or(0.0) as i64;
                    let k = if k < 0 { items.len() as i64 + k } else { k };
                    return Ok(items.get(k.max(0) as usize).cloned());
                }
                let mut out = Vec::new();
                for x in items {
                    let r = self.eval_in(p, &Some(x.clone()))?;
                    let keep = match &r {
                        Some(Value::Number(n)) => {
                            // computed index
                            out.len() == n.as_f64().unwrap_or(-1.0) as usize
                        }
                        _ => truthy(&r),
                    };
                    if keep {
                        out.push(x);
                    }
                }
                match out.len() {
                    0 => None,
                    1 => out.pop(),
                    _ => Some(Value::Array(out)),
                }
            }
            Expr::Array(items) => {
                let mut out = Vec::new();
                for x in items {
                    match x {
                        Expr::Range(a, b) => {
                            let (a, b) = (self.eval_in(a, ctx)?, self.eval_in(b, ctx)?);
                            if let (Some(a), Some(b)) =
                                (a.as_ref().and_then(as_f64), b.as_ref().and_then(as_f64))
                            {
                                for k in a as i64..=b as i64 {
                                    out.push(Value::from(k));
                                }
                            }
                        }
                        _ => match self.eval_in(x, ctx)? {
                            Some(Value::Array(y)) => out.extend(y),
                            Some(y) => out.push(y),
                            None => {}
                        },
                    }
                }
                Some(Value::Array(out))
            }
            Expr::Object(pairs) => {
                let mut m = Map::new();
                for (k, v) in pairs {
                    let k = self
                        .eval_in(k, ctx)?
                        .map(|k| string_of(&k))
                        .unwrap_or_default();
                    if let Some(v) = self.eval_in(v, ctx)? {
                        m.insert(k, v);
                    }
                }
                Some(Value::Object(m))
            }
            Expr::Unary(_, a) => self
                .eval_in(a, ctx)?
                .and_then(|v| as_f64(&v))
                .map(|n| num(-n)),
            Expr::Bin(op, a, b) => self.binary(op, a, b, ctx)?,
            Expr::Cond(c, a, b) => {
                if truthy(&self.eval_in(c, ctx)?) {
                    self.eval_in(a, ctx)?
                } else {
                    match b {
                        Some(b) => self.eval_in(b, ctx)?,
                        None => None,
                    }
                }
            }
            Expr::Call(f, args) => self.call(f, args, ctx)?,
            Expr::Lambda(..) => return err("a function is not a value here"),
            Expr::Range(..) => return err("a range is only allowed in an array"),
        })
    }

    fn binary(&self, op: &str, a: &Expr, b: &Expr, ctx: &V) -> R<V> {
        if op == "and" {
            return Ok(Some(Value::Bool(
                truthy(&self.eval_in(a, ctx)?) && truthy(&self.eval_in(b, ctx)?),
            )));
        }
        if op == "or" {
            return Ok(Some(Value::Bool(
                truthy(&self.eval_in(a, ctx)?) || truthy(&self.eval_in(b, ctx)?),
            )));
        }
        let (x, y) = (self.eval_in(a, ctx)?, self.eval_in(b, ctx)?);
        Ok(match op {
            "&" => Some(Value::String(
                x.map(|v| string_of(&v)).unwrap_or_default()
                    + &y.map(|v| string_of(&v)).unwrap_or_default(),
            )),
            "=" | "!=" => {
                let (Some(x), Some(y)) = (x, y) else {
                    return Ok(Some(Value::Bool(false)));
                };
                let eq = match (as_f64(&x), as_f64(&y)) {
                    (Some(p), Some(q)) => p == q,
                    _ => x == y,
                };
                Some(Value::Bool(if op == "=" { eq } else { !eq }))
            }
            "<" | "<=" | ">" | ">=" => {
                let (Some(x), Some(y)) = (x, y) else {
                    return Ok(Some(Value::Bool(false)));
                };
                let o = match (&x, &y) {
                    (Value::String(p), Value::String(q)) => p.partial_cmp(q),
                    _ => match (as_f64(&x), as_f64(&y)) {
                        (Some(p), Some(q)) => p.partial_cmp(&q),
                        _ => return err(format!("cannot compare {x} and {y}")),
                    },
                };
                let Some(o) = o else {
                    return Ok(Some(Value::Bool(false)));
                };
                use std::cmp::Ordering::*;
                Some(Value::Bool(match op {
                    "<" => o == Less,
                    "<=" => o != Greater,
                    ">" => o == Greater,
                    _ => o != Less,
                }))
            }
            "in" => {
                let (Some(x), Some(y)) = (x, y) else {
                    return Ok(Some(Value::Bool(false)));
                };
                Some(Value::Bool(match y {
                    Value::Array(a) => a.contains(&x),
                    o => o == x,
                }))
            }
            _ => {
                let (Some(x), Some(y)) = (x, y) else {
                    return Ok(None);
                };
                let (Some(p), Some(q)) = (as_f64(&x), as_f64(&y)) else {
                    return err(format!("{op}: {x} and {y} are not numbers"));
                };
                Some(num(match op {
                    "+" => p + q,
                    "-" => p - q,
                    "*" => p * q,
                    "/" => p / q,
                    _ => p % q,
                }))
            }
        })
    }

    fn call(&self, f: &str, args: &[Expr], ctx: &V) -> R<V> {
        // $filter / $map: the lambda is applied to each element
        if matches!(f, "filter" | "map") {
            let items = match self.eval_in(&args[0], ctx)? {
                None => return Ok(None),
                Some(Value::Array(a)) => a,
                Some(o) => vec![o],
            };
            let Some(Expr::Lambda(ps, body)) = args.get(1) else {
                return err(format!("${f} needs a function"));
            };
            let mut out = Vec::new();
            for (k, x) in items.into_iter().enumerate() {
                let mut env = self.clone();
                if let Some(p) = ps.first() {
                    env.vars.insert(p.clone(), x.clone());
                }
                if let Some(p) = ps.get(1) {
                    env.vars.insert(p.clone(), Value::from(k));
                }
                let r = env.eval_in(body, ctx)?;
                if f == "filter" {
                    if truthy(&r) {
                        out.push(x);
                    }
                } else if let Some(r) = r {
                    out.push(r);
                }
            }
            return Ok(Some(Value::Array(out)));
        }
        let mut v = Vec::new();
        for a in args {
            v.push(self.eval_in(a, ctx)?);
        }
        let s = |k: usize| -> Option<String> { v.get(k).cloned().flatten().map(|x| string_of(&x)) };
        let n = |k: usize| -> Option<f64> {
            v.get(k).cloned().flatten().and_then(|x| match x {
                Value::Number(n) => n.as_f64(),
                Value::String(s) => s.trim().parse().ok(),
                Value::Bool(b) => Some(f64::from(u8::from(b))),
                _ => None,
            })
        };
        Ok(match f {
            "exists" => Some(Value::Bool(v.first().is_some_and(Option::is_some))),
            "not" => Some(Value::Bool(!truthy(&v.first().cloned().flatten()))),
            "boolean" => Some(Value::Bool(truthy(&v.first().cloned().flatten()))),
            "number" => match v.first().cloned().flatten() {
                None => None,
                Some(Value::String(t)) => match t.trim().parse::<f64>() {
                    Ok(x) => Some(num(x)),
                    Err(_) => return err(format!("$number: {t:?} is not a number")),
                },
                Some(_) => n(0).map(num),
            },
            "string" => v
                .first()
                .cloned()
                .flatten()
                .map(|x| Value::String(string_of(&x))),
            "length" => s(0).map(|t| Value::from(t.chars().count())),
            "count" => Some(Value::from(match v.first().cloned().flatten() {
                None => 0,
                Some(Value::Array(a)) => a.len(),
                Some(_) => 1,
            })),
            "sum" => match v.first().cloned().flatten() {
                Some(Value::Array(a)) => Some(num(a.iter().filter_map(as_f64).sum())),
                Some(x) => as_f64(&x).map(num),
                None => Some(num(0.0)),
            },
            "abs" => n(0).map(|x| num(x.abs())),
            "floor" => n(0).map(|x| num(x.floor())),
            "ceil" => n(0).map(|x| num(x.ceil())),
            "round" => n(0).map(|x| {
                let p = 10f64.powi(n(1).unwrap_or(0.0) as i32);
                num((x * p).round() / p)
            }),
            "trim" => {
                s(0).map(|t| Value::String(t.split_whitespace().collect::<Vec<_>>().join(" ")))
            }
            "uppercase" => s(0).map(|t| Value::String(t.to_uppercase())),
            "lowercase" => s(0).map(|t| Value::String(t.to_lowercase())),
            "substring" => s(0).map(|t| {
                let c: Vec<char> = t.chars().collect();
                let len = c.len() as i64;
                let mut st = n(1).unwrap_or(0.0) as i64;
                if st < 0 {
                    st = (len + st).max(0);
                }
                let st = st.min(len) as usize;
                let cnt = n(2).map_or(len, |x| x.max(0.0) as i64) as usize;
                Value::String(c[st..(st + cnt).min(c.len())].iter().collect())
            }),
            "substringBefore" => match (s(0), s(1)) {
                (Some(t), Some(p)) => Some(Value::String(
                    t.find(&p).map_or(t.clone(), |i| t[..i].to_string()),
                )),
                _ => None,
            },
            "substringAfter" => match (s(0), s(1)) {
                (Some(t), Some(p)) => Some(Value::String(
                    t.find(&p)
                        .map_or(t.clone(), |i| t[i + p.len()..].to_string()),
                )),
                _ => None,
            },
            "contains" => match (s(0), v.get(1).cloned().flatten()) {
                (Some(t), Some(Value::String(p))) => Some(Value::Bool(match regex_of(&p) {
                    Some(r) => r.is_match(&t),
                    None => t.contains(&p),
                })),
                _ => None,
            },
            "replace" => match (s(0), v.get(1).cloned().flatten(), s(2)) {
                (Some(t), Some(Value::String(p)), Some(r)) => {
                    Some(Value::String(match regex_of(&p) {
                        Some(re) => re.replace_all(&t, r.as_str()).into_owned(),
                        None => t.replace(&p, &r),
                    }))
                }
                _ => None,
            },
            "match" => match (s(0), v.get(1).cloned().flatten()) {
                (Some(t), Some(Value::String(p))) => {
                    let Some(re) = regex_of(&p) else {
                        return err("$match needs a regular expression");
                    };
                    let m: Vec<Value> = re
                        .find_iter(&t)
                        .map(|m| {
                            let mut o = Map::new();
                            o.insert("match".into(), Value::String(m.as_str().into()));
                            o.insert("index".into(), Value::from(m.start()));
                            Value::Object(o)
                        })
                        .collect();
                    match m.len() {
                        0 => None,
                        1 => m.into_iter().next(),
                        _ => Some(Value::Array(m)),
                    }
                }
                _ => None,
            },
            "pad" => match (s(0), n(1)) {
                (Some(t), Some(w)) => {
                    let ch = s(2).and_then(|c| c.chars().next()).unwrap_or(' ');
                    let have = t.chars().count() as i64;
                    let w = w as i64;
                    let fill: String =
                        std::iter::repeat_n(ch, (w.abs() - have).max(0) as usize).collect();
                    Some(Value::String(if w < 0 { fill + &t } else { t + &fill }))
                }
                _ => None,
            },
            "formatNumber" => match (n(0), s(1)) {
                (Some(x), Some(p)) => Some(Value::String(format_number(x, &p))),
                _ => None,
            },
            "now" => Some(Value::String(now_iso())),
            "join" => match v.first().cloned().flatten() {
                Some(Value::Array(a)) => Some(Value::String(
                    a.iter()
                        .map(string_of)
                        .collect::<Vec<_>>()
                        .join(&s(1).unwrap_or_default()),
                )),
                Some(x) => Some(Value::String(string_of(&x))),
                None => None,
            },
            other => return err(format!("function ${other} is not supported")),
        })
    }
}

/// A regular expression written `/.../` (the pattern argument of
/// $match, $replace, $contains).
fn regex_of(p: &str) -> Option<regex::Regex> {
    let inner = p.strip_prefix('/')?.strip_suffix('/')?;
    regex::Regex::new(inner).ok()
}

/// `$formatNumber` with a picture of 0 and # digits (and one point).
fn format_number(x: f64, pic: &str) -> String {
    let (ip, fp) = pic.split_once('.').unwrap_or((pic, ""));
    let decimals = fp.chars().filter(|c| *c == '0' || *c == '#').count();
    let min_int = ip.chars().filter(|c| *c == '0').count();
    let neg = x < 0.0;
    let t = format!("{:.*}", decimals, x.abs());
    let (i, f) = t.split_once('.').unwrap_or((&t, ""));
    let mut i = i.to_string();
    while i.len() < min_int {
        i.insert(0, '0');
    }
    if min_int == 0 && i == "0" {
        i.clear();
    }
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push_str(&i);
    if decimals > 0 {
        out.push('.');
        out.push_str(f);
    }
    out
}

/// The current time in ISO 8601 (UTC, milliseconds).
fn now_iso() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = d.as_secs() as i64;
    let days = secs.div_euclid(86400);
    let sod = secs.rem_euclid(86400);
    // civil date from days since 1970-01-01
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        sod / 3600,
        sod / 60 % 60,
        sod % 60,
        d.subsec_millis()
    )
}

/// Evaluates `{{expression}}` parts of a template; the rest is literal
/// text. A template that is one expression gives its value unchanged.
pub fn template(env: &Env, t: &str) -> R<V> {
    let trimmed = t.trim();
    if let Some(inner) = trimmed
        .strip_prefix("{{")
        .and_then(|x| x.strip_suffix("}}"))
        && !inner.contains("{{")
    {
        let v = env.eval(&parse(inner)?)?;
        // a template gives text
        return Ok(v.map(|x| match x {
            Value::String(_) => x,
            Value::Null => Value::String(String::new()),
            o => Value::String(string_of(&o)),
        }));
    }
    let mut out = String::new();
    let mut rest = t;
    while let Some(i) = rest.find("{{") {
        out.push_str(&rest[..i]);
        let Some(j) = rest[i..].find("}}") else {
            return err(format!("unterminated {{{{ in {t:?}"));
        };
        let e = parse(&rest[i + 2..i + j])?;
        if let Some(v) = env.eval(&e)? {
            out.push_str(&string_of(&v));
        }
        rest = &rest[i + j + 2..];
    }
    out.push_str(rest);
    Ok(Some(Value::String(out)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn env() -> Env {
        let mut e = Env::default();
        e.vars.insert(
            "pathParameters".into(),
            json!({"accountId": "987654-00000012", "customerId": "15"}),
        );
        e.vars.insert(
            "body".into(),
            json!({"dateOfBirth": "1985-12-10", "amount": 12.5, "address": {"city": "Leeds"}}),
        );
        e.vars.insert(
            "zosAssetResponse".into(),
            json!({"commarea": {"A-B": {"OK": "Y", "LIST": [
                {"N": 1, "AMT": -5}, {"N": 0, "AMT": 3}, {"N": 2, "AMT": 7}]}}}),
        );
        e.context = json!({"body": {"firstName": "Ada"}});
        e
    }

    fn ev(s: &str) -> Option<Value> {
        env().eval(&parse(s).unwrap()).unwrap()
    }

    #[test]
    fn paths_and_operators() {
        assert_eq!(ev("$body.address.city"), Some(json!("Leeds")));
        assert_eq!(
            ev("$zosAssetResponse.commarea.\"A-B\".\"OK\" = 'Y'"),
            Some(json!(true))
        );
        assert_eq!(
            ev("$zosAssetResponse.commarea.\"A-B\".OK != \"Y\""),
            Some(json!(false))
        );
        assert_eq!(ev("$exists($body.nothing) ? 1 : 2"), Some(json!(2)));
        assert_eq!(ev("$exists(body.firstName) ? $body.firstName : \"\""), None);
        assert_eq!(ev("$exists(body.lastName) ? 1 : \"\""), Some(json!("")));
        assert_eq!(
            ev("$not($exists($zosAssetResponse.abendCode))"),
            Some(json!(true))
        );
        assert_eq!(ev("$body.amount * 2 + 1"), Some(json!(26)));
        assert_eq!(ev("'a' & 1 & 'b'"), Some(json!("a1b")));
        assert_eq!(ev("$body.missing = 'x'"), Some(json!(false)));
    }

    #[test]
    fn functions() {
        assert_eq!(
            ev("$number($substringBefore($pathParameters.accountId, '-'))"),
            Some(json!(987654))
        );
        assert_eq!(
            ev("$number($substringAfter($pathParameters.accountId, '-'))"),
            Some(json!(12))
        );
        assert_eq!(
            ev("$number($substring($body.dateOfBirth, 5, 2))"),
            Some(json!(12))
        );
        assert_eq!(
            ev(
                "$exists($body.dateOfBirth) and $match($body.dateOfBirth, /^[0-9]{4}-[0-9]{2}-[0-9]{2}$/) ? 1 : 0"
            ),
            Some(json!(1))
        );
        assert_eq!(
            ev("$number($replace('2024-01-31', '-', ''))"),
            Some(json!(20240131))
        );
        assert_eq!(
            ev("$formatNumber($number($pathParameters.customerId), '0000000000')"),
            Some(json!("0000000015"))
        );
        assert_eq!(
            ev("$substring('00000000', 0, 8 - $length($string(42))) & $string(42)"),
            Some(json!("00000042"))
        );
        assert_eq!(
            ev("$count($zosAssetResponse.commarea.\"A-B\".LIST)"),
            Some(json!(3))
        );
        assert_eq!(ev("$abs(-4.5)"), Some(json!(4.5)));
        assert_eq!(
            ev(
                "[$filter($zosAssetResponse.commarea.\"A-B\".LIST, function($v) { $number($v.N) > 0 })].AMT"
            ),
            Some(json!([-5, 7]))
        );
    }

    #[test]
    fn templates() {
        let e = env();
        assert_eq!(
            template(&e, "{{$pathParameters.customerId}}").unwrap(),
            Some(json!("15"))
        );
        assert_eq!(
            template(&e, "{{$body.address.city}}-{{$body.amount}}").unwrap(),
            Some(json!("Leeds-12.5"))
        );
        assert_eq!(template(&e, "GBP").unwrap(), Some(json!("GBP")));
    }
}

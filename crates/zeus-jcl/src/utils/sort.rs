//! SORT: sorts, merges or copies records (DFSORT control statements).
//!
//! Supported statements: SORT / MERGE FIELDS=(p,l,f,o,...) or
//! FIELDS=COPY (with FORMAT=f), INCLUDE / OMIT COND=(...), INREC /
//! OUTREC FIELDS= / BUILD=, SUM FIELDS=NONE, OPTION COPY. Field formats:
//! CH, AC (character), ZD (zoned decimal), PD (packed decimal), BI
//! (unsigned binary), FI (signed binary). Symbols from SYMNAMES
//! (`name,p,l,f` or `name,constant`) may be used for fields and
//! constants. Input is SORTIN (SORTINnn for MERGE), output SORTOUT.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::io::Write;

use zeus_cobol::ebcdic;
use zeus_decimal::packed;
use zeus_decimal::zoned;
use zeus_ds::catalog::WriteMode;

use crate::job::{Outcome, ProgramResult, StepCtx};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fmt {
    Ch,
    Zd,
    Pd,
    Bi,
    Fi,
}

impl Fmt {
    fn parse(s: &str) -> Option<Fmt> {
        Some(match s {
            "CH" | "AC" | "CSL" => Fmt::Ch,
            "ZD" => Fmt::Zd,
            "PD" => Fmt::Pd,
            "BI" => Fmt::Bi,
            "FI" => Fmt::Fi,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct Field {
    pos: usize,
    len: usize,
    fmt: Fmt,
}

impl Field {
    fn bytes<'a>(&self, r: &'a [u8]) -> &'a [u8] {
        let s = (self.pos - 1).min(r.len());
        let e = (self.pos - 1 + self.len).min(r.len());
        &r[s..e]
    }

    fn number(&self, r: &[u8]) -> i128 {
        let b = self.bytes(r);
        match self.fmt {
            Fmt::Zd => {
                let (v, neg) = zoned::decode_unchecked(b, zoned::SignPosition::TrailingOverpunch);
                if neg { -(v as i128) } else { v as i128 }
            }
            Fmt::Pd => {
                let (v, neg) = packed::decode_unchecked(b);
                if neg { -(v as i128) } else { v as i128 }
            }
            Fmt::Bi => b.iter().fold(0i128, |a, &x| a * 256 + x as i128),
            Fmt::Fi => {
                let u = b.iter().fold(0i128, |a, &x| a * 256 + x as i128);
                let bits = 8 * b.len() as u32;
                if bits > 0 && u >= 1i128 << (bits - 1) {
                    u - (1i128 << bits)
                } else {
                    u
                }
            }
            Fmt::Ch => 0,
        }
    }

    fn cmp(&self, a: &[u8], b: &[u8]) -> Ordering {
        match self.fmt {
            Fmt::Ch => self.bytes(a).cmp(self.bytes(b)),
            _ => self.number(a).cmp(&self.number(b)),
        }
    }
}

/// A condition operand: another field or a constant.
#[derive(Debug, Clone)]
enum Operand {
    Field(Field),
    Bytes(Vec<u8>),
    Number(i128),
}

#[derive(Debug, Clone)]
enum Cond {
    Cmp(Field, String, Operand),
    And(Box<Cond>, Box<Cond>),
    Or(Box<Cond>, Box<Cond>),
}

impl Cond {
    fn eval(&self, r: &[u8]) -> bool {
        match self {
            Cond::And(a, b) => a.eval(r) && b.eval(r),
            Cond::Or(a, b) => a.eval(r) || b.eval(r),
            Cond::Cmp(f, op, rhs) => {
                let o = match rhs {
                    Operand::Field(g) => match f.fmt {
                        Fmt::Ch => f.bytes(r).cmp(g.bytes(r)),
                        _ => f.number(r).cmp(&g.number(r)),
                    },
                    Operand::Bytes(c) => {
                        // the constant is padded with blanks / truncated
                        let mut c = c.clone();
                        c.resize(f.len, ebcdic::SPACE);
                        f.bytes(r).cmp(&c[..])
                    }
                    Operand::Number(n) => f.number(r).cmp(n),
                };
                match op.as_str() {
                    "EQ" => o == Ordering::Equal,
                    "NE" => o != Ordering::Equal,
                    "GT" => o == Ordering::Greater,
                    "GE" => o != Ordering::Less,
                    "LT" => o == Ordering::Less,
                    "LE" => o != Ordering::Greater,
                    _ => false,
                }
            }
        }
    }
}

/// OUTREC / INREC item.
#[derive(Debug, Clone)]
enum Item {
    Column(usize),
    Copy(usize, usize),
    Blanks(usize),
    Const(Vec<u8>),
    /// A numeric field edited with an `EDIT=` pattern.
    Edit(Field, String),
}

/// Edits a number with a DFSORT pattern: `I` is a leading digit shown as
/// a blank while insignificant, `T` a digit always shown; any other
/// character is shown once a digit has been shown to its left (or is
/// under a `T`), otherwise replaced by a blank. No sign is produced.
fn edit(v: i128, pattern: &str) -> Vec<u8> {
    let slots = pattern.chars().filter(|c| matches!(c, 'I' | 'T')).count();
    let digits = format!("{:0>slots$}", v.unsigned_abs());
    let digits = &digits.as_bytes()[digits.len() - slots..];
    let mut out = String::new();
    let mut d = 0;
    let mut significant = false;
    for c in pattern.chars() {
        match c {
            'I' | 'T' => {
                let x = digits[d] as char;
                d += 1;
                if c == 'T' || x != '0' {
                    significant = true;
                }
                out.push(if significant { x } else { ' ' });
            }
            _ => out.push(if significant { c } else { ' ' }),
        }
    }
    ebcdic::encode(&out)
}

#[derive(Debug, Default)]
struct Control {
    copy: bool,
    keys: Vec<(Field, bool)>,
    include: Option<(bool, Cond)>,
    inrec: Option<Vec<Item>>,
    outrec: Option<Vec<Item>>,
    sum_none: bool,
}

/// Splits a statement's operand list at top-level commas.
fn split_top(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut quoted = false;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '\'' => {
                quoted = !quoted;
                cur.push(c);
            }
            '(' if !quoted => {
                depth += 1;
                cur.push(c);
            }
            ')' if !quoted => {
                depth -= 1;
                cur.push(c);
            }
            ',' if !quoted && depth == 0 => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn strip_parens(s: &str) -> &str {
    s.strip_prefix('(')
        .and_then(|x| x.strip_suffix(')'))
        .unwrap_or(s)
}

/// Value of keyword `key` in a statement's operands.
fn keyword<'a>(ops: &'a [String], key: &str) -> Option<&'a str> {
    ops.iter().find_map(|o| {
        let (k, v) = o.split_once('=')?;
        k.trim().eq_ignore_ascii_case(key).then_some(v.trim())
    })
}

/// Character / hexadecimal / decimal constant.
fn constant(s: &str) -> Option<Operand> {
    if let Some(c) = s.strip_prefix("C'").and_then(|c| c.strip_suffix('\'')) {
        return Some(Operand::Bytes(ebcdic::encode(&c.replace("''", "'"))));
    }
    if let Some(h) = s.strip_prefix("X'").and_then(|c| c.strip_suffix('\'')) {
        let bytes: Option<Vec<u8>> = (0..h.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(h.get(i..i + 2)?, 16).ok())
            .collect();
        return bytes.map(Operand::Bytes);
    }
    s.trim_start_matches('+')
        .parse::<i128>()
        .ok()
        .map(Operand::Number)
}

struct Parser {
    symbols: HashMap<String, String>,
}

impl Parser {
    /// Replaces symbols in a list of operands.
    fn expand(&self, items: Vec<String>) -> Vec<String> {
        let mut out = Vec::new();
        for i in items {
            match self.symbols.get(i.trim()) {
                Some(v) => out.extend(split_top(v)),
                None => out.push(i.trim().to_string()),
            }
        }
        out
    }

    fn fields(&self, s: &str, default: Option<Fmt>) -> Result<Vec<(Field, bool)>, String> {
        let items = self.expand(split_top(strip_parens(s)));
        let mut out = Vec::new();
        let mut i = 0;
        while i < items.len() {
            let pos: usize = items[i]
                .parse()
                .map_err(|_| format!("bad field {}", items[i]))?;
            let len: usize = items
                .get(i + 1)
                .and_then(|l| l.parse().ok())
                .ok_or("bad length")?;
            i += 2;
            let fmt = match items.get(i).and_then(|f| Fmt::parse(f)) {
                Some(f) => {
                    i += 1;
                    f
                }
                None => default.unwrap_or(Fmt::Ch),
            };
            let asc = match items.get(i).map(String::as_str) {
                Some("A") => true,
                Some("D") => false,
                _ => return Err("A or D expected".into()),
            };
            i += 1;
            out.push((Field { pos, len, fmt }, asc));
        }
        Ok(out)
    }

    fn cond(&self, s: &str, default: Option<Fmt>) -> Result<Cond, String> {
        let items = self.expand(split_top(strip_parens(s)));
        let mut i = 0;
        let mut acc: Option<Cond> = None;
        let mut join: Option<String> = None;
        while i < items.len() {
            let field = |i: &mut usize| -> Result<Field, String> {
                let pos: usize = items[*i]
                    .parse()
                    .map_err(|_| format!("bad field {}", items[*i]))?;
                let len: usize = items
                    .get(*i + 1)
                    .and_then(|l| l.parse().ok())
                    .ok_or("bad length")?;
                *i += 2;
                let fmt = match items.get(*i).and_then(|f| Fmt::parse(f)) {
                    Some(f) => {
                        *i += 1;
                        f
                    }
                    None => default.unwrap_or(Fmt::Ch),
                };
                Ok(Field { pos, len, fmt })
            };
            let f = field(&mut i)?;
            let op = items.get(i).cloned().ok_or("operator expected")?;
            i += 1;
            let rhs_s = items.get(i).cloned().ok_or("operand expected")?;
            let rhs = if rhs_s.chars().next().is_some_and(|c| c.is_ascii_digit())
                && items.get(i + 1).is_some_and(|l| l.parse::<usize>().is_ok())
                && !matches!(
                    items.get(i + 2).map(String::as_str),
                    Some("AND" | "OR") | None
                ) {
                Operand::Field(field(&mut i)?)
            } else {
                i += 1;
                constant(&rhs_s).ok_or_else(|| format!("bad constant {rhs_s}"))?
            };
            let c = Cond::Cmp(f, op, rhs);
            acc = Some(match (acc, join.take().as_deref()) {
                (None, _) => c,
                (Some(a), Some("OR")) => Cond::Or(Box::new(a), Box::new(c)),
                (Some(a), _) => Cond::And(Box::new(a), Box::new(c)),
            });
            if let Some(j) = items.get(i) {
                join = Some(j.clone());
                i += 1;
            }
        }
        acc.ok_or_else(|| "empty condition".into())
    }

    fn items(&self, s: &str) -> Result<Vec<Item>, String> {
        let raw = self.expand(split_top(strip_parens(s)));
        let mut out = Vec::new();
        let mut i = 0;
        while i < raw.len() {
            let mut t = raw[i].clone();
            if let Some((c, rest)) = t.split_once(':')
                && let Ok(col) = c.parse::<usize>()
            {
                out.push(Item::Column(col));
                t = rest.to_string();
            }
            if t == "X" {
                out.push(Item::Blanks(1));
                i += 1;
            } else if let Some(n) = t.strip_suffix('X').and_then(|n| n.parse().ok()) {
                out.push(Item::Blanks(n));
                i += 1;
            } else if let Some(Operand::Bytes(b)) = constant(&t) {
                out.push(Item::Const(b));
                i += 1;
            } else {
                let p: usize = t.parse().map_err(|_| format!("bad item {t}"))?;
                let l: usize = raw
                    .get(i + 1)
                    .and_then(|l| l.parse().ok())
                    .ok_or("bad length")?;
                i += 2;
                let fmt = raw.get(i).and_then(|f| Fmt::parse(f));
                if fmt.is_some() {
                    i += 1;
                }
                match raw.get(i).and_then(|e| e.strip_prefix("EDIT=")) {
                    Some(pat) => {
                        let field = Field {
                            pos: p,
                            len: l,
                            fmt: fmt.unwrap_or(Fmt::Zd),
                        };
                        out.push(Item::Edit(field, strip_parens(pat).to_string()));
                        i += 1;
                    }
                    None => out.push(Item::Copy(p, l)),
                }
            }
        }
        Ok(out)
    }
}

/// Joins continued control statements (a line ending with a comma) and
/// removes comment lines.
fn statements(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for l in lines {
        let l: String = l.chars().take(71).collect();
        if l.starts_with('*') || l.trim().is_empty() {
            continue;
        }
        let t = l.trim();
        // operands end at the first blank outside quotes
        let mut field = String::new();
        let mut quoted = false;
        let mut seen_op = !cur.is_empty();
        let mut started = false;
        for c in t.chars() {
            if c == '\'' {
                quoted = !quoted;
            }
            if c == ' ' && !quoted {
                if !seen_op {
                    seen_op = true;
                    field.push(' ');
                    continue;
                }
                if started {
                    break;
                }
                continue;
            }
            if seen_op {
                started = true;
            }
            field.push(c);
        }
        cur.push_str(&field);
        if !cur.ends_with(',') {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn symnames(lines: &[String]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    for l in lines {
        if l.starts_with('*') {
            continue;
        }
        let t = l.trim_start();
        // the definition ends at the first blank outside quotes
        let mut def = String::new();
        let mut quoted = false;
        for c in t.chars() {
            if c == '\'' {
                quoted = !quoted;
            }
            if c == ' ' && !quoted {
                break;
            }
            def.push(c);
        }
        if let Some((name, value)) = def.split_once(',') {
            out.insert(name.to_string(), value.to_string());
        }
    }
    out
}

fn parse_control(stmts: &[String], symbols: HashMap<String, String>) -> Result<Control, String> {
    let p = Parser { symbols };
    let mut c = Control::default();
    for s in stmts {
        let (op, rest) = s.split_once(' ').unwrap_or((s.as_str(), ""));
        let ops = split_top(rest);
        let format = keyword(&ops, "FORMAT").and_then(Fmt::parse);
        match op.to_ascii_uppercase().as_str() {
            "SORT" | "MERGE" => {
                let f = keyword(&ops, "FIELDS").ok_or("FIELDS expected")?;
                if f.eq_ignore_ascii_case("COPY") {
                    c.copy = true;
                } else {
                    c.keys = p.fields(f, format)?;
                }
            }
            "INCLUDE" | "OMIT" => {
                let cond = keyword(&ops, "COND").ok_or("COND expected")?;
                c.include = Some((op.eq_ignore_ascii_case("INCLUDE"), p.cond(cond, format)?));
            }
            "OUTREC" | "INREC" => {
                let f = keyword(&ops, "FIELDS")
                    .or_else(|| keyword(&ops, "BUILD"))
                    .ok_or("FIELDS or BUILD expected")?;
                let items = p.items(f)?;
                if op.eq_ignore_ascii_case("INREC") {
                    c.inrec = Some(items);
                } else {
                    c.outrec = Some(items);
                }
            }
            "SUM" => {
                if keyword(&ops, "FIELDS").is_some_and(|f| f.eq_ignore_ascii_case("NONE")) {
                    c.sum_none = true;
                } else {
                    return Err("SUM with fields is not supported".into());
                }
            }
            "OPTION" => {
                if ops.iter().any(|o| o.eq_ignore_ascii_case("COPY")) {
                    c.copy = true;
                }
            }
            "END" | "RECORD" => {}
            other => return Err(format!("statement {other} not supported")),
        }
    }
    Ok(c)
}

fn reformat(items: &[Item], r: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    for it in items {
        match it {
            Item::Column(c) => {
                if out.len() < c - 1 {
                    out.resize(c - 1, ebcdic::SPACE);
                }
            }
            Item::Copy(p, l) => {
                let s = (p - 1).min(r.len());
                let e = (p - 1 + l).min(r.len());
                let mut f = r[s..e].to_vec();
                f.resize(*l, ebcdic::SPACE);
                out.extend(f);
            }
            Item::Blanks(n) => out.extend(std::iter::repeat_n(ebcdic::SPACE, *n)),
            Item::Const(b) => out.extend(b),
            Item::Edit(f, pat) => out.extend(edit(f.number(r), pat)),
        }
    }
    out
}

fn sort_records(c: &Control, mut recs: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    if let Some((include, cond)) = &c.include {
        recs.retain(|r| cond.eval(r) == *include);
    }
    if let Some(items) = &c.inrec {
        recs = recs.iter().map(|r| reformat(items, r)).collect();
    }
    if !c.copy {
        let cmp = |a: &Vec<u8>, b: &Vec<u8>| {
            for (f, asc) in &c.keys {
                let o = f.cmp(a, b);
                if o != Ordering::Equal {
                    return if *asc { o } else { o.reverse() };
                }
            }
            Ordering::Equal
        };
        recs.sort_by(cmp);
        if c.sum_none {
            recs.dedup_by(|b, a| cmp(a, b) == Ordering::Equal);
        }
    }
    if let Some(items) = &c.outrec {
        recs = recs.iter().map(|r| reformat(items, r)).collect();
    }
    recs
}

pub fn run(ctx: &mut StepCtx) -> ProgramResult {
    let mut out = ctx.sysout("SYSOUT").map_err(Outcome::JclError)?;
    let lines = ctx.read_text("SYSIN").unwrap_or_default();
    let syms = if ctx.dd("SYMNAMES").is_some() {
        symnames(&ctx.read_text("SYMNAMES").unwrap_or_default())
    } else {
        HashMap::new()
    };
    let stmts = statements(&lines);
    for s in &stmts {
        let _ = writeln!(out, " {s}");
    }
    let control = match parse_control(&stmts, syms) {
        Ok(c) => c,
        Err(e) => {
            let _ = writeln!(out, "ICE005A STATEMENT ERROR: {e}");
            return Ok(16);
        }
    };
    let mut recs = Vec::new();
    let inputs: Vec<String> = if ctx.dd("SORTIN").is_some() {
        vec!["SORTIN".into()]
    } else {
        (1..100)
            .map(|n| format!("SORTIN{n:02}"))
            .filter(|n| ctx.dd(n).is_some())
            .collect()
    };
    for i in &inputs {
        match ctx.read(i) {
            Ok(r) => recs.extend(r),
            Err(e) => {
                let _ = writeln!(out, "ICE083A {e}");
                return Ok(16);
            }
        }
    }
    let n_in = recs.len();
    let sorted = sort_records(&control, recs);
    let first_in = inputs.first().cloned().unwrap_or_default();
    ctx.inherit_attributes("SORTOUT", &first_in, &sorted);
    if let Err(e) = ctx.write("SORTOUT", &sorted, WriteMode::Replace) {
        let _ = writeln!(out, "ICE083A {e}");
        return Ok(16);
    }
    let _ = writeln!(out, "ICE054I RECORDS - IN: {n_in}, OUT: {}", sorted.len());
    let _ = writeln!(out, "ICE052I END OF SORT/MERGE");
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(s: &str) -> Vec<u8> {
        ebcdic::encode(s)
    }

    #[test]
    fn edit_patterns() {
        assert_eq!(edit(1234, "TTTTT.TT"), rec("00012.34"));
        assert_eq!(edit(-1234, "IIIIT.TT"), rec("   12.34"));
        assert_eq!(edit(1234567, "II,III,IIT.TT"), rec("    12,345.67"));
        assert_eq!(edit(5, "III,IIT"), rec("      5"));
        let p = Parser {
            symbols: symnames(&["BAL,3,4,ZD".to_string()]),
        };
        let items = p.items("(1,2,X,BAL,EDIT=(IIT.T))").unwrap();
        assert_eq!(reformat(&items, &rec("AB0123")), rec("AB  12.3"));
    }

    #[test]
    fn symnames_include_sort_outrec() {
        let syms = symnames(&[
            "KEY,1,3,ZD".to_string(),
            "DT,5,4,CH".to_string(),
            "FROM,C'2022'                       //comment".to_string(),
        ]);
        let stmts = statements(&[
            " SORT FIELDS=(KEY,D)".to_string(),
            " INCLUDE COND=(DT,GE,FROM,AND,".to_string(),
            "         DT,LE,C'2023')".to_string(),
            "  OUTREC FIELDS=(1:5,4,X,1,3,10:C'!')".to_string(),
        ]);
        assert_eq!(stmts.len(), 3);
        let c = parse_control(&stmts, syms).unwrap();
        let out = sort_records(
            &c,
            vec![
                rec("002 2022"),
                rec("010 2021"),
                rec("005 2023"),
                rec("001 2024"),
            ],
        );
        assert_eq!(out, vec![rec("2023 005 !"), rec("2022 002 !")]);
    }

    #[test]
    fn copy_and_sum_none() {
        let c = parse_control(
            &["SORT FIELDS=(1,1,CH,A)".into(), "SUM FIELDS=NONE".into()],
            HashMap::new(),
        )
        .unwrap();
        assert_eq!(
            sort_records(&c, vec![rec("B1"), rec("A1"), rec("A2")]),
            vec![rec("A1"), rec("B1")]
        );
        let c = parse_control(&["OPTION COPY".into()], HashMap::new()).unwrap();
        assert_eq!(
            sort_records(&c, vec![rec("B"), rec("A")]),
            vec![rec("B"), rec("A")]
        );
    }
}

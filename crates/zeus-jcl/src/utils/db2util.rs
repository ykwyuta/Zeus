//! Db2 utilities: DSNTIAUL (the sample unload program, run by TSO DSN RUN
//! PROGRAM) and the LOAD utility (DSNUTILB).
//!
//! DSNTIAUL writes the rows of each query (PARMS('SQL'): the statements
//! of SYSIN; else a table name, with an optional WHERE clause, per line)
//! to SYSREC00, SYSREC01, ... as fixed-length records of the columns in
//! their Db2 internal formats (a null indicator byte after a nullable
//! column: X'00', or '?' for NULL), and LOAD statements for them to
//! SYSPUNCH.
//!
//! LOAD reads INDDN (SYSREC) records: delimited (FORMAT DELIMITED COLDEL
//! CHARDEL DECPT) or positional (POSITION, CHAR, VARCHAR, INTEGER,
//! SMALLINT, BIGINT, DECIMAL PACKED / ZONED / EXTERNAL, FLOAT, DATE /
//! TIME / TIMESTAMP EXTERNAL, NULLIF), with REPLACE or RESUME, and
//! inserts the rows into the table.

use std::fmt::Write as _;
use std::io::Write;

use zeus_cobol::ebcdic;
use zeus_db2::sql::{Column, SqlCa, Value};
use zeus_decimal::{Dec, Rounding};
use zeus_ds::catalog::{Recfm, WriteMode};

use crate::job::{DdKind, Outcome, ProgramResult, StepCtx};

/// The null indicator byte of a NULL value ('?').
const NULL_IND: u8 = 0x6F;

/// The internal length of a column in an unloaded record.
fn field_len(c: &Column) -> usize {
    match c.drda_type {
        0x02 => 4,
        0x04 => 2,
        0x16 => 8,
        0x0E => usize::from(c.len >> 8) / 2 + 1,
        0x0A => 8,
        0x0C => 4,
        0x32 | 0x3E | 0x28 | 0x34 | 0x40 => 2 + usize::from(c.len),
        _ => usize::from(c.len),
    }
}

/// A value in a column's internal format.
fn field(c: &Column, v: &Value) -> Vec<u8> {
    let n = field_len(c);
    let mut b = vec![0u8; n];
    let text = |s: &str, out: &mut Vec<u8>, len: usize| {
        let mut e = ebcdic::encode(s);
        e.truncate(len);
        e.resize(len, ebcdic::SPACE);
        out.copy_from_slice(&e);
    };
    let num = || -> Option<Dec> {
        match v {
            Value::Int(i) => Some(Dec::from_i64(*i)),
            Value::Dec(s) | Value::Str(s) => Dec::parse(s.trim()),
            Value::Float(f) => Dec::parse(&format!("{f}")),
            _ => None,
        }
    };
    match (c.drda_type, v) {
        (_, Value::Null) => {}
        (0x02 | 0x04 | 0x16, _) => {
            let i = num()
                .and_then(|d| d.rescale(0, Rounding::Truncate))
                .map_or(0, |d| d.coef);
            let be = (i as i64).to_be_bytes();
            b.copy_from_slice(&be[8 - n..]);
        }
        (0x0E, _) => {
            let scale = u32::from(c.len & 0xFF);
            let d = num()
                .and_then(|d| d.rescale(scale, Rounding::Truncate))
                .unwrap_or(Dec::new(0, scale));
            zeus_decimal::packed::encode(
                &mut b,
                d.coef.unsigned_abs(),
                if d.coef < 0 { 0x0D } else { 0x0C },
            );
        }
        (0x0A | 0x0C, _) => {
            let f = match v {
                Value::Float(f) => *f,
                _ => num().map_or(0.0, Dec::to_f64),
            };
            zeus_decimal::hfp::encode(f, &mut b);
        }
        (0x32 | 0x3E | 0x28 | 0x34 | 0x40, _) => {
            let data = match v {
                Value::Bytes(x) => x.clone(),
                Value::Str(s) | Value::Dec(s) => ebcdic::encode(s),
                other => ebcdic::encode(&scalar_text(other)),
            };
            let k = data.len().min(n - 2);
            b[..2].copy_from_slice(&(k as u16).to_be_bytes());
            b[2..2 + k].copy_from_slice(&data[..k]);
        }
        (0x26, Value::Bytes(x)) => {
            let k = x.len().min(n);
            b[..k].copy_from_slice(&x[..k]);
        }
        (_, other) => text(&scalar_text(other), &mut b, n),
    }
    b
}

fn scalar_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Int(i) => i.to_string(),
        Value::Dec(s) | Value::Str(s) => s.clone(),
        Value::Float(f) => format!("{f:E}"),
        Value::Bytes(b) => b.iter().map(|x| format!("{x:02X}")).collect(),
    }
}

/// The LOAD field type of a column (DSNTIAUL's SYSPUNCH).
fn load_type(c: &Column) -> String {
    match c.drda_type {
        0x02 => "INTEGER".into(),
        0x04 => "SMALLINT".into(),
        0x16 => "BIGINT".into(),
        0x0E => format!("DECIMAL({},{})", c.len >> 8, c.len & 0xFF),
        0x0A | 0x0C => "FLOAT".into(),
        0x20 => "DATE EXTERNAL".into(),
        0x22 => "TIME EXTERNAL".into(),
        0x24 => "TIMESTAMP EXTERNAL".into(),
        0x32 | 0x3E | 0x28 | 0x34 | 0x40 => "VARCHAR".into(),
        _ => format!("CHAR({})", c.len),
    }
}

fn db2_or_message(ctx: &StepCtx, program: &str) -> Result<zeus_db2::Db2Host, ProgramResult> {
    match zeus_db2::Db2Host::from_env() {
        Some(h) => Ok(h),
        None => {
            if let Ok(mut out) = ctx.sysout("SYSPRINT") {
                let _ = writeln!(out, "{program}: no Db2 server configured (ZEUS_DB2)");
            }
            Err(Ok(12))
        }
    }
}

/// The SYSIN statements of a utility as one text (columns 1-72).
fn sysin(ctx: &StepCtx) -> String {
    ctx.read_text("SYSIN")
        .unwrap_or_default()
        .iter()
        .map(|l| {
            let l: String = l.chars().take(72).collect();
            format!("{l}\n")
        })
        .collect()
}

/// DSNTIAUL: unloads the rows of the queries of SYSIN.
pub fn dsntiaul(ctx: &mut StepCtx, parms: &str) -> ProgramResult {
    let mut out = ctx
        .sysout("SYSPRINT")
        .map_err(|e| Outcome::JclError(e.to_string()))?;
    let db2 = match db2_or_message(ctx, "DSNTIAUL") {
        Ok(h) => h,
        Err(r) => return r,
    };
    let _ = writeln!(out, "DSNT490I SAMPLE DATA UNLOAD PROGRAM");
    let text = sysin(ctx);
    let sql_mode = parms.to_ascii_uppercase().contains("SQL");
    let statements: Vec<String> = if sql_mode {
        zeus_db2::dialect::split_script(&text)
    } else {
        text.lines()
            .map(|l| l.trim().trim_end_matches(';').trim().to_string())
            .filter(|l| !l.is_empty())
            .map(|l| format!("SELECT * FROM {l}"))
            .collect()
    };
    let mut punch = Vec::new();
    let mut rc = 0;
    let mut k = 0;
    for stmt in statements {
        let first = stmt
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        if !matches!(first.as_str(), "SELECT" | "WITH" | "VALUES") {
            // other statements (LOCK TABLE, SET ...) are run
            let ca = db2.execute_sql(&stmt);
            if ca.sqlcode < 0 {
                let _ = writeln!(out, "DSNT408I SQLCODE = {}, {stmt}", ca.sqlcode);
                rc = rc.max(8);
            }
            continue;
        }
        let dd = format!("SYSREC{k:02}");
        k += 1;
        let rows = match db2.select(&stmt) {
            Ok(r) => r,
            Err(ca) => {
                let _ = writeln!(
                    out,
                    "DSNT408I SQLCODE = {}, SQLSTATE = {}: {stmt}",
                    ca.sqlcode, ca.sqlstate
                );
                rc = rc.max(8);
                break;
            }
        };
        let mut recs = Vec::with_capacity(rows.rows.len());
        for row in &rows.rows {
            let mut rec = Vec::new();
            for (c, v) in rows.columns.iter().zip(row) {
                rec.extend(field(c, v));
                if c.nullable {
                    rec.push(if matches!(v, Value::Null) {
                        NULL_IND
                    } else {
                        0
                    });
                }
            }
            recs.push(rec);
        }
        let lrecl: usize = rows
            .columns
            .iter()
            .map(|c| field_len(c) + usize::from(c.nullable))
            .sum();
        if ctx.dd(&dd).is_none() {
            let _ = writeln!(out, "DSNT496I DD {dd} IS MISSING");
            rc = rc.max(8);
            break;
        }
        if let Some(DdKind::Dataset { dsn, member: None }) = ctx.dd(&dd).map(|d| d.kind.clone()) {
            ctx.set_attributes(&dsn, Recfm::Fb, lrecl.max(1) as u32);
        }
        if let Err(e) = ctx.write(&dd, &recs, WriteMode::Replace) {
            let _ = writeln!(out, "DSNT496I {dd}: {e}");
            rc = rc.max(8);
            break;
        }
        // the LOAD statement for the records
        let table = from_table(&stmt).unwrap_or_else(|| "TABLE".into());
        let names = column_names(&db2, &stmt, &table, rows.columns.len());
        let mut pos = 1;
        let mut fields = Vec::new();
        for (n, c) in rows.columns.iter().enumerate() {
            let name = if !c.name.is_empty() {
                c.name.clone()
            } else {
                names
                    .get(n)
                    .cloned()
                    .flatten()
                    .unwrap_or_else(|| format!("COL{}", n + 1))
            };
            let mut f = format!("   \"{name}\" POSITION( {pos} ) {}", load_type(c));
            pos += field_len(c);
            if c.nullable {
                let _ = write!(f, " NULLIF({pos})='?'");
                pos += 1;
            }
            fields.push(f);
        }
        punch.push(format!("  LOAD DATA INDDN {dd} LOG NO RESUME YES"));
        punch.push(format!("   EBCDIC INTO TABLE {table}"));
        for (n, f) in fields.iter().enumerate() {
            let sep = if n == 0 { "  (" } else { "  ," };
            punch.push(format!("{sep}{}", f.trim_start()));
        }
        punch.push("  )".into());
        let _ = writeln!(
            out,
            "DSNT495I SUCCESSFUL UNLOAD {:>9} ROWS OF TABLE {table}",
            recs.len()
        );
    }
    if ctx.dd("SYSPUNCH").is_some() && !punch.is_empty() {
        let recs: Vec<Vec<u8>> = punch
            .iter()
            .map(|l| {
                let mut r = ebcdic::encode(l);
                r.resize(80, ebcdic::SPACE);
                r
            })
            .collect();
        let _ = ctx.write("SYSPUNCH", &recs, WriteMode::Replace);
    }
    let _ = db2.end_uow(rc == 0);
    Ok(rc)
}

/// The names of the result columns when the server does not describe
/// them: the select list's names (AS name, or a column), or for SELECT *
/// the table's columns in the catalog.
fn column_names(db2: &zeus_db2::Db2Host, stmt: &str, table: &str, n: usize) -> Vec<Option<String>> {
    let up = stmt.to_ascii_uppercase();
    let (Some(s), Some(f)) = (up.find("SELECT"), up.find(" FROM ")) else {
        return vec![None; n];
    };
    let list = stmt[s + 6..f].trim();
    if list == "*" {
        let (schema, name) = table.rsplit_once('.').unwrap_or(("", table));
        let q = format!(
            "SELECT COLNAME FROM SYSCAT.COLUMNS WHERE TABSCHEMA = '{}' AND TABNAME = '{}' ORDER BY COLNO",
            schema.trim_matches('"').to_ascii_uppercase(),
            name.trim_matches('"').to_ascii_uppercase()
        );
        return match db2.select(&q) {
            Ok(r) => r
                .rows
                .iter()
                .map(|row| match row.first() {
                    Some(Value::Str(s)) => Some(s.trim().to_string()),
                    _ => None,
                })
                .collect(),
            Err(_) => vec![None; n],
        };
    }
    // the items of the list (commas outside parentheses)
    let mut items = Vec::new();
    let (mut depth, mut cur) = (0, String::new());
    for ch in list.chars() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                items.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    items.push(cur);
    items
        .iter()
        .map(|it| {
            let w: Vec<&str> = it.split_whitespace().collect();
            let ident = |x: &str| {
                x.chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '"' | '#' | '@' | '$'))
            };
            match w.as_slice() {
                [.., a, name] if a.eq_ignore_ascii_case("AS") => Some(name.to_string()),
                [col] if ident(col) => Some(col.rsplit('.').next().unwrap_or(col).to_string()),
                _ => None,
            }
        })
        .collect()
}

/// The table of a simple query (FROM name).
fn from_table(stmt: &str) -> Option<String> {
    let w: Vec<&str> = stmt.split_whitespace().collect();
    let at = w.iter().position(|x| x.eq_ignore_ascii_case("FROM"))?;
    Some(w.get(at + 1)?.trim_end_matches(';').to_string())
}

// ---- LOAD ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Word(String),
    Str(String),
    Hex(Vec<u8>),
    P(char),
}

fn tokens(s: &str) -> Vec<Tok> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch == '\'' || ch == '"' {
            let mut t = String::new();
            i += 1;
            while i < c.len() {
                if c[i] == ch {
                    if c.get(i + 1) == Some(&ch) {
                        t.push(ch);
                        i += 2;
                        continue;
                    }
                    break;
                }
                t.push(c[i]);
                i += 1;
            }
            i += 1;
            out.push(Tok::Str(t));
        } else if (ch == 'X' || ch == 'x') && c.get(i + 1) == Some(&'\'') {
            let end = c[i + 2..]
                .iter()
                .position(|&x| x == '\'')
                .map_or(c.len(), |p| i + 2 + p);
            let h: String = c[i + 2..end].iter().collect();
            let bytes = (0..h.len() / 2)
                .filter_map(|k| u8::from_str_radix(&h[2 * k..2 * k + 2], 16).ok())
                .collect();
            out.push(Tok::Hex(bytes));
            i = end + 1;
        } else if ch.is_alphanumeric() || matches!(ch, '_' | '#' | '@' | '$' | '.' | '-') {
            let st = i;
            while i < c.len()
                && (c[i].is_alphanumeric() || matches!(c[i], '_' | '#' | '@' | '$' | '.' | '-'))
            {
                i += 1;
            }
            out.push(Tok::Word(c[st..i].iter().collect::<String>()));
        } else {
            out.push(Tok::P(ch));
            i += 1;
        }
    }
    out
}

/// How a field is stored in the input record.
#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Char,
    Varchar,
    Int(usize),
    Packed(u32, u32),
    Zoned(u32, u32),
    /// Numeric or datetime characters.
    External,
    Float(usize),
}

#[derive(Debug, Clone)]
struct Field {
    name: String,
    pos: Option<(usize, Option<usize>)>,
    kind: Kind,
    len: Option<usize>,
    /// The SQL literal is a string (character and datetime columns).
    quoted: bool,
    nullif: Option<(usize, Vec<u8>)>,
}

#[derive(Debug, Default)]
struct LoadSpec {
    indd: String,
    replace: bool,
    delimited: bool,
    coldel: char,
    chardel: char,
    decpt: char,
    ascii: bool,
    table: String,
    fields: Vec<Field>,
}

fn word(t: Option<&Tok>) -> String {
    match t {
        Some(Tok::Word(w)) => w.to_ascii_uppercase(),
        _ => String::new(),
    }
}

fn parse_load(text: &str) -> Result<LoadSpec, String> {
    let t = tokens(text);
    let mut s = LoadSpec {
        indd: "SYSREC".into(),
        coldel: ',',
        chardel: '"',
        decpt: '.',
        ..LoadSpec::default()
    };
    let mut i = 0;
    let delim = |t: Option<&Tok>, d: char| match t {
        Some(Tok::Str(x)) => x.chars().next().unwrap_or(d),
        Some(Tok::Hex(b)) => ebcdic::decode(b).chars().next().unwrap_or(d),
        _ => d,
    };
    if word(t.first()) != "LOAD" {
        return Err("LOAD statement expected".into());
    }
    while i < t.len() {
        match word(t.get(i)).as_str() {
            "INDDN" => {
                s.indd = word(t.get(i + 1));
                i += 2;
            }
            "REPLACE" => {
                s.replace = true;
                i += 1;
            }
            "RESUME" => {
                s.replace = false;
                i += 2;
            }
            "DELIMITED" => {
                s.delimited = true;
                i += 1;
            }
            "COLDEL" => {
                s.coldel = delim(t.get(i + 1), ',');
                i += 2;
            }
            "CHARDEL" => {
                s.chardel = delim(t.get(i + 1), '"');
                i += 2;
            }
            "DECPT" => {
                s.decpt = delim(t.get(i + 1), '.');
                i += 2;
            }
            "ASCII" | "UNICODE" => {
                s.ascii = true;
                i += 1;
            }
            "INTO" if word(t.get(i + 1)) == "TABLE" => {
                // the table name: words and dots
                i += 2;
                let mut name = String::new();
                while let Some(tok) = t.get(i) {
                    match tok {
                        Tok::Word(w) if !w.eq_ignore_ascii_case("WHEN") => name.push_str(w),
                        Tok::Str(w) => name.push_str(w),
                        Tok::P('.') => name.push('.'),
                        _ => break,
                    }
                    i += 1;
                }
                s.table = name;
                // the field list
                while i < t.len() && t[i] != Tok::P('(') {
                    i += 1;
                }
                i += 1;
                let mut depth = 0;
                let mut cur: Vec<Tok> = Vec::new();
                while i < t.len() {
                    match &t[i] {
                        Tok::P('(') => {
                            depth += 1;
                            cur.push(t[i].clone());
                        }
                        Tok::P(')') if depth == 0 => break,
                        Tok::P(')') => {
                            depth -= 1;
                            cur.push(t[i].clone());
                        }
                        Tok::P(',') if depth == 0 => {
                            s.fields.push(parse_field(&std::mem::take(&mut cur))?);
                        }
                        x => cur.push(x.clone()),
                    }
                    i += 1;
                }
                if !cur.is_empty() {
                    s.fields.push(parse_field(&cur)?);
                }
                break;
            }
            _ => i += 1,
        }
    }
    if s.table.is_empty() {
        return Err("INTO TABLE expected".into());
    }
    Ok(s)
}

fn parse_field(t: &[Tok]) -> Result<Field, String> {
    let name = match t.first() {
        Some(Tok::Word(w)) | Some(Tok::Str(w)) => w.clone(),
        _ => return Err("field name expected".into()),
    };
    let mut f = Field {
        name,
        pos: None,
        kind: Kind::Char,
        len: None,
        quoted: true,
        nullif: None,
    };
    // numbers inside parentheses after position i
    let args = |i: usize| -> Vec<usize> {
        let mut v = Vec::new();
        if t.get(i) != Some(&Tok::P('(')) {
            return v;
        }
        for x in &t[i + 1..] {
            match x {
                Tok::Word(w) => {
                    for part in w.split(':') {
                        if let Ok(n) = part.parse() {
                            v.push(n);
                        }
                    }
                }
                Tok::P(')') => break,
                _ => {}
            }
        }
        v
    };
    let mut i = 1;
    while i < t.len() {
        let w = word(t.get(i));
        match w.as_str() {
            "POSITION" => {
                let a = args(i + 1);
                f.pos = a.first().map(|&s| (s, a.get(1).copied()));
            }
            "CHAR" | "CHARACTER" | "GRAPHIC" => {
                f.kind = Kind::Char;
                f.len = args(i + 1).first().copied();
            }
            "VARCHAR" | "VARGRAPHIC" => f.kind = Kind::Varchar,
            "INTEGER" | "INT" => {
                f.kind = Kind::Int(4);
                f.quoted = false;
            }
            "SMALLINT" => {
                f.kind = Kind::Int(2);
                f.quoted = false;
            }
            "BIGINT" => {
                f.kind = Kind::Int(8);
                f.quoted = false;
            }
            "DECIMAL" | "DEC" => {
                f.quoted = false;
                let a = args(i + 1);
                let (p, q) = (
                    a.first().copied().unwrap_or(5) as u32,
                    a.get(1).copied().unwrap_or(0) as u32,
                );
                f.kind = Kind::Packed(p, q);
                if word(t.get(i + 1)) == "ZONED" {
                    let a = args(i + 2);
                    let (p, q) = (
                        a.first().copied().unwrap_or(5) as u32,
                        a.get(1).copied().unwrap_or(0) as u32,
                    );
                    f.kind = Kind::Zoned(p, q);
                } else if word(t.get(i + 1)) == "PACKED" {
                    let a = args(i + 2);
                    let (p, q) = (
                        a.first().copied().unwrap_or(5) as u32,
                        a.get(1).copied().unwrap_or(0) as u32,
                    );
                    f.kind = Kind::Packed(p, q);
                }
            }
            "FLOAT" | "REAL" | "DOUBLE" => {
                f.quoted = false;
                let n = args(i + 1).first().copied().unwrap_or(53);
                f.kind = Kind::Float(if n <= 21 { 4 } else { 8 });
            }
            "DATE" | "TIME" | "TIMESTAMP" => {
                f.quoted = true;
                f.kind = Kind::External;
                f.len = Some(match w.as_str() {
                    "DATE" => 10,
                    "TIME" => 8,
                    _ => 26,
                });
            }
            "EXTERNAL" => {
                f.kind = Kind::External;
                if let Some(&n) = args(i + 1).first() {
                    f.len = Some(n);
                }
            }
            "NULLIF" => {
                let a = args(i + 1);
                let close = t[i..]
                    .iter()
                    .position(|x| *x == Tok::P(')'))
                    .map_or(t.len(), |p| i + p);
                let value = match (t.get(close + 1), t.get(close + 2)) {
                    (Some(Tok::P('=')), Some(Tok::Str(s))) => ebcdic::encode(s),
                    (Some(Tok::P('=')), Some(Tok::Hex(b))) => b.clone(),
                    _ => Vec::new(),
                };
                if let Some(&p) = a.first() {
                    f.nullif = Some((p, value));
                }
            }
            _ => {}
        }
        i += 1;
    }
    Ok(f)
}

/// The values of a delimited record.
fn delimited(line: &str, s: &LoadSpec) -> Vec<Option<String>> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut was_quoted = false;
    let c: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if quoted {
            if ch == s.chardel {
                if c.get(i + 1) == Some(&s.chardel) {
                    cur.push(ch);
                    i += 1;
                } else {
                    quoted = false;
                }
            } else {
                cur.push(ch);
            }
        } else if ch == s.chardel {
            quoted = true;
            was_quoted = true;
        } else if ch == s.coldel {
            out.push(if cur.is_empty() && !was_quoted {
                None
            } else {
                Some(std::mem::take(&mut cur))
            });
            cur.clear();
            was_quoted = false;
        } else {
            cur.push(ch);
        }
        i += 1;
    }
    out.push(if cur.is_empty() && !was_quoted {
        None
    } else {
        Some(cur)
    });
    out
}

/// The SQL literal of a field's text.
fn literal(f: &Field, v: Option<String>, decpt: char) -> Result<String, String> {
    let Some(v) = v else {
        return Ok("NULL".into());
    };
    if f.quoted {
        return Ok(format!("'{}'", v.replace('\'', "''")));
    }
    let t = v.trim().replace(decpt, ".");
    if t.is_empty() {
        return Ok("NULL".into());
    }
    let ok = t
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '-' | '+' | 'E' | 'e'));
    if !ok {
        return Err(format!("{}: invalid number {v}", f.name));
    }
    Ok(t)
}

/// A field of a positional record as an SQL literal.
fn positional(f: &Field, rec: &[u8], at: usize) -> Result<(String, usize), String> {
    let start = f.pos.map_or(at, |(s, _)| s.saturating_sub(1));
    let len = match (f.pos.and_then(|p| p.1), &f.kind) {
        (Some(end), _) => end.saturating_sub(start),
        (None, Kind::Int(n)) => *n,
        (None, Kind::Packed(p, _)) => *p as usize / 2 + 1,
        (None, Kind::Zoned(p, _)) => *p as usize,
        (None, Kind::Float(n)) => *n,
        (None, Kind::Varchar) => {
            let l = rec
                .get(start..start + 2)
                .map_or(0, |b| u16::from_be_bytes([b[0], b[1]]) as usize);
            2 + l
        }
        (None, _) => f.len.unwrap_or(1),
    };
    let end = start + len;
    let next = end;
    if let Some((p, v)) = &f.nullif {
        let p = p.saturating_sub(1);
        if !v.is_empty() && rec.get(p..p + v.len()) == Some(v.as_slice()) {
            return Ok(("NULL".into(), next));
        }
    }
    let b = rec.get(start..end.min(rec.len())).unwrap_or(&[]);
    let lit = match &f.kind {
        Kind::Char => format!("'{}'", ebcdic::decode(b).trim_end().replace('\'', "''")),
        Kind::Varchar => {
            let d = b.get(2..).unwrap_or(&[]);
            format!("'{}'", ebcdic::decode(d).replace('\'', "''"))
        }
        Kind::External if f.quoted => format!("'{}'", ebcdic::decode(b).trim().replace('\'', "''")),
        Kind::External => literal(f, Some(ebcdic::decode(b)), '.')?,
        Kind::Int(_) => {
            let mut v: i64 = if b.first().is_some_and(|&x| x & 0x80 != 0) {
                -1
            } else {
                0
            };
            for &x in b {
                v = (v << 8) | i64::from(x);
            }
            v.to_string()
        }
        Kind::Packed(_, q) => {
            let (m, neg) = zeus_decimal::packed::decode_unchecked(b);
            Dec::new(if neg { -(m as i128) } else { m as i128 }, *q).to_string()
        }
        Kind::Zoned(_, q) => {
            let (m, neg) = zeus_decimal::zoned::decode_unchecked(
                b,
                zeus_decimal::zoned::SignPosition::TrailingOverpunch,
            );
            Dec::new(if neg { -(m as i128) } else { m as i128 }, *q).to_string()
        }
        Kind::Float(_) => format!("{:E}", zeus_decimal::hfp::decode(b)),
    };
    Ok((lit, next))
}

/// DSNUTILB: the LOAD utility.
pub fn dsnutilb(ctx: &mut StepCtx) -> ProgramResult {
    let mut out = ctx
        .sysout("SYSPRINT")
        .map_err(|e| Outcome::JclError(e.to_string()))?;
    let text = sysin(ctx);
    let _ = writeln!(out, "DSNU000I DSNUGUTC - OUTPUT START FOR UTILITY");
    let _ = writeln!(
        out,
        "DSNU050I DSNUGUTC - {}",
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    );
    let spec = match parse_load(&text) {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(out, "DSNU082I DSNUGUTC - {e}");
            return Ok(8);
        }
    };
    let db2 = match db2_or_message(ctx, "DSNUTILB") {
        Ok(h) => h,
        Err(r) => return r,
    };
    let recs = match ctx.read(&spec.indd) {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(out, "DSNU036I DSNUGBAC - {e}");
            return Ok(8);
        }
    };
    let cols: Vec<String> = spec.fields.iter().map(|f| f.name.clone()).collect();
    let mut rows = Vec::new();
    let mut rejected = 0;
    for (n, rec) in recs.iter().enumerate() {
        let values: Result<Vec<String>, String> = if spec.delimited {
            let line = if spec.ascii {
                String::from_utf8_lossy(rec).into_owned()
            } else {
                ebcdic::decode(rec)
            };
            let vals = delimited(line.trim_end(), &spec);
            spec.fields
                .iter()
                .enumerate()
                .map(|(k, f)| literal(f, vals.get(k).cloned().flatten(), spec.decpt))
                .collect()
        } else {
            let mut at = 0;
            let mut v = Vec::new();
            let mut err = None;
            for f in &spec.fields {
                match positional(f, rec, at) {
                    Ok((lit, next)) => {
                        v.push(lit);
                        at = next;
                    }
                    Err(e) => {
                        err = Some(e);
                        break;
                    }
                }
            }
            match err {
                Some(e) => Err(e),
                None => Ok(v),
            }
        };
        match values {
            Ok(v) => rows.push(v),
            Err(e) => {
                let _ = writeln!(out, "DSNU334I INPUT RECORD {} REJECTED: {e}", n + 1);
                rejected += 1;
            }
        }
    }
    let mut rc = 0;
    if spec.replace {
        let ca = db2.execute_sql(&format!("DELETE FROM {}", spec.table));
        if ca.sqlcode < 0 {
            let _ = writeln!(
                out,
                "DSNU056I DELETE FROM {}: SQLCODE {}",
                spec.table, ca.sqlcode
            );
            let _ = db2.end_uow(false);
            return Ok(8);
        }
    }
    let head = format!("INSERT INTO {} ({}) VALUES ", spec.table, cols.join(", "));
    let mut loaded = 0usize;
    for chunk in rows.chunks(100) {
        let values: Vec<String> = chunk
            .iter()
            .map(|r| format!("({})", r.join(", ")))
            .collect();
        let ca = db2.execute_sql(&format!("{head}{}", values.join(", ")));
        if ca.sqlcode >= 0 {
            loaded += chunk.len();
            continue;
        }
        // row by row, to find the rows in error
        for v in values {
            let ca: SqlCa = db2.execute_sql(&format!("{head}{v}"));
            if ca.sqlcode < 0 {
                let _ = writeln!(out, "DSNU334I ROW {v} REJECTED: SQLCODE {}", ca.sqlcode);
                rejected += 1;
            } else {
                loaded += 1;
            }
        }
    }
    if rejected > 0 {
        rc = 4;
    }
    let _ = db2.end_uow(true);
    let _ = writeln!(
        out,
        "DSNU304I DSNURWT - (RE)LOAD PHASE STATISTICS - NUMBER OF RECORDS={loaded} FOR TABLE {}",
        spec.table
    );
    if rejected > 0 {
        let _ = writeln!(out, "DSNU399I DSNURWT - {rejected} RECORDS REJECTED");
    }
    let _ = writeln!(
        out,
        "DSNU010I DSNUGBAC - UTILITY EXECUTION COMPLETE, HIGHEST RETURN CODE={rc}"
    );
    Ok(rc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_statements() {
        let s = parse_load(
            "  LOAD DATA REPLACE\n  FORMAT DELIMITED COLDEL ',' CHARDEL '\"' DECPT '.'\n  INTO TABLE BANK.TXN\n  (TXID BIGINT,\n   TIMESTMP TIMESTAMP EXTERNAL,\n   TRANSTYP CHAR,\n   AMOUNT DECIMAL)\n",
        )
        .unwrap();
        assert!(s.replace && s.delimited);
        assert_eq!(s.table, "BANK.TXN");
        assert_eq!(s.fields.len(), 4);
        assert!(!s.fields[0].quoted && s.fields[1].quoted && !s.fields[3].quoted);
        let v = delimited("1,\"2003-07-23 17:56:00.64\",\"d\",8830.00,,\"a\"\"b\"", &s);
        assert_eq!(v[1].as_deref(), Some("2003-07-23 17:56:00.64"));
        assert_eq!(v[4], None);
        assert_eq!(v[5].as_deref(), Some("a\"b"));
        let p = parse_load(
            "LOAD DATA INDDN SYSREC00 RESUME YES INTO TABLE T\n (\"A\" POSITION( 1 ) CHAR(2), \"B\" POSITION( 3 ) DECIMAL(5,2) NULLIF(6)='?')",
        )
        .unwrap();
        assert_eq!(p.indd, "SYSREC00");
        assert_eq!(p.fields[1].pos, Some((3, None)));
        assert_eq!(p.fields[1].nullif.as_ref().map(|n| n.0), Some(6));
        let mut rec = ebcdic::encode("AB");
        rec.extend([0x12, 0x34, 0x5C, 0x00]);
        assert_eq!(positional(&p.fields[0], &rec, 0).unwrap().0, "'AB'");
        assert_eq!(positional(&p.fields[1], &rec, 2).unwrap().0, "123.45");
        rec[5] = NULL_IND;
        assert_eq!(positional(&p.fields[1], &rec, 2).unwrap().0, "NULL");
    }
}

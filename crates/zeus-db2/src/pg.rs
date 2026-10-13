//! Db2 SQL on PostgreSQL: the statements in PostgreSQL's dialect, the
//! results in Db2's forms (timestamps, times) and the SQLCODE of
//! PostgreSQL's SQLSTATE.
//!
//! A session runs each statement in a transaction block (Db2's unit of
//! work starts with the first statement) behind a savepoint, so that a
//! failing statement does not end the unit of work, as on Db2.

use zeus_drda::sql::{Rows, SqlCa, Value};
use zeus_pg::{Connection, Error, Params, ServerError, Statement, oid};

// ---- tokens ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Tok {
    /// A word: keyword, name, number (with its dots: `A.B`, `1.5`).
    Word(String),
    /// A string literal with its quotes.
    Str(String),
    /// A quoted name with its quotes.
    Quoted(String),
    Space(String),
    Punct(char),
}

fn tokens(sql: &str) -> Vec<Tok> {
    let c: Vec<char> = sql.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '\'' || ch == '"' {
            let mut j = i + 1;
            while j < c.len() {
                if c[j] == ch {
                    if c.get(j + 1) == Some(&ch) {
                        j += 2;
                        continue;
                    }
                    break;
                }
                j += 1;
            }
            let t: String = c[i..(j + 1).min(c.len())].iter().collect();
            out.push(if ch == '\'' {
                Tok::Str(t)
            } else {
                Tok::Quoted(t)
            });
            i = j + 1;
        } else if ch.is_whitespace() {
            let mut j = i;
            while j < c.len() && c[j].is_whitespace() {
                j += 1;
            }
            out.push(Tok::Space(c[i..j].iter().collect()));
            i = j;
        } else if ch.is_alphanumeric() || matches!(ch, '_' | '#' | '@' | '$') {
            let mut j = i;
            while j < c.len()
                && (c[j].is_alphanumeric() || matches!(c[j], '_' | '#' | '@' | '$' | '.'))
            {
                j += 1;
            }
            out.push(Tok::Word(c[i..j].iter().collect()));
            i = j;
        } else {
            out.push(Tok::Punct(ch));
            i += 1;
        }
    }
    out
}

fn text(t: &[Tok]) -> String {
    t.iter()
        .map(|t| match t {
            Tok::Word(s) | Tok::Str(s) | Tok::Quoted(s) | Tok::Space(s) => s.clone(),
            Tok::Punct(c) => c.to_string(),
        })
        .collect()
}

fn upper(t: &Tok) -> String {
    match t {
        Tok::Word(s) => s.to_ascii_uppercase(),
        _ => String::new(),
    }
}

/// The index of the next token that is not a space, from `i`.
fn next(t: &[Tok], i: usize) -> Option<usize> {
    (i..t.len()).find(|&k| !matches!(t[k], Tok::Space(_)))
}

/// Whether the words from `i` are `kws` (spaces between); the index past
/// them.
fn words_at(t: &[Tok], i: usize, kws: &[&str]) -> Option<usize> {
    let mut k = i;
    for (n, kw) in kws.iter().enumerate() {
        let at = if n == 0 { Some(k) } else { next(t, k) }?;
        if upper(t.get(at)?) != *kw {
            return None;
        }
        k = at + 1;
    }
    Some(k)
}

/// The index of the `)` closing the `(` at `open`.
fn close_paren(t: &[Tok], open: usize) -> Option<usize> {
    let mut depth = 0;
    for (k, x) in t.iter().enumerate().skip(open) {
        match x {
            Tok::Punct('(') => depth += 1,
            Tok::Punct(')') => {
                depth -= 1;
                if depth == 0 {
                    return Some(k);
                }
            }
            _ => {}
        }
    }
    None
}

/// The arguments (token ranges) of the call whose `(` is at `open`.
fn args(t: &[Tok], open: usize, close: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut start = open + 1;
    for (k, x) in t.iter().enumerate().take(close).skip(open + 1) {
        match x {
            Tok::Punct('(') => depth += 1,
            Tok::Punct(')') => depth -= 1,
            Tok::Punct(',') if depth == 0 => {
                out.push((start, k));
                start = k + 1;
            }
            _ => {}
        }
    }
    out.push((start, close));
    out
}

// ---- statements -----------------------------------------------------------

/// A Db2 timestamp (`2026-10-12-13.45.01.123456`) as PostgreSQL reads it.
fn timestamp_literal(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let digits =
        |r: std::ops::Range<usize>| b.get(r).is_some_and(|x| x.iter().all(u8::is_ascii_digit));
    if b.len() < 19
        || !digits(0..4)
        || b[4] != b'-'
        || !digits(5..7)
        || b[7] != b'-'
        || !digits(8..10)
        || b[10] != b'-'
        || !digits(11..13)
        || b[13] != b'.'
        || !digits(14..16)
        || b[16] != b'.'
        || !digits(17..19)
        || (b.len() > 19 && (b[19] != b'.' || !digits(20..b.len())))
    {
        return None;
    }
    Some(format!(
        "{} {}:{}:{}{}",
        &s[..10],
        &s[11..13],
        &s[14..16],
        &s[17..19],
        &s[19..]
    ))
}

/// A Db2 date or time in another of its formats (EUR `dd.mm.yyyy`, USA
/// `mm/dd/yyyy`, ISO / EUR time `hh.mm.ss`) as PostgreSQL reads it.
fn datetime_literal(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let d = |r: std::ops::Range<usize>| b.get(r).is_some_and(|x| x.iter().all(u8::is_ascii_digit));
    match b.len() {
        10 if d(0..2) && b[2] == b'.' && d(3..5) && b[5] == b'.' && d(6..10) => {
            Some(format!("{}-{}-{}", &s[6..10], &s[3..5], &s[0..2]))
        }
        10 if d(0..2) && b[2] == b'/' && d(3..5) && b[5] == b'/' && d(6..10) => {
            Some(format!("{}-{}-{}", &s[6..10], &s[0..2], &s[3..5]))
        }
        8 if d(0..2) && b[2] == b'.' && d(3..5) && b[5] == b'.' && d(6..8) => {
            Some(s.replace('.', ":"))
        }
        _ => None,
    }
}

/// The default of `NOT NULL WITH DEFAULT` for a column type.
fn type_default(def: &str) -> &'static str {
    let u = def.to_ascii_uppercase();
    let ty = u.split_whitespace().nth(1).unwrap_or("");
    if ty.starts_with("TIMESTAMP") {
        "LOCALTIMESTAMP"
    } else if ty.starts_with("DATE") {
        "CURRENT_DATE"
    } else if ty.starts_with("TIME") {
        "LOCALTIME"
    } else if ty.starts_with("CHAR")
        || ty.starts_with("VARCHAR")
        || ty.starts_with("GRAPHIC")
        || ty.starts_with("VARGRAPHIC")
    {
        "''"
    } else {
        "0"
    }
}

/// The statement in PostgreSQL's dialect (after [`crate::dialect::translate`]).
pub fn translate(sql: &str) -> String {
    let mut t = tokens(sql.trim().trim_end_matches(';'));
    let first = next(&t, 0).map(|k| upper(&t[k])).unwrap_or_default();
    let second = next(&t, 0)
        .and_then(|k| next(&t, k + 1))
        .map(|k| upper(&t[k]))
        .unwrap_or_default();
    let ddl =
        matches!(first.as_str(), "CREATE" | "ALTER") || (first == "DECLARE" && second == "GLOBAL");
    let mut prefix = String::new();
    // SET CURRENT SCHEMA = 'X' / SET SCHEMA X
    if first == "SET" && matches!(second.as_str(), "CURRENT" | "SCHEMA") {
        let name = t
            .iter()
            .rev()
            .find_map(|x| match x {
                Tok::Str(s) => Some(s.trim_matches('\'').to_string()),
                Tok::Word(w)
                    if !matches!(
                        w.to_ascii_uppercase().as_str(),
                        "SET" | "CURRENT" | "SCHEMA" | "SQLID"
                    ) =>
                {
                    Some(w.clone())
                }
                _ => None,
            })
            .unwrap_or_default();
        return format!("CREATE SCHEMA IF NOT EXISTS {name}; SET search_path TO {name}");
    }
    // the schema of a qualified object being created
    if first == "CREATE" {
        let mut k = next(&t, 0).unwrap() + 1;
        // CREATE [UNIQUE] TABLE | INDEX | VIEW | SEQUENCE name
        while let Some(at) = next(&t, k) {
            let u = upper(&t[at]);
            k = at + 1;
            if matches!(u.as_str(), "TABLE" | "VIEW" | "SEQUENCE" | "INDEX") {
                if let Some(n) = next(&t, k)
                    && let Tok::Word(name) = &t[n]
                    && let Some((schema, object)) = name.split_once('.')
                {
                    if u == "INDEX" {
                        // the index is in its table's schema
                        t[n] = Tok::Word(object.to_string());
                    } else {
                        prefix = format!("CREATE SCHEMA IF NOT EXISTS {schema}; ");
                    }
                }
                break;
            }
            if !matches!(u.as_str(), "UNIQUE" | "OR" | "REPLACE") {
                break;
            }
        }
    }
    let mut out: Vec<Tok> = Vec::with_capacity(t.len());
    let mut i = 0;
    while i < t.len() {
        let u = upper(&t[i]);
        // X'..': a binary string
        if u == "X"
            && let Some(Tok::Str(h)) = t.get(i + 1)
        {
            let h = h.trim_matches('\'');
            out.push(Tok::Str(format!("'\\x{h}'::BYTEA")));
            i += 2;
            continue;
        }
        // string literals: Db2 timestamps
        if let Tok::Str(s) = &t[i] {
            let inner = &s[1..s.len().saturating_sub(1)];
            if let Some(ts) = timestamp_literal(inner).or_else(|| datetime_literal(inner)) {
                out.push(Tok::Str(format!("'{ts}'")));
                i += 1;
                continue;
            }
        }
        // special registers
        if u == "CURRENT" || u == "CURRENT_TIMESTAMP" {
            if u == "CURRENT_TIMESTAMP" {
                out.push(Tok::Word("LOCALTIMESTAMP".into()));
                i += 1;
                continue;
            }
            if let Some(n) = next(&t, i + 1) {
                let reg = match upper(&t[n]).as_str() {
                    "TIMESTAMP" => Some("LOCALTIMESTAMP"),
                    "DATE" => Some("CURRENT_DATE"),
                    "TIME" => Some("LOCALTIME"),
                    "SCHEMA" | "SQLID" => Some("CURRENT_SCHEMA"),
                    "SERVER" => Some("current_database()"),
                    _ => None,
                };
                if let Some(r) = reg {
                    out.push(Tok::Word(r.into()));
                    i = n + 1;
                    continue;
                }
            }
        }
        // SYSIBM.SYSDUMMY1: a table of one row
        if u == "SYSIBM.SYSDUMMY1" {
            out.push(Tok::Word("(SELECT 'Y' AS IBMREQD)".into()));
            let alias = next(&t, i + 1).is_some_and(|n| {
                matches!(t[n], Tok::Word(_))
                    && !matches!(
                        upper(&t[n]).as_str(),
                        "WHERE"
                            | "UNION"
                            | "EXCEPT"
                            | "INTERSECT"
                            | "ORDER"
                            | "GROUP"
                            | "HAVING"
                            | "FETCH"
                            | "WITH"
                            | "FOR"
                            | "OPTIMIZE"
                            | "JOIN"
                            | "INNER"
                            | "LEFT"
                            | "RIGHT"
                            | "FULL"
                            | "CROSS"
                            | "ON"
                            | "QUERYNO"
                    )
            });
            if !alias {
                out.push(Tok::Word(" AS SYSDUMMY1".into()));
            }
            i += 1;
            continue;
        }
        // ALTER TABLE t FOREIGN KEY / PRIMARY KEY / UNIQUE / CHECK /
        // CONSTRAINT: ADD
        if first == "ALTER"
            && second == "TABLE"
            && matches!(
                u.as_str(),
                "FOREIGN" | "PRIMARY" | "UNIQUE" | "CHECK" | "CONSTRAINT"
            )
            && out
                .iter()
                .rev()
                .find(|x| !matches!(x, Tok::Space(_)))
                .is_some_and(|x| matches!(x, Tok::Word(_)) && upper(x) != "ADD")
            && out.iter().filter(|x| matches!(x, Tok::Word(_))).count() == 3
        {
            out.push(Tok::Word("ADD ".into()));
        }
        // isolation and access clauses
        if u == "WITH"
            && let Some(n) = next(&t, i + 1)
            && matches!(upper(&t[n]).as_str(), "UR" | "CS" | "RS" | "RR")
            && next(&t, n + 1)
                .is_none_or(|m| matches!(upper(&t[m]).as_str(), "USE" | "QUERYNO" | "SKIP"))
        {
            break;
        }
        if let Some(end) = words_at(&t, i, &["FOR", "FETCH", "ONLY"])
            .or_else(|| words_at(&t, i, &["FOR", "READ", "ONLY"]))
        {
            i = end;
            continue;
        }
        if u == "OPTIMIZE"
            && let Some(end) = words_at(&t, i, &["OPTIMIZE", "FOR"])
            && let Some(n) = next(&t, end)
            && let Some(r) = next(&t, n + 1)
            && matches!(upper(&t[r]).as_str(), "ROW" | "ROWS")
        {
            i = r + 1;
            continue;
        }
        // FOR UPDATE OF columns: the rows are locked
        if let Some(end) = words_at(&t, i, &["FOR", "UPDATE", "OF"]) {
            out.push(Tok::Word("FOR UPDATE".into()));
            // the column list runs to the end or the next clause
            let mut k = end;
            while let Some(n) = next(&t, k) {
                if matches!(t[n], Tok::Word(_) | Tok::Quoted(_) | Tok::Punct(','))
                    && !matches!(
                        upper(&t[n]).as_str(),
                        "WITH" | "SKIP" | "OPTIMIZE" | "QUERYNO"
                    )
                {
                    k = n + 1;
                } else {
                    break;
                }
            }
            i = k;
            continue;
        }
        // NEXT VALUE FOR s / PREVIOUS VALUE FOR s
        if let Some(end) = words_at(&t, i, &["NEXT", "VALUE", "FOR"])
            .map(|e| (e, "nextval"))
            .or_else(|| words_at(&t, i, &["PREVIOUS", "VALUE", "FOR"]).map(|e| (e, "currval")))
            && let Some(n) = next(&t, end.0)
            && let Tok::Word(s) = &t[n]
        {
            out.push(Tok::Word(format!(
                "{}('{}')",
                end.1,
                s.to_ascii_lowercase()
            )));
            i = n + 1;
            continue;
        }
        // functions (not a type after AS: CAST(x AS CHAR(50)))
        let type_name = (0..i)
            .rev()
            .find(|&k| !matches!(t[k], Tok::Space(_)))
            .is_some_and(|k| upper(&t[k]) == "AS");
        if let Tok::Word(_) = &t[i]
            && !type_name
            && let Some(open) = next(&t, i + 1).filter(|&n| t[n] == Tok::Punct('('))
            && let Some(close) = close_paren(&t, open)
        {
            let a = args(&t, open, close);
            let arg = |n: usize| -> String {
                a.get(n)
                    .map(|&(s, e)| translate_part(&t[s..e]))
                    .unwrap_or_default()
            };
            let call = match (u.as_str(), a.len()) {
                ("VALUE", _) => Some(format!(
                    "COALESCE({})",
                    (0..a.len()).map(arg).collect::<Vec<_>>().join(", ")
                )),
                ("IDENTITY_VAL_LOCAL", _) => Some("lastval()".into()),
                ("STRIP", 1) => Some(format!("TRIM({})", arg(0))),
                ("YEAR" | "MONTH" | "DAY" | "HOUR" | "MINUTE" | "SECOND", 1) => {
                    Some(format!("CAST(EXTRACT({u} FROM {}) AS INTEGER)", arg(0)))
                }
                ("DAYS", 1) => Some(format!(
                    "(CAST({} AS DATE) - DATE '0001-01-01' + 1)",
                    arg(0)
                )),
                ("CHAR", 2) if !ddl => {
                    let f = match arg(1).trim().to_ascii_uppercase().as_str() {
                        "USA" => "MM/DD/YYYY",
                        "EUR" => "DD.MM.YYYY",
                        "JIS" | "ISO" | "LOCAL" => "YYYY-MM-DD",
                        _ => "YYYY-MM-DD",
                    };
                    Some(format!("to_char({}, '{f}')", arg(0)))
                }
                ("CHAR" | "VARCHAR", 1) if !ddl => Some(format!("CAST({} AS VARCHAR)", arg(0))),
                ("INTEGER" | "INT", 1) if !ddl => Some(format!("CAST({} AS INTEGER)", arg(0))),
                ("SMALLINT", 1) if !ddl => Some(format!("CAST({} AS SMALLINT)", arg(0))),
                ("BIGINT", 1) if !ddl => Some(format!("CAST({} AS BIGINT)", arg(0))),
                ("DOUBLE" | "FLOAT", 1) if !ddl => {
                    Some(format!("CAST({} AS DOUBLE PRECISION)", arg(0)))
                }
                ("DECIMAL" | "DEC", n) if !ddl && n >= 1 => {
                    let p = if n >= 2 { arg(1) } else { "5".into() };
                    let s = if n >= 3 { arg(2) } else { "0".into() };
                    Some(format!(
                        "CAST({} AS DECIMAL({},{}))",
                        arg(0),
                        p.trim(),
                        s.trim()
                    ))
                }
                ("DATE", 1) if !ddl => Some(format!("CAST({} AS DATE)", arg(0))),
                ("TIMESTAMP", 1) if !ddl => Some(format!("CAST({} AS TIMESTAMP)", arg(0))),
                ("DIGITS", 1) => Some(format!(
                    "lpad(CAST(abs({0}) AS VARCHAR), length(CAST(abs({0}) AS VARCHAR)), '0')",
                    arg(0)
                )),
                _ => None,
            };
            if let Some(c) = call {
                out.push(Tok::Word(c));
                i = close + 1;
                continue;
            }
        }
        // DDL
        if ddl {
            // NOT NULL WITH DEFAULT [value]
            if let Some(end) = words_at(&t, i, &["WITH", "DEFAULT"]) {
                let value_follows = next(&t, end).is_some_and(|n| {
                    !matches!(t[n], Tok::Punct(',') | Tok::Punct(')'))
                        && !matches!(
                            upper(&t[n]).as_str(),
                            "NOT" | "PRIMARY" | "UNIQUE" | "CONSTRAINT" | "REFERENCES" | "CHECK"
                        )
                });
                if value_follows {
                    out.push(Tok::Word("DEFAULT".into()));
                } else {
                    // the column definition so far: back to the comma or
                    // parenthesis that starts it
                    let mut depth = 0;
                    let mut s = 0;
                    for (k, x) in out.iter().enumerate().rev() {
                        match x {
                            Tok::Punct(')') => depth += 1,
                            Tok::Punct('(') if depth > 0 => depth -= 1,
                            Tok::Punct('(') | Tok::Punct(',') if depth == 0 => {
                                s = k + 1;
                                break;
                            }
                            _ => {}
                        }
                    }
                    let def = text(&out[s.min(out.len())..]);
                    out.push(Tok::Word(format!("DEFAULT {}", type_default(def.trim()))));
                }
                i = end;
                continue;
            }
            // identity options: spaces between them, without those
            // PostgreSQL lacks
            if let Some(end) = words_at(&t, i, &["AS", "IDENTITY"])
                && let Some(open) = next(&t, end).filter(|&n| t[n] == Tok::Punct('('))
                && let Some(close) = close_paren(&t, open)
            {
                let opts: Vec<String> = args(&t, open, close)
                    .into_iter()
                    .map(|(s, e)| text(&t[s..e]).trim().to_string())
                    .filter(|o| {
                        let u = o.to_ascii_uppercase();
                        !matches!(u.as_str(), "NO CACHE" | "ORDER" | "NO ORDER") && !u.is_empty()
                    })
                    .collect();
                out.push(Tok::Word(format!("AS IDENTITY ({})", opts.join(" "))));
                i = close + 1;
                continue;
            }
            // types
            if u == "FOR"
                && let Some(end) = words_at(&t, i, &["FOR", "BIT", "DATA"])
            {
                // CHAR(n) FOR BIT DATA: bytes
                if let Some(p) = out
                    .iter()
                    .rposition(|x| matches!(upper(x).as_str(), "CHAR" | "VARCHAR" | "CHARACTER"))
                {
                    out.truncate(p);
                    out.push(Tok::Word("BYTEA".into()));
                }
                i = end;
                continue;
            }
            if matches!(u.as_str(), "GRAPHIC" | "VARGRAPHIC") {
                out.push(Tok::Word("VARCHAR".into()));
                i += 1;
                continue;
            }
            if let Some(end) = words_at(&t, i, &["LONG", "VARCHAR"]) {
                out.push(Tok::Word("TEXT".into()));
                i = end;
                continue;
            }
            if matches!(u.as_str(), "CLOB" | "DBCLOB" | "BLOB") {
                out.push(Tok::Word(if u == "BLOB" { "BYTEA" } else { "TEXT" }.into()));
                // the length
                if let Some(open) = next(&t, i + 1).filter(|&n| t[n] == Tok::Punct('('))
                    && let Some(close) = close_paren(&t, open)
                {
                    i = close + 1;
                } else {
                    i += 1;
                }
                continue;
            }
            if u == "DOUBLE" && next(&t, i + 1).is_none_or(|n| upper(&t[n]) != "PRECISION") {
                out.push(Tok::Word("DOUBLE PRECISION".into()));
                i += 1;
                continue;
            }
            // CREATE INDEX ... CLUSTER
            if u == "CLUSTER" && first == "CREATE" {
                i += 1;
                continue;
            }
        }
        out.push(t[i].clone());
        i += 1;
    }
    let mut sql = format!("{prefix}{}", text(&out));
    // DROP TABLE drops the referential constraints on the table too
    if first == "DROP" && second == "TABLE" && !sql.to_ascii_uppercase().ends_with("CASCADE") {
        sql.push_str(" CASCADE");
    }
    sql
}

/// A part of a statement (a function argument) in PostgreSQL's dialect.
fn translate_part(t: &[Tok]) -> String {
    translate(&text(t))
}

// ---- results --------------------------------------------------------------

/// A PostgreSQL value in the form Db2 gives it.
/// A result column described in DRDA terms (type code, length).
fn column(c: &zeus_pg::Column, nullable: bool) -> zeus_drda::sql::Column {
    let m = c.type_mod;
    let (t, len) = match c.type_oid {
        oid::INT2 | oid::BOOL => (0x04, 2),
        oid::INT4 => (0x02, 4),
        oid::INT8 => (0x16, 8),
        oid::NUMERIC if m >= 4 => {
            let m = m - 4;
            (0x0E, ((((m >> 16) & 0xFF) << 8) | (m & 0xFF)) as u16)
        }
        oid::NUMERIC => (0x0E, 31 << 8),
        oid::FLOAT4 => (0x0C, 4),
        oid::FLOAT8 => (0x0A, 8),
        oid::DATE => (0x20, 10),
        oid::TIME => (0x22, 8),
        oid::TIMESTAMP | oid::TIMESTAMPTZ => (0x24, 26),
        oid::BPCHAR if m >= 4 => (0x30, (m - 4) as u16),
        oid::VARCHAR if m >= 4 => (0x32, (m - 4) as u16),
        oid::BYTEA => (0x28, 32704),
        _ => (0x32, 32704),
    };
    zeus_drda::sql::Column {
        drda_type: t,
        nullable,
        len,
        name: c.name.to_ascii_uppercase(),
    }
}

fn value(v: Option<String>, type_oid: u32) -> Value {
    let Some(s) = v else { return Value::Null };
    match type_oid {
        oid::INT2 | oid::INT4 | oid::INT8 => s.parse().map_or(Value::Str(s), Value::Int),
        oid::BOOL => Value::Int(i64::from(s == "t")),
        oid::NUMERIC => Value::Dec(s),
        oid::FLOAT4 | oid::FLOAT8 => s.parse().map_or(Value::Str(s), Value::Float),
        // hh.mm.ss
        oid::TIME => Value::Str(s.get(..8).unwrap_or(&s).replace(':', ".")),
        // yyyy-mm-dd-hh.mm.ss.nnnnnn
        oid::TIMESTAMP | oid::TIMESTAMPTZ => {
            let s = s.split(['+']).next().unwrap_or(&s).to_string();
            let (date, time) = s.split_once(' ').unwrap_or((&s, "00:00:00"));
            let (hms, frac) = time.split_once('.').unwrap_or((time, ""));
            let mut frac = frac.to_string();
            frac.truncate(6);
            while frac.len() < 6 {
                frac.push('0');
            }
            Value::Str(format!("{date}-{}.{frac}", hms.replace(':', ".")))
        }
        oid::BYTEA => {
            let h = s.trim_start_matches("\\x");
            Value::Bytes(
                (0..h.len() / 2)
                    .filter_map(|k| u8::from_str_radix(&h[2 * k..2 * k + 2], 16).ok())
                    .collect(),
            )
        }
        _ => Value::Str(s),
    }
}

/// The SQLCODE of an SQLSTATE (the Db2 code with the same meaning).
fn sqlcode(e: &ServerError, sql: &str) -> i32 {
    let delete = sql.trim_start().to_ascii_uppercase().starts_with("DELETE");
    match e.sqlstate.as_str() {
        "23505" => -803,
        "23503" if delete => -532,
        "23503" => -530,
        "23502" => -407,
        "23514" => -545,
        "22001" => -404,
        "22003" | "22012" => -802,
        "22007" => -180,
        "22008" => -181,
        "22P02" => -420,
        "21000" => -811,
        "34000" => -504,
        "42P01" | "3F000" | "42704" => -204,
        "42703" => -206,
        "42601" => -104,
        "42883" => -440,
        "42804" | "42846" => -401,
        "42P07" | "42710" | "42P06" => -601,
        "42701" => -612,
        "42501" => -551,
        "40001" | "40P01" => -911,
        "55P03" => -913,
        "57014" => -905,
        "0A000" => -142,
        s if s.starts_with("08") => -30081,
        _ => -901,
    }
}

/// The message tokens: the name an error is about, else its message.
fn tokens_of(e: &ServerError) -> Vec<String> {
    if let Some(c) = e
        .constraint
        .as_ref()
        .or(e.column.as_ref())
        .or(e.table.as_ref())
    {
        return vec![c.to_ascii_uppercase()];
    }
    match e.message.split('"').nth(1) {
        Some(name) => vec![name.to_ascii_uppercase()],
        None => vec![e.message.clone()],
    }
}

fn error_ca(e: &ServerError, sql: &str) -> SqlCa {
    SqlCa {
        sqlcode: sqlcode(e, sql),
        sqlstate: e.sqlstate.clone(),
        tokens: tokens_of(e),
        errproc: "ZEUSPG".into(),
        ..SqlCa::default()
    }
}

/// The SQLCA of a statement that ran: SQLERRD(3) the rows changed; +100
/// for an INSERT, UPDATE or DELETE that changed none.
fn ok_ca(st: &Statement) -> SqlCa {
    let mut ca = SqlCa::ok();
    let verb = st.tag.split(' ').next().unwrap_or("");
    if matches!(verb, "INSERT" | "UPDATE" | "DELETE" | "MERGE") {
        let n = st.count().unwrap_or(0);
        ca.errd[2] = n.min(i32::MAX as u64) as i32;
        if n == 0 {
            ca.sqlcode = 100;
            ca.sqlstate = "02000".into();
        }
    }
    ca
}

// ---- sessions -------------------------------------------------------------

pub struct Session {
    conn: Connection,
    /// Whether table columns are NOT NULL, by (table, column number).
    not_null: std::collections::HashMap<(u32, i16), bool>,
}

impl Session {
    pub fn connect(p: &Params) -> Result<Session, String> {
        let conn = Connection::connect(p).map_err(|e| e.to_string())?;
        Ok(Session {
            conn,
            not_null: std::collections::HashMap::new(),
        })
    }

    /// The qualifier of unqualified names (the schema is created if need
    /// be), for the whole session.
    pub fn set_schema(&mut self, schema: &str) -> Result<(), String> {
        self.conn
            .execute(&format!(
                "CREATE SCHEMA IF NOT EXISTS {schema}; SET search_path TO {schema}"
            ))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Whether the result columns are NOT NULL: columns of tables as the
    /// catalog says, expressions taken as not null.
    fn not_null_of(&mut self, cols: &[zeus_pg::Column]) -> Vec<bool> {
        let missing: Vec<(u32, i16)> = cols
            .iter()
            .filter(|c| c.table_oid != 0)
            .map(|c| (c.table_oid, c.attnum))
            .filter(|k| !self.not_null.contains_key(k))
            .collect();
        if !missing.is_empty() && self.conn.status != b'E' {
            let list: Vec<String> = missing.iter().map(|(t, a)| format!("({t},{a})")).collect();
            let q = format!(
                "SELECT attrelid, attnum, attnotnull FROM pg_catalog.pg_attribute \
                 WHERE (attrelid, attnum) IN ({})",
                list.join(",")
            );
            if let Ok((sts, None)) = self.conn.simple_query(&q) {
                for row in sts.iter().flat_map(|s| s.rows.iter()) {
                    let get = |k: usize| row.get(k).cloned().flatten().unwrap_or_default();
                    if let (Ok(t), Ok(a)) = (get(0).parse(), get(1).parse()) {
                        self.not_null.insert((t, a), get(2) == "t");
                    }
                }
            }
        }
        cols.iter()
            .map(|c| {
                c.table_oid == 0
                    || self
                        .not_null
                        .get(&(c.table_oid, c.attnum))
                        .copied()
                        .unwrap_or(false)
            })
            .collect()
    }

    /// Runs a statement (in Db2's dialect) in the unit of work: its
    /// result, or the SQLCA of its error.
    fn run(&mut self, sql: &str) -> Result<Result<Statement, SqlCa>, String> {
        // text cannot hold NUL characters (LOW-VALUES of host variables)
        let pg = translate(sql).replace('\0', " ");
        // Db2's unit of work starts with its first statement
        let begin = if self.conn.status == b'I' {
            "BEGIN; "
        } else {
            ""
        };
        let q = format!("{begin}SAVEPOINT zeus_stmt; {pg}; RELEASE SAVEPOINT zeus_stmt");
        let (mut done, error) = self.conn.simple_query(&q).map_err(|e| e.to_string())?;
        match error {
            None => {
                // the statement's result is before RELEASE
                done.pop();
                Ok(Ok(done.pop().unwrap_or_default()))
            }
            Some(e) => {
                if self.conn.status == b'E' {
                    let _ = self
                        .conn
                        .simple_query("ROLLBACK TO SAVEPOINT zeus_stmt")
                        .map_err(|e| e.to_string())?;
                    if self.conn.status == b'E' {
                        // the savepoint was not set: the block ends
                        let _ = self.conn.simple_query("ROLLBACK");
                    }
                }
                Ok(Err(error_ca(&e, sql)))
            }
        }
    }

    pub fn execute(&mut self, sql: &str) -> Result<SqlCa, String> {
        Ok(match self.run(sql)? {
            Ok(st) => ok_ca(&st),
            Err(ca) => ca,
        })
    }

    pub fn query(&mut self, sql: &str) -> Result<Rows, String> {
        Ok(match self.run(sql)? {
            Ok(st) => Rows {
                columns: {
                    let nn = self.not_null_of(&st.columns);
                    st.columns
                        .iter()
                        .zip(nn)
                        .map(|(c, n)| column(c, !n))
                        .collect()
                },
                rows: st
                    .rows
                    .into_iter()
                    .map(|r| {
                        r.into_iter()
                            .zip(&st.columns)
                            .map(|(v, c)| value(v, c.type_oid))
                            .collect()
                    })
                    .collect(),
                sqlca: SqlCa::ok(),
            },
            Err(ca) => Rows {
                sqlca: ca,
                ..Rows::default()
            },
        })
    }

    /// Ends the unit of work.
    pub fn end(&mut self, commit: bool) -> Result<SqlCa, String> {
        if self.conn.status == b'I' {
            return Ok(SqlCa::ok());
        }
        let sql = if commit { "COMMIT" } else { "ROLLBACK" };
        match self.conn.execute(sql) {
            Ok(_) => Ok(SqlCa::ok()),
            Err(Error::Server(e)) => Ok(error_ca(&e, sql)),
            Err(Error::Io(e)) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(s: &str) -> String {
        s.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn statements() {
        assert_eq!(
            norm(&translate(
                "SELECT VALUE(MAX(A), 0), CURRENT TIMESTAMP INTO ? FROM SYSIBM.SYSDUMMY1 WITH UR"
            )),
            "SELECT COALESCE(MAX(A), 0), LOCALTIMESTAMP INTO ? FROM (SELECT 'Y' AS IBMREQD) AS SYSDUMMY1"
        );
        assert_eq!(
            norm(&translate(
                "SELECT A FROM T WHERE B = 'X' FOR UPDATE OF A, C"
            )),
            "SELECT A FROM T WHERE B = 'X' FOR UPDATE"
        );
        assert_eq!(
            norm(&translate("UPDATE T SET D = '2026-10-12-13.45.01.123456'")),
            "UPDATE T SET D = '2026-10-12 13:45:01.123456'"
        );
        assert_eq!(
            norm(&translate("SELECT CHAR(D, ISO), YEAR(D) FROM T")),
            "SELECT to_char(D, 'YYYY-MM-DD'), CAST(EXTRACT(YEAR FROM D) AS INTEGER) FROM T"
        );
        // a type after AS is not the CHAR function
        assert_eq!(
            norm(&translate("SELECT CAST(CHAR(A) AS CHAR(50)) FROM T")),
            "SELECT CAST(CAST(A AS VARCHAR) AS CHAR(50)) FROM T"
        );
        assert_eq!(
            norm(&translate("VALUES (IDENTITY_VAL_LOCAL())")),
            "VALUES (lastval())"
        );
        assert_eq!(
            norm(&translate(
                "INSERT INTO T VALUES ('23.01.1986', '12/31/2026', '13.45.01')"
            )),
            "INSERT INTO T VALUES ('1986-01-23', '2026-12-31', '13:45:01')"
        );
        assert_eq!(
            norm(&translate("SET CURRENT SCHEMA = 'GENASA1'")),
            "CREATE SCHEMA IF NOT EXISTS GENASA1; SET search_path TO GENASA1"
        );
    }

    #[test]
    fn ddl() {
        assert_eq!(
            norm(&translate(
                "CREATE TABLE G.P (N INTEGER NOT NULL GENERATED BY DEFAULT AS IDENTITY \
                 (START WITH 1, INCREMENT BY 1, NO CACHE), L TIMESTAMP NOT NULL WITH DEFAULT, \
                 C CHAR(4) NOT NULL WITH DEFAULT, B CHAR(8) FOR BIT DATA)"
            )),
            "CREATE SCHEMA IF NOT EXISTS G; CREATE TABLE G.P (N INTEGER NOT NULL GENERATED BY \
             DEFAULT AS IDENTITY (START WITH 1 INCREMENT BY 1), L TIMESTAMP NOT NULL DEFAULT \
             LOCALTIMESTAMP, C CHAR(4) NOT NULL DEFAULT '', B BYTEA)"
        );
        assert_eq!(
            norm(&translate(
                "ALTER TABLE G.C FOREIGN KEY (T) REFERENCES G.T (T) ON DELETE RESTRICT"
            )),
            "ALTER TABLE G.C ADD FOREIGN KEY (T) REFERENCES G.T (T) ON DELETE RESTRICT"
        );
        assert_eq!(
            norm(&translate(
                "CREATE UNIQUE INDEX G.PX ON G.P (N ASC) CLUSTER"
            )),
            "CREATE UNIQUE INDEX PX ON G.P (N ASC)"
        );
    }

    #[test]
    fn results() {
        assert_eq!(
            value(Some("2026-10-12 13:45:01.5".into()), oid::TIMESTAMP),
            Value::Str("2026-10-12-13.45.01.500000".into())
        );
        assert_eq!(
            value(Some("13:45:01".into()), oid::TIME),
            Value::Str("13.45.01".into())
        );
        let e = ServerError {
            sqlstate: "23505".into(),
            constraint: Some("cust_pk".into()),
            ..ServerError::default()
        };
        assert_eq!(error_ca(&e, "INSERT INTO T VALUES (1)").sqlcode, -803);
        let st = Statement {
            tag: "UPDATE 0".into(),
            ..Statement::default()
        };
        assert_eq!(ok_ca(&st).sqlcode, 100);
    }
}

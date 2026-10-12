//! Runs EXEC SQL statements of COBOL programs on a Db2 server.
//!
//! Host variables are bound as SQL literals: character data is converted
//! from EBCDIC, numbers are written as decimal literals and an indicator
//! variable below zero makes the value NULL. Results come back the other
//! way. The SQLCA of the program receives SQLCODE, SQLSTATE, SQLERRD and
//! the message tokens of each statement.

use std::cell::RefCell;
use std::collections::HashMap;

use zeus_cobol::ebcdic;
use zeus_cobol::vm::exec::{Area, ExecArg, ExecCall, ExecHost, ExecOutcome};
use zeus_cobol::vm::{Machine, RResult, RunError, data};
use zeus_cobol::zir::{DataType, ExecKind};
use zeus_decimal::Dec;
use zeus_drda::sql::{Rows, SqlCa, Value};
use zeus_drda::{Connection, Params};

struct Cursor {
    rows: Vec<Vec<Value>>,
    next: usize,
}

struct State {
    params: Option<Params>,
    schema: Option<String>,
    conn: Option<Connection>,
    cursors: HashMap<String, Cursor>,
}

/// The Db2 interface of a run unit or a CICS region.
pub struct Db2Host {
    st: RefCell<State>,
}

impl Db2Host {
    /// A host for the server named by ZEUS_DB2 (`db2://user:pw@host:port/DB`)
    /// with the qualifier ZEUS_DB2_SCHEMA; None when ZEUS_DB2 is not set.
    pub fn from_env() -> Option<Db2Host> {
        let url = std::env::var("ZEUS_DB2").ok()?;
        let params = Params::from_url(&url).ok();
        Some(Db2Host::new(params, std::env::var("ZEUS_DB2_SCHEMA").ok()))
    }

    pub fn new(params: Option<Params>, schema: Option<String>) -> Db2Host {
        Db2Host {
            st: RefCell::new(State {
                params,
                schema,
                conn: None,
                cursors: HashMap::new(),
            }),
        }
    }

    fn with_conn<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> zeus_drda::client::Result<T>,
    ) -> Result<T, SqlCa> {
        let mut st = self.st.borrow_mut();
        if st.conn.is_none() {
            let Some(p) = st.params.clone() else {
                return Err(comm_error("no Db2 server configured (ZEUS_DB2)"));
            };
            let mut c = Connection::connect(&p).map_err(|e| comm_error(&e.0))?;
            if let Some(s) = st.schema.clone() {
                c.execute(&format!("SET CURRENT SCHEMA = '{s}'"))
                    .map_err(|e| comm_error(&e.0))?;
            }
            st.conn = Some(c);
        }
        let c = st.conn.as_mut().unwrap();
        let r = f(c).map_err(|e| comm_error(&e.0));
        if r.is_err() {
            // the connection is no longer usable
            st.conn = None;
        }
        r
    }

    /// Ends the unit of work (commit or rollback); cursors are closed.
    pub fn end_uow(&self, commit: bool) -> Result<SqlCa, SqlCa> {
        self.st.borrow_mut().cursors.clear();
        if self.st.borrow().conn.is_none() {
            return Ok(SqlCa::ok());
        }
        self.with_conn(|c| if commit { c.commit() } else { c.rollback() })
    }

    /// Runs one SQL statement of a script (DSNTIAD), translated for the
    /// server; Ok(None) when the statement was skipped.
    pub fn run_script_statement(&self, sql: &str) -> Result<Option<SqlCa>, SqlCa> {
        self.run_script_query(sql, &mut Vec::new())
    }

    /// As [`Db2Host::run_script_statement`]; the rows of a query are added
    /// to `rows` as text (DSNTEP2 output).
    pub fn run_script_query(
        &self,
        sql: &str,
        rows: &mut Vec<Vec<String>>,
    ) -> Result<Option<SqlCa>, SqlCa> {
        let Some(t) = crate::dialect::translate(sql) else {
            return Ok(None);
        };
        let first = t
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        let r = if first == "SELECT" || first == "WITH" || first == "VALUES" {
            self.with_conn(|c| c.query(&t)).map(|r| {
                for row in &r.rows {
                    rows.push(
                        row.iter()
                            .map(|v| match v {
                                zeus_drda::sql::Value::Null => "-".to_string(),
                                zeus_drda::sql::Value::Int(i) => i.to_string(),
                                zeus_drda::sql::Value::Dec(d) => d.clone(),
                                zeus_drda::sql::Value::Float(f) => f.to_string(),
                                zeus_drda::sql::Value::Str(s) => s.clone(),
                                zeus_drda::sql::Value::Bytes(b) => {
                                    b.iter().map(|x| format!("{x:02X}")).collect()
                                }
                            })
                            .collect(),
                    );
                }
                r.sqlca
            })
        } else {
            self.with_conn(|c| c.execute(&t))
        };
        r.map(|ca| Some(crate::dialect::adjust_sqlca(&t, ca)))
    }
}

fn comm_error(msg: &str) -> SqlCa {
    SqlCa {
        sqlcode: -30081,
        sqlstate: "08001".into(),
        tokens: vec![msg.to_string()],
        ..SqlCa::default()
    }
}

/// Text of an EBCDIC field in the active code page (mixed data: SO / SI
/// shift to double-byte characters).
fn text_of(b: &[u8]) -> String {
    ebcdic::decode(b)
}

/// EBCDIC of a text in the active code page (characters it lacks become
/// the substitution character).
fn ebcdic_of(s: &str) -> Vec<u8> {
    ebcdic::encode(s)
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// The SQL literal of a host variable.
fn literal(m: &Machine, area: &Area, varchar: bool, ind: Option<&Area>) -> String {
    if let Some(i) = ind
        && num_of(m, i) < 0
    {
        return "NULL".into();
    }
    let b = &m.storage()[area.addr..area.addr + area.len];
    if varchar && b.len() >= 2 {
        let n = (u16::from_be_bytes([b[0], b[1]]) as usize).min(b.len() - 2);
        return quote(&text_of(&b[2..2 + n]));
    }
    match &area.ty {
        DataType::Numeric(t) => data::num_value(t, b).to_string(),
        DataType::Pointer | DataType::Index => {
            i32::from_be_bytes([b[0], b[1], b[2], b[3]]).to_string()
        }
        // DBCS (PIC G): double-byte characters without SO / SI
        DataType::Dbcs => quote(&zeus_cobol::codepage::active().decode_dbcs(b)),
        // NATIONAL (PIC N): UTF-16 without trailing national spaces
        DataType::National => quote(data::from_utf16(b).trim_end_matches(' ')),
        _ => quote(&text_of(b)),
    }
}

fn num_of(m: &Machine, a: &Area) -> i64 {
    let b = &m.storage()[a.addr..a.addr + a.len];
    let d = match &a.ty {
        DataType::Numeric(t) => data::num_value(t, b),
        _ => data::alnum_as_number(b),
    };
    d.rescale(0, zeus_decimal::Rounding::Truncate)
        .map_or(0, |d| d.coef as i64)
}

/// Stores a result value in a host variable; false: a NULL without an
/// indicator variable.
fn store(m: &mut Machine, area: &Area, varchar: bool, ind: Option<&Area>, v: &Value) -> bool {
    if let Some(i) = ind {
        let n = if matches!(v, Value::Null) { -1 } else { 0 };
        store_int(m, i, n);
    }
    let text = match v {
        Value::Null => return ind.is_some(),
        Value::Int(i) => i.to_string(),
        Value::Dec(s) => s.clone(),
        Value::Float(f) => format!("{f}"),
        Value::Str(s) => s.clone(),
        Value::Bytes(b) => {
            let n = b.len().min(area.len);
            m.storage_mut()[area.addr..area.addr + n].copy_from_slice(&b[..n]);
            return true;
        }
    };
    let mem = &mut m.storage_mut()[area.addr..area.addr + area.len];
    if varchar && mem.len() >= 2 {
        let e = ebcdic_of(&text);
        let n = e.len().min(mem.len() - 2);
        mem[..2].copy_from_slice(&(n as u16).to_be_bytes());
        mem[2..2 + n].copy_from_slice(&e[..n]);
        return true;
    }
    match &area.ty {
        DataType::Numeric(t) => {
            let d = Dec::parse(text.trim()).unwrap_or(Dec::ZERO);
            data::store_num(t, mem, d, false);
        }
        DataType::Dbcs => {
            let e = zeus_cobol::codepage::active().encode_dbcs(&text);
            data::move_alnum(&e, mem, false);
        }
        DataType::National => data::move_national(&data::utf16(&text), mem),
        _ => {
            let e = ebcdic_of(&text);
            data::move_alnum(&e, mem, false);
        }
    }
    true
}

fn store_int(m: &mut Machine, a: &Area, v: i64) {
    let mem = &mut m.storage_mut()[a.addr..a.addr + a.len];
    match &a.ty {
        DataType::Numeric(t) => {
            data::store_num(t, mem, Dec::from_i64(v), false);
        }
        _ => {
            let b = v.to_be_bytes();
            let n = mem.len().min(8);
            let l = mem.len();
            mem[l - n..].copy_from_slice(&b[8 - n..]);
        }
    }
}

/// Replaces the `?` markers outside literals by `values` in order.
fn bind(sql: &str, values: &[String]) -> String {
    let mut out = String::with_capacity(sql.len());
    let mut q = false;
    let mut k = 0;
    for c in sql.chars() {
        match c {
            '\'' => {
                q = !q;
                out.push(c);
            }
            '?' if !q => {
                out.push_str(values.get(k).map(String::as_str).unwrap_or("NULL"));
                k += 1;
            }
            _ => out.push(c),
        }
    }
    out
}

/// Positions (byte offsets) of top-level keyword `kw` outside literals and
/// parentheses.
fn find_kw(sql: &str, kw: &str, from: usize) -> Option<usize> {
    let up = sql.to_ascii_uppercase();
    let b = up.as_bytes();
    let (mut q, mut depth) = (false, 0i32);
    for i in 0..b.len() {
        match b[i] {
            b'\'' => q = !q,
            b'(' if !q => depth += 1,
            b')' if !q => depth -= 1,
            _ if !q && depth == 0 && i >= from && up[i..].starts_with(kw) => {
                let before = i == 0 || !b[i - 1].is_ascii_alphanumeric();
                let after = b
                    .get(i + kw.len())
                    .is_none_or(|c| !c.is_ascii_alphanumeric() && *c != b'_');
                if before && after {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

fn count_markers(s: &str) -> usize {
    let mut q = false;
    s.chars()
        .filter(|&c| {
            if c == '\'' {
                q = !q;
            }
            c == '?' && !q
        })
        .count()
}

/// Writes an SQLCA to the program's SQLCA area.
fn set_sqlca(m: &mut Machine, area: &Area, ca: &SqlCa) {
    if area.len < 136 {
        return;
    }
    let mut b = Vec::with_capacity(136);
    b.extend(ebcdic::encode("SQLCA   "));
    b.extend(136i32.to_be_bytes());
    b.extend(ca.sqlcode.to_be_bytes());
    // SQLERRMC tokens are separated by X'FF'
    let msg = ca
        .tokens
        .iter()
        .map(|t| ebcdic_of(t))
        .collect::<Vec<_>>()
        .join(&0xFFu8);
    let mut msg: Vec<u8> = msg.into_iter().take(70).collect();
    b.extend((msg.len() as i16).to_be_bytes());
    msg.resize(70, ebcdic::SPACE);
    b.extend(msg);
    let mut p = ebcdic_of(&ca.errproc);
    p.resize(8, ebcdic::SPACE);
    b.extend(p);
    for e in ca.errd {
        b.extend(e.to_be_bytes());
    }
    let mut w = ebcdic_of(&ca.warn);
    w.resize(11, ebcdic::SPACE);
    b.extend(w);
    let mut s = ebcdic_of(&ca.sqlstate);
    s.resize(5, ebcdic::ZERO);
    b.extend(s);
    m.storage_mut()[area.addr..area.addr + 136].copy_from_slice(&b);
}

impl Db2Host {
    fn inputs(m: &Machine, call: &ExecCall, range: std::ops::Range<usize>) -> Vec<String> {
        range
            .filter_map(|k| {
                let (a, i) = call.host.get(k)?;
                let vc = call.host_varchar.get(k).copied().unwrap_or(false);
                Some(literal(m, a, vc, i.as_ref()))
            })
            .collect()
    }

    /// Assigns a row to the host variables `range`; SQLCODE -305 when a
    /// NULL has no indicator variable.
    fn assign(
        m: &mut Machine,
        call: &ExecCall,
        range: std::ops::Range<usize>,
        row: &[Value],
    ) -> SqlCa {
        for (j, k) in range.enumerate() {
            let (Some((a, i)), Some(v)) = (call.host.get(k), row.get(j)) else {
                continue;
            };
            let vc = call.host_varchar.get(k).copied().unwrap_or(false);
            if !store(m, a, vc, i.as_ref(), v) {
                return SqlCa {
                    sqlcode: -305,
                    sqlstate: "22002".into(),
                    ..SqlCa::default()
                };
            }
        }
        SqlCa::ok()
    }

    fn query(&self, sql: &str) -> Result<Rows, SqlCa> {
        let t = crate::dialect::translate(sql).unwrap_or_else(|| sql.to_string());
        self.with_conn(|c| c.query(&t))
    }

    /// One row into the host variables (SELECT INTO, VALUES INTO, SET).
    fn single_row(
        &self,
        m: &mut Machine,
        call: &ExecCall,
        query: &str,
        out: std::ops::Range<usize>,
    ) -> SqlCa {
        let r = match self.query(query) {
            Ok(r) => r,
            Err(ca) => return ca,
        };
        if r.sqlca.sqlcode < 0 {
            return r.sqlca;
        }
        match r.rows.len() {
            0 => SqlCa {
                sqlcode: 100,
                sqlstate: "02000".into(),
                ..SqlCa::default()
            },
            1 => {
                let ca = Self::assign(m, call, out, &r.rows[0]);
                if ca.sqlcode == 0 { r.sqlca } else { ca }
            }
            _ => SqlCa {
                sqlcode: -811,
                sqlstate: "21000".into(),
                ..SqlCa::default()
            },
        }
    }

    fn statement(&self, m: &mut Machine, call: &ExecCall) -> Option<SqlCa> {
        let sql = call.sql.clone().unwrap_or_default();
        let words: Vec<String> = sql
            .split_whitespace()
            .map(|w| w.to_ascii_uppercase())
            .collect();
        let w = |k: usize| words.get(k).map(String::as_str).unwrap_or("");
        let n = call.host.len();
        Some(match w(0) {
            // declarations and precompiler directives
            "DECLARE" | "INCLUDE" | "WHENEVER" | "BEGIN" | "END" => return None,
            "OPEN" => {
                let name = w(1).to_string();
                let query = match call.arg("QUERY") {
                    Some(ExecArg::Name(q)) => q.clone(),
                    _ => {
                        return Some(SqlCa {
                            sqlcode: -504,
                            sqlstate: "34000".into(),
                            ..SqlCa::default()
                        });
                    }
                };
                let q = bind(&query, &Self::inputs(m, call, 0..n));
                match self.query(&q) {
                    Ok(r) if r.sqlca.sqlcode >= 0 => {
                        self.st.borrow_mut().cursors.insert(
                            name,
                            Cursor {
                                rows: r.rows,
                                next: 0,
                            },
                        );
                        r.sqlca
                    }
                    Ok(r) => r.sqlca,
                    Err(ca) => ca,
                }
            }
            "FETCH" => {
                let name = words[1..]
                    .iter()
                    .find(|x| !matches!(x.as_str(), "NEXT" | "FROM"))
                    .cloned()
                    .unwrap_or_default();
                let row = {
                    let mut st = self.st.borrow_mut();
                    let Some(c) = st.cursors.get_mut(&name) else {
                        return Some(SqlCa {
                            sqlcode: -501,
                            sqlstate: "24501".into(),
                            ..SqlCa::default()
                        });
                    };
                    let r = c.rows.get(c.next).cloned();
                    c.next += 1;
                    r
                };
                match row {
                    Some(r) => Self::assign(m, call, 0..n, &r),
                    None => SqlCa {
                        sqlcode: 100,
                        sqlstate: "02000".into(),
                        ..SqlCa::default()
                    },
                }
            }
            "CLOSE" => {
                if self.st.borrow_mut().cursors.remove(w(1)).is_some() {
                    SqlCa::ok()
                } else {
                    SqlCa {
                        sqlcode: -501,
                        sqlstate: "24501".into(),
                        ..SqlCa::default()
                    }
                }
            }
            "COMMIT" => self.end_uow(true).unwrap_or_else(|e| e),
            "ROLLBACK" => self.end_uow(false).unwrap_or_else(|e| e),
            "SET" if w(1) == "?" => {
                // SET :hv = expression
                let eq = sql.find('=').unwrap_or(sql.len());
                let expr = sql[(eq + 1).min(sql.len())..].trim();
                let q = format!("VALUES ({})", bind(expr, &Self::inputs(m, call, 1..n)));
                self.single_row(m, call, &q, 0..1)
            }
            "SELECT" | "VALUES" | "WITH" => match find_kw(&sql, "INTO", 0) {
                Some(at) => {
                    let end = find_kw(&sql, "FROM", at).unwrap_or(sql.len());
                    let before = count_markers(&sql[..at]);
                    let outs = count_markers(&sql[at..end]);
                    let mut values = Self::inputs(m, call, 0..before);
                    values.extend(Self::inputs(m, call, before + outs..n));
                    let q = format!("{}{}", &sql[..at], &sql[end..]);
                    let q = bind(&q, &values);
                    self.single_row(m, call, &q, before..before + outs)
                }
                None => match self.query(&bind(&sql, &Self::inputs(m, call, 0..n))) {
                    Ok(r) => r.sqlca,
                    Err(ca) => ca,
                },
            },
            _ => {
                let s = bind(&sql, &Self::inputs(m, call, 0..n));
                match crate::dialect::translate(&s) {
                    None => SqlCa::ok(),
                    Some(t) => self.with_conn(|c| c.execute(&t)).unwrap_or_else(|e| e),
                }
            }
        })
    }
}

impl ExecHost for Db2Host {
    fn exec(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        if call.kind != ExecKind::Sql {
            return Err(RunError(format!(
                "{}: EXEC {:?} {} needs {} (line {})",
                call.program,
                call.kind,
                call.command,
                if call.kind == ExecKind::Cics {
                    "CICS"
                } else {
                    "IMS"
                },
                call.line
            )));
        }
        if let Some(ca) = self.statement(m, call)
            && let Some(ExecArg::Area(a)) = call.arg("SQLCA")
        {
            set_sqlca(m, a, &ca);
        }
        Ok(ExecOutcome::Continue)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_and_into() {
        assert_eq!(
            bind("SELECT A FROM T WHERE B = ? AND C = '?'", &["'X'".into()]),
            "SELECT A FROM T WHERE B = 'X' AND C = '?'"
        );
        let s = "SELECT A, (SELECT X FROM U) INTO ?, ? FROM T WHERE K = ?";
        let at = find_kw(s, "INTO", 0).unwrap();
        assert_eq!(&s[at..at + 4], "INTO");
        let end = find_kw(s, "FROM", at).unwrap();
        assert_eq!(count_markers(&s[at..end]), 2);
        assert_eq!(quote("O'Hara"), "'O''Hara'");
    }
}

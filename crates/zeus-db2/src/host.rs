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
use zeus_cobol::zir::{DataType, ExecKind, NumType, NumUsage, SignPos};
use zeus_decimal::Dec;
use zeus_drda::sql::{Column, Rows, SqlCa, Value};

/// The database server: Db2 (DRDA) or PostgreSQL.
#[derive(Clone)]
pub enum Server {
    Db2(zeus_drda::Params),
    Postgres(zeus_pg::Params),
}

impl Server {
    /// `db2://user:pw@host:port/DB` or `postgres://user:pw@host:port/db`.
    pub fn from_url(url: &str) -> Result<Server, String> {
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            zeus_pg::Params::from_url(url).map(Server::Postgres)
        } else {
            zeus_drda::Params::from_url(url)
                .map(Server::Db2)
                .map_err(|e| e.0)
        }
    }
}

/// A connection to the server.
enum Connection {
    Db2(zeus_drda::Connection),
    Postgres(crate::pg::Session),
}

impl Connection {
    fn connect(s: &Server, schema: Option<&str>) -> Result<Connection, String> {
        Ok(match s {
            Server::Db2(p) => {
                let mut c = zeus_drda::Connection::connect(p).map_err(|e| e.0)?;
                if let Some(s) = schema {
                    c.execute(&format!("SET CURRENT SCHEMA = '{s}'"))
                        .map_err(|e| e.0)?;
                }
                Connection::Db2(c)
            }
            Server::Postgres(p) => {
                let mut c = crate::pg::Session::connect(p)?;
                if let Some(s) = schema {
                    c.set_schema(s)?;
                }
                Connection::Postgres(c)
            }
        })
    }

    fn execute(&mut self, sql: &str) -> Result<SqlCa, String> {
        match self {
            Connection::Db2(c) => c.execute(sql).map_err(|e| e.0),
            Connection::Postgres(c) => c.execute(sql),
        }
    }

    fn query(&mut self, sql: &str) -> Result<Rows, String> {
        match self {
            Connection::Db2(c) => c.query(sql).map_err(|e| e.0),
            Connection::Postgres(c) => c.query(sql),
        }
    }

    /// Whether positioned updates need cursors of the server.
    fn server_cursors(&self) -> bool {
        matches!(self, Connection::Postgres(_))
    }

    fn end(&mut self, commit: bool) -> Result<SqlCa, String> {
        match self {
            Connection::Db2(c) if commit => c.commit().map_err(|e| e.0),
            Connection::Db2(c) => c.rollback().map_err(|e| e.0),
            Connection::Postgres(c) => c.end(commit),
        }
    }
}

struct Cursor {
    rows: Vec<Vec<Value>>,
    next: usize,
    /// The rows come from a cursor of the server, one FETCH at a time
    /// (a cursor FOR UPDATE on PostgreSQL, for WHERE CURRENT OF).
    server: bool,
}

struct State {
    params: Option<Server>,
    schema: Option<String>,
    conn: Option<Connection>,
    cursors: HashMap<String, Cursor>,
    /// Prepared statements (PREPARE): their text.
    prepared: HashMap<String, String>,
}

/// The Db2 interface of a run unit or a CICS region.
pub struct Db2Host {
    st: RefCell<State>,
}

impl Db2Host {
    /// A host for the server named by ZEUS_DB2 (`db2://user:pw@host:port/DB`,
    /// or `postgres://user:pw@host:port/db` for PostgreSQL) with the
    /// qualifier ZEUS_DB2_SCHEMA; None when ZEUS_DB2 is not set.
    pub fn from_env() -> Option<Db2Host> {
        let url = std::env::var("ZEUS_DB2").ok()?;
        let params = Server::from_url(&url).ok();
        Some(Db2Host::new(params, std::env::var("ZEUS_DB2_SCHEMA").ok()))
    }

    pub fn new(params: Option<Server>, schema: Option<String>) -> Db2Host {
        Db2Host {
            st: RefCell::new(State {
                params,
                schema,
                conn: None,
                cursors: HashMap::new(),
                prepared: HashMap::new(),
            }),
        }
    }

    fn with_conn<T>(
        &self,
        f: impl FnOnce(&mut Connection) -> Result<T, String>,
    ) -> Result<T, SqlCa> {
        let mut st = self.st.borrow_mut();
        if st.conn.is_none() {
            let Some(p) = st.params.clone() else {
                return Err(comm_error("no Db2 server configured (ZEUS_DB2)"));
            };
            let c = Connection::connect(&p, st.schema.as_deref()).map_err(|e| comm_error(&e))?;
            st.conn = Some(c);
        }
        let c = st.conn.as_mut().unwrap();
        let r = f(c).map_err(|e| comm_error(&e));
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
        self.with_conn(|c| c.end(commit))
    }

    /// Runs one SQL statement of a script (DSNTIAD), translated for the
    /// server; Ok(None) when the statement was skipped.
    pub fn run_script_statement(&self, sql: &str) -> Result<Option<SqlCa>, SqlCa> {
        self.run_script_query(sql, &mut Vec::new())
    }

    /// As [`Db2Host::run_script_statement`]; the rows of a query are added
    /// to `rows` as text (DSNTEP2 output).
    /// The rows of a query, with their column descriptions (utilities:
    /// DSNTIAUL).
    pub fn select(&self, sql: &str) -> Result<Rows, SqlCa> {
        let r = self.query(sql)?;
        if r.sqlca.sqlcode < 0 {
            return Err(r.sqlca);
        }
        Ok(r)
    }

    /// Runs a statement in the unit of work (utilities: LOAD).
    pub fn execute_sql(&self, sql: &str) -> SqlCa {
        match crate::dialect::translate(sql) {
            None => SqlCa::ok(),
            Some(t) => self.with_conn(|c| c.execute(&t)).unwrap_or_else(|e| e),
        }
    }

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
        // COMP-1 / COMP-2, PL/I FLOAT: hexadecimal floating point
        DataType::Float { .. } => format!("{:E}", zeus_decimal::hfp::decode(b)),
        // bytes (PL/I BIT): a binary string
        DataType::Group => {
            let h: String = b.iter().map(|x| format!("{x:02X}")).collect();
            format!("X'{h}'")
        }
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
            let mut t = *t;
            // Db2 uses the complete binary host-variable storage width,
            // independently of the COBOL PICTURE and TRUNC option.
            if t.usage == NumUsage::Binary {
                t.usage = NumUsage::Native;
            }
            data::store_num(&t, mem, d, false);
        }
        DataType::Float { .. } => {
            let f: f64 = text.trim().parse().unwrap_or(0.0);
            zeus_decimal::hfp::encode(f, mem);
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

/// Original string length when retrieval truncates a character/binary
/// host variable. Graphic indicators count two-byte code units.
fn truncated_length(area: &Area, varchar: bool, value: &Value) -> Option<usize> {
    let capacity = area.len.saturating_sub(if varchar { 2 } else { 0 });
    let (bytes, units) = match value {
        Value::Bytes(b) => (b.len(), b.len()),
        Value::Str(text) => match area.ty {
            DataType::Numeric(_) | DataType::Float { .. } => return None,
            DataType::National => {
                let n = text.encode_utf16().count();
                (n * 2, n)
            }
            DataType::Dbcs => {
                let n = zeus_cobol::codepage::active().encode_dbcs(text).len();
                (n, n / 2)
            }
            _ => {
                let n = ebcdic_of(text).len();
                (n, n)
            }
        },
        _ => return None,
    };
    (bytes > capacity).then_some(units)
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

fn num(usage: NumUsage, digits: u32, scale: i32) -> DataType {
    DataType::Numeric(NumType {
        usage,
        digits,
        scale,
        signed: true,
        sign: SignPos::Trailing,
    })
}

/// SQLTYPE and SQLLEN of a result column (in an SQLDA).
fn sqltype(c: &Column) -> (i16, i16) {
    let (t, l): (i16, i16) = match c.drda_type {
        0x02 => (496, 4),
        0x04 => (500, 2),
        0x16 => (492, 8),
        0x0E => (484, c.len as i16),
        0x0A => (480, 8),
        0x0C => (480, 4),
        0x20 => (384, 10),
        0x22 => (388, 8),
        0x24 => (392, 26),
        0x30 | 0x3C | 0x26 => (452, c.len as i16),
        _ => (448, c.len.min(32704) as i16),
    };
    (t + i16::from(c.nullable), l)
}

const SQLDA_HEADER: usize = 16;
const SQLVAR_LEN: usize = 44;

fn half(b: &[u8], at: usize) -> i16 {
    i16::from_be_bytes([b[at], b[at + 1]])
}

fn word(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The host variables an SQLDA describes: SQLDATA and SQLIND of its first
/// SQLD entries, typed by SQLTYPE and SQLLEN.
fn sqlda_vars(m: &Machine, a: &Area) -> (Vec<(Area, Option<Area>)>, Vec<bool>) {
    let st = m.storage();
    let base = a.addr;
    let mut host = Vec::new();
    let mut vc = Vec::new();
    if base + SQLDA_HEADER > st.len() {
        return (host, vc);
    }
    let n = half(st, base + 12).max(0) as usize;
    let d = half(st, base + 14).max(0) as usize;
    let null = |p: u32| p == 0 || p == 0xFF00_0000;
    for k in 0..n.min(d) {
        let v = base + SQLDA_HEADER + SQLVAR_LEN * k;
        if v + SQLVAR_LEN > st.len() {
            break;
        }
        let ty = half(st, v);
        let len = half(st, v + 2) as u16 as usize;
        let (data, ind) = (word(st, v + 4), word(st, v + 8));
        let (dt, l, varchar) = match ty & !1 {
            448 => (DataType::Alphanumeric { just: false }, len + 2, true),
            484 => {
                let (p, q) = (len >> 8, len & 0xFF);
                (num(NumUsage::Packed, p as u32, q as i32), p / 2 + 1, false)
            }
            496 => (num(NumUsage::Native, 9, 0), 4, false),
            500 => (num(NumUsage::Native, 4, 0), 2, false),
            492 => (num(NumUsage::Native, 18, 0), 8, false),
            480 => (DataType::Float { long: len == 8 }, len, false),
            468 => (DataType::Dbcs, 2 * len, false),
            _ => (DataType::Alphanumeric { just: false }, len, false),
        };
        let area = if null(data) {
            Area {
                addr: 0,
                len: 0,
                ty: dt,
            }
        } else {
            Area {
                addr: data as usize,
                len: l,
                ty: dt,
            }
        };
        let ind = (ty & 1 == 1 && !null(ind)).then(|| Area {
            addr: ind as usize,
            len: 2,
            ty: num(NumUsage::Native, 4, 0),
        });
        host.push((area, ind));
        vc.push(varchar);
    }
    (host, vc)
}

/// Describes result columns in an SQLDA (DESCRIBE, PREPARE INTO): SQLD
/// and, when SQLN is large enough, the SQLTYPE, SQLLEN and SQLNAME of each.
fn write_sqlda(m: &mut Machine, a: &Area, cols: &[Column]) -> SqlCa {
    let mem = m.storage_mut();
    let base = a.addr;
    if base + SQLDA_HEADER > mem.len() {
        return SqlCa::ok();
    }
    let n = half(mem, base + 12).max(0) as usize;
    mem[base..base + 8].copy_from_slice(&ebcdic::encode("SQLDA   "));
    mem[base + 8..base + 12]
        .copy_from_slice(&((SQLDA_HEADER + SQLVAR_LEN * n) as i32).to_be_bytes());
    mem[base + 14..base + 16].copy_from_slice(&(cols.len() as i16).to_be_bytes());
    if n < cols.len() {
        // SQLN too small: only SQLD is set
        return SqlCa {
            sqlcode: 236,
            sqlstate: "01005".into(),
            ..SqlCa::default()
        };
    }
    for (k, c) in cols.iter().enumerate() {
        let v = base + SQLDA_HEADER + SQLVAR_LEN * k;
        if v + SQLVAR_LEN > mem.len() {
            break;
        }
        let (t, l) = sqltype(c);
        mem[v..v + 2].copy_from_slice(&t.to_be_bytes());
        mem[v + 2..v + 4].copy_from_slice(&l.to_be_bytes());
        let mut name = ebcdic::encode(&c.name);
        name.truncate(30);
        mem[v + 12..v + 14].copy_from_slice(&(name.len() as i16).to_be_bytes());
        name.resize(30, ebcdic::SPACE);
        mem[v + 14..v + 44].copy_from_slice(&name);
    }
    SqlCa::ok()
}

/// The text of a host variable (a statement string).
fn host_text(m: &Machine, a: &Area, varchar: bool) -> String {
    let b = &m.storage()[a.addr..a.addr + a.len];
    if varchar && b.len() >= 2 {
        let n = (u16::from_be_bytes([b[0], b[1]]) as usize).min(b.len() - 2);
        return text_of(&b[2..2 + n]);
    }
    text_of(b).trim_end().to_string()
}

impl Db2Host {
    /// The statement string after keyword `kw` (FROM of PREPARE, IMMEDIATE
    /// of EXECUTE IMMEDIATE): a host variable or a string constant.
    fn source_text(m: &Machine, call: &ExecCall, sql: &str, kw: &str) -> Option<String> {
        let at = find_kw(sql, kw, 0)?;
        let rest = sql[at + kw.len()..].trim();
        if rest.starts_with('?') {
            let k = count_markers(&sql[..at]);
            let (a, _) = call.host.get(k)?;
            let vc = call.host_varchar.get(k).copied().unwrap_or(false);
            return Some(host_text(m, a, vc));
        }
        let t = rest.strip_prefix('\'')?.strip_suffix('\'')?;
        Some(t.replace("''", "'"))
    }

    /// Runs a statement text with these host variables (EXECUTE).
    fn run_text(
        &self,
        m: &mut Machine,
        call: &ExecCall,
        text: String,
        host: (Vec<(Area, Option<Area>)>, Vec<bool>),
    ) -> SqlCa {
        let mut c = call.clone();
        c.command = text
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        c.sql = Some(text);
        c.host = host.0;
        c.host_varchar = host.1;
        self.statement(m, &c).unwrap_or_else(SqlCa::ok)
    }

    /// The columns of a query's result.
    fn describe(&self, text: &str) -> Result<Vec<Column>, SqlCa> {
        // parameter markers: NULL
        let r = self.query(&bind(text, &[]))?;
        if r.sqlca.sqlcode < 0 {
            return Err(r.sqlca);
        }
        Ok(r.columns)
    }

    fn not_prepared(name: &str) -> SqlCa {
        SqlCa {
            sqlcode: -514,
            sqlstate: "26501".into(),
            tokens: vec![name.to_string()],
            ..SqlCa::default()
        }
    }

    /// Dynamic SQL: PREPARE, EXECUTE (IMMEDIATE), DESCRIBE, and OPEN /
    /// FETCH of a prepared query or with an SQLDA (USING DESCRIPTOR).
    fn dynamic(&self, m: &mut Machine, call: &ExecCall, sql: &str) -> Option<SqlCa> {
        let words: Vec<String> = sql
            .split_whitespace()
            .map(|w| w.to_ascii_uppercase())
            .collect();
        let w = |k: usize| words.get(k).map(String::as_str).unwrap_or("");
        let descriptor = find_kw(sql, "DESCRIPTOR", 0);
        let sqlda = || {
            let k = count_markers(&sql[..descriptor.unwrap_or(0)]);
            call.host.get(k).map(|(a, _)| sqlda_vars(m, a))
        };
        Some(match w(0) {
            "PREPARE" => {
                let Some(text) = Self::source_text(m, call, sql, "FROM") else {
                    return Some(SqlCa {
                        sqlcode: -104,
                        sqlstate: "42601".into(),
                        ..SqlCa::default()
                    });
                };
                let name = w(1).to_string();
                self.st
                    .borrow_mut()
                    .prepared
                    .insert(name, text.trim().to_string());
                // PREPARE ... INTO :sqlda: also described
                if find_kw(sql, "INTO", 0).is_some()
                    && let Some((a, _)) = call.host.first()
                {
                    let a = a.clone();
                    return Some(match self.describe(text.trim()) {
                        Ok(cols) => write_sqlda(m, &a, &cols),
                        Err(ca) => ca,
                    });
                }
                SqlCa::ok()
            }
            "EXECUTE" if w(1) == "IMMEDIATE" => {
                match Self::source_text(m, call, sql, "IMMEDIATE") {
                    Some(text) => self.run_text(m, call, text, (Vec::new(), Vec::new())),
                    None => SqlCa {
                        sqlcode: -104,
                        sqlstate: "42601".into(),
                        ..SqlCa::default()
                    },
                }
            }
            "EXECUTE" => {
                let name = w(1);
                let Some(text) = self.st.borrow().prepared.get(name).cloned() else {
                    return Some(Self::not_prepared(name));
                };
                let host = if descriptor.is_some() {
                    sqlda().unwrap_or_default()
                } else {
                    (call.host.clone(), call.host_varchar.clone())
                };
                self.run_text(m, call, text, host)
            }
            "DESCRIBE" => {
                let name = if w(1) == "OUTPUT" { w(2) } else { w(1) };
                let Some(text) = self.st.borrow().prepared.get(name).cloned() else {
                    return Some(Self::not_prepared(name));
                };
                let Some((a, _)) = call.host.last() else {
                    return Some(SqlCa::ok());
                };
                let a = a.clone();
                match self.describe(&text) {
                    Ok(cols) => write_sqlda(m, &a, &cols),
                    Err(ca) => ca,
                }
            }
            "OPEN" => {
                // a cursor for a prepared statement: its text, the USING
                // values (or the SQLDA's)
                let query = match call.arg("QUERY") {
                    Some(ExecArg::Name(q)) => q.trim().to_ascii_uppercase(),
                    _ => return None,
                };
                let prepared = self.st.borrow().prepared.get(&query).cloned();
                if prepared.is_none() && descriptor.is_none() {
                    return None;
                }
                let mut c = call.clone();
                if let Some(text) = prepared {
                    c.options.retain(|(n, _)| n != "QUERY");
                    c.options
                        .push(("QUERY".to_string(), Some(ExecArg::Name(text))));
                } else if !query.contains(' ') {
                    return Some(Self::not_prepared(&query));
                }
                if descriptor.is_some() {
                    let (h, v) = sqlda().unwrap_or_default();
                    c.host = h;
                    c.host_varchar = v;
                }
                let words = sql.split_whitespace().take(2).collect::<Vec<_>>().join(" ");
                c.sql = Some(words);
                self.statement(m, &c)?
            }
            "FETCH" if descriptor.is_some() => {
                let (h, v) = sqlda().unwrap_or_default();
                let mut c = call.clone();
                c.host = h;
                c.host_varchar = v;
                let at = find_kw(sql, "USING", 0).unwrap_or(sql.len());
                c.sql = Some(sql[..at].to_string());
                self.statement(m, &c)?
            }
            _ => return None,
        })
    }

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
        let mut ca = SqlCa::ok();
        let mut warn = [b' '; 11];
        if row.len() > range.len() {
            warn[0] = b'W';
            warn[3] = b'W';
        }
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
            if let Some(length) = truncated_length(a, vc, v) {
                warn[0] = b'W';
                warn[1] = b'W';
                ca.sqlstate = "01004".into();
                if let Some(indicator) = i {
                    store_int(m, indicator, length as i64);
                }
            }
        }
        ca.warn = String::from_utf8(warn.to_vec()).unwrap();
        ca
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
                if ca.sqlcode != 0 {
                    return ca;
                }
                let mut result = r.sqlca;
                let mut warn = result.warn.into_bytes();
                warn.resize(11, b' ');
                for (i, c) in ca.warn.bytes().enumerate() {
                    if c != b' ' {
                        warn[i] = c;
                    }
                }
                result.warn = String::from_utf8(warn).unwrap();
                if ca.sqlstate != "00000" {
                    result.sqlstate = ca.sqlstate;
                }
                result
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
        if let Some(ca) = self.dynamic(m, call, &sql) {
            return Some(ca);
        }
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
                // a cursor FOR UPDATE stays on the server where positioned
                // updates need it
                let server = find_kw(&q, "FOR", 0).is_some_and(|at| {
                    q[at..].to_ascii_uppercase().split_whitespace().nth(1) == Some("UPDATE")
                }) && self.with_conn(|c| Ok(c.server_cursors())).unwrap_or(false);
                if server {
                    if self
                        .st
                        .borrow_mut()
                        .cursors
                        .remove(&name)
                        .is_some_and(|c| c.server)
                    {
                        let _ = self.with_conn(|c| c.execute(&format!("CLOSE {name}")));
                    }
                    let t = crate::dialect::translate(&q).unwrap_or(q);
                    return Some(
                        match self
                            .with_conn(|c| c.execute(&format!("DECLARE {name} CURSOR FOR {t}")))
                        {
                            Ok(ca) if ca.sqlcode >= 0 => {
                                self.st.borrow_mut().cursors.insert(
                                    name,
                                    Cursor {
                                        rows: Vec::new(),
                                        next: 0,
                                        server: true,
                                    },
                                );
                                SqlCa::ok()
                            }
                            Ok(ca) | Err(ca) => ca,
                        },
                    );
                }
                match self.query(&q) {
                    Ok(r) if r.sqlca.sqlcode >= 0 => {
                        self.st.borrow_mut().cursors.insert(
                            name,
                            Cursor {
                                rows: r.rows,
                                next: 0,
                                server: false,
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
                let server = match self.st.borrow().cursors.get(&name) {
                    None => {
                        return Some(SqlCa {
                            sqlcode: -501,
                            sqlstate: "24501".into(),
                            ..SqlCa::default()
                        });
                    }
                    Some(c) => c.server,
                };
                let row = if server {
                    match self.with_conn(|c| c.query(&format!("FETCH NEXT FROM {name}"))) {
                        Ok(r) if r.sqlca.sqlcode < 0 => return Some(r.sqlca),
                        Ok(r) => r.rows.into_iter().next(),
                        Err(ca) => return Some(ca),
                    }
                } else {
                    let mut st = self.st.borrow_mut();
                    let c = st.cursors.get_mut(&name).unwrap();
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
                let closed = self.st.borrow_mut().cursors.remove(w(1));
                if let Some(c) = closed {
                    if c.server {
                        let _ = self.with_conn(|cn| cn.execute(&format!("CLOSE {}", w(1))));
                    }
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
        let ca = self.statement(m, call);
        // ZEUS_DB2_TRACE: each statement and its result on standard error
        if std::env::var_os("ZEUS_DB2_TRACE").is_some()
            && let Some(ca) = &ca
        {
            eprintln!(
                "ZEUS DB2: {} line {}: {} -> SQLCODE {} {}",
                call.program,
                call.line,
                call.sql
                    .as_deref()
                    .unwrap_or("")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" "),
                ca.sqlcode,
                ca.tokens.join(" ")
            );
        }
        let outcome = ca
            .as_ref()
            .map_or(ExecOutcome::Continue, |ca| whenever_outcome(call, ca));
        if let Some(ca) = ca
            && let Some(ExecArg::Area(a)) = call.arg("SQLCA")
        {
            set_sqlca(m, a, &ca);
        }
        Ok(outcome)
    }
}

fn whenever_outcome(call: &ExecCall, ca: &SqlCa) -> ExecOutcome {
    let condition = if ca.sqlcode < 0 {
        "SQLERROR"
    } else if ca.sqlcode == 100 {
        "NOTFOUND"
    } else if ca.sqlcode > 0 || ca.warn.starts_with('W') {
        "SQLWARNING"
    } else {
        return ExecOutcome::Continue;
    };
    match call.arg(condition) {
        Some(ExecArg::Label(label)) => ExecOutcome::Goto(*label),
        _ => ExecOutcome::Continue,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_retrieval_uses_storage_width_instead_of_picture_digits() {
        let mut machine = Machine::new(vec![]);
        let addr = machine.getmain(2, 0);
        let area = Area {
            addr,
            len: 2,
            ty: num(NumUsage::Binary, 4, 0),
        };
        for value in [12345, -12345, 32767, -32768] {
            assert!(store(&mut machine, &area, false, None, &Value::Int(value)));
            assert_eq!(
                i16::from_be_bytes(machine.storage()[addr..addr + 2].try_into().unwrap()),
                value as i16
            );
        }
    }

    #[test]
    fn retrieval_truncation_sets_warning_and_original_length() {
        let mut machine = Machine::new(vec![]);
        let addr = machine.getmain(8, ebcdic::SPACE);
        let indicator = Area {
            addr: addr + 6,
            len: 2,
            ty: DataType::Group,
        };
        let mut call = ExecCall {
            kind: ExecKind::Sql,
            command: "FETCH".into(),
            line: 1,
            program: "T".into(),
            instance: 0,
            sql: None,
            options: vec![],
            host: vec![(
                Area {
                    addr,
                    len: 4,
                    ty: DataType::Group,
                },
                Some(indicator.clone()),
            )],
            host_varchar: vec![true],
        };
        let ca = Db2Host::assign(&mut machine, &call, 0..1, &[Value::Str("ABCDE".into())]);
        assert_eq!(ca.sqlcode, 0);
        assert_eq!(ca.sqlstate, "01004");
        assert!(ca.warn.starts_with("WW"));
        assert_eq!(
            &machine.storage()[addr..addr + 4],
            &[0, 2, ebcdic::from_latin1(b'A'), ebcdic::from_latin1(b'B')]
        );
        assert_eq!(&machine.storage()[addr + 6..addr + 8], &[0, 5]);
        call.host_varchar[0] = false;
        call.host[0].0.len = 2;
        let ca = Db2Host::assign(&mut machine, &call, 0..1, &[Value::Str("Z".into())]);
        assert_eq!(ca.sqlstate, "00000");
        assert!(!ca.warn.contains('W'));
        assert_eq!(&machine.storage()[addr + 6..addr + 8], &[0, 0]);
        let original = machine.storage()[addr..addr + 2].to_vec();
        let ca = Db2Host::assign(&mut machine, &call, 0..1, &[Value::Null]);
        assert_eq!(ca.sqlcode, 0);
        assert_eq!(&machine.storage()[addr + 6..addr + 8], &[255, 255]);
        assert_eq!(&machine.storage()[addr..addr + 2], original);
        call.host[0].1 = None;
        let ca = Db2Host::assign(&mut machine, &call, 0..1, &[Value::Null]);
        assert_eq!(ca.sqlcode, -305);
    }

    #[test]
    fn whenever_uses_sqlcode_and_warning_flag() {
        let mut call = ExecCall {
            kind: ExecKind::Sql,
            command: "SELECT".into(),
            line: 1,
            program: "T".into(),
            instance: 0,
            sql: None,
            host: vec![],
            host_varchar: vec![],
            options: vec![
                ("SQLERROR".into(), Some(ExecArg::Label(10))),
                ("NOTFOUND".into(), Some(ExecArg::Label(20))),
                ("SQLWARNING".into(), Some(ExecArg::Label(30))),
            ],
        };
        for (code, warn, expected) in [
            (0, "", ExecOutcome::Continue),
            (0, " W", ExecOutcome::Continue),
            (0, "W", ExecOutcome::Goto(30)),
            (1, "", ExecOutcome::Goto(30)),
            (100, "W", ExecOutcome::Goto(20)),
            (-811, "W", ExecOutcome::Goto(10)),
        ] {
            let ca = SqlCa {
                sqlcode: code,
                warn: warn.into(),
                ..SqlCa::ok()
            };
            assert_eq!(whenever_outcome(&call, &ca), expected);
        }
        call.options.clear();
        assert_eq!(
            whenever_outcome(
                &call,
                &SqlCa {
                    sqlcode: -501,
                    ..SqlCa::ok()
                }
            ),
            ExecOutcome::Continue
        );
    }

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

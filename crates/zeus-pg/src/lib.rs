//! A client of the PostgreSQL frontend/backend protocol (version 3), as
//! the PostgreSQL documentation describes it: startup, password and
//! SCRAM-SHA-256 authentication, and the simple query protocol with
//! results in text form. Zeus uses it to run the embedded SQL of COBOL
//! and PL/I programs on PostgreSQL instead of Db2.

pub mod scram;

use std::io::{Read, Write};
use std::net::TcpStream;

/// An error of the server (ErrorResponse): its SQLSTATE and messages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServerError {
    pub severity: String,
    pub sqlstate: String,
    pub message: String,
    pub detail: Option<String>,
    pub table: Option<String>,
    pub column: Option<String>,
    pub constraint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The connection failed (or the server ended it).
    Io(String),
    Server(Box<ServerError>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "{e}"),
            Error::Server(e) => write!(f, "{} {}", e.sqlstate, e.message),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

fn io(e: std::io::Error) -> Error {
    Error::Io(e.to_string())
}

/// A column of a result: its name and type (pg_type OID).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Column {
    pub name: String,
    pub type_oid: u32,
    /// The type modifier (`atttypmod`): -1, or for VARCHAR / CHAR the length
    /// + 4, for NUMERIC ((precision << 16) | scale) + 4.
    pub type_mod: i32,
    /// The table and column number of a table's column (0 otherwise).
    pub table_oid: u32,
    pub attnum: i16,
}

/// Type OIDs of the built-in types.
pub mod oid {
    pub const BOOL: u32 = 16;
    pub const BYTEA: u32 = 17;
    pub const INT8: u32 = 20;
    pub const INT2: u32 = 21;
    pub const INT4: u32 = 23;
    pub const TEXT: u32 = 25;
    pub const FLOAT4: u32 = 700;
    pub const FLOAT8: u32 = 701;
    pub const BPCHAR: u32 = 1042;
    pub const VARCHAR: u32 = 1043;
    pub const DATE: u32 = 1082;
    pub const TIME: u32 = 1083;
    pub const TIMESTAMP: u32 = 1114;
    pub const TIMESTAMPTZ: u32 = 1184;
    pub const NUMERIC: u32 = 1700;
}

/// The result of one statement: its rows (text values, None: NULL) and
/// command tag (`INSERT 0 1`, `UPDATE 3`, `SELECT 2`, ...).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Statement {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Option<String>>>,
    pub tag: String,
}

impl Statement {
    /// The row count of the command tag.
    pub fn count(&self) -> Option<u64> {
        self.tag.rsplit(' ').next()?.parse().ok()
    }
}

/// Connection parameters: `postgres://user:password@host:port/database`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Params {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
}

impl Params {
    pub fn from_url(url: &str) -> std::result::Result<Params, String> {
        let rest = url
            .strip_prefix("postgres://")
            .or_else(|| url.strip_prefix("postgresql://"))
            .ok_or_else(|| format!("{url}: not a postgres:// URL"))?;
        let (auth, rest) = rest.rsplit_once('@').unwrap_or(("", rest));
        let (user, password) = auth.split_once(':').unwrap_or((auth, ""));
        let (hostport, database) = rest.split_once('/').unwrap_or((rest, ""));
        let (host, port) = match hostport.rsplit_once(':') {
            Some((h, p)) => (
                h,
                p.parse()
                    .map_err(|_| format!("{url}: the port is not a number"))?,
            ),
            None => (hostport, 5432),
        };
        let user = if user.is_empty() { "postgres" } else { user };
        Ok(Params {
            host: if host.is_empty() { "localhost" } else { host }.to_string(),
            port,
            user: user.to_string(),
            password: password.to_string(),
            database: if database.is_empty() { user } else { database }.to_string(),
        })
    }
}

pub struct Connection {
    stream: TcpStream,
    /// The transaction status of the last ReadyForQuery: I (idle), T (in a
    /// transaction block), E (in a failed transaction block).
    pub status: u8,
}

/// A message of the server: its type and body.
struct Message {
    kind: u8,
    body: Vec<u8>,
}

fn cstr(b: &[u8], at: &mut usize) -> String {
    let start = (*at).min(b.len());
    let end = b[start..]
        .iter()
        .position(|&c| c == 0)
        .map_or(b.len(), |p| start + p);
    *at = end + 1;
    String::from_utf8_lossy(&b[start..end]).into_owned()
}

fn int16(b: &[u8], at: &mut usize) -> i16 {
    let v = b
        .get(*at..*at + 2)
        .map_or(0, |x| i16::from_be_bytes([x[0], x[1]]));
    *at += 2;
    v
}

fn int32(b: &[u8], at: &mut usize) -> i32 {
    let v = b
        .get(*at..*at + 4)
        .map_or(0, |x| i32::from_be_bytes([x[0], x[1], x[2], x[3]]));
    *at += 4;
    v
}

fn server_error(body: &[u8]) -> ServerError {
    let mut e = ServerError::default();
    let mut at = 0;
    while at < body.len() && body[at] != 0 {
        let code = body[at];
        at += 1;
        let v = cstr(body, &mut at);
        match code {
            b'S' => e.severity = v,
            b'C' => e.sqlstate = v,
            b'M' => e.message = v,
            b'D' => e.detail = Some(v),
            b't' => e.table = Some(v),
            b'c' => e.column = Some(v),
            b'n' => e.constraint = Some(v),
            _ => {}
        }
    }
    e
}

/// Random characters for the SCRAM nonce.
fn nonce() -> String {
    let mut b = [0u8; 18];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .is_err()
    {
        let t = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        for (k, x) in b.iter_mut().enumerate() {
            *x = (t >> (k % 16 * 8)) as u8 ^ (k as u8).wrapping_mul(37);
        }
    }
    scram::base64(&b)
}

impl Connection {
    /// Connects and authenticates; the session uses UTF-8 and ISO dates.
    pub fn connect(p: &Params) -> Result<Connection> {
        let stream = TcpStream::connect((p.host.as_str(), p.port))
            .map_err(|e| Error::Io(format!("{}:{}: {e}", p.host, p.port)))?;
        let _ = stream.set_nodelay(true);
        let mut c = Connection {
            stream,
            status: b'I',
        };
        let mut body = 196608i32.to_be_bytes().to_vec();
        for (k, v) in [
            ("user", p.user.as_str()),
            ("database", p.database.as_str()),
            ("client_encoding", "UTF8"),
            ("DateStyle", "ISO"),
            ("application_name", "zeus"),
        ] {
            body.extend(k.as_bytes());
            body.push(0);
            body.extend(v.as_bytes());
            body.push(0);
        }
        body.push(0);
        let mut m = ((body.len() + 4) as i32).to_be_bytes().to_vec();
        m.extend(body);
        c.stream.write_all(&m).map_err(io)?;
        let mut scram: Option<scram::Scram> = None;
        loop {
            let msg = c.read()?;
            match msg.kind {
                b'R' => {
                    let mut at = 0;
                    match int32(&msg.body, &mut at) {
                        0 => {}
                        3 => {
                            let mut pw = p.password.as_bytes().to_vec();
                            pw.push(0);
                            c.send(b'p', &pw)?;
                        }
                        10 => {
                            let mut mechanisms = Vec::new();
                            while at < msg.body.len() && msg.body[at] != 0 {
                                mechanisms.push(cstr(&msg.body, &mut at));
                            }
                            if !mechanisms.iter().any(|m| m == "SCRAM-SHA-256") {
                                return Err(Error::Io(format!(
                                    "the server offers {mechanisms:?}, not SCRAM-SHA-256"
                                )));
                            }
                            let s = scram::Scram::new(&p.password, &nonce());
                            let first = s.client_first();
                            let mut b = b"SCRAM-SHA-256\0".to_vec();
                            b.extend((first.len() as i32).to_be_bytes());
                            b.extend(first.as_bytes());
                            c.send(b'p', &b)?;
                            scram = Some(s);
                        }
                        11 => {
                            let s = scram
                                .as_mut()
                                .ok_or(Error::Io("SCRAM out of order".into()))?;
                            let server_first = String::from_utf8_lossy(&msg.body[4..]).into_owned();
                            let fin = s.client_final(&server_first).map_err(Error::Io)?;
                            c.send(b'p', fin.as_bytes())?;
                        }
                        12 => {
                            let s = scram
                                .as_ref()
                                .ok_or(Error::Io("SCRAM out of order".into()))?;
                            s.verify(&String::from_utf8_lossy(&msg.body[4..]))
                                .map_err(Error::Io)?;
                        }
                        n => {
                            return Err(Error::Io(format!(
                                "authentication method {n} is not supported (use scram-sha-256 or password)"
                            )));
                        }
                    }
                }
                b'E' => return Err(Error::Server(Box::new(server_error(&msg.body)))),
                b'Z' => {
                    c.status = msg.body.first().copied().unwrap_or(b'I');
                    return Ok(c);
                }
                // ParameterStatus, BackendKeyData, notices
                _ => {}
            }
        }
    }

    fn send(&mut self, kind: u8, body: &[u8]) -> Result<()> {
        let mut m = vec![kind];
        m.extend(((body.len() + 4) as i32).to_be_bytes());
        m.extend(body);
        self.stream.write_all(&m).map_err(io)
    }

    fn read(&mut self) -> Result<Message> {
        let mut h = [0u8; 5];
        self.stream.read_exact(&mut h).map_err(io)?;
        let len = i32::from_be_bytes([h[1], h[2], h[3], h[4]]).max(4) as usize - 4;
        let mut body = vec![0u8; len];
        self.stream.read_exact(&mut body).map_err(io)?;
        Ok(Message { kind: h[0], body })
    }

    /// Runs the statements of `sql` (simple query protocol): the results
    /// of those that ran, and the error that stopped the others.
    pub fn simple_query(&mut self, sql: &str) -> Result<(Vec<Statement>, Option<ServerError>)> {
        let mut q = sql.as_bytes().to_vec();
        q.push(0);
        self.send(b'Q', &q)?;
        let mut done = Vec::new();
        let mut cur = Statement::default();
        let mut error = None;
        loop {
            let msg = self.read()?;
            let b = &msg.body;
            match msg.kind {
                b'T' => {
                    let mut at = 0;
                    let n = int16(b, &mut at).max(0);
                    cur.columns = (0..n)
                        .map(|_| {
                            let name = cstr(b, &mut at);
                            let table_oid = int32(b, &mut at) as u32;
                            let attnum = int16(b, &mut at);
                            let type_oid = int32(b, &mut at) as u32;
                            at += 2;
                            let type_mod = int32(b, &mut at);
                            at += 2;
                            Column {
                                name,
                                type_oid,
                                type_mod,
                                table_oid,
                                attnum,
                            }
                        })
                        .collect();
                }
                b'D' => {
                    let mut at = 0;
                    let n = int16(b, &mut at).max(0);
                    let row = (0..n)
                        .map(|_| {
                            let len = int32(b, &mut at);
                            if len < 0 {
                                return None;
                            }
                            let end = (at + len as usize).min(b.len());
                            let v = String::from_utf8_lossy(&b[at..end]).into_owned();
                            at = end;
                            Some(v)
                        })
                        .collect();
                    cur.rows.push(row);
                }
                b'C' => {
                    let mut at = 0;
                    cur.tag = cstr(b, &mut at);
                    done.push(std::mem::take(&mut cur));
                }
                b'I' => done.push(std::mem::take(&mut cur)),
                b'E' => error = Some(server_error(b)),
                b'Z' => {
                    self.status = b.first().copied().unwrap_or(b'I');
                    return Ok((done, error));
                }
                // notices, parameter changes, COPY is not used
                _ => {}
            }
        }
    }

    /// Runs one statement; an error of the server is an Err.
    pub fn execute(&mut self, sql: &str) -> Result<Statement> {
        let (mut done, error) = self.simple_query(sql)?;
        match error {
            Some(e) => Err(Error::Server(Box::new(e))),
            None => Ok(done.pop().unwrap_or_default()),
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.send(b'X', &[]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls() {
        let p = Params::from_url("postgres://zeus:pw@db:5433/apps").unwrap();
        assert_eq!(
            (
                p.user.as_str(),
                p.password.as_str(),
                p.host.as_str(),
                p.port,
                p.database.as_str()
            ),
            ("zeus", "pw", "db", 5433, "apps")
        );
        let p = Params::from_url("postgresql://u@h/").unwrap();
        assert_eq!((p.port, p.database.as_str()), (5432, "u"));
        assert!(Params::from_url("db2://u@h/d").is_err());
    }

    #[test]
    fn error_fields() {
        let body = b"SERROR\0C23505\0Mduplicate key\0ncust_pk\0\0";
        let e = server_error(body);
        assert_eq!(e.sqlstate, "23505");
        assert_eq!(e.constraint.as_deref(), Some("cust_pk"));
    }
}

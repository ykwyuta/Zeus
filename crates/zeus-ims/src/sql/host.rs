//! EXEC SQLIMS dynamic statement/cursor state, scoped to an IMS schedule.
use super::{
    SqlError, SqlResult, SqlValue,
    parser::{self, Prepared, Statement},
};
use crate::dli::Session;
use zeus_cobol::{
    ebcdic,
    vm::{
        Machine, RResult, RunError, data,
        exec::{Area, ExecArg, ExecCall, ExecOutcome},
    },
    zir::{DataType, NumType, NumUsage, SignPos},
};
use zeus_decimal::Dec;

type Result<T> = std::result::Result<T, SqlError>;

#[derive(Default)]
pub(crate) struct State {
    pub enabled: bool,
    statement: Option<(String, String, Prepared)>,
    cursor: Option<Cursor>,
}

struct Cursor {
    program: String,
    name: String,
    result: SqlResult,
    next: usize,
}

#[derive(Default)]
struct Status {
    code: i32,
    state: Option<&'static str>,
    rows: usize,
    truncated: bool,
    more_columns: bool,
    unqualified: bool,
}

fn error(state: &'static str, message: impl Into<String>) -> SqlError {
    SqlError::new(state, message)
}

fn bytes<'a>(m: &'a Machine, a: &Area) -> Result<&'a [u8]> {
    m.storage()
        .get(
            a.addr
                ..a.addr
                    .checked_add(a.len)
                    .ok_or_else(|| error("HY090", "invalid host area"))?,
        )
        .ok_or_else(|| error("HY090", "host area outside storage"))
}

fn read_host(m: &Machine, call: &ExecCall, index: usize) -> Result<SqlValue> {
    let (area, indicator) = call
        .host
        .get(index)
        .ok_or_else(|| error("07001", "missing host variable"))?;
    if let Some(indicator) = indicator {
        if indicator.len != 2 || !matches!(indicator.ty, DataType::Numeric(_)) {
            return Err(error("07006", "indicator must be a binary halfword"));
        }
        if i16::from_be_bytes(bytes(m, indicator)?.try_into().unwrap()) < 0 {
            return Ok(SqlValue::Null);
        }
    }
    let value = bytes(m, area)?;
    if call.host_varchar.get(index) == Some(&true) {
        let header = value
            .get(..2)
            .ok_or_else(|| error("HY090", "invalid VARCHAR structure"))?;
        let len = i16::from_be_bytes(header.try_into().unwrap());
        if len < 0 || len as usize > value.len() - 2 {
            return Err(error("22001", "VARCHAR length exceeds host buffer"));
        }
        return Ok(SqlValue::Text(ebcdic::decode(&value[2..2 + len as usize])));
    }
    Ok(match &area.ty {
        DataType::Numeric(t) => SqlValue::Number(data::num_value(t, value)),
        DataType::Alphanumeric { .. } | DataType::Alphabetic { .. } => {
            SqlValue::Text(ebcdic::decode(value).trim_end_matches(' ').into())
        }
        _ => return Err(error("07006", "unsupported SQLIMS host variable category")),
    })
}

fn host_text(m: &Machine, call: &ExecCall, index: usize) -> Result<String> {
    match read_host(m, call, index)? {
        SqlValue::Text(s) => Ok(s),
        _ => Err(error(
            "07006",
            "SQL statement source must be character data",
        )),
    }
}

fn parameters(m: &Machine, call: &ExecCall, prepared: &Prepared) -> Result<Vec<SqlValue>> {
    // IMS ignores USING when the prepared statement has no parameter markers.
    if prepared.parameters == 0 {
        return Ok(Vec::new());
    }
    if call.host.len() != prepared.parameters {
        return Err(error("07001", "USING parameter count mismatch"));
    }
    (0..call.host.len())
        .map(|i| read_host(m, call, i))
        .collect()
}

/// Prepare updates without modifying application storage until every column
/// conversion has succeeded. Truncation is a successful operation with WARN1.
fn fetch_row(
    m: &mut Machine,
    call: &ExecCall,
    row: &[SqlValue],
    columns: &[super::Column],
) -> Result<Status> {
    if call.host.is_empty() {
        return Err(error("07001", "FETCH requires output host variables"));
    }
    if call.host.len() > row.len() {
        return Err(error("07001", "more host variables than result columns"));
    }
    let mut updates = Vec::new();
    let mut status = Status {
        more_columns: row.len() > call.host.len(),
        ..Status::default()
    };
    for (i, value) in row.iter().take(call.host.len()).enumerate() {
        let (area, indicator) = &call.host[i];
        let mut buffer = bytes(m, area)?.to_vec();
        let mut ind: i16 = 0;
        if *value == SqlValue::Null {
            if indicator.is_none() {
                return Err(error("22002", "NULL requires an indicator variable"));
            }
            ind = -1;
        } else if call.has(&format!("BINARY-{i}")) {
            let SqlValue::Binary(value) = value else {
                return Err(error("07006", "binary output requires a binary column"));
            };
            if value.len() > buffer.len() {
                status.truncated = true;
                ind = i16::try_from(value.len())
                    .map_err(|_| error("22001", "binary length overflow"))?;
            }
            buffer.fill(0);
            let count = buffer.len().min(value.len());
            buffer[..count].copy_from_slice(&value[..count]);
        } else if call.host_varchar.get(i) == Some(&true)
            || matches!(
                area.ty,
                DataType::Alphanumeric { .. } | DataType::Alphabetic { .. }
            )
        {
            let text = match value {
                SqlValue::Text(s) => s.clone(),
                SqlValue::Number(d) => d.to_string(),
                _ => return Err(error("07006", "incompatible output host variable")),
            };
            let mut encoded = ebcdic::encode(&text);
            if ebcdic::decode(&encoded) != text {
                return Err(error(
                    "22021",
                    "character not representable in COBOL code page",
                ));
            }
            if matches!(columns[i].sql_type & !1, 384 | 388 | 392 | 452) && columns[i].length >= 0 {
                encoded.resize(encoded.len().max(columns[i].length as usize), ebcdic::SPACE);
            }
            let varchar = call.host_varchar.get(i) == Some(&true);
            let offset = if varchar { 2 } else { 0 };
            if buffer.len() < offset {
                return Err(error("HY090", "invalid VARCHAR area"));
            }
            let capacity = buffer.len() - offset;
            let count = encoded.len().min(capacity);
            if count < encoded.len() {
                status.truncated = true;
                ind = i16::try_from(encoded.len())
                    .map_err(|_| error("22001", "result exceeds indicator length range"))?;
            }
            buffer[offset..].fill(ebcdic::SPACE);
            buffer[offset..offset + count].copy_from_slice(&encoded[..count]);
            if varchar {
                buffer[..2].copy_from_slice(&(count as i16).to_be_bytes());
            }
        } else if let DataType::Numeric(t) = area.ty {
            let d = match value {
                SqlValue::Number(d) => *d,
                SqlValue::Text(s) => {
                    Dec::parse(s).ok_or_else(|| error("22018", "invalid numeric output"))?
                }
                _ => return Err(error("07006", "numeric output required")),
            };
            if data::store_num(&t, &mut buffer, d, false) {
                return Err(error("22003", "numeric host variable overflow"));
            }
        } else if matches!(area.ty, DataType::Float { .. }) {
            let SqlValue::Number(value) = value else {
                return Err(error("07006", "floating output requires a numeric column"));
            };
            zeus_decimal::hfp::encode(value.to_f64(), &mut buffer);
        } else {
            return Err(error("07006", "unsupported output host category"));
        }
        if *value != SqlValue::Null {
            updates.push((area.addr, buffer));
        }
        if let Some(indicator) = indicator {
            bytes(m, indicator)?;
            if indicator.len != 2 {
                return Err(error("07006", "indicator must be a binary halfword"));
            }
            updates.push((indicator.addr, ind.to_be_bytes().to_vec()));
        }
    }
    for (addr, buffer) in updates {
        m.storage_mut()[addr..addr + buffer.len()].copy_from_slice(&buffer);
    }
    Ok(status)
}

fn descriptor_header(m: &Machine, area: &Area) -> Result<(usize, usize)> {
    let data = bytes(m, area)?;
    if data.len() < 16 {
        return Err(error("HY090", "SQLIMSDA header is shorter than 16 bytes"));
    }
    let n = i16::from_be_bytes(data[12..14].try_into().unwrap());
    if n < 0 || 16 + n as usize * 44 > data.len() {
        return Err(error(
            "HY090",
            "SQLIMSN exceeds supplied descriptor storage",
        ));
    }
    Ok((n as usize, 16 + n as usize * 44))
}

fn describe(m: &mut Machine, area: &Area, columns: &[super::Column]) -> Result<Status> {
    let (n, length) = descriptor_header(m, area)?;
    let d = i16::try_from(columns.len()).map_err(|_| error("54011", "too many result columns"))?;
    let mut data = bytes(m, area)?[..length].to_vec();
    data[..8].copy_from_slice(&ebcdic::encode("SQLIMSDA"));
    data[8..12].copy_from_slice(&(length as i32).to_be_bytes());
    data[14..16].copy_from_slice(&d.to_be_bytes());
    for (i, col) in columns.iter().take(n).enumerate() {
        let p = 16 + i * 44;
        data[p..p + 2].copy_from_slice(&col.sql_type.to_be_bytes());
        data[p + 2..p + 4].copy_from_slice(&col.length.to_be_bytes());
        // SQLIMSDATA and SQLIMSIND are application-owned pointer slots.
        let name = ebcdic::encode(&col.name);
        let len = name.len().min(30);
        data[p + 12..p + 14].copy_from_slice(&(len as i16).to_be_bytes());
        data[p + 14..p + 44].fill(ebcdic::SPACE);
        data[p + 14..p + 14 + len].copy_from_slice(&name[..len]);
    }
    m.storage_mut()[area.addr..area.addr + length].copy_from_slice(&data);
    Ok(Status {
        code: if columns.len() > n { 239 } else { 0 },
        state: if columns.len() > n {
            Some("01005")
        } else {
            None
        },
        ..Status::default()
    })
}

fn descriptor_fetch_call(m: &Machine, call: &ExecCall) -> Result<ExecCall> {
    if call.host.len() != 1 {
        return Err(error(
            "07001",
            "FETCH DESCRIPTOR requires one descriptor area",
        ));
    }
    let area = &call.host[0].0;
    let (n, required) = descriptor_header(m, area)?;
    let data = bytes(m, area)?;
    let length = i32::from_be_bytes(data[8..12].try_into().unwrap());
    let d = i16::from_be_bytes(data[14..16].try_into().unwrap());
    if length < required as i32 || d < 0 || d as usize > n {
        return Err(error("HY090", "invalid SQLIMSD or SQLIMSDABC"));
    }
    let mut bound = call.clone();
    bound.host.clear();
    bound.host_varchar.clear();
    for i in 0..d as usize {
        let p = 16 + i * 44;
        let kind = i16::from_be_bytes(data[p..p + 2].try_into().unwrap());
        let length = i16::from_be_bytes(data[p + 2..p + 4].try_into().unwrap());
        let address = u32::from_be_bytes(data[p + 4..p + 8].try_into().unwrap()) as usize;
        let ind = u32::from_be_bytes(data[p + 8..p + 12].try_into().unwrap()) as usize;
        let numeric = |usage, digits, scale| {
            DataType::Numeric(NumType {
                usage,
                digits,
                scale,
                signed: true,
                sign: SignPos::Trailing,
            })
        };
        let (ty, len) = match kind & !1 {
            384 | 388 | 392 | 452 | 912 if length >= 0 => {
                (DataType::Alphanumeric { just: false }, length as usize)
            }
            480 if matches!(length, 4 | 8) => {
                (DataType::Float { long: length == 8 }, length as usize)
            }
            484 | 504 => {
                let precision = ((length as u16) >> 8) as u32;
                let scale = (length as u16 & 0xff) as u32;
                if precision == 0 || precision > 31 || scale > precision {
                    return Err(error(
                        "07006",
                        "invalid descriptor decimal precision or scale",
                    ));
                }
                if kind & !1 == 484 {
                    (
                        numeric(NumUsage::Packed, precision, scale as i32),
                        precision as usize / 2 + 1,
                    )
                } else {
                    (
                        DataType::Numeric(NumType {
                            usage: NumUsage::Display,
                            digits: precision,
                            scale: scale as i32,
                            signed: true,
                            sign: SignPos::LeadingSeparate,
                        }),
                        precision as usize + 1,
                    )
                }
            }
            492 if length == 8 => (numeric(NumUsage::Native, 18, 0), 8),
            496 if length == 4 => (numeric(NumUsage::Native, 9, 0), 4),
            500 if length == 2 => (numeric(NumUsage::Native, 4, 0), 2),
            _ => {
                return Err(error(
                    "07006",
                    "unsupported SQLIMS descriptor type or length",
                ));
            }
        };
        if address == 0 {
            return Err(error("HY009", "SQLIMSDATA pointer is null"));
        }
        let indicator = if kind & 1 != 0 {
            if ind == 0 {
                return Err(error("HY009", "nullable descriptor requires SQLIMSIND"));
            }
            Some(Area {
                addr: ind,
                len: 2,
                ty: numeric(NumUsage::Native, 4, 0),
            })
        } else {
            None
        };
        bound.host.push((
            Area {
                addr: address,
                len,
                ty,
            },
            indicator,
        ));
        bound.host_varchar.push(false);
        if kind & !1 == 912 {
            bound.options.push((format!("BINARY-{i}"), None));
        }
    }
    Ok(bound)
}

impl State {
    pub fn syncpoint(&mut self) {
        self.cursor = None;
    }

    fn prepared(&self, program: &str, name: &str) -> Result<&Prepared> {
        match &self.statement {
            Some((p, n, s)) if p == program && n == name => Ok(s),
            _ => Err(error(
                "26501",
                "SQL statement is not prepared in this program",
            )),
        }
    }

    fn execute(
        &mut self,
        m: &mut Machine,
        call: &ExecCall,
        session: &mut Session,
    ) -> Result<Status> {
        let text = call.sql.as_deref().unwrap_or_default();
        let words: Vec<_> = text.split_whitespace().collect();
        let word = |n: usize| {
            words
                .get(n)
                .map(|w| w.to_ascii_uppercase())
                .ok_or_else(|| error("42601", "incomplete EXEC SQLIMS statement"))
        };
        match call.command.as_str() {
            "DECLARE" | "BEGIN" | "END" => return Ok(Status::default()),
            _ => {}
        }
        if !self.enabled {
            return Err(error(
                "0A000",
                "SQLIMS requires an IMS BMP, IFP or MPR region; DLI/DBB and CICS are unsupported",
            ));
        }
        if !matches!(zeus_cobol::codepage::active().ccsid, 37 | 1140) {
            return Err(error("0A000", "SQLIMS COBOL code page must be 37 or 1140"));
        }
        match call.command.as_str() {
            "PREPARE" => {
                if self.cursor.is_some() {
                    return Err(error("24506", "close the active cursor before PREPARE"));
                }
                let into = word(2)? == "INTO";
                let from = if into { 4 } else { 2 };
                if (into && words.get(3) != Some(&"?"))
                    || word(from)? != "FROM"
                    || words.len() != from + 2
                    || words[from + 1] != "?"
                    || call.host.len() != if into { 2 } else { 1 }
                {
                    return Err(error(
                        "42601",
                        "PREPARE requires a statement name, optional INTO descriptor and FROM host variable",
                    ));
                }
                let name = word(1)?;
                // An unsuccessful re-PREPARE must not leave the old statement usable.
                self.statement = None;
                let source = host_text(m, call, usize::from(into))?;
                let prepared = parser::prepare(&source)?;
                let columns = session.describe_sql(&prepared)?;
                let unqualified = matches!(&prepared.statement,Statement::Update {condition,..} | Statement::Delete {condition,..} if condition.is_empty());
                self.statement = Some((call.program.clone(), name, prepared));
                if into {
                    return describe(m, &call.host[0].0, &columns);
                }
                Ok(Status {
                    unqualified,
                    ..Status::default()
                })
            }
            "DESCRIBE" => {
                let output = word(1)? == "OUTPUT";
                let s = if output { 2 } else { 1 };
                if word(s + 1)? != "INTO"
                    || words.len() != s + 3
                    || words[s + 2] != "?"
                    || call.host.len() != 1
                {
                    return Err(error(
                        "42601",
                        "DESCRIBE [OUTPUT] requires statement name and INTO descriptor",
                    ));
                }
                let columns = session.describe_sql(self.prepared(&call.program, &word(s)?)?)?;
                describe(m, &call.host[0].0, &columns)
            }
            "OPEN" => {
                if self.cursor.is_some() {
                    return Err(error("24502", "only one SQLIMS cursor can be open"));
                }
                let Some(ExecArg::Name(statement)) = call.arg("QUERY") else {
                    return Err(error("26501", "cursor has no declared statement"));
                };
                let prepared = self.prepared(&call.program, &statement.to_ascii_uppercase())?;
                if !matches!(prepared.statement, Statement::Select { .. }) {
                    return Err(error("07005", "cursor requires a prepared SELECT"));
                }
                let params = parameters(m, call, prepared)?;
                let result = session.execute_prepared(prepared, &params)?;
                self.cursor = Some(Cursor {
                    program: call.program.clone(),
                    name: word(1)?,
                    result,
                    next: 0,
                });
                Ok(Status::default())
            }
            "FETCH" => {
                let name = word(1)?;
                let cursor = self
                    .cursor
                    .as_mut()
                    .filter(|c| c.program == call.program && c.name == name)
                    .ok_or_else(|| error("24501", "cursor is not open in this program"))?;
                if word(2)? != "INTO" {
                    return Err(error(
                        "42601",
                        "FETCH requires INTO host variables or DESCRIPTOR",
                    ));
                }
                let descriptor = words
                    .get(3)
                    .is_some_and(|w| w.eq_ignore_ascii_case("DESCRIPTOR"));
                let bound = if descriptor {
                    if words.len() != 5 || words[4] != "?" {
                        return Err(error(
                            "42601",
                            "FETCH INTO DESCRIPTOR requires a descriptor host variable",
                        ));
                    }
                    Some(descriptor_fetch_call(m, call)?)
                } else {
                    None
                };
                let Some(row) = cursor.result.rows.get(cursor.next) else {
                    return Ok(Status {
                        code: 100,
                        state: Some("02000"),
                        ..Status::default()
                    });
                };
                let status = fetch_row(
                    m,
                    bound.as_ref().unwrap_or(call),
                    row,
                    &cursor.result.columns,
                )?;
                cursor.next += 1;
                Ok(status)
            }
            "CLOSE" => {
                let name = word(1)?;
                if words.len() != 2 || !call.host.is_empty() {
                    return Err(error("42601", "CLOSE requires only a cursor name"));
                }
                if !self
                    .cursor
                    .as_ref()
                    .is_some_and(|c| c.program == call.program && c.name == name)
                {
                    return Err(error("24501", "cursor is not open"));
                }
                self.cursor = None;
                Ok(Status::default())
            }
            "EXECUTE" => {
                if self.cursor.is_some() {
                    return Err(error(
                        "24506",
                        "close the active cursor before executing another statement",
                    ));
                }
                let result = if word(1)? == "IMMEDIATE" {
                    if words.len() != 3 || words[2] != "?" || call.host.len() != 1 {
                        return Err(error(
                            "42601",
                            "EXECUTE IMMEDIATE requires a character host variable",
                        ));
                    }
                    let prepared = parser::prepare(&host_text(m, call, 0)?)?;
                    if matches!(prepared.statement, Statement::Select { .. }) {
                        return Err(error("07003", "EXECUTE IMMEDIATE cannot execute SELECT"));
                    }
                    session.execute_prepared(&prepared, &[])?
                } else {
                    let prepared = self.prepared(&call.program, &word(1)?)?;
                    if matches!(prepared.statement, Statement::Select { .. }) {
                        return Err(error("07003", "use OPEN and FETCH for SELECT"));
                    }
                    session.execute_prepared(prepared, &parameters(m, call, prepared)?)?
                };
                Ok(Status {
                    rows: result.affected,
                    unqualified: result.unqualified_write,
                    ..Status::default()
                })
            }
            _ => Err(error(
                "42601",
                format!("unsupported EXEC SQLIMS command {}", call.command),
            )),
        }
    }

    pub fn exec(
        &mut self,
        m: &mut Machine,
        call: &ExecCall,
        session: &mut Session,
    ) -> RResult<ExecOutcome> {
        let ca = match call.arg("SQLIMSCA") {
            Some(ExecArg::Area(a)) if a.len == 224 => a.clone(),
            _ => return Err(RunError("SQLIMS requires a 224-byte SQLIMSCA".into())),
        };
        bytes(m, &ca).map_err(|e| RunError(e.to_string()))?;
        let outcome = self.execute(m, call, session);
        let (status, message) = match outcome {
            Ok(status) => (status, String::new()),
            Err(e) => (
                Status {
                    code: e.code,
                    state: Some(e.state),
                    ..Status::default()
                },
                e.message,
            ),
        };
        let mut report = vec![ebcdic::SPACE; 224];
        report[..8].copy_from_slice(&ebcdic::encode("SQLIMSCA"));
        report[8..12].copy_from_slice(&224i32.to_be_bytes());
        report[12..16].copy_from_slice(&status.code.to_be_bytes());
        let msg = ebcdic::encode(&message);
        let len = msg.len().min(158);
        report[16..18].copy_from_slice(&(len as i16).to_be_bytes());
        report[18..18 + len].copy_from_slice(&msg[..len]);
        report[176..184].copy_from_slice(&ebcdic::encode("ZEUSIMS "));
        report[184..208].fill(0);
        report[192..196]
            .copy_from_slice(&(status.rows.min(i32::MAX as usize) as i32).to_be_bytes());
        if status.truncated {
            report[209] = ebcdic::from_latin1(b'W');
        }
        if status.more_columns {
            report[211] = ebcdic::from_latin1(b'W');
        }
        if status.unqualified {
            report[212] = ebcdic::from_latin1(b'W');
        }
        let warning = status.truncated || status.more_columns || status.unqualified;
        if warning {
            report[208] = ebcdic::from_latin1(b'W');
        }
        report[219..224].copy_from_slice(&ebcdic::encode(
            status
                .state
                .unwrap_or(if status.truncated { "01004" } else { "00000" }),
        ));
        m.storage_mut()[ca.addr..ca.addr + 224].copy_from_slice(&report);
        let condition = if status.code < 0 {
            Some("SQLERROR")
        } else if status.code == 100 {
            Some("NOTFOUND")
        } else if warning || status.code > 0 {
            Some("SQLWARNING")
        } else {
            None
        };
        if let Some(condition) = condition
            && let Some(ExecArg::Label(label)) = call.arg(condition)
        {
            return Ok(ExecOutcome::Goto(*label));
        }
        Ok(ExecOutcome::Continue)
    }
}

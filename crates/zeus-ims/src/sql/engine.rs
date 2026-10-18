use super::SqlError;
use super::parser::{self, Condition, Name, Prepared, Projection, Statement, Value};
use crate::catalog::{Dbd, Field, Segment};
use crate::dli::{Session, delete_subtree};
use crate::store::{Record, StoreError};
use std::cmp::Ordering;
use zeus_cobol::codepage;
use zeus_decimal::{Dec, Rounding, packed, zoned};

type Result<T> = std::result::Result<T, SqlError>;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SqlValue {
    Null,
    Number(Dec),
    Text(String),
    Binary(Vec<u8>),
}

#[derive(Clone, Debug)]
pub struct Column {
    pub name: String,
    pub sql_type: i16,
    pub length: i16,
}

#[derive(Clone, Debug, Default)]
pub struct SqlResult {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<SqlValue>>,
    pub affected: usize,
    pub unqualified_write: bool,
}

#[derive(Clone)]
struct Table {
    dbd: Dbd,
    segment: Segment,
    virtual_keys: Vec<(String, String, Field)>,
    schema: String,
    hidden: Vec<String>,
    readonly: Vec<String>,
}

fn store(error: StoreError) -> SqlError {
    let message = match &error {
        StoreError::Duplicate(_) => format!("IMS PCB status II: {error}"),
        _ => error.to_string(),
    };
    SqlError::new("58030", message)
}

fn invalid(message: impl Into<String>) -> SqlError {
    SqlError::new("23614", message)
}
fn numeric(message: impl Into<String>) -> SqlError {
    SqlError::new("22003", message)
}

fn sql_field_name(field: &Field) -> &str {
    field.external_name.as_deref().unwrap_or(&field.name)
}
fn sql_table_name(table: &Table) -> &str {
    table
        .segment
        .external_name
        .as_deref()
        .unwrap_or(&table.segment.name)
}
fn converter(field: &Field) -> Result<&str> {
    if field.marshal.as_ref().is_some_and(|m| m.custom.is_some()) {
        return Err(SqlError::new(
            "0A000",
            "Java/custom IMS type converters require an external implementation",
        ));
    }
    Ok(field
        .marshal
        .as_ref()
        .and_then(|m| m.converter.as_deref())
        .unwrap_or_else(|| {
            let datatype = field.datatype.as_deref().unwrap_or(&field.field_type);
            if matches!(datatype, "DATE" | "TIME" | "TIMESTAMP") {
                "LONG"
            } else if datatype.starts_with("DECIMAL(") {
                "PACKEDDECIMAL"
            } else {
                datatype
            }
        }))
}

fn zoned_sign(field: &Field) -> zoned::SignPosition {
    if field.marshal.as_ref().is_some_and(|m| !m.signed) {
        zoned::SignPosition::Unsigned
    } else {
        zoned::SignPosition::TrailingOverpunch
    }
}

fn decimal_shape(field: &Field) -> Result<Option<(u32, u32)>> {
    let Some(kind) = field.datatype.as_deref() else {
        return Ok(None);
    };
    if !kind.starts_with("DECIMAL(") {
        return Ok(None);
    }
    let (p, s) = kind
        .strip_prefix("DECIMAL(")
        .and_then(|s| s.strip_suffix(')'))
        .and_then(|s| s.split_once(','))
        .ok_or_else(|| invalid("invalid catalog decimal type"))?;
    let p: u32 = p.trim().parse().map_err(|_| invalid("invalid precision"))?;
    let s: u32 = s.trim().parse().map_err(|_| invalid("invalid scale"))?;
    let width = match converter(field)? {
        "PACKEDDECIMAL" => (p as usize + 2) / 2,
        "ZONEDDECIMAL" => p as usize,
        other => {
            return Err(SqlError::new(
                "23611",
                format!("unsupported decimal converter {other}"),
            ));
        }
    };
    if p == 0 || p > 31 || s > p || field.bytes != width {
        return Err(invalid("invalid catalog decimal shape"));
    }
    Ok(Some((p, s)))
}

fn metadata(field: &Field) -> Result<Column> {
    let physical = converter(field)?;
    if field.marshal.as_ref().is_some_and(|m| m.pattern.is_some())
        && !matches!(
            field.datatype.as_deref(),
            Some("DATE" | "TIME" | "TIMESTAMP")
        )
    {
        return Err(SqlError::new("23611", "PATTERN requires a date/time field"));
    }
    let (sql_type, length) = if let Some((p, s)) = decimal_shape(field)? {
        (484, ((p << 8) | s) as i16)
    } else {
        match field.datatype.as_deref().unwrap_or(&field.field_type) {
            kind @ ("DATE" | "TIME" | "TIMESTAMP") => {
                if (physical == "LONG" && field.bytes == 8)
                    || (physical == "CHAR"
                        && field.marshal.as_ref().is_some_and(|m| m.pattern.is_some()))
                {
                    match kind {
                        "DATE" => (384, 10),
                        "TIME" => (388, 8),
                        _ => (392, 26),
                    }
                } else {
                    return Err(SqlError::new(
                        "23611",
                        "datetime requires LONG(8) or CHAR with a PATTERN",
                    ));
                }
            }
            "BYTE" if field.bytes == 1 && physical == "BYTE" => (500, 2),
            "INT" | "SHORT" | "LONG" | "F" | "H" => match field.bytes {
                2 if matches!(physical, "SHORT" | "H") => (500, 2),
                4 if matches!(physical, "INT" | "F") => (496, 4),
                8 if physical == "LONG" => (492, 8),
                _ => return Err(invalid("invalid integer width")),
            },
            "BINARY" if physical == "BINARY" => (
                912,
                i16::try_from(field.bytes).map_err(|_| invalid("binary column too long"))?,
            ),
            "CHAR" | "C" if matches!(physical, "CHAR" | "C") => (
                452,
                i16::try_from(field.bytes).map_err(|_| invalid("character column too long"))?,
            ),
            kind => {
                return Err(SqlError::new(
                    "23611",
                    format!("unsupported catalog field format {kind}"),
                ));
            }
        }
    };
    Ok(Column {
        name: sql_field_name(field).into(),
        sql_type,
        length,
    })
}

fn codepage(dbd: &Dbd) -> Result<&'static codepage::CodePage> {
    named_codepage(dbd.encoding.as_deref().unwrap_or("CP1047"))
}

fn field_codepage(table: &Table, field: &Field) -> Result<&'static codepage::CodePage> {
    match field.marshal.as_ref().and_then(|m| m.encoding.as_deref()) {
        Some(name) => named_codepage(name),
        None => codepage(&table.dbd),
    }
}

fn named_codepage(name: &str) -> Result<&'static codepage::CodePage> {
    let name = name.trim_matches('\'').to_ascii_uppercase();
    let number = name
        .strip_prefix("CP")
        .or_else(|| name.strip_prefix("IBM-"))
        .or_else(|| name.strip_prefix("IBM"))
        .unwrap_or(&name);
    let ccsid = number
        .parse()
        .map_err(|_| SqlError::new("22021", format!("unsupported IMS catalog encoding {name}")))?;
    codepage::get(ccsid).ok_or_else(|| SqlError::new("22021", "catalog encoding unavailable"))
}

fn read_value(
    session: &Session,
    table: &Table,
    field: &Field,
    record: &Record,
) -> Result<SqlValue> {
    if let Some((_, ancestor, definition)) = table
        .virtual_keys
        .iter()
        .find(|(name, _, _)| *name == field.name)
    {
        let mut current = record.clone();
        while current.segment != *ancestor {
            current = current
                .parent
                .and_then(|id| session.transaction.record(id))
                .ok_or_else(|| invalid("virtual foreign key has no ancestor record"))?;
        }
        return read_physical_value(table, definition, &current);
    }
    read_physical_value(table, field, record)
}

fn read_physical_value(table: &Table, field: &Field, record: &Record) -> Result<SqlValue> {
    let start = field
        .start
        .checked_sub(1)
        .ok_or_else(|| invalid("field position is zero"))?;
    let end = start
        .checked_add(field.bytes)
        .ok_or_else(|| invalid("field exceeds segment"))?;
    let bytes = record
        .data
        .get(start..end)
        .ok_or_else(|| invalid("field exceeds segment"))?;
    let ty = metadata(field)?;
    Ok(match ty.sql_type {
        384 | 388 | 392 => {
            let date = if converter(field)? == "LONG" {
                super::datetime::DateTime::from_millis(i64::from_be_bytes(
                    bytes
                        .try_into()
                        .map_err(|_| invalid("datetime LONG width must be eight"))?,
                ))?
            } else {
                let pattern = field
                    .marshal
                    .as_ref()
                    .and_then(|m| m.pattern.as_deref())
                    .ok_or_else(|| invalid("missing datetime PATTERN"))?;
                super::datetime::DateTime::parse(
                    field_codepage(table, field)?
                        .decode(bytes)
                        .trim_end_matches(' '),
                    pattern,
                )?
            };
            SqlValue::Text(date.canonical(field.datatype.as_deref().unwrap())?)
        }
        484 => {
            let (_, scale) = decimal_shape(field)?.unwrap();
            if converter(field)? == "ZONEDDECIMAL" {
                let sign = zoned_sign(field);
                if !zoned::is_valid(bytes, sign) {
                    return Err(SqlError::new("23611", "invalid zoned decimal"));
                }
                return Ok(SqlValue::Number(zoned::to_dec(bytes, sign, scale)));
            }
            if bytes.len() > 16 || !packed::is_valid(bytes) {
                return Err(SqlError::new("23611", "invalid packed decimal"));
            }
            SqlValue::Number(
                packed::to_dec(bytes, scale).ok_or_else(|| numeric("packed decimal overflow"))?,
            )
        }
        492 | 496 | 500 => {
            let mut value = if bytes.first().is_some_and(|b| b & 0x80 != 0) {
                -1i64
            } else {
                0
            };
            for &byte in bytes {
                value = (value << 8) | i64::from(byte);
            }
            SqlValue::Number(Dec::from_i64(value))
        }
        912 => SqlValue::Binary(bytes.to_vec()),
        _ => SqlValue::Text(
            field_codepage(table, field)?
                .decode(bytes)
                .trim_end_matches(' ')
                .to_string(),
        ),
    })
}

fn as_number(value: &SqlValue) -> Result<Dec> {
    match value {
        SqlValue::Number(d) => Ok(*d),
        SqlValue::Text(s) => {
            Dec::parse(s).ok_or_else(|| SqlError::new("22018", "invalid numeric value"))
        }
        _ => Err(SqlError::new("22018", "numeric value required")),
    }
}

fn write_value(table: &Table, field: &Field, value: &SqlValue, data: &mut [u8]) -> Result<()> {
    if *value == SqlValue::Null {
        return Err(SqlError::new("23502", "IMS field cannot be assigned NULL"));
    }
    let ty = metadata(field)?;
    let bytes = match ty.sql_type {
        384 | 388 | 392 => {
            let SqlValue::Text(text) = value else {
                return Err(SqlError::new(
                    "22007",
                    "datetime value must be character data",
                ));
            };
            let date = super::datetime::DateTime::parse_canonical(
                text,
                field.datatype.as_deref().unwrap(),
            )?;
            if converter(field)? == "LONG" {
                date.millis()?.to_be_bytes().to_vec()
            } else {
                let pattern = field
                    .marshal
                    .as_ref()
                    .and_then(|m| m.pattern.as_deref())
                    .ok_or_else(|| invalid("missing datetime PATTERN"))?;
                let mut value = field_codepage(table, field)?.encode(&date.format(pattern)?);
                if value.len() > field.bytes {
                    return Err(SqlError::new(
                        "22001",
                        "datetime PATTERN exceeds column storage",
                    ));
                }
                value.resize(field.bytes, 0x40);
                value
            }
        }
        484 => {
            let (precision, scale) = decimal_shape(field)?.unwrap();
            let d = as_number(value)?
                .rescale(scale, Rounding::Truncate)
                .ok_or_else(|| numeric("decimal overflow"))?;
            if d.digits() > precision {
                return Err(numeric("decimal precision exceeded"));
            }
            if field.marshal.as_ref().is_some_and(|m| !m.signed) && d.is_negative() {
                return Err(numeric("negative value in unsigned decimal field"));
            }
            let mut bytes = vec![0; field.bytes];
            if converter(field)? == "ZONEDDECIMAL" {
                if zoned::store(&mut bytes, d, zoned_sign(field)) {
                    return Err(numeric("zoned decimal overflow"));
                }
                return write_field_bytes(field, &bytes, data);
            }
            if packed::encode(
                &mut bytes,
                d.coef.unsigned_abs(),
                if d.is_negative() {
                    0x0d
                } else if field.marshal.as_ref().is_some_and(|m| !m.signed) {
                    0x0f
                } else {
                    0x0c
                },
            ) {
                return Err(numeric("packed decimal overflow"));
            }
            bytes
        }
        492 | 496 | 500 => {
            let d = as_number(value)?
                .rescale(0, Rounding::Truncate)
                .ok_or_else(|| numeric("integer overflow"))?;
            let bits = field.bytes * 8 - 1;
            if d.coef < -(1i128 << bits) || d.coef >= (1i128 << bits) {
                return Err(numeric("integer out of range"));
            }
            let all = (d.coef as i64).to_be_bytes();
            all[8 - field.bytes..].to_vec()
        }
        912 => match value {
            SqlValue::Binary(b) if b.len() == field.bytes => b.clone(),
            _ => return Err(SqlError::new("22001", "fixed binary length mismatch")),
        },
        _ => {
            let text = match value {
                SqlValue::Text(s) => s.clone(),
                SqlValue::Number(d) => d.to_string(),
                _ => return Err(SqlError::new("22002", "NULL or binary in character column")),
            };
            let cp = field_codepage(table, field)?;
            let mut encoded = cp.encode(&text);
            if cp.decode(&encoded) != text {
                return Err(SqlError::new(
                    "22021",
                    "character is not representable in catalog encoding",
                ));
            }
            if encoded.len() > field.bytes {
                return Err(SqlError::new("22001", "character column overflow"));
            }
            encoded.resize(field.bytes, 0x40);
            encoded
        }
    };
    write_field_bytes(field, &bytes, data)
}

fn write_field_bytes(field: &Field, bytes: &[u8], data: &mut [u8]) -> Result<()> {
    let start = field
        .start
        .checked_sub(1)
        .ok_or_else(|| invalid("field position is zero"))?;
    let end = start
        .checked_add(field.bytes)
        .ok_or_else(|| invalid("field exceeds segment"))?;
    data.get_mut(start..end)
        .ok_or_else(|| invalid("field exceeds segment"))?
        .copy_from_slice(bytes);
    Ok(())
}

fn literal(value: &Value, parameters: &[SqlValue]) -> Result<SqlValue> {
    Ok(match value {
        Value::Text(s) => SqlValue::Text(s.clone()),
        Value::Number(s) => {
            SqlValue::Number(Dec::parse(s).ok_or_else(|| numeric("numeric literal overflow"))?)
        }
        Value::Parameter(n) => parameters
            .get(*n)
            .cloned()
            .ok_or_else(|| SqlError::new("07001", "missing parameter value"))?,
    })
}

fn column(tables: &[Table], name: &Name) -> Result<(usize, usize)> {
    let mut found = None;
    let mut hidden = false;
    for (t, table) in tables.iter().enumerate() {
        if name
            .qualifier
            .as_ref()
            .is_some_and(|q| *q != sql_table_name(table))
        {
            continue;
        }
        hidden |= table.hidden.contains(&name.name);
        for (f, field) in table.segment.fields.iter().enumerate() {
            if sql_field_name(field) == name.name {
                if found.is_some() {
                    return Err(SqlError::new(
                        "42702",
                        format!("ambiguous column {}", name.name),
                    ));
                }
                metadata(field)?;
                found = Some((t, f));
            }
        }
    }
    found.ok_or_else(|| {
        if hidden {
            SqlError::new(
                "42000",
                format!("PCB is not sensitive to field {}", name.name),
            )
        } else {
            invalid(format!("unknown column {}", name.name))
        }
    })
}

fn compare(
    left: &SqlValue,
    right: &SqlValue,
    table: &Table,
    field: &Field,
) -> Result<Option<Ordering>> {
    Ok(match (left, right) {
        (SqlValue::Null, _) | (_, SqlValue::Null) => None,
        (SqlValue::Number(_), _) => Some(as_number(left)?.cmp(&as_number(right)?)),
        (SqlValue::Text(a), SqlValue::Text(b)) => {
            let cp = field_codepage(table, field)?;
            let mut a = cp.encode(a);
            let mut b = cp.encode(b);
            let length = a.len().max(b.len());
            a.resize(length, 0x40);
            b.resize(length, 0x40);
            Some(a.cmp(&b))
        }
        (SqlValue::Binary(a), SqlValue::Binary(b)) => Some(a.cmp(b)),
        _ => return Err(SqlError::new("42818", "incompatible comparison types")),
    })
}

fn matches(
    session: &Session,
    tables: &[Table],
    row: &[Record],
    condition: &Condition,
    parameters: &[SqlValue],
) -> Result<bool> {
    if condition.is_empty() {
        return Ok(true);
    }
    for group in condition {
        let mut all = true;
        for p in group {
            let (t, f) = column(tables, &p.column)?;
            let left = read_value(session, &tables[t], &tables[t].segment.fields[f], &row[t])?;
            let right = literal(&p.value, parameters)?;
            let ordering = compare(&left, &right, &tables[t], &tables[t].segment.fields[f])?;
            all &= ordering.is_some_and(|o| match p.op.as_str() {
                "=" => o == Ordering::Equal,
                "<>" | "!=" => o != Ordering::Equal,
                "<" => o == Ordering::Less,
                ">" => o == Ordering::Greater,
                "<=" => o != Ordering::Greater,
                ">=" => o != Ordering::Less,
                _ => false,
            });
        }
        if all {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validate_tables(tables: &[Table]) -> Result<()> {
    let first = &tables[0];
    for (i, table) in tables.iter().enumerate() {
        if table.schema != first.schema || table.dbd.name != first.dbd.name {
            return Err(SqlError::new(
                "23600",
                "one SQL statement cannot use multiple PCBs",
            ));
        }
        if tables[..i]
            .iter()
            .any(|old| old.segment.name == table.segment.name)
        {
            return Err(SqlError::new("42712", "duplicate FROM table"));
        }
        for old in &tables[..i] {
            let ancestor = |child: &Table, parent: &Table| {
                let mut current = Some(&child.segment);
                while let Some(segment) = current {
                    if segment.name == parent.segment.name {
                        return true;
                    }
                    current = segment.parent.as_deref().and_then(|p| child.dbd.segment(p));
                }
                false
            };
            if !ancestor(table, old) && !ancestor(old, table) {
                return Err(SqlError::new(
                    "23601",
                    "FROM tables must lie on one hierarchical path",
                ));
            }
        }
    }
    Ok(())
}

fn validate_condition(tables: &[Table], condition: &Condition) -> Result<()> {
    let mut sequence = Vec::new();
    let mut previous = None;
    for group in condition {
        let mut has_integer = false;
        let mut has_decimal = false;
        for predicate in group {
            let (t, f) = column(tables, &predicate.column)?;
            let field = &tables[t].segment.fields[f];
            let ty = metadata(field)?.sql_type;
            has_integer |= matches!(ty, 492 | 496 | 500);
            has_decimal |= ty == 484;
            if previous != Some(t) {
                if sequence.contains(&t) {
                    return Err(SqlError::new(
                        "42201",
                        "segment qualifications must be consecutive",
                    ));
                }
                sequence.push(t);
                previous = Some(t);
            }
        }
        if has_integer && group.len() > 11 {
            return Err(SqlError::new(
                "23615",
                "more than ten consecutive AND operators with an integer predicate",
            ));
        }
        if has_decimal && group.len() > 1 {
            return Err(SqlError::new(
                "42601",
                "IMS COBOL decimal predicates can only be combined using OR",
            ));
        }
    }
    if condition.len() > 1 && sequence.len() > 1 {
        return Err(SqlError::new("42201", "OR cannot span segment boundaries").with_code(-8002));
    }
    Ok(())
}

fn writable(table: &Table, field: &Field) -> Result<()> {
    let own = metadata(field)?;
    for other in &table.segment.fields {
        if other.name == field.name
            || table
                .virtual_keys
                .iter()
                .any(|(name, _, _)| *name == other.name)
        {
            continue;
        }
        if field.start < other.start + other.bytes && other.start < field.start + field.bytes {
            let other_type = metadata(other)?;
            if (own.sql_type, own.length, converter(field)?)
                != (other_type.sql_type, other_type.length, converter(other)?)
            {
                return Err(SqlError::new(
                    "23613",
                    "field overlaps another field of a different type",
                ));
            }
        }
    }
    Ok(())
}

impl Session {
    pub(crate) fn parameter_columns(&self, prepared: &Prepared) -> Result<Vec<Option<Column>>> {
        let mut output = vec![None; prepared.parameters];
        let (tables, condition) = match &prepared.statement {
            Statement::Select {
                tables, condition, ..
            } => (
                tables
                    .iter()
                    .map(|t| self.sql_table(t, &['G', 'R', 'D', 'A']))
                    .collect::<Result<Vec<_>>>()?,
                Some(condition),
            ),
            Statement::Insert {
                table,
                columns,
                values,
            } => {
                let table = self.sql_table(table, &['I', 'A', 'L'])?;
                for (name, value) in columns.iter().zip(values) {
                    if let Value::Parameter(index) = value {
                        let (_, f) = column(std::slice::from_ref(&table), name)?;
                        output[*index] = Some(metadata(&table.segment.fields[f])?);
                    }
                }
                (vec![table], None)
            }
            Statement::Update {
                table,
                assignments,
                condition,
            } => {
                let table = self.sql_table(table, &['R', 'A'])?;
                for (name, value) in assignments {
                    if let Value::Parameter(index) = value {
                        let (_, f) = column(std::slice::from_ref(&table), name)?;
                        output[*index] = Some(metadata(&table.segment.fields[f])?);
                    }
                }
                (vec![table], Some(condition))
            }
            Statement::Delete { table, condition } => {
                (vec![self.sql_table(table, &['D', 'A'])?], Some(condition))
            }
        };
        if let Some(condition) = condition {
            for predicate in condition.iter().flatten() {
                if let Value::Parameter(index) = predicate.value {
                    let (t, f) = column(&tables, &predicate.column)?;
                    output[index] = Some(metadata(&tables[t].segment.fields[f])?);
                }
            }
        }
        Ok(output)
    }

    /// Describe without reading data or performing writes. In particular,
    /// malformed stored decimal bytes must not make DESCRIBE fail.
    pub(crate) fn describe_sql(&self, prepared: &Prepared) -> Result<Vec<Column>> {
        let Statement::Select {
            columns,
            tables,
            condition,
            group,
            order,
        } = &prepared.statement
        else {
            match &prepared.statement {
                Statement::Insert { table, columns, .. } => {
                    let table = self.sql_table(table, &['I', 'A', 'L'])?;
                    for n in columns {
                        let (_, f) = column(std::slice::from_ref(&table), n)?;
                        writable(&table, &table.segment.fields[f])?;
                    }
                }
                Statement::Update {
                    table,
                    assignments,
                    condition,
                } => {
                    let table = self.sql_table(table, &['R', 'A'])?;
                    for (n, _) in assignments {
                        let (_, f) = column(std::slice::from_ref(&table), n)?;
                        writable(&table, &table.segment.fields[f])?;
                    }
                    validate_condition(std::slice::from_ref(&table), condition)?;
                }
                Statement::Delete { table, condition } => {
                    let table = self.sql_table(table, &['D', 'A'])?;
                    validate_condition(std::slice::from_ref(&table), condition)?;
                }
                _ => {}
            }
            return Ok(Vec::new());
        };
        let tables = tables
            .iter()
            .map(|t| self.sql_table(t, &['G', 'R', 'D', 'A']))
            .collect::<Result<Vec<_>>>()?;
        validate_tables(&tables)?;
        validate_condition(&tables, condition)?;
        let groups = group
            .iter()
            .map(|n| column(&tables, n))
            .collect::<Result<Vec<_>>>()?;
        let aggregate = !group.is_empty()
            || columns
                .iter()
                .any(|p| matches!(p, Projection::Aggregate(..)));
        for p in condition.iter().flatten() {
            column(&tables, &p.column)?;
        }
        for (n, _) in order {
            let col = column(&tables, n)?;
            if aggregate && !groups.contains(&col) {
                return Err(SqlError::new("42803", "ORDER BY column must be grouped"));
            }
        }
        if columns.is_empty() {
            let mut output = Vec::new();
            for (table_index, t) in tables.iter().enumerate() {
                let mut fields: Vec<_> = t.segment.fields.iter().collect();
                fields.sort_by_key(|f| (f.start, f.bytes));
                for f in fields {
                    if aggregate
                        && !groups.iter().any(|&(t, i)| {
                            t == table_index && tables[t].segment.fields[i].name == f.name
                        })
                    {
                        return Err(SqlError::new(
                            "42803",
                            "SELECT * requires every column in GROUP BY",
                        ));
                    }
                    output.push(metadata(f)?);
                }
            }
            return Ok(output);
        }
        columns
            .iter()
            .map(|p| match p {
                Projection::Column(n) => {
                    let (t, f) = column(&tables, n)?;
                    if aggregate && !groups.contains(&(t, f)) {
                        return Err(SqlError::new(
                            "42803",
                            "selected column must be in GROUP BY",
                        ));
                    }
                    metadata(&tables[t].segment.fields[f])
                }
                Projection::Aggregate(function, n) => {
                    let field = n
                        .as_ref()
                        .map(|n| {
                            let (t, f) = column(&tables, n)?;
                            metadata(&tables[t].segment.fields[f])
                        })
                        .transpose()?;
                    let name = match n {
                        Some(n) => format!(
                            "{}_{}{}",
                            function,
                            n.qualifier
                                .as_ref()
                                .map(|q| format!("{q}_"))
                                .unwrap_or_default(),
                            n.name
                        ),
                        None => format!("{function}_*"),
                    };
                    let (sql_type, length) = if function == "COUNT" {
                        (496, 4)
                    } else {
                        let field = field.unwrap();
                        if matches!(function.as_str(), "SUM" | "AVG") {
                            match field.sql_type {
                                484 => (485, 0x1f00 | (field.length & 0xff)),
                                492 | 496 | 500 => (493, 8),
                                _ => {
                                    return Err(SqlError::new(
                                        "23609",
                                        "aggregate requires numeric column",
                                    ));
                                }
                            }
                        } else {
                            (field.sql_type | 1, field.length)
                        }
                    };
                    Ok(Column {
                        name,
                        sql_type,
                        length,
                    })
                }
            })
            .collect()
    }

    fn sql_table(&self, name: &Name, permissions: &[char]) -> Result<Table> {
        let pcbs: Vec<_> = self.psb.pcbs.iter().filter(|p| p.kind == "DB").collect();
        let pcb = if let Some(schema) = &name.qualifier {
            pcbs.iter()
                .copied()
                .find(|p| p.external_name.as_ref().or(p.name.as_ref()) == Some(schema))
                .ok_or_else(|| SqlError::new("42704", format!("PCB schema {schema} not found")))?
        } else if pcbs.len() == 1 {
            pcbs[0]
        } else {
            return Err(SqlError::new(
                "42702",
                "qualify table with a PCB schema when multiple database PCBs are scheduled",
            ));
        };
        let dbd = pcb
            .dbd_name
            .as_deref()
            .and_then(|n| self.database.definition(n))
            .ok_or_else(|| invalid("missing DBD catalog metadata"))?;
        if dbd.access == "INDEX" {
            return Err(SqlError::new(
                "0A000",
                "SQL against index database PCBs is unsupported",
            ));
        }
        let mut segment = dbd
            .segments
            .iter()
            .find(|s| s.external_name.as_deref().unwrap_or(&s.name) == name.name)
            .cloned()
            .ok_or_else(|| invalid(format!("table {} not found", name.name)))?;
        let mut virtual_keys = Vec::new();
        let procopt = pcb.segment_procopt(&segment.name);
        if !procopt.contains(permissions) {
            return Err(SqlError::new(
                "58030",
                "SENSEG/PCB PROCOPT does not permit this operation",
            ));
        }
        let mut hidden = Vec::new();
        let mut readonly = Vec::new();
        if let Some(view) = pcb.view(&segment.name).filter(|v| !v.fields.is_empty()) {
            segment.fields.retain(|f| {
                if view.fields.iter().any(|v| v.name == f.name) {
                    if view
                        .fields
                        .iter()
                        .filter(|v| v.name == f.name)
                        .all(|v| !v.replace)
                    {
                        readonly.push(f.name.clone());
                    }
                    true
                } else {
                    hidden.push(sql_field_name(f).to_string());
                    false
                }
            });
        }
        if !pcb.sensitive_segments.is_empty() && !pcb.sensitive_segments.contains(&segment.name) {
            return Err(SqlError::new(
                "42000",
                format!("PCB is not sensitive to {}", name.name),
            ));
        }
        let mut parent = segment.parent.as_deref();
        while let Some(ancestor) = parent.and_then(|name| dbd.segment(name)) {
            if let Some(key) = ancestor.key() {
                let name = format!(
                    "{}_{}",
                    ancestor.external_name.as_deref().unwrap_or(&ancestor.name),
                    sql_field_name(key)
                );
                if segment.fields.iter().any(|f| f.name == name) {
                    return Err(invalid(
                        "virtual foreign key collides with a physical column",
                    ));
                }
                virtual_keys.push((name, ancestor.name.clone(), key.clone()));
            }
            parent = ancestor.parent.as_deref();
        }
        for (name, _, key) in &virtual_keys {
            let mut field = key.clone();
            field.name = name.clone();
            field.external_name = None;
            field.start = segment.max_bytes + 1 + segment.fields.len();
            field.sequence = false;
            field.unique = false;
            segment.fields.push(field);
        }
        let schema = pcb
            .external_name
            .as_ref()
            .or(pcb.name.as_ref())
            .cloned()
            .unwrap_or_else(|| {
                format!(
                    "PCB{}",
                    self.psb
                        .pcbs
                        .iter()
                        .position(|p| std::ptr::eq(p, pcb))
                        .unwrap()
                )
            });
        Ok(Table {
            dbd,
            segment,
            virtual_keys,
            schema,
            hidden,
            readonly,
        })
    }

    fn sql_records(&self, table: &Table) -> Vec<Record> {
        fn walk(session: &Session, dbd: &Dbd, segment: &str) -> Vec<Record> {
            let def = dbd.segment(segment).unwrap();
            if let Some(parent) = &def.parent {
                walk(session, dbd, parent)
                    .into_iter()
                    .flat_map(|p| session.transaction.children(&dbd.name, Some(p.id), segment))
                    .collect()
            } else {
                session.transaction.children(&dbd.name, None, segment)
            }
        }
        walk(self, &table.dbd, &table.segment.name)
    }

    /// Execute dynamic COBOL SQL in this session's existing DL/I unit of work.
    /// A statement error undoes only that statement; CHKP/SYNC/ROLB still own
    /// transaction boundaries and the DL/I navigation position is preserved.
    pub fn execute_sql(&mut self, source: &str, parameters: &[SqlValue]) -> Result<SqlResult> {
        let prepared = parser::prepare(source)?;
        self.execute_prepared(&prepared, parameters)
    }

    pub(crate) fn execute_prepared(
        &mut self,
        prepared: &Prepared,
        parameters: &[SqlValue],
    ) -> Result<SqlResult> {
        if prepared.parameters != parameters.len() {
            return Err(SqlError::new("07001", "parameter count mismatch"));
        }
        let mark = self.transaction.statement_mark();
        let result = self.sql_statement(&prepared.statement, parameters);
        if result.is_err() {
            self.transaction.undo_statement(mark);
        }
        result
    }

    fn sql_statement(
        &mut self,
        statement: &Statement,
        parameters: &[SqlValue],
    ) -> Result<SqlResult> {
        match statement {
            Statement::Select {
                columns,
                tables,
                condition,
                group,
                order,
            } => {
                let tables: Vec<_> = tables
                    .iter()
                    .map(|t| self.sql_table(t, &['G', 'R', 'D', 'A']))
                    .collect::<Result<_>>()?;
                validate_tables(&tables)?;
                validate_condition(&tables, condition)?;
                for (i, t) in tables.iter().enumerate() {
                    if tables[..i].iter().any(|p| p.segment.name == t.segment.name) {
                        return Err(invalid("duplicate table"));
                    }
                    if t.dbd.name != tables[0].dbd.name {
                        return Err(invalid("tables must share one hierarchical database"));
                    }
                }
                let deepest = tables
                    .iter()
                    .enumerate()
                    .max_by_key(|(_, t)| {
                        let mut depth = 0;
                        let mut s = &t.segment;
                        while let Some(p) = s.parent.as_deref().and_then(|p| t.dbd.segment(p)) {
                            depth += 1;
                            s = p;
                        }
                        depth
                    })
                    .map(|(i, _)| i)
                    .unwrap();
                let mut ancestor_names = vec![tables[deepest].segment.name.clone()];
                let mut parent = tables[deepest].segment.parent.as_deref();
                while let Some(p) = parent {
                    ancestor_names.push(p.into());
                    parent = tables[deepest]
                        .dbd
                        .segment(p)
                        .and_then(|s| s.parent.as_deref());
                }
                if tables
                    .iter()
                    .any(|t| !ancestor_names.contains(&t.segment.name))
                {
                    return Err(invalid(
                        "FROM tables must lie on the same hierarchical path",
                    ));
                }
                // Resolve even on an empty table: misspelled fields are errors.
                for predicate in condition.iter().flatten() {
                    column(&tables, &predicate.column)?;
                }
                let group_cols = group
                    .iter()
                    .map(|n| column(&tables, n))
                    .collect::<Result<Vec<_>>>()?;
                let order_cols = order
                    .iter()
                    .map(|(n, desc)| column(&tables, n).map(|c| (c, *desc)))
                    .collect::<Result<Vec<_>>>()?;
                let projections = if columns.is_empty() {
                    let mut out = Vec::new();
                    for table in &tables {
                        let mut fields: Vec<_> = table.segment.fields.iter().collect();
                        fields.sort_by_key(|f| (f.start, f.bytes));
                        out.extend(fields.into_iter().map(|f| {
                            Projection::Column(Name {
                                qualifier: Some(sql_table_name(table).into()),
                                name: sql_field_name(f).into(),
                            })
                        }));
                    }
                    out
                } else {
                    columns.clone()
                };
                let aggregate = projections
                    .iter()
                    .any(|p| matches!(p, Projection::Aggregate(..)))
                    || !group.is_empty();
                let mut result = SqlResult {
                    columns: self.describe_sql(&Prepared {
                        statement: statement.clone(),
                        parameters: 0,
                        semicolon: false,
                    })?,
                    ..SqlResult::default()
                };
                if aggregate && order_cols.iter().any(|(c, _)| !group_cols.contains(c)) {
                    return Err(SqlError::new("42803", "ORDER BY column must be grouped"));
                }
                let mut rows = Vec::new();
                for record in self.sql_records(&tables[deepest]) {
                    let mut chain = vec![record];
                    while let Some(parent) = chain.last().unwrap().parent {
                        chain.push(
                            self.transaction
                                .record(parent)
                                .ok_or_else(|| invalid("missing ancestor"))?,
                        );
                    }
                    let row: Vec<_> = tables
                        .iter()
                        .map(|t| {
                            chain
                                .iter()
                                .find(|r| r.segment == t.segment.name)
                                .cloned()
                                .ok_or_else(|| invalid("missing ancestor"))
                        })
                        .collect::<Result<_>>()?;
                    if matches(self, &tables, &row, condition, parameters)? {
                        rows.push(row);
                    }
                }
                let value_at = |row: &[Record], (t, f): (usize, usize)| {
                    read_value(self, &tables[t], &tables[t].segment.fields[f], &row[t])
                };
                let mut groups: Vec<(Vec<SqlValue>, Vec<Vec<Record>>)> = Vec::new();
                if aggregate && group.is_empty() {
                    groups.push((Vec::new(), rows));
                } else {
                    for row in rows {
                        let key = if aggregate {
                            group_cols
                                .iter()
                                .map(|c| value_at(&row, *c))
                                .collect::<Result<Vec<_>>>()?
                        } else {
                            Vec::new()
                        };
                        if aggregate
                            && let Some((_, rows)) = groups.iter_mut().find(|(k, _)| *k == key)
                        {
                            rows.push(row);
                            continue;
                        }
                        groups.push((key, vec![row]));
                    }
                }
                let mut output = Vec::new();
                for (_, rows) in groups {
                    let mut projected = Vec::new();
                    for p in &projections {
                        projected.push(match p {
                            Projection::Column(n) => value_at(&rows[0], column(&tables, n)?)?,
                            Projection::Aggregate(function, name) => {
                                let col = name.as_ref().map(|n| column(&tables, n)).transpose()?;
                                let values = rows
                                    .iter()
                                    .map(|r| match col {
                                        Some(c) => value_at(r, c),
                                        None => Ok(SqlValue::Number(Dec::new(1, 0))),
                                    })
                                    .collect::<Result<Vec<_>>>()?;
                                let values: Vec<_> = values
                                    .into_iter()
                                    .filter(|v| *v != SqlValue::Null)
                                    .collect();
                                if function == "COUNT" {
                                    if values.len() > i32::MAX as usize {
                                        return Err(numeric("COUNT exceeds the SQL integer range"));
                                    }
                                    SqlValue::Number(Dec::new(values.len() as i128, 0))
                                } else if values.is_empty() {
                                    SqlValue::Null
                                } else if function == "MIN" || function == "MAX" {
                                    let table = &tables[col.unwrap().0];
                                    let mut best = values[0].clone();
                                    for v in &values[1..] {
                                        let cmp = compare(
                                            v,
                                            &best,
                                            table,
                                            &table.segment.fields[col.unwrap().1],
                                        )?
                                        .unwrap();
                                        if (function == "MIN" && cmp == Ordering::Less)
                                            || (function == "MAX" && cmp == Ordering::Greater)
                                        {
                                            best = v.clone();
                                        }
                                    }
                                    best
                                } else {
                                    let field =
                                        &tables[col.unwrap().0].segment.fields[col.unwrap().1];
                                    let shape = decimal_shape(field)?;
                                    let mut sum = Dec::ZERO;
                                    for v in &values {
                                        sum = sum
                                            .checked_add(as_number(v)?)
                                            .ok_or_else(|| numeric("SUM overflow"))?;
                                        if function == "SUM"
                                            && ((shape.is_some() && sum.digits() > 31)
                                                || (shape.is_none()
                                                    && i64::try_from(sum.coef).is_err()))
                                        {
                                            return Err(numeric(
                                                "SUM intermediate result exceeds result type",
                                            ));
                                        }
                                    }
                                    let field =
                                        &tables[col.unwrap().0].segment.fields[col.unwrap().1];
                                    let shape = decimal_shape(field)?;
                                    let scale = shape.map_or(0, |(_, s)| s);
                                    if function == "AVG" {
                                        sum = sum
                                            .checked_div(
                                                Dec::new(values.len() as i128, 0),
                                                scale,
                                                Rounding::Truncate,
                                            )
                                            .ok_or_else(|| numeric("AVG overflow"))?;
                                    }
                                    if shape.is_some() && sum.digits() > 31 {
                                        return Err(numeric(
                                            "decimal aggregate exceeds precision 31",
                                        ));
                                    }
                                    if shape.is_none() && i64::try_from(sum.coef).is_err() {
                                        return Err(numeric("integer aggregate overflow"));
                                    }
                                    SqlValue::Number(sum)
                                }
                            }
                        });
                    }
                    let keys = if rows.is_empty() {
                        Vec::new()
                    } else {
                        order_cols
                            .iter()
                            .map(|(c, _)| value_at(&rows[0], *c))
                            .collect::<Result<Vec<_>>>()?
                    };
                    output.push((keys, projected));
                }
                // Build keys before sorting, so conversion failures cannot be swallowed.
                let mut sort_error = None;
                output.sort_by(|(a, _), (b, _)| {
                    for (i, ((t, f), desc)) in order_cols.iter().enumerate() {
                        let comparison = match compare(
                            &a[i],
                            &b[i],
                            &tables[*t],
                            &tables[*t].segment.fields[*f],
                        ) {
                            Ok(Some(c)) => c,
                            Ok(None) => a[i].cmp(&b[i]),
                            Err(e) => {
                                sort_error = Some(e);
                                Ordering::Equal
                            }
                        };
                        let comparison = if *desc {
                            comparison.reverse()
                        } else {
                            comparison
                        };
                        if comparison != Ordering::Equal {
                            return comparison;
                        }
                    }
                    Ordering::Equal
                });
                if let Some(error) = sort_error {
                    return Err(error);
                }
                result.rows = output.into_iter().map(|(_, r)| r).collect();
                Ok(result)
            }
            Statement::Insert {
                table,
                columns,
                values,
            } => {
                let table = self.sql_table(table, &['I', 'A', 'L'])?;
                let mut data = vec![0; table.segment.max_bytes];
                for field in &table.dbd.segment(&table.segment.name).unwrap().fields {
                    if field.field_type == "C" {
                        data[field.start - 1..field.start - 1 + field.bytes].fill(0x40);
                    }
                }
                for field in &table.segment.fields {
                    if table
                        .virtual_keys
                        .iter()
                        .any(|(name, _, _)| *name == field.name)
                    {
                        continue;
                    }
                    let initial = match metadata(field)?.sql_type {
                        384 => SqlValue::Text("1970-01-01".into()),
                        388 => SqlValue::Text("00:00:00".into()),
                        392 => SqlValue::Text("1970-01-01 00:00:00.000000".into()),
                        484 | 492 | 496 | 500 => SqlValue::Number(Dec::ZERO),
                        912 => SqlValue::Binary(vec![0; field.bytes]),
                        _ => SqlValue::Text(String::new()),
                    };
                    write_value(&table, field, &initial, &mut data)?;
                }
                let mut assigned = Vec::new();
                let mut foreign_values = Vec::new();
                for (name, value) in columns.iter().zip(values) {
                    let (_, f) = column(std::slice::from_ref(&table), name)?;
                    if assigned.contains(&f) {
                        return Err(invalid("duplicate INSERT column"));
                    }
                    assigned.push(f);
                    if table
                        .virtual_keys
                        .iter()
                        .any(|(name, _, _)| *name == table.segment.fields[f].name)
                    {
                        foreign_values
                            .push((table.segment.fields[f].clone(), literal(value, parameters)?));
                        continue;
                    }
                    writable(&table, &table.segment.fields[f])?;
                    write_value(
                        &table,
                        &table.segment.fields[f],
                        &literal(value, parameters)?,
                        &mut data,
                    )?;
                }
                if table
                    .dbd
                    .segment(&table.segment.name)
                    .unwrap()
                    .key()
                    .is_some_and(|key| {
                        !assigned
                            .iter()
                            .any(|&f| table.segment.fields[f].name == key.name)
                    })
                {
                    return Err(SqlError::new(
                        "23502",
                        "INSERT must set the IMS sequence key",
                    ));
                }
                let parent = if let Some(parent) = &table.segment.parent {
                    if foreign_values.len() != table.virtual_keys.len() {
                        return Err(SqlError::new(
                            "23502",
                            "child INSERT requires the virtual keys of its ancestors",
                        ));
                    }
                    let parent_table = Table {
                        dbd: table.dbd.clone(),
                        segment: table.dbd.segment(parent).unwrap().clone(),
                        virtual_keys: Vec::new(),
                        schema: table.schema.clone(),
                        hidden: Vec::new(),
                        readonly: Vec::new(),
                    };
                    let mut found = None;
                    for record in self.sql_records(&parent_table) {
                        let mut selected = true;
                        for (field, value) in &foreign_values {
                            selected &= compare(
                                &read_value(self, &table, field, &record)?,
                                value,
                                &table,
                                field,
                            )? == Some(Ordering::Equal);
                        }
                        if selected {
                            if found.is_some() {
                                return Err(SqlError::new(
                                    "21000",
                                    "virtual foreign keys do not identify a unique parent",
                                ));
                            }
                            found = Some(record.id);
                        }
                    }
                    Some(found.ok_or_else(|| {
                        SqlError::new("23503", "parent of child INSERT does not exist")
                    })?)
                } else {
                    None
                };
                self.transaction
                    .insert(
                        &table.dbd.name,
                        &table.segment.name,
                        parent,
                        data,
                        self.lock_timeout,
                    )
                    .map_err(store)?;
                Ok(SqlResult {
                    affected: 1,
                    ..SqlResult::default()
                })
            }
            Statement::Update {
                table,
                assignments,
                condition,
            } => {
                let table = self.sql_table(table, &['R', 'A'])?;
                let tables = std::slice::from_ref(&table);
                let mut assigned = Vec::new();
                for (name, value) in assignments {
                    let (_, f) = column(tables, name)?;
                    if assigned.iter().any(|(old, _)| *old == f) {
                        return Err(invalid("duplicate UPDATE column"));
                    }
                    let field = &table.segment.fields[f];
                    if table.readonly.contains(&field.name) {
                        return Err(SqlError::new(
                            "58030",
                            "SENFLD REPLACE=NO prohibits field replacement",
                        ));
                    }
                    writable(&table, field)?;
                    if table
                        .virtual_keys
                        .iter()
                        .any(|(name, _, _)| *name == field.name)
                    {
                        return Err(SqlError::new(
                            "23504",
                            "virtual foreign key cannot be updated",
                        ));
                    }
                    if field.sequence {
                        return Err(SqlError::new(
                            "58030",
                            "IMS PCB status DA: sequence key cannot be updated",
                        )
                        .with_code(-8009));
                    }
                    assigned.push((f, literal(value, parameters)?));
                }
                for p in condition.iter().flatten() {
                    column(tables, &p.column)?;
                }
                validate_condition(tables, condition)?;
                let mut affected = 0;
                for record in self.sql_records(&table) {
                    if matches(
                        self,
                        tables,
                        std::slice::from_ref(&record),
                        condition,
                        parameters,
                    )? {
                        let mut data = record.data.clone();
                        for (f, value) in &assigned {
                            write_value(&table, &table.segment.fields[*f], value, &mut data)?;
                        }
                        self.transaction
                            .replace(record.id, data, self.lock_timeout)
                            .map_err(store)?;
                        affected += 1;
                    }
                }
                Ok(SqlResult {
                    affected,
                    unqualified_write: condition.is_empty(),
                    ..SqlResult::default()
                })
            }
            Statement::Delete { table, condition } => {
                let table = self.sql_table(table, &['D', 'A'])?;
                let tables = std::slice::from_ref(&table);
                validate_condition(tables, condition)?;
                for p in condition.iter().flatten() {
                    column(tables, &p.column)?;
                }
                let mut affected = 0;
                for record in self.sql_records(&table) {
                    if matches(
                        self,
                        tables,
                        std::slice::from_ref(&record),
                        condition,
                        parameters,
                    )? {
                        delete_subtree(
                            &mut self.transaction,
                            &table.dbd,
                            &table.dbd.name,
                            record.id,
                            self.lock_timeout,
                        )
                        .map_err(|e| SqlError::new("58000", e))?;
                        affected += 1;
                    }
                }
                Ok(SqlResult {
                    affected,
                    unqualified_write: condition.is_empty(),
                    ..SqlResult::default()
                })
            }
        }
    }
}

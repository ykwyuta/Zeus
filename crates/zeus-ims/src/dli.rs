use crate::catalog::{Dbd, Psb};
use crate::store::{Database, Record, StoreError, Transaction};
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadOp {
    Gu,
    Gn,
    Gnp,
    Ghu,
    Ghn,
    Ghnp,
}
impl ReadOp {
    fn base(self) -> Self {
        match self {
            Self::Ghu => Self::Gu,
            Self::Ghn => Self::Gn,
            Self::Ghnp => Self::Gnp,
            other => other,
        }
    }
    fn is_hold(self) -> bool {
        matches!(self, Self::Ghu | Self::Ghn | Self::Ghnp)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOp {
    Isrt,
    Repl,
    Dlet,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    HigherLevel,
    OtherSegmentType,
    NotFound,
    End,
    Duplicate,
    NotHeld,
    NotAllowed,
}
impl Status {
    pub fn code(self) -> [u8; 2] {
        match self {
            Self::Ok => *b"  ",
            Self::HigherLevel => *b"GA",
            Self::OtherSegmentType => *b"GK",
            Self::NotFound => *b"GE",
            Self::End => *b"GB",
            Self::Duplicate => *b"II",
            Self::NotHeld => *b"DJ",
            Self::NotAllowed => *b"AM",
        }
    }
}

#[derive(Clone, Debug)]
pub struct ReadOutcome {
    pub status: Status,
    pub record: Option<Record>,
    /// The parents a path call (command code D) also retrieves, from the
    /// top down.
    pub path: Vec<Record>,
    pub segment_name: Option<String>,
    pub key_feedback: Vec<u8>,
    pub secondary_key: bool,
}
impl ReadOutcome {
    fn found(record: Record, status: Status) -> Self {
        let mut key_feedback = record.root_key.clone();
        if record.parent.is_some() {
            key_feedback.extend_from_slice(&record.key);
        }
        Self {
            status,
            segment_name: Some(record.segment.clone()),
            key_feedback,
            secondary_key: false,
            record: Some(record),
            path: Vec::new(),
        }
    }
    fn empty(status: Status) -> Self {
        Self {
            status,
            record: None,
            path: Vec::new(),
            segment_name: None,
            key_feedback: Vec::new(),
            secondary_key: false,
        }
    }
}

#[derive(Default)]
struct Position {
    current: Option<u64>,
    parent: Option<u64>,
    exhausted: bool,
    held: Option<u64>,
    last_deleted: Option<Record>,
    secondary: Option<(Vec<u8>, u64)>,
}

pub struct Session {
    pub(crate) database: Database,
    pub(crate) psb: Psb,
    positions: Vec<Position>,
    pub(crate) transaction: Transaction,
    pub(crate) lock_timeout: Duration,
}

/// A relational operator of a qualification statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RelOp {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
}

impl RelOp {
    /// The operator of the two bytes after a field name (`EQ`, `= `, `>=`,
    /// `¬=`, ...).
    fn parse(b: &[u8]) -> Option<RelOp> {
        let t: String = b
            .iter()
            .map(|&c| if c == 0xAC { '¬' } else { c as char })
            .collect::<String>()
            .replace(' ', "");
        Some(match t.as_str() {
            "EQ" | "=" => RelOp::Eq,
            "NE" | "¬=" | "=¬" | "!=" | "=!" => RelOp::Ne,
            "GT" | ">" => RelOp::Gt,
            "GE" | ">=" | "=>" => RelOp::Ge,
            "LT" | "<" => RelOp::Lt,
            "LE" | "<=" | "=<" => RelOp::Le,
            _ => return None,
        })
    }

    fn holds(self, o: std::cmp::Ordering) -> bool {
        use std::cmp::Ordering::*;
        match self {
            RelOp::Eq => o == Equal,
            RelOp::Ne => o != Equal,
            RelOp::Gt => o == Greater,
            RelOp::Ge => o != Less,
            RelOp::Lt => o == Less,
            RelOp::Le => o != Greater,
        }
    }
}

/// A qualification statement: field, operator, value.
#[derive(Clone, Debug)]
struct Qual {
    field: String,
    op: RelOp,
    value: Vec<u8>,
}

/// A segment search argument: the segment, its command codes, and its
/// qualification statements (sets of statements joined by AND, the sets
/// joined by OR) or the concatenated key of command code C.
struct Ssa {
    segment: String,
    codes: String,
    quals: Vec<Vec<Qual>>,
    concatenated_key: Option<Vec<u8>>,
}

/// The length of the concatenated key of `segment` (its key and the keys
/// of its parents).
pub(crate) fn concatenated_key_len(dbd: &Dbd, segment: &str) -> usize {
    let mut n = 0;
    let mut cur = dbd.segment(segment);
    while let Some(s) = cur {
        n += s.key().map_or(0, |k| k.bytes);
        cur = s.parent.as_deref().and_then(|p| dbd.segment(p));
    }
    n
}

/// The parts of an SSA (in ASCII control bytes): command codes from
/// byte 9, then the qualification. `field_len` gives the length of a
/// field's value. Each part is (start, end) in `raw`.
pub(crate) struct SsaLayout {
    pub codes: (usize, usize),
    /// Field name, operator and value of each statement, and the
    /// connector or closing parenthesis after it.
    pub statements: Vec<(usize, usize, usize, usize)>,
    /// The value of command code C, then the closing parenthesis.
    pub concatenated_key: Option<(usize, usize)>,
    pub open: Option<usize>,
}

/// Lays out an SSA whose control bytes `ctl` reads (as ASCII).
pub(crate) fn ssa_layout(
    raw: &[u8],
    ctl: impl Fn(u8) -> u8,
    segment: &str,
    dbd: &Dbd,
    field_len: impl Fn(&str) -> Option<usize>,
) -> Result<SsaLayout, String> {
    let mut at = 8;
    let mut codes = (at, at);
    let mut c_code = false;
    if raw.get(at).map(|&b| ctl(b)) == Some(b'*') {
        at += 1;
        let start = at;
        while let Some(&b) = raw.get(at) {
            let c = ctl(b);
            if c == b'(' || c == b' ' {
                break;
            }
            if c == b'C' {
                c_code = true;
            }
            at += 1;
        }
        codes = (start, at);
    }
    let mut layout = SsaLayout {
        codes,
        statements: Vec::new(),
        concatenated_key: None,
        open: None,
    };
    if raw.get(at).is_none_or(|&b| ctl(b) == b' ') {
        return Ok(layout);
    }
    if ctl(raw[at]) != b'(' {
        return Err(format!(
            "{segment}: unsupported SSA command or qualification"
        ));
    }
    layout.open = Some(at);
    at += 1;
    if c_code {
        let n = concatenated_key_len(dbd, segment);
        if raw.get(at + n).map(|&b| ctl(b)) != Some(b')') {
            return Err(format!("{segment}: SSA concatenated key length is not {n}"));
        }
        layout.concatenated_key = Some((at, at + n));
        return Ok(layout);
    }
    loop {
        if raw.len() < at + 10 {
            return Err(format!("{segment}: qualification statement is too short"));
        }
        let field: String = raw[at..at + 8]
            .iter()
            .map(|&b| ctl(b) as char)
            .collect::<String>()
            .trim()
            .to_ascii_uppercase();
        let n = field_len(&field).ok_or_else(|| format!("{segment}: unknown SSA field {field}"))?;
        let value = at + 10;
        let end = value + n;
        let Some(&conn) = raw.get(end) else {
            return Err(format!(
                "{segment}: SSA value length does not match DBD field {field}"
            ));
        };
        layout.statements.push((at, value, end, end));
        match ctl(conn) {
            b')' => return Ok(layout),
            b'*' | b'&' | b'#' | b'+' | b'|' => at = end + 1,
            _ => {
                return Err(format!(
                    "{segment}: SSA value length does not match DBD field {field}"
                ));
            }
        }
    }
}

impl Ssa {
    fn parse(raw: &[u8], dbd: &Dbd) -> Result<Self, String> {
        if raw.len() < 8 {
            return Err("SSA must contain an 8-byte segment name".into());
        }
        let segment = std::str::from_utf8(&raw[..8])
            .map_err(|_| "SSA segment name is not ASCII")?
            .trim()
            .to_ascii_uppercase();
        let def = dbd
            .segment(&segment)
            .ok_or_else(|| format!("unknown SSA segment {segment}"))?;
        let layout = ssa_layout(
            raw,
            |b| b,
            &segment,
            dbd,
            |f| def.fields.iter().find(|x| x.name == f).map(|x| x.bytes),
        )?;
        let codes =
            String::from_utf8_lossy(&raw[layout.codes.0..layout.codes.1]).to_ascii_uppercase();
        let mut quals: Vec<Vec<Qual>> = Vec::new();
        let mut group = Vec::new();
        for &(f, v, e, conn) in &layout.statements {
            let field = String::from_utf8_lossy(&raw[f..f + 8])
                .trim()
                .to_ascii_uppercase();
            let op = RelOp::parse(&raw[f + 8..v])
                .ok_or_else(|| format!("{segment}: unknown SSA operator"))?;
            group.push(Qual {
                field,
                op,
                value: raw[v..e].to_vec(),
            });
            if matches!(raw[conn], b'+' | b'|' | b')') {
                quals.push(std::mem::take(&mut group));
            }
        }
        Ok(Self {
            segment,
            codes,
            quals,
            concatenated_key: layout.concatenated_key.map(|(s, e)| raw[s..e].to_vec()),
        })
    }

    fn code(&self, c: char) -> bool {
        self.codes.contains(c)
    }

    fn qualified(&self) -> bool {
        !self.quals.is_empty() || self.concatenated_key.is_some()
    }

    /// The field and value of a qualification that is one equality.
    fn condition(&self) -> Option<(&str, &[u8])> {
        match self.quals.as_slice() {
            [g] if g.len() == 1 && g[0].op == RelOp::Eq => {
                Some((g[0].field.as_str(), g[0].value.as_slice()))
            }
            _ => None,
        }
    }

    fn matches(&self, record: &Record, dbd: &Dbd, tx: &Transaction) -> bool {
        if record.segment != self.segment {
            return false;
        }
        if let Some(k) = &self.concatenated_key {
            return &concatenated_key(tx, record) == k;
        }
        if self.quals.is_empty() {
            return true;
        }
        let Some(def) = dbd.segment(&self.segment) else {
            return false;
        };
        self.quals.iter().any(|group| {
            group.iter().all(|q| {
                let Some(f) = def.fields.iter().find(|f| f.name == q.field) else {
                    return false;
                };
                let Some(v) = record.data.get(f.start - 1..f.start - 1 + f.bytes) else {
                    return false;
                };
                let order = match (f.field_type.as_str(), packed(v), packed(&q.value)) {
                    // packed decimal fields compare arithmetically
                    ("P", Some(a), Some(b)) => a.cmp(&b),
                    _ => v.cmp(q.value.as_slice()),
                };
                q.op.holds(order)
            })
        })
    }
}

/// A packed decimal value.
fn packed(b: &[u8]) -> Option<i128> {
    let mut v: i128 = 0;
    for (k, &x) in b.iter().enumerate() {
        let hi = (x >> 4) as i128;
        let lo = (x & 15) as i128;
        if hi > 9 {
            return None;
        }
        v = v * 10 + hi;
        if k + 1 < b.len() {
            if lo > 9 {
                return None;
            }
            v = v * 10 + lo;
        } else if matches!(lo, 0xB | 0xD) {
            v = -v;
        } else if lo < 0xA {
            return None;
        }
    }
    Some(v)
}

/// The concatenated key of a record: the keys of its parents and its own.
fn concatenated_key(tx: &Transaction, record: &Record) -> Vec<u8> {
    let mut parts = vec![record.key.clone()];
    let mut parent = record.parent;
    while let Some(id) = parent {
        let Some(p) = tx.record(id) else { break };
        parts.push(p.key.clone());
        parent = p.parent;
    }
    parts.reverse();
    parts.concat()
}

/// The ancestor of `record` (or the record itself) that is a `segment`.
fn ancestor(tx: &Transaction, record: &Record, segment: &str) -> Option<Record> {
    let mut cur = Some(record.clone());
    while let Some(r) = cur {
        if r.segment == segment {
            return Some(r);
        }
        cur = r.parent.and_then(|id| tx.record(id));
    }
    None
}

/// Whether `record` is below the record `id`.
fn under(tx: &Transaction, record: &Record, id: u64) -> bool {
    let mut parent = record.parent;
    while let Some(p) = parent {
        if p == id {
            return true;
        }
        parent = tx.record(p).and_then(|r| r.parent);
    }
    false
}

/// Whether `record` satisfies the SSAs of a call: it is the segment of the
/// last one, and its parents those of the others.
fn path_matches(tx: &Transaction, dbd: &Dbd, ssas: &[Ssa], record: &Record) -> bool {
    let Some((last, upper)) = ssas.split_last() else {
        return true;
    };
    last.matches(record, dbd, tx)
        && upper
            .iter()
            .all(|ssa| ancestor(tx, record, &ssa.segment).is_some_and(|a| ssa.matches(&a, dbd, tx)))
}

impl Session {
    pub(crate) fn held_record(&self, pcb: usize) -> Option<Record> {
        let position = self.positions.get(pcb)?;
        position
            .held
            .filter(|id| position.current == Some(*id))
            .and_then(|id| self.transaction.record(id))
    }
    pub fn indexed_fields(&self, pcb_index: usize) -> Result<Vec<(String, String, usize)>, String> {
        let pcb = self
            .psb
            .pcbs
            .get(pcb_index)
            .ok_or("PCB index out of range")?;
        let dbd = pcb.dbd_name.as_deref().ok_or("DB PCB has no DBDNAME")?;
        let mut result = Vec::new();
        if let Some(index) = pcb.procseq.as_deref() {
            result.push(self.procseq_field(index, dbd)?);
        }
        for (_, indexes) in &pcb.indices {
            for index in indexes {
                result.push(self.procseq_field(index, dbd)?);
            }
        }
        Ok(result)
    }
    pub fn procseq_field(&self, index: &str, dbd: &str) -> Result<(String, String, usize), String> {
        self.database
            .procseq_field(index, dbd)
            .map_err(|error| error.to_string())
    }
    pub fn new(database: Database, psb: Psb) -> Result<Self, String> {
        let dbds: Vec<Dbd> = psb
            .pcbs
            .iter()
            .filter_map(|pcb| pcb.dbd_name.as_deref())
            .filter_map(|name| database.definition(name))
            .collect();
        psb.validate(&dbds)?;
        for pcb in &psb.pcbs {
            if pcb.kind == "DB"
                && pcb
                    .dbd_name
                    .as_deref()
                    .and_then(|name| database.definition(name))
                    .is_some_and(|dbd| dbd.access == "INDEX")
            {
                if pcb.procseq.is_some() {
                    return Err("index PCB cannot specify PROCSEQ".into());
                }
                database
                    .index_pointer(pcb.dbd_name.as_deref().unwrap())
                    .map_err(|error| error.to_string())?;
            }
            if let Some(index) = pcb.procseq.as_deref() {
                if pcb.kind != "DB" {
                    return Err("PROCSEQ requires a DB PCB".into());
                }
                database
                    .procseq_field(index, pcb.dbd_name.as_deref().ok_or("missing DBDNAME")?)
                    .map_err(|error| error.to_string())?;
            }
            for (segment, indexes) in &pcb.indices {
                if pcb.kind != "DB" {
                    return Err("SENSEG INDICES requires a DB PCB".into());
                }
                for index in indexes {
                    let (target, _, _) = database
                        .procseq_field(index, pcb.dbd_name.as_deref().ok_or("missing DBDNAME")?)
                        .map_err(|error| error.to_string())?;
                    if &target != segment {
                        return Err(format!(
                            "{index}: SENSEG INDICES target is {target}, not {segment}"
                        ));
                    }
                }
            }
        }
        let positions = (0..psb.pcbs.len()).map(|_| Position::default()).collect();
        Ok(Self {
            transaction: database.begin(),
            database,
            psb,
            positions,
            lock_timeout: Duration::from_secs(5),
        })
    }
    pub fn set_lock_timeout(&mut self, timeout: Duration) {
        self.lock_timeout = timeout;
    }
    /// EXEC DLI get commands have update eligibility like their Get Hold call forms.
    pub fn read_exec(
        &mut self,
        pcb_index: usize,
        op: ReadOp,
        raw_ssas: &[&[u8]],
    ) -> Result<ReadOutcome, String> {
        let hold = match op.base() {
            ReadOp::Gu => ReadOp::Ghu,
            ReadOp::Gn => ReadOp::Ghn,
            ReadOp::Gnp => ReadOp::Ghnp,
            _ => unreachable!(),
        };
        self.read(pcb_index, hold, raw_ssas)
    }
    pub fn commit(&mut self) -> Result<(), StoreError> {
        self.commit_checkpoint(None)
    }
    pub fn commit_checkpoint(&mut self, checkpoint: Option<[u8; 8]>) -> Result<(), StoreError> {
        if let Some(id) = checkpoint {
            self.transaction.set_checkpoint(id);
        }
        let previous = std::mem::replace(&mut self.transaction, self.database.begin());
        self.positions = (0..self.psb.pcbs.len())
            .map(|_| Position::default())
            .collect();
        previous.commit()
    }
    /// A symbolic checkpoint: commits with checkpoint `id` and the areas
    /// it saves.
    pub fn commit_symbolic(&mut self, id: [u8; 8], data: Vec<u8>) -> Result<(), StoreError> {
        self.transaction.set_symbolic_checkpoint(id, data);
        self.commit_checkpoint(None)
    }

    /// The areas of symbolic checkpoint `id` (None: the latest checkpoint),
    /// with its ID.
    pub fn checkpoint_data(&self, id: Option<[u8; 8]>) -> Option<([u8; 8], Vec<u8>)> {
        let id = id.or_else(|| self.database.latest_checkpoint())?;
        self.database.checkpoint_data(id).map(|d| (id, d))
    }

    pub fn rollback(&mut self) {
        let previous = std::mem::replace(&mut self.transaction, self.database.begin());
        previous.rollback();
        self.positions = (0..self.psb.pcbs.len())
            .map(|_| Position::default())
            .collect();
    }
    pub fn write(
        &mut self,
        pcb_index: usize,
        op: WriteOp,
        segment: &str,
        data: Option<&[u8]>,
        raw_ssas: &[&[u8]],
    ) -> Result<ReadOutcome, String> {
        let pcb = self
            .psb
            .pcbs
            .get(pcb_index)
            .ok_or("PCB index out of range")?;
        if pcb.kind != "DB" {
            return Err("GSAM and I/O PCBs require a separate adapter".into());
        }
        let dbd_name = pcb.dbd_name.as_deref().ok_or("DB PCB has no DBDNAME")?;
        let dbd = self.database.definition(dbd_name).ok_or("missing DBD")?;
        if dbd.access == "INDEX" {
            return Ok(ReadOutcome::empty(Status::NotAllowed));
        }
        let def = dbd
            .segment(segment)
            .ok_or_else(|| format!("unknown segment {segment}"))?;
        if !pcb.sensitive_segments.is_empty()
            && !pcb.sensitive_segments.iter().any(|s| s == segment)
        {
            return Err(format!("PCB is not sensitive to {segment}"));
        }
        let procopt = pcb.segment_procopt(segment);
        let allowed = match op {
            WriteOp::Isrt => procopt.contains(['I', 'A', 'L']),
            WriteOp::Repl => procopt.contains(['R', 'A']),
            WriteOp::Dlet => procopt.contains(['D', 'A']),
        };
        if !allowed {
            return Ok(ReadOutcome::empty(Status::NotAllowed));
        }
        let ssas: Vec<Ssa> = raw_ssas
            .iter()
            .map(|raw| Ssa::parse(raw, &dbd))
            .collect::<Result<_, _>>()?;
        if ssas.last().is_some_and(|ssa| ssa.segment != segment) {
            return Err("last SSA does not name the update segment".into());
        }
        let position = &mut self.positions[pcb_index];
        match op {
            WriteOp::Isrt => {
                let bytes = data.ok_or("ISRT requires segment data")?;
                let parent = if let Some(expected) = def.parent.as_deref() {
                    let candidate = position
                        .current
                        .and_then(|id| self.transaction.record(id))
                        .filter(|record| record.segment == expected)
                        .or_else(|| {
                            position
                                .parent
                                .and_then(|id| self.transaction.record(id))
                                .filter(|record| record.segment == expected)
                        });
                    let Some(candidate) = candidate else {
                        return Ok(ReadOutcome::empty(Status::NotFound));
                    };
                    Some(candidate.id)
                } else {
                    None
                };
                let id = match self.transaction.insert(
                    dbd_name,
                    segment,
                    parent,
                    bytes.to_vec(),
                    self.lock_timeout,
                ) {
                    Ok(id) => id,
                    Err(StoreError::Duplicate(_)) => {
                        return Ok(ReadOutcome::empty(Status::Duplicate));
                    }
                    Err(StoreError::Missing(_)) => return Ok(ReadOutcome::empty(Status::NotFound)),
                    Err(error) => return Err(error.to_string()),
                };
                let record = self
                    .transaction
                    .record(id)
                    .ok_or("inserted record disappeared")?;
                position.current = Some(id);
                position.parent = parent.or(Some(id));
                position.held = None;
                position.last_deleted = None;
                Ok(ReadOutcome::found(record, Status::Ok))
            }
            WriteOp::Repl | WriteOp::Dlet => {
                let Some(id) = position.held.filter(|id| position.current == Some(*id)) else {
                    return Ok(ReadOutcome::empty(Status::NotHeld));
                };
                let Some(record) = self.transaction.record(id) else {
                    position.held = None;
                    return Ok(ReadOutcome::empty(Status::NotHeld));
                };
                if record.segment != segment {
                    return Ok(ReadOutcome::empty(Status::NotHeld));
                }
                if op == WriteOp::Repl {
                    let bytes = data.ok_or("REPL requires segment data")?;
                    match self
                        .transaction
                        .replace(id, bytes.to_vec(), self.lock_timeout)
                    {
                        Ok(()) => {}
                        Err(StoreError::Definition(_)) => {
                            return Ok(ReadOutcome::empty(Status::NotAllowed));
                        }
                        Err(error) => return Err(error.to_string()),
                    }
                    let updated = self
                        .transaction
                        .record(id)
                        .ok_or("replaced record disappeared")?;
                    Ok(ReadOutcome::found(updated, Status::Ok))
                } else {
                    delete_subtree(&mut self.transaction, &dbd, dbd_name, id, self.lock_timeout)?;
                    position.held = None;
                    position.last_deleted = Some(record.clone());
                    Ok(ReadOutcome::found(record, Status::Ok))
                }
            }
        }
    }
    pub fn read(
        &mut self,
        pcb_index: usize,
        op: ReadOp,
        raw_ssas: &[&[u8]],
    ) -> Result<ReadOutcome, String> {
        let pcb = self
            .psb
            .pcbs
            .get(pcb_index)
            .ok_or("PCB index out of range")?;
        if pcb.kind != "DB" {
            return Err("GSAM and I/O PCBs require a separate adapter".into());
        }
        let procopt = pcb.procopt.as_deref().unwrap_or("");
        if !procopt.contains(['G', 'R', 'D', 'A']) {
            return Err(format!("PCB {} does not permit get calls", pcb_index));
        }
        // A Get Hold on a get-only PCB cannot be followed by REPL or DLET.
        // Such reads must not retain update locks across other PCB reads.
        let update_hold = op.is_hold() && procopt.contains(['R', 'D', 'A']);
        let dbd_name = pcb.dbd_name.as_deref().ok_or("DB PCB has no DBDNAME")?;
        let dbd = self.database.definition(dbd_name).ok_or("missing DBD")?;
        if dbd.access == "INDEX" {
            let name = dbd_name.to_string();
            return self.read_index(pcb_index, &name, op, raw_ssas);
        }
        if let Some(index) = pcb.procseq.as_ref()
            && op.base() != ReadOp::Gnp
        {
            let index = index.clone();
            let dbd_name = dbd_name.to_string();
            return self.read_procseq(pcb_index, &index, &dbd_name, op, raw_ssas);
        }
        if op.base() == ReadOp::Gu && raw_ssas.len() == 1 && !pcb.indices.is_empty() {
            let raw = raw_ssas[0];
            if raw.len() >= 19 && raw[8] == b'(' {
                let segment = std::str::from_utf8(&raw[..8])
                    .map_err(|_| "invalid SSA segment")?
                    .trim()
                    .to_ascii_uppercase();
                let field = std::str::from_utf8(&raw[9..17])
                    .map_err(|_| "invalid SSA field")?
                    .trim()
                    .to_ascii_uppercase();
                let mut selected = None;
                for (sensitive, indexes) in &pcb.indices {
                    if *sensitive != segment {
                        continue;
                    }
                    for index in indexes {
                        let (target, name, length) = self
                            .database
                            .procseq_field(index, dbd_name)
                            .map_err(|error| error.to_string())?;
                        if target == segment && name == field {
                            if selected.is_some() {
                                return Err(format!("ambiguous SENSEG INDICES field {field}"));
                            }
                            selected = Some((index.clone(), length));
                        }
                    }
                }
                if let Some((index, length)) = selected {
                    return self.read_senseg_index(pcb_index, &index, length, op, raw);
                }
            }
        }
        let ssas: Vec<Ssa> = raw_ssas
            .iter()
            .map(|raw| Ssa::parse(raw, &dbd))
            .collect::<Result<_, _>>()?;
        for ssa in &ssas {
            if !pcb.sensitive_segments.is_empty() && !pcb.sensitive_segments.contains(&ssa.segment)
            {
                return Err(format!("PCB is not sensitive to {}", ssa.segment));
            }
        }
        let position = &mut self.positions[pcb_index];
        position.held = None;
        let previous = position
            .current
            .and_then(|id| self.transaction.record(id))
            .or_else(|| position.last_deleted.clone());
        let base = op.base();
        let sensitive = pcb.sensitive_segments.clone();
        let tx = &self.transaction;
        let fits = |r: &Record| {
            (sensitive.is_empty() || sensitive.contains(&r.segment))
                && (pcb
                    .segment_procopt(&r.segment)
                    .contains(['G', 'R', 'D', 'A'])
                    || (pcb.segment_procopt(&r.segment).contains('K')
                        && !(base == ReadOp::Gn && ssas.is_empty())))
                && path_matches(tx, &dbd, &ssas, r)
        };
        let last_code = |c: char| ssas.last().is_some_and(|s| s.code(c));
        let first_root = || {
            dbd.segments
                .first()
                .and_then(|root| tx.first_child(dbd_name, None, &root.name))
        };
        // the next record from `start` (itself included) that fits
        let search = |mut next: Option<Record>, within: Option<u64>| {
            while let Some(r) = next {
                if within.is_some_and(|p| !under(tx, &r, p)) {
                    return None;
                }
                if fits(&r) {
                    return Some(r);
                }
                next = next_after(tx, &dbd, &r);
            }
            None
        };
        // command code L: the last of the occurrences that fit
        let last_of = |found: Option<Record>| {
            let mut found = found?;
            if last_code('L') {
                while let Some(n) = tx.next_sibling(&found).filter(fits) {
                    found = n;
                }
            }
            Some(found)
        };
        let found = match base {
            ReadOp::Gu => {
                position.current = None;
                position.parent = None;
                position.exhausted = false;
                position.last_deleted = None;
                let keyed = !ssas.is_empty()
                    && ssas.iter().enumerate().all(|(level, ssa)| {
                        let def = dbd.segment(&ssa.segment);
                        def.is_some_and(|d| d.parent.is_some() == (level > 0))
                            && ssa.codes.chars().all(|c| !matches!(c, 'L' | 'F'))
                            && ssa.condition().is_some_and(|(field, _)| {
                                def.and_then(|d| d.key())
                                    .is_some_and(|k| k.name == field && k.unique)
                            })
                    });
                if keyed {
                    // fully keyed: down the hierarchy by the keys
                    let mut parent = None;
                    let mut result = None;
                    for ssa in &ssas {
                        result = find_match(tx, &dbd, dbd_name, parent, ssa);
                        parent = result.as_ref().map(|r| r.id);
                        if result.is_none() {
                            break;
                        }
                    }
                    result.filter(|r| fits(r))
                } else {
                    last_of(search(first_root(), None))
                }
            }
            ReadOp::Gn => {
                if position.exhausted || last_code('F') {
                    position.current = None;
                    position.parent = None;
                    position.exhausted = false;
                }
                let start = match position.current {
                    Some(id) => {
                        let record = tx
                            .record(id)
                            .or_else(|| position.last_deleted.clone())
                            .ok_or("current segment was deleted")?;
                        next_after(tx, &dbd, &record)
                    }
                    None => first_root(),
                };
                let next = last_of(search(start, None));
                if next.is_none() {
                    position.exhausted = true;
                }
                next
            }
            ReadOp::Gnp => {
                let parent_id = position
                    .parent
                    .ok_or("GNP requires a current parent position")?;
                let parent = tx.record(parent_id).ok_or("current parent was deleted")?;
                let current = position
                    .current
                    .and_then(|id| tx.record(id).or_else(|| position.last_deleted.clone()))
                    .filter(|r| under(tx, r, parent_id));
                let start = match current {
                    Some(r) if !last_code('F') => next_after(tx, &dbd, &r),
                    _ => next_after(tx, &dbd, &parent),
                };
                last_of(search(start, Some(parent_id)))
            }
            ReadOp::Ghu | ReadOp::Ghn | ReadOp::Ghnp => unreachable!(),
        };
        // the parents a path call retrieves too, and the parentage a P
        // command code sets
        let path: Vec<Record> = found.as_ref().map_or(Vec::new(), |r| {
            ssas.iter()
                .take(ssas.len().saturating_sub(1))
                .filter(|ssa| ssa.code('D'))
                .filter_map(|ssa| ancestor(tx, r, &ssa.segment))
                .collect()
        });
        let parentage = found.as_ref().and_then(|r| {
            ssas.iter()
                .take(ssas.len().saturating_sub(1))
                .rfind(|ssa| ssa.code('P'))
                .and_then(|ssa| ancestor(tx, r, &ssa.segment))
                .map(|a| a.id)
        });
        if let Some(record) = found {
            let record = if update_hold {
                self.transaction
                    .hold(record.id, self.lock_timeout)
                    .map_err(|e| e.to_string())?
            } else {
                record
            };
            position.current = Some(record.id);
            // parentage: the segment a GU or GN retrieves (or the level of
            // command code P); GNP keeps it
            if base != ReadOp::Gnp {
                position.parent = Some(parentage.unwrap_or(record.id));
            }
            position.last_deleted = None;
            if update_hold {
                position.held = Some(record.id);
            }
            let status = if base == ReadOp::Gn && ssas.is_empty() {
                if let Some(previous) = previous {
                    let old_depth = depth(&dbd, &previous.segment);
                    let new_depth = depth(&dbd, &record.segment);
                    if new_depth < old_depth {
                        Status::HigherLevel
                    } else if new_depth == old_depth && record.segment != previous.segment {
                        Status::OtherSegmentType
                    } else {
                        Status::Ok
                    }
                } else {
                    Status::Ok
                }
            } else {
                Status::Ok
            };
            let mut outcome = ReadOutcome::found(record, status);
            outcome.path = path;
            Ok(outcome)
        } else {
            Ok(ReadOutcome::empty(
                if base == ReadOp::Gn && ssas.iter().all(|ssa| !ssa.qualified()) {
                    Status::End
                } else {
                    Status::NotFound
                },
            ))
        }
    }

    fn read_procseq(
        &mut self,
        pcb_index: usize,
        index: &str,
        dbd_name: &str,
        op: ReadOp,
        raw_ssas: &[&[u8]],
    ) -> Result<ReadOutcome, String> {
        let (segment, field, length) = self
            .database
            .procseq_field(index, dbd_name)
            .map_err(|error| error.to_string())?;
        if raw_ssas.len() > 1 {
            return Err("PROCSEQ GU/GN accepts at most one root SSA".into());
        }
        let search = if let Some(raw) = raw_ssas.first() {
            if raw.len() < 8
                || std::str::from_utf8(&raw[..8])
                    .map_err(|_| "invalid SSA segment")?
                    .trim()
                    .to_ascii_uppercase()
                    != segment
            {
                return Err(format!("PROCSEQ SSA must name {segment}"));
            }
            if raw.get(8).is_none_or(|byte| *byte == b' ') {
                None
            } else {
                if raw.len() != 20 + length
                    || raw[8] != b'('
                    || &raw[17..19] != b"= " && &raw[17..19] != b"EQ"
                    || raw[19 + length] != b')'
                    || std::str::from_utf8(&raw[9..17])
                        .map_err(|_| "invalid SSA field")?
                        .trim()
                        .to_ascii_uppercase()
                        != field
                {
                    return Err(format!(
                        "PROCSEQ SSA requires {field} equality with {length} bytes"
                    ));
                }
                Some(&raw[19..19 + length])
            }
        } else {
            None
        };
        let pcb = &self.psb.pcbs[pcb_index];
        let procopt = pcb.procopt.as_deref().unwrap_or("");
        if !procopt.contains(['G', 'R', 'D', 'A']) {
            return Err(format!("PCB {pcb_index} does not permit get calls"));
        }
        let update_hold = op.is_hold() && procopt.contains(['R', 'D', 'A']);
        if !pcb.sensitive_segments.is_empty() && !pcb.sensitive_segments.contains(&segment) {
            return Err(format!("PCB is not sensitive to {segment}"));
        }
        let position = &mut self.positions[pcb_index];
        position.held = None;
        if op.base() == ReadOp::Gu {
            position.secondary = None;
            position.current = None;
            position.parent = None;
            position.exhausted = false;
        } else if position.exhausted {
            position.secondary = None;
            position.exhausted = false;
        }
        let after = if op.base() == ReadOp::Gn {
            position
                .secondary
                .as_ref()
                .map(|(key, id)| (key.as_slice(), *id))
        } else {
            None
        };
        let entry = self
            .transaction
            .secondary_entry(index, search, after)
            .map_err(|error| error.to_string())?;
        let Some((key, source_id, record)) = entry else {
            if op.base() == ReadOp::Gn {
                position.exhausted = true;
            }
            return Ok(ReadOutcome::empty(
                if search.is_some() || op.base() == ReadOp::Gu {
                    Status::NotFound
                } else {
                    Status::End
                },
            ));
        };
        let record = self
            .transaction
            .record(record.id)
            .ok_or("indexed target was deleted")?;
        let record = if update_hold {
            self.transaction
                .hold(record.id, self.lock_timeout)
                .map_err(|e| e.to_string())?
        } else {
            record
        };
        position.secondary = Some((key.clone(), source_id));
        position.current = Some(record.id);
        position.parent = Some(record.id);
        position.last_deleted = None;
        if update_hold {
            position.held = Some(record.id);
        }
        let mut outcome = ReadOutcome::found(record, Status::Ok);
        outcome.key_feedback = key;
        outcome.secondary_key = true;
        Ok(outcome)
    }

    fn read_index(
        &mut self,
        pcb_index: usize,
        index: &str,
        op: ReadOp,
        raw_ssas: &[&[u8]],
    ) -> Result<ReadOutcome, String> {
        if op.base() == ReadOp::Gnp {
            return Ok(ReadOutcome::empty(Status::NotAllowed));
        }
        if raw_ssas.len() > 1 {
            return Err("index PCB GU/GN accepts at most one pointer SSA".into());
        }
        let (pointer, key_field, _) = self
            .database
            .index_pointer(index)
            .map_err(|error| error.to_string())?;
        let pcb = &self.psb.pcbs[pcb_index];
        if !pcb.sensitive_segments.is_empty() && !pcb.sensitive_segments.contains(&pointer) {
            return Err(format!("PCB is not sensitive to {pointer}"));
        }
        let search = if let Some(raw) = raw_ssas.first() {
            let dbd = self.database.definition(index).ok_or("missing index DBD")?;
            let ssa = Ssa::parse(raw, &dbd)?;
            if ssa.segment != pointer {
                return Err(format!("index PCB SSA must name {pointer}"));
            }
            match ssa.condition() {
                Some((field, value)) if field == key_field => Some(value.to_vec()),
                Some(_) => return Err(format!("index PCB SSA requires the full {key_field} key")),
                None if ssa.qualified() => {
                    return Err(format!("index PCB SSA requires the full {key_field} key"));
                }
                None => None,
            }
        } else {
            None
        };
        let position = &mut self.positions[pcb_index];
        position.held = None;
        if op.base() == ReadOp::Gu || position.exhausted {
            position.secondary = None;
            position.exhausted = false;
        }
        let after = if op.base() == ReadOp::Gn {
            position
                .secondary
                .as_ref()
                .map(|(key, id)| (key.as_slice(), *id))
        } else {
            None
        };
        let entry = self
            .transaction
            .secondary_pointer_entry(index, search.as_deref(), after)
            .map_err(|error| error.to_string())?;
        let Some((key, source_id, _)) = entry else {
            if op.base() == ReadOp::Gn {
                position.exhausted = true;
            }
            return Ok(ReadOutcome::empty(
                if search.is_some() || op.base() == ReadOp::Gu {
                    Status::NotFound
                } else {
                    Status::End
                },
            ));
        };
        position.secondary = Some((key.clone(), source_id));
        position.current = None;
        position.parent = None;
        position.last_deleted = None;
        let record = Record {
            id: source_id,
            dbd: index.to_string(),
            segment: pointer,
            parent: None,
            root_key: key.clone(),
            key: key.clone(),
            data: key,
        };
        Ok(ReadOutcome::found(record, Status::Ok))
    }

    fn read_senseg_index(
        &mut self,
        pcb_index: usize,
        index: &str,
        length: usize,
        op: ReadOp,
        raw: &[u8],
    ) -> Result<ReadOutcome, String> {
        if raw.len() != 20 + length
            || (&raw[17..19] != b"EQ" && &raw[17..19] != b"= ")
            || raw[19 + length] != b')'
        {
            return Err(format!("{index}: invalid secondary search SSA"));
        }
        let position = &mut self.positions[pcb_index];
        position.current = None;
        position.parent = None;
        position.held = None;
        position.exhausted = false;
        position.last_deleted = None;
        let entry = self
            .transaction
            .secondary_entry(index, Some(&raw[19..19 + length]), None)
            .map_err(|error| error.to_string())?;
        let Some((_, _, record)) = entry else {
            return Ok(ReadOutcome::empty(Status::NotFound));
        };
        let record = self
            .transaction
            .record(record.id)
            .ok_or("indexed target was deleted")?;
        let update_hold = op.is_hold()
            && self.psb.pcbs[pcb_index]
                .procopt
                .as_deref()
                .unwrap_or("")
                .contains(['R', 'D', 'A']);
        let record = if update_hold {
            self.transaction
                .hold(record.id, self.lock_timeout)
                .map_err(|error| error.to_string())?
        } else {
            record
        };
        position.current = Some(record.id);
        position.parent = Some(record.id);
        if update_hold {
            position.held = Some(record.id);
        }
        Ok(ReadOutcome::found(record, Status::Ok))
    }
}

pub(crate) fn delete_subtree(
    transaction: &mut Transaction,
    dbd: &Dbd,
    dbd_name: &str,
    id: u64,
    timeout: Duration,
) -> Result<(), String> {
    let record = transaction
        .record(id)
        .ok_or("segment to delete is missing")?;
    for child in dbd
        .segments
        .iter()
        .filter(|definition| definition.parent.as_deref() == Some(record.segment.as_str()))
    {
        for descendant in transaction.children(dbd_name, Some(id), &child.name) {
            delete_subtree(transaction, dbd, dbd_name, descendant.id, timeout)?;
        }
    }
    transaction
        .delete(id, timeout)
        .map_err(|error| error.to_string())
}

fn depth(dbd: &Dbd, segment: &str) -> usize {
    let mut result = 0;
    let mut current = dbd.segment(segment);
    while let Some(parent) = current.and_then(|s| s.parent.as_deref()) {
        result += 1;
        current = dbd.segment(parent);
    }
    result
}

fn find_match(
    transaction: &Transaction,
    dbd: &Dbd,
    dbd_name: &str,
    parent: Option<u64>,
    ssa: &Ssa,
) -> Option<Record> {
    let segment = dbd.segment(&ssa.segment)?;
    if let Some((field, value)) = ssa.condition()
        && segment
            .key()
            .is_some_and(|key| key.name == field && key.unique)
    {
        return transaction.get(dbd_name, parent, &ssa.segment, value);
    }
    transaction
        .children(dbd_name, parent, &ssa.segment)
        .into_iter()
        .find(|r| ssa.matches(r, dbd, transaction))
}

fn next_after(transaction: &Transaction, dbd: &Dbd, record: &Record) -> Option<Record> {
    for child in dbd
        .segments
        .iter()
        .filter(|s| s.parent.as_deref() == Some(&record.segment))
    {
        if let Some(first) = transaction.first_child(&record.dbd, Some(record.id), &child.name) {
            return Some(first);
        }
    }
    let mut current = record.clone();
    loop {
        if let Some(next) = transaction.next_sibling(&current) {
            return Some(next);
        }
        let parent_id = current.parent?;
        let parent = transaction.record(parent_id)?;
        let mut after_current = false;
        for sibling in dbd
            .segments
            .iter()
            .filter(|s| s.parent.as_deref() == Some(&parent.segment))
        {
            if after_current
                && let Some(first) =
                    transaction.first_child(&record.dbd, Some(parent_id), &sibling.name)
            {
                return Some(first);
            }
            if sibling.name == current.segment {
                after_current = true;
            }
        }
        current = parent;
    }
}

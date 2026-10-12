//! Job execution: data set allocation, step selection (COND), program
//! execution, data set disposition and the job's output (spool).
//!
//! The spool of a job is a directory `<spool>/<jobname>`: `JESMSGLG`
//! holds the job log (one line per step with its completion), and each
//! SYSOUT data set is a host text file `<nn>.<step>[.<procstep>].<dd>`.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};

use zeus_cobol::ebcdic;
use zeus_ds::catalog::{Catalog, Entry, EntryKind, Recfm, WriteMode};
use zeus_ds::dd::Allocation;
use zeus_ds::format::RecordFormat;

use crate::expand::{Job, ProcLibrary, Step};
use crate::parse::{self, Param, Stmt, Value};

/// The system a job runs on.
pub struct Env {
    pub catalog: Catalog,
    /// Root of the job output directories.
    pub spool: PathBuf,
    /// Procedure libraries searched after JCLLIB (host directories).
    pub proclibs: Vec<PathBuf>,
    /// Program libraries searched after STEPLIB / JOBLIB.
    pub loadlibs: Vec<PathBuf>,
}

/// How a step ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Rc(i32),
    /// System (`S806`) or user (`U4038`) abend.
    Abend(String),
    /// Not run (COND, or an earlier abend).
    Bypassed,
    /// The step could not be started (data set not found, ...).
    JclError(String),
}

#[derive(Debug, Clone)]
pub struct StepResult {
    pub step: String,
    pub proc_step: Option<String>,
    pub program: String,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Status {
    New,
    Old,
    Shr,
    Mod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum End {
    Delete,
    Keep,
    Pass,
    Catlg,
    Uncatlg,
}

#[derive(Debug, Clone)]
pub enum DdKind {
    Dummy,
    Sysout,
    /// In-stream data cards.
    Instream(Vec<String>),
    /// A catalogued data set (or a member of a partitioned one).
    Dataset {
        dsn: String,
        member: Option<String>,
    },
}

/// An allocated DD statement.
#[derive(Debug, Clone)]
pub struct Dd {
    pub name: String,
    pub kind: DdKind,
    /// Host file holding the data.
    pub path: PathBuf,
    pub format: Option<RecordFormat>,
    status: Status,
    normal: End,
    abnormal: End,
    /// Concatenated data sets (unnamed DD statements following this one).
    pub concat: Vec<Dd>,
    /// The data set was created by this step without RECFM / LRECL: its
    /// attributes come from what the program writes.
    open_attributes: bool,
}

/// Context of a running step, given to the programs.
pub struct StepCtx<'a> {
    pub env: &'a mut Env,
    pub dds: Vec<Dd>,
    pub parm: String,
    /// Prefix of the spool files of this step.
    spool_prefix: PathBuf,
}

/// Error of a program: the step ends with this outcome.
pub type ProgramResult = Result<i32, Outcome>;

fn text_of(rec: &[u8]) -> String {
    ebcdic::decode(rec).trim_end_matches(' ').to_string()
}

impl StepCtx<'_> {
    pub fn dd(&self, name: &str) -> Option<&Dd> {
        self.dds.iter().find(|d| d.name == name)
    }

    /// Records of a DD (EBCDIC); in-stream data are 80-byte card images.
    pub fn read(&self, name: &str) -> Result<Vec<Vec<u8>>, String> {
        let dd = self.dd(name).ok_or_else(|| format!("DD {name} missing"))?;
        let mut out = Vec::new();
        for d in std::iter::once(dd).chain(dd.concat.iter()) {
            out.extend(self.read_dd(d)?);
        }
        Ok(out)
    }

    fn read_dd(&self, dd: &Dd) -> Result<Vec<Vec<u8>>, String> {
        Ok(match &dd.kind {
            DdKind::Dummy | DdKind::Sysout => Vec::new(),
            DdKind::Instream(lines) => lines
                .iter()
                .map(|l| {
                    let mut r = ebcdic::encode(l);
                    r.resize(80, ebcdic::SPACE);
                    r
                })
                .collect(),
            DdKind::Dataset { dsn, member: None } => self
                .env
                .catalog
                .read_records(dsn)
                .map_err(|e| format!("{dsn}: {e}"))?,
            DdKind::Dataset {
                dsn,
                member: Some(_),
            } => {
                let fmt = dd.format.unwrap_or(RecordFormat::Fixed(80));
                zeus_ds::seq::SequentialDataset::load(&dd.path, fmt, ebcdic::SPACE)
                    .map_err(|e| format!("{dsn}: {e}"))?
                    .map(|d| d.records)
                    .unwrap_or_default()
            }
        })
    }

    /// In-stream data or data set records of a DD as text lines.
    pub fn read_text(&self, name: &str) -> Result<Vec<String>, String> {
        let dd = self.dd(name).ok_or_else(|| format!("DD {name} missing"))?;
        if let DdKind::Instream(lines) = &dd.kind {
            return Ok(lines.clone());
        }
        Ok(self.read(name)?.iter().map(|r| text_of(r)).collect())
    }

    /// Writes records to a DD. Returns the number of records rejected.
    pub fn write(
        &mut self,
        name: &str,
        recs: &[Vec<u8>],
        mode: WriteMode,
    ) -> Result<usize, String> {
        let dd = self
            .dd(name)
            .ok_or_else(|| format!("DD {name} missing"))?
            .clone();
        let mode = if dd.status == Status::Mod {
            WriteMode::Append
        } else {
            mode
        };
        match &dd.kind {
            DdKind::Dummy | DdKind::Instream(_) => Ok(0),
            DdKind::Sysout => {
                let mut f = self.sysout(name)?;
                for r in recs {
                    writeln!(f, "{}", text_of(r)).map_err(|e| e.to_string())?;
                }
                Ok(0)
            }
            DdKind::Dataset { dsn, member: None } => self
                .env
                .catalog
                .write_records(dsn, recs, mode)
                .map_err(|e| format!("{dsn}: {e}")),
            DdKind::Dataset {
                dsn,
                member: Some(_),
            } => {
                let mut d = zeus_ds::seq::SequentialDataset::new(
                    dd.format.unwrap_or(RecordFormat::Fixed(80)),
                    ebcdic::SPACE,
                );
                d.records = recs.to_vec();
                d.store(&dd.path).map_err(|e| format!("{dsn}: {e}"))?;
                Ok(0)
            }
        }
    }

    /// Appends to the spool file of a SYSOUT DD (or of the job log when
    /// the DD is not allocated).
    pub fn sysout(&self, name: &str) -> Result<std::fs::File, String> {
        let path = match self.dd(name) {
            Some(d) if matches!(d.kind, DdKind::Sysout) => d.path.clone(),
            Some(d) if matches!(d.kind, DdKind::Dummy) => PathBuf::from("/dev/null"),
            _ => self.spool_prefix.with_extension(name),
        };
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Gives a data set created by this step without DCB attributes the
    /// attributes of the input DD `from` (or of the records written).
    pub fn inherit_attributes(&mut self, to: &str, from: &str, recs: &[Vec<u8>]) {
        let Some(dd) = self.dd(to) else { return };
        if !dd.open_attributes {
            return;
        }
        let DdKind::Dataset { dsn, member: None } = &dd.kind else {
            return;
        };
        let dsn = dsn.clone();
        let source = self
            .entry(from)
            .and_then(|e| self.env.catalog.base_of(&e.name))
            .map(|e| e.kind.clone());
        let max = recs.iter().map(Vec::len).max().unwrap_or(80) as u32;
        let (recfm, lrecl) = match source {
            Some(EntryKind::Sequential { recfm, lrecl })
                if recs.iter().all(|r| r.len() as u32 == lrecl) =>
            {
                (recfm, lrecl)
            }
            _ => (Recfm::Fb, max),
        };
        self.set_attributes(&dsn, recfm, lrecl);
    }

    /// Sets RECFM / LRECL of a sequential data set.
    pub fn set_attributes(&mut self, dsn: &str, recfm: Recfm, lrecl: u32) {
        let cat = &mut self.env.catalog;
        if let Some(e) = cat.get(dsn).cloned()
            && let EntryKind::Sequential { .. } = e.kind
        {
            let _ = cat.delete_entry_only(dsn);
            let _ = cat.define(Entry {
                name: e.name,
                kind: EntryKind::Sequential { recfm, lrecl },
            });
        }
    }

    /// Catalog entry of the data set of a DD.
    pub fn entry(&self, name: &str) -> Option<&Entry> {
        match &self.dd(name)?.kind {
            DdKind::Dataset { dsn, .. } => self.env.catalog.get(dsn),
            _ => None,
        }
    }
}

/// Procedure lookup: JCLLIB data sets (members of catalogued partitioned
/// data sets), then the configured host directories.
struct Procs<'a> {
    catalog: &'a Catalog,
    dirs: &'a [PathBuf],
}

impl ProcLibrary for Procs<'_> {
    fn find(&self, jcllib: &[String], name: &str) -> Option<String> {
        let dirs = jcllib
            .iter()
            .map(|d| self.catalog.path(d))
            .chain(self.dirs.iter().cloned());
        for d in dirs {
            for f in [
                d.join(name),
                d.join(format!("{name}.prc")),
                d.join(format!("{name}.PRC")),
            ] {
                if let Ok(b) = std::fs::read(&f) {
                    return Some(member_text(&b));
                }
            }
        }
        None
    }
}

/// Text of a library member: EBCDIC 80-byte records (a catalogued
/// library) or host text (a host directory).
pub(crate) fn member_text(b: &[u8]) -> String {
    if !b.contains(&b'\n') && b.len().is_multiple_of(80) && !b.is_empty() {
        b.chunks(80).map(|r| text_of(r) + "\n").collect()
    } else {
        String::from_utf8_lossy(b).into_owned()
    }
}

/// Parses `name(member)` / `name(+1)`.
fn split_dsn(dsn: &str) -> (String, Option<String>) {
    match dsn.find('(') {
        Some(i) if dsn.ends_with(')') => (
            dsn[..i].to_ascii_uppercase(),
            Some(dsn[i + 1..dsn.len() - 1].to_ascii_uppercase()),
        ),
        _ => (dsn.to_ascii_uppercase(), None),
    }
}

fn disp_of(dd: &Stmt) -> (Status, Option<End>, Option<End>) {
    let items: Vec<String> = match dd.get("DISP") {
        Some(v) => v
            .items()
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_ascii_uppercase())
            .collect(),
        None => Vec::new(),
    };
    let status = match items.first().map(String::as_str) {
        Some("OLD") => Status::Old,
        Some("SHR") => Status::Shr,
        Some("MOD") => Status::Mod,
        _ => Status::New,
    };
    let end = |s: Option<&String>| match s.map(String::as_str) {
        Some("DELETE") => Some(End::Delete),
        Some("KEEP") => Some(End::Keep),
        Some("PASS") => Some(End::Pass),
        Some("CATLG") => Some(End::Catlg),
        Some("UNCATLG") => Some(End::Uncatlg),
        _ => None,
    };
    (status, end(items.get(1)), end(items.get(2)))
}

/// DCB attributes of a DD statement: RECFM and LRECL (DCB=(...),
/// DCB=*.ddname or the keywords directly).
fn dcb_of(dd: &Stmt, earlier: &[Dd], catalog: &Catalog) -> (Option<Recfm>, Option<u32>) {
    let mut recfm = dd.get_str("RECFM").and_then(Recfm::parse);
    let mut lrecl = dd.get_str("LRECL").and_then(|s| s.parse().ok());
    if let Some(v) = dd.get("DCB") {
        let refback = |name: &str| -> Option<(Option<Recfm>, Option<u32>)> {
            let n = name.rsplit('.').next()?;
            let d = earlier.iter().find(|d| d.name == n)?;
            if let DdKind::Dataset { dsn, .. } = &d.kind {
                match &catalog.base_of(dsn)?.kind {
                    EntryKind::Sequential { recfm, lrecl }
                    | EntryKind::Partitioned { recfm, lrecl } => {
                        return Some((Some(*recfm), Some(*lrecl)));
                    }
                    EntryKind::Cluster { record_size, .. } => {
                        return Some((Some(Recfm::Fb), Some(record_size.1)));
                    }
                    _ => {}
                }
            }
            None
        };
        match v {
            Value::Str(s) if s.starts_with("*.") => {
                if let Some((r, l)) = refback(s) {
                    recfm = recfm.or(r);
                    lrecl = lrecl.or(l);
                }
            }
            Value::List(l) => {
                for p in l {
                    match (&p.key, &p.value) {
                        (Some(k), Value::Str(s)) if k == "RECFM" => recfm = Recfm::parse(s),
                        (Some(k), Value::Str(s)) if k == "LRECL" => lrecl = s.parse().ok(),
                        (None, Value::Str(s)) if s.starts_with("*.") => {
                            if let Some((r, l)) = refback(s) {
                                recfm = recfm.or(r);
                                lrecl = lrecl.or(l);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    (recfm, lrecl)
}

struct Runner<'a> {
    env: &'a mut Env,
    job_name: String,
    dir: PathBuf,
    log: String,
    /// Relative generations resolved in this job: (base, relative) ->
    /// data set name.
    generations: HashMap<(String, i32), String>,
    /// Current generation of each GDG at the start of the job.
    gdg_start: HashMap<String, Option<u32>>,
    /// Temporary data sets (&&name) -> catalogued name.
    temps: HashMap<String, String>,
    /// Data sets to delete at the end of the job (passed temporaries).
    passed: Vec<String>,
}

/// Resolved data set name, member and new GDG generation (base, number).
type Resolved = (String, Option<String>, Option<(String, u32)>);

impl Runner<'_> {
    fn note(&mut self, s: &str) {
        self.log.push_str(s);
        self.log.push('\n');
    }

    /// Resolves the data set name of a DD (GDG relative generations,
    /// temporary names); Some(name, new generation (base, number)).
    fn resolve_dsn(&mut self, dsn: &str, status: Status) -> Result<Resolved, String> {
        if let Some(t) = dsn.strip_prefix("&&") {
            let key = t.to_ascii_uppercase();
            let name = self
                .temps
                .entry(key.clone())
                .or_insert_with(|| format!("SYS.ZEUS.{}.{key}", self.job_name))
                .clone();
            return Ok((name, None, None));
        }
        let (base, inner) = split_dsn(dsn);
        let Some(inner) = inner else {
            return Ok((base, None, None));
        };
        let rel: Option<i32> = inner.strip_prefix('+').unwrap_or(&inner).parse().ok();
        let is_gdg = matches!(
            self.env.catalog.get(&base).map(|e| &e.kind),
            Some(EntryKind::Gdg { .. })
        );
        match (rel, is_gdg) {
            (Some(rel), true) => {
                if let Some(n) = self.generations.get(&(base.clone(), rel)) {
                    return Ok((n.clone(), None, None));
                }
                let start = *self.gdg_start.entry(base.clone()).or_insert_with(|| {
                    match self.env.catalog.get(&base).map(|e| &e.kind) {
                        Some(EntryKind::Gdg { generations, .. }) => generations.last().copied(),
                        _ => None,
                    }
                });
                let g = start.unwrap_or(0) as i64 + rel as i64;
                if g < 1 {
                    return Err(format!("generation {base}({rel}) does not exist"));
                }
                let name = zeus_ds::catalog::generation_name(&base, g as u32);
                self.generations.insert((base.clone(), rel), name.clone());
                let new = (rel > 0 && status == Status::New).then(|| (base.clone(), g as u32));
                Ok((name, None, new))
            }
            _ => Ok((base, Some(inner), None)),
        }
    }

    fn allocate(
        &mut self,
        st: &Stmt,
        earlier: &[Dd],
        seq: usize,
        step: &Step,
    ) -> Result<Dd, String> {
        let name = st.name.clone().unwrap_or_default();
        let (status, normal, abnormal) = disp_of(st);
        let mut dd = Dd {
            name: name.clone(),
            kind: DdKind::Dummy,
            path: PathBuf::from("/dev/null"),
            format: None,
            status,
            normal: normal.unwrap_or(End::Keep),
            abnormal: abnormal.or(normal).unwrap_or(End::Keep),
            concat: Vec::new(),
            open_attributes: false,
        };
        if let Some(lines) = &st.data {
            let path = self
                .dir
                .join(format!("{seq:02}.{}.{name}.input", step.name));
            let mut bytes = Vec::new();
            for l in lines {
                let mut r = ebcdic::encode(l);
                r.resize(80, ebcdic::SPACE);
                bytes.extend(r);
            }
            std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
            dd.kind = DdKind::Instream(lines.clone());
            dd.path = path;
            dd.format = Some(RecordFormat::Fixed(80));
            return Ok(dd);
        }
        if st.get("SYSOUT").is_some() {
            let mut f = format!("{seq:02}.{}", step.name);
            if let Some(p) = &step.proc_step {
                f.push('.');
                f.push_str(p);
            }
            dd.kind = DdKind::Sysout;
            dd.path = self.dir.join(format!("{f}.{name}"));
            dd.format = Some(RecordFormat::Text);
            return Ok(dd);
        }
        let dummy = st.params.iter().any(|p| {
            p.key.is_none() && matches!(&p.value, Value::Str(s) if s.eq_ignore_ascii_case("DUMMY"))
        });
        let dsn = st.get_str("DSN").or_else(|| st.get_str("DSNAME"));
        let Some(dsn) = dsn.filter(|d| !dummy && !d.eq_ignore_ascii_case("NULLFILE")) else {
            return Ok(dd);
        };
        let (dsn, member, new_gen) = self.resolve_dsn(dsn, status)?;
        let cat = &mut self.env.catalog;
        // MOD of a data set that does not exist creates it
        let status = if status == Status::Mod && member.is_none() && cat.get(&dsn).is_none() {
            dd.status = Status::New;
            Status::New
        } else {
            status
        };
        if status == Status::New && member.is_none() {
            if cat.get(&dsn).is_some() {
                return Err(format!("DUPLICATE NAME {dsn}"));
            }
            let (recfm, lrecl) = dcb_of(st, earlier, cat);
            dd.open_attributes = recfm.is_none() || lrecl.is_none();
            let kind = EntryKind::Sequential {
                recfm: recfm.unwrap_or(Recfm::Fb),
                lrecl: lrecl.unwrap_or(80),
            };
            match &new_gen {
                Some((base, g)) => {
                    cat.add_generation(base, *g, kind)
                        .map_err(|e| format!("{dsn}: {e}"))?;
                }
                None => cat
                    .define(Entry {
                        name: dsn.clone(),
                        kind,
                    })
                    .map_err(|e| format!("{dsn}: {e}"))?,
            }
            std::fs::write(cat.path(&dsn), b"").map_err(|e| e.to_string())?;
            // a new data set is deleted at the end unless kept
            dd.normal = normal.unwrap_or(End::Delete);
            dd.abnormal = abnormal.or(normal).unwrap_or(End::Delete);
            if dsn.starts_with("SYS.ZEUS.") && dd.normal == End::Pass {
                self.passed.push(dsn.clone());
            }
        } else if cat.get(&dsn).is_none() {
            return Err(format!("DATA SET {dsn} NOT FOUND"));
        }
        let base_path = cat
            .base_of(&dsn)
            .map(|e| cat.path(&e.name))
            .unwrap_or_else(|| cat.path(&dsn));
        dd.path = match &member {
            Some(m) => base_path.join(m),
            None => base_path,
        };
        dd.format = if dd.open_attributes {
            None
        } else {
            cat.record_format(&dsn)
        };
        dd.kind = DdKind::Dataset { dsn, member };
        Ok(dd)
    }

    /// Applies the disposition of the step's data sets.
    fn dispose(&mut self, dds: &[Dd], abnormal: bool) {
        for dd in dds
            .iter()
            .flat_map(|d| std::iter::once(d).chain(d.concat.iter()))
        {
            let DdKind::Dataset { dsn, member: None } = &dd.kind else {
                continue;
            };
            let end = if abnormal { dd.abnormal } else { dd.normal };
            match end {
                End::Delete => {
                    let _ = self.env.catalog.delete(dsn);
                    self.note(&format!("         {dsn} DELETED"));
                }
                End::Uncatlg => {
                    let _ = self.env.catalog.delete(dsn);
                }
                End::Keep | End::Catlg | End::Pass => {}
            }
        }
        let _ = self.env.catalog.save();
    }

    fn cond_bypass(&self, exec: &Stmt, results: &[StepResult], abend_test: bool) -> bool {
        let abended = !abend_test
            && results
                .iter()
                .any(|r| matches!(r.outcome, Outcome::Abend(_)));
        let Some(cond) = exec.get("COND") else {
            return abended;
        };
        // COND=(code,op[,step]) or ((code,op[,step]),...,EVEN|ONLY)
        let items = cond.items();
        let tests: Vec<Vec<String>> = if items.iter().any(|v| matches!(v, Value::List(_))) {
            items
                .iter()
                .filter_map(|v| match v {
                    Value::List(_) => Some(
                        v.items()
                            .iter()
                            .map(|x| x.as_str().unwrap_or("").to_ascii_uppercase())
                            .collect(),
                    ),
                    _ => None,
                })
                .collect()
        } else if items.len() >= 2 {
            vec![
                items
                    .iter()
                    .map(|x| x.as_str().unwrap_or("").to_ascii_uppercase())
                    .collect(),
            ]
        } else {
            Vec::new()
        };
        let words: Vec<String> = items
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::to_ascii_uppercase)
            .collect();
        let even = words.iter().any(|w| w == "EVEN");
        let only = words.iter().any(|w| w == "ONLY");
        if only && !abended || abended && !even && !only {
            return true;
        }
        for t in &tests {
            let (Some(code), Some(op)) = (t.first().and_then(|c| c.parse::<i32>().ok()), t.get(1))
            else {
                continue;
            };
            let target = t.get(2);
            for r in results {
                if let Some(s) = target
                    && *s != r.step
                    && r.proc_step.as_ref() != Some(s)
                {
                    continue;
                }
                let Outcome::Rc(rc) = r.outcome else { continue };
                let hit = match op.as_str() {
                    "GT" => code > rc,
                    "GE" => code >= rc,
                    "EQ" => code == rc,
                    "LT" => code < rc,
                    "LE" => code <= rc,
                    "NE" => code != rc,
                    _ => false,
                };
                if hit {
                    return true;
                }
            }
        }
        false
    }
}

/// Runs a COBOL program compiled by zcobc.
pub(crate) fn run_cobol(
    ctx: &mut StepCtx,
    program: &str,
    libs: &[PathBuf],
    job: &str,
    step: &Step,
) -> ProgramResult {
    let module = libs
        .iter()
        .flat_map(|d| [d.join(program), d.join(format!("{program}.zo"))])
        .find(|p| p.is_file())
        .and_then(|p| zeus_cobol::load_module(&p).ok())
        .ok_or_else(|| Outcome::Abend("S806".into()))?;
    let mut m = zeus_cobol::vm::Machine::new(libs.to_vec());
    crate::native::register(&mut m);
    // embedded SQL runs on the Db2 server named by ZEUS_DB2
    let db2 = zeus_db2::Db2Host::from_env().map(std::rc::Rc::new);
    if let Some(h) = &db2 {
        m.set_exec_host(h.clone());
    }
    let mut tiot = Vec::new();
    for dd in &ctx.dds {
        m.allocate(
            &dd.name,
            Allocation {
                path: dd.path.clone(),
                format: dd.format,
            },
        );
        tiot.push((dd.name.clone(), !matches!(dd.kind, DdKind::Dummy)));
    }
    m.set_step(zeus_cobol::vm::StepInfo {
        job: job.to_string(),
        step: step.name.clone(),
        proc_step: step.proc_step.clone().unwrap_or_default(),
        dds: Some(tiot),
        parm: ctx.parm.clone(),
    });
    if let Ok(out) = ctx.sysout("SYSOUT") {
        m.set_stdout(Box::new(out));
    }
    if ctx.dd("SYSIN").is_some() {
        m.set_stdin(ctx.read_text("SYSIN").unwrap_or_default());
    }
    let r = m.run_main(module);
    // the unit of work ends with the step: committed, or backed out when
    // the program failed
    if let Some(h) = &db2 {
        let _ = h.end_uow(r.is_ok());
    }
    // attributes of new data sets from the files the program opened
    for (dd, fmt, max) in m.opened_formats().to_vec() {
        let Some(d) = ctx.dd(&dd).filter(|d| d.open_attributes) else {
            continue;
        };
        let DdKind::Dataset { dsn, member: None } = &d.kind else {
            continue;
        };
        let dsn = dsn.clone();
        match fmt {
            RecordFormat::Fixed(n) => ctx.set_attributes(&dsn, Recfm::Fb, n),
            RecordFormat::Variable => ctx.set_attributes(&dsn, Recfm::Vb, max + 4),
            RecordFormat::Text => {}
        }
    }
    match r {
        Ok(rc) => Ok(rc),
        Err(e) => match e.abend() {
            Some(code) => Err(Outcome::Abend(code.to_string())),
            None => {
                if let Ok(mut f) = ctx.sysout("SYSOUT") {
                    let _ = writeln!(f, "ZEUS RUN-TIME ERROR: {e}");
                }
                Err(Outcome::Abend("U4038".into()))
            }
        },
    }
}

/// Runs a job; returns the results of its steps. The job log is written
/// to `<spool>/<jobname>/JESMSGLG` and to `log`.
pub fn run_job(env: &mut Env, text: &str, log: &mut dyn Write) -> Result<Vec<StepResult>, String> {
    let stmts = parse::parse(text).map_err(|e| e.to_string())?;
    let job = {
        let procs = Procs {
            catalog: &env.catalog,
            dirs: &env.proclibs,
        };
        crate::expand::build_job(&stmts, &procs).map_err(|e| e.to_string())?
    };
    let dir = env.spool.join(&job.name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut r = Runner {
        env,
        job_name: job.name.clone(),
        dir: dir.clone(),
        log: String::new(),
        generations: HashMap::new(),
        gdg_start: HashMap::new(),
        temps: HashMap::new(),
        passed: Vec::new(),
    };
    // TYPRUN=SCAN: the JCL is checked (it was parsed and expanded above)
    // but no step runs
    let scan = job
        .job
        .as_ref()
        .and_then(|j| j.get_str("TYPRUN"))
        .is_some_and(|t| t.eq_ignore_ascii_case("SCAN"));
    if scan {
        r.note(&format!(
            "ZEUS JOB {} SCANNED (TYPRUN=SCAN), NOT RUN",
            job.name
        ));
        let _ = writeln!(log, "{}", r.log.trim_end());
        std::fs::write(dir.join("JESMSGLG"), &r.log).map_err(|e| e.to_string())?;
        return Ok(Vec::new());
    }
    r.note(&format!("ZEUS JOB {} STARTED", job.name));
    let results = run_steps(&mut r, &job)?;
    for t in std::mem::take(&mut r.passed) {
        let _ = r.env.catalog.delete(&t);
    }
    let _ = r.env.catalog.save();
    r.note(&format!("ZEUS JOB {} ENDED", job.name));
    std::fs::write(dir.join("JESMSGLG"), &r.log).map_err(|e| e.to_string())?;
    log.write_all(r.log.as_bytes()).map_err(|e| e.to_string())?;
    Ok(results)
}

fn run_steps(r: &mut Runner, job: &Job) -> Result<Vec<StepResult>, String> {
    let mut results: Vec<StepResult> = Vec::new();
    let mut joblib: Vec<Dd> = Vec::new();
    for (i, st) in job.joblib.iter().enumerate() {
        let step = Step {
            name: "JOBLIB".into(),
            proc_step: None,
            exec: st.clone(),
            dds: Vec::new(),
            guards: Vec::new(),
        };
        match r.allocate(st, &[], i, &step) {
            Ok(d) => joblib.push(d),
            Err(e) => r.note(&format!("JOBLIB: {e}")),
        }
    }
    let mut jcl_error = false;
    let mut if_values: HashMap<usize, bool> = HashMap::new();
    for (n, step) in job.steps.iter().enumerate() {
        let seq = n + 1;
        let program = step
            .exec
            .get_str("PGM")
            .map(str::to_ascii_uppercase)
            .unwrap_or_default();
        let mut result = StepResult {
            step: step.name.clone(),
            proc_step: step.proc_step.clone(),
            program: program.clone(),
            outcome: Outcome::Bypassed,
        };
        // IF/THEN/ELSE: each IF is evaluated when the first step under
        // it is reached (the steps before have all completed then)
        let mut guarded = true;
        for &(n, branch) in &step.guards {
            let v = *if_values
                .entry(n)
                .or_insert_with(|| job.ifs[n].eval(&results));
            if v != branch {
                guarded = false;
                break;
            }
        }
        // after an abend only steps under an IF testing ABEND (or with
        // COND=EVEN / ONLY) run
        let abend_test = step.guards.iter().any(|&(n, _)| job.ifs[n].tests_abend());
        if jcl_error || !guarded || r.cond_bypass(&step.exec, &results, abend_test) {
            log_step(r, &result);
            results.push(result);
            continue;
        }
        // allocation
        let mut dds: Vec<Dd> = Vec::new();
        let mut failed = None;
        for st in &step.dds {
            match r.allocate(st, &dds, seq, step) {
                Ok(d) if st.name.is_none() => match dds.last_mut() {
                    Some(prev) => prev.concat.push(d),
                    None => dds.push(d),
                },
                Ok(d) => dds.push(d),
                Err(e) => {
                    failed = Some(format!("{} - {e}", st.name.clone().unwrap_or_default()));
                    break;
                }
            }
        }
        if let Some(e) = failed {
            result.outcome = Outcome::JclError(e);
            r.dispose(&dds, true);
            log_step(r, &result);
            results.push(result);
            jcl_error = true;
            continue;
        }
        let libs: Vec<PathBuf> = dds
            .iter()
            .filter(|d| d.name == "STEPLIB")
            .chain(if dds.iter().any(|d| d.name == "STEPLIB") {
                [].iter()
            } else {
                joblib.iter()
            })
            .flat_map(|d| std::iter::once(d).chain(d.concat.iter()))
            .map(|d| d.path.clone())
            .chain(r.env.loadlibs.iter().cloned())
            .collect();
        let job_name = r.job_name.clone();
        let mut ctx = StepCtx {
            env: r.env,
            dds,
            parm: step.exec.get_str("PARM").unwrap_or("").to_string(),
            spool_prefix: r.dir.join(format!("{seq:02}.{}", step.name)),
        };
        let outcome = match crate::utils::run(&program, &mut ctx) {
            Some(res) => res,
            None if matches!(program.as_str(), "IKJEFT01" | "IKJEFT1A" | "IKJEFT1B") => {
                crate::utils::tso::run(&program, &mut ctx, &libs, &job_name, step)
            }
            None => run_cobol(&mut ctx, &program, &libs, &job_name, step),
        };
        let dds = ctx.dds;
        result.outcome = match outcome {
            Ok(rc) => Outcome::Rc(rc),
            Err(o) => o,
        };
        r.dispose(&dds, matches!(result.outcome, Outcome::Abend(_)));
        log_step(r, &result);
        results.push(result);
    }
    Ok(results)
}

fn log_step(r: &mut Runner, s: &StepResult) {
    let mut line = String::new();
    let _ = write!(
        line,
        "  {:<8} {:<8} {:<8} ",
        s.step,
        s.proc_step.as_deref().unwrap_or(""),
        s.program
    );
    match &s.outcome {
        Outcome::Rc(rc) => {
            let _ = write!(line, "RC={rc:04}");
        }
        Outcome::Abend(c) => {
            let _ = write!(line, "ABEND={c}");
        }
        Outcome::Bypassed => line.push_str("FLUSH"),
        Outcome::JclError(e) => {
            let _ = write!(line, "JCL ERROR: {e}");
        }
    }
    r.note(&line);
}

/// Highest completion of a job: Ok(max rc) or the first abend / JCL
/// error.
pub fn job_completion(results: &[StepResult]) -> Result<i32, String> {
    let mut max = 0;
    for r in results {
        match &r.outcome {
            Outcome::Rc(rc) => max = max.max(*rc),
            Outcome::Abend(c) => return Err(format!("ABEND {c} in {}", r.step)),
            Outcome::JclError(e) => return Err(format!("JCL ERROR in {}: {e}", r.step)),
            Outcome::Bypassed => {}
        }
    }
    Ok(max)
}

/// Keyword operand helper for the utilities.
pub fn param_str<'a>(params: &'a [Param], key: &str) -> Option<&'a str> {
    parse::get(params, key).and_then(Value::as_str)
}

/// Host path helper used by tests and tools.
pub fn spool_dir(env: &Env, job: &str) -> PathBuf {
    Path::new(&env.spool).join(job)
}

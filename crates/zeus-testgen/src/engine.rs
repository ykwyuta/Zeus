//! Test generation: runs a program on inputs with concolic execution,
//! and for each branch that went one way only, asks the solver for
//! inputs that keep the run's earlier decisions and take the other way
//! (generational search). Inputs are the program's USING parameters, the
//! lines it ACCEPTs, the records of the files it reads, I/O faults, what
//! stubbed calls return and what EXEC CICS / SQL statements give it.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use zeus_cobol::concolic::{FileInput, Pred, RecordField, Step, SymState, Var, VarKind};
use zeus_cobol::ebcdic;
use zeus_cobol::vm::Machine;
use zeus_cobol::vm::data as vdata;
use zeus_cobol::zir::{
    AcceptFrom, Base, DataType, Desc, Instr, Module, NumType, NumUsage, OpenMode, Org, ProgramCode,
};
use zeus_decimal::Rounding;
use zeus_ds::dd::Allocation;
use zeus_ds::format::RecordFormat;

use crate::solver::{Builtin, Outcome, Solver};
use crate::stubhost::{AREA_BYTES, Out, StubHost};

/// An elementary item of a parameter that is an input.
#[derive(Debug, Clone)]
pub struct Field {
    pub param: usize,
    pub name: String,
    pub offset: usize,
    pub len: usize,
    /// Numeric (a variable for its value), else bytes (a variable each).
    pub num: Option<NumType>,
}

/// The inputs of a run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Inputs {
    /// The storage of each USING parameter (EBCDIC).
    pub params: Vec<Vec<u8>>,
    /// The lines ACCEPTed (SYSIN).
    pub accept: Vec<String>,
    /// The records of each input file (`FileSpec::input`, in order).
    pub files: Vec<Vec<Vec<u8>>>,
    /// I/O statements made to fail: (file, statement, occurrence) -> how
    /// (`testenv::FAULT`, `FAULT_VSAM`).
    pub faults: BTreeMap<(String, String, u32), u8>,
    /// What the stubs of the programs not there set: (program, call,
    /// argument) -> leading bytes.
    pub stubs: BTreeMap<StubArg, Vec<u8>>,
    /// What EXEC statements give the program (`StubHost` keys).
    pub exec: BTreeMap<String, Out>,
    /// The time of day the program sees (seconds since 1970-01-01 UTC;
    /// None: `CLOCK`).
    pub clock: Option<u64>,
}

/// An argument of a stubbed call: (program, call, argument).
pub type StubArg = (String, u32, u32);

/// What a run did.
#[derive(Debug, Clone)]
pub struct RunOut {
    pub path: Vec<Step>,
    pub vars: Vec<Var>,
    /// Conditional jumps executed: (program, instruction, jumped).
    pub branches: Vec<(String, u32, bool)>,
    pub display: String,
    pub params: Vec<Vec<u8>>,
    pub return_code: i32,
    /// The error that ended the run (abend), if any.
    pub error: Option<String>,
    /// The records written to each output file: (file, records).
    pub files: Vec<(String, Vec<Vec<u8>>)>,
    /// The programs called that ran as stubs.
    pub stubbed: BTreeSet<String>,
    /// The EXEC statements executed with what they sent out.
    pub exec: Vec<String>,
    /// Instructions executed (concolic runs).
    pub instructions: u64,
}

/// A direction of a conditional jump.
pub type Dir = (String, u32, bool);

#[derive(Debug, Clone)]
pub struct Options {
    /// Directories of the programs it calls (.zo).
    pub search: Vec<PathBuf>,
    pub max_runs: usize,
    pub time: Duration,
    /// Instructions per run.
    pub limit: u64,
    /// Solver attempts per branch direction.
    pub attempts: u32,
    /// Lines of input when the program ACCEPTs.
    pub accept_lines: usize,
    /// Mutated inputs tried in a row without new coverage before giving
    /// up (when the solver has nothing more).
    pub fuzz: usize,
    /// Iterations a loop is taken to (one more per run) while some
    /// direction depends on the inputs only through loops.
    pub loop_deepen: u32,
    /// Use Z3 (when its library is found) for what the built-in solver
    /// does not solve in time.
    pub smt: bool,
    /// Initial inputs (case files: earlier cases, test data, inputs
    /// written by hand), run before the default ones.
    pub seeds: Vec<crate::case::CaseFile>,
    /// Threads solving conditions.
    pub threads: usize,
    /// Programs called that run as stubs even when they are there (their
    /// outputs are inputs).
    pub stub: BTreeSet<String>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            search: Vec::new(),
            max_runs: 2000,
            time: Duration::from_secs(120),
            limit: 2_000_000,
            attempts: 4,
            accept_lines: 4,
            fuzz: 60,
            loop_deepen: 20,
            smt: true,
            seeds: Vec::new(),
            stub: BTreeSet::new(),
            threads: std::thread::available_parallelism()
                .map_or(1, |n| n.get())
                .min(8),
        }
    }
}

/// The elementary items of `descs` in `[start, start+len)`, without
/// overlaps (the first one wins): name, offset from `start`, length and
/// numeric type. Tables give an item per occurrence (at most 100, or
/// as many as fit in `AREA_BYTES`: byte tables such as a COMMAREA).
pub(crate) fn items(
    descs: &[&Desc],
    start: usize,
    len: usize,
) -> Vec<(String, usize, usize, Option<NumType>)> {
    let mut out: Vec<(String, usize, usize, Option<NumType>)> = Vec::new();
    let mut used = vec![false; len];
    for d in descs {
        if matches!(
            d.ty,
            DataType::Group | DataType::Pointer | DataType::Index | DataType::Float { .. }
        ) {
            continue;
        }
        // each occurrence of a table
        let mut occ: Vec<(String, usize)> = vec![(String::new(), d.offset as usize)];
        for dim in &d.dims {
            let mut next = Vec::new();
            for (s, off) in &occ {
                let cap = 100.max(AREA_BYTES as u32 / dim.stride.max(1));
                for i in 0..dim.max.min(cap) as usize {
                    let sep = if s.is_empty() { "" } else { "," };
                    next.push((format!("{s}{sep}{}", i + 1), off + i * dim.stride as usize));
                }
            }
            occ = next;
        }
        for (s, off) in occ {
            let l = d.len as usize;
            if off < start || off + l > start + len {
                continue;
            }
            let off = off - start;
            if used[off..off + l].iter().any(|u| *u) {
                continue;
            }
            used[off..off + l].iter_mut().for_each(|u| *u = true);
            let num = match &d.ty {
                DataType::Numeric(t)
                    if t.scale >= 0 && t.digits <= 31 && t.usage != NumUsage::National =>
                {
                    Some(*t)
                }
                _ => None,
            };
            let mut name = if s.is_empty() {
                d.name.clone()
            } else {
                format!("{}({s})", d.name)
            };
            // names are the variables' names: FILLER and the like get
            // their position
            if out.iter().any(|(n, ..)| *n == name) {
                name = format!("{name}@{}", off + 1);
            }
            out.push((name, off, l, num));
        }
    }
    out
}

/// The input fields of the USING parameters of a program.
pub fn fields(code: &ProgramCode) -> (Vec<Field>, Vec<usize>) {
    let mut fields = Vec::new();
    let mut lens = Vec::new();
    for (k, slot) in code.using.iter().enumerate() {
        let descs: Vec<_> = code
            .descs
            .iter()
            .filter(|d| d.base == Base::Linkage(*slot))
            .collect();
        let extent = |d: &Desc| -> usize {
            let mut e = d.offset as usize + d.len as usize;
            for dim in &d.dims {
                e += (dim.max.saturating_sub(1) as usize) * dim.stride as usize;
            }
            e
        };
        let len = descs.iter().map(|d| extent(d)).max().unwrap_or(0);
        lens.push(len);
        for (name, offset, len, num) in items(&descs, 0, len) {
            fields.push(Field {
                param: k,
                name,
                offset,
                len,
                num,
            });
        }
    }
    (fields, lens)
}

/// A file of a program. Every file gets a data set of its own in a
/// temporary directory; the records of the sequential files it opens
/// INPUT are inputs, those of the files it writes are results.
#[derive(Debug, Clone)]
pub struct FileSpec {
    pub name: String,
    /// DD name.
    pub assign: String,
    pub format: RecordFormat,
    /// Record length (the largest).
    pub len: usize,
    pub input: bool,
    pub output: bool,
    pub fields: Vec<RecordField>,
    /// Indexed files: the primary key and the alternate keys (offset,
    /// length, duplicates allowed).
    pub keys: Vec<(usize, usize, bool)>,
}

impl FileSpec {
    pub fn indexed(&self) -> bool {
        !self.keys.is_empty()
    }
}

/// The time of day of every run (2025-01-01 09:00:00 UTC): cases repeat
/// whatever the program does with the date.
pub const CLOCK: u64 = 1_735_722_000;

/// Whether a direction is of a text statement's "is a delimiter here"
/// decision (not its length or NUMVAL ones).
fn delimiter_decision(d: &Dir) -> bool {
    d.0.ends_with("#TEXT") && d.1 % 4096 < 4094
}

/// The solver's time for the first attempt at a direction, and for the
/// later ones and other contexts.
const SOLVE_TIME: Duration = Duration::from_millis(150);
const RETRY_TIME: Duration = Duration::from_millis(50);

/// Conditions to solve for a run's inputs.
struct Job {
    cons: Vec<Pred>,
    /// The solver's time.
    time: Duration,
    /// The inputs found go to the queue of the text statements.
    text: bool,
    /// The direction aimed at (an attempt counted for it).
    target: Option<Dir>,
}

/// Solves the jobs, on as many threads as there are solvers.
fn solve_all(solvers: &mut [Builtin], jobs: &[Job], vars: &[Var], start: &[i128]) -> Vec<Outcome> {
    let one = |solver: &mut Builtin, job: &Job| {
        solver.time = job.time;
        // Z3 for the first attempt at a direction only
        solver.use_smt = job.time == SOLVE_TIME && job.target.is_some();
        let r = solve(solver, &job.cons, vars, start);
        solver.time = SOLVE_TIME;
        r
    };
    if solvers.len() == 1 || jobs.len() <= 1 {
        return jobs.iter().map(|j| one(&mut solvers[0], j)).collect();
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<Outcome>>> =
        jobs.iter().map(|_| std::sync::Mutex::new(None)).collect();
    std::thread::scope(|sc| {
        for solver in solvers.iter_mut() {
            let (next, results) = (&next, &results);
            sc.spawn(move || {
                loop {
                    let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(job) = jobs.get(k) else { break };
                    let r = one(solver, job);
                    if let Ok(mut slot) = results[k].lock() {
                        *slot = Some(r);
                    }
                }
            });
        }
    });
    results
        .into_iter()
        .map(|m| m.into_inner().ok().flatten().unwrap_or(Outcome::Unknown))
        .collect()
}

/// Tries of an enabler for a direction it may enable (see
/// `infeasible::enablers`).
const ENABLE_TRIES: u32 = 3;

/// Instructions a run may take at least before it is taken for an
/// endless loop (see `generate`).
const LIMIT_FLOOR: u64 = 200_000;

/// Decisions of text statements solved for in a generation (at most).
const TEXT_FLIPS: usize = 400;

/// Times of day the mutations try (boundaries of days, months, years and
/// leap days), seconds since 1970-01-01 UTC.
pub const CLOCKS: [u64; 8] = [
    1_735_689_599, // 2024-12-31 23:59:59
    1_709_208_000, // 2024-02-29 12:00:00
    1_738_281_600, // 2025-01-31 00:00:00
    1_740_700_800, // 2025-02-28 00:00:00
    1_751_241_600, // 2025-06-30 00:00:00
    1_764_547_200, // 2025-12-01 00:00:00
    1_767_182_400, // 2025-12-31 12:00:00
    946_684_800,   // 2000-01-01 00:00:00
];

/// Whether the programs look at the time of day.
fn uses_clock(module: &Module) -> bool {
    module.programs.iter().any(|p| {
        p.code.iter().any(|i| {
            matches!(i, Instr::Accept { from, .. } if *from != AcceptFrom::Stdin) || {
                let t = serde_json::to_string(i).unwrap_or_default();
                t.contains("CurrentDate") || t.contains("CEELOCT") || t.contains("CEEGMT")
            }
        }) || p.execs.iter().any(|e| e.command == "ASKTIME")
    })
}

/// Records an input file has at most.
pub const MAX_RECORDS: usize = 99;

/// The files of the module's programs (by DD name, once).
pub fn file_specs(module: &Module, search: &[PathBuf]) -> Vec<FileSpec> {
    let mut out: Vec<FileSpec> = Vec::new();
    let called = called_modules(module, search);
    let programs = module
        .programs
        .iter()
        .chain(called.iter().flat_map(|m| m.programs.iter()));
    for p in programs {
        for (f, def) in p.files.iter().enumerate() {
            if out
                .iter()
                .any(|x| x.assign.eq_ignore_ascii_case(&def.assign))
            {
                continue;
            }
            let text = def.print || def.org == Org::LineSequential;
            let format = if text {
                RecordFormat::Text
            } else if def.recording_mode == Some('F')
                || (def.recording_mode.is_none()
                    && def.min_len == def.max_len
                    && def.record_depending.is_none())
            {
                RecordFormat::Fixed(def.max_len)
            } else {
                RecordFormat::Variable
            };
            let opens = |m: &[OpenMode]| {
                p.code
                    .iter()
                    .any(|i| matches!(i, Instr::Open(x, o) if *x as usize == f && m.contains(o)))
            };
            let indexed = def.org == Org::Indexed;
            let input = (def.org == Org::Sequential && !text && opens(&[OpenMode::Input]))
                || (indexed && opens(&[OpenMode::Input, OpenMode::InputOutput]));
            let output = (matches!(def.org, Org::Sequential | Org::LineSequential)
                && opens(&[OpenMode::Output, OpenMode::Extend]))
                || (indexed && opens(&[OpenMode::Output, OpenMode::InputOutput, OpenMode::Extend]));
            let mut fields = Vec::new();
            let mut keys = Vec::new();
            if indexed && let Some(rec) = p.descs.get(def.record as usize) {
                let key = |l: &zeus_cobol::zir::Loc| {
                    let d = &p.descs[l.desc as usize];
                    (d.offset.saturating_sub(rec.offset) as usize, d.len as usize)
                };
                if let Some(l) = &def.record_key {
                    let (o, n) = key(l);
                    keys.push((o, n, false));
                    for (l, dup) in &def.alternate_keys {
                        let (o, n) = key(l);
                        keys.push((o, n, *dup));
                    }
                }
            }
            if input && let Some(rec) = p.descs.get(def.record as usize) {
                let descs: Vec<_> = p.descs.iter().filter(|d| d.base == rec.base).collect();
                for (name, offset, len, num) in
                    items(&descs, rec.offset as usize, def.max_len as usize)
                {
                    fields.push(RecordField {
                        name,
                        offset,
                        len,
                        num,
                    });
                }
            }
            out.push(FileSpec {
                name: def.name.clone(),
                assign: def.assign.clone(),
                format,
                len: def.max_len as usize,
                input,
                output,
                fields,
                keys,
            });
        }
    }
    out
}

/// The modules (.zo in `search`) of the programs the module's programs
/// call by name, and of those they call.
pub fn called_modules(module: &Module, search: &[PathBuf]) -> Vec<Module> {
    use zeus_cobol::zir::Src;
    let mut out: Vec<Module> = Vec::new();
    let mut seen: HashSet<String> = module.programs.iter().map(|p| p.name.clone()).collect();
    let mut todo: Vec<String> = Vec::new();
    let calls = |m: &Module, todo: &mut Vec<String>| {
        for p in &m.programs {
            for i in &p.code {
                if let Instr::Call {
                    program: Src::Alnum(b),
                    ..
                } = i
                {
                    todo.push(ebcdic::decode(b).trim().to_ascii_uppercase());
                }
            }
        }
    };
    calls(module, &mut todo);
    while let Some(name) = todo.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let found = search.iter().find_map(|d| {
            [
                format!("{name}.zo"),
                format!("{}.zo", name.to_ascii_lowercase()),
            ]
            .iter()
            .map(|f| d.join(f))
            .find(|p| p.is_file())
        });
        if let Some(path) = found
            && let Ok(m) = zeus_cobol::load_module(&path)
        {
            calls(&m, &mut todo);
            out.push(m);
        }
    }
    out
}

/// A record with every numeric field zero and the rest spaces.
pub fn default_record(f: &FileSpec) -> Vec<u8> {
    let mut r = vec![0x40; f.len];
    for rf in &f.fields {
        if let Some(t) = &rf.num {
            vdata::store_raw(t, &mut r[rf.offset..rf.offset + rf.len], 0);
        }
    }
    r
}

/// The records of the indexed input files without the ones whose primary
/// key or unique alternate key an earlier record has (a data set cannot
/// hold them).
fn normalize(inputs: &mut Inputs, specs: &[FileSpec]) {
    for (f, recs) in specs
        .iter()
        .filter(|f| f.input)
        .zip(inputs.files.iter_mut())
    {
        recs.truncate(MAX_RECORDS);
        if !f.indexed() {
            continue;
        }
        let mut seen: HashSet<(usize, Vec<u8>)> = HashSet::new();
        recs.retain(|r| {
            let ks: Vec<(usize, Vec<u8>)> = f
                .keys
                .iter()
                .enumerate()
                .filter(|(_, k)| !k.2)
                .filter_map(|(i, &(o, l, _))| Some((i, r.get(o..o + l)?.to_vec())))
                .collect();
            if ks.iter().any(|k| seen.contains(k)) {
                return false;
            }
            seen.extend(ks);
            true
        });
    }
}

/// Inputs with every numeric field zero and the others spaces.
pub fn default_inputs(fields: &[Field], lens: &[usize]) -> Inputs {
    let mut params: Vec<Vec<u8>> = lens.iter().map(|&l| vec![0x40; l]).collect();
    for f in fields {
        if let Some(t) = &f.num {
            vdata::store_raw(t, &mut params[f.param][f.offset..f.offset + f.len], 0);
        }
    }
    Inputs {
        params,
        ..Default::default()
    }
}

/// The integer of a numeric field's digits.
fn raw(t: &NumType, bytes: &[u8]) -> i128 {
    let v = vdata::num_value(t, bytes);
    v.rescale(t.scale.max(0) as u32, Rounding::Truncate)
        .map_or(0, |d| d.coef)
}

#[derive(Clone)]
struct Buf(Rc<RefCell<Vec<u8>>>);

impl Write for Buf {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Runs `program` of `module` on `inputs`: with concolic execution when
/// `symbolic`, and recording coverage when it is on.
pub fn run(
    module: &Rc<Module>,
    program: &str,
    fields: &[Field],
    inputs: &Inputs,
    opts: &Options,
    symbolic: bool,
) -> RunOut {
    zeus_cobol::clock::fix(Some(Duration::from_secs(inputs.clock.unwrap_or(CLOCK))));
    let mut m = Machine::new(opts.search.clone());
    m.add_module(module.clone());
    let specs = file_specs(module, &opts.search);
    let dir = scratch_dir();
    let mut k = 0;
    for f in &specs {
        let path = dir.join(&f.assign);
        if f.input {
            let mut data = Vec::new();
            let format = if f.indexed() {
                RecordFormat::Variable
            } else {
                f.format
            };
            for r in inputs.files.get(k).map(|r| &r[..]).unwrap_or(&[]) {
                zeus_ds::format::encode_into(format, r, 0x40, &mut data);
            }
            k += 1;
            let _ = std::fs::write(&path, data);
        }
        m.allocate(
            &f.assign,
            Allocation {
                path,
                format: (f.format != RecordFormat::Text && !f.indexed()).then_some(f.format),
            },
        );
    }
    let out = Buf(Rc::new(RefCell::new(Vec::new())));
    m.set_stdout(Box::new(out.clone()));
    m.set_stdin(inputs.accept.clone());
    m.set_io_faults(inputs.faults.clone());
    m.set_call_stubs(inputs.stubs.clone());
    m.set_forced_stubs(opts.stub.clone());
    let addrs: Vec<usize> = inputs
        .params
        .iter()
        .map(|b| {
            let a = m.getmain(b.len().max(1), 0);
            m.storage_mut()[a..a + b.len()].copy_from_slice(b);
            a
        })
        .collect();
    let host = uses_exec(module, &opts.search).then(|| {
        let mut h = StubHost::new(inputs.exec.clone());
        h.module = Some(module.clone());
        h.eib = eib_param(module, program).and_then(|k| addrs.get(k).copied());
        Rc::new(h)
    });
    if let Some(h) = &host {
        m.set_exec_host(h.clone());
    }
    // replays too stop at the instruction limit (with no variables)
    m.set_symbolic(SymState::new(opts.limit));
    if symbolic {
        for f in fields {
            let a = addrs[f.param] + f.offset;
            let bytes = inputs.params[f.param][f.offset..f.offset + f.len].to_vec();
            match &f.num {
                Some(t) => {
                    let st = m.symbolic().unwrap();
                    let v = st.var(
                        format!("P{}.{}", f.param + 1, f.name),
                        VarKind::Num {
                            digits: t.digits,
                            signed: t.signed,
                        },
                        raw(t, &bytes),
                    );
                    m.sym_bind_num(a, f.len, *t, v);
                }
                None => {
                    for (i, b) in bytes.iter().enumerate() {
                        let st = m.symbolic().unwrap();
                        let v = st.var(
                            format!("P{}.{}#{}", f.param + 1, f.name, i + 1),
                            VarKind::Byte,
                            i128::from(*b),
                        );
                        m.sym_bind_byte(a + i, v);
                    }
                }
            }
        }
        let st = m.symbolic().unwrap();
        for (k, f) in specs.iter().filter(|f| f.input).enumerate() {
            let n = inputs.files.get(k).map_or(0, |r| r.len());
            let count = st.var(
                format!("F{}.N", f.name),
                VarKind::Num {
                    digits: 2,
                    signed: false,
                },
                n as i128,
            );
            let records = inputs.files.get(k).cloned().unwrap_or_default();
            st.files.push(FileInput {
                name: f.name.clone(),
                fields: f.fields.clone(),
                count,
                keys: f.keys.iter().map(|&(o, l, _)| (o, l)).collect(),
                blank: if f.indexed() && records.len() < MAX_RECORDS {
                    default_record(f)
                } else {
                    Vec::new()
                },
                records: if f.indexed() { records } else { Vec::new() },
                ..Default::default()
            });
        }
    }
    let args: Vec<Option<usize>> = addrs.iter().map(|a| Some(*a)).collect();
    let r = m.run_program(program, &args);
    m.flush_coverage();
    let error = match r {
        Ok(Some(_)) => None,
        Ok(None) => Some(format!("program {program} not found")),
        Err(e) => Some(e.0),
    };
    let params = inputs
        .params
        .iter()
        .zip(&addrs)
        .map(|(b, &a)| m.storage()[a..a + b.len()].to_vec())
        .collect();
    let rc = m.return_code();
    let stubbed = m.stub_calls().into_iter().map(|c| c.0).collect();
    let st = m.take_symbolic().unwrap_or_default();
    let instructions = st.steps;
    if std::env::var_os("ZTESTGEN_DEBUG").is_some() {
        eprintln!(
            "  state: {} vars, {} steps, {} instructions",
            st.vars.len(),
            st.path.len(),
            st.steps
        );
    }
    drop(m);
    let exec = host.map(|h| h.log.take()).unwrap_or_default();
    let files = specs
        .iter()
        .filter(|f| f.output)
        .map(|f| {
            let data = std::fs::read(dir.join(&f.assign)).unwrap_or_default();
            let format = if f.indexed() {
                RecordFormat::Variable
            } else {
                f.format
            };
            let recs = zeus_ds::format::decode(format, &data).unwrap_or_default();
            (f.name.clone(), recs)
        })
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    zeus_cobol::clock::fix(None);
    let branches = st
        .path
        .iter()
        .filter_map(|s| match s {
            Step::Branch(b) => Some((b.program.clone(), b.pc, b.jumped)),
            _ => None,
        })
        .collect();
    let display = String::from_utf8_lossy(&out.0.borrow()).into_owned();
    RunOut {
        path: st.path,
        vars: st.vars,
        branches,
        display,
        params,
        return_code: rc,
        error,
        files,
        stubbed,
        exec,
        instructions,
    }
}

/// Whether the module's programs (or those they call) have EXEC
/// statements.
fn uses_exec(module: &Module, search: &[PathBuf]) -> bool {
    module.programs.iter().any(|p| !p.execs.is_empty())
        || called_modules(module, search)
            .iter()
            .any(|m| m.programs.iter().any(|p| !p.execs.is_empty()))
}

/// The USING parameter of a program that is its EIB (DFHEIBLK).
fn eib_param(module: &Module, program: &str) -> Option<usize> {
    let code = module.programs.iter().find(|p| p.name == program)?;
    code.using.iter().position(|slot| {
        code.descs
            .iter()
            .any(|d| d.base == Base::Linkage(*slot) && d.offset == 0 && d.name == "DFHEIBLK")
    })
}

/// A new empty directory for the data sets of a run.
fn scratch_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "ztestgen-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// A generated test case: its inputs, what the program did, and the
/// branch directions it covers.
#[derive(Debug, Clone)]
pub struct Case {
    pub inputs: Inputs,
    pub out: RunOut,
    pub covers: Vec<Dir>,
}

/// Why a branch direction has no test case.
#[derive(Debug, Clone, PartialEq)]
pub enum Gap {
    /// The instruction never ran.
    NotReached,
    /// Its condition did not depend on the inputs (or was not followed).
    NotSymbolic,
    /// The solver proved every path to it infeasible.
    Infeasible,
    /// No inputs found within the attempts.
    NotFound,
    /// Impossible by static analysis (the values a flag can hold along
    /// the control flow, see `infeasible`).
    Impossible,
}

pub struct Report {
    pub cases: Vec<Case>,
    /// Every branch direction of the programs, covered or not.
    pub all: Vec<Dir>,
    pub gaps: Vec<(Dir, Gap)>,
    pub runs: usize,
    pub fields: Vec<Field>,
    pub files: Vec<FileSpec>,
    /// The programs called that ran as stubs (in some run).
    pub stubbed: BTreeSet<String>,
    /// Z3 helped the built-in solver.
    pub smt: bool,
}

/// The conditional jumps of the module's programs.
pub fn directions(module: &Module) -> Vec<Dir> {
    let mut out = Vec::new();
    for p in &module.programs {
        for (pc, ins) in p.code.iter().enumerate() {
            if matches!(ins, Instr::JumpIf(..) | Instr::JumpUnless(..)) {
                out.push((p.name.clone(), pc as u32, true));
                out.push((p.name.clone(), pc as u32, false));
            }
        }
    }
    out
}

/// The conditions a path keeps up to step `i`, and the other way of the
/// branch at `i`; only those sharing variables with that branch.
fn conditions(path: &[Step], i: usize) -> Option<Vec<Pred>> {
    let Step::Branch(b) = &path[i] else {
        return None;
    };
    let p = b.pred.as_ref()?;
    let target = if b.jumped {
        Pred::not(p.clone())
    } else {
        p.clone()
    };
    let mut all: Vec<Pred> = Vec::new();
    for s in &path[..i] {
        match s {
            Step::Assume(p) => all.push(p.clone()),
            Step::Branch(x) => {
                if let Some(p) = &x.pred {
                    all.push(if x.jumped {
                        p.clone()
                    } else {
                        Pred::not(p.clone())
                    });
                }
            }
        }
    }
    // the conditions connected to the target through their variables
    let mut vars = Vec::new();
    target.vars(&mut vars);
    let mut vset: HashSet<u32> = vars.into_iter().collect();
    let mut keep = vec![false; all.len()];
    loop {
        let mut grew = false;
        for (k, c) in all.iter().enumerate() {
            if keep[k] {
                continue;
            }
            let mut v = Vec::new();
            c.vars(&mut v);
            if v.iter().any(|x| vset.contains(x)) {
                keep[k] = true;
                vset.extend(v);
                grew = true;
            }
        }
        if !grew {
            break;
        }
    }
    let mut out: Vec<Pred> = all
        .into_iter()
        .zip(keep)
        .filter(|(_, k)| *k)
        .map(|(c, _)| c)
        .collect();
    // the same condition is kept once
    let mut seen = HashSet::new();
    out.retain(|c| seen.insert(c.clone()));
    out.push(target);
    Some(out)
}

/// The conditions of a run's path connected through their variables,
/// built step by step (union-find over the variables, each set with its
/// steps): the slice for a step is the sets of the target's variables
/// among the steps before it. Gives what `conditions` gives, for steps
/// asked in increasing order.
struct Slicer<'a> {
    path: &'a [Step],
    /// Steps added (those before `next`).
    next: usize,
    parent: Vec<u32>,
    /// The steps of each set (at its root).
    steps: Vec<Vec<u32>>,
}

impl<'a> Slicer<'a> {
    fn new(path: &'a [Step], vars: usize) -> Self {
        Slicer {
            path,
            next: 0,
            parent: (0..vars as u32).collect(),
            steps: vec![Vec::new(); vars],
        }
    }

    fn find(&mut self, mut v: u32) -> u32 {
        while self.parent[v as usize] != v {
            let p = self.parent[v as usize];
            self.parent[v as usize] = self.parent[p as usize];
            v = p;
        }
        v
    }

    fn union(&mut self, a: u32, b: u32) -> u32 {
        let (a, b) = (self.find(a), self.find(b));
        if a == b {
            return a;
        }
        // the larger set's root stays
        let (big, small) = if self.steps[a as usize].len() >= self.steps[b as usize].len() {
            (a, b)
        } else {
            (b, a)
        };
        self.parent[small as usize] = big;
        let moved = std::mem::take(&mut self.steps[small as usize]);
        self.steps[big as usize].extend(moved);
        big
    }

    fn pred(step: &Step) -> Option<Pred> {
        match step {
            Step::Assume(p) => Some(p.clone()),
            Step::Branch(x) => x.pred.as_ref().map(|p| {
                if x.jumped {
                    p.clone()
                } else {
                    Pred::not(p.clone())
                }
            }),
        }
    }

    fn vars_of(step: &Step) -> Vec<u32> {
        let mut v = Vec::new();
        match step {
            Step::Assume(p) => p.vars(&mut v),
            Step::Branch(x) => {
                if let Some(p) = &x.pred {
                    p.vars(&mut v);
                }
            }
        }
        v
    }

    /// The conditions before step `i` connected to the other way of the
    /// branch at `i`, then that other way.
    fn slice(&mut self, i: usize) -> Option<Vec<Pred>> {
        let t = Instant::now();
        let r = self.slice_timed(i);
        prof_add(1, t.elapsed());
        r
    }

    fn slice_timed(&mut self, i: usize) -> Option<Vec<Pred>> {
        if i < self.next {
            return conditions(self.path, i);
        }
        while self.next < i {
            let k = self.next;
            self.next += 1;
            let vs = Self::vars_of(&self.path[k]);
            let Some((&first, rest)) = vs.split_first() else {
                continue;
            };
            let mut root = self.find(first);
            for &v in rest {
                root = self.union(root, v);
            }
            self.steps[root as usize].push(k as u32);
        }
        let Step::Branch(b) = &self.path[i] else {
            return None;
        };
        let p = b.pred.as_ref()?;
        let target = if b.jumped {
            Pred::not(p.clone())
        } else {
            p.clone()
        };
        let mut vs = Vec::new();
        target.vars(&mut vs);
        let mut roots: Vec<u32> = vs.into_iter().map(|v| self.find(v)).collect();
        roots.sort();
        roots.dedup();
        let mut idx: Vec<u32> = roots
            .iter()
            .flat_map(|r| self.steps[*r as usize].iter().copied())
            .collect();
        idx.sort();
        let mut out: Vec<Pred> = Vec::with_capacity(idx.len() + 1);
        let mut seen = HashSet::new();
        for k in idx {
            if let Some(c) = Self::pred(&self.path[k as usize])
                && seen.insert(c.clone())
            {
                out.push(c);
            }
        }
        out.push(target);
        Some(out)
    }
}

/// Inputs with the variables' values from a solution.
fn apply(
    inputs: &Inputs,
    fields: &[Field],
    specs: &[FileSpec],
    vars: &[Var],
    values: &[i128],
) -> Inputs {
    let t = Instant::now();
    let r = apply_timed(inputs, fields, specs, vars, values);
    prof_add(2, t.elapsed());
    r
}

fn apply_timed(
    inputs: &Inputs,
    fields: &[Field],
    specs: &[FileSpec],
    vars: &[Var],
    values: &[i128],
) -> Inputs {
    let mut new = inputs.clone();
    let by_name: HashMap<&str, i128> = vars
        .iter()
        .zip(values)
        .map(|(v, x)| (v.name.as_str(), *x))
        .collect();
    for f in fields {
        let p = f.param + 1;
        match &f.num {
            Some(t) => {
                if let Some(&x) = by_name.get(format!("P{p}.{}", f.name).as_str()) {
                    vdata::store_raw(t, &mut new.params[f.param][f.offset..f.offset + f.len], x);
                }
            }
            None => {
                for i in 0..f.len {
                    if let Some(&x) = by_name.get(format!("P{p}.{}#{}", f.name, i + 1).as_str()) {
                        new.params[f.param][f.offset + i] = x as u8;
                    }
                }
            }
        }
    }
    // ACCEPTed lines: ACCEPTn.i is byte i of line n
    let mut lines: HashMap<usize, Vec<u8>> = HashMap::new();
    for (v, &x) in vars.iter().zip(values) {
        if let Some(rest) = v.name.strip_prefix("ACCEPT")
            && let Some((n, i)) = rest.split_once('.')
            && let (Ok(n), Ok(i)) = (n.parse::<usize>(), i.parse::<usize>())
        {
            let l = lines.entry(n).or_default();
            if l.len() < i {
                l.resize(i, 0x40);
            }
            l[i - 1] = x as u8;
        }
    }
    for (n, bytes) in lines {
        while new.accept.len() < n {
            new.accept.push(String::new());
        }
        new.accept[n - 1] = ebcdic::decode(&bytes).trim_end().to_string();
    }
    // input files: F<file>.N records, F<file>.R<n>.<field>[#i]
    for (k, f) in specs.iter().filter(|f| f.input).enumerate() {
        let Some(recs) = new.files.get_mut(k) else {
            continue;
        };
        if let Some(&n) = by_name.get(format!("F{}.N", f.name).as_str()) {
            recs.resize(n.clamp(0, MAX_RECORDS as i128) as usize, default_record(f));
        }
        for (n, r) in recs.iter_mut().enumerate() {
            let pre = format!("F{}.R{}.", f.name, n + 1);
            for rf in &f.fields {
                match &rf.num {
                    Some(t) => {
                        if let Some(&x) = by_name.get(format!("{pre}{}", rf.name).as_str()) {
                            vdata::store_raw(t, &mut r[rf.offset..rf.offset + rf.len], x);
                        }
                    }
                    None => {
                        for i in 0..rf.len {
                            if let Some(&x) =
                                by_name.get(format!("{pre}{}#{}", rf.name, i + 1).as_str())
                            {
                                r[rf.offset + i] = x as u8;
                            }
                        }
                    }
                }
            }
        }
    }
    // I/O statements made to fail: F<file>.<KIND><n> is 1 or 2 (other
    // values: it does not fail)
    // stubbed calls: C<program>.<n>.A<k>#<i> is byte i of argument k
    let mut outs: BTreeMap<StubArg, Vec<(usize, u8, bool)>> = BTreeMap::new();
    for (v, &x) in vars.iter().zip(values) {
        if let Some(f) = fault_of(&v.name) {
            if x == 1 || x == 2 {
                new.faults.insert(f, x as u8);
            } else {
                new.faults.remove(&f);
            }
        } else if let Some(rest) = v.name.strip_prefix('C')
            && let Some((head, i)) = rest.rsplit_once('#')
            && let Some((head, k)) = head.rsplit_once(".A")
            && let Some((prog, n)) = head.rsplit_once('.')
            && let (Ok(n), Ok(k), Ok(i)) = (n.parse(), k.parse(), i.parse::<usize>())
        {
            outs.entry((prog.to_string(), n, k))
                .or_default()
                .push((i, x as u8, x != v.value));
        }
    }
    for (key, bytes) in outs {
        // kept when the solution changes it, or it was set already
        if !bytes.iter().any(|b| b.2) && !new.stubs.contains_key(&key) {
            continue;
        }
        let mut out = vec![0x40u8; bytes.iter().map(|b| b.0).max().unwrap_or(0)];
        for (i, x, _) in bytes {
            out[i - 1] = x;
        }
        new.stubs.insert(key, out);
    }
    // EXEC statements: X<key> (a number) or X<key>#<i> (byte i)
    let mut areas: BTreeMap<String, Vec<(usize, u8, bool)>> = BTreeMap::new();
    for (v, &x) in vars.iter().zip(values) {
        let Some(rest) = v.name.strip_prefix('X') else {
            continue;
        };
        match rest.rsplit_once('#') {
            Some((key, i)) => {
                if let Ok(i) = i.parse::<usize>() {
                    areas
                        .entry(key.to_string())
                        .or_default()
                        .push((i, x as u8, x != v.value));
                }
            }
            None => {
                if x != v.value || new.exec.contains_key(rest) {
                    new.exec.insert(rest.to_string(), Out::Num(x));
                }
            }
        }
    }
    for (key, bytes) in areas {
        if !bytes.iter().any(|b| b.2) && !new.exec.contains_key(&key) {
            continue;
        }
        let mut out = vec![0x40u8; bytes.iter().map(|b| b.0).max().unwrap_or(0)];
        for (i, x, _) in bytes {
            out[i - 1] = x;
        }
        new.exec.insert(key, Out::Hex(crate::case::hex(&out)));
    }
    normalize(&mut new, specs);
    new
}

// Time spent solving, slicing and applying solutions (ZTESTGEN_PROFILE).
thread_local! {
    static PROF: std::cell::Cell<[Duration; 3]> = const { std::cell::Cell::new([Duration::ZERO; 3]) };
}

fn prof_add(k: usize, d: Duration) {
    PROF.with(|p| {
        let mut a = p.get();
        a[k] += d;
        p.set(a);
    });
}

/// Solves `cons`, first with no I/O statement made to fail that does not
/// fail already (inputs as the program usually gets them), then with any.
fn solve(solver: &mut Builtin, cons: &[Pred], vars: &[Var], start: &[i128]) -> Outcome {
    let t = Instant::now();
    let r = solve_timed(solver, cons, vars, start);
    prof_add(0, t.elapsed());
    r
}

fn solve_timed(solver: &mut Builtin, cons: &[Pred], vars: &[Var], start: &[i128]) -> Outcome {
    let t = Instant::now();
    let r = solve_inner(solver, cons, vars, start);
    if std::env::var_os("ZTESTGEN_DEBUG").is_some() {
        eprintln!(
            "solve took {:?} ({} conditions) {}",
            t.elapsed(),
            cons.len(),
            match &r {
                Outcome::Sat(_) => "sat",
                Outcome::Unsat => "unsat",
                Outcome::Unknown => "unknown",
            }
        );
    }
    r
}

fn solve_inner(solver: &mut Builtin, cons: &[Pred], vars: &[Var], start: &[i128]) -> Outcome {
    use zeus_cobol::concolic::Term;
    let pins: Vec<Pred> = vars
        .iter()
        .enumerate()
        .filter(|(_, v)| v.value == 0 && fault_of(&v.name).is_some())
        .map(|(i, _)| {
            Pred::Cmp(
                Term::Var(i as u32),
                zeus_cobol::zir::Rel::Eq,
                Term::Const(0),
            )
        })
        .collect();
    if !pins.is_empty() {
        let mut first = cons.to_vec();
        first.extend(pins);
        if let r @ Outcome::Sat(_) = solver.solve(&first, vars, start) {
            return r;
        }
    }
    solver.solve(cons, vars, start)
}

/// The I/O statement of a variable `F<file>.<KIND><n>`.
fn fault_of(name: &str) -> Option<(String, String, u32)> {
    let (file, last) = name.strip_prefix('F')?.split_once('.')?;
    let d = last.find(|c: char| c.is_ascii_digit())?;
    let (kind, n) = last.split_at(d);
    if !zeus_cobol::vm::testenv::FAULT_KINDS.contains(&kind) {
        return None;
    }
    Some((file.to_string(), kind.to_string(), n.parse().ok()?))
}

fn next(s: &mut u64) -> u64 {
    *s ^= *s >> 12;
    *s ^= *s << 25;
    *s ^= *s >> 27;
    s.wrapping_mul(0x2545_F491_4F6C_DD1D)
}

/// The literals of a module's programs (alphanumeric bytes, numbers as
/// value and scale): values the mutations try.
#[derive(Default)]
pub struct Dictionary {
    pub text: Vec<Vec<u8>>,
    pub nums: Vec<(i128, u32)>,
    /// The programs look at the time of day (it is an input then).
    pub clock: bool,
}

impl Dictionary {
    pub fn of(module: &Module) -> Self {
        use zeus_cobol::zir::{Cx, Src};
        fn src(s: &Src, d: &mut Dictionary) {
            match s {
                Src::Alnum(b) if !b.is_empty() && !d.text.contains(b) => d.text.push(b.clone()),
                Src::NumLit(v, _) if !d.nums.contains(&(v.coef, v.scale)) => {
                    d.nums.push((v.coef, v.scale))
                }
                _ => {}
            }
        }
        fn cx(c: &Cx, d: &mut Dictionary) {
            match c {
                Cx::Cmp(a, _, b, _) => {
                    src(a, d);
                    src(b, d);
                }
                Cx::Not(x) => cx(x, d),
                Cx::And(a, b) | Cx::Or(a, b) => {
                    cx(a, d);
                    cx(b, d);
                }
                _ => {}
            }
        }
        let mut d = Dictionary {
            clock: uses_clock(module),
            ..Default::default()
        };
        for p in &module.programs {
            for ins in &p.code {
                match ins {
                    Instr::Move(s, _) => src(s, &mut d),
                    Instr::JumpIf(c, _) | Instr::JumpUnless(c, _) => cx(c, &mut d),
                    _ => {}
                }
            }
        }
        d
    }
}

/// Inputs with one field (or an ACCEPTed line) changed to a value often
/// found at boundaries, or to a literal of the program.
fn mutate(
    base: &Inputs,
    fields: &[Field],
    specs: &[FileSpec],
    dict: &Dictionary,
    rng: &mut u64,
) -> Inputs {
    let mut new = base.clone();
    // another time of day, at a boundary of days, months or years
    if dict.clock && next(rng).is_multiple_of(5) {
        new.clock = Some(CLOCKS[next(rng) as usize % CLOCKS.len()]);
        return new;
    }
    let inputs: Vec<&FileSpec> = specs.iter().filter(|f| f.input).collect();
    let k = next(rng) as usize % (fields.len() + new.accept.len() + inputs.len()).max(1);
    if k >= fields.len() + new.accept.len() && !inputs.is_empty() {
        // an input file: another number of records, or a field of a record
        let fk = k - fields.len() - new.accept.len();
        let spec = inputs[fk];
        let recs = &mut new.files[fk];
        if recs.is_empty() || spec.fields.is_empty() || next(rng).is_multiple_of(3) {
            let n = [0, 1, 2, 3, recs.len() + 1, recs.len().saturating_sub(1)];
            let n = n[next(rng) as usize % n.len()].min(MAX_RECORDS);
            let old = recs.len();
            recs.resize(n, default_record(spec));
            // records added to an indexed file need keys of their own
            if let Some(&(ko, kl, _)) = spec.keys.first() {
                let key: Vec<Field> = spec
                    .fields
                    .iter()
                    .filter(|rf| rf.offset < ko + kl && ko < rf.offset + rf.len)
                    .map(|rf| Field {
                        param: 0,
                        name: rf.name.clone(),
                        offset: rf.offset,
                        len: rf.len,
                        num: rf.num,
                    })
                    .collect();
                for r in recs.iter_mut().skip(old) {
                    for f in &key {
                        let one = Inputs {
                            params: vec![r.clone()],
                            ..Default::default()
                        };
                        *r = mutate(&one, std::slice::from_ref(f), &[], dict, rng)
                            .params
                            .remove(0);
                    }
                }
            }
            normalize(&mut new, specs);
            return new;
        }
        let r = next(rng) as usize % recs.len();
        let rf = &spec.fields[next(rng) as usize % spec.fields.len()];
        let field = Field {
            param: 0,
            name: rf.name.clone(),
            offset: rf.offset,
            len: rf.len,
            num: rf.num,
        };
        let mut one = Inputs {
            params: vec![recs[r].clone()],
            ..Default::default()
        };
        one = mutate(&one, &[field], &[], dict, rng);
        recs[r] = one.params.remove(0);
        normalize(&mut new, specs);
        return new;
    }
    if let Some(f) = fields.get(k) {
        let b = &mut new.params[f.param][f.offset..f.offset + f.len];
        match &f.num {
            Some(t) => {
                let max = 10i128.pow(t.digits.min(30)) - 1;
                let picks = [0, 1, 2, 3, 5, 7, 10, 20, 50, 99, 100, 1000, max, max / 2];
                let mut x = picks[next(rng) as usize % picks.len()].min(max);
                if next(rng).is_multiple_of(4) {
                    x = (next(rng) as i128).rem_euclid(max + 1);
                }
                if t.signed && next(rng).is_multiple_of(3) {
                    x = -x;
                }
                // a numeric literal of the program, at the field's scale
                if !dict.nums.is_empty() && next(rng).is_multiple_of(3) {
                    let (c, sc) = dict.nums[next(rng) as usize % dict.nums.len()];
                    let fs = t.scale.max(0) as u32;
                    x = if fs >= sc {
                        c.saturating_mul(10i128.pow((fs - sc).min(30)))
                    } else {
                        c / 10i128.pow((sc - fs).min(30))
                    };
                    x = x.clamp(-max, max);
                }
                vdata::store_raw(t, b, x);
            }
            None => {
                // an alphanumeric literal of the program
                if !dict.text.is_empty() && next(rng).is_multiple_of(3) {
                    let l = &dict.text[next(rng) as usize % dict.text.len()];
                    for (i, x) in b.iter_mut().enumerate() {
                        *x = l.get(i).copied().unwrap_or(0x40);
                    }
                    return new;
                }
                // spaces, digits or letters
                let kind = next(rng) % 3;
                for x in b.iter_mut() {
                    *x = match kind {
                        0 => 0x40,
                        1 => 0xF0 + (next(rng) % 10) as u8,
                        _ => 0xC1 + (next(rng) % 9) as u8,
                    };
                }
            }
        }
    } else if !new.accept.is_empty() {
        let n = k - fields.len();
        let picks = ["", "0", "1", "Y", "N", "A", "99999", "ZZZZ"];
        new.accept[n] = if !dict.text.is_empty() && next(rng).is_multiple_of(2) {
            ebcdic::decode(&dict.text[next(rng) as usize % dict.text.len()])
                .trim_end()
                .to_string()
        } else {
            picks[next(rng) as usize % picks.len()].to_string()
        };
    }
    new
}

/// Generates test cases for `program` of `module`.
pub fn generate(module: &Rc<Module>, program: &str, opts: &Options) -> Result<Report, String> {
    let code = module
        .programs
        .iter()
        .find(|p| p.name.eq_ignore_ascii_case(program))
        .ok_or_else(|| format!("no program {program} in the module"))?;
    let (fields, lens) = fields(code);
    let specs = file_specs(module, &opts.search);
    let all = directions(module);
    // decisions recorded outside the module's branches (EXEC responses)
    // are solved for, and kept by cases, but not counted
    let counted: HashSet<Dir> = all.iter().cloned().collect();
    // directions proved impossible without running (flags' values): for
    // the program run, entered once per run with its VALUEs; not when
    // nested programs may set its data (GLOBAL) or it is recursive
    let shared = code.recursive
        || module.programs.iter().any(|p| {
            p.descs
                .iter()
                .any(|d| matches!(d.base, Base::Outer(_) | Base::OuterLinkage(..)))
        });
    // for the directions of jumps on flags, the directions that lead to
    // the MOVEs of the values they need (enabler -> directions)
    let mut enables: HashMap<Dir, Vec<Dir>> = HashMap::new();
    if !shared {
        for ((pc, j), es) in crate::infeasible::enablers(code) {
            for (epc, ej) in es {
                enables
                    .entry((code.name.clone(), epc, ej))
                    .or_default()
                    .push((code.name.clone(), pc, j));
            }
        }
    }
    let mut enabled: HashMap<(Dir, Dir), u32> = HashMap::new();
    let mut impossible: HashSet<Dir> = if shared {
        HashSet::new()
    } else {
        crate::infeasible::impossible(code)
            .into_iter()
            .map(|(pc, j)| (code.name.clone(), pc, j))
            .collect()
    };
    let kept_dir = |d: &Dir| counted.contains(d) || d.0.ends_with("#EXEC");
    let start = Instant::now();
    // a solver per thread (each with its own Z3 context)
    let threads = opts.threads.max(1);
    let mut solvers: Vec<Builtin> = (0..threads)
        .map(|_| {
            let mut b = Builtin::default();
            if opts.smt {
                b.smt = crate::smt::Z3::load();
            }
            b
        })
        .collect();
    let smt = solvers[0].smt.is_some();
    let mut first = default_inputs(&fields, &lens);
    // a CICS program starts with a COMMAREA (EIBCALEN its length, at most
    // AREA_BYTES): the program reads what it moves from it
    if let Some(k) = eib_param(module, &code.name)
        && let Some(&calen) = lens.get(k + 1)
        && let Some(f) = fields.iter().find(|f| f.param == k && f.name == "EIBCALEN")
        && let Some(t) = &f.num
    {
        let n = calen.min(AREA_BYTES) as i128;
        vdata::store_raw(t, &mut first.params[k][f.offset..f.offset + f.len], n);
    }
    let accepts = module.programs.iter().any(|p| {
        p.code.iter().any(|i| {
            matches!(
                i,
                Instr::Accept {
                    from: AcceptFrom::Stdin,
                    ..
                }
            )
        })
    });
    if accepts {
        first.accept = vec![String::new(); opts.accept_lines];
    }
    // one record in each input file to start with
    first.files = specs
        .iter()
        .filter(|f| f.input)
        .map(|f| vec![default_record(f)])
        .collect();
    let mut queue: VecDeque<Inputs> = VecDeque::new();
    let mut seeds: Vec<Inputs> = Vec::new();
    for (k, c) in opts.seeds.iter().enumerate() {
        match crate::case::seed_inputs(c, &fields, &lens, &specs) {
            Ok(mut i) => {
                if accepts && i.accept.is_empty() {
                    i.accept = first.accept.clone();
                }
                seeds.push(i);
            }
            Err(e) => eprintln!("ztestgen: seed {}: {e}", k + 1),
        }
    }
    let mut seen: HashSet<Inputs> = HashSet::new();
    let mut covered: HashSet<Dir> = HashSet::new();
    let mut reached: HashSet<(String, u32)> = HashSet::new();
    let mut symbolic: HashSet<(String, u32)> = HashSet::new();
    let mut attempts: HashMap<Dir, (u32, u32)> = HashMap::new(); // (tries, unsat)
    let mut deepened: HashMap<(String, u32), u32> = HashMap::new();
    // covered directions solved for again in other contexts: target ->
    // the contexts (directions taken before it) tried
    let mut contexts: HashMap<Dir, HashSet<u64>> = HashMap::new();
    // decisions of text statements (UNSTRING delimiters...) solved for:
    // by the values the conditions before them have, at most TEXT_FLIPS
    let mut text_tried: HashSet<u64> = HashSet::new();
    let mut text_queue: VecDeque<Inputs> = VecDeque::new();
    let mut prof = [Duration::ZERO; 5];
    let mut longest: u64 = 0;
    // runs extended with one more delimiter: (program, statement,
    // delimiters found) -> how many
    let mut extended: HashMap<(String, u32, usize), u32> = HashMap::new();
    let mut cases: Vec<Case> = Vec::new();
    let mut runs = 0;
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let dict = Dictionary::of(module);
    // mutated inputs tried in a row without new coverage
    let mut fruitless = 0;
    let mut stubbed = BTreeSet::new();
    // the seeds run first without solving, so that the search knows what
    // they cover before it picks directions; then they run again with it
    for inp in seeds {
        if runs >= opts.max_runs || start.elapsed() > opts.time {
            break;
        }
        runs += 1;
        let out = run(module, program, &fields, &inp, opts, true);
        stubbed.extend(out.stubbed.iter().cloned());
        let mut new: Vec<Dir> = Vec::new();
        for d in &out.branches {
            if !covered.contains(d) && kept_dir(d) && !new.contains(d) {
                new.push(d.clone());
            }
        }
        covered.extend(out.branches.iter().cloned());
        for st in &out.path {
            if let Step::Branch(b) = st {
                reached.insert((b.program.clone(), b.pc));
                if b.pred.is_some() {
                    symbolic.insert((b.program.clone(), b.pc));
                }
            }
        }
        queue.push_back(inp.clone());
        if !new.is_empty() {
            cases.push(Case {
                inputs: inp,
                out,
                covers: new,
            });
        }
    }
    // a program that looks at the time of day also starts at the end of
    // a year
    if uses_clock(module) {
        let mut late = first.clone();
        late.clock = Some(CLOCKS[6]);
        queue.push_back(first);
        queue.push_back(late);
    } else {
        queue.push_back(first);
    }
    loop {
        if runs >= opts.max_runs || start.elapsed() > opts.time {
            break;
        }
        let (inp, fuzz) = match queue.pop_front().or_else(|| text_queue.pop_front()) {
            Some(i) => (i, false),
            None => {
                // the solver has nothing more: mutate the inputs of a case
                // while some branch direction that runs is not covered
                let open = all
                    .iter()
                    .any(|d| !covered.contains(d) && reached.contains(&(d.0.clone(), d.1)));
                if !open || fruitless >= opts.fuzz || cases.is_empty() {
                    break;
                }
                let base = &cases[(next(&mut rng) % cases.len() as u64) as usize].inputs;
                (mutate(base, &fields, &specs, &dict, &mut rng), true)
            }
        };
        if !seen.insert(inp.clone()) {
            if fuzz {
                fruitless += 1;
            }
            continue;
        }
        runs += 1;
        let t_loop = Instant::now();
        let t_run = Instant::now();
        // runs in endless loops stop at ten times the instructions of the
        // longest run that ended (at least LIMIT_FLOOR)
        let limited = Options {
            limit: opts.limit.min(LIMIT_FLOOR.max(longest.saturating_mul(10))),
            ..opts.clone()
        };
        let out = run(module, program, &fields, &inp, &limited, true);
        if out.instructions < limited.limit {
            longest = longest.max(out.instructions);
        }
        if std::env::var_os("ZTESTGEN_DEBUG").is_some() {
            eprintln!("run took {:?}, {} steps", t_run.elapsed(), out.path.len());
        }
        stubbed.extend(out.stubbed.iter().cloned());
        if std::env::var_os("ZTESTGEN_DEBUG").is_some() {
            eprintln!(
                "run {runs}: {:?} {:?} {:?}",
                inp.accept,
                inp.params
                    .iter()
                    .map(|p| ebcdic::decode(p))
                    .collect::<Vec<_>>(),
                out.error
            );
            for v in out.vars.iter().filter(|v| v.name.starts_with('X')) {
                eprintln!("  var {} = {}", v.name, v.value);
            }
            for s in &out.path {
                eprintln!("  {s:?}");
            }
        }
        let new: Vec<Dir> = out
            .branches
            .iter()
            .filter(|d| !covered.contains(*d) && kept_dir(d))
            .cloned()
            .collect();
        let mut new_dedup = Vec::new();
        for d in new {
            if !new_dedup.contains(&d) {
                new_dedup.push(d);
            }
        }
        // new directions of the branches or of decisions other than the
        // delimiters of text statements (their lengths, NUMVAL)
        let fresh = cases.is_empty()
            || out
                .branches
                .iter()
                .any(|d| !covered.contains(d) && !delimiter_decision(d));
        covered.extend(out.branches.iter().cloned());
        for (p, pc, _) in &out.branches {
            reached.insert((p.clone(), *pc));
        }
        let t_flip = Instant::now();
        // the other way of each branch, where not covered yet
        let mut before: BTreeSet<Dir> = BTreeSet::new();
        let mut slicer = Slicer::new(&out.path, out.vars.len());
        // the conditions to solve for this run (solved together, in
        // parallel, after the path is gone through)
        let mut jobs: Vec<Job> = Vec::new();
        // each branch's executions in the run (loops), and whether some
        // direction that runs depends on the inputs only implicitly
        let mut total: HashMap<(&str, u32), u32> = HashMap::new();
        for st in &out.path {
            if let Step::Branch(x) = st {
                *total.entry((x.program.as_str(), x.pc)).or_default() += 1;
            }
        }
        let mut so_far: HashMap<(&str, u32), u32> = HashMap::new();
        let mut last_at: HashMap<(&str, u32), usize> = HashMap::new();
        for (k, st) in out.path.iter().enumerate() {
            if let Step::Branch(x) = st {
                last_at.insert((x.program.as_str(), x.pc), k);
            }
        }
        let implicit = all.iter().any(|d| {
            !covered.contains(d)
                && !impossible.contains(d)
                && reached.contains(&(d.0.clone(), d.1))
                && !symbolic.contains(&(d.0.clone(), d.1))
        });
        for i in 0..out.path.len() {
            if start.elapsed() > opts.time {
                break;
            }
            let Step::Branch(b) = &out.path[i] else {
                continue;
            };
            let first = before.insert((b.program.clone(), b.pc, b.jumped));
            let depth = {
                let n = so_far.entry((b.program.as_str(), b.pc)).or_default();
                *n += 1;
                *n
            };
            if b.pred.is_none() {
                continue;
            }
            symbolic.insert((b.program.clone(), b.pc));
            let target = (b.program.clone(), b.pc, !b.jumped);
            // a text statement's decision (is a delimiter here?): taken the
            // other way after other data (the bytes before it), as long as
            // some direction is open: it moves or adds delimiters
            if b.program.ends_with("#TEXT") {
                // from the runs that cover something new only
                let open = all.iter().any(|d| !covered.contains(d));
                if !open || !fresh || text_tried.len() >= TEXT_FLIPS {
                    continue;
                }
                // the data before it: the variables of the statement's
                // decisions before it in this run
                let key = {
                    use std::hash::{Hash, Hasher};
                    let mut vs = Vec::new();
                    for st in &out.path[..i] {
                        if let Step::Branch(x) = st
                            && x.program == b.program
                            && x.pc / 4096 == b.pc / 4096
                            && let Some(p) = &x.pred
                        {
                            p.vars(&mut vs);
                        }
                    }
                    vs.sort();
                    vs.dedup();
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    target.hash(&mut h);
                    for v in vs {
                        (v, out.vars[v as usize].value).hash(&mut h);
                    }
                    h.finish()
                };
                if !text_tried.insert(key) {
                    continue;
                }
                let Some(cons) = slicer.slice(i) else {
                    continue;
                };
                jobs.push(Job {
                    cons,
                    time: SOLVE_TIME,
                    text: true,
                    target: None,
                });
                continue;
            }
            // the other way of this branch leads to the MOVE of a flag value
            // that a direction reached later in this run needs
            let wanted: Vec<Dir> = enables
                .get(&target)
                .into_iter()
                .flatten()
                .filter(|t| {
                    !covered.contains(*t)
                        && !impossible.contains(*t)
                        && last_at.get(&(t.0.as_str(), t.1)).is_some_and(|&k| k > i)
                })
                .cloned()
                .collect();
            let mut enabling = false;
            for t in wanted {
                let n = enabled.entry((target.clone(), t)).or_insert(0);
                if *n < ENABLE_TRIES {
                    *n += 1;
                    enabling = true;
                }
            }
            if enabling && let Some(cons) = slicer.slice(i) {
                jobs.push(Job {
                    cons,
                    time: RETRY_TIME,
                    text: false,
                    target: None,
                });
            }
            if covered.contains(&target) {
                // a loop's last decision taken the other way gives a run
                // with one more iteration: tried while some direction
                // depends on the inputs only through the values computed
                // in loops (not followed)
                let last = depth == total[&(b.program.as_str(), b.pc)];
                let looped = depth > 1;
                // from the deepest run of the loop so far only
                let n = deepened.entry((b.program.clone(), b.pc)).or_insert(0);
                let deepen = last && looped && implicit && depth > *n && depth <= opts.loop_deepen;
                // the other way after other decisions (a new context): the
                // directions not followed may depend on the inputs there
                // (input that a screen sends, where the first way came from
                // a path with constants)
                let context = !deepen && implicit && first && {
                    use std::hash::{Hash, Hasher};
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    before.hash(&mut h);
                    let tried = contexts.entry(target.clone()).or_default();
                    tried.len() < 12 && tried.insert(h.finish())
                };
                if !deepen && !context {
                    continue;
                }
                if deepen {
                    *n = depth;
                }
                if let Some(cons) = slicer.slice(i) {
                    jobs.push(Job {
                        cons,
                        time: RETRY_TIME,
                        text: false,
                        target: None,
                    });
                }
                continue;
            }
            if impossible.contains(&target) {
                continue;
            }
            let a = attempts.entry(target.clone()).or_default();
            if a.0 >= opts.attempts {
                continue;
            }
            a.0 += 1;
            let Some(cons) = slicer.slice(i) else {
                continue;
            };
            // the first attempt at a direction gets the full time, the
            // later ones (after other paths) less
            jobs.push(Job {
                cons,
                time: if a.0 > 1 { RETRY_TIME } else { SOLVE_TIME },
                text: false,
                target: Some(target),
            });
        }
        prof[1] += t_flip.elapsed();
        // text statements with fewer delimiters than they may take: one
        // more delimiter in the last field (a few runs per number of
        // delimiters), so that later receivers get the data
        if all.iter().any(|d| !covered.contains(d)) && text_tried.len() < TEXT_FLIPS {
            let mut groups: BTreeMap<(String, u32), Vec<usize>> = BTreeMap::new();
            for (i, st) in out.path.iter().enumerate() {
                if let Step::Branch(x) = st
                    && x.program.ends_with("#TEXT")
                    && x.pc % 4096 < 4094
                {
                    groups
                        .entry((x.program.clone(), x.pc / 4096))
                        .or_default()
                        .push(i);
                }
            }
            let jumped = |i: usize| matches!(&out.path[i], Step::Branch(x) if x.jumped);
            for ((prog, ins), idx) in groups {
                let found = idx.iter().filter(|&&i| jumped(i)).count();
                let after = idx.iter().rposition(|&i| jumped(i)).map_or(0, |k| k + 1);
                let last = &idx[after..];
                if found >= 24 || last.is_empty() {
                    continue;
                }
                let n = extended.entry((prog, ins, found)).or_insert(0);
                if *n >= 2 {
                    continue;
                }
                *n += 1;
                let pick = last[7.min(last.len() - 1)];
                let Some(cons) = conditions(&out.path, pick) else {
                    continue;
                };
                text_tried.insert(pick as u64 ^ (u64::from(ins) << 32) ^ (found as u64) << 48);
                jobs.push(Job {
                    cons,
                    time: SOLVE_TIME,
                    text: true,
                    target: None,
                });
            }
        }
        // solve, in parallel, and queue the inputs found
        let startv: Vec<i128> = out.vars.iter().map(|v| v.value).collect();
        let outcomes = solve_all(&mut solvers, &jobs, &out.vars, &startv);
        for (job, r) in jobs.into_iter().zip(outcomes) {
            if std::env::var_os("ZTESTGEN_DEBUG").is_some()
                && let Some(t) = &job.target
            {
                eprintln!("target {t:?}: {r:?}");
                for c in &job.cons {
                    eprintln!("    {c:?}");
                }
            }
            match r {
                Outcome::Sat(x) => {
                    let next = apply(&inp, &fields, &specs, &out.vars, &x);
                    if !seen.contains(&next) {
                        if job.text {
                            text_queue.push_back(next);
                        } else {
                            queue.push_back(next);
                        }
                    }
                }
                Outcome::Unsat => {
                    if let Some(t) = job.target {
                        attempts.entry(t).or_default().1 += 1;
                    }
                }
                Outcome::Unknown => {}
            }
        }
        prof[0] += t_loop.elapsed();
        if fuzz {
            fruitless = if new_dedup.is_empty() {
                fruitless + 1
            } else {
                0
            };
        }
        // a run stopped before the instruction limit of the options would
        // not replay the same
        let cut = out.instructions >= limited.limit && limited.limit < opts.limit;
        if (!new_dedup.is_empty() || cases.is_empty()) && !cut {
            cases.push(Case {
                inputs: inp,
                out,
                covers: new_dedup,
            });
        }
    }
    if std::env::var_os("ZTESTGEN_PROFILE").is_some() {
        eprintln!(
            "profile: loop {:?}, flips {:?}, total {:?}; solve/slice/apply {:?}",
            prof[0],
            prof[1],
            start.elapsed(),
            PROF.with(|p| p.get())
        );
    }
    // the fewest cases covering the same directions (greedy set cover)
    let mut left: HashSet<Dir> = covered.iter().filter(|d| kept_dir(d)).cloned().collect();
    let mut kept: Vec<Case> = Vec::new();
    let mut pool: Vec<(Case, HashSet<Dir>)> = cases
        .into_iter()
        .map(|c| {
            let s: HashSet<Dir> = c.out.branches.iter().cloned().collect();
            (c, s)
        })
        .collect();
    while !left.is_empty() {
        let Some((k, n)) = pool
            .iter()
            .enumerate()
            .map(|(k, (_, s))| (k, s.intersection(&left).count()))
            .max_by_key(|&(k, n)| (n, std::cmp::Reverse(k)))
        else {
            break;
        };
        if n == 0 {
            break;
        }
        let (mut c, s) = pool.remove(k);
        let mut cov: Vec<Dir> = s.intersection(&left).cloned().collect();
        cov.sort();
        for d in &cov {
            left.remove(d);
        }
        c.covers = cov;
        kept.push(c);
    }
    if kept.is_empty()
        && let Some((c, _)) = pool.into_iter().next()
    {
        kept.push(c);
    }
    // a direction covered cannot be impossible: the analysis was wrong
    // there (reported, not used)
    let wrong: Vec<&Dir> = impossible.iter().filter(|d| covered.contains(*d)).collect();
    if !wrong.is_empty() {
        eprintln!("ztestgen: static analysis wrong for covered directions {wrong:?}");
        let wrong: Vec<Dir> = wrong.into_iter().cloned().collect();
        for d in wrong {
            impossible.remove(&d);
        }
    }
    let gaps = all
        .iter()
        .filter(|d| !covered.contains(*d))
        .map(|d| {
            let at = (d.0.clone(), d.1);
            let gap = if impossible.contains(d) {
                Gap::Impossible
            } else if !reached.contains(&at) {
                Gap::NotReached
            } else if !symbolic.contains(&at) {
                Gap::NotSymbolic
            } else {
                match attempts.get(d) {
                    Some(&(t, u)) if t > 0 && u == t => Gap::Infeasible,
                    _ => Gap::NotFound,
                }
            };
            (d.clone(), gap)
        })
        .collect();
    Ok(Report {
        cases: kept,
        all,
        gaps,
        runs,
        fields,
        files: specs,
        stubbed,
        smt,
    })
}

/// The value of a field in a parameter's bytes, as text.
pub fn field_text(f: &Field, bytes: &[u8]) -> String {
    let b = &bytes[f.offset..f.offset + f.len];
    match &f.num {
        Some(t) => {
            let r = raw(t, b);
            if t.scale > 0 {
                let s = t.scale as u32;
                let k = 10i128.pow(s);
                let sign = if r < 0 { "-" } else { "" };
                format!("{sign}{}.{:0w$}", r.abs() / k, r.abs() % k, w = s as usize)
            } else {
                r.to_string()
            }
        }
        None => ebcdic::decode(b),
    }
}

/// The term of a numeric usage, for reports.
pub fn usage_name(t: &NumType) -> &'static str {
    match t.usage {
        NumUsage::Display => "DISPLAY",
        NumUsage::Packed => "COMP-3",
        NumUsage::Binary => "COMP",
        NumUsage::Native => "COMP-5",
        NumUsage::National => "NATIONAL",
    }
}

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

use zeus_cobol::concolic::{FileInput, Pred, RecordField, Step, SymState, Term, Var, VarKind};
use zeus_cobol::ebcdic;
use zeus_cobol::vm::Machine;
use zeus_cobol::vm::data as vdata;
use zeus_cobol::zir::{
    AcceptFrom, Base, DataType, Desc, Instr, Module, NumType, NumUsage, OpenMode, Org, ProgramCode,
    Src,
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

/// An elementary item of an area: name, offset, length, numeric type.
pub(crate) type AreaItem = (String, usize, usize, Option<NumType>);

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

/// An input area of a program: a USING parameter, or the storage a
/// POINTER item of another area points to.
#[derive(Debug, Clone)]
pub struct Area {
    /// The LINKAGE records on the area (a parameter's record, or the
    /// records SET ADDRESS OF ... TO the pointer).
    pub slots: Vec<u32>,
    /// For a pointed-to area: the area (parameter index), offset and
    /// length of the POINTER item that holds its address, and its name.
    pub pointer: Option<(usize, usize, usize, String)>,
}

/// Areas pointed to, at most (pointers in pointed-to areas included).
const MAX_POINTED: usize = 16;

/// The input areas of a program: its USING parameters, then the areas
/// that POINTER items in them address, when the program bases LINKAGE
/// records on those items (`SET ADDRESS OF R TO P`, as exits that get a
/// list of pointers do). Each pointed-to area is an extra parameter.
pub fn areas(code: &ProgramCode) -> Vec<Area> {
    let mut out: Vec<Area> = code
        .using
        .iter()
        .map(|&slot| Area {
            slots: vec![slot],
            pointer: None,
        })
        .collect();
    let mut k = 0;
    while k < out.len() && out.len() < code.using.len() + MAX_POINTED {
        let slots = out[k].slots.clone();
        for (i, d) in code.descs.iter().enumerate() {
            let Base::Linkage(s) = d.base else { continue };
            if d.ty != DataType::Pointer || !d.dims.is_empty() || !slots.contains(&s) {
                continue;
            }
            let mut targets: Vec<u32> = code
                .code
                .iter()
                .filter_map(|ins| match ins {
                    Instr::SetAddress(n, zeus_cobol::zir::Ex::Load(l))
                        if l.desc as usize == i && l.subs.is_empty() && l.refmod.is_none() =>
                    {
                        Some(*n)
                    }
                    _ => None,
                })
                .collect();
            targets.sort_unstable();
            targets.dedup();
            // a record already on an input area keeps it
            targets.retain(|n| !out.iter().any(|a| a.slots.contains(n)));
            if targets.is_empty() || out.len() >= code.using.len() + MAX_POINTED {
                continue;
            }
            out.push(Area {
                slots: targets,
                pointer: Some((k, d.offset as usize, d.len as usize, d.name.clone())),
            });
        }
        k += 1;
    }
    out
}

/// The items of the group a parameter with no items of its own (one
/// `PIC X(n)` record) is moved to as a whole (`MOVE PARM TO RECORD`): the
/// parameter's fields are the record's (names and numeric items), so that
/// inputs start with valid numbers where the program expects them.
fn moved_to(code: &ProgramCode, slots: &[u32], len: usize) -> Option<Vec<AreaItem>> {
    let mut own = code
        .descs
        .iter()
        .enumerate()
        .filter(|(_, d)| matches!(d.base, Base::Linkage(s) if slots.contains(&s)));
    let (k, d) = own.next()?;
    if own.next().is_some()
        || !matches!(d.ty, DataType::Alphanumeric { .. })
        || d.offset != 0
        || d.len as usize != len
        || !d.dims.is_empty()
    {
        return None;
    }
    code.code.iter().find_map(|ins| {
        let Instr::Move(Src::Loc(from), to) = ins else {
            return None;
        };
        if from.desc as usize != k
            || from.refmod.is_some()
            || !to.subs.is_empty()
            || to.refmod.is_some()
        {
            return None;
        }
        let g = code.descs.get(to.desc as usize)?;
        if g.ty != DataType::Group || !g.dims.is_empty() {
            return None;
        }
        let start = g.offset as usize;
        let n = len.min(g.len as usize);
        let record: Vec<&Desc> = code
            .descs
            .iter()
            .filter(|x| {
                x.base == g.base && x.offset as usize >= start && (x.offset as usize) < start + n
            })
            .collect();
        let found = items(&record, start, n);
        (!found.is_empty()).then_some(found)
    })
}

/// The input fields of the input areas of a program ([`areas`]) and the
/// length of each area.
pub fn fields(code: &ProgramCode) -> (Vec<Field>, Vec<usize>) {
    let mut fields = Vec::new();
    let mut lens = Vec::new();
    let extent = |d: &Desc| -> usize {
        let mut e = d.offset as usize + d.len as usize;
        for dim in &d.dims {
            e += (dim.max.saturating_sub(1) as usize) * dim.stride as usize;
        }
        e
    };
    for (k, area) in areas(code).iter().enumerate() {
        let descs: Vec<_> = code
            .descs
            .iter()
            .filter(|d| matches!(d.base, Base::Linkage(s) if area.slots.contains(&s)))
            .collect();
        let mut len = descs.iter().map(|d| extent(d)).max().unwrap_or(0);
        // a record put on the parameter (SET ADDRESS OF R TO ADDRESS OF
        // P: an IMS PCB passed as a POINTER item) fits in its area
        for i in &code.code {
            if let Instr::SetAddress(n, zeus_cobol::zir::Ex::AddressOf(l)) = i
                && let Some(d) = code.descs.get(l.desc as usize)
                && matches!(d.base, Base::Linkage(s) if area.slots.contains(&s))
            {
                let r = code
                    .descs
                    .iter()
                    .filter(|x| x.base == Base::Linkage(*n))
                    .map(extent)
                    .max()
                    .unwrap_or(0);
                len = len.max(d.offset as usize + r);
            }
        }
        lens.push(len);
        let mut area_items = items(&descs, 0, len);
        if let Some(record) = moved_to(code, &area.slots, len) {
            area_items = record;
        }
        for (name, offset, len, num) in area_items {
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
    /// The record's items of a file the program writes (to show the
    /// records written).
    pub out_fields: Vec<RecordField>,
    /// The blocks of the data set (BLOCK CONTAINS, RECORDING MODE U/S), as
    /// the program reads and writes it; None: records as they are.
    pub layout: Option<zeus_ds::physical::Layout>,
    /// An EXTERNAL file the program reads without opening it (its caller
    /// opens it INPUT).
    pub preopened: bool,
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
    d.0.ends_with("#TEXT") && d.1 % 4096 < 4093
}

/// The solver's time for the first attempt at a direction, and for the
/// later ones and other contexts.
const SOLVE_TIME: Duration = Duration::from_millis(150);
const RETRY_TIME: Duration = Duration::from_millis(50);

/// Inputs waiting to run: in turn the nearest to the directions not
/// covered and the oldest (the search goes deep without leaving the
/// rest behind).
#[derive(Default)]
struct Queue {
    items: Vec<(u32, u64, Inputs)>,
    seq: u64,
    pops: u64,
}

impl Queue {
    fn push(&mut self, i: Inputs, prio: u32) {
        self.seq += 1;
        self.items.push((prio, self.seq, i));
    }

    fn pop(&mut self) -> Option<Inputs> {
        self.pops += 1;
        let k = if self.pops.is_multiple_of(2) {
            (0..self.items.len()).min_by_key(|&k| (self.items[k].0, self.items[k].1))?
        } else {
            (0..self.items.len()).min_by_key(|&k| self.items[k].1)?
        };
        Some(self.items.swap_remove(k).2)
    }
}

/// A distance for what does not lead to an open direction.
const FAR: u32 = u32::MAX / 2;

/// For each instruction of the program, the instructions to go to reach
/// a conditional jump with a direction not covered (nor impossible).
fn distances(
    preds: &[Vec<u32>],
    code: &ProgramCode,
    all: &[Dir],
    covered: &HashSet<Dir>,
    impossible: &HashSet<Dir>,
) -> Vec<u32> {
    let mut dist = vec![FAR; preds.len()];
    let mut todo = VecDeque::new();
    for d in all {
        if d.0 == code.name
            && !covered.contains(d)
            && !impossible.contains(d)
            && let Some(x) = dist.get_mut(d.1 as usize)
            && *x != 0
        {
            *x = 0;
            todo.push_back(d.1);
        }
    }
    while let Some(pc) = todo.pop_front() {
        let next = dist[pc as usize] + 1;
        for &q in &preds[pc as usize] {
            if next < dist[q as usize] {
                dist[q as usize] = next;
                todo.push_back(q);
            }
        }
    }
    dist
}

/// The UNSTRING statements of `program` that ran on followed bytes: for
/// each, the positions decided ("is a delimiter here") with the variable
/// of the byte there and the one-byte delimiters.
fn unstring_bytes(path: &[Step], program: &str) -> BTreeMap<u32, BTreeMap<u32, (u32, Vec<u8>)>> {
    let text = format!("{program}#TEXT");
    let mut out: BTreeMap<u32, BTreeMap<u32, (u32, Vec<u8>)>> = BTreeMap::new();
    for st in path {
        let Step::Branch(x) = st else { continue };
        if x.program != text || x.pc % 4096 >= 4093 {
            continue;
        }
        let Some(p) = &x.pred else { continue };
        let single = |p: &Pred| match p {
            Pred::Cmp(
                zeus_cobol::concolic::Term::Var(v),
                zeus_cobol::zir::Rel::Eq,
                zeus_cobol::concolic::Term::Const(c),
            ) => u8::try_from(*c).ok().map(|c| (*v, c)),
            _ => None,
        };
        let found: Vec<(u32, u8)> = match p {
            Pred::Or(ps) => ps.iter().filter_map(single).collect(),
            p => single(p).into_iter().collect(),
        };
        let Some(&(v, _)) = found.first() else {
            continue;
        };
        out.entry(x.pc / 4096)
            .or_default()
            .entry(x.pc % 4096)
            .or_insert_with(|| (v, found.iter().map(|f| f.1).collect()));
    }
    out
}

/// Conditions to solve for a run's inputs.
struct Job {
    cons: Vec<Pred>,
    /// The solver's time.
    time: Duration,
    /// The inputs found go to the queue of the text statements.
    text: bool,
    /// The direction aimed at (an attempt counted for it).
    target: Option<Dir>,
    /// How far the inputs are expected to get from directions not covered
    /// (instructions; smaller runs first).
    prio: u32,
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
            // an EXTERNAL file the program never opens: its caller has it
            // open (INPUT) when the program runs
            let preopened = def.external
                && !opens(&[
                    OpenMode::Input,
                    OpenMode::Output,
                    OpenMode::InputOutput,
                    OpenMode::Extend,
                ])
                && (indexed || def.org == Org::Sequential && !text);
            let input = (def.org == Org::Sequential && !text && opens(&[OpenMode::Input]))
                || (indexed && opens(&[OpenMode::Input, OpenMode::InputOutput]))
                || preopened;
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
            let mut record_items = Vec::new();
            if (input || output)
                && let Some(rec) = p.descs.get(def.record as usize)
            {
                let descs: Vec<_> = p.descs.iter().filter(|d| d.base == rec.base).collect();
                for (name, offset, len, num) in
                    items(&descs, rec.offset as usize, def.max_len as usize)
                {
                    record_items.push(RecordField {
                        name,
                        offset,
                        len,
                        num,
                    });
                }
            }
            if input {
                fields = record_items.clone();
            }
            let out_fields = if output { record_items } else { Vec::new() };
            out.push(FileSpec {
                name: def.name.clone(),
                assign: def.assign.clone(),
                format,
                layout: zeus_cobol::vm::dataset_layout(def, &Default::default(), false),
                preopened,
                len: def.max_len as usize,
                input,
                output,
                fields,
                out_fields,
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
            let records = inputs.files.get(k).map(|r| &r[..]).unwrap_or(&[]);
            match f.layout.map(|l| l.encode(records, 0x40)) {
                Some(Ok(blocks)) => data = blocks.concat(),
                _ => {
                    for r in records {
                        zeus_ds::format::encode_into(format, r, 0x40, &mut data);
                    }
                }
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
    m.set_preopened(
        specs
            .iter()
            .filter(|f| f.preopened)
            .map(|f| f.name.clone())
            .collect(),
    );
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
        st.inputs_bound();
    }
    // the pointers to the pointed-to areas; the USING parameters are the
    // arguments
    let mut using = addrs.len();
    if let Some(code) = module.programs.iter().find(|p| p.name == program) {
        using = code.using.len().min(addrs.len());
        for (k, a) in areas(code).iter().enumerate() {
            if let (Some((p, offset, len)), Some(&target)) = (
                a.pointer.as_ref().map(|(p, o, l, _)| (*p, *o, *l)),
                addrs.get(k),
            ) && let Some(&base) = addrs.get(p)
                && offset + len <= inputs.params[p].len()
            {
                zeus_decimal::binary::write(
                    &mut m.storage_mut()[base + offset..base + offset + len],
                    target as i128,
                );
            }
        }
    }
    let args: Vec<Option<usize>> = addrs[..using].iter().map(|a| Some(*a)).collect();
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
            let recs = match f.layout {
                Some(l) => l
                    .split(&data)
                    .and_then(|b| l.decode(&b))
                    .unwrap_or_default(),
                None => zeus_ds::format::decode(format, &data).unwrap_or_default(),
            };
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
    /// Taken only by runs stopped at the instruction limit (no case: they
    /// would not replay the same).
    LimitOnly,
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

/// Whether a condition is on a counter's term (a sum of conditions, see
/// `Branch::counted`).
fn on_counter(p: &Pred) -> bool {
    fn term(t: &Term) -> bool {
        match t {
            Term::If(..) => true,
            Term::Const(_) | Term::Var(_) => false,
            Term::Add(a, b) | Term::Sub(a, b) | Term::Mul(a, b) => term(a) || term(b),
            Term::Neg(a) | Term::Abs(a) | Term::Div(a, _) | Term::Rem(a, _) => term(a),
        }
    }
    match p {
        Pred::Bool(_) => false,
        Pred::Cmp(a, _, b) => term(a) || term(b),
        Pred::Bytes(a, _, b) => a.iter().chain(b).any(term),
        Pred::In(t, _) => term(t),
        Pred::Not(p) => on_counter(p),
        Pred::And(v) | Pred::Or(v) => v.iter().any(on_counter),
    }
}

/// The conditions a path keeps up to step `i`, and the other way of the
/// branch at `i`; only those sharing variables with that branch. For a
/// condition on a counter, the earlier conditions on counters (the counts
/// along a loop, the loop's end) are left free: the path may change.
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
    let counter = on_counter(&target);
    for s in &path[..i] {
        match s {
            Step::Assume(p) => all.push(p.clone()),
            Step::Branch(x) if counter && x.pred.as_ref().is_some_and(on_counter) => {}
            Step::Branch(x) if !x.counted => {
                if let Some(p) = &x.pred {
                    all.push(if x.jumped {
                        p.clone()
                    } else {
                        Pred::not(p.clone())
                    });
                }
            }
            // free: the counter's term holds it
            Step::Branch(_) => {}
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
            Step::Branch(x) if x.counted => None,
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
        let counter = on_counter(&target);
        for k in idx {
            if let Some(c) = Self::pred(&self.path[k as usize])
                && !(counter && matches!(self.path[k as usize], Step::Branch(_)) && on_counter(&c))
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
            .chain(crate::ranges::impossible(code))
            .map(|(pc, j)| (code.name.clone(), pc, j))
            .collect()
    };
    let kept_dir = |d: &Dir| counted.contains(d) || d.0.ends_with("#EXEC");
    // the program's control flow, backwards, for the distances to the
    // directions not covered
    let preds: Vec<Vec<u32>> = crate::infeasible::successors(code).map_or_else(Vec::new, |succ| {
        let mut p = vec![Vec::new(); succ.len()];
        for (pc, ss) in succ.iter().enumerate() {
            for &t in ss {
                p[t as usize].push(pc as u32);
            }
        }
        p
    });
    let mut dist: Vec<u32> = Vec::new();
    let mut dist_of = 0usize; // covered.len() when computed
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
    let mut queue = Queue::default();
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
    // the limit of runs while none has ended (raised when one is stopped)
    let mut floor = LIMIT_FLOOR;
    // runs extended with one more delimiter: (program, statement,
    // delimiters found) -> how many
    let mut extended: HashMap<(String, u32, usize), u32> = HashMap::new();
    // the UNSTRING statements given data shaped like their fields
    let mut shaped: HashSet<u32> = HashSet::new();
    let mut cases: Vec<Case> = Vec::new();
    let mut runs = 0;
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let dict = Dictionary::of(module);
    // mutated inputs tried in a row without new coverage
    let mut fruitless = 0;
    // rounds of the search (each from the cases of the ones before), and
    // the directions covered when the last one started
    let mut rounds = 0u32;
    let mut round_start = 0usize;
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
        queue.push(inp.clone(), 0);
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
        queue.push(first, 0);
        queue.push(late, 0);
    } else {
        queue.push(first, 0);
    }
    loop {
        if runs >= opts.max_runs || start.elapsed() > opts.time {
            break;
        }
        let (inp, fuzz) = match queue.pop().or_else(|| text_queue.pop_front()) {
            Some(i) => (i, false),
            None => {
                // the solver has nothing more: mutate the inputs of a case
                // while some branch direction that runs is not covered
                let open = all
                    .iter()
                    .any(|d| !covered.contains(d) && reached.contains(&(d.0.clone(), d.1)));
                let branches = covered.iter().filter(|d| counted.contains(*d)).count();
                if open && fruitless >= opts.fuzz && branches > round_start {
                    // a new round: the attempts and contexts start again,
                    // from the cases so far (they run again)
                    rounds += 1;
                    if std::env::var_os("ZTESTGEN_PROFILE").is_some() {
                        eprintln!("round {rounds}: {runs} runs, {} covered", covered.len());
                    }
                    attempts.clear();
                    contexts.clear();
                    deepened.clear();
                    enabled.clear();
                    // the decisions outside the branches (EXEC responses,
                    // delimiters) are taken again
                    covered.retain(|d| !d.0.contains('#'));
                    text_tried.clear();
                    extended.clear();
                    // the inputs run before may run again (with other
                    // decisions to take from them now)
                    seen.clear();
                    for c in &cases {
                        queue.push(c.inputs.clone(), 0);
                    }
                    fruitless = 0;
                    round_start = branches;
                    continue;
                }
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
        // longest run that ended (at least `floor`)
        let limited = Options {
            limit: opts.limit.min(floor.max(longest.saturating_mul(10))),
            ..opts.clone()
        };
        let out = run(module, program, &fields, &inp, &limited, true);
        if out.instructions < limited.limit {
            longest = longest.max(out.instructions);
        } else if longest == 0 {
            // no run has ended: a long run, not necessarily an endless one
            floor = floor.saturating_mul(4).min(opts.limit);
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
        // the distances to the directions still open (when coverage grew)
        if dist.is_empty() || covered.len() != dist_of {
            dist = distances(&preds, code, &all, &covered, &impossible);
            dist_of = covered.len();
        }
        // how far taking a direction gets from the open ones
        let away = |d: &Dir| -> u32 {
            if d.0 != code.name {
                return FAR;
            }
            let next = match code.code.get(d.1 as usize) {
                Some(Instr::JumpIf(_, t) | Instr::JumpUnless(_, t)) if d.2 => *t,
                _ => d.1 + 1,
            };
            dist.get(next as usize).copied().unwrap_or(FAR)
        };
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
        // some direction this run reaches was solved for and not found:
        // the decisions before it may have to change too
        let stuck = all.iter().any(|d| {
            !covered.contains(d)
                && !impossible.contains(d)
                && reached.contains(&(d.0.clone(), d.1))
                && attempts.get(d).is_some_and(|a| a.0 > 0)
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
                    prio: 0,
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
                    prio: 1,
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
                // a path with constants), or a direction not found may be
                // found after it (when the other way gets nearer to one)
                let nearer = || stuck && away(&target) < away(&(b.program.clone(), b.pc, b.jumped));
                let context = !deepen && first && (implicit || nearer()) && {
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
                        prio: away(&target),
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
                prio: 0,
            });
        }
        prof[1] += t_flip.elapsed();
        // an UNSTRING of the program on bytes that follow the inputs: once,
        // data with each field digits '1' (as many as its receiver takes,
        // at most 9, else one), a one-byte delimiter between them (records
        // of comma separated numbers)
        let mut shapes: Vec<Vec<i128>> = Vec::new();
        for (ins, pos) in unstring_bytes(&out.path, &code.name) {
            let Some(Instr::Unstring { into, .. }) = code.code.get(ins as usize) else {
                continue;
            };
            if !shaped.insert(ins) || into.len() < 2 {
                continue;
            }
            let Some(&d) = pos
                .values()
                .flat_map(|(_, ds)| ds.iter())
                .min_by_key(|&&c| (c != 0x6B, c))
            else {
                continue;
            };
            let wide: Vec<usize> = into
                .iter()
                .map(|(l, ..)| {
                    code.descs
                        .get(l.desc as usize)
                        .map_or(1, |x| (x.len as usize).clamp(1, 9))
                })
                .collect();
            for widths in [wide, vec![1; into.len()]] {
                let mut values: Vec<i128> = out.vars.iter().map(|v| v.value).collect();
                let mut k = 0u32;
                let mut ok = true;
                for (f, w) in widths.iter().enumerate() {
                    let delim = f + 1 < widths.len();
                    for j in 0..w + usize::from(delim) {
                        match pos.get(&k) {
                            Some((v, _)) => {
                                values[*v as usize] = if j < *w { 0xF1 } else { i128::from(d) }
                            }
                            None => ok = false,
                        }
                        k += 1;
                    }
                }
                if ok {
                    shapes.push(values);
                    break;
                }
            }
        }
        for values in shapes {
            let next = apply(&inp, &fields, &specs, &out.vars, &values);
            if !seen.contains(&next) {
                queue.push(next, 0);
            }
        }
        // text statements with fewer delimiters than they may take: one
        // more delimiter in the last field (a few runs per number of
        // delimiters), so that later receivers get the data
        if all.iter().any(|d| !covered.contains(d)) && text_tried.len() < TEXT_FLIPS {
            let mut groups: BTreeMap<(String, u32), Vec<usize>> = BTreeMap::new();
            for (i, st) in out.path.iter().enumerate() {
                if let Step::Branch(x) = st
                    && x.program.ends_with("#TEXT")
                    && x.pc % 4096 < 4093
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
                    prio: 0,
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
                            queue.push(next, job.prio);
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
    // covered: by the cases kept (runs stopped at the instruction limit
    // took the others)
    let in_cases: HashSet<Dir> = kept
        .iter()
        .flat_map(|c| c.out.branches.iter().cloned())
        .collect();
    let gaps = all
        .iter()
        .filter(|d| !in_cases.contains(*d))
        .map(|d| {
            let at = (d.0.clone(), d.1);
            let gap = if covered.contains(d) {
                Gap::LimitOnly
            } else if impossible.contains(d) {
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

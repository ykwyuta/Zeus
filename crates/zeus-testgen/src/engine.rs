//! Test generation: runs a program on inputs with concolic execution,
//! and for each branch that went one way only, asks the solver for
//! inputs that keep the run's earlier decisions and take the other way
//! (generational search). Inputs are the program's USING parameters and
//! the lines it ACCEPTs.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, VecDeque};
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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Inputs {
    /// The storage of each USING parameter (EBCDIC).
    pub params: Vec<Vec<u8>>,
    /// The lines ACCEPTed (SYSIN).
    pub accept: Vec<String>,
    /// The records of each input file (`FileSpec::input`, in order).
    pub files: Vec<Vec<Vec<u8>>>,
}

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
}

impl Default for Options {
    fn default() -> Self {
        Options {
            search: Vec::new(),
            max_runs: 500,
            time: Duration::from_secs(120),
            limit: 2_000_000,
            attempts: 4,
            accept_lines: 4,
            fuzz: 60,
            loop_deepen: 20,
        }
    }
}

/// The elementary items of `descs` in `[start, start+len)`, without
/// overlaps (the first one wins): name, offset from `start`, length and
/// numeric type. Tables give an item per occurrence (at most 100).
fn items(
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
                for i in 0..dim.max.min(100) as usize {
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

/// Records an input file has at most.
pub const MAX_RECORDS: usize = 99;

/// The files of the module's programs (by DD name, once).
pub fn file_specs(module: &Module, program: &str) -> Vec<FileSpec> {
    let mut out: Vec<FileSpec> = Vec::new();
    for p in &module.programs {
        let main = p.name.eq_ignore_ascii_case(program);
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
            let input = main
                && ((def.org == Org::Sequential && !text && opens(&[OpenMode::Input]))
                    || (indexed && opens(&[OpenMode::Input, OpenMode::InputOutput])));
            let output = main
                && (matches!(def.org, Org::Sequential | Org::LineSequential)
                    && opens(&[OpenMode::Output, OpenMode::Extend])
                    || indexed
                        && opens(&[OpenMode::Output, OpenMode::InputOutput, OpenMode::Extend]));
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
        accept: Vec::new(),
        files: Vec::new(),
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
    let mut m = Machine::new(opts.search.clone());
    m.add_module(module.clone());
    let specs = file_specs(module, program);
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
    let addrs: Vec<usize> = inputs
        .params
        .iter()
        .map(|b| {
            let a = m.getmain(b.len().max(1), 0);
            m.storage_mut()[a..a + b.len()].copy_from_slice(b);
            a
        })
        .collect();
    if symbolic {
        m.set_symbolic(SymState::new(opts.limit));
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
    let st = m.take_symbolic().unwrap_or_default();
    drop(m);
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
    }
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
}

pub struct Report {
    pub cases: Vec<Case>,
    /// Every branch direction of the programs, covered or not.
    pub all: Vec<Dir>,
    pub gaps: Vec<(Dir, Gap)>,
    pub runs: usize,
    pub fields: Vec<Field>,
    pub files: Vec<FileSpec>,
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

/// Inputs with the variables' values from a solution.
fn apply(
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
    normalize(&mut new, specs);
    new
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
        let mut d = Dictionary::default();
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
    let inputs: Vec<&FileSpec> = specs.iter().filter(|f| f.input).collect();
    let k = next(rng) as usize % (fields.len() + new.accept.len() + inputs.len()).max(1);
    if k >= fields.len() + new.accept.len() {
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
                            accept: Vec::new(),
                            files: Vec::new(),
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
            accept: Vec::new(),
            files: Vec::new(),
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
    let specs = file_specs(module, &code.name);
    let all = directions(module);
    let start = Instant::now();
    let mut solver = Builtin::default();
    let mut first = default_inputs(&fields, &lens);
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
    let mut queue: VecDeque<Inputs> = VecDeque::from([first]);
    let mut seen: HashSet<Inputs> = HashSet::new();
    let mut covered: HashSet<Dir> = HashSet::new();
    let mut reached: HashSet<(String, u32)> = HashSet::new();
    let mut symbolic: HashSet<(String, u32)> = HashSet::new();
    let mut attempts: HashMap<Dir, (u32, u32)> = HashMap::new(); // (tries, unsat)
    let mut deepened: HashMap<(String, u32), u32> = HashMap::new();
    let mut cases: Vec<Case> = Vec::new();
    let mut runs = 0;
    let mut rng = 0x2545_F491_4F6C_DD1Du64;
    let dict = Dictionary::of(module);
    // mutated inputs tried in a row without new coverage
    let mut fruitless = 0;
    loop {
        if runs >= opts.max_runs || start.elapsed() > opts.time {
            break;
        }
        let (inp, fuzz) = match queue.pop_front() {
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
        let out = run(module, program, &fields, &inp, opts, true);
        if std::env::var_os("ZTESTGEN_DEBUG").is_some() {
            eprintln!("run {runs}: {:?} {:?}", inp.accept, out.error);
            for s in &out.path {
                eprintln!("  {s:?}");
            }
        }
        let new: Vec<Dir> = out
            .branches
            .iter()
            .filter(|d| !covered.contains(*d))
            .cloned()
            .collect();
        let mut new_dedup = Vec::new();
        for d in new {
            if !new_dedup.contains(&d) {
                new_dedup.push(d);
            }
        }
        covered.extend(out.branches.iter().cloned());
        for (p, pc, _) in &out.branches {
            reached.insert((p.clone(), *pc));
        }
        // the other way of each branch, where not covered yet
        for i in 0..out.path.len() {
            let Step::Branch(b) = &out.path[i] else {
                continue;
            };
            if b.pred.is_none() {
                continue;
            }
            symbolic.insert((b.program.clone(), b.pc));
            let target = (b.program.clone(), b.pc, !b.jumped);
            if covered.contains(&target) {
                // a loop's last decision taken the other way gives a run
                // with one more iteration: tried while some direction
                // depends on the inputs only through the values computed
                // in loops (not followed)
                let last = !out.path[i + 1..].iter().any(
                    |s| matches!(s, Step::Branch(x) if x.program == b.program && x.pc == b.pc),
                );
                let looped = out.path[..i].iter().any(
                    |s| matches!(s, Step::Branch(x) if x.program == b.program && x.pc == b.pc),
                );
                let implicit = all.iter().any(|d| {
                    !covered.contains(d)
                        && reached.contains(&(d.0.clone(), d.1))
                        && !symbolic.contains(&(d.0.clone(), d.1))
                });
                let depth = out.path[..=i]
                    .iter()
                    .filter(
                        |s| matches!(s, Step::Branch(x) if x.program == b.program && x.pc == b.pc),
                    )
                    .count() as u32;
                // from the deepest run of the loop so far only
                let n = deepened.entry((b.program.clone(), b.pc)).or_insert(0);
                if !(last && looped && implicit && depth > *n && depth <= opts.loop_deepen) {
                    continue;
                }
                *n = depth;
                if let Some(cons) = conditions(&out.path, i)
                    && let Outcome::Sat(x) = solver.solve(
                        &cons,
                        &out.vars,
                        &out.vars.iter().map(|v| v.value).collect::<Vec<_>>(),
                    )
                {
                    let next = apply(&inp, &fields, &specs, &out.vars, &x);
                    if !seen.contains(&next) {
                        queue.push_back(next);
                    }
                }
                continue;
            }
            let a = attempts.entry(target.clone()).or_default();
            if a.0 >= opts.attempts {
                continue;
            }
            a.0 += 1;
            let Some(cons) = conditions(&out.path, i) else {
                continue;
            };
            let startv: Vec<i128> = out.vars.iter().map(|v| v.value).collect();
            let result = solver.solve(&cons, &out.vars, &startv);
            if std::env::var_os("ZTESTGEN_DEBUG").is_some() {
                eprintln!("target {target:?}: {result:?}");
                for c in &cons {
                    eprintln!("    {c:?}");
                }
            }
            match result {
                Outcome::Sat(x) => {
                    let next = apply(&inp, &fields, &specs, &out.vars, &x);
                    if !seen.contains(&next) {
                        queue.push_back(next);
                    }
                }
                Outcome::Unsat => a.1 += 1,
                Outcome::Unknown => {}
            }
        }
        if fuzz {
            fruitless = if new_dedup.is_empty() {
                fruitless + 1
            } else {
                0
            };
        }
        if !new_dedup.is_empty() || cases.is_empty() {
            cases.push(Case {
                inputs: inp,
                out,
                covers: new_dedup,
            });
        }
    }
    // the fewest cases covering the same directions (greedy set cover)
    let mut left: HashSet<Dir> = covered.clone();
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
    let gaps = all
        .iter()
        .filter(|d| !covered.contains(*d))
        .map(|d| {
            let at = (d.0.clone(), d.1);
            let gap = if !reached.contains(&at) {
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

//! The ZIR interpreter.

pub mod data;
mod eval;
mod files;
mod func;
mod text;

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;

use zeus_decimal::Dec;

use crate::ebcdic;
use crate::zir::{self, Instr, Module, ProgramCode};
use files::FileState;

/// A run-time error that ends the run unit.
#[derive(Debug)]
pub struct RunError(pub String);

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub type RResult<T> = Result<T, RunError>;

pub fn err<T>(msg: impl Into<String>) -> RResult<T> {
    Err(RunError(msg.into()))
}

/// An activated program.
struct Instance {
    code: Rc<ProgramCode>,
    /// Module the program came from (for nested program calls).
    module: Rc<Module>,
    module_index: u32,
    static_base: usize,
    linkage: Vec<Option<usize>>,
    perform: Vec<(u32, u32)>,
    temps: Vec<Dec>,
    alter: Vec<u32>,
    /// Slots in `Machine::file_states` of the program's files.
    files: Vec<usize>,
    /// Addresses of the EXTERNAL data areas the program uses.
    ext_bases: Vec<usize>,
    active: bool,
    /// The program has been entered before (state is kept between calls).
    initialized: bool,
}

struct Frame {
    inst: usize,
    pc: u32,
}

pub struct Machine {
    mem: Vec<u8>,
    instances: Vec<Instance>,
    file_states: Vec<FileState>,
    /// EXTERNAL data areas by name.
    externals: HashMap<String, usize>,
    /// EXTERNAL files by name (slot in `file_states`).
    ext_files: HashMap<String, usize>,
    by_name: HashMap<String, usize>,
    search_dirs: Vec<PathBuf>,
    switches: HashMap<String, bool>,
    size_error: bool,
    io_cond: bool,
    /// Status code of the last I/O statement (for USE procedures).
    last_status: [u8; 2],
    /// Mode of the last OPEN attempted.
    last_open_mode: Option<zir::OpenMode>,
    call_exception: bool,
    overflow: bool,
    stdout: Box<dyn Write>,
    stdin_lines: Option<std::io::Lines<std::io::StdinLock<'static>>>,
    /// Records released to the active SORT.
    sort_records: Vec<Vec<u8>>,
    sort_return: Option<(Vec<Vec<u8>>, usize)>,
    return_code: i32,
}

pub enum Exit {
    StopRun(i32),
}

impl Machine {
    pub fn new(search_dirs: Vec<PathBuf>) -> Machine {
        let mut switches = HashMap::new();
        for i in 0..8 {
            let on = std::env::var(format!("ZEUS_SWITCH_{i}"))
                .map(|v| v.eq_ignore_ascii_case("ON"))
                .unwrap_or(false);
            switches.insert(format!("SWITCH-{i}"), on);
            switches.insert(format!("UPSI-{i}"), on);
        }
        Machine {
            mem: Vec::new(),
            instances: Vec::new(),
            file_states: Vec::new(),
            externals: HashMap::new(),
            ext_files: HashMap::new(),
            by_name: HashMap::new(),
            search_dirs,
            switches,
            size_error: false,
            io_cond: false,
            last_status: *b"00",
            last_open_mode: None,
            call_exception: false,
            overflow: false,
            stdout: Box::new(std::io::BufWriter::new(std::io::stdout())),
            stdin_lines: None,
            sort_records: Vec::new(),
            sort_return: None,
            return_code: 0,
        }
    }

    fn alloc(&mut self, bytes: &[u8]) -> usize {
        let base = self.mem.len().next_multiple_of(8);
        self.mem.resize(base, 0);
        self.mem.extend_from_slice(bytes);
        base
    }

    /// Creates (or returns) the instance of program `index` of `module`.
    fn instantiate(&mut self, module: &Rc<Module>, index: u32) -> usize {
        let code = Rc::new(module.programs[index as usize].clone());
        if let Some(&i) = self.by_name.get(&code.name) {
            return i;
        }
        let static_base = self.alloc(&code.static_init);
        let mut ext_bases = Vec::new();
        for e in &code.externals {
            let a = match self.externals.get(&e.name) {
                Some(&a) => a,
                None => {
                    let a = self.alloc(&vec![0x40; e.len as usize]);
                    self.externals.insert(e.name.clone(), a);
                    a
                }
            };
            ext_bases.push(a);
        }
        let mut files = Vec::new();
        for def in &code.files {
            let slot = if let Some((p, k)) = def.owner {
                let o = self.instantiate(module, p);
                self.instances[o].files[k as usize]
            } else if def.external {
                match self.ext_files.get(&def.name) {
                    Some(&s) => s,
                    None => {
                        self.file_states.push(FileState::default());
                        let s = self.file_states.len() - 1;
                        self.ext_files.insert(def.name.clone(), s);
                        s
                    }
                }
            } else {
                self.file_states.push(FileState::default());
                self.file_states.len() - 1
            };
            files.push(slot);
        }
        let inst = Instance {
            linkage: vec![None; code.linkage_count as usize],
            perform: Vec::new(),
            temps: vec![Dec::ZERO; code.temps as usize],
            alter: code.alter_init.clone(),
            files,
            ext_bases,
            active: false,
            initialized: false,
            module: module.clone(),
            module_index: index,
            static_base,
            code,
        };
        self.instances.push(inst);
        let i = self.instances.len() - 1;
        self.by_name.insert(self.instances[i].code.name.clone(), i);
        i
    }

    /// Resets a program to its initial state (INITIAL programs, CANCEL).
    fn reset(&mut self, i: usize) {
        let base = self.instances[i].static_base;
        let init = self.instances[i].code.static_init.clone();
        self.mem[base..base + init.len()].copy_from_slice(&init);
        let inst = &mut self.instances[i];
        inst.alter = inst.code.alter_init.clone();
        inst.temps.iter_mut().for_each(|t| *t = Dec::ZERO);
    }

    /// Runs the first program of `module` as the main program.
    pub fn run_main(&mut self, module: Module) -> RResult<i32> {
        let module = Rc::new(module);
        let main = self.instantiate(&module, 0);
        let r = self.run(main);
        self.close_all();
        let _ = self.stdout.flush();
        match r {
            Ok(Exit::StopRun(code)) => Ok(code),
            Err(e) => Err(e),
        }
    }

    fn close_all(&mut self) {
        for i in 0..self.instances.len() {
            for f in 0..self.instances[i].files.len() {
                let _ = self.close_file(i, f as u32, false);
            }
        }
    }

    fn return_code_value(&self) -> i32 {
        self.return_code
    }

    /// Executes from the entry of instance `main` until STOP RUN.
    fn run(&mut self, main: usize) -> RResult<Exit> {
        let mut stack: Vec<Frame> = Vec::new();
        let mut cur = main;
        let mut pc: u32 = 0;
        self.instances[cur].active = true;
        self.instances[cur].initialized = true;
        loop {
            let code = self.instances[cur].code.clone();
            let Some(ins) = code.code.get(pc as usize) else {
                return err(format!(
                    "{}: control ran past the end of the program",
                    code.name
                ));
            };
            pc += 1;
            match ins {
                Instr::Nop => {}
                Instr::Move(src, dst) => {
                    let v = self.eval_src(cur, src)?;
                    self.move_val(cur, &v, dst)?;
                }
                Instr::Store {
                    value,
                    dst,
                    rounded,
                    check,
                } => match self.eval_ex(cur, value) {
                    Ok(v) => self.store(cur, v, dst, *rounded, *check)?,
                    Err(e) if e.0 == SIZE_ERROR => self.size_error = true,
                    Err(e) => return Err(e),
                },
                Instr::DivRem {
                    dividend,
                    divisor,
                    quot,
                    rounded,
                    rem,
                    check,
                } => {
                    let a = self.eval_ex(cur, dividend)?;
                    let b = self.eval_ex(cur, divisor)?;
                    self.div_rem(cur, a, b, quot, *rounded, rem, *check)?;
                }
                Instr::ClearSizeError => self.size_error = false,
                Instr::SetTemp(t, ex) => {
                    let v = self.eval_ex(cur, ex)?;
                    self.instances[cur].temps[*t as usize] = v;
                }
                Instr::Jump(t) => pc = *t,
                Instr::JumpIf(c, t) => {
                    if self.eval_cx(cur, c)? {
                        pc = *t;
                    }
                }
                Instr::JumpUnless(c, t) => {
                    if !self.eval_cx(cur, c)? {
                        pc = *t;
                    }
                }
                Instr::Perform { start, end, ret } => {
                    let inst = &mut self.instances[cur];
                    if inst.perform.len() > 10_000 {
                        return err("PERFORM nesting too deep (recursive PERFORM?)");
                    }
                    inst.perform.push((*end, *ret));
                    pc = *start;
                }
                Instr::ParaEnd(p) => {
                    let inst = &mut self.instances[cur];
                    if inst.perform.last().is_some_and(|&(end, _)| end == *p) {
                        let (_, ret) = inst.perform.pop().unwrap();
                        if ret == USE_RETURN {
                            // end of a GLOBAL declarative of a containing program
                            let f = stack.pop().unwrap();
                            cur = f.inst;
                            pc = f.pc;
                        } else {
                            pc = ret;
                        }
                    }
                }
                Instr::UseCheck { file, handled } => {
                    let st = std::mem::replace(&mut self.last_status, *b"00");
                    let skip = st[0] == b'0'
                        || (st[0] == b'1' && handled & 1 != 0)
                        || (st[0] == b'2' && handled & 2 != 0);
                    if !skip && let Some((inst, start, end)) = self.find_use(cur, *file) {
                        let target = self.instances[inst].code.paragraphs[start as usize].start;
                        if inst == cur {
                            self.instances[cur].perform.push((end, pc));
                        } else {
                            stack.push(Frame { inst: cur, pc });
                            cur = inst;
                            self.instances[cur].perform.push((end, USE_RETURN));
                        }
                        pc = target;
                    }
                }
                Instr::GoDepending(ex, targets) => {
                    let v = self.eval_ex(cur, ex)?;
                    let n = v
                        .rescale(0, zeus_decimal::Rounding::Truncate)
                        .map(|d| d.coef)
                        .unwrap_or(0);
                    if n >= 1 && (n as usize) <= targets.len() {
                        pc = targets[n as usize - 1];
                    }
                }
                Instr::AlterableGo(slot) => pc = self.instances[cur].alter[*slot as usize],
                Instr::Alter(slot, target) => self.instances[cur].alter[*slot as usize] = *target,
                Instr::StopRun => return Ok(Exit::StopRun(self.return_code_value())),
                Instr::StopLiteral(src) => {
                    let v = self.eval_src(cur, src)?;
                    let bytes = self.display_bytes(cur, &v)?;
                    let _ = self.stdout.write_all(&ebcdic::decode_latin1(&bytes));
                    let _ = self.stdout.write_all(b"\n");
                    let _ = self.stdout.flush();
                }
                Instr::ExitProgram | Instr::GoBack => {
                    let is_main = stack.is_empty();
                    if is_main {
                        if matches!(ins, Instr::GoBack) {
                            return Ok(Exit::StopRun(self.return_code_value()));
                        }
                        // EXIT PROGRAM in a main program: continue
                        continue;
                    }
                    self.instances[cur].active = false;
                    self.instances[cur].perform.clear();
                    if self.instances[cur].code.initial {
                        self.close_program_files(cur);
                    }
                    let f = stack.pop().unwrap();
                    cur = f.inst;
                    pc = f.pc;
                }
                Instr::Display {
                    items,
                    upon_console: _,
                    advance,
                } => {
                    let mut out = Vec::new();
                    for it in items {
                        let v = self.eval_src(cur, it)?;
                        out.extend(self.display_bytes(cur, &v)?);
                    }
                    let _ = self.stdout.write_all(&ebcdic::decode_latin1(&out));
                    if *advance {
                        let _ = self.stdout.write_all(b"\n");
                    }
                }
                Instr::Accept { dst, from } => self.accept(cur, dst, *from)?,
                Instr::Open(f, mode) => self.open_file(cur, *f, *mode)?,
                Instr::Close(f, lock) => self.close_file(cur, *f, *lock)?,
                Instr::CloseReel(f) => self.close_reel(cur, *f)?,
                Instr::Write {
                    file,
                    record,
                    from,
                    advance,
                } => self.write_file(cur, *file, *record, from.as_ref(), advance.as_ref())?,
                Instr::Rewrite { file, record, from } => {
                    self.rewrite_file(cur, *file, *record, from.as_ref())?
                }
                Instr::Read {
                    file,
                    next,
                    into,
                    key,
                } => self.read_file(cur, *file, *next, into.as_ref(), *key)?,
                Instr::Delete(f) => self.delete_record(cur, *f)?,
                Instr::Start { file, rel, key } => {
                    self.start_file(cur, *file, *rel, key.as_ref())?
                }
                Instr::Call {
                    program,
                    args,
                    returning: _,
                } => {
                    let v = self.eval_src(cur, program)?;
                    let name_bytes = self.alnum_bytes(cur, &v, 0)?;
                    let name = String::from_utf8_lossy(&ebcdic::decode_latin1(&name_bytes))
                        .trim()
                        .to_ascii_uppercase();
                    self.call_exception = false;
                    match self.resolve_program(cur, &name) {
                        Some(callee) => {
                            if self.instances[callee].active {
                                return err(format!("recursive CALL of active program {name}"));
                            }
                            let addrs = self.call_args(cur, args)?;
                            let ci = &mut self.instances[callee];
                            if ci.code.initial || !ci.initialized {
                                let first = !ci.initialized;
                                ci.initialized = true;
                                if !first {
                                    self.reset(callee);
                                }
                            }
                            let ci = &mut self.instances[callee];
                            for (k, slot) in ci.code.using.clone().iter().enumerate() {
                                ci.linkage[*slot as usize] = addrs.get(k).copied().flatten();
                            }
                            ci.active = true;
                            ci.perform.clear();
                            stack.push(Frame { inst: cur, pc });
                            if stack.len() > 1000 {
                                return err("CALL nesting too deep");
                            }
                            cur = callee;
                            pc = 0;
                        }
                        None => {
                            // ON EXCEPTION / ON OVERFLOW handles the failure.
                            let next = code.code.get(pc as usize);
                            let handled =
                                matches!(next, Some(Instr::JumpUnless(zir::Cx::CallException, _)));
                            if !handled {
                                return err(format!("program {name} not found"));
                            }
                            self.call_exception = true;
                        }
                    }
                }
                Instr::Cancel(src) => {
                    let v = self.eval_src(cur, src)?;
                    let name_bytes = self.alnum_bytes(cur, &v, 0)?;
                    let name = String::from_utf8_lossy(&ebcdic::decode_latin1(&name_bytes))
                        .trim()
                        .to_ascii_uppercase();
                    if let Some(&i) = self.by_name.get(&name)
                        && !self.instances[i].active
                    {
                        self.close_program_files(i);
                        self.reset(i);
                        self.instances[i].initialized = false;
                    }
                }
                Instr::Initialize { .. } => return err("INITIALIZE instruction not expected"),
                Instr::Inspect(loc, steps) => self.inspect(cur, loc, steps)?,
                Instr::StringStmt {
                    sources,
                    into,
                    pointer,
                } => self.string_stmt(cur, sources, into, pointer.as_ref())?,
                Instr::Unstring {
                    source,
                    delims,
                    into,
                    pointer,
                    tallying,
                } => self.unstring(
                    cur,
                    source,
                    delims,
                    into,
                    pointer.as_ref(),
                    tallying.as_ref(),
                )?,
                Instr::SetIndex(loc, ex) => {
                    let v = self.eval_ex(cur, ex)?;
                    self.set_value(cur, loc, v)?;
                }
                Instr::SetSwitch(idx, on) => {
                    let name = code.switches[*idx as usize].clone();
                    self.switches.insert(name, *on);
                }
                Instr::SearchStep { .. } => return err("SEARCH step not expected"),
                Instr::Sort {
                    file,
                    keys,
                    duplicates: _,
                    using,
                    giving,
                    input,
                    output,
                    collating,
                    merge,
                } => {
                    let sort = files::SortSpec {
                        file: *file,
                        keys: keys.clone(),
                        collating: collating.clone(),
                        merge: *merge,
                    };
                    // Input phase
                    self.sort_records.clear();
                    for &u in using {
                        self.sort_read_file(cur, u)?;
                    }
                    if let Some((start, end)) = input {
                        self.sort_records.clear();
                        self.run_procedure(cur, *start, *end, &mut stack)?;
                    }
                    let records = std::mem::take(&mut self.sort_records);
                    let sorted = self.sort_sort(cur, &sort, records)?;
                    if let Some((start, end)) = output {
                        self.sort_return = Some((sorted, 0));
                        self.run_procedure(cur, *start, *end, &mut stack)?;
                        self.sort_return = None;
                    } else {
                        for &g in giving {
                            self.sort_write_file(cur, g, &sorted)?;
                        }
                    }
                }
                Instr::Release(f, from) => {
                    let rec = self.sort_record_bytes(cur, *f, from.as_ref())?;
                    self.sort_records.push(rec);
                }
                Instr::Return(f, into) => self.sort_return_record(cur, *f, into.as_ref())?,
                Instr::Unsupported(name) => {
                    return err(format!(
                        "{}: statement {name} is not supported yet",
                        code.name
                    ));
                }
            }
        }
    }

    /// Runs an input/output procedure (paragraphs `start`..=`end`) to
    /// completion within the current program.
    fn run_procedure(
        &mut self,
        cur: usize,
        start: u32,
        end: u32,
        _stack: &mut Vec<Frame>,
    ) -> RResult<()> {
        // Execute a nested interpreter loop restricted to this program: a
        // PERFORM frame returning to a sentinel address.
        let code = self.instances[cur].code.clone();
        let sentinel = code.code.len() as u32 + 1;
        self.instances[cur].perform.push((end, sentinel));
        let mut pc = start;
        let depth = self.instances[cur].perform.len();
        loop {
            if pc == sentinel {
                return Ok(());
            }
            let Some(ins) = code.code.get(pc as usize) else {
                return err("procedure ran past the end of the program");
            };
            // Only a subset of control is needed inside procedures; reuse
            // the main loop by single-stepping.
            match ins {
                Instr::Perform { start, end, ret } => {
                    self.instances[cur].perform.push((*end, *ret));
                    pc = *start;
                    continue;
                }
                Instr::ParaEnd(p) => {
                    let inst = &mut self.instances[cur];
                    if inst.perform.last().is_some_and(|&(e, _)| e == *p) {
                        let (_, ret) = inst.perform.pop().unwrap();
                        pc = ret;
                        if inst.perform.len() < depth && pc == sentinel {
                            return Ok(());
                        }
                        continue;
                    }
                    pc += 1;
                    continue;
                }
                Instr::Jump(t) => {
                    pc = *t;
                    continue;
                }
                Instr::JumpIf(c, t) => {
                    pc = if self.eval_cx(cur, c)? { *t } else { pc + 1 };
                    continue;
                }
                Instr::JumpUnless(c, t) => {
                    pc = if !self.eval_cx(cur, c)? { *t } else { pc + 1 };
                    continue;
                }
                Instr::UseCheck { file, handled } => {
                    let st = std::mem::replace(&mut self.last_status, *b"00");
                    let skip = st[0] == b'0'
                        || (st[0] == b'1' && handled & 1 != 0)
                        || (st[0] == b'2' && handled & 2 != 0);
                    if !skip
                        && let Some((inst, start, end)) = self.find_use(cur, *file)
                        && inst == cur
                    {
                        self.instances[cur].perform.push((end, pc + 1));
                        pc = code.paragraphs[start as usize].start;
                        continue;
                    }
                    pc += 1;
                    continue;
                }
                Instr::GoBack | Instr::ExitProgram | Instr::StopRun => {
                    return err("STOP RUN / EXIT PROGRAM inside a SORT procedure is not supported");
                }
                _ => {}
            }
            // Execute a single straight-line instruction through a
            // temporary one-instruction program view.
            self.step(cur, ins)?;
            pc += 1;
        }
    }

    /// Executes one non-control instruction.
    fn step(&mut self, cur: usize, ins: &Instr) -> RResult<()> {
        match ins {
            Instr::Move(src, dst) => {
                let v = self.eval_src(cur, src)?;
                self.move_val(cur, &v, dst)
            }
            Instr::Store {
                value,
                dst,
                rounded,
                check,
            } => match self.eval_ex(cur, value) {
                Ok(v) => self.store(cur, v, dst, *rounded, *check),
                Err(e) if e.0 == SIZE_ERROR => {
                    self.size_error = true;
                    Ok(())
                }
                Err(e) => Err(e),
            },
            Instr::ClearSizeError => {
                self.size_error = false;
                Ok(())
            }
            Instr::SetTemp(t, ex) => {
                let v = self.eval_ex(cur, ex)?;
                self.instances[cur].temps[*t as usize] = v;
                Ok(())
            }
            Instr::Release(f, from) => {
                let rec = self.sort_record_bytes(cur, *f, from.as_ref())?;
                self.sort_records.push(rec);
                Ok(())
            }
            Instr::Return(f, into) => self.sort_return_record(cur, *f, into.as_ref()),
            Instr::SetIndex(loc, ex) => {
                let v = self.eval_ex(cur, ex)?;
                self.set_value(cur, loc, v)
            }
            Instr::Display { items, advance, .. } => {
                let mut out = Vec::new();
                for it in items {
                    let v = self.eval_src(cur, it)?;
                    out.extend(self.display_bytes(cur, &v)?);
                }
                let _ = self.stdout.write_all(&ebcdic::decode_latin1(&out));
                if *advance {
                    let _ = self.stdout.write_all(b"\n");
                }
                Ok(())
            }
            Instr::Open(f, mode) => self.open_file(cur, *f, *mode),
            Instr::Close(f, lock) => self.close_file(cur, *f, *lock),
            Instr::CloseReel(f) => self.close_reel(cur, *f),
            Instr::Write {
                file,
                record,
                from,
                advance,
            } => self.write_file(cur, *file, *record, from.as_ref(), advance.as_ref()),
            Instr::Read {
                file,
                next,
                into,
                key,
            } => self.read_file(cur, *file, *next, into.as_ref(), *key),
            Instr::Inspect(loc, steps) => self.inspect(cur, loc, steps),
            Instr::StringStmt {
                sources,
                into,
                pointer,
            } => self.string_stmt(cur, sources, into, pointer.as_ref()),
            Instr::Unstring {
                source,
                delims,
                into,
                pointer,
                tallying,
            } => self.unstring(
                cur,
                source,
                delims,
                into,
                pointer.as_ref(),
                tallying.as_ref(),
            ),
            Instr::Nop | Instr::Alter(..) => Ok(()),
            other => err(format!(
                "statement not supported inside a SORT procedure: {other:?}"
            )),
        }
    }

    fn resolve_program(&mut self, cur: usize, name: &str) -> Option<usize> {
        // Programs contained in the caller's module.
        let module = self.instances[cur].module.clone();
        let me = self.instances[cur].module_index;
        if let Some(idx) = module.programs.iter().position(|p| p.name == name) {
            let p = &module.programs[idx];
            // directly contained, or COMMON sibling, or any for simplicity
            let visible = p.parent == Some(me)
                || (p.common && p.parent == module.programs[me as usize].parent)
                || p.parent.is_none()
                || p.parent.is_some();
            if visible {
                return Some(self.instantiate(&module, idx as u32));
            }
        }
        if let Some(&i) = self.by_name.get(name) {
            return Some(i);
        }
        // Separately compiled program: NAME.zo in the search directories.
        for dir in self.search_dirs.clone() {
            let path = dir.join(format!("{name}.zo"));
            if path.is_file()
                && let Ok(m) = crate::load_module(&path)
            {
                let m = Rc::new(m);
                return Some(self.instantiate(&m, 0));
            }
        }
        None
    }

    /// The USE AFTER ERROR procedure applicable to file `f` of instance
    /// `cur`: (instance, first paragraph, last paragraph).
    fn find_use(&mut self, cur: usize, f: u32) -> Option<(usize, u32, u32)> {
        let slot = self.instances[cur].files[f as usize];
        let mode = self.file_states[slot].open_mode().or(self.last_open_mode);
        let mut inst = cur;
        loop {
            let code = self.instances[inst].code.clone();
            // the file as known in this program
            let fi = if inst == cur {
                Some(f)
            } else {
                self.instances[inst]
                    .files
                    .iter()
                    .position(|&s| s == slot)
                    .map(|i| i as u32)
            };
            let local = inst == cur;
            let usable = code.uses.iter().filter(|u| local || u.global);
            let named = usable
                .clone()
                .find(|u| fi.is_some_and(|fi| u.files.contains(&fi)));
            let by_mode = || usable.clone().find(|u| u.mode.is_some() && u.mode == mode);
            if let Some(u) = named.or_else(by_mode) {
                return Some((inst, u.start, u.end));
            }
            let parent = code.parent?;
            let module = self.instances[inst].module.clone();
            inst = self.instantiate(&module, parent);
        }
    }

    fn close_program_files(&mut self, i: usize) {
        for f in 0..self.instances[i].files.len() {
            let def = &self.instances[i].code.files[f];
            if def.owner.is_some() || def.external {
                continue;
            }
            let _ = self.close_file(i, f as u32, false);
        }
    }
}

/// PERFORM return address marking the end of a declarative run on behalf
/// of a contained program.
const USE_RETURN: u32 = u32::MAX;

pub const SIZE_ERROR: &str = "\u{0}size error";

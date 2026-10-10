//! The ZIR interpreter.

pub mod data;
mod eval;
mod files;
mod func;
mod le;
mod report;
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
    /// Segment of the paragraph being executed (segmentation).
    segment: u8,
    /// Report Writer state of each report.
    reports: Vec<report::ReportState>,
    /// Debugging: how control is reaching the next procedure, and the
    /// source line responsible.
    debug_cause: (u8, u32),
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
    /// DD allocations of the run.
    dd: zeus_ds::dd::DdTable,
    /// Record formats of the sequential files opened (by assignment name):
    /// format and maximum record length.
    opened: Vec<(String, zeus_ds::format::RecordFormat, u32)>,
    /// Job, step and procedure step names, and the DD names of the step
    /// with whether each has a device (shown in the TIOT).
    step: StepInfo,
    /// SUPPRESS PRINTING was executed (USE BEFORE REPORTING).
    suppress_printing: bool,
    /// Object-time debugging switch (ZEUS_DEBUG=Y): USE FOR DEBUGGING
    /// sections run only when it is on.
    debug: bool,
    /// Status code of the last I/O statement (for USE procedures).
    last_status: [u8; 2],
    /// Mode of the last OPEN attempted.
    last_open_mode: Option<zir::OpenMode>,
    call_exception: bool,
    overflow: bool,
    stdout: Box<dyn Write>,
    stdin_lines: Option<Box<dyn Iterator<Item = std::io::Result<String>>>>,
    /// Records released to the active SORT.
    sort_records: Vec<Vec<u8>>,
    sort_return: Option<(Vec<Vec<u8>>, usize)>,
    return_code: i32,
    /// Programs implemented by the host (see [`Machine::add_native`]).
    natives: HashMap<String, NativeFn>,
}

/// A program implemented by the host: it gets the storage and the
/// addresses of the CALL arguments (None: OMITTED) and returns the
/// return code or an error that ends the run unit.
pub type NativeFn = fn(&mut [u8], &[Option<usize>]) -> Result<i32, String>;

/// The job step a program runs in (z/OS TIOT contents).
#[derive(Debug, Clone, Default)]
pub struct StepInfo {
    pub job: String,
    pub step: String,
    pub proc_step: String,
    /// DD names; None: the names allocated in the DD table.
    pub dds: Option<Vec<(String, bool)>>,
    /// EXEC PARM, passed to the main program as a halfword length
    /// followed by the text.
    pub parm: String,
}

/// Size of the low-storage block at address 0 (the PSA).
const PSA_SIZE: usize = 4096;
/// PSA fields holding the current TCB address (PSATNEW / PSATOLD).
const PSA_TCB: [usize; 2] = [0x218, 0x21C];
/// TCB field holding the TIOT address (TCBTIO).
const TCB_TIOT: usize = 0x0C;

pub enum Exit {
    StopRun(i32),
    /// A program run by `run_from(_, true)` returned to its caller.
    Returned,
}

/// Error used to unwind a STOP RUN executed below a SORT procedure.
const STOP_RUN: &str = "\u{0}stop run";

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
            mem: vec![0; PSA_SIZE],
            instances: Vec::new(),
            file_states: Vec::new(),
            externals: HashMap::new(),
            ext_files: HashMap::new(),
            by_name: HashMap::new(),
            search_dirs,
            switches,
            size_error: false,
            io_cond: false,
            suppress_printing: false,
            step: StepInfo::default(),
            opened: Vec::new(),
            natives: HashMap::new(),
            dd: zeus_ds::dd::DdTable::from_env().unwrap_or_else(|e| {
                eprintln!("zcobc: ZEUS_DD_CONFIG: {e}");
                zeus_ds::dd::DdTable::default()
            }),
            debug: std::env::var("ZEUS_DEBUG")
                .is_ok_and(|v| matches!(v.to_ascii_uppercase().as_str(), "Y" | "YES" | "ON" | "1")),
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
            segment: 0,
            reports: vec![report::ReportState::default(); code.reports.len()],
            debug_cause: (zir::debug_cause::START_PROGRAM, 0),
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
        inst.segment = 0;
        inst.temps.iter_mut().for_each(|t| *t = Dec::ZERO);
    }

    /// Runs the first program of `module` as the main program.
    /// Sends DISPLAY output to `out` instead of standard output.
    pub fn set_stdout(&mut self, out: Box<dyn Write>) {
        self.stdout = out;
    }

    /// Record format used for each sequential file opened during the run
    /// (assignment name, format, maximum record length): the attributes
    /// of new data sets whose DD gave none.
    pub fn opened_formats(&self) -> &[(String, zeus_ds::format::RecordFormat, u32)] {
        &self.opened
    }

    /// Allocates a DD name with the data set's attributes.
    pub fn allocate(&mut self, name: &str, a: zeus_ds::dd::Allocation) {
        self.dd.allocate(name, a);
    }

    /// Input of ACCEPT (the SYSIN data of the step).
    pub fn set_stdin(&mut self, lines: Vec<String>) {
        self.stdin_lines = Some(Box::new(lines.into_iter().map(Ok)));
    }

    /// Makes `name` callable as a host implemented program; programs
    /// found in the search directories take precedence.
    pub fn add_native(&mut self, name: &str, f: NativeFn) {
        self.natives.insert(name.to_ascii_uppercase(), f);
    }

    /// Sets the job step identification shown to programs (TIOT).
    pub fn set_step(&mut self, step: StepInfo) {
        self.step = step;
    }

    /// Builds the TCB and the TIOT of the step and anchors them in the
    /// PSA at address 0, as programs that walk the z/OS control blocks
    /// expect.
    fn build_control_blocks(&mut self) {
        let name = |s: &str| {
            let mut b = ebcdic::encode(&format!("{s:<8}"));
            b.truncate(8);
            b
        };
        let dds = match &self.step.dds {
            Some(d) => d.clone(),
            None => self.dd.names().into_iter().map(|n| (n, true)).collect(),
        };
        let job = if self.step.job.is_empty() {
            "ZEUSJOB"
        } else {
            &self.step.job
        };
        let step = if self.step.step.is_empty() {
            "STEP1"
        } else {
            &self.step.step
        };
        let mut tiot = Vec::new();
        tiot.extend(name(job));
        tiot.extend(name(step));
        tiot.extend(name(&self.step.proc_step));
        for (dd, device) in &dds {
            let mut e = vec![0u8; 20];
            e[0] = 20; // TIOELNGH
            e[4..12].copy_from_slice(&name(dd));
            if *device {
                e[17..20].copy_from_slice(&[0x00, 0x10, 0x00]); // UCB address
            }
            tiot.extend(e);
        }
        tiot.extend([0u8; 4]); // end of the TIOT
        let tiot_addr = self.alloc(&tiot);
        let mut tcb = vec![0u8; 256];
        tcb[TCB_TIOT..TCB_TIOT + 4].copy_from_slice(&(tiot_addr as u32).to_be_bytes());
        let tcb_addr = self.alloc(&tcb);
        for f in PSA_TCB {
            self.mem[f..f + 4].copy_from_slice(&(tcb_addr as u32).to_be_bytes());
        }
    }

    /// Assigns a file (by ASSIGN name or file name) to a path.
    pub fn set_dd(&mut self, name: &str, path: PathBuf) {
        self.dd.set(name, path);
    }

    /// Sets the object-time debugging switch (default: `ZEUS_DEBUG`).
    pub fn set_debug(&mut self, on: bool) {
        self.debug = on;
    }

    pub fn run_main(&mut self, module: Module) -> RResult<i32> {
        self.build_control_blocks();
        let module = Rc::new(module);
        let main = self.instantiate(&module, 0);
        // PARM: the first USING parameter of the main program
        if let Some(&slot) = self.instances[main].code.using.first() {
            let text = ebcdic::encode(&self.step.parm);
            let mut area = (text.len() as u16).to_be_bytes().to_vec();
            area.extend(text);
            area.resize(area.len().max(102), ebcdic::SPACE);
            let a = self.alloc(&area);
            self.instances[main].linkage[slot as usize] = Some(a);
        }
        let r = self.run(main);
        self.close_all();
        let _ = self.stdout.flush();
        match r {
            Ok(Exit::StopRun(code)) => Ok(code),
            Ok(Exit::Returned) => Ok(self.return_code_value()),
            Err(e) if e.0 == STOP_RUN => Ok(self.return_code_value()),
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
    /// Resolves and sets up the program named by a CALL: binds its
    /// USING parameters and puts it in its initial state when needed.
    /// Returns None for a missing program handled by ON EXCEPTION.
    fn prepare_call(
        &mut self,
        cur: usize,
        program: &zir::Src,
        args: &[zir::CallArgMode],
        handled: bool,
    ) -> RResult<Option<usize>> {
        let v = self.eval_src(cur, program)?;
        let name_bytes = self.alnum_bytes(cur, &v, 0)?;
        let name = String::from_utf8_lossy(&ebcdic::decode_latin1(&name_bytes))
            .trim()
            .to_ascii_uppercase();
        self.call_exception = false;
        if matches!(name.as_str(), "CEE3ABD" | "CEEDAYS") {
            let addrs = self.call_args(cur, args)?;
            self.le_service(&name, &addrs)?;
            return Ok(None);
        }
        let Some(callee) = self.resolve_program(cur, &name) else {
            if let Some(f) = self.natives.get(&name).copied() {
                let addrs = self.call_args(cur, args)?;
                self.return_code = f(&mut self.mem, &addrs).map_err(RunError)?;
                return Ok(None);
            }
            if !handled {
                return err(format!("program {name} not found"));
            }
            self.call_exception = true;
            return Ok(None);
        };
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
        Ok(Some(callee))
    }

    /// Executes from the entry of instance `main` until STOP RUN.
    fn run(&mut self, main: usize) -> RResult<Exit> {
        self.instances[main].active = true;
        self.instances[main].initialized = true;
        self.run_from(main, false)
    }

    /// Runs instance `start` from its entry. A `called` program returns
    /// (Exit::Returned) at EXIT PROGRAM / GOBACK.
    fn run_from(&mut self, start: usize, called: bool) -> RResult<Exit> {
        let mut stack: Vec<Frame> = Vec::new();
        let mut cur = start;
        let mut pc: u32 = 0;
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
                        let code = &inst.code;
                        if !code.segments.is_empty() && ret != USE_RETURN {
                            // returning from a PERFORM does not
                            // reinitialize the caller's segment
                            let i = code.segments.partition_point(|&(s, _)| s <= ret);
                            if i > 0 {
                                inst.segment = code.segments[i - 1].1;
                            }
                        }
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
                Instr::Segment(seg) => self.enter_segment(cur, *seg),
                Instr::SetAddress(n, ex) => self.set_address(cur, *n, ex)?,
                Instr::Initiate(r) => self.report_initiate(cur, *r)?,
                Instr::Generate { report, group } => self.report_generate(cur, *report, *group)?,
                Instr::Terminate(r) => self.report_terminate(cur, *r)?,
                Instr::SuppressPrinting => self.suppress_printing = true,
                Instr::DebugCause(c, line) => self.instances[cur].debug_cause = (*c, *line),
                Instr::DebugProc { .. } | Instr::DebugData { .. } | Instr::DebugFile { .. } => {
                    if let Some((target, end)) = self.debug_fire(cur, ins)? {
                        self.instances[cur].perform.push((end, pc));
                        pc = target;
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
                    // The run waits for the operator's reply; an interrupt
                    // (ETX) cancels it.
                    if self.stdin_line().is_some_and(|l| l.contains('\u{3}')) {
                        return Ok(Exit::StopRun(self.return_code_value()));
                    }
                }
                Instr::ExitProgram | Instr::GoBack => {
                    let is_main = stack.is_empty();
                    if is_main && called {
                        self.instances[cur].active = false;
                        self.instances[cur].perform.clear();
                        if self.instances[cur].code.initial {
                            self.close_program_files(cur);
                        }
                        return Ok(Exit::Returned);
                    }
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
                    // ON EXCEPTION / ON OVERFLOW handles a missing program.
                    let next = code.code.get(pc as usize);
                    let handled =
                        matches!(next, Some(Instr::JumpUnless(zir::Cx::CallException, _)));
                    if let Some(callee) = self.prepare_call(cur, program, args, handled)? {
                        stack.push(Frame { inst: cur, pc });
                        if stack.len() > 1000 {
                            return err("CALL nesting too deep");
                        }
                        cur = callee;
                        pc = 0;
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
                        self.instances[cur].debug_cause = (zir::debug_cause::SORT_INPUT, 0);
                        self.run_procedure(cur, *start, *end, &mut stack)?;
                    }
                    let records = std::mem::take(&mut self.sort_records);
                    let sorted = self.sort_sort(cur, &sort, records)?;
                    if let Some((start, end)) = output {
                        self.sort_return = Some((sorted, 0));
                        self.instances[cur].debug_cause = if *merge {
                            (zir::debug_cause::MERGE_OUTPUT, 0)
                        } else {
                            (zir::debug_cause::SORT_OUTPUT, 0)
                        };
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
                Instr::AlterableGo(slot) => {
                    pc = self.instances[cur].alter[*slot as usize];
                    continue;
                }
                Instr::GoDepending(ex, targets) => {
                    let v = self.eval_ex(cur, ex)?;
                    let n = v
                        .rescale(0, zeus_decimal::Rounding::Truncate)
                        .map(|d| d.coef)
                        .unwrap_or(0);
                    pc = if n >= 1 && (n as usize) <= targets.len() {
                        targets[n as usize - 1]
                    } else {
                        pc + 1
                    };
                    continue;
                }
                Instr::DebugProc { .. } | Instr::DebugData { .. } | Instr::DebugFile { .. } => {
                    if let Some((target, end)) = self.debug_fire(cur, ins)? {
                        self.instances[cur].perform.push((end, pc + 1));
                        pc = target;
                        continue;
                    }
                    pc += 1;
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
            Instr::Alter(slot, target) => {
                self.instances[cur].alter[*slot as usize] = *target;
                Ok(())
            }
            Instr::Segment(seg) => {
                self.enter_segment(cur, *seg);
                Ok(())
            }
            Instr::DebugCause(c, line) => {
                self.instances[cur].debug_cause = (*c, *line);
                Ok(())
            }
            Instr::SetAddress(n, ex) => self.set_address(cur, *n, ex),
            Instr::Initiate(r) => self.report_initiate(cur, *r),
            Instr::Generate { report, group } => self.report_generate(cur, *report, *group),
            Instr::Terminate(r) => self.report_terminate(cur, *r),
            Instr::SuppressPrinting => {
                self.suppress_printing = true;
                Ok(())
            }
            Instr::Call { program, args, .. } => {
                if let Some(callee) = self.prepare_call(cur, program, args, false)? {
                    match self.run_from(callee, true)? {
                        Exit::Returned => {}
                        Exit::StopRun(_) => return err(STOP_RUN),
                    }
                }
                Ok(())
            }
            Instr::Nop => Ok(()),
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
        // Separately compiled program: NAME.zo, or member NAME of a load
        // library, in the search directories.
        for dir in self.search_dirs.clone() {
            for path in [dir.join(format!("{name}.zo")), dir.join(name)] {
                if path.is_file()
                    && let Ok(m) = crate::load_module(&path)
                {
                    let m = Rc::new(m);
                    return Some(self.instantiate(&m, 0));
                }
            }
        }
        None
    }

    /// Fills DEBUG-ITEM for a debugging trigger and returns the debugging
    /// section to perform: (first instruction, last paragraph).
    fn debug_fire(&mut self, cur: usize, ins: &Instr) -> RResult<Option<(u32, u32)>> {
        if !self.debug {
            return Ok(None);
        }
        let code = self.instances[cur].code.clone();
        let Some(item) = code.debug_item else {
            return Ok(None);
        };
        let text = |s: &str| ebcdic::encode(s);
        let (decl, name, line, subs, contents) = match ins {
            Instr::DebugProc {
                decl,
                name,
                contents,
                line,
            } => {
                let (cause, cline) = self.instances[cur].debug_cause;
                let c = match contents {
                    Some(c) => c.clone(),
                    None => match cause {
                        zir::debug_cause::FALL_THROUGH => "FALL THROUGH",
                        zir::debug_cause::START_PROGRAM => "START PROGRAM",
                        zir::debug_cause::PERFORM => "PERFORM LOOP",
                        zir::debug_cause::SORT_INPUT => "SORT INPUT",
                        zir::debug_cause::SORT_OUTPUT => "SORT OUTPUT",
                        zir::debug_cause::MERGE_OUTPUT => "MERGE OUTPUT",
                        _ => "",
                    }
                    .to_string(),
                };
                let l = if contents.is_none() && cline != 0 {
                    cline
                } else {
                    *line
                };
                (*decl, name.clone(), l, Vec::new(), text(&c))
            }
            Instr::DebugData {
                decl,
                name,
                loc,
                line,
            } => {
                let mut subs = Vec::new();
                for e in &loc.subs {
                    subs.push(self.eval_ex(cur, e)?);
                }
                let (a, l) = self.addr(cur, loc)?;
                (
                    *decl,
                    name.clone(),
                    *line,
                    subs,
                    self.mem[a..a + l].to_vec(),
                )
            }
            Instr::DebugFile {
                decl,
                name,
                record,
                line,
            } => {
                // READ: not after AT END or INVALID KEY
                if record.is_some() && self.last_status[0] != b'0' {
                    return Ok(None);
                }
                let c = match record {
                    Some(d) => {
                        let loc = zir::Loc {
                            desc: *d,
                            subs: Vec::new(),
                            refmod: None,
                        };
                        let (a, l) = self.addr(cur, &loc)?;
                        self.mem[a..a + l].to_vec()
                    }
                    None => Vec::new(),
                };
                (*decl, name.clone(), *line, Vec::new(), c)
            }
            _ => return Ok(None),
        };
        let loc = zir::Loc {
            desc: item,
            subs: Vec::new(),
            refmod: None,
        };
        let (a, len) = self.addr(cur, &loc)?;
        let mut buf = vec![ebcdic::SPACE; len];
        let mut put = |at: usize, bytes: &[u8], width: usize| {
            for i in 0..width.min(len.saturating_sub(at)) {
                buf[at + i] = bytes.get(i).copied().unwrap_or(ebcdic::SPACE);
            }
        };
        put(0, &text(&format!("{line:>6}")), 6);
        put(7, &text(&name), 30);
        for (k, s) in subs.iter().take(3).enumerate() {
            let n = s
                .rescale(0, zeus_decimal::Rounding::Truncate)
                .map(|d| d.coef)
                .unwrap_or(0);
            let t = format!(
                "{}{:04}",
                if n < 0 { '-' } else { '+' },
                n.unsigned_abs() % 10000
            );
            put(38 + 6 * k, &text(&t), 5);
        }
        put(56, &contents, len.saturating_sub(56));
        self.mem[a..a + len].copy_from_slice(&buf);
        let (start, end) = code.debug_decls[decl as usize];
        Ok(Some((code.paragraphs[start as usize].start, end)))
    }

    /// SET ADDRESS OF a linkage record: NULL (0) makes it unaddressable.
    fn set_address(&mut self, cur: usize, n: u32, ex: &zir::Ex) -> RResult<()> {
        let v = self.eval_ex(cur, ex)?;
        let a = v
            .rescale(0, zeus_decimal::Rounding::Truncate)
            .map(|d| d.coef)
            .unwrap_or(0);
        if a < 0 || a as usize >= self.mem.len() {
            return err(format!("SET ADDRESS OF: address {a} outside storage"));
        }
        self.instances[cur].linkage[n as usize] = Some(a as usize);
        Ok(())
    }

    /// Control reaches a paragraph of segment `seg`: an independent
    /// segment entered from another segment starts in its initial state
    /// (its ALTERed GO TOs are reset).
    fn enter_segment(&mut self, cur: usize, seg: u8) {
        let inst = &mut self.instances[cur];
        if inst.segment != seg && seg >= 50 {
            for (k, &s) in inst.code.alter_segments.iter().enumerate() {
                if s == seg {
                    inst.alter[k] = inst.code.alter_init[k];
                }
            }
        }
        inst.segment = seg;
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

//! The ZIR interpreter.

mod assembler;
mod assembler_io;
mod calls;
mod checkpoint;
pub mod data;
pub(crate) mod eval;
pub mod exec;
mod files;
mod func;
mod le;
mod numeric;
mod report;
mod sort;
mod sym;
mod text;
pub mod threaded;
mod unicode_case;

pub use eval::{civil_from_days, days_from_civil};
pub(crate) use func::timestamp;

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use zeus_decimal::binary;

use zeus_decimal::Dec;

use crate::ebcdic;
use crate::zir::{self, Instr, Module, ProgramCode};
use files::FileState;
pub use files::TapeAccessRequest;
type TapeAuthorizer = Box<dyn FnMut(&TapeAccessRequest) -> bool>;

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
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Instance {
    #[serde(default)]
    invocation_bases: HashMap<u32, usize>,
    #[serde(default)]
    dll_linked: bool,
    #[serde(default)]
    retired: bool,
    #[serde(default)]
    dynamic_calls: std::collections::HashSet<String>,
    #[serde(default)]
    static_calls: HashMap<String, usize>,
    #[serde(default)]
    bindings_ready: bool,
    rerun_counts: Vec<u64>,
    numeric_context: (i32, bool),
    wide_temps: Vec<Option<crate::numeric::number::Number>>,
    entry_pc: u32,
    code: Rc<ProgramCode>,
    /// Module the program came from (for nested program calls).
    module: Rc<Module>,
    module_index: u32,
    static_base: usize,
    /// Invocation-specific LOCAL-STORAGE; WORKING-STORAGE stays shared.
    local_base: Option<usize>,
    activation_of: Option<usize>,
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

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Frame {
    inst: usize,
    pc: u32,
    result: Option<PendingResult>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct PendingResult {
    dst: zir::Loc,
    src: Option<zir::Loc>,
    saved_rc: i32,
    resolved: bool,
    foreign_value: Option<Vec<u8>>,
}

/// A program's coverage counts: executions of each instruction, and the
/// times each conditional jump went to its target.
struct CovCounts {
    code: Rc<ProgramCode>,
    module: Rc<Module>,
    hits: Vec<u64>,
    jumped: Vec<u64>,
}

impl Drop for Machine {
    fn drop(&mut self) {
        self.flush_coverage();
    }
}

impl Machine {
    fn program_ccsid(&self, instance: usize) -> u32 {
        self.instances[instance]
            .code
            .ccsid
            .unwrap_or(self.instances[instance].module.ccsid)
    }
    fn compilation_time(&self, instance: usize) -> Option<&str> {
        self.instances[instance]
            .code
            .compiled_at
            .as_deref()
            .or(self.instances[instance].module.compiled_at.as_deref())
    }
    fn entry_binding(&self, callee: usize, name: &str) -> (u32, Vec<u32>, Vec<bool>) {
        let code = &self.instances[callee].code;
        match code.entries.iter().find(|e| e.name == name) {
            Some(entry) => (entry.pc, entry.using.clone(), entry.using_by_value.clone()),
            None => (0, code.using.clone(), code.using_by_value.clone()),
        }
    }

    fn set_entry(
        &mut self,
        cur: usize,
        targets: &[zir::Loc],
        name: &zir::Src,
        interface: Option<zir::CallInterface>,
    ) -> RResult<()> {
        let literal_name = matches!(name, zir::Src::Alnum(_));
        let convention = interface.unwrap_or(zir::CallInterface::default_for(
            self.instances[cur].code.dynam,
            self.instances[cur].code.dll,
        ));
        let dll_binding = interface.unwrap_or(zir::CallInterface::default_for(
            self.instances[cur].code.dynam,
            self.instances[cur].code.dll,
        )) == zir::CallInterface::Dll;
        let static_binding = matches!(name, zir::Src::Alnum(_))
            && interface.unwrap_or(zir::CallInterface::default_for(
                self.instances[cur].code.dynam,
                self.instances[cur].code.dll,
            )) == zir::CallInterface::Static;
        let value = self.eval_src(cur, name)?;
        let bytes = self.alnum_bytes(cur, &value, 0)?;
        let name = ebcdic::decode(&bytes).trim().to_ascii_uppercase();
        if dll_binding {
            let Some(callee) = self.resolve_dll(cur, &name, literal_name) else {
                return err(format!("SET TO ENTRY: DLL export {name} not found"));
            };
            if self.instances[callee].code.lp64 != self.instances[cur].code.lp64 {
                return err("DLL caller and target must have the same LP address mode");
            }
            if self.instances[callee].code.parent.is_some() {
                return err("SET TO ENTRY cannot reference a nested program");
            }
            let caller = self.instances[cur].activation_of.unwrap_or(cur);
            self.instances[caller]
                .static_calls
                .insert(name.clone(), callee);
            self.instances[callee].dll_linked = true;
        }
        if static_binding {
            let caller = self.instances[cur].activation_of.unwrap_or(cur);
            if let Some(callee) = self.instances[caller]
                .static_calls
                .get(&name)
                .copied()
                .or_else(|| self.resolve_program(cur, &name))
            {
                self.instances[caller]
                    .static_calls
                    .insert(name.clone(), callee);
            }
        }
        let address = if let Some((&address, _)) =
            self.entry_pointers.iter().find(|(address, n)| {
                **n == name
                    && self
                        .entry_interfaces
                        .get(address)
                        .is_none_or(|mode| *mode == convention)
            }) {
            address
        } else {
            let known = if let Some(callee) = self.resolve_program(cur, &name) {
                if self.instances[callee].code.parent.is_some() {
                    return err("SET TO ENTRY cannot reference a nested program");
                }
                true
            } else {
                le::parameter_count(&name).is_some()
                    || self.natives.contains_key(&name)
                    || self.natives_mach.contains_key(&name)
                    || self.linker.as_ref().is_some_and(|l| l.load(&name))
                    || self.load_hlasm(&name).map_err(RunError)?
            };
            if !known {
                return err(format!("SET TO ENTRY: program {name} not found"));
            }
            // A run-unit address identifies an entry; it is not a host CPU address.
            let address = self.alloc(&[0; 8]) as u64;
            self.entry_pointers.insert(address, name);
            self.entry_interfaces.insert(address, convention);
            address
        };
        for target in targets {
            self.set_value(cur, target, Dec::from_i64(address as i64))?;
        }
        Ok(())
    }

    fn rc_before_call(&mut self, cur: usize) -> RResult<i32> {
        if let Some(loc) = self.rc_loc(cur) {
            return Ok(self
                .eval_ex(cur, &zir::Ex::Load(loc))?
                .rescale(0, zeus_decimal::Rounding::Truncate)
                .map_or(0, |d| d.coef as i32));
        }
        Ok(self.return_code)
    }

    fn bind_program_result(&mut self, callee: usize) -> RResult<Option<zir::Loc>> {
        let Some(loc) = self.instances[callee].code.result.clone() else {
            return Ok(None);
        };
        let desc = self.instances[callee].code.descs[loc.desc as usize].clone();
        let zir::Base::Linkage(slot) = desc.base else {
            return err("invalid RETURNING descriptor");
        };
        let base = self.alloc(&vec![0; (desc.offset + desc.len) as usize]);
        self.instances[callee].linkage[slot as usize] = Some(base);
        Ok(Some(loc))
    }

    fn pending_result(
        &mut self,
        cur: usize,
        callee: Option<usize>,
        dst: &zir::Loc,
        saved_rc: i32,
    ) -> RResult<PendingResult> {
        let src = match callee {
            Some(c) => self.bind_program_result(c)?,
            None => None,
        };
        if callee.is_some() && src.is_none() {
            return err(
                "CALL RETURNING requires PROCEDURE DIVISION RETURNING in the called COBOL program",
            );
        }
        if let (Some(c), Some(src)) = (callee, &src) {
            let from = &self.instances[c].code.descs[src.desc as usize];
            let to = &self.instances[cur].code.descs[dst.desc as usize];
            if from.ty.abi_type() != to.ty.abi_type()
                || from.len != to.len
                || (!from.abi.is_empty() && !to.abi.is_empty() && from.abi != to.abi)
            {
                return err("CALL and PROCEDURE RETURNING attributes must match");
            }
        }
        Ok(PendingResult {
            dst: dst.clone(),
            src,
            saved_rc,
            resolved: !self.call_exception,
            foreign_value: self.call_value.take(),
        })
    }

    fn finish_result(
        &mut self,
        cur: usize,
        callee: Option<usize>,
        result: PendingResult,
    ) -> RResult<()> {
        if result.resolved {
            let value = match (callee, result.src) {
                (Some(c), Some(src)) => self.eval_src(c, &zir::Src::Loc(src))?,
                _ => {
                    if let Some(bytes) = result.foreign_value {
                        let ty = self.loc_type(cur, &result.dst);
                        let (_, len) = self.addr(cur, &result.dst)?;
                        if bytes.len() != len {
                            return err("foreign CALL RETURNING representation length mismatch");
                        }
                        eval::Val::Item(ty, bytes)
                    } else {
                        eval::Val::Num(Dec::from_i64(self.return_code as i64))
                    }
                }
            };
            self.move_val(cur, &value, &result.dst)?;
        }
        self.return_code = result.saved_rc;
        self.store_rc(cur);
        self.call_exception = !result.resolved;
        Ok(())
    }
    /// The coverage counts so far go to the process's totals (counting
    /// goes on afresh).
    pub fn flush_coverage(&mut self) {
        if let Some(cov) = self.cov.as_mut().map(std::mem::take) {
            for c in cov.into_values() {
                crate::coverage::add(crate::coverage::cobol_program(
                    &c.code,
                    &c.module.sources,
                    &c.hits,
                    &c.jumped,
                ));
            }
        }
    }
}

/// Programs compiled to machine code: `run` runs the program of
/// instance `s.cur` from `s.pc` until it leaves that program or the run
/// ends; None when that program has no machine code (the interpreter
/// runs it).
pub trait NativeCode {
    fn run(&self, m: &mut Machine, s: &mut RunState) -> Option<RResult<Option<Exit>>>;
}

/// Where a run unit's interpreter is: the program instance, the next
/// instruction, the instances that called it (in this module), and
/// whether the first was called by another program.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct RunState {
    stack: Vec<Frame>,
    pub cur: usize,
    pub pc: u32,
    called: bool,
}

pub struct Machine {
    tape_authorizer: Option<TapeAuthorizer>,
    threaded_run_unit: bool,
    recycled_file_states: Vec<usize>,
    recycled_program_storage: Vec<(usize, usize)>,
    range_checking: bool,
    pending_checkpoints: Vec<PathBuf>,
    procedure_depth: usize,
    checkpoint_files: Vec<PathBuf>,
    sort_checkpoint_pending: bool,
    mem: Vec<u8>,
    hlasm: HashMap<String, (Rc<crate::hlasm::Module>, usize, u32)>,
    hlasm_storage: crate::hlasm::DynamicStorage,
    hlasm_call_depth: usize,
    hlasm_residency: assembler::Residency,
    hlasm_io: assembler_io::Files,
    le_conditions: le::handlers::Conditions,
    /// State of FUNCTION RANDOM.
    random: u64,
    instances: Vec<Instance>,
    file_states: Vec<FileState>,
    /// EXTERNAL data areas by name.
    externals: HashMap<String, (usize, u32)>,
    compatibility_error: Option<String>,
    /// EXTERNAL files by name (slot in `file_states`).
    ext_files: HashMap<String, usize>,
    active_sort_areas: Vec<Vec<usize>>,
    by_name: HashMap<String, usize>,
    entry_pointers: HashMap<u64, String>,
    entry_interfaces: HashMap<u64, zir::CallInterface>,
    search_dirs: Vec<PathBuf>,
    switches: HashMap<String, bool>,
    size_error: bool,
    io_cond: bool,
    /// DD allocations of the run.
    dd: zeus_ds::dd::DdTable,
    /// Record formats of the sequential files opened (by assignment name):
    /// format and maximum record length.
    opened: Vec<(String, zeus_ds::format::RecordFormat, u32)>,
    opened_buffers: Vec<(String, u32, u32)>,
    tape_positions: HashMap<PathBuf, usize>,
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
    call_value: Option<Vec<u8>>,
    overflow: bool,
    stdout: Box<dyn Write>,
    stdin_lines: Option<Box<dyn Iterator<Item = std::io::Result<String>>>>,
    stdin_buffer: Option<std::collections::VecDeque<String>>,
    /// Records released to the active SORT.
    sort_records: Vec<Vec<u8>>,
    active_sort_controls: Option<sort::Controls>,
    sort_return: Option<(Vec<Vec<u8>>, usize)>,
    return_code: i32,
    /// Programs implemented by the host (see [`Machine::add_native`]).
    natives: HashMap<String, NativeFn>,
    natives_ref: HashMap<String, NativeRefFn>,
    natives_mach: HashMap<String, NativeMachFn>,
    /// Runs EXEC CICS / SQL / DLI statements.
    exec_host: Option<Rc<dyn exec::ExecHost>>,
    /// Programs of other run times (PL/I).
    linker: Option<Rc<dyn crate::interlang::Linker>>,
    /// A foreign runtime has received execution state; an unused loader alone
    /// does not prevent a standalone COBOL checkpoint in a JCL step.
    linker_used: bool,
    control_blocks: bool,
    /// Addresses of the main program's USING parameters, when the host
    /// passes them (IMS PCB masks) instead of the PARM.
    main_args: Option<Vec<Option<usize>>>,
    /// Called every few thousand instructions (the dispatcher of a CICS
    /// region gives other tasks the processor).
    tick: Option<fn()>,
    /// Machine code of programs (native executables).
    native: Option<Rc<dyn NativeCode>>,
    /// Coverage counts (ZEUS_COVERAGE) by program code.
    cov: Option<HashMap<*const ProgramCode, CovCounts>>,
    ticks: u32,
    /// Concolic execution (test generation).
    sym: Option<Box<crate::concolic::SymState>>,
}

impl crate::hlasm::Host for Machine {
    fn data(
        &mut self,
        request: crate::hlasm::DataRequest,
        memory: &mut Vec<u8>,
        storage: &mut crate::hlasm::DynamicStorage,
    ) -> Result<crate::hlasm::DataReply, String> {
        self.with_hlasm_memory(memory, storage, |machine| machine.assembler_data(request))
    }
    fn parameter_count(
        &mut self,
        name: &str,
        memory: &mut Vec<u8>,
        storage: &mut crate::hlasm::DynamicStorage,
    ) -> Result<Option<usize>, String> {
        self.with_hlasm_memory(memory, storage, |machine| {
            Ok(machine
                .find_program(name)
                .map(|index| machine.instances[index].code.using.len())
                .or_else(|| le::parameter_count(name))
                .or_else(|| {
                    machine
                        .linker
                        .as_ref()
                        .and_then(|linker| linker.parameter_count(name))
                }))
        })
    }
    fn message(&mut self, message: crate::hlasm::OperatorMessage) -> Result<u32, String> {
        writeln!(self.stdout, "{}", message.text).map_err(|e| e.to_string())?;
        self.stdout.flush().map_err(|e| e.to_string())?;
        self.hlasm_residency.message_id = self.hlasm_residency.message_id.wrapping_add(1).max(1);
        if self.hlasm_residency.messages.len() == 1024 {
            self.hlasm_residency.messages.pop_front();
        }
        self.hlasm_residency.messages.push_back(message);
        Ok(self.hlasm_residency.message_id)
    }
    fn wait(
        &mut self,
        events: &[usize],
        needed: usize,
        memory: &mut Vec<u8>,
        storage: &mut crate::hlasm::DynamicStorage,
    ) -> Result<(), String> {
        self.with_hlasm_memory(memory, storage, |machine| {
            while crate::hlasm::completed_events(events, &machine.mem)? < needed {
                let (ecb, completion) = machine.hlasm_residency.posts.pop_front().ok_or(
                    "WAIT deadlock: no pending event producer in this serialized run unit",
                )?;
                crate::hlasm::post_event(&mut machine.mem, ecb, completion)?;
                machine.hlasm_storage.verify_protection(&mut machine.mem)?;
            }
            Ok(())
        })
    }
    fn load(
        &mut self,
        name: &str,
        memory: &mut Vec<u8>,
        storage: &mut crate::hlasm::DynamicStorage,
    ) -> Result<(u32, u32), String> {
        self.with_hlasm_memory(memory, storage, |machine| {
            machine.load_assembler_service(name)
        })
    }
    fn delete(
        &mut self,
        name: &str,
        memory: &mut Vec<u8>,
        storage: &mut crate::hlasm::DynamicStorage,
    ) -> Result<i32, String> {
        self.with_hlasm_memory(memory, storage, |machine| {
            machine.delete_assembler_service(name)
        })
    }
    fn branch(
        &mut self,
        target: usize,
        registers: &mut crate::hlasm::Registers,
        memory: &mut Vec<u8>,
        storage: &mut crate::hlasm::DynamicStorage,
    ) -> Result<usize, String> {
        self.with_hlasm_memory(memory, storage, |machine| {
            machine.branch_assembler_service(target, registers)
        })
    }
    fn call(
        &mut self,
        request: crate::hlasm::AssemblerCall<'_>,
        memory: &mut Vec<u8>,
        storage: &mut crate::hlasm::DynamicStorage,
    ) -> Result<i32, String> {
        self.with_hlasm_memory(memory, storage, |machine| {
            machine.dispatch_assembler_target(request)
        })
    }
}

/// A program implemented by the host: it gets the storage and the
/// addresses of the CALL arguments (None: OMITTED) and returns the
/// return code or an error that ends the run unit.
pub type NativeFn = fn(&mut [u8], &[Option<usize>]) -> Result<i32, String>;

/// A host implemented program that needs the machine (to call programs
/// back, as MQCTL calls message consumers).
pub type NativeMachFn = fn(&mut Machine, &[Option<usize>]) -> Result<i32, String>;

/// A host implemented program called with the lengths of its arguments
/// (a PL/I caller's arguments: aggregates and pointers differ).
pub type NativeRefFn = fn(&mut [u8], &[crate::interlang::XRef]) -> Result<i32, String>;

/// The job step a program runs in (z/OS TIOT contents).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
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
            tape_authorizer: None,
            threaded_run_unit: false,
            recycled_file_states: Vec::new(),
            recycled_program_storage: Vec::new(),
            range_checking: true,
            pending_checkpoints: vec![],
            procedure_depth: 0,
            checkpoint_files: vec![],
            sort_checkpoint_pending: false,
            stdin_buffer: None,
            random: 0x0005_DEEC_E66D,
            mem: vec![0; PSA_SIZE],
            hlasm: HashMap::new(),
            hlasm_storage: crate::hlasm::DynamicStorage::default(),
            hlasm_call_depth: 0,
            hlasm_residency: assembler::Residency::default(),
            hlasm_io: assembler_io::Files::default(),
            le_conditions: le::handlers::Conditions::default(),
            instances: Vec::new(),
            file_states: Vec::new(),
            externals: HashMap::new(),
            compatibility_error: None,
            ext_files: HashMap::new(),
            active_sort_areas: vec![],
            by_name: HashMap::new(),
            entry_pointers: HashMap::new(),
            entry_interfaces: HashMap::new(),
            search_dirs,
            switches,
            size_error: false,
            io_cond: false,
            suppress_printing: false,
            step: StepInfo::default(),
            opened: Vec::new(),
            opened_buffers: Vec::new(),
            tape_positions: HashMap::new(),
            natives: HashMap::new(),
            natives_ref: HashMap::new(),
            natives_mach: HashMap::new(),
            exec_host: None,
            tick: None,
            native: None,
            cov: crate::coverage::enabled().then(HashMap::new),
            ticks: 0,
            sym: None,
            dd: zeus_ds::dd::DdTable::from_env().unwrap_or_else(|e| {
                eprintln!("zcobc: ZEUS_DD_CONFIG: {e}");
                zeus_ds::dd::DdTable::default()
            }),
            debug: std::env::var("ZEUS_DEBUG")
                .is_ok_and(|v| matches!(v.to_ascii_uppercase().as_str(), "Y" | "YES" | "ON" | "1")),
            last_status: *b"00",
            last_open_mode: None,
            call_exception: false,
            call_value: None,
            overflow: false,
            stdout: Box::new(std::io::BufWriter::new(std::io::stdout())),
            stdin_lines: None,
            sort_records: Vec::new(),
            active_sort_controls: None,
            sort_return: None,
            return_code: 0,
            linker: None,
            linker_used: false,
            control_blocks: false,
            main_args: None,
        }
    }

    fn alloc(&mut self, bytes: &[u8]) -> usize {
        let base = self.mem.len().next_multiple_of(8);
        self.mem.resize(base, 0);
        self.mem.extend_from_slice(bytes);
        base
    }

    fn load_hlasm(&mut self, name: &str) -> Result<bool, String> {
        let key = name.to_ascii_uppercase();
        if self.hlasm.contains_key(&key) {
            return Ok(true);
        }
        for dir in self.search_dirs.clone() {
            for filename in [
                format!("{key}.zo"),
                format!("{key}.obj"),
                format!("{key}.goff"),
                key.clone(),
                format!("{}.zo", key.to_ascii_lowercase()),
                format!("{}.obj", key.to_ascii_lowercase()),
                format!("{}.goff", key.to_ascii_lowercase()),
            ] {
                let path = dir.join(filename);
                let Ok(bytes) = std::fs::read(&path) else {
                    continue;
                };
                if !crate::hlasm::is_module(&bytes) {
                    continue;
                }
                let module = crate::hlasm::load_module(&bytes)?;
                let module = crate::hlasm::link_with_libraries(vec![module], &self.search_dirs)?;
                self.install_hlasm(&key, module)?;
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn install_hlasm(&mut self, key: &str, module: crate::hlasm::Module) -> Result<(), String> {
        let base = self.hlasm_storage.module(&mut self.mem, &module)?;
        let entry = if key == module.name {
            module.entry
        } else {
            module.entries.get(key).copied().unwrap_or(module.entry)
        };
        let module = Rc::new(module);
        for (alias, address) in &module.entries {
            let alias_entry = if alias == &module.name {
                module.entry
            } else {
                *address
            };
            self.hlasm
                .entry(alias.clone())
                .or_insert_with(|| (Rc::clone(&module), base, alias_entry));
        }
        self.hlasm.insert(key.to_string(), (module, base, entry));
        Ok(())
    }

    /// A loaded HLASM module takes precedence over a Rust replacement.
    pub fn call_hlasm(
        &mut self,
        name: &str,
        args: &[Option<usize>],
    ) -> Option<Result<i32, String>> {
        self.call_hlasm_mode(
            name,
            args,
            true,
            true,
            assembler::Invocation {
                parameter_mode: None,
                allow_64: true,
                floating: None,
            },
        )
    }

    fn call_hlasm_mode(
        &mut self,
        name: &str,
        args: &[Option<usize>],
        terminated: bool,
        pin: bool,
        invocation: assembler::Invocation<'_>,
    ) -> Option<Result<i32, String>> {
        match self.load_hlasm(name) {
            Ok(false) => None,
            Err(e) => Some(Err(e)),
            Ok(true) => {
                if self.hlasm_call_depth >= 128 {
                    return Some(Err("HLASM nested call limit exceeded".into()));
                }
                let (module, base, entry) = self.hlasm[&name.to_ascii_uppercase()].clone();
                if module.entry_mode(entry) == 64 && !invocation.allow_64 {
                    let cleanup = self.discard_hlasm_if_unused(base);
                    return Some(cleanup.and(Err(
                        "LINKX/XCTLX requires AMODE64OK=YES for an AMODE 64 target".into(),
                    )));
                }
                if pin {
                    self.hlasm_residency.pinned.insert(base);
                }
                *self.hlasm_residency.active.entry(base).or_default() += 1;
                let mut memory = std::mem::take(&mut self.mem);
                let mut storage = std::mem::take(&mut self.hlasm_storage);
                self.hlasm_call_depth += 1;
                let frame = self.enter_language_frame();
                let result = crate::hlasm::execute_entry_with_host(
                    &module,
                    &mut memory,
                    base,
                    args,
                    entry,
                    &mut storage,
                    self,
                    terminated,
                    invocation.parameter_mode,
                    None,
                    invocation.floating,
                );
                self.leave_language_frame(frame);
                self.hlasm_call_depth -= 1;
                self.mem = memory;
                self.hlasm_storage = storage;
                let cleanup = self.leave_hlasm(base);
                Some(result.and_then(|rc| cleanup.map(|_| rc)))
            }
        }
    }

    /// JCL/main entry: R1 addresses a list whose first entry points to the
    /// halfword-length-prefixed EXEC PARM, unless explicit arguments were set.
    pub fn run_hlasm_main(&mut self, module: crate::hlasm::Module) -> Result<i32, String> {
        let module = crate::hlasm::link_with_libraries(vec![module], &self.search_dirs)?;
        let exec_parameters = self.main_args.is_none();
        let args = if let Some(args) = self.main_args.take() {
            args
        } else {
            let text = crate::ebcdic::encode(&self.step.parm);
            let len = u16::try_from(text.len()).map_err(|_| "EXEC PARM exceeds 65535 bytes")?;
            let mut data = len.to_be_bytes().to_vec();
            data.extend(text);
            vec![Some(self.alloc(&data))]
        };
        let name = module.name.clone();
        self.install_hlasm(&name, module)?;
        let result = self
            .call_hlasm_mode(
                &name,
                &args,
                true,
                true,
                assembler::Invocation {
                    parameter_mode: exec_parameters.then_some(31),
                    allow_64: true,
                    floating: None,
                },
            )
            .ok_or_else(|| "HLASM main not loaded".to_string())?;
        let result = match result {
            Err(error) if error == STOP_RUN => Ok(self.return_code),
            result => result,
        };
        let cleanup = self.finish_assembler_io();
        result.and_then(|rc| cleanup.map(|_| rc))
    }

    /// Creates (or returns) the instance of program `index` of `module`.
    fn instantiate(&mut self, module: &Rc<Module>, index: u32) -> usize {
        let code = Rc::new(module.programs[index as usize].clone());
        if let Some(i) = self.instances.iter().position(|i| {
            !i.retired
                && i.activation_of.is_none()
                && Rc::ptr_eq(&i.module, module)
                && i.module_index == index
        }) {
            return i;
        }
        if code.parent.is_none()
            && let Some(&i) = self.by_name.get(&code.name)
        {
            return i;
        }
        let static_base = self.program_storage(&code.static_init);
        let mut ext_bases = Vec::new();
        for e in &code.externals {
            let a = match self.externals.get(&e.name) {
                Some(&(a, len)) => {
                    if len != e.len {
                        self.compatibility_error =
                            Some(format!("EXTERNAL {} storage lengths must agree", e.name));
                    }
                    a
                }
                None => {
                    let a = self.alloc(&vec![0x40; e.len as usize]);
                    self.externals.insert(e.name.clone(), (a, e.len));
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
                        let s = self.program_file_slot();
                        self.ext_files.insert(def.name.clone(), s);
                        s
                    }
                }
            } else {
                self.program_file_slot()
            };
            files.push(slot);
        }
        let invocation_bases = self.allocate_invocation_storage(&code);
        let inst = Instance {
            invocation_bases,
            dll_linked: false,
            retired: false,
            dynamic_calls: Default::default(),
            static_calls: HashMap::new(),
            bindings_ready: false,
            rerun_counts: vec![0; code.rerun.len()],
            numeric_context: (0, false),
            wide_temps: vec![None; code.temps as usize],
            entry_pc: 0,
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
            local_base: None,
            activation_of: None,
            code,
        };
        let i = if let Some(i) = self.instances.iter().position(|i| i.retired) {
            self.instances[i] = inst;
            i
        } else {
            self.instances.push(inst);
            self.instances.len() - 1
        };
        if self.instances[i].code.parent.is_none() {
            self.by_name.insert(self.instances[i].code.name.clone(), i);
            for entry in &self.instances[i].code.entries {
                self.by_name.insert(entry.name.clone(), i);
            }
        }
        if let Err(error) = self.initialize_dynamic(i, false) {
            self.compatibility_error = Some(error.0);
        }
        if let Err(error) = self.validate_rerun_allocations(i) {
            self.compatibility_error = Some(error.0);
        }
        i
    }

    fn initialize_dynamic(&mut self, inst: usize, local_only: bool) -> RResult<()> {
        let code = self.instances[inst].code.clone();
        for (loc, bytes) in &code.dynamic_init {
            let desc = &code.descs[loc.desc as usize];
            if local_only
                && !code
                    .local_storage
                    .is_some_and(|(start, len)| desc.offset >= start && desc.offset < start + len)
            {
                continue;
            }
            let mut locations = vec![loc.clone()];
            for dim in &desc.dims {
                let count = match &dim.count {
                    Some(e) => self.eval_ex(inst, e)?.coef.max(0).min(dim.max as i128) as u32,
                    None => dim.max,
                };
                let mut expanded = Vec::new();
                for location in locations {
                    for n in 1..=count {
                        let mut next = location.clone();
                        next.subs.push(zir::Ex::Lit(Dec::from_i64(n as i64)));
                        expanded.push(next);
                    }
                }
                locations = expanded;
            }
            for location in locations {
                let (address, len) = self.addr(inst, &location)?;
                let len = len.min(bytes.len());
                self.mem[address..address + len].copy_from_slice(&bytes[..len]);
            }
        }
        Ok(())
    }

    /// Resets a program to its initial state (INITIAL programs, CANCEL).
    fn reset(&mut self, i: usize) {
        self.reset_invocation_storage(i);
        let base = self.instances[i].static_base;
        let init = self.instances[i].code.static_init.clone();
        self.mem[base..base + init.len()].copy_from_slice(&init);
        let inst = &mut self.instances[i];
        inst.alter = inst.code.alter_init.clone();
        inst.segment = 0;
        inst.temps.iter_mut().for_each(|t| *t = Dec::ZERO);
        inst.wide_temps.fill(None);
        inst.numeric_context = (0, false);
        inst.rerun_counts.fill(0);
        if let Err(error) = self.initialize_dynamic(i, false) {
            self.compatibility_error = Some(error.0);
        }
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

    pub fn opened_buffer_counts(&self) -> &[(String, u32, u32)] {
        &self.opened_buffers
    }

    /// Allocates a DD name with the data set's attributes.
    pub fn allocate(&mut self, name: &str, a: zeus_ds::dd::Allocation) {
        self.dd.allocate(name, a);
    }
    pub fn set_dd_io(&mut self, name: &str, attributes: zeus_ds::dd::IoAttributes) {
        self.dd.set_io(name, attributes);
    }

    /// Catalog used by DSN(...) allocation environment variables.
    pub fn set_dynamic_catalog(&mut self, root: PathBuf) {
        self.dd.set_dynamic_catalog(root);
    }

    /// Set a run-unit-local allocation environment value without changing the
    /// process environment shared with other run units.
    pub fn set_file_environment(&mut self, name: &str, value: String) {
        self.dd.set_file_environment(name, value);
    }

    /// Input of ACCEPT (the SYSIN data of the step).
    pub fn set_stdin(&mut self, lines: Vec<String>) {
        self.stdin_lines = None;
        self.stdin_buffer = Some(lines.into());
    }

    /// Makes `name` callable as a host implemented program; programs
    /// found in the search directories take precedence.
    pub fn add_native(&mut self, name: &str, f: NativeFn) {
        self.natives.insert(name.to_ascii_uppercase(), f);
    }

    /// Makes `name` callable as a host implemented program that gets the
    /// machine.
    pub fn add_native_machine(&mut self, name: &str, f: NativeMachFn) {
        self.natives_mach.insert(name.to_ascii_uppercase(), f);
    }

    /// Makes `name` callable from other languages (PL/I) as a host
    /// implemented program that sees its arguments' lengths.
    pub fn add_native_ref(&mut self, name: &str, f: NativeRefFn) {
        self.natives_ref.insert(name.to_ascii_uppercase(), f);
    }

    /// Calls host implemented program `name` for another language's
    /// program; None when there is no such program.
    pub fn call_native(
        &mut self,
        name: &str,
        args: &[crate::interlang::XRef],
    ) -> Option<Result<i32, String>> {
        let name = name.to_ascii_uppercase();
        let addrs: Vec<_> = args.iter().map(|a| a.addr).collect();
        if le::parameter_count(&name).is_some() {
            return Some(self.le_service(&name, &addrs).map(|_| 0).map_err(|e| e.0));
        }
        if let Some(result) = self.call_hlasm(&name, &addrs) {
            return Some(result);
        }
        if let Some(f) = self.natives_ref.get(&name).copied() {
            let frame = self.enter_language_frame();
            let result = f(&mut self.mem, args);
            self.leave_language_frame(frame);
            return Some(result);
        }
        if let Some(f) = self.natives_mach.get(&name).copied() {
            let addrs: Vec<Option<usize>> = args.iter().map(|a| a.addr).collect();
            let frame = self.enter_language_frame();
            let result = f(self, &addrs);
            self.leave_language_frame(frame);
            return Some(result);
        }
        let f = self.natives.get(&name).copied()?;
        let addrs: Vec<Option<usize>> = args.iter().map(|a| a.addr).collect();
        let frame = self.enter_language_frame();
        let result = f(&mut self.mem, &addrs);
        self.leave_language_frame(frame);
        Some(result)
    }

    /// Calls program `name`: a COBOL program of the run unit, else one
    /// the linker finds (a PL/I module), with this machine's storage
    /// lent to it; None when there is no such program.
    pub fn call_external(
        &mut self,
        name: &str,
        args: &[crate::interlang::XRef],
    ) -> Result<Option<crate::interlang::XEnd>, String> {
        if let Some(e) = self.call_program(name, args)? {
            return Ok(Some(e));
        }
        let Some(l) = self.linker.clone() else {
            return Ok(None);
        };
        self.linker_used = true;
        let _ = self.stdout.flush();
        let frame = self.enter_language_frame();
        let me = std::mem::replace(self, Machine::placeholder());
        let (back, r) = l.call(&name.to_ascii_uppercase(), me, args);
        *self = back;
        self.leave_language_frame(frame);
        r.transpose()
    }

    /// Adds the programs of a module to the run unit (programs found by
    /// name before the search directories).
    pub fn add_module(&mut self, module: Rc<Module>) {
        for i in 0..module.programs.len() {
            self.instantiate(&module, i as u32);
        }
    }

    /// Passes `args` (addresses in this machine's storage, see
    /// [`Machine::getmain`]) to the USING parameters of the main program
    /// in place of the PARM.
    pub fn set_main_args(&mut self, args: Vec<Option<usize>>) {
        self.main_args = Some(args);
    }

    /// The name and the number of instructions of program instance
    /// `inst` (for its machine code).
    pub fn program_shape(&self, inst: usize) -> (&str, usize) {
        let code = &self.instances[inst].code;
        (&code.name, code.code.len())
    }

    /// The run unit's storage and the static storage address of program
    /// instance `inst` in it (for machine code that accesses data
    /// directly). The storage moves when it grows.
    pub fn native_memory(&mut self, inst: usize) -> (*mut u8, usize) {
        (self.mem.as_mut_ptr(), self.instances[inst].static_base)
    }

    /// Runs the programs that have machine code with it.
    pub fn set_native(&mut self, n: Rc<dyn NativeCode>) {
        self.native = Some(n);
    }

    /// Finds the programs that are not COBOL programs of the search
    /// directories.
    pub fn set_linker(&mut self, l: Rc<dyn crate::interlang::Linker>) {
        self.linker = Some(l);
    }

    /// Calls a separately compiled program from another run time: the
    /// arguments are placed in this machine's storage for the call and
    /// copied back.
    pub fn call_program(
        &mut self,
        name: &str,
        args: &[crate::interlang::XRef],
    ) -> Result<Option<crate::interlang::XEnd>, String> {
        if !self.control_blocks {
            self.build_control_blocks();
        }
        let name = name.to_ascii_uppercase();
        let Some(callee) = self.find_program(&name) else {
            return Ok(None);
        };
        self.bind_static_calls(callee);
        let callee = self.activation(callee).map_err(|e| e.0)?;
        let ci = &mut self.instances[callee];
        if ci.code.initial || !ci.initialized {
            let first = !ci.initialized;
            ci.initialized = true;
            if !first {
                self.reset(callee);
            }
        }
        self.reset_local_storage(callee);
        let (entry_pc, using, _) = self.entry_binding(callee, &name);
        let ci = &mut self.instances[callee];
        ci.entry_pc = entry_pc;
        ci.linkage.fill(None);
        for (k, slot) in using.iter().enumerate() {
            ci.linkage[*slot as usize] = args.get(k).and_then(|a| a.addr);
        }
        ci.active = true;
        ci.perform.clear();
        let result = self.bind_program_result(callee).map_err(|e| e.0)?;
        self.return_code = 0;
        let r = self.run_from(callee, true);
        self.instances[callee].active = false;
        let _ = self.stdout.flush();
        let stop = match r {
            Ok(Exit::Returned) => false,
            Ok(Exit::StopRun(_)) => true,
            Err(e) if e.0 == STOP_RUN => true,
            Err(e) => return Err(e.0),
        };
        Ok(Some(crate::interlang::XEnd {
            rc: self.return_code,
            ret: result
                .map(|loc| {
                    let (addr, len) = self.addr(callee, &loc)?;
                    Ok(self.mem[addr..addr + len].to_vec())
                })
                .transpose()
                .map_err(|e: RunError| e.0)?,
            stop,
        }))
    }

    /// A machine with no programs and no storage, standing in for one
    /// that is lent to another run time.
    pub fn placeholder() -> Machine {
        Machine::new(Vec::new())
    }

    /// The interface that runs EXEC statements.
    /// Sets the function called now and then while programs run.
    pub fn set_tick(&mut self, f: fn()) {
        self.tick = Some(f);
    }

    /// Counts an instruction (or a statement of another run time): every
    /// few thousand, the function of [`Machine::set_tick`] is called.
    #[inline]
    pub fn count_tick(&mut self) {
        if let Some(f) = self.tick {
            self.ticks = self.ticks.wrapping_add(1);
            if self.ticks.is_multiple_of(4096) {
                f();
            }
        }
    }

    pub fn exec_host(&self) -> Option<Rc<dyn exec::ExecHost>> {
        self.exec_host.clone()
    }

    /// The linker of the run unit (PL/I modules).
    pub fn linker(&self) -> Option<Rc<dyn crate::interlang::Linker>> {
        self.linker.clone()
    }

    /// Writes the output of the programs (DISPLAY) so far.
    pub fn flush(&mut self) {
        let _ = self.stdout.flush();
    }

    /// Closes the files of all programs (the end of the run unit).
    pub fn finish(&mut self) {
        let _ = self.try_finish();
    }

    /// Finish a host-driven run unit and report CLOSE/deallocation failures.
    pub fn try_finish(&mut self) -> RResult<()> {
        self.flush_coverage();
        let files = self.close_all();
        let assembler = self.finish_assembler_io().map_err(RunError);
        let output = self
            .stdout
            .flush()
            .map_err(|error| RunError(error.to_string()));
        files.and(assembler).and(output)
    }

    /// Sets the job step identification shown to programs (TIOT).
    pub fn set_step(&mut self, step: StepInfo) {
        self.step = step;
    }

    /// COBOL 4.2 CHECK(ON/OFF); only programs compiled with SSRANGE check.
    pub fn set_range_checking(&mut self, enabled: bool) {
        self.range_checking = enabled;
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
        self.control_blocks = true;
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
        let ccsid = crate::codepage::runtime_ccsid(
            module
                .programs
                .first()
                .and_then(|p| p.ccsid)
                .unwrap_or(module.ccsid),
        );
        if crate::codepage::get(ccsid).is_none() {
            return err("unsupported program CODEPAGE CCSID");
        }
        let _codepage = crate::codepage::scope(Some(ccsid));
        if module.programs.first().is_some_and(|p| p.result.is_some()) {
            return err("a program with PROCEDURE RETURNING must be called as a subprogram");
        }
        self.random = 0x0005_DEEC_E66D;
        self.build_control_blocks();
        self.control_blocks = true;
        let module = Rc::new(module);
        let main = self.instantiate(&module, 0);
        self.bind_static_calls(main);
        if let Some(args) = self.main_args.take() {
            let using = self.instances[main].code.using.clone();
            for (k, slot) in using.iter().enumerate() {
                self.instances[main].linkage[*slot as usize] = args.get(k).copied().flatten();
            }
        } else if let Some(&slot) = self.instances[main].code.using.first() {
            // PARM: the first USING parameter of the main program
            let text = ebcdic::encode(&self.step.parm);
            let mut area = (text.len() as u16).to_be_bytes().to_vec();
            area.extend(text);
            area.resize(area.len().max(102), ebcdic::SPACE);
            let a = self.alloc(&area);
            self.instances[main].linkage[slot as usize] = Some(a);
        }
        let r = self.run(main);
        let files = self.close_all();
        let cleanup = files.and(self.finish_assembler_io().map_err(RunError));
        let _ = self.stdout.flush();
        let result = match r {
            Ok(Exit::StopRun(code)) => Ok(code),
            Ok(Exit::Returned) => Ok(self.return_code_value()),
            Err(e) if e.0 == STOP_RUN => Ok(self.return_code_value()),
            Err(e) => Err(e),
        };
        result.and_then(|rc| cleanup.map(|_| rc))
    }

    fn close_all(&mut self) -> RResult<()> {
        let mut failure = None;
        for i in 0..self.instances.len() {
            if self.instances[i].retired {
                continue;
            }
            for f in 0..self.instances[i].files.len() {
                if self.state(i, f as u32).open_mode().is_some()
                    && let Err(error) = self.close_file(i, f as u32, false)
                    && failure.is_none()
                {
                    failure = Some(error);
                }
            }
        }
        if let Err(error) = self.dd.clear_dynamic()
            && failure.is_none()
        {
            failure = Some(RunError(error.to_string()));
        }
        failure.map_or(Ok(()), Err)
    }

    fn return_code_value(&self) -> i32 {
        self.return_code
    }

    /// The return code of the last program run (RETURN-CODE).
    pub fn return_code(&self) -> i32 {
        self.return_code
    }

    /// The RETURN-CODE special register of an instance.
    fn rc_loc(&self, inst: usize) -> Option<zir::Loc> {
        self.instances[inst].code.return_code.map(|desc| zir::Loc {
            desc,
            subs: Vec::new(),
            refmod: None,
        })
    }

    /// Takes the return code from the RETURN-CODE register of `inst`.
    fn load_rc(&mut self, inst: usize) {
        if let Some(loc) = self.rc_loc(inst)
            && let Ok((a, l)) = self.addr(inst, &loc)
        {
            let v = binary::read(&self.mem[a..a + l], true);
            self.return_code = v as i32;
        }
    }

    /// Sets the RETURN-CODE register of `inst` to the return code.
    fn store_rc(&mut self, inst: usize) {
        if let Some(loc) = self.rc_loc(inst)
            && let Ok((a, l)) = self.addr(inst, &loc)
        {
            binary::write(&mut self.mem[a..a + l], i128::from(self.return_code));
        }
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
        interface: Option<zir::CallInterface>,
        handled: bool,
    ) -> RResult<Option<usize>> {
        let v = self.eval_src(cur, program)?;
        let mut pointer_interface = None;
        let name = if let eval::Val::Item(ty, bytes) = &v
            && ty.is_entry_pointer()
        {
            let address = binary::read(bytes, false) as u64;
            pointer_interface = self.entry_interfaces.get(&address).copied();
            self.entry_pointers.get(&address).cloned().ok_or_else(|| {
                RunError("CALL through a null or invalid procedure/function-pointer".into())
            })?
        } else {
            let name_bytes = self.alnum_bytes(cur, &v, 0)?;
            ebcdic::decode(&name_bytes).trim().to_ascii_uppercase()
        };
        let interface = pointer_interface.or(interface);
        self.call_exception = false;
        self.call_value = None;
        if le::parameter_count(&name).is_some() {
            let addrs = self.call_args(cur, args)?;
            self.le_service(&name, &addrs)?;
            return Ok(None);
        }
        let static_call = matches!(program, zir::Src::Alnum(_))
            && interface.unwrap_or(zir::CallInterface::default_for(
                self.instances[cur].code.dynam,
                self.instances[cur].code.dll,
            )) == zir::CallInterface::Static;
        let caller = self.instances[cur].activation_of.unwrap_or(cur);
        let dll_call = interface.unwrap_or(zir::CallInterface::default_for(
            self.instances[cur].code.dynam,
            self.instances[cur].code.dll,
        )) == zir::CallInterface::Dll;
        let target = if static_call {
            self.instances[caller].static_calls.get(&name).copied()
        } else {
            None
        };
        let resolved = target.or_else(|| {
            if dll_call {
                self.resolve_dll(
                    cur,
                    &name,
                    matches!(program, zir::Src::Alnum(_)) || pointer_interface.is_some(),
                )
            } else {
                self.resolve_program(cur, &name)
            }
        });
        if dll_call && resolved.is_none() {
            if handled {
                self.call_exception = true;
                return Ok(None);
            }
            return err(format!("DLL export {name} not found"));
        }
        let Some(callee) = resolved else {
            if self.load_hlasm(&name).map_err(RunError)? {
                let addrs = self.call_args(cur, args)?;
                let result = self.call_hlasm(&name, &addrs).expect("loaded HLASM module");
                self.return_code = result.map_err(RunError)?;
                self.store_rc(cur);
                return Ok(None);
            }
            if let Some(f) = self.natives_mach.get(&name).copied() {
                let addrs = self.call_args(cur, args)?;
                let frame = self.enter_language_frame();
                let result = f(self, &addrs);
                self.leave_language_frame(frame);
                let rc = result.map_err(RunError)?;
                self.return_code = rc;
                self.store_rc(cur);
                return Ok(None);
            }
            if let Some(f) = self.natives.get(&name).copied() {
                let addrs = self.call_args(cur, args)?;
                let frame = self.enter_language_frame();
                let result = f(&mut self.mem, &addrs);
                self.leave_language_frame(frame);
                self.return_code = result.map_err(RunError)?;
                self.store_rc(cur);
                return Ok(None);
            }
            if let Some(l) = self.linker.clone() {
                self.linker_used = true;
                let refs: Vec<crate::interlang::XRef> = self
                    .call_args_sized(cur, args)?
                    .iter()
                    .map(|s| crate::interlang::XRef {
                        addr: s.map(|x| x.0),
                        len: s.map_or(0, |x| x.1),
                        ..Default::default()
                    })
                    .collect();
                let _ = self.stdout.flush();
                // the run unit's storage (this machine) is lent to the callee
                let frame = self.enter_language_frame();
                let me = std::mem::replace(self, Machine::placeholder());
                let (back, r) = l.call(&name, me, &refs);
                *self = back;
                self.leave_language_frame(frame);
                if let Some(r) = r {
                    let end = r.map_err(RunError)?;
                    self.return_code = end.rc;
                    self.call_value = end.ret;
                    self.store_rc(cur);
                    if end.stop {
                        return err(STOP_RUN);
                    }
                    return Ok(None);
                }
            }
            if !handled {
                return err(format!("program {name} not found"));
            }
            self.call_exception = true;
            return Ok(None);
        };
        if dll_call && self.instances[callee].code.lp64 != self.instances[cur].code.lp64 {
            return err("DLL caller and target must have the same LP address mode");
        }
        if dll_call {
            self.instances[callee].dll_linked = true;
        }
        if static_call || dll_call {
            self.instances[caller]
                .static_calls
                .insert(name.clone(), callee);
        } else {
            self.instances[callee].dynamic_calls.insert(name.clone());
        }
        self.bind_static_calls(callee);
        let sized = self.call_args_sized(cur, args)?;
        let addrs: Vec<_> = sized
            .iter()
            .map(|a| a.map(|(address, _)| address))
            .collect();
        let callee = self.activation(callee)?;
        let ci = &mut self.instances[callee];
        if ci.code.initial || !ci.initialized {
            let first = !ci.initialized;
            ci.initialized = true;
            if !first {
                self.reset(callee);
            }
        }
        self.reset_local_storage(callee);
        let (entry_pc, using, by_value) = self.entry_binding(callee, &name);
        for (index, &value) in by_value.iter().enumerate() {
            let Some(arg) = args.get(index) else {
                continue;
            };
            if matches!(arg, zir::CallArgMode::Omitted) {
                continue;
            }
            let actual_value = matches!(
                arg,
                zir::CallArgMode::Value(_) | zir::CallArgMode::ValueData(_)
            );
            if value != actual_value {
                return err("CALL and formal USING BY VALUE/REFERENCE modes must agree");
            }
            if value {
                let slot = using[index];
                let code = &self.instances[callee].code;
                let formal = code
                    .descs
                    .iter()
                    .find(|d| d.base == zir::Base::Linkage(slot) && d.offset == 0)
                    .unwrap();
                if sized[index].is_some_and(|(_, len)| len != formal.len as usize) {
                    return err(
                        "CALL BY VALUE argument and formal parameter storage widths must agree",
                    );
                }
            }
        }
        let ci = &mut self.instances[callee];
        ci.entry_pc = entry_pc;
        ci.linkage.fill(None);
        for (k, slot) in using.iter().enumerate() {
            ci.linkage[*slot as usize] = addrs.get(k).copied().flatten();
        }
        ci.active = true;
        ci.perform.clear();
        Ok(Some(callee))
    }

    /// Nonrecursive invocations can reuse their LOCAL-STORAGE addresses,
    /// but must restore the initial values independently of WORKING-STORAGE.
    fn reset_local_storage(&mut self, inst: usize) {
        self.reset_invocation_storage(inst);
        let ci = &self.instances[inst];
        if let Some((offset, len)) = ci.code.local_storage {
            let start = offset as usize;
            let end = start + len as usize;
            let base = ci.local_base.unwrap_or(ci.static_base + start);
            self.mem[base..base + len as usize].copy_from_slice(&ci.code.static_init[start..end]);
        }
        if let Err(error) = self.initialize_dynamic(inst, true) {
            self.compatibility_error = Some(error.0);
        }
    }

    fn reset_invocation_storage(&mut self, inst: usize) {
        let code = self.instances[inst].code.clone();
        for (&desc, &address) in &self.instances[inst].invocation_bases {
            let d = &code.descs[desc as usize];
            self.mem[address..address + d.len as usize]
                .copy_from_slice(&code.static_init[d.offset as usize..(d.offset + d.len) as usize]);
        }
    }

    /// Allocate or reuse an inactive invocation without copying shared WS.
    fn activation(&mut self, root: usize) -> RResult<usize> {
        if self.threaded_run_unit && !self.instances[root].code.thread {
            return err(format!(
                "threaded run unit requires THREAD for {}",
                self.instances[root].code.name
            ));
        }
        if let Some(error) = self.compatibility_error.take() {
            return err(error);
        }
        if !self.instances[root].active {
            return Ok(root);
        }
        if !self.instances[root].code.recursive {
            return err(format!(
                "recursive CALL of active program {}",
                self.instances[root].code.name
            ));
        }
        if self.instances.iter().filter(|i| i.active).count() >= 1000 {
            return err("CALL nesting too deep");
        }
        if let Some(i) = self
            .instances
            .iter()
            .position(|i| !i.retired && i.activation_of == Some(root) && !i.active)
        {
            return Ok(i);
        }
        let mut instance = self.instances[root].clone();
        instance.invocation_bases = self.allocate_invocation_storage(&instance.code);
        instance.active = false;
        instance.activation_of = Some(root);
        instance.local_base = instance.code.local_storage.map(|(offset, len)| {
            self.alloc(&instance.code.static_init[offset as usize..(offset + len) as usize])
        });
        instance.perform.clear();
        instance.linkage.fill(None);
        instance.temps.fill(Dec::ZERO);
        instance.wide_temps.fill(None);
        instance.numeric_context = (0, false);
        let index = self.instances.len();
        self.instances.push(instance);
        Ok(index)
    }

    /// Executes from the entry of instance `main` until STOP RUN.
    fn run(&mut self, main: usize) -> RResult<Exit> {
        if let Some(error) = self.compatibility_error.take() {
            return err(error);
        }
        self.instances[main].active = true;
        self.instances[main].initialized = true;
        self.run_from(main, false)
    }

    /// Runs instance `start` from its entry. A `called` program returns
    /// (Exit::Returned) at EXIT PROGRAM / GOBACK.
    fn run_from(&mut self, start: usize, called: bool) -> RResult<Exit> {
        let mut s = RunState {
            stack: Vec::new(),
            cur: start,
            pc: self.instances[start].entry_pc,
            called,
        };
        let frame = self.enter_language_frame();
        let result = self.run_loop(&mut s);
        self.leave_language_frame(frame);
        result
    }

    fn run_loop(&mut self, s: &mut RunState) -> RResult<Exit> {
        loop {
            // the program's machine code, when it has some
            if let Some(n) = self.native.clone()
                && !self.instances[s.cur].code.recursive
                && !self.dynamic_object(s.cur)
                && let Some(r) = n.run(self, s)
            {
                if let Some(e) = r? {
                    return Ok(e);
                }
                continue;
            }
            if let Some(e) = self.exec_step(s)? {
                return Ok(e);
            }
        }
    }

    /// Runs the instruction at `s.pc` of program instance `s.cur`; the
    /// end of the run unit's main program or of a called one.
    pub fn exec_step(&mut self, s: &mut RunState) -> RResult<Option<Exit>> {
        let ccsid = crate::codepage::runtime_ccsid(self.program_ccsid(s.cur));
        if crate::codepage::get(ccsid).is_none() {
            return err("unsupported program CODEPAGE CCSID");
        }
        let _codepage = crate::codepage::scope(Some(ccsid));
        if self.sym.is_some() {
            return self.sym_step(s);
        }
        self.exec_counted(s)
    }

    /// Runs an instruction, counting it for coverage.
    fn exec_counted(&mut self, s: &mut RunState) -> RResult<Option<Exit>> {
        if self.cov.is_none() {
            let result = self.exec_instr(s)?;
            self.write_pending_checkpoints(s)?;
            return Ok(result);
        }
        let (cur, pc) = (s.cur, s.pc);
        let r = self.exec_instr(s);
        let jumped = s.cur == cur && s.pc != pc + 1;
        self.cov_count(cur, pc, jumped);
        if r.is_ok() {
            self.write_pending_checkpoints(s)?;
        }
        r
    }

    /// Counts an execution of instruction `pc` of instance `cur` (and
    /// whether a conditional jump there went to its target).
    fn cov_count(&mut self, cur: usize, pc: u32, jumped: bool) {
        let Some(cov) = self.cov.as_mut() else {
            return;
        };
        let inst = &self.instances[cur];
        let c = cov
            .entry(Rc::as_ptr(&inst.code))
            .or_insert_with(|| CovCounts {
                code: inst.code.clone(),
                module: inst.module.clone(),
                hits: vec![0; inst.code.code.len()],
                jumped: vec![0; inst.code.code.len()],
            });
        let pc = pc as usize;
        if pc < c.hits.len() {
            c.hits[pc] += 1;
            if jumped
                && matches!(
                    inst.code.code[pc],
                    Instr::JumpIf(..) | Instr::JumpUnless(..)
                )
            {
                c.jumped[pc] += 1;
            }
        }
    }

    /// Counts a conditional jump to its target (the instruction itself
    /// was counted).
    fn cov_jumped(&mut self, cur: usize, pc: u32, jumped: bool) {
        if jumped
            && let Some(c) = self
                .cov
                .as_mut()
                .and_then(|m| m.get_mut(&Rc::as_ptr(&self.instances[cur].code)))
            && let Some(j) = c.jumped.get_mut(pc as usize)
        {
            *j += 1;
        }
    }

    fn exec_instr(&mut self, s: &mut RunState) -> RResult<Option<Exit>> {
        let cur = s.cur;
        let code = self.instances[cur].code.clone();
        let Some(ins) = code.code.get(s.pc as usize) else {
            return err(format!(
                "{}: control ran past the end of the program",
                code.name
            ));
        };
        s.pc += 1;
        if matches!(
            ins,
            Instr::Release(..) | Instr::Return(..) | Instr::SortOrder { .. } | Instr::SortWrite(_)
        ) && self.abort_sort_if_requested(s)?
        {
            return Ok(None);
        }
        let pc = s.pc;
        self.count_tick();
        let ins = match ins {
            Instr::Exec(n) => match self.exec(cur, &code, *n)? {
                exec::ExecOutcome::Continue => return Ok(None),
                exec::ExecOutcome::Goto(t) => {
                    s.pc = t;
                    return Ok(None);
                }
                exec::ExecOutcome::Return => &Instr::GoBack,
            },
            _ => ins,
        };
        match ins {
            Instr::Nop => {}
            Instr::MoreLabels => return err("MORE-LABELS outside an active label procedure"),
            Instr::NumericContext { decimals, floating } => {
                self.instances[cur].numeric_context = (*decimals, *floating)
            }
            Instr::Move(src, dst) => {
                let v = self.eval_src(cur, src)?;
                self.move_val(cur, &v, dst)?;
            }
            Instr::Store {
                value,
                dst,
                rounded,
                check,
            } => match self.numeric_store(cur, value, dst, *rounded, *check) {
                Ok(()) => {}
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
                self.numeric_div_rem(cur, dividend, divisor, quot, *rounded, rem, *check)?;
            }
            Instr::ClearSizeError => self.size_error = false,
            Instr::SetTemp(t, ex) => {
                self.numeric_temp(cur, *t, ex)?;
            }
            Instr::Jump(t) => s.pc = *t,
            Instr::JumpIf(c, t) => {
                if self.eval_cx(cur, c)? {
                    s.pc = *t;
                }
            }
            Instr::JumpUnless(c, t) => {
                if !self.eval_cx(cur, c)? {
                    s.pc = *t;
                }
            }
            Instr::Perform { start, end, ret } => {
                let inst = &mut self.instances[cur];
                if inst.perform.len() > 10_000 {
                    return err("PERFORM nesting too deep (recursive PERFORM?)");
                }
                inst.perform.push((*end, *ret));
                s.pc = *start;
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
                        let f = s.stack.pop().unwrap();
                        s.cur = f.inst;
                        s.pc = f.pc;
                    } else {
                        s.pc = ret;
                    }
                }
            }
            Instr::Segment(seg) => self.enter_segment(cur, *seg),
            Instr::SetAddress(n, ex) => self.set_address(cur, *n, ex)?,
            Instr::SetEntry(targets, name) => self.set_entry(cur, targets, name, None)?,
            Instr::SetEntryWithInterface(targets, name, mode) => {
                self.set_entry(cur, targets, name, Some(*mode))?
            }
            Instr::Initiate(r) => self.report_initiate(cur, *r)?,
            Instr::Generate { report, group } => self.report_generate(cur, *report, *group)?,
            Instr::Terminate(r) => self.report_terminate(cur, *r)?,
            Instr::SuppressPrinting => self.suppress_printing = true,
            Instr::DebugCause(c, line) => self.instances[cur].debug_cause = (*c, *line),
            Instr::DebugProc { .. } | Instr::DebugData { .. } | Instr::DebugFile { .. } => {
                if let Some((target, end)) = self.debug_fire(cur, ins)? {
                    self.instances[cur].perform.push((end, pc));
                    s.pc = target;
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
                        s.stack.push(Frame {
                            inst: cur,
                            pc,
                            result: None,
                        });
                        s.cur = inst;
                        self.instances[cur].perform.push((end, USE_RETURN));
                    }
                    s.pc = target;
                }
            }
            Instr::GoDepending(ex, targets) => {
                let v = self.eval_ex(cur, ex)?;
                let n = v
                    .rescale(0, zeus_decimal::Rounding::Truncate)
                    .map(|d| d.coef)
                    .unwrap_or(0);
                if n >= 1 && (n as usize) <= targets.len() {
                    s.pc = targets[n as usize - 1];
                }
            }
            Instr::AlterableGo(slot) => s.pc = self.instances[cur].alter[*slot as usize],
            Instr::Alter(slot, target) => self.instances[cur].alter[*slot as usize] = *target,
            Instr::StopRun => {
                self.load_rc(cur);
                return Ok(Some(Exit::StopRun(self.return_code_value())));
            }
            Instr::StopLiteral(src) => {
                let v = self.eval_src(cur, src)?;
                let bytes = self.display_bytes(cur, &v)?;
                let _ = self.stdout.write_all(&ebcdic::to_host(&bytes));
                let _ = self.stdout.write_all(b"\n");
                let _ = self.stdout.flush();
                // The run waits for the operator's reply; an interrupt
                // (ETX) cancels it.
                if self.stdin_line().is_some_and(|l| l.contains('\u{3}')) {
                    return Ok(Some(Exit::StopRun(self.return_code_value())));
                }
            }
            Instr::ExitProgram | Instr::GoBack => {
                let is_main = s.stack.is_empty();
                // RETURN-CODE goes back to the caller (or ends the run)
                self.load_rc(cur);
                if is_main && s.called {
                    self.instances[cur].active = false;
                    self.instances[cur].perform.clear();
                    if self.instances[cur].code.initial {
                        self.close_program_files(cur)?;
                        self.reset_initial_children(cur)?;
                    }
                    return Ok(Some(Exit::Returned));
                }
                if is_main {
                    if matches!(ins, Instr::GoBack) {
                        return Ok(Some(Exit::StopRun(self.return_code_value())));
                    }
                    // EXIT PROGRAM in a main program: continue
                    return Ok(None);
                }
                self.instances[cur].active = false;
                self.instances[cur].perform.clear();
                if self.instances[cur].code.initial {
                    self.close_program_files(cur)?;
                    self.reset_initial_children(cur)?;
                }
                self.le_conditions.frames.pop();
                let f = s.stack.pop().unwrap();
                s.cur = f.inst;
                s.pc = f.pc;
                if let Some(result) = f.result {
                    self.finish_result(s.cur, Some(cur), result)?;
                } else {
                    self.store_rc(s.cur);
                }
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
                let _ = self.stdout.write_all(&ebcdic::to_host(&out));
                if *advance {
                    let _ = self.stdout.write_all(b"\n");
                }
            }
            Instr::Accept { dst, from } => self.accept(cur, dst, *from)?,
            Instr::Open(f, mode) => self.open_file(cur, *f, *mode)?,
            Instr::OpenTape {
                file,
                mode,
                reversed,
                no_rewind,
            } => self.open_tape_options(cur, *file, *mode, *reversed, *no_rewind)?,
            Instr::CloseTape {
                file,
                no_rewind,
                removal,
            } => self.close_tape_options(cur, *file, *no_rewind, *removal)?,
            Instr::Close(f, lock) => self.close_file(cur, *f, *lock)?,
            Instr::CloseReel(f) => self.close_reel(cur, *f)?,
            Instr::Write {
                file,
                record,
                from,
                advance,
            } => {
                self.write_file(cur, *file, *record, from.as_ref(), advance.as_ref())?;
                self.rerun_record(cur, *file)?;
            }
            Instr::Rewrite { file, record, from } => {
                self.rewrite_file(cur, *file, *record, from.as_ref())?
            }
            Instr::Read {
                file,
                next,
                into,
                key,
            } => {
                self.read_file(cur, *file, *next, into.as_ref(), *key)?;
                self.rerun_record(cur, *file)?;
            }
            Instr::Delete(f) => self.delete_record(cur, *f)?,
            Instr::Start { file, rel, key } => self.start_file(cur, *file, *rel, key.as_ref())?,
            Instr::Call {
                interface,
                program,
                args,
                returning,
            } => {
                // ON EXCEPTION / ON OVERFLOW handles a missing program.
                let next = code.code.get(pc as usize);
                let handled = matches!(next, Some(Instr::JumpUnless(zir::Cx::CallException, _)));
                let saved_rc = self.rc_before_call(cur)?;
                let callee = self.prepare_call(cur, program, args, *interface, handled)?;
                let result = returning
                    .as_ref()
                    .map(|dst| self.pending_result(cur, callee, dst, saved_rc))
                    .transpose()?;
                if let Some(callee) = callee {
                    if returning.is_none() && self.instances[callee].code.result.is_some() {
                        self.bind_program_result(callee)?;
                    }
                    s.stack.push(Frame {
                        inst: cur,
                        pc,
                        result,
                    });
                    if s.stack.len() > 1000 {
                        return err("CALL nesting too deep");
                    }
                    self.enter_language_frame();
                    s.cur = callee;
                    s.pc = self.instances[callee].entry_pc;
                } else if let Some(result) = result {
                    self.finish_result(cur, None, result)?;
                }
            }
            Instr::Cancel(src) => {
                let v = self.eval_src(cur, src)?;
                let name_bytes = self.alnum_bytes(cur, &v, 0)?;
                let name = ebcdic::decode(&name_bytes).trim().to_ascii_uppercase();
                self.cancel_cobol(cur, &name)?;
                if let Some((module, base, _)) = self.hlasm.get(&name) {
                    let storage = module.storage_at(*base).map_err(RunError)?;
                    self.mem[*base..*base + module.storage.len()].copy_from_slice(&storage);
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
            Instr::SortBegin(file) => {
                if !self.active_sort_areas.is_empty() {
                    return err("nested SORT/MERGE is not permitted");
                }
                let peers = self.sort_area_peers(cur, *file)?;
                self.active_sort_areas.push(peers);
                self.sort_records.clear();
                self.sort_return = None;
                self.begin_sort_controls(cur, *file)?;
                let end = code
                    .code
                    .iter()
                    .enumerate()
                    .skip(s.pc as usize)
                    .find_map(|(i, ins)| matches!(ins, Instr::SortEnd).then_some(i as u32))
                    .ok_or_else(|| RunError("SORT has no matching end instruction".into()))?;
                self.active_sort_controls.as_mut().unwrap().continuation =
                    Some(sort::Continuation {
                        owner: cur,
                        end,
                        performs: self.instances[cur].perform.len(),
                        stack: s.stack.len(),
                        language_frames: self.le_conditions.frames.len(),
                    });
            }
            Instr::SortRead(file) => {
                self.sort_read_file(cur, *file)?;
                if self.abort_sort_if_requested(s)? {
                    return Ok(None);
                }
            }
            Instr::SortOrder {
                file,
                keys,
                collating,
                merge,
            } => {
                let spec = files::SortSpec {
                    file: *file,
                    keys: keys.clone(),
                    collating: collating.clone(),
                    merge: *merge,
                };
                let records = std::mem::take(&mut self.sort_records);
                self.active_sort_controls.as_mut().unwrap().input_phase = false;
                let sorted = self.sort_sort(cur, &spec, records)?;
                if self.abort_sort_if_requested(s)? {
                    return Ok(None);
                }
                self.sort_return = Some((sorted, 0));
            }
            Instr::SortWrite(file) => {
                let records = self
                    .sort_return
                    .as_ref()
                    .ok_or_else(|| RunError("SORT output unavailable".into()))?
                    .0
                    .clone();
                self.sort_write_file(cur, *file, &records)?;
            }
            Instr::SortCheckpoint => self.rerun_sort(cur)?,
            Instr::SortEnd => {
                self.sort_return = None;
                self.active_sort_controls = None;
                self.active_sort_areas.pop();
            }
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
                if !self.active_sort_areas.is_empty() {
                    return err("nested SORT/MERGE is not permitted");
                }
                let peers = self.sort_area_peers(cur, *file)?;
                self.active_sort_areas.push(peers);
                let result = (|| -> RResult<()> {
                    let sort = files::SortSpec {
                        file: *file,
                        keys: keys.clone(),
                        collating: collating.clone(),
                        merge: *merge,
                    };
                    // Input phase
                    self.sort_records.clear();
                    self.begin_sort_controls(cur, *file)?;
                    for &u in using {
                        self.sort_read_file(cur, u)?;
                        if self.sort_cancelled(cur)? {
                            return err(sort::CANCELLED);
                        }
                    }
                    if let Some((start, end)) = input {
                        self.sort_records.clear();
                        self.instances[cur].debug_cause = (zir::debug_cause::SORT_INPUT, 0);
                        self.run_procedure(cur, *start, *end, &mut s.stack)?;
                    }
                    let records = std::mem::take(&mut self.sort_records);
                    self.active_sort_controls.as_mut().unwrap().input_phase = false;
                    if self.sort_cancelled(cur)? {
                        return err(sort::CANCELLED);
                    }
                    let sorted = self.sort_sort(cur, &sort, records)?;
                    if self.sort_cancelled(cur)? {
                        return err(sort::CANCELLED);
                    }
                    if let Some((start, end)) = output {
                        self.sort_return = Some((sorted, 0));
                        self.instances[cur].debug_cause = if *merge {
                            (zir::debug_cause::MERGE_OUTPUT, 0)
                        } else {
                            (zir::debug_cause::SORT_OUTPUT, 0)
                        };
                        self.run_procedure(cur, *start, *end, &mut s.stack)?;
                        self.sort_return = None;
                    } else {
                        for &g in giving {
                            self.sort_write_file(cur, g, &sorted)?;
                        }
                    }
                    Ok(())
                })();
                self.active_sort_areas.pop();
                self.active_sort_controls = None;
                self.sort_return = None;
                if let Err(error) = result
                    && error.0 != sort::CANCELLED
                {
                    return Err(error);
                }
            }
            Instr::Release(f, from) => {
                if self.sort_cancelled(cur)? {
                    return err(sort::CANCELLED);
                }
                let rec = self.sort_record_bytes(cur, *f, from.as_ref())?;
                self.sort_records.push(rec);
            }
            Instr::Return(f, into) => self.sort_return_record(cur, *f, into.as_ref())?,
            Instr::Exec(_) => unreachable!(),
            Instr::Unsupported(name) => {
                return err(format!(
                    "{}: statement {name} is not supported yet",
                    code.name
                ));
            }
        }

        Ok(None)
    }

    /// Runs an input/output procedure (paragraphs `start`..=`end`) to
    /// completion within the current program.
    fn run_procedure(
        &mut self,
        cur: usize,
        start: u32,
        end: u32,
        stack: &mut Vec<Frame>,
    ) -> RResult<()> {
        self.procedure_depth += 1;
        let performs = self.instances[cur].perform.len();
        let result = self.run_procedure_inner(cur, start, end, stack);
        if result.is_err() {
            self.instances[cur].perform.truncate(performs);
        }
        self.procedure_depth -= 1;
        result
    }

    fn run_procedure_inner(
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
            self.cov_count(cur, pc, false);
            // Only a subset of control is needed inside procedures; reuse
            // the main loop by single-stepping.
            match ins {
                Instr::MoreLabels => {
                    self.instances[cur].perform.truncate(depth - 1);
                    return err("ZEUS_MORE_LABELS");
                }
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
                    let jump = self.eval_cx(cur, c)?;
                    self.cov_jumped(cur, pc, jump);
                    pc = if jump { *t } else { pc + 1 };
                    continue;
                }
                Instr::JumpUnless(c, t) => {
                    let jump = !self.eval_cx(cur, c)?;
                    self.cov_jumped(cur, pc, jump);
                    pc = if jump { *t } else { pc + 1 };
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
        let ccsid = crate::codepage::runtime_ccsid(self.program_ccsid(cur));
        if crate::codepage::get(ccsid).is_none() {
            return err("unsupported program CODEPAGE CCSID");
        }
        let _codepage = crate::codepage::scope(Some(ccsid));
        match ins {
            Instr::NumericContext { decimals, floating } => {
                self.instances[cur].numeric_context = (*decimals, *floating);
                Ok(())
            }
            Instr::Move(src, dst) => {
                let v = self.eval_src(cur, src)?;
                self.move_val(cur, &v, dst)
            }
            Instr::Store {
                value,
                dst,
                rounded,
                check,
            } => match self.numeric_store(cur, value, dst, *rounded, *check) {
                Ok(()) => Ok(()),
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
            Instr::SetTemp(t, ex) => self.numeric_temp(cur, *t, ex),
            Instr::Release(f, from) => {
                if self.sort_cancelled(cur)? {
                    return err(sort::CANCELLED);
                }
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
                let _ = self.stdout.write_all(&ebcdic::to_host(&out));
                if *advance {
                    let _ = self.stdout.write_all(b"\n");
                }
                Ok(())
            }
            Instr::Open(f, mode) => self.open_file(cur, *f, *mode),
            Instr::OpenTape {
                file,
                mode,
                reversed,
                no_rewind,
            } => self.open_tape_options(cur, *file, *mode, *reversed, *no_rewind),
            Instr::CloseTape {
                file,
                no_rewind,
                removal,
            } => self.close_tape_options(cur, *file, *no_rewind, *removal),
            Instr::Close(f, lock) => self.close_file(cur, *f, *lock),
            Instr::CloseReel(f) => self.close_reel(cur, *f),
            Instr::Write {
                file,
                record,
                from,
                advance,
            } => {
                self.write_file(cur, *file, *record, from.as_ref(), advance.as_ref())?;
                self.rerun_record(cur, *file)
            }
            Instr::Read {
                file,
                next,
                into,
                key,
            } => {
                self.read_file(cur, *file, *next, into.as_ref(), *key)?;
                self.rerun_record(cur, *file)
            }
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
            Instr::SetEntry(targets, name) => self.set_entry(cur, targets, name, None),
            Instr::SetEntryWithInterface(targets, name, mode) => {
                self.set_entry(cur, targets, name, Some(*mode))
            }
            Instr::Initiate(r) => self.report_initiate(cur, *r),
            Instr::Generate { report, group } => self.report_generate(cur, *report, *group),
            Instr::Terminate(r) => self.report_terminate(cur, *r),
            Instr::SuppressPrinting => {
                self.suppress_printing = true;
                Ok(())
            }
            Instr::Call {
                interface,
                program,
                args,
                returning,
            } => {
                let saved_rc = self.rc_before_call(cur)?;
                let callee = self.prepare_call(cur, program, args, *interface, false)?;
                let result = returning
                    .as_ref()
                    .map(|dst| self.pending_result(cur, callee, dst, saved_rc))
                    .transpose()?;
                if let Some(callee) = callee {
                    if returning.is_none() && self.instances[callee].code.result.is_some() {
                        self.bind_program_result(callee)?;
                    }
                    match self.run_from(callee, true)? {
                        Exit::Returned => {}
                        Exit::StopRun(_) => return err(STOP_RUN),
                    }
                }
                if let Some(result) = result {
                    self.finish_result(cur, callee, result)?;
                } else {
                    self.store_rc(cur);
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
        self.resolve_local_program(cur, name)
            .or_else(|| self.find_program(name))
    }

    fn resolve_local_program(&mut self, cur: usize, name: &str) -> Option<usize> {
        // Programs contained in the caller's module.
        let module = self.instances[cur].module.clone();
        let me = self.instances[cur].module_index;
        for (idx, p) in module.programs.iter().enumerate() {
            if p.name != name && !p.entries.iter().any(|e| e.name == name) {
                continue;
            }
            let mut ancestor = Some(me);
            let mut common_visible = false;
            while let Some(a) = ancestor {
                common_visible |= p.common && p.parent == Some(a);
                ancestor = module.programs[a as usize].parent;
            }
            if p.parent == Some(me) || common_visible || p.parent.is_none() {
                return Some(self.instantiate(&module, idx as u32));
            }
        }
        None
    }

    /// Whether program `name` can be called (it is loaded, if need be).
    pub fn load_program(&mut self, name: &str) -> bool {
        self.find_program(name).is_some()
            || self.load_hlasm(name).unwrap_or(false)
            || self.natives.contains_key(name)
            || self.natives_ref.contains_key(name)
    }

    /// A separately compiled program by name (loaded on first use).
    fn find_program(&mut self, name: &str) -> Option<usize> {
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
                    let index = m
                        .programs
                        .iter()
                        .position(|p| p.name == name || p.entries.iter().any(|e| e.name == name))
                        .unwrap_or(0);
                    return Some(self.instantiate(&m, index as u32));
                }
            }
            // An ENTRY exported by an object need not have a separate host file.
            let mut members: Vec<_> = std::fs::read_dir(&dir)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file() && p.extension().is_none_or(|x| x.eq_ignore_ascii_case("zo"))
                })
                .collect();
            members.sort();
            for path in members {
                if let Ok(module) = crate::load_module(&path)
                    && let Some(index) = module.programs.iter().position(|p| {
                        p.parent.is_none()
                            && (p.name == name || p.entries.iter().any(|e| e.name == name))
                    })
                {
                    return Some(self.instantiate(&Rc::new(module), index as u32));
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

    fn close_program_files(&mut self, i: usize) -> RResult<()> {
        for f in 0..self.instances[i].files.len() {
            let def = &self.instances[i].code.files[f];
            if def.owner.is_some() || def.external {
                continue;
            }
            self.close_file_implicit(i, f as u32)?;
        }
        Ok(())
    }
}

/// PERFORM return address marking the end of a declarative run on behalf
/// of a contained program.
const USE_RETURN: u32 = u32::MAX;

pub const SIZE_ERROR: &str = "\u{0}size error";

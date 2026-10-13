//! Loading the machine code of COBOL programs ([`zeus_cobol::native`])
//! and running it. This is the only unsafe code of the run time: the code
//! is copied to memory made executable, and called through a function
//! pointer with a context whose `step` routine is called back by it.

#![allow(unsafe_code)]

use std::collections::HashMap;
use std::rc::Rc;

use zeus_cobol::native::{LEAVE, NativeModule};
use zeus_cobol::vm::{Exit, Machine, NativeCode, RResult, RunState};

/// The context the machine code gets (its layout is fixed: `step` first).
#[repr(C)]
struct Ctx {
    step: extern "C" fn(*mut Ctx, u32) -> u32,
    /// The run unit's storage and the program's static storage offset in
    /// it, read by the machine code (zeus_cobol::native).
    mem: *mut u8,
    base: usize,
    machine: *mut Machine,
    state: *mut RunState,
    /// How the program was left, when by an end or an error.
    result: Option<RResult<Option<Exit>>>,
}

type ProgramFn = unsafe extern "C" fn(*mut Ctx, u32) -> u32;

/// Runs instruction `pc` with the interpreter; the next instruction, or
/// LEAVE when control left the program.
extern "C" fn step(ctx: *mut Ctx, pc: u32) -> u32 {
    // SAFETY: `ctx` is the context `Programs::run` made for this call of
    // the machine code; its machine and state outlive the call and are
    // not used elsewhere while it runs.
    let c = unsafe { &mut *ctx };
    let (m, s) = unsafe { (&mut *c.machine, &mut *c.state) };
    let cur = s.cur;
    s.pc = pc;
    let r = m.exec_step(s);
    // the storage may have moved (it grows)
    (c.mem, c.base) = m.native_memory(cur);
    match r {
        Ok(None) if s.cur == cur => s.pc,
        Ok(None) => LEAVE,
        r => {
            c.result = Some(r);
            LEAVE
        }
    }
}

/// The machine code of the programs of an executable.
pub struct Programs {
    /// The executable memory holding all the code.
    _memory: memmap2::Mmap,
    /// Program name -> (number of instructions, entry point).
    entries: HashMap<String, (usize, ProgramFn)>,
}

impl Programs {
    /// Loads the machine code of modules (those for this platform).
    pub fn load(modules: &[NativeModule]) -> Result<Option<Programs>, String> {
        let host = zeus_cobol::native::host_platform();
        let mine: Vec<&NativeModule> = modules.iter().filter(|m| m.platform == host).collect();
        if let Some(m) = modules.iter().find(|m| m.platform != host) {
            return Err(format!(
                "the machine code is for {}, this is {host}",
                m.platform
            ));
        }
        let total: usize = mine
            .iter()
            .flat_map(|m| m.programs.iter())
            .map(|p| p.2.len().div_ceil(16) * 16)
            .sum();
        if total == 0 {
            return Ok(None);
        }
        let mut mem = memmap2::MmapMut::map_anon(total).map_err(|e| e.to_string())?;
        let mut offsets = Vec::new();
        let mut at = 0;
        for p in mine.iter().flat_map(|m| m.programs.iter()) {
            mem[at..at + p.2.len()].copy_from_slice(&p.2);
            offsets.push((p.0.clone(), p.1 as usize, at));
            at += p.2.len().div_ceil(16) * 16;
        }
        let code = mem.make_exec().map_err(|e| e.to_string())?;
        // SAFETY: the code was written above and is not changed again
        unsafe {
            wasmtime_jit_icache_coherence::clear_cache(code.as_ptr().cast(), code.len())
                .map_err(|e| e.to_string())?;
            wasmtime_jit_icache_coherence::pipeline_flush_mt().map_err(|e| e.to_string())?;
        }
        let mut entries = HashMap::new();
        for (name, n, off) in offsets {
            // SAFETY: each program's code starts at its offset and is a
            // function of type ProgramFn (zeus_cobol::native)
            let f: ProgramFn = unsafe { std::mem::transmute(code.as_ptr().add(off)) };
            entries.insert(name, (n, f));
        }
        Ok(Some(Programs {
            _memory: code,
            entries,
        }))
    }
}

impl NativeCode for Programs {
    fn run(&self, m: &mut Machine, s: &mut RunState) -> Option<RResult<Option<Exit>>> {
        let (name, n) = m.program_shape(s.cur);
        let &(count, f) = self.entries.get(name)?;
        if count != n {
            return None;
        }
        let (mem, base) = m.native_memory(s.cur);
        let mut ctx = Ctx {
            step,
            mem,
            base,
            machine: m,
            state: s,
            result: None,
        };
        // SAFETY: the function was compiled for this platform with this
        // context layout; it calls back only `step`.
        unsafe { f(&mut ctx, s.pc) };
        Some(ctx.result.take().unwrap_or(Ok(None)))
    }
}

/// Makes the machine code of the image's `.zx` members run the programs.
pub fn install(m: &mut Machine, image: &zeus_cobol::image::Image) -> Result<(), String> {
    let modules: Vec<NativeModule> = image
        .members
        .iter()
        .filter_map(|(_, d)| NativeModule::from_bytes(d))
        .collect();
    if let Some(p) = Programs::load(&modules)? {
        m.set_native(Rc::new(p));
    }
    Ok(())
}

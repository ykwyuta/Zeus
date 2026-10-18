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

#[cfg(test)]
mod tests {
    use super::*;
    use zeus_cobol::zir::Instr;

    #[repr(C)]
    struct TestCtx<'a> {
        ctx: Ctx,
        code: &'a [Instr],
        calls: Vec<u32>,
    }

    // Integer arithmetic sets precision metadata and clears the size-error
    // flag before its native store. Permit only that bookkeeping; arithmetic,
    // moves, noninteger precision contexts and other callbacks stop the test.
    extern "C" fn first_step(ctx: *mut Ctx, pc: u32) -> u32 {
        // SAFETY: the test passes TestCtx, whose first field is Ctx.
        let c = unsafe { &mut *ctx.cast::<TestCtx<'_>>() };
        c.calls.push(pc);
        if matches!(
            c.code[pc as usize],
            Instr::ClearSizeError
                | Instr::NumericContext {
                    decimals: 0,
                    floating: false
                }
        ) {
            pc + 1
        } else {
            LEAVE
        }
    }

    #[test]
    fn display_arithmetic_and_moves_do_not_call_the_interpreter() {
        let source = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. DIRECT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 T PIC S9(4) VALUE 0.
       01 L PIC S9(4) SIGN LEADING VALUE 0.
       01 LS PIC S9(4) SIGN LEADING SEPARATE VALUE 0.
       01 TS PIC S9(4) SIGN TRAILING SEPARATE VALUE 0.
       01 U PIC 9(4) VALUE 0.
       01 P PIC S9(4) COMP-3 VALUE 0.
       01 B PIC S9(4) COMP VALUE 0.
       01 X PIC X(6) VALUE 'ABCDEF'.
       01 Y PIC X(3) JUSTIFIED RIGHT.
       01 Z PIC X(8) JUSTIFIED RIGHT.
       01 SUB-NUM PIC S9(4) COMP VALUE 2.
       01 TABLE-AREA.
          02 TABLE-N PIC S9(4) OCCURS 3 TIMES.
       01 MATRIX-AREA.
          02 ROW-ITEM OCCURS 2 TIMES.
             03 CELL PIC S9(4) COMP-3 OCCURS 3 TIMES.
       01 TEXT-TABLE.
          02 WORD-ITEM PIC X(6) OCCURS 2 TIMES.
       PROCEDURE DIVISION.
           COMPUTE TABLE-N(SUB-NUM) = -12
           COMPUTE CELL(SUB-NUM, SUB-NUM + 1) = TABLE-N(SUB-NUM) * 2
           MOVE CELL(2, 3) TO TABLE-N(SUB-NUM - 1)
           IF TABLE-N(1) NOT = -24 OR TABLE-N(SUB-NUM) NOT = -12
             DISPLAY 'BAD TABLE'
           END-IF
           MOVE 'ABCDEF' TO WORD-ITEM(2)
           MOVE 'UVWXYZ' TO WORD-ITEM(SUB-NUM - 1)
           MOVE WORD-ITEM(SUB-NUM)(SUB-NUM:SUB-NUM + 1) TO Z
           IF Z NOT = '     BCD' DISPLAY 'BAD DYNAMIC RIGHT' END-IF
           MOVE 'AB' TO WORD-ITEM(SUB-NUM)(SUB-NUM:SUB-NUM)
           IF WORD-ITEM(SUB-NUM)(SUB-NUM:SUB-NUM) NOT = 'AB'
             DISPLAY 'BAD DYNAMIC SLICE'
           END-IF
           MOVE ALL 'AB' TO WORD-ITEM(SUB-NUM)
           IF WORD-ITEM(SUB-NUM) NOT = ALL 'AB'
             DISPLAY 'BAD DYNAMIC ALL'
           END-IF
           MOVE 'ABCDEF' TO WORD-ITEM(SUB-NUM)
           MOVE WORD-ITEM(2)(2:3) TO WORD-ITEM(1)(1:3)
           IF WORD-ITEM(1)(1:3) NOT = 'BCD'
             DISPLAY 'BAD REFMOD'
           END-IF
           MOVE -123 TO T
           MOVE T TO L LS TS P B U
           ADD 1 TO L
           SUBTRACT 1 FROM LS
           COMPUTE TS = TS * -1
           COMPUTE T = T * 2 + 3
           MOVE X TO Y Z
           MOVE 'XY' TO Z
           MOVE 'LONGER' TO Y
           IF T IS NOT NEGATIVE OR U IS NOT POSITIVE
             DISPLAY 'BAD SIGN'
           END-IF
           IF X NOT = 'ABCDEF' OR Y NOT = 'GER' OR Z NOT = '      XY'
             DISPLAY 'BAD TEXT'
           END-IF
           MOVE ALL 'AB' TO Z
           IF Z NOT = ALL 'AB' DISPLAY 'BAD ALL' END-IF
           MOVE SPACES TO Z
           IF Z NOT = SPACES DISPLAY 'BAD SPACES' END-IF
           MOVE HIGH-VALUES TO Z
           IF Z NOT = HIGH-VALUES DISPLAY 'BAD HIGH' END-IF
           MOVE LOW-VALUES TO Z
           IF Z NOT = LOW-VALUES DISPLAY 'BAD LOW' END-IF
           MOVE QUOTES TO Z
           IF Z NOT = QUOTES DISPLAY 'BAD QUOTES' END-IF
           MOVE ZEROES TO Z
           IF Z NOT = ALL '0' DISPLAY 'BAD ZERO' END-IF
           MOVE 'XY' TO Z
           IF T NOT = -243 OR L NOT = -122
             DISPLAY 'BAD'
           END-IF
           IF LS NOT = -124 OR TS NOT = 123
             DISPLAY 'BAD'
           END-IF
           IF U NOT = 123 OR P NOT = -123 OR B NOT = -123
             DISPLAY 'BAD'
           END-IF
           IF 'A' < 'B' MOVE 'XY' TO Z
             ELSE MOVE 'UV' TO Z END-IF
           DISPLAY 'DONE'
           STOP RUN.
";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("direct.cbl");
        std::fs::write(&path, source).unwrap();
        let initial = zeus_cobol::compile_file(&path, &Default::default()).unwrap();
        for reverse in [false, true] {
            let mut module = initial.clone();
            if reverse {
                module.programs[0].collating = Some((0..256).map(|b| 255 - b).collect());
                module.programs[0].high_value = 0;
                module.programs[0].low_value = 255;
            }
            let p = &module.programs[0];
            let expected = p
                .code
                .iter()
                .rposition(|i| matches!(i, Instr::Display { .. }))
                .unwrap();
            let native =
                zeus_cobol::native::compile(&module, &zeus_cobol::native::host_triple()).unwrap();
            let programs = Programs::load(&[native]).unwrap().unwrap();
            let mut memory = p.static_init.clone();
            let mut ctx = TestCtx {
                ctx: Ctx {
                    step: first_step,
                    mem: memory.as_mut_ptr(),
                    base: 0,
                    machine: std::ptr::null_mut(),
                    state: std::ptr::null_mut(),
                    result: None,
                },
                code: &p.code,
                calls: Vec::new(),
            };
            let f = programs.entries["DIRECT"].1;
            // SAFETY: the host-compiled code uses the live context and storage;
            // first_step does not use the null machine/state pointers.
            unsafe { f(&mut ctx.ctx, 0) };
            let last = *ctx.calls.last().expect("DISPLAY callback") as usize;
            assert_eq!(
                last, expected,
                "unexpected interpreter fallback: {:?}",
                p.code[last]
            );
            assert!(ctx.calls[..ctx.calls.len() - 1].iter().all(|&pc| matches!(
                p.code[pc as usize],
                Instr::ClearSizeError
                    | Instr::NumericContext {
                        decimals: 0,
                        floating: false
                    }
            )));
            for (name, text) in [
                ("Y", "GER"),
                ("Z", if reverse { "      UV" } else { "      XY" }),
            ] {
                let d = p.descs.iter().find(|d| d.name == name).unwrap();
                let bytes = &memory[d.offset as usize..(d.offset + d.len) as usize];
                assert_eq!(bytes, zeus_cobol::ebcdic::encode(text));
            }
        }
    }
}

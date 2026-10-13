//! Machine code for COBOL programs (native executables, `zcobc compile
//! --native`): each program of a module becomes one function, compiled
//! with Cranelift for the target's instruction set.
//!
//! The function keeps the program's control flow in machine code: an
//! instruction is a basic block, GO TO is a jump, and the instructions
//! that branch (IF, PERFORM, the ends of paragraphs, CALL) continue at
//! their next block or at the block of the address they computed. The
//! work of each instruction is done by the run time's `step` routine
//! (the interpreter's code for that instruction), called through a table
//! in the context passed to the function, so the code needs no
//! relocations and can be loaded anywhere.
//!
//! The function: `fn(ctx: *mut Ctx, pc: u32) -> u32` starts at
//! instruction `pc` and returns when the program is left (a CALL, the
//! end of a called program, the end of the run, an error): then the run
//! time's state says where to go on. `step(ctx, pc) -> u32` runs
//! instruction `pc` and returns the next instruction of the same program,
//! or [`LEAVE`].
//!
//! The context (`#[repr(C)]` in the run time) begins with the address of
//! `step`, the address of the run unit's storage and the address of the
//! program's static storage in it (refreshed by `step`).
//!
//! Some instructions are machine code of their own when their data is
//! simple (WORKING-STORAGE items without subscripts or reference
//! modification): arithmetic and assignments of binary (COMP) and packed
//! decimal (COMP-3) integers, numeric comparisons in conditions, and
//! alphanumeric MOVE. When the values do not fit (a size error, an
//! overflow of the 64-bit arithmetic) they leave the instruction to
//! `step`, before storing anything.

use cranelift_codegen::control::ControlPlane;
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, JumpTableData, MemFlags, Signature, UserFuncName, types,
};
use cranelift_codegen::isa::{self, OwnedTargetIsa};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};

use cranelift_codegen::ir::{Block, Value};

use crate::zir::{
    Base, CmpMode, Cx, DataType, Ex, Instr, Loc, Module, NumType, NumUsage, Op, ProgramCode, Rel,
    Src,
};

/// `step`'s answer when control leaves the program.
pub const LEAVE: u32 = u32::MAX;

const MAGIC: &[u8; 8] = b"ZEUSMC01";

/// The machine code of a module's programs for one platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeModule {
    /// The instruction set and calling convention: `arch-abi` (see
    /// [`platform`]).
    pub platform: String,
    /// (program name, number of instructions, code).
    pub programs: Vec<(String, u32, Vec<u8>)>,
}

/// The platform key of a target triple: the architecture and the calling
/// convention family (`x86_64-sysv`, `x86_64-windows`, `aarch64-apple`,
/// `aarch64-sysv`, ...).
pub fn platform(triple: &str) -> String {
    let arch = triple.split('-').next().unwrap_or("");
    let abi = if triple.contains("windows") {
        "windows"
    } else if triple.contains("apple") || triple.contains("darwin") {
        "apple"
    } else {
        "sysv"
    };
    format!("{arch}-{abi}")
}

/// The platform key of the running program.
pub fn host_platform() -> String {
    let abi = if cfg!(windows) {
        "windows"
    } else if cfg!(target_vendor = "apple") {
        "apple"
    } else {
        "sysv"
    };
    format!("{}-{abi}", std::env::consts::ARCH)
}

/// The target triple of the running program's platform (for compiling
/// without `--target`).
pub fn host_triple() -> String {
    let os = if cfg!(windows) {
        "pc-windows-msvc"
    } else if cfg!(target_os = "macos") {
        "apple-darwin"
    } else {
        "unknown-linux-gnu"
    };
    format!("{}-{os}", std::env::consts::ARCH)
}

impl NativeModule {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        let text = |out: &mut Vec<u8>, s: &str| {
            out.extend((s.len() as u32).to_le_bytes());
            out.extend(s.as_bytes());
        };
        text(&mut out, &self.platform);
        out.extend((self.programs.len() as u32).to_le_bytes());
        for (name, n, code) in &self.programs {
            text(&mut out, name);
            out.extend(n.to_le_bytes());
            out.extend((code.len() as u32).to_le_bytes());
            out.extend(code);
        }
        out
    }

    pub fn from_bytes(data: &[u8]) -> Option<NativeModule> {
        let rest = data.strip_prefix(MAGIC)?;
        let mut at = 0;
        let u32_at = |at: &mut usize| -> Option<u32> {
            let v = u32::from_le_bytes(rest.get(*at..*at + 4)?.try_into().ok()?);
            *at += 4;
            Some(v)
        };
        let n = u32_at(&mut at)? as usize;
        let platform = String::from_utf8(rest.get(at..at + n)?.to_vec()).ok()?;
        at += n;
        let count = u32_at(&mut at)?;
        let mut programs = Vec::new();
        for _ in 0..count {
            let n = u32_at(&mut at)? as usize;
            let name = String::from_utf8(rest.get(at..at + n)?.to_vec()).ok()?;
            at += n;
            let instrs = u32_at(&mut at)?;
            let len = u32_at(&mut at)? as usize;
            programs.push((name, instrs, rest.get(at..at + len)?.to_vec()));
            at += len;
        }
        Some(NativeModule { platform, programs })
    }
}

/// The instruction set of a target triple.
fn target_isa(triple: &str) -> Result<OwnedTargetIsa, String> {
    let mut flags = settings::builder();
    flags.set("opt_level", "speed").map_err(|e| e.to_string())?;
    flags.set("is_pic", "true").map_err(|e| e.to_string())?;
    let builder = isa::lookup_by_name(triple).map_err(|e| format!("{triple}: {e}"))?;
    let isa = builder
        .finish(settings::Flags::new(flags))
        .map_err(|e| format!("{triple}: {e}"))?;
    if isa.pointer_bits() != 64 {
        return Err(format!("{triple}: only 64-bit targets are supported"));
    }
    Ok(isa)
}

/// Compiles the programs of a module for a target triple.
pub fn compile(module: &Module, triple: &str) -> Result<NativeModule, String> {
    let isa = target_isa(triple)?;
    let mut programs = Vec::new();
    for p in &module.programs {
        let code = compile_program(p, &isa).map_err(|e| format!("{}: {e}", p.name))?;
        programs.push((p.name.clone(), p.code.len() as u32, code));
    }
    Ok(NativeModule {
        platform: platform(triple),
        programs,
    })
}

/// Where control may go after instruction `i` without computing it:
/// its static successor.
fn static_target(ins: &Instr) -> Option<u32> {
    match ins {
        Instr::JumpIf(_, t) | Instr::JumpUnless(_, t) => Some(*t),
        Instr::Perform { start, .. } => Some(*start),
        _ => None,
    }
}

fn compile_program(p: &ProgramCode, isa: &OwnedTargetIsa) -> Result<Vec<u8>, String> {
    let ptr = isa.pointer_type();
    let cc = isa.default_call_conv();
    let mut sig = Signature::new(cc);
    sig.params.push(AbiParam::new(ptr));
    sig.params.push(AbiParam::new(types::I32));
    sig.returns.push(AbiParam::new(types::I32));
    let mut func = Function::with_name_signature(UserFuncName::user(0, 0), sig.clone());
    let mut fctx = FunctionBuilderContext::new();
    let mut b = FunctionBuilder::new(&mut func, &mut fctx);
    let n = p.code.len();
    let entry = b.create_block();
    b.append_block_params_for_function_params(entry);
    let blocks: Vec<_> = (0..n).map(|_| b.create_block()).collect();
    let dispatch = b.create_block();
    b.append_block_param(dispatch, types::I32);
    let exit = b.create_block();
    b.append_block_param(exit, types::I32);
    let step_sig = b.import_signature(sig);

    b.switch_to_block(entry);
    let ctx = b.block_params(entry)[0];
    let start = b.block_params(entry)[1];
    b.ins().jump(dispatch, &[start.into()]);

    // the instructions
    for (i, ins) in p.code.iter().enumerate() {
        b.switch_to_block(blocks[i]);
        let next = i as u32 + 1;
        if let Instr::Jump(t) = ins {
            match blocks.get(*t as usize) {
                Some(&blk) => b.ins().jump(blk, &[]),
                None => {
                    let t = b.ins().iconst(types::I32, i64::from(*t));
                    b.ins().jump(dispatch, &[t.into()])
                }
            };
            continue;
        }
        // the instruction in machine code, falling back to `step`
        let slow = b.create_block();
        let succ = |t: u32| blocks.get(t as usize).copied();
        let fast = {
            let mut l = Lower {
                b: &mut b,
                p,
                ctx,
                ptr,
                slow,
            };
            l.instr(ins, succ(next), static_target(ins).and_then(succ))
        };
        if !fast {
            b.ins().jump(slow, &[]);
        }
        b.switch_to_block(slow);
        let f = b.ins().load(ptr, MemFlags::trusted(), ctx, 0);
        let pc = b.ins().iconst(types::I32, i as i64);
        let call = b.ins().call_indirect(step_sig, f, &[ctx, pc]);
        let v = b.inst_results(call)[0];
        // the usual successors, then the computed address
        let mut tests: Vec<u32> = vec![next];
        if let Some(t) = static_target(ins)
            && t != next
        {
            tests.push(t);
        }
        for t in tests {
            let Some(&target) = blocks.get(t as usize) else {
                continue;
            };
            let is = b.ins().icmp_imm(IntCC::Equal, v, i64::from(t));
            let other = b.create_block();
            b.ins().brif(is, target, &[], other, &[]);
            b.switch_to_block(other);
        }
        b.ins().jump(dispatch, &[v.into()]);
    }

    // a computed address: an instruction of this program, or out
    b.switch_to_block(dispatch);
    let v = b.block_params(dispatch)[0];
    let inside = b.ins().icmp_imm(IntCC::UnsignedLessThan, v, n as i64);
    let table = b.create_block();
    b.ins().brif(inside, table, &[], exit, &[v.into()]);
    b.switch_to_block(table);
    let calls: Vec<_> = blocks
        .iter()
        .map(|&blk| b.func.dfg.block_call(blk, &[]))
        .collect();
    let default = b.func.dfg.block_call(exit, &[v.into()]);
    let jt = b.create_jump_table(JumpTableData::new(default, &calls));
    b.ins().br_table(v, jt);

    b.switch_to_block(exit);
    let r = b.block_params(exit)[0];
    b.ins().return_(&[r]);

    b.seal_all_blocks();
    b.finalize();

    let mut ctx = cranelift_codegen::Context::for_function(func);
    let compiled = ctx
        .compile(&**isa, &mut ControlPlane::default())
        .map_err(|e| format!("code generation failed: {:?}", e.inner))?;
    if !compiled.buffer.relocs().is_empty() {
        return Err("the code needs relocations".into());
    }
    Ok(compiled.code_buffer().to_vec())
}

/// Offsets in the context.
const CTX_MEM: i32 = 8;
const CTX_BASE: i32 = 16;

/// A WORKING-STORAGE item addressed without computation: its offset in
/// the program's static storage and length.
fn simple<'a>(p: &'a ProgramCode, loc: &Loc) -> Option<(u32, u32, &'a DataType)> {
    let d = p.descs.get(loc.desc as usize)?;
    (matches!(d.base, Base::Static)
        && loc.subs.is_empty()
        && loc.refmod.is_none()
        && d.dims.is_empty()
        && d.odo.is_none())
    .then_some((d.offset, d.len, &d.ty))
}

/// A numeric integer item the machine code reads (binary of 2, 4 or 8
/// bytes, packed decimal of up to 9 bytes).
fn int_item(p: &ProgramCode, loc: &Loc) -> Option<(u32, u32, NumType)> {
    let (off, len, ty) = simple(p, loc)?;
    let &DataType::Numeric(t) = ty else {
        return None;
    };
    let ok = t.scale == 0
        && match t.usage {
            NumUsage::Binary | NumUsage::Native => matches!(len, 2 | 4 | 8),
            NumUsage::Packed => len <= 9,
            NumUsage::Display => false,
        };
    ok.then_some((off, len, t))
}

struct Lower<'a, 'b> {
    b: &'a mut FunctionBuilder<'b>,
    p: &'a ProgramCode,
    ctx: Value,
    ptr: cranelift_codegen::ir::Type,
    /// The block that leaves the instruction to `step`.
    slow: Block,
}

impl Lower<'_, '_> {
    /// Emits instruction `ins`; false when it has no machine code of its
    /// own (nothing was emitted).
    fn instr(&mut self, ins: &Instr, next: Option<Block>, target: Option<Block>) -> bool {
        let Some(next) = next else {
            return false;
        };
        match ins {
            Instr::Store {
                value,
                dst,
                rounded: false,
                check: false,
            }
            | Instr::SetIndex(dst, value) => {
                let Some(item) = int_item(self.p, dst) else {
                    return false;
                };
                if item.2.usage == NumUsage::Native || !self.ex_ok(value) {
                    return false;
                }
                let v = self.ex(value);
                self.store_int(item, v);
                self.b.ins().jump(next, &[]);
                true
            }
            Instr::JumpIf(c, _) | Instr::JumpUnless(c, _) => {
                let Some(target) = target else {
                    return false;
                };
                if !self.cx_ok(c) {
                    return false;
                }
                let v = self.cx(c);
                if matches!(ins, Instr::JumpIf(..)) {
                    self.b.ins().brif(v, target, &[], next, &[]);
                } else {
                    self.b.ins().brif(v, next, &[], target, &[]);
                }
                true
            }
            Instr::Move(src, dst) => {
                let Some((doff, dlen, DataType::Alphanumeric { just: false } | DataType::Group)) =
                    simple(self.p, dst)
                else {
                    return false;
                };
                if dlen > 256 {
                    return false;
                }
                match src {
                    Src::Alnum(bytes) => {
                        let mut data = bytes.clone();
                        data.resize(dlen as usize, 0x40);
                        let base = self.static_addr();
                        self.store_const(base, doff, &data);
                    }
                    Src::Loc(l) => {
                        let Some((soff, slen, DataType::Alphanumeric { .. } | DataType::Group)) =
                            simple(self.p, l)
                        else {
                            return false;
                        };
                        // overlapping fields are left to the interpreter
                        if soff < doff + dlen && doff < soff + slen {
                            return false;
                        }
                        let base = self.static_addr();
                        let n = slen.min(dlen);
                        self.copy(base, soff, doff, n);
                        if n < dlen {
                            let pad = vec![0x40; (dlen - n) as usize];
                            self.store_const(base, doff + n, &pad);
                        }
                    }
                    _ => return false,
                }
                self.b.ins().jump(next, &[]);
                true
            }
            _ => false,
        }
    }

    /// The address of the program's static storage.
    fn static_addr(&mut self) -> Value {
        let f = MemFlags::trusted();
        let mem = self.b.ins().load(self.ptr, f, self.ctx, CTX_MEM);
        let base = self.b.ins().load(self.ptr, f, self.ctx, CTX_BASE);
        self.b.ins().iadd(mem, base)
    }

    fn store_const(&mut self, base: Value, off: u32, data: &[u8]) {
        let f = MemFlags::trusted();
        let mut at = 0;
        while at < data.len() {
            let (ty, n) = match data.len() - at {
                8.. => (types::I64, 8),
                4.. => (types::I32, 4),
                2.. => (types::I16, 2),
                _ => (types::I8, 1),
            };
            let mut word = [0u8; 8];
            word[..n].copy_from_slice(&data[at..at + n]);
            let v = self.b.ins().iconst(ty, i64::from_le_bytes(word));
            self.b.ins().store(f, v, base, (off as usize + at) as i32);
            at += n;
        }
    }

    fn copy(&mut self, base: Value, from: u32, to: u32, len: u32) {
        let f = MemFlags::trusted();
        let mut at = 0;
        while at < len {
            let (ty, n) = match len - at {
                8.. => (types::I64, 8),
                4.. => (types::I32, 4),
                2.. => (types::I16, 2),
                _ => (types::I8, 1),
            };
            let v = self.b.ins().load(ty, f, base, (from + at) as i32);
            self.b.ins().store(f, v, base, (to + at) as i32);
            at += n;
        }
    }

    // ---- integer expressions ---------------------------------------------

    fn ex_ok(&self, e: &Ex) -> bool {
        match e {
            Ex::Lit(d) => d.scale == 0 && i64::try_from(d.coef).is_ok(),
            Ex::Load(l) => int_item(self.p, l).is_some(),
            Ex::Bin(Op::Add | Op::Sub | Op::Mul, a, b) => self.ex_ok(a) && self.ex_ok(b),
            Ex::Neg(a) => self.ex_ok(a),
            _ => false,
        }
    }

    /// The value of an expression (checked by `ex_ok`) as i64; overflow
    /// goes to the slow block.
    fn ex(&mut self, e: &Ex) -> Value {
        match e {
            Ex::Lit(d) => self.b.ins().iconst(types::I64, d.coef as i64),
            Ex::Load(l) => {
                let item = int_item(self.p, l).expect("checked");
                self.load_int(item)
            }
            Ex::Bin(op, a, c) => {
                let x = self.ex(a);
                let y = self.ex(c);
                let (v, of) = match op {
                    Op::Add => self.b.ins().sadd_overflow(x, y),
                    Op::Sub => self.b.ins().ssub_overflow(x, y),
                    _ => self.b.ins().smul_overflow(x, y),
                };
                self.or_slow(of);
                v
            }
            Ex::Neg(a) => {
                let x = self.ex(a);
                let zero = self.b.ins().iconst(types::I64, 0);
                let (v, of) = self.b.ins().ssub_overflow(zero, x);
                self.or_slow(of);
                v
            }
            _ => unreachable!("checked by ex_ok"),
        }
    }

    /// Continues when `cond` is false, else goes to the slow block.
    fn or_slow(&mut self, cond: Value) {
        let go = self.b.create_block();
        self.b.ins().brif(cond, self.slow, &[], go, &[]);
        self.b.switch_to_block(go);
    }

    /// The value of a binary or packed decimal integer item.
    fn load_int(&mut self, (off, len, t): (u32, u32, NumType)) -> Value {
        let base = self.static_addr();
        let f = MemFlags::trusted();
        let off = off as i32;
        match t.usage {
            NumUsage::Packed => {
                // digits (invalid nibbles modulo 10) and the sign nibble
                let mut acc = self.b.ins().iconst(types::I64, 0);
                let mut neg = None;
                for k in 0..len as i32 {
                    let byte = self.b.ins().uload8(types::I64, f, base, off + k);
                    let hi = self.b.ins().ushr_imm(byte, 4);
                    let lo = self.b.ins().band_imm(byte, 15);
                    acc = self.digit(acc, hi);
                    if k + 1 < len as i32 {
                        acc = self.digit(acc, lo);
                    } else if t.signed {
                        let b = self.b.ins().icmp_imm(IntCC::Equal, lo, 0xB);
                        let d = self.b.ins().icmp_imm(IntCC::Equal, lo, 0xD);
                        neg = Some(self.b.ins().bor(b, d));
                    }
                }
                match neg {
                    Some(n) => {
                        let m = self.b.ins().ineg(acc);
                        self.b.ins().select(n, m, acc)
                    }
                    None => acc,
                }
            }
            _ => {
                let ty = match len {
                    2 => types::I16,
                    4 => types::I32,
                    _ => types::I64,
                };
                let raw = self.b.ins().load(ty, f, base, off);
                // z/Architecture binary data is big-endian
                let v = self.b.ins().bswap(raw);
                if ty == types::I64 {
                    if !t.signed {
                        // an unsigned value beyond i64 is left to the interpreter
                        let neg = self.b.ins().icmp_imm(IntCC::SignedLessThan, v, 0);
                        self.or_slow(neg);
                    }
                    v
                } else if t.signed {
                    self.b.ins().sextend(types::I64, v)
                } else {
                    self.b.ins().uextend(types::I64, v)
                }
            }
        }
    }

    /// acc * 10 + (nibble modulo 10).
    fn digit(&mut self, acc: Value, nib: Value) -> Value {
        let big = self
            .b
            .ins()
            .icmp_imm(IntCC::UnsignedGreaterThanOrEqual, nib, 10);
        let less = self.b.ins().iadd_imm(nib, -10);
        let d = self.b.ins().select(big, less, nib);
        let t = self.b.ins().imul_imm(acc, 10);
        self.b.ins().iadd(t, d)
    }

    /// Stores an integer in a binary (COMP) or packed decimal item; a value
    /// with more digits than the picture (a size error) goes to the slow
    /// block.
    fn store_int(&mut self, (off, len, t): (u32, u32, NumType), v: Value) {
        let cap = 10i64.pow(t.digits.min(18));
        let neg = self.b.ins().icmp_imm(IntCC::SignedLessThan, v, 0);
        let m = self.b.ins().ineg(v);
        let abs = self.b.ins().select(neg, m, v);
        let fits = self.b.ins().icmp_imm(IntCC::UnsignedLessThan, abs, cap);
        let go = self.b.create_block();
        self.b.ins().brif(fits, go, &[], self.slow, &[]);
        self.b.switch_to_block(go);
        let value = if t.signed { v } else { abs };
        let base = self.static_addr();
        let f = MemFlags::trusted();
        let off = off as i32;
        match t.usage {
            NumUsage::Packed => {
                let mut a = abs;
                let sign = if t.signed {
                    let minus = self.b.ins().iconst(types::I64, 0xD);
                    let plus = self.b.ins().iconst(types::I64, 0xC);
                    self.b.ins().select(neg, minus, plus)
                } else {
                    self.b.ins().iconst(types::I64, 0xF)
                };
                for k in (0..len as i32).rev() {
                    let (hi, lo) = if k + 1 == len as i32 {
                        let d = self.b.ins().urem_imm(a, 10);
                        a = self.b.ins().udiv_imm(a, 10);
                        (d, sign)
                    } else {
                        let lo = self.b.ins().urem_imm(a, 10);
                        a = self.b.ins().udiv_imm(a, 10);
                        let hi = self.b.ins().urem_imm(a, 10);
                        a = self.b.ins().udiv_imm(a, 10);
                        (hi, lo)
                    };
                    let h = self.b.ins().ishl_imm(hi, 4);
                    let byte = self.b.ins().bor(h, lo);
                    let byte = self.b.ins().ireduce(types::I8, byte);
                    self.b.ins().store(f, byte, base, off + k);
                }
            }
            _ => {
                let ty = match len {
                    2 => types::I16,
                    4 => types::I32,
                    _ => types::I64,
                };
                let x = if ty == types::I64 {
                    value
                } else {
                    self.b.ins().ireduce(ty, value)
                };
                let x = self.b.ins().bswap(x);
                self.b.ins().store(f, x, base, off);
            }
        }
    }

    // ---- conditions ---------------------------------------------------------

    fn num_src(&self, s: &Src) -> Option<Ex> {
        match s {
            Src::Num(e) => Some(e.clone()),
            Src::Loc(l) => int_item(self.p, l).map(|_| Ex::Load(l.clone())),
            Src::NumLit(d, _) => Some(Ex::Lit(*d)),
            _ => None,
        }
    }

    fn cx_ok(&self, c: &Cx) -> bool {
        match c {
            Cx::Cmp(a, _, b, CmpMode::Numeric) => match (self.num_src(a), self.num_src(b)) {
                (Some(x), Some(y)) => self.ex_ok(&x) && self.ex_ok(&y),
                _ => false,
            },
            Cx::Not(x) => self.cx_ok(x),
            Cx::And(x, y) | Cx::Or(x, y) => self.cx_ok(x) && self.cx_ok(y),
            Cx::True => true,
            _ => false,
        }
    }

    /// The truth of a condition (checked by `cx_ok`) as an i8.
    fn cx(&mut self, c: &Cx) -> Value {
        match c {
            Cx::Cmp(a, rel, b, _) => {
                let x = self.num_src(a).expect("checked");
                let y = self.num_src(b).expect("checked");
                let x = self.ex(&x);
                let y = self.ex(&y);
                let cc = match rel {
                    Rel::Eq => IntCC::Equal,
                    Rel::Ne => IntCC::NotEqual,
                    Rel::Lt => IntCC::SignedLessThan,
                    Rel::Le => IntCC::SignedLessThanOrEqual,
                    Rel::Gt => IntCC::SignedGreaterThan,
                    Rel::Ge => IntCC::SignedGreaterThanOrEqual,
                };
                self.b.ins().icmp(cc, x, y)
            }
            Cx::Not(x) => {
                let v = self.cx(x);
                self.b.ins().bxor_imm(v, 1)
            }
            Cx::And(x, y) => {
                // both evaluated: they have no side effects
                let a = self.cx(x);
                let b = self.cx(y);
                self.b.ins().band(a, b)
            }
            Cx::Or(x, y) => {
                let a = self.cx(x);
                let b = self.cx(y);
                self.b.ins().bor(a, b)
            }
            _ => self.b.ins().iconst(types::I8, 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_for_each_platform() {
        let src = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. LOOP.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 I PIC 9(4) COMP VALUE 0.
       PROCEDURE DIVISION.
           PERFORM COUNT-UP UNTIL I > 10
           IF I = 11 DISPLAY 'DONE' ELSE DISPLAY 'BAD' END-IF
           STOP RUN.
       COUNT-UP.
           ADD 1 TO I.
";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loop.cbl");
        std::fs::write(&path, src).unwrap();
        let module = crate::compile_file(&path, &Default::default()).unwrap();
        for triple in [
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu",
            "x86_64-pc-windows-msvc",
            "aarch64-apple-darwin",
        ] {
            let m = compile(&module, triple).unwrap();
            assert_eq!(m.programs.len(), 1);
            assert!(!m.programs[0].2.is_empty());
            assert_eq!(NativeModule::from_bytes(&m.to_bytes()), Some(m));
        }
        assert_eq!(platform("x86_64-pc-windows-gnu"), "x86_64-windows");
        assert_eq!(platform(&host_triple()), host_platform());
    }
}

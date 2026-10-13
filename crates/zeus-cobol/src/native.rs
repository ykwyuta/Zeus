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
//! in WORKING-STORAGE: arithmetic and assignments of binary (COMP) and packed
//! decimal (COMP-3) and zoned decimal (DISPLAY) integers, numeric
//! comparisons in conditions, and alphanumeric MOVE (including JUSTIFIED
//! RIGHT and figurative/ALL constants). Alphanumeric comparisons use
//! space padding and the program's collating sequence; integer sign
//! conditions are also lowered directly. When the values do not fit (a size error, an
//! overflow of the 64-bit arithmetic) they leave the instruction to
//! `step`, before storing anything.
//! Integer items can use computed subscripts (including multiple dimensions).
//! Text items (up to 256 bytes) can use computed subscripts and reference
//! modification, including variable start/length and omitted length.
//! Subscripts outside the declared table are left to the interpreter.

use cranelift_codegen::control::ControlPlane;
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{
    AbiParam, Function, InstBuilder, JumpTableData, MemFlags, Signature, StackSlotData,
    StackSlotKind, UserFuncName, types,
};
use cranelift_codegen::isa::{self, OwnedTargetIsa};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};

use cranelift_codegen::ir::{Block, Value};

use crate::zir::{
    Base, CmpMode, Cx, DataType, Ex, Fig, Instr, Loc, Module, NumType, NumUsage, Op, ProgramCode,
    Rel, SignKind, SignPos, Src,
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
    // Dynamic MOVE has one source; dynamic comparison has at least one
    // storage operand. One shared literal buffer suffices for both, keeping
    // the stack frame bounded even in programs with many such instructions.
    let scratch_slot =
        b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 256, 3));
    let scratch = b.ins().stack_addr(ptr, scratch_slot, 0);
    // One immutable lookup table shared by all custom-order comparisons.
    let collating = if let Some(weights) = &p.collating {
        if weights.len() != 256 {
            return Err("collating sequence must contain 256 weights".into());
        }
        let slot =
            b.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, 512, 3));
        let addr = b.ins().stack_addr(ptr, slot, 0);
        let data: Vec<_> = weights.iter().flat_map(|w| w.to_le_bytes()).collect();
        Lower {
            b: &mut b,
            p,
            ctx,
            ptr,
            slow: exit,
            collating: None,
            scratch,
        }
        .store_const(addr, 0, &data);
        Some(addr)
    } else {
        None
    };
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
                collating,
                scratch,
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

/// A WORKING-STORAGE item with a compile-time address and length, including
/// constant subscripts and reference modification within the item.
fn simple<'a>(p: &'a ProgramCode, loc: &Loc) -> Option<(u32, u32, &'a DataType)> {
    let d = p.descs.get(loc.desc as usize)?;
    if !matches!(d.base, Base::Static) || d.odo.is_some() || loc.subs.len() > d.dims.len() {
        return None;
    }
    let mut off = d.offset;
    for (i, dim) in d.dims.iter().enumerate() {
        let sub = match loc.subs.get(i) {
            Some(e) => const_int(e)?.max(1),
            None => 1,
        };
        if sub > i64::from(dim.max) {
            return None;
        }
        off = off.checked_add(u32::try_from(sub - 1).ok()?.checked_mul(dim.stride)?)?;
    }
    let mut len = d.len;
    let mut ty = &d.ty;
    if let Some((start, length)) = &loc.refmod {
        if matches!(d.ty, DataType::National | DataType::Dbcs) {
            return None;
        }
        let start = u32::try_from(const_int(start)?.max(1) - 1).ok()?;
        len = match length {
            Some(e) => u32::try_from(const_int(e)?.max(0)).ok()?,
            None => len.checked_sub(start)?,
        };
        if start.checked_add(len)? > d.len {
            return None;
        }
        off = off.checked_add(start)?;
        ty = &DataType::Alphanumeric { just: false };
    }
    (usize::try_from(off.checked_add(len)?).ok()? <= p.static_init.len()).then_some((off, len, ty))
}

fn const_int(e: &Ex) -> Option<i64> {
    match e {
        Ex::Lit(d) if d.scale == 0 => i64::try_from(d.coef).ok(),
        Ex::Neg(e) => const_int(e)?.checked_neg(),
        Ex::Bin(op, a, b) => {
            let (a, b) = (const_int(a)?, const_int(b)?);
            match op {
                Op::Add => a.checked_add(b),
                Op::Sub => a.checked_sub(b),
                Op::Mul => a.checked_mul(b),
                _ => None,
            }
        }
        _ => None,
    }
}

fn int_ex_ok(p: &ProgramCode, e: &Ex) -> bool {
    match e {
        Ex::Lit(d) => d.scale == 0 && i64::try_from(d.coef).is_ok(),
        Ex::Load(l) => int_item(p, l).is_some(),
        Ex::Bin(Op::Add | Op::Sub | Op::Mul, a, b) => int_ex_ok(p, a) && int_ex_ok(p, b),
        Ex::Neg(a) => int_ex_ok(p, a),
        _ => false,
    }
}

/// A numeric integer item the machine code reads (binary of 2, 4 or 8
/// bytes, packed decimal of up to 9 bytes, DISPLAY of up to 18 digits).
fn int_item(p: &ProgramCode, loc: &Loc) -> Option<(u32, u32, NumType)> {
    let d = p.descs.get(loc.desc as usize)?;
    if !matches!(d.base, Base::Static)
        || d.odo.is_some()
        || loc.refmod.is_some()
        || loc.subs.len() > d.dims.len()
        || !loc.subs.iter().all(|e| int_ex_ok(p, e))
    {
        return None;
    }
    // Check the entire declared table fits static storage before emitting
    // trusted loads. Out-of-declaration subscripts use the interpreter.
    let mut end = u64::from(d.offset) + u64::from(d.len);
    for dim in &d.dims {
        end = end.checked_add(u64::from(dim.max.checked_sub(1)?) * u64::from(dim.stride))?;
    }
    if end > p.static_init.len() as u64 {
        return None;
    }
    let (off, len) = (d.offset, d.len);
    let DataType::Numeric(t) = d.ty else {
        return None;
    };
    let ok = t.scale == 0
        && match t.usage {
            NumUsage::Binary | NumUsage::Native => matches!(len, 2 | 4 | 8),
            NumUsage::Packed => len <= 9,
            NumUsage::Display => {
                let separate = t.signed
                    && matches!(t.sign, SignPos::LeadingSeparate | SignPos::TrailingSeparate);
                (1..=18).contains(&t.digits) && len == t.digits + u32::from(separate)
            }
        };
    ok.then_some((off, len, t))
}

/// Digit range, sign byte and whether that byte is separate from the digits.
fn display_layout(len: u32, t: NumType) -> (u32, u32, Option<u32>, bool) {
    if !t.signed {
        return (0, len, None, false);
    }
    match t.sign {
        SignPos::LeadingSeparate => (1, len, Some(0), true),
        SignPos::TrailingSeparate => (0, len - 1, Some(len - 1), true),
        SignPos::Leading => (0, len, Some(0), false),
        SignPos::None | SignPos::Trailing => (0, len, Some(len - 1), false),
    }
}

struct Lower<'a, 'b> {
    b: &'a mut FunctionBuilder<'b>,
    p: &'a ProgramCode,
    ctx: Value,
    ptr: cranelift_codegen::ir::Type,
    /// The block that leaves the instruction to `step`.
    slow: Block,
    collating: Option<Value>,
    scratch: Value,
}

fn text_loc(p: &ProgramCode, loc: &Loc) -> Option<(u32, bool)> {
    let d = p.descs.get(loc.desc as usize)?;
    if !matches!(d.base, Base::Static)
        || d.odo.is_some()
        || loc.subs.len() > d.dims.len()
        || !loc.subs.iter().all(|e| int_ex_ok(p, e))
    {
        return None;
    }
    let mut end = u64::from(d.offset) + u64::from(d.len);
    for dim in &d.dims {
        end = end.checked_add(u64::from(dim.max.checked_sub(1)?) * u64::from(dim.stride))?;
    }
    if end > p.static_init.len() as u64 || d.len > 256 {
        return None;
    }
    if let Some((start, len)) = &loc.refmod {
        if matches!(d.ty, DataType::National | DataType::Dbcs)
            || !int_ex_ok(p, start)
            || len.as_ref().is_some_and(|e| !int_ex_ok(p, e))
        {
            return None;
        }
        Some((d.len, false))
    } else {
        match d.ty {
            DataType::Alphanumeric { just } | DataType::Alphabetic { just } => Some((d.len, just)),
            DataType::Group => Some((d.len, false)),
            _ => None,
        }
    }
}

struct TextView {
    addr: Value,
    len: Value,
    repeat: bool,
}

/// Byte sources whose alphanumeric representation needs no conversion.
enum Text {
    Item(u32, u32),
    Dynamic(Loc, u32),
    Literal(Vec<u8>),
    Repeat(Vec<u8>),
}

impl Text {
    fn len(&self) -> u32 {
        match self {
            Self::Item(_, n) => *n,
            Self::Dynamic(_, n) => *n,
            Self::Literal(b) => b.len() as u32,
            Self::Repeat(_) => 0,
        }
    }
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
                self.store_int(item, v, dst);
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
                if let Some(item) = int_item(self.p, dst) {
                    let Some(value) = self.num_src(src) else {
                        return false;
                    };
                    if item.2.usage == NumUsage::Native || !self.ex_ok(&value) {
                        return false;
                    }
                    let v = self.ex(&value);
                    self.store_int(item, v, dst);
                    self.b.ins().jump(next, &[]);
                    return true;
                }
                let dynamic = simple(self.p, dst).is_none()
                    || matches!(src, Src::Loc(l) if simple(self.p, l).is_none());
                if dynamic {
                    return self.dynamic_move(src, dst, next);
                }
                let Some((
                    doff,
                    dlen,
                    ty @ (DataType::Alphanumeric { .. }
                    | DataType::Alphabetic { .. }
                    | DataType::Group),
                )) = simple(self.p, dst)
                else {
                    return false;
                };
                let just = matches!(
                    ty,
                    DataType::Alphanumeric { just: true } | DataType::Alphabetic { just: true }
                );
                if dlen > 256 {
                    return false;
                }
                match src {
                    Src::Fig(_) | Src::All(_) => {
                        let Some(Text::Repeat(pattern)) = self.text(src) else {
                            return false;
                        };
                        let data: Vec<_> = pattern
                            .iter()
                            .copied()
                            .cycle()
                            .take(dlen as usize)
                            .collect();
                        let base = self.static_addr();
                        self.store_const(base, doff, &data);
                    }
                    Src::Alnum(bytes) => {
                        let mut data = vec![0x40; dlen as usize];
                        let n = bytes.len().min(data.len());
                        let from = if just { bytes.len() - n } else { 0 };
                        let to = if just { data.len() - n } else { 0 };
                        data[to..to + n].copy_from_slice(&bytes[from..from + n]);
                        let base = self.static_addr();
                        self.store_const(base, doff, &data);
                    }
                    Src::Loc(l) => {
                        let Some((
                            soff,
                            slen,
                            DataType::Alphanumeric { .. }
                            | DataType::Alphabetic { .. }
                            | DataType::Group,
                        )) = simple(self.p, l)
                        else {
                            return false;
                        };
                        // overlapping fields are left to the interpreter
                        if soff < doff + dlen && doff < soff + slen {
                            return false;
                        }
                        let base = self.static_addr();
                        let n = slen.min(dlen);
                        let from = soff + if just { slen - n } else { 0 };
                        let to = doff + if just { dlen - n } else { 0 };
                        self.copy(base, from, to, n);
                        if n < dlen {
                            let pad = vec![0x40; (dlen - n) as usize];
                            let pad_off = if just { doff } else { doff + n };
                            self.store_const(base, pad_off, &pad);
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

    /// Address of an integer item, checking each computed table subscript
    /// before accessing memory.
    fn numeric_addr(&mut self, loc: &Loc) -> Value {
        let d = &self.p.descs[loc.desc as usize];
        let mut offset = self.b.ins().iconst(types::I64, i64::from(d.offset));
        for (i, dim) in d.dims.iter().enumerate() {
            let sub = match loc.subs.get(i) {
                Some(e) => self.ex(e),
                None => self.b.ins().iconst(types::I64, 1),
            };
            // The interpreter clamps subscripts below one to one.
            let low = self.b.ins().icmp_imm(IntCC::SignedLessThan, sub, 1);
            let one = self.b.ins().iconst(types::I64, 1);
            let sub = self.b.ins().select(low, one, sub);
            let high = self
                .b
                .ins()
                .icmp_imm(IntCC::UnsignedGreaterThan, sub, i64::from(dim.max));
            self.or_slow(high);
            let index = self.b.ins().iadd_imm(sub, -1);
            let bytes = self.b.ins().imul_imm(index, i64::from(dim.stride));
            offset = self.b.ins().iadd(offset, bytes);
        }
        let base = self.static_addr();
        self.b.ins().iadd(base, offset)
    }

    fn text_addr(&mut self, loc: &Loc) -> TextView {
        let mut addr = self.numeric_addr(loc);
        let total = self.p.descs[loc.desc as usize].len;
        let mut len = self.b.ins().iconst(types::I64, i64::from(total));
        if let Some((start, length)) = &loc.refmod {
            let s = self.ex(start);
            let low = self.b.ins().icmp_imm(IntCC::SignedLessThan, s, 1);
            let one = self.b.ins().iconst(types::I64, 1);
            let s = self.b.ins().select(low, one, s);
            let start = self.b.ins().iadd_imm(s, -1);
            let outside = self.b.ins().icmp(IntCC::UnsignedGreaterThan, start, len);
            self.or_slow(outside);
            let available = self.b.ins().isub(len, start);
            len = if let Some(e) = length {
                let n = self.ex(e);
                let negative = self.b.ins().icmp_imm(IntCC::SignedLessThan, n, 0);
                let zero = self.b.ins().iconst(types::I64, 0);
                let n = self.b.ins().select(negative, zero, n);
                let outside = self.b.ins().icmp(IntCC::UnsignedGreaterThan, n, available);
                self.or_slow(outside);
                n
            } else {
                available
            };
            addr = self.b.ins().iadd(addr, start);
        }
        TextView {
            addr,
            len,
            repeat: false,
        }
    }

    fn text_view(&mut self, t: &Text) -> TextView {
        match t {
            Text::Dynamic(loc, _) => self.text_addr(loc),
            Text::Item(off, len) => {
                let base = self.static_addr();
                let addr = self.b.ins().iadd_imm(base, i64::from(*off));
                let len = self.b.ins().iconst(types::I64, i64::from(*len));
                TextView {
                    addr,
                    len,
                    repeat: false,
                }
            }
            Text::Literal(bytes) | Text::Repeat(bytes) => {
                let addr = self.scratch;
                self.store_const(addr, 0, bytes);
                let len = self.b.ins().iconst(types::I64, bytes.len() as i64);
                TextView {
                    addr,
                    len,
                    repeat: matches!(t, Text::Repeat(_)),
                }
            }
        }
    }

    /// Branch before loading: a select of an out-of-bounds load is unsafe.
    fn view_byte(&mut self, t: &TextView, index: Value) -> Value {
        if t.repeat {
            let index = self.b.ins().urem(index, t.len);
            let addr = self.b.ins().iadd(t.addr, index);
            return self
                .b
                .ins()
                .uload8(types::I64, MemFlags::trusted(), addr, 0);
        }
        let present = self.b.ins().icmp(IntCC::UnsignedLessThan, index, t.len);
        let load = self.b.create_block();
        let pad = self.b.create_block();
        let join = self.b.create_block();
        self.b.append_block_param(join, types::I64);
        self.b.ins().brif(present, load, &[], pad, &[]);
        self.b.switch_to_block(load);
        let addr = self.b.ins().iadd(t.addr, index);
        let byte = self
            .b
            .ins()
            .uload8(types::I64, MemFlags::trusted(), addr, 0);
        self.b.ins().jump(join, &[byte.into()]);
        self.b.switch_to_block(pad);
        let space = self
            .b
            .ins()
            .iconst(types::I64, i64::from(crate::ebcdic::SPACE));
        self.b.ins().jump(join, &[space.into()]);
        self.b.switch_to_block(join);
        self.b.block_params(join)[0]
    }

    fn dynamic_move(&mut self, src: &Src, dst: &Loc, next: Block) -> bool {
        let Some((_, just)) = text_loc(self.p, dst) else {
            return false;
        };
        let Some(source) = self.text(src) else {
            return false;
        };
        if source.len() > 256 || matches!(&source, Text::Repeat(b) if b.len() > 256) {
            return false;
        }
        let source = self.text_view(&source);
        let dest = self.text_addr(dst);
        if matches!(src, Src::Loc(_)) {
            let send = self.b.ins().iadd(source.addr, source.len);
            let dend = self.b.ins().iadd(dest.addr, dest.len);
            let a = self
                .b
                .ins()
                .icmp(IntCC::UnsignedLessThan, source.addr, dend);
            let b = self.b.ins().icmp(IntCC::UnsignedLessThan, dest.addr, send);
            let overlap = self.b.ins().band(a, b);
            self.or_slow(overlap);
        }
        let header = self.b.create_block();
        let body = self.b.create_block();
        self.b.append_block_param(header, types::I64);
        let zero = self.b.ins().iconst(types::I64, 0);
        self.b.ins().jump(header, &[zero.into()]);
        self.b.switch_to_block(header);
        let i = self.b.block_params(header)[0];
        let inside = self.b.ins().icmp(IntCC::UnsignedLessThan, i, dest.len);
        self.b.ins().brif(inside, body, &[], next, &[]);
        self.b.switch_to_block(body);
        let index = if just && !source.repeat {
            let shift = self.b.ins().isub(source.len, dest.len);
            self.b.ins().iadd(i, shift)
        } else {
            i
        };
        let byte = self.view_byte(&source, index);
        let addr = self.b.ins().iadd(dest.addr, i);
        let byte = self.b.ins().ireduce(types::I8, byte);
        self.b.ins().store(MemFlags::trusted(), byte, addr, 0);
        let i = self.b.ins().iadd_imm(i, 1);
        self.b.ins().jump(header, &[i.into()]);
        true
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
        int_ex_ok(self.p, e)
    }

    /// The value of an expression (checked by `ex_ok`) as i64; overflow
    /// goes to the slow block.
    fn ex(&mut self, e: &Ex) -> Value {
        match e {
            Ex::Lit(d) => self.b.ins().iconst(types::I64, d.coef as i64),
            Ex::Load(l) => {
                let item = int_item(self.p, l).expect("checked");
                self.load_int(item, l)
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

    /// The value of a binary, packed or zoned decimal integer item.
    fn load_int(&mut self, (_, len, t): (u32, u32, NumType), loc: &Loc) -> Value {
        let base = self.numeric_addr(loc);
        let f = MemFlags::trusted();
        let off = 0;
        match t.usage {
            NumUsage::Display => {
                let (start, end, sign, separate) = display_layout(len, t);
                let mut acc = self.b.ins().iconst(types::I64, 0);
                for k in start..end {
                    let byte = self.b.ins().uload8(types::I64, f, base, off + k as i32);
                    let nib = self.b.ins().band_imm(byte, 15);
                    acc = self.digit(acc, nib);
                }
                if let Some(k) = sign {
                    let byte = self.b.ins().uload8(types::I64, f, base, off + k as i32);
                    let neg = if separate {
                        self.b.ins().icmp_imm(IntCC::Equal, byte, 0x60)
                    } else {
                        let zone = self.b.ins().ushr_imm(byte, 4);
                        let b = self.b.ins().icmp_imm(IntCC::Equal, zone, 0xB);
                        let d = self.b.ins().icmp_imm(IntCC::Equal, zone, 0xD);
                        self.b.ins().bor(b, d)
                    };
                    let minus = self.b.ins().ineg(acc);
                    self.b.ins().select(neg, minus, acc)
                } else {
                    acc
                }
            }
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

    /// Stores an integer in a binary, packed or zoned decimal item; a value
    /// with more digits than the picture (a size error) goes to the slow
    /// block.
    fn store_int(&mut self, (_, len, t): (u32, u32, NumType), v: Value, loc: &Loc) {
        let cap = 10i64.pow(t.digits.min(18));
        let neg = self.b.ins().icmp_imm(IntCC::SignedLessThan, v, 0);
        let m = self.b.ins().ineg(v);
        let abs = self.b.ins().select(neg, m, v);
        let fits = self.b.ins().icmp_imm(IntCC::UnsignedLessThan, abs, cap);
        let go = self.b.create_block();
        self.b.ins().brif(fits, go, &[], self.slow, &[]);
        self.b.switch_to_block(go);
        let value = if t.signed { v } else { abs };
        let base = self.numeric_addr(loc);
        let f = MemFlags::trusted();
        let off = 0;
        match t.usage {
            NumUsage::Display => {
                let (start, end, sign, separate) = display_layout(len, t);
                let mut a = abs;
                for k in (start..end).rev() {
                    let digit = self.b.ins().urem_imm(a, 10);
                    a = self.b.ins().udiv_imm(a, 10);
                    let zone = if sign == Some(k) && !separate {
                        let minus = self.b.ins().iconst(types::I64, 0xD0);
                        let plus = self.b.ins().iconst(types::I64, 0xC0);
                        self.b.ins().select(neg, minus, plus)
                    } else {
                        self.b.ins().iconst(types::I64, 0xF0)
                    };
                    let byte = self.b.ins().bor(zone, digit);
                    let byte = self.b.ins().ireduce(types::I8, byte);
                    self.b.ins().store(f, byte, base, off + k as i32);
                }
                if separate {
                    let minus = self.b.ins().iconst(types::I8, 0x60);
                    let plus = self.b.ins().iconst(types::I8, 0x4E);
                    let byte = self.b.ins().select(neg, minus, plus);
                    self.b
                        .ins()
                        .store(f, byte, base, off + sign.expect("separate sign") as i32);
                }
            }
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

    fn text(&self, s: &Src) -> Option<Text> {
        Some(match s {
            Src::Loc(l) => {
                if simple(self.p, l).is_none() {
                    return text_loc(self.p, l).map(|(n, _)| Text::Dynamic(l.clone(), n));
                }
                let (
                    off,
                    len,
                    DataType::Alphanumeric { .. } | DataType::Alphabetic { .. } | DataType::Group,
                ) = simple(self.p, l)?
                else {
                    return None;
                };
                Text::Item(off, len)
            }
            Src::Alnum(b) => Text::Literal(b.clone()),
            Src::All(b) if !b.is_empty() => Text::Repeat(b.clone()),
            Src::Fig(f) => Text::Repeat(vec![match f {
                Fig::Zero => crate::ebcdic::ZERO,
                Fig::Space => crate::ebcdic::SPACE,
                Fig::Quote => crate::ebcdic::QUOTE,
                Fig::High => self.p.high_value,
                Fig::Low => self.p.low_value,
            }]),
            _ => return None,
        })
    }

    fn text_byte(&mut self, t: &Text, at: u32, base: Value) -> Value {
        match t {
            Text::Item(off, len) if at < *len => {
                self.b
                    .ins()
                    .uload8(types::I64, MemFlags::trusted(), base, (off + at) as i32)
            }
            _ => {
                let byte = match t {
                    Text::Literal(b) => b.get(at as usize).copied().unwrap_or(crate::ebcdic::SPACE),
                    Text::Repeat(b) => b[at as usize % b.len()],
                    Text::Item(..) => crate::ebcdic::SPACE,
                    Text::Dynamic(..) => unreachable!("dynamic comparison uses views"),
                };
                self.b.ins().iconst(types::I64, i64::from(byte))
            }
        }
    }

    fn weight(&mut self, byte: Value) -> Value {
        if let Some(table) = self.collating {
            let off = self.b.ins().ishl_imm(byte, 1);
            let addr = self.b.ins().iadd(table, off);
            self.b
                .ins()
                .uload16(types::I64, MemFlags::trusted(), addr, 0)
        } else {
            byte
        }
    }

    fn dynamic_cmp(&mut self, a: &Text, rel: Rel, b: &Text) -> Value {
        let a = self.text_view(a);
        let b = self.text_view(b);
        let zero = self.b.ins().iconst(types::I64, 0);
        let alen = if a.repeat { zero } else { a.len };
        let blen = if b.repeat { zero } else { b.len };
        let a_longer = self.b.ins().icmp(IntCC::UnsignedGreaterThan, alen, blen);
        let n = self.b.ins().select(a_longer, alen, blen);
        let empty = self.b.ins().icmp_imm(IntCC::Equal, n, 0);
        let one = self.b.ins().iconst(types::I64, 1);
        let n = self.b.ins().select(empty, one, n);
        let header = self.b.create_block();
        let body = self.b.create_block();
        let advance = self.b.create_block();
        let done = self.b.create_block();
        self.b.append_block_param(header, types::I64);
        self.b.append_block_param(done, types::I64);
        self.b.ins().jump(header, &[zero.into()]);
        self.b.switch_to_block(header);
        let i = self.b.block_params(header)[0];
        let inside = self.b.ins().icmp(IntCC::UnsignedLessThan, i, n);
        self.b.ins().brif(inside, body, &[], done, &[zero.into()]);
        self.b.switch_to_block(body);
        let x = self.view_byte(&a, i);
        let y = self.view_byte(&b, i);
        let x = self.weight(x);
        let y = self.weight(y);
        let diff = self.b.ins().isub(x, y);
        let equal = self.b.ins().icmp(IntCC::Equal, x, y);
        self.b.ins().brif(equal, advance, &[], done, &[diff.into()]);
        self.b.switch_to_block(advance);
        let i = self.b.ins().iadd_imm(i, 1);
        self.b.ins().jump(header, &[i.into()]);
        self.b.switch_to_block(done);
        let order = self.b.block_params(done)[0];
        self.b.ins().icmp_imm(rel_cc(rel), order, 0)
    }

    fn text_cmp(&mut self, a: &Src, rel: Rel, b: &Src) -> Value {
        let a = self.text(a).expect("checked");
        let b = self.text(b).expect("checked");
        if matches!(a, Text::Dynamic(..)) || matches!(b, Text::Dynamic(..)) {
            return self.dynamic_cmp(&a, rel, &b);
        }
        let base = self.static_addr();
        let mut order = self.b.ins().iconst(types::I64, 0);
        // Earlier differing characters override the suffix's ordering.
        for i in (0..a.len().max(b.len()).max(1)).rev() {
            let x = self.text_byte(&a, i, base);
            let y = self.text_byte(&b, i, base);
            let x = self.weight(x);
            let y = self.weight(y);
            let diff = self.b.ins().isub(x, y);
            let equal = self.b.ins().icmp(IntCC::Equal, x, y);
            order = self.b.ins().select(equal, order, diff);
        }
        self.b.ins().icmp_imm(rel_cc(rel), order, 0)
    }

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
            Cx::Cmp(a, _, b, CmpMode::Alnum) => match (self.text(a), self.text(b)) {
                (Some(a), Some(b)) => {
                    a.len().max(b.len()) <= 256
                        && [&a, &b]
                            .iter()
                            .all(|t| !matches!(t, Text::Repeat(p) if p.len() > 256))
                }
                _ => false,
            },
            Cx::Sign(e, _) => self.ex_ok(e),
            Cx::Not(x) => self.cx_ok(x),
            Cx::And(x, y) | Cx::Or(x, y) => self.cx_ok(x) && self.cx_ok(y),
            Cx::True => true,
            _ => false,
        }
    }

    /// The truth of a condition (checked by `cx_ok`) as an i8.
    fn cx(&mut self, c: &Cx) -> Value {
        match c {
            Cx::Cmp(a, rel, b, CmpMode::Alnum) => self.text_cmp(a, *rel, b),
            Cx::Sign(e, kind) => {
                let value = self.ex(e);
                let cc = match kind {
                    SignKind::Positive => IntCC::SignedGreaterThan,
                    SignKind::Negative => IntCC::SignedLessThan,
                    SignKind::Zero => IntCC::Equal,
                };
                self.b.ins().icmp_imm(cc, value, 0)
            }
            Cx::Cmp(a, rel, b, CmpMode::Numeric) => {
                let x = self.num_src(a).expect("checked");
                let y = self.num_src(b).expect("checked");
                let x = self.ex(&x);
                let y = self.ex(&y);
                let cc = rel_cc(*rel);
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

fn rel_cc(rel: Rel) -> IntCC {
    match rel {
        Rel::Eq => IntCC::Equal,
        Rel::Ne => IntCC::NotEqual,
        Rel::Lt => IntCC::SignedLessThan,
        Rel::Le => IntCC::SignedLessThanOrEqual,
        Rel::Gt => IntCC::SignedGreaterThan,
        Rel::Ge => IntCC::SignedGreaterThanOrEqual,
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
       01 T PIC S9(4) VALUE 0.
       01 L PIC S9(4) SIGN LEADING VALUE 0.
       01 LS PIC S9(4) SIGN LEADING SEPARATE VALUE 0.
       01 TS PIC S9(4) SIGN TRAILING SEPARATE VALUE 0.
       01 TEXT-OUT PIC X(6) JUSTIFIED RIGHT.
       01 TABLE-AREA.
          02 TABLE-N PIC S9(4) OCCURS 3 TIMES.
       01 TEXT-TABLE.
          02 WORD-ITEM PIC X(6) OCCURS 3 TIMES.
       PROCEDURE DIVISION.
           MOVE -123 TO T
           MOVE 2 TO I
           MOVE 'ABCDEF' TO WORD-ITEM(I)
           MOVE WORD-ITEM(I)(I:I + 1) TO TEXT-OUT
           IF WORD-ITEM(I)(I:) = 'BCDEF'
             MOVE ALL 'AB' TO WORD-ITEM(I)
           END-IF
           COMPUTE TABLE-N(I) = T * 2
           IF TABLE-N(I) < 0 MOVE 'OK' TO TEXT-OUT(2:2) END-IF
           MOVE T TO L LS TS
           COMPUTE LS = LS * 2 + 1
           IF LS < 0 MOVE 'OK' TO TEXT-OUT END-IF
           MOVE ALL 'AB' TO TEXT-OUT
           IF TEXT-OUT = ALL 'AB' AND LS IS NEGATIVE
             MOVE SPACES TO TEXT-OUT
           END-IF
           PERFORM COUNT-UP UNTIL I > 10
           IF I = 11 DISPLAY 'DONE' ELSE DISPLAY 'BAD' END-IF
           STOP RUN.
       COUNT-UP.
           ADD 1 TO I.
";
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loop.cbl");
        std::fs::write(&path, src).unwrap();
        let initial = crate::compile_file(&path, &Default::default()).unwrap();
        for custom in [false, true] {
            let mut module = initial.clone();
            if custom {
                module.programs[0].collating = Some((0..256).map(|b| 255 - b).collect());
            }
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
        }
        assert_eq!(platform("x86_64-pc-windows-gnu"), "x86_64-windows");
        assert_eq!(platform(&host_triple()), host_platform());
    }
}

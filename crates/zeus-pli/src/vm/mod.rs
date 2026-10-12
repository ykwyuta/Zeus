//! The interpreter: storage, block activations, statements and
//! conditions.

mod bif;
mod cics;
mod conv;
mod record;
mod sql;
mod stream;
pub mod value;

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;

use crate::ir::*;
use crate::types::{AREA_HEADER, Ext, Ty, Var as Varying};
pub use value::Value;
use value::{NULL, SPACE};
use zeus_cobol::interlang::{ABEND, Linker, XEnd, XRef};

/// Addresses below this one are not valid for PL/I pointers (SYSNULL and
/// the PSA).
const NULL_PAGE: u32 = 0x10;
/// Size of a stack segment.
const STACK_SEGMENT: u32 = 256 * 1024;

/// A data set allocated to a DD name.
#[derive(Debug, Clone)]
pub struct DdInfo {
    pub path: PathBuf,
    pub format: Option<zeus_ds::format::RecordFormat>,
    /// KEYS(length offset) of a key-sequenced data set.
    pub keys: Option<(u32, u32)>,
    /// A SYSOUT data set (text).
    pub sysout: bool,
}

/// The environment of a run.
pub struct RunEnv {
    pub dds: HashMap<String, DdInfo>,
    /// PARM of the step.
    pub parm: String,
    /// SYSPRINT when it is not allocated.
    pub stdout: Box<dyn Write>,
    /// Messages (MSGFILE).
    pub msgout: Box<dyn Write>,
    /// SYSIN when it is not allocated (host text lines).
    pub stdin: Option<Vec<String>>,
    /// Read for SYSIN when `stdin` is None and SYSIN is not allocated.
    pub stdin_reader: Option<Box<dyn std::io::Read>>,
    /// Finds external procedures of other modules (PL/I or COBOL).
    pub linker: Option<Rc<dyn Linker>>,
    /// Runs EXEC SQL statements (the Db2 interface).
    pub exec_host: Option<Rc<dyn zeus_cobol::vm::exec::ExecHost>>,
}

/// How a run ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunEnd {
    /// Return code (PLIRETC).
    Rc(i32),
    /// Abnormal termination (ERROR): user abend code.
    Abend(String),
}

/// Non-sequential control transfer.
#[derive(Debug)]
pub enum Intr {
    Goto(LabelId, usize),
    Leave(LoopId),
    Iterate(LoopId),
    Return(Option<Value>),
    Stop,
    /// The program ends abnormally after ERROR.
    Terminate,
    Fatal(String),
    /// EXEC CICS RETURN (or XCTL): the program ends.
    CicsReturn,
}

pub type R<T> = std::result::Result<T, Intr>;

/// Dimensions of a reference: lower bound, upper bound, stride (bits).
pub type Dims = Vec<(i64, i64, u64)>;

/// A parameter bound to an argument.
#[derive(Debug, Clone)]
pub struct Binding {
    /// Bit address.
    pub bit: u64,
    pub dims: Vec<(i64, i64, u64)>,
    pub len: Option<u32>,
}

#[derive(Debug, Clone, Default)]
struct CondInfo {
    code: i32,
    onchar: u8,
    onsource: Vec<u8>,
    onkey: Vec<u8>,
    onfile: String,
    onloc: String,
}

struct Act {
    block: BlockId,
    parent: Option<usize>,
    frame: u32,
    params: Vec<Option<Binding>>,
    slots: Vec<i64>,
    on: Vec<(Cond, Option<(BlockId, usize)>)>,
    stack_mark: (usize, u32),
    /// Indexes of aggregate assignments and repetitive specifications.
    index: Vec<i64>,
    subject: Option<Value>,
    cond: Option<CondInfo>,
    /// The handler being run (for RESIGNAL): condition and the activation
    /// where the search continues.
    handling: Option<(Cond, usize)>,
}

/// Location of a scalar: bit address and resolved type.
#[derive(Debug, Clone)]
pub struct Loc {
    pub bit: u64,
    pub ty: Ty,
}

pub struct Machine {
    /// Static storage initialised (by `run` or the first `call_entry`).
    started: bool,
    module: Rc<Module>,
    /// The storage of the run unit (its COBOL machine), lent to this
    /// module while it runs.
    sys: Option<zeus_cobol::vm::Machine>,
    /// Address of the static storage in the run unit's storage (0: not
    /// allocated yet).
    static_base: u32,
    /// Stack segments in the run unit's storage (address, length) and the
    /// stack pointer (segment, offset).
    stack: Vec<(u32, u32)>,
    sp: (usize, u32),
    acts: Vec<Act>,
    pub env: RunEnv,
    files: Vec<stream::FileState>,
    rc: i32,
    random: u64,
    depth: usize,
    /// The program's instance for the CICS host (its HANDLE CONDITION
    /// labels), set by the task that runs it.
    pub cics_instance: usize,
    /// The load module of each fetched entry (FETCH TITLE).
    titles: std::collections::HashMap<String, String>,
    /// The subscripts of the references to iSUB-defined arrays being
    /// located (innermost last).
    isubs: Vec<Vec<i64>>,
}

/// Whether an expression uses iSUB.
fn has_isub(e: &Expr) -> bool {
    match &*e.k {
        Ek::ISub(_) => true,
        Ek::Arith(_, a, b) => has_isub(a) || has_isub(b),
        Ek::Neg(a) | Ek::Conv(a) => has_isub(a),
        _ => false,
    }
}

fn err<T>(m: impl Into<String>) -> R<T> {
    Err(Intr::Fatal(m.into()))
}

impl Machine {
    /// A machine for `module` with a run unit of its own (no COBOL
    /// programs).
    pub fn new(module: Module, env: RunEnv) -> Machine {
        let mut m = Machine::module(module, env);
        m.attach(zeus_cobol::vm::Machine::placeholder());
        m
    }

    /// A machine for `module` running in the run unit `sys` (whose COBOL
    /// programs it can call).
    pub fn with_storage(module: Module, env: RunEnv, sys: zeus_cobol::vm::Machine) -> Machine {
        let mut m = Machine::module(module, env);
        m.attach(sys);
        m
    }

    /// A machine for `module` with no storage: it runs when a run unit's
    /// storage is lent to it (see [`Machine::attach`]).
    pub fn module(module: Module, env: RunEnv) -> Machine {
        let files = module.files.iter().map(stream::FileState::new).collect();
        Machine {
            started: false,
            module: Rc::new(module),
            sys: None,
            static_base: 0,
            stack: Vec::new(),
            sp: (0, 0),
            acts: Vec::new(),
            env,
            files,
            rc: 0,
            random: 0x2545_F491_4F6C_DD1D,
            depth: 0,
            cics_instance: 0,
            titles: std::collections::HashMap::new(),
            isubs: Vec::new(),
        }
    }

    // ---- memory --------------------------------------------------------

    /// Lends the run unit's storage to this module: its static storage is
    /// allocated there the first time.
    pub fn attach(&mut self, mut sys: zeus_cobol::vm::Machine) {
        if self.static_base == 0 {
            let n = self.module.static_size as usize;
            let a = sys.getmain(n.max(8), 0);
            sys.storage_mut()[a..a + n].copy_from_slice(&self.module.static_init);
            self.static_base = a as u32;
        }
        self.sys = Some(sys);
    }

    /// Gives the run unit's storage back.
    pub fn detach(&mut self) -> zeus_cobol::vm::Machine {
        // the stack segments stay allocated in the run unit for the next
        // call
        self.sys
            .take()
            .unwrap_or_else(zeus_cobol::vm::Machine::placeholder)
    }

    /// The run unit (its COBOL machine).
    pub fn sys(&mut self) -> &mut zeus_cobol::vm::Machine {
        self.sys
            .as_mut()
            .expect("a PL/I module runs with the storage of its run unit")
    }

    fn storage(&self) -> &[u8] {
        self.sys.as_ref().map_or(&[], |s| s.storage())
    }

    fn mem(&self, addr: u32, len: usize) -> R<&[u8]> {
        if !(NULL_PAGE..NULL).contains(&addr) {
            return err(format!("address {addr:08X} is not valid (null pointer?)"));
        }
        let a = addr as usize;
        self.storage()
            .get(a..a + len)
            .ok_or_else(|| Intr::Fatal(format!("address {addr:08X} out of storage")))
    }

    fn mem_mut(&mut self, addr: u32, len: usize) -> R<&mut [u8]> {
        if !(NULL_PAGE..NULL).contains(&addr) {
            return err(format!("address {addr:08X} is not valid (null pointer?)"));
        }
        let a = addr as usize;
        self.sys()
            .storage_mut()
            .get_mut(a..a + len)
            .ok_or_else(|| Intr::Fatal(format!("address {addr:08X} out of storage")))
    }

    fn read(&self, addr: u32, len: usize) -> R<Vec<u8>> {
        Ok(self.mem(addr, len)?.to_vec())
    }

    fn write(&mut self, addr: u32, data: &[u8]) -> R<()> {
        self.mem_mut(addr, data.len())?.copy_from_slice(data);
        Ok(())
    }

    /// Heap storage (ALLOCATE), doubleword aligned and zeroed.
    /// The address of the first allocation of an AREA variable.
    pub(crate) fn area_base(&mut self, a: VarId, act: usize) -> R<u32> {
        let r = Ref {
            var: a,
            subs: Vec::new(),
            locator: None,
        };
        let l = self.locate(&r, act)?;
        Ok((l.bit / 8) as u32 + AREA_HEADER)
    }

    /// Allocates `size` bytes in an area (ALLOCATE IN); None when it is
    /// full (after the AREA condition).
    fn area_alloc(&mut self, area: &Ref, size: u32, act: usize) -> R<Option<u32>> {
        let l = self.locate(area, act)?;
        let Ty::Area(cap) = l.ty else {
            return err("ALLOCATE IN: not an AREA");
        };
        let addr = (l.bit / 8) as u32;
        let h = self.read(addr, 8)?;
        let used = u32::from_be_bytes([h[4], h[5], h[6], h[7]]);
        // doubleword boundaries within the area
        let at = used.div_ceil(8) * 8;
        if at + size > cap {
            self.raise(Cond::Area, 360, act)?;
            return Ok(None);
        }
        self.write(addr, &cap.to_be_bytes())?;
        self.write(addr + 4, &(at + size).to_be_bytes())?;
        let p = addr + AREA_HEADER + at;
        self.write(p, &vec![0u8; size as usize])?;
        Ok(Some(p))
    }

    fn heap_alloc(&mut self, size: u32) -> u32 {
        self.sys().getmain(size.max(1) as usize, 0) as u32
    }

    /// Storage of an activation (zeroed); released by `deactivate`.
    fn stack_alloc(&mut self, size: u32) -> u32 {
        let (seg, off) = self.sp;
        let at = off.div_ceil(8) * 8;
        if let Some(&(base, len)) = self.stack.get(seg)
            && at + size <= len
        {
            self.sp = (seg, at + size);
            let a = base + at;
            self.sys().storage_mut()[a as usize..(a + size) as usize].fill(0);
            return a;
        }
        // the next segment, or a new one
        let next = if self.stack.is_empty() { 0 } else { seg + 1 };
        if self.stack.get(next).is_none_or(|&(_, len)| len < size) {
            let len = size.max(STACK_SEGMENT);
            let base = self.sys().getmain(len as usize, 0) as u32;
            self.stack.truncate(next);
            self.stack.push((base, len));
        }
        let base = self.stack[next].0;
        self.sp = (next, size);
        self.sys().storage_mut()[base as usize..(base + size) as usize].fill(0);
        base
    }

    fn read_bits(&self, bit: u64, n: u64) -> R<Vec<bool>> {
        let first = (bit / 8) as u32;
        let last = (bit + n).div_ceil(8) as u32;
        let bytes = self.read(first, (last - first) as usize)?;
        let off = bit % 8;
        Ok((0..n)
            .map(|i| {
                let k = off + i;
                bytes[(k / 8) as usize] >> (7 - k % 8) & 1 == 1
            })
            .collect())
    }

    fn write_bits(&mut self, bit: u64, bits: &[bool]) -> R<()> {
        let n = bits.len() as u64;
        let first = (bit / 8) as u32;
        let last = (bit + n).div_ceil(8) as u32;
        let mut bytes = self.read(first, (last - first) as usize)?;
        let off = bit % 8;
        for (i, &b) in bits.iter().enumerate() {
            let k = off + i as u64;
            let mask = 1u8 << (7 - k % 8);
            if b {
                bytes[(k / 8) as usize] |= mask;
            } else {
                bytes[(k / 8) as usize] &= !mask;
            }
        }
        self.write(first, &bytes)
    }

    // ---- activations ---------------------------------------------------

    /// The activation of `block` visible from `act` (static chain).
    fn act_of(&self, act: usize, block: BlockId) -> R<usize> {
        let mut a = Some(act);
        while let Some(i) = a {
            if self.acts[i].block == block {
                return Ok(i);
            }
            a = self.acts[i].parent;
        }
        err(format!(
            "block {} is not active",
            self.module.blocks[block as usize].name
        ))
    }

    /// Activates a block: frame, extents, initial values.
    fn activate(
        &mut self,
        block: BlockId,
        parent: Option<usize>,
        params: Vec<Option<Binding>>,
        _returns: Option<Ty>,
    ) -> R<usize> {
        let m = self.module.clone();
        let b = &m.blocks[block as usize];
        let mark = self.sp;
        let frame = self.stack_alloc(b.auto_size.max(8));
        self.acts.push(Act {
            block,
            parent,
            frame,
            params,
            slots: Vec::new(),
            on: Vec::new(),
            stack_mark: mark,
            index: Vec::new(),
            subject: None,
            cond: None,
            handling: None,
        });
        let act = self.acts.len() - 1;
        for e in &b.extents {
            let v = self.eval(e, act)?;
            self.acts[act].slots.push(v.as_i64());
        }
        // storage of adjustable automatic variables
        for (i, v) in m.vars.iter().enumerate() {
            if v.block == block
                && let Storage::AutoDyn(off) = v.storage
            {
                let size = self.var_size(i as VarId, act, None)?;
                let a = self.stack_alloc(size as u32);
                let fa = self.acts[act].frame + off;
                self.write(fa, &a.to_be_bytes())?;
            }
        }
        for init in &b.inits {
            self.initialize(init, act)?;
        }
        Ok(act)
    }

    fn deactivate(&mut self, act: usize) {
        let mark = self.acts[act].stack_mark;
        self.acts.truncate(act);
        self.sp = mark;
    }

    /// Size in bytes of all elements of a level-1 variable (`rb`: the bit
    /// address of the generation, for REFER extents).
    fn var_size(&self, v: VarId, act: usize, rb: Option<u64>) -> R<u64> {
        let var = &self.module.vars[v as usize];
        if var.ty.is_none() && self.has_refers(v) {
            // a structure with REFER extents: its mapped size, and what the
            // REFER members take beyond their mapped sizes
            let mut size = i64::from(var.size);
            for k in 0..self.module.vars.len() {
                if self.module.vars[k].root == v && k as VarId != v {
                    size += self.refer_delta(k as VarId, act, rb)?;
                }
            }
            return Ok(size.max(0) as u64);
        }
        let ty = self.resolve_ty(var.ty.as_ref(), v, act, rb)?;
        let elem = match &ty {
            Some(t) => {
                let aligned = !matches!(t, Ty::Char { .. } | Ty::Bit { .. } | Ty::Pic(_));
                u64::from(t.storage(aligned).0)
            }
            None => u64::from(var.size),
        };
        let mut n = 1u64;
        for d in &var.dims {
            let (lb, ub) = (self.bound(d.lb, act, v, rb)?, self.bound(d.ub, act, v, rb)?);
            n *= (ub - lb + 1).max(0) as u64;
        }
        Ok(elem * n)
    }

    /// The bytes a scalar member with REFER extents takes beyond its
    /// mapped size (the mapping counts the extents as 1 element and a
    /// length of 0); 0 for other members.
    fn refer_delta(&self, w: VarId, act: usize, rb: Option<u64>) -> R<i64> {
        let var = &self.module.vars[w as usize];
        let refer_len = matches!(var.ty.as_ref().and_then(Ty::str_len), Some(Ext::Refer(_)));
        let refer_dim = var
            .dims
            .iter()
            .any(|d| matches!(d.lb, Bound::Refer(_)) || matches!(d.ub, Bound::Refer(_)));
        if var.ty.is_none() || !(refer_len || refer_dim) {
            return Ok(0);
        }
        let mut mapped = i64::from(var.size);
        for d in &var.dims {
            if let (Bound::Const(l), Bound::Const(u)) = (d.lb, d.ub) {
                mapped *= (u - l + 1).max(0);
            }
        }
        Ok(self.var_size(w, act, rb)? as i64 - mapped)
    }

    /// The bit offset of a member in its level-1 structure: after the
    /// members with REFER extents before it, by their sizes now.
    fn member_bit_offset(&self, v: VarId, act: usize, rb: u64) -> R<u64> {
        let var = &self.module.vars[v as usize];
        let root = var.root;
        if v == root || self.module.refers.is_empty() || !self.has_refers(root) {
            return Ok(var.bit_offset);
        }
        let mut delta = 0i64;
        for w in (root + 1)..v {
            if self.module.vars[w as usize].root == root {
                delta += self.refer_delta(w, act, Some(rb))?;
            }
        }
        Ok((var.bit_offset as i64 + delta * 8).max(0) as u64)
    }

    /// Whether a level-1 variable has REFER extents.
    fn has_refers(&self, root: VarId) -> bool {
        self.module
            .refers
            .iter()
            .any(|r| self.module.vars[r.object as usize].root == root)
    }

    /// The bit address of the generation of a reference's root when the
    /// variable's own extents need it (REFER).
    fn refer_base(&mut self, r: &Ref, act: usize) -> R<Option<u64>> {
        let root = self.module.vars[r.var as usize].root;
        if self.module.refers.is_empty() || !self.has_refers(root) {
            return Ok(None);
        }
        let rr = Ref {
            var: root,
            subs: Vec::new(),
            locator: r.locator.clone(),
        };
        Ok(Some(self.base_of(&rr, act)?.0))
    }

    /// `refer_base` when the variable's string length is a REFER.
    fn refer_base_for_len(&mut self, r: &Ref, act: usize) -> R<Option<u64>> {
        let var = &self.module.vars[r.var as usize];
        if matches!(var.ty.as_ref().and_then(Ty::str_len), Some(Ext::Refer(_))) {
            self.refer_base(r, act)
        } else {
            Ok(None)
        }
    }

    /// The value of a REFER extent.
    fn refer_value(&self, n: u32, rb: Option<u64>) -> R<i64> {
        if let Some(v) = REFER_OVERRIDE.with(|o| o.borrow().as_ref().map(|v| v[n as usize])) {
            return Ok(v);
        }
        let obj = &self.module.vars[self.module.refers[n as usize].object as usize];
        let rb = rb.ok_or_else(|| Intr::Fatal("REFER extent without its structure".into()))?;
        let loc = Loc {
            bit: rb + obj.bit_offset,
            ty: obj.ty.clone().unwrap_or(Ty::fixed_bin(31, 0)),
        };
        Ok(self.load(&loc)?.as_i64())
    }

    /// Pushes a new generation of a CONTROLLED variable: a header (the
    /// previous generation, the data address, the extent slots) and the
    /// data, initialised by its INITIAL.
    fn allocate_controlled(
        &mut self,
        v: VarId,
        slots: &[Option<Expr>],
        given: &[Option<Expr>],
        act: usize,
    ) -> R<()> {
        let m = self.module.clone();
        let var = &m.vars[v as usize];
        let decl_act = self.act_of(act, var.block).unwrap_or(act);
        let mut vals = Vec::new();
        for (k, s) in slots.iter().enumerate() {
            let x = match (given.get(k).and_then(Option::as_ref), s) {
                (Some(g), _) => self.eval(g, act)?.as_i64(),
                (None, Some(e)) => self.eval(e, decl_act)?.as_i64(),
                (None, None) => {
                    return err(format!("ALLOCATE of {} needs its extents", var.name));
                }
            };
            vals.push(x);
        }
        let anchor = self.anchor_addr(self.module.vars[v as usize].root, act)?;
        let prev = self.generation(anchor)?;
        let h = self.heap_alloc(16 + 8 * vals.len() as u32);
        let mut hdr = Vec::new();
        hdr.extend(prev.to_be_bytes());
        hdr.extend(0u32.to_be_bytes());
        hdr.extend((vals.len() as u32).to_be_bytes());
        hdr.extend([0; 4]);
        for x in &vals {
            hdr.extend(x.to_be_bytes());
        }
        self.write(h, &hdr)?;
        self.write(anchor, &h.to_be_bytes())?;
        let size = self.var_size(v, act, None)?;
        let p = self.heap_alloc(size as u32);
        self.write(h + 4, &p.to_be_bytes())?;
        for init in &m.static_inits {
            if m.vars[init.var as usize].root == v {
                self.initialize(init, act)?;
            }
        }
        Ok(())
    }

    /// The number of generations of a CONTROLLED variable.
    pub fn allocation(&self, v: VarId, act: usize) -> R<i64> {
        let root_id = self.module.vars[v as usize].root;
        let root = &self.module.vars[root_id as usize];
        let Storage::Controlled(..) = &root.storage else {
            return Ok(1);
        };
        let mut g = self.generation(self.anchor_addr(root_id, act)?)?;
        let mut n = 0;
        while g != 0 {
            n += 1;
            g = self.word_at(g)?;
        }
        Ok(n)
    }

    /// The address of the anchor of a CONTROLLED variable (`root`): in
    /// static storage, or the caller's for a CONTROLLED parameter.
    fn anchor_addr(&self, root: VarId, act: usize) -> R<u32> {
        let var = &self.module.vars[root as usize];
        let Storage::Controlled(a, _) = &var.storage else {
            return err(format!("{} is not CONTROLLED", var.name));
        };
        if a & crate::ir::CTL_PARAM == 0 {
            return Ok(self.static_base + a);
        }
        let k = (a & !crate::ir::CTL_PARAM) as usize;
        let at = self.act_of(act, var.block)?;
        let b = self.acts[at]
            .params
            .get(k)
            .cloned()
            .flatten()
            .ok_or_else(|| Intr::Fatal(format!("parameter {} has no argument", var.name)))?;
        Ok((b.bit / 8) as u32)
    }

    /// The header of the current generation of a CONTROLLED variable
    /// (from the address of its anchor).
    fn generation(&self, anchor: u32) -> R<u32> {
        let p = self.read(anchor, 4)?;
        Ok(u32::from_be_bytes([p[0], p[1], p[2], p[3]]))
    }

    fn word_at(&self, a: u32) -> R<u32> {
        let p = self.read(a, 4)?;
        Ok(u32::from_be_bytes([p[0], p[1], p[2], p[3]]))
    }

    /// Extent slot `s` of a variable: of its CONTROLLED generation or of
    /// the activation.
    fn slot(&self, v: VarId, s: u32, act: usize) -> R<i64> {
        let root_id = self.module.vars[v as usize].root;
        let root = &self.module.vars[root_id as usize];
        if let Storage::Controlled(..) = &root.storage {
            let g = self.generation(self.anchor_addr(root_id, act)?)?;
            if g == 0 {
                return err(format!("{} is not allocated", root.name));
            }
            let b = self.read(g + 16 + 8 * s, 8)?;
            return Ok(i64::from_be_bytes(b.try_into().unwrap()));
        }
        let a = self.act_of(act, root.block).unwrap_or(act);
        self.acts[a]
            .slots
            .get(s as usize)
            .copied()
            .ok_or_else(|| Intr::Fatal("extent not evaluated".into()))
    }

    fn bound(&self, b: Bound, act: usize, v: VarId, rb: Option<u64>) -> R<i64> {
        Ok(match b {
            Bound::Const(n) => n,
            Bound::Slot(s) => self.slot(v, s, act)?,
            Bound::Refer(n) => self.refer_value(n, rb)?,
            Bound::Star => 1,
        })
    }

    /// The type of a variable with `*` and adjustable lengths resolved.
    fn resolve_ty(&self, ty: Option<&Ty>, v: VarId, act: usize, rb: Option<u64>) -> R<Option<Ty>> {
        let Some(ty) = ty else { return Ok(None) };
        let len = match ty.str_len() {
            Some(Ext::Slot(s)) => Some(self.slot(v, s, act)?.max(0) as u32),
            Some(Ext::Refer(n)) => Some(self.refer_value(n, rb)?.max(0) as u32),
            Some(Ext::Star) => {
                let root = self.module.vars[v as usize].root;
                match &self.module.vars[root as usize].storage {
                    Storage::Param(k) => {
                        let a = self.act_of(act, self.module.vars[root as usize].block)?;
                        self.acts[a]
                            .params
                            .get(*k as usize)
                            .cloned()
                            .flatten()
                            .and_then(|b| b.len)
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        Ok(Some(match (ty, len) {
            (Ty::Char { var, .. }, Some(n)) => Ty::Char {
                len: Ext::Const(n),
                var: *var,
            },
            (t @ Ty::Wchar { .. }, Some(n)) => t.with_len(Ext::Const(n)),
            (Ty::Bit { var, aligned, .. }, Some(n)) => Ty::Bit {
                len: Ext::Const(n),
                var: *var,
                aligned: *aligned,
            },
            (t, _) => t.clone(),
        }))
    }

    /// Base bit address and dimensions of a reference's variable.
    fn base_of(&mut self, r: &Ref, act: usize) -> R<(u64, Dims)> {
        let m = self.module.clone();
        let var = &m.vars[r.var as usize];
        let root = &m.vars[var.root as usize];
        let mut param_dims: Option<Vec<(i64, i64, u64)>> = None;
        let base: u64 = match &root.storage {
            Storage::Static(off) => u64::from(self.static_base + off) * 8,
            Storage::Auto(off) => {
                let a = self.act_of(act, root.block)?;
                u64::from(self.acts[a].frame + off) * 8
            }
            Storage::AutoDyn(off) => {
                let a = self.act_of(act, root.block)?;
                let fa = self.acts[a].frame + off;
                let p = self.read(fa, 4)?;
                u64::from(u32::from_be_bytes([p[0], p[1], p[2], p[3]])) * 8
            }
            Storage::Param(k) => {
                let a = self.act_of(act, root.block)?;
                let b = self.acts[a]
                    .params
                    .get(*k as usize)
                    .cloned()
                    .flatten()
                    .ok_or_else(|| {
                        Intr::Fatal(format!("parameter {} has no argument", root.name))
                    })?;
                if !b.dims.is_empty() {
                    param_dims = Some(b.dims.clone());
                }
                b.bit
            }
            Storage::Based(loc) => {
                let p = match (&r.locator, loc) {
                    (Some(l), _) => self.eval(l, act)?,
                    (None, Some(l)) => {
                        let a = self.act_of(act, root.block).unwrap_or(act);
                        self.eval(l, a)?
                    }
                    (None, None) => {
                        return err(format!("{} is BASED without a locator", root.name));
                    }
                };
                // an OFFSET locator: in its area
                let lty = match (&r.locator, loc) {
                    (Some(l), _) | (None, Some(l)) => l.ty.clone(),
                    _ => Ty::Pointer,
                };
                let p = if matches!(lty, Ty::Offset(Some(_))) {
                    self.convert(p, &lty, &Ty::Pointer, act)?
                } else {
                    p
                };
                let Value::Ptr(p) = p else {
                    return err("locator is not a pointer");
                };
                if p == NULL || p == 0 {
                    return err(format!("reference to {} through a null pointer", root.name));
                }
                u64::from(p) * 8
            }
            Storage::Defined(base, pos) => {
                let b = base.clone();
                let (bit, _) = self.element_or_start(&b, act)?;
                let unit = if root.ty.as_ref().is_some_and(|t| t.is_bit()) {
                    1
                } else {
                    8
                };
                let p = match pos {
                    Some(p) => self.eval(p, act)?.as_i64().max(1) as u64,
                    None => 1,
                };
                bit + (p - 1) * unit
            }
            Storage::Controlled(..) => {
                let g = self.generation(self.anchor_addr(var.root, act)?)?;
                if g == 0 {
                    return err(format!("{} is not allocated", root.name));
                }
                u64::from(self.word_at(g + 4)?) * 8
            }
            Storage::Member => return err("invalid member storage"),
        };
        // dimensions
        let mut dims = Vec::new();
        let nroot = root.dims.len();
        let dynamic = var.dims.iter().any(|d| {
            matches!(d.lb, Bound::Slot(_) | Bound::Refer(_))
                || matches!(d.ub, Bound::Slot(_) | Bound::Refer(_))
        });
        let rb = Some(base);
        if dynamic {
            // strides from the extents (level-1 arrays of scalars)
            let ty = self.resolve_ty(var.ty.as_ref(), r.var, act, rb)?;
            let elem = match &ty {
                Some(Ty::Bit {
                    len: Ext::Const(n),
                    var: Varying::Fixed,
                    aligned: false,
                }) => u64::from(*n),
                Some(t) => {
                    let aligned = !matches!(t, Ty::Char { .. } | Ty::Bit { .. } | Ty::Pic(_));
                    u64::from(t.storage(aligned).0) * 8
                }
                None => u64::from(var.size) * 8,
            };
            let a = self.act_of(act, var.block)?;
            let mut bounds = Vec::new();
            for d in &var.dims {
                bounds.push((
                    self.bound(d.lb, a, r.var, rb)?,
                    self.bound(d.ub, a, r.var, rb)?,
                ));
            }
            let mut stride = elem;
            let mut strides = vec![0; bounds.len()];
            for k in (0..bounds.len()).rev() {
                strides[k] = stride;
                stride *= (bounds[k].1 - bounds[k].0 + 1).max(0) as u64;
            }
            for (k, (lb, ub)) in bounds.into_iter().enumerate() {
                dims.push((lb, ub, strides[k]));
            }
        } else {
            for (k, d) in var.dims.iter().enumerate() {
                if k < nroot
                    && let Some(pd) = &param_dims
                    && let Some(x) = pd.get(k)
                {
                    dims.push(*x);
                    continue;
                }
                dims.push((
                    self.bound(d.lb, act, r.var, rb)?,
                    self.bound(d.ub, act, r.var, rb)?,
                    u64::from(d.stride),
                ));
            }
        }
        let off = self.member_bit_offset(r.var, act, base)?;
        Ok((base + off, dims))
    }

    /// The base of an array DEFINED with iSUB subscripts.
    fn isub_base(&self, v: VarId) -> Option<Ref> {
        let root = &self.module.vars[self.module.vars[v as usize].root as usize];
        match &root.storage {
            Storage::Defined(base, _) if base.subs.iter().flatten().any(has_isub) => {
                Some((**base).clone())
            }
            _ => None,
        }
    }

    /// The bit address of a reference's element (free subscripts: the
    /// first element).
    fn element_or_start(&mut self, r: &Ref, act: usize) -> R<(u64, Dims)> {
        // iSUB defining: the element of the base its subscripts select
        if let Some(base) = self.isub_base(r.var) {
            let (_, dims) = self.base_of(r, act)?;
            let mut subs = Vec::new();
            for (k, s) in r.subs.iter().enumerate() {
                subs.push(match s {
                    Some(e) => self.eval(e, act)?.as_i64(),
                    None => dims.get(k).map_or(1, |d| d.0),
                });
            }
            self.isubs.push(subs);
            let at = self.element_or_start(&base, act);
            self.isubs.pop();
            return Ok((at?.0, dims));
        }
        let (mut bit, dims) = self.base_of(r, act)?;
        let enabled = true;
        for (k, s) in r.subs.iter().enumerate() {
            let (lb, ub, stride) = dims.get(k).copied().unwrap_or((1, 1, 0));
            let i = match s {
                Some(e) => self.eval(e, act)?.as_i64(),
                None => lb,
            };
            if enabled && (i < lb || i > ub) {
                if self.enabled_now(act, cond::SUBSCRIPTRANGE) {
                    self.raise(Cond::SubscriptRange, 520, act)?;
                    self.raise(Cond::Error, 520, act)?;
                }
                if i < lb || i > ub + 1_000_000 {
                    return err(format!("subscript {i} out of range {lb}:{ub}"));
                }
            }
            bit = (bit as i64 + (i - lb) * stride as i64) as u64;
        }
        Ok((bit, dims))
    }

    fn enabled_now(&self, _act: usize, c: u16) -> bool {
        CUR_ENABLED.with(|e| e.get() & c != 0)
    }

    /// Location of a scalar reference.
    pub fn locate(&mut self, r: &Ref, act: usize) -> R<Loc> {
        let (bit, _) = self.element_or_start(r, act)?;
        let rb = self.refer_base_for_len(r, act)?;
        let var = &self.module.vars[r.var as usize];
        let ty = self
            .resolve_ty(var.ty.as_ref(), r.var, act, rb)?
            .ok_or_else(|| Intr::Fatal(format!("{} is a structure", var.name)))?;
        Ok(Loc { bit, ty })
    }

    // ---- loading and storing scalars -------------------------------------

    pub fn load(&self, loc: &Loc) -> R<Value> {
        let addr = (loc.bit / 8) as u32;
        Ok(match &loc.ty {
            Ty::Fixed {
                dec: true, p, q, ..
            } => {
                let n = usize::from(*p) / 2 + 1;
                let b = self.read(addr, n)?;
                let (mag, neg) = zeus_decimal::packed::decode_unchecked(&b);
                let m = mag as i128;
                Value::fixed(if neg { -m } else { m }, *q, true)
            }
            Ty::Fixed {
                dec: false,
                q,
                unsigned,
                ..
            } => {
                let (n, _) = loc.ty.storage(true);
                let b = self.read(addr, n as usize)?;
                let v = zeus_decimal::binary::read(&b, !unsigned);
                Value::fixed(v, *q, false)
            }
            Ty::Float { .. } => {
                let n = crate::types::float_bytes(&loc.ty);
                Value::Float(value::hfp_decode(&self.read(addr, n as usize)?))
            }
            Ty::Char { len, var } => {
                let n = match len {
                    Ext::Const(n) => *n as usize,
                    _ => 0,
                };
                match var {
                    Varying::Fixed => Value::Char(self.read(addr, n)?),
                    Varying::Varying => {
                        let l = self.read(addr, 2)?;
                        let cur = (usize::from(u16::from_be_bytes([l[0], l[1]]))).min(n);
                        Value::Char(self.read(addr + 2, cur)?)
                    }
                    Varying::VaryingZ => {
                        let b = self.read(addr, n + 1)?;
                        let end = b.iter().position(|&x| x == 0).unwrap_or(n);
                        Value::Char(b[..end].to_vec())
                    }
                }
            }
            Ty::Bit { len, var, .. } => {
                let n = match len {
                    Ext::Const(n) => u64::from(*n),
                    _ => 0,
                };
                match var {
                    Varying::Fixed => Value::Bit(self.read_bits(loc.bit, n)?),
                    _ => {
                        let l = self.read(addr, 2)?;
                        let cur = u64::from(u16::from_be_bytes([l[0], l[1]])).min(n);
                        Value::Bit(self.read_bits(loc.bit + 16, cur)?)
                    }
                }
            }
            Ty::Pic(p) => Value::Char(self.read(addr, p.len)?),
            Ty::Complex(t) => {
                let (n, _) = t.storage(true);
                let re = self.load(&Loc {
                    bit: loc.bit,
                    ty: (**t).clone(),
                })?;
                let im = self.load(&Loc {
                    bit: loc.bit + u64::from(n) * 8,
                    ty: (**t).clone(),
                })?;
                Value::cplx(re, im)
            }
            // GRAPHIC, WIDECHAR: the stored bytes
            Ty::Wchar { len, var, .. } => {
                let n = match len {
                    Ext::Const(n) => *n as usize,
                    _ => 0,
                };
                match var {
                    Varying::Fixed => Value::Char(self.read(addr, 2 * n)?),
                    _ => {
                        let l = self.read(addr, 2)?;
                        let cur = (usize::from(u16::from_be_bytes([l[0], l[1]]))).min(n);
                        Value::Char(self.read(addr + 2, 2 * cur)?)
                    }
                }
            }
            Ty::Pointer | Ty::Offset(_) => {
                let b = self.read(addr, 4)?;
                Value::Ptr(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            }
            // an area: its header and the bytes in use
            Ty::Area(cap) => {
                let h = self.read(addr, AREA_HEADER as usize)?;
                let used = u32::from_be_bytes([h[4], h[5], h[6], h[7]]).min(*cap);
                let mut b = h.clone();
                b.extend(self.read(addr + AREA_HEADER, used as usize)?);
                Value::Char(b)
            }
            Ty::File => {
                let b = self.read(addr, 4)?;
                Value::File(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            }
            Ty::Label | Ty::Format => {
                let b = self.read(addr, 8)?;
                Value::Label(
                    u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
                    u32::from_be_bytes([b[4], b[5], b[6], b[7]]) as usize,
                )
            }
            Ty::Entry => {
                let b = self.read(addr, 8)?;
                Value::Entry(
                    u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
                    u32::from_be_bytes([b[4], b[5], b[6], b[7]]) as usize,
                )
            }
        })
    }

    /// Stores a value already converted to the location's type.
    pub fn store(&mut self, loc: &Loc, v: &Value) -> R<()> {
        let addr = (loc.bit / 8) as u32;
        match (&loc.ty, v) {
            (Ty::Fixed { dec: true, p, .. }, Value::Fixed { m, .. }) => {
                let n = usize::from(*p) / 2 + 1;
                let mut b = vec![0u8; n];
                let cap = 10u128.pow((2 * n - 1) as u32);
                let mag = m.unsigned_abs() % cap;
                zeus_decimal::packed::encode(&mut b, mag, if *m < 0 { 0x0D } else { 0x0C });
                self.write(addr, &b)?;
            }
            (Ty::Fixed { dec: false, .. }, Value::Fixed { m, .. }) => {
                let (n, _) = loc.ty.storage(true);
                let mut b = vec![0u8; n as usize];
                zeus_decimal::binary::write(&mut b, *m);
                self.write(addr, &b)?;
            }
            (Ty::Float { .. }, Value::Float(f)) => {
                let n = crate::types::float_bytes(&loc.ty);
                let mut b = vec![0u8; n as usize];
                value::hfp_encode(*f, &mut b);
                self.write(addr, &b)?;
            }
            (Ty::Char { len, var }, Value::Char(s)) => {
                let n = match len {
                    Ext::Const(n) => *n as usize,
                    _ => s.len(),
                };
                match var {
                    Varying::Fixed => {
                        let mut b = s.clone();
                        b.resize(n, SPACE);
                        self.write(addr, &b)?;
                    }
                    Varying::Varying => {
                        let cur = s.len().min(n);
                        self.write(addr, &(cur as u16).to_be_bytes())?;
                        self.write(addr + 2, &s[..cur])?;
                    }
                    Varying::VaryingZ => {
                        let cur = s.len().min(n);
                        let mut b = s[..cur].to_vec();
                        b.push(0);
                        self.write(addr, &b)?;
                    }
                }
            }
            (Ty::Bit { len, var, .. }, Value::Bit(bits)) => {
                let n = match len {
                    Ext::Const(n) => *n as usize,
                    _ => bits.len(),
                };
                match var {
                    Varying::Fixed => {
                        let mut b = bits.clone();
                        b.resize(n, false);
                        self.write_bits(loc.bit, &b)?;
                    }
                    _ => {
                        let cur = bits.len().min(n);
                        self.write(addr, &(cur as u16).to_be_bytes())?;
                        self.write_bits(loc.bit + 16, &bits[..cur])?;
                    }
                }
            }
            (Ty::Pic(p), Value::Char(s)) => {
                let mut b = s.clone();
                b.resize(p.len, SPACE);
                self.write(addr, &b)?;
            }
            (Ty::Complex(t), Value::Cplx(c)) => {
                let (n, _) = t.storage(true);
                self.store(
                    &Loc {
                        bit: loc.bit,
                        ty: (**t).clone(),
                    },
                    &c.0,
                )?;
                self.store(
                    &Loc {
                        bit: loc.bit + u64::from(n) * 8,
                        ty: (**t).clone(),
                    },
                    &c.1,
                )?;
            }
            (Ty::Wchar { len, var, graphic }, Value::Char(s)) => {
                let n = match len {
                    Ext::Const(n) => *n as usize,
                    _ => s.len() / 2,
                };
                let cur = (s.len() / 2).min(n);
                match var {
                    Varying::Fixed => {
                        let mut b = s[..2 * cur].to_vec();
                        let pad = conv::wide_space(*graphic);
                        while b.len() < 2 * n {
                            b.extend(pad);
                        }
                        self.write(addr, &b)?;
                    }
                    _ => {
                        self.write(addr, &(cur as u16).to_be_bytes())?;
                        self.write(addr + 2, &s[..2 * cur])?;
                    }
                }
            }
            (Ty::Pointer | Ty::Offset(_), Value::Ptr(p)) => self.write(addr, &p.to_be_bytes())?,
            (Ty::Area(cap), Value::Char(b)) => {
                let used = b
                    .get(4..8)
                    .map_or(0, |h| u32::from_be_bytes([h[0], h[1], h[2], h[3]]));
                if used > *cap {
                    return Err(Intr::Fatal(format!(
                        "AREA: {used} bytes in use do not fit an area of {cap}"
                    )));
                }
                let mut h = vec![0u8; AREA_HEADER as usize];
                h[..4].copy_from_slice(&cap.to_be_bytes());
                h[4..8].copy_from_slice(&used.to_be_bytes());
                self.write(addr, &h)?;
                let data = b.get(AREA_HEADER as usize..).unwrap_or(&[]);
                let n = data.len().min(used as usize);
                self.write(addr + AREA_HEADER, &data[..n])?;
            }
            (Ty::File, Value::File(f)) => self.write(addr, &f.to_be_bytes())?,
            (Ty::Label | Ty::Format, Value::Label(l, a)) => {
                let mut b = l.to_be_bytes().to_vec();
                b.extend((*a as u32).to_be_bytes());
                self.write(addr, &b)?;
            }
            (Ty::Entry, Value::Entry(p, a)) => {
                let mut b = p.to_be_bytes().to_vec();
                b.extend((*a as u32).to_be_bytes());
                self.write(addr, &b)?;
            }
            (t, v) => return err(format!("cannot store {v:?} as {t:?}")),
        }
        Ok(())
    }

    /// Converts and stores a value of type `from` into a location.
    pub fn assign_loc(&mut self, loc: &Loc, v: Value, from: &Ty, act: usize) -> R<()> {
        let v = self.convert(v, from, &loc.ty, act)?;
        self.store(loc, &v)
    }

    // ---- aggregates ----------------------------------------------------

    /// The elements (scalar references) of an aggregate reference in
    /// row-major order, structures in member order.
    pub fn elements(&mut self, r: &Ref, act: usize) -> R<Vec<Loc>> {
        let m = self.module.clone();
        let var = &m.vars[r.var as usize];
        if var.ty.is_none() {
            let mut out = Vec::new();
            // a structure: each element of its dimensions, members in
            // order (array of structures: element by element)
            let free: Vec<usize> = r
                .subs
                .iter()
                .enumerate()
                .filter(|(_, s)| s.is_none())
                .map(|(i, _)| i)
                .collect();
            let (_, dims) = self.base_of(r, act)?;
            let combos = index_space(
                &free
                    .iter()
                    .map(|&k| (dims[k].0, dims[k].1))
                    .collect::<Vec<_>>(),
            );
            for combo in combos {
                let mut subs = r.subs.clone();
                for (k, i) in free.iter().zip(&combo) {
                    subs[*k] = Some(crate::sema::expr::mk(
                        Ek::Fixed(i128::from(*i), 0),
                        Ty::fixed_bin(31, 0),
                    ));
                }
                for &mem in &var.members {
                    let n = m.vars[mem as usize].dims.len();
                    let mut ms = subs.clone();
                    ms.resize(n, None);
                    let mr = Ref {
                        var: mem,
                        subs: ms,
                        locator: r.locator.clone(),
                    };
                    out.extend(self.elements(&mr, act)?);
                }
            }
            return Ok(out);
        }
        // iSUB defining: each element located through the base
        if self.isub_base(r.var).is_some() {
            let (_, dims) = self.base_of(r, act)?;
            let free: Vec<usize> = (0..r.subs.len()).filter(|&k| r.subs[k].is_none()).collect();
            let combos = index_space(
                &free
                    .iter()
                    .map(|&k| (dims[k].0, dims[k].1))
                    .collect::<Vec<_>>(),
            );
            let mut out = Vec::new();
            for c in combos {
                let mut subs = r.subs.clone();
                for (k, i) in free.iter().zip(&c) {
                    subs[*k] = Some(crate::sema::expr::mk(
                        Ek::Fixed(i128::from(*i), 0),
                        Ty::fixed_bin(31, 0),
                    ));
                }
                out.push(self.locate(
                    &Ref {
                        var: r.var,
                        subs,
                        locator: r.locator.clone(),
                    },
                    act,
                )?);
            }
            return Ok(out);
        }
        let (base, dims) = self.base_of(r, act)?;
        let rb = self.refer_base_for_len(r, act)?;
        let ty = self.resolve_ty(var.ty.as_ref(), r.var, act, rb)?.unwrap();
        let mut fixed_off: i64 = 0;
        let mut free = Vec::new();
        for (k, s) in r.subs.iter().enumerate() {
            let (lb, _, stride) = dims[k];
            match s {
                Some(e) => fixed_off += (self.eval(e, act)?.as_i64() - lb) * stride as i64,
                None => free.push(k),
            }
        }
        let combos = index_space(
            &free
                .iter()
                .map(|&k| (dims[k].0, dims[k].1))
                .collect::<Vec<_>>(),
        );
        Ok(combos
            .into_iter()
            .map(|c| {
                let mut off = fixed_off;
                for (k, i) in free.iter().zip(&c) {
                    off += (i - dims[*k].0) * dims[*k].2 as i64;
                }
                Loc {
                    bit: (base as i64 + off) as u64,
                    ty: ty.clone(),
                }
            })
            .collect())
    }

    /// The bytes of an aggregate (or scalar) reference's storage.
    pub fn aggregate_bytes(&mut self, r: &Ref, act: usize) -> R<Vec<u8>> {
        let var = self.module.vars[r.var as usize].clone();
        let root_dims = self.module.vars[var.root as usize].dims.len();
        if var.ty.is_some() && !r.subs.iter().any(|s| s.is_none()) {
            let loc = self.locate(r, act)?;
            let (n, _) = loc.ty.storage(true);
            return self.read((loc.bit / 8) as u32, n as usize);
        }
        // contiguous: from the start for the size of the whole
        let (bit, _) = self.element_or_start(r, act)?;
        let rb = self.refer_base(r, act)?;
        let size = if var.parent.is_none() {
            self.var_size(r.var, act, rb)? as usize
        } else {
            let mut n = u64::from(var.size);
            for d in &var.dims[root_dims.min(var.dims.len())..] {
                n *= (self.bound(d.ub, act, r.var, rb)? - self.bound(d.lb, act, r.var, rb)? + 1)
                    .max(0) as u64;
            }
            n as usize
        };
        self.read((bit / 8) as u32, size)
    }

    // ---- statements ----------------------------------------------------

    fn run_list(&mut self, list: &List, act: usize) -> R<()> {
        let mut i = 0;
        while i < list.stmts.len() {
            let node = &list.stmts[i];
            CUR_ENABLED.with(|e| e.set(node.enabled));
            CUR_LINE.with(|e| e.set(node.line));
            match self.exec(&node.stmt, act) {
                Ok(()) => i += 1,
                Err(Intr::Goto(l, a))
                    if a == act && self.module.labels[l as usize].list == list.id =>
                {
                    i = self.module.labels[l as usize].index;
                }
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    fn exec(&mut self, s: &Stmt, act: usize) -> R<()> {
        if let Some(sys) = self.sys.as_mut() {
            sys.count_tick();
        }
        match s {
            Stmt::Null => Ok(()),
            Stmt::Exec(x) => self.exec_stmt(x, act),
            Stmt::Assign(targets, value) => {
                // subscripts of the targets first, then the value
                let mut locs = Vec::new();
                for t in targets {
                    locs.push(self.target_loc(t, act)?);
                }
                let v = self.eval(value, act)?;
                for (t, l) in targets.iter().zip(locs) {
                    self.assign_target(t, l, v.clone(), &value.ty, act)?;
                }
                Ok(())
            }
            Stmt::ArrayAssign {
                targets,
                value,
                shape,
                rank,
            } => {
                let (_, dims) = self.base_of(shape, act)?;
                let free: Vec<(i64, i64)> = shape
                    .subs
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.is_none())
                    .map(|(k, _)| (dims[k].0, dims[k].1))
                    .collect();
                let _ = rank;
                let save = std::mem::take(&mut self.acts[act].index);
                for combo in index_space(&free) {
                    self.acts[act].index = combo;
                    let v = self.eval(value, act)?;
                    for t in targets {
                        let l = self.target_loc(t, act)?;
                        self.assign_target(t, l, v.clone(), &value.ty, act)?;
                    }
                }
                self.acts[act].index = save;
                Ok(())
            }
            Stmt::If(c, t, e) => {
                if self.eval(c, act)?.truth() {
                    self.run_list(t, act)
                } else {
                    self.run_list(e, act)
                }
            }
            Stmt::Group(l) => self.run_list(l, act),
            Stmt::Do(id, kind, body) => self.do_loop(*id, kind, body, act),
            Stmt::Select(subject, whens, other) => {
                let save = self.acts[act].subject.take();
                if let Some(sx) = subject {
                    let v = self.eval(sx, act)?;
                    self.acts[act].subject = Some(v);
                }
                let mut chosen = None;
                'w: for (conds, unit) in whens {
                    for c in conds {
                        if self.eval(c, act)?.truth() {
                            chosen = Some(unit);
                            break 'w;
                        }
                    }
                }
                self.acts[act].subject = save;
                match chosen {
                    Some(u) => self.run_list(u, act),
                    None if other.id != 0 => self.run_list(other, act),
                    None => {
                        self.raise(Cond::Error, 3, act)?;
                        Ok(())
                    }
                }
            }
            Stmt::Call(callee, args) => {
                self.call(callee, args, act, None)?;
                Ok(())
            }
            Stmt::CallBuiltin(b, args) => self.call_builtin(*b, args, act),
            Stmt::Return(v) => {
                let v = match v {
                    Some(e) => Some(self.eval(e, act)?),
                    None => None,
                };
                Err(Intr::Return(v))
            }
            Stmt::GoTo(e) => match self.eval(e, act)? {
                Value::Label(l, a) => {
                    let target = if let Ek::Label(_) = &*e.k {
                        self.act_of(act, self.module.labels[l as usize].block)?
                    } else {
                        a
                    };
                    Err(Intr::Goto(l, target))
                }
                v => err(format!("GO TO a non-label value {v:?}")),
            },
            Stmt::Leave(id) => Err(Intr::Leave(*id)),
            Stmt::Iterate(id) => Err(Intr::Iterate(*id)),
            Stmt::On(conds, unit, _snap) => {
                for c in conds {
                    let entry = unit.map(|b| (b, act));
                    if let Some(x) = self.acts[act].on.iter_mut().find(|x| x.0 == *c) {
                        x.1 = entry;
                    } else {
                        self.acts[act].on.push((c.clone(), entry));
                    }
                }
                Ok(())
            }
            Stmt::Revert(conds) => {
                self.acts[act].on.retain(|x| !conds.contains(&x.0));
                Ok(())
            }
            Stmt::Signal(c) => {
                let code = signal_code(c);
                let enabled = match c {
                    Cond::Size => self.enabled_now(act, cond::SIZE),
                    Cond::StringRange => self.enabled_now(act, cond::STRINGRANGE),
                    Cond::StringSize => self.enabled_now(act, cond::STRINGSIZE),
                    Cond::SubscriptRange => self.enabled_now(act, cond::SUBSCRIPTRANGE),
                    _ => true,
                };
                if enabled {
                    self.signal(c.clone(), code, act, true)?;
                }
                Ok(())
            }
            Stmt::Resignal => {
                // continue the search beyond the current handler
                let mut a = Some(act);
                while let Some(i) = a {
                    if let Some((c, from)) = self.acts[i].handling.clone() {
                        let code = self.acts[i].cond.as_ref().map_or(0, |x| x.code);
                        return self
                            .search_from(c, code, from.wrapping_sub(1), act, false)
                            .map(|_| ());
                    }
                    a = self.acts[i].parent;
                }
                Ok(())
            }
            Stmt::Stop => Err(Intr::Stop),
            Stmt::Exit => Err(Intr::Stop),
            Stmt::Put(s) => self.put(s, act),
            Stmt::Get(s) => self.get(s, act),
            Stmt::Open(specs) => {
                for s in specs {
                    self.open(s, act)?;
                }
                Ok(())
            }
            Stmt::Close(files) => {
                for f in files {
                    if let Value::File(id) = self.eval(f, act)? {
                        self.close(id)?;
                    }
                }
                Ok(())
            }
            Stmt::Read(r) => self.read_stmt(r, act),
            Stmt::Write(r) => self.write_stmt(r, act),
            Stmt::Rewrite(r) => self.rewrite_stmt(r, act),
            Stmt::Delete(r) => self.delete_stmt(r, act),
            Stmt::Allocate(items) => {
                for (v, set, given, area) in items {
                    if let Storage::Controlled(_, slots) = &self.module.vars[*v as usize].storage {
                        let slots = slots.clone();
                        self.allocate_controlled(*v, &slots, given, act)?;
                        continue;
                    }
                    // REFER extents: their initial values size the generation
                    let m = self.module.clone();
                    let refers: Vec<usize> = (0..m.refers.len())
                        .filter(|&n| m.vars[m.refers[n].object as usize].root == *v)
                        .collect();
                    let mut rv = vec![0i64; m.refers.len()];
                    for &n in &refers {
                        let a = self.act_of(act, m.vars[*v as usize].block).unwrap_or(act);
                        rv[n] = self.eval(&m.refers[n].init, a)?.as_i64();
                    }
                    REFER_OVERRIDE.with(|o| *o.borrow_mut() = Some(rv.clone()));
                    let size = self.var_size(*v, act, None);
                    REFER_OVERRIDE.with(|o| *o.borrow_mut() = None);
                    let size = size?;
                    let p = match area {
                        Some(a) => match self.area_alloc(a, size as u32, act)? {
                            Some(p) => p,
                            None => continue,
                        },
                        None => self.heap_alloc(size as u32),
                    };
                    for &n in &refers {
                        let obj = &m.vars[m.refers[n].object as usize];
                        let loc = Loc {
                            bit: u64::from(p) * 8 + obj.bit_offset,
                            ty: obj.ty.clone().unwrap(),
                        };
                        self.store(&loc, &Value::int(rv[n]))?;
                    }
                    let ptr = Value::Ptr(p);
                    match set {
                        Some(t) => {
                            let l = self.target_loc(t, act)?;
                            self.assign_target(t, l, ptr, &Ty::Pointer, act)?;
                        }
                        None => {
                            // the declared locator
                            let Storage::Based(Some(loc)) = &self.module.vars[*v as usize].storage
                            else {
                                return err("ALLOCATE of a BASED variable without a locator");
                            };
                            let loc = loc.clone();
                            let Ek::Load(r) = &*loc.k else {
                                return err("ALLOCATE: the locator is not a variable");
                            };
                            let l = self.locate(r, act)?;
                            self.assign_loc(&l, ptr, &Ty::Pointer, act)?;
                        }
                    }
                    // INITIAL of the based variable and its members
                    let m = self.module.clone();
                    for init in &m.static_inits {
                        if m.vars[init.var as usize].root == *v
                            && matches!(m.vars[*v as usize].storage, Storage::Based(_))
                        {
                            self.initialize_at(init, act, Some(p))?;
                        }
                    }
                }
                Ok(())
            }
            Stmt::Fetch(items) => {
                for (name, title) in items {
                    let module = match title {
                        Some(t) => match self.eval(t, act)? {
                            Value::Char(c) => {
                                zeus_cobol::ebcdic::decode(&c).trim().to_ascii_uppercase()
                            }
                            _ => name.clone(),
                        },
                        None => name.clone(),
                    };
                    let known = self.sys().load_program(&module)
                        || self.env.linker.as_ref().is_some_and(|l| l.load(&module));
                    if !known {
                        // the module cannot be loaded: ERROR
                        self.raise(Cond::Error, 9251, act)?;
                        continue;
                    }
                    if module != *name {
                        self.titles.insert(name.clone(), module);
                    }
                }
                Ok(())
            }
            Stmt::Release(names) => {
                let names: Vec<String> = if names.is_empty() {
                    self.titles.keys().cloned().collect()
                } else {
                    names.clone()
                };
                for n in names {
                    let module = self.titles.remove(&n).unwrap_or(n);
                    if let Some(l) = &self.env.linker {
                        l.release(&module);
                    }
                }
                Ok(())
            }
            Stmt::Free(items) => {
                for (v, _) in items {
                    let root = self.module.vars[*v as usize].root;
                    if let Storage::Controlled(..) = self.module.vars[root as usize].storage {
                        let anchor = self.anchor_addr(root, act)?;
                        let g = self.generation(anchor)?;
                        if g == 0 {
                            return err(format!(
                                "FREE of {}, which is not allocated",
                                self.module.vars[root as usize].name
                            ));
                        }
                        let prev = self.word_at(g)?;
                        self.write(anchor, &prev.to_be_bytes())?;
                    }
                }
                Ok(())
            }
            Stmt::Display(v, reply) => {
                let s = self.eval(v, act)?;
                let Value::Char(b) = s else {
                    return err("DISPLAY of a non-character value");
                };
                let _ = self.env.msgout.write_all(&zeus_cobol::ebcdic::to_host(&b));
                let _ = self.env.msgout.write_all(b"\n");
                let _ = self.env.msgout.flush();
                if let Some((t, ty)) = reply {
                    let line = self
                        .env
                        .stdin
                        .as_mut()
                        .and_then(|l| {
                            if l.is_empty() {
                                None
                            } else {
                                Some(l.remove(0))
                            }
                        })
                        .unwrap_or_default();
                    let l = self.target_loc(t, act)?;
                    let v = Value::Char(zeus_cobol::ebcdic::encode(&line));
                    self.assign_target(t, l, v, &crate::sema::expr::char_any(), act)?;
                    let _ = ty;
                }
                Ok(())
            }
            Stmt::Delay(e) => {
                let ms = self.eval(e, act)?.as_i64().clamp(0, 86_400_000);
                std::thread::sleep(std::time::Duration::from_millis(ms as u64));
                Ok(())
            }
            Stmt::Begin(b) => {
                let a = self.activate(*b, Some(act), Vec::new(), None)?;
                let body = self.module.blocks[*b as usize].body.clone();
                let r = self.run_list(&body, a);
                self.deactivate(a);
                r
            }
        }
    }

    fn do_loop(&mut self, id: LoopId, kind: &DoKind, body: &List, act: usize) -> R<()> {
        // runs the body once; Ok(false) when LEAVE
        let once = |me: &mut Machine| -> R<bool> {
            match me.run_list(body, act) {
                Ok(()) => Ok(true),
                Err(Intr::Leave(l)) if l == id => Ok(false),
                Err(Intr::Iterate(l)) if l == id => Ok(true),
                Err(e) => Err(e),
            }
        };
        match kind {
            DoKind::Group => {
                once(self)?;
                Ok(())
            }
            DoKind::Loop { while_, until } => loop {
                if let Some(w) = while_
                    && !self.eval(w, act)?.truth()
                {
                    return Ok(());
                }
                if !once(self)? {
                    return Ok(());
                }
                if let Some(u) = until
                    && self.eval(u, act)?.truth()
                {
                    return Ok(());
                }
            },
            DoKind::Iter { var, var_ty, specs } => {
                for s in specs {
                    let loc = self.target_loc(var, act)?;
                    let start = self.eval(&s.start, act)?;
                    let to = match &s.to {
                        Some(e) => Some((self.eval(e, act)?, e.ty.clone())),
                        None => None,
                    };
                    let by = match &s.by {
                        Some(e) => Some((self.eval(e, act)?, e.ty.clone())),
                        None if to.is_some() => {
                            Some((Value::fixed(1, 0, true), Ty::fixed_dec(1, 0)))
                        }
                        None => None,
                    };
                    self.assign_target(var, loc.clone(), start, var_ty, act)?;
                    let single = s.to.is_none() && s.by.is_none() && s.repeat.is_none();
                    loop {
                        let cur = self.target_value(var, &loc, act)?;
                        if let (Some((tv, tt)), Some((bv, _))) = (&to, &by) {
                            let neg = bv.as_f64() < 0.0;
                            let past = if neg {
                                self.cmp_arith(&cur, var_ty, tv, tt)? == std::cmp::Ordering::Less
                            } else {
                                self.cmp_arith(&cur, var_ty, tv, tt)? == std::cmp::Ordering::Greater
                            };
                            if past {
                                break;
                            }
                        }
                        if let Some(w) = &s.while_
                            && !self.eval(w, act)?.truth()
                        {
                            break;
                        }
                        if !once(self)? {
                            return Ok(());
                        }
                        if let Some(u) = &s.until
                            && self.eval(u, act)?.truth()
                        {
                            break;
                        }
                        if single {
                            break;
                        }
                        if let Some(rep) = &s.repeat {
                            let v = self.eval(rep, act)?;
                            self.assign_target(var, loc.clone(), v, &rep.ty, act)?;
                        } else if let Some((bv, bt)) = &by {
                            let cur = self.target_value(var, &loc, act)?;
                            let (v, t) = self.add_values(&cur, var_ty, bv, bt, act)?;
                            self.assign_target(var, loc.clone(), v, &t, act)?;
                        }
                    }
                }
                Ok(())
            }
        }
    }

    // ---- targets ---------------------------------------------------------

    /// Evaluates the location parts of a target (subscripts, SUBSTR
    /// positions).
    fn target_loc(&mut self, t: &Target, act: usize) -> R<TargetLoc> {
        Ok(match t {
            Target::Ref(r) => TargetLoc::Loc(self.locate(r, act)?),
            Target::Substr(r, s, n) => {
                let loc = self.locate(r, act)?;
                let s = self.eval(s, act)?.as_i64();
                let n = match n {
                    Some(n) => Some(self.eval(n, act)?.as_i64()),
                    None => None,
                };
                TargetLoc::Substr(loc, s, n)
            }
            Target::Unspec(r) | Target::String(r) => TargetLoc::Bytes(r.clone()),
        })
    }

    fn target_value(&mut self, _t: &Target, l: &TargetLoc, _act: usize) -> R<Value> {
        match l {
            TargetLoc::Loc(loc) => self.load(loc),
            _ => err("pseudovariable as a DO control variable"),
        }
    }

    pub(crate) fn assign_target(
        &mut self,
        t: &Target,
        l: TargetLoc,
        v: Value,
        from: &Ty,
        act: usize,
    ) -> R<()> {
        match l {
            TargetLoc::Loc(loc) => self.assign_loc(&loc, v, from, act),
            TargetLoc::Substr(loc, s, n) => {
                let cur = self.load(&loc)?;
                let bit = loc.ty.is_bit();
                let len = match &cur {
                    Value::Char(c) => c.len(),
                    Value::Bit(b) => b.len(),
                    _ => 0,
                } as i64;
                let n = n.unwrap_or(len - s + 1);
                if s < 1 || n < 0 || s + n - 1 > len {
                    if self.enabled_now(act, cond::STRINGRANGE) {
                        self.raise(Cond::StringRange, 350, act)?;
                    }
                    if s < 1 || s + n - 1 > len {
                        return Ok(());
                    }
                }
                let (s, n) = ((s - 1) as usize, n.max(0) as usize);
                if bit {
                    let v = self.convert(v, from, &Ty::bit(n as u32), act)?;
                    let (Value::Bit(mut whole), Value::Bit(part)) = (cur, v) else {
                        return err("SUBSTR type");
                    };
                    whole[s..s + n].copy_from_slice(&part[..n]);
                    self.store(&loc, &Value::Bit(whole))
                } else {
                    let v = self.convert(v, from, &Ty::char(n as u32), act)?;
                    let (Value::Char(mut whole), Value::Char(part)) = (cur, v) else {
                        return err("SUBSTR type");
                    };
                    whole[s..s + n].copy_from_slice(&part[..n]);
                    self.store(&loc, &Value::Char(whole))
                }
            }
            TargetLoc::Bytes(r) => {
                let unspec = matches!(t, Target::Unspec(_));
                let start = self.element_or_start(&r, act)?.0;
                let size = self.aggregate_bytes(&r, act)?.len();
                if unspec {
                    let v = self.convert(v, from, &Ty::bit(size as u32 * 8), act)?;
                    let Value::Bit(bits) = v else {
                        return err("UNSPEC");
                    };
                    self.write_bits(start, &bits)
                } else {
                    let v = self.convert(v, from, &Ty::char(size as u32), act)?;
                    let Value::Char(b) = v else {
                        return err("STRING");
                    };
                    self.write((start / 8) as u32, &b)
                }
            }
        }
    }

    // ---- calls -----------------------------------------------------------

    /// Binds arguments and runs a procedure; its value (functions).
    fn call(
        &mut self,
        callee: &Callee,
        args: &[Arg],
        act: usize,
        rt: Option<&Ty>,
    ) -> R<Option<Value>> {
        // the activation the static parent is found from: the caller's,
        // or the one an entry value was taken in
        let (pid, entry, env) = match callee {
            Callee::Proc(p) => (*p, None, act),
            Callee::Entry(p, k) => (*p, Some(*k), act),
            Callee::External(name) => return self.call_external(name, args, act, rt),
            Callee::Var(e) => match self.eval(e, act)? {
                Value::Entry(p, a) => (p, None, a),
                _ => return err("call of a non-entry value"),
            },
        };
        let m = self.module.clone();
        let proc = &m.procs[pid as usize];
        let (params, start) = match entry {
            Some(k) => (proc.entries[k].1.clone(), proc.entries[k].2),
            None => (proc.params.clone(), 0),
        };
        // bind arguments (evaluated in the caller)
        let block = proc.block;
        let nparams = m
            .vars
            .iter()
            .filter(|v| v.block == block && matches!(v.storage, Storage::Param(_)))
            .count()
            .max(params.len());
        let mut bindings: Vec<Option<Binding>> = vec![None; nparams];
        let mut dummies = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let Some(&pv) = params.get(i) else { break };
            // a CONTROLLED parameter: the anchor of the argument
            if let Storage::Controlled(c, _) = m.vars[pv as usize].storage
                && c & crate::ir::CTL_PARAM != 0
            {
                let k = (c & !crate::ir::CTL_PARAM) as usize;
                let Arg::Ref(r) = a else {
                    return err("the argument of a CONTROLLED parameter is not a variable");
                };
                let root = m.vars[r.var as usize].root;
                let anchor = self.anchor_addr(root, act)?;
                if k < bindings.len() {
                    bindings[k] = Some(Binding {
                        bit: u64::from(anchor) * 8,
                        dims: Vec::new(),
                        len: None,
                    });
                }
                continue;
            }
            let Storage::Param(k) = m.vars[pv as usize].storage else {
                continue;
            };
            let b = self.bind_arg(a, pv, act, &mut dummies)?;
            if (k as usize) < bindings.len() {
                bindings[k as usize] = b;
            }
        }
        self.depth += 1;
        if self.depth > 2000 {
            return err("procedure calls nested too deeply");
        }
        // the static parent: the activation of the containing block
        let parent = match m.blocks[block as usize].parent {
            Some(pb) if m.blocks[pb as usize].name != "*PACKAGE" => Some(self.act_of(env, pb)?),
            _ => None,
        };
        let a = self.activate(block, parent, bindings, proc.returns.clone());
        let a = match a {
            Ok(a) => a,
            Err(e) => {
                self.depth -= 1;
                return Err(e);
            }
        };
        let body = &m.blocks[block as usize].body;
        let r = if start > 0 {
            let tail = List {
                id: body.id,
                stmts: body.stmts[start..].to_vec(),
            };
            self.run_list(&tail, a)
        } else {
            self.run_list(body, a)
        };
        self.deactivate(a);
        self.depth -= 1;
        match r {
            Ok(()) => Ok(None),
            Err(Intr::Return(v)) => Ok(v),
            Err(e) => Err(e),
        }
    }

    /// The binding of an argument: the variable itself or a dummy.
    fn bind_arg(
        &mut self,
        a: &Arg,
        param: VarId,
        act: usize,
        dummies: &mut Vec<u32>,
    ) -> R<Option<Binding>> {
        let m = self.module.clone();
        let pvar = &m.vars[param as usize];
        Ok(match a {
            Arg::Omitted => None,
            Arg::Ref(r) => {
                let (bit, dims) = self.element_or_start(r, act)?;
                let free: Vec<(i64, i64, u64)> = r
                    .subs
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.is_none())
                    .map(|(k, _)| dims[k])
                    .collect();
                let rb = self.refer_base_for_len(r, act)?;
                let var = &m.vars[r.var as usize];
                let ty = self.resolve_ty(var.ty.as_ref(), r.var, act, rb)?;
                let len = ty.as_ref().and_then(|t| match t.str_len() {
                    Some(Ext::Const(n)) => Some(n),
                    _ => None,
                });
                let _ = pvar;
                Some(Binding {
                    bit,
                    dims: free,
                    len,
                })
            }
            Arg::Dummy(e) => {
                let v = self.eval(e, act)?;
                let ty = match &pvar.ty {
                    Some(t) if !matches!(t.str_len(), Some(Ext::Star)) => t.clone(),
                    _ => match (&e.ty, &v) {
                        (Ty::Char { var, .. }, Value::Char(c)) => Ty::Char {
                            len: Ext::Const(c.len() as u32),
                            var: *var,
                        },
                        (Ty::Bit { var, .. }, Value::Bit(b)) => Ty::Bit {
                            len: Ext::Const(b.len() as u32),
                            var: *var,
                            aligned: false,
                        },
                        (t, _) => t.clone(),
                    },
                };
                let (size, _) = ty.storage(true);
                let addr = self.stack_alloc(size.max(1));
                dummies.push(addr);
                let loc = Loc {
                    bit: u64::from(addr) * 8,
                    ty: ty.clone(),
                };
                let v = self.convert(v, &e.ty, &ty, act)?;
                self.store(&loc, &v)?;
                let len = match ty.str_len() {
                    Some(Ext::Const(n)) => Some(n),
                    _ => None,
                };
                Some(Binding {
                    bit: loc.bit,
                    dims: Vec::new(),
                    len,
                })
            }
        })
    }

    /// CALL of an external entry: a COBOL program of the run unit, or a
    /// procedure of another PL/I module (through the linker, with the run
    /// unit's storage lent to it). Arguments are passed by reference.
    fn call_external(
        &mut self,
        name: &str,
        args: &[Arg],
        act: usize,
        rt: Option<&Ty>,
    ) -> R<Option<Value>> {
        let mut refs = Vec::new();
        for a in args {
            refs.push(match a {
                Arg::Omitted => XRef::default(),
                Arg::Ref(r) => {
                    let (bit, dims) = self.element_or_start(r, act)?;
                    let len = self.aggregate_bytes(r, act)?.len();
                    let rb = self.refer_base_for_len(r, act)?;
                    let var = &self.module.vars[r.var as usize];
                    let ty = self.resolve_ty(var.ty.as_ref(), r.var, act, rb)?;
                    let free = r
                        .subs
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| s.is_none())
                        .map(|(k, _)| (dims[k].0, dims[k].1))
                        .collect();
                    XRef {
                        addr: Some((bit / 8) as usize),
                        len,
                        dims: free,
                        strlen: match ty.as_ref().and_then(Ty::str_len) {
                            Some(Ext::Const(n)) => Some(n),
                            _ => None,
                        },
                    }
                }
                Arg::Dummy(e) => {
                    let v = self.eval(e, act)?;
                    let ty = match (&e.ty, &v) {
                        (t, Value::Char(c)) if !matches!(t.str_len(), Some(Ext::Const(_))) => {
                            t.with_len(Ext::Const(c.len() as u32))
                        }
                        (t, _) => t.clone(),
                    };
                    let (n, _) = ty.storage(true);
                    let tmp = self.heap_alloc(n);
                    self.store(
                        &Loc {
                            bit: u64::from(tmp) * 8,
                            ty: ty.clone(),
                        },
                        &v,
                    )?;
                    XRef {
                        addr: Some(tmp as usize),
                        len: n as usize,
                        dims: Vec::new(),
                        strlen: match ty.str_len() {
                            Some(Ext::Const(n)) => Some(n),
                            _ => None,
                        },
                    }
                }
            });
        }
        // a fetched entry: its load module
        let titled = self.titles.get(name).cloned();
        let name = titled.as_deref().unwrap_or(name);
        // the callee may be another PL/I machine on this thread
        let (enabled, line) = (CUR_ENABLED.with(|c| c.get()), CUR_LINE.with(|c| c.get()));
        let _ = self.env.stdout.flush();
        // a COBOL program of the run unit, else a PL/I module
        let mut r = match self.sys().call_program(name, &refs) {
            Ok(Some(e)) => Some(Ok(e)),
            Ok(None) => None,
            Err(m) => Some(Err(m)),
        };
        // a program of the host (PLITDLI, ...)
        if r.is_none() {
            r = self.sys().call_native(name, &refs).map(|x| {
                x.map(|rc| XEnd {
                    rc,
                    ret: None,
                    stop: false,
                })
            });
        }
        if r.is_none()
            && let Some(l) = self.env.linker.clone()
        {
            let sys = self.sys.take().expect("storage of the run unit");
            let (back, x) = l.call(name, sys, &refs);
            self.sys = Some(back);
            r = x;
        }
        CUR_ENABLED.with(|c| c.set(enabled));
        CUR_LINE.with(|c| c.set(line));
        let Some(r) = r else {
            return err(format!("external procedure {name} not found"));
        };
        let end = match r {
            Ok(e) => e,
            Err(m) if m.starts_with(ABEND) => return Err(Intr::Terminate),
            Err(m) => return err(m),
        };
        self.rc = end.rc;
        if end.stop {
            return Err(Intr::Stop);
        }
        match (rt, end.ret) {
            (Some(t), Some(bytes)) => {
                let t = match t.str_len() {
                    Some(Ext::Const(_)) | None => t.clone(),
                    _ => {
                        let n = bytes.len() as u32;
                        t.with_len(Ext::Const(if t.varying() == crate::types::Var::Varying {
                            n.saturating_sub(2)
                        } else {
                            n
                        }))
                    }
                };
                let tmp = self.heap_alloc(bytes.len() as u32);
                self.write(tmp, &bytes)?;
                Ok(Some(self.load(&Loc {
                    bit: u64::from(tmp) * 8,
                    ty: t,
                })?))
            }
            (Some(_), None) => err(format!("{name} returned no value")),
            _ => Ok(None),
        }
    }

    /// Whether this module has external procedure (or entry) `name`.
    pub fn has_entry(&self, name: &str) -> bool {
        self.module
            .procs
            .iter()
            .any(|p| p.external && (p.name == name || p.entries.iter().any(|e| e.0 == name)))
    }

    /// Runs external procedure (or entry) `name` of this module for a
    /// caller of another module, with the arguments at their addresses in
    /// the run unit's storage (lent to this module). None: no such entry.
    pub fn call_entry(&mut self, name: &str, args: &[XRef]) -> Result<Option<XEnd>, String> {
        let m = self.module.clone();
        let name = name.to_ascii_uppercase();
        let found = m.procs.iter().enumerate().find_map(|(i, p)| {
            if !p.external {
                return None;
            }
            if p.name == name {
                return Some((i, p.params.clone(), 0));
            }
            p.entries
                .iter()
                .find(|e| e.0 == name)
                .map(|e| (i, e.1.clone(), e.2))
        });
        let Some((pid, params, start)) = found else {
            return Ok(None);
        };
        if !self.started {
            self.started = true;
            if let Err(e) = self.static_init() {
                return Err(self.report(e));
            }
        }
        let proc = &m.procs[pid];
        let nparams = m
            .vars
            .iter()
            .filter(|v| v.block == proc.block && matches!(v.storage, Storage::Param(_)))
            .count()
            .max(params.len());
        let mut bindings: Vec<Option<Binding>> = vec![None; nparams];
        for (i, x) in args.iter().enumerate() {
            let Some(&pv) = params.get(i) else { break };
            let Storage::Param(k) = m.vars[pv as usize].storage else {
                continue;
            };
            let Some(addr) = x.addr else { continue };
            let var = &m.vars[pv as usize];
            let star = var
                .dims
                .iter()
                .any(|d| d.lb == Bound::Star || d.ub == Bound::Star);
            let dims = if star && !x.dims.is_empty() {
                let elem = match &var.ty {
                    Some(t) => {
                        u64::from(
                            t.with_len(Ext::Const(x.strlen.unwrap_or(0)))
                                .storage(true)
                                .0,
                        ) * 8
                    }
                    None => u64::from(var.size) * 8,
                };
                let mut strides = vec![0u64; x.dims.len()];
                let mut s = elem;
                for k in (0..x.dims.len()).rev() {
                    strides[k] = s;
                    s *= (x.dims[k].1 - x.dims[k].0 + 1).max(0) as u64;
                }
                x.dims
                    .iter()
                    .zip(strides)
                    .map(|(&(l, u), s)| (l, u, s))
                    .collect()
            } else {
                Vec::new()
            };
            let len = match var.ty.as_ref().and_then(Ty::str_len) {
                Some(Ext::Star) => {
                    Some(
                        x.strlen
                            .unwrap_or_else(|| match var.ty.as_ref().map(Ty::varying) {
                                Some(crate::types::Var::Varying) => {
                                    (x.len as u32).saturating_sub(2)
                                }
                                _ => x.len as u32,
                            }),
                    )
                }
                Some(Ext::Const(n)) => Some(n),
                _ => None,
            };
            if (k as usize) < bindings.len() {
                bindings[k as usize] = Some(Binding {
                    bit: addr as u64 * 8,
                    dims,
                    len,
                });
            }
        }
        let saved = (CUR_ENABLED.with(|c| c.get()), CUR_LINE.with(|c| c.get()));
        CUR_ENABLED.with(|c| c.set(cond::DEFAULT));
        let r = (|| -> R<Option<Value>> {
            let a = self.activate(proc.block, None, bindings, proc.returns.clone())?;
            let body = &m.blocks[proc.block as usize].body;
            let r = if start > 0 {
                let tail = List {
                    id: body.id,
                    stmts: body.stmts[start..].to_vec(),
                };
                self.run_list(&tail, a)
            } else {
                self.run_list(body, a)
            };
            self.deactivate(a);
            match r {
                Ok(()) => Ok(None),
                Err(Intr::Return(v)) => Ok(v),
                Err(e) => Err(e),
            }
        })();
        CUR_ENABLED.with(|c| c.set(saved.0));
        CUR_LINE.with(|c| c.set(saved.1));
        self.complete_lines();
        let _ = self.env.stdout.flush();
        let (v, stop) = match r {
            Ok(v) => (v, false),
            Err(Intr::Stop) => (None, true),
            Err(e) => return Err(self.report(e)),
        };
        let ret = match (v, &proc.returns) {
            (Some(v), Some(t)) => {
                let t = match (&v, t.str_len()) {
                    (Value::Char(c), Some(e)) if !matches!(e, Ext::Const(_)) => {
                        t.with_len(Ext::Const(c.len() as u32))
                    }
                    _ => t.clone(),
                };
                let (n, _) = t.storage(true);
                let tmp = self.heap_alloc(n);
                let loc = Loc {
                    bit: u64::from(tmp) * 8,
                    ty: t,
                };
                self.store(&loc, &v).map_err(|e| self.report(e))?;
                Some(self.read(tmp, n as usize).map_err(|e| self.report(e))?)
            }
            _ => None,
        };
        Ok(Some(XEnd {
            rc: self.rc,
            ret,
            stop,
        }))
    }

    /// The error of a call from another module: the message and the
    /// abend.
    fn report(&mut self, e: Intr) -> String {
        if let Intr::Fatal(m) = &e {
            let _ = writeln!(
                self.env.msgout,
                "ZEUS PL/I: {m} (line {})",
                CUR_LINE.with(|l| l.get())
            );
        }
        let _ = self.env.msgout.flush();
        format!("{ABEND}U4038")
    }

    /// The return code (PLIRETC).
    pub fn return_code(&self) -> i32 {
        self.rc
    }

    /// Ends a module used through `call_entry`: closes its files.
    pub fn finish(&mut self) {
        self.close_all();
        let _ = self.env.stdout.flush();
        let _ = self.env.msgout.flush();
    }

    fn call_builtin(&mut self, b: crate::builtin::Bif, args: &[Expr], act: usize) -> R<()> {
        use crate::builtin::Bif::*;
        match b {
            PliRetC => {
                self.rc = self.eval(&args[0], act)?.as_i64() as i32;
            }
            PliDump => {}
            PliFill => {
                let p = self.eval(&args[0], act)?;
                let c = self.eval(&args[1], act)?;
                let n = self.eval(&args[2], act)?.as_i64();
                if let (Value::Ptr(p), Value::Char(c)) = (p, c) {
                    let byte = c.first().copied().unwrap_or(SPACE);
                    self.write(p, &vec![byte; n.max(0) as usize])?;
                }
            }
            PliMove => {
                let d = self.eval(&args[0], act)?;
                let s = self.eval(&args[1], act)?;
                let n = self.eval(&args[2], act)?.as_i64();
                if let (Value::Ptr(d), Value::Ptr(s)) = (d, s) {
                    let b = self.read(s, n.max(0) as usize)?;
                    self.write(d, &b)?;
                }
            }
            _ => return err(format!("CALL {b:?} is not supported")),
        }
        Ok(())
    }

    // ---- conditions ------------------------------------------------------

    /// Raises a condition (not by SIGNAL).
    pub(crate) fn raise(&mut self, c: Cond, code: i32, act: usize) -> R<()> {
        self.signal(c, code, act, false)
    }

    /// Raises a condition: runs the established ON-unit or the implicit
    /// action. Ok: execution resumes.
    pub(crate) fn signal(&mut self, c: Cond, code: i32, act: usize, signalled: bool) -> R<()> {
        let top = self.acts.len().saturating_sub(1).max(act);
        self.search_from(c, code, top, act, signalled)
    }

    fn search_from(
        &mut self,
        c: Cond,
        code: i32,
        from: usize,
        act: usize,
        signalled: bool,
    ) -> R<()> {
        let mut i = from.min(self.acts.len().saturating_sub(1));
        loop {
            if i >= self.acts.len() {
                break;
            }
            let found = self.acts[i]
                .on
                .iter()
                .find(|x| x.0 == c)
                .or_else(|| {
                    self.acts[i]
                        .on
                        .iter()
                        .find(|x| x.0 == Cond::AnyCondition && !matches!(c, Cond::Finish))
                })
                .cloned();
            if let Some((_, unit)) = found {
                match unit {
                    None => break, // SYSTEM
                    Some((block, def)) => {
                        self.run_on_unit(block, def, &c, code, act, i)?;
                        return self.after_unit(c, code, act, signalled);
                    }
                }
            }
            if i == 0 {
                break;
            }
            i -= 1;
        }
        self.implicit(c, code, act, signalled)
    }

    fn run_on_unit(
        &mut self,
        block: BlockId,
        def: usize,
        c: &Cond,
        code: i32,
        act: usize,
        found_at: usize,
    ) -> R<()> {
        let info = self.cond_info(c, code, act);
        let a = self.activate(block, Some(def), Vec::new(), None)?;
        self.acts[a].cond = Some(info);
        self.acts[a].handling = Some((c.clone(), found_at));
        let body = self.module.blocks[block as usize].body.clone();
        let save = CUR_ENABLED.with(|e| e.get());
        let r = self.run_list(&body, a);
        CUR_ENABLED.with(|e| e.set(save));
        self.deactivate(a);
        r
    }

    fn cond_info(&self, c: &Cond, code: i32, act: usize) -> CondInfo {
        let mut info = CondInfo {
            code,
            ..Default::default()
        };
        if let Cond::EndFile(f)
        | Cond::Key(f)
        | Cond::Record(f)
        | Cond::UndefinedFile(f)
        | Cond::EndPage(f)
        | Cond::Transmit(f)
        | Cond::Name(f) = c
        {
            info.onfile = self.module.files[*f as usize].name.clone();
            info.onkey = self.files[*f as usize].last_key.clone();
        }
        if let Some(x) = PENDING_CONV.with(|p| p.borrow().clone()) {
            info.onsource = x.0;
            info.onchar = x.1;
        }
        info.onloc = self.proc_name(act);
        info
    }

    fn proc_name(&self, act: usize) -> String {
        let mut a = Some(act);
        while let Some(i) = a {
            if let Some(p) = self.module.blocks[self.acts[i].block as usize].proc {
                return self.module.procs[p as usize].name.clone();
            }
            a = self.acts[i].parent;
        }
        String::new()
    }

    /// What happens after a normal return from an ON-unit.
    fn after_unit(&mut self, c: Cond, code: i32, act: usize, signalled: bool) -> R<()> {
        match c {
            Cond::Error => self.terminate(act),
            Cond::ZeroDivide
            | Cond::Overflow
            | Cond::SubscriptRange
            | Cond::Storage
            | Cond::InvalidOp
                if !signalled =>
            {
                self.raise(Cond::Error, code, act)
            }
            Cond::Conversion if !signalled => self.raise(Cond::Error, code, act),
            _ => Ok(()),
        }
    }

    /// The implicit action of a condition with no ON-unit.
    fn implicit(&mut self, c: Cond, code: i32, act: usize, signalled: bool) -> R<()> {
        let line = CUR_LINE.with(|l| l.get());
        let msg = |me: &mut Machine, text: String| {
            let _ = writeln!(
                me.env.msgout,
                "ZEUS PL/I: ONCODE={code} {text} (line {line} in {})",
                me.proc_name(act)
            );
            let _ = me.env.msgout.flush();
        };
        match &c {
            Cond::Finish | Cond::AnyCondition => Ok(()),
            Cond::EndPage(f) => {
                if !signalled {
                    self.new_page(*f)?;
                }
                Ok(())
            }
            Cond::Condition(n) => {
                let name = self
                    .module
                    .conditions
                    .get(*n as usize)
                    .cloned()
                    .unwrap_or_default();
                msg(
                    self,
                    format!("The CONDITION condition ({name}) was raised."),
                );
                Ok(())
            }
            Cond::StringRange | Cond::StringSize => {
                msg(self, format!("The {} condition was raised.", c.name()));
                Ok(())
            }
            Cond::Underflow => Ok(()),
            Cond::Error => {
                if code != 9 && code != 3 {
                    // the message was given for the original condition
                } else {
                    msg(
                        self,
                        format!(
                            "The ERROR condition was raised{}.",
                            if code == 3 {
                                " (no WHEN clause matched in a SELECT group)"
                            } else {
                                ""
                            }
                        ),
                    );
                }
                self.terminate(act)
            }
            other => {
                let name = match other {
                    Cond::EndFile(f)
                    | Cond::Key(f)
                    | Cond::Record(f)
                    | Cond::UndefinedFile(f)
                    | Cond::Transmit(f)
                    | Cond::Name(f) => {
                        format!(
                            "{} condition was raised for file {}",
                            other.name(),
                            self.module.files[*f as usize].name
                        )
                    }
                    o => format!("{} condition was raised", o.name()),
                };
                msg(self, format!("The {name}."));
                if matches!(other, Cond::Name(_)) {
                    return Ok(());
                }
                self.raise(Cond::Error, code, act)
            }
        }
    }

    /// ERROR without recovery: FINISH, then the program ends.
    fn terminate(&mut self, act: usize) -> R<()> {
        let _ = self.raise(Cond::Finish, 4, act);
        Err(Intr::Terminate)
    }

    // ---- running -----------------------------------------------------------

    /// Runs the main procedure.
    pub fn run(&mut self) -> RunEnd {
        let m = self.module.clone();
        let Some(main) = m.main.or(if m.procs.is_empty() { None } else { Some(0) }) else {
            return RunEnd::Abend("U4038".into());
        };
        // static INITIAL values
        self.started = true;
        if let Err(e) = self.static_init() {
            return self.end(Err(e));
        }
        let proc = &m.procs[main as usize];
        // PARM: the first parameter (CHAR VARYING)
        let mut bindings = Vec::new();
        if let Some(&pv) = proc.params.first() {
            let parm = if proc.main && !m.procs[main as usize].name.is_empty() {
                // OPTIONS(MAIN) without NOEXECOPS: run-time options end
                // with '/'
                match self.env.parm.find('/') {
                    Some(i) => self.env.parm[i + 1..].to_string(),
                    None => self.env.parm.clone(),
                }
            } else {
                self.env.parm.clone()
            };
            let text = zeus_cobol::ebcdic::encode(&parm);
            let addr = self.heap_alloc(text.len() as u32 + 2);
            let _ = self.write(addr, &(text.len() as u16).to_be_bytes());
            let _ = self.write(addr + 2, &text);
            let len = match m.vars[pv as usize].ty.as_ref().and_then(|t| t.str_len()) {
                Some(Ext::Const(n)) => n,
                _ => text.len() as u32,
            };
            bindings.push(Some(Binding {
                bit: u64::from(addr) * 8,
                dims: Vec::new(),
                len: Some(len.max(text.len() as u32)),
            }));
        }
        let r = (|| -> R<()> {
            let a = self.activate(proc.block, None, bindings, proc.returns.clone())?;
            let body = m.blocks[proc.block as usize].body.clone();
            let r = self.run_list(&body, a);
            let r = match r {
                Ok(()) | Err(Intr::Return(_)) => {
                    let _ = self.raise(Cond::Finish, 4, a);
                    Ok(())
                }
                Err(Intr::Stop) => {
                    let _ = self.raise(Cond::Finish, 4, a);
                    Ok(())
                }
                Err(e) => Err(e),
            };
            self.deactivate(a);
            r
        })();
        self.end(r)
    }

    fn end(&mut self, r: R<()>) -> RunEnd {
        self.close_all();
        let _ = self.env.stdout.flush();
        let _ = self.env.msgout.flush();
        match r {
            Ok(()) => RunEnd::Rc(self.rc),
            Err(Intr::Terminate) => RunEnd::Abend("U4038".into()),
            Err(Intr::Stop) => RunEnd::Rc(self.rc),
            Err(Intr::Fatal(m)) => {
                let _ = writeln!(
                    self.env.msgout,
                    "ZEUS PL/I: {m} (line {})",
                    CUR_LINE.with(|l| l.get())
                );
                let _ = self.env.msgout.flush();
                RunEnd::Abend("U4038".into())
            }
            Err(e) => {
                let _ = writeln!(self.env.msgout, "ZEUS PL/I: unexpected {e:?}");
                RunEnd::Abend("U4038".into())
            }
        }
    }

    fn static_init(&mut self) -> R<()> {
        let m = self.module.clone();
        // static INITIAL is evaluated in an activation of the package
        let mark = self.sp;
        let frame = self.stack_alloc(8);
        self.acts.push(Act {
            block: 0,
            parent: None,
            frame,
            params: Vec::new(),
            slots: Vec::new(),
            on: Vec::new(),
            stack_mark: mark,
            index: Vec::new(),
            subject: None,
            cond: None,
            handling: None,
        });
        for init in &m.static_inits {
            if matches!(
                m.vars[m.vars[init.var as usize].root as usize].storage,
                Storage::Static(_)
            ) {
                self.initialize(init, 0)?;
            }
        }
        self.acts.clear();
        self.sp = mark;
        Ok(())
    }

    fn initialize(&mut self, init: &Init, act: usize) -> R<()> {
        self.initialize_at(init, act, None)
    }

    /// Applies INITIAL values to the elements of a variable (a based
    /// variable at `at`).
    fn initialize_at(&mut self, init: &Init, act: usize, at: Option<u32>) -> R<()> {
        let n = self.module.vars[init.var as usize].dims.len();
        let r = Ref {
            var: init.var,
            subs: vec![None; n],
            locator: at.map(|p| {
                Box::new(crate::sema::expr::mk(
                    Ek::Fixed(i128::from(p), 0),
                    Ty::Pointer,
                ))
            }),
        };
        let locs = if let Some(p) = at {
            // locator as a constant pointer
            let saved = r.locator.clone();
            let _ = saved;
            self.elements_at(&r, act, p)?
        } else {
            self.elements(&r, act)?
        };
        let mut values = init.values.iter();
        let mut last = None;
        for loc in locs {
            let v = match values.next() {
                Some(v) => {
                    last = v.clone();
                    v.clone()
                }
                None if init.fill => last.clone(),
                None => break,
            };
            if let Some(e) = v {
                let x = self.eval(&e, act)?;
                self.assign_loc(&loc, x, &e.ty, act)?;
            }
        }
        Ok(())
    }

    fn elements_at(&mut self, r: &Ref, act: usize, p: u32) -> R<Vec<Loc>> {
        let save = PTR_OVERRIDE.with(|x| x.replace(Some(p)));
        let res = self.elements(r, act);
        PTR_OVERRIDE.with(|x| x.set(save));
        res
    }

    // ---- evaluation ------------------------------------------------------

    pub fn eval(&mut self, e: &Expr, act: usize) -> R<Value> {
        Ok(match &*e.k {
            Ek::Fixed(m, q) => match &e.ty {
                Ty::Pointer => Value::Ptr(PTR_OVERRIDE.with(|x| x.get()).unwrap_or(*m as u32)),
                Ty::Fixed { dec, .. } => Value::fixed(*m, *q, *dec),
                _ => Value::fixed(*m, *q, true),
            },
            Ek::Float(f) => Value::Float(*f),
            Ek::Char(b) => Value::Char(b.clone()),
            Ek::Bit(b) => Value::Bit(b.clone()),
            Ek::Null => Value::Ptr(NULL),
            Ek::SysNull => Value::Ptr(0),
            Ek::Load(r) => {
                let loc = self.locate(r, act)?;
                self.load(&loc)?
            }
            Ek::Conv(x) => {
                let v = self.eval(x, act)?;
                self.convert(v, &x.ty, &e.ty, act)?
            }
            Ek::Neg(x) => match self.eval(x, act)? {
                Value::Fixed { m, q, dec } => Value::fixed(-m, q, dec),
                Value::Float(f) => Value::Float(-f),
                Value::Cplx(c) => {
                    let neg = |v: Value| match v {
                        Value::Fixed { m, q, dec } => Value::fixed(-m, q, dec),
                        Value::Float(f) => Value::Float(-f),
                        v => v,
                    };
                    Value::cplx(neg(c.0), neg(c.1))
                }
                v => return err(format!("negation of {v:?}")),
            },
            Ek::Arith(op, a, b) => {
                let x = self.eval(a, act)?;
                let y = self.eval(b, act)?;
                self.arith(*op, x, &a.ty, y, &b.ty, &e.ty, act)?
            }
            Ek::Cmp(op, a, b) => {
                let x = self.eval(a, act)?;
                let y = self.eval(b, act)?;
                let ord = self.compare(&x, &a.ty, &y, &b.ty)?;
                use std::cmp::Ordering::*;
                Value::bit1(match op {
                    CmpOp::Eq => ord == Equal,
                    CmpOp::Ne => ord != Equal,
                    CmpOp::Lt => ord == Less,
                    CmpOp::Le => ord != Greater,
                    CmpOp::Gt => ord == Greater,
                    CmpOp::Ge => ord != Less,
                })
            }
            Ek::Concat(a, b) => {
                let x = self.eval(a, act)?;
                let y = self.eval(b, act)?;
                match (x, y) {
                    (Value::Char(mut p), Value::Char(q)) => {
                        p.extend(q);
                        Value::Char(p)
                    }
                    (Value::Bit(mut p), Value::Bit(q)) => {
                        p.extend(q);
                        Value::Bit(p)
                    }
                    (x, y) => return err(format!("concatenation of {x:?} and {y:?}")),
                }
            }
            Ek::Bits(op, a, b) => {
                // short-circuit for single bits
                let x = self.eval(a, act)?;
                if let Value::Bit(xb) = &x
                    && xb.len() == 1
                    && matches!(b.ty.str_len(), Some(Ext::Const(1)))
                {
                    match op {
                        BitOp::And if !xb[0] => return Ok(Value::bit1(false)),
                        BitOp::Or if xb[0] => return Ok(Value::bit1(true)),
                        _ => {}
                    }
                }
                let y = self.eval(b, act)?;
                let (Value::Bit(mut p), Value::Bit(mut q)) = (x, y) else {
                    return err("bit operation on non-bit values");
                };
                let n = p.len().max(q.len());
                p.resize(n, false);
                q.resize(n, false);
                Value::Bit(
                    p.iter()
                        .zip(&q)
                        .map(|(a, b)| match op {
                            BitOp::And => *a && *b,
                            BitOp::Or => *a || *b,
                            BitOp::Xor => a != b,
                        })
                        .collect(),
                )
            }
            Ek::Not(x) => match self.eval(x, act)? {
                Value::Bit(b) => Value::Bit(b.iter().map(|x| !x).collect()),
                v => return err(format!("¬ of {v:?}")),
            },
            Ek::Call(callee, args) => self
                .call(callee, args, act, Some(&e.ty))?
                .ok_or_else(|| Intr::Fatal("function returned no value".into()))?,
            Ek::Builtin(b, args) => self.builtin(*b, args, &e.ty, act)?,
            Ek::RefBuiltin(b, r, args) => self.ref_builtin(*b, r, args, &e.ty, act)?,
            Ek::Label(l) => {
                let a = self.act_of(act, self.module.labels[*l as usize].block)?;
                Value::Label(*l, a)
            }
            Ek::EntryConst(p) => Value::Entry(*p, act),
            Ek::FileConst(f) => Value::File(*f),
            Ek::OnValue(b) => self.on_value(*b, act),
            Ek::Aggregate(r) => Value::Char(self.aggregate_bytes(r, act)?),
            Ek::ISub(i) => Value::int(
                self.isubs
                    .last()
                    .and_then(|s| s.get(usize::from(*i).saturating_sub(1)))
                    .copied()
                    .unwrap_or(1),
            ),
            Ek::Index(k) => {
                // the innermost activation with indexes (aggregate
                // assignment or repetitive specification)
                let mut a = Some(act);
                let mut v = None;
                while let Some(i) = a {
                    if let Some(x) = self.acts[i].index.get(*k as usize) {
                        v = Some(*x);
                        break;
                    }
                    a = self.acts[i].parent;
                }
                Value::int(v.unwrap_or(1))
            }
            Ek::Subject => {
                let mut a = Some(act);
                let mut v = None;
                while let Some(i) = a {
                    if let Some(x) = &self.acts[i].subject {
                        v = Some(x.clone());
                        break;
                    }
                    a = self.acts[i].parent;
                }
                v.ok_or_else(|| Intr::Fatal("SELECT subject not set".into()))?
            }
        })
    }

    fn on_value(&self, b: crate::builtin::Bif, act: usize) -> Value {
        use crate::builtin::Bif::*;
        let mut a = Some(act);
        let mut info = None;
        while let Some(i) = a {
            if let Some(c) = &self.acts[i].cond {
                info = Some(c.clone());
                break;
            }
            a = self.acts[i].parent;
        }
        let info = info.unwrap_or_default();
        match b {
            OnCode => Value::int(i64::from(info.code)),
            OnCount => Value::int(0),
            OnChar => Value::Char(vec![if info.onchar == 0 { SPACE } else { info.onchar }]),
            OnSource | DataField => Value::Char(info.onsource),
            OnKey => Value::Char(info.onkey),
            OnFile => Value::Char(zeus_cobol::ebcdic::encode(&info.onfile)),
            OnLoc => Value::Char(zeus_cobol::ebcdic::encode(&info.onloc)),
            _ => Value::int(0),
        }
    }
}

/// A target's evaluated location.
#[derive(Debug, Clone)]
pub enum TargetLoc {
    Loc(Loc),
    Substr(Loc, i64, Option<i64>),
    Bytes(Ref),
}

/// All index combinations of the bounds, row-major.
fn index_space(bounds: &[(i64, i64)]) -> Vec<Vec<i64>> {
    let mut out = vec![Vec::new()];
    for &(lb, ub) in bounds {
        let mut next = Vec::new();
        for prefix in &out {
            for i in lb..=ub {
                let mut p = prefix.clone();
                p.push(i);
                next.push(p);
            }
        }
        out = next;
    }
    out
}

fn signal_code(c: &Cond) -> i32 {
    match c {
        Cond::Error => 9,
        Cond::Finish => 4,
        Cond::Name(_) => 10,
        Cond::Record(_) => 20,
        Cond::Transmit(_) => 40,
        Cond::Key(_) => 50,
        Cond::EndFile(_) => 70,
        Cond::UndefinedFile(_) => 80,
        Cond::EndPage(_) => 90,
        Cond::Overflow => 300,
        Cond::FixedOverflow => 310,
        Cond::ZeroDivide => 320,
        Cond::Underflow => 330,
        Cond::Size => 340,
        Cond::StringRange => 350,
        Cond::StringSize => 150,
        Cond::Condition(_) => 500,
        Cond::SubscriptRange => 520,
        Cond::Conversion => 600,
        Cond::Storage => 450,
        Cond::Area => 360,
        Cond::Attention => 400,
        Cond::InvalidOp => 290,
        Cond::AnyCondition => 0,
    }
}

thread_local! {
    static CUR_ENABLED: std::cell::Cell<u16> = const { std::cell::Cell::new(cond::DEFAULT) };
    static CUR_LINE: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    static PTR_OVERRIDE: std::cell::Cell<Option<u32>> = const { std::cell::Cell::new(None) };
    /// REFER values while ALLOCATE sizes a new generation.
    static REFER_OVERRIDE: std::cell::RefCell<Option<Vec<i64>>> = const { std::cell::RefCell::new(None) };
    static PENDING_CONV: std::cell::RefCell<Option<(Vec<u8>, u8)>> = const { std::cell::RefCell::new(None) };
}

//! The interpreter: storage, block activations, statements and
//! conditions.

mod bif;
mod conv;
mod record;
mod stream;
pub mod value;

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;

use crate::ir::*;
use crate::types::{Ext, Ty, Var as Varying};
pub use value::Value;
use value::{NULL, SPACE};

/// Base address of static storage; addresses below are invalid.
const STATIC_BASE: u32 = 0x1000;
/// Base address of the stack (automatic storage).
const STACK_BASE: u32 = 0x4000_0000;

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
    stack_mark: usize,
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
    module: Rc<Module>,
    low: Vec<u8>,
    stack: Vec<u8>,
    acts: Vec<Act>,
    pub env: RunEnv,
    files: Vec<stream::FileState>,
    rc: i32,
    random: u64,
    depth: usize,
}

fn err<T>(m: impl Into<String>) -> R<T> {
    Err(Intr::Fatal(m.into()))
}

impl Machine {
    pub fn new(module: Module, env: RunEnv) -> Machine {
        let mut low = vec![0u8; (STATIC_BASE + module.static_size) as usize];
        low[STATIC_BASE as usize..].copy_from_slice(&module.static_init);
        let files = module.files.iter().map(stream::FileState::new).collect();
        Machine {
            module: Rc::new(module),
            low,
            stack: Vec::new(),
            acts: Vec::new(),
            env,
            files,
            rc: 0,
            random: 0x2545_F491_4F6C_DD1D,
            depth: 0,
        }
    }

    // ---- memory --------------------------------------------------------

    fn mem(&self, addr: u32, len: usize) -> R<&[u8]> {
        if addr >= STACK_BASE {
            let a = (addr - STACK_BASE) as usize;
            self.stack
                .get(a..a + len)
                .ok_or_else(|| Intr::Fatal(format!("address {addr:08X} out of storage")))
        } else {
            let a = addr as usize;
            if a < STATIC_BASE as usize {
                return err(format!("address {addr:08X} is not valid (null pointer?)"));
            }
            self.low
                .get(a..a + len)
                .ok_or_else(|| Intr::Fatal(format!("address {addr:08X} out of storage")))
        }
    }

    fn mem_mut(&mut self, addr: u32, len: usize) -> R<&mut [u8]> {
        if addr >= STACK_BASE {
            let a = (addr - STACK_BASE) as usize;
            self.stack
                .get_mut(a..a + len)
                .ok_or_else(|| Intr::Fatal(format!("address {addr:08X} out of storage")))
        } else {
            let a = addr as usize;
            if a < STATIC_BASE as usize {
                return err(format!("address {addr:08X} is not valid (null pointer?)"));
            }
            self.low
                .get_mut(a..a + len)
                .ok_or_else(|| Intr::Fatal(format!("address {addr:08X} out of storage")))
        }
    }

    fn read(&self, addr: u32, len: usize) -> R<Vec<u8>> {
        Ok(self.mem(addr, len)?.to_vec())
    }

    fn write(&mut self, addr: u32, data: &[u8]) -> R<()> {
        self.mem_mut(addr, data.len())?.copy_from_slice(data);
        Ok(())
    }

    /// Heap storage (ALLOCATE), doubleword aligned.
    fn heap_alloc(&mut self, size: u32) -> u32 {
        let at = self.low.len().div_ceil(8) * 8;
        self.low.resize(at + size.max(1) as usize, 0);
        at as u32
    }

    fn stack_alloc(&mut self, size: u32) -> u32 {
        let at = self.stack.len().div_ceil(8) * 8;
        self.stack.resize(at + size as usize, 0);
        STACK_BASE + at as u32
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
        let mark = self.stack.len();
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
                let size = self.var_size(i as VarId, act)?;
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
        self.stack.truncate(mark);
    }

    /// Size in bytes of all elements of a level-1 variable.
    fn var_size(&self, v: VarId, act: usize) -> R<u64> {
        let var = &self.module.vars[v as usize];
        let ty = self.resolve_ty(var.ty.as_ref(), v, act)?;
        let elem = match &ty {
            Some(t) => {
                let aligned = !matches!(t, Ty::Char { .. } | Ty::Bit { .. } | Ty::Pic(_));
                u64::from(t.storage(aligned).0)
            }
            None => u64::from(var.size),
        };
        let mut n = 1u64;
        for d in &var.dims {
            let (lb, ub) = (self.bound(d.lb, act)?, self.bound(d.ub, act)?);
            n *= (ub - lb + 1).max(0) as u64;
        }
        Ok(elem * n)
    }

    fn bound(&self, b: Bound, act: usize) -> R<i64> {
        Ok(match b {
            Bound::Const(n) => n,
            Bound::Slot(s) => *self.acts[act]
                .slots
                .get(s as usize)
                .ok_or_else(|| Intr::Fatal("extent not evaluated".into()))?,
            Bound::Star => 1,
        })
    }

    /// The type of a variable with `*` and adjustable lengths resolved.
    fn resolve_ty(&self, ty: Option<&Ty>, v: VarId, act: usize) -> R<Option<Ty>> {
        let Some(ty) = ty else { return Ok(None) };
        let len = match ty.str_len() {
            Some(Ext::Slot(s)) => {
                let block = self.module.vars[v as usize].block;
                let a = self.act_of(act, block)?;
                Some(self.acts[a].slots[s as usize].max(0) as u32)
            }
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
            Storage::Static(off) => u64::from(STATIC_BASE + off) * 8,
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
            Storage::Controlled(_) => return err("CONTROLLED storage is not supported"),
            Storage::Member => return err("invalid member storage"),
        };
        // dimensions
        let mut dims = Vec::new();
        let nroot = root.dims.len();
        let dynamic = var
            .dims
            .iter()
            .any(|d| matches!(d.lb, Bound::Slot(_)) || matches!(d.ub, Bound::Slot(_)));
        if dynamic {
            // strides from the extents (level-1 arrays of scalars)
            let ty = self.resolve_ty(var.ty.as_ref(), r.var, act)?;
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
                bounds.push((self.bound(d.lb, a)?, self.bound(d.ub, a)?));
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
                    self.bound(d.lb, act)?,
                    self.bound(d.ub, act)?,
                    u64::from(d.stride),
                ));
            }
        }
        Ok((base + var.bit_offset, dims))
    }

    /// The bit address of a reference's element (free subscripts: the
    /// first element).
    fn element_or_start(&mut self, r: &Ref, act: usize) -> R<(u64, Dims)> {
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
        let var = &self.module.vars[r.var as usize];
        let ty = self
            .resolve_ty(var.ty.as_ref(), r.var, act)?
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
            Ty::Pointer | Ty::Offset | Ty::Area => {
                let b = self.read(addr, 4)?;
                Value::Ptr(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
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
            (Ty::Pointer | Ty::Offset | Ty::Area, Value::Ptr(p)) => {
                self.write(addr, &p.to_be_bytes())?
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
        let (base, dims) = self.base_of(r, act)?;
        let ty = self.resolve_ty(var.ty.as_ref(), r.var, act)?.unwrap();
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
        let size = if var.parent.is_none() {
            self.var_size(r.var, act)? as usize
        } else {
            let mut n = u64::from(var.size);
            for d in &var.dims[root_dims.min(var.dims.len())..] {
                n *= (self.bound(d.ub, act)? - self.bound(d.lb, act)? + 1).max(0) as u64;
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
        match s {
            Stmt::Null => Ok(()),
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
                self.call(callee, args, act)?;
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
                for (v, set) in items {
                    let size = self.var_size(*v, act)?;
                    let p = self.heap_alloc(size as u32);
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
                            self.store(&l, &ptr)?;
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
            Stmt::Free(_) => Ok(()),
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
    fn call(&mut self, callee: &Callee, args: &[Arg], act: usize) -> R<Option<Value>> {
        let (pid, entry) = match callee {
            Callee::Proc(p) => (*p, None),
            Callee::Entry(p, k) => (*p, Some(*k)),
            Callee::External(name) => return self.call_external(name, args, act),
            Callee::Var(e) => match self.eval(e, act)? {
                Value::Entry(p, _) => (p, None),
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
            Some(pb) if m.blocks[pb as usize].name != "*PACKAGE" => Some(self.act_of(act, pb)?),
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
                let var = &m.vars[r.var as usize];
                let ty = self.resolve_ty(var.ty.as_ref(), r.var, act)?;
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

    /// CALL of an external entry: a procedure of another module in the
    /// load libraries.
    fn call_external(&mut self, name: &str, args: &[Arg], act: usize) -> R<Option<Value>> {
        let _ = (args, act);
        err(format!("external procedure {name} not found"))
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
        let frame = self.stack_alloc(8);
        self.acts.push(Act {
            block: 0,
            parent: None,
            frame,
            params: Vec::new(),
            slots: Vec::new(),
            on: Vec::new(),
            stack_mark: 0,
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
        self.stack.clear();
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
                .call(callee, args, act)?
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
    static PENDING_CONV: std::cell::RefCell<Option<(Vec<u8>, u8)>> = const { std::cell::RefCell::new(None) };
}

//! The interpreter's side of concolic execution (`crate::concolic`):
//! before an instruction, the symbolic value of its operands; after it,
//! the symbolic value of what it stored and the condition of a
//! conditional jump. Data written by instructions not followed here are
//! found out by their bytes: a shadow whose bytes changed is dropped.

use zeus_decimal::Dec;

use super::{Exit, Machine, RResult, RunState, err};
use crate::concolic::{Branch, NumShadow, Pred, Step, SymState, Term, VarKind};
use crate::zir::{
    AcceptFrom, ClassKind, CmpMode, Cx, DataType, Ex, Instr, NumType, NumUsage, Op, Rel, SignKind,
    SignPos, Src,
};

/// A symbolic value: a number (term, scale) or alphanumeric bytes.
#[derive(Debug, Clone)]
enum SymVal {
    Num(Term, i32),
    Str(Vec<Term>),
}

/// What an instruction needs after it ran.
enum Pre {
    None,
    Move(Option<SymVal>, Option<(usize, usize, DataType)>),
    Store(Option<(Term, i32)>, Option<(usize, usize, DataType)>, bool),
    Temp(u32, Option<(Term, i32)>),
    Jump(Option<Pred>, u32, bool),
    Pin(Option<(Term, i32)>),
    ClearSize,
    Accept(Option<(usize, usize)>),
    /// An instruction that may set the size-error flag.
    Arith,
    /// An I/O statement (sets the I/O condition).
    File(super::sym_io::FilePre),
    Inspect(Option<super::sym_text::InspectPre>),
    Unstring(Option<super::sym_text::UnstringPre>),
    StringStmt(Option<super::sym_text::StringPre>),
}

const SPACE: u8 = 0x40;

fn pow10(n: u32) -> Option<i128> {
    10i128.checked_pow(n)
}

fn dec_term(d: Dec) -> (Term, i32) {
    (Term::Const(d.coef), d.scale as i32)
}

/// Both numbers at the larger scale.
fn align(a: (Term, i32), b: (Term, i32)) -> Option<(Term, Term, i32)> {
    let s = a.1.max(b.1);
    Some((a.0.rescale(a.1, s)?, b.0.rescale(b.1, s)?, s))
}

/// Unsigned zoned decimal: every byte a digit with the F zone.
pub(super) fn plain_display(t: &NumType) -> bool {
    t.usage == NumUsage::Display && !t.signed && t.sign == SignPos::None
}

impl Machine {
    /// Runs with concolic execution (see `crate::concolic`).
    pub fn set_symbolic(&mut self, st: SymState) {
        self.sym = Some(Box::new(st));
    }

    pub fn take_symbolic(&mut self) -> Option<SymState> {
        self.sym.take().map(|b| *b)
    }

    pub fn symbolic(&mut self) -> Option<&mut SymState> {
        self.sym.as_deref_mut()
    }

    /// Binds numeric data at `addr` to input variable `var` (its value
    /// already stored there).
    pub fn sym_bind_num(&mut self, addr: usize, len: usize, ty: NumType, var: u32) {
        let bytes = self.mem[addr..addr + len].to_vec();
        if let Some(st) = self.sym.as_mut() {
            st.nums.push(NumShadow {
                addr,
                len,
                ty,
                term: Term::Var(var),
                bytes,
            });
        }
    }

    /// Binds a byte to input variable `var`.
    pub fn sym_bind_byte(&mut self, addr: usize, var: u32) {
        let b = self.mem[addr];
        if let Some(st) = self.sym.as_mut() {
            st.bytes.insert(addr, (Term::Var(var), b));
        }
    }

    /// Drops what follows inputs in `[a, a+l)` (storage set from outside).
    pub fn sym_forget_area(&mut self, a: usize, l: usize) {
        self.sym_forget(a, l);
    }

    /// Drops what follows inputs in `[a, a+l)`.
    pub(super) fn sym_forget(&mut self, a: usize, l: usize) {
        let Some(st) = self.sym.as_mut() else { return };
        st.nums.retain(|n| n.addr + n.len <= a || n.addr >= a + l);
        if !st.bytes.is_empty() {
            for i in a..a + l {
                st.bytes.remove(&i);
            }
        }
    }

    pub(super) fn sym_read_num(&mut self, a: usize, l: usize, t: &NumType) -> Option<Term> {
        let mem = &self.mem;
        let st = self.sym.as_mut()?;
        st.nums
            .retain(|n| mem[n.addr..n.addr + n.len] == n.bytes[..]);
        if let Some(n) = st
            .nums
            .iter()
            .find(|n| n.addr == a && n.len == l && n.ty == *t)
        {
            return Some(n.term.clone());
        }
        if !(a..a + l).any(|i| st.bytes.get(&i).is_some_and(|(_, b)| *b == mem[i])) {
            return None;
        }
        // the bytes as terms (input bytes, else their values)
        let byte = |i: usize| match st.bytes.get(&i) {
            Some((t, b)) if *b == mem[i] => t.clone(),
            _ => Term::Const(i128::from(mem[i])),
        };
        let st_value = |t: &Term| t.eval(&|v| st.vars[v as usize].value);
        match t.usage {
            // binary: big-endian, two's complement when signed
            NumUsage::Binary | NumUsage::Native if matches!(l, 2 | 4 | 8) => {
                let mut raw = Term::Const(0);
                for i in a..a + l {
                    raw = Term::add(Term::mul(raw, Term::Const(256)), byte(i));
                }
                if !t.signed {
                    return Some(raw);
                }
                let top = Pred::Cmp(byte(a), Rel::Ge, Term::Const(128));
                let wrapped = Term::sub(raw.clone(), Term::Const(1i128 << (8 * l)));
                return Some(Term::ite(top, wrapped, raw));
            }
            // packed decimal: a digit per half byte, the sign last
            NumUsage::Packed => {
                let mut term = Term::Const(0);
                let mut assume = Vec::new();
                for i in a..a + l {
                    let b = byte(i);
                    let hi = Term::div(b.clone(), 16);
                    let lo = Term::rem(b, 16);
                    let mut digits = vec![hi];
                    if i + 1 < a + l {
                        digits.push(lo);
                    }
                    for d in digits {
                        // a valid digit in this run is assumed to stay one
                        let valid = st_value(&d).is_some_and(|x| (0..=9).contains(&x));
                        if d.konst().is_none() && valid {
                            assume.push(Pred::Cmp(d.clone(), Rel::Le, Term::Const(9)));
                        }
                        term = Term::add(Term::mul(term, Term::Const(10)), d);
                    }
                }
                let sign = Term::rem(byte(a + l - 1), 16);
                for p in assume {
                    st.assume(p);
                }
                if !t.signed {
                    return Some(term);
                }
                let negative = Pred::Or(vec![
                    Pred::Cmp(sign.clone(), Rel::Eq, Term::Const(0x0D)),
                    Pred::Cmp(sign, Rel::Eq, Term::Const(0x0B)),
                ]);
                return Some(Term::ite(negative, Term::neg(term.clone()), term));
            }
            _ => {}
        }
        // digits that are input bytes
        if !plain_display(t) {
            return None;
        }
        let mut term = Term::Const(0);
        let mut assume = Vec::new();
        for (i, &m) in mem.iter().enumerate().skip(a).take(l) {
            let k = pow10((a + l - 1 - i) as u32)?;
            let digit = match st.bytes.get(&i) {
                // a digit (assumed to stay one), else its low half byte
                // as the interpreter reads it (no assumption the run
                // breaks)
                Some((t, b)) if *b == m && (0xF0..=0xF9).contains(&m) => {
                    assume.push(Pred::In(t.clone(), vec![(0xF0, 0xF9)]));
                    Term::sub(t.clone(), Term::Const(0xF0))
                }
                Some((t, b)) if *b == m => Term::rem(t.clone(), 16),
                _ => Term::Const(i128::from(m & 0x0F)),
            };
            term = Term::add(term, Term::mul(digit, Term::Const(k)));
        }
        for p in assume {
            st.assume(p);
        }
        Some(term)
    }

    pub(super) fn sym_read_bytes(&mut self, a: usize, l: usize) -> Option<Vec<Term>> {
        let mem = &self.mem;
        let st = self.sym.as_mut()?;
        if st.bytes.is_empty() && st.nums.is_empty() {
            return None;
        }
        st.nums
            .retain(|n| mem[n.addr..n.addr + n.len] == n.bytes[..]);
        let mut out = Vec::with_capacity(l);
        let mut any = false;
        for (i, &m) in mem.iter().enumerate().skip(a).take(l) {
            let t = match st.bytes.get(&i) {
                Some((t, b)) if *b == m => Some(t.clone()),
                _ => st
                    .nums
                    .iter()
                    .find(|n| n.addr <= i && i < n.addr + n.len && plain_display(&n.ty))
                    .and_then(|n| {
                        let k = pow10((n.addr + n.len - 1 - i) as u32)?;
                        Some(Term::add(
                            Term::rem(Term::div(n.term.clone(), k), 10),
                            Term::Const(0xF0),
                        ))
                    }),
            };
            match t {
                Some(t) => {
                    any = true;
                    out.push(t);
                }
                None => out.push(Term::Const(i128::from(m))),
            }
        }
        any.then_some(out)
    }

    fn sym_src(&mut self, cur: usize, s: &Src) -> Option<SymVal> {
        if self
            .sym
            .as_ref()
            .is_some_and(|st| st.nums.is_empty() && st.bytes.is_empty() && st.temps.is_empty())
        {
            return None;
        }
        match s {
            Src::Loc(loc) => {
                if let Some(v) = self.sym_indexed(cur, loc, false) {
                    return Some(v);
                }
                let (a, l) = self.addr(cur, loc).ok()?;
                match self.loc_type(cur, loc) {
                    DataType::Numeric(t) => {
                        let tm = self.sym_read_num(a, l, &t)?;
                        Some(SymVal::Num(tm, t.scale))
                    }
                    DataType::Alphanumeric { .. }
                    | DataType::Alphabetic { .. }
                    | DataType::Group => self.sym_read_bytes(a, l).map(SymVal::Str),
                    _ => None,
                }
            }
            Src::Num(e) => self.sym_ex(cur, e).map(|(t, s)| SymVal::Num(t, s)),
            _ => None,
        }
    }

    /// An operand of an alphanumeric comparison: an unsigned DISPLAY
    /// integer item compares as its bytes (not read as a number, which
    /// would assume its digits valid).
    fn sym_alnum_src(&mut self, cur: usize, s: &Src) -> Option<SymVal> {
        if let Src::Loc(loc) = s
            && let Some(v) = self.sym_indexed(cur, loc, true)
        {
            return Some(v);
        }
        if let Src::Loc(loc) = s
            && let DataType::Numeric(t) = self.loc_type(cur, loc)
            && plain_display(&t)
            && t.scale == 0
        {
            let (a, l) = self.addr(cur, loc).ok()?;
            return self.sym_read_bytes(a, l).map(SymVal::Str);
        }
        self.sym_src(cur, s)
    }

    /// An element of a one-dimension table with a subscript that follows
    /// the inputs: for each occurrence, its value when the subscript is
    /// that occurrence (the subscript is assumed in range). `bytes`: an
    /// unsigned DISPLAY integer element as its bytes.
    fn sym_indexed(&mut self, cur: usize, loc: &crate::zir::Loc, bytes: bool) -> Option<SymVal> {
        let [sub] = &loc.subs[..] else { return None };
        // a reference modification that does not follow the inputs
        if let Some((start, len)) = &loc.refmod
            && (self
                .sym_ex(cur, start)
                .is_some_and(|t| t.0.konst().is_none())
                || len
                    .as_ref()
                    .is_some_and(|l| self.sym_ex(cur, l).is_some_and(|t| t.0.konst().is_none())))
        {
            return None;
        }
        let code = self.instances[cur].code.clone();
        let desc = &code.descs[loc.desc as usize];
        let [dim] = &desc.dims[..] else { return None };
        if dim.dynamic_stride.is_some() || desc.dynamic_offset.is_some() {
            return None;
        }
        let (st, sc) = self.sym_ex(cur, sub)?;
        if sc != 0 || st.konst().is_some() {
            return None;
        }
        let k = self.sym_value(&st)?;
        let (ar, l) = self.addr(cur, loc).ok()?;
        let ty = self.loc_type(cur, loc);
        // the element's start (a reference modification is an offset in it)
        let whole = crate::zir::Loc {
            refmod: None,
            ..loc.clone()
        };
        let (a, _) = self.addr(cur, &whole).ok()?;
        let off = ar.checked_sub(a)?;
        let max = i128::from(dim.max.min(64));
        if !(1..=max).contains(&k) {
            return None;
        }
        let stride = dim.stride as usize;
        let first = a.checked_sub((k as usize - 1) * stride)?;
        if first + (max as usize - 1) * stride + off + l > self.mem.len() {
            return None;
        }
        let num = match &ty {
            DataType::Numeric(t) if !(bytes && plain_display(t) && t.scale == 0) => Some(*t),
            DataType::Numeric(t) if !plain_display(t) => return None,
            DataType::Numeric(_)
            | DataType::Alphanumeric { .. }
            | DataType::Alphabetic { .. }
            | DataType::Group => None,
            _ => return None,
        };
        let at = |j: i128| first + (j as usize - 1) * stride + off;
        let is = |j: i128| Pred::Cmp(st.clone(), Rel::Eq, Term::Const(j));
        let out = match num {
            Some(t) => {
                let mut v: Option<Term> = None;
                for j in (1..=max).rev() {
                    let x = match self.sym_read_num(at(j), l, &t) {
                        Some(x) => x,
                        None => {
                            let d = super::data::num_value(&t, &self.mem[at(j)..at(j) + l]);
                            Term::Const(
                                d.rescale(t.scale.max(0) as u32, zeus_decimal::Rounding::Truncate)?
                                    .coef,
                            )
                        }
                    };
                    v = Some(match v {
                        None => x,
                        Some(rest) => Term::ite(is(j), x, rest),
                    });
                }
                SymVal::Num(v?, t.scale)
            }
            None => {
                let mut v: Vec<Term> = Vec::new();
                for j in (1..=max).rev() {
                    let x = match self.sym_read_bytes(at(j), l) {
                        Some(x) => x,
                        None => self.mem[at(j)..at(j) + l]
                            .iter()
                            .map(|b| Term::Const(i128::from(*b)))
                            .collect(),
                    };
                    v = if v.is_empty() {
                        x
                    } else {
                        x.into_iter()
                            .zip(v)
                            .map(|(x, rest)| Term::ite(is(j), x, rest))
                            .collect()
                    };
                }
                SymVal::Str(v)
            }
        };
        if let Some(s) = self.sym.as_mut() {
            s.assume(Pred::And(vec![
                Pred::Cmp(st.clone(), Rel::Ge, Term::Const(1)),
                Pred::Cmp(st, Rel::Le, Term::Const(max)),
            ]));
        }
        Some(out)
    }

    pub(super) fn sym_ex(&mut self, cur: usize, e: &Ex) -> Option<(Term, i32)> {
        match e {
            Ex::Load(loc) => {
                if let Some(SymVal::Num(t, s)) = self.sym_indexed(cur, loc, false) {
                    return Some((t, s));
                }
                let (a, l) = self.addr(cur, loc).ok()?;
                match self.loc_type(cur, loc) {
                    DataType::Numeric(t) => self.sym_read_num(a, l, &t).map(|tm| (tm, t.scale)),
                    _ => None,
                }
            }
            Ex::Temp(t) => self.sym.as_ref()?.temps.get(&(cur, *t)).cloned(),
            Ex::Func(crate::zir::Func::NumVal | crate::zir::Func::NumValC, args) => {
                match args.first() {
                    Some(crate::zir::Arg::Src(s)) => self.sym_numval(cur, s),
                    _ => None,
                }
            }
            Ex::Neg(x) => self.sym_ex(cur, x).map(|(t, s)| (Term::neg(t), s)),
            Ex::Bin(op, x, y) => {
                let a = self.sym_ex(cur, x);
                let b = self.sym_ex(cur, y);
                if a.is_none() && b.is_none() {
                    return None;
                }
                let a = match a {
                    Some(a) => a,
                    None => dec_term(self.eval_ex(cur, x).ok()?),
                };
                let b = match b {
                    Some(b) => b,
                    None => dec_term(self.eval_ex(cur, y).ok()?),
                };
                match op {
                    Op::Add => align(a, b).map(|(p, q, s)| (Term::add(p, q), s)),
                    Op::Sub => align(a, b).map(|(p, q, s)| (Term::sub(p, q), s)),
                    Op::Mul => Some((Term::mul(a.0, b.0), a.1 + b.1)),
                    Op::Div => {
                        // as the interpreter: truncated at scale max(20, scales)
                        let k = b.0.konst()?;
                        if k == 0 {
                            return None;
                        }
                        let s = 20.max(a.1).max(b.1);
                        let num = Term::mul(a.0, Term::Const(pow10((s - a.1 + b.1) as u32)?));
                        let q = Term::div(num, k.abs());
                        Some((if k < 0 { Term::neg(q) } else { q }, s))
                    }
                    Op::Pow => None,
                }
            }
            _ => None,
        }
    }

    /// The concrete value of a term in this run.
    pub(super) fn sym_value(&self, t: &Term) -> Option<i128> {
        let st = self.sym.as_ref()?;
        t.eval(&|v| st.value(v))
    }

    fn sym_cx(&mut self, cur: usize, c: &Cx) -> Option<Pred> {
        match c {
            Cx::Not(x) => self.sym_cx(cur, x).map(Pred::not),
            Cx::And(x, y) | Cx::Or(x, y) => {
                let a = self.sym_cx(cur, x);
                let b = self.sym_cx(cur, y);
                if a.is_none() && b.is_none() {
                    return None;
                }
                let a = match a {
                    Some(p) => p,
                    None => Pred::Bool(self.eval_cx(cur, x).ok()?),
                };
                let b = match b {
                    Some(p) => p,
                    None => Pred::Bool(self.eval_cx(cur, y).ok()?),
                };
                Some(if matches!(c, Cx::And(..)) {
                    Pred::And(vec![a, b])
                } else {
                    Pred::Or(vec![a, b])
                })
            }
            Cx::SizeError => self.sym.as_ref()?.size_error.clone(),
            Cx::IoCond => self.sym.as_ref()?.io_cond.clone(),
            Cx::Sign(e, k) => {
                let (t, _) = self.sym_ex(cur, e)?;
                let r = match k {
                    SignKind::Positive => Rel::Gt,
                    SignKind::Negative => Rel::Lt,
                    SignKind::Zero => Rel::Eq,
                };
                Some(Pred::Cmp(t, r, Term::Const(0)))
            }
            Cx::Class(s, k) => {
                let Some(SymVal::Str(bytes)) = self.sym_alnum_src(cur, s) else {
                    return None;
                };
                let ranges: Vec<(i128, i128)> = match k {
                    ClassKind::Numeric => vec![(0xF0, 0xF9)],
                    ClassKind::Alphabetic => vec![
                        (0x40, 0x40),
                        (0x81, 0x89),
                        (0x91, 0x99),
                        (0xA2, 0xA9),
                        (0xC1, 0xC9),
                        (0xD1, 0xD9),
                        (0xE2, 0xE9),
                    ],
                    ClassKind::Upper => {
                        vec![(0x40, 0x40), (0xC1, 0xC9), (0xD1, 0xD9), (0xE2, 0xE9)]
                    }
                    ClassKind::Lower => {
                        vec![(0x40, 0x40), (0x81, 0x89), (0x91, 0x99), (0xA2, 0xA9)]
                    }
                    ClassKind::Set(set) => set
                        .iter()
                        .map(|&b| (i128::from(b), i128::from(b)))
                        .collect(),
                };
                Some(Pred::And(
                    bytes
                        .into_iter()
                        .map(|b| Pred::In(b, ranges.clone()))
                        .collect(),
                ))
            }
            Cx::Cmp(a, r, b, mode) => {
                let (sa, sb) = if *mode == CmpMode::Alnum {
                    (self.sym_alnum_src(cur, a), self.sym_alnum_src(cur, b))
                } else {
                    (self.sym_src(cur, a), self.sym_src(cur, b))
                };
                if sa.is_none() && sb.is_none() {
                    return None;
                }
                match mode {
                    CmpMode::Numeric => {
                        let num = |m: &mut Machine,
                                   s: Option<SymVal>,
                                   src: &Src|
                         -> Option<(Term, i32)> {
                            match s {
                                Some(SymVal::Num(t, sc)) => Some((t, sc)),
                                Some(SymVal::Str(_)) => None,
                                None => {
                                    let v = m.eval_src(cur, src).ok()?;
                                    Some(dec_term(m.val_num(&v)))
                                }
                            }
                        };
                        let x = num(self, sa, a)?;
                        let y = num(self, sb, b)?;
                        let (p, q, _) = align(x, y)?;
                        Some(Pred::Cmp(p, *r, q))
                    }
                    CmpMode::Alnum => {
                        if self.instances[cur].code.collating.is_some() {
                            return None;
                        }
                        let len =
                            |m: &mut Machine, s: &Option<SymVal>, src: &Src| -> Option<usize> {
                                match s {
                                    Some(SymVal::Str(b)) => Some(b.len()),
                                    Some(SymVal::Num(..)) => None,
                                    None => {
                                        let v = m.eval_src(cur, src).ok()?;
                                        Some(m.alnum_len(&v))
                                    }
                                }
                            };
                        let n = len(self, &sa, a)?.max(len(self, &sb, b)?).max(1);
                        let bytes = |m: &mut Machine,
                                     s: Option<SymVal>,
                                     src: &Src|
                         -> Option<Vec<Term>> {
                            match s {
                                Some(SymVal::Str(mut b)) => {
                                    b.resize(n, Term::Const(i128::from(SPACE)));
                                    Some(b)
                                }
                                Some(SymVal::Num(..)) => None,
                                None => {
                                    let v = m.eval_src(cur, src).ok()?;
                                    let b = m.alnum_bytes(cur, &v, n).ok()?;
                                    Some(
                                        b.into_iter().map(|x| Term::Const(i128::from(x))).collect(),
                                    )
                                }
                            }
                        };
                        let x = bytes(self, sa, a)?;
                        let y = bytes(self, sb, b)?;
                        Some(Pred::Bytes(x, *r, y))
                    }
                }
            }
            _ => None,
        }
    }

    /// The term a numeric item of type `t` gets from a number.
    pub(super) fn sym_store_term(t: &NumType, term: Term, scale: i32) -> Option<(Term, Term)> {
        let raw = term.rescale(scale, t.scale)?;
        let mut v = raw.clone();
        if t.usage != NumUsage::Native {
            v = Term::rem(v, pow10(t.digits)?);
        }
        if !t.signed {
            v = Term::Abs(Box::new(v));
        }
        Some((raw, v))
    }

    fn sym_dst(&mut self, cur: usize, loc: &crate::zir::Loc) -> Option<(usize, usize, DataType)> {
        let (a, l) = self.addr(cur, loc).ok()?;
        Some((a, l, self.loc_type(cur, loc)))
    }

    /// Runs one instruction with concolic execution.
    pub(super) fn sym_step(&mut self, s: &mut RunState) -> RResult<Option<Exit>> {
        {
            let st = self.sym.as_mut().unwrap();
            st.steps += 1;
            if st.limit > 0 && st.steps > st.limit {
                return err("instruction limit reached (endless loop?)");
            }
        }
        let cur = s.cur;
        let pc = s.pc;
        let code = self.instances[cur].code.clone();
        if let Some(st) = self.sym.as_mut() {
            st.at = Some((code.name.clone(), pc));
        }
        let pre = match code.code.get(pc as usize) {
            Some(Instr::Move(src, dst)) => {
                let v = self.sym_src(cur, src);
                Pre::Move(v, self.sym_dst(cur, dst))
            }
            Some(Instr::Store {
                value, dst, check, ..
            }) => {
                let v = self.sym_ex(cur, value);
                Pre::Store(v, self.sym_dst(cur, dst), *check)
            }
            Some(Instr::DivRem { .. }) => Pre::Arith,
            Some(Instr::SetTemp(t, ex)) => Pre::Temp(*t, self.sym_ex(cur, ex)),
            Some(Instr::JumpIf(c, t)) => Pre::Jump(self.sym_cx(cur, c), *t, true),
            Some(Instr::JumpUnless(c, t)) => Pre::Jump(self.sym_cx(cur, c), *t, false),
            Some(Instr::GoDepending(ex, _)) => Pre::Pin(self.sym_ex(cur, ex)),
            Some(Instr::ClearSizeError) => Pre::ClearSize,
            Some(
                i @ (Instr::Read { .. }
                | Instr::Write { .. }
                | Instr::Rewrite { .. }
                | Instr::Delete(_)
                | Instr::Start { .. }
                | Instr::Open(..)
                | Instr::Close(..)
                | Instr::SortRead(_)),
            ) => Pre::File(self.sym_io_pre(cur, i)),
            Some(Instr::Accept {
                dst,
                from: AcceptFrom::Stdin,
            }) => Pre::Accept(self.addr(cur, dst).ok()),
            Some(Instr::Inspect(target, steps)) => {
                Pre::Inspect(self.sym_inspect_pre(cur, target, steps))
            }
            Some(Instr::Unstring {
                source,
                delims,
                into,
                pointer,
                tallying,
            }) => Pre::Unstring(self.sym_unstring_pre(
                cur,
                pc,
                source,
                delims,
                into,
                pointer.as_ref(),
                tallying.as_ref(),
            )),
            Some(Instr::StringStmt {
                sources,
                into,
                pointer,
            }) => Pre::StringStmt(self.sym_string_pre(cur, pc, sources, into, pointer.as_ref())),
            _ => Pre::None,
        };
        let size_before = self.size_error;
        let r = self.exec_counted(s);
        match pre {
            Pre::None => {}
            Pre::Move(v, dst) => {
                let Some((a, l, ty)) = dst else { return r };
                self.sym_forget(a, l);
                match (v, ty) {
                    (Some(SymVal::Num(t, sc)), DataType::Numeric(nt)) => {
                        if let Some((_, v)) = Self::sym_store_term(&nt, t, sc) {
                            self.sym_bind_term(a, l, nt, v);
                        }
                    }
                    (
                        Some(SymVal::Str(b)),
                        DataType::Alphanumeric { just: false }
                        | DataType::Alphabetic { just: false }
                        | DataType::Group,
                    ) => {
                        for i in 0..l {
                            let t = b.get(i).cloned().unwrap_or(Term::Const(i128::from(SPACE)));
                            if t.konst().is_none() {
                                self.sym_bind_byte_term(a + i, t);
                            }
                        }
                    }
                    (
                        Some(SymVal::Str(b)),
                        DataType::Alphanumeric { just: true } | DataType::Alphabetic { just: true },
                    ) => {
                        // JUSTIFIED RIGHT: the rightmost bytes, spaces on the left
                        let n = b.len();
                        for i in 0..l {
                            let t = (i + n)
                                .checked_sub(l)
                                .and_then(|k| b.get(k).cloned())
                                .unwrap_or(Term::Const(i128::from(SPACE)));
                            if t.konst().is_none() {
                                self.sym_bind_byte_term(a + i, t);
                            }
                        }
                    }
                    (Some(SymVal::Str(b)), DataType::Numeric(nt))
                        if plain_display(&nt)
                            && nt.scale == 0
                            && b.len() == l
                            && nt.digits as usize == l =>
                    {
                        for (i, t) in b.into_iter().enumerate() {
                            if t.konst().is_none() {
                                self.sym_bind_byte_term(a + i, t);
                            }
                        }
                    }
                    _ => {}
                }
            }
            Pre::Store(v, dst, check) => {
                let mut overflow = None;
                if let Some((a, l, ty)) = dst {
                    self.sym_forget(a, l);
                    if let (Some((t, sc)), DataType::Numeric(nt)) = (v, ty)
                        && let Some((raw, stored)) = Self::sym_store_term(&nt, t, sc)
                    {
                        let cap = pow10(nt.digits);
                        let over = cap.and_then(|c| {
                            Some((
                                Pred::Or(vec![
                                    Pred::Cmp(raw.clone(), Rel::Ge, Term::Const(c)),
                                    Pred::Cmp(raw.clone(), Rel::Le, Term::Const(-c)),
                                ]),
                                self.sym_value(&raw)?.abs() >= c,
                            ))
                        });
                        if let Some((p, happened)) = over {
                            if !(check && happened) {
                                self.sym_bind_term(a, l, nt, stored);
                            }
                            if nt.usage != NumUsage::Native {
                                overflow = Some(p);
                            }
                        }
                    }
                }
                let prev = self.sym.as_ref().and_then(|st| st.size_error.clone());
                let flag = match (prev, overflow) {
                    (None, None) => None,
                    (prev, over) => Some(Pred::Or(vec![
                        prev.unwrap_or(Pred::Bool(size_before)),
                        over.unwrap_or(Pred::Bool(self.size_error && !size_before)),
                    ])),
                };
                if let Some(st) = self.sym.as_mut() {
                    st.size_error = flag;
                }
            }
            Pre::Arith => {
                let now = self.size_error && !size_before;
                if let Some(st) = self.sym.as_mut()
                    && let Some(p) = st.size_error.take()
                {
                    st.size_error = Some(Pred::Or(vec![p, Pred::Bool(now)]));
                }
            }
            Pre::ClearSize => {
                if let Some(st) = self.sym.as_mut() {
                    st.size_error = None;
                }
            }
            Pre::Temp(t, v) => {
                if let Some(st) = self.sym.as_mut() {
                    match v {
                        Some(v) => st.temps.insert((cur, t), v),
                        None => st.temps.remove(&(cur, t)),
                    };
                }
            }
            Pre::Jump(p, target, is_if) => {
                let jumped = s.cur == cur && s.pc != pc + 1;
                let _ = target;
                let program = code.name.clone();
                let pred = p.filter(|p| !p.is_ground());
                // the condition as "the jump is taken"
                let pred = pred.map(|p| if is_if { p } else { Pred::not(p) });
                if let Some(st) = self.sym.as_mut() {
                    // a long path (an endless loop): each direction once more
                    if st.long() && !st.late.insert((program.clone(), pc, jumped)) {
                        return r;
                    }
                    let pred = pred.filter(|_| !st.long());
                    st.path.push(Step::Branch(Branch {
                        program,
                        pc,
                        jumped,
                        pred,
                    }));
                }
            }
            Pre::Pin(v) => {
                if let Some((t, _)) = v
                    && let Some(x) = self.sym_value(&t)
                    && let Some(st) = self.sym.as_mut()
                {
                    st.assume(Pred::Cmp(t, Rel::Eq, Term::Const(x)));
                }
            }
            Pre::File(p) => self.sym_io_post(cur, p),
            Pre::Unstring(pre) => {
                if r.is_ok()
                    && let Some(pre) = pre
                {
                    self.sym_unstring_post(pre);
                }
            }
            Pre::StringStmt(pre) => {
                if r.is_ok()
                    && let Some(pre) = pre
                {
                    self.sym_string_post(pre);
                }
            }
            Pre::Inspect(pre) => {
                if r.is_ok()
                    && let Some(pre) = pre
                {
                    self.sym_inspect_post(pre);
                }
            }
            Pre::Accept(dst) => {
                if let Some((a, l)) = dst
                    && let Some(st) = self.sym.as_mut()
                {
                    st.accepts += 1;
                    let n = st.accepts;
                    for i in 0..l {
                        let name = format!("ACCEPT{n}.{}", i + 1);
                        let b = self.mem[a + i];
                        let st = self.sym.as_mut().unwrap();
                        let v = st.var(name, VarKind::Byte, i128::from(b));
                        st.bytes.insert(a + i, (Term::Var(v), b));
                    }
                }
            }
        }
        r
    }

    pub(super) fn sym_bind_term(&mut self, a: usize, l: usize, ty: NumType, term: Term) {
        if term.konst().is_some() || !term.small(crate::concolic::MAX_TERM) {
            return;
        }
        let bytes = self.mem[a..a + l].to_vec();
        if let Some(st) = self.sym.as_mut() {
            st.nums.push(NumShadow {
                addr: a,
                len: l,
                ty,
                term,
                bytes,
            });
        }
    }

    pub(super) fn sym_bind_byte_term(&mut self, a: usize, t: Term) {
        if !t.small(crate::concolic::MAX_TERM) {
            return;
        }
        let b = self.mem[a];
        if let Some(st) = self.sym.as_mut() {
            st.bytes.insert(a, (t, b));
        }
    }
}

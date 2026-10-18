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

    fn sym_read_num(&mut self, a: usize, l: usize, t: &NumType) -> Option<Term> {
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
        // digits that are input bytes
        if !plain_display(t)
            || !(a..a + l).any(|i| st.bytes.get(&i).is_some_and(|(_, b)| *b == mem[i]))
        {
            return None;
        }
        let mut term = Term::Const(0);
        let mut assume = Vec::new();
        for (i, &m) in mem.iter().enumerate().skip(a).take(l) {
            let k = pow10((a + l - 1 - i) as u32)?;
            let digit = match st.bytes.get(&i) {
                Some((t, b)) if *b == m => {
                    assume.push(Pred::In(t.clone(), vec![(0xF0, 0xF9)]));
                    Term::sub(t.clone(), Term::Const(0xF0))
                }
                _ => Term::Const(i128::from(m & 0x0F)),
            };
            term = Term::add(term, Term::mul(digit, Term::Const(k)));
        }
        for p in assume {
            st.path.push(Step::Assume(p));
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
            && let DataType::Numeric(t) = self.loc_type(cur, loc)
            && plain_display(&t)
            && t.scale == 0
        {
            let (a, l) = self.addr(cur, loc).ok()?;
            return self.sym_read_bytes(a, l).map(SymVal::Str);
        }
        self.sym_src(cur, s)
    }

    fn sym_ex(&mut self, cur: usize, e: &Ex) -> Option<(Term, i32)> {
        match e {
            Ex::Load(loc) => {
                let (a, l) = self.addr(cur, loc).ok()?;
                match self.loc_type(cur, loc) {
                    DataType::Numeric(t) => self.sym_read_num(a, l, &t).map(|tm| (tm, t.scale)),
                    _ => None,
                }
            }
            Ex::Temp(t) => self.sym.as_ref()?.temps.get(&(cur, *t)).cloned(),
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
                let Some(SymVal::Str(bytes)) = self.sym_src(cur, s) else {
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
    fn sym_store_term(t: &NumType, term: Term, scale: i32) -> Option<(Term, Term)> {
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
                    st.path
                        .push(Step::Assume(Pred::Cmp(t, Rel::Eq, Term::Const(x))));
                }
            }
            Pre::File(p) => self.sym_io_post(cur, p),
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
        if term.konst().is_some() {
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
        let b = self.mem[a];
        if let Some(st) = self.sym.as_mut() {
            st.bytes.insert(a, (t, b));
        }
    }
}

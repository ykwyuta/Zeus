//! Concolic execution: while the interpreter runs a program on concrete
//! inputs, values that come from the inputs are followed as symbolic
//! terms, and each conditional jump records the condition it decided on
//! (see docs/proposal/0012-c1-test-generation.md). The terms and
//! conditions here are what a solver gets to find inputs that take the
//! other way.
//!
//! Numbers are integers (a value with scale `s` is `term / 10^s`);
//! alphanumeric data is a sequence of byte-valued terms.

use crate::zir::Rel;

/// An integer term over the input variables.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Term {
    Const(i128),
    /// Input variable (numeric: the integer of its digits; byte: 0-255).
    Var(u32),
    Add(Box<Term>, Box<Term>),
    Sub(Box<Term>, Box<Term>),
    Mul(Box<Term>, Box<Term>),
    Neg(Box<Term>),
    Abs(Box<Term>),
    /// Division truncated toward zero by a positive constant.
    Div(Box<Term>, i128),
    /// Remainder (with the sign of the dividend) of a division by a
    /// positive constant.
    Rem(Box<Term>, i128),
    /// The first term when the condition holds, else the second.
    If(Box<Pred>, Box<Term>, Box<Term>),
}

// The constructors fold constants (they are not the operators).
#[allow(clippy::should_implement_trait)]
impl Term {
    pub fn konst(&self) -> Option<i128> {
        match self {
            Term::Const(c) => Some(*c),
            _ => None,
        }
    }

    pub fn add(a: Term, b: Term) -> Term {
        match (a.konst(), b.konst()) {
            (Some(x), Some(y)) => x
                .checked_add(y)
                .map_or(Term::Add(Box::new(a), Box::new(b)), Term::Const),
            (Some(0), _) => b,
            (_, Some(0)) => a,
            _ => Term::Add(Box::new(a), Box::new(b)),
        }
    }

    pub fn sub(a: Term, b: Term) -> Term {
        match (a.konst(), b.konst()) {
            (Some(x), Some(y)) => x
                .checked_sub(y)
                .map_or(Term::Sub(Box::new(a), Box::new(b)), Term::Const),
            (_, Some(0)) => a,
            _ => Term::Sub(Box::new(a), Box::new(b)),
        }
    }

    pub fn mul(a: Term, b: Term) -> Term {
        match (a.konst(), b.konst()) {
            (Some(x), Some(y)) => x
                .checked_mul(y)
                .map_or(Term::Mul(Box::new(a), Box::new(b)), Term::Const),
            (Some(1), _) => b,
            (_, Some(1)) => a,
            (Some(0), _) | (_, Some(0)) => Term::Const(0),
            _ => Term::Mul(Box::new(a), Box::new(b)),
        }
    }

    pub fn neg(a: Term) -> Term {
        match a.konst() {
            Some(x) => Term::Const(-x),
            None => Term::Neg(Box::new(a)),
        }
    }

    pub fn div(a: Term, k: i128) -> Term {
        match a {
            Term::Const(x) => Term::Const(x / k),
            a if k == 1 => a,
            // (x * c) / k = x * (c / k) when k divides c
            Term::Mul(x, c) if c.konst().is_some_and(|c| c % k == 0) => {
                Term::mul(*x, Term::Const(c.konst().unwrap() / k))
            }
            Term::Mul(c, x) if c.konst().is_some_and(|c| c % k == 0) => {
                Term::mul(*x, Term::Const(c.konst().unwrap() / k))
            }
            a => Term::Div(Box::new(a), k),
        }
    }

    pub fn ite(p: Pred, a: Term, b: Term) -> Term {
        match p {
            Pred::Bool(true) => a,
            Pred::Bool(false) => b,
            _ if a == b => a,
            p => Term::If(Box::new(p), Box::new(a), Box::new(b)),
        }
    }

    pub fn rem(a: Term, k: i128) -> Term {
        match a.konst() {
            Some(x) => Term::Const(x % k),
            None => Term::Rem(Box::new(a), k),
        }
    }

    /// The term scaled from scale `from` to scale `to` (truncating).
    pub fn rescale(self, from: i32, to: i32) -> Option<Term> {
        if to >= from {
            let k = 10i128.checked_pow((to - from) as u32)?;
            Some(Term::mul(self, Term::Const(k)))
        } else {
            let k = 10i128.checked_pow((from - to) as u32)?;
            Some(Term::div(self, k))
        }
    }

    /// Whether the term has at most `limit` nodes.
    pub fn small(&self, limit: usize) -> bool {
        let mut n = 0;
        self.count(&mut n, limit)
    }

    fn count(&self, n: &mut usize, limit: usize) -> bool {
        *n += 1;
        if *n > limit {
            return false;
        }
        match self {
            Term::Const(_) | Term::Var(_) => true,
            Term::Add(a, b) | Term::Sub(a, b) | Term::Mul(a, b) => {
                a.count(n, limit) && b.count(n, limit)
            }
            Term::Neg(a) | Term::Abs(a) | Term::Div(a, _) | Term::Rem(a, _) => a.count(n, limit),
            Term::If(p, a, b) => p.count(n, limit) && a.count(n, limit) && b.count(n, limit),
        }
    }

    /// The value with the variables' values from `v`; None on overflow.
    pub fn eval(&self, v: &dyn Fn(u32) -> i128) -> Option<i128> {
        Some(match self {
            Term::Const(c) => *c,
            Term::Var(i) => v(*i),
            Term::Add(a, b) => a.eval(v)?.checked_add(b.eval(v)?)?,
            Term::Sub(a, b) => a.eval(v)?.checked_sub(b.eval(v)?)?,
            Term::Mul(a, b) => a.eval(v)?.checked_mul(b.eval(v)?)?,
            Term::Neg(a) => a.eval(v)?.checked_neg()?,
            Term::Abs(a) => a.eval(v)?.checked_abs()?,
            Term::Div(a, k) => a.eval(v)? / k,
            Term::Rem(a, k) => a.eval(v)? % k,
            Term::If(p, a, b) => {
                if p.eval(v)? {
                    a.eval(v)?
                } else {
                    b.eval(v)?
                }
            }
        })
    }

    pub fn vars(&self, out: &mut Vec<u32>) {
        match self {
            Term::Const(_) => {}
            Term::Var(i) => {
                if !out.contains(i) {
                    out.push(*i);
                }
            }
            Term::Add(a, b) | Term::Sub(a, b) | Term::Mul(a, b) => {
                a.vars(out);
                b.vars(out);
            }
            Term::Neg(a) | Term::Abs(a) | Term::Div(a, _) | Term::Rem(a, _) => a.vars(out),
            Term::If(p, a, b) => {
                p.vars(out);
                a.vars(out);
                b.vars(out);
            }
        }
    }
}

/// A condition over the input variables.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pred {
    Bool(bool),
    /// Integer comparison.
    Cmp(Term, Rel, Term),
    /// Comparison of two byte strings of the same length (EBCDIC order).
    Bytes(Vec<Term>, Rel, Vec<Term>),
    /// The term is in one of the ranges.
    In(Term, Vec<(i128, i128)>),
    Not(Box<Pred>),
    And(Vec<Pred>),
    Or(Vec<Pred>),
}

fn rel(o: std::cmp::Ordering, r: Rel) -> bool {
    use std::cmp::Ordering::*;
    match r {
        Rel::Eq => o == Equal,
        Rel::Ne => o != Equal,
        Rel::Lt => o == Less,
        Rel::Le => o != Greater,
        Rel::Gt => o == Greater,
        Rel::Ge => o != Less,
    }
}

#[allow(clippy::should_implement_trait)]
impl Pred {
    fn count(&self, n: &mut usize, limit: usize) -> bool {
        *n += 1;
        if *n > limit {
            return false;
        }
        match self {
            Pred::Bool(_) => true,
            Pred::Cmp(a, _, b) => a.count(n, limit) && b.count(n, limit),
            Pred::Bytes(a, _, b) => a.iter().chain(b).all(|t| t.count(n, limit)),
            Pred::In(t, _) => t.count(n, limit),
            Pred::Not(p) => p.count(n, limit),
            Pred::And(ps) | Pred::Or(ps) => ps.iter().all(|p| p.count(n, limit)),
        }
    }

    pub fn not(p: Pred) -> Pred {
        match p {
            Pred::Bool(b) => Pred::Bool(!b),
            Pred::Not(x) => *x,
            p => Pred::Not(Box::new(p)),
        }
    }

    /// Whether the condition has no variables.
    pub fn is_ground(&self) -> bool {
        let mut v = Vec::new();
        self.vars(&mut v);
        v.is_empty()
    }

    /// The truth with the variables' values from `v`; None when a term
    /// overflows.
    pub fn eval(&self, v: &dyn Fn(u32) -> i128) -> Option<bool> {
        Some(match self {
            Pred::Bool(b) => *b,
            Pred::Cmp(a, r, b) => rel(a.eval(v)?.cmp(&b.eval(v)?), *r),
            Pred::Bytes(a, r, b) => {
                let mut o = std::cmp::Ordering::Equal;
                for (x, y) in a.iter().zip(b) {
                    o = x.eval(v)?.cmp(&y.eval(v)?);
                    if o != std::cmp::Ordering::Equal {
                        break;
                    }
                }
                rel(o, *r)
            }
            Pred::In(t, ranges) => {
                let x = t.eval(v)?;
                ranges.iter().any(|&(lo, hi)| lo <= x && x <= hi)
            }
            Pred::Not(p) => !p.eval(v)?,
            Pred::And(ps) => {
                for p in ps {
                    if !p.eval(v)? {
                        return Some(false);
                    }
                }
                true
            }
            Pred::Or(ps) => {
                for p in ps {
                    if p.eval(v)? {
                        return Some(true);
                    }
                }
                false
            }
        })
    }

    pub fn vars(&self, out: &mut Vec<u32>) {
        match self {
            Pred::Bool(_) => {}
            Pred::Cmp(a, _, b) => {
                a.vars(out);
                b.vars(out);
            }
            Pred::Bytes(a, _, b) => {
                for t in a.iter().chain(b) {
                    t.vars(out);
                }
            }
            Pred::In(t, _) => t.vars(out),
            Pred::Not(p) => p.vars(out),
            Pred::And(ps) | Pred::Or(ps) => {
                for p in ps {
                    p.vars(out);
                }
            }
        }
    }
}

/// The kind of an input variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKind {
    /// A number of `digits` digits (the integer of its digits, the scale
    /// applied by the terms using it).
    Num { digits: u32, signed: bool },
    /// A byte of alphanumeric data.
    Byte,
}

/// An input variable: its name (stable across runs), kind and value in
/// this run.
#[derive(Debug, Clone)]
pub struct Var {
    pub name: String,
    pub kind: VarKind,
    pub value: i128,
}

impl Var {
    /// The range of its values.
    pub fn range(&self) -> (i128, i128) {
        match self.kind {
            VarKind::Num { digits, signed } => {
                let m = 10i128.pow(digits.min(36)) - 1;
                (if signed { -m } else { 0 }, m)
            }
            VarKind::Byte => (0, 255),
        }
    }
}

/// A conditional jump taken in a run: program, instruction, whether it
/// jumped, and the condition deciding it (true: the jump is taken).
#[derive(Debug, Clone)]
pub struct Branch {
    pub program: String,
    pub pc: u32,
    pub jumped: bool,
    /// None: the condition does not depend on the inputs.
    pub pred: Option<Pred>,
}

/// A step of a run's path: a decision on a condition over the inputs.
#[derive(Debug, Clone)]
pub enum Step {
    Branch(Branch),
    /// A condition the run relied on without a branch (concretized
    /// values, valid digits).
    Assume(Pred),
}

/// Numeric data following an input: its place, type and term.
#[derive(Debug, Clone)]
pub(crate) struct NumShadow {
    pub addr: usize,
    pub len: usize,
    pub ty: crate::zir::NumType,
    pub term: Term,
    /// The bytes it had when the term was bound (a different content
    /// means an instruction not followed changed it).
    pub bytes: Vec<u8>,
}

/// A field of an input file's record: its name, place in the record and
/// numeric type (None: bytes).
#[derive(Debug, Clone)]
pub struct RecordField {
    pub name: String,
    pub offset: usize,
    pub len: usize,
    pub num: Option<crate::zir::NumType>,
}

/// An input file whose records are inputs: record `n` has the variables
/// `F<file>.R<n>.<field>` (numeric) or `F<file>.R<n>.<field>#<i>`
/// (bytes), and its number of records is the variable `count`
/// (`F<file>.N`).
///
/// Sequential files: record `n` is the `n`-th read, and AT END of that
/// READ is `count < n`. Indexed files (`keys` not empty): record `n` is
/// the `n`-th of `records`, and a READ or START by key finds a record
/// when `n <= count` and its key compares with the search key; one record
/// more than there are (`blank`) lets the solver add a record.
#[derive(Debug, Clone, Default)]
pub struct FileInput {
    /// The file's name (FD).
    pub name: String,
    pub fields: Vec<RecordField>,
    pub count: u32,
    /// Indexed files: the primary key and the alternate keys (offset and
    /// length in the record).
    pub keys: Vec<(usize, usize)>,
    /// Indexed files: the records, in the order of their variables.
    pub records: Vec<Vec<u8>>,
    /// Indexed files: the contents of a record added.
    pub blank: Vec<u8>,
    /// READ NEXTs since the file was opened or positioned by START.
    pub reads: u64,
    /// Records were written, rewritten or deleted (the data set is no
    /// longer the inputs).
    pub modified: bool,
    /// Indexed files: the number of records READ NEXT gets from the
    /// position (all from the open, those from the key after START; None:
    /// not known, after a READ by key or a change).
    pub left: Option<Term>,
}

/// The symbolic state of a run.
#[derive(Debug, Default)]
pub struct SymState {
    pub vars: Vec<Var>,
    /// The variables by name.
    names: std::collections::HashMap<String, u32>,
    /// Variables at most (see `full`).
    vars_cap: usize,
    /// Directions recorded after the path became long (each once).
    pub(crate) late: std::collections::HashSet<(String, u32, bool)>,
    pub path: Vec<Step>,
    /// Instructions executed (the run stops at `limit`).
    pub steps: u64,
    pub limit: u64,
    pub(crate) nums: Vec<NumShadow>,
    /// Bytes following inputs: address -> (term, byte when bound).
    pub(crate) bytes: std::collections::HashMap<usize, (Term, u8)>,
    /// Temporaries holding input terms: (instance, temp) -> (term, scale).
    pub(crate) temps: std::collections::HashMap<(usize, u32), (Term, i32)>,
    /// The size-error flag as a condition (None: concrete).
    pub(crate) size_error: Option<Pred>,
    /// ACCEPT statements executed (their data are inputs).
    pub accepts: u32,
    /// Values of the input variables for ACCEPT data, by name, for the
    /// variables created while running.
    pub preset: std::collections::HashMap<String, i128>,
    /// Input files read sequentially.
    pub files: Vec<FileInput>,
    /// The I/O condition of the last I/O statement as a condition (None:
    /// concrete).
    pub(crate) io_cond: Option<Pred>,
    /// The program and instruction being run.
    pub(crate) at: Option<(String, u32)>,
}

/// The variables a run has at most (see `SymState::full`).
pub const MAX_VARS: usize = 20_000;
/// Variables made after the inputs at least (see `inputs_bound`).
pub const LATE_VARS: usize = 4_000;

/// The nodes a term bound to data has at most: a larger one (a value
/// computed again and again in a loop) is left concrete.
pub const MAX_TERM: usize = 2_000;

/// The steps a run's path has with conditions at most (`SymState::long`).
pub const MAX_STEPS: usize = 50_000;

impl SymState {
    pub fn new(limit: u64) -> Self {
        SymState {
            limit,
            ..Default::default()
        }
    }

    /// A new input variable.
    pub fn var(&mut self, name: String, kind: VarKind, value: i128) -> u32 {
        self.names.insert(name.clone(), self.vars.len() as u32);
        self.vars.push(Var { name, kind, value });
        (self.vars.len() - 1) as u32
    }

    /// Whether a run has as many variables as it can have: data from
    /// outside (stubs) is no longer followed, so that a program that loops
    /// on it does not make the run too large to solve.
    pub fn full(&self) -> bool {
        self.vars.len() >= self.vars_cap.max(MAX_VARS)
    }

    /// The variables made so far are the inputs of the run: past
    /// `MAX_VARS`, the limit (`full`) leaves room for `LATE_VARS` more (a
    /// large COMMAREA does not stop the stubs' outputs from being followed).
    pub fn inputs_bound(&mut self) {
        self.vars_cap = self.vars.len() + LATE_VARS;
    }

    /// Records a condition the run relies on (not when the path is long:
    /// the decisions after that have no conditions).
    pub fn assume(&mut self, p: Pred) {
        if !self.long() {
            self.path.push(Step::Assume(p));
        }
    }

    /// Whether the path is as long as it can be: further decisions are
    /// recorded without their conditions (the directions still count).
    pub fn long(&self) -> bool {
        self.path.len() >= MAX_STEPS
    }

    /// The variable named `name`, created with `value` if there is none.
    pub fn var_named(&mut self, name: String, kind: VarKind, value: i128) -> u32 {
        match self.names.get(&name) {
            Some(&i) => i,
            None => self.var(name, kind, value),
        }
    }

    pub fn value(&self, v: u32) -> i128 {
        self.vars[v as usize].value
    }
}

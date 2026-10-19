//! The built-in solver: finds values of the input variables that make a
//! set of conditions true, starting from the values of a run (so that
//! only what the conditions need changes).
//!
//! - Infeasibility: conditions on a single variable that are linear give
//!   it an interval; an empty interval proves there is no solution.
//! - Search: each condition has a distance (0 when it holds: how far a
//!   comparison is from its other side); moves change one variable to a
//!   value that solves an atom for it (linear atoms directly, monotone
//!   ones by bisection), to the constants of the conditions, or by
//!   growing steps (the alternating variable method), keeping the move
//!   that lowers the total distance most.

use std::collections::{HashMap, HashSet};

use zeus_cobol::concolic::{Pred, Term, Var, VarKind};
use zeus_cobol::zir::Rel;

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    /// Values of the variables (by index).
    Sat(Vec<i128>),
    /// No solution (proved).
    Unsat,
    /// None found within the budget.
    Unknown,
}

/// A solver for the conditions of runs (a trait, so that an SMT solver
/// can take the built-in one's place).
pub trait Solver {
    fn solve(&mut self, cons: &[Pred], vars: &[Var], start: &[i128]) -> Outcome;
}

/// The built-in solver.
pub struct Builtin {
    /// Search steps per call (each tries the candidate moves).
    pub budget: usize,
    /// Time per call.
    pub time: std::time::Duration,
    /// An SMT solver for what the search does not solve in time.
    pub smt: Option<crate::smt::Z3>,
    /// Whether to use it for the next calls.
    pub use_smt: bool,
    seed: u64,
}

impl Default for Builtin {
    fn default() -> Self {
        Builtin {
            budget: 400,
            time: std::time::Duration::from_millis(150),
            smt: None,
            use_smt: true,
            seed: 0x9E37_79B9_7F4A_7C15,
        }
    }
}

const FALSE: f64 = 1.0e6;

fn rel_dist(x: i128, r: Rel) -> f64 {
    let f = x as f64;
    match r {
        Rel::Eq => f.abs(),
        Rel::Ne => {
            if x == 0 {
                1.0
            } else {
                0.0
            }
        }
        Rel::Lt => {
            if x < 0 {
                0.0
            } else {
                f + 1.0
            }
        }
        Rel::Le => {
            if x <= 0 {
                0.0
            } else {
                f
            }
        }
        Rel::Gt => {
            if x > 0 {
                0.0
            } else {
                -f + 1.0
            }
        }
        Rel::Ge => {
            if x >= 0 {
                0.0
            } else {
                -f
            }
        }
    }
}

fn flip(r: Rel) -> Rel {
    match r {
        Rel::Eq => Rel::Ne,
        Rel::Ne => Rel::Eq,
        Rel::Lt => Rel::Ge,
        Rel::Le => Rel::Gt,
        Rel::Gt => Rel::Le,
        Rel::Ge => Rel::Lt,
    }
}

/// The distance of a condition from holding (0: it holds).
pub fn dist(p: &Pred, neg: bool, v: &dyn Fn(u32) -> i128) -> f64 {
    match p {
        Pred::Bool(b) => {
            if *b != neg {
                0.0
            } else {
                FALSE
            }
        }
        Pred::Not(q) => dist(q, !neg, v),
        Pred::And(ps) | Pred::Or(ps) => {
            let all = matches!(p, Pred::And(_)) != neg;
            let ds = ps.iter().map(|q| dist(q, neg, v));
            if all {
                ds.sum()
            } else {
                ds.fold(f64::INFINITY, f64::min).min(FALSE)
            }
        }
        Pred::Cmp(a, r, b) => {
            let r = if neg { flip(*r) } else { *r };
            match (a.eval(v), b.eval(v)) {
                (Some(x), Some(y)) => x.checked_sub(y).map_or(FALSE, |d| rel_dist(d, r)),
                _ => FALSE,
            }
        }
        Pred::Bytes(a, r, b) => {
            let r = if neg { flip(*r) } else { *r };
            let mut xs = Vec::with_capacity(a.len());
            for (x, y) in a.iter().zip(b) {
                match (x.eval(v), y.eval(v)) {
                    (Some(x), Some(y)) => xs.push(x - y),
                    _ => return FALSE,
                }
            }
            let first = xs.iter().position(|&d| d != 0);
            match r {
                Rel::Eq => xs.iter().map(|d| d.abs() as f64).sum(),
                Rel::Ne => {
                    if first.is_some() {
                        0.0
                    } else {
                        1.0
                    }
                }
                _ => match first {
                    // equal strings: one byte must change
                    None => rel_dist(0, r),
                    Some(i) => rel_dist(xs[i], r),
                },
            }
        }
        Pred::In(t, ranges) => {
            let Some(x) = t.eval(v) else { return FALSE };
            let inside = ranges.iter().find(|&&(lo, hi)| lo <= x && x <= hi);
            match (inside, neg) {
                (Some(_), false) | (None, true) => 0.0,
                (None, false) => ranges
                    .iter()
                    .map(|&(lo, hi)| {
                        if x < lo {
                            (lo - x) as f64
                        } else {
                            (x - hi) as f64
                        }
                    })
                    .fold(FALSE, f64::min),
                (Some(&(lo, hi)), true) => ((x - lo + 1).min(hi - x + 1)) as f64,
            }
        }
    }
}

/// The constants of a condition.
fn constants(p: &Pred, out: &mut HashSet<i128>) {
    fn term(t: &Term, out: &mut HashSet<i128>) {
        match t {
            Term::Const(c) => {
                out.insert(*c);
            }
            Term::Var(_) => {}
            Term::Add(a, b) | Term::Sub(a, b) | Term::Mul(a, b) => {
                term(a, out);
                term(b, out);
            }
            Term::Neg(a) | Term::Abs(a) | Term::Div(a, _) | Term::Rem(a, _) => term(a, out),
            Term::If(p, a, b) => {
                constants(p, out);
                term(a, out);
                term(b, out);
            }
        }
    }
    match p {
        Pred::Bool(_) => {}
        Pred::Cmp(a, _, b) => {
            term(a, out);
            term(b, out);
        }
        Pred::Bytes(a, _, b) => {
            for t in a.iter().chain(b) {
                term(t, out);
            }
        }
        Pred::In(t, r) => {
            term(t, out);
            for &(lo, hi) in r {
                out.insert(lo);
                out.insert(hi);
            }
        }
        Pred::Not(q) => constants(q, out),
        Pred::And(ps) | Pred::Or(ps) => {
            for q in ps {
                constants(q, out);
            }
        }
    }
}

/// Pairs of terms whose difference decides an atom (for solving a
/// variable out of it).
fn atoms<'a>(p: &'a Pred, out: &mut Vec<(&'a Term, &'a Term)>) {
    match p {
        Pred::Bool(_) => {}
        Pred::Cmp(a, _, b) => out.push((a, b)),
        Pred::Bytes(a, _, b) => out.extend(a.iter().zip(b)),
        Pred::In(t, _) => out.push((t, t)),
        Pred::Not(q) => atoms(q, out),
        Pred::And(ps) | Pred::Or(ps) => {
            for q in ps {
                atoms(q, out);
            }
        }
    }
}

/// Whether a term is linear (sums, constant multiples).
fn linear(t: &Term) -> bool {
    match t {
        Term::Const(_) | Term::Var(_) => true,
        Term::Add(a, b) | Term::Sub(a, b) => linear(a) && linear(b),
        Term::Mul(a, b) => (a.konst().is_some() && linear(b)) || (b.konst().is_some() && linear(a)),
        Term::Neg(a) => linear(a),
        _ => false,
    }
}

/// A condition with its negations moved to the atoms (negation normal
/// form), constant parts folded.
pub fn nnf(p: &Pred, neg: bool) -> Pred {
    match p {
        Pred::Bool(b) => Pred::Bool(*b != neg),
        Pred::Not(q) => nnf(q, !neg),
        Pred::And(ps) | Pred::Or(ps) => {
            let and = matches!(p, Pred::And(_)) != neg;
            let mut out = Vec::new();
            for q in ps {
                match nnf(q, neg) {
                    Pred::Bool(b) if b == and => {}
                    Pred::Bool(b) => return Pred::Bool(b),
                    q => out.push(q),
                }
            }
            match out.len() {
                0 => Pred::Bool(and),
                1 => out.pop().unwrap(),
                _ if and => Pred::And(out),
                _ => Pred::Or(out),
            }
        }
        Pred::Cmp(a, r, b) => Pred::Cmp(a.clone(), if neg { flip(*r) } else { *r }, b.clone()),
        Pred::Bytes(a, r, b) => Pred::Bytes(a.clone(), if neg { flip(*r) } else { *r }, b.clone()),
        Pred::In(..) if neg => Pred::Not(Box::new(p.clone())),
        Pred::In(..) => p.clone(),
    }
}

/// Splits a conjunction into its conditions.
fn flatten(p: &Pred, out: &mut Vec<Pred>) {
    match p {
        Pred::And(ps) => {
            for q in ps {
                flatten(q, out);
            }
        }
        p => out.push(p.clone()),
    }
}

impl Builtin {
    fn rand(&mut self) -> u64 {
        // xorshift64*
        self.seed ^= self.seed >> 12;
        self.seed ^= self.seed << 25;
        self.seed ^= self.seed >> 27;
        self.seed.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Domains narrowed by the linear conditions on single variables;
    /// None: a domain is empty.
    fn domains(cons: &[Pred], vars: &[Var]) -> Option<HashMap<u32, (i128, i128)>> {
        let mut dom: HashMap<u32, (i128, i128)> = HashMap::new();
        for _ in 0..2 {
            for c in cons {
                let (t, rel, ranges) = match c {
                    Pred::Bool(false) => return None,
                    Pred::Cmp(a, r, b) => (Term::sub(a.clone(), b.clone()), Some(*r), None),
                    Pred::Not(q) => match &**q {
                        Pred::Cmp(a, r, b) => {
                            (Term::sub(a.clone(), b.clone()), Some(flip(*r)), None)
                        }
                        _ => continue,
                    },
                    Pred::In(t, r) => (t.clone(), None, Some(r)),
                    _ => continue,
                };
                let mut vs = Vec::new();
                t.vars(&mut vs);
                if vs.len() != 1 || !linear(&t) {
                    continue;
                }
                let v = vs[0];
                let (lo, hi) = *dom.entry(v).or_insert_with(|| vars[v as usize].range());
                // f(x) = s*x + c
                let at = |x: i128| t.eval(&|_| x);
                let (Some(c), Some(f1)) = (at(0), at(1)) else {
                    continue;
                };
                let s = f1 - c;
                let (mut nlo, mut nhi) = (lo, hi);
                match (rel, ranges) {
                    (Some(r), _) if s != 0 => {
                        // s*x + c r 0
                        let root = |up: bool| -> i128 {
                            let (n, d) = (-c, s);
                            let q = n.div_euclid(d);
                            if up && q * d != n { q + 1 } else { q }
                        };
                        let r = if s < 0 {
                            match r {
                                Rel::Lt => Rel::Gt,
                                Rel::Le => Rel::Ge,
                                Rel::Gt => Rel::Lt,
                                Rel::Ge => Rel::Le,
                                r => r,
                            }
                        } else {
                            r
                        };
                        let exact = (-c) % s == 0;
                        match r {
                            Rel::Eq => {
                                if !exact {
                                    return None;
                                }
                                nlo = nlo.max(-c / s);
                                nhi = nhi.min(-c / s);
                            }
                            Rel::Ne => {}
                            Rel::Lt => nhi = nhi.min(if exact { -c / s - 1 } else { root(false) }),
                            Rel::Le => nhi = nhi.min(root(false)),
                            Rel::Gt => nlo = nlo.max(if exact { -c / s + 1 } else { root(true) }),
                            Rel::Ge => nlo = nlo.max(root(true)),
                        }
                    }
                    (Some(r), _) => {
                        // a constant: holds or not
                        if rel_dist(c, r) > 0.0 {
                            return None;
                        }
                    }
                    (None, Some(rs)) if s == 1 => {
                        let l = rs.iter().map(|r| r.0 - c).min().unwrap_or(lo);
                        let h = rs.iter().map(|r| r.1 - c).max().unwrap_or(hi);
                        nlo = nlo.max(l);
                        nhi = nhi.min(h);
                    }
                    _ => {}
                }
                if nlo > nhi {
                    return None;
                }
                dom.insert(v, (nlo, nhi));
            }
        }
        Some(dom)
    }
}

impl Solver for Builtin {
    /// The search, then the SMT solver (if any) when the search found
    /// nothing in time.
    fn solve(&mut self, cons: &[Pred], vars: &[Var], start: &[i128]) -> Outcome {
        match self.search(cons, vars, start) {
            Outcome::Unknown => match &mut self.smt {
                Some(z3) if self.use_smt => z3.solve(cons, vars, start),
                _ => Outcome::Unknown,
            },
            r => r,
        }
    }
}

impl Builtin {
    /// Interval reasoning and the local search.
    fn search(&mut self, cons: &[Pred], vars: &[Var], start: &[i128]) -> Outcome {
        let mut flat = Vec::new();
        for c in cons {
            flatten(&nnf(c, false), &mut flat);
        }
        flat.retain(|c| c != &Pred::Bool(true));
        let Some(dom) = Self::domains(&flat, vars) else {
            return Outcome::Unsat;
        };
        let range = |v: u32| {
            dom.get(&v)
                .copied()
                .unwrap_or_else(|| vars[v as usize].range())
        };
        let mut val: Vec<i128> = start.to_vec();
        for (i, x) in val.iter_mut().enumerate() {
            let (lo, hi) = range(i as u32);
            *x = (*x).clamp(lo, hi);
        }
        let mut consts = HashSet::new();
        for c in &flat {
            constants(c, &mut consts);
        }
        let consts: Vec<i128> = consts.into_iter().collect();
        // the conditions of each variable (a move is scored on those)
        let mut occurs: HashMap<u32, Vec<usize>> = HashMap::new();
        for (i, c) in flat.iter().enumerate() {
            let mut vs = Vec::new();
            c.vars(&mut vs);
            for v in vs {
                occurs.entry(v).or_default().push(i);
            }
        }
        let each = |val: &[i128]| -> Vec<f64> {
            let f = |v: u32| val[v as usize];
            flat.iter().map(|c| dist(c, false, &f)).collect()
        };
        let mut dists = each(&val);
        let mut cur: f64 = dists.iter().sum();
        let mut best = (cur, val.clone());
        let mut stall = 0;
        let mut moves = 0;
        let started = std::time::Instant::now();
        while moves < self.budget && started.elapsed() < self.time {
            if cur == 0.0 {
                return Outcome::Sat(val);
            }
            // the variables of the conditions that do not hold
            let f = |v: u32| val[v as usize];
            let mut vs: Vec<u32> = Vec::new();
            let mut ats = Vec::new();
            for c in flat.iter().filter(|c| dist(c, false, &f) > 0.0) {
                c.vars(&mut vs);
                atoms(c, &mut ats);
            }
            if vs.is_empty() {
                // ground conditions that do not hold
                return Outcome::Unsat;
            }
            let mut cand: Vec<(u32, i128)> = Vec::new();
            for &v in &vs {
                let (lo, hi) = range(v);
                let x0 = val[v as usize];
                let mut push = |x: i128| {
                    if (lo..=hi).contains(&x) && x != x0 {
                        cand.push((v, x));
                    }
                };
                if matches!(vars[v as usize].kind, VarKind::Byte) || hi - lo <= 300 {
                    for x in lo..=hi {
                        push(x);
                    }
                    continue;
                }
                for x in [lo, hi, 0, 1, -1, x0 + 1, x0 - 1] {
                    push(x);
                }
                for &c in &consts {
                    for x in [c - 1, c, c + 1] {
                        push(x);
                    }
                }
                let mut k = 10i128;
                while k < hi {
                    push(x0 + k);
                    push(x0 - k);
                    k = k.saturating_mul(10);
                }
                // solve each atom for the variable
                for (a, b) in &ats {
                    let mut tv = Vec::new();
                    a.vars(&mut tv);
                    b.vars(&mut tv);
                    if !tv.contains(&v) {
                        continue;
                    }
                    let g = |x: i128| -> Option<i128> {
                        let h = |w: u32| if w == v { x } else { val[w as usize] };
                        a.eval(&h)?
                            .checked_sub(if std::ptr::eq(*a, *b) { 0 } else { b.eval(&h)? })
                    };
                    let (Some(f0), Some(f1), Some(f2)) = (g(x0), g(x0 + 1), g(x0 + 2)) else {
                        continue;
                    };
                    let s = f1 - f0;
                    if s != 0 && f2 - f1 == s {
                        let r = x0 - f0 / s;
                        for x in [r - 1, r, r + 1] {
                            push(x);
                        }
                    } else if let (Some(gl), Some(gh)) = (g(lo), g(hi))
                        && gl.signum() != gh.signum()
                    {
                        // bisection for a change of sign (monotone terms)
                        let (mut l, mut h) = (lo, hi);
                        for _ in 0..130 {
                            if h - l <= 1 {
                                break;
                            }
                            let m = l + (h - l) / 2;
                            match g(m) {
                                Some(gm) if gm.signum() == gl.signum() => l = m,
                                Some(_) => h = m,
                                None => break,
                            }
                        }
                        for x in [l, h, h + 1, l - 1] {
                            push(x);
                        }
                    }
                }
            }
            // the best move
            let mut best_move: Option<(f64, u32, i128)> = None;
            moves += 1;
            for (n, &(v, x)) in cand.iter().enumerate() {
                if n % 512 == 511 && started.elapsed() >= self.time {
                    break;
                }
                let old = val[v as usize];
                val[v as usize] = x;
                let f = |w: u32| val[w as usize];
                let mut s = cur;
                for &i in occurs.get(&v).map_or(&[][..], |o| &o[..]) {
                    s += dist(&flat[i], false, &f) - dists[i];
                }
                val[v as usize] = old;
                if best_move.is_none_or(|b| s < b.0) {
                    best_move = Some((s, v, x));
                }
            }
            match best_move {
                Some((s, v, x)) if s < cur - 1e-9 => {
                    val[v as usize] = x;
                    let f = |w: u32| val[w as usize];
                    for &i in occurs.get(&v).map_or(&[][..], |o| &o[..]) {
                        dists[i] = dist(&flat[i], false, &f);
                    }
                    cur = dists.iter().sum();
                    stall = 0;
                    if cur < best.0 {
                        best = (cur, val.clone());
                    }
                }
                _ => {
                    // stuck: a random move among the candidates, else
                    // start again near the best values
                    stall += 1;
                    if stall > 20 || cand.is_empty() {
                        val = best.1.clone();
                        stall = 0;
                    }
                    if !cand.is_empty() {
                        let (v, x) = cand[(self.rand() % cand.len() as u64) as usize];
                        val[v as usize] = x;
                    }
                    dists = each(&val);
                    cur = dists.iter().sum();
                }
            }
        }
        if best.0 == 0.0 {
            return Outcome::Sat(best.1);
        }
        Outcome::Unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(name: &str, digits: u32, signed: bool) -> Var {
        Var {
            name: name.into(),
            kind: VarKind::Num { digits, signed },
            value: 0,
        }
    }

    fn v(i: u32) -> Term {
        Term::Var(i)
    }

    fn c(x: i128) -> Term {
        Term::Const(x)
    }

    #[test]
    fn linear_and_ranges() {
        let vars = vec![num("A", 5, false), num("B", 3, true)];
        // A > 1000 and A + B = 1234 and B < 0
        let cons = vec![
            Pred::Cmp(v(0), Rel::Gt, c(1000)),
            Pred::Cmp(Term::add(v(0), v(1)), Rel::Eq, c(1234)),
            Pred::Cmp(v(1), Rel::Lt, c(0)),
        ];
        let Outcome::Sat(x) = Builtin::default().solve(&cons, &vars, &[0, 0]) else {
            panic!("no solution")
        };
        assert!(x[0] > 1000 && x[0] + x[1] == 1234 && x[1] < 0, "{x:?}");
    }

    #[test]
    fn strings_and_division() {
        let vars: Vec<Var> = (0..3)
            .map(|i| Var {
                name: format!("B{i}"),
                kind: VarKind::Byte,
                value: 0x40,
            })
            .chain([num("N", 7, false)])
            .collect();
        // bytes = 'ABC' (EBCDIC) and N / 7 = 1234
        let cons = vec![
            Pred::Bytes(
                vec![v(0), v(1), v(2)],
                Rel::Eq,
                vec![c(0xC1), c(0xC2), c(0xC3)],
            ),
            Pred::Cmp(Term::div(v(3), 7), Rel::Eq, c(1234)),
        ];
        let Outcome::Sat(x) = Builtin::default().solve(&cons, &vars, &[0x40, 0x40, 0x40, 0]) else {
            panic!("no solution")
        };
        assert_eq!(&x[..3], &[0xC1, 0xC2, 0xC3]);
        assert_eq!(x[3] / 7, 1234);
    }

    #[test]
    fn infeasible() {
        let vars = vec![num("A", 3, false)];
        let cons = vec![
            Pred::Cmp(v(0), Rel::Gt, c(5)),
            Pred::Cmp(v(0), Rel::Lt, c(3)),
        ];
        assert_eq!(Builtin::default().solve(&cons, &vars, &[0]), Outcome::Unsat);
        // out of the digits
        let cons = vec![Pred::Cmp(v(0), Rel::Ge, c(1000))];
        assert_eq!(Builtin::default().solve(&cons, &vars, &[0]), Outcome::Unsat);
    }
}

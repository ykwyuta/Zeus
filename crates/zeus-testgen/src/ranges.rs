//! Branch directions proved impossible by the ranges of numeric items:
//! an abstract interpretation with intervals, like the flags' values in
//! `infeasible` (the same control flow).
//!
//! An item here is a numeric elementary item of the working-storage (no
//! table, not in the local storage, not set by the run time). Its
//! interval is that of its digits (the integer of its digits, at its
//! scale) and starts at its VALUE. A MOVE of a constant, `X + k`,
//! `X - k`, `k * X` (ADD, SUBTRACT, COMPUTE, SET, PERFORM VARYING) are
//! followed; any other instruction that may set it makes it unknown (any
//! value of its picture). Each way of a conditional jump comparing it to
//! a constant (a literal, or an item never set: its VALUE) keeps the part
//! of the interval that goes that way. Loops are widened. A direction no
//! value goes is impossible:
//!
//! ```cobol
//! PERFORM VARYING WS-IDX FROM 1 BY 1 UNTIL WS-IDX > WS-COUNT   *> VALUE 11
//!     EVALUATE WS-IDX
//!         WHEN 12 ...                     *> never: WS-IDX is 1 to 11
//! ```

use std::collections::BTreeSet;

use zeus_cobol::vm::data as vdata;
use zeus_cobol::zir::{
    Base, CmpMode, Cx, DataType, Ex, Instr, Loc, NumType, NumUsage, Op, ProgramCode, Rel, SignKind,
    Src,
};
use zeus_decimal::Dec;

use crate::infeasible::{extent, locs_in, successors};

/// Joins at a loop head before its interval is widened.
const WIDEN_AFTER: u32 = 4;
/// Joins elsewhere before the interval is widened.
const WIDEN_LATE: u32 = 64;

/// A closed interval of integers (the integer of the digits at `scale`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Iv {
    lo: i128,
    hi: i128,
}

impl Iv {
    fn one(x: i128) -> Iv {
        Iv { lo: x, hi: x }
    }

    fn join(self, o: Iv) -> Iv {
        Iv {
            lo: self.lo.min(o.lo),
            hi: self.hi.max(o.hi),
        }
    }

    fn meet(self, o: Iv) -> Option<Iv> {
        let r = Iv {
            lo: self.lo.max(o.lo),
            hi: self.hi.min(o.hi),
        };
        (r.lo <= r.hi).then_some(r)
    }
}

/// An interval with the scale of its integers.
#[derive(Debug, Clone, Copy)]
struct Val {
    iv: Iv,
    scale: u32,
}

fn pow10(n: u32) -> Option<i128> {
    10i128.checked_pow(n)
}

fn rescale(v: Val, scale: u32) -> Option<Iv> {
    if scale >= v.scale {
        let k = pow10(scale - v.scale)?;
        Some(Iv {
            lo: v.iv.lo.checked_mul(k)?,
            hi: v.iv.hi.checked_mul(k)?,
        })
    } else {
        // truncation toward zero keeps the order
        let k = pow10(v.scale - scale)?;
        Some(Iv {
            lo: v.iv.lo / k,
            hi: v.iv.hi / k,
        })
    }
}

/// The values a numeric item's storage can hold.
fn full(t: &NumType, len: usize) -> Iv {
    match t.usage {
        NumUsage::Binary | NumUsage::Native => {
            let bits = (8 * len.min(15)) as u32;
            if t.signed {
                Iv {
                    lo: -(1i128 << (bits - 1)),
                    hi: (1i128 << (bits - 1)) - 1,
                }
            } else {
                Iv {
                    lo: 0,
                    hi: (1i128 << bits) - 1,
                }
            }
        }
        _ => {
            let max = pow10(t.digits.min(36)).map_or(i128::MAX, |m| m - 1);
            Iv {
                lo: if t.signed { -max } else { 0 },
                hi: max,
            }
        }
    }
}

/// The range of the digits (what a store keeps without truncating).
fn digits(t: &NumType) -> Iv {
    let max = pow10(t.digits.min(36)).map_or(i128::MAX, |m| m - 1);
    Iv {
        lo: if t.signed { -max } else { 0 },
        hi: max,
    }
}

/// Special registers the run time sets.
const SPECIAL: &[&str] = &[
    "RETURN-CODE",
    "SORT-RETURN",
    "SORT-CONTROL",
    "SORT-CORE-SIZE",
    "SORT-FILE-SIZE",
    "SORT-MESSAGE",
    "SORT-MODE-SIZE",
    "TALLY",
    "LINAGE-COUNTER",
    "XML-CODE",
    "JSON-CODE",
];

struct Item {
    desc: u32,
    ty: NumType,
    len: usize,
    init: Iv,
}

/// Whether an item may be set by the run time without an instruction
/// (file definitions, record areas, special registers).
fn implicit(code: &ProgramCode, desc: u32) -> bool {
    let Some((b, fo, fe)) = extent(code, desc) else {
        return true;
    };
    let d = &code.descs[desc as usize];
    if SPECIAL.contains(&d.name.as_str()) {
        return true;
    }
    let overlaps = |x: u32| extent(code, x).is_some_and(|(b2, s, e)| b2 == b && s < fe && fo < e);
    let mut ls = Vec::new();
    locs_in(&code.files, &mut ls);
    if ls.iter().any(|&x| overlaps(x)) {
        return true;
    }
    code.files.iter().any(|f| {
        code.descs.get(f.record as usize).is_some_and(|r| {
            let (s, e) = (r.offset as usize, r.offset as usize + f.max_len as usize);
            r.base == b && s < fe && fo < e
        })
    })
}

fn item(code: &ProgramCode, desc: u32) -> Option<Item> {
    let d = code.descs.get(desc as usize)?;
    let DataType::Numeric(t) = d.ty else {
        return None;
    };
    if d.base != Base::Static
        || !d.dims.is_empty()
        || t.usage == NumUsage::National
        || code
            .local_storage
            .is_some_and(|(o, l)| d.offset >= o && d.offset < o + l)
        || implicit(code, desc)
    {
        return None;
    }
    let (o, l) = (d.offset as usize, d.len as usize);
    let v = vdata::num_value(&t, code.static_init.get(o..o + l)?);
    let raw = v
        .rescale(t.scale.max(0) as u32, zeus_decimal::Rounding::Truncate)?
        .coef;
    Some(Item {
        desc,
        ty: t,
        len: l,
        init: Iv::one(raw),
    })
}

fn is_loc(l: &Loc, desc: u32) -> bool {
    l.desc == desc && l.subs.is_empty() && l.refmod.is_none()
}

/// The items no instruction sets (their VALUE is their value).
fn constants(code: &ProgramCode) -> Vec<Option<Dec>> {
    let mut written = vec![false; code.descs.len()];
    let mark = |d: u32, written: &mut Vec<bool>| {
        if let Some((b, s, e)) = extent(code, d) {
            for (k, w) in written.iter_mut().enumerate() {
                if extent(code, k as u32).is_some_and(|(b2, s2, e2)| b2 == b && s2 < e && s < e2) {
                    *w = true;
                }
            }
        }
    };
    for i in &code.code {
        match i {
            Instr::Move(_, dst) | Instr::Store { dst, .. } | Instr::SetIndex(dst, _) => {
                mark(dst.desc, &mut written)
            }
            Instr::JumpIf(..)
            | Instr::JumpUnless(..)
            | Instr::Display { .. }
            | Instr::SetTemp(..)
            | Instr::GoDepending(..)
            | Instr::Jump(_)
            | Instr::Perform { .. }
            | Instr::ParaEnd(_) => {}
            Instr::Exec(k) => {
                let mut ls = Vec::new();
                if let Some(e) = code.execs.get(*k as usize) {
                    locs_in(e, &mut ls);
                }
                for l in ls {
                    mark(l, &mut written);
                }
            }
            other => {
                let mut ls = Vec::new();
                locs_in(other, &mut ls);
                for l in ls {
                    mark(l, &mut written);
                }
            }
        }
    }
    (0..code.descs.len() as u32)
        .map(|k| {
            let d = &code.descs[k as usize];
            let DataType::Numeric(t) = d.ty else {
                return None;
            };
            if written[k as usize]
                || d.base != Base::Static
                || !d.dims.is_empty()
                || implicit(code, k)
                || code
                    .local_storage
                    .is_some_and(|(o, l)| d.offset >= o && d.offset < o + l)
            {
                return None;
            }
            let (o, l) = (d.offset as usize, d.len as usize);
            Some(vdata::num_value(&t, code.static_init.get(o..o + l)?))
        })
        .collect()
}

struct Ctx<'a> {
    item: &'a Item,
    consts: &'a [Option<Dec>],
}

impl Ctx<'_> {
    fn scale(&self) -> u32 {
        self.item.ty.scale.max(0) as u32
    }

    /// A constant operand.
    fn constant_src(&self, s: &Src) -> Option<Dec> {
        match s {
            Src::NumLit(d, _) => Some(*d),
            Src::Fig(zeus_cobol::zir::Fig::Zero) => Some(Dec::from_i64(0)),
            Src::Loc(l) | Src::Num(Ex::Load(l)) if l.subs.is_empty() && l.refmod.is_none() => {
                self.consts.get(l.desc as usize).copied().flatten()
            }
            Src::Num(Ex::Lit(d)) => Some(*d),
            _ => None,
        }
    }

    /// The operand is the item.
    fn is_item(&self, s: &Src) -> bool {
        match s {
            Src::Loc(l) | Src::Num(Ex::Load(l)) => is_loc(l, self.item.desc),
            _ => false,
        }
    }

    /// The interval of an expression, given the item's.
    fn eval(&self, e: &Ex, x: Iv) -> Option<Val> {
        Some(match e {
            Ex::Lit(d) => Val {
                iv: Iv::one(d.coef),
                scale: d.scale,
            },
            Ex::Load(l) if is_loc(l, self.item.desc) => Val {
                iv: x,
                scale: self.scale(),
            },
            Ex::Load(l) if l.subs.is_empty() && l.refmod.is_none() => {
                let d = self.consts.get(l.desc as usize).copied().flatten()?;
                Val {
                    iv: Iv::one(d.coef),
                    scale: d.scale,
                }
            }
            Ex::Neg(a) => {
                let v = self.eval(a, x)?;
                Val {
                    iv: Iv {
                        lo: v.iv.hi.checked_neg()?,
                        hi: v.iv.lo.checked_neg()?,
                    },
                    scale: v.scale,
                }
            }
            Ex::Bin(op @ (Op::Add | Op::Sub), a, b) => {
                let (a, b) = (self.eval(a, x)?, self.eval(b, x)?);
                let s = a.scale.max(b.scale);
                let (p, q) = (rescale(a, s)?, rescale(b, s)?);
                let iv = if *op == Op::Add {
                    Iv {
                        lo: p.lo.checked_add(q.lo)?,
                        hi: p.hi.checked_add(q.hi)?,
                    }
                } else {
                    Iv {
                        lo: p.lo.checked_sub(q.hi)?,
                        hi: p.hi.checked_sub(q.lo)?,
                    }
                };
                Val { iv, scale: s }
            }
            Ex::Bin(Op::Mul, a, b) => {
                let (a, b) = (self.eval(a, x)?, self.eval(b, x)?);
                let c = [
                    a.iv.lo.checked_mul(b.iv.lo)?,
                    a.iv.lo.checked_mul(b.iv.hi)?,
                    a.iv.hi.checked_mul(b.iv.lo)?,
                    a.iv.hi.checked_mul(b.iv.hi)?,
                ];
                Val {
                    iv: Iv {
                        lo: *c.iter().min()?,
                        hi: *c.iter().max()?,
                    },
                    scale: a.scale + b.scale,
                }
            }
            _ => return None,
        })
    }

    /// The item after a store of `value` (None: unknown).
    fn store(&self, value: &Ex, x: Iv, rounded: bool, check: bool) -> Iv {
        let t = &self.item.ty;
        let all = full(t, self.item.len);
        let Some(v) = self.eval(value, x).and_then(|v| rescale(v, self.scale())) else {
            return all;
        };
        let mut v = if rounded {
            Iv {
                lo: v.lo.saturating_sub(1),
                hi: v.hi.saturating_add(1),
            }
        } else {
            v
        };
        if !t.signed && v.lo < 0 {
            // an unsigned item keeps the absolute value
            let m = v.lo.unsigned_abs().max(v.hi.unsigned_abs()) as i128;
            v = Iv {
                lo: if v.hi >= 0 { 0 } else { v.hi.abs() },
                hi: m,
            };
        }
        let range = digits(t);
        match v.meet(range) {
            Some(inside) if inside == v => v,
            // ON SIZE ERROR: the item stays as it was when it overflows
            inside if check => inside.map_or(x, |i| i.join(x)),
            _ => all,
        }
    }

    /// (may hold, may not hold) for a condition.
    fn holds(&self, c: &Cx, x: Iv) -> (bool, bool) {
        match c {
            Cx::True => (true, false),
            Cx::Not(a) => {
                let (t, f) = self.holds(a, x);
                (f, t)
            }
            Cx::And(a, b) => {
                let (at, af) = self.holds(a, x);
                let (bt, bf) = self.holds(b, x);
                (at && bt, af || bf)
            }
            Cx::Or(a, b) => {
                let (at, af) = self.holds(a, x);
                let (bt, bf) = self.holds(b, x);
                (at || bt, af && bf)
            }
            _ => match self.atom(c) {
                Some((rel, k)) => (
                    refine(x, rel, k, self.scale()).is_some(),
                    refine(x, negate(rel), k, self.scale()).is_some(),
                ),
                None => (true, true),
            },
        }
    }

    /// A condition `item rel constant`.
    fn atom(&self, c: &Cx) -> Option<(Rel, Dec)> {
        match c {
            Cx::Cmp(a, r, b, CmpMode::Numeric) => {
                if self.is_item(a) {
                    Some((*r, self.constant_src(b)?))
                } else if self.is_item(b) {
                    Some((flip(*r), self.constant_src(a)?))
                } else {
                    None
                }
            }
            Cx::Sign(Ex::Load(l), k) if is_loc(l, self.item.desc) => Some((
                match k {
                    SignKind::Positive => Rel::Gt,
                    SignKind::Negative => Rel::Lt,
                    SignKind::Zero => Rel::Eq,
                },
                Dec::from_i64(0),
            )),
            _ => None,
        }
    }

    /// The item's interval where the condition is `want`.
    fn narrow(&self, c: &Cx, x: Iv, want: bool) -> Option<Iv> {
        match c {
            Cx::Not(a) => self.narrow(a, x, !want),
            Cx::And(a, b) if want => self.narrow(b, self.narrow(a, x, true)?, true),
            Cx::Or(a, b) if !want => self.narrow(b, self.narrow(a, x, false)?, false),
            Cx::And(a, b) | Cx::Or(a, b) => {
                match (self.narrow(a, x, want), self.narrow(b, x, want)) {
                    (Some(p), Some(q)) => Some(p.join(q)),
                    (p, q) => p.or(q),
                }
            }
            _ => match self.atom(c) {
                Some((rel, k)) => refine(x, if want { rel } else { negate(rel) }, k, self.scale()),
                None => {
                    let (t, f) = self.holds(c, x);
                    (if want { t } else { f }).then_some(x)
                }
            },
        }
    }
}

fn flip(r: Rel) -> Rel {
    match r {
        Rel::Lt => Rel::Gt,
        Rel::Le => Rel::Ge,
        Rel::Gt => Rel::Lt,
        Rel::Ge => Rel::Le,
        r => r,
    }
}

fn negate(r: Rel) -> Rel {
    match r {
        Rel::Eq => Rel::Ne,
        Rel::Ne => Rel::Eq,
        Rel::Lt => Rel::Ge,
        Rel::Le => Rel::Gt,
        Rel::Gt => Rel::Le,
        Rel::Ge => Rel::Lt,
    }
}

/// The part of `x` (integers at `scale`) where `x rel k` holds.
fn refine(x: Iv, rel: Rel, k: Dec, scale: u32) -> Option<Iv> {
    // raw / 10^scale rel coef / 10^ks  <=>  raw * 10^ks rel coef * 10^scale
    let a = pow10(k.scale)?;
    let c = k.coef.checked_mul(pow10(scale)?)?;
    let floor = c.div_euclid(a);
    let exact = c.rem_euclid(a) == 0;
    let ceil = if exact { floor } else { floor + 1 };
    let r = match rel {
        Rel::Eq => {
            if !exact {
                return None;
            }
            Iv::one(floor)
        }
        Rel::Ne => {
            return if exact && x.lo == floor && x.hi == floor {
                None
            } else if exact && x.lo == floor {
                Some(Iv { lo: floor + 1, ..x })
            } else if exact && x.hi == floor {
                Some(Iv { hi: floor - 1, ..x })
            } else {
                Some(x)
            };
        }
        Rel::Lt => Iv {
            lo: i128::MIN,
            hi: ceil - 1,
        },
        Rel::Le => Iv {
            lo: i128::MIN,
            hi: floor,
        },
        Rel::Gt => Iv {
            lo: floor + 1,
            hi: i128::MAX,
        },
        Rel::Ge => Iv {
            lo: ceil,
            hi: i128::MAX,
        },
    };
    x.meet(r)
}

/// Whether an instruction may set the item other than as followed.
fn touches(code: &ProgramCode, ins: &Instr, desc: u32) -> bool {
    let Some((b, s, e)) = extent(code, desc) else {
        return true;
    };
    let overlaps = |x: u32| extent(code, x).is_some_and(|(b2, s2, e2)| b2 == b && s2 < e && s < e2);
    let mut ls = Vec::new();
    match ins {
        Instr::Move(_, dst) | Instr::Store { dst, .. } | Instr::SetIndex(dst, _) => {
            return overlaps(dst.desc);
        }
        Instr::JumpIf(..)
        | Instr::JumpUnless(..)
        | Instr::Display { .. }
        | Instr::SetTemp(..)
        | Instr::GoDepending(..)
        | Instr::Jump(_)
        | Instr::Perform { .. }
        | Instr::ParaEnd(_) => return false,
        Instr::Exec(k) => {
            if let Some(x) = code.execs.get(*k as usize) {
                locs_in(x, &mut ls);
            }
        }
        other => locs_in(other, &mut ls),
    }
    ls.iter().any(|&x| overlaps(x))
}

/// The directions (instruction, jumped) no value of a numeric item takes.
pub fn impossible(code: &ProgramCode) -> BTreeSet<(u32, bool)> {
    let mut out = BTreeSet::new();
    let Some(succ) = successors(code) else {
        return out;
    };
    let consts = constants(code);
    // the items compared in conditions
    let mut compared = BTreeSet::new();
    for i in &code.code {
        if let Instr::JumpIf(c, _) | Instr::JumpUnless(c, _) = i {
            cmp_items(c, &mut compared);
        }
    }
    let entries: Vec<u32> = std::iter::once(0)
        .chain(code.entries.iter().map(|e| e.pc))
        .collect();
    for desc in compared {
        let Some(it) = item(code, desc) else {
            continue;
        };
        if consts.get(desc as usize).copied().flatten().is_some() {
            continue; // a constant: the conditions on it are constant too
        }
        let cx = Ctx {
            item: &it,
            consts: &consts,
        };
        let n = code.code.len();
        let mut state: Vec<Option<Iv>> = vec![None; n];
        let mut joins = vec![0u32; n];
        let mut work: Vec<u32> = Vec::new();
        for &e in &entries {
            if (e as usize) < n {
                state[e as usize] = Some(it.init);
                work.push(e);
            }
        }
        let all = full(&it.ty, it.len);
        while let Some(pc) = work.pop() {
            let Some(x) = state[pc as usize] else {
                continue;
            };
            let ins = &code.code[pc as usize];
            let outs: Vec<(u32, Option<Iv>)> = match ins {
                Instr::JumpIf(c, t) | Instr::JumpUnless(c, t) => {
                    let yes = cx.narrow(c, x, true);
                    let no = cx.narrow(c, x, false);
                    let (taken, fall) = if matches!(ins, Instr::JumpIf(..)) {
                        (yes, no)
                    } else {
                        (no, yes)
                    };
                    vec![(*t, taken), (pc + 1, fall)]
                }
                _ => {
                    let after = match ins {
                        Instr::Move(src, dst) if is_loc(dst, desc) => match cx.constant_src(src) {
                            Some(d) => cx.store(&Ex::Lit(d), x, false, false),
                            None => match src {
                                Src::Num(e) => cx.store(e, x, false, false),
                                _ => all,
                            },
                        },
                        Instr::Store {
                            value,
                            dst,
                            rounded,
                            check,
                        } if is_loc(dst, desc) => cx.store(value, x, *rounded, *check),
                        Instr::SetIndex(dst, e) if is_loc(dst, desc) => {
                            cx.store(e, x, false, false)
                        }
                        other if touches(code, other, desc) => all,
                        _ => x,
                    };
                    succ[pc as usize]
                        .iter()
                        .map(|&t| (t, Some(after)))
                        .collect()
                }
            };
            for (t, v) in outs {
                let (Some(v), Some(slot)) = (v, state.get_mut(t as usize)) else {
                    continue;
                };
                let new = match *slot {
                    None => v,
                    Some(old) => {
                        let mut j = old.join(v);
                        if j == old {
                            continue;
                        }
                        joins[t as usize] += 1;
                        // at loop heads (backward jumps), else much later
                        // (termination)
                        let limit = if t <= pc { WIDEN_AFTER } else { WIDEN_LATE };
                        if joins[t as usize] > limit {
                            // widen the bound that grows
                            if j.lo < old.lo {
                                j.lo = all.lo.min(j.lo);
                            }
                            if j.hi > old.hi {
                                j.hi = all.hi.max(j.hi);
                            }
                        }
                        j
                    }
                };
                *slot = Some(new);
                work.push(t);
            }
        }
        for (pc, ins) in code.code.iter().enumerate() {
            let (c, is_if) = match ins {
                Instr::JumpIf(c, _) => (c, true),
                Instr::JumpUnless(c, _) => (c, false),
                _ => continue,
            };
            let mut cs = BTreeSet::new();
            cmp_items(c, &mut cs);
            match state[pc] {
                None => {
                    out.insert((pc as u32, true));
                    out.insert((pc as u32, false));
                }
                Some(x) if cs.contains(&desc) => {
                    let (t, f) = cx.holds(c, x);
                    let (jump, next) = if is_if { (t, f) } else { (f, t) };
                    if !jump {
                        out.insert((pc as u32, true));
                    }
                    if !next {
                        out.insert((pc as u32, false));
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// The items a condition compares numerically.
fn cmp_items(c: &Cx, out: &mut BTreeSet<u32>) {
    match c {
        Cx::Not(x) => cmp_items(x, out),
        Cx::And(a, b) | Cx::Or(a, b) => {
            cmp_items(a, out);
            cmp_items(b, out);
        }
        Cx::Cmp(a, _, b, CmpMode::Numeric) => {
            for s in [a, b] {
                if let Src::Loc(l) | Src::Num(Ex::Load(l)) = s
                    && l.subs.is_empty()
                    && l.refmod.is_none()
                {
                    out.insert(l.desc);
                }
            }
        }
        Cx::Sign(Ex::Load(l), _) if l.subs.is_empty() && l.refmod.is_none() => {
            out.insert(l.desc);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refine_by_constants() {
        let x = Iv { lo: 1, hi: 20 };
        let d = |c: i128, s: u32| Dec { coef: c, scale: s };
        assert_eq!(refine(x, Rel::Gt, d(11, 0), 0), Some(Iv { lo: 12, hi: 20 }));
        assert_eq!(refine(x, Rel::Le, d(11, 0), 0), Some(Iv { lo: 1, hi: 11 }));
        assert_eq!(refine(x, Rel::Eq, d(25, 0), 0), None);
        // 1.5 at scale 0: x < 1.5 is x <= 1
        assert_eq!(refine(x, Rel::Lt, d(15, 1), 0), Some(Iv { lo: 1, hi: 1 }));
        assert_eq!(refine(x, Rel::Eq, d(15, 1), 0), None);
    }
}

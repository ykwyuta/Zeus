//! Branch directions proved impossible without running: the values an
//! alphanumeric flag can hold, followed along the program's control flow.
//!
//! A flag here is an elementary alphanumeric item of the working-storage
//! (no table, not in the local storage) that the program only ever sets
//! from literals (`MOVE 'Y' TO WS-EOF`, `SET FLAG-ON TO TRUE`) and does
//! not otherwise touch (no CALL argument, READ INTO, group move,
//! INITIALIZE, EXEC area, FILE STATUS or other item of a file
//! definition the run time sets...). Its values are its VALUE and those
//! literals. An abstract interpretation over the instructions gives the
//! values possible at each one: a MOVE of a literal sets the value, and
//! each way of a conditional jump on the flag keeps the values for which
//! the condition goes that way. A direction whose condition no possible
//! value takes, or an instruction not reached at all, is impossible:
//!
//! ```cobol
//! PERFORM UNTIL END-OF-FILE = 'Y'
//!     IF END-OF-FILE = 'N'      *> never false: the values are N and Y
//! ```
//!
//! PERFORM returns are approximated: a paragraph's end may return to any
//! PERFORM of it (more paths than there are, never fewer).

use std::collections::{BTreeSet, HashMap};

use zeus_cobol::zir::{
    Base, CmpMode, Cx, DataType, ExecVal, Fig, Instr, Loc, ProgramCode, Rel, Src,
};

/// Instructions gone back over to find the jumps leading to a MOVE.
const GUARD_STEPS: usize = 400;

/// The flags' values at most (more: the flag is not analyzed).
const MAX_VALUES: usize = 64;

/// The byte range of an item, with its tables' occurrences.
pub(crate) fn extent(code: &ProgramCode, desc: u32) -> Option<(Base, usize, usize)> {
    let d = code.descs.get(desc as usize)?;
    let mut end = d.offset as usize + d.len as usize;
    for dim in &d.dims {
        end += (dim.max.saturating_sub(1) as usize) * dim.stride as usize;
    }
    Some((d.base, d.offset as usize, end))
}

/// The items (descriptors) an instruction refers to, found in its
/// serialized form (every `Loc`, not the items its subscripts and
/// reference modifications read).
pub(crate) fn locs_in<T: serde::Serialize>(x: &T, out: &mut Vec<u32>) {
    fn walk(v: &serde_json::Value, out: &mut Vec<u32>) {
        match v {
            serde_json::Value::Object(m) => {
                if let (Some(d), true, true) = (
                    m.get("desc").and_then(|d| d.as_u64()),
                    m.contains_key("subs"),
                    m.contains_key("refmod"),
                ) {
                    // its subscripts and reference modification are read
                    out.push(d as u32);
                    return;
                }
                for x in m.values() {
                    walk(x, out);
                }
            }
            serde_json::Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    if let Ok(v) = serde_json::to_value(x) {
        walk(&v, out);
    }
}

/// The bytes a literal gives an item of `len` bytes.
fn literal(s: &Src, len: usize) -> Option<Vec<u8>> {
    let fill = |b: &[u8]| -> Vec<u8> {
        (0..len)
            .map(|i| b.get(i).copied().unwrap_or(0x40))
            .collect()
    };
    Some(match s {
        Src::Alnum(b) => fill(b),
        Src::NumLit(v, digits) if v.scale == 0 && v.coef >= 0 => fill(digits),
        Src::All(b) if !b.is_empty() => (0..len).map(|i| b[i % b.len()]).collect(),
        Src::Fig(f) => vec![
            match f {
                Fig::Space => 0x40,
                Fig::Zero => 0xF0,
                Fig::High => 0xFF,
                Fig::Low => 0x00,
                Fig::Quote => 0x7F,
                Fig::Apostrophe => 0x7D,
            };
            len
        ],
        _ => return None,
    })
}

fn is_flag_loc(l: &Loc, flag: u32) -> bool {
    l.desc == flag && l.subs.is_empty() && l.refmod.is_none()
}

/// What an instruction stores in a flag: Some(Some(value)) a constant
/// (a MOVE of a literal to it, or to a group containing it, an INITIALIZE
/// of such a group), Some(None) nothing, None something not followed.
fn flag_write(code: &ProgramCode, ins: &Instr, flag: u32, len: usize) -> Option<Option<Vec<u8>>> {
    let (_, fo, fe) = extent(code, flag)?;
    let base = code.descs.get(flag as usize)?.base;
    let overlaps =
        |desc: u32| extent(code, desc).is_some_and(|(b, s, e)| b == base && s < fe && fo < e);
    // a whole group (no table) containing the flag: the flag's offset in it
    let within = |l: &Loc| -> Option<usize> {
        if !l.subs.is_empty() || l.refmod.is_some() {
            return None;
        }
        let d = code.descs.get(l.desc as usize)?;
        let (b, s, e) = extent(code, l.desc)?;
        (b == base && d.dims.is_empty() && s <= fo && fe <= e).then(|| fo - s)
    };
    match ins {
        Instr::Move(src, dst) if is_flag_loc(dst, flag) => literal(src, len).map(Some),
        Instr::Move(src, dst) if overlaps(dst.desc) => {
            let at = within(dst)?;
            let dl = code.descs.get(dst.desc as usize)?.len as usize;
            let v = literal(src, dl)?;
            Some(Some(v.get(at..at + len)?.to_vec()))
        }
        Instr::Initialize { dst, replacing } if overlaps(dst.desc) => {
            // INITIALIZE without REPLACING: alphanumeric items to spaces
            (replacing.is_empty() && within(dst).is_some()).then(|| Some(vec![0x40; len]))
        }
        Instr::Store { dst, .. } if overlaps(dst.desc) => None,
        Instr::Move(..)
        | Instr::Store { .. }
        | Instr::JumpIf(..)
        | Instr::JumpUnless(..)
        | Instr::Display { .. }
        | Instr::SetTemp(..)
        | Instr::GoDepending(..)
        | Instr::Jump(_)
        | Instr::Perform { .. }
        | Instr::ParaEnd(_) => Some(None),
        Instr::Exec(k) => {
            let mut ls = Vec::new();
            if let Some(e) = code.execs.get(*k as usize) {
                locs_in(e, &mut ls);
            }
            (!ls.iter().any(|&x| overlaps(x))).then_some(None)
        }
        other => {
            let mut ls = Vec::new();
            locs_in(other, &mut ls);
            (!ls.iter().any(|&x| overlaps(x))).then_some(None)
        }
    }
}

/// Whether a condition holds for a value of the flag (None: it depends on
/// something else).
fn eval(cx: &Cx, flag: u32, len: usize, v: &[u8]) -> Option<bool> {
    match cx {
        Cx::True => Some(true),
        Cx::Not(x) => eval(x, flag, len, v).map(|b| !b),
        Cx::And(a, b) => match (eval(a, flag, len, v), eval(b, flag, len, v)) {
            (Some(false), _) | (_, Some(false)) => Some(false),
            (Some(true), Some(true)) => Some(true),
            _ => None,
        },
        Cx::Or(a, b) => match (eval(a, flag, len, v), eval(b, flag, len, v)) {
            (Some(true), _) | (_, Some(true)) => Some(true),
            (Some(false), Some(false)) => Some(false),
            _ => None,
        },
        Cx::Cmp(a, r, b, CmpMode::Alnum) => {
            let (lit, rel) = match (a, b) {
                (Src::Loc(l), other) if is_flag_loc(l, flag) => (other, *r),
                (other, Src::Loc(l)) if is_flag_loc(l, flag) => (other, flip(*r)),
                _ => return None,
            };
            // both padded with spaces to the longer length
            let lit = match lit {
                Src::Alnum(b) | Src::NumLit(_, b) => b.clone(),
                other => literal(other, len)?,
            };
            let n = len.max(lit.len());
            let pad = |s: &[u8]| -> Vec<u8> {
                (0..n).map(|i| s.get(i).copied().unwrap_or(0x40)).collect()
            };
            let o = pad(v).cmp(&pad(&lit));
            Some(match rel {
                Rel::Eq => o.is_eq(),
                Rel::Ne => o.is_ne(),
                Rel::Lt => o.is_lt(),
                Rel::Le => o.is_le(),
                Rel::Gt => o.is_gt(),
                Rel::Ge => o.is_ge(),
            })
        }
        _ => None,
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

/// The flags a condition compares.
fn flags_in(cx: &Cx, out: &mut BTreeSet<u32>) {
    match cx {
        Cx::Not(x) => flags_in(x, out),
        Cx::And(a, b) | Cx::Or(a, b) => {
            flags_in(a, out);
            flags_in(b, out);
        }
        Cx::Cmp(a, _, b, CmpMode::Alnum) => {
            for s in [a, b] {
                if let Src::Loc(l) = s
                    && l.subs.is_empty()
                    && l.refmod.is_none()
                {
                    out.insert(l.desc);
                }
            }
        }
        _ => {}
    }
}

/// The successors of each instruction (None: control flow not modelled,
/// such as ALTER).
pub(crate) fn successors(code: &ProgramCode) -> Option<Vec<Vec<u32>>> {
    let n = code.code.len() as u32;
    // paragraph -> the instructions a PERFORM of it returns to
    let mut returns: HashMap<u32, Vec<u32>> = HashMap::new();
    for (pc, i) in code.code.iter().enumerate() {
        match i {
            Instr::Perform { end, ret, .. } => returns.entry(*end).or_default().push(*ret),
            Instr::UseCheck { .. } => {
                for u in &code.uses {
                    returns.entry(u.end).or_default().push(pc as u32 + 1);
                }
            }
            _ => {}
        }
    }
    // HANDLE CONDITION / WHENEVER labels: any EXEC may go there
    let mut labels = Vec::new();
    for e in &code.execs {
        for o in &e.options {
            if let Some(ExecVal::Label(l)) = &o.arg {
                labels.push(*l);
            }
        }
    }
    let use_starts: Vec<u32> = code
        .uses
        .iter()
        .filter_map(|u| code.paragraphs.get(u.start as usize).map(|p| p.start))
        .collect();
    let mut out = Vec::with_capacity(n as usize);
    for (pc, i) in code.code.iter().enumerate() {
        let pc = pc as u32;
        let next = pc + 1;
        let s: Vec<u32> = match i {
            Instr::Jump(t) => vec![*t],
            Instr::JumpIf(_, t) | Instr::JumpUnless(_, t) => vec![*t, next],
            Instr::Perform { start, .. } => vec![*start],
            Instr::ParaEnd(p) => {
                let mut v = vec![next];
                v.extend(returns.get(p).into_iter().flatten().copied());
                v
            }
            Instr::UseCheck { .. } => {
                let mut v = vec![next];
                v.extend(use_starts.iter().copied());
                v
            }
            Instr::GoDepending(_, ts) => {
                let mut v = ts.clone();
                v.push(next);
                v
            }
            Instr::SearchStep { at_end, .. } => vec![*at_end, next],
            Instr::Exec(_) => {
                let mut v = vec![next];
                v.extend(labels.iter().copied());
                v
            }
            Instr::StopRun | Instr::GoBack => vec![],
            Instr::AlterableGo(_) | Instr::Alter(..) | Instr::Sort { .. } => return None,
            _ => vec![next],
        };
        out.push(s.into_iter().filter(|&t| t < n).collect());
    }
    Some(out)
}

/// The values a flag can hold: its VALUE and the literals moved to it.
/// None: something else writes it.
fn values(code: &ProgramCode, flag: u32) -> Option<Vec<Vec<u8>>> {
    let d = code.descs.get(flag as usize)?;
    if d.base != Base::Static
        || !d.dims.is_empty()
        || !matches!(
            d.ty,
            DataType::Alphanumeric { just: false } | DataType::Alphabetic { just: false }
        )
        || code
            .local_storage
            .is_some_and(|(o, l)| d.offset >= o && d.offset < o + l)
    {
        return None;
    }
    let (o, l) = (d.offset as usize, d.len as usize);
    let (_, fo, fe) = extent(code, flag)?;
    let overlaps = |desc: u32| {
        extent(code, desc).is_some_and(|(b, s, e)| b == Base::Static && s < fe && fo < e)
    };
    // items the run time sets without an instruction: FILE STATUS, keys,
    // record lengths... (anything a file definition refers to)
    let mut implicit = Vec::new();
    locs_in(&code.files, &mut implicit);
    if implicit.iter().any(|&x| overlaps(x)) {
        return None;
    }
    // the record areas of the files (READ, RETURN and the like set them)
    for f in &code.files {
        if let Some(r) = code.descs.get(f.record as usize) {
            let (s, e) = (r.offset as usize, r.offset as usize + f.max_len as usize);
            if r.base == d.base && s < fe && fo < e {
                return None;
            }
        }
    }
    let mut vals: Vec<Vec<u8>> = vec![code.static_init.get(o..o + l)?.to_vec()];
    for i in &code.code {
        if let Some(v) = flag_write(code, i, flag, l)? {
            if !vals.contains(&v) {
                vals.push(v);
            }
            if vals.len() > MAX_VALUES {
                return None;
            }
        }
    }
    Some(vals)
}

/// The directions (instruction, jumped) of the conditional jumps of a
/// program that are impossible: not reached at all, or a condition on a
/// flag no possible value of it takes that way.
pub fn impossible(code: &ProgramCode) -> BTreeSet<(u32, bool)> {
    let mut out = BTreeSet::new();
    let Some(succ) = successors(code) else {
        return out;
    };
    let mut flags = BTreeSet::new();
    for i in &code.code {
        if let Instr::JumpIf(c, _) | Instr::JumpUnless(c, _) = i {
            flags_in(c, &mut flags);
        }
    }
    let entries: Vec<u32> = std::iter::once(0)
        .chain(code.entries.iter().map(|e| e.pc))
        .collect();
    for flag in flags {
        let Some(vals) = values(code, flag) else {
            continue;
        };
        let len = code.descs[flag as usize].len as usize;
        let all: u64 = if vals.len() == 64 {
            u64::MAX
        } else {
            (1u64 << vals.len()) - 1
        };
        let mut state = vec![0u64; code.code.len()];
        let mut work: Vec<u32> = Vec::new();
        for &e in &entries {
            if (e as usize) < state.len() {
                state[e as usize] |= 1; // the VALUE
                work.push(e);
            }
        }
        // the values for which a condition goes each way
        let split = |cx: &Cx, s: u64| -> (u64, u64) {
            let (mut t, mut f) = (0u64, 0u64);
            for (k, v) in vals.iter().enumerate() {
                if s & (1 << k) == 0 {
                    continue;
                }
                match eval(cx, flag, len, v) {
                    Some(true) => t |= 1 << k,
                    Some(false) => f |= 1 << k,
                    None => {
                        t |= 1 << k;
                        f |= 1 << k;
                    }
                }
            }
            (t, f)
        };
        while let Some(pc) = work.pop() {
            let s = state[pc as usize];
            let ins = &code.code[pc as usize];
            // the state after the instruction, by successor
            let outs: Vec<(u32, u64)> = match ins {
                Instr::JumpIf(c, t) | Instr::JumpUnless(c, t) => {
                    let (yes, no) = split(c, s);
                    let (taken, fall) = if matches!(ins, Instr::JumpIf(..)) {
                        (yes, no)
                    } else {
                        (no, yes)
                    };
                    vec![(*t, taken), (pc + 1, fall)]
                }
                _ => {
                    let after = match flag_write(code, ins, flag, len) {
                        Some(Some(v)) => vals.iter().position(|x| *x == v).map_or(all, |k| 1 << k),
                        Some(None) => s,
                        None => all,
                    };
                    succ[pc as usize].iter().map(|&t| (t, after)).collect()
                }
            };
            for (t, v) in outs {
                let Some(slot) = state.get_mut(t as usize) else {
                    continue;
                };
                if v & !*slot != 0 {
                    *slot |= v;
                    work.push(t);
                }
            }
        }
        for (pc, ins) in code.code.iter().enumerate() {
            let (c, is_if) = match ins {
                Instr::JumpIf(c, _) => (c, true),
                Instr::JumpUnless(c, _) => (c, false),
                _ => continue,
            };
            let s = state[pc];
            let (yes, no) = split(c, s);
            // jumped: the condition holds for JumpIf, fails for JumpUnless
            let (jump, next) = if is_if { (yes, no) } else { (no, yes) };
            let mut fs = BTreeSet::new();
            flags_in(c, &mut fs);
            if s == 0 {
                // not reached with any value: no way goes
                out.insert((pc as u32, true));
                out.insert((pc as u32, false));
            } else if fs.contains(&flag) {
                if jump == 0 {
                    out.insert((pc as u32, true));
                }
                if next == 0 {
                    out.insert((pc as u32, false));
                }
            }
        }
    }
    out
}

/// For the directions of conditional jumps on flags, the directions that
/// lead to a MOVE of a value they need: going back from the MOVE over the
/// control flow (into the PERFORMs of its paragraph), the first
/// conditional jump of each path, taken the way that reaches it. Taking an enabler the other way in a
/// run that reaches the direction may give the flag the value it needs
/// (a flag set to a constant does not follow the inputs).
pub fn enablers(code: &ProgramCode) -> HashMap<(u32, bool), BTreeSet<(u32, bool)>> {
    let mut out: HashMap<(u32, bool), BTreeSet<(u32, bool)>> = HashMap::new();
    let Some(succ) = successors(code) else {
        return out;
    };
    let mut preds: Vec<Vec<u32>> = vec![Vec::new(); code.code.len()];
    for (pc, ss) in succ.iter().enumerate() {
        for &t in ss {
            preds[t as usize].push(pc as u32);
        }
    }
    for &e in std::iter::once(&0).chain(code.entries.iter().map(|e| &e.pc)) {
        if let Some(p) = preds.get_mut(e as usize) {
            p.push(u32::MAX); // entered from outside
        }
    }
    // the conditional jump directions reaching an instruction: going back
    // over the control flow (through PERFORMs too), the first conditional
    // jump on each path; a jump found both ways does not decide it
    let guards = |w: u32| -> BTreeSet<(u32, bool)> {
        let mut found = BTreeSet::new();
        let mut seen = vec![false; preds.len()];
        let mut todo = vec![w];
        let mut steps = 0;
        while let Some(p) = todo.pop() {
            steps += 1;
            if steps > GUARD_STEPS {
                break;
            }
            for &q in &preds[p as usize] {
                if q == u32::MAX {
                    continue;
                }
                match &code.code[q as usize] {
                    Instr::JumpIf(_, t) | Instr::JumpUnless(_, t) if *t != q + 1 => {
                        found.insert((q, *t == p));
                    }
                    _ => {
                        if !std::mem::replace(&mut seen[q as usize], true) {
                            todo.push(q);
                        }
                    }
                }
            }
        }
        let both: Vec<u32> = found
            .iter()
            .filter(|(q, j)| found.contains(&(*q, !*j)))
            .map(|(q, _)| *q)
            .collect();
        found.retain(|(q, _)| !both.contains(q));
        found
    };
    let mut flags = BTreeSet::new();
    for i in &code.code {
        if let Instr::JumpIf(c, _) | Instr::JumpUnless(c, _) = i {
            flags_in(c, &mut flags);
        }
    }
    for flag in flags {
        let Some(vals) = values(code, flag) else {
            continue;
        };
        let len = code.descs[flag as usize].len as usize;
        // the guards of the MOVEs of each value
        let mut by_value: Vec<BTreeSet<(u32, bool)>> = vec![BTreeSet::new(); vals.len()];
        for (pc, i) in code.code.iter().enumerate() {
            if let Some(Some(v)) = flag_write(code, i, flag, len)
                && let Some(k) = vals.iter().position(|x| *x == v)
            {
                by_value[k].extend(guards(pc as u32));
            }
        }
        for (pc, ins) in code.code.iter().enumerate() {
            let (c, is_if) = match ins {
                Instr::JumpIf(c, _) => (c, true),
                Instr::JumpUnless(c, _) => (c, false),
                _ => continue,
            };
            let mut fs = BTreeSet::new();
            flags_in(c, &mut fs);
            if !fs.contains(&flag) {
                continue;
            }
            for (k, v) in vals.iter().enumerate() {
                // the direction this value takes
                let Some(holds) = eval(c, flag, len, v) else {
                    continue;
                };
                let jumped = holds == is_if;
                let e = out.entry((pc as u32, jumped)).or_default();
                e.extend(by_value[k].iter().filter(|g| g.0 != pc as u32));
            }
        }
    }
    out.retain(|_, v| !v.is_empty());
    out
}

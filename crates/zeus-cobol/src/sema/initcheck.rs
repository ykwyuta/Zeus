//! INITCHECK: compile-time warnings for data items used before they are
//! initialized (Programming Guide, INITCHECK compiler option).
//!
//! A forward data-flow analysis over the instructions of a program. The
//! tracked items are the elementary WORKING-STORAGE and LOCAL-STORAGE
//! items without a VALUE clause (on themselves or a containing group);
//! they are named by their storage extent, so a write through a group, a
//! redefinition or a table element initializes what it overlaps, and one
//! initialized element initializes the whole table, as the Programming
//! Guide says the analysis does.
//!
//! LAX warns when no path to the statement initializes the item; STRICT
//! warns unless every path does. PERFORM is followed context-insensitively:
//! the end of a paragraph flows to the return point of every PERFORM that
//! ends there. Code reached only through declaratives is not analyzed.
//! Not compared with the IBM compiler's messages: only the presence of a
//! warning (return code 4) is meant to match.

use crate::diag::Diag;
use crate::front::source::Pos;
use crate::zir::{Base, CallArgMode, Desc, Instr, Loc, ProgramCode};

/// Storage extent of a descriptor in the static image: its first byte and
/// the byte after the last occurrence.
fn extent(d: &Desc) -> (u32, u32) {
    let mut end = d.offset + d.len;
    for dim in &d.dims {
        end += dim.stride * dim.max.saturating_sub(1);
    }
    (d.offset, end)
}

#[derive(Clone, PartialEq)]
struct State {
    /// Initialized on at least one path (LAX).
    may: Vec<bool>,
    /// Initialized on every path (STRICT).
    must: Vec<bool>,
}

impl State {
    fn join(&mut self, other: &State) -> bool {
        let mut changed = false;
        for (a, b) in self.may.iter_mut().zip(&other.may) {
            if *b && !*a {
                *a = true;
                changed = true;
            }
        }
        for (a, b) in self.must.iter_mut().zip(&other.must) {
            if !*b && *a {
                *a = false;
                changed = true;
            }
        }
        changed
    }
}

/// The locations an instruction reads and writes. A reference-modified
/// item counts as written (the Programming Guide: such an item is
/// considered initialized); so does an argument passed BY REFERENCE.
fn effects(ins: &Instr) -> (Vec<Loc>, Vec<Loc>) {
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    if let Instr::Call {
        args, returning, ..
    } = ins
    {
        for a in args {
            match a {
                CallArgMode::Reference(l) => writes.push(l.clone()),
                CallArgMode::Content(s, _) | CallArgMode::ValueData(s) => {
                    if let crate::zir::Src::Loc(l) = s {
                        reads.push(l.clone());
                    }
                }
                _ => {}
            }
        }
        writes.extend(returning.iter().cloned());
        return (reads, writes);
    }
    for (loc, receiving) in super::codegen::instr_locs(ins) {
        if receiving || loc.refmod.is_some() {
            writes.push(loc);
        } else {
            reads.push(loc);
        }
    }
    (reads, writes)
}

pub fn analyze(p: &ProgramCode, tracked: &[u32], strict: bool) -> Vec<Diag> {
    if tracked.is_empty() || p.code.is_empty() {
        return Vec::new();
    }
    let spans: Vec<(u32, u32)> = tracked.iter().map(|&d| extent(&p.descs[d as usize])).collect();
    let hits = |loc: &Loc| -> Vec<usize> {
        let d = &p.descs[loc.desc as usize];
        if !matches!(d.base, Base::Static) {
            return Vec::new();
        }
        let (s, e) = extent(d);
        spans
            .iter()
            .enumerate()
            .filter(|(_, (a, b))| *a < e && s < *b)
            .map(|(k, _)| k)
            .collect()
    };
    let n = p.code.len();
    // return points of the PERFORMs ending at each paragraph
    let mut returns: std::collections::HashMap<u32, Vec<usize>> = Default::default();
    let mut altered = Vec::new();
    for ins in &p.code {
        match ins {
            Instr::Perform { end, ret, .. } => returns.entry(*end).or_default().push(*ret as usize),
            Instr::Alter(_, t) => altered.push(*t as usize),
            _ => {}
        }
    }
    let succ = |pc: usize| -> Vec<usize> {
        let next = vec![pc + 1];
        let mut out = match &p.code[pc] {
            Instr::Jump(t) => vec![*t as usize],
            Instr::JumpIf(_, t) | Instr::JumpUnless(_, t) => vec![*t as usize, pc + 1],
            Instr::Perform { start, .. } => vec![*start as usize],
            Instr::ParaEnd(para) => {
                let mut v = next;
                v.extend(returns.get(para).into_iter().flatten().copied());
                v
            }
            Instr::GoDepending(_, ts) => {
                let mut v: Vec<usize> = ts.iter().map(|&t| t as usize).collect();
                v.push(pc + 1);
                v
            }
            Instr::AlterableGo(t) => {
                let mut v = vec![*t as usize];
                v.extend(altered.iter().copied());
                v
            }
            Instr::SearchStep { at_end, .. } => vec![*at_end as usize, pc + 1],
            Instr::StopRun | Instr::GoBack => vec![],
            _ => next,
        };
        out.retain(|&t| t < n);
        out
    };
    let m = tracked.len();
    let mut states: Vec<Option<State>> = vec![None; n];
    states[0] = Some(State {
        may: vec![false; m],
        must: vec![false; m],
    });
    let mut work = vec![0usize];
    let mut queued = vec![false; n];
    queued[0] = true;
    while let Some(pc) = work.pop() {
        queued[pc] = false;
        let mut st = states[pc].clone().unwrap();
        let (_, writes) = effects(&p.code[pc]);
        for w in &writes {
            for k in hits(w) {
                st.may[k] = true;
                st.must[k] = true;
            }
        }
        for t in succ(pc) {
            let changed = match &mut states[t] {
                Some(s) => s.join(&st),
                slot @ None => {
                    *slot = Some(st.clone());
                    true
                }
            };
            if changed && !queued[t] {
                queued[t] = true;
                work.push(t);
            }
        }
    }
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for pc in 0..n {
        let Some(st) = &states[pc] else { continue };
        let (reads, _) = effects(&p.code[pc]);
        let stmt = p.instr_stmt.get(pc).copied().unwrap_or(u32::MAX);
        for r in &reads {
            // the first uninitialized item of the referenced item
            let Some(k) = hits(r)
                .into_iter()
                .find(|&k| if strict { !st.must[k] } else { !st.may[k] })
            else {
                continue;
            };
            if !seen.insert((stmt, k)) {
                continue;
            }
            let (file, line) = p.stmt_pos.get(stmt as usize).copied().unwrap_or_default();
            out.push(Diag::warning(
                Pos { file, line },
                format!(
                    "{} is used before it is initialized (INITCHECK{})",
                    p.descs[tracked[k] as usize].name,
                    if strict { "(STRICT)" } else { "" }
                ),
            ));
        }
    }
    out
}

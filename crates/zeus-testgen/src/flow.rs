//! The control flow the static analyses (`infeasible`, `ranges`) follow,
//! with the PERFORMs active: a state per instruction and call string (the
//! innermost PERFORMs, at most `DEPTH`). A paragraph's end returns to the
//! PERFORM whose last paragraph it is, else goes on to the next one, as
//! the interpreter does. When the call string was cut (deeper PERFORMs),
//! a paragraph's end with no known PERFORM may do either (any PERFORM of
//! it, or the next paragraph): more paths than there are, never fewer.

use std::collections::HashMap;

use zeus_cobol::zir::{Instr, ProgramCode};

use crate::infeasible::successors;

/// PERFORMs kept in a call string.
const DEPTH: usize = 3;

/// States at most (more: the analysis gives up).
const MAX_STATES: usize = 200_000;

/// The active PERFORMs (last paragraph, return instruction), innermost
/// last, and whether outer ones were cut off.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
struct Calls {
    stack: Vec<(u32, u32)>,
    cut: bool,
}

impl Calls {
    fn push(&self, end: u32, ret: u32) -> Calls {
        let mut c = self.clone();
        c.stack.push((end, ret));
        if c.stack.len() > DEPTH {
            c.stack.remove(0);
            c.cut = true;
        }
        c
    }
}

/// What an instruction gives a state.
pub(crate) enum Out<S> {
    /// To this instruction (the ways of a conditional jump).
    To(u32, S),
    /// To the instructions that follow it in the control flow.
    Next(S),
}

/// The states at each instruction (joined over the call strings; None: not
/// reached), from `init` at the entries; None when the program's control
/// flow is not modelled or too large.
///
/// `transfer` gives the states after an instruction; `join` joins an old
/// state and a new one (`widen` at a loop head after a few joins).
pub(crate) fn fixpoint<S: Clone + PartialEq>(
    code: &ProgramCode,
    init: S,
    transfer: &dyn Fn(u32, &S) -> Vec<Out<S>>,
    join: &dyn Fn(&S, &S, bool) -> S,
    widen_after: u32,
) -> Option<Vec<Option<S>>> {
    let succ = successors(code)?;
    let n = code.code.len();
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
    let use_starts: Vec<(u32, u32)> = code
        .uses
        .iter()
        .filter_map(|u| {
            code.paragraphs
                .get(u.start as usize)
                .map(|p| (p.start, u.end))
        })
        .collect();
    let mut states: HashMap<(u32, Calls), (S, u32)> = HashMap::new();
    let mut work: Vec<(u32, Calls)> = Vec::new();
    for e in std::iter::once(0).chain(code.entries.iter().map(|e| e.pc)) {
        if (e as usize) < n {
            states.insert((e, Calls::default()), (init.clone(), 0));
            work.push((e, Calls::default()));
        }
    }
    let next_of = |pc: u32, calls: &Calls| -> Vec<(u32, Calls)> {
        match &code.code[pc as usize] {
            Instr::Perform { start, end, ret } => vec![(*start, calls.push(*end, *ret))],
            Instr::ParaEnd(p) => match calls.stack.last() {
                Some(&(e, r)) if e == *p => {
                    let mut c = calls.clone();
                    c.stack.pop();
                    vec![(r, c)]
                }
                Some(_) => vec![(pc + 1, calls.clone())],
                None if calls.cut => {
                    let mut v = vec![(pc + 1, calls.clone())];
                    v.extend(
                        returns
                            .get(p)
                            .into_iter()
                            .flatten()
                            .map(|&r| (r, calls.clone())),
                    );
                    v
                }
                None => vec![(pc + 1, calls.clone())],
            },
            Instr::UseCheck { .. } => {
                let mut v = vec![(pc + 1, calls.clone())];
                v.extend(use_starts.iter().map(|&(s, e)| (s, calls.push(e, pc + 1))));
                v
            }
            _ => succ[pc as usize]
                .iter()
                .map(|&t| (t, calls.clone()))
                .collect(),
        }
    };
    while let Some((pc, calls)) = work.pop() {
        let Some((s, _)) = states.get(&(pc, calls.clone())).cloned() else {
            continue;
        };
        let mut outs: Vec<(u32, Calls, S)> = Vec::new();
        for o in transfer(pc, &s) {
            match o {
                Out::To(t, v) => outs.push((t, calls.clone(), v)),
                Out::Next(v) => {
                    for (t, c) in next_of(pc, &calls) {
                        outs.push((t, c, v.clone()));
                    }
                }
            }
        }
        for (t, c, v) in outs {
            if t as usize >= n {
                continue;
            }
            let key = (t, c);
            match states.get_mut(&key) {
                None => {
                    states.insert(key.clone(), (v, 0));
                    work.push(key);
                }
                Some((old, joins)) => {
                    *joins += 1;
                    let widen = t <= pc && *joins > widen_after;
                    let j = join(old, &v, widen);
                    if j != *old {
                        *old = j;
                        work.push(key);
                    }
                }
            }
            if states.len() > MAX_STATES {
                return None;
            }
        }
    }
    let mut out: Vec<Option<S>> = vec![None; n];
    for ((pc, _), (s, _)) in states {
        let slot = &mut out[pc as usize];
        *slot = Some(match slot.take() {
            None => s,
            Some(o) => join(&o, &s, false),
        });
    }
    Some(out)
}

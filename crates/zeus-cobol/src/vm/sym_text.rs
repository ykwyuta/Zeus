//! INSPECT in concolic execution: the bytes REPLACING and CONVERTING
//! leave and the count TALLYING adds, as terms over the bytes inspected.
//! Followed: one phrase without BEFORE / AFTER, on one-character
//! operands (CONVERTING: strings of the same length). Other forms leave
//! the data to the byte check (a shadow whose bytes changed is dropped).

use super::Machine;
use super::sym::plain_display;
use crate::concolic::{Pred, Term};
use crate::zir::{DataType, InspectKind, InspectStep, Loc, NumType, Rel, Src};

/// What an INSPECT looks for: one character (or every character).
#[derive(Debug, Clone, Copy)]
enum Look {
    Characters,
    All(u8),
    Leading(u8),
    First(u8),
}

#[derive(Debug, Clone)]
enum Op {
    Replace(Look, u8),
    Convert(Vec<(u8, u8)>),
    /// The counter (address, length, type) and its value before.
    Tally((usize, usize, NumType), Term, Look),
}

/// An INSPECT on followed bytes: the target and its bytes before.
#[derive(Debug, Clone)]
pub(super) struct InspectPre {
    a: usize,
    l: usize,
    bytes: Vec<Term>,
    op: Op,
}

fn eq(t: &Term, c: u8) -> Pred {
    Pred::Cmp(t.clone(), Rel::Eq, Term::Const(i128::from(c)))
}

/// For each position, whether the phrase applies there.
fn applies(bytes: &[Term], look: Look) -> Vec<Pred> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut run: Vec<Pred> = Vec::new();
    for b in bytes {
        out.push(match look {
            Look::Characters => Pred::Bool(true),
            Look::All(c) => eq(b, c),
            Look::Leading(c) => {
                run.push(eq(b, c));
                Pred::And(run.clone())
            }
            Look::First(c) => {
                let p = Pred::And(run.iter().cloned().chain([eq(b, c)]).collect());
                run.push(Pred::not(eq(b, c)));
                p
            }
        });
    }
    out
}

impl Machine {
    /// One character of an operand (a figurative constant repeats).
    fn sym_char(&mut self, cur: usize, s: &Src) -> Option<u8> {
        let v = self.eval_src(cur, s).ok()?;
        let n = self.alnum_len(&v);
        if n > 1 {
            return None;
        }
        self.alnum_bytes(cur, &v, 1).ok()?.first().copied()
    }

    fn sym_look(&mut self, cur: usize, k: &InspectKind) -> Option<Look> {
        Some(match k {
            InspectKind::Characters => Look::Characters,
            InspectKind::All(s) => Look::All(self.sym_char(cur, s)?),
            InspectKind::Leading(s) => Look::Leading(self.sym_char(cur, s)?),
            InspectKind::First(s) => Look::First(self.sym_char(cur, s)?),
        })
    }

    pub(super) fn sym_inspect_pre(
        &mut self,
        cur: usize,
        target: &Loc,
        steps: &[InspectStep],
    ) -> Option<InspectPre> {
        let [step] = steps else { return None };
        let (a, l) = self.addr(cur, target).ok()?;
        match self.loc_type(cur, target) {
            DataType::Alphanumeric { .. } | DataType::Alphabetic { .. } | DataType::Group => {}
            DataType::Numeric(t) if plain_display(&t) => {}
            _ => return None,
        }
        let op = match step {
            InspectStep::Replace {
                kind,
                by,
                before: None,
                after: None,
            } => Op::Replace(self.sym_look(cur, kind)?, self.sym_char(cur, by)?),
            InspectStep::Convert {
                from,
                to,
                before: None,
                after: None,
            } => {
                let f = self.eval_src(cur, from).ok()?;
                let n = self.alnum_len(&f).max(1);
                let f = self.alnum_bytes(cur, &f, n).ok()?;
                let t = self.eval_src(cur, to).ok()?;
                let t = self.alnum_bytes(cur, &t, n).ok()?;
                if f.len() != t.len() {
                    return None;
                }
                Op::Convert(f.into_iter().zip(t).collect())
            }
            InspectStep::Tally {
                counter,
                kind,
                before: None,
                after: None,
            } => {
                let (ca, cl) = self.addr(cur, counter).ok()?;
                let DataType::Numeric(t) = self.loc_type(cur, counter) else {
                    return None;
                };
                if t.scale != 0 {
                    return None;
                }
                let look = self.sym_look(cur, kind)?;
                let old = match self.sym_read_num(ca, cl, &t) {
                    Some(o) => o,
                    None => {
                        let v = super::data::num_value(&t, &self.mem[ca..ca + cl]);
                        Term::Const(v.rescale(0, zeus_decimal::Rounding::Truncate)?.coef)
                    }
                };
                Op::Tally((ca, cl, t), old, look)
            }
            _ => return None,
        };
        let bytes = self.sym_read_bytes(a, l)?;
        Some(InspectPre { a, l, bytes, op })
    }

    pub(super) fn sym_inspect_post(&mut self, pre: InspectPre) {
        let InspectPre { a, l, bytes, op } = pre;
        match op {
            Op::Replace(look, by) => {
                self.sym_forget(a, l);
                for (i, (b, p)) in bytes.iter().zip(applies(&bytes, look)).enumerate() {
                    let t = Term::ite(p, Term::Const(i128::from(by)), b.clone());
                    if t.konst().is_none() {
                        self.sym_bind_byte_term(a + i, t);
                    }
                }
            }
            Op::Convert(pairs) => {
                self.sym_forget(a, l);
                for (i, b) in bytes.iter().enumerate() {
                    // the first occurrence of a character in FROM counts
                    let mut t = b.clone();
                    for (f, to) in pairs.iter().rev() {
                        t = Term::ite(eq(b, *f), Term::Const(i128::from(*to)), t);
                    }
                    if t.konst().is_none() {
                        self.sym_bind_byte_term(a + i, t);
                    }
                }
            }
            Op::Tally((ca, cl, t), old, look) => {
                let mut sum = old;
                for p in applies(&bytes, look) {
                    sum = Term::add(sum, Term::ite(p, Term::Const(1), Term::Const(0)));
                }
                self.sym_forget(ca, cl);
                if let Some((_, v)) = Self::sym_store_term(&t, sum, 0) {
                    self.sym_bind_term(ca, cl, t, v);
                }
            }
        }
    }
}

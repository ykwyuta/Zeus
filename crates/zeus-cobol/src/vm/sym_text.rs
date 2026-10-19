//! Text statements in concolic execution.
//!
//! INSPECT: the bytes REPLACING and CONVERTING leave and the count
//! TALLYING adds, as terms over the bytes inspected. Followed: one phrase
//! without BEFORE / AFTER, on one-character operands (CONVERTING: strings
//! of the same length).
//!
//! UNSTRING and STRING with one-character delimiters (or SIZE): the
//! fields are the bytes between the delimiters of the run, and whether a
//! position is a delimiter is a decision (not counted in C1) the solver
//! can take the other way, which moves or adds delimiters. NUMVAL of
//! followed bytes records whether its argument is a number.
//!
//! Other forms leave the data to the byte check (a shadow whose bytes
//! changed is dropped).

use super::Machine;
use super::sym::plain_display;
use crate::concolic::{Branch, Pred, Step, Term};
use crate::zir::{DataType, Ex, InspectKind, InspectStep, Loc, NumType, Rel, Src};

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

/// Positions from the start of a field (UNSTRING) or a source (STRING
/// with a delimiter) whose "is a delimiter" is a decision.
const SCAN: usize = 32;

/// The decisions of a text statement are not the module's branches:
/// program `P#TEXT`, instruction `pc * 4096 + position`.
fn text_id(pc: u32, pos: usize) -> u32 {
    pc.saturating_mul(4096).saturating_add(pos.min(4095) as u32)
}

fn is_delim(t: &Term, delims: &[u8]) -> Pred {
    match delims {
        [d] => eq(t, *d),
        _ => Pred::Or(delims.iter().map(|d| eq(t, *d)).collect()),
    }
}

/// An UNSTRING with one-character delimiters on followed bytes.
#[derive(Debug, Clone)]
pub(super) struct UnstringPre {
    prog: String,
    pc: u32,
    data: Vec<u8>,
    bytes: Vec<Term>,
    /// (ALL, delimiter)
    delims: Vec<(bool, u8)>,
    start: usize,
    /// The receivers (address, length, type), and the other items the
    /// statement sets.
    into: Vec<(usize, usize, DataType)>,
    other: Vec<(usize, usize)>,
}

/// A STRING on followed bytes: each source's bytes (terms, values) and
/// delimiter (None: SIZE).
#[derive(Debug, Clone)]
pub(super) struct StringPre {
    prog: String,
    pc: u32,
    sources: Vec<(Vec<Term>, Vec<u8>, Option<u8>)>,
    dst: (usize, usize),
    start: usize,
}

impl Machine {
    /// A decision of a text statement (see `text_id`).
    fn sym_decide(&mut self, prog: &str, id: u32, p: Pred) {
        let Some(st) = self.sym.as_mut() else { return };
        if st.long() || st.full() {
            return;
        }
        let mut vs = Vec::new();
        p.vars(&mut vs);
        if vs.is_empty() {
            return;
        }
        let hit = p.eval(&|v| st.value(v)).unwrap_or(false);
        st.path.push(Step::Branch(Branch {
            program: format!("{prog}#TEXT"),
            pc: id,
            jumped: hit,
            pred: Some(p),
        }));
    }

    /// An operand's bytes as terms (constants where not followed).
    fn sym_operand_bytes(&mut self, cur: usize, s: &Src) -> Option<(Vec<Term>, Vec<u8>)> {
        let v = self.eval_src(cur, s).ok()?;
        let n = self.alnum_len(&v).max(1);
        let concrete = self.alnum_bytes(cur, &v, n).ok()?;
        let followed = match s {
            Src::Loc(loc) => {
                let (a, l) = self.addr(cur, loc).ok()?;
                if l == concrete.len() {
                    self.sym_read_bytes(a, l)
                } else {
                    None
                }
            }
            _ => None,
        };
        let terms = followed.unwrap_or_else(|| {
            concrete
                .iter()
                .map(|b| Term::Const(i128::from(*b)))
                .collect()
        });
        Some((terms, concrete))
    }

    fn sym_pointer(&mut self, cur: usize, pointer: Option<&Loc>) -> Option<usize> {
        match pointer {
            Some(p) => {
                let v = self.eval_ex(cur, &Ex::Load(p.clone())).ok()?;
                usize::try_from(super::data::to_raw(v, 0, false) - 1).ok()
            }
            None => Some(0),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn sym_unstring_pre(
        &mut self,
        cur: usize,
        pc: u32,
        source: &Loc,
        delims: &[(bool, Src)],
        into: &[(Loc, Option<Loc>, Option<Loc>)],
        pointer: Option<&Loc>,
        tallying: Option<&Loc>,
    ) -> Option<UnstringPre> {
        if delims.is_empty() {
            return None;
        }
        // a length that follows the inputs (data received with its
        // length): whether it is the whole item is a decision
        if let Some((_, Some(len))) = &source.refmod
            && let Some((lt, 0)) = self.sym_ex(cur, len)
            && lt.konst().is_none()
        {
            let whole = Loc {
                refmod: None,
                ..source.clone()
            };
            let (_, wl) = self.addr(cur, &whole).ok()?;
            let prog = self.instances[cur].code.name.clone();
            let whole_len = Pred::Cmp(lt, Rel::Eq, Term::Const(wl as i128));
            self.sym_decide(&prog, text_id(pc, 4094), whole_len);
        }
        let (a, l) = self.addr(cur, source).ok()?;
        let bytes = self.sym_read_bytes(a, l)?;
        let mut ds = Vec::new();
        for (all, d) in delims {
            ds.push((*all, self.sym_char(cur, d)?));
        }
        let start = self.sym_pointer(cur, pointer)?;
        let mut targets = Vec::new();
        let mut other = Vec::new();
        for (t, d, c) in into {
            let (ta, tl) = self.addr(cur, t).ok()?;
            let ty = if t.refmod.is_some() {
                DataType::Alphanumeric { just: false }
            } else {
                self.loc_type(cur, t)
            };
            targets.push((ta, tl, ty));
            for x in [d, c].into_iter().flatten() {
                other.push(self.addr(cur, x).ok()?);
            }
        }
        for x in [pointer, tallying].into_iter().flatten() {
            other.push(self.addr(cur, x).ok()?);
        }
        Some(UnstringPre {
            prog: self.instances[cur].code.name.clone(),
            pc,
            data: self.mem[a..a + l].to_vec(),
            bytes,
            delims: ds,
            start,
            into: targets,
            other,
        })
    }

    /// After an UNSTRING: the fields as the bytes between the delimiters
    /// of this run; each position near a field's start is a decision ("is
    /// a delimiter") the solver can take the other way.
    pub(super) fn sym_unstring_post(&mut self, pre: UnstringPre) {
        let UnstringPre {
            prog,
            pc,
            data,
            bytes,
            delims,
            start,
            into,
            other,
        } = pre;
        let chars: Vec<u8> = delims.iter().map(|d| d.1).collect();
        for (a, l) in other {
            self.sym_forget(a, l);
        }
        let mut p = start;
        for (ta, tl, ty) in into {
            if p >= data.len() {
                break;
            }
            let first = p;
            let mut end = data.len();
            let mut found = None;
            for i in p..data.len() {
                let hit = delims.iter().position(|d| d.1 == data[i]);
                if i - first < SCAN {
                    self.sym_decide(&prog, text_id(pc, i), is_delim(&bytes[i], &chars));
                }
                if let Some(k) = hit {
                    end = i;
                    found = Some(k);
                    break;
                }
            }
            self.sym_forget(ta, tl);
            if matches!(ty, DataType::Alphanumeric { just: false } | DataType::Group) {
                for j in 0..tl {
                    let t = bytes[first..end]
                        .get(j)
                        .cloned()
                        .unwrap_or(Term::Const(0x40));
                    if t.konst().is_none() {
                        self.sym_bind_byte_term(ta + j, t);
                    }
                }
            }
            p = end;
            if let Some(k) = found {
                let (repeat, d) = delims[k];
                p = end + 1;
                if repeat {
                    while p < data.len() && data[p] == d {
                        self.sym_decide(&prog, text_id(pc, p), eq(&bytes[p], d));
                        p += 1;
                    }
                    if p < data.len() && p - end <= SCAN {
                        self.sym_decide(&prog, text_id(pc, p), eq(&bytes[p], d));
                    }
                }
            }
        }
    }

    pub(super) fn sym_string_pre(
        &mut self,
        cur: usize,
        pc: u32,
        sources: &[(Src, Option<Src>)],
        into: &Loc,
        pointer: Option<&Loc>,
    ) -> Option<StringPre> {
        let mut out = Vec::new();
        let mut followed = false;
        for (s, d) in sources {
            let (terms, concrete) = self.sym_operand_bytes(cur, s)?;
            followed |= terms.iter().any(|t| t.konst().is_none());
            let d = match d {
                Some(d) => Some(self.sym_char(cur, d)?),
                None => None,
            };
            out.push((terms, concrete, d));
        }
        if !followed {
            return None;
        }
        let dst = self.addr(cur, into).ok()?;
        let start = self.sym_pointer(cur, pointer)?;
        Some(StringPre {
            prog: self.instances[cur].code.name.clone(),
            pc,
            sources: out,
            dst,
            start,
        })
    }

    /// After a STRING: the bytes it put in the receiver as the sources'
    /// terms; where a delimited source ends is a decision.
    pub(super) fn sym_string_post(&mut self, pre: StringPre) {
        let StringPre {
            prog,
            pc,
            sources,
            dst: (a, l),
            start,
        } = pre;
        let mut p = start;
        let mut pos = 0;
        for (terms, concrete, d) in sources {
            let end = match d {
                None => concrete.len(),
                Some(d) => {
                    let mut end = concrete.len();
                    for (i, b) in concrete.iter().enumerate() {
                        if i < SCAN {
                            self.sym_decide(&prog, text_id(pc, pos + i), eq(&terms[i], d));
                        }
                        if *b == d {
                            end = i;
                            break;
                        }
                    }
                    end
                }
            };
            pos += concrete.len();
            for t in terms.into_iter().take(end) {
                if p >= l {
                    return;
                }
                self.sym_forget(a + p, 1);
                if t.konst().is_none() {
                    self.sym_bind_byte_term(a + p, t);
                }
                p += 1;
            }
        }
    }

    /// NUMVAL / NUMVAL-C of followed bytes: whether the argument is a
    /// number (spaces, digits and a point, with a digit) is a decision (an
    /// invalid argument ends the run); with spaces, digits and a point at
    /// the positions of this run, the digits give the value.
    pub(super) fn sym_numval(&mut self, cur: usize, s: &Src) -> Option<(Term, i32)> {
        let (terms, text) = self.sym_operand_bytes(cur, s)?;
        if terms.iter().all(|t| t.konst().is_some()) {
            return None;
        }
        let (prog, pc) = self.sym.as_ref()?.at.clone()?;
        let digit = |t: &Term| Pred::In(t.clone(), vec![(0xF0, 0xF9)]);
        let valid = Pred::And(vec![
            Pred::And(
                terms
                    .iter()
                    .map(|t| Pred::In(t.clone(), vec![(0x40, 0x40), (0x4B, 0x4B), (0xF0, 0xF9)]))
                    .collect(),
            ),
            Pred::Or(terms.iter().map(digit).collect()),
        ]);
        self.sym_decide(&prog, text_id(pc, 4095), valid);
        // spaces, digits [. digits], spaces
        let first = text.iter().position(|b| *b != 0x40)?;
        let last = text.iter().rposition(|b| *b != 0x40)?;
        let body = &text[first..=last];
        let point = body.iter().position(|b| *b == 0x4B);
        let ok = body
            .iter()
            .enumerate()
            .all(|(i, b)| (0xF0..=0xF9).contains(b) || Some(i) == point);
        if !ok || body.len() > 30 {
            return None;
        }
        let mut term = Term::Const(0);
        let mut assume = Vec::new();
        for (i, t) in terms.iter().enumerate() {
            if i < first || i > last {
                assume.push(eq(t, 0x40));
            } else if Some(i - first) == point {
                assume.push(eq(t, 0x4B));
            } else {
                assume.push(digit(t));
                term = Term::add(
                    Term::mul(term, Term::Const(10)),
                    Term::sub(t.clone(), Term::Const(0xF0)),
                );
            }
        }
        let st = self.sym.as_mut()?;
        for p in assume {
            st.assume(p);
        }
        let scale = point.map_or(0, |k| body.len() - k - 1) as i32;
        Some((term, scale))
    }
}

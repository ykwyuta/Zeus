//! Text statements in concolic execution.
//!
//! INSPECT: the bytes REPLACING and CONVERTING leave and the count
//! TALLYING adds, as terms over the bytes inspected. Followed: one phrase
//! on one-character operands (CONVERTING: strings of the same length),
//! with BEFORE / AFTER INITIAL of one character (up to 64 bytes).
//!
//! UNSTRING and STRING with delimiters (or SIZE): the
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
use crate::zir::{DataType, Ex, InspectKind, InspectStep, Loc, NumType, NumUsage, Rel, Src};

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
    /// BEFORE INITIAL and AFTER INITIAL characters.
    region: (Option<u8>, Option<u8>),
}

fn eq(t: &Term, c: u8) -> Pred {
    Pred::Cmp(t.clone(), Rel::Eq, Term::Const(i128::from(c)))
}

/// For each position, whether it is in the region of BEFORE INITIAL
/// `before` (no `before` up to it) and AFTER INITIAL `after` (an `after`
/// before it).
fn region(bytes: &[Term], (before, after): (Option<u8>, Option<u8>)) -> Vec<Pred> {
    let mut out = Vec::with_capacity(bytes.len());
    let (mut none_yet, mut seen): (Vec<Pred>, Vec<Pred>) = (Vec::new(), Vec::new());
    for b in bytes {
        let mut r = Vec::new();
        if let Some(y) = before {
            none_yet.push(Pred::not(eq(b, y)));
            r.push(Pred::And(none_yet.clone()));
        }
        if let Some(y) = after {
            r.push(Pred::Or(seen.clone()));
            seen.push(eq(b, y));
        }
        out.push(match r.len() {
            0 => Pred::Bool(true),
            1 => r.remove(0),
            _ => Pred::And(r),
        });
    }
    out
}

/// For each position, whether the phrase applies there (in the region).
fn applies(bytes: &[Term], look: Look, within: (Option<u8>, Option<u8>)) -> Vec<Pred> {
    let r = region(bytes, within);
    let mut out = Vec::with_capacity(bytes.len());
    // the region's positions before: all matches (LEADING), none (FIRST)
    let mut run: Vec<Pred> = Vec::new();
    for (b, ri) in bytes.iter().zip(&r) {
        out.push(match look {
            Look::Characters => ri.clone(),
            Look::All(c) => Pred::And(vec![ri.clone(), eq(b, c)]),
            Look::Leading(c) => {
                let p = Pred::And(run.iter().cloned().chain([ri.clone(), eq(b, c)]).collect());
                run.push(Pred::Or(vec![Pred::not(ri.clone()), eq(b, c)]));
                p
            }
            Look::First(c) => {
                let p = Pred::And(run.iter().cloned().chain([ri.clone(), eq(b, c)]).collect());
                run.push(Pred::Or(vec![Pred::not(ri.clone()), Pred::not(eq(b, c))]));
                p
            }
        });
    }
    out
}

/// Characters of BEFORE / AFTER INITIAL up to which the region of INSPECT
/// is followed (the conditions grow with the square of the length).
const REGION_BYTES: usize = 64;

impl Machine {
    /// One character of an operand (a figurative constant repeats).
    /// A delimiter (a figurative constant: one character).
    fn sym_delim(&mut self, cur: usize, s: &Src) -> Option<Vec<u8>> {
        let v = self.eval_src(cur, s).ok()?;
        let n = self.alnum_len(&v).max(1);
        let d = self.alnum_bytes(cur, &v, n).ok()?;
        (!d.is_empty()).then_some(d)
    }

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
        let (before, after) = match step {
            InspectStep::Replace { before, after, .. }
            | InspectStep::Convert { before, after, .. }
            | InspectStep::Tally { before, after, .. } => (before, after),
        };
        let mut within = (None, None);
        if let Some(b) = before {
            within.0 = Some(self.sym_char(cur, b)?);
        }
        if let Some(x) = after {
            within.1 = Some(self.sym_char(cur, x)?);
        }
        let (a, l) = self.addr(cur, target).ok()?;
        match self.loc_type(cur, target) {
            DataType::Alphanumeric { .. } | DataType::Alphabetic { .. } | DataType::Group => {}
            DataType::Numeric(t) if plain_display(&t) => {}
            _ => return None,
        }
        let op = match step {
            InspectStep::Replace { kind, by, .. } => {
                Op::Replace(self.sym_look(cur, kind)?, self.sym_char(cur, by)?)
            }
            InspectStep::Convert { from, to, .. } => {
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
            InspectStep::Tally { counter, kind, .. } => {
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
        };
        if within != (None, None) && l > REGION_BYTES {
            return None;
        }
        let bytes = self.sym_read_bytes(a, l)?;
        Some(InspectPre {
            a,
            l,
            bytes,
            op,
            region: within,
        })
    }

    pub(super) fn sym_inspect_post(&mut self, pre: InspectPre) {
        let InspectPre {
            a,
            l,
            bytes,
            op,
            region: within,
        } = pre;
        match op {
            Op::Replace(look, by) => {
                self.sym_forget(a, l);
                for (i, (b, p)) in bytes.iter().zip(applies(&bytes, look, within)).enumerate() {
                    let t = Term::ite(p, Term::Const(i128::from(by)), b.clone());
                    if t.konst().is_none() {
                        self.sym_bind_byte_term(a + i, t);
                    }
                }
            }
            Op::Convert(pairs) => {
                self.sym_forget(a, l);
                let r = region(&bytes, within);
                for (i, (b, ri)) in bytes.iter().zip(r).enumerate() {
                    // the first occurrence of a character in FROM counts
                    let mut t = b.clone();
                    for (f, to) in pairs.iter().rev() {
                        t = Term::ite(eq(b, *f), Term::Const(i128::from(*to)), t);
                    }
                    let t = Term::ite(ri, t, b.clone());
                    if t.konst().is_none() {
                        self.sym_bind_byte_term(a + i, t);
                    }
                }
            }
            Op::Tally((ca, cl, t), old, look) => {
                let mut sum = old;
                for p in applies(&bytes, look, within) {
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

/// The decision "a reference modification is in its item" (`text_id`).
const REFMOD: usize = 4093;

/// The decisions of a text statement are not the module's branches:
/// program `P#TEXT`, instruction `pc * 4096 + position`.
fn text_id(pc: u32, pos: usize) -> u32 {
    pc.saturating_mul(4096).saturating_add(pos.min(4095) as u32)
}

/// Whether the data has the delimiter at `i`.
fn matches_at(data: &[u8], i: usize, d: &[u8]) -> bool {
    data.get(i..i + d.len()) == Some(d)
}

/// "The delimiter is at `i`" over the bytes' terms.
fn delim_at(bytes: &[Term], i: usize, d: &[u8]) -> Pred {
    match bytes.get(i..i + d.len()) {
        Some([t]) => eq(t, d[0]),
        Some(ts) => Pred::And(ts.iter().zip(d).map(|(t, c)| eq(t, *c)).collect()),
        None => Pred::Bool(false),
    }
}

/// "One of the delimiters is at `i`".
fn some_delim_at(bytes: &[Term], i: usize, delims: &[(bool, Vec<u8>)]) -> Pred {
    match delims {
        [(_, d)] => delim_at(bytes, i, d),
        _ => Pred::Or(delims.iter().map(|(_, d)| delim_at(bytes, i, d)).collect()),
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
    delims: Vec<(bool, Vec<u8>)>,
    start: usize,
    /// The receivers (address, length, type), and the other items the
    /// statement sets.
    into: Vec<(usize, usize, DataType)>,
    other: Vec<(usize, usize)>,
}

/// A source of a STRING: its bytes as terms and values, and its
/// delimiter (None: SIZE).
type StringSource = (Vec<Term>, Vec<u8>, Option<Vec<u8>>);

/// A STRING on followed bytes: each source's bytes (terms, values) and
/// delimiter (None: SIZE).
#[derive(Debug, Clone)]
pub(super) struct StringPre {
    prog: String,
    pc: u32,
    sources: Vec<StringSource>,
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
            counted: false,
        }));
    }

    /// A reference modification whose start or length follows the inputs
    /// (`item(:len)` with a length received with the data): whether it is
    /// in the item is a decision (`P#TEXT`, position 4093).
    pub(super) fn sym_refmod_decide(&mut self, cur: usize, s: &Src) {
        let Src::Loc(loc) = s else { return };
        let Some((start, len)) = &loc.refmod else {
            return;
        };
        // an integer that follows the inputs, else its value
        let int = |m: &mut Machine, e: &Ex| -> Option<Term> {
            match m.sym_ex(cur, e) {
                Some((t, 0)) => Some(t),
                Some(_) => None,
                None => {
                    let v = m.eval_ex(cur, e).ok()?;
                    Some(Term::Const(super::data::to_raw(v, 0, false)))
                }
            }
        };
        let Some(ts) = int(self, start) else {
            return;
        };
        let tl = match len {
            Some(l) => match int(self, l) {
                Some(t) => Some(t),
                None => return,
            },
            None => None,
        };
        if ts.konst().is_some() && tl.as_ref().is_none_or(|t| t.konst().is_some()) {
            return;
        }
        let whole = Loc {
            refmod: None,
            ..loc.clone()
        };
        let Ok((_, size)) = self.addr(cur, &whole) else {
            return;
        };
        let Some((prog, pc)) = self.sym.as_ref().and_then(|st| st.at.clone()) else {
            return;
        };
        let size = Term::Const(size as i128);
        let one = Term::Const(1);
        let mut ok = vec![Pred::Cmp(ts.clone(), Rel::Ge, one.clone())];
        match tl {
            Some(tl) => {
                ok.push(Pred::Cmp(tl.clone(), Rel::Ge, one.clone()));
                ok.push(Pred::Cmp(Term::sub(Term::add(ts, tl), one), Rel::Le, size));
            }
            None => ok.push(Pred::Cmp(ts, Rel::Le, size)),
        }
        self.sym_decide(&prog, text_id(pc, REFMOD), Pred::And(ok));
    }

    /// An operand's bytes as terms (constants where not followed).
    pub(super) fn sym_operand_bytes(
        &mut self,
        cur: usize,
        s: &Src,
    ) -> Option<(Vec<Term>, Vec<u8>)> {
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
            ds.push((*all, self.sym_delim(cur, d)?));
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
                let hit = delims.iter().position(|d| matches_at(&data, i, &d.1));
                if i - first < SCAN {
                    self.sym_decide(&prog, text_id(pc, i), some_delim_at(&bytes, i, &delims));
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
            } else if let DataType::Numeric(n) = &ty
                && n.usage == NumUsage::Display
                && !n.signed
                && n.scale == 0
                && tl == n.digits as usize
            {
                // an integer: the field's bytes right-aligned, zeros on
                // the left; a byte as it is, or its low half byte with the
                // F zone (as the move stored it)
                let field = &bytes[first..end];
                let k = field.len().min(tl);
                for (j, t) in field[field.len() - k..].iter().enumerate() {
                    let at = ta + tl - k + j;
                    let Some(v) = self.sym.as_ref().and_then(|st| t.eval(&|v| st.value(v))) else {
                        continue;
                    };
                    let m = i128::from(self.mem[at]);
                    if t.konst().is_some() {
                        continue;
                    }
                    if m == v {
                        self.sym_bind_byte_term(at, t.clone());
                    } else if m == 0xF0 | (v & 0x0F) {
                        let z = Term::add(Term::Const(0xF0), Term::rem(t.clone(), 16));
                        self.sym_bind_byte_term(at, z);
                    }
                }
            }
            p = end;
            if let Some(k) = found {
                let (repeat, d) = &delims[k];
                p = end + d.len();
                if *repeat {
                    while matches_at(&data, p, d) {
                        self.sym_decide(&prog, text_id(pc, p), delim_at(&bytes, p, d));
                        p += d.len();
                    }
                    if p < data.len() && p - end <= SCAN {
                        self.sym_decide(&prog, text_id(pc, p), delim_at(&bytes, p, d));
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
                Some(d) => Some(self.sym_delim(cur, d)?),
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
                    // as the interpreter: no delimiter longer than the source
                    let mut end = concrete.len();
                    if d.len() <= concrete.len() {
                        for i in 0..=concrete.len() - d.len() {
                            if i < SCAN {
                                let at = delim_at(&terms, i, &d);
                                self.sym_decide(&prog, text_id(pc, pos + i), at);
                            }
                            if matches_at(&concrete, i, &d) {
                                end = i;
                                break;
                            }
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

impl Machine {
    /// FUNCTION LENGTH(FUNCTION TRIM(x)) of followed bytes: 0 when they
    /// are all spaces, else the length without the leading and trailing
    /// spaces (exactly up to 24 bytes; longer: 1 or more, the value of
    /// this run).
    pub(super) fn sym_trim_length(&mut self, cur: usize, s: &Src) -> Option<(Term, i32)> {
        let (terms, text) = self.sym_operand_bytes(cur, s)?;
        if terms.iter().all(|t| t.konst().is_some()) {
            return None;
        }
        let space = |t: &Term| eq(t, 0x40);
        let all = Pred::And(terms.iter().map(space).collect());
        let n = terms.len();
        let rest = if n <= 24 {
            // leading and trailing spaces counted
            let count = |order: &mut dyn Iterator<Item = &Term>| {
                let mut run: Vec<Pred> = Vec::new();
                let mut c = Term::Const(0);
                for t in order {
                    run.push(space(t));
                    c = Term::add(
                        c,
                        Term::ite(Pred::And(run.clone()), Term::Const(1), Term::Const(0)),
                    );
                }
                c
            };
            let lead = count(&mut terms.iter());
            let trail = count(&mut terms.iter().rev());
            Term::sub(Term::Const(n as i128), Term::add(lead, trail))
        } else {
            let first = text.iter().position(|b| *b != 0x40);
            let last = text.iter().rposition(|b| *b != 0x40);
            let now = match (first, last) {
                (Some(f), Some(l)) => l - f + 1,
                _ => 1,
            };
            Term::Const(now as i128)
        };
        Some((Term::ite(all, Term::Const(0), rest), 0))
    }

    /// TEST-NUMVAL / TEST-NUMVAL-C of followed bytes: 0 when the bytes are
    /// a number (spaces, digits, a point, signs; for -C also the currency
    /// sign and commas, with a digit), else the value of this run (or 1).
    pub(super) fn sym_test_numval(
        &mut self,
        cur: usize,
        e: &Ex,
        s: &Src,
        currency: bool,
    ) -> Option<(Term, i32)> {
        let (terms, _) = self.sym_operand_bytes(cur, s)?;
        if terms.iter().all(|t| t.konst().is_some()) {
            return None;
        }
        let now = self.eval_ex(cur, e).ok()?;
        let now = now.rescale(0, zeus_decimal::Rounding::Truncate)?.coef;
        let mut ok = vec![
            (0x40, 0x40),
            (0x4B, 0x4B),
            (0x4E, 0x4E),
            (0x60, 0x60),
            (0xF0, 0xF9),
        ];
        if currency {
            ok.extend([(0x5B, 0x5B), (0x6B, 0x6B)]);
        }
        let valid = Pred::And(vec![
            Pred::And(
                terms
                    .iter()
                    .map(|t| Pred::In(t.clone(), ok.clone()))
                    .collect(),
            ),
            Pred::Or(
                terms
                    .iter()
                    .map(|t| Pred::In(t.clone(), vec![(0xF0, 0xF9)]))
                    .collect(),
            ),
        ]);
        Some((Term::ite(valid, Term::Const(0), Term::Const(now.max(1))), 0))
    }
}

impl Machine {
    /// FUNCTION UPPER-CASE / LOWER-CASE / REVERSE of followed
    /// alphanumeric bytes: the letters' bytes moved (EBCDIC a-i, j-r, s-z
    /// to A-I, J-R, S-Z and back), or the bytes reversed.
    pub(super) fn sym_case(
        &mut self,
        cur: usize,
        f: crate::zir::Func,
        s: &Src,
    ) -> Option<Vec<Term>> {
        use crate::zir::Func;
        if let Src::Loc(l) = s
            && self.loc_type(cur, l).is_national()
        {
            return None;
        }
        let (terms, _) = self.sym_operand_bytes(cur, s)?;
        if terms.iter().all(|t| t.konst().is_some()) {
            return None;
        }
        let lower = vec![(0x81, 0x89), (0x91, 0x99), (0xA2, 0xA9)];
        let upper = vec![(0xC1, 0xC9), (0xD1, 0xD9), (0xE2, 0xE9)];
        Some(match f {
            Func::Reverse => terms.into_iter().rev().collect(),
            Func::UpperCase => terms
                .into_iter()
                .map(|t| {
                    Term::ite(
                        Pred::In(t.clone(), lower.clone()),
                        Term::add(t.clone(), Term::Const(0x40)),
                        t,
                    )
                })
                .collect(),
            Func::LowerCase => terms
                .into_iter()
                .map(|t| {
                    Term::ite(
                        Pred::In(t.clone(), upper.clone()),
                        Term::sub(t.clone(), Term::Const(0x40)),
                        t,
                    )
                })
                .collect(),
            _ => return None,
        })
    }
}

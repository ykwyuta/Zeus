//! Concolic execution of file I/O (see `sym.rs`): the records of the
//! input files (`SymState::files`) are inputs. After a READ the record
//! area follows the variables of the record read; AT END, INVALID KEY
//! and FILE STATUS become conditions on the records and their number.

use super::Machine;
use super::sym::plain_display;
use crate::concolic::{FileInput, Pred, Term, VarKind};
use crate::zir::{AccessMode, Instr, Loc, NumType, OpenMode, Org, Rel};

/// What an I/O statement needs after it ran.
pub(super) enum FilePre {
    /// Not an input file: the I/O condition is concrete.
    Other,
    Open(usize),
    /// READ of a sequential file: input, record index, INTO target.
    SeqRead(u32, usize, u64, Option<(usize, usize)>),
    /// READ of an indexed file: input, (key of reference, search key) for
    /// a READ by key, INTO target.
    KeyRead(
        u32,
        usize,
        Option<(usize, Vec<Term>)>,
        Option<(usize, usize)>,
    ),
    /// START: input, key of reference, search key, relation.
    Start(u32, usize, usize, Vec<Term>, Rel),
    Write(u32, usize),
    /// REWRITE or DELETE.
    Change(usize),
}

/// The integer of a numeric field's digits.
fn field_value(t: &NumType, bytes: &[u8]) -> i128 {
    crate::vm::data::num_value(t, bytes)
        .rescale(t.scale.max(0) as u32, zeus_decimal::Rounding::Truncate)
        .map_or(0, |d| d.coef)
}

fn record_loc(desc: u32) -> Loc {
    Loc {
        desc,
        subs: Vec::new(),
        refmod: None,
    }
}

impl Machine {
    /// The input of file `f`, if it is one and open for input.
    fn sym_input(&mut self, cur: usize, f: u32) -> Option<usize> {
        let name = &self.instances[cur].code.files[f as usize].name;
        self.sym
            .as_ref()?
            .files
            .iter()
            .position(|x| &x.name == name)
    }

    fn sym_file(&self, k: usize) -> FileInput {
        self.sym.as_ref().unwrap().files[k].clone()
    }

    fn sym_file_mut(&mut self, k: usize) -> &mut FileInput {
        &mut self.sym.as_mut().unwrap().files[k]
    }

    /// The terms of the bytes at `[a, a+l)` (constants where nothing
    /// follows an input).
    fn sym_terms(&mut self, a: usize, l: usize) -> Vec<Term> {
        self.sym_read_bytes(a, l).unwrap_or_else(|| {
            self.mem[a..a + l]
                .iter()
                .map(|b| Term::Const(i128::from(*b)))
                .collect()
        })
    }

    fn sym_set_io(&mut self, p: Option<Pred>) {
        // the condition must give what happened
        let p = p.filter(|p| {
            let st = self.sym.as_ref().unwrap();
            p.eval(&|v| st.value(v)) == Some(self.io_cond)
        });
        self.sym.as_mut().unwrap().io_cond = p;
    }

    pub(super) fn sym_io_pre(&mut self, cur: usize, ins: &Instr) -> FilePre {
        let file = match ins {
            Instr::Open(f, _)
            | Instr::Read { file: f, .. }
            | Instr::Start { file: f, .. }
            | Instr::Write { file: f, .. }
            | Instr::Rewrite { file: f, .. }
            | Instr::Delete(f) => *f,
            _ => return FilePre::Other,
        };
        let Some(k) = self.sym_input(cur, file) else {
            return FilePre::Other;
        };
        let def = self.instances[cur].code.files[file as usize].clone();
        let (open, pos, _) = self.read_position(cur, file);
        let reading = matches!(open, Some(OpenMode::Input | OpenMode::InputOutput));
        let indexed = def.org == Org::Indexed;
        match ins {
            Instr::Open(..) => FilePre::Open(k),
            Instr::Read {
                next, into, key, ..
            } if reading => {
                let into = into.as_ref().and_then(|l| self.addr(cur, l).ok());
                match def.org {
                    Org::Sequential | Org::LineSequential => FilePre::SeqRead(file, k, pos, into),
                    Org::Indexed => {
                        let by_key = !(def.access == AccessMode::Sequential
                            || (def.access == AccessMode::Dynamic && *next));
                        let search = if by_key {
                            let which = key.unwrap_or(0) as usize;
                            let fi = self.sym_file(k);
                            let Some(&(off, len)) = fi.keys.get(which) else {
                                return FilePre::Other;
                            };
                            let Ok((ra, _)) = self.addr(cur, &record_loc(def.record)) else {
                                return FilePre::Other;
                            };
                            Some((which, self.sym_terms(ra + off, len)))
                        } else {
                            None
                        };
                        FilePre::KeyRead(file, k, search, into)
                    }
                    Org::Relative => FilePre::Other,
                }
            }
            Instr::Start { rel, key, .. } if indexed => {
                let Ok((ra, _)) = self.addr(cur, &record_loc(def.record)) else {
                    return FilePre::Other;
                };
                let fi = self.sym_file(k);
                let (which, a, l) = match key {
                    Some(loc) => {
                        let Ok((ka, kl)) = self.addr(cur, loc) else {
                            return FilePre::Other;
                        };
                        let which = fi.keys.iter().position(|x| x.0 == ka - ra).unwrap_or(0);
                        (which, ka, kl)
                    }
                    None => match fi.keys.first() {
                        Some(&(off, len)) => (0, ra + off, len),
                        None => return FilePre::Other,
                    },
                };
                let rel = match rel {
                    Rel::Gt => Rel::Gt,
                    Rel::Ge => Rel::Ge,
                    _ => Rel::Eq,
                };
                FilePre::Start(file, k, which, self.sym_terms(a, l), rel)
            }
            Instr::Write { .. } if indexed => FilePre::Write(file, k),
            Instr::Rewrite { .. } | Instr::Delete(_) => FilePre::Change(k),
            _ => FilePre::Other,
        }
    }

    pub(super) fn sym_io_post(&mut self, cur: usize, pre: FilePre) {
        self.sym_io_cond(cur, pre);
        self.sym_fault(cur);
    }

    /// After an I/O statement that could be made to fail (see
    /// `testenv`): the variable `F<file>.<KIND><n>` (1: it fails with 35 or
    /// 30, 2: with '9' and a binary byte) decides the I/O condition and
    /// FILE STATUS.
    fn sym_fault(&mut self, cur: usize) {
        let Some((file, kind, n, how)) = self.take_fault_point() else {
            return;
        };
        let Some(f) = self.instances[cur]
            .code
            .files
            .iter()
            .position(|d| d.name == file)
        else {
            return;
        };
        let def = self.instances[cur].code.files[f].clone();
        let st = self.sym.as_mut().unwrap();
        let v = st.var_named(
            format!("F{file}.{kind}{n}"),
            VarKind::Num {
                digits: 1,
                signed: false,
            },
            i128::from(how),
        );
        let failed = how != 0;
        let fault = Pred::And(vec![
            Pred::Cmp(Term::Var(v), Rel::Ge, Term::Const(1)),
            Pred::Cmp(Term::Var(v), Rel::Le, Term::Const(2)),
        ]);
        let vsam = Pred::Cmp(Term::Var(v), Rel::Eq, Term::Const(2));
        let prev = st.io_cond.take();
        st.io_cond = match (failed, prev) {
            (true, _) => Some(fault.clone()),
            (false, Some(p)) => Some(Pred::Or(vec![fault.clone(), p])),
            (false, None) if !self.io_cond => Some(fault.clone()),
            (false, None) => None,
        };
        let Some(loc) = &def.status else { return };
        let Ok((a, l)) = self.addr(cur, loc) else {
            return;
        };
        if l < 2 {
            return;
        }
        let (open, _, _) = self.read_position(cur, f as u32);
        let code: [u8; 2] =
            if kind == "OPEN" && matches!(open, Some(OpenMode::Input | OpenMode::InputOutput)) {
                [0xF3, 0xF5]
            } else {
                [0xF3, 0xF0]
            };
        let now = self.sym_terms(a, 2);
        self.sym_forget(a, 2);
        for i in 0..2 {
            let other = if failed {
                Term::Const(0xF0)
            } else {
                now[i].clone()
            };
            let t = Term::ite(
                vsam.clone(),
                Term::Const(i128::from(super::testenv::VSAM_STATUS[i])),
                Term::ite(fault.clone(), Term::Const(i128::from(code[i])), other),
            );
            self.sym_bind_byte_term(a + i, t);
        }
    }

    /// The status code of a PCB set by a DL/I stub (bytes 11-12) follows
    /// the variables `C<program>.<n>.A2#11` and `#12`.
    pub(super) fn sym_dli_status(&mut self, prog: &str, n: u32, pcb: usize) {
        self.sym_forget(pcb + 10, 2);
        for i in 10..12 {
            let b = self.mem[pcb + i];
            let st = self.sym.as_mut().unwrap();
            let v = st.var_named(
                format!("C{prog}.{n}.A2#{}", i + 1),
                VarKind::Byte,
                i128::from(b),
            );
            self.sym_bind_byte(pcb + i, v);
        }
    }

    /// The output arguments of a stubbed call follow the variables
    /// `C<program>.<n>.A<k>#<i>`.
    pub(super) fn sym_stub(&mut self, name: &str, n: u32, args: &[Option<(usize, usize)>]) {
        if self.sym.as_ref().is_some_and(|st| st.full()) {
            for (addr, len) in args.iter().flatten() {
                self.sym_forget(*addr, (*len).min(super::testenv::STUB_BYTES));
            }
            return;
        }
        for (k, a) in args.iter().enumerate() {
            let Some((addr, len)) = *a else { continue };
            let l = len.min(super::testenv::STUB_BYTES);
            self.sym_forget(addr, l);
            for i in 0..l {
                let b = self.mem[addr + i];
                let st = self.sym.as_mut().unwrap();
                let v = st.var_named(
                    format!("C{name}.{n}.A{}#{}", k + 1, i + 1),
                    VarKind::Byte,
                    i128::from(b),
                );
                self.sym_bind_byte(addr + i, v);
            }
        }
    }

    fn sym_io_cond(&mut self, cur: usize, pre: FilePre) {
        match pre {
            FilePre::Other => self.sym.as_mut().unwrap().io_cond = None,
            FilePre::Open(k) => {
                let fi = self.sym_file_mut(k);
                fi.reads = 0;
                fi.left = (!fi.modified).then_some(Term::Var(fi.count));
                self.sym.as_mut().unwrap().io_cond = None;
            }
            FilePre::Change(k) => {
                let fi = self.sym_file_mut(k);
                fi.modified = true;
                fi.left = None;
                self.sym.as_mut().unwrap().io_cond = None;
            }
            FilePre::SeqRead(f, k, pos, into) => self.sym_seq_read(cur, f, k, pos, into),
            FilePre::KeyRead(f, k, search, into) => self.sym_key_read(cur, f, k, search, into),
            FilePre::Start(f, k, which, search, rel) => {
                let fi = self.sym_file(k);
                let (p, left) = if fi.modified {
                    (None, None)
                } else {
                    let each = self.sym_matches(&fi, which, &search, rel);
                    let left = each.iter().fold(Term::Const(0), |a, m| {
                        Term::add(a, Term::ite(m.clone(), Term::Const(1), Term::Const(0)))
                    });
                    (Some(Pred::not(Pred::Or(each))), Some(left))
                };
                let fi = self.sym_file_mut(k);
                fi.reads = 0;
                fi.left = left;
                self.sym_set_io(p);
                self.sym_status(cur, f, (0xF2, 0xF3));
            }
            FilePre::Write(f, k) => {
                let fi = self.sym_file(k);
                let def = self.instances[cur].code.files[f as usize].clone();
                let mut p = None;
                if !fi.modified
                    && let Some(&(off, len)) = fi.keys.first()
                    && let Ok((ra, _)) = self.addr(cur, &record_loc(def.record))
                {
                    let key = self.sym_terms(ra + off, len);
                    p = Some(self.sym_found(&fi, 0, &key, Rel::Eq));
                }
                let fi = self.sym_file_mut(k);
                fi.modified = true;
                fi.left = None;
                self.sym_set_io(p);
                self.sym_status(cur, f, (0xF2, 0xF2));
            }
        }
    }

    /// After a READ of a sequential input file: the record's fields follow
    /// its variables, AT END is `count <= pos`.
    fn sym_seq_read(
        &mut self,
        cur: usize,
        f: u32,
        k: usize,
        pos: u64,
        into: Option<(usize, usize)>,
    ) {
        let (_, _, last) = self.read_position(cur, f);
        let fi = self.sym_file(k);
        let got = last == Some(pos) && !self.io_cond;
        if !got && !self.io_cond {
            self.sym.as_mut().unwrap().io_cond = None;
            return;
        }
        let at_end = Pred::Cmp(Term::Var(fi.count), Rel::Le, Term::Const(pos as i128));
        self.sym_set_io(Some(at_end));
        if got {
            self.sym_bind_record(cur, f, &fi, pos as usize, into);
        }
        self.sym_status(cur, f, (0xF1, 0xF0));
    }

    /// After a READ of an indexed input file: by key, INVALID KEY is "no
    /// record has the key"; sequentially (from the start, in key order),
    /// AT END is `count <= reads`.
    fn sym_key_read(
        &mut self,
        cur: usize,
        f: u32,
        k: usize,
        search: Option<(usize, Vec<Term>)>,
        into: Option<(usize, usize)>,
    ) {
        let fi = self.sym_file(k);
        let (_, _, last) = self.read_position(cur, f);
        let got = last.is_some() && !self.io_cond;
        let (p, miss) = match &search {
            Some((which, key)) => (
                (!fi.modified).then(|| Pred::not(self.sym_found(&fi, *which, key, Rel::Eq))),
                (0xF2, 0xF3),
            ),
            None => (
                fi.left
                    .clone()
                    .map(|t| Pred::Cmp(t, Rel::Le, Term::Const(fi.reads as i128))),
                (0xF1, 0xF0),
            ),
        };
        let st = self.sym_file_mut(k);
        if search.is_some() {
            st.left = None;
        } else {
            st.reads += 1;
        }
        self.sym_set_io(p);
        if got {
            // the record read: the input with its primary key
            let def = self.instances[cur].code.files[f as usize].clone();
            if let Some(&(off, len)) = fi.keys.first()
                && let Ok((ra, _)) = self.addr(cur, &record_loc(def.record))
            {
                let key = self.mem[ra + off..ra + off + len].to_vec();
                if let Some(n) = fi
                    .records
                    .iter()
                    .position(|r| r.get(off..off + len) == Some(&key[..]))
                {
                    self.sym_bind_record(cur, f, &fi, n, into);
                }
            }
        }
        self.sym_status(cur, f, miss);
    }

    /// The condition that a record of the input has key `which` in the
    /// relation `rel` with `search` (a leading part of the key): one of
    /// the records there are, or the one more.
    fn sym_found(&mut self, fi: &FileInput, which: usize, search: &[Term], rel: Rel) -> Pred {
        Pred::Or(self.sym_matches(fi, which, search, rel))
    }

    /// For each record (and the one more), the condition that it is there
    /// and its key `which` is in the relation `rel` with `search`.
    fn sym_matches(
        &mut self,
        fi: &FileInput,
        which: usize,
        search: &[Term],
        rel: Rel,
    ) -> Vec<Pred> {
        let Some(&(off, len)) = fi.keys.get(which) else {
            return vec![Pred::Bool(self.io_cond)];
        };
        let l = search.len().min(len);
        let mut any = Vec::new();
        let n = fi.records.len() + usize::from(!fi.blank.is_empty());
        for i in 0..n {
            let key = self.sym_record_terms(fi, i, off, l);
            let mut all = vec![
                Pred::Cmp(Term::Var(fi.count), Rel::Gt, Term::Const(i as i128)),
                Pred::Bytes(key, rel, search[..l].to_vec()),
            ];
            // the one more has a primary key of its own
            if i == fi.records.len()
                && let Some(&(po, pl)) = fi.keys.first()
            {
                let mine = self.sym_record_terms(fi, i, po, pl);
                for j in 0..i {
                    let other = self.sym_record_terms(fi, j, po, pl);
                    all.push(Pred::Bytes(mine.clone(), Rel::Ne, other));
                }
            }
            any.push(Pred::And(all));
        }
        any
    }

    /// The terms of bytes `[off, off+len)` of record `n` of an input
    /// (`n` = the number of records: the one more).
    fn sym_record_terms(&mut self, fi: &FileInput, n: usize, off: usize, len: usize) -> Vec<Term> {
        let rec = fi.records.get(n).unwrap_or(&fi.blank).clone();
        let st = self.sym.as_mut().unwrap();
        (off..off + len)
            .map(|i| {
                let byte = Term::Const(i128::from(rec.get(i).copied().unwrap_or(0x40)));
                let Some(rf) = fi.fields.iter().find(|rf| {
                    rf.offset <= i && i < rf.offset + rf.len && rf.offset + rf.len <= rec.len()
                }) else {
                    return byte;
                };
                match &rf.num {
                    None => Term::Var(st.var_named(
                        format!("F{}.R{}.{}#{}", fi.name, n + 1, rf.name, i - rf.offset + 1),
                        VarKind::Byte,
                        i128::from(rec[i]),
                    )),
                    Some(t) if plain_display(t) => {
                        let v = st.var_named(
                            format!("F{}.R{}.{}", fi.name, n + 1, rf.name),
                            VarKind::Num {
                                digits: t.digits,
                                signed: t.signed,
                            },
                            field_value(t, &rec[rf.offset..rf.offset + rf.len]),
                        );
                        let k = 10i128.pow((rf.offset + rf.len - 1 - i) as u32);
                        Term::add(Term::rem(Term::div(Term::Var(v), k), 10), Term::Const(0xF0))
                    }
                    Some(_) => byte,
                }
            })
            .collect()
    }

    /// The record area (and the INTO target) follow the variables of
    /// record `n` (0-based) of an input.
    fn sym_bind_record(
        &mut self,
        cur: usize,
        f: u32,
        fi: &FileInput,
        n: usize,
        into: Option<(usize, usize)>,
    ) {
        let def = self.instances[cur].code.files[f as usize].clone();
        let Ok((ra, rl)) = self.addr(cur, &record_loc(def.record)) else {
            return;
        };
        let n = n + 1;
        let mut places = vec![(ra, rl)];
        places.extend(into);
        for &(a, l) in &places {
            self.sym_forget(a, l);
        }
        for rf in &fi.fields {
            if rf.offset + rf.len > rl {
                continue;
            }
            let bytes = self.mem[ra + rf.offset..ra + rf.offset + rf.len].to_vec();
            match &rf.num {
                Some(t) => {
                    let st = self.sym.as_mut().unwrap();
                    let v = st.var_named(
                        format!("F{}.R{n}.{}", fi.name, rf.name),
                        VarKind::Num {
                            digits: t.digits,
                            signed: t.signed,
                        },
                        field_value(t, &bytes),
                    );
                    for &(a, l) in &places {
                        if rf.offset + rf.len <= l {
                            self.sym_bind_num(a + rf.offset, rf.len, *t, v);
                        }
                    }
                }
                None => {
                    for (i, b) in bytes.iter().enumerate() {
                        let st = self.sym.as_mut().unwrap();
                        let v = st.var_named(
                            format!("F{}.R{n}.{}#{}", fi.name, rf.name, i + 1),
                            VarKind::Byte,
                            i128::from(*b),
                        );
                        for &(a, l) in &places {
                            if rf.offset + i < l {
                                self.sym_bind_byte(a + rf.offset + i, v);
                            }
                        }
                    }
                }
            }
        }
    }

    /// FILE STATUS after an I/O statement whose condition is followed:
    /// `miss` (e.g. "10", "23") when the condition holds, else "00".
    fn sym_status(&mut self, cur: usize, f: u32, miss: (u8, u8)) {
        let def = &self.instances[cur].code.files[f as usize];
        let Some(loc) = def.status.clone() else {
            return;
        };
        let Some(p) = self.sym.as_ref().unwrap().io_cond.clone() else {
            return;
        };
        let Ok((a, l)) = self.addr(cur, &loc) else {
            return;
        };
        let want = if self.io_cond { miss } else { (0xF0, 0xF0) };
        if l < 2 || self.mem[a..a + 2] != [want.0, want.1] {
            return;
        }
        self.sym_forget(a, 2);
        for (i, m) in [miss.0, miss.1].into_iter().enumerate() {
            let t = Term::ite(p.clone(), Term::Const(i128::from(m)), Term::Const(0xF0));
            if t.konst().is_none() {
                self.sym_bind_byte_term(a + i, t);
            }
        }
    }
}

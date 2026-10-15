//! Record-oriented transmission: consecutive data sets, key-sequenced
//! (VSAM KSDS) data sets, REGIONAL(1) and relative (VSAM RRDS) data sets
//! with numbered records, and LOCATE.

use super::stream::FileState;
use super::value::{SPACE, Value};
use super::*;
use zeus_ds::format::RecordFormat;
use zeus_ds::keyed::{KeySpec, KeyedDataset, Position};

impl Machine {
    pub(crate) fn open_records(&mut self, f: FileId, act: usize) -> R<()> {
        let st = &mut self.files[f as usize];
        let Some(path) = st.path.clone() else {
            return self.undefined_file(f, act).map(|_| ());
        };
        if st.relative {
            st.relative_ds = Some(
                zeus_ds::relative::RelativeDataset::load(&path)
                    .ok()
                    .flatten()
                    .filter(|_| !st.output || st.update)
                    .unwrap_or_default(),
            );
            st.dirty = st.output && !st.update;
        } else if let Some((len, off)) = st.keys {
            let spec = vec![KeySpec {
                offset: off as usize,
                len: len as usize,
                duplicates: false,
            }];
            st.keyed_ds = Some(match KeyedDataset::load(&path, spec.clone(), SPACE) {
                Ok(Some(d)) => d,
                _ => KeyedDataset::new(spec, SPACE),
            });
            st.keyed = true;
        } else {
            let fmt = st.format.unwrap_or(RecordFormat::Fixed(80));
            st.recs = if st.output && !st.update {
                Vec::new()
            } else {
                let data = std::fs::read(&path).unwrap_or_default();
                zeus_ds::format::decode(fmt, &data).unwrap_or_default()
            };
            st.dirty = st.output && !st.update;
        }
        st.rpos = 0;
        st.last_read = None;
        st.last_key.clear();
        Ok(())
    }

    pub(crate) fn close_records(&mut self, f: FileId) -> R<()> {
        // the buffer of the last LOCATE
        if let Some((addr, len, key)) = self.files[f as usize].pending.take() {
            let rec = self.read(addr, len)?;
            let _ = self.put_record(f, rec, key);
        }
        let st = &mut self.files[f as usize];
        let Some(path) = st.path.clone() else {
            return Ok(());
        };
        if let Some(ds) = &st.relative_ds {
            if st.dirty {
                let _ = ds.store(&path);
            }
            st.relative_ds = None;
        } else if let Some(ds) = &st.keyed_ds {
            if st.dirty {
                let _ = ds.store(&path);
            }
            st.keyed_ds = None;
        } else if st.dirty {
            let fmt = st.format.unwrap_or(RecordFormat::Fixed(80));
            let mut data = Vec::new();
            for r in &st.recs {
                zeus_ds::format::encode_into(fmt, r, SPACE, &mut data);
            }
            let _ = zeus_ds::format::write_file(&path, &data);
        }
        st.dirty = false;
        st.recs.clear();
        Ok(())
    }

    fn record_file(
        &mut self,
        e: &Option<Expr>,
        output: bool,
        update: bool,
        act: usize,
    ) -> R<Option<FileId>> {
        let Some(e) = e else {
            return err("record statement without FILE");
        };
        let Value::File(f) = self.eval(e, act)? else {
            return err("FILE option is not a file");
        };
        if !self.files[f as usize].open {
            let upd = update || self.files[f as usize].decl.update;
            if !self.open_file(f, output && !upd, upd, true, act)? {
                return Ok(None);
            }
        }
        Ok(Some(f))
    }

    fn key_of(&mut self, k: &Option<Expr>, act: usize) -> R<Option<Vec<u8>>> {
        Ok(match k {
            Some(e) => match self.eval(e, act)? {
                Value::Char(c) => Some(c),
                _ => return err("KEY is not a character string"),
            },
            None => None,
        })
    }

    /// Stores a record into the INTO target (RECORD when the sizes
    /// differ).
    fn deliver(&mut self, r: &RecordIo, f: FileId, rec: Vec<u8>, act: usize) -> R<()> {
        if let Some((t, size)) = &r.into {
            let Target::Ref(rf) = t else {
                return err("INTO target");
            };
            let var = self.module.vars[rf.var as usize].clone();
            let varying = matches!(
                var.ty,
                Some(Ty::Char {
                    var: Varying::Varying,
                    ..
                })
            );
            if varying && !self.is_aggregate_ref(rf) {
                let loc = self.locate(rf, act)?;
                self.store(&loc, &Value::Char(rec.clone()))?;
            } else {
                let (bit, _) = self.element_or_start(rf, act)?;
                let n = if *size > 0 && !self.is_aggregate_ref(rf) {
                    *size as usize
                } else {
                    self.aggregate_bytes(rf, act)?.len()
                };
                let mut data = rec.clone();
                let mismatch = data.len() != n;
                data.resize(n, 0);
                self.write((bit / 8) as u32, &data[..n])?;
                if mismatch
                    && self.files[f as usize]
                        .format
                        .is_some_and(|x| matches!(x, RecordFormat::Fixed(_)))
                    && rec.len() > n
                {
                    self.raise(Cond::Record(f), 21, act)?;
                }
            }
        }
        if let Some(t) = &r.set {
            let p = self.heap_alloc(rec.len() as u32);
            self.write(p, &rec)?;
            let l = self.target_loc(t, act)?;
            self.assign_target(t, l, Value::Ptr(p), &Ty::Pointer, act)?;
        }
        Ok(())
    }

    fn is_aggregate_ref(&self, r: &Ref) -> bool {
        r.subs.iter().any(|s| s.is_none()) || self.module.vars[r.var as usize].ty.is_none()
    }

    pub(crate) fn read_stmt(&mut self, r: &RecordIo, act: usize) -> R<()> {
        let Some(f) = self.record_file(&r.file, false, false, act)? else {
            return Ok(());
        };
        let key = self.key_of(&r.key, act)?;
        if self.is_slotted(f) {
            return self.read_slot(r, f, key, act);
        }
        let st = &mut self.files[f as usize];
        if let Some(ds) = &st.keyed_ds {
            let found = match &key {
                Some(k) => {
                    let mut k = k.clone();
                    k.resize(ds.keys()[0].len, SPACE);
                    match ds.get(&k) {
                        Some(rec) => {
                            let rec = rec.to_vec();
                            st.last_key = k.clone();
                            Some((k, rec))
                        }
                        None => {
                            st.last_key = k;
                            None
                        }
                    }
                }
                None => {
                    let from = if st.last_key.is_empty() && st.last_read.is_none() {
                        None
                    } else {
                        Some(Position {
                            key: st.last_key.clone(),
                            seq: 0,
                        })
                    };
                    let inclusive = st.last_read.is_none() && !st.last_key.is_empty();
                    match ds.next(0, from.as_ref(), inclusive) {
                        Some(x) => {
                            st.last_key = x.primary.clone();
                            Some((x.primary, x.record))
                        }
                        None => {
                            st.last_read = Some(usize::MAX);
                            return self.raise(Cond::EndFile(f), 70, act);
                        }
                    }
                }
            };
            let Some((k, rec)) = found else {
                return self.raise(Cond::Key(f), 51, act);
            };
            self.files[f as usize].last_read = Some(0);
            self.deliver(r, f, rec, act)?;
            if let Some((t, ty)) = &r.keyto {
                let l = self.target_loc(t, act)?;
                self.assign_target(t, l, Value::Char(k), ty, act)?;
            }
            return Ok(());
        }
        if let Some(n) = &r.ignore {
            let n = self.eval(n, act)?.as_i64().max(0) as usize;
            let st = &mut self.files[f as usize];
            st.rpos = (st.rpos + n).min(st.recs.len() + 1);
            return Ok(());
        }
        let st = &mut self.files[f as usize];
        if st.rpos >= st.recs.len() {
            return self.raise(Cond::EndFile(f), 70, act);
        }
        let rec = st.recs[st.rpos].clone();
        st.last_read = Some(st.rpos);
        st.rpos += 1;
        self.deliver(r, f, rec, act)
    }

    fn source_bytes(&mut self, r: &RecordIo, act: usize) -> R<Vec<u8>> {
        let Some(e) = &r.from else {
            return err("WRITE without FROM");
        };
        Ok(match self.eval(e, act)? {
            Value::Char(c) => c,
            Value::Bit(b) => b
                .chunks(8)
                .map(|c| c.iter().fold(0u8, |a, &x| (a << 1) | u8::from(x)) << (8 - c.len()))
                .collect(),
            _ => return err("FROM is not a variable"),
        })
    }

    pub(crate) fn write_stmt(&mut self, r: &RecordIo, act: usize) -> R<()> {
        let Some(f) = self.record_file(&r.file, true, false, act)? else {
            return Ok(());
        };
        self.flush_locate(f, act)?;
        let rec = self.source_bytes(r, act)?;
        let key = self.key_of(&r.keyfrom, act)?;
        if !self.put_record(f, rec, key)? {
            return self.raise(Cond::Key(f), 52, act);
        }
        Ok(())
    }

    /// Writes the buffer of the file's last LOCATE.
    fn flush_locate(&mut self, f: FileId, act: usize) -> R<()> {
        if let Some((addr, len, key)) = self.files[f as usize].pending.take() {
            let rec = self.read(addr, len)?;
            if !self.put_record(f, rec, key)? {
                return self.raise(Cond::Key(f), 52, act);
            }
        }
        Ok(())
    }

    /// LOCATE: writes the previous buffer, then allocates the based
    /// variable as the next record (its locator set) to be written later.
    pub(crate) fn locate_stmt(&mut self, item: &AllocItem, r: &RecordIo, act: usize) -> R<()> {
        let Some(f) = self.record_file(&r.file, true, false, act)? else {
            return Ok(());
        };
        self.flush_locate(f, act)?;
        self.exec(&Stmt::Allocate(vec![item.clone()]), act)?;
        // the buffer: where the locator now points
        let p = match &item.1 {
            Some(t) => {
                let TargetLoc::Loc(l) = self.target_loc(t, act)? else {
                    return err("LOCATE SET: not a locator variable");
                };
                self.load(&l)?
            }
            None => {
                let Storage::Based(Some(loc)) = &self.module.vars[item.0 as usize].storage else {
                    return err("LOCATE of a BASED variable without a locator");
                };
                let loc = loc.clone();
                self.eval(&loc, act)?
            }
        };
        let Value::Ptr(p) = p else {
            return err("LOCATE: the locator is not a pointer");
        };
        let len = self.var_size(item.0, act, Some(u64::from(p) * 8))? as usize;
        let key = self.key_of(&r.keyfrom, act)?;
        self.files[f as usize].pending = Some((p, len, key));
        Ok(())
    }

    /// Adds a record to an output file (at its key for keyed and
    /// numbered files); false: the key is in use or not valid.
    fn put_record(&mut self, f: FileId, rec: Vec<u8>, key: Option<Vec<u8>>) -> R<bool> {
        if self.is_slotted(f) {
            return Ok(self.write_slot(f, rec, key));
        }
        let st = &mut self.files[f as usize];
        st.dirty = true;
        if let Some(ds) = &mut st.keyed_ds {
            let mut rec = rec;
            if let Some(k) = key {
                let spec = ds.keys()[0];
                if rec.len() < spec.offset + spec.len {
                    rec.resize(spec.offset + spec.len, SPACE);
                }
                let mut k = k;
                k.resize(spec.len, SPACE);
                rec[spec.offset..spec.offset + spec.len].copy_from_slice(&k);
            }
            let k = ds.key_of(&rec, 0);
            st.last_key = k;
            return Ok(ds.insert(rec).is_ok());
        }
        let rec = match st.format {
            Some(RecordFormat::Fixed(n)) => {
                let mut x = rec;
                x.resize(n as usize, SPACE);
                x
            }
            _ => rec,
        };
        st.recs.push(rec);
        Ok(true)
    }

    // ---- REGIONAL(1) and relative (RRDS) data sets ------------------------

    /// Records in numbered slots: REGIONAL(1) (regions from 0, a dummy
    /// record has X'FF' first) or VSAM RRDS (records from 1).
    fn is_slotted(&self, f: FileId) -> bool {
        let st = &self.files[f as usize];
        st.relative_ds.is_some() || (st.decl.regional && st.keyed_ds.is_none())
    }

    /// The slot number of a key (a character string of digits).
    fn slot_of(key: &[u8]) -> Option<u64> {
        zeus_cobol::ebcdic::decode(key).trim().parse().ok()
    }

    fn slot_text(n: u64) -> Vec<u8> {
        zeus_cobol::ebcdic::encode(&n.to_string())
    }

    fn dummy(rec: &[u8]) -> bool {
        rec.first() == Some(&0xFF)
    }

    /// The record in slot `n`, if any.
    fn slot_get(&self, f: FileId, n: u64) -> Option<Vec<u8>> {
        let st = &self.files[f as usize];
        match &st.relative_ds {
            Some(ds) => ds.get(n).map(<[u8]>::to_vec),
            None => st.recs.get(n as usize).filter(|r| !Self::dummy(r)).cloned(),
        }
    }

    /// The first record in slot `from` or after.
    fn slot_next(&self, f: FileId, from: u64) -> Option<(u64, Vec<u8>)> {
        let st = &self.files[f as usize];
        match &st.relative_ds {
            Some(ds) => ds.next_from(from).map(|(n, r)| (n, r.to_vec())),
            None => st
                .recs
                .iter()
                .enumerate()
                .skip(from as usize)
                .find(|(_, r)| !Self::dummy(r))
                .map(|(i, r)| (i as u64, r.clone())),
        }
    }

    fn read_slot(&mut self, r: &RecordIo, f: FileId, key: Option<Vec<u8>>, act: usize) -> R<()> {
        let relative = self.files[f as usize].relative_ds.is_some();
        let found = match &key {
            Some(k) => Self::slot_of(k).and_then(|n| self.slot_get(f, n).map(|r| (n, r))),
            None => {
                let st = &self.files[f as usize];
                let from = match st.last_read {
                    Some(n) => n as u64 + 1,
                    None => u64::from(relative),
                };
                match self.slot_next(f, from) {
                    Some(x) => Some(x),
                    None => return self.raise(Cond::EndFile(f), 70, act),
                }
            }
        };
        let Some((n, rec)) = found else {
            return self.raise(Cond::Key(f), 51, act);
        };
        self.files[f as usize].last_read = Some(n as usize);
        self.deliver(r, f, rec, act)?;
        if let Some((t, ty)) = &r.keyto {
            let l = self.target_loc(t, act)?;
            self.assign_target(t, l, Value::Char(Self::slot_text(n)), ty, act)?;
        }
        Ok(())
    }

    /// WRITE to a numbered slot (KEYFROM) or the next one.
    fn write_slot(&mut self, f: FileId, rec: Vec<u8>, key: Option<Vec<u8>>) -> bool {
        let n = match &key {
            Some(k) => match Self::slot_of(k) {
                Some(n) => Some(n),
                None => return false,
            },
            None => None,
        };
        let st = &mut self.files[f as usize];
        st.dirty = true;
        if let Some(ds) = &mut st.relative_ds {
            return match n {
                Some(n) => ds.insert(n, rec).is_ok(),
                None => {
                    ds.append(rec);
                    true
                }
            };
        }
        let len = match st.format {
            Some(RecordFormat::Fixed(n)) => n as usize,
            _ => rec.len(),
        };
        let mut rec = rec;
        rec.resize(len, SPACE);
        match n {
            Some(n) => {
                let n = n as usize;
                while st.recs.len() <= n {
                    st.recs.push(vec![0xFF; len]);
                }
                if !Self::dummy(&st.recs[n]) && st.decl.direct {
                    return false;
                }
                st.recs[n] = rec;
            }
            None => st.recs.push(rec),
        }
        true
    }

    pub(crate) fn rewrite_stmt(&mut self, r: &RecordIo, act: usize) -> R<()> {
        let Some(f) = self.record_file(&r.file, false, true, act)? else {
            return Ok(());
        };
        if r.from.is_none() {
            return Ok(());
        }
        let rec = self.source_bytes(r, act)?;
        let key = self.key_of(&r.key, act)?;
        if self.is_slotted(f) {
            let n = match &key {
                Some(k) => Self::slot_of(k),
                None => self.files[f as usize].last_read.map(|n| n as u64),
            };
            let Some(n) = n.filter(|&n| self.slot_get(f, n).is_some()) else {
                return self.raise(Cond::Key(f), 51, act);
            };
            let st = &mut self.files[f as usize];
            st.dirty = true;
            match &mut st.relative_ds {
                Some(ds) => {
                    let _ = ds.replace(n, rec);
                }
                None => {
                    let slot = &mut st.recs[n as usize];
                    let mut rec = rec;
                    rec.resize(slot.len(), SPACE);
                    *slot = rec;
                }
            }
            return Ok(());
        }
        let st = &mut self.files[f as usize];
        st.dirty = true;
        if let Some(ds) = &mut st.keyed_ds {
            let mut rec = rec;
            let spec = ds.keys()[0];
            let k = match key {
                Some(mut k) => {
                    k.resize(spec.len, SPACE);
                    k
                }
                None => st.last_key.clone(),
            };
            if rec.len() < spec.offset + spec.len {
                rec.resize(spec.offset + spec.len, SPACE);
            }
            rec[spec.offset..spec.offset + spec.len].copy_from_slice(&k);
            if ds.replace(rec).is_err() {
                return self.raise(Cond::Key(f), 51, act);
            }
            return Ok(());
        }
        let Some(i) = st.last_read else {
            return self.raise(Cond::Error, 1010, act);
        };
        if let Some(slot) = st.recs.get_mut(i) {
            let n = slot.len();
            let mut rec = rec;
            if matches!(st.format, Some(RecordFormat::Fixed(_))) {
                rec.resize(n, SPACE);
            }
            *slot = rec;
        }
        Ok(())
    }

    pub(crate) fn delete_stmt(&mut self, r: &RecordIo, act: usize) -> R<()> {
        let Some(f) = self.record_file(&r.file, false, true, act)? else {
            return Ok(());
        };
        let key = self.key_of(&r.key, act)?;
        if self.is_slotted(f) {
            let n = match &key {
                Some(k) => Self::slot_of(k),
                None => self.files[f as usize].last_read.map(|n| n as u64),
            };
            let Some(n) = n.filter(|&n| self.slot_get(f, n).is_some()) else {
                return self.raise(Cond::Key(f), 51, act);
            };
            let st = &mut self.files[f as usize];
            st.dirty = true;
            match &mut st.relative_ds {
                Some(ds) => {
                    let _ = ds.delete(n);
                }
                // a dummy record
                None => st.recs[n as usize].fill(0xFF),
            }
            return Ok(());
        }
        let st = &mut self.files[f as usize];
        st.dirty = true;
        if let Some(ds) = &mut st.keyed_ds {
            let spec = ds.keys()[0];
            let k = match key {
                Some(mut k) => {
                    k.resize(spec.len, SPACE);
                    k
                }
                None => st.last_key.clone(),
            };
            if ds.delete(&k).is_err() {
                return self.raise(Cond::Key(f), 51, act);
            }
            return Ok(());
        }
        err("DELETE needs a keyed file")
    }
}

#[allow(dead_code)]
fn unused(_: &FileState) {}

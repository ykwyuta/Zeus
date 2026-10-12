//! Record-oriented transmission: consecutive data sets and key-sequenced
//! (VSAM KSDS) data sets.

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
        if let Some((len, off)) = st.keys {
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
        let st = &mut self.files[f as usize];
        let Some(path) = st.path.clone() else {
            return Ok(());
        };
        if let Some(ds) = &st.keyed_ds {
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
        let rec = self.source_bytes(r, act)?;
        let key = self.key_of(&r.keyfrom, act)?;
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
            if ds.insert(rec).is_err() {
                return self.raise(Cond::Key(f), 52, act);
            }
            return Ok(());
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
        Ok(())
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

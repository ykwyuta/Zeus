//! File input-output.
//!
//! Files are held in memory while open and written back on CLOSE:
//!
//! * print files (written with ADVANCING) and LINE SEQUENTIAL files are
//!   host text (ISO 8859-1), one line per record;
//! * record sequential files hold EBCDIC records: fixed-length records
//!   back to back, or variable-length records with a 4-byte record
//!   descriptor word (as RECFM=F and RECFM=V on z/OS).
//!
//! The external file is found through the environment variable
//! `DD_<assignment-name>`, or `DD_<file-name>`, or the assignment name is
//! used as the path.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use super::data as d;
use super::eval::Val;
use super::{Machine, RResult, err};
use crate::ebcdic;
use crate::zir::{AccessMode, Advance, FileDef, Loc, OpenMode, Org, Rel, SortKey, Src};

impl FileState {
    pub(super) fn open_mode(&self) -> Option<OpenMode> {
        self.open
    }
}

#[derive(Default)]
pub struct FileState {
    open: Option<OpenMode>,
    path: Option<PathBuf>,
    /// Sequential and relative records (None = empty relative slot).
    records: Vec<Option<Vec<u8>>>,
    /// Indexed file: primary key -> record.
    index: BTreeMap<Vec<u8>, Vec<u8>>,
    /// Next record position for sequential reading.
    pos: usize,
    /// Current key position for indexed sequential reading.
    key_pos: Option<Vec<u8>>,
    /// Key of reference for sequential reads (0 primary, n alternate).
    key_of_ref: u32,
    at_end: bool,
    /// The previous READ succeeded (for REWRITE/DELETE in sequential access).
    last_read: Option<usize>,
    last_key: Option<Vec<u8>>,
    /// Text output of print / line sequential files.
    text: Vec<u8>,
    /// Line position on the current page (print files).
    line: u32,
    /// A record was written since open (print files).
    written: bool,
    /// File was absent at OPEN (OPTIONAL).
    absent: bool,
    modified: bool,
}

/// (key of reference, primary key, record)
type KeyedRecord = (Vec<u8>, Vec<u8>, Vec<u8>);

pub struct SortSpec {
    pub file: u32,
    pub keys: Vec<SortKey>,
    pub collating: Option<Vec<u16>>,
    pub merge: bool,
}

fn fixed_length(def: &FileDef) -> bool {
    def.min_len == def.max_len && def.record_depending.is_none()
}

impl Machine {
    fn file_def(&self, cur: usize, f: u32) -> FileDef {
        self.instances[cur].code.files[f as usize].clone()
    }

    fn state(&mut self, cur: usize, f: u32) -> &mut FileState {
        let slot = self.instances[cur].files[f as usize];
        &mut self.file_states[slot]
    }

    fn file_path(def: &FileDef) -> PathBuf {
        for key in [
            format!("DD_{}", def.assign),
            format!("DD_{}", def.name),
            format!("dd_{}", def.assign),
        ] {
            if let Ok(p) = std::env::var(&key) {
                return PathBuf::from(p);
            }
        }
        PathBuf::from(&def.assign)
    }

    /// Sets the FILE STATUS item and the I/O condition.
    fn set_status(&mut self, cur: usize, def: &FileDef, code: &str) -> RResult<()> {
        self.io_cond = !code.starts_with('0');
        let b = code.as_bytes();
        self.last_status = [b[0], b[1]];
        if let Some(loc) = &def.status {
            let v = Val::Alnum(ebcdic::encode(code));
            self.move_val(cur, &v, loc)?;
        }
        Ok(())
    }

    /// Reports an I/O error: with a FILE STATUS item or an exception
    /// handler the program continues, otherwise the run ends.
    fn io_error(&mut self, cur: usize, def: &FileDef, code: &str, what: &str) -> RResult<()> {
        self.set_status(cur, def, code)?;
        let f = self.instances[cur]
            .code
            .files
            .iter()
            .position(|d| d.name == def.name);
        let has_use = f.is_some_and(|f| self.find_use(cur, f as u32).is_some());
        if def.status.is_none() && !has_use {
            return err(format!(
                "{}: {what} failed on file {} (status {code})",
                self.instances[cur].code.name, def.name
            ));
        }
        Ok(())
    }

    pub(super) fn open_file(&mut self, cur: usize, f: u32, mode: OpenMode) -> RResult<()> {
        self.last_open_mode = Some(mode);
        let def = self.file_def(cur, f);
        if self.state(cur, f).open.is_some() {
            return self.io_error(cur, &def, "41", "OPEN");
        }
        let path = Self::file_path(&def);
        let exists = path.is_file();
        let mut st = FileState {
            path: Some(path.clone()),
            ..FileState::default()
        };
        match mode {
            OpenMode::Output => {
                st.modified = true;
            }
            OpenMode::Input | OpenMode::InputOutput | OpenMode::Extend => {
                if !exists {
                    if def.optional {
                        st.absent = true;
                    } else if mode == OpenMode::Extend || mode == OpenMode::InputOutput {
                        if def.optional {
                            st.absent = true;
                        } else {
                            return self.io_error(cur, &def, "35", "OPEN");
                        }
                    } else {
                        return self.io_error(cur, &def, "35", "OPEN");
                    }
                } else {
                    let data = fs::read(&path).map_err(|e| super::RunError(e.to_string()))?;
                    load_records(&def, &data, &mut st)?;
                }
                if mode != OpenMode::Input {
                    st.modified = true;
                }
            }
        }
        if def.print && mode == OpenMode::Extend {
            st.text = fs::read(&path).unwrap_or_default();
        }
        st.open = Some(mode);
        let code = if st.absent && def.optional {
            "05"
        } else {
            "00"
        };
        *self.state(cur, f) = st;
        self.set_status(cur, &def, code)
    }

    pub(super) fn close_file(&mut self, cur: usize, f: u32, _lock: bool) -> RResult<()> {
        let def = self.file_def(cur, f);
        let st = std::mem::take(self.state(cur, f));
        let Some(_mode) = st.open else {
            *self.state(cur, f) = st;
            return self.io_error(cur, &def, "42", "CLOSE");
        };
        if st.modified
            && let Some(path) = &st.path
        {
            let data = if def.print || def.org == Org::LineSequential {
                let mut t = st.text.clone();
                if def.org == Org::LineSequential && !def.print {
                    t.clear();
                    for r in st.records.iter().flatten() {
                        let mut line = ebcdic::decode_latin1(r);
                        while line.last() == Some(&b' ') {
                            line.pop();
                        }
                        t.extend(line);
                        t.push(b'\n');
                    }
                }
                t
            } else {
                store_records(&def, &st)
            };
            fs::write(path, data)
                .map_err(|e| super::RunError(format!("writing {}: {e}", path.display())))?;
        }
        self.set_status(cur, &def, "00")
    }

    fn record_len(&mut self, cur: usize, def: &FileDef, record_desc: u32) -> RResult<usize> {
        if let Some(dep) = &def.record_depending {
            let v = self.eval_ex(cur, &crate::zir::Ex::Load(dep.clone()))?;
            let n = d::to_raw(v, 0, false).max(0) as usize;
            return Ok(n.min(def.max_len as usize));
        }
        if fixed_length(def) {
            return Ok(def.max_len as usize);
        }
        Ok(self.instances[cur].code.descs[record_desc as usize].len as usize)
    }

    /// Record area bytes of a file (`len` bytes).
    fn record_bytes(&mut self, cur: usize, def: &FileDef, len: usize) -> RResult<Vec<u8>> {
        let loc = Loc {
            desc: def.record,
            subs: Vec::new(),
            refmod: None,
        };
        let (a, l) = self.addr(cur, &loc)?;
        Ok(self.mem[a..a + l.min(len.max(1))].to_vec())
    }

    fn put_record(
        &mut self,
        cur: usize,
        def: &FileDef,
        rec: &[u8],
        into: Option<&Loc>,
    ) -> RResult<()> {
        let loc = Loc {
            desc: def.record,
            subs: Vec::new(),
            refmod: None,
        };
        let (a, l) = self.addr(cur, &loc)?;
        let n = rec.len().min(l);
        self.mem[a..a + n].copy_from_slice(&rec[..n]);
        if n < l {
            self.mem[a + n..a + l].fill(ebcdic::SPACE);
        }
        if let Some(dep) = &def.record_depending {
            self.set_value(cur, dep, zeus_decimal::Dec::from_i64(rec.len() as i64))?;
        }
        if let Some(into) = into {
            let v = Val::Item(crate::zir::DataType::Group, rec.to_vec());
            self.move_val(cur, &v, into)?;
        }
        Ok(())
    }

    pub(super) fn write_file(
        &mut self,
        cur: usize,
        f: u32,
        record_desc: u32,
        from: Option<&Src>,
        advance: Option<&(bool, Advance)>,
    ) -> RResult<()> {
        let def = self.file_def(cur, f);
        if let Some(src) = from {
            let v = self.eval_src(cur, src)?;
            let loc = Loc {
                desc: record_desc,
                subs: Vec::new(),
                refmod: None,
            };
            self.move_val(cur, &v, &loc)?;
        }
        let mode = self.state(cur, f).open;
        if !matches!(
            mode,
            Some(OpenMode::Output | OpenMode::Extend | OpenMode::InputOutput)
        ) {
            return self.io_error(cur, &def, "48", "WRITE");
        }
        let len = self.record_len(cur, &def, record_desc)?;
        let rec = self.record_bytes(cur, &def, len)?;
        if def.print || def.org == Org::LineSequential && advance.is_some() {
            let mut lines_before = 0u32;
            let mut page_before = false;
            let mut lines_after = 0u32;
            let mut page_after = false;
            match advance {
                Some((before, adv)) => {
                    let n = match adv {
                        Advance::Page => None,
                        Advance::Lines(e) => {
                            let v = self.eval_ex(cur, e)?;
                            Some(d::to_raw(v, 0, false).max(0) as u32)
                        }
                    };
                    match (before, n) {
                        (false, None) => page_before = true,
                        (false, Some(n)) => lines_before = n,
                        (true, None) => page_after = true,
                        (true, Some(n)) => lines_after = n,
                    }
                }
                None => lines_before = 1,
            }
            let st = self.state(cur, f);
            let mut line = ebcdic::decode_latin1(&rec);
            while line.last() == Some(&b' ') {
                line.pop();
            }
            if page_before && st.written {
                st.text.push(0x0C);
            }
            // AFTER n: n-1 blank lines, then the record on the next line.
            if st.written {
                for _ in 1..lines_before.max(1) {
                    st.text.push(b'\n');
                }
            } else {
                for _ in 1..lines_before {
                    st.text.push(b'\n');
                }
            }
            if lines_before == 0 && st.written && !page_before {
                // overprint: carriage return without line feed
                if st.text.last() == Some(&b'\n') {
                    st.text.pop();
                    st.text.push(b'\r');
                }
            }
            st.text.extend_from_slice(&line);
            st.text.push(b'\n');
            for _ in 1..lines_after {
                st.text.push(b'\n');
            }
            if page_after {
                st.text.push(0x0C);
            }
            st.written = true;
            st.modified = true;
            st.line += lines_before.max(1);
            self.io_cond = false;
            return self.set_status(cur, &def, "00");
        }
        match def.org {
            Org::Sequential | Org::LineSequential => {
                let st = self.state(cur, f);
                st.records.push(Some(rec));
                st.modified = true;
                self.set_status(cur, &def, "00")
            }
            Org::Relative => self.relative_write(cur, f, &def, rec),
            Org::Indexed => self.indexed_write(cur, f, &def, rec),
        }
    }

    pub(super) fn rewrite_file(
        &mut self,
        cur: usize,
        f: u32,
        record_desc: u32,
        from: Option<&Src>,
    ) -> RResult<()> {
        let def = self.file_def(cur, f);
        if let Some(src) = from {
            let v = self.eval_src(cur, src)?;
            let loc = Loc {
                desc: record_desc,
                subs: Vec::new(),
                refmod: None,
            };
            self.move_val(cur, &v, &loc)?;
        }
        if self.state(cur, f).open != Some(OpenMode::InputOutput) {
            return self.io_error(cur, &def, "49", "REWRITE");
        }
        let len = self.record_len(cur, &def, record_desc)?;
        let rec = self.record_bytes(cur, &def, len)?;
        match def.org {
            Org::Sequential | Org::LineSequential => {
                let st = self.state(cur, f);
                let Some(i) = st.last_read.take() else {
                    return self.io_error(cur, &def, "43", "REWRITE");
                };
                let old_len = st.records[i].as_ref().map_or(0, |r| r.len());
                if old_len != rec.len() {
                    return self.io_error(cur, &def, "44", "REWRITE");
                }
                st.records[i] = Some(rec);
                st.modified = true;
                self.set_status(cur, &def, "00")
            }
            Org::Relative => self.relative_rewrite(cur, f, &def, rec),
            Org::Indexed => self.indexed_rewrite(cur, f, &def, rec),
        }
    }

    pub(super) fn read_file(
        &mut self,
        cur: usize,
        f: u32,
        next: bool,
        into: Option<&Loc>,
        key: Option<u32>,
    ) -> RResult<()> {
        let def = self.file_def(cur, f);
        let mode = self.state(cur, f).open;
        if !matches!(mode, Some(OpenMode::Input | OpenMode::InputOutput)) {
            return self.io_error(cur, &def, "47", "READ");
        }
        let sequential =
            def.access == AccessMode::Sequential || (def.access == AccessMode::Dynamic && next);
        match def.org {
            Org::Sequential | Org::LineSequential => {
                let st = self.state(cur, f);
                if st.at_end {
                    return self.io_error(cur, &def, "46", "READ");
                }
                if st.absent || st.pos >= st.records.len() {
                    st.at_end = true;
                    self.set_status(cur, &def, "10")?;
                    return Ok(());
                }
                let i = st.pos;
                st.pos += 1;
                st.last_read = Some(i);
                let rec = st.records[i].clone().unwrap_or_default();
                self.put_record(cur, &def, &rec, into)?;
                let code = if rec.len() > def.max_len as usize {
                    "04"
                } else {
                    "00"
                };
                self.set_status(cur, &def, code)
            }
            Org::Relative => {
                if sequential {
                    self.relative_read_next(cur, f, &def, into)
                } else {
                    self.relative_read_key(cur, f, &def, into)
                }
            }
            Org::Indexed => {
                if sequential {
                    self.indexed_read_next(cur, f, &def, into)
                } else {
                    self.indexed_read_key(cur, f, &def, into, key.unwrap_or(0))
                }
            }
        }
    }

    pub(super) fn delete_record(&mut self, cur: usize, f: u32) -> RResult<()> {
        let def = self.file_def(cur, f);
        if self.state(cur, f).open != Some(OpenMode::InputOutput) {
            return self.io_error(cur, &def, "49", "DELETE");
        }
        match def.org {
            Org::Relative => self.relative_delete(cur, f, &def),
            Org::Indexed => self.indexed_delete(cur, f, &def),
            _ => self.io_error(cur, &def, "37", "DELETE"),
        }
    }

    pub(super) fn start_file(
        &mut self,
        cur: usize,
        f: u32,
        rel: Rel,
        key: Option<&Loc>,
    ) -> RResult<()> {
        let def = self.file_def(cur, f);
        if !matches!(
            self.state(cur, f).open,
            Some(OpenMode::Input | OpenMode::InputOutput)
        ) {
            return self.io_error(cur, &def, "47", "START");
        }
        match def.org {
            Org::Relative => self.relative_start(cur, f, &def, rel),
            Org::Indexed => self.indexed_start(cur, f, &def, rel, key),
            _ => self.io_error(cur, &def, "37", "START"),
        }
    }

    // ---- relative files ------------------------------------------------

    fn relative_key(&mut self, cur: usize, def: &FileDef) -> RResult<Option<usize>> {
        match &def.relative_key {
            Some(k) => {
                let v = self.eval_ex(cur, &crate::zir::Ex::Load(k.clone()))?;
                Ok(Some(d::to_raw(v, 0, false).max(0) as usize))
            }
            None => Ok(None),
        }
    }

    fn set_relative_key(&mut self, cur: usize, def: &FileDef, n: usize) -> RResult<()> {
        if let Some(k) = &def.relative_key {
            self.set_value(cur, k, zeus_decimal::Dec::from_i64(n as i64))?;
        }
        Ok(())
    }

    fn relative_write(&mut self, cur: usize, f: u32, def: &FileDef, rec: Vec<u8>) -> RResult<()> {
        let slot = if def.access == AccessMode::Sequential {
            let st = self.state(cur, f);
            st.records.push(Some(rec));
            st.modified = true;
            let n = self.state(cur, f).records.len();
            self.set_relative_key(cur, def, n)?;
            return self.set_status(cur, def, "00");
        } else {
            self.relative_key(cur, def)?.unwrap_or(0)
        };
        if slot == 0 {
            return self.set_status(cur, def, "24");
        }
        let st = self.state(cur, f);
        if st.records.len() < slot {
            st.records.resize(slot, None);
        }
        if st.records[slot - 1].is_some() {
            return self.set_status(cur, def, "22");
        }
        st.records[slot - 1] = Some(rec);
        st.modified = true;
        self.set_status(cur, def, "00")
    }

    fn relative_read_next(
        &mut self,
        cur: usize,
        f: u32,
        def: &FileDef,
        into: Option<&Loc>,
    ) -> RResult<()> {
        let st = self.state(cur, f);
        if st.at_end {
            return self.io_error(cur, def, "46", "READ");
        }
        while st.pos < st.records.len() && st.records[st.pos].is_none() {
            st.pos += 1;
        }
        if st.pos >= st.records.len() {
            st.at_end = true;
            return self.set_status(cur, def, "10");
        }
        let i = st.pos;
        st.pos += 1;
        st.last_read = Some(i);
        let rec = st.records[i].clone().unwrap();
        self.set_relative_key(cur, def, i + 1)?;
        self.put_record(cur, def, &rec, into)?;
        self.set_status(cur, def, "00")
    }

    fn relative_read_key(
        &mut self,
        cur: usize,
        f: u32,
        def: &FileDef,
        into: Option<&Loc>,
    ) -> RResult<()> {
        let slot = self.relative_key(cur, def)?.unwrap_or(0);
        let st = self.state(cur, f);
        let rec = if slot >= 1 {
            st.records.get(slot - 1).cloned().flatten()
        } else {
            None
        };
        match rec {
            Some(r) => {
                st.last_read = Some(slot - 1);
                st.pos = slot;
                self.put_record(cur, def, &r, into)?;
                self.set_status(cur, def, "00")
            }
            None => self.set_status(cur, def, "23"),
        }
    }

    fn relative_rewrite(&mut self, cur: usize, f: u32, def: &FileDef, rec: Vec<u8>) -> RResult<()> {
        let slot = if def.access == AccessMode::Sequential {
            match self.state(cur, f).last_read.take() {
                Some(i) => i + 1,
                None => return self.io_error(cur, def, "43", "REWRITE"),
            }
        } else {
            self.relative_key(cur, def)?.unwrap_or(0)
        };
        let st = self.state(cur, f);
        if slot == 0 || st.records.get(slot - 1).is_none_or(|r| r.is_none()) {
            return self.set_status(cur, def, "23");
        }
        st.records[slot - 1] = Some(rec);
        st.modified = true;
        self.set_status(cur, def, "00")
    }

    fn relative_delete(&mut self, cur: usize, f: u32, def: &FileDef) -> RResult<()> {
        let slot = if def.access == AccessMode::Sequential {
            match self.state(cur, f).last_read.take() {
                Some(i) => i + 1,
                None => return self.io_error(cur, def, "43", "DELETE"),
            }
        } else {
            self.relative_key(cur, def)?.unwrap_or(0)
        };
        let st = self.state(cur, f);
        if slot == 0 || st.records.get(slot - 1).is_none_or(|r| r.is_none()) {
            return self.set_status(cur, def, "23");
        }
        st.records[slot - 1] = None;
        st.modified = true;
        self.set_status(cur, def, "00")
    }

    fn relative_start(&mut self, cur: usize, f: u32, def: &FileDef, rel: Rel) -> RResult<()> {
        let key = self.relative_key(cur, def)?.unwrap_or(0);
        let st = self.state(cur, f);
        let n = st.records.len();
        let found = (1..=n).find(|&i| {
            st.records[i - 1].is_some()
                && match rel {
                    Rel::Eq => i == key,
                    Rel::Gt => i > key,
                    Rel::Ge => i >= key,
                    _ => false,
                }
        });
        match found {
            Some(i) => {
                st.pos = i - 1;
                st.at_end = false;
                self.set_status(cur, def, "00")
            }
            None => self.set_status(cur, def, "23"),
        }
    }

    // ---- indexed files -------------------------------------------------

    fn key_bytes(&mut self, cur: usize, def: &FileDef, rec: &[u8], which: u32) -> RResult<Vec<u8>> {
        let loc = if which == 0 {
            def.record_key.clone()
        } else {
            def.alternate_keys
                .get(which as usize - 1)
                .map(|(l, _)| l.clone())
        };
        let Some(loc) = loc else {
            return err(format!("file {} has no such key", def.name));
        };
        // Key position relative to the record area.
        let rec_loc = Loc {
            desc: def.record,
            subs: Vec::new(),
            refmod: None,
        };
        let (ra, _) = self.addr(cur, &rec_loc)?;
        let (ka, kl) = self.addr(cur, &loc)?;
        let off = ka - ra;
        let mut k = rec
            .get(off..off + kl)
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        k.resize(kl, ebcdic::SPACE);
        Ok(k)
    }

    fn indexed_write(&mut self, cur: usize, f: u32, def: &FileDef, rec: Vec<u8>) -> RResult<()> {
        let key = self.key_bytes(cur, def, &rec, 0)?;
        // alternate keys without duplicates must be unique
        for (i, (_, dup)) in def.alternate_keys.iter().enumerate() {
            if !dup {
                let ak = self.key_bytes(cur, def, &rec, i as u32 + 1)?;
                let others: Vec<Vec<u8>> = self.state(cur, f).index.values().cloned().collect();
                for o in others {
                    if self.key_bytes(cur, def, &o, i as u32 + 1)? == ak {
                        return self.set_status(cur, def, "22");
                    }
                }
            }
        }
        let st = self.state(cur, f);
        if def.access == AccessMode::Sequential
            && st.open == Some(OpenMode::Output)
            && st.index.keys().next_back().is_some_and(|k| *k >= key)
        {
            return self.set_status(cur, def, "21");
        }
        if st.index.contains_key(&key) {
            return self.set_status(cur, def, "22");
        }
        st.index.insert(key, rec);
        st.modified = true;
        self.set_status(cur, def, "00")
    }

    fn indexed_read_key(
        &mut self,
        cur: usize,
        f: u32,
        def: &FileDef,
        into: Option<&Loc>,
        which: u32,
    ) -> RResult<()> {
        let rec_now = self.record_bytes(cur, def, def.max_len as usize)?;
        let key = self.key_bytes(cur, def, &rec_now, which)?;
        let found = if which == 0 {
            self.state(cur, f)
                .index
                .get(&key)
                .cloned()
                .map(|r| (key.clone(), r))
        } else {
            let all: Vec<(Vec<u8>, Vec<u8>)> = self
                .state(cur, f)
                .index
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let mut hit = None;
            for (pk, r) in all {
                if self.key_bytes(cur, def, &r, which)? == key {
                    hit = Some((pk, r));
                    break;
                }
            }
            hit
        };
        match found {
            Some((pk, r)) => {
                let st = self.state(cur, f);
                st.last_key = Some(pk.clone());
                st.key_pos = Some(pk);
                st.key_of_ref = which;
                st.at_end = false;
                self.put_record(cur, def, &r, into)?;
                self.set_status(cur, def, "00")
            }
            None => self.set_status(cur, def, "23"),
        }
    }

    /// Records in the order of the key of reference.
    fn ordered(&mut self, cur: usize, f: u32, def: &FileDef) -> RResult<Vec<KeyedRecord>> {
        let which = self.state(cur, f).key_of_ref;
        let all: Vec<(Vec<u8>, Vec<u8>)> = self
            .state(cur, f)
            .index
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let mut out = Vec::new();
        for (pk, r) in all {
            let k = if which == 0 {
                pk.clone()
            } else {
                self.key_bytes(cur, def, &r, which)?
            };
            out.push((k, pk, r));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(out)
    }

    fn indexed_read_next(
        &mut self,
        cur: usize,
        f: u32,
        def: &FileDef,
        into: Option<&Loc>,
    ) -> RResult<()> {
        if self.state(cur, f).at_end {
            return self.io_error(cur, def, "46", "READ");
        }
        let ordered = self.ordered(cur, f, def)?;
        let st = self.state(cur, f);
        let next = match (&st.key_pos, st.last_key.is_some()) {
            (None, _) => ordered.first().cloned(),
            (Some(pos), false) => {
                // positioned by START: first record >= position
                ordered.iter().find(|(_, pk, _)| pk == pos).cloned()
            }
            (Some(pos), true) => {
                let idx = ordered.iter().position(|(_, pk, _)| pk == pos);
                match idx {
                    Some(i) => ordered.get(i + 1).cloned(),
                    None => ordered.iter().find(|(_, pk, _)| pk > pos).cloned(),
                }
            }
        };
        match next {
            Some((_, pk, r)) => {
                st.key_pos = Some(pk.clone());
                st.last_key = Some(pk);
                self.put_record(cur, def, &r, into)?;
                self.set_status(cur, def, "00")
            }
            None => {
                st.at_end = true;
                self.set_status(cur, def, "10")
            }
        }
    }

    fn indexed_rewrite(&mut self, cur: usize, f: u32, def: &FileDef, rec: Vec<u8>) -> RResult<()> {
        let key = self.key_bytes(cur, def, &rec, 0)?;
        let st = self.state(cur, f);
        if def.access == AccessMode::Sequential && st.last_key.as_ref() != Some(&key) {
            return self.set_status(cur, def, "21");
        }
        if !st.index.contains_key(&key) {
            return self.set_status(cur, def, "23");
        }
        st.index.insert(key, rec);
        st.modified = true;
        self.set_status(cur, def, "00")
    }

    fn indexed_delete(&mut self, cur: usize, f: u32, def: &FileDef) -> RResult<()> {
        let rec_now = self.record_bytes(cur, def, def.max_len as usize)?;
        let key = if def.access == AccessMode::Sequential {
            match self.state(cur, f).last_key.clone() {
                Some(k) => k,
                None => return self.io_error(cur, def, "43", "DELETE"),
            }
        } else {
            self.key_bytes(cur, def, &rec_now, 0)?
        };
        let st = self.state(cur, f);
        if st.index.remove(&key).is_none() {
            return self.set_status(cur, def, "23");
        }
        st.modified = true;
        self.set_status(cur, def, "00")
    }

    fn indexed_start(
        &mut self,
        cur: usize,
        f: u32,
        def: &FileDef,
        rel: Rel,
        key: Option<&Loc>,
    ) -> RResult<()> {
        let which = match key {
            Some(k) => {
                if def.record_key.as_ref().is_some_and(|rk| rk.desc == k.desc) {
                    0
                } else {
                    def.alternate_keys
                        .iter()
                        .position(|(l, _)| l.desc == k.desc)
                        .map_or(0, |p| p as u32 + 1)
                }
            }
            None => 0,
        };
        // The search value: the key item (or a leading part of it).
        let search = match key {
            Some(k) => {
                let (a, l) = self.addr(cur, k)?;
                self.mem[a..a + l].to_vec()
            }
            None => {
                let rec_now = self.record_bytes(cur, def, def.max_len as usize)?;
                self.key_bytes(cur, def, &rec_now, 0)?
            }
        };
        self.state(cur, f).key_of_ref = which;
        let ordered = self.ordered(cur, f, def)?;
        let n = search.len();
        let found = ordered.iter().find(|(k, _, _)| {
            let kp = &k[..n.min(k.len())];
            match rel {
                Rel::Eq => kp == search.as_slice(),
                Rel::Gt => kp > search.as_slice(),
                Rel::Ge => kp >= search.as_slice(),
                _ => false,
            }
        });
        let st = self.state(cur, f);
        match found {
            Some((_, pk, _)) => {
                st.key_pos = Some(pk.clone());
                st.last_key = None;
                st.at_end = false;
                self.set_status(cur, def, "00")
            }
            None => self.set_status(cur, def, "23"),
        }
    }

    // ---- SORT / MERGE --------------------------------------------------

    pub(super) fn sort_record_bytes(
        &mut self,
        cur: usize,
        f: u32,
        from: Option<&Src>,
    ) -> RResult<Vec<u8>> {
        let def = self.file_def(cur, f);
        if let Some(src) = from {
            let v = self.eval_src(cur, src)?;
            let loc = Loc {
                desc: def.record,
                subs: Vec::new(),
                refmod: None,
            };
            self.move_val(cur, &v, &loc)?;
        }
        self.record_bytes(cur, &def, def.max_len as usize)
    }

    pub(super) fn sort_read_file(&mut self, cur: usize, f: u32) -> RResult<()> {
        let def = self.file_def(cur, f);
        let path = Self::file_path(&def);
        let data = fs::read(&path).unwrap_or_default();
        let mut st = FileState::default();
        load_records(&def, &data, &mut st)?;
        for r in st.records.into_iter().flatten() {
            let mut r = r;
            r.resize(self.file_def(cur, f).max_len as usize, ebcdic::SPACE);
            self.sort_records.push(r);
        }
        for r in st.index.into_values() {
            self.sort_records.push(r);
        }
        Ok(())
    }

    pub(super) fn sort_sort(
        &mut self,
        cur: usize,
        spec: &SortSpec,
        mut records: Vec<Vec<u8>>,
    ) -> RResult<Vec<Vec<u8>>> {
        let def = self.file_def(cur, spec.file);
        let rec_loc = Loc {
            desc: def.record,
            subs: Vec::new(),
            refmod: None,
        };
        let (ra, _) = self.addr(cur, &rec_loc)?;
        let code = self.instances[cur].code.clone();
        let mut keys = Vec::new();
        for k in &spec.keys {
            let loc = Loc {
                desc: k.desc,
                subs: Vec::new(),
                refmod: None,
            };
            let (ka, kl) = self.addr(cur, &loc)?;
            keys.push((
                ka - ra,
                kl,
                k.ascending,
                code.descs[k.desc as usize].ty.clone(),
            ));
        }
        let coll = spec.collating.clone();
        let _ = spec.merge;
        records.sort_by(|a, b| {
            for (off, len, asc, ty) in &keys {
                let x = a.get(*off..off + len).unwrap_or(&[]);
                let y = b.get(*off..off + len).unwrap_or(&[]);
                let o = if ty.is_numeric() {
                    d::value_of(ty, x).cmp(&d::value_of(ty, y))
                } else {
                    let mut o = std::cmp::Ordering::Equal;
                    for (p, q) in x.iter().zip(y) {
                        let (wp, wq) = match &coll {
                            Some(c) => (c[*p as usize], c[*q as usize]),
                            None => (u16::from(*p), u16::from(*q)),
                        };
                        o = wp.cmp(&wq);
                        if o != std::cmp::Ordering::Equal {
                            break;
                        }
                    }
                    o
                };
                let o = if *asc { o } else { o.reverse() };
                if o != std::cmp::Ordering::Equal {
                    return o;
                }
            }
            std::cmp::Ordering::Equal
        });
        Ok(records)
    }

    pub(super) fn sort_write_file(
        &mut self,
        cur: usize,
        f: u32,
        records: &[Vec<u8>],
    ) -> RResult<()> {
        self.open_file(cur, f, OpenMode::Output)?;
        let def = self.file_def(cur, f);
        for r in records {
            let loc = Loc {
                desc: def.record,
                subs: Vec::new(),
                refmod: None,
            };
            let v = Val::Item(crate::zir::DataType::Group, r.clone());
            self.move_val(cur, &v, &loc)?;
            self.write_file(cur, f, def.record, None, None)?;
        }
        self.close_file(cur, f, false)
    }

    pub(super) fn sort_return_record(
        &mut self,
        cur: usize,
        f: u32,
        into: Option<&Loc>,
    ) -> RResult<()> {
        let def = self.file_def(cur, f);
        let next = match &mut self.sort_return {
            Some((recs, pos)) => {
                let r = recs.get(*pos).cloned();
                *pos += 1;
                r
            }
            None => return err("RETURN outside an output procedure"),
        };
        match next {
            Some(r) => {
                self.put_record(cur, &def, &r, into)?;
                self.io_cond = false;
            }
            None => self.io_cond = true,
        }
        Ok(())
    }
}

fn load_records(def: &FileDef, data: &[u8], st: &mut FileState) -> RResult<()> {
    let recs: Vec<Vec<u8>> = if def.org == Org::LineSequential || def.print {
        data.split(|&b| b == b'\n')
            .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
            .map(|l| l.iter().map(|&b| ebcdic::from_latin1(b)).collect())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .skip_while(|_| false)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    } else if fixed_length(def) && def.org != Org::Relative && def.org != Org::Indexed {
        data.chunks(def.max_len.max(1) as usize)
            .map(<[u8]>::to_vec)
            .collect()
    } else {
        // RDW-prefixed records; empty relative slots have length 4.
        let mut out = Vec::new();
        let mut i = 0;
        while i + 4 <= data.len() {
            let len = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
            if len < 4 || i + len > data.len() {
                return err(format!("file {} is damaged", def.name));
            }
            out.push(data[i + 4..i + len].to_vec());
            i += len;
        }
        out
    };
    let mut recs = recs;
    if (def.org == Org::LineSequential || def.print) && recs.last().is_some_and(|r| r.is_empty()) {
        recs.pop();
    }
    match def.org {
        Org::Indexed => {
            // records are stored with the key's offset known only at run
            // time; keep them in insertion order keyed by a sequence.
            for (n, r) in recs.into_iter().enumerate() {
                if r.is_empty() {
                    continue;
                }
                let (k, rec) = split_keyed(&r, n);
                st.index.insert(k, rec);
            }
        }
        Org::Relative => {
            st.records = recs
                .into_iter()
                .map(|r| if r.is_empty() { None } else { Some(r) })
                .collect();
        }
        _ => st.records = recs.into_iter().map(Some).collect(),
    }
    Ok(())
}

/// Indexed files are stored as RDW records whose first two bytes give the
/// primary key length, followed by the key and the record.
fn split_keyed(r: &[u8], n: usize) -> (Vec<u8>, Vec<u8>) {
    if r.len() >= 2 {
        let kl = u16::from_be_bytes([r[0], r[1]]) as usize;
        if 2 + kl <= r.len() {
            return (r[2..2 + kl].to_vec(), r[2 + kl..].to_vec());
        }
    }
    ((n as u64).to_be_bytes().to_vec(), r.to_vec())
}

fn store_records(def: &FileDef, st: &FileState) -> Vec<u8> {
    let mut out = Vec::new();
    let rdw = |out: &mut Vec<u8>, r: &[u8]| {
        let len = (r.len() + 4) as u16;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(r);
    };
    match def.org {
        Org::Indexed => {
            for (k, r) in &st.index {
                let mut rec = Vec::with_capacity(2 + k.len() + r.len());
                rec.extend_from_slice(&(k.len() as u16).to_be_bytes());
                rec.extend_from_slice(k);
                rec.extend_from_slice(r);
                rdw(&mut out, &rec);
            }
        }
        Org::Relative => {
            for r in &st.records {
                rdw(&mut out, r.as_deref().unwrap_or(&[]));
            }
        }
        _ if fixed_length(def) => {
            for r in st.records.iter().flatten() {
                let mut r = r.clone();
                r.resize(def.max_len as usize, ebcdic::SPACE);
                out.extend_from_slice(&r);
            }
        }
        _ => {
            for r in st.records.iter().flatten() {
                rdw(&mut out, r);
            }
        }
    }
    out
}

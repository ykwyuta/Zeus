//! File input-output: COBOL file semantics over the Zeus data set layer.
//!
//! The data sets themselves (record formats, relative slots, keyed data
//! sets with alternate indexes, DD allocation) are in `zeus-ds`. This
//! module adds what COBOL defines: OPEN modes, FILE STATUS codes, the
//! record area, AT END / INVALID KEY, RECORD VARYING, and the print files
//! (ADVANCING, LINAGE, Report Writer), which are written as host text.
//!
//! Record formats on disk:
//!
//! * print files and LINE SEQUENTIAL files: host text (ISO 8859-1), one
//!   line per record;
//! * record sequential files: fixed-length EBCDIC records back to back, or
//!   variable-length records with a record descriptor word (RECFM=F / V);
//! * relative and indexed files: variable-length records (see `zeus-ds`).

use std::path::PathBuf;

use zeus_ds::DsError;
use zeus_ds::format::RecordFormat;
use zeus_ds::keyed::{KeySpec, KeyedDataset, Position};
use zeus_ds::physical::Layout;
use zeus_ds::relative::RelativeDataset;
use zeus_ds::seq::BufferPool;
use zeus_ds::seq::SequentialDataset;
use zeus_ds::tape::{self, Entry};

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

/// The records of an open file.
#[derive(Default)]
enum Data {
    #[default]
    None,
    Seq(SequentialDataset),
    Rel(RelativeDataset),
    Keyed(KeyedDataset),
}

#[derive(Default)]
pub struct FileState {
    buffers: BufferPool,
    tape_start: usize,
    tape_next: usize,
    tape_entries: Vec<Entry>,
    no_rewind: bool,
    removed: bool,
    open: Option<OpenMode>,
    path: Option<PathBuf>,
    data: Data,
    /// Sequential files: index of the next record. Relative files: number
    /// of the next slot to read.
    pos: u64,
    /// Indexed files: position for READ NEXT in the key of reference.
    key_pos: Option<Position>,
    /// Key of reference for sequential reads (0 primary, n alternate).
    key_of_ref: u32,
    at_end: bool,
    /// The previous READ succeeded (for REWRITE/DELETE in sequential
    /// access): record index / slot number.
    last_read: Option<u64>,
    /// Indexed files: primary key of the last record read.
    last_key: Option<Vec<u8>>,
    /// Text output of print / line sequential files.
    pub(super) text: Vec<u8>,
    /// Line position on the current page (print files).
    line: u32,
    /// A record was written since open (print files).
    pub(super) written: bool,
    /// File was absent at OPEN (OPTIONAL).
    absent: bool,
    pub(super) modified: bool,
    /// Closed WITH LOCK: the file cannot be opened again in this run.
    locked: bool,
    /// LINAGE files: page body lines, footing, top and bottom margins.
    linage: (u32, u32, u32, u32),
    /// LINAGE-COUNTER: current line of the page body.
    lc: u32,
    /// Something was printed on the current line.
    line_used: bool,
}

impl FileState {
    fn seq(&mut self) -> &mut SequentialDataset {
        match &mut self.data {
            Data::Seq(d) => d,
            _ => unreachable!("not a sequential file"),
        }
    }

    fn rel(&mut self) -> &mut RelativeDataset {
        match &mut self.data {
            Data::Rel(d) => d,
            _ => unreachable!("not a relative file"),
        }
    }

    fn keyed(&mut self) -> &mut KeyedDataset {
        match &mut self.data {
            Data::Keyed(d) => d,
            _ => unreachable!("not an indexed file"),
        }
    }
}

pub struct SortSpec {
    pub file: u32,
    pub keys: Vec<SortKey>,
    pub collating: Option<Vec<u16>>,
    pub merge: bool,
}

fn fixed_length(def: &FileDef) -> bool {
    def.min_len == def.max_len && def.record_depending.is_none()
}

/// Host text files: print files and LINE SEQUENTIAL files.
fn is_text(def: &FileDef) -> bool {
    def.print || def.org == Org::LineSequential
}

/// Record format of a sequential file.
fn seq_format(def: &FileDef) -> RecordFormat {
    if is_text(def) {
        RecordFormat::Text
    } else if def.recording_mode == Some('F') || (def.recording_mode.is_none() && fixed_length(def))
    {
        RecordFormat::Fixed(def.max_len)
    } else {
        RecordFormat::Variable
    }
}

fn ds_err(e: DsError) -> super::RunError {
    super::RunError(e.to_string())
}

impl Machine {
    fn tape_label(
        &self,
        def: &FileDef,
        layout: Layout,
        id: &str,
        number: u8,
        blocks: usize,
        sequence: usize,
    ) -> Vec<u8> {
        let mut label = vec![b' '; 80];
        label[..3].copy_from_slice(id.as_bytes());
        label[3] = b'0' + number;
        if number == 1 {
            let name = def.assign.as_bytes();
            let len = name.len().min(17);
            label[4..4 + len].copy_from_slice(&name[..len]);
            label[21..27].copy_from_slice(b"ZEUS01");
            label[27..41].copy_from_slice(b"00010001000100");
            label[31..35].copy_from_slice(format!("{:04}", sequence.min(9999)).as_bytes());
            // Current creation date in the IBM century/year/day form.
            let days = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
                / 86400;
            let (year, _, _) = super::civil_from_days(days);
            let day = days - super::days_from_civil(year, 1, 1) + 1;
            let date = format!(
                "{}{:02}{:03}",
                if year >= 2000 { '0' } else { ' ' },
                year % 100,
                day
            );
            label[41..47].copy_from_slice(date.as_bytes());
            label[47..53].copy_from_slice(b" 00000");
            label[53] = b'0';
            label[54..60].copy_from_slice(format!("{:06}", blocks.min(999999)).as_bytes());
            label[60..64].copy_from_slice(b"ZEUS");
        } else {
            label[4] = if layout.mode == 'S' {
                b'V'
            } else {
                layout.mode as u8
            };
            label[5..10].copy_from_slice(format!("{:05}", layout.block_size).as_bytes());
            label[10..15].copy_from_slice(
                format!(
                    "{:05}",
                    layout.record_len
                        + if matches!(layout.mode, 'V' | 'S') {
                            4
                        } else {
                            0
                        }
                )
                .as_bytes(),
            );
            label[38] = match layout.mode {
                'S' => b'R',
                'F' | 'V' => b'B',
                _ => b' ',
            };
        }
        ebcdic::from_host(&label)
    }

    fn tape_headers(&self, def: &FileDef, layout: Layout, entries: &mut Vec<Entry>) {
        let sequence = 1 + entries
            .iter()
            .filter(
                |entry| matches!(entry, Entry::Block(b) if b.starts_with(&ebcdic::encode("EOF1"))),
            )
            .count();
        if entries.is_empty() {
            let mut volume = vec![ebcdic::SPACE; 80];
            volume[..10].copy_from_slice(&ebcdic::encode("VOL1ZEUS01"));
            entries.push(Entry::Block(volume));
        }
        entries.push(Entry::Block(
            self.tape_label(def, layout, "HDR", 1, 0, sequence),
        ));
        entries.push(Entry::Block(
            self.tape_label(def, layout, "HDR", 2, 0, sequence),
        ));
        entries.push(Entry::Mark);
    }

    fn tape_trailers(&self, def: &FileDef, layout: Layout, count: usize, entries: &mut Vec<Entry>) {
        let sequence = 1 + entries
            .iter()
            .filter(
                |entry| matches!(entry, Entry::Block(b) if b.starts_with(&ebcdic::encode("EOF1"))),
            )
            .count();
        entries.push(Entry::Block(
            self.tape_label(def, layout, "EOF", 1, count, sequence),
        ));
        entries.push(Entry::Block(
            self.tape_label(def, layout, "EOF", 2, count, sequence),
        ));
        entries.push(Entry::Mark);
    }
    fn is_tape(&self, def: &FileDef, path: &std::path::Path) -> bool {
        self.dd.io_attributes(&[&def.assign, &def.name]).tape
            || path
                .extension()
                .is_some_and(|s| s.eq_ignore_ascii_case("aws"))
    }

    fn physical_layout(&self, def: &FileDef, path: &std::path::Path) -> Option<Layout> {
        if def.org != Org::Sequential || def.print {
            return None;
        }
        let dd = self.dd.io_attributes(&[&def.assign, &def.name]);
        let tape = self.is_tape(def, path);
        if !tape
            && def.block_size.is_none()
            && dd.block_size.is_none()
            && !matches!(def.recording_mode, Some('U' | 'S'))
        {
            return None;
        }
        let mode = def
            .recording_mode
            .unwrap_or(if fixed_length(def) { 'F' } else { 'V' });
        let automatic = match mode {
            'F' => (32760 / def.max_len.max(1)).max(1) * def.max_len,
            'U' => def.max_len,
            _ => 32760,
        };
        let block_size = dd
            .block_size
            .or(def.block_size)
            .filter(|&n| n != 0)
            .unwrap_or(if def.block_size.is_none() && dd.block_size.is_none() {
                def.max_len + if matches!(mode, 'V' | 'S') { 8 } else { 0 }
            } else {
                automatic
            });
        Some(Layout {
            mode,
            record_len: def.max_len as usize,
            block_size: block_size as usize,
            write_only: def.write_only,
        })
    }

    pub(super) fn open_tape_options(
        &mut self,
        cur: usize,
        f: u32,
        mode: OpenMode,
        reversed: bool,
        no_rewind: bool,
    ) -> RResult<()> {
        let def = self.file_def(cur, f);
        let path = self.file_path(&def);
        let tape = self.is_tape(&def, &path);
        // On direct access media these phrases have no effect per IBM.
        let old = self.tape_positions.get(&path).copied();
        if tape && !no_rewind {
            self.tape_positions.insert(path.clone(), 0);
        }
        self.open_file(cur, f, mode)?;
        if self.state(cur, f).open.is_none() {
            if let Some(pos) = old {
                self.tape_positions.insert(path, pos);
            }
            return Ok(());
        }
        if tape && reversed {
            if self.file_def(cur, f).recording_mode != Some('F') {
                return err("OPEN REVERSED requires fixed-format tape records");
            }
            self.state(cur, f).seq().records.reverse();
        }
        Ok(())
    }

    pub(super) fn close_tape_options(
        &mut self,
        cur: usize,
        f: u32,
        no_rewind: bool,
        removal: bool,
    ) -> RResult<()> {
        let def = self.file_def(cur, f);
        let tape = self.is_tape(&def, &self.file_path(&def));
        if tape {
            self.state(cur, f).no_rewind = no_rewind;
        }
        self.close_file(cur, f, false)?;
        if tape {
            self.state(cur, f).removed = removal;
        }
        Ok(())
    }
    fn file_def(&self, cur: usize, f: u32) -> FileDef {
        self.instances[cur].code.files[f as usize].clone()
    }

    pub(super) fn state(&mut self, cur: usize, f: u32) -> &mut FileState {
        let slot = self.instances[cur].files[f as usize];
        &mut self.file_states[slot]
    }

    fn file_path(&self, def: &FileDef) -> PathBuf {
        self.dd
            .resolve(&[&def.assign, &def.name])
            .unwrap_or_else(|| PathBuf::from(&def.assign))
    }

    /// Record format of a sequential file: the allocated data set's when
    /// known, otherwise derived from the FD.
    fn file_format(&self, def: &FileDef) -> RecordFormat {
        if is_text(def) {
            return RecordFormat::Text;
        }
        self.dd
            .allocation(&[&def.assign, &def.name])
            .and_then(|a| a.format)
            .unwrap_or_else(|| seq_format(def))
    }

    /// Key specifications of an indexed file: offsets of the RECORD KEY
    /// and ALTERNATE RECORD KEYs in the record area.
    fn key_specs(&mut self, cur: usize, def: &FileDef) -> RResult<Vec<KeySpec>> {
        let (ra, _) = self.addr(cur, &record_loc(def))?;
        let mut keys = Vec::new();
        let Some(rk) = &def.record_key else {
            return err(format!("indexed file {} has no RECORD KEY", def.name));
        };
        let (ka, kl) = self.addr(cur, rk)?;
        keys.push(KeySpec {
            offset: ka - ra,
            len: kl,
            duplicates: false,
        });
        for (l, dup) in &def.alternate_keys {
            let (ka, kl) = self.addr(cur, l)?;
            keys.push(KeySpec {
                offset: ka - ra,
                len: kl,
                duplicates: *dup,
            });
        }
        Ok(keys)
    }

    /// Loads the data set of a file; None when it does not exist.
    fn load_data(
        &mut self,
        cur: usize,
        def: &FileDef,
        path: &std::path::Path,
        buffers: &mut BufferPool,
    ) -> RResult<Option<Data>> {
        Ok(match def.org {
            Org::Indexed => {
                let keys = self.key_specs(cur, def)?;
                KeyedDataset::load(path, keys, ebcdic::SPACE)
                    .map_err(ds_err)?
                    .map(Data::Keyed)
            }
            Org::Relative => RelativeDataset::load(path).map_err(ds_err)?.map(Data::Rel),
            Org::Sequential | Org::LineSequential => {
                let text = self.file_format(def) == RecordFormat::Text;
                if let Some(layout) = self.physical_layout(def, path) {
                    let Some(bytes) = buffers.read(path).map_err(ds_err)? else {
                        return Ok(None);
                    };
                    let blocks = if self.is_tape(def, path) {
                        let entries = tape::decode(&bytes).map_err(ds_err)?;
                        let pos = self.tape_positions.get(path).copied().unwrap_or(0);
                        tape::file_with_labels(&entries, pos, def.label_standard)
                            .map_err(ds_err)?
                            .blocks
                    } else {
                        layout.split(&bytes).map_err(ds_err)?
                    };
                    let mut data = SequentialDataset::new(self.file_format(def), ebcdic::SPACE);
                    data.records = layout.decode(&blocks).map_err(ds_err)?;
                    if def.code_set_ascii {
                        for r in &mut data.records {
                            *r = r.iter().map(|&b| ebcdic::from_latin1(b)).collect();
                        }
                    }
                    return Ok(Some(Data::Seq(data)));
                }
                buffers
                    .read(path)
                    .and_then(|data| {
                        data.map(|bytes| {
                            let mut data =
                                SequentialDataset::new(self.file_format(def), ebcdic::SPACE);
                            data.records = zeus_ds::format::decode(self.file_format(def), &bytes)?;
                            Ok(data)
                        })
                        .transpose()
                    })
                    .map_err(ds_err)?
                    .map(|mut d| {
                        if text || def.code_set_ascii {
                            for r in &mut d.records {
                                *r = if text {
                                    ebcdic::from_host(r)
                                } else {
                                    r.iter().map(|&b| ebcdic::from_latin1(b)).collect()
                                };
                            }
                        }
                        Data::Seq(d)
                    })
            }
        })
    }

    /// An empty data set for a file.
    fn empty_data(&mut self, cur: usize, def: &FileDef) -> RResult<Data> {
        Ok(match def.org {
            Org::Indexed => {
                Data::Keyed(KeyedDataset::new(self.key_specs(cur, def)?, ebcdic::SPACE))
            }
            Org::Relative => Data::Rel(RelativeDataset::default()),
            _ => Data::Seq(SequentialDataset::new(self.file_format(def), ebcdic::SPACE)),
        })
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
        if let Some(loc) = &def.vsam_status {
            let (addr, len) = self.addr(cur, loc)?;
            self.mem[addr..addr + len].fill(0);
        }
        Ok(())
    }

    /// A logical failure from the virtual VSAM access method. Validation
    /// failures detected before accessing it keep the extended status zero.
    fn set_vsam_status(
        &mut self,
        cur: usize,
        def: &FileDef,
        code: &str,
        reason: u16,
        component: u16,
    ) -> RResult<()> {
        self.set_status(cur, def, code)?;
        if let Some(loc) = &def.vsam_status {
            let (addr, _) = self.addr(cur, loc)?;
            for (slot, value) in [8u16, component, reason].iter().enumerate() {
                self.mem[addr + slot * 2..addr + slot * 2 + 2]
                    .copy_from_slice(&value.to_be_bytes());
            }
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
        if def.external
            && self
                .instances
                .iter()
                .flat_map(|i| i.code.files.iter())
                .any(|other| {
                    other.external && other.name == def.name && other.reserve != def.reserve
                })
        {
            return err("EXTERNAL file RESERVE declarations disagree");
        }
        let peers: Vec<u32> = self.instances[cur]
            .code
            .files
            .iter()
            .enumerate()
            .filter(|(_, d)| def.same_area.contains(&d.name))
            .map(|(i, _)| i as u32)
            .collect();
        for peer in peers {
            if self.state(cur, peer).open.is_some() {
                return err(format!(
                    "SAME AREA: {} cannot be opened while another file in its area is open",
                    def.name
                ));
            }
        }
        if self.state(cur, f).open.is_some() {
            return self.io_error(cur, &def, "41", "OPEN");
        }
        if self.state(cur, f).locked {
            return self.io_error(cur, &def, "38", "OPEN");
        }
        // In a job step only the step's DD statements are allocated: a
        // file without one cannot be opened (LE CBLQDA(OFF), the default),
        // except an OPTIONAL file opened INPUT, which is absent.
        let unallocated =
            self.step.dds.is_some() && self.dd.allocation(&[&def.assign, &def.name]).is_none();
        if unallocated && !(def.optional && mode == OpenMode::Input) {
            return self.io_error(cur, &def, "35", "OPEN");
        }
        let path = self.file_path(&def);
        if self.state(cur, f).removed {
            return self.io_error(cur, &def, "30", "OPEN (tape removed)");
        }
        let attributes = self.dd.io_attributes(&[&def.assign, &def.name]);
        let layout = self.physical_layout(&def, &path);
        if let Some(layout) = layout {
            layout.validate().map_err(ds_err)?;
            if layout.mode == 'U' && !self.is_tape(&def, &path) {
                return self.io_error(cur, &def, "39", "OPEN U requires AWSTAPE");
            }
        }
        let count = def
            .reserve
            .filter(|&n| n != 0)
            .or(attributes.buffers.filter(|&n| n != 0))
            .unwrap_or(2);
        let buffer_size = layout.map_or(def.max_len.max(1) as usize, |l| l.block_size);
        let mut st = FileState {
            path: Some(path.clone()),
            buffers: BufferPool::new(count, buffer_size).map_err(ds_err)?,
            pos: 1,
            ..FileState::default()
        };
        if self.is_tape(&def, &path) {
            st.tape_entries = st
                .buffers
                .read(&path)
                .map_err(ds_err)?
                .map(|b| tape::decode(&b).map_err(ds_err))
                .transpose()?
                .unwrap_or_default();
            let position = if let Some(number) = attributes.file_sequence {
                tape::sequence_with_labels(&st.tape_entries, number, def.label_standard)
                    .map_err(ds_err)?
            } else {
                self.tape_positions.get(&path).copied().unwrap_or(0)
            };
            st.tape_start = position;
            st.tape_next = if position < st.tape_entries.len() && mode != OpenMode::Output {
                tape::file_with_labels(&st.tape_entries, position, def.label_standard)
                    .map_err(ds_err)?
                    .next
            } else {
                position
            };
            self.tape_positions.insert(path.clone(), position);
        }
        if mode == OpenMode::Output {
            st.modified = true;
            st.data = self.empty_data(cur, &def)?;
        } else {
            let data = if unallocated {
                None
            } else {
                self.load_data(cur, &def, &path, &mut st.buffers)?
            };
            match data {
                Some(data) => st.data = data,
                None if def.optional => {
                    st.absent = true;
                    st.data = self.empty_data(cur, &def)?;
                }
                None => return self.io_error(cur, &def, "35", "OPEN"),
            }
            if mode != OpenMode::Input {
                st.modified = true;
            }
        }
        if matches!(st.data, Data::Seq(_)) {
            st.pos = 0;
            let fmt = self.file_format(&def);
            self.opened
                .push((def.assign.to_ascii_uppercase(), fmt, def.max_len));
        }
        if def.print && mode == OpenMode::Extend {
            st.text = zeus_ds::format::read_file(&path)
                .map_err(ds_err)?
                .unwrap_or_default();
        }
        st.open = Some(mode);
        self.opened_buffers
            .push((def.assign.clone(), count, buffer_size as u32));
        let code = if st.absent && def.optional {
            "05"
        } else {
            "00"
        };
        *self.state(cur, f) = st;
        if def.linage.is_some() && mode != OpenMode::Input {
            self.linage_page(cur, f, &def)?;
            let top = self.state(cur, f).linage.2;
            let st = self.state(cur, f);
            st.text.extend(std::iter::repeat_n(b'\n', top as usize));
            self.set_linage_counter(cur, &def, 1)?;
        }
        self.set_status(cur, &def, code)
    }

    /// Evaluates the LINAGE values for a new page.
    fn linage_page(&mut self, cur: usize, f: u32, def: &FileDef) -> RResult<()> {
        let l = def.linage.as_ref().unwrap();
        let mut num = |e: &Option<crate::zir::Ex>, dflt: u32| -> RResult<u32> {
            Ok(match e {
                Some(e) => {
                    let v = self.eval_ex(cur, e)?;
                    d::to_raw(v, 0, false).max(0) as u32
                }
                None => dflt,
            })
        };
        let lines = num(&Some(l.lines.clone()), 1)?.max(1);
        let footing = num(&l.footing, lines)?;
        let top = num(&l.top, 0)?;
        let bottom = num(&l.bottom, 0)?;
        let st = self.state(cur, f);
        st.linage = (lines, footing, top, bottom);
        st.lc = 1;
        st.line_used = false;
        Ok(())
    }

    fn set_linage_counter(&mut self, cur: usize, def: &FileDef, lc: u32) -> RResult<()> {
        let loc = Loc {
            desc: def.linage.as_ref().unwrap().counter,
            subs: Vec::new(),
            refmod: None,
        };
        self.set_value(cur, &loc, zeus_decimal::Dec::from_i64(lc as i64))
    }

    /// Moves a LINAGE file to the first body line of the next page.
    fn linage_new_page(&mut self, cur: usize, f: u32, def: &FileDef) -> RResult<()> {
        let (lines, _, _, bottom) = self.state(cur, f).linage;
        let lc = self.state(cur, f).lc;
        self.linage_page(cur, f, def)?;
        let top = self.state(cur, f).linage.2;
        let n = lines.saturating_sub(lc) + 1 + bottom + top;
        let st = self.state(cur, f);
        st.text.extend(std::iter::repeat_n(b'\n', n as usize));
        Ok(())
    }

    /// WRITE to a file with a LINAGE clause. Returns the END-OF-PAGE
    /// condition.
    fn linage_write(
        &mut self,
        cur: usize,
        f: u32,
        def: &FileDef,
        line: Vec<u8>,
        before: bool,
        n: Option<u32>,
    ) -> RResult<bool> {
        let mut overflow = false;
        let print = |st: &mut FileState, line: &[u8]| {
            if st.line_used {
                st.text.push(b'\r');
            }
            st.text.extend_from_slice(line);
            st.line_used = true;
            st.written = true;
            st.modified = true;
        };
        let advance = |m: &mut Machine, k: u32, overflow: &mut bool| -> RResult<()> {
            let st = m.state(cur, f);
            let lines = st.linage.0;
            if st.lc + k > lines {
                m.linage_new_page(cur, f, def)?;
                *overflow = true;
            } else {
                st.lc += k;
                st.text.extend(std::iter::repeat_n(b'\n', k as usize));
                if k > 0 {
                    st.line_used = false;
                }
            }
            Ok(())
        };
        if before {
            print(self.state(cur, f), &line);
            match n {
                None => self.linage_new_page(cur, f, def)?,
                Some(k) => advance(self, k, &mut overflow)?,
            }
        } else {
            match n {
                None => self.linage_new_page(cur, f, def)?,
                Some(k) => advance(self, k, &mut overflow)?,
            }
            print(self.state(cur, f), &line);
        }
        let (lc, footing) = {
            let st = self.state(cur, f);
            (st.lc, st.linage.1)
        };
        self.set_linage_counter(cur, def, lc)?;
        Ok(overflow || lc >= footing)
    }

    /// CLOSE REEL/UNIT: no effect on a non-reel file (status 07).
    pub(super) fn close_reel(&mut self, cur: usize, f: u32) -> RResult<()> {
        let def = self.file_def(cur, f);
        if self.is_tape(&def, &self.file_path(&def)) {
            return self.close_tape_options(cur, f, false, true);
        }
        if self.state(cur, f).open.is_none() {
            return self.io_error(cur, &def, "42", "CLOSE");
        }
        self.set_status(cur, &def, "07")
    }

    pub(super) fn close_file(&mut self, cur: usize, f: u32, lock: bool) -> RResult<()> {
        let def = self.file_def(cur, f);
        let mut st = std::mem::take(self.state(cur, f));
        let Some(_mode) = st.open else {
            *self.state(cur, f) = st;
            return self.io_error(cur, &def, "42", "CLOSE");
        };
        if st.modified
            && let Some(path) = &st.path
        {
            let r = if let Some(layout) = self.physical_layout(&def, path) {
                let Data::Seq(data) = &st.data else {
                    return err("physical file is not sequential");
                };
                let mut records = data.records.clone();
                if def.code_set_ascii {
                    for r in &mut records {
                        *r = ebcdic::decode_latin1(r);
                    }
                }
                let blocks = layout
                    .encode(
                        &records,
                        if def.code_set_ascii {
                            b' '
                        } else {
                            ebcdic::SPACE
                        },
                    )
                    .map_err(ds_err)?;
                let bytes = if self.is_tape(&def, path) {
                    let mut entries =
                        st.tape_entries[..st.tape_start.min(st.tape_entries.len())].to_vec();
                    if def.label_standard {
                        self.tape_headers(&def, layout, &mut entries);
                    }
                    entries.extend(blocks.iter().cloned().map(Entry::Block));
                    entries.push(Entry::Mark);
                    if def.label_standard {
                        self.tape_trailers(&def, layout, blocks.len(), &mut entries);
                    }
                    st.tape_next = entries.len();
                    entries.push(Entry::Mark);
                    tape::encode(&entries).map_err(ds_err)?
                } else {
                    blocks.concat()
                };
                st.buffers.write(path, &bytes)
            } else if def.print {
                let mut t = st.text.clone();
                if def.linage.is_some() && st.line_used {
                    t.push(b'\n');
                }
                zeus_ds::format::write_file(path, &t)
            } else {
                match &st.data {
                    Data::Seq(d) if self.file_format(&def) == RecordFormat::Text => {
                        let mut text = d.clone();
                        for r in &mut text.records {
                            let mut line = ebcdic::to_host(r);
                            while line.last() == Some(&b' ') {
                                line.pop();
                            }
                            *r = line;
                        }
                        st.buffers.write(path, &text.to_bytes())
                    }
                    Data::Seq(d) if def.code_set_ascii => {
                        let mut converted = d.clone();
                        for r in &mut converted.records {
                            *r = ebcdic::decode_latin1(r);
                        }
                        st.buffers.write(path, &converted.to_bytes())
                    }
                    Data::Seq(d) => st.buffers.write(path, &d.to_bytes()),
                    Data::Rel(d) => d.store(path),
                    Data::Keyed(d) => d.store(path),
                    Data::None => Ok(()),
                }
            };
            r.map_err(ds_err)?;
        }
        if let Some(path) = &st.path
            && self.is_tape(&def, path)
        {
            self.tape_positions
                .insert(path.clone(), if st.no_rewind { st.tape_next } else { 0 });
        }
        self.state(cur, f).locked = lock;
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
        let sequential = matches!(def.org, Org::Sequential | Org::LineSequential);
        let allowed = match mode {
            Some(OpenMode::Output | OpenMode::Extend) => true,
            Some(OpenMode::InputOutput) => !sequential,
            _ => false,
        };
        if !allowed {
            return self.io_error(cur, &def, "48", "WRITE");
        }
        if let Some(dep) = &def.record_depending {
            let v = self.eval_ex(cur, &crate::zir::Ex::Load(dep.clone()))?;
            let n = d::to_raw(v, 0, false);
            if n < def.min_len as i128 || n > def.max_len as i128 {
                return self.io_error(cur, &def, "44", "WRITE");
            }
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
            let mut line = ebcdic::to_host(&rec);
            while line.last() == Some(&b' ') {
                line.pop();
            }
            if def.linage.is_some() {
                let (before, n) = if page_before {
                    (false, None)
                } else if page_after {
                    (true, None)
                } else if lines_after > 0 || matches!(advance, Some((true, _))) {
                    (true, Some(lines_after))
                } else {
                    (false, Some(lines_before))
                };
                let eop = self.linage_write(cur, f, &def, line, before, n)?;
                self.set_status(cur, &def, "00")?;
                self.io_cond = eop;
                return Ok(());
            }
            let st = self.state(cur, f);
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
                st.seq().records.push(rec);
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
                let records = &mut st.seq().records;
                if records[i as usize].len() != rec.len() {
                    return self.io_error(cur, &def, "44", "REWRITE");
                }
                records[i as usize] = rec;
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
        // REWRITE / DELETE need a successful READ immediately before.
        self.state(cur, f).last_read = None;
        match def.org {
            Org::Sequential | Org::LineSequential => {
                let st = self.state(cur, f);
                if st.at_end {
                    return self.io_error(cur, &def, "46", "READ");
                }
                let i = st.pos;
                let Some(rec) = st.seq().records.get(i as usize).cloned() else {
                    st.at_end = true;
                    self.set_status(cur, &def, "10")?;
                    return Ok(());
                };
                st.pos += 1;
                st.last_read = Some(i);
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
        if def.access == AccessMode::Sequential {
            let st = self.state(cur, f);
            let n = st.rel().append(rec);
            st.modified = true;
            self.set_relative_key(cur, def, n as usize)?;
            return self.set_status(cur, def, "00");
        }
        let slot = self.relative_key(cur, def)?.unwrap_or(0) as u64;
        let st = self.state(cur, f);
        match st.rel().insert(slot, rec) {
            Ok(()) => {
                st.modified = true;
                self.set_status(cur, def, "00")
            }
            Err(DsError::InvalidRecordNumber) => self.set_status(cur, def, "24"),
            Err(_) => self.set_vsam_status(cur, def, "22", 8, 0),
        }
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
        let pos = st.pos;
        let Some((n, rec)) = st.rel().next_from(pos).map(|(n, r)| (n, r.to_vec())) else {
            st.at_end = true;
            return self.set_vsam_status(cur, def, "10", 4, 0);
        };
        // the record number must fit the RELATIVE KEY item
        if let Some(k) = &def.relative_key
            && let crate::zir::DataType::Numeric(t) =
                &self.instances[cur].code.descs[k.desc as usize].ty
            && t.digits < 18
            && n >= 10u64.pow(t.digits)
        {
            return self.set_status(cur, def, "14");
        }
        let st = self.state(cur, f);
        st.pos = n + 1;
        st.last_read = Some(n);
        self.set_relative_key(cur, def, n as usize)?;
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
        let slot = self.relative_key(cur, def)?.unwrap_or(0) as u64;
        let st = self.state(cur, f);
        match st.rel().get(slot).map(<[u8]>::to_vec) {
            Some(r) => {
                st.last_read = Some(slot);
                st.pos = slot + 1;
                self.put_record(cur, def, &r, into)?;
                self.set_status(cur, def, "00")
            }
            None => self.set_vsam_status(cur, def, "23", 16, 0),
        }
    }

    /// Slot addressed by REWRITE / DELETE: the record read last in
    /// sequential access, the RELATIVE KEY otherwise.
    fn relative_target(&mut self, cur: usize, f: u32, def: &FileDef) -> RResult<Option<u64>> {
        if def.access == AccessMode::Sequential {
            Ok(self.state(cur, f).last_read.take())
        } else {
            Ok(Some(self.relative_key(cur, def)?.unwrap_or(0) as u64))
        }
    }

    fn relative_rewrite(&mut self, cur: usize, f: u32, def: &FileDef, rec: Vec<u8>) -> RResult<()> {
        let Some(slot) = self.relative_target(cur, f, def)? else {
            return self.io_error(cur, def, "43", "REWRITE");
        };
        let st = self.state(cur, f);
        if st.rel().replace(slot, rec).is_err() {
            return self.set_vsam_status(cur, def, "23", 16, 0);
        }
        st.modified = true;
        self.set_status(cur, def, "00")
    }

    fn relative_delete(&mut self, cur: usize, f: u32, def: &FileDef) -> RResult<()> {
        let Some(slot) = self.relative_target(cur, f, def)? else {
            return self.io_error(cur, def, "43", "DELETE");
        };
        let st = self.state(cur, f);
        if st.rel().delete(slot).is_err() {
            return self.set_vsam_status(cur, def, "23", 16, 0);
        }
        st.modified = true;
        self.set_status(cur, def, "00")
    }

    fn relative_start(&mut self, cur: usize, f: u32, def: &FileDef, rel: Rel) -> RResult<()> {
        let key = self.relative_key(cur, def)?.unwrap_or(0) as u64;
        let st = self.state(cur, f);
        match st.rel().position(ds_rel(rel), key) {
            Some(n) => {
                st.pos = n;
                st.at_end = false;
                self.set_status(cur, def, "00")
            }
            None => self.set_vsam_status(cur, def, "23", 16, 0),
        }
    }

    // ---- indexed files -------------------------------------------------

    /// Value of key `which` in the record area.
    fn area_key(&mut self, cur: usize, f: u32, def: &FileDef, which: usize) -> RResult<Vec<u8>> {
        let rec = self.record_bytes(cur, def, def.max_len as usize)?;
        Ok(self.state(cur, f).keyed().key_of(&rec, which))
    }

    fn indexed_write(&mut self, cur: usize, f: u32, def: &FileDef, rec: Vec<u8>) -> RResult<()> {
        let st = self.state(cur, f);
        let ds = st.keyed();
        let key = ds.key_of(&rec, 0);
        if def.access == AccessMode::Sequential
            && st.open == Some(OpenMode::Output)
            && st
                .keyed()
                .last_primary()
                .is_some_and(|k| k >= key.as_slice())
        {
            return self.set_status(cur, def, "21");
        }
        let st = self.state(cur, f);
        if let Err(e) = st.keyed().insert(rec) {
            let component = if e == DsError::DuplicateAlternateKey {
                2
            } else {
                0
            };
            return self.set_vsam_status(cur, def, "22", 8, component);
        }
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
        let key = self.area_key(cur, f, def, which as usize)?;
        let st = self.state(cur, f);
        match st.keyed().get_by(which as usize, &key) {
            Some(found) => {
                st.last_key = Some(found.primary);
                st.last_read = Some(0);
                st.key_pos = Some(found.position);
                st.key_of_ref = which;
                st.at_end = false;
                self.put_record(cur, def, &found.record, into)?;
                self.set_status(cur, def, "00")
            }
            None => self.set_vsam_status(cur, def, "23", 16, if which == 0 { 0 } else { 2 }),
        }
    }

    fn indexed_read_next(
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
        // after START the positioned record itself is read next
        let inclusive = st.last_key.is_none();
        let which = st.key_of_ref as usize;
        let from = st.key_pos.clone();
        match st.keyed().next(which, from.as_ref(), inclusive) {
            Some(found) => {
                st.key_pos = Some(found.position);
                st.last_read = Some(0);
                st.last_key = Some(found.primary);
                self.put_record(cur, def, &found.record, into)?;
                self.set_status(cur, def, "00")
            }
            None => {
                st.at_end = true;
                self.set_vsam_status(cur, def, "10", 4, if which == 0 { 0 } else { 2 })
            }
        }
    }

    fn indexed_rewrite(&mut self, cur: usize, f: u32, def: &FileDef, rec: Vec<u8>) -> RResult<()> {
        let st = self.state(cur, f);
        let key = st.keyed().key_of(&rec, 0);
        if def.access == AccessMode::Sequential {
            if st.last_read.take().is_none() {
                return self.io_error(cur, def, "43", "REWRITE");
            }
            if st.last_key.as_ref() != Some(&key) {
                return self.set_status(cur, def, "21");
            }
        }
        match st.keyed().replace(rec) {
            Ok(()) => {
                st.modified = true;
                self.set_status(cur, def, "00")
            }
            Err(DsError::NotFound) => self.set_vsam_status(cur, def, "23", 16, 0),
            Err(e) => self.set_vsam_status(
                cur,
                def,
                "22",
                8,
                if e == DsError::DuplicateAlternateKey {
                    2
                } else {
                    0
                },
            ),
        }
    }

    fn indexed_delete(&mut self, cur: usize, f: u32, def: &FileDef) -> RResult<()> {
        let key = if def.access == AccessMode::Sequential {
            let st = self.state(cur, f);
            match (st.last_read.take(), st.last_key.clone()) {
                (Some(_), Some(k)) => k,
                _ => return self.io_error(cur, def, "43", "DELETE"),
            }
        } else {
            self.area_key(cur, f, def, 0)?
        };
        let st = self.state(cur, f);
        if st.keyed().delete(&key).is_err() {
            return self.set_vsam_status(cur, def, "23", 16, 0);
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
        // The key of reference: the key whose leftmost character is the
        // leftmost character of the named item; the search value is that
        // item (a leading part of the key).
        let (which, search) = match key {
            Some(k) => {
                let (ka, kl) = self.addr(cur, k)?;
                let (ra, _) = self.addr(cur, &record_loc(def))?;
                let offset = ka - ra;
                let which = self
                    .state(cur, f)
                    .keyed()
                    .keys()
                    .iter()
                    .position(|s| s.offset == offset)
                    .unwrap_or(0);
                (which, self.mem[ka..ka + kl].to_vec())
            }
            None => (0, self.area_key(cur, f, def, 0)?),
        };
        let st = self.state(cur, f);
        st.key_of_ref = which as u32;
        match st.keyed().position(which, &search, ds_rel(rel)) {
            Some(found) => {
                st.key_pos = Some(found.position);
                st.last_key = None;
                st.at_end = false;
                self.set_status(cur, def, "00")
            }
            None => self.set_vsam_status(cur, def, "23", 16, if which == 0 { 0 } else { 2 }),
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
        let path = self.file_path(&def);
        let len = def.max_len as usize;
        let add = |m: &mut Machine, r: &[u8]| {
            let mut r = r.to_vec();
            r.resize(len, ebcdic::SPACE);
            m.sort_records.push(r);
        };
        let mut buffers = BufferPool::new(2, def.max_len.max(1) as usize).map_err(ds_err)?;
        match self.load_data(cur, &def, &path, &mut buffers)? {
            Some(Data::Seq(d)) => d.records.iter().for_each(|r| add(self, r)),
            Some(Data::Rel(d)) => {
                let mut n = 1;
                while let Some((k, r)) = d.next_from(n) {
                    add(self, r);
                    n = k + 1;
                }
            }
            Some(Data::Keyed(d)) => d.records().for_each(|r| add(self, r)),
            Some(Data::None) | None => {}
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

fn record_loc(def: &FileDef) -> Loc {
    Loc {
        desc: def.record,
        subs: Vec::new(),
        refmod: None,
    }
}

fn ds_rel(rel: Rel) -> zeus_ds::Rel {
    match rel {
        Rel::Gt => zeus_ds::Rel::Gt,
        Rel::Ge => zeus_ds::Rel::Ge,
        _ => zeus_ds::Rel::Eq,
    }
}

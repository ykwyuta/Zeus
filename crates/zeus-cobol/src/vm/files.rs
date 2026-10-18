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

/// Authorization supplied by the virtual tape operator/password service.
/// A true response confirms that the host has authorized this access.
#[derive(Debug, Clone)]
pub struct TapeAccessRequest {
    pub path: PathBuf,
    pub dataset: String,
    pub writing: bool,
    pub security: u8,
    pub unexpired: bool,
}

fn tape_today() -> (i64, u16, u16) {
    let days = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
        / 86400;
    let (year, _, _) = super::civil_from_days(days);
    (
        days,
        year as u16,
        (days - super::days_from_civil(year, 1, 1) + 1) as u16,
    )
}

impl FileState {
    pub(super) fn anchor_path(&mut self, base: &std::path::Path) {
        if let Some(path) = &mut self.path
            && path.is_relative()
        {
            *path = base.join(&*path);
        }
    }
    pub(super) fn open_mode(&self) -> Option<OpenMode> {
        self.open
    }
}

/// The records of an open file.
#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
enum Data {
    #[default]
    None,
    Seq(SequentialDataset),
    Rel(RelativeDataset),
    Keyed(#[serde(with = "keyed_checkpoint")] KeyedDataset),
}

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct FileState {
    #[serde(default)]
    physical_layout: Option<Layout>,
    #[serde(default)]
    prefix_reel_breaks: Vec<(usize, usize)>,
    #[serde(default)]
    reel_breaks: Vec<usize>,
    #[serde(default)]
    prefix_volume_events: Vec<VolumeEvent>,
    volume_events: Vec<VolumeEvent>,
    volume_done: usize,
    header_labels: Vec<Vec<u8>>,
    ending_labels_done: bool,
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

#[derive(Default, Clone, serde::Serialize, serde::Deserialize)]
struct VolumeEvent {
    after_records: usize,
    ending: Vec<Vec<u8>>,
    beginning: Vec<Vec<u8>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct VolumeDirectory {
    version: u32,
    continued: Vec<bool>,
}

#[derive(Default)]
struct Mount {
    volumes: Vec<Vec<Entry>>,
    continued: Vec<bool>,
}
impl Mount {
    fn logical(&self, standard: bool) -> zeus_ds::Result<Vec<Entry>> {
        if standard {
            tape::join_volumes(&self.volumes, true)
        } else {
            tape::join_unlabeled(&self.volumes, &self.continued)
        }
    }
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

mod keyed_checkpoint {
    use super::KeyedDataset;
    use serde::{Deserialize, Serialize};
    pub fn serialize<S: serde::Serializer>(data: &KeyedDataset, s: S) -> Result<S::Ok, S::Error> {
        data.checkpoint().serialize(s)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<KeyedDataset, D::Error> {
        KeyedDataset::restore(zeus_ds::keyed::Checkpoint::deserialize(d)?)
            .map_err(serde::de::Error::custom)
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
    /// Connect a host's operator/password service. Without one, accesses that
    /// require a password or permission to scratch retained data are rejected.
    pub fn set_tape_authorizer(
        &mut self,
        authorize: impl FnMut(&TapeAccessRequest) -> bool + 'static,
    ) {
        self.tape_authorizer = Some(Box::new(authorize));
    }

    fn authorize_tape_labels(
        &mut self,
        entries: &[Entry],
        mount: &Mount,
        path: &std::path::Path,
        position: usize,
        mode: OpenMode,
        def: &FileDef,
    ) -> RResult<()> {
        let attributes = self.dd.io_attributes(&[&def.assign, &def.name]);
        let expected_name = attributes.dataset_name.as_deref().unwrap_or(&def.assign);
        let new_security = attributes.tape_security;
        let writing = mode != OpenMode::Input;
        let (_, year, day) = tape_today();
        let target = if position < entries.len() && entries[position] != Entry::Mark {
            Some(tape::file_with_labels(entries, position, true).map_err(ds_err)?)
        } else {
            None
        };
        let end = if mode == OpenMode::Output {
            entries.len()
        } else {
            target.as_ref().map_or(position, |f| f.next)
        };
        let selected: Vec<_> = entries[position.min(entries.len())..end.min(entries.len())]
            .iter()
            .filter_map(|entry| match entry {
                Entry::Block(b) if b.starts_with(&ebcdic::encode("HDR1")) => Some(b),
                _ => None,
            })
            .collect();
        // Joining EOV sections removes continuation headers. Authorize the
        // selected dataset on every physical volume before returning any data.
        let keys: Vec<_> = selected
            .iter()
            .map(|b| (&b[4..21], tape::data_sequence(b)))
            .map(|(name, sequence)| sequence.map(|n| (name, n)))
            .collect::<zeus_ds::Result<_>>()
            .map_err(ds_err)?;
        for bytes in mount
            .volumes
            .iter()
            .flatten()
            .filter_map(|entry| match entry {
                Entry::Block(b) if b.starts_with(&ebcdic::encode("HDR1")) => Some(b),
                _ => None,
            })
        {
            let key = (&bytes[4..21], tape::data_sequence(bytes).map_err(ds_err)?);
            if !keys.contains(&key) {
                continue;
            }
            let header = ebcdic::decode_latin1(bytes);
            let security = match header[53] {
                b'0' | b' ' => 0,
                b'1' => 1,
                b'3' => 3,
                _ => return err("invalid tape security level"),
            };
            let password = security == 1 || security == 3 && writing;
            // Input ignores expiration. EXTEND does not scratch existing data.
            let unexpired = mode == OpenMode::Output
                && tape::Expiration::from_label(&header[47..53])
                    .map_err(ds_err)?
                    .is_some_and(|date| date.retained(year, day));
            let dataset = String::from_utf8_lossy(&header[4..21])
                .trim_end()
                .to_string();
            if password && writing && keys.first() == Some(&key) {
                let tail = &expected_name[expected_name.len().saturating_sub(17)..];
                if dataset != tail {
                    return err("protected tape dataset identifier mismatch");
                }
            }
            if password || unexpired {
                let request = TapeAccessRequest {
                    path: path.to_path_buf(),
                    dataset,
                    writing,
                    security,
                    unexpired,
                };
                if !self
                    .tape_authorizer
                    .as_mut()
                    .is_some_and(|authorize| authorize(&request))
                {
                    return err("tape operator/password authorization required");
                }
            }
        }
        // Appending a new file inherits the preceding file's protection level.
        if mode == OpenMode::Output
            && target.is_none()
            && let Some(previous) = entries[..position.min(entries.len())]
                .iter()
                .rev()
                .find_map(|entry| match entry {
                    Entry::Block(b) if b.starts_with(&ebcdic::encode("EOF1")) => Some(b),
                    _ => None,
                })
        {
            if previous[53] != ebcdic::encode(&new_security.to_string())[0] {
                return err("new tape dataset security differs from preceding dataset");
            }
            if new_security != 0 {
                let request = TapeAccessRequest {
                    path: path.to_path_buf(),
                    dataset: String::from_utf8_lossy(&ebcdic::decode_latin1(&previous[4..21]))
                        .trim_end()
                        .into(),
                    writing: true,
                    security: new_security,
                    unexpired: false,
                };
                if !self
                    .tape_authorizer
                    .as_mut()
                    .is_some_and(|authorize| authorize(&request))
                {
                    return err("preceding tape dataset requires password authorization");
                }
            }
        }
        Ok(())
    }
    fn user_labels(
        &mut self,
        cur: usize,
        file: u32,
        mode: OpenMode,
        beginning: bool,
        reel: bool,
        input: Option<&[Vec<u8>]>,
    ) -> RResult<Vec<Vec<u8>>> {
        let def = self.file_def(cur, file);
        if !def.label_standard || def.label_records.is_empty() {
            return Ok(vec![]);
        }
        let slot = self.instances[cur].files[file as usize];
        let mut instance = cur;
        let (owner, handler, record_owner, record) = loop {
            let code = self.instances[instance].code.clone();
            let local_file = self.instances[instance]
                .files
                .iter()
                .position(|s| *s == slot)
                .map(|n| n as u32);
            let mut handlers: Vec<_> = code
                .label_uses
                .iter()
                .filter(|u| {
                    (instance == cur || u.global)
                        && u.beginning.is_none_or(|b| b == beginning)
                        && u.reel.is_none_or(|r| r == reel)
                        && (local_file.is_some_and(|f| u.files.contains(&f))
                            || (u.files.is_empty() && u.mode == Some(mode)))
                })
                .collect();
            handlers.sort_by_key(|u| u.files.is_empty());
            if let Some(handler) = handlers.first() {
                let record = if let Some(f) = local_file {
                    code.files[f as usize].label_records.first().cloned()
                } else {
                    code.files
                        .iter()
                        .find_map(|file| file.label_records.first().cloned())
                };
                let record_owner = if record.is_some() { instance } else { cur };
                break (
                    instance,
                    (*handler).clone(),
                    record_owner,
                    record.unwrap_or_else(|| def.label_records[0].clone()),
                );
            }
            let Some(parent) = code.parent else {
                return Ok(vec![]);
            };
            let module = self.instances[instance].module.clone();
            instance = self.instantiate(&module, parent);
        };
        let mut labels = Vec::new();
        let mut index = 0;
        loop {
            if let Some(input) = input {
                let Some(label) = input.get(index) else {
                    break;
                };
                let (addr, len) = self.addr(record_owner, &record)?;
                if len != 80 {
                    return err("user label record must contain 80 bytes");
                }
                self.mem[addr..addr + len].copy_from_slice(label);
            }
            let start = self.instances[owner].code.paragraphs[handler.start as usize].start;
            let more = match self.run_procedure(owner, start, handler.end, &mut vec![]) {
                Ok(()) => false,
                Err(error) if error.0 == "ZEUS_MORE_LABELS" => true,
                Err(error) => return Err(error),
            };
            if input.is_none() {
                let (addr, len) = self.addr(record_owner, &record)?;
                let label = self.mem[addr..addr + len].to_vec();
                let prefix = ebcdic::encode(if beginning { "UHL" } else { "UTL" });
                if len != 80 || !label.starts_with(&prefix) {
                    return err("user tape label requires UHL/UTL identification and 80 bytes");
                }
                labels.push(label);
            }
            if !more {
                break;
            }
            index += 1;
        }
        Ok(labels)
    }

    fn input_user_labels(&self, cur: usize, file: u32, beginning: bool) -> Vec<Vec<u8>> {
        let state = &self.file_states[self.instances[cur].files[file as usize]];
        let id = ebcdic::encode(if beginning { "UHL" } else { "UTL" });
        let Ok(file) = tape::file_with_labels(&state.tape_entries, state.tape_start, true) else {
            return vec![];
        };
        let range = if beginning {
            state.tape_start..file.data_start
        } else {
            file.trailer_start..file.next
        };
        state.tape_entries[range]
            .iter()
            .filter_map(|e| match e {
                Entry::Block(b) if b.len() == 80 && b.starts_with(&id) => Some(b.clone()),
                _ => None,
            })
            .collect()
    }

    fn tape_volume_path(path: &std::path::Path, number: usize) -> PathBuf {
        if number == 0 {
            return path.to_path_buf();
        }
        path.with_file_name(format!(
            "{}.vol{:03}.aws",
            path.file_stem().unwrap_or_default().to_string_lossy(),
            number + 1
        ))
    }

    fn labels_in(entries: &[Entry], beginning: bool) -> Vec<Vec<u8>> {
        let prefix = ebcdic::encode(if beginning { "UHL" } else { "UTL" });
        entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Block(bytes) if bytes.len() == 80 && bytes.starts_with(&prefix) => {
                    Some(bytes.clone())
                }
                _ => None,
            })
            .collect()
    }

    fn input_volume_events(
        volumes: &[Vec<Entry>],
        logical: &[Entry],
        position: usize,
        layout: Layout,
    ) -> RResult<Vec<VolumeEvent>> {
        let target = tape::file_with_labels(logical, position, true).map_err(ds_err)?;
        let id = logical[position..target.data_start]
            .iter()
            .find_map(|e| match e {
                Entry::Block(b) if b.starts_with(&ebcdic::encode("HDR1")) => Some(b),
                _ => None,
            })
            .ok_or_else(|| super::RunError("missing logical tape header".into()))?;
        let (_, ranges) = layout.decode_with_blocks(&target.blocks).map_err(ds_err)?;
        let mut events: Vec<VolumeEvent> = vec![];
        let mut blocks = 0;
        for volume in volumes {
            let mut position = 0;
            while position < volume.len() && volume[position] != Entry::Mark {
                let file = tape::file_with_labels(volume, position, true).map_err(ds_err)?;
                let header = volume[position..file.data_start]
                    .iter()
                    .find_map(|e| match e {
                        Entry::Block(b) if b.starts_with(&ebcdic::encode("HDR1")) => Some(b),
                        _ => None,
                    })
                    .unwrap();
                if header[4..21] == id[4..21] && header[31..35] == id[31..35] {
                    if let Some(event) = events.last_mut() {
                        event.beginning = Self::labels_in(&volume[position..file.data_start], true);
                    }
                    blocks += file.blocks.len();
                    if file.continued {
                        events.push(VolumeEvent {
                            after_records: ranges.iter().filter(|(_, end)| *end < blocks).count(),
                            ending: Self::labels_in(&volume[file.trailer_start..file.next], false),
                            beginning: vec![],
                        });
                    }
                }
                position = file.next;
            }
        }
        Ok(events)
    }

    fn input_volume_boundary(&mut self, cur: usize, file: u32) -> RResult<()> {
        loop {
            let st = self.state(cur, file);
            let Some(event) = st.volume_events.get(st.volume_done).cloned() else {
                return Ok(());
            };
            if event.after_records > st.pos as usize {
                return Ok(());
            }
            st.volume_done += 1;
            let mode = st.open.unwrap();
            self.user_labels(cur, file, mode, false, true, Some(&event.ending))?;
            self.user_labels(cur, file, mode, true, true, Some(&event.beginning))?;
            self.rerun_volume(cur, file)?;
        }
    }

    fn input_nl_events(
        mounted: &Mount,
        logical: &[Entry],
        target_position: usize,
        layout: Layout,
    ) -> RResult<Vec<VolumeEvent>> {
        let target = tape::file_with_labels(logical, target_position, false).map_err(ds_err)?;
        let (_, ranges) = layout.decode_with_blocks(&target.blocks).map_err(ds_err)?;
        let mut target_number = 0;
        let mut position = 0;
        while position < target_position {
            position = tape::file_with_labels(logical, position, false)
                .map_err(ds_err)?
                .next;
            target_number += 1;
        }
        let mut number = 0;
        let mut blocks = 0;
        let mut events = vec![];
        for (volume, continuation) in mounted.volumes.iter().zip(&mounted.continued) {
            let mut position = 0;
            loop {
                let file = tape::file_with_labels(volume, position, false).map_err(ds_err)?;
                let last = volume.get(file.next).is_none_or(|e| *e == Entry::Mark);
                if number == target_number {
                    blocks += file.blocks.len();
                    if last && *continuation {
                        events.push(VolumeEvent {
                            after_records: ranges.iter().filter(|(_, end)| *end < blocks).count(),
                            ending: vec![],
                            beginning: vec![],
                        });
                    }
                }
                if !last || !continuation {
                    number += 1;
                }
                if last {
                    break;
                }
                position = file.next;
            }
        }
        Ok(events)
    }

    fn output_volume_boundary(&mut self, cur: usize, file: u32, def: &FileDef) -> RResult<()> {
        let path = self.file_path(def);
        let attributes = self.dd.io_attributes(&[&def.assign, &def.name]);
        let capacity = attributes.volume_capacity.unwrap_or(usize::MAX);
        if !self.is_tape(def, &path) {
            return Ok(());
        }
        if attributes.volume_capacity.is_none() && self.state(cur, file).reel_breaks.is_empty() {
            return Ok(());
        }
        let Some(layout) = self
            .state(cur, file)
            .physical_layout
            .or_else(|| self.physical_layout(def, &path))
        else {
            return Ok(());
        };
        let st = self.state(cur, file);
        let prefix = &st.tape_entries[..st.tape_start.min(st.tape_entries.len())];
        let (mut base, mut used) =
            Self::tape_usage(prefix, def.label_standard, capacity, &st.prefix_reel_breaks)?;
        let reel_breaks = st.reel_breaks.clone();
        let records = self.state(cur, file).seq().records.clone();
        let (blocks, breaks) =
            Self::encode_tape_records(layout, &records, ebcdic::SPACE, &reel_breaks)
                .map_err(ds_err)?;
        if blocks.first().is_some_and(|b| used + b.len() > capacity) && used != 0 {
            base += 1;
            used = 0;
        }
        self.state(cur, file)
            .prefix_volume_events
            .resize_with(base, VolumeEvent::default);
        let mut boundaries = 0;
        for index in 0..=blocks.len() {
            let block = blocks.get(index);
            if block.is_some_and(|b| b.len() > capacity) {
                return err("tape block exceeds volume capacity");
            }
            let forced = breaks.iter().filter(|&&b| b == index).count();
            let switches = forced.max(usize::from(
                block.is_some_and(|b| used + b.len() > capacity),
            ));
            if switches > 0 {
                boundaries += switches;
                used = 0;
            }
            used += block.map_or(0, Vec::len);
        }
        while self.state(cur, file).volume_done < boundaries {
            let mode = self.state(cur, file).open.unwrap();
            let ending = self.user_labels(cur, file, mode, false, true, None)?;
            let beginning = self.user_labels(cur, file, mode, true, true, None)?;
            let st = self.state(cur, file);
            st.volume_done += 1;
            st.volume_events.push(VolumeEvent {
                after_records: records.len(),
                ending,
                beginning,
            });
            self.rerun_volume(cur, file)?;
        }
        Ok(())
    }

    fn tape_usage(
        entries: &[Entry],
        standard: bool,
        capacity: usize,
        breaks: &[(usize, usize)],
    ) -> RResult<(usize, usize)> {
        if capacity == 0 {
            return err("zero tape volume capacity");
        }
        if entries.is_empty() {
            return Ok((0, 0));
        }
        let volumes = if standard {
            tape::split_volumes_at(entries, capacity, breaks).map_err(ds_err)?
        } else {
            tape::split_unlabeled_at(entries, capacity, breaks)
                .map_err(ds_err)?
                .0
        };
        let last = volumes.last().unwrap();
        let mut position = 0;
        let mut used = 0;
        while position < last.len() && last[position] != Entry::Mark {
            let file = tape::file_with_labels(last, position, standard).map_err(ds_err)?;
            for block in file.blocks {
                used += block.len();
            }
            position = file.next;
        }
        Ok((volumes.len() - 1, used))
    }

    fn encode_tape_records(
        layout: Layout,
        records: &[Vec<u8>],
        pad: u8,
        breaks: &[usize],
    ) -> zeus_ds::Result<(Vec<Vec<u8>>, Vec<usize>)> {
        let mut blocks = vec![];
        let mut boundaries = vec![];
        let mut start = 0;
        for &end in breaks {
            let section = records
                .get(start..end)
                .ok_or_else(|| DsError::Damaged("invalid tape reel boundary".into()))?;
            blocks.extend(layout.encode(section, pad)?);
            boundaries.push(blocks.len());
            start = end;
        }
        blocks.extend(layout.encode(&records[start..], pad)?);
        Ok((blocks, boundaries))
    }

    fn mounted_reel_breaks(mount: &Mount, standard: bool) -> RResult<Vec<(usize, usize)>> {
        let mut breaks = vec![];
        let mut number = 0;
        let mut blocks = 0;
        for (index, volume) in mount.volumes.iter().enumerate() {
            let mut position = 0;
            loop {
                let file = tape::file_with_labels(volume, position, standard).map_err(ds_err)?;
                blocks += file.blocks.len();
                let last = volume.get(file.next).is_none_or(|e| *e == Entry::Mark);
                let continued = last && mount.continued[index];
                if continued {
                    breaks.push((number, blocks));
                } else {
                    number += 1;
                    blocks = 0;
                }
                if last {
                    break;
                }
                position = file.next;
            }
        }
        Ok(breaks)
    }

    fn mounted_volume_events(mount: &Mount, standard: bool) -> RResult<Vec<VolumeEvent>> {
        let mut events = vec![];
        for pair in mount.volumes.windows(2) {
            let mut event = VolumeEvent::default();
            if standard && tape::continues(&pair[0], true).map_err(ds_err)? {
                let mut position = 0;
                while position < pair[0].len() && pair[0][position] != Entry::Mark {
                    let file = tape::file_with_labels(&pair[0], position, true).map_err(ds_err)?;
                    if file.continued {
                        event.ending =
                            Self::labels_in(&pair[0][file.trailer_start..file.next], false);
                    }
                    position = file.next;
                }
                let file = tape::file_with_labels(&pair[1], 0, true).map_err(ds_err)?;
                event.beginning = Self::labels_in(&pair[1][..file.data_start], true);
            }
            events.push(event);
        }
        Ok(events)
    }

    fn mount_volumes(
        &self,
        def: &FileDef,
        path: &std::path::Path,
        buffers: &mut BufferPool,
    ) -> RResult<Option<Mount>> {
        let attributes = self.dd.io_attributes(&[&def.assign, &def.name]);
        let first = attributes
            .volumes
            .first()
            .cloned()
            .unwrap_or_else(|| path.to_path_buf());
        let Some(bytes) = buffers.read(&first).map_err(ds_err)? else {
            return Ok(None);
        };
        let mut volumes = vec![tape::decode(&bytes).map_err(ds_err)?];
        let directory = buffers
            .read(&path.with_extension("aws.volumes.json"))
            .map_err(ds_err)?
            .map(|bytes| {
                serde_json::from_slice::<VolumeDirectory>(&bytes)
                    .map_err(|e| super::RunError(e.to_string()))
            })
            .transpose()?;
        if directory.as_ref().is_some_and(|d| {
            d.version != 1
                || d.continued.is_empty()
                || d.continued.len() > 10000
                || d.continued.last() == Some(&true)
        }) {
            return err("invalid tape volume directory");
        }
        let mut continued = vec![];
        loop {
            let physical_continuation = def.label_standard
                && tape::continues(volumes.last().unwrap(), true).map_err(ds_err)?;
            let needs_next = if let Some(directory) = &directory {
                volumes.len() < directory.continued.len()
            } else if def.label_standard {
                physical_continuation || volumes.len() < attributes.volumes.len()
            } else {
                volumes.len()
                    < directory
                        .as_ref()
                        .map_or(attributes.volumes.len().max(1), |d| d.continued.len())
            };
            continued.push(directory.as_ref().map_or(
                if def.label_standard {
                    physical_continuation
                } else {
                    needs_next
                },
                |d| d.continued[volumes.len() - 1],
            ));
            if def.label_standard && continued.last() != Some(&physical_continuation) {
                return err("tape volume directory disagrees with EOV labels");
            }
            if !needs_next {
                break;
            }
            let number = volumes.len();
            let next = if attributes.volumes.is_empty() {
                Self::tape_volume_path(path, number)
            } else {
                attributes.volumes.get(number).cloned().ok_or_else(|| {
                    super::RunError("missing tape continuation volume allocation".into())
                })?
            };
            let bytes = buffers.read(&next).map_err(ds_err)?.ok_or_else(|| {
                super::RunError(format!(
                    "missing tape continuation volume {}",
                    next.display()
                ))
            })?;
            volumes.push(tape::decode(&bytes).map_err(ds_err)?);
        }
        Ok(Some(Mount { volumes, continued }))
    }

    fn mount_tape(
        &self,
        def: &FileDef,
        path: &std::path::Path,
        buffers: &mut BufferPool,
    ) -> RResult<Option<Vec<Entry>>> {
        self.mount_volumes(def, path, buffers)?
            .map(|mounted| mounted.logical(def.label_standard).map_err(ds_err))
            .transpose()
    }

    fn save_tape(
        &self,
        def: &FileDef,
        path: &std::path::Path,
        entries: &[Entry],
        events: &[VolumeEvent],
        breaks: &[(usize, usize)],
        buffers: &mut BufferPool,
    ) -> zeus_ds::Result<()> {
        let attributes = self.dd.io_attributes(&[&def.assign, &def.name]);
        let (mut volumes, continued) = if !def.label_standard {
            tape::split_unlabeled_at(
                entries,
                attributes.volume_capacity.unwrap_or(usize::MAX),
                breaks,
            )?
        } else {
            let volumes = tape::split_volumes_at(
                entries,
                attributes.volume_capacity.unwrap_or(usize::MAX),
                breaks,
            )?;
            let continued = volumes
                .iter()
                .map(|v| tape::continues(v, true))
                .collect::<zeus_ds::Result<Vec<_>>>()?;
            (volumes, continued)
        };
        if !events.is_empty() && events.len() + 1 != volumes.len() {
            return Err(DsError::Damaged(
                "tape volume event count differs from physical volumes".into(),
            ));
        }
        for (number, volume) in volumes.iter_mut().enumerate() {
            if !def.label_standard {
                continue;
            }
            if let Some(event) = events.get(number) {
                let mut position = 0;
                let mut last = None;
                while position < volume.len() && volume[position] != Entry::Mark {
                    let file = tape::file_with_labels(volume, position, def.label_standard)?;
                    position = file.next;
                    last = Some(file);
                }
                if let Some(file) = last {
                    volume.splice(
                        file.next - 1..file.next - 1,
                        event.ending.iter().cloned().map(Entry::Block),
                    );
                }
            }
            if number > 0
                && let Some(event) = events.get(number - 1)
            {
                let file = tape::file_with_labels(volume, 0, def.label_standard)?;
                volume.splice(
                    file.data_start - 1..file.data_start - 1,
                    event.beginning.iter().cloned().map(Entry::Block),
                );
            }
        }
        if !attributes.volumes.is_empty() && volumes.len() > attributes.volumes.len() {
            return Err(DsError::Damaged(
                "insufficient tape volume allocations".into(),
            ));
        }
        // Validate every image before writing the first physical volume.
        let mut images = Vec::new();
        let mut first_serials = std::collections::HashMap::<Vec<u8>, Vec<u8>>::new();
        for (number, volume) in volumes.iter_mut().enumerate() {
            let serial = attributes
                .volume_serials
                .get(number)
                .cloned()
                .unwrap_or_else(|| format!("ZEUS{:02}", number + 1));
            if serial.is_empty()
                || serial.len() > 6
                || !serial
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
            {
                return Err(DsError::Damaged("invalid tape volume serial".into()));
            }
            if def.label_standard
                && let Some(Entry::Block(label)) = volume.first_mut()
                && label.starts_with(&ebcdic::encode("VOL1"))
            {
                label[4..10].fill(ebcdic::SPACE);
                label[4..4 + serial.len()].copy_from_slice(&ebcdic::encode(&serial));
            }
            if def.label_standard {
                let mut serial = ebcdic::encode(&serial);
                serial.resize(6, ebcdic::SPACE);
                let mut position = 0;
                while position < volume.len() && volume[position] != Entry::Mark {
                    let file = tape::file_with_labels(volume, position, true)?;
                    let header = (position..file.data_start).find(|&i|matches!(&volume[i],Entry::Block(b) if b.starts_with(&ebcdic::encode("HDR1")))).unwrap();
                    let Entry::Block(label) = &volume[header] else {
                        unreachable!()
                    };
                    let mut key = label[4..21].to_vec();
                    key.extend_from_slice(&tape::data_sequence(label)?.to_le_bytes());
                    let first = first_serials.entry(key).or_insert_with(|| serial.clone());
                    for index in [header, file.trailer_start] {
                        let Entry::Block(label) = &mut volume[index] else {
                            unreachable!()
                        };
                        label[21..27].copy_from_slice(first);
                    }
                    position = file.next;
                    if file.continued {
                        break;
                    }
                }
            }
            images.push(tape::encode(volume)?);
        }
        for (number, image) in images.iter().enumerate() {
            let destination = attributes
                .volumes
                .get(number)
                .cloned()
                .unwrap_or_else(|| Self::tape_volume_path(path, number));
            buffers.write(&destination, image)?;
        }
        {
            let bytes = serde_json::to_vec(&VolumeDirectory {
                version: 1,
                continued,
            })
            .map_err(|e| DsError::Io(e.to_string()))?;
            buffers.write(&path.with_extension("aws.volumes.json"), &bytes)?;
        }
        Ok(())
    }

    fn tape_label(
        &self,
        def: &FileDef,
        layout: Layout,
        id: &str,
        number: u8,
        blocks: usize,
        sequence: usize,
    ) -> RResult<Vec<u8>> {
        let mut label = vec![b' '; 80];
        label[..3].copy_from_slice(id.as_bytes());
        label[3] = b'0' + number;
        if number == 1 {
            let attributes = self.dd.io_attributes(&[&def.assign, &def.name]);
            let name = attributes
                .dataset_name
                .as_deref()
                .unwrap_or(&def.assign)
                .as_bytes();
            let name = &name[name.len().saturating_sub(17)..];
            label[4..4 + name.len()].copy_from_slice(name);
            label[21..27].copy_from_slice(b"ZEUS01");
            label[27..35].copy_from_slice(b"00010001");
            if name.len() >= 9 && name[name.len() - 9] == b'.' {
                let suffix = &name[name.len() - 8..];
                if suffix[0] == b'G'
                    && suffix[5] == b'V'
                    && suffix[1..5]
                        .iter()
                        .chain(&suffix[6..8])
                        .all(u8::is_ascii_digit)
                {
                    label[35..39].copy_from_slice(&suffix[1..5]);
                    label[39..41].copy_from_slice(&suffix[6..8]);
                }
            }
            if sequence <= 9999 {
                label[31..35].copy_from_slice(format!("{sequence:04}").as_bytes());
            }
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
            let expiration = if let Some(days) = attributes.retention_days {
                let end =
                    (tape_today().0 + i64::from(days)).min(super::days_from_civil(2156, 1, 1) - 1);
                let (year, _, _) = super::civil_from_days(end);
                Some(tape::Expiration::Date {
                    year: year as u16,
                    day: (end - super::days_from_civil(year, 1, 1) + 1) as u16,
                })
            } else {
                attributes.expiration
            };
            label[47..53].copy_from_slice(
                &expiration
                    .map(tape::Expiration::label)
                    .transpose()
                    .map_err(ds_err)?
                    .unwrap_or(*b" 00000"),
            );
            label[53] = b'0' + attributes.tape_security;
            label[54..60].copy_from_slice(format!("{:06}", blocks % 1_000_000).as_bytes());
            if blocks >= 1_000_000 {
                let high = (blocks / 1_000_000).to_string();
                label[80 - high.len()..].copy_from_slice(high.as_bytes());
            }
            label[60..64].copy_from_slice(b"ZEUS");
        } else {
            label[4] = if layout.mode == 'S' {
                b'V'
            } else {
                layout.mode as u8
            };
            let short_size = if layout.block_size > 32760 {
                0
            } else {
                layout.block_size
            };
            label[5..10].copy_from_slice(format!("{short_size:05}").as_bytes());
            if layout.block_size > 32760 {
                label[70..80].copy_from_slice(format!("{:010}", layout.block_size).as_bytes());
            }
            if layout.mode != 'U'
                && layout.record_len
                    + (if matches!(layout.mode, 'V' | 'S') {
                        4
                    } else {
                        0
                    })
                    > 99999
            {
                return err("record length cannot be represented in a standard tape label");
            }
            label[10..15].copy_from_slice(
                format!(
                    "{:05}",
                    if layout.mode == 'U' {
                        0
                    } else {
                        layout.record_len
                    } + if matches!(layout.mode, 'V' | 'S') {
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
        let mut label = ebcdic::from_host(&label);
        if number == 1 {
            tape::write_data_sequence(&mut label, sequence).map_err(ds_err)?;
        }
        Ok(label)
    }

    fn tape_headers(&self, def: &FileDef, layout: Layout, entries: &mut Vec<Entry>) -> RResult<()> {
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
            self.tape_label(def, layout, "HDR", 1, 0, sequence)?,
        ));
        entries.push(Entry::Block(
            self.tape_label(def, layout, "HDR", 2, 0, sequence)?,
        ));
        entries.push(Entry::Mark);
        Ok(())
    }

    fn tape_trailers(
        &self,
        def: &FileDef,
        layout: Layout,
        count: usize,
        entries: &mut Vec<Entry>,
    ) -> RResult<()> {
        let sequence = 1 + entries
            .iter()
            .filter(
                |entry| matches!(entry, Entry::Block(b) if b.starts_with(&ebcdic::encode("EOF1"))),
            )
            .count();
        entries.push(Entry::Block(
            self.tape_label(def, layout, "EOF", 1, count, sequence)?,
        ));
        entries.push(Entry::Block(
            self.tape_label(def, layout, "EOF", 2, count, sequence)?,
        ));
        entries.push(Entry::Mark);
        Ok(())
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
        // The virtual tape's system-selected device block is at most 256 KiB.
        // Explicit sizes and sizes read from an existing label take precedence.
        let limit = if tape && mode != 'S' {
            dd.block_size_limit
                .filter(|&n| n != 0)
                .unwrap_or(32760)
                .min(256 * 1024)
        } else {
            32760
        };
        let automatic = match mode {
            'F' => (limit / def.max_len.max(1)).max(1) * def.max_len,
            'U' => def.max_len,
            _ => limit,
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

    fn tape_input_layout(
        &self,
        def: &FileDef,
        mut layout: Layout,
        entries: &[Entry],
        position: usize,
    ) -> RResult<Layout> {
        if !def.label_standard {
            return Ok(layout);
        }
        let file = tape::file_with_labels(entries, position, true).map_err(ds_err)?;
        let header = entries[position..file.data_start]
            .iter()
            .find_map(|e| match e {
                Entry::Block(b) if b.starts_with(&ebcdic::encode("HDR2")) => {
                    Some(ebcdic::decode_latin1(b))
                }
                _ => None,
            });
        let Some(header) = header else {
            // VSE tapes may have only label 1. The FD/DD supplies the record
            // format; physical AWS boundaries supply an omitted block size.
            let attributes = self.dd.io_attributes(&[&def.assign, &def.name]);
            if attributes
                .block_size
                .filter(|&n| n != 0)
                .or(def.block_size.filter(|&n| n != 0))
                .is_none()
            {
                layout.block_size = file
                    .blocks
                    .iter()
                    .map(Vec::len)
                    .max()
                    .unwrap_or(layout.block_size)
                    .max(
                        layout.record_len
                            + if matches!(layout.mode, 'V' | 'S') {
                                8
                            } else {
                                0
                            },
                    );
            }
            layout.validate().map_err(ds_err)?;
            return Ok(layout);
        };
        let number = |range: std::ops::Range<usize>| -> RResult<usize> {
            std::str::from_utf8(&header[range])
                .ok()
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| super::RunError("invalid HDR2 length".into()))
        };
        let record = number(10..15)?;
        let mut block = number(5..10)?;
        if block == 0 {
            block = number(70..80)?;
        }
        let attributes = self.dd.io_attributes(&[&def.assign, &def.name]);
        let expected_record = if layout.mode == 'U' {
            0
        } else {
            layout.record_len
                + if matches!(layout.mode, 'V' | 'S') {
                    4
                } else {
                    0
                }
        };
        let attribute_ok = match layout.mode {
            'S' => matches!(header[38], b'S' | b'R'),
            'V' => matches!(header[38], b' ' | b'B'),
            'F' => matches!(header[38], b' ' | b'B' | b'S' | b'R'),
            'U' => header[38] == b' ',
            _ => false,
        };
        let explicit_block = attributes
            .block_size
            .filter(|&n| n != 0)
            .or(def.block_size.filter(|&n| n != 0));
        if record != expected_record
            || !attribute_ok
            || header[4]
                != if layout.mode == 'S' {
                    b'V'
                } else {
                    layout.mode as u8
                }
            || explicit_block.is_some_and(|n| n as usize != block)
        {
            return err("FD/DD/HDR2 attributes mismatch");
        }
        layout.block_size = block;
        layout.validate().map_err(ds_err)?;
        Ok(layout)
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
        if let Err(error) = self.open_file_options(cur, f, mode, reversed) {
            if let Some(position) = old {
                self.tape_positions.insert(path, position);
            } else {
                self.tape_positions.remove(&path);
            }
            return Err(error);
        }
        if self.state(cur, f).open.is_none() {
            if let Some(pos) = old {
                self.tape_positions.insert(path, pos);
            }
            return Ok(());
        }
        if tape && reversed {
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
        layout_override: Option<Layout>,
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
                if let Some(mut layout) =
                    layout_override.or_else(|| self.physical_layout(def, path))
                {
                    let blocks = if self.is_tape(def, path) {
                        let Some(entries) = self.mount_tape(def, path, buffers)? else {
                            return Ok(None);
                        };
                        let pos = self.tape_positions.get(path).copied().unwrap_or(0);
                        layout = self.tape_input_layout(def, layout, &entries, pos)?;
                        tape::file_with_labels(&entries, pos, def.label_standard)
                            .map_err(ds_err)?
                            .blocks
                    } else {
                        let Some(bytes) = buffers.read(path).map_err(ds_err)? else {
                            return Ok(None);
                        };
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
        self.open_file_options(cur, f, mode, false)
    }

    fn open_file_options(
        &mut self,
        cur: usize,
        f: u32,
        mode: OpenMode,
        reversed: bool,
    ) -> RResult<()> {
        self.last_open_mode = Some(mode);
        let def = self.file_def(cur, f);
        if def.external {
            fn signature(code: &crate::zir::ProgramCode, def: &FileDef) -> serde_json::Value {
                let record = &code.descs[def.record as usize];
                let key = |loc: &Loc, duplicates: bool| {
                    let d = &code.descs[loc.desc as usize];
                    (d.offset as i64 - record.offset as i64, d.len, duplicates)
                };
                let keys: Vec<_> = def
                    .record_key
                    .iter()
                    .map(|k| key(k, false))
                    .chain(def.alternate_keys.iter().map(|(k, dup)| key(k, *dup)))
                    .collect();
                let relative = def.relative_key.as_ref().map(|loc| {
                    let d = &code.descs[loc.desc as usize];
                    let name = match d.base {
                        crate::zir::Base::External(k) => code.externals[k as usize].name.clone(),
                        _ => String::new(),
                    };
                    (name, d.offset, d.len)
                });
                let assignment = zeus_ds::dd::assignment_ddname(&def.assign)
                    .unwrap_or_else(|| def.assign.clone());
                serde_json::json!({"assign": assignment, "org": def.org, "access": def.access,
                    "optional": def.optional, "max": def.max_len, "mode": def.recording_mode,
                    "standard": def.label_standard, "reserve": def.reserve,
                    "keys": keys, "relative": relative})
            }
            let expected = signature(&self.instances[cur].code, &def);
            if self
                .instances
                .iter()
                .filter(|i| !i.retired)
                .flat_map(|i| i.code.files.iter().map(move |f| (&i.code, f)))
                .any(|(code, other)| {
                    other.external && other.name == def.name && signature(code, other) != expected
                })
            {
                return err(format!("EXTERNAL file {} declarations disagree", def.name));
            }
        }
        let slot = self.instances[cur].files[f as usize];
        if self
            .active_sort_areas
            .iter()
            .any(|area| area.contains(&slot))
        {
            return err(format!(
                "SAME SORT AREA: {} must remain closed during SORT/MERGE",
                def.name
            ));
        }
        if def.external
            && self
                .instances
                .iter()
                .filter(|i| !i.retired)
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
        let before = self.dd.clone();
        let previous_path = before.resolve(&[&def.assign, &def.name]);
        let active = previous_path.as_ref().is_some_and(|path| {
            self.file_states
                .iter()
                .any(|s| s.open.is_some() && s.path.as_ref() == Some(path))
        });
        let model = (def.org == Org::Sequential).then_some((seq_format(&def), def.max_len));
        if let Err(error) = self
            .dd
            .prepare_dynamic(&[&def.assign, &def.name], model, active)
        {
            if active {
                self.dd = before;
            }
            return self.io_error(cur, &def, "98", &format!("OPEN {error}"));
        }
        if active && previous_path != self.dd.resolve(&[&def.assign, &def.name]) {
            self.dd = before;
            return self.io_error(cur, &def, "98", "OPEN cannot reallocate an active dataset");
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
        let attributes = self.dd.io_attributes(&[&def.assign, &def.name]);
        let media_mode =
            if mode == OpenMode::Output && attributes.append && def.org == Org::Sequential {
                OpenMode::Extend
            } else {
                mode
            };
        if !matches!(attributes.tape_security, 0 | 1 | 3)
            || attributes.retention_days.is_some() && attributes.expiration.is_some()
        {
            return self.io_error(
                cur,
                &def,
                "39",
                "OPEN invalid tape retention/security attributes",
            );
        }
        if self.state(cur, f).removed {
            return self.io_error(cur, &def, "30", "OPEN (tape removed)");
        }
        if attributes
            .standard_labels
            .is_some_and(|standard| standard != def.label_standard)
        {
            return self.io_error(cur, &def, "39", "OPEN label attributes mismatch");
        }
        if def.org == Org::Sequential
            && !is_text(&def)
            && let Some(format) = self
                .dd
                .allocation(&[&def.assign, &def.name])
                .and_then(|a| a.format)
        {
            let mismatch = match format {
                RecordFormat::Fixed(length) => {
                    !fixed_length(&def)
                        || length != def.max_len
                        || matches!(def.recording_mode, Some('V' | 'S' | 'U'))
                }
                RecordFormat::Variable => {
                    def.recording_mode == Some('F')
                        || (def.recording_mode.is_none() && fixed_length(&def))
                }
                RecordFormat::Text => true,
            };
            if mismatch {
                return self.io_error(cur, &def, "39", "OPEN FD/DD record attributes mismatch");
            }
        }
        let mut layout = self.physical_layout(&def, &path);
        if let Some(layout) = layout {
            if layout.block_size > 32760 && !self.is_tape(&def, &path) {
                return self.io_error(cur, &def, "90", "OPEN large block size requires tape LBI");
            }
            if layout.block_size > 32760 && mode == OpenMode::InputOutput {
                return self.io_error(cur, &def, "37", "OPEN I-O cannot use tape LBI");
            }
            if layout.validate().is_err() {
                return self.io_error(cur, &def, "39", "OPEN invalid block attributes");
            }
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
            let mut volumes = match self.mount_volumes(&def, &path, &mut st.buffers) {
                Ok(entries) => entries.unwrap_or_default(),
                Err(_) => {
                    return self.io_error(cur, &def, "39", "OPEN invalid tape labels/volume set");
                }
            };
            // JCL allocates a new data set as an empty host file. It has
            // no tape marks/labels until the first OPEN OUTPUT writes it.
            // Do not relax validation of INPUT or of continuation volumes.
            if media_mode == OpenMode::Output
                && volumes.volumes.len() == 1
                && volumes.volumes[0].is_empty()
                && attributes.file_sequence.is_none_or(|n| n <= 1)
                && self.tape_positions.get(&path).copied().unwrap_or(0) == 0
            {
                volumes = Mount::default();
            } else if volumes.volumes.iter().any(Vec::is_empty) {
                return self.io_error(cur, &def, "39", "OPEN unformatted tape volume");
            }
            if def.label_standard {
                for (volume, serial) in volumes.volumes.iter().zip(&attributes.volume_serials) {
                    let mut expected = ebcdic::encode(serial);
                    if expected.is_empty() || expected.len() > 6 {
                        return self.io_error(
                            cur,
                            &def,
                            "39",
                            "OPEN invalid volume serial allocation",
                        );
                    }
                    expected.resize(6, ebcdic::SPACE);
                    if !matches!(volume.first(),Some(Entry::Block(label)) if label.starts_with(&ebcdic::encode("VOL1")) && label[4..10] == expected)
                    {
                        return self.io_error(cur, &def, "39", "OPEN volume serial mismatch");
                    }
                }
            }
            let mut warnings = Vec::new();
            let logical = if def.label_standard {
                tape::join_volumes_with_warnings(&volumes.volumes, true, |message| {
                    warnings.push(message.to_string())
                })
            } else {
                volumes.logical(false)
            };
            st.tape_entries = match logical {
                Ok(entries) => entries,
                Err(_) => return self.io_error(cur, &def, "39", "OPEN invalid tape volume labels"),
            };
            for message in warnings {
                writeln!(self.stdout, "{message}").map_err(|e| super::RunError(e.to_string()))?;
            }
            let position = if let Some(number) = attributes.file_sequence {
                if !(1..=65535).contains(&number) {
                    return self.io_error(cur, &def, "39", "OPEN invalid tape sequence number");
                }
                tape::sequence_with_labels(&st.tape_entries, number, def.label_standard)
                    .map_err(ds_err)?
            } else {
                self.tape_positions.get(&path).copied().unwrap_or(0)
            };
            st.tape_start = position;
            if def.label_standard {
                if let Err(error) = self.authorize_tape_labels(
                    &st.tape_entries,
                    &volumes,
                    &path,
                    position,
                    media_mode,
                    &def,
                ) {
                    return self.io_error(cur, &def, "37", &format!("OPEN {}", error.0));
                }
            }
            if media_mode != OpenMode::Output
                && position < st.tape_entries.len()
                && let Some(initial) = layout
            {
                layout = match self.tape_input_layout(&def, initial, &st.tape_entries, position) {
                    Ok(merged) => Some(merged),
                    Err(_) => {
                        return self.io_error(
                            cur,
                            &def,
                            "39",
                            "OPEN FD/DD/HDR2 attributes mismatch",
                        );
                    }
                };
                if layout.unwrap().block_size > 32760 && mode == OpenMode::InputOutput {
                    return self.io_error(cur, &def, "37", "OPEN I-O cannot use tape LBI");
                }
                st.buffers = BufferPool::new(count, layout.unwrap().block_size).map_err(ds_err)?;
            }
            if matches!(media_mode, OpenMode::Output | OpenMode::Extend) {
                st.prefix_volume_events =
                    Self::mounted_volume_events(&volumes, def.label_standard)?;
                let mut prefix_files = 0;
                let mut prefix_position = 0;
                while prefix_position < position.min(st.tape_entries.len()) {
                    prefix_position = tape::file_with_labels(
                        &st.tape_entries,
                        prefix_position,
                        def.label_standard,
                    )
                    .map_err(ds_err)?
                    .next;
                    prefix_files += 1;
                }
                st.prefix_reel_breaks = Self::mounted_reel_breaks(&volumes, def.label_standard)?
                    .into_iter()
                    .filter(|(f, _)| *f < prefix_files)
                    .collect();
                let base = {
                    Self::tape_usage(
                        &st.tape_entries[..position.min(st.tape_entries.len())],
                        def.label_standard,
                        attributes.volume_capacity.unwrap_or(usize::MAX),
                        &st.prefix_reel_breaks,
                    )?
                    .0
                };
                st.prefix_volume_events
                    .resize_with(base, VolumeEvent::default);
            }
            if mode == OpenMode::Input
                && !def.label_standard
                && volumes.volumes.len() > 1
                && let Some(layout) = layout
            {
                st.volume_events =
                    Self::input_nl_events(&volumes, &st.tape_entries, position, layout)?;
            }
            if mode == OpenMode::Input
                && def.label_standard
                && volumes.volumes.len() > 1
                && let Some(layout) = layout
            {
                st.volume_events = Self::input_volume_events(
                    &volumes.volumes,
                    &st.tape_entries,
                    position,
                    layout,
                )?;
            }
            if reversed {
                if layout.is_none_or(|l| l.mode != 'F') {
                    return err("OPEN REVERSED requires fixed-format tape records");
                }
                if !st.volume_events.is_empty() {
                    return err("OPEN REVERSED requires a single-reel tape file");
                }
            }
            if media_mode != OpenMode::Output
                && position < st.tape_entries.len()
                && def.label_standard
            {
                let Ok(file) = tape::file_with_labels(&st.tape_entries, position, true) else {
                    return self.io_error(cur, &def, "39", "OPEN tape label mismatch");
                };
                if let Some(name) = &attributes.dataset_name {
                    let mut expected = ebcdic::encode(name);
                    expected = expected[expected.len().saturating_sub(17)..].to_vec();
                    expected.resize(17, ebcdic::SPACE);
                    let header = st.tape_entries[position..file.data_start]
                        .iter()
                        .find_map(|entry| match entry {
                            Entry::Block(b) if b.starts_with(&ebcdic::encode("HDR1")) => Some(b),
                            _ => None,
                        })
                        .unwrap();
                    if header[4..21] != expected {
                        return self.io_error(
                            cur,
                            &def,
                            "39",
                            "OPEN tape dataset identifier mismatch",
                        );
                    }
                }
            }
            st.tape_next = if position < st.tape_entries.len() && media_mode != OpenMode::Output {
                tape::file_with_labels(&st.tape_entries, position, def.label_standard)
                    .map_err(ds_err)?
                    .next
            } else {
                position
            };
            self.tape_positions.insert(path.clone(), position);
        }
        if media_mode == OpenMode::Output {
            st.modified = true;
            st.data = self.empty_data(cur, &def)?;
        } else {
            let data = if unallocated {
                None
            } else {
                self.load_data(cur, &def, &path, &mut st.buffers, layout)?
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
        if def.print && media_mode == OpenMode::Extend {
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
        st.physical_layout = layout;
        *self.state(cur, f) = st;
        if self.is_tape(&def, &path) && def.label_standard {
            if matches!(mode, OpenMode::Output | OpenMode::Extend) {
                let labels = self.user_labels(cur, f, mode, true, false, None)?;
                self.state(cur, f).header_labels = labels;
            } else {
                let labels = self.input_user_labels(cur, f, true);
                self.user_labels(cur, f, mode, true, false, Some(&labels))?;
            }
        }
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

    pub(super) fn sort_area_peers(&self, cur: usize, file: u32) -> RResult<Vec<usize>> {
        let code = &self.instances[cur].code;
        let def = &code.files[file as usize];
        let mut peers = vec![];
        for (f, peer) in code.files.iter().enumerate() {
            if !def.same_sort_area.contains(&peer.name) {
                continue;
            }
            let slot = self.instances[cur].files[f];
            if self.file_states[slot].open.is_some()
                || self.active_sort_areas.iter().any(|a| a.contains(&slot))
            {
                return err("SAME SORT AREA files must be closed during SORT/MERGE");
            }
            peers.push(slot);
        }
        // An outer sort also reserves its own SD against a nested sort.
        let slot = self.instances[cur].files[file as usize];
        if self.active_sort_areas.iter().any(|a| a.contains(&slot)) {
            return err("SAME SORT AREA files cannot participate in nested SORT/MERGE");
        }
        peers.push(slot);
        Ok(peers)
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

    /// CLOSE REEL/UNIT keeps the file open and advances only the volume.
    pub(super) fn close_reel(&mut self, cur: usize, f: u32) -> RResult<()> {
        let def = self.file_def(cur, f);
        let Some(mode) = self.state(cur, f).open else {
            return self.io_error(cur, &def, "42", "CLOSE");
        };
        if !self.is_tape(&def, &self.file_path(&def)) {
            return self.set_status(cur, &def, "07");
        }
        if matches!(mode, OpenMode::Output | OpenMode::Extend) {
            let st = self.state(cur, f);
            let count = st.seq().records.len();
            st.reel_breaks.push(count);
            st.modified = true;
            self.output_volume_boundary(cur, f, &def)?;
        } else {
            let st = self.state(cur, f);
            if let Some(event) = st.volume_events.get(st.volume_done) {
                st.pos = st.pos.max(event.after_records as u64);
                self.input_volume_boundary(cur, f)?;
            }
        }
        self.set_status(cur, &def, "00")
    }

    pub(super) fn close_file(&mut self, cur: usize, f: u32, lock: bool) -> RResult<()> {
        self.close_file_options(cur, f, lock, true)
    }

    pub(super) fn close_file_implicit(&mut self, cur: usize, file: u32) -> RResult<()> {
        if self.state(cur, file).open.is_some() {
            self.close_file_options(cur, file, false, false)
        } else {
            Ok(())
        }
    }

    pub(super) fn close_file_options(
        &mut self,
        cur: usize,
        f: u32,
        lock: bool,
        run_labels: bool,
    ) -> RResult<()> {
        let def = self.file_def(cur, f);
        let mode = self.state(cur, f).open;
        let mut ending_labels = Vec::new();
        let path = self.file_path(&def);
        if run_labels
            && self.is_tape(&def, &path)
            && def.label_standard
            && let Some(mode) = mode
        {
            if matches!(mode, OpenMode::Output | OpenMode::Extend) {
                ending_labels = self.user_labels(cur, f, mode, false, false, None)?;
            } else if !self.state(cur, f).ending_labels_done {
                let labels = self.input_user_labels(cur, f, false);
                self.user_labels(cur, f, mode, false, false, Some(&labels))?;
                self.state(cur, f).ending_labels_done = true;
            }
        }
        let mut st = std::mem::take(self.state(cur, f));
        let Some(_mode) = st.open else {
            *self.state(cur, f) = st;
            return self.io_error(cur, &def, "42", "CLOSE");
        };
        if st.modified
            && let Some(path) = &st.path
        {
            let r = if let Some(layout) = st
                .physical_layout
                .or_else(|| self.physical_layout(&def, path))
            {
                let Data::Seq(data) = &st.data else {
                    return err("physical file is not sequential");
                };
                let mut records = data.records.clone();
                if def.code_set_ascii {
                    for r in &mut records {
                        *r = ebcdic::decode_latin1(r);
                    }
                }
                let (blocks, reel_breaks) = Self::encode_tape_records(
                    layout,
                    &records,
                    if def.code_set_ascii {
                        b' '
                    } else {
                        ebcdic::SPACE
                    },
                    &st.reel_breaks,
                )
                .map_err(ds_err)?;
                let bytes = if self.is_tape(&def, path) {
                    let mut entries =
                        st.tape_entries[..st.tape_start.min(st.tape_entries.len())].to_vec();
                    let mut position = 0;
                    let mut file_number = 0;
                    while position < entries.len() {
                        position = tape::file_with_labels(&entries, position, def.label_standard)
                            .map_err(ds_err)?
                            .next;
                        file_number += 1;
                    }
                    let mut breaks = st.prefix_reel_breaks.clone();
                    breaks.extend(reel_breaks.iter().map(|&b| (file_number, b)));
                    if def.label_standard {
                        self.tape_headers(&def, layout, &mut entries)?;
                        entries.pop();
                        entries.extend(st.header_labels.iter().cloned().map(Entry::Block));
                        entries.push(Entry::Mark);
                    }
                    entries.extend(blocks.iter().cloned().map(Entry::Block));
                    entries.push(Entry::Mark);
                    if def.label_standard {
                        self.tape_trailers(&def, layout, blocks.len(), &mut entries)?;
                        entries.pop();
                        entries.extend(ending_labels.iter().cloned().map(Entry::Block));
                        entries.push(Entry::Mark);
                    }
                    st.tape_next = entries.len();
                    entries.push(Entry::Mark);
                    let mut events = st.prefix_volume_events.clone();
                    events.extend_from_slice(&st.volume_events);
                    self.save_tape(&def, path, &entries, &events, &breaks, &mut st.buffers)
                        .map_err(ds_err)?;
                    Vec::new()
                } else {
                    blocks.concat()
                };
                if self.is_tape(&def, path) {
                    Ok(())
                } else {
                    st.buffers.write(path, &bytes)
                }
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
                self.output_volume_boundary(cur, f, &def)?;
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
        if def.org == Org::Sequential {
            self.input_volume_boundary(cur, f)?;
        }
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
                    st.ending_labels_done = true;
                    if self.is_tape(&def, &self.file_path(&def)) {
                        let labels = self.input_user_labels(cur, f, false);
                        self.user_labels(cur, f, mode.unwrap(), false, false, Some(&labels))?;
                    }
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
        if !self.active_sort_controls.as_ref().is_some_and(|c| {
            c.input_phase && c.file_slot == Some(self.instances[cur].files[f as usize])
        }) {
            return err("RELEASE requires the active SORT input procedure and SD file");
        }
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
        self.open_file(cur, f, OpenMode::Input)?;
        if self.state(cur, f).open != Some(OpenMode::Input) {
            if let Some(c) = self.active_sort_controls.clone() {
                self.sort_control_failure(cur, &c, "SORT input could not be opened")?;
            }
            return Ok(());
        }
        let len = def.max_len as usize;
        let add = |m: &mut Machine, r: &[u8]| {
            let mut r = r.to_vec();
            r.resize(len, ebcdic::SPACE);
            m.sort_records.push(r);
        };
        match self.state(cur, f).data.clone() {
            Data::Seq(d) => d.records.iter().for_each(|r| add(self, r)),
            Data::Rel(d) => {
                let mut n = 1;
                while let Some((k, r)) = d.next_from(n) {
                    add(self, r);
                    n = k + 1;
                }
            }
            Data::Keyed(d) => d.records().for_each(|r| add(self, r)),
            Data::None => {}
        }
        self.close_file(cur, f, false)?;
        if let Some(c) = &mut self.active_sort_controls {
            c.input_runs.push(self.sort_records.len());
        }
        Ok(())
    }

    pub(super) fn sort_sort(
        &mut self,
        cur: usize,
        spec: &SortSpec,
        records: Vec<Vec<u8>>,
    ) -> RResult<Vec<Vec<u8>>> {
        let controls = match &self.active_sort_controls {
            Some(c) => c.clone(),
            None => self.sort_controls(cur)?,
        };
        if controls
            .exact_count
            .is_some_and(|count| count != records.len() as u128)
        {
            self.sort_control_failure(
                cur,
                &controls,
                &format!(
                    "ICE047A: SORT record count mismatch; actual {} expected {}",
                    records.len(),
                    controls.exact_count.unwrap()
                ),
            )?;
            return Ok(vec![]);
        }
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
            let offset = ka
                .checked_sub(ra)
                .filter(|&offset| {
                    offset
                        .checked_add(kl)
                        .is_some_and(|end| end <= def.max_len as usize)
                })
                .ok_or_else(|| super::RunError("SORT key lies outside its SD record".into()))?;
            keys.push((
                offset,
                kl,
                k.ascending,
                code.descs[k.desc as usize].ty.clone(),
            ));
        }
        let coll = spec.collating.clone();
        let compare = |a: &[u8], b: &[u8]| {
            for (off, len, asc, ty) in &keys {
                let x = a.get(*off..off + len).unwrap_or(&[]);
                let y = b.get(*off..off + len).unwrap_or(&[]);
                let o = if ty.is_numeric() {
                    crate::numeric::number::Number::from_item(ty, x)
                        .compare(&crate::numeric::number::Number::from_item(ty, y))
                } else {
                    let mut o = std::cmp::Ordering::Equal;
                    for (p, q) in x.iter().zip(y) {
                        let (wp, wq) = match &coll {
                            Some(c) if !ty.is_national() => (c[*p as usize], c[*q as usize]),
                            _ => (u16::from(*p), u16::from(*q)),
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
        };
        if spec.merge {
            let mut start = 0;
            for &end in &controls.input_runs {
                if end > records.len() || start > end {
                    return err("invalid MERGE input boundary");
                }
                if records[start..end]
                    .windows(2)
                    .any(|r| compare(&r[0], &r[1]) == std::cmp::Ordering::Greater)
                {
                    self.sort_control_failure(cur, &controls, "MERGE input is not in key order")?;
                    return Ok(vec![]);
                }
                start = end;
            }
        }
        super::sort::order(records, &controls, compare)
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
        if !self.active_sort_controls.as_ref().is_some_and(|c| {
            !c.input_phase && c.file_slot == Some(self.instances[cur].files[f as usize])
        }) {
            return err("RETURN requires the active SORT/MERGE output procedure and SD file");
        }
        if self.sort_cancelled(cur)? {
            return err(super::sort::CANCELLED);
        }
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

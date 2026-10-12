//! IMS for the Zeus run times: the DBD and PSB library, the database of a
//! system, scheduled PSBs, the CALL interface (CBLTDLI) and EXEC DLI.
//!
//! Programs see the z/OS forms: PCB masks, SSAs and DIB fields in EBCDIC,
//! segment data as stored (raw bytes).

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use zeus_cobol::ebcdic;

use crate::call_adapter::{CallAdapter, PCB_HEADER_LEN, PcbEncoding};
use crate::catalog::{parse_dbd, parse_psb};
use crate::gsam::GsamSession;
use crate::{Database, Dbd, Psb};

/// DBDs and PSBs from their macro sources (DBDLIB and PSBLIB members).
#[derive(Clone, Default)]
pub struct Library {
    pub dbds: Vec<Dbd>,
    pub psbs: HashMap<String, Psb>,
}

fn has_word(text: &str, w: &str) -> bool {
    text.lines()
        .filter(|l| !l.starts_with('*'))
        .any(|l| l.split_whitespace().any(|x| x.eq_ignore_ascii_case(w)))
}

/// The text of a library member: host text, or EBCDIC 80-byte records
/// (a member of a cataloged FB 80 PDS).
fn member_text(bytes: &[u8]) -> String {
    if !bytes.contains(&b'\n') && bytes.len().is_multiple_of(80) && bytes.contains(&ebcdic::SPACE) {
        bytes
            .chunks(80)
            .map(|r| String::from_utf8_lossy(&ebcdic::decode_latin1(r)).into_owned())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        String::from_utf8_lossy(bytes).into_owned()
    }
}

impl Library {
    /// The library of `(member, source)` pairs, in search order: the first
    /// definition of a name is used.
    pub fn from_sources(
        sources: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Self, String> {
        let mut lib = Library::default();
        for (member, text) in sources {
            if has_word(&text, "PSBGEN") {
                let psb = parse_psb(&text).map_err(|e| format!("PSB {member}: {e}"))?;
                lib.psbs.entry(psb.name.clone()).or_insert(psb);
            } else if has_word(&text, "DBDGEN") || has_word(&text, "DBD") {
                let dbd = parse_dbd(&text).map_err(|e| format!("DBD {member}: {e}"))?;
                if !lib.dbds.iter().any(|d| d.name == dbd.name) {
                    lib.dbds.push(dbd);
                }
            }
        }
        Ok(lib)
    }

    /// The members of directories (PDS libraries), in order.
    pub fn from_dirs(dirs: &[PathBuf]) -> Result<Self, String> {
        let mut sources = Vec::new();
        for d in dirs {
            let mut files: Vec<PathBuf> = std::fs::read_dir(d)
                .map_err(|e| format!("{}: {e}", d.display()))?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect();
            files.sort();
            for f in files {
                let bytes = std::fs::read(&f).map_err(|e| format!("{}: {e}", f.display()))?;
                let text = member_text(&bytes);
                let name = f
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                sources.push((name, text));
            }
        }
        Library::from_sources(sources)
    }

    pub fn dbd(&self, name: &str) -> Option<&Dbd> {
        self.dbds.iter().find(|d| d.name == name)
    }
}

/// An IMS system: its library and its database (all DBDs, one log).
#[derive(Clone)]
pub struct Ims {
    pub library: Library,
    pub database: Database,
}

impl Ims {
    pub fn open(wal: &Path, library: Library) -> Result<Self, String> {
        if let Some(d) = wal.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        let database = Database::open(wal, library.dbds.clone())
            .map_err(|e| format!("{}: {e}", wal.display()))?;
        Ok(Ims { library, database })
    }
}

/// A PCB of a scheduled PSB, in the order the program sees them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PcbKind {
    /// The I/O PCB.
    Io,
    /// A database PCB (index in the PSB).
    Db(usize),
    Gsam(usize),
    /// An alternate TP PCB.
    Tp(usize),
}

/// Length of the I/O PCB mask.
pub const IO_PCB_LEN: usize = 48;
/// Length of a GSAM PCB mask.
pub const GSAM_PCB_LEN: usize = 44;

/// A scheduled PSB: its PCBs and the unit of work of its database calls.
pub struct Schedule {
    pub psb: Psb,
    pub pcbs: Vec<PcbKind>,
    /// Message processing (an MPP): the input messages and the output.
    pub tm: Option<Tm>,
    adapter: CallAdapter,
    gsam: HashMap<usize, GsamSession>,
    dbds: HashMap<String, Dbd>,
}

fn ebcdic_text(s: &str, len: usize) -> Vec<u8> {
    let mut b = vec![ebcdic::SPACE; len];
    for (d, c) in b.iter_mut().zip(s.bytes()) {
        *d = ebcdic::from_latin1(c);
    }
    b
}

fn text_of(b: &[u8]) -> String {
    String::from_utf8_lossy(&ebcdic::decode_latin1(b))
        .trim()
        .to_string()
}

impl Schedule {
    /// Schedules PSB `name`; `io_pcb`: the program's PCB list starts with
    /// the I/O PCB. GSAM data sets are found through `dds`.
    pub fn new(
        ims: &Ims,
        name: &str,
        io_pcb: bool,
        dds: &HashMap<String, PathBuf>,
    ) -> Result<Self, String> {
        let name = name.trim().to_ascii_uppercase();
        let psb = ims
            .library
            .psbs
            .get(&name)
            .cloned()
            .ok_or_else(|| format!("PSB {name} is not in the PSB library"))?;
        let mut dbds = HashMap::new();
        for p in &psb.pcbs {
            if let Some(d) = &p.dbd_name {
                let dbd = ims
                    .library
                    .dbd(d)
                    .ok_or_else(|| format!("PSB {name}: DBD {d} is not in the DBD library"))?;
                dbds.insert(d.clone(), dbd.clone());
            }
        }
        psb.validate(&dbds.values().cloned().collect::<Vec<_>>())?;
        let adapter = CallAdapter::new(ims.database.clone(), psb.clone(), PcbEncoding::Cp1047)?;
        let mut pcbs = Vec::new();
        if io_pcb {
            pcbs.push(PcbKind::Io);
        }
        let mut gsam = HashMap::new();
        for (i, p) in psb.pcbs.iter().enumerate() {
            pcbs.push(match p.kind.as_str() {
                "GSAM" => {
                    let d = p.dbd_name.as_deref().unwrap_or_default();
                    let def = dbds
                        .get(d)
                        .and_then(|x| x.gsam.clone())
                        .ok_or_else(|| format!("PSB {name}: {d} is not a GSAM DBD"))?;
                    gsam.insert(
                        i,
                        GsamSession::new(d, p, def, PcbEncoding::Cp1047).with_dds(dds.clone()),
                    );
                    PcbKind::Gsam(i)
                }
                "TP" => PcbKind::Tp(i),
                _ => PcbKind::Db(i),
            });
        }
        Ok(Schedule {
            psb,
            pcbs,
            tm: None,
            adapter,
            gsam,
            dbds,
        })
    }

    /// The initial PCB mask of the program's PCB `k`.
    pub fn mask(&self, k: usize) -> Vec<u8> {
        match self.pcbs[k] {
            PcbKind::Io => {
                let mut m = ebcdic_text("", IO_PCB_LEN);
                m[12..32].fill(0);
                m
            }
            PcbKind::Tp(i) => {
                let mut m =
                    ebcdic_text(self.psb.pcbs[i].lterm.as_deref().unwrap_or(""), IO_PCB_LEN);
                m[12..32].fill(0);
                m
            }
            PcbKind::Gsam(i) => {
                let mut m = ebcdic_text(
                    self.psb.pcbs[i].dbd_name.as_deref().unwrap_or(""),
                    GSAM_PCB_LEN,
                );
                m[12..].fill(0);
                m
            }
            PcbKind::Db(i) => {
                let p = &self.psb.pcbs[i];
                let mut m = vec![0u8; PCB_HEADER_LEN + p.keylen.unwrap_or(0)];
                m[..8].copy_from_slice(&ebcdic_text(p.dbd_name.as_deref().unwrap_or(""), 8));
                m[8..12].copy_from_slice(&ebcdic_text("", 4));
                m[12..16].copy_from_slice(&ebcdic_text(p.procopt.as_deref().unwrap_or(""), 4));
                m[20..28].copy_from_slice(&ebcdic_text("", 8));
                m
            }
        }
    }

    /// The length of the PCB mask of the program's PCB `k`.
    pub fn mask_len(&self, k: usize) -> usize {
        match self.pcbs[k] {
            PcbKind::Io | PcbKind::Tp(_) => IO_PCB_LEN,
            PcbKind::Gsam(_) => GSAM_PCB_LEN,
            PcbKind::Db(i) => PCB_HEADER_LEN + self.psb.pcbs[i].keylen.unwrap_or(0),
        }
    }

    /// The length of the I/O area a call of `function` (ASCII) uses.
    pub fn io_len(&self, k: usize, function: &str, ssas: &[&[u8]]) -> usize {
        match self.pcbs[k] {
            PcbKind::Io | PcbKind::Tp(_) => match function {
                "CHKP" => 8,
                _ => 0,
            },
            PcbKind::Gsam(i) => self
                .psb
                .pcbs
                .get(i)
                .and_then(|p| p.dbd_name.as_deref())
                .and_then(|d| self.dbds.get(d))
                .and_then(|d| d.gsam.as_ref())
                .map_or(0, |g| g.record_len),
            PcbKind::Db(i) => {
                let dbd = self.psb.pcbs[i]
                    .dbd_name
                    .as_deref()
                    .and_then(|d| self.dbds.get(d));
                let Some(dbd) = dbd else { return 0 };
                let named = ssas
                    .last()
                    .filter(|_| matches!(function, "ISRT" | "REPL" | "DLET"))
                    .map(|s| text_of(&s[..s.len().min(8)]))
                    .and_then(|n| dbd.segment(&n).map(|s| s.max_bytes));
                let one = named
                    .unwrap_or_else(|| dbd.segments.iter().map(|s| s.max_bytes).max().unwrap_or(0));
                // a path call (command code D) gets the parents' segments too
                let path: usize = ssas
                    .iter()
                    .take(ssas.len().saturating_sub(1))
                    .filter(|s| path_call(s))
                    .filter_map(|s| {
                        dbd.segment(&text_of(&s[..s.len().min(8)]))
                            .map(|d| d.max_bytes)
                    })
                    .sum();
                one + path
            }
        }
    }

    /// A DL/I call on the program's PCB `k` with the PCB mask `pcb`;
    /// `function`, the SSAs and the masks are EBCDIC. The status code is
    /// in the mask.
    pub fn call(
        &mut self,
        k: usize,
        function: &str,
        pcb: &mut [u8],
        io: &mut [u8],
        ssas: &[&[u8]],
        rsa: Option<&mut [u8]>,
    ) -> Result<(), String> {
        let status = |pcb: &mut [u8], s: &str| pcb[10..12].copy_from_slice(&ebcdic_text(s, 2));
        match self.pcbs.get(k).copied().ok_or("PCB number out of range")? {
            PcbKind::Io | PcbKind::Tp(_) => match function {
                "CHKP" => {
                    let id: Option<[u8; 8]> = io.get(..8).map(|b| b.try_into().unwrap());
                    self.commit(id)?;
                    status(pcb, "");
                }
                "SYNC" => {
                    self.commit(None)?;
                    status(pcb, "");
                }
                "ROLB" => {
                    self.rollback();
                    if let Some(tm) = self.tm.as_mut() {
                        tm.building.clear();
                    }
                    status(pcb, "");
                }
                _ => self.tm_call(k, function, pcb, io)?,
            },
            PcbKind::Gsam(i) => {
                let g = self.gsam.get_mut(&i).ok_or("GSAM PCB not scheduled")?;
                g.call(function, pcb, io, rsa)?;
            }
            PcbKind::Db(i) => {
                // a variable-length segment is written with the length in
                // its first halfword
                let len = match function {
                    "ISRT" | "REPL" => self.variable_len(i, ssas, io),
                    _ => None,
                };
                let io = match len {
                    Some(n) => &mut io[..n],
                    None => io,
                };
                self.adapter
                    .call_encoded(i, &ebcdic_text(function, 4), pcb, io, ssas)?;
            }
        }
        Ok(())
    }

    /// Holds segment `name` of the current path of DB PCB `k` again (a
    /// get hold qualified by the keys of the path in the key feedback).
    /// False when it is not on the path.
    pub fn hold_path(&mut self, k: usize, name: &str, pcb: &mut [u8]) -> Result<bool, String> {
        let PcbKind::Db(i) = self.pcbs[k] else {
            return Ok(false);
        };
        let Some(dbd) = self.psb.pcbs[i]
            .dbd_name
            .as_deref()
            .and_then(|d| self.dbds.get(d))
            .cloned()
        else {
            return Ok(false);
        };
        // the path from the root to the segment
        let mut path = Vec::new();
        let mut seg = dbd.segment(name);
        while let Some(s) = seg {
            path.push(s.clone());
            seg = s.parent.as_deref().and_then(|p| dbd.segment(p));
        }
        path.reverse();
        let kfb = u32::from_be_bytes([pcb[28], pcb[29], pcb[30], pcb[31]]) as usize;
        let feedback =
            pcb[PCB_HEADER_LEN..PCB_HEADER_LEN + kfb.min(pcb.len() - PCB_HEADER_LEN)].to_vec();
        let mut ssas = Vec::new();
        let mut at = 0;
        for s in &path {
            let Some(key) = s.key() else { return Ok(false) };
            if at + key.bytes > feedback.len() {
                return Ok(false);
            }
            let mut q = ebcdic_text(&s.name, 8);
            q.push(ebcdic::from_latin1(b'('));
            q.extend(ebcdic_text(&key.name, 8));
            q.extend(ebcdic_text("= ", 2));
            q.extend_from_slice(&feedback[at..at + key.bytes]);
            q.push(ebcdic::from_latin1(b')'));
            ssas.push(q);
            at += key.bytes;
        }
        let refs: Vec<&[u8]> = ssas.iter().map(Vec::as_slice).collect();
        let mut scratch = vec![0u8; dbd.segments.iter().map(|s| s.max_bytes).max().unwrap_or(0)];
        self.adapter
            .call_encoded(i, &ebcdic_text("GHU", 4), pcb, &mut scratch, &refs)?;
        Ok(text_of(&pcb[10..12]).is_empty())
    }

    fn variable_len(&self, i: usize, ssas: &[&[u8]], io: &[u8]) -> Option<usize> {
        let dbd = self.psb.pcbs[i]
            .dbd_name
            .as_deref()
            .and_then(|d| self.dbds.get(d))?;
        let name = text_of(&ssas.last()?[..8]);
        let seg = dbd.segment(&name)?;
        if seg.min_bytes == seg.max_bytes || io.len() < 2 {
            return None;
        }
        Some((u16::from_be_bytes([io[0], io[1]]) as usize).clamp(seg.min_bytes, io.len()))
    }

    /// Commits the unit of work (with a checkpoint ID).
    pub fn commit(&mut self, checkpoint: Option<[u8; 8]>) -> Result<(), String> {
        self.adapter.commit_checkpoint(checkpoint)?;
        for g in self.gsam.values_mut() {
            g.commit()?;
        }
        Ok(())
    }

    /// A symbolic checkpoint (CHKP with areas): commits, saving the areas
    /// with checkpoint `id`.
    pub fn symbolic_checkpoint(&mut self, id: [u8; 8], areas: &[&[u8]]) -> Result<(), String> {
        let mut data = Vec::new();
        for a in areas {
            data.extend((a.len() as u32).to_be_bytes());
            data.extend_from_slice(a);
        }
        self.adapter.commit_symbolic(id, data)?;
        for g in self.gsam.values_mut() {
            g.commit()?;
        }
        Ok(())
    }

    /// XRST: the checkpoint ID in `id` (blank: a normal start, nothing to
    /// restore; `LAST`: the latest checkpoint); the areas it saved go back
    /// into `areas`. The ID restarted from, or None for a normal start.
    pub fn restart(&self, id: &[u8], areas: &mut [&mut [u8]]) -> Result<Option<[u8; 8]>, String> {
        let text = text_of(id);
        if text.is_empty() {
            return Ok(None);
        }
        let wanted = if text.eq_ignore_ascii_case("LAST") {
            None
        } else {
            let mut b = [ebcdic::SPACE; 8];
            let n = id.len().min(8);
            b[..n].copy_from_slice(&id[..n]);
            Some(b)
        };
        let (found, data) = self
            .adapter
            .checkpoint_data(wanted)
            .ok_or_else(|| format!("XRST: no symbolic checkpoint {text}"))?;
        let mut at = 0;
        for a in areas.iter_mut() {
            let Some(len) = data.get(at..at + 4) else {
                break;
            };
            let len = u32::from_be_bytes(len.try_into().unwrap()) as usize;
            let saved = &data[at + 4..(at + 4 + len).min(data.len())];
            let n = saved.len().min(a.len());
            a[..n].copy_from_slice(&saved[..n]);
            at += 4 + len;
        }
        Ok(Some(found))
    }

    pub fn rollback(&mut self) {
        self.adapter.rollback();
        for g in self.gsam.values_mut() {
            g.rollback();
        }
    }
}

// ---- message processing (IMS TM) ---------------------------------------

/// A message: its destination (transaction code or LTERM), its source
/// (LTERM) and its segments (each with its LL ZZ prefix).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TmMessage {
    pub destination: String,
    pub source: String,
    pub segments: Vec<Vec<u8>>,
}

impl TmMessage {
    /// A one-segment input message for transaction `tran`: LL ZZ, the
    /// transaction code (padded to 8) and `data`.
    pub fn input(tran: &str, source: &str, data: &[u8]) -> TmMessage {
        let mut text = ebcdic_text(tran, 8);
        text.extend_from_slice(data);
        let mut seg = ((text.len() + 4) as u16).to_be_bytes().to_vec();
        seg.extend([0, 0]);
        seg.extend(text);
        TmMessage {
            destination: tran.to_ascii_uppercase(),
            source: source.to_string(),
            segments: vec![seg],
        }
    }

    /// The segments one after the other (as kept in the message queue).
    pub fn data(&self) -> Vec<u8> {
        self.segments.concat()
    }

    /// The segments of queue data (each starts with its LL).
    pub fn split(data: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        let mut i = 0;
        while i + 2 <= data.len() {
            let ll = (u16::from_be_bytes([data[i], data[i + 1]]) as usize).max(4);
            let end = (i + ll).min(data.len());
            out.push(data[i..end].to_vec());
            i = end;
        }
        out
    }
}

/// The messages of a message processing program: the input messages it
/// gets (GU, GN on the I/O PCB) and the output messages it inserts (ISRT,
/// PURG, CHNG). Its unit of work ends at each GU for a new message.
#[derive(Debug, Default)]
pub struct Tm {
    pub inputs: std::collections::VecDeque<TmMessage>,
    /// The current input message and its next segment.
    current: Option<(TmMessage, usize)>,
    /// Output messages complete (ISRT ... PURG, or the next GU).
    pub outputs: Vec<TmMessage>,
    /// Output being built, per PCB.
    building: HashMap<usize, TmMessage>,
    /// Destinations set by CHNG, per PCB.
    changed: HashMap<usize, String>,
    pub userid: String,
    seq: u32,
}

impl Tm {
    pub fn new(inputs: Vec<TmMessage>, userid: &str) -> Tm {
        Tm {
            inputs: inputs.into(),
            userid: userid.to_string(),
            ..Tm::default()
        }
    }

    /// Ends the output messages being built.
    pub fn flush(&mut self) {
        let mut keys: Vec<usize> = self.building.keys().copied().collect();
        keys.sort_unstable();
        for k in keys {
            if let Some(m) = self.building.remove(&k) {
                self.outputs.push(m);
            }
        }
    }

    /// The length of the segment the next GU / GN returns.
    fn next_len(&self, function: &str) -> usize {
        match function {
            "GU" => self.inputs.front().and_then(|m| m.segments.first()),
            _ => self.current.as_ref().and_then(|(m, i)| m.segments.get(*i)),
        }
        .map_or(0, Vec::len)
    }
}

/// Packed date 0CYYDDDF and time HHMMSSTF of now (UTC).
fn pcb_date_time() -> ([u8; 4], [u8; 4]) {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = unix.div_euclid(86400);
    let sod = unix.rem_euclid(86400);
    let (y, _, _) = zeus_cobol::vm::civil_from_days(days);
    let doy = days - zeus_cobol::vm::days_from_civil(y, 1, 1) + 1;
    let c = (y - 1900) / 100;
    let date = c * 100_000 + (y % 100) * 1000 + doy;
    let time = (sod / 3600) * 100_000 + (sod / 60 % 60) * 1000 + (sod % 60) * 10;
    let pack = |v: i64| -> [u8; 4] {
        let mut b = [0u8; 4];
        zeus_decimal::packed::store(&mut b, zeus_decimal::Dec::from_i64(v), false);
        b
    };
    (pack(date), pack(time))
}

impl Schedule {
    /// A message call on the I/O PCB or an alternate PCB (PCB `k`).
    fn tm_call(
        &mut self,
        k: usize,
        function: &str,
        pcb: &mut [u8],
        io: &mut [u8],
    ) -> Result<(), String> {
        let status = |pcb: &mut [u8], s: &str| pcb[10..12].copy_from_slice(&ebcdic_text(s, 2));
        let alt = match self.pcbs[k] {
            PcbKind::Tp(i) => Some(i),
            _ => None,
        };
        let Some(tm) = self.tm.as_mut() else {
            // no message processing in a batch region
            status(pcb, "QC");
            return Ok(());
        };
        match function {
            "GU" if alt.is_none() => {
                // a new message: the previous one's unit of work ends
                tm.flush();
                let Some(m) = tm.inputs.pop_front() else {
                    tm.current = None;
                    status(pcb, "QC");
                    return Ok(());
                };
                let seg = m.segments.first().cloned().unwrap_or_default();
                let n = seg.len().min(io.len());
                io[..n].copy_from_slice(&seg[..n]);
                pcb[..8].copy_from_slice(&ebcdic_text(&m.source, 8));
                let (d, t) = pcb_date_time();
                pcb[12..16].copy_from_slice(&d);
                pcb[16..20].copy_from_slice(&t);
                tm.seq += 1;
                pcb[20..24].copy_from_slice(&tm.seq.to_be_bytes());
                if pcb.len() >= 40 {
                    let u = tm.userid.clone();
                    pcb[32..40].copy_from_slice(&ebcdic_text(&u, 8));
                }
                tm.current = Some((m, 1));
                // the end of the previous message's unit of work
                self.commit(None)?;
                status(pcb, "");
            }
            "GN" if alt.is_none() => {
                let next = tm
                    .current
                    .as_mut()
                    .and_then(|(m, i)| m.segments.get(*i).cloned().inspect(|_| *i += 1));
                match next {
                    Some(seg) => {
                        let n = seg.len().min(io.len());
                        io[..n].copy_from_slice(&seg[..n]);
                        status(pcb, "");
                    }
                    None => status(pcb, if tm.current.is_some() { "QD" } else { "QC" }),
                }
            }
            "ISRT" => {
                let destination = match alt {
                    Some(i) => tm
                        .changed
                        .get(&k)
                        .cloned()
                        .unwrap_or_else(|| self.psb.pcbs[i].lterm.clone().unwrap_or_default()),
                    None => tm
                        .current
                        .as_ref()
                        .map(|(m, _)| m.source.clone())
                        .unwrap_or_default(),
                };
                if destination.is_empty() {
                    status(pcb, "QH");
                    return Ok(());
                }
                let ll = if io.len() >= 2 {
                    (u16::from_be_bytes([io[0], io[1]]) as usize).clamp(4, io.len())
                } else {
                    io.len()
                };
                let seg = io[..ll].to_vec();
                let m = tm.building.entry(k).or_insert_with(|| TmMessage {
                    destination,
                    source: String::new(),
                    segments: Vec::new(),
                });
                m.segments.push(seg);
                status(pcb, "");
            }
            "PURG" => {
                if let Some(m) = tm.building.remove(&k) {
                    tm.outputs.push(m);
                }
                status(pcb, "");
            }
            "CHNG" if alt.is_some() => {
                let d = text_of(&io[..io.len().min(8)]);
                tm.changed.insert(k, d);
                status(pcb, "");
            }
            other => {
                return Err(format!("DL/I call {other} on a TP PCB is not supported"));
            }
        }
        Ok(())
    }

    /// The I/O area length of a message call (`ll`: the halfword at the
    /// start of the I/O area).
    pub fn tm_io_len(&self, function: &str, ll: Option<usize>) -> usize {
        match function {
            "GU" | "GN" => self.tm.as_ref().map_or(0, |t| t.next_len(function)),
            "ISRT" => ll.unwrap_or(0),
            "CHNG" => 8,
            "CHKP" => 8,
            _ => 0,
        }
    }
}

// ---- the CALL interface of a batch program ----------------------------

struct Batch {
    sched: Schedule,
    /// Addresses of the PCB masks in the program's storage.
    addrs: Vec<usize>,
}

thread_local! {
    static BATCH: RefCell<Option<Batch>> = const { RefCell::new(None) };
}

/// Starts a batch program (DFSRRC00) with its PSB; its PCB masks are at
/// `addrs` in its storage.
pub fn start_batch(sched: Schedule, addrs: Vec<usize>) {
    BATCH.with(|b| *b.borrow_mut() = Some(Batch { sched, addrs }));
}

/// Ends the batch program: its unit of work is committed (normal end) or
/// backed out.
pub fn end_batch(commit: bool) -> Result<(), String> {
    let out = end_region(commit)?;
    OUTPUTS.with(|o| o.borrow_mut().extend(out));
    Ok(())
}

thread_local! {
    static OUTPUTS: RefCell<Vec<TmMessage>> = const { RefCell::new(Vec::new()) };
}

/// The output messages of the message processing programs that ended
/// (with [`end_batch`]) since the last call.
pub fn take_outputs() -> Vec<TmMessage> {
    OUTPUTS.with(|o| std::mem::take(&mut *o.borrow_mut()))
}

/// Ends the program as [`end_batch`]; the output messages of a message
/// processing program (none when it ended abnormally).
pub fn end_region(commit: bool) -> Result<Vec<TmMessage>, String> {
    let Some(mut b) = BATCH.with(|b| b.borrow_mut().take()) else {
        return Ok(Vec::new());
    };
    if commit {
        b.sched.commit(None)?;
        Ok(b.sched.tm.take().map_or(Vec::new(), |mut t| {
            t.flush();
            t.outputs
        }))
    } else {
        b.sched.rollback();
        Ok(Vec::new())
    }
}

fn be32(b: &[u8]) -> i32 {
    i32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// The length of an SSA at `a` (8-byte name, or a qualification up to
/// its closing parenthesis).
fn ssa_len(mem: &[u8], a: usize) -> usize {
    let end = mem.len().min(a + 512);
    // command codes (*) or a qualification follow the name
    if mem.get(a + 8) != Some(&ebcdic::from_latin1(b'('))
        && mem.get(a + 8) != Some(&ebcdic::from_latin1(b'*'))
    {
        return 9.min(end - a);
    }
    // the value may contain any byte: the closing parenthesis after the
    // field name, operator and value is found by the call; give it all
    end - a
}

/// Whether an SSA (EBCDIC) has command code D.
fn path_call(ssa: &[u8]) -> bool {
    if ssa.get(8) != Some(&ebcdic::from_latin1(b'*')) {
        return false;
    }
    ssa[9..]
        .iter()
        .map(|&b| ebcdic::to_latin1(b))
        .take_while(|&c| c != b'(' && c != b' ')
        .any(|c| c == b'D')
}

/// CALL 'CBLTDLI' USING [count] function pcb [io-area [ssa...]].
pub fn cbltdli(mem: &mut [u8], args: &[Option<usize>]) -> Result<i32, String> {
    BATCH.with(|b| {
        let mut b = b.borrow_mut();
        let b = b
            .as_mut()
            .ok_or("CBLTDLI: the program does not run under DFSRRC00 (no PSB is scheduled)")?;
        let arg = |i: usize| args.get(i).copied().flatten();
        let first = arg(0).ok_or("CBLTDLI: no function")?;
        // an optional parameter count (binary fullword)
        let (start, count) = if mem.get(first) == Some(&0) {
            (1, Some(be32(&mem[first..first + 4]).max(0) as usize))
        } else {
            (0, None)
        };
        let n = count.map_or(args.len(), |c| (start + c).min(args.len()));
        let f = arg(start).ok_or("CBLTDLI: no function")?;
        let function = text_of(&mem[f..f + 4]);
        let pa = arg(start + 1).ok_or("CBLTDLI: no PCB")?;
        let k = b
            .addrs
            .iter()
            .position(|&a| a == pa)
            .ok_or("CBLTDLI: the PCB is not one of the program's PCBs")?;
        let tp = matches!(b.sched.pcbs[k], PcbKind::Io | PcbKind::Tp(_));
        // symbolic CHKP and XRST: I/O area length, I/O area (the checkpoint
        // ID), then the length and address of each area to save or restore
        if tp && (function == "XRST" || (function == "CHKP" && n >= start + 6)) {
            let io = arg(start + 3).ok_or("CBLTDLI: no I/O area")?;
            let mut spans = Vec::new();
            let mut j = start + 4;
            while j + 1 < n {
                let (Some(l), Some(a)) = (arg(j), arg(j + 1)) else {
                    break;
                };
                let len = be32(&mem[l..l + 4]).max(0) as usize;
                spans.push((a, len.min(mem.len() - a)));
                j += 2;
            }
            let status =
                |mem: &mut [u8]| mem[pa + 10..pa + 12].copy_from_slice(&ebcdic_text("", 2));
            if function == "CHKP" {
                let mut id = [ebcdic::SPACE; 8];
                id.copy_from_slice(&mem[io..io + 8]);
                let areas: Vec<&[u8]> = spans.iter().map(|&(a, l)| &mem[a..a + l]).collect();
                b.sched.symbolic_checkpoint(id, &areas)?;
            } else {
                let id = mem[io..io + 8].to_vec();
                let mut saved: Vec<Vec<u8>> =
                    spans.iter().map(|&(a, l)| mem[a..a + l].to_vec()).collect();
                let mut refs: Vec<&mut [u8]> = saved.iter_mut().map(Vec::as_mut_slice).collect();
                if let Some(found) = b.sched.restart(&id, &mut refs)? {
                    for (&(a, l), d) in spans.iter().zip(&saved) {
                        mem[a..a + l].copy_from_slice(d);
                    }
                    mem[io..io + 8].copy_from_slice(&found);
                }
            }
            status(mem);
            return Ok(0);
        }
        let ssa_addrs: Vec<usize> = (start + 3..n).filter_map(arg).collect();
        let ssas: Vec<Vec<u8>> = ssa_addrs
            .iter()
            .map(|&a| mem[a..a + ssa_len(mem, a)].to_vec())
            .collect();
        let refs: Vec<&[u8]> = ssas.iter().map(Vec::as_slice).collect();
        let io_addr = arg(start + 2);
        let gsam = matches!(b.sched.pcbs[k], PcbKind::Gsam(_));
        let io_len = io_addr.map_or(0, |a| {
            if tp {
                let ll = mem
                    .get(a..a + 2)
                    .map(|h| u16::from_be_bytes([h[0], h[1]]) as usize);
                b.sched.tm_io_len(&function, ll)
            } else {
                b.sched.io_len(k, &function, &refs)
            }
            .min(mem.len() - a)
        });
        let mut io = io_addr.map_or(Vec::new(), |a| mem[a..a + io_len].to_vec());
        let plen = b.sched.mask_len(k).min(mem.len() - pa);
        let mut pcb = mem[pa..pa + plen].to_vec();
        // GSAM: the record search argument follows the I/O area
        let rsa_addr = if gsam { arg(start + 3) } else { None };
        let mut rsa = rsa_addr.map(|a| mem[a..a + 8].to_vec());
        let ssa_refs: &[&[u8]] = if gsam { &[] } else { &refs };
        b.sched.call(
            k,
            &function,
            &mut pcb,
            &mut io,
            ssa_refs,
            rsa.as_deref_mut(),
        )?;
        mem[pa..pa + plen].copy_from_slice(&pcb);
        if let Some(a) = io_addr {
            mem[a..a + io.len()].copy_from_slice(&io);
        }
        if let (Some(a), Some(r)) = (rsa_addr, rsa) {
            mem[a..a + 8].copy_from_slice(&r);
        }
        Ok(0)
    })
}

/// The scheduled batch program's PCB addresses and schedule, for EXEC DLI.
pub fn with_batch<T>(f: impl FnOnce(&mut Schedule, &[usize]) -> T) -> Option<T> {
    BATCH.with(|b| b.borrow_mut().as_mut().map(|b| f(&mut b.sched, &b.addrs)))
}

/// CALL PLITDLI(count, function, pcb, io-area, ssa...) from PL/I: the PCB
/// is a POINTER to the PCB, or the PCB (a based structure) itself.
pub fn plitdli(mem: &mut [u8], args: &[zeus_cobol::interlang::XRef]) -> Result<i32, String> {
    let mut addrs: Vec<Option<usize>> = args.iter().map(|a| a.addr).collect();
    if let (Some(x), Some(Some(a))) = (args.get(2), addrs.get(2).copied())
        && x.len == 4
    {
        let p = be32(&mem[a..a + 4]) as u32 as usize;
        addrs[2] = Some(p);
    }
    // a message segment of a PL/I program has a fullword LL (the segment
    // length less 2): ISRT on a TP PCB gets the usual halfword LL
    let function = addrs
        .get(1)
        .copied()
        .flatten()
        .map(|f| text_of(&mem[f..f + 4]))
        .unwrap_or_default();
    let tp = addrs.get(2).copied().flatten().is_some_and(|pcb| {
        with_batch(|s, a| {
            a.iter()
                .position(|&x| x == pcb)
                .is_some_and(|k| matches!(s.pcbs[k], PcbKind::Io | PcbKind::Tp(_)))
        })
        .unwrap_or(false)
    });
    if function == "ISRT"
        && tp
        && let Some(Some(io)) = addrs.get(3).copied()
    {
        let len = (be32(&mem[io..io + 4]).max(2) as usize + 2).min(mem.len() - io);
        let saved = mem[io..io + len].to_vec();
        let mut seg = ((len - 2) as u16).to_be_bytes().to_vec();
        seg.extend_from_slice(&saved[4.min(len)..]);
        mem[io..io + seg.len()].copy_from_slice(&seg);
        let r = cbltdli(mem, &addrs);
        mem[io..io + len].copy_from_slice(&saved);
        return r;
    }
    cbltdli(mem, &addrs)
}

/// Makes CBLTDLI (and PLITDLI for PL/I) callable by the programs of `m`.
pub fn register(m: &mut zeus_cobol::vm::Machine) {
    m.add_native("CBLTDLI", cbltdli);
    m.add_native_ref("PLITDLI", plitdli);
}

// ---- EXEC DLI ---------------------------------------------------------

use zeus_cobol::vm::exec::{Area, ExecArg, ExecCall, ExecHost, ExecOutcome};
use zeus_cobol::vm::{Machine, RResult, RunError, data};
use zeus_cobol::zir::{DataType, ExecKind};

/// Where the PCB masks of EXEC DLI are: in the program's storage (batch:
/// the masks passed to it) or kept by the run time (CICS).
pub enum Masks<'a> {
    Memory(&'a [usize]),
    Own(&'a mut Vec<Vec<u8>>),
}

fn arg_text(m: &Machine, a: &ExecArg) -> String {
    match a {
        ExecArg::Name(s) => s.clone(),
        ExecArg::Alnum(b) => text_of(b),
        ExecArg::Area(ar) => text_of(&m.storage()[ar.addr..ar.addr + ar.len]),
        ExecArg::Num(d) => d.to_string(),
        _ => String::new(),
    }
}

fn arg_num(m: &Machine, a: &ExecArg) -> Option<i64> {
    let d = match a {
        ExecArg::Num(d) => *d,
        ExecArg::Area(Area {
            addr,
            len,
            ty: DataType::Numeric(t),
        }) => data::num_value(t, &m.storage()[*addr..*addr + *len]),
        _ => return arg_text(m, a).trim().parse().ok(),
    };
    d.rescale(0, zeus_decimal::Rounding::Truncate)
        .map(|d| d.coef as i64)
}

fn area(a: Option<&ExecArg>) -> Option<Area> {
    match a {
        Some(ExecArg::Area(ar)) => Some(ar.clone()),
        _ => None,
    }
}

/// Sets the DIB of the program from a PCB mask (`None`: status only).
fn set_dib(m: &mut Machine, call: &ExecCall, status: &str, pcb: Option<(&[u8], &str)>) {
    let Some(ExecArg::Area(dib)) = call.arg("DIB") else {
        return;
    };
    if dib.len < 34 {
        return;
    }
    let mem = &mut m.storage_mut()[dib.addr..dib.addr + dib.len];
    mem[0..2].copy_from_slice(&ebcdic_text("1", 2));
    mem[2..4].copy_from_slice(&ebcdic_text(status, 2));
    if let Some((p, org)) = pcb {
        mem[4..12].copy_from_slice(&p[20..28]);
        mem[14..16].copy_from_slice(&p[8..10]);
        let kfb = u32::from_be_bytes([p[28], p[29], p[30], p[31]]) as i16;
        mem[16..18].copy_from_slice(&kfb.to_be_bytes());
        mem[18..26].copy_from_slice(&p[0..8]);
        mem[26..34].copy_from_slice(&ebcdic_text(org, 8));
    }
}

/// An SSA from `SEGMENT(name)` and its `WHERE(field op value)`.
fn ssa(m: &Machine, name: &str, cond: Option<(&str, Option<&Area>)>) -> Vec<u8> {
    let mut s = ebcdic_text(name, 8);
    match cond {
        None => s.push(ebcdic::SPACE),
        Some((text, value)) => {
            let mut w = text.split_whitespace();
            let field = w.next().unwrap_or("");
            let op = match w.next().unwrap_or("=").to_ascii_uppercase().as_str() {
                "=" | "EQ" => "= ",
                ">=" | "GE" | "=>" => ">=",
                "<=" | "LE" | "=<" => "<=",
                ">" | "GT" => "> ",
                "<" | "LT" => "< ",
                _ => "¬=",
            };
            s.push(ebcdic::from_latin1(b'('));
            s.extend(ebcdic_text(field, 8));
            s.extend(ebcdic::encode(op));
            match value {
                Some(a) => s.extend_from_slice(&m.storage()[a.addr..a.addr + a.len]),
                None => {
                    let lit = w.collect::<Vec<_>>().join(" ");
                    s.extend(ebcdic::encode(lit.trim_matches(|c| c == '\'' || c == '"')));
                }
            }
            s.push(ebcdic::from_latin1(b')'));
        }
    }
    s
}

/// Runs a DL/I command of EXEC DLI on a scheduled PSB.
pub fn exec_command(
    m: &mut Machine,
    call: &ExecCall,
    sched: &mut Schedule,
    masks: Masks,
) -> RResult<()> {
    let cmd = call.command.to_ascii_uppercase();
    match cmd.as_str() {
        "CHKP" | "SYNC" => {
            let id = call.arg("ID").map(|a| {
                let mut b = [ebcdic::SPACE; 8];
                let t = match a {
                    ExecArg::Area(ar) => m.storage()[ar.addr..ar.addr + ar.len.min(8)].to_vec(),
                    other => ebcdic_text(&arg_text(m, other), 8),
                };
                b[..t.len()].copy_from_slice(&t);
                b
            });
            sched.commit(id).map_err(RunError)?;
            set_dib(m, call, "", None);
            return Ok(());
        }
        "ROLB" | "ROLL" => {
            sched.rollback();
            set_dib(m, call, "", None);
            return Ok(());
        }
        // SYMCHKP ID() AREA1() LENGTH1() ...; XRST ID() AREA1() ...
        "SYMCHKP" | "XRST" => {
            let mut spans = Vec::new();
            for k in 1..=7 {
                let Some(ExecArg::Area(a)) = call.arg(&format!("AREA{k}")) else {
                    continue;
                };
                let len = call
                    .arg(&format!("LENGTH{k}"))
                    .and_then(|l| arg_num(m, l))
                    .map_or(a.len, |l| l.max(0) as usize);
                spans.push((a.addr, len.min(m.storage().len() - a.addr)));
            }
            let id_area = match call.arg("ID") {
                Some(ExecArg::Area(a)) => Some(a.clone()),
                _ => None,
            };
            let id: Vec<u8> = match call.arg("ID") {
                Some(ExecArg::Area(a)) => m.storage()[a.addr..a.addr + a.len.min(8)].to_vec(),
                Some(other) => ebcdic_text(&arg_text(m, other), 8),
                None => ebcdic_text("", 8),
            };
            if cmd == "SYMCHKP" {
                let mut b = [ebcdic::SPACE; 8];
                b[..id.len().min(8)].copy_from_slice(&id[..id.len().min(8)]);
                let areas: Vec<Vec<u8>> = spans
                    .iter()
                    .map(|&(a, l)| m.storage()[a..a + l].to_vec())
                    .collect();
                let refs: Vec<&[u8]> = areas.iter().map(Vec::as_slice).collect();
                sched.symbolic_checkpoint(b, &refs).map_err(RunError)?;
            } else {
                let mut saved: Vec<Vec<u8>> = spans
                    .iter()
                    .map(|&(a, l)| m.storage()[a..a + l].to_vec())
                    .collect();
                let mut refs: Vec<&mut [u8]> = saved.iter_mut().map(Vec::as_mut_slice).collect();
                if let Some(found) = sched.restart(&id, &mut refs).map_err(RunError)? {
                    for (&(a, l), d) in spans.iter().zip(&saved) {
                        m.storage_mut()[a..a + l].copy_from_slice(d);
                    }
                    if let Some(a) = id_area {
                        let n = a.len.min(8);
                        m.storage_mut()[a.addr..a.addr + n].copy_from_slice(&found[..n]);
                    }
                }
            }
            set_dib(m, call, "", None);
            return Ok(());
        }
        _ => {}
    }
    let function = match cmd.as_str() {
        // the get commands hold their segment (as GHU, GHN, GHNP)
        "GU" | "GHU" => "GHU",
        "GN" | "GHN" => "GHN",
        "GNP" | "GHNP" => "GHNP",
        "ISRT" | "LOAD" => "ISRT",
        "REPL" => "REPL",
        "DLET" => "DLET",
        other => return Err(RunError(format!("EXEC DLI {other} is not supported"))),
    };
    let k = call
        .arg("PCB")
        .and_then(|a| arg_num(m, a))
        .unwrap_or(1)
        .max(1) as usize
        - 1;
    if k >= sched.pcbs.len() {
        return Err(RunError(format!(
            "EXEC DLI: PCB({}) is not in the PSB",
            k + 1
        )));
    }
    // the segments and their qualifications, and the I/O area
    let mut ssas = Vec::new();
    let mut io: Option<Area> = None;
    let opts = &call.options;
    let mut i = 0;
    while i < opts.len() {
        let (name, a) = (&opts[i].0, opts[i].1.as_ref());
        match name.as_str() {
            "SEGMENT" => {
                let seg = a.map(|a| arg_text(m, a)).unwrap_or_default();
                let mut cond = None;
                let mut j = i + 1;
                while j < opts.len() && opts[j].0 != "SEGMENT" {
                    if opts[j].0 == "WHERE" {
                        let text = opts[j]
                            .1
                            .as_ref()
                            .map(|a| arg_text(m, a))
                            .unwrap_or_default();
                        let value = match opts.get(j + 1) {
                            Some((n, Some(ExecArg::Area(v)))) if n == "WHEREVAL" => Some(v.clone()),
                            _ => None,
                        };
                        cond = Some((text, value));
                    }
                    j += 1;
                }
                ssas.push(ssa(
                    m,
                    &seg,
                    cond.as_ref().map(|(t, v)| (t.as_str(), v.as_ref())),
                ));
            }
            "INTO" | "FROM" => io = area(a),
            _ => {}
        }
        i += 1;
    }
    let refs: Vec<&[u8]> = ssas.iter().map(Vec::as_slice).collect();
    let mut pcb = match &masks {
        Masks::Memory(addrs) => {
            let a = addrs[k];
            let n = sched.mask_len(k);
            m.storage()[a..a + n].to_vec()
        }
        Masks::Own(v) => v[k].clone(),
    };
    let mut buf = match &io {
        Some(a) => m.storage()[a.addr..a.addr + a.len].to_vec(),
        None => Vec::new(),
    };
    sched
        .call(k, function, &mut pcb, &mut buf, &refs, None)
        .map_err(RunError)?;
    // DLET / REPL of a segment named on the current path (a parent whose
    // dependents were read since): it is held again, then changed
    if matches!(function, "DLET" | "REPL")
        && text_of(&pcb[10..12]) == "DJ"
        && let Some(last) = ssas.last()
        && sched
            .hold_path(k, &text_of(&last[..8]), &mut pcb)
            .map_err(RunError)?
    {
        sched
            .call(k, function, &mut pcb, &mut buf, &refs, None)
            .map_err(RunError)?;
    }
    if let Some(a) = &io {
        m.storage_mut()[a.addr..a.addr + a.len].copy_from_slice(&buf);
    }
    let status = text_of(&pcb[10..12]);
    let org = match sched.pcbs[k] {
        PcbKind::Db(i) => sched.psb.pcbs[i]
            .dbd_name
            .as_deref()
            .and_then(|d| sched.dbds.get(d))
            .map(|d| d.access.clone())
            .unwrap_or_default(),
        _ => String::new(),
    };
    set_dib(m, call, &status, Some((&pcb, &org)));
    match masks {
        Masks::Memory(addrs) => {
            let a = addrs[k];
            m.storage_mut()[a..a + pcb.len()].copy_from_slice(&pcb);
        }
        Masks::Own(v) => v[k] = pcb,
    }
    Ok(())
}

/// The EXEC interface of a batch program under DFSRRC00: EXEC DLI on the
/// scheduled PSB; EXEC SQL goes to `sql` (Db2).
pub struct BatchHost {
    pub sql: Option<std::rc::Rc<dyn ExecHost>>,
}

impl ExecHost for BatchHost {
    fn exec(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        if call.kind != ExecKind::Dli {
            return match &self.sql {
                Some(h) => h.exec(m, call),
                None => Err(RunError(format!(
                    "{}: EXEC {:?} {} needs its subsystem (line {})",
                    call.program, call.kind, call.command, call.line
                ))),
            };
        }
        let cmd = call.command.to_ascii_uppercase();
        let r = BATCH.with(|b| {
            let mut b = b.borrow_mut();
            let b = b
                .as_mut()
                .ok_or_else(|| RunError("EXEC DLI: no PSB is scheduled".into()))?;
            match cmd.as_str() {
                // the PSB of the region is scheduled for the program
                "SCHD" => {
                    set_dib(m, call, "", None);
                    Ok(())
                }
                "TERM" => {
                    b.sched.commit(None).map_err(RunError)?;
                    set_dib(m, call, "", None);
                    Ok(())
                }
                _ => {
                    let addrs = b.addrs.clone();
                    exec_command(m, call, &mut b.sched, Masks::Memory(&addrs))
                }
            }
        });
        r.map(|()| ExecOutcome::Continue)
    }
}

/// EXEC DLI of a CICS task: SCHD schedules a PSB (its PCB masks are kept
/// by the run time), TERM and the task's sync points end its unit of
/// work.
pub struct TaskDli {
    ims: Ims,
    sched: RefCell<Option<(Schedule, Vec<Vec<u8>>)>>,
}

impl TaskDli {
    pub fn new(ims: Ims) -> TaskDli {
        TaskDli {
            ims,
            sched: RefCell::new(None),
        }
    }

    pub fn exec(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let cmd = call.command.to_ascii_uppercase();
        let mut slot = self.sched.borrow_mut();
        match cmd.as_str() {
            "SCHD" => {
                if slot.is_some() {
                    // the PSB is scheduled already
                    set_dib(m, call, "TC", None);
                } else {
                    let name = call.arg("PSB").map(|a| arg_text(m, a)).unwrap_or_default();
                    let name = name
                        .trim_matches(|c| c == '(' || c == ')' || c == ' ')
                        .to_string();
                    match Schedule::new(&self.ims, &name, false, &HashMap::new()) {
                        Ok(s) => {
                            let masks = (0..s.pcbs.len()).map(|k| s.mask(k)).collect();
                            *slot = Some((s, masks));
                            set_dib(m, call, "", None);
                        }
                        // the PSB cannot be scheduled
                        Err(_) => set_dib(m, call, "TE", None),
                    }
                }
            }
            "TERM" => {
                if let Some((mut s, _)) = slot.take() {
                    s.commit(None).map_err(RunError)?;
                }
                set_dib(m, call, "", None);
            }
            _ => {
                let Some((s, masks)) = slot.as_mut() else {
                    // no PSB scheduled
                    set_dib(m, call, "TA", None);
                    return Ok(ExecOutcome::Continue);
                };
                exec_command(m, call, s, Masks::Own(masks))?;
            }
        }
        Ok(ExecOutcome::Continue)
    }

    /// Ends the unit of work (SYNCPOINT, end of task): the PSB stays
    /// scheduled after a sync point, and is released at the end of the task.
    pub fn syncpoint(&self, commit: bool, end_of_task: bool) -> Result<(), String> {
        let mut slot = self.sched.borrow_mut();
        if let Some((s, _)) = slot.as_mut() {
            if commit {
                s.commit(None)?;
            } else {
                s.rollback();
            }
        }
        if end_of_task {
            *slot = None;
        }
        Ok(())
    }
}

/// Reloads database `dbd` from a DFSURGU0 unload (DFSURGL0): its
/// segments are inserted in one unit of work. The number of segments.
pub fn reload(ims: &Ims, dbd: &str, unload: &[u8]) -> Result<usize, String> {
    let def = ims
        .library
        .dbd(dbd)
        .ok_or_else(|| format!("DBD {dbd} is not in the DBD library"))?;
    let records = crate::unload::parse_dfsurgu0(unload, def)?;
    let mut tx = ims.database.begin();
    let mut ids = Vec::with_capacity(records.len());
    for r in &records {
        let parent = r.parent_index.map(|i| ids[i]);
        let id = tx
            .insert(
                dbd,
                &r.segment,
                parent,
                r.data.clone(),
                std::time::Duration::from_secs(5),
            )
            .map_err(|e| e.to_string())?;
        ids.push(id);
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(records.len())
}

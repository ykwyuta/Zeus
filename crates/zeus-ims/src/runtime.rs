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
                named.unwrap_or_else(|| dbd.segments.iter().map(|s| s.max_bytes).max().unwrap_or(0))
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
                    status(pcb, "");
                }
                // no message processing in a batch region
                "GU" | "GN" => status(pcb, "QC"),
                other => return Err(format!("DL/I call {other} on the I/O PCB is not supported")),
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

    pub fn rollback(&mut self) {
        self.adapter.rollback();
        for g in self.gsam.values_mut() {
            g.rollback();
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
    let Some(mut b) = BATCH.with(|b| b.borrow_mut().take()) else {
        return Ok(());
    };
    if commit {
        b.sched.commit(None)
    } else {
        b.sched.rollback();
        Ok(())
    }
}

fn be32(b: &[u8]) -> i32 {
    i32::from_be_bytes([b[0], b[1], b[2], b[3]])
}

/// The length of an SSA at `a` (8-byte name, or a qualification up to
/// its closing parenthesis).
fn ssa_len(mem: &[u8], a: usize) -> usize {
    let end = mem.len().min(a + 512);
    if mem.get(a + 8) != Some(&ebcdic::from_latin1(b'(')) {
        return 9.min(end - a);
    }
    // the value may contain any byte: the closing parenthesis after the
    // field name, operator and value is found by the call; give it all
    end - a
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
        let ssa_addrs: Vec<usize> = (start + 3..n).filter_map(arg).collect();
        let ssas: Vec<Vec<u8>> = ssa_addrs
            .iter()
            .map(|&a| mem[a..a + ssa_len(mem, a)].to_vec())
            .collect();
        let refs: Vec<&[u8]> = ssas.iter().map(Vec::as_slice).collect();
        let io_addr = arg(start + 2);
        let gsam = matches!(b.sched.pcbs[k], PcbKind::Gsam(_));
        let io_len = io_addr.map_or(0, |a| {
            b.sched.io_len(k, &function, &refs).min(mem.len() - a)
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

/// Makes CBLTDLI callable by the programs of `m`.
pub fn register(m: &mut zeus_cobol::vm::Machine) {
    m.add_native("CBLTDLI", cbltdli);
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

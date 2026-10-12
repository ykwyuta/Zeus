//! The CICS region: resources and terminals.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::rc::Rc;

use zeus_bms::Mapset;
use zeus_cobol::ebcdic;
use zeus_ds::catalog::Catalog;
use zeus_ds::keyed::KeyedDataset;

use crate::csd::Csd;
use crate::screen::Screen;

/// 3270 attention identifiers.
pub mod aid {
    pub const ENTER: u8 = 0x7D;
    pub const CLEAR: u8 = 0x6D;
    pub const PA1: u8 = 0x6C;
    pub const PA2: u8 = 0x6E;
    pub const PA3: u8 = 0x6B;

    /// AID of PF key `n` (1-24).
    pub fn pf(n: u8) -> u8 {
        const PF: [u8; 24] = [
            0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0x7A, 0x7B, 0x7C, 0xC1, 0xC2,
            0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0x4A, 0x4B, 0x4C,
        ];
        PF[(n.clamp(1, 24) - 1) as usize]
    }

    /// AID of a key name: ENTER, CLEAR, PA1-PA3, PF1-PF24.
    pub fn by_name(name: &str) -> Option<u8> {
        let n = name.to_ascii_uppercase();
        Some(match n.as_str() {
            "ENTER" => ENTER,
            "CLEAR" => CLEAR,
            "PA1" => PA1,
            "PA2" => PA2,
            "PA3" => PA3,
            _ => {
                let k: u8 = n.strip_prefix("PF")?.parse().ok()?;
                if !(1..=24).contains(&k) {
                    return None;
                }
                pf(k)
            }
        })
    }

    /// The HANDLE AID option name of an AID.
    pub fn name(a: u8) -> String {
        match a {
            ENTER => "ENTER".into(),
            CLEAR => "CLEAR".into(),
            PA1 => "PA1".into(),
            PA2 => "PA2".into(),
            PA3 => "PA3".into(),
            _ => match (1..=24).find(|&k| pf(k) == a) {
                Some(k) => format!("PF{k}"),
                None => "ANYKEY".into(),
            },
        }
    }
}

/// A file opened by file control: the keyed data set and which of its
/// keys the file uses (0: primary; 1: the alternate key of a path).
pub struct OpenFile {
    pub dataset: KeyedDataset,
    pub path: PathBuf,
    pub which: usize,
    /// Record length of the cluster (maximum).
    pub max_len: usize,
}

pub struct Region {
    pub applid: String,
    pub sysid: String,
    pub csd: Csd,
    pub catalog: Catalog,
    /// Directories of the compiled programs (load libraries).
    pub loadlibs: Vec<PathBuf>,
    /// Directories holding mapsets: NAME.json (zbms --map) or NAME.bms.
    pub mapset_dirs: Vec<PathBuf>,
    mapsets: HashMap<String, Rc<Mapset>>,
    /// Temporary storage queues.
    pub ts: BTreeMap<String, Vec<Vec<u8>>>,
    /// Transient data queues (destination -> records written).
    pub td: BTreeMap<String, Vec<Vec<u8>>>,
    /// Data sets in use by file control, by data set name.
    pub files: HashMap<String, OpenFile>,
    pub task_count: u32,
    /// Messages of the region (abends, ...).
    pub log: Vec<String>,
    /// Transactions started by START, run when due and their terminal is
    /// free.
    pub starts: Vec<StartRequest>,
    /// Terminals of the region by terminal identifier (displays and
    /// printers), for START TERMID.
    pub terminals: HashMap<String, std::rc::Weak<RefCell<Terminal>>>,
    /// Named counters by (pool, name).
    pub counters: BTreeMap<(String, String), Counter>,
    /// Whether the region is attached to Db2 (ZEUS_DB2), for EXEC SQL.
    pub db2: bool,
    /// The IMS system of the region (DBCTL): EXEC DLI of the tasks.
    pub ims: Option<zeus_ims::runtime::Ims>,
    /// Db2 connections not in use by a task (each task has its own unit
    /// of work).
    db2_free: Vec<Rc<zeus_db2::Db2Host>>,
    /// MQ initiation queues the trigger monitor (CKTI) reads.
    pub initqs: Vec<String>,
    last_trigger_poll: Option<std::time::Instant>,
}

pub type Shared<T> = Rc<RefCell<T>>;

impl Region {
    /// A Db2 connection for a task (a free one, or a new one).
    pub fn db2_acquire(&mut self) -> Option<Rc<zeus_db2::Db2Host>> {
        if !self.db2 {
            return None;
        }
        self.db2_free
            .pop()
            .or_else(|| zeus_db2::Db2Host::from_env().map(Rc::new))
    }

    /// Gives back a connection whose unit of work has ended.
    pub fn db2_release(&mut self, h: Rc<zeus_db2::Db2Host>) {
        self.db2_free.push(h);
    }
}

/// A START request.
#[derive(Debug, Clone)]
pub struct StartRequest {
    pub transid: String,
    pub termid: Option<String>,
    pub data: Option<Vec<u8>>,
    pub reqid: Option<String>,
    pub due: std::time::Instant,
}

/// A named counter: the next value to assign and its range.
#[derive(Debug, Clone)]
pub struct Counter {
    pub value: i128,
    pub min: i128,
    pub max: i128,
}

impl Region {
    pub fn new(
        csd: Csd,
        catalog: Catalog,
        loadlibs: Vec<PathBuf>,
        mapset_dirs: Vec<PathBuf>,
    ) -> Region {
        Region {
            applid: "ZEUSCICS".into(),
            sysid: "ZEUS".into(),
            csd,
            catalog,
            loadlibs,
            mapset_dirs,
            mapsets: HashMap::new(),
            ts: BTreeMap::new(),
            td: BTreeMap::new(),
            files: HashMap::new(),
            task_count: 0,
            counters: BTreeMap::new(),
            starts: Vec::new(),
            terminals: HashMap::new(),
            log: Vec::new(),
            db2: std::env::var_os("ZEUS_DB2").is_some(),
            ims: None,
            db2_free: Vec::new(),
            initqs: Vec::new(),
            last_trigger_poll: None,
        }
    }

    /// A mapset by name (loaded on first use).
    pub fn mapset(&mut self, name: &str) -> Option<Rc<Mapset>> {
        let name = name.trim().to_ascii_uppercase();
        if let Some(m) = self.mapsets.get(&name) {
            return Some(m.clone());
        }
        for dir in &self.mapset_dirs {
            for (file, json) in [
                (format!("{name}.json"), true),
                (format!("{name}.bms"), false),
            ] {
                let p = dir.join(&file);
                let Ok(text) = std::fs::read(&p) else {
                    continue;
                };
                let ms = if json {
                    serde_json::from_slice(&text).ok()
                } else {
                    zeus_bms::parse(&String::from_utf8_lossy(&text)).ok()
                };
                if let Some(ms) = ms {
                    let ms = Rc::new(ms);
                    self.mapsets.insert(name.clone(), ms.clone());
                    return Some(ms);
                }
            }
        }
        None
    }
}

/// What happened when the operator pressed an attention key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// No transaction was started (CLEAR on an idle terminal, ...).
    Idle,
    Ended {
        transid: String,
    },
    Abended {
        transid: String,
        code: String,
    },
}

pub struct Terminal {
    pub id: String,
    pub screen: Screen,
    pub userid: String,
    /// Transaction and COMMAREA named by the last RETURN TRANSID.
    pub next: Option<(String, Option<Vec<u8>>)>,
    /// Waits for the next input of the terminal operator during a task (a
    /// conversational RECEIVE): it updates the screen and returns the
    /// attention key, None when no more input comes.
    pub converse: Option<Converse>,
    /// The terminal model: alternate screen size and extended attributes.
    pub model: Model,
    /// Records for the terminal outside the screen (structured fields of
    /// SEND STRFIELD), sent before the next screen.
    pub outbound: Vec<Vec<u8>>,
    /// Structured fields the terminal sent (AID X'88') for RECEIVE.
    pub inbound_sf: Option<Vec<u8>>,
    /// A printer: what tasks send to it is printed (pages in `printed`).
    pub printer: bool,
    /// Pages printed and not yet delivered: lines of EBCDIC text.
    pub printed: Vec<Vec<Vec<u8>>>,
    /// A task runs at the terminal.
    pub busy: bool,
    /// A file transfer (IND$FILE) is in progress: only structured fields
    /// go to the terminal.
    pub transfer: bool,
    /// Input is translated to upper case (SET TERMINAL UCTRANST).
    pub uctran: bool,
    /// The next transaction (RETURN TRANSID IMMEDIATE) starts at once.
    pub immediate: bool,
}

impl Terminal {
    /// A printer terminal.
    pub fn new_printer(id: &str) -> Terminal {
        let mut t = Terminal::new(id);
        t.printer = true;
        t
    }

    /// Prints the buffer of a printer: its lines (trailing blanks and
    /// empty lines at the end dropped) become a page.
    pub fn print_buffer(&mut self) {
        let s = &self.screen;
        let mut lines: Vec<Vec<u8>> = (0..s.rows)
            .map(|r| {
                let mut l: Vec<u8> = (0..s.cols)
                    .map(|c| {
                        let p = r * s.cols + c;
                        match (s.attrs[p], s.chars[p]) {
                            (Some(_), _) | (None, 0) => ebcdic::SPACE,
                            (None, ch) => ch,
                        }
                    })
                    .collect();
                while l.last() == Some(&ebcdic::SPACE) {
                    l.pop();
                }
                l
            })
            .collect();
        while lines.last().is_some_and(Vec::is_empty) {
            lines.pop();
        }
        if !lines.is_empty() {
            self.printed.push(lines);
        }
        self.screen.erase();
    }
}

/// What a terminal can do (ASSIGN ALTSCRNHT, COLOR, ...).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    pub alternate: (usize, usize),
    pub extended: bool,
    pub color: bool,
    pub hilight: bool,
    /// Double-byte characters (SO / SI).
    pub dbcs: bool,
}

impl Default for Model {
    fn default() -> Model {
        Model {
            alternate: (24, 80),
            extended: true,
            color: true,
            hilight: true,
            // a script terminal shows what the code page has
            dbcs: zeus_cobol::codepage::active().is_mixed(),
        }
    }
}

/// The default screen size (Erase/Write).
pub const DEFAULT_SIZE: (usize, usize) = (24, 80);

pub type Converse = Box<dyn FnMut(&Shared<Terminal>) -> Option<u8>>;

impl Terminal {
    /// The screen of a newly connected terminal: an input field on the
    /// first line and a prompt on the last.
    pub fn good_morning(&mut self) {
        use crate::screen::Attr;
        let s = &mut self.screen;
        s.erase();
        s.set_attr(0, Attr::default());
        let last = s.addr(s.rows, 1);
        s.set_attr(
            last - 1,
            Attr {
                protected: true,
                numeric: true,
                color: 0xF5,
                ..Attr::default()
            },
        );
        s.write(
            last,
            &ebcdic::encode(
                "Zeus CICS - type a transaction identifier (CLEAR clears) and press ENTER",
            ),
        );
        s.cursor = 1;
    }

    pub fn new(id: &str) -> Terminal {
        Terminal {
            id: id.to_string(),
            screen: Screen::new(24, 80),
            userid: "ZEUSUSER".into(),
            next: None,
            converse: None,
            model: Model::default(),
            outbound: Vec::new(),
            inbound_sf: None,
            printer: false,
            printed: Vec::new(),
            busy: false,
            transfer: false,
            uctran: false,
            immediate: false,
        }
    }
}

/// The operator pressed attention key `aid` at terminal `term`.
pub fn attention(region: &Shared<Region>, term: &Shared<Terminal>, aid: u8) -> Event {
    if aid == aid::CLEAR {
        // CLEAR erases the screen and gives it the default size
        let (r, c) = DEFAULT_SIZE;
        term.borrow_mut().screen.erase_to(r, c);
    }
    let next = term.borrow_mut().next.take();
    let (transid, commarea) = match next {
        Some(n) => n,
        None => {
            if aid == aid::CLEAR {
                term.borrow_mut().screen.erase();
                return Event::Idle;
            }
            // the transaction identifier starts the input: the unformatted
            // screen, or the first modified field of a formatted one
            let input = {
                let t = term.borrow();
                if t.screen.fields().is_empty() {
                    t.screen.unformatted_input()
                } else {
                    t.screen
                        .modified_fields()
                        .into_iter()
                        .next()
                        .map(|(_, d)| d)
                        .unwrap_or_default()
                }
            };
            let text: String = ebcdic::decode_latin1(&input)
                .iter()
                .map(|&b| b as char)
                .collect();
            let t: String = text
                .trim_start()
                .chars()
                .take_while(|c| !c.is_whitespace())
                .take(4)
                .collect::<String>()
                .to_ascii_uppercase();
            if t.is_empty() {
                return Event::Idle;
            }
            // file transfer from the emulator (DFT)
            let command = text.trim();
            if command.to_ascii_uppercase().starts_with("IND$FILE ") {
                let msg = crate::indfile::run(region, term, command);
                let mut t = term.borrow_mut();
                t.screen.erase();
                t.screen.write(0, &ebcdic::encode(&msg));
                region.borrow_mut().log.push(format!("IND$FILE: {msg}"));
                return Event::Ended {
                    transid: "IND$".into(),
                };
            }
            (t, None)
        }
    };
    if !region.borrow().csd.transactions.contains_key(&transid) {
        let mut t = term.borrow_mut();
        t.screen.erase();
        let msg = format!("DFHAC2001 Transaction '{transid}' is not recognized.");
        t.screen.write(0, &ebcdic::encode(&msg));
        return Event::Idle;
    }
    let (mut transid, mut commarea) = (transid, commarea);
    loop {
        match crate::task::run(region, term, &transid, aid, commarea) {
            Ok(()) => {
                // RETURN TRANSID IMMEDIATE: the next transaction starts
                // without waiting for input
                let next = {
                    let mut t = term.borrow_mut();
                    if std::mem::take(&mut t.immediate) {
                        t.next.take()
                    } else {
                        None
                    }
                };
                match next {
                    Some((n, c)) if region.borrow().csd.transactions.contains_key(&n) => {
                        transid = n;
                        commarea = c;
                        continue;
                    }
                    _ => return Event::Ended { transid },
                }
            }
            Err(code) => return abended(term, transid, code),
        }
    }
}

/// A transaction ended abnormally: its message on the terminal.
fn abended(term: &Shared<Terminal>, transid: String, code: String) -> Event {
    let mut t = term.borrow_mut();
    t.screen.erase();
    t.next = None;
    t.immediate = false;
    let msg = format!("DFHAC2206 Transaction {transid} failed with abend {code}.");
    t.screen.write(0, &ebcdic::encode(&msg));
    Event::Abended { transid, code }
}

/// Runs the started transactions that are due and whose terminal (if any)
/// is free; their events, with the terminal identifier.
/// The trigger monitor (CKTI): each trigger message on the initiation
/// queues starts the transaction its ApplId names, with the message as the
/// data it RETRIEVEs (at most every half second).
fn poll_triggers(region: &Shared<Region>) {
    let queues = {
        let mut r = region.borrow_mut();
        let now = std::time::Instant::now();
        if r.initqs.is_empty()
            || r.last_trigger_poll
                .is_some_and(|t| now.duration_since(t) < std::time::Duration::from_millis(500))
        {
            return;
        }
        r.last_trigger_poll = Some(now);
        r.initqs.clone()
    };
    for q in queues {
        let msgs = match zeus_mq::monitor::trigger_messages(&q) {
            Ok(m) => m,
            Err(e) => {
                region.borrow_mut().log.push(format!("CKTI {q}: {e}"));
                continue;
            }
        };
        for m in msgs {
            let transid = zeus_mq::monitor::transaction(&m);
            let mut r = region.borrow_mut();
            if !r.csd.transactions.contains_key(&transid) {
                r.log.push(format!(
                    "CKTI {q}: trigger for {} names transaction {transid:?}, not defined",
                    zeus_mq::monitor::queue(&m)
                ));
                continue;
            }
            r.log.push(format!(
                "CKTI {q}: queue {} starts {transid}",
                zeus_mq::monitor::queue(&m)
            ));
            r.starts.push(StartRequest {
                transid,
                termid: None,
                data: Some(m),
                reqid: None,
                due: std::time::Instant::now(),
            });
        }
    }
}

pub fn run_due(region: &Shared<Region>) -> Vec<(String, Event)> {
    poll_triggers(region);
    let mut events = Vec::new();
    loop {
        let now = std::time::Instant::now();
        let pick = {
            let r = region.borrow();
            r.starts.iter().position(|s| {
                s.due <= now
                    && match &s.termid {
                        None => true,
                        Some(t) => r
                            .terminals
                            .get(t)
                            .and_then(std::rc::Weak::upgrade)
                            .is_some_and(|t| !t.borrow().busy),
                    }
            })
        };
        let Some(i) = pick else { break };
        let s = region.borrow_mut().starts.remove(i);
        let term = match &s.termid {
            Some(t) => region.borrow().terminals[t].upgrade().unwrap(),
            // no terminal: the task has no principal facility
            None => Rc::new(RefCell::new(Terminal::new(""))),
        };
        if !region.borrow().csd.transactions.contains_key(&s.transid) {
            continue;
        }
        let e = match crate::task::run_started(region, &term, &s.transid, s.data) {
            Ok(()) => Event::Ended {
                transid: s.transid.clone(),
            },
            Err(code) => Event::Abended {
                transid: s.transid.clone(),
                code,
            },
        };
        events.push((s.termid.unwrap_or_default(), e));
    }
    events
}

/// When the next started transaction is due.
pub fn next_due(region: &Shared<Region>) -> Option<std::time::Instant> {
    region.borrow().starts.iter().map(|s| s.due).min()
}

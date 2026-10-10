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
}

pub type Shared<T> = Rc<RefCell<T>>;

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
            log: Vec::new(),
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
}

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
        }
    }
}

/// The operator pressed attention key `aid` at terminal `term`.
pub fn attention(region: &Shared<Region>, term: &Shared<Terminal>, aid: u8) -> Event {
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
    match crate::task::run(region, term, &transid, aid, commarea) {
        Ok(()) => Event::Ended { transid },
        Err(code) => {
            let mut t = term.borrow_mut();
            t.screen.erase();
            t.next = None;
            let msg = format!("DFHAC2206 Transaction {transid} failed with abend {code}.");
            t.screen.write(0, &ebcdic::encode(&msg));
            Event::Abended { transid, code }
        }
    }
}

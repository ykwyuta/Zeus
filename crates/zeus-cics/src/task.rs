//! A CICS task: runs a transaction's programs on the zcobc VM and executes
//! their EXEC CICS commands.
//!
//! Conditions: with RESP the response goes to the program; with NOHANDLE
//! or an IGNORE CONDITION it is ignored; with a HANDLE CONDITION label
//! (or the ERROR label) control goes there; otherwise the task abends.
//! There is no backout: changes to files are written when made, and
//! SYNCPOINT ROLLBACK does not undo them.

use std::collections::HashMap;
use std::rc::Rc;

use zeus_cobol::ebcdic;
use zeus_cobol::vm::exec::{Area, ExecArg, ExecCall, ExecHost, ExecOutcome, ProgramEnd};
use zeus_cobol::vm::{Machine, RResult, RunError, data};
use zeus_cobol::zir::DataType;
use zeus_decimal::Dec;
use zeus_ds::keyed::{KeyedDataset, Position};

use crate::bms::{self, SendMode};
use crate::region::{self, OpenFile, Region, Shared, Terminal, aid};
use crate::time;

/// EIB field offsets (the layout of DFHEIBLK).
mod eib {
    pub const TIME: usize = 0;
    pub const DATE: usize = 4;
    pub const TRNID: usize = 8;
    pub const TASKN: usize = 12;
    pub const TRMID: usize = 16;
    pub const CPOSN: usize = 22;
    pub const CALEN: usize = 24;
    pub const AID: usize = 26;
    pub const DS: usize = 35;
    pub const RSRCE: usize = 51;
    pub const RESP: usize = 76;
    pub const RESP2: usize = 80;
    pub const LEN: usize = 85;
}

/// Prefix of the VM error that abends a task.
const ABEND: &str = "\u{0}abend ";

fn abend(code: &str) -> RunError {
    RunError(format!("{ABEND}{code}"))
}

/// Abend code for a condition that is not handled.
fn abend_code(cond: &str) -> String {
    match cond {
        "PGMIDERR" => "AEI0",
        "MAPFAIL" => "AEI9",
        "NOTFND" => "AEIM",
        "DUPREC" => "AEIN",
        "INVREQ" => "AEIP",
        "LENGERR" => "AEIV",
        "FILENOTFOUND" => "AEIL",
        "QIDERR" => "AEYH",
        _ => "AEIZ",
    }
    .to_string()
}

/// Condition name -> (EIBRESP).
fn resp_value(cond: &str) -> i32 {
    zeus_cobol::front::cics::resp_value(cond).unwrap_or(1) as i32
}

#[derive(Default)]
struct Handlers {
    /// Condition -> label (None: IGNORE CONDITION).
    cond: HashMap<String, Option<u32>>,
    aid: HashMap<String, u32>,
    abend: Option<u32>,
}

struct Browse {
    which: usize,
    pos: Option<Position>,
    /// The next READNEXT returns the record at `pos` itself.
    inclusive: bool,
    /// The browse ended (ENDFILE reached) in this direction.
    started: bool,
}

struct State {
    transid: String,
    taskn: u32,
    aid: u8,
    eib: usize,
    /// Programs in LINK order.
    programs: Vec<String>,
    handlers: HashMap<usize, Handlers>,
    xctl: Option<(String, Option<(usize, usize)>)>,
    /// The terminal input that started the task has not been read nor
    /// overwritten by a SEND; otherwise RECEIVE waits for new input.
    input_pending: bool,
    browses: HashMap<String, Browse>,
    /// Primary key of the record each file last read for UPDATE.
    updates: HashMap<String, Vec<u8>>,
    /// Changes to recoverable files in the unit of work: data set, primary
    /// key and the record before (None: it was added).
    undo: Vec<(String, Vec<u8>, Option<Vec<u8>>)>,
    abcode: String,
}

pub struct Task {
    region: Shared<Region>,
    term: Shared<Terminal>,
    st: std::cell::RefCell<State>,
    /// The task's Db2 connection, taken at its first SQL statement.
    db2: std::cell::RefCell<Option<Rc<zeus_db2::Db2Host>>>,
}

/// Runs transaction `transid` for an attention key at a terminal; the
/// error is the abend code.
pub fn run(
    region: &Shared<Region>,
    term: &Shared<Terminal>,
    transid: &str,
    aid_byte: u8,
    commarea: Option<Vec<u8>>,
) -> Result<(), String> {
    let program = region.borrow().csd.transactions[transid].clone();
    let loadlibs = region.borrow().loadlibs.clone();
    let taskn = {
        let mut r = region.borrow_mut();
        r.task_count += 1;
        r.task_count
    };
    let mut m = Machine::new(loadlibs);
    // DISPLAY output of the programs goes to the region log (CEEOUT)
    let out = Capture::default();
    m.set_stdout(Box::new(out.clone()));
    let eib_addr = m.getmain(eib::LEN, 0);
    let task = Rc::new(Task {
        region: region.clone(),
        term: term.clone(),
        st: std::cell::RefCell::new(State {
            transid: transid.to_string(),
            taskn,
            aid: aid_byte,
            eib: eib_addr,
            programs: Vec::new(),
            handlers: HashMap::new(),
            xctl: None,
            input_pending: true,
            browses: HashMap::new(),
            updates: HashMap::new(),
            undo: Vec::new(),
            abcode: String::new(),
        }),
        db2: std::cell::RefCell::new(None),
    });
    m.set_exec_host(task.clone());
    task.init_eib(&mut m);
    let ca = commarea.map(|c| {
        let a = m.getmain(c.len(), 0);
        m.storage_mut()[a..a + c.len()].copy_from_slice(&c);
        (a, c.len())
    });
    let r = task.run_level(&mut m, &program, ca, true);
    term.borrow_mut().screen.locked = false;
    // end of task: the unit of work is committed, or backed out on abend
    let r = match task.end_uow(r.is_ok()) {
        Ok(()) => r,
        Err(e) => r.and(Err(e)),
    };
    if let Some(h) = task.db2.borrow_mut().take() {
        region.borrow_mut().db2_release(h);
    }
    for line in String::from_utf8_lossy(&out.0.borrow()).lines() {
        region
            .borrow_mut()
            .log
            .push(format!("CEEOUT {transid} {}", line.trim_end()));
    }
    match r {
        Ok(()) => Ok(()),
        Err(e) => {
            let code = match e.0.strip_prefix(ABEND) {
                Some(c) => c.to_string(),
                None => {
                    region
                        .borrow_mut()
                        .log
                        .push(format!("task {taskn} {transid}: {}", e.0));
                    "ASRA".to_string()
                }
            };
            region
                .borrow_mut()
                .log
                .push(format!("task {taskn} {transid} abended {code}"));
            Err(code)
        }
    }
}

/// Collects what programs DISPLAY.
#[derive(Clone, Default)]
struct Capture(Rc<std::cell::RefCell<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn put_packed(mem: &mut [u8], at: usize, len: usize, v: i64) {
    zeus_decimal::packed::store(&mut mem[at..at + len], Dec::from_i64(v), true);
}

fn put_text(mem: &mut [u8], at: usize, len: usize, s: &str) {
    let mut b = ebcdic::encode(s);
    b.resize(len, ebcdic::SPACE);
    mem[at..at + len].copy_from_slice(&b[..len]);
}

impl Task {
    fn init_eib(&self, m: &mut Machine) {
        let st = self.st.borrow();
        let e = st.eib;
        let (y, _, _, doy, h, mi, s, _) = time::split(time::abstime_now());
        let term_id = self.term.borrow().id.clone();
        let cursor = self.term.borrow().screen.cursor;
        let mem = m.storage_mut();
        put_packed(mem, e + eib::TIME, 4, h * 10000 + mi * 100 + s);
        put_packed(
            mem,
            e + eib::DATE,
            4,
            (y - 1900) / 100 * 100000 + y % 100 * 1000 + doy,
        );
        put_text(mem, e + eib::TRNID, 4, &st.transid);
        put_packed(mem, e + eib::TASKN, 4, st.taskn as i64);
        put_text(mem, e + eib::TRMID, 4, &term_id);
        mem[e + eib::CPOSN..e + eib::CPOSN + 2].copy_from_slice(&(cursor as u16).to_be_bytes());
        mem[e + eib::AID] = st.aid;
    }

    /// Runs a program and the programs it transfers control to (XCTL) at
    /// one logical level.
    fn run_level(
        &self,
        m: &mut Machine,
        program: &str,
        mut ca: Option<(usize, usize)>,
        top: bool,
    ) -> RResult<()> {
        let mut program = program.to_string();
        let eib_addr = self.st.borrow().eib;
        loop {
            let calen = ca.map_or(0, |c| c.1) as u16;
            m.storage_mut()[eib_addr + eib::CALEN..eib_addr + eib::CALEN + 2]
                .copy_from_slice(&calen.to_be_bytes());
            self.st.borrow_mut().programs.push(program.clone());
            let r = m.run_program(&program, &[Some(eib_addr), ca.map(|c| c.0)]);
            self.st.borrow_mut().programs.pop();
            match r? {
                None => {
                    return Err(if top {
                        abend("APCT")
                    } else {
                        RunError(format!("{ABEND}PGMIDERR"))
                    });
                }
                Some(ProgramEnd::StopRun(_)) | Some(ProgramEnd::Returned) => {}
            }
            match self.st.borrow_mut().xctl.take() {
                Some((p, c)) => {
                    program = p;
                    ca = c;
                }
                None => return Ok(()),
            }
        }
    }

    // ---- operands -------------------------------------------------------

    fn bytes(m: &Machine, a: &ExecArg) -> Vec<u8> {
        match a {
            ExecArg::Area(x) => m.storage()[x.addr..x.addr + x.len].to_vec(),
            ExecArg::Alnum(b) => b.clone(),
            ExecArg::Num(d) => ebcdic::encode(&d.to_string()),
            ExecArg::Name(s) => ebcdic::encode(s),
            ExecArg::Pointer { addr, .. } => (*addr as u32).to_be_bytes().to_vec(),
            ExecArg::Label(_) => Vec::new(),
        }
    }

    fn text(m: &Machine, a: &ExecArg) -> String {
        let b = Self::bytes(m, a);
        ebcdic::decode_latin1(&b)
            .iter()
            .map(|&c| c as char)
            .collect::<String>()
            .trim()
            .to_ascii_uppercase()
    }

    fn num(m: &Machine, a: &ExecArg) -> i64 {
        let d = match a {
            ExecArg::Area(x) => match &x.ty {
                DataType::Pointer | DataType::Index => {
                    let b = &m.storage()[x.addr..x.addr + 4];
                    return i32::from_be_bytes([b[0], b[1], b[2], b[3]]) as i64;
                }
                ty => data::value_of(ty, &m.storage()[x.addr..x.addr + x.len]),
            },
            ExecArg::Num(d) => *d,
            ExecArg::Pointer { addr, .. } => return *addr as i64,
            other => data::alnum_as_number(&Self::bytes(m, other)),
        };
        d.rescale(0, zeus_decimal::Rounding::Truncate)
            .map_or(0, |d| d.coef as i64)
    }

    fn store_num(m: &mut Machine, a: &Area, v: i64) {
        let mem = &mut m.storage_mut()[a.addr..a.addr + a.len];
        match &a.ty {
            DataType::Numeric(t) => {
                data::store_num(t, mem, Dec::from_i64(v), false);
            }
            _ => {
                let b = v.to_be_bytes();
                let n = mem.len().min(8);
                let l = mem.len();
                mem[l - n..].copy_from_slice(&b[8 - n..]);
            }
        }
    }

    fn store_bytes(m: &mut Machine, a: &Area, data: &[u8]) {
        let n = data.len().min(a.len);
        m.storage_mut()[a.addr..a.addr + n].copy_from_slice(&data[..n]);
    }

    fn opt_text(m: &Machine, call: &ExecCall, name: &str) -> Option<String> {
        call.arg(name).map(|a| Self::text(m, a))
    }

    fn opt_num(m: &Machine, call: &ExecCall, name: &str) -> Option<i64> {
        call.arg(name).map(|a| Self::num(m, a))
    }

    fn area(call: &ExecCall, name: &str) -> Option<Area> {
        match call.arg(name) {
            Some(ExecArg::Area(a)) => Some(a.clone()),
            _ => None,
        }
    }

    /// Data of a FROM option, LENGTH bytes long when LENGTH is given.
    fn from_data(m: &Machine, call: &ExecCall) -> Vec<u8> {
        let len = Self::opt_num(m, call, "LENGTH").or(Self::opt_num(m, call, "FLENGTH"));
        // FROM names an address: LENGTH bytes from there, past the end of
        // the data item if need be
        if let (Some(ExecArg::Area(a)), Some(l)) = (call.arg("FROM"), len) {
            let mem = m.storage();
            let end = (a.addr + l.max(0) as usize).min(mem.len());
            let mut d = mem[a.addr.min(end)..end].to_vec();
            d.resize(l.max(0) as usize, 0);
            return d;
        }
        let mut d = call
            .arg("FROM")
            .map(|a| Self::bytes(m, a))
            .unwrap_or_default();
        if let Some(l) = len {
            d.resize(l.max(0) as usize, 0);
        }
        d
    }

    /// Moves data into an INTO area and sets LENGTH; LENGERR when the data
    /// is longer than the area (or than LENGTH).
    fn deliver(m: &mut Machine, call: &ExecCall, data: &[u8]) -> &'static str {
        let max = match Self::opt_num(m, call, "LENGTH") {
            Some(l) => l.max(0) as usize,
            None => Self::area(call, "INTO").map_or(data.len(), |a| a.len),
        };
        if let Some(into) = Self::area(call, "INTO") {
            Self::store_bytes(m, &into, &data[..data.len().min(max).min(into.len)]);
        }
        if let Some(l) = Self::area(call, "LENGTH") {
            Self::store_num(m, &l, data.len().min(max) as i64);
        }
        if data.len() > max {
            "LENGERR"
        } else {
            "NORMAL"
        }
    }

    // ---- responses ------------------------------------------------------

    /// Completes a command with condition `cond`.
    fn finish(
        &self,
        m: &mut Machine,
        call: &ExecCall,
        cond: &str,
        resp2: i32,
    ) -> RResult<ExecOutcome> {
        let resp = if cond == "NORMAL" {
            0
        } else {
            resp_value(cond)
        };
        let e = self.st.borrow().eib;
        {
            let mem = m.storage_mut();
            mem[e + eib::RESP..e + eib::RESP + 4].copy_from_slice(&resp.to_be_bytes());
            mem[e + eib::RESP2..e + eib::RESP2 + 4].copy_from_slice(&resp2.to_be_bytes());
        }
        if let Some(a) = Self::area(call, "RESP") {
            Self::store_num(m, &a, resp as i64);
            if let Some(a) = Self::area(call, "RESP2") {
                Self::store_num(m, &a, resp2 as i64);
            }
            return Ok(ExecOutcome::Continue);
        }
        if cond == "NORMAL" || call.has("NOHANDLE") {
            return Ok(ExecOutcome::Continue);
        }
        let st = self.st.borrow();
        if let Some(h) = st.handlers.get(&call.instance) {
            match h.cond.get(cond) {
                Some(Some(l)) => return Ok(ExecOutcome::Goto(*l)),
                Some(None) => return Ok(ExecOutcome::Continue),
                None => {}
            }
            if let Some(Some(l)) = h.cond.get("ERROR") {
                return Ok(ExecOutcome::Goto(*l));
            }
        }
        drop(st);
        self.raise_abend(call, &abend_code(cond))
    }

    /// Abends the task, or goes to the HANDLE ABEND label of the program.
    fn raise_abend(&self, call: &ExecCall, code: &str) -> RResult<ExecOutcome> {
        let mut st = self.st.borrow_mut();
        st.abcode = code.to_string();
        if let Some(l) = st
            .handlers
            .get_mut(&call.instance)
            .and_then(|h| h.abend.take())
        {
            return Ok(ExecOutcome::Goto(l));
        }
        Err(abend(code))
    }

    // ---- commands -------------------------------------------------------

    fn handle(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let mut st = self.st.borrow_mut();
        let h = st.handlers.entry(call.instance).or_default();
        if call.has("ABEND") {
            h.abend = match call.arg("LABEL") {
                Some(ExecArg::Label(l)) => Some(*l),
                _ => None,
            };
        } else if call.has("AID") {
            for (name, arg) in &call.options {
                if name == "AID" {
                    continue;
                }
                match arg {
                    Some(ExecArg::Label(l)) => {
                        h.aid.insert(name.clone(), *l);
                    }
                    _ => {
                        h.aid.remove(name);
                    }
                }
            }
        } else {
            for (name, arg) in &call.options {
                if name == "CONDITION" {
                    continue;
                }
                match arg {
                    Some(ExecArg::Label(l)) => {
                        h.cond.insert(name.clone(), Some(*l));
                    }
                    _ => {
                        h.cond.remove(name);
                    }
                }
            }
        }
        drop(st);
        self.finish(m, call, "NORMAL", 0)
    }

    fn ignore(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let mut st = self.st.borrow_mut();
        let h = st.handlers.entry(call.instance).or_default();
        for (name, _) in &call.options {
            if name != "CONDITION" {
                h.cond.insert(name.clone(), None);
            }
        }
        drop(st);
        self.finish(m, call, "NORMAL", 0)
    }

    fn commarea(m: &Machine, call: &ExecCall) -> Option<(usize, usize)> {
        let a = Self::area(call, "COMMAREA")?;
        let len = Self::opt_num(m, call, "LENGTH").map_or(a.len, |l| l.max(0) as usize);
        Some((a.addr, len))
    }

    fn program_control(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let program = Self::opt_text(m, call, "PROGRAM").unwrap_or_default();
        let ca = Self::commarea(m, call);
        match call.command.as_str() {
            "RETURN" => {
                let top = self.st.borrow().programs.len() == 1;
                if top {
                    let transid = Self::opt_text(m, call, "TRANSID");
                    let data = ca.map(|(a, l)| m.storage()[a..a + l].to_vec());
                    let mut t = self.term.borrow_mut();
                    t.next = transid.map(|t| (t, data));
                }
                self.st.borrow_mut().handlers.remove(&call.instance);
                Ok(ExecOutcome::Return)
            }
            "XCTL" => {
                if !self.program_exists(&program) {
                    return self.finish(m, call, "PGMIDERR", 3);
                }
                // the COMMAREA is copied: the issuing program ends
                let ca = ca.map(|(a, l)| {
                    let d = m.storage()[a..a + l].to_vec();
                    let n = m.getmain(l, 0);
                    m.storage_mut()[n..n + l].copy_from_slice(&d);
                    (n, l)
                });
                let mut st = self.st.borrow_mut();
                st.handlers.remove(&call.instance);
                st.xctl = Some((program, ca));
                Ok(ExecOutcome::Return)
            }
            _ => {
                // LINK
                if !self.program_exists(&program) {
                    return self.finish(m, call, "PGMIDERR", 3);
                }
                let e = self.st.borrow().eib;
                let saved = m.storage()[e + eib::CALEN..e + eib::CALEN + 2].to_vec();
                self.run_level(m, &program, ca, false)?;
                m.storage_mut()[e + eib::CALEN..e + eib::CALEN + 2].copy_from_slice(&saved);
                self.finish(m, call, "NORMAL", 0)
            }
        }
    }

    fn program_exists(&self, name: &str) -> bool {
        let r = self.region.borrow();
        r.loadlibs
            .iter()
            .any(|d| d.join(format!("{name}.zo")).is_file() || d.join(name).is_file())
    }

    // ---- terminal control and BMS ---------------------------------------

    fn send(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        self.st.borrow_mut().input_pending = false;
        let mut t = self.term.borrow_mut();
        let screen = &mut t.screen;
        if call.has("MAP") {
            let map_name = Self::opt_text(m, call, "MAP").unwrap_or_default();
            let set = Self::opt_text(m, call, "MAPSET").unwrap_or_else(|| map_name.clone());
            let Some(ms) = self.region.borrow_mut().mapset(&set) else {
                drop(t);
                return self.finish(m, call, "INVREQ", 0);
            };
            let Some(map) = ms.maps.iter().find(|x| x.name == map_name) else {
                drop(t);
                return self.finish(m, call, "INVREQ", 0);
            };
            let data = call.arg("FROM").map(|a| Self::bytes(m, a));
            let mode = if call.has("MAPONLY") {
                SendMode::MapOnly
            } else if call.has("DATAONLY") {
                SendMode::DataOnly
            } else {
                SendMode::Both
            };
            let cursor = if call.has("CURSOR") {
                Some(Self::opt_num(m, call, "CURSOR").map(|c| c.max(0) as usize))
            } else {
                None
            };
            bms::send_map(
                screen,
                map,
                data.as_deref(),
                mode,
                call.has("ERASE"),
                cursor,
            );
        } else if call.has("TEXT") {
            let data = Self::from_data(m, call);
            if call.has("ERASE") {
                screen.erase();
            }
            let cols = screen.cols;
            let mut pos = 0;
            for line in data.split(|&b| b == 0x15) {
                for chunk in line.chunks(cols) {
                    screen.write(pos, chunk);
                    pos = (pos / cols + 1) * cols % screen.size();
                }
            }
        } else if call.has("CONTROL") {
            if call.has("ERASE") {
                screen.erase();
            }
            if let Some(c) = Self::opt_num(m, call, "CURSOR") {
                screen.cursor = c.max(0) as usize % screen.size();
            }
        } else {
            // SEND FROM: unformatted output
            let data = Self::from_data(m, call);
            if call.has("ERASE") {
                screen.erase();
            }
            screen.write(0, &data);
        }
        if call.has("FREEKB") {
            screen.locked = false;
        }
        if call.has("ALARM") {
            screen.alarm = true;
        }
        drop(t);
        self.finish(m, call, "NORMAL", 0)
    }

    fn receive(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        if !self.st.borrow().input_pending {
            // conversational: wait for the operator's next input
            let hook = self.term.borrow_mut().converse.take();
            let Some(mut hook) = hook else {
                return Err(RunError("RECEIVE: no terminal input can come".into()));
            };
            let got = hook(&self.term);
            self.term.borrow_mut().converse = Some(hook);
            let Some(a) = got else {
                return Err(RunError("RECEIVE: the terminal input ended".into()));
            };
            self.st.borrow_mut().aid = a;
            self.init_eib(m);
        }
        self.st.borrow_mut().input_pending = false;
        let aid_byte = self.st.borrow().aid;
        // HANDLE AID
        if !call.has("RESP") && !call.has("NOHANDLE") {
            let st = self.st.borrow();
            if let Some(h) = st.handlers.get(&call.instance) {
                let name = aid::name(aid_byte);
                let l = h.aid.get(&name).or_else(|| {
                    if aid_byte != aid::ENTER {
                        h.aid.get("ANYKEY")
                    } else {
                        None
                    }
                });
                if let Some(&l) = l {
                    return Ok(ExecOutcome::Goto(l));
                }
            }
        }
        if call.has("MAP") {
            let map_name = Self::opt_text(m, call, "MAP").unwrap_or_default();
            let set = Self::opt_text(m, call, "MAPSET").unwrap_or_else(|| map_name.clone());
            let Some(ms) = self.region.borrow_mut().mapset(&set) else {
                return self.finish(m, call, "INVREQ", 0);
            };
            let Some(map) = ms.maps.iter().find(|x| x.name == map_name) else {
                return self.finish(m, call, "INVREQ", 0);
            };
            let short_read = matches!(aid_byte, aid::CLEAR | aid::PA1 | aid::PA2 | aid::PA3);
            let data = if short_read {
                None
            } else {
                bms::receive_map(&self.term.borrow().screen, map)
            };
            let Some(data) = data else {
                return self.finish(m, call, "MAPFAIL", 0);
            };
            if let Some(into) = Self::area(call, "INTO") {
                Self::store_bytes(m, &into, &data);
            }
            return self.finish(m, call, "NORMAL", 0);
        }
        // RECEIVE INTO: the input data stream after the AID and cursor
        // address: the data of an unformatted screen, SBA and data of each
        // modified field of a formatted one
        let input = {
            let t = self.term.borrow();
            if t.screen.fields().is_empty() {
                t.screen.unformatted_input()
            } else {
                let mut d = Vec::new();
                for (f, data) in t.screen.modified_fields() {
                    d.push(0x11);
                    d.extend(crate::tn3270::encode_addr(f.start));
                    d.extend(data);
                }
                d
            }
        };
        let cond = Self::deliver(m, call, &input);
        self.finish(m, call, cond, 0)
    }

    // ---- named counters ---------------------------------------------------

    /// DEFINE / DELETE / GET / QUERY / UPDATE / REWIND COUNTER (fullword)
    /// and DCOUNTER (doubleword) in the region's counter pools.
    fn counters(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let double = call.has("DCOUNTER");
        let name = Self::opt_text(m, call, if double { "DCOUNTER" } else { "COUNTER" })
            .unwrap_or_default()
            .trim_end()
            .to_string();
        let pool = Self::opt_text(m, call, "POOL")
            .unwrap_or_default()
            .trim_end()
            .to_string();
        let key = (pool, name);
        let top = if double {
            u64::MAX as i128 - 1
        } else {
            i32::MAX as i128
        };
        let num = |o: &str| {
            call.arg(o)
                .map(|_| Self::opt_num(m, call, o).unwrap_or(0) as i128)
        };
        let mut r = self.region.borrow_mut();
        let cond = match call.command.as_str() {
            "DEFINE" => match r.counters.entry(key) {
                std::collections::btree_map::Entry::Occupied(_) => "DUPREC",
                std::collections::btree_map::Entry::Vacant(e) => {
                    let min = num("MINIMUM").unwrap_or(0);
                    let max = num("MAXIMUM").unwrap_or(top);
                    let value = num("VALUE").unwrap_or(min);
                    e.insert(region::Counter { value, min, max });
                    "NORMAL"
                }
            },
            "DELETE" => match r.counters.remove(&key) {
                Some(_) => "NORMAL",
                None => "NOTFND",
            },
            "REWIND" => match r.counters.get_mut(&key) {
                Some(c) => {
                    c.value = c.min;
                    "NORMAL"
                }
                None => "NOTFND",
            },
            "UPDATE" => match r.counters.get_mut(&key) {
                Some(c) => {
                    let ok = num("COMPAREMIN").is_none_or(|v| c.value >= v)
                        && num("COMPAREMAX").is_none_or(|v| c.value <= v);
                    if ok {
                        c.value = num("VALUE").unwrap_or(c.value);
                        "NORMAL"
                    } else {
                        "SUPPRESSED"
                    }
                }
                None => "NOTFND",
            },
            "QUERY" => match r.counters.get(&key) {
                Some(c) => {
                    let c = c.clone();
                    drop(r);
                    for (o, v) in [("VALUE", c.value), ("MINIMUM", c.min), ("MAXIMUM", c.max)] {
                        if let Some(a) = Self::area(call, o) {
                            Self::store_num(m, &a, v as i64);
                        }
                    }
                    return self.finish(m, call, "NORMAL", 0);
                }
                None => "NOTFND",
            },
            // GET: the current value, then the counter moves on by
            // INCREMENT (the next value past MAXIMUM is SUPPRESSED unless
            // WRAP starts again at MINIMUM)
            _ => match r.counters.get_mut(&key) {
                Some(c) => {
                    let inc = num("INCREMENT").unwrap_or(1);
                    if c.value + inc - 1 > c.max && call.has("WRAP") {
                        c.value = c.min;
                    }
                    if c.value + inc - 1 > c.max {
                        "SUPPRESSED"
                    } else {
                        let v = c.value;
                        c.value += inc;
                        drop(r);
                        if let Some(a) = Self::area(call, "VALUE") {
                            Self::store_num(m, &a, v as i64);
                        }
                        return self.finish(m, call, "NORMAL", 0);
                    }
                }
                None => "NOTFND",
            },
        };
        drop(r);
        self.finish(m, call, cond, 0)
    }

    // ---- units of work ------------------------------------------------------

    /// Ends the unit of work: Db2 commits or rolls back with it, and on
    /// rollback the changes to recoverable files are backed out.
    fn end_uow(&self, commit: bool) -> RResult<()> {
        let db2 = self.db2.borrow().clone();
        if let Some(h) = db2 {
            let _ = h.end_uow(commit);
        }
        let undo = std::mem::take(&mut self.st.borrow_mut().undo);
        if commit {
            return Ok(());
        }
        let mut touched = Vec::new();
        {
            let mut r = self.region.borrow_mut();
            for (key, primary, before) in undo.into_iter().rev() {
                let Some(f) = r.files.get_mut(&key) else {
                    continue;
                };
                let ds = &mut f.dataset;
                let _ = match before {
                    None => ds.delete(&primary).map(|_| ()),
                    Some(rec) if ds.get(&primary).is_some() => ds.replace(rec),
                    Some(rec) => ds.insert(rec),
                };
                if !touched.contains(&key) {
                    touched.push(key);
                }
            }
        }
        for key in touched {
            self.save_file(&key)?;
        }
        Ok(())
    }

    /// Whether changes to FILE `fname` are backed out: LOG of the VSAM
    /// cluster, else RECOVERY of the file definition.
    fn recoverable(&self, fname: &str) -> bool {
        let r = self.region.borrow();
        let Some(dsn) = r.csd.files.get(fname) else {
            return false;
        };
        r.catalog
            .recoverable(dsn)
            .unwrap_or_else(|| r.csd.recoverable.contains(fname))
    }

    fn log_change(&self, fname: &str, key: &str, primary: Vec<u8>, before: Option<Vec<u8>>) {
        if self.recoverable(fname) {
            self.st
                .borrow_mut()
                .undo
                .push((key.to_string(), primary, before));
        }
    }

    // ---- file control ---------------------------------------------------

    /// The open data set of a FILE / DATASET option; None: FILENOTFOUND.
    fn open_file(&self, m: &Machine, call: &ExecCall) -> Option<(String, usize)> {
        let name =
            Self::opt_text(m, call, "FILE").or_else(|| Self::opt_text(m, call, "DATASET"))?;
        let mut r = self.region.borrow_mut();
        let dsn = r.csd.files.get(&name)?.clone();
        let (path, specs, which, max_len) = r.catalog.keyed_access(&dsn).ok()?;
        let key = path.to_string_lossy().to_string();
        if !r.files.contains_key(&key) {
            let ds = KeyedDataset::load(&path, specs.clone(), 0x40)
                .ok()?
                .unwrap_or_else(|| KeyedDataset::new(specs, 0x40));
            r.files.insert(
                key.clone(),
                OpenFile {
                    dataset: ds,
                    path,
                    which: 0,
                    max_len,
                },
            );
        }
        Some((key, which))
    }

    fn save_file(&self, key: &str) -> RResult<()> {
        let r = self.region.borrow();
        let f = &r.files[key];
        f.dataset
            .store(&f.path)
            .map_err(|e| RunError(format!("{}: {e:?}", f.path.display())))
    }

    /// The search key: RIDFLD, KEYLENGTH bytes long with GENERIC.
    fn ridfld(m: &Machine, call: &ExecCall, key_len: usize) -> Vec<u8> {
        let mut k = call
            .arg("RIDFLD")
            .map(|a| Self::bytes(m, a))
            .unwrap_or_default();
        let n = if call.has("GENERIC") {
            Self::opt_num(m, call, "KEYLENGTH").map_or(key_len, |l| l.max(0) as usize)
        } else {
            key_len
        };
        k.truncate(n);
        k
    }

    fn file_control(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let fname = Self::opt_text(m, call, "FILE").or_else(|| Self::opt_text(m, call, "DATASET"));
        if let Some(f) = &fname {
            let e = self.st.borrow().eib;
            put_text(m.storage_mut(), e + eib::DS, 8, f);
            put_text(m.storage_mut(), e + eib::RSRCE, 8, f);
        }
        let Some((key, which)) = self.open_file(m, call) else {
            return self.finish(m, call, "FILENOTFOUND", 0);
        };
        let fname = fname.unwrap_or_default();
        let key_len = {
            let r = self.region.borrow();
            r.files[&key].dataset.keys()[which].len
        };
        let cmd = call.command.as_str();
        match cmd {
            "READ" => {
                let k = Self::ridfld(m, call, key_len);
                let rel = if call.has("GTEQ") {
                    zeus_ds::Rel::Ge
                } else {
                    zeus_ds::Rel::Eq
                };
                let found = {
                    let r = self.region.borrow();
                    r.files[&key].dataset.position(which, &k, rel)
                };
                let Some(f) = found else {
                    return self.finish(m, call, "NOTFND", 0);
                };
                if let Some(a) = Self::area(call, "RIDFLD") {
                    Self::store_bytes(m, &a, &f.position.key);
                }
                if call.has("UPDATE") {
                    self.st
                        .borrow_mut()
                        .updates
                        .insert(fname.clone(), f.primary.clone());
                }
                let cond = Self::deliver(m, call, &f.record);
                let dup = which > 0
                    && self.region.borrow().files[&key].dataset.keys()[which].duplicates
                    && self.region.borrow().files[&key]
                        .dataset
                        .next(which, Some(&f.position), false)
                        .is_some_and(|n| n.position.key == f.position.key);
                self.finish(
                    m,
                    call,
                    if cond == "NORMAL" && dup {
                        "DUPKEY"
                    } else {
                        cond
                    },
                    0,
                )
            }
            "WRITE" => {
                let rec = Self::from_data(m, call);
                let (r, pk) = {
                    let mut r = self.region.borrow_mut();
                    let ds = &mut r.files.get_mut(&key).unwrap().dataset;
                    let pk = ds.key_of(&rec, 0);
                    (ds.insert(rec), pk)
                };
                match r {
                    Ok(()) => {
                        self.log_change(&fname, &key, pk, None);
                        self.save_file(&key)?;
                        self.finish(m, call, "NORMAL", 0)
                    }
                    Err(_) => self.finish(m, call, "DUPREC", 0),
                }
            }
            "REWRITE" => {
                if self.st.borrow_mut().updates.remove(&fname).is_none() {
                    // REWRITE needs a READ UPDATE first
                    return self.finish(m, call, "INVREQ", 0);
                }
                let rec = Self::from_data(m, call);
                let (r, pk, before) = {
                    let mut r = self.region.borrow_mut();
                    let ds = &mut r.files.get_mut(&key).unwrap().dataset;
                    let pk = ds.key_of(&rec, 0);
                    let before = ds.get(&pk).map(<[u8]>::to_vec);
                    (ds.replace(rec), pk, before)
                };
                match r {
                    Ok(()) => {
                        self.log_change(&fname, &key, pk, before);
                        self.save_file(&key)?;
                        self.finish(m, call, "NORMAL", 0)
                    }
                    Err(zeus_ds::DsError::NotFound) => self.finish(m, call, "INVREQ", 0),
                    Err(_) => self.finish(m, call, "DUPREC", 0),
                }
            }
            "DELETE" => {
                // without RIDFLD: the record read for UPDATE
                let primary = if call.has("RIDFLD") {
                    let k = Self::ridfld(m, call, key_len);
                    let r = self.region.borrow();
                    r.files[&key]
                        .dataset
                        .position(which, &k, zeus_ds::Rel::Eq)
                        .map(|f| f.primary)
                } else {
                    match self.st.borrow_mut().updates.remove(&fname) {
                        Some(p) => Some(p),
                        None => return self.finish(m, call, "INVREQ", 0),
                    }
                };
                let Some(p) = primary else {
                    return self.finish(m, call, "NOTFND", 0);
                };
                let before = {
                    let mut r = self.region.borrow_mut();
                    r.files.get_mut(&key).unwrap().dataset.delete(&p).ok()
                };
                if before.is_some() {
                    self.log_change(&fname, &key, p, before);
                }
                self.save_file(&key)?;
                self.finish(m, call, "NORMAL", 0)
            }
            "UNLOCK" => {
                self.st.borrow_mut().updates.remove(&fname);
                self.finish(m, call, "NORMAL", 0)
            }
            "STARTBR" | "RESETBR" => {
                let k = Self::ridfld(m, call, key_len);
                let rel = if call.has("EQUAL") {
                    zeus_ds::Rel::Eq
                } else {
                    zeus_ds::Rel::Ge
                };
                // RIDFLD all X'FF': position after the last record
                let at_end = !k.is_empty() && k.iter().all(|&b| b == 0xFF);
                let found = if at_end {
                    None
                } else {
                    self.region.borrow().files[&key]
                        .dataset
                        .position(which, &k, rel)
                };
                if found.is_none() && !at_end {
                    return self.finish(m, call, "NOTFND", 0);
                }
                let reqid = Self::opt_num(m, call, "REQID").unwrap_or(0);
                self.st.borrow_mut().browses.insert(
                    format!("{fname}/{reqid}"),
                    Browse {
                        which,
                        pos: found.map(|f| f.position),
                        inclusive: true,
                        started: false,
                    },
                );
                self.finish(m, call, "NORMAL", 0)
            }
            "READNEXT" | "READPREV" => {
                let reqid = Self::opt_num(m, call, "REQID").unwrap_or(0);
                let bkey = format!("{fname}/{reqid}");
                let Some((bw, pos, inclusive, started)) = self
                    .st
                    .borrow()
                    .browses
                    .get(&bkey)
                    .map(|b| (b.which, b.pos.clone(), b.inclusive, b.started))
                else {
                    return self.finish(m, call, "INVREQ", 0);
                };
                let found = {
                    let r = self.region.borrow();
                    let ds = &r.files[&key].dataset;
                    if cmd == "READNEXT" {
                        ds.next(bw, pos.as_ref(), inclusive && !started)
                    } else {
                        ds.prev(bw, pos.as_ref(), inclusive && !started)
                    }
                };
                let Some(f) = found else {
                    return self.finish(m, call, "ENDFILE", 0);
                };
                if let Some(b) = self.st.borrow_mut().browses.get_mut(&bkey) {
                    b.pos = Some(f.position.clone());
                    b.started = true;
                }
                if let Some(a) = Self::area(call, "RIDFLD") {
                    Self::store_bytes(m, &a, &f.position.key);
                }
                let cond = Self::deliver(m, call, &f.record);
                self.finish(m, call, cond, 0)
            }
            "ENDBR" => {
                let reqid = Self::opt_num(m, call, "REQID").unwrap_or(0);
                self.st
                    .borrow_mut()
                    .browses
                    .remove(&format!("{fname}/{reqid}"));
                self.finish(m, call, "NORMAL", 0)
            }
            _ => self.finish(m, call, "INVREQ", 0),
        }
    }

    // ---- queues ---------------------------------------------------------

    fn queues(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let q = Self::opt_text(m, call, "QUEUE")
            .or_else(|| Self::opt_text(m, call, "QNAME"))
            .unwrap_or_default();
        let td = call.has("TD");
        match call.command.as_str() {
            "WRITEQ" => {
                let data = Self::from_data(m, call);
                let mut r = self.region.borrow_mut();
                if td {
                    r.td.entry(q).or_default().push(data);
                    drop(r);
                    return self.finish(m, call, "NORMAL", 0);
                }
                let items = r.ts.entry(q).or_default();
                if call.has("REWRITE") {
                    let n = Self::opt_num(m, call, "ITEM").unwrap_or(1);
                    if n < 1 || n as usize > items.len() {
                        drop(r);
                        return self.finish(m, call, "ITEMERR", 0);
                    }
                    items[n as usize - 1] = data;
                } else {
                    items.push(data);
                    let n = items.len() as i64;
                    drop(r);
                    if let Some(a) = Self::area(call, "ITEM") {
                        Self::store_num(m, &a, n);
                    }
                    return self.finish(m, call, "NORMAL", 0);
                }
                drop(r);
                self.finish(m, call, "NORMAL", 0)
            }
            "READQ" => {
                if td {
                    let item = {
                        let mut r = self.region.borrow_mut();
                        match r.td.get_mut(&q) {
                            Some(v) if !v.is_empty() => Some(v.remove(0)),
                            Some(_) => None,
                            None => return self.finish(m, call, "QIDERR", 0),
                        }
                    };
                    let Some(item) = item else {
                        return self.finish(m, call, "QZERO", 0);
                    };
                    let cond = Self::deliver(m, call, &item);
                    return self.finish(m, call, cond, 0);
                }
                let key = format!("ts-next/{q}");
                let item = {
                    let r = self.region.borrow();
                    let Some(items) = r.ts.get(&q) else {
                        drop(r);
                        return self.finish(m, call, "QIDERR", 0);
                    };
                    let n = match Self::opt_num(m, call, "ITEM") {
                        Some(n) if !call.has("NEXT") => n,
                        _ => {
                            let st = self.st.borrow();
                            st.browses
                                .get(&key)
                                .map_or(1, |b| b.pos.as_ref().map_or(1, |p| p.seq as i64 + 1))
                        }
                    };
                    if n < 1 || n as usize > items.len() {
                        None
                    } else {
                        Some((n, items[n as usize - 1].clone()))
                    }
                };
                let Some((n, item)) = item else {
                    return self.finish(m, call, "ITEMERR", 0);
                };
                self.st.borrow_mut().browses.insert(
                    key,
                    Browse {
                        which: 0,
                        pos: Some(Position {
                            key: Vec::new(),
                            seq: n as u64,
                        }),
                        inclusive: false,
                        started: true,
                    },
                );
                if let Some(a) = Self::area(call, "NUMITEMS") {
                    let total = self.region.borrow().ts.get(&q).map_or(0, |v| v.len());
                    Self::store_num(m, &a, total as i64);
                }
                let cond = Self::deliver(m, call, &item);
                self.finish(m, call, cond, 0)
            }
            _ => {
                // DELETEQ
                let gone = {
                    let mut r = self.region.borrow_mut();
                    if td {
                        r.td.remove(&q).is_some()
                    } else {
                        r.ts.remove(&q).is_some()
                    }
                };
                self.finish(m, call, if gone { "NORMAL" } else { "QIDERR" }, 0)
            }
        }
    }

    // ---- other services -------------------------------------------------

    fn assign(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let (applid, sysid) = {
            let r = self.region.borrow();
            (r.applid.clone(), r.sysid.clone())
        };
        let (termid, userid) = {
            let t = self.term.borrow();
            (t.id.clone(), t.userid.clone())
        };
        let st = self.st.borrow();
        let program = st.programs.last().cloned().unwrap_or_default();
        let invoking = if st.programs.len() > 1 {
            st.programs[st.programs.len() - 2].clone()
        } else {
            String::new()
        };
        let abcode = st.abcode.clone();
        drop(st);
        let mut cond = "NORMAL";
        for (name, arg) in &call.options {
            let Some(ExecArg::Area(a)) = arg else {
                continue;
            };
            let text = match name.as_str() {
                "APPLID" => applid.clone(),
                "SYSID" => sysid.clone(),
                "USERID" | "USERNAME" => userid.clone(),
                "PROGRAM" => program.clone(),
                "INVOKINGPROG" => invoking.clone(),
                "STARTCODE" => "TD".into(),
                "FACILITY" | "NETNAME" | "TERMCODE" => termid.clone(),
                "OPID" => String::new(),
                "ABCODE" => abcode.clone(),
                "SCRNHT" => {
                    Self::store_num(m, a, 24);
                    continue;
                }
                "SCRNWD" => {
                    Self::store_num(m, a, 80);
                    continue;
                }
                "CWALENG" | "TWALENG" | "TCTUALENG" => {
                    Self::store_num(m, a, 0);
                    continue;
                }
                "RESP" | "RESP2" => continue,
                _ => {
                    cond = "INVREQ";
                    continue;
                }
            };
            put_text(m.storage_mut(), a.addr, a.len, &text);
        }
        self.finish(m, call, cond, 0)
    }

    fn asktime(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        if let Some(a) = Self::area(call, "ABSTIME") {
            let now = time::abstime_now();
            match a.ty {
                DataType::Numeric(_) => Self::store_num(m, &a, now),
                _ => put_packed(m.storage_mut(), a.addr, a.len, now),
            }
        }
        self.finish(m, call, "NORMAL", 0)
    }

    fn formattime(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        let t = match call.arg("ABSTIME") {
            Some(ExecArg::Area(a)) if !matches!(a.ty, DataType::Numeric(_)) => {
                zeus_decimal::packed::decode_unchecked(&m.storage()[a.addr..a.addr + a.len]).0
                    as i64
            }
            Some(a) => Self::num(m, a),
            None => time::abstime_now(),
        };
        let (y, mo, d, doy, h, mi, s, ms) = time::split(t);
        let sep = |name: &str, dflt: char| -> Option<String> {
            if !call.has(name) {
                return None;
            }
            Some(match call.arg(name) {
                Some(a) => {
                    let b = Self::bytes(m, a);
                    ebcdic::decode_latin1(&b[..b.len().min(1)])
                        .iter()
                        .map(|&c| c as char)
                        .collect()
                }
                None => dflt.to_string(),
            })
        };
        let ds = sep("DATESEP", '/').unwrap_or_default();
        let ts = sep("TIMESEP", ':').unwrap_or_default();
        let yy = y % 100;
        for (name, arg) in &call.options {
            let Some(ExecArg::Area(a)) = arg else {
                continue;
            };
            let text = match name.as_str() {
                "YYYYMMDD" => format!("{y:04}{ds}{mo:02}{ds}{d:02}"),
                "YYMMDD" => format!("{yy:02}{ds}{mo:02}{ds}{d:02}"),
                "MMDDYY" => format!("{mo:02}{ds}{d:02}{ds}{yy:02}"),
                "DDMMYY" => format!("{d:02}{ds}{mo:02}{ds}{yy:02}"),
                "MMDDYYYY" => format!("{mo:02}{ds}{d:02}{ds}{y:04}"),
                "DDMMYYYY" => format!("{d:02}{ds}{mo:02}{ds}{y:04}"),
                "YYYYDDD" => format!("{y:04}{ds}{doy:03}"),
                "YYDDD" => format!("{yy:02}{ds}{doy:03}"),
                "TIME" => format!("{h:02}{ts}{mi:02}{ts}{s:02}"),
                "MILLISECONDS" => {
                    Self::store_num(m, a, ms);
                    continue;
                }
                "YEAR" => {
                    Self::store_num(m, a, y);
                    continue;
                }
                "DAYOFMONTH" => {
                    Self::store_num(m, a, d);
                    continue;
                }
                "MONTHOFYEAR" => {
                    Self::store_num(m, a, mo);
                    continue;
                }
                "DAYCOUNT" => {
                    Self::store_num(m, a, t / 86_400_000);
                    continue;
                }
                _ => continue,
            };
            put_text(m.storage_mut(), a.addr, a.len, &text);
        }
        self.finish(m, call, "NORMAL", 0)
    }

    /// BIF DEEDIT: keeps the digits, right-justified with leading zeros.
    fn deedit(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        if let Some(a) = Self::area(call, "FIELD") {
            let len =
                Self::opt_num(m, call, "LENGTH").map_or(a.len, |l| (l.max(0) as usize).min(a.len));
            let src = m.storage()[a.addr..a.addr + len].to_vec();
            let digits: Vec<u8> = src
                .iter()
                .copied()
                .filter(|b| (0xF0..=0xF9).contains(b))
                .collect();
            let mut out = vec![ebcdic::ZERO; len];
            let n = digits.len().min(len);
            out[len - n..].copy_from_slice(&digits[digits.len() - n..]);
            m.storage_mut()[a.addr..a.addr + len].copy_from_slice(&out);
        }
        self.finish(m, call, "NORMAL", 0)
    }
}

impl ExecHost for Task {
    fn exec(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome> {
        if call.kind == zeus_cobol::zir::ExecKind::Sql {
            let db2 = {
                let mut d = self.db2.borrow_mut();
                if d.is_none() {
                    *d = self.region.borrow_mut().db2_acquire();
                }
                d.clone()
            };
            return match db2 {
                Some(h) => h.exec(m, call),
                None => Err(RunError(format!(
                    "{}: EXEC SQL needs a Db2 server (ZEUS_DB2) (line {})",
                    call.program, call.line
                ))),
            };
        }
        if call.kind != zeus_cobol::zir::ExecKind::Cics {
            return Err(RunError(format!(
                "EXEC {:?} {} needs a {} interface, not available in CICS yet",
                call.kind,
                call.command,
                if call.kind == zeus_cobol::zir::ExecKind::Sql {
                    "Db2"
                } else {
                    "DL/I"
                }
            )));
        }
        match call.command.as_str() {
            "HANDLE" => self.handle(m, call),
            "IGNORE" => self.ignore(m, call),
            "PUSH" | "POP" => self.finish(m, call, "NORMAL", 0),
            "RETURN" | "XCTL" | "LINK" => self.program_control(m, call),
            "ABEND" => {
                let code = Self::opt_text(m, call, "ABCODE").unwrap_or_else(|| "????".into());
                if call.has("CANCEL") {
                    return Err(abend(&code));
                }
                self.raise_abend(call, &code)
            }
            "DEFINE" | "DELETE" | "GET" | "QUERY" | "UPDATE" | "REWIND"
                if call.has("COUNTER") || call.has("DCOUNTER") =>
            {
                self.counters(m, call)
            }
            "SEND" => self.send(m, call),
            "RECEIVE" => self.receive(m, call),
            "READ" | "WRITE" | "REWRITE" | "DELETE" | "UNLOCK" | "STARTBR" | "READNEXT"
            | "READPREV" | "ENDBR" | "RESETBR" => self.file_control(m, call),
            "WRITEQ" | "READQ" | "DELETEQ" => self.queues(m, call),
            "ASSIGN" => self.assign(m, call),
            "ASKTIME" => self.asktime(m, call),
            "FORMATTIME" => self.formattime(m, call),
            "BIF" if call.has("DEEDIT") => self.deedit(m, call),
            "INQUIRE" if call.has("PROGRAM") && call.options.len() <= 4 => {
                // whether a program is installed
                let p = Self::opt_text(m, call, "PROGRAM").unwrap_or_default();
                let cond = if self.program_exists(&p) {
                    "NORMAL"
                } else {
                    "PGMIDERR"
                };
                self.finish(m, call, cond, 0)
            }
            "SYNCPOINT" => {
                self.end_uow(!call.has("ROLLBACK"))?;
                self.finish(m, call, "NORMAL", 0)
            }
            "ENQ" | "DEQ" | "DELAY" | "SUSPEND" => self.finish(m, call, "NORMAL", 0),
            other => Err(RunError(format!(
                "{}: EXEC CICS {other} is not supported yet (line {})",
                call.program, call.line
            ))),
        }
    }
}

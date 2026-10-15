//! `zcics` - runs CICS transactions at a scripted 3270 terminal.
//!
//! ```text
//! zcics [--home DIR] [--csd FILE]... [--loadlib DIR]... [--mapsets DIR]... SCRIPT
//! zcics [options] --serve PORT
//! zcics [options] --serve-tls PORT --tls-cert CERT.pem --tls-key KEY.pem
//! ```
//!
//! `--serve-tls` listens for TN3270 over TLS (with `--serve`, or alone);
//! with a certificate, the `--serve` port offers STARTTLS.
//!
//! `--model N` (2-5) gives the script's terminal the alternate screen size
//! of a 3278/3279 model N (24x80, 32x80, 43x80, 27x132). `--printer ID`
//! defines a printer terminal (START TERMID(ID)); what it prints is
//! appended to DIR/print/ID.txt (a form feed after each page).
//!
//! `--mq-initq Q` runs the trigger monitor (CKTI) on MQ initiation queue
//! Q (repeatable): a trigger message starts the transaction its ApplId
//! names, which RETRIEVEs the MQTM (ZEUS_MQ names the queue manager).
//!
//! `--event-binding FILE` (repeatable) loads an event binding (an .evbind
//! file): events captured at EXEC CICS LINK start the transaction of its
//! transaction start adapter with channel DFHEP.EVENT.
//!
//! With `--serve`, zcics is a TN3270 server: 3270 emulators connect to the
//! port (each connection is a terminal).
//!
//! The catalog is DIR/catalog (default `$ZEUS_HOME` or `./zeus`). Records
//! written to the transient data queue JOBS (the internal reader) are
//! submitted as a job when a `/*EOF` record ends them; job output goes to
//! DIR/spool. Script lines:
//!
//! ```text
//! tran CC00          clear the screen, type CC00 and press ENTER
//! counter POOL NAME N   the named counter's next value is N
//! type 12 43 USER01  type text at line 12, column 43
//! eof 12 43          erase to the end of the field at line 12, column 43
//! key PF3            press an attention key (ENTER, CLEAR, PA1-3, PF1-24)
//! sf 0005810101      send structured fields (hex) with AID X'88'
//! wait 2             wait (seconds); then started transactions due run
//! show               print the screen
//! fields             list the fields (position, attributes, data)
//! expect 4 Sign-on   fail unless line 4 contains the text (* any line)
//! # comment
//! ```
//!
//! A task that waits for terminal input (a RECEIVE after a SEND) gets the
//! screen as the script lines up to the next key leave it.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use zeus_cics::csd::Csd;
use zeus_cics::region::{self, Event, Region, Terminal, aid};
use zeus_ds::catalog::Catalog;

#[path = "../../cli_help.rs"]
mod cli_help;

fn usage() -> ExitCode {
    eprintln!(
        "usage: zcics [--home DIR] [--csd FILE]... [--loadlib DIR]... [--mapsets DIR]... SCRIPT"
    );
    ExitCode::from(2)
}

fn show(t: &Terminal) {
    let w = t.screen.cols;
    println!("+{}+", "-".repeat(w));
    for l in t.screen.render().lines() {
        println!("|{l:<w$}|");
    }
    println!("+{}+", "-".repeat(w));
}

/// Prints the records a task sent beside the screen (structured fields).
fn outbound(t: &RefCell<Terminal>) {
    for r in std::mem::take(&mut t.borrow_mut().outbound) {
        let hex: String = r.iter().map(|b| format!("{b:02X}")).collect();
        println!("< {hex}");
    }
}

/// A terminal script: the lines up to the next attention key are applied
/// to the terminal by `next_key`.
struct Script {
    name: String,
    lines: Vec<String>,
    pos: usize,
    /// The region (for `counter`).
    region: Option<std::rc::Weak<RefCell<Region>>>,
}

impl Script {
    /// Runs script lines up to an attention key and returns it (None at the
    /// end of the script); Err is the exit code of a failed line.
    fn next_key(&mut self, term: &Rc<RefCell<Terminal>>) -> Result<Option<u8>, i32> {
        while self.pos < self.lines.len() {
            let n = self.pos;
            self.pos += 1;
            let line = self.lines[n].trim_end().to_string();
            let fail = |msg: String| {
                eprintln!("zcics: {}:{}: {msg}", self.name, n + 1);
                12
            };
            let mut words = line.splitn(2, ' ');
            let cmd = words.next().unwrap_or_default();
            let rest = words.next().unwrap_or_default();
            let pos = |rest: &str| -> Option<(usize, usize, String)> {
                let mut p = rest.splitn(3, ' ');
                let l = p.next()?.parse().ok()?;
                let c = p.next()?.parse().ok()?;
                Some((l, c, p.next().unwrap_or_default().to_string()))
            };
            match cmd {
                "" => {}
                _ if cmd.starts_with('#') => {}
                "tran" => {
                    {
                        let mut t = term.borrow_mut();
                        t.next = None;
                        t.screen.erase();
                        t.screen.type_at(1, 1, rest).ok();
                    }
                    println!("> tran {rest}");
                    return Ok(Some(aid::ENTER));
                }
                "type" => {
                    let Some((l, c, s)) = pos(rest) else {
                        return Err(fail("type LINE COLUMN TEXT expected".into()));
                    };
                    if let Err(e) = term.borrow_mut().screen.type_at(l, c, &s) {
                        return Err(fail(e));
                    }
                }
                "eof" => {
                    let Some((l, c, _)) = pos(rest) else {
                        return Err(fail("eof LINE COLUMN expected".into()));
                    };
                    if let Err(e) = term.borrow_mut().screen.erase_eof(l, c) {
                        return Err(fail(e));
                    }
                }
                "key" => {
                    let Some(a) = aid::by_name(rest) else {
                        return Err(fail(format!("unknown key {rest}")));
                    };
                    println!("> key {rest}");
                    return Ok(Some(a));
                }
                "sf" => {
                    let hex: String = rest.split_whitespace().collect();
                    let data: Option<Vec<u8>> = (0..hex.len() / 2)
                        .map(|i| u8::from_str_radix(hex.get(2 * i..2 * i + 2)?, 16).ok())
                        .collect();
                    let Some(data) = data.filter(|_| hex.len().is_multiple_of(2)) else {
                        return Err(fail("sf HEX expected".into()));
                    };
                    term.borrow_mut().inbound_sf = Some(data);
                    println!("> sf {hex}");
                    return Ok(Some(zeus_cics::tn3270::AID_STRUCTURED_FIELD));
                }
                "wait" => {
                    let Ok(secs) = rest.trim().parse::<f64>() else {
                        return Err(fail("wait SECONDS expected".into()));
                    };
                    println!("> wait {secs}");
                    std::thread::sleep(std::time::Duration::from_secs_f64(secs));
                    return Ok(Some(WAITED));
                }
                "counter" => {
                    // counter POOL NAME VALUE: the named counter's next value
                    let w: Vec<&str> = rest.split_whitespace().collect();
                    let [pool, name, want] = w[..] else {
                        return Err(fail("counter POOL NAME VALUE expected".into()));
                    };
                    let got = self
                        .region
                        .as_ref()
                        .and_then(std::rc::Weak::upgrade)
                        .and_then(|r| {
                            r.borrow()
                                .counters
                                .get(&(pool.to_string(), name.to_string()))
                                .map(|c| c.value.to_string())
                        });
                    if got.as_deref() != Some(want) {
                        return Err(fail(format!(
                            "counter {pool} {name} is {}, not {want}",
                            got.unwrap_or_else(|| "not defined".into())
                        )));
                    }
                    println!("> counter {pool} {name} {want}");
                }
                "show" => show(&term.borrow()),
                "fields" => fields(&term.borrow()),
                "expect" => {
                    let Some((l, s)) = rest.split_once(' ') else {
                        return Err(fail("expect LINE TEXT expected".into()));
                    };
                    let screen = term.borrow().screen.render();
                    let ok = match l {
                        "*" => screen.contains(s),
                        _ => match l.parse::<usize>() {
                            Ok(l) => screen
                                .lines()
                                .nth(l.saturating_sub(1))
                                .is_some_and(|got| got.contains(s)),
                            Err(_) => return Err(fail("expect LINE TEXT expected".into())),
                        },
                    };
                    if !ok {
                        show(&term.borrow());
                        return Err(fail(format!("line {l} does not contain '{s}'")));
                    }
                }
                other => return Err(fail(format!("unknown command {other}"))),
            }
        }
        Ok(None)
    }
}

fn fields(t: &Terminal) {
    let s = &t.screen;
    for f in s.fields() {
        let a = s.attrs[f.attr_pos].unwrap();
        let data: String = zeus_cobol::ebcdic::decode(&s.field_data(&f))
            .chars()
            .map(|c| if c < ' ' { '.' } else { c })
            .collect();
        println!(
            "({:2},{:2}) {}{}{} len {:3} |{}|",
            f.attr_pos / s.cols + 1,
            f.attr_pos % s.cols + 1,
            if a.protected { "P" } else { "U" },
            if a.numeric { "N" } else { " " },
            if a.mdt { "M" } else { " " },
            f.len,
            data.trim_end_matches('.')
        );
    }
}

mod server {
    //! The TN3270 server: a thread per connection does the Telnet work,
    //! the region runs in the main thread.

    use std::cell::RefCell;
    use std::collections::{HashMap, VecDeque};
    use std::io::Write;
    use std::net::TcpListener;
    use std::path::Path;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex, mpsc};
    use std::time::{Duration, Instant};

    use zeus_cics::csd::Csd;
    use zeus_cics::dispatch;
    use zeus_cics::region::{self, Event, Region, Terminal};
    use zeus_cics::tn3270::{self, DeviceRequest, Inbound, Refusal, Session};
    use zeus_cobol::ebcdic;

    use zeus_cics::tls::{self, Conn};

    type Writer = Arc<Mutex<Conn>>;

    enum Msg {
        Connect(u32, Writer, Session),
        Input(u32, Vec<u8>),
        Sscp(u32, Vec<u8>),
        SysReq(u32),
        Gone(u32),
        /// A started transaction may be due.
        Tick,
    }

    /// LU names: those in use, and the TERMINAL definitions (identifier,
    /// NETNAME, PRINTER) that name terminals and pair displays with
    /// printers.
    struct Names {
        in_use: HashMap<String, (u32, bool)>,
        terminals: Vec<(String, zeus_cics::csd::TerminalDef)>,
    }

    impl Names {
        /// The LU name for a connection.
        fn name(&self, id: u32, ttype: &str, req: DeviceRequest) -> Result<String, Refusal> {
            let printer = ttype.to_ascii_uppercase().starts_with("IBM-3287");
            match req {
                DeviceRequest::Any => Ok(if printer {
                    format!("ZEUSP{id:03}")
                } else {
                    format!("ZEUS{id:04}")
                }),
                DeviceRequest::Connect(n) => {
                    let n = n.to_ascii_uppercase();
                    if n.is_empty() || n.len() > 8 {
                        Err(Refusal::InvalidName)
                    } else if self.in_use.contains_key(&n) {
                        Err(Refusal::InUse)
                    } else {
                        Ok(n)
                    }
                }
                DeviceRequest::Associate(d) => {
                    let d = d.to_ascii_uppercase();
                    if self.in_use.get(&d).is_none_or(|(_, p)| *p) {
                        return Err(Refusal::InvalidAssociate);
                    }
                    // the PRINTER of the display's TERMINAL definition
                    let by_csd = self
                        .terminals
                        .iter()
                        .find(|(_, t)| t.netname.as_deref() == Some(&d))
                        .and_then(|(_, t)| t.printer.clone())
                        .and_then(|p| {
                            self.terminals
                                .iter()
                                .find(|(id, _)| *id == p)
                                .and_then(|(_, t)| t.netname.clone())
                        });
                    let n = by_csd.unwrap_or_else(|| format!("{}P", &d[..d.len().min(7)]));
                    if self.in_use.contains_key(&n) {
                        Err(Refusal::InUse)
                    } else {
                        Ok(n)
                    }
                }
            }
        }

        /// The terminal identifier of an LU: the TERMINAL with that
        /// NETNAME, else T001 / P001 from the connection number.
        fn termid(&self, id: u32, lu: Option<&str>, printer: bool) -> String {
            lu.and_then(|lu| {
                self.terminals
                    .iter()
                    .find(|(_, t)| t.netname.as_deref() == Some(lu))
                    .map(|(tid, _)| tid.clone())
            })
            .unwrap_or_else(|| format!("{}{id:03}", if printer { 'P' } else { 'T' }))
        }
    }

    fn connection(id: u32, mut stream: Conn, tx: mpsc::Sender<Msg>, names: Arc<Mutex<Names>>) {
        let session = tn3270::negotiate(&mut stream, |ttype, req| {
            names.lock().unwrap().name(id, ttype, req)
        });
        let session = match session {
            Ok(s) => s,
            Err(e) => {
                eprintln!("zcics: connection {id}: {e}");
                return;
            }
        };
        let lu = session.device.clone();
        if let Some(n) = &lu {
            names
                .lock()
                .unwrap()
                .in_use
                .insert(n.clone(), (id, session.is_printer()));
        }
        let Ok(writer) = stream.try_clone() else {
            return;
        };
        let writer = Arc::new(Mutex::new(writer));
        let reader_session = session.clone();
        if tx.send(Msg::Connect(id, writer.clone(), session)).is_err() {
            return;
        }
        let mut r = tn3270::Reader::new(stream);
        loop {
            let msg = match r.next_event() {
                Ok(tn3270::Event::Record(rec)) => match reader_session.inbound(rec) {
                    Some(Inbound::Data(d)) => Msg::Input(id, d),
                    Some(Inbound::Sscp(d)) => Msg::Sscp(id, d),
                    None => continue,
                },
                Ok(tn3270::Event::Option(cmd, opt)) => {
                    if let Some(a) = tn3270::answer_option(cmd, opt) {
                        let mut w = writer.lock().unwrap();
                        let _ = w.write_all(&a).and_then(|_| w.flush());
                    }
                    continue;
                }
                Ok(tn3270::Event::SysReq) => Msg::SysReq(id),
                // ATTN has no meaning for CICS terminals
                Ok(_) => continue,
                Err(_) => {
                    let _ = tx.send(Msg::Gone(id));
                    break;
                }
            };
            if tx.send(msg).is_err() {
                break;
            }
        }
        if let Some(n) = &lu {
            names.lock().unwrap().in_use.remove(n);
        }
    }

    fn send(w: &Writer, session: &RefCell<Session>, rec: &[u8]) {
        let mut w = w.lock().unwrap();
        let _ = session.borrow_mut().send(&mut *w, rec);
    }

    /// Sends the records a task left for the terminal, then its screen.
    fn send_screen(w: &Writer, session: &RefCell<Session>, t: &RefCell<Terminal>) {
        if session.borrow().sscp {
            return;
        }
        let records = std::mem::take(&mut t.borrow_mut().outbound);
        for r in records {
            send(w, session, &r);
        }
        // a file transfer exchanges structured fields only
        if t.borrow().transfer {
            return;
        }
        let rec = tn3270::write_screen(&t.borrow().screen, &session.borrow().caps);
        send(w, session, &rec);
    }

    /// Applies terminal input; Some(AID) for an attention key. The reply
    /// to the server's query updates the terminal model; other structured
    /// fields go to the program (AID X'88').
    fn input(
        session: &RefCell<Session>,
        w: &Writer,
        t: &RefCell<Terminal>,
        rec: &[u8],
    ) -> Option<u8> {
        if rec.first() == Some(&tn3270::AID_STRUCTURED_FIELD) {
            if session.borrow().query_pending {
                let mut s = session.borrow_mut();
                s.query_pending = false;
                s.caps.apply_query_reply(rec);
                t.borrow_mut().model = s.caps.model();
                eprintln!("zcics: {} query reply: {:?}", t.borrow().id, s.caps);
                // a BIND with the terminal's own alternate size, then the
                // screen again
                let rebound = s.rebind(&mut *w.lock().unwrap()).unwrap_or(false);
                drop(s);
                if rebound {
                    send_screen(w, session, t);
                }
                return None;
            }
            t.borrow_mut().inbound_sf = Some(rec[1..].to_vec());
            return Some(tn3270::AID_STRUCTURED_FIELD);
        }
        tn3270::apply_input(&mut t.borrow_mut().screen, rec)
    }

    #[derive(Clone)]
    struct Term {
        term: Rc<RefCell<Terminal>>,
        writer: Writer,
        session: Rc<RefCell<Session>>,
        /// A task of the terminal waits for its input.
        waiting: Rc<RefCell<bool>>,
    }

    /// A task of the region: its coroutine, the connection of its terminal
    /// and the input given to it while it waits.
    struct Running {
        task: dispatch::Task,
        conn: Option<u32>,
        input: Option<Option<u8>>,
    }

    /// The region's side of the server. Each task runs as a coroutine
    /// (`zeus_cics::dispatch`): it runs until it waits for its terminal's
    /// input, for a time or a resource, or has used its time slice; the
    /// main loop then takes the terminals' input and runs the other tasks.
    /// Input for a terminal whose task runs is held until the task waits
    /// for it or ends.
    struct Server {
        region: Rc<RefCell<Region>>,
        home: std::path::PathBuf,
        rx: mpsc::Receiver<Msg>,
        held: RefCell<VecDeque<Msg>>,
        terms: RefCell<HashMap<u32, Term>>,
        names: Arc<Mutex<Names>>,
        tasks: RefCell<Vec<Running>>,
    }

    fn target(m: &Msg) -> Option<u32> {
        match m {
            Msg::Connect(id, ..)
            | Msg::Input(id, _)
            | Msg::Sscp(id, _)
            | Msg::SysReq(id)
            | Msg::Gone(id) => Some(*id),
            Msg::Tick => None,
        }
    }

    impl Server {
        fn is_waiting(&self, id: u32) -> bool {
            self.terms
                .borrow()
                .get(&id)
                .is_some_and(|t| *t.waiting.borrow())
        }

        /// A task of the terminal of connection `id` runs (or waits).
        fn busy(&self, id: u32) -> bool {
            self.tasks.borrow().iter().any(|r| r.conn == Some(id))
        }

        /// Whether a message can be dispatched now.
        fn deliverable(&self, m: &Msg) -> bool {
            match (target(m), m) {
                (None, _) | (_, Msg::Connect(..) | Msg::SysReq(_) | Msg::Sscp(..)) => true,
                (Some(id), Msg::Input(..) | Msg::Gone(_)) => !self.busy(id) || self.is_waiting(id),
                (Some(id), _) => !self.busy(id),
            }
        }

        /// The next held message that can be dispatched, else the next one
        /// received until `deadline` (then a Tick); None when the server
        /// ends.
        fn next(&self, deadline: Option<Instant>) -> Option<Msg> {
            let pos = self.held.borrow().iter().position(|m| self.deliverable(m));
            if let Some(i) = pos {
                return self.held.borrow_mut().remove(i);
            }
            match deadline {
                Some(d) => {
                    let wait = d.saturating_duration_since(Instant::now());
                    match self.rx.recv_timeout(wait) {
                        Ok(m) => Some(m),
                        Err(mpsc::RecvTimeoutError::Timeout) => Some(Msg::Tick),
                        Err(mpsc::RecvTimeoutError::Disconnected) => None,
                    }
                }
                None => self.rx.recv().ok(),
            }
        }

        /// When the main loop has to look again without a message: a task
        /// can run, a DELAY ends, a started transaction falls due.
        fn deadline(&self) -> Option<Instant> {
            let now = Instant::now();
            let mut d = region::next_due(&self.region)
                // due but its terminal is busy: look again a little later
                .map(|t| t.max(now + Duration::from_millis(20)));
            for r in self.tasks.borrow().iter() {
                let t = match r.task.wait {
                    dispatch::Wait::Dispatch => now,
                    dispatch::Wait::Until(t) => t,
                    dispatch::Wait::Input if r.input.is_some() => now,
                    dispatch::Wait::Input => continue,
                };
                d = Some(d.map_or(t, |d| d.min(t)));
            }
            d
        }

        /// Starts a task: it runs until it first waits.
        fn spawn(&self, conn: Option<u32>, f: impl FnOnce() + 'static) {
            let mut task = dispatch::Task::new(f);
            if !task.resume(dispatch::Resume::Go) {
                self.tasks.borrow_mut().push(Running {
                    task,
                    conn,
                    input: None,
                });
            }
        }

        /// Runs each task that can run, once.
        fn run_ready(&self) {
            let mut list = std::mem::take(&mut *self.tasks.borrow_mut());
            let mut i = 0;
            while i < list.len() {
                let r = &mut list[i];
                if let Some(how) = r.task.ready(&mut r.input, Instant::now())
                    && r.task.resume(how)
                {
                    list.remove(i);
                    continue;
                }
                i += 1;
            }
            let mut tasks = self.tasks.borrow_mut();
            list.append(&mut tasks);
            *tasks = list;
        }

        fn dispatch(self: &Rc<Self>, msg: Msg) {
            if let Some(id) = target(&msg)
                && self.busy(id)
            {
                match msg {
                    // the input a task waits for
                    Msg::Input(id, rec) if self.is_waiting(id) => {
                        let Some(t) = self.terms.borrow().get(&id).cloned() else {
                            return;
                        };
                        if let Some(aid) = input(&t.session, &t.writer, &t.term, &rec) {
                            *t.waiting.borrow_mut() = false;
                            self.give_input(id, Some(aid));
                        }
                        return;
                    }
                    Msg::Gone(id) => {
                        if self.is_waiting(id)
                            && let Some(t) = self.terms.borrow().get(&id)
                        {
                            *t.waiting.borrow_mut() = false;
                            self.give_input(id, None);
                        }
                        self.held.borrow_mut().push_back(msg);
                        return;
                    }
                    Msg::Connect(..) | Msg::SysReq(_) | Msg::Sscp(..) => {}
                    msg => {
                        self.held.borrow_mut().push_back(msg);
                        return;
                    }
                }
            }
            match msg {
                Msg::Connect(id, w, session) => self.connect(id, w, session),
                Msg::Input(id, rec) => {
                    let Some(t) = self.terms.borrow().get(&id).cloned() else {
                        return;
                    };
                    if t.term.borrow().printer {
                        return;
                    }
                    let Some(aid) = input(&t.session, &t.writer, &t.term, &rec) else {
                        return;
                    };
                    let server = self.clone();
                    self.spawn(Some(id), move || server.attention(&t, aid));
                }
                Msg::SysReq(id) => {
                    let Some(t) = self.terms.borrow().get(&id).cloned() else {
                        return;
                    };
                    let sscp = !t.session.borrow().sscp;
                    t.session.borrow_mut().sscp = sscp;
                    if sscp {
                        self.sscp_say(&t, "ZEUS SSCP: ENTER LOGOFF TO END THE SESSION");
                    } else {
                        send_screen(&t.writer, &t.session, &t.term);
                    }
                }
                Msg::Sscp(id, data) => {
                    let Some(t) = self.terms.borrow().get(&id).cloned() else {
                        return;
                    };
                    let text: String = ebcdic::decode_latin1(&data)
                        .iter()
                        .map(|&b| b as char)
                        .collect::<String>()
                        .to_ascii_uppercase();
                    if text.contains("LOGOFF") {
                        let mut w = t.writer.lock().unwrap();
                        let _ = t.session.borrow_mut().unbind(&mut *w);
                        let _ = w.shutdown();
                    } else {
                        self.sscp_say(&t, "ZEUS SSCP: COMMAND NOT RECOGNIZED");
                    }
                }
                Msg::Gone(id) => {
                    if let Some(t) = self.terms.borrow_mut().remove(&id) {
                        // the hook refers to the terminal
                        t.term.borrow_mut().converse = None;
                        let tid = t.term.borrow().id.clone();
                        self.region.borrow_mut().terminals.remove(&tid);
                        eprintln!("zcics: terminal {tid} disconnected");
                    }
                }
                Msg::Tick => {}
            }
        }

        /// Gives the input of terminal `id` to its waiting task.
        fn give_input(&self, id: u32, aid: Option<u8>) {
            if let Some(r) = self
                .tasks
                .borrow_mut()
                .iter_mut()
                .find(|r| r.conn == Some(id))
            {
                r.input = Some(aid);
            }
        }

        /// The task of an attention key: the transaction, then the screen.
        fn attention(&self, t: &Term, aid: u8) {
            let e = region::attention(&self.region, &t.term, aid);
            if let Event::Abended { transid, code } = &e {
                eprintln!("zcics: {} {transid} abended {code}", t.term.borrow().id);
            }
            for m in self.region.borrow_mut().log.drain(..) {
                eprintln!("zcics: {m}");
            }
            super::submit_jobs(&self.region, &self.home);
            // a blank screen left by a transaction gets the prompt again
            // (CLEAR leaves it blank)
            if e == Event::Idle
                && aid != zeus_cics::region::aid::CLEAR
                && t.term.borrow().screen.fields().is_empty()
            {
                let blank = t.term.borrow().screen.chars.iter().all(|&c| c == 0);
                if blank {
                    t.term.borrow_mut().good_morning();
                }
            }
            send_screen(&t.writer, &t.session, &t.term);
            t.term.borrow_mut().screen.alarm = false;
        }

        /// The task of a started transaction; its display gets the screen.
        fn started(&self, s: region::StartRequest) {
            let (termid, e) = region::run_start(&self.region, s);
            if let Event::Abended { transid, code } = &e {
                eprintln!("zcics: started {transid} abended {code}");
            }
            let t = self
                .terms
                .borrow()
                .values()
                .find(|t| t.term.borrow().id == termid)
                .cloned();
            if let Some(t) = t
                && !t.term.borrow().printer
            {
                send_screen(&t.writer, &t.session, &t.term);
            }
        }

        fn sscp_say(&self, t: &Term, text: &str) {
            let mut w = t.writer.lock().unwrap();
            let _ = t
                .session
                .borrow_mut()
                .send_sscp(&mut *w, &ebcdic::encode(text));
        }

        /// Between events: started transactions that are due start, the
        /// log is written, printers get what was printed.
        fn after(self: &Rc<Self>) {
            for s in region::take_due(&self.region) {
                let conn = s.termid.as_ref().and_then(|tid| {
                    self.terms
                        .borrow()
                        .iter()
                        .find(|(_, t)| &t.term.borrow().id == tid)
                        .map(|(id, _)| *id)
                });
                let server = self.clone();
                self.spawn(conn, move || server.started(s));
            }
            for m in self.region.borrow_mut().log.drain(..) {
                eprintln!("zcics: {m}");
            }
            let printers: Vec<Term> = self
                .terms
                .borrow()
                .values()
                .filter(|t| t.term.borrow().printer)
                .cloned()
                .collect();
            for t in printers {
                let pages = std::mem::take(&mut t.term.borrow_mut().printed);
                if !pages.is_empty() {
                    eprintln!(
                        "zcics: printer {} printed {} page(s)",
                        t.term.borrow().id,
                        pages.len()
                    );
                    let mut w = t.writer.lock().unwrap();
                    let _ = t.session.borrow_mut().print(&mut *w, &pages);
                }
            }
        }

        /// Waits for the input of a terminal (a task's RECEIVE): the task
        /// is suspended until it comes.
        fn wait(&self, t: &Term) -> Option<u8> {
            *t.waiting.borrow_mut() = true;
            send_screen(&t.writer, &t.session, &t.term);
            let got = dispatch::suspend(dispatch::Wait::Input);
            *t.waiting.borrow_mut() = false;
            match got {
                Some(dispatch::Resume::Input(a)) => a,
                _ => None,
            }
        }

        fn run(self: &Rc<Self>) {
            loop {
                self.run_ready();
                self.after();
                let Some(msg) = self.next(self.deadline()) else {
                    break;
                };
                self.dispatch(msg);
            }
        }

        fn connect(self: &Rc<Self>, id: u32, w: Writer, session: Session) {
            let printer = session.is_printer();
            let termid = self
                .names
                .lock()
                .unwrap()
                .termid(id, session.device.as_deref(), printer);
            let term = if printer {
                Terminal::new_printer(&termid)
            } else {
                Terminal::new(&termid)
            };
            let t = Term {
                term: Rc::new(RefCell::new(term)),
                writer: w,
                session: Rc::new(RefCell::new(session)),
                waiting: Rc::default(),
            };
            {
                let s = t.session.borrow();
                eprintln!(
                    "zcics: {} {termid} connected ({}{}{})",
                    if printer { "printer" } else { "terminal" },
                    s.ttype,
                    if s.tn3270e { ", TN3270E" } else { "" },
                    s.device
                        .as_deref()
                        .map(|d| format!(", LU {d}"))
                        .unwrap_or_default()
                );
            }
            self.region
                .borrow_mut()
                .terminals
                .insert(termid, Rc::downgrade(&t.term));
            if !printer {
                t.term.borrow_mut().good_morning();
                let server = Rc::downgrade(self);
                let me = t.clone();
                t.term.borrow_mut().converse = Some(Box::new(move |_| server.upgrade()?.wait(&me)));
                t.term.borrow_mut().model = t.session.borrow().caps.model();
                // the capabilities of a terminal with extended attributes
                if t.session.borrow().caps.extended {
                    t.session.borrow_mut().query_pending = true;
                    send(&t.writer, &t.session, &tn3270::query());
                }
                send_screen(&t.writer, &t.session, &t.term);
            }
            self.terms.borrow_mut().insert(id, t);
        }
    }

    /// A port the server listens on, with TLS when it has a configuration.
    pub struct Listen {
        pub port: u16,
        pub tls: Option<Arc<rustls::ServerConfig>>,
        /// A plain port that offers STARTTLS with this configuration.
        pub starttls: Option<Arc<rustls::ServerConfig>>,
    }

    pub fn run(
        region: Rc<RefCell<Region>>,
        home: &Path,
        ports: Vec<Listen>,
    ) -> std::io::Result<()> {
        let csd: Csd = region.borrow().csd.clone();
        let names = Arc::new(Mutex::new(Names {
            in_use: HashMap::new(),
            terminals: csd.terminals.into_iter().collect(),
        }));
        let (tx, rx) = mpsc::channel();
        let ids = Arc::new(AtomicU32::new(1));
        for l in ports {
            // IPv6 and IPv4 (localhost may name ::1), else IPv4 only
            let listener = TcpListener::bind(("::", l.port))
                .or_else(|_| TcpListener::bind(("0.0.0.0", l.port)))?;
            eprintln!(
                "zcics: TN3270 server listening on port {}{}",
                l.port,
                if l.tls.is_some() { " (TLS)" } else { "" }
            );
            let (names, tx, ids) = (names.clone(), tx.clone(), ids.clone());
            std::thread::spawn(move || {
                for s in listener.incoming().flatten() {
                    let id = ids.fetch_add(1, Ordering::SeqCst);
                    let (tx, names, tls) = (tx.clone(), names.clone(), l.tls.clone());
                    let starttls = l.starttls.clone();
                    std::thread::spawn(move || {
                        let conn = match (tls, starttls) {
                            (Some(cfg), _) => tls::accept(s, cfg),
                            (None, Some(cfg)) => tls::starttls(s, cfg),
                            (None, None) => Ok(Conn::Tcp(s)),
                        };
                        let conn = match conn {
                            Ok(c) => c,
                            Err(e) => {
                                eprintln!("zcics: connection {id}: TLS: {e}");
                                return;
                            }
                        };
                        connection(id, conn, tx, names)
                    });
                }
            });
        }
        drop(tx);
        let server = Rc::new(Server {
            region,
            home: home.to_path_buf(),
            rx,
            held: RefCell::default(),
            terms: RefCell::default(),
            names,
            tasks: RefCell::default(),
        });
        server.run();
        Ok(())
    }
}

/// Submits the jobs written to the internal reader queue JOBS.
fn submit_jobs(region: &Rc<RefCell<Region>>, home: &std::path::Path) {
    let records = {
        let mut r = region.borrow_mut();
        let Some(q) = r.td.get_mut("JOBS") else {
            return;
        };
        let Some(end) = q
            .iter()
            .position(|rec| zeus_cobol::ebcdic::decode_latin1(rec).starts_with(b"/*EOF"))
        else {
            return;
        };
        q.drain(..=end).collect::<Vec<_>>()
    };
    let text: String = records[..records.len() - 1]
        .iter()
        .map(|rec| {
            let line: String = zeus_cobol::ebcdic::decode_latin1(rec)
                .iter()
                .map(|&b| b as char)
                .collect();
            format!("{}\n", line.trim_end())
        })
        .collect();
    let catalog = match Catalog::open(&home.join("catalog")) {
        Ok(c) => c,
        Err(e) => {
            println!("* internal reader: catalog: {e:?}");
            return;
        }
    };
    let mut env = zeus_jcl::job::Env {
        catalog,
        spool: home.join("spool"),
        proclibs: Vec::new(),
        loadlibs: Vec::new(),
    };
    let mut log = Vec::new();
    let r = zeus_jcl::job::run_job(&mut env, &text, &mut log);
    for l in String::from_utf8_lossy(&log).lines() {
        println!("* JES {l}");
    }
    if let Err(e) = r {
        println!("* internal reader: {e}");
    }
    let _ = std::io::Write::flush(&mut std::io::stdout());
    // the job may have catalogued data sets
    if let Ok(c) = Catalog::open(&home.join("catalog")) {
        region.borrow_mut().catalog = c;
    }
}

fn main() -> ExitCode {
    if cli_help::show(
        include_str!("../../../docs/help/zcics.txt"),
        &[
            "--home",
            "--csd",
            "--loadlib",
            "--mapsets",
            "--serve",
            "--serve-tls",
            "--tls-cert",
            "--tls-key",
            "--model",
            "--printer",
            "--ims-lib",
            "--ims-wal",
            "--mq-initq",
            "--event-binding",
        ],
    ) {
        return ExitCode::SUCCESS;
    }
    // MQGET with MQGMO_WAIT lets the other tasks run while it waits
    zeus_mq::mqi::set_wait_hook(|d| {
        zeus_cics::dispatch::delay_until(std::time::Instant::now() + d)
    });
    let mut home = std::env::var("ZEUS_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("zeus"));
    let mut csd = Csd::default();
    let mut loadlibs = Vec::new();
    let mut mapsets = Vec::new();
    let mut script = None;
    let mut serve: Option<u16> = None;
    let mut serve_tls: Option<u16> = None;
    let mut tls_cert: Option<String> = None;
    let mut tls_key: Option<String> = None;
    let mut model: Option<u8> = None;
    let mut printers: Vec<String> = Vec::new();
    let mut ims_libs: Vec<String> = Vec::new();
    let mut ims_wal: Option<PathBuf> = None;
    let mut initqs: Vec<String> = Vec::new();
    let mut bindings: Vec<zeus_cics::events::Binding> = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--home" => home = PathBuf::from(it.next().unwrap_or_default()),
            "--csd" => {
                let f = it.next().unwrap_or_default();
                match std::fs::read(&f) {
                    Ok(t) => csd.merge(Csd::parse(&String::from_utf8_lossy(&t))),
                    Err(e) => {
                        eprintln!("zcics: {f}: {e}");
                        return ExitCode::from(16);
                    }
                }
            }
            "--loadlib" => loadlibs.push(PathBuf::from(it.next().unwrap_or_default())),
            "--mapsets" => mapsets.push(PathBuf::from(it.next().unwrap_or_default())),
            "--serve" => serve = it.next().and_then(|p| p.parse().ok()),
            "--serve-tls" => serve_tls = it.next().and_then(|p| p.parse().ok()),
            "--tls-cert" => tls_cert = it.next(),
            "--tls-key" => tls_key = it.next(),
            "--model" => model = it.next().and_then(|p| p.parse().ok()),
            "--printer" => printers.push(it.next().unwrap_or_default().to_ascii_uppercase()),
            "--ims-lib" => ims_libs.push(it.next().unwrap_or_default()),
            "--ims-wal" => ims_wal = it.next().map(PathBuf::from),
            "--mq-initq" => initqs.push(it.next().unwrap_or_default()),
            "--event-binding" => {
                let f = it.next().unwrap_or_default();
                let name = Path::new(&f)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                match std::fs::read_to_string(&f)
                    .map_err(|e| format!("{f}: {e}"))
                    .and_then(|t| zeus_cics::events::parse(&name, &t))
                {
                    Ok((b, notes)) => {
                        for n in notes {
                            eprintln!("zcics: {n}");
                        }
                        bindings.push(b);
                    }
                    Err(e) => {
                        eprintln!("zcics: {e}");
                        return ExitCode::from(16);
                    }
                }
            }
            _ => script = Some(a),
        }
    }
    let catalog = match Catalog::open(&home.join("catalog")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("zcics: catalog: {e:?}");
            return ExitCode::from(16);
        }
    };
    // the IMS system (DBCTL): DBD and PSB libraries (cataloged PDSs or
    // directories) and the database store, as zjob's DFSRRC00 steps use
    let ims = if ims_libs.is_empty() {
        None
    } else {
        let dirs: Vec<PathBuf> = ims_libs
            .iter()
            .map(|l| {
                let p = PathBuf::from(l);
                if p.is_dir() {
                    p
                } else {
                    catalog.path(&l.to_ascii_uppercase())
                }
            })
            .collect();
        let wal = ims_wal.unwrap_or_else(|| catalog.root().join("ims").join("ims.wal"));
        match zeus_ims::runtime::Library::from_dirs(&dirs)
            .and_then(|lib| zeus_ims::runtime::Ims::open(&wal, lib))
        {
            Ok(i) => Some(i),
            Err(e) => {
                eprintln!("zcics: IMS: {e}");
                return ExitCode::from(16);
            }
        }
    };
    if serve.is_some() || serve_tls.is_some() {
        let cfg = match (&tls_cert, &tls_key) {
            (Some(cert), Some(key)) => match zeus_cics::tls::config(cert, key) {
                Ok(c) => Some(c),
                Err(e) => {
                    eprintln!("zcics: {e}");
                    return ExitCode::from(16);
                }
            },
            _ => None,
        };
        let mut ports = Vec::new();
        if let Some(port) = serve {
            // with a certificate, the plain port offers STARTTLS
            ports.push(server::Listen {
                port,
                tls: None,
                starttls: cfg.clone(),
            });
        }
        if let Some(port) = serve_tls {
            let Some(cfg) = cfg else {
                eprintln!("zcics: --serve-tls needs --tls-cert and --tls-key");
                return ExitCode::from(2);
            };
            ports.push(server::Listen {
                port,
                tls: Some(cfg),
                starttls: None,
            });
        }
        let region = Rc::new(RefCell::new(Region::new(csd, catalog, loadlibs, mapsets)));
        region.borrow_mut().ims = ims.clone();
        region.borrow_mut().initqs = initqs.clone();
        region.borrow_mut().event_bindings = bindings.clone();
        return match server::run(region, &home, ports) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("zcics: {e}");
                ExitCode::from(16)
            }
        };
    }
    let Some(script) = script else {
        return usage();
    };
    let text = match std::fs::read_to_string(&script) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("zcics: {script}: {e}");
            return ExitCode::from(16);
        }
    };
    let region = Rc::new(RefCell::new(Region::new(csd, catalog, loadlibs, mapsets)));
    region.borrow_mut().ims = ims.clone();
    region.borrow_mut().initqs = initqs.clone();
    region.borrow_mut().event_bindings = bindings;
    let term = Rc::new(RefCell::new(Terminal::new("T001")));
    if let Some(n) = model {
        let caps = zeus_cics::tn3270::Capabilities::of_type(&format!("IBM-3279-{n}-E"));
        term.borrow_mut().model = caps.model();
    }
    let script = Rc::new(RefCell::new(Script {
        name: script,
        lines: text.lines().map(str::to_string).collect(),
        pos: 0,
        region: Some(Rc::downgrade(&region)),
    }));
    region
        .borrow_mut()
        .terminals
        .insert("T001".into(), Rc::downgrade(&term));
    // printers: what is printed goes to DIR/print/<termid>.txt
    let printers: Vec<Rc<RefCell<Terminal>>> = printers
        .iter()
        .map(|p| {
            let t = Rc::new(RefCell::new(Terminal::new_printer(p)));
            region
                .borrow_mut()
                .terminals
                .insert(p.clone(), Rc::downgrade(&t));
            t
        })
        .collect();
    // a task waiting for input (conversational RECEIVE) reads the script on
    let s = script.clone();
    term.borrow_mut().converse = Some(Box::new(move |t| {
        loop {
            match s.borrow_mut().next_key(t) {
                Ok(Some(WAITED)) => continue,
                Ok(k) => return k,
                Err(code) => std::process::exit(code),
            }
        }
    }));
    let report = |e: &Event, started: bool| {
        let how = if started { "started " } else { "" };
        match e {
            Event::Abended { transid, code } => println!("* {how}{transid} abended {code}"),
            Event::Ended { transid } => println!("* {how}{transid} ended"),
            Event::Idle => {}
        }
    };
    let after = || {
        for (_, e) in region::run_due(&region) {
            report(&e, true);
        }
        outbound(&term);
        for msg in region.borrow_mut().log.drain(..) {
            println!("* {msg}");
        }
        submit_jobs(&region, &home);
        for p in &printers {
            let pages = std::mem::take(&mut p.borrow_mut().printed);
            if pages.is_empty() {
                continue;
            }
            let id = p.borrow().id.clone();
            let dir = home.join("print");
            let mut text = String::new();
            for page in &pages {
                for l in page {
                    let line = zeus_cobol::ebcdic::decode(l);
                    text.push_str(&line);
                    text.push('\n');
                }
                text.push('\u{c}');
            }
            let r = std::fs::create_dir_all(&dir).and_then(|_| {
                use std::io::Write;
                std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(dir.join(format!("{id}.txt")))
                    .and_then(|mut f| f.write_all(text.as_bytes()))
            });
            match r {
                Ok(()) => println!("* printer {id} printed {} page(s)", pages.len()),
                Err(e) => println!("* printer {id}: {e}"),
            }
        }
    };
    loop {
        let key = script.borrow_mut().next_key(&term);
        match key {
            Ok(Some(WAITED)) => after(),
            Ok(Some(a)) => {
                let e = region::attention(&region, &term, a);
                report(&e, false);
                after();
            }
            Ok(None) => break,
            Err(code) => return ExitCode::from(code as u8),
        }
    }
    after();
    ExitCode::SUCCESS
}

/// What `next_key` returns after a `wait` line (not an AID).
const WAITED: u8 = 0;

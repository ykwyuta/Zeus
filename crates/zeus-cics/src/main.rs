//! `zcics` - runs CICS transactions at a scripted 3270 terminal.
//!
//! ```text
//! zcics [--home DIR] [--csd FILE]... [--loadlib DIR]... [--mapsets DIR]... SCRIPT
//! zcics [options] --serve PORT
//! ```
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
//! type 12 43 USER01  type text at line 12, column 43
//! eof 12 43          erase to the end of the field at line 12, column 43
//! key PF3            press an attention key (ENTER, CLEAR, PA1-3, PF1-24)
//! show               print the screen
//! fields             list the fields (position, attributes, data)
//! expect 4 Sign-on   fail unless line 4 contains the text (* any line)
//! # comment
//! ```
//!
//! A task that waits for terminal input (a RECEIVE after a SEND) gets the
//! screen as the script lines up to the next key leave it.

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;

use zeus_cics::csd::Csd;
use zeus_cics::region::{self, Event, Region, Terminal, aid};
use zeus_ds::catalog::Catalog;

fn usage() -> ExitCode {
    eprintln!(
        "usage: zcics [--home DIR] [--csd FILE]... [--loadlib DIR]... [--mapsets DIR]... SCRIPT"
    );
    ExitCode::from(2)
}

fn show(t: &Terminal) {
    println!("+{}+", "-".repeat(80));
    for l in t.screen.render().lines() {
        println!("|{l:<80}|");
    }
    println!("+{}+", "-".repeat(80));
}

/// A terminal script: the lines up to the next attention key are applied
/// to the terminal by `next_key`.
struct Script {
    name: String,
    lines: Vec<String>,
    pos: usize,
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
        let data: String = zeus_cobol::ebcdic::decode_latin1(&s.field_data(&f))
            .iter()
            .map(|&b| if b < 0x20 { '.' } else { b as char })
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
    use std::collections::{HashMap, HashSet, VecDeque};
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::path::Path;
    use std::rc::Rc;
    use std::sync::{Arc, Mutex, mpsc};

    use zeus_cics::region::{self, Event, Region, Terminal};
    use zeus_cics::tn3270::{self, Session};

    type Writer = Arc<Mutex<TcpStream>>;

    enum Msg {
        Connect(u32, Writer, Session),
        Input(u32, Vec<u8>),
        Gone(u32),
    }

    fn connection(
        id: u32,
        mut stream: TcpStream,
        tx: mpsc::Sender<Msg>,
        names: Arc<Mutex<HashSet<String>>>,
    ) {
        let device = format!("ZEUS{id:04}");
        let session = tn3270::negotiate(&mut stream, &device, |n| {
            names.lock().unwrap().contains(&n.to_ascii_uppercase())
        });
        let session = match session {
            Ok(s) => s,
            Err(e) => {
                eprintln!("zcics: connection {id}: {e}");
                return;
            }
        };
        let name = session.device.clone().map(|n| n.to_ascii_uppercase());
        if let Some(n) = &name {
            names.lock().unwrap().insert(n.clone());
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
            match r.next_event() {
                Ok(tn3270::Event::Record(rec)) => {
                    let Some(rec) = reader_session.inbound(rec) else {
                        continue;
                    };
                    if tx.send(Msg::Input(id, rec)).is_err() {
                        break;
                    }
                }
                Ok(tn3270::Event::Option(cmd, opt)) => {
                    if let Some(a) = tn3270::answer_option(cmd, opt) {
                        let mut w = writer.lock().unwrap();
                        let _ = w.write_all(&a).and_then(|_| w.flush());
                    }
                }
                // ATTN has no meaning for CICS terminals
                Ok(_) => {}
                Err(_) => {
                    let _ = tx.send(Msg::Gone(id));
                    break;
                }
            }
        }
        if let Some(n) = &name {
            names.lock().unwrap().remove(n);
        }
    }

    fn send(w: &Writer, session: &RefCell<Session>, rec: &[u8]) {
        let mut w = w.lock().unwrap();
        let _ = session.borrow_mut().send(&mut *w, rec);
    }

    fn send_screen(w: &Writer, session: &RefCell<Session>, t: &Terminal) {
        let rec = tn3270::write_screen(&t.screen, &session.borrow().caps);
        send(w, session, &rec);
    }

    /// Applies terminal input: a Query Reply updates the session; Some(AID)
    /// for an attention key.
    fn input(session: &RefCell<Session>, t: &RefCell<Terminal>, rec: &[u8]) -> Option<u8> {
        if rec.first() == Some(&tn3270::AID_STRUCTURED_FIELD) {
            session.borrow_mut().caps.apply_query_reply(rec);
            eprintln!(
                "zcics: {} query reply: {:?}",
                t.borrow().id,
                session.borrow().caps
            );
            return None;
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

    /// The region's side of the server. Terminal input is dispatched here
    /// by the main loop and, while a task waits for its terminal's input,
    /// by that wait: other terminals' transactions then run (nested), and
    /// input for waiting terminals is held for them.
    struct Server {
        region: Rc<RefCell<Region>>,
        home: std::path::PathBuf,
        rx: mpsc::Receiver<Msg>,
        held: RefCell<VecDeque<Msg>>,
        terms: RefCell<HashMap<u32, Term>>,
    }

    fn target(m: &Msg) -> u32 {
        match m {
            Msg::Connect(id, ..) | Msg::Input(id, _) | Msg::Gone(id) => *id,
        }
    }

    impl Server {
        fn is_waiting(&self, id: u32) -> bool {
            self.terms
                .borrow()
                .get(&id)
                .is_some_and(|t| *t.waiting.borrow())
        }

        /// The next held message `want` accepts, else the next one received.
        fn next(&self, want: impl Fn(&Msg) -> bool) -> Option<Msg> {
            let mut held = self.held.borrow_mut();
            if let Some(i) = held.iter().position(want) {
                return held.remove(i);
            }
            drop(held);
            self.rx.recv().ok()
        }

        fn dispatch(self: &Rc<Self>, msg: Msg) {
            match msg {
                Msg::Connect(id, w, session) => self.connect(id, w, session),
                Msg::Input(id, rec) => {
                    if self.is_waiting(id) {
                        self.held.borrow_mut().push_back(Msg::Input(id, rec));
                        return;
                    }
                    let Some(t) = self.terms.borrow().get(&id).cloned() else {
                        return;
                    };
                    let Some(aid) = input(&t.session, &t.term, &rec) else {
                        return;
                    };
                    let e = region::attention(&self.region, &t.term, aid);
                    if let Event::Abended { transid, code } = &e {
                        eprintln!("zcics: T{id:03} {transid} abended {code}");
                    }
                    for m in self.region.borrow_mut().log.drain(..) {
                        eprintln!("zcics: {m}");
                    }
                    super::submit_jobs(&self.region, &self.home);
                    // a blank screen left by a transaction gets the prompt
                    // again (CLEAR leaves it blank)
                    if e == Event::Idle
                        && aid != zeus_cics::region::aid::CLEAR
                        && t.term.borrow().screen.fields().is_empty()
                    {
                        let blank = t.term.borrow().screen.chars.iter().all(|&c| c == 0);
                        if blank {
                            t.term.borrow_mut().good_morning();
                        }
                    }
                    send_screen(&t.writer, &t.session, &t.term.borrow());
                    t.term.borrow_mut().screen.alarm = false;
                }
                Msg::Gone(id) => {
                    if self.is_waiting(id) {
                        self.held.borrow_mut().push_back(Msg::Gone(id));
                        return;
                    }
                    if let Some(t) = self.terms.borrow_mut().remove(&id) {
                        // the hook refers to the terminal
                        t.term.borrow_mut().converse = None;
                    }
                    eprintln!("zcics: terminal T{id:03} disconnected");
                }
            }
        }

        /// Waits for the input of terminal `id` (a task's RECEIVE), running
        /// the other terminals meanwhile.
        fn wait(self: &Rc<Self>, id: u32, t: &Term) -> Option<u8> {
            *t.waiting.borrow_mut() = true;
            send_screen(&t.writer, &t.session, &t.term.borrow());
            let got = loop {
                let msg = self.next(|m| {
                    let i = target(m);
                    i == id || !self.is_waiting(i)
                });
                let Some(msg) = msg else { break None };
                match msg {
                    Msg::Input(i, rec) if i == id => {
                        if let Some(a) = input(&t.session, &t.term, &rec) {
                            break Some(a);
                        }
                    }
                    Msg::Gone(i) if i == id => {
                        self.held.borrow_mut().push_back(Msg::Gone(i));
                        break None;
                    }
                    other => self.dispatch(other),
                }
            };
            *t.waiting.borrow_mut() = false;
            got
        }

        fn connect(self: &Rc<Self>, id: u32, w: Writer, session: Session) {
            let t = Term {
                term: Rc::new(RefCell::new(Terminal::new(&format!("T{id:03}")))),
                writer: w,
                session: Rc::new(RefCell::new(session)),
                waiting: Rc::default(),
            };
            t.term.borrow_mut().good_morning();
            {
                let server = Rc::downgrade(self);
                let me = t.clone();
                t.term.borrow_mut().converse =
                    Some(Box::new(move |_| server.upgrade()?.wait(id, &me)));
            }
            {
                let s = t.session.borrow();
                eprintln!(
                    "zcics: terminal T{id:03} connected ({}{}{})",
                    s.ttype,
                    if s.tn3270e { ", TN3270E" } else { "" },
                    s.device
                        .as_deref()
                        .map(|d| format!(", LU {d}"))
                        .unwrap_or_default()
                );
            }
            // the capabilities of a terminal with extended attributes
            if t.session.borrow().caps.extended {
                send(&t.writer, &t.session, &tn3270::query());
            }
            send_screen(&t.writer, &t.session, &t.term.borrow());
            self.terms.borrow_mut().insert(id, t);
        }
    }

    pub fn run(region: Rc<RefCell<Region>>, home: &Path, port: u16) -> std::io::Result<()> {
        let listener = TcpListener::bind(("0.0.0.0", port))?;
        eprintln!("zcics: TN3270 server listening on port {port}");
        let (tx, rx) = mpsc::channel();
        let names: Arc<Mutex<HashSet<String>>> = Arc::default();
        std::thread::spawn(move || {
            for (n, s) in listener.incoming().enumerate() {
                if let Ok(s) = s {
                    let (tx, names) = (tx.clone(), names.clone());
                    std::thread::spawn(move || connection(n as u32 + 1, s, tx, names));
                }
            }
        });
        let server = Rc::new(Server {
            region,
            home: home.to_path_buf(),
            rx,
            held: RefCell::default(),
            terms: RefCell::default(),
        });
        while let Some(msg) = server.next(|_| true) {
            server.dispatch(msg);
        }
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
    let mut home = std::env::var("ZEUS_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("zeus"));
    let mut csd = Csd::default();
    let mut loadlibs = Vec::new();
    let mut mapsets = Vec::new();
    let mut script = None;
    let mut serve: Option<u16> = None;
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
    if let Some(port) = serve {
        let region = Rc::new(RefCell::new(Region::new(csd, catalog, loadlibs, mapsets)));
        return match server::run(region, &home, port) {
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
    let term = Rc::new(RefCell::new(Terminal::new("T001")));
    let script = Rc::new(RefCell::new(Script {
        name: script,
        lines: text.lines().map(str::to_string).collect(),
        pos: 0,
    }));
    // a task waiting for input (conversational RECEIVE) reads the script on
    let s = script.clone();
    term.borrow_mut().converse = Some(Box::new(move |t| match s.borrow_mut().next_key(t) {
        Ok(k) => k,
        Err(code) => std::process::exit(code),
    }));
    loop {
        let key = script.borrow_mut().next_key(&term);
        match key {
            Ok(Some(a)) => {
                let e = region::attention(&region, &term, a);
                match &e {
                    Event::Abended { transid, code } => println!("* {transid} abended {code}"),
                    Event::Ended { transid } => println!("* {transid} ended"),
                    Event::Idle => {}
                }
                for msg in region.borrow_mut().log.drain(..) {
                    println!("* {msg}");
                }
                submit_jobs(&region, &home);
            }
            Ok(None) => break,
            Err(code) => return ExitCode::from(code as u8),
        }
    }
    ExitCode::SUCCESS
}

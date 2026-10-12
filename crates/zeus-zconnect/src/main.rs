//! `zconn` - a REST gateway for the CICS programs and IMS transactions of
//! a Zeus system, driven by a z/OS Connect API project.
//!
//! ```text
//! zconn --api DIR [--home DIR] [--csd FILE]... [--loadlib DIR]...
//!       [--ims-region JOB.jcl] serve PORT
//! zconn [options] call METHOD PATH [JSON]
//! zconn [options] script FILE
//! ```
//!
//! `--api` is the project (its `src/main` with `api/openapi.yaml`,
//! `operations` and `zosAssets`). CICS programs run in a region of the
//! CSD and load libraries, as a distributed program link (a task without a
//! terminal under the asset's transaction identifier); IMS transactions
//! are queued and run by the message processing region job
//! `--ims-region` (DFSRRC00 PARM='MSG'). The catalog is DIR/catalog
//! (default `$ZEUS_HOME` or `./zeus`).
//!
//! `--trace` shows the data passed to and from each program and what the
//! programs display.
//!
//! `serve` answers HTTP/1.1 requests on the port (one at a time). `call`
//! prints the status and the JSON body of one request. A script has lines
//! `call METHOD PATH [JSON]`, `status N` and `expect TEXT` (the JSON of the
//! last response contains TEXT).

use std::cell::{Cell, RefCell};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;

use serde_json::Value;
use zeus_cics::csd::Csd;
use zeus_cics::region::Region;
use zeus_ds::catalog::Catalog;
use zeus_zconnect::api::{Api, project_dir};
use zeus_zconnect::gateway::{Backend, Request, Response, handle};

struct Zeus {
    home: PathBuf,
    region: Rc<RefCell<Region>>,
    loadlibs: Vec<PathBuf>,
    ims_region: Option<String>,
    sequence: Cell<u32>,
    trace: bool,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

impl Backend for Zeus {
    fn link(&self, transid: &str, program: &str, commarea: Vec<u8>) -> Result<Vec<u8>, String> {
        if self.trace {
            eprintln!(
                "zconn: LINK {program} ({transid}) COMMAREA {}",
                hex(&commarea)
            );
        }
        let before = self.region.borrow().log.len();
        let r = zeus_cics::task::link(&self.region, transid, program, commarea);
        if self.trace {
            for l in &self.region.borrow().log[before..] {
                eprintln!("zconn: {l}");
            }
            match &r {
                Ok(b) => eprintln!("zconn: returned COMMAREA {}", hex(b)),
                Err(e) => eprintln!("zconn: abend {e}"),
            }
        }
        r
    }

    fn transaction(&self, code: &str, segment: Vec<u8>) -> Result<Vec<Vec<u8>>, String> {
        let Some(jcl) = &self.ims_region else {
            return Err("no IMS region (--ims-region)".into());
        };
        let root = self.home.join("catalog");
        let queue = zeus_jcl::ims::message_queue(&root)?;
        // the reply comes to an LTERM of this request
        let n = self.sequence.get() + 1;
        self.sequence.set(n);
        let lterm = format!("ZCON{n:04}");
        queue.enqueue(
            zeus_jcl::ims::name8(code),
            zeus_jcl::ims::name8(&lterm),
            &segment,
        )?;
        let mut env = zeus_jcl::job::Env {
            catalog: Catalog::open(&root).map_err(|e| format!("{e:?}"))?,
            spool: self.home.join("spool"),
            proclibs: Vec::new(),
            loadlibs: self.loadlibs.clone(),
        };
        let mut log = Vec::new();
        zeus_jcl::job::run_job(&mut env, jcl, &mut log)?;
        let owner = zeus_ims::tm::process_owner();
        match queue.get(zeus_jcl::ims::name8(&lterm), owner)? {
            Some(m) => {
                queue.complete(owner, &[m.id], &[])?;
                Ok(zeus_ims::runtime::TmMessage::split(&m.data))
            }
            None => Err("the transaction sent no reply".into()),
        }
    }
}

fn usage() -> ExitCode {
    eprintln!(
        "usage: zconn --api DIR [--home DIR] [--csd FILE]... [--loadlib DIR]... [--ims-region JOB]"
    );
    eprintln!("             serve PORT | call METHOD PATH [JSON] | script FILE");
    ExitCode::from(2)
}

/// A request from a method, a path with a query and a JSON body.
fn request(method: &str, target: &str, body: Option<&str>) -> Result<Request, String> {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let query = query
        .split('&')
        .filter(|q| !q.is_empty())
        .map(|q| {
            let (k, v) = q.split_once('=').unwrap_or((q, ""));
            (
                zeus_zconnect::api::url_decode(k),
                zeus_zconnect::api::url_decode(v),
            )
        })
        .collect();
    let body = match body.map(str::trim).filter(|b| !b.is_empty()) {
        Some(b) => Some(serde_json::from_str(b).map_err(|e| format!("request body: {e}"))?),
        None => None,
    };
    Ok(Request {
        method: method.to_ascii_uppercase(),
        path: path.to_string(),
        query,
        headers: Vec::new(),
        body,
    })
}

fn show(r: &Response) -> String {
    format!(
        "{} {}",
        r.status,
        serde_json::to_string_pretty(&r.body).unwrap_or_default()
    )
}

fn serve(api: &Api, zeus: &Zeus, port: u16) -> std::io::Result<()> {
    let listener = std::net::TcpListener::bind(("0.0.0.0", port))?;
    eprintln!("zconn: listening on port {port}");
    for conn in listener.incoming() {
        let Ok(mut conn) = conn else { continue };
        let mut reader = BufReader::new(conn.try_clone()?);
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            continue;
        }
        let mut parts = line.split_whitespace();
        let (method, target) = (
            parts.next().unwrap_or("").to_string(),
            parts.next().unwrap_or("/").to_string(),
        );
        let mut headers = Vec::new();
        let mut len = 0;
        loop {
            let mut h = String::new();
            if reader.read_line(&mut h)? == 0 || h.trim().is_empty() {
                break;
            }
            if let Some((k, v)) = h.split_once(':') {
                if k.trim().eq_ignore_ascii_case("content-length") {
                    len = v.trim().parse().unwrap_or(0);
                }
                headers.push((k.trim().to_string(), v.trim().to_string()));
            }
        }
        let mut body = vec![0u8; len];
        reader.read_exact(&mut body)?;
        let body = String::from_utf8_lossy(&body).into_owned();
        let resp = if method == "OPTIONS" {
            Response {
                status: 204,
                body: Value::Null,
            }
        } else {
            match request(&method, &target, Some(&body)) {
                Ok(mut req) => {
                    req.headers = headers;
                    handle(api, zeus, &req)
                }
                Err(e) => Response {
                    status: 400,
                    body: serde_json::json!({"code": "BAD_REQUEST", "message": e}),
                },
            }
        };
        eprintln!("zconn: {method} {target} -> {}", resp.status);
        let text = if resp.body.is_null() {
            String::new()
        } else {
            resp.body.to_string()
        };
        write!(
            conn,
            "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\
             Access-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: *\r\n\
             Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n\
             Connection: close\r\n\r\n{text}",
            resp.status,
            if resp.status < 400 { "OK" } else { "Error" },
            text.len()
        )?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let mut home = std::env::var("ZEUS_HOME").map_or_else(|_| PathBuf::from("zeus"), PathBuf::from);
    let mut api_dir = None;
    let mut csd = Csd::default();
    let mut loadlibs = Vec::new();
    let mut ims_region = None;
    let mut trace = false;
    let mut rest = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--home" => home = PathBuf::from(it.next().unwrap_or_default()),
            "--api" => api_dir = it.next().map(PathBuf::from),
            "--csd" => {
                let f = it.next().unwrap_or_default();
                match std::fs::read(&f) {
                    Ok(t) => csd.merge(Csd::parse(&String::from_utf8_lossy(&t))),
                    Err(e) => {
                        eprintln!("zconn: {f}: {e}");
                        return ExitCode::from(16);
                    }
                }
            }
            "--loadlib" => loadlibs.push(PathBuf::from(it.next().unwrap_or_default())),
            "--trace" => trace = true,
            "--ims-region" => {
                let f = it.next().unwrap_or_default();
                match std::fs::read_to_string(&f) {
                    Ok(t) => ims_region = Some(t),
                    Err(e) => {
                        eprintln!("zconn: {f}: {e}");
                        return ExitCode::from(16);
                    }
                }
            }
            _ => rest.push(a),
        }
    }
    let Some(api_dir) = api_dir else {
        return usage();
    };
    let api = match Api::load(&project_dir(&api_dir)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("zconn: {e}");
            return ExitCode::from(16);
        }
    };
    let catalog = match Catalog::open(&home.join("catalog")) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("zconn: catalog: {e:?}");
            return ExitCode::from(16);
        }
    };
    let region = Rc::new(RefCell::new(Region::new(
        csd,
        catalog,
        loadlibs.clone(),
        Vec::new(),
    )));
    let zeus = Zeus {
        home,
        region: region.clone(),
        loadlibs,
        ims_region,
        sequence: Cell::new(0),
        trace,
    };
    let r: Result<(), String> = match rest
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["serve", port] => match port.parse() {
            Ok(p) => serve(&api, &zeus, p).map_err(|e| e.to_string()),
            Err(_) => return usage(),
        },
        ["call", method, path, body @ ..] => {
            request(method, path, body.first().copied()).map(|req| {
                println!("{}", show(&handle(&api, &zeus, &req)));
            })
        }
        ["script", file] => script(&api, &zeus, file),
        _ => return usage(),
    };
    for l in region.borrow().log.iter() {
        if l.contains("abended") || l.starts_with("task ") {
            eprintln!("zconn: {l}");
        }
    }
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("zconn: {e}");
            ExitCode::from(1)
        }
    }
}

fn script(api: &Api, zeus: &Zeus, file: &str) -> Result<(), String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
    let mut last: Option<Response> = None;
    for (n, line) in text.lines().enumerate() {
        let l = line.trim();
        let at = |m: String| format!("{file}:{}: {m}", n + 1);
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        let (cmd, arg) = l.split_once(' ').unwrap_or((l, ""));
        match cmd {
            "call" => {
                let mut p = arg.splitn(3, ' ');
                let (m, path, body) = (p.next().unwrap_or(""), p.next().unwrap_or(""), p.next());
                let req = request(m, path, body).map_err(at)?;
                let r = handle(api, zeus, &req);
                println!("> {l}\n{}", show(&r));
                last = Some(r);
            }
            "status" => {
                let want: u16 = arg.trim().parse().map_err(|_| at("status N".into()))?;
                let got = last.as_ref().map_or(0, |r| r.status);
                if got != want {
                    return Err(at(format!("status {got}, not {want}")));
                }
            }
            "expect" => {
                let body = last
                    .as_ref()
                    .map(|r| r.body.to_string())
                    .unwrap_or_default();
                if !body.contains(arg) {
                    return Err(at(format!("the response does not contain {arg}")));
                }
            }
            _ => return Err(at(format!("unknown command {cmd}"))),
        }
    }
    Ok(())
}

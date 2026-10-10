//! `zjob` - runs z/OS batch jobs and manages the Zeus catalog.
//!
//! ```text
//! zjob [options] submit <job.jcl>...
//! zjob [options] catalog list
//! zjob [options] catalog import <dsn> <file> --recfm FB --lrecl 300 [--text]
//! zjob [options] catalog define-pds <dsn> [--recfm FB --lrecl 80]
//! zjob [options] catalog put <dsn(member)> <file>
//! zjob [options] catalog print <dsn>
//!
//! options: --home DIR (default $ZEUS_HOME or ./zeus): catalog in
//!          DIR/catalog, job output in DIR/spool
//!          --proclib DIR, --loadlib DIR (repeatable)
//! ```
//!
//! `import` stores a host file holding EBCDIC records (with `--text`, host
//! text lines converted to EBCDIC records); `put` stores a
//! member: host text converted to EBCDIC 80-byte records for RECFM F/FB
//! libraries, the file unchanged for RECFM U (load modules).

use std::path::PathBuf;
use std::process::ExitCode;

use zeus_cobol::ebcdic;
use zeus_ds::catalog::{Catalog, Entry, EntryKind, Recfm};
use zeus_jcl::job::{Env, job_completion, run_job};

fn usage() -> ExitCode {
    eprintln!("usage: zjob [--home DIR] [--proclib DIR]... [--loadlib DIR]... submit <job.jcl>...");
    eprintln!(
        "       zjob [--home DIR] catalog list | import <dsn> <file> --recfm R --lrecl N [--text]"
    );
    eprintln!("       zjob [--home DIR] catalog define-pds <dsn> [--recfm R --lrecl N]");
    eprintln!("       zjob [--home DIR] catalog put <dsn(member)> <file> | print <dsn>");
    ExitCode::from(2)
}

struct Opts {
    home: PathBuf,
    proclibs: Vec<PathBuf>,
    loadlibs: Vec<PathBuf>,
    recfm: Option<Recfm>,
    lrecl: Option<u32>,
    text: bool,
    args: Vec<String>,
}

fn parse_opts(args: Vec<String>) -> Option<Opts> {
    let mut o = Opts {
        home: std::env::var("ZEUS_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("zeus")),
        proclibs: Vec::new(),
        loadlibs: Vec::new(),
        recfm: None,
        lrecl: None,
        text: false,
        args: Vec::new(),
    };
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--home" => o.home = PathBuf::from(it.next()?),
            "--proclib" => o.proclibs.push(PathBuf::from(it.next()?)),
            "--loadlib" => o.loadlibs.push(PathBuf::from(it.next()?)),
            "--recfm" => o.recfm = Some(Recfm::parse(&it.next()?)?),
            "--lrecl" => o.lrecl = Some(it.next()?.parse().ok()?),
            "--text" => o.text = true,
            _ => o.args.push(a),
        }
    }
    Some(o)
}

fn fail(msg: impl std::fmt::Display) -> ExitCode {
    eprintln!("zjob: {msg}");
    ExitCode::from(16)
}

/// Host text lines as fixed-length EBCDIC records.
fn text_records(data: &[u8], lrecl: u32) -> Vec<u8> {
    let mut out = Vec::new();
    for line in String::from_utf8_lossy(data).lines() {
        let mut r = ebcdic::encode(line.trim_end_matches('\r'));
        r.resize(lrecl as usize, ebcdic::SPACE);
        out.extend(r);
    }
    out
}

fn catalog_cmd(o: &Opts, cat: &mut Catalog) -> ExitCode {
    let a: Vec<&str> = o.args.iter().map(String::as_str).collect();
    match a.as_slice() {
        ["catalog", "list"] => {
            for e in cat.entries() {
                println!("{:<44} {:?}", e.name, e.kind);
            }
        }
        ["catalog", "import", dsn, file] => {
            let (Some(recfm), Some(lrecl)) = (o.recfm, o.lrecl) else {
                return fail("import needs --recfm and --lrecl");
            };
            let data = match std::fs::read(file) {
                Ok(d) => d,
                Err(e) => return fail(format!("{file}: {e}")),
            };
            let data = if o.text {
                text_records(&data, lrecl)
            } else {
                data
            };
            if cat.get(dsn).is_some() {
                let _ = cat.delete(dsn);
            }
            if let Err(e) = cat.define(Entry {
                name: dsn.to_string(),
                kind: EntryKind::Sequential { recfm, lrecl },
            }) {
                return fail(e);
            }
            if let Err(e) = std::fs::write(cat.path(dsn), data) {
                return fail(e);
            }
        }
        ["catalog", "define-pds", dsn] => {
            let kind = EntryKind::Partitioned {
                recfm: o.recfm.unwrap_or(Recfm::Fb),
                lrecl: o.lrecl.unwrap_or(80),
            };
            if cat.get(dsn).is_none()
                && let Err(e) = cat.define(Entry {
                    name: dsn.to_string(),
                    kind,
                })
            {
                return fail(e);
            }
        }
        ["catalog", "put", target, file] => {
            let Some((dsn, member)) = target.strip_suffix(')').and_then(|t| t.split_once('('))
            else {
                return fail("put needs dsn(member)");
            };
            let Some(EntryKind::Partitioned { recfm, lrecl }) =
                cat.get(dsn).map(|e| e.kind.clone())
            else {
                return fail(format!("{dsn} is not a partitioned data set"));
            };
            let data = match std::fs::read(file) {
                Ok(d) => d,
                Err(e) => return fail(format!("{file}: {e}")),
            };
            let data = match recfm {
                Recfm::U => data,
                _ => text_records(&data, lrecl),
            };
            let path = cat.path(dsn).join(member.to_ascii_uppercase());
            if let Err(e) = std::fs::write(&path, data) {
                return fail(format!("{}: {e}", path.display()));
            }
        }
        ["catalog", "print", dsn] => match cat.read_records(dsn) {
            Ok(recs) => {
                use std::io::Write;
                let mut out = std::io::stdout().lock();
                for r in recs {
                    let t: String = ebcdic::decode_latin1(&r)
                        .iter()
                        .map(|&b| b as char)
                        .collect();
                    // a closed pipe (| head) ends the listing
                    if writeln!(out, "{}", t.trim_end()).is_err() {
                        return ExitCode::SUCCESS;
                    }
                }
            }
            Err(e) => return fail(format!("{dsn}: {e}")),
        },
        _ => return usage(),
    }
    match cat.save() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => fail(e),
    }
}

fn main() -> ExitCode {
    let Some(o) = parse_opts(std::env::args().skip(1).collect()) else {
        return usage();
    };
    let mut catalog = match Catalog::open(&o.home.join("catalog")) {
        Ok(c) => c,
        Err(e) => return fail(e),
    };
    match o.args.first().map(String::as_str) {
        Some("catalog") => catalog_cmd(&o, &mut catalog),
        Some("submit") if o.args.len() > 1 => {
            let mut env = Env {
                catalog,
                spool: o.home.join("spool"),
                proclibs: o.proclibs.clone(),
                loadlibs: o.loadlibs.clone(),
            };
            let mut worst = 0u8;
            for f in &o.args[1..] {
                let text = match std::fs::read_to_string(f) {
                    Ok(t) => t,
                    Err(e) => return fail(format!("{f}: {e}")),
                };
                let mut out = std::io::stdout();
                match run_job(&mut env, &text, &mut out) {
                    Ok(results) => match job_completion(&results) {
                        Ok(rc) => worst = worst.max(rc.clamp(0, 255) as u8),
                        Err(e) => {
                            eprintln!("zjob: {f}: {e}");
                            worst = worst.max(16);
                        }
                    },
                    Err(e) => {
                        eprintln!("zjob: {f}: {e}");
                        worst = worst.max(16);
                    }
                }
            }
            ExitCode::from(worst)
        }
        _ => usage(),
    }
}

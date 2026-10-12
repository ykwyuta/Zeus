//! `zpli` - the Zeus PL/I compiler and interpreter.
//!
//! ```text
//! zpli compile [-I dir]... [--margins m,n] [--ccsid N] [-o out.zo] source.pli
//! zpli run [--parm text] [--dd NAME=path]... module.zo
//! zpli source.pli            compile and run
//! ```
//!
//! SYSPRINT goes to standard output and SYSIN comes from standard input
//! unless allocated with `--dd`. Messages go to standard error. The exit
//! status is the return code (PLIRETC), or 16 after an ERROR condition.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::ExitCode;

use zeus_pli::vm::{DdInfo, RunEnd, RunEnv};

fn usage() -> ExitCode {
    eprintln!("usage: zpli compile [-I dir]... [--margins m,n] [--ccsid N] [-o out] source.pli");
    eprintln!("       zpli run [--parm text] [--dd NAME=path]... module");
    eprintln!("       zpli [options] source.pli");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut opts = zeus_pli::Options::default();
    let mut out: Option<PathBuf> = None;
    let mut parm = String::new();
    let mut dds: HashMap<String, DdInfo> = HashMap::new();
    let mut files = Vec::new();
    let mut cmd = None;
    let mut ccsid = std::env::var("ZEUS_CCSID")
        .ok()
        .and_then(|c| c.parse().ok());
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "compile" | "run" if cmd.is_none() && files.is_empty() => cmd = Some(a),
            "-I" => opts
                .include_dirs
                .push(it.next().map(PathBuf::from).unwrap_or_default()),
            "-o" => out = it.next().map(PathBuf::from),
            "--parm" => parm = it.next().unwrap_or_default(),
            "--ccsid" => ccsid = it.next().and_then(|c| c.parse().ok()),
            "--margins" => {
                let v = it.next().unwrap_or_default();
                let mut p = v.split(',').filter_map(|x| x.parse().ok());
                if let (Some(a), Some(b)) = (p.next(), p.next()) {
                    opts.margins = Some((a, b));
                }
            }
            "--dd" => {
                let v = it.next().unwrap_or_default();
                if let Some((n, p)) = v.split_once('=') {
                    dds.insert(
                        n.to_ascii_uppercase(),
                        DdInfo {
                            path: PathBuf::from(p),
                            format: None,
                            keys: None,
                            sysout: true,
                        },
                    );
                }
            }
            _ if a.starts_with('-') => return usage(),
            _ => files.push(PathBuf::from(a)),
        }
    }
    if let Some(c) = ccsid
        && !zeus_cobol::codepage::set_active(c)
    {
        eprintln!("zpli: CCSID {c} is not supported");
        return ExitCode::from(2);
    }
    let [file] = files.as_slice() else {
        return usage();
    };
    let module = if cmd.as_deref() == Some("run") {
        match zeus_pli::load_module(file) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("zpli: {}: {e}", file.display());
                return ExitCode::from(16);
            }
        }
    } else {
        let (rc, c) = zeus_pli::compile_file(file, &opts);
        for m in &c.messages {
            eprintln!("{m}");
        }
        let Some(m) = c.module else {
            return ExitCode::from(rc.clamp(0, 255) as u8);
        };
        if cmd.as_deref() == Some("compile") {
            let out = out.unwrap_or_else(|| file.with_extension("zo"));
            if let Err(e) = zeus_pli::save_module(&m, &out) {
                eprintln!("zpli: {}: {e}", out.display());
                return ExitCode::from(16);
            }
            return ExitCode::from(rc.clamp(0, 255) as u8);
        }
        m
    };
    let env = RunEnv {
        dds,
        parm,
        stdout: Box::new(std::io::stdout()),
        msgout: Box::new(std::io::stderr()),
        stdin: None,
        stdin_reader: Some(Box::new(std::io::stdin())),
    };
    match zeus_pli::run(module, env) {
        RunEnd::Rc(rc) => ExitCode::from(rc.clamp(0, 255) as u8),
        RunEnd::Abend(code) => {
            eprintln!("zpli: abnormal end {code}");
            ExitCode::from(16)
        }
    }
}

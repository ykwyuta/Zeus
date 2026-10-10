//! `zcobc` - the Zeus COBOL compiler driver.
//!
//! ```text
//! zcobc compile [-I <dir>]... [--subprogram] -o <artifact> <source>
//! zcobc run [--dd NAME=PATH]... <artifact>   (files: see zeus_ds::dd)
//! zcobc check [-I <dir>]... <source>...     (syntax check only)
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use zeus_cobol::front::{self, FileTable, FrontOptions};

fn usage() -> ExitCode {
    eprintln!(
        "usage: zcobc compile [-I <dir>]... [--subprogram] [--debugging-lines] -o <artifact> <source>"
    );
    eprintln!("       zcobc run [--dd NAME=PATH]... <artifact>");
    eprintln!("       zcobc check [-I <dir>]... <source>...");
    ExitCode::from(2)
}

struct Args {
    copy_dirs: Vec<PathBuf>,
    output: Option<PathBuf>,
    subprogram: bool,
    debugging_lines: bool,
    /// DD allocations (`--dd NAME=PATH`).
    dds: Vec<(String, PathBuf)>,
    inputs: Vec<PathBuf>,
}

fn parse_args(args: &[String]) -> Option<Args> {
    let mut a = Args {
        copy_dirs: Vec::new(),
        output: None,
        subprogram: false,
        debugging_lines: false,
        dds: Vec::new(),
        inputs: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-I" => a.copy_dirs.push(PathBuf::from(it.next()?)),
            "-o" => a.output = Some(PathBuf::from(it.next()?)),
            "--subprogram" => a.subprogram = true,
            "--debugging-lines" => a.debugging_lines = true,
            "--dd" => {
                let (n, p) = it.next()?.split_once('=')?;
                a.dds.push((n.to_string(), PathBuf::from(p)));
            }
            s if s.starts_with("-I") => a.copy_dirs.push(PathBuf::from(&s[2..])),
            s => a.inputs.push(PathBuf::from(s)),
        }
    }
    Some(a)
}

fn check(args: &Args) -> ExitCode {
    let opts = FrontOptions {
        copy_dirs: args.copy_dirs.clone(),
        debugging_lines: args.debugging_lines,
    };
    let mut worst = 0;
    for input in &args.inputs {
        let mut files = FileTable::default();
        match front::parse_file(input, &opts, &mut files) {
            Ok(programs) => {
                println!("{}: OK ({} program(s))", input.display(), programs.len());
            }
            Err(mut d) => {
                d.file = files.names.get(d.pos.file as usize).cloned();
                eprintln!("{d}");
                worst = worst.max(d.severity.return_code());
            }
        }
    }
    ExitCode::from(worst as u8)
}

fn compile(args: &Args) -> ExitCode {
    let [input] = args.inputs.as_slice() else {
        return usage();
    };
    let opts = FrontOptions {
        copy_dirs: args.copy_dirs.clone(),
        debugging_lines: args.debugging_lines,
    };
    match zeus_cobol::compile_file(input, &opts) {
        Ok(module) => {
            let out = args
                .output
                .clone()
                .unwrap_or_else(|| input.with_extension("zo"));
            if let Err(e) = zeus_cobol::save_module(&module, &out) {
                eprintln!("zcobc: cannot write {}: {e}", out.display());
                return ExitCode::from(16);
            }
            ExitCode::SUCCESS
        }
        Err(d) => {
            eprintln!("{d}");
            ExitCode::from(d.severity.return_code() as u8)
        }
    }
}

fn run(args: &Args) -> ExitCode {
    let [artifact] = args.inputs.as_slice() else {
        return usage();
    };
    let module = match zeus_cobol::load_module(artifact) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("zcobc: cannot load {}: {e}", artifact.display());
            return ExitCode::from(16);
        }
    };
    let dir = artifact
        .parent()
        .map(|p| {
            if p.as_os_str().is_empty() {
                PathBuf::from(".")
            } else {
                p.to_path_buf()
            }
        })
        .unwrap_or_else(|| PathBuf::from("."));
    let mut machine = zeus_cobol::vm::Machine::new(vec![dir]);
    for (n, p) in &args.dds {
        machine.set_dd(n, p.clone());
    }
    match machine.run_main(module) {
        Ok(code) => ExitCode::from((code & 0xFF) as u8),
        Err(e) => {
            eprintln!("zcobc: run-time error: {e}");
            ExitCode::from(16)
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        return usage();
    };
    let Some(parsed) = parse_args(&args[1..]) else {
        return usage();
    };
    match cmd.as_str() {
        "check" => check(&parsed),
        "compile" => compile(&parsed),
        "run" => run(&parsed),
        "--version" => {
            println!("zcobc {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => usage(),
    }
}

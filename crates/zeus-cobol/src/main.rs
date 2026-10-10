//! `zcobc` - the Zeus COBOL compiler driver.
//!
//! ```text
//! zcobc compile [-I <dir>]... [--subprogram] -o <artifact> <source>
//! zcobc run <artifact>
//! zcobc check [-I <dir>]... <source>...     (syntax check only)
//! ```

use std::path::PathBuf;
use std::process::ExitCode;

use zeus_cobol::front::{self, FileTable, FrontOptions};

fn usage() -> ExitCode {
    eprintln!("usage: zcobc compile [-I <dir>]... [--subprogram] -o <artifact> <source>");
    eprintln!("       zcobc run <artifact>");
    eprintln!("       zcobc check [-I <dir>]... <source>...");
    ExitCode::from(2)
}

struct Args {
    copy_dirs: Vec<PathBuf>,
    output: Option<PathBuf>,
    subprogram: bool,
    inputs: Vec<PathBuf>,
}

fn parse_args(args: &[String]) -> Option<Args> {
    let mut a = Args {
        copy_dirs: Vec::new(),
        output: None,
        subprogram: false,
        inputs: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-I" => a.copy_dirs.push(PathBuf::from(it.next()?)),
            "-o" => a.output = Some(PathBuf::from(it.next()?)),
            "--subprogram" => a.subprogram = true,
            s if s.starts_with("-I") => a.copy_dirs.push(PathBuf::from(&s[2..])),
            s => a.inputs.push(PathBuf::from(s)),
        }
    }
    Some(a)
}

fn check(args: &Args) -> ExitCode {
    let opts = FrontOptions {
        copy_dirs: args.copy_dirs.clone(),
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
        "compile" | "run" => {
            eprintln!("zcobc: not implemented yet");
            ExitCode::from(1)
        }
        "--version" => {
            println!("zcobc {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => usage(),
    }
}

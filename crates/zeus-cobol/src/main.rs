//! `zcobc` - the Zeus COBOL compiler driver.
//!
//! ```text
//! zcobc compile [-I <dir>]... [--subprogram] [--cics] -o <artifact> <source>
//! zcobc compile --native [--target <triple>] [--with <module>]... -o <exe> <source>
//! zcobc run [--dd NAME=PATH]... [--parm TEXT] [--coverage DIR] <artifact>   (files: see zeus_ds::dd)
//! zcobc check [-I <dir>]... [--cics] <source>...   (compile without output)
//! zcobc inventory [-I <dir>]... <source>...  (statements, functions, EXEC usage)
//! ```
//!
//! `--ccsid N` (or ZEUS_CCSID) chooses the EBCDIC code page of literals and
//! of the data a run reads and writes as text: 37 (default), or the
//! Japanese code pages 930, 939, 1390, 1399. `--nsymbol dbcs` makes N
//! literals and PICTURE N DBCS data (default NATIONAL: UTF-16).
//!
//! `--native` writes an executable for the platform (or for `--target`,
//! a Rust target triple) instead of a `.zo` module: the Zeus run time
//! with the module, and the modules of `--with` (separately compiled
//! COBOL or PL/I subprograms), inside. See [`zeus_cobol::image`]. The
//! COBOL programs are compiled to machine code for the target
//! ([`zeus_cobol::native`]) unless `--no-machine-code` is given.

use std::path::PathBuf;
use std::process::ExitCode;

use zeus_cobol::front::FrontOptions;

#[path = "../../cli_help.rs"]
mod cli_help;

fn usage() -> ExitCode {
    eprintln!(
        "usage: zcobc compile [-I <dir>]... [--subprogram] [--debugging-lines] [--cics] [--ccsid N] [--nsymbol dbcs|national] -o <artifact> <source>"
    );
    eprintln!(
        "       zcobc compile --native [--target <triple>] [--with <module.zo>]... [--no-machine-code] [options] -o <executable> <source>"
    );
    eprintln!(
        "       zcobc run [--dd NAME=PATH]... [--parm TEXT] [--ccsid N] [--coverage DIR] <artifact>"
    );
    eprintln!("       zcobc check [-I <dir>]... [--cics] <source>...");
    eprintln!("       zcobc inventory [-I <dir>]... <source>...");
    ExitCode::from(2)
}

struct Args {
    copy_dirs: Vec<PathBuf>,
    output: Option<PathBuf>,
    subprogram: bool,
    debugging_lines: bool,
    /// `--cics`: translate as a CICS program.
    cics: bool,
    /// `--nsymbol dbcs`: N literals and PICTURE N are DBCS.
    nsymbol_dbcs: bool,
    /// DD allocations (`--dd NAME=PATH`).
    dds: Vec<(String, PathBuf)>,
    /// `--parm TEXT`: the EXEC PARM of the main program.
    parm: String,
    inputs: Vec<PathBuf>,
    /// `--native`: an executable instead of a module.
    native: bool,
    /// `--target`: the platform of the executable.
    target: Option<String>,
    /// `--with`: modules put into the executable.
    with: Vec<PathBuf>,
    /// `--no-machine-code`: the executable interprets the programs.
    no_machine_code: bool,
}

fn parse_args(args: &[String]) -> Option<Args> {
    let mut a = Args {
        copy_dirs: Vec::new(),
        output: None,
        subprogram: false,
        debugging_lines: false,
        cics: false,
        nsymbol_dbcs: false,
        dds: Vec::new(),
        parm: String::new(),
        inputs: Vec::new(),
        native: false,
        target: None,
        with: Vec::new(),
        no_machine_code: false,
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "-I" => a.copy_dirs.push(PathBuf::from(it.next()?)),
            "-o" => a.output = Some(PathBuf::from(it.next()?)),
            "--subprogram" => a.subprogram = true,
            "--debugging-lines" => a.debugging_lines = true,
            "--cics" => a.cics = true,
            "--native" => a.native = true,
            "--coverage" => zeus_cobol::coverage::set_dir(std::path::Path::new(it.next()?)),
            "--no-machine-code" => a.no_machine_code = true,
            "--target" => a.target = Some(it.next()?.clone()),
            "--with" => a.with.push(PathBuf::from(it.next()?)),
            "--nsymbol" => {
                a.nsymbol_dbcs = match it.next()?.to_ascii_uppercase().as_str() {
                    "DBCS" => true,
                    "NATIONAL" => false,
                    _ => return None,
                }
            }
            "--parm" => a.parm = it.next()?.clone(),
            "--ccsid" => {
                let c: u32 = it.next()?.parse().ok()?;
                if !zeus_cobol::codepage::set_active(c) {
                    eprintln!(
                        "zcobc: CCSID {c} is not supported ({:?})",
                        zeus_cobol::codepage::CCSIDS
                    );
                    return None;
                }
            }
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
        cics: args.cics,
        nsymbol_dbcs: args.nsymbol_dbcs,
        ..Default::default()
    };
    let mut worst = 0;
    for input in &args.inputs {
        match zeus_cobol::compile_file(input, &opts) {
            Ok(module) => {
                println!(
                    "{}: OK ({} program(s))",
                    input.display(),
                    module.programs.len()
                );
            }
            Err(d) => {
                eprintln!("{d}");
                worst = worst.max(d.severity.return_code());
            }
        }
    }
    ExitCode::from(worst as u8)
}

fn inventory(args: &Args) -> ExitCode {
    let opts = FrontOptions {
        copy_dirs: args.copy_dirs.clone(),
        debugging_lines: args.debugging_lines,
        cics: args.cics,
        nsymbol_dbcs: args.nsymbol_dbcs,
        ..Default::default()
    };
    let paths: Vec<&std::path::Path> = args.inputs.iter().map(|p| p.as_path()).collect();
    let (inv, failed) = zeus_cobol::inventory::inventory(&paths, &opts);
    print!("{}", inv.report());
    for (name, msg) in &failed {
        println!("not compiled\t{name}\t1\t1\t{msg}");
    }
    ExitCode::SUCCESS
}

fn compile(args: &Args) -> ExitCode {
    let [input] = args.inputs.as_slice() else {
        return usage();
    };
    let opts = FrontOptions {
        copy_dirs: args.copy_dirs.clone(),
        debugging_lines: args.debugging_lines,
        cics: args.cics,
        nsymbol_dbcs: args.nsymbol_dbcs,
        ..Default::default()
    };
    match zeus_cobol::compile_file(input, &opts) {
        Ok(module) if args.native => {
            let out = args
                .output
                .clone()
                .unwrap_or_else(|| input.with_extension(""));
            let name = module
                .programs
                .first()
                .map(|p| p.name.clone())
                .unwrap_or_default();
            match native(name, zeus_cobol::module_bytes(&module), args, &out) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("zcobc: {e}");
                    ExitCode::from(16)
                }
            }
        }
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

/// Writes the executable of a module (`--native`).
fn native(
    name: String,
    module: std::io::Result<Vec<u8>>,
    args: &Args,
    out: &std::path::Path,
) -> Result<(), String> {
    let mut image = zeus_cobol::image::Image::default();
    image
        .members
        .push((name, module.map_err(|e| e.to_string())?));
    for w in &args.with {
        let data = std::fs::read(w).map_err(|e| format!("{}: {e}", w.display()))?;
        image
            .members
            .push((zeus_cobol::image::member_name(w), data));
    }
    if !args.no_machine_code {
        let triple = args
            .target
            .clone()
            .unwrap_or_else(zeus_cobol::native::host_triple);
        image.add_machine_code(&triple)?;
    }
    let runtime = zeus_cobol::image::find_runtime(args.target.as_deref())?;
    image
        .write_executable(&runtime, out)
        .map_err(|e| format!("cannot write {}: {e}", out.display()))
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
    // the module's code page, unless --ccsid or ZEUS_CCSID chose one
    if std::env::var_os("ZEUS_CCSID").is_none() {
        zeus_cobol::codepage::set_active(module.ccsid);
    }
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
    machine.set_step(zeus_cobol::vm::StepInfo {
        parm: args.parm.clone(),
        ..Default::default()
    });
    let r = machine.run_main(module);
    // the coverage counts (ZEUS_COVERAGE, --coverage)
    drop(machine);
    if let Err(e) = zeus_cobol::coverage::save() {
        eprintln!("zcobc: coverage: {e}");
    }
    match r {
        Ok(code) => ExitCode::from((code & 0xFF) as u8),
        Err(e) => match e.abend() {
            Some(code) => {
                eprintln!("zcobc: ABEND {code}");
                ExitCode::from(12)
            }
            None => {
                eprintln!("zcobc: run-time error: {e}");
                ExitCode::from(16)
            }
        },
    }
}

fn main() -> ExitCode {
    if cli_help::show(
        include_str!("../../../docs/help/zcobc.txt"),
        &[
            "-I",
            "-o",
            "--coverage",
            "--target",
            "--with",
            "--nsymbol",
            "--parm",
            "--ccsid",
            "--dd",
        ],
    ) {
        return ExitCode::SUCCESS;
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first() else {
        return usage();
    };
    let Some(parsed) = parse_args(&args[1..]) else {
        return usage();
    };
    match cmd.as_str() {
        "check" => check(&parsed),
        "inventory" => inventory(&parsed),
        "compile" => compile(&parsed),
        "run" => run(&parsed),
        "--version" => {
            println!("zcobc {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => usage(),
    }
}

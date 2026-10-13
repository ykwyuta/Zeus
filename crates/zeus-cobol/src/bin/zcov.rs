//! `zcov` - reports the coverage of COBOL and PL/I programs recorded by
//! the Zeus interpreters (ZEUS_COVERAGE=DIR, `zcobc run --coverage DIR`,
//! `zpli --coverage DIR`; see `zeus_cobol::coverage`).
//!
//! ```text
//! zcov report [--detail] [--lcov FILE] [--html DIR] DIR...
//! ```
//!
//! The coverage files of the directories are merged. The text report
//! lists each program's lines, branches (each way) and paragraphs or
//! procedures covered; `--detail` adds the lines not run, the branches
//! not taken both ways and the paragraphs not entered. `--lcov` writes an
//! LCOV tracefile, `--html` a page per source file.

use std::path::PathBuf;
use std::process::ExitCode;

use zeus_cobol::coverage;

fn usage() -> ExitCode {
    eprintln!("usage: zcov report [--detail] [--lcov FILE] [--html DIR] DIR...");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("report") {
        return usage();
    }
    let mut detail = false;
    let mut lcov: Option<PathBuf> = None;
    let mut html: Option<PathBuf> = None;
    let mut dirs = Vec::new();
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--detail" => detail = true,
            "--lcov" => lcov = it.next().map(PathBuf::from),
            "--html" => html = it.next().map(PathBuf::from),
            s if s.starts_with('-') => return usage(),
            s => dirs.push(PathBuf::from(s)),
        }
    }
    if dirs.is_empty() {
        return usage();
    }
    let mut programs: Vec<coverage::ProgramCov> = Vec::new();
    for d in &dirs {
        match coverage::load_dir(d) {
            Ok(ps) => {
                for p in ps {
                    match programs.iter_mut().find(|x| {
                        x.language == p.language
                            && x.name == p.name
                            && x.sources.first() == p.sources.first()
                    }) {
                        Some(x) => x.merge(&p),
                        None => programs.push(p),
                    }
                }
            }
            Err(e) => {
                eprintln!("zcov: {}: {e}", d.display());
                return ExitCode::from(8);
            }
        }
    }
    if programs.is_empty() {
        eprintln!("zcov: no coverage data");
        return ExitCode::from(4);
    }
    print!("{}", coverage::text_report(&programs, detail));
    if let Some(f) = lcov
        && let Err(e) = std::fs::write(&f, coverage::lcov(&programs))
    {
        eprintln!("zcov: {}: {e}", f.display());
        return ExitCode::from(8);
    }
    if let Some(d) = html
        && let Err(e) = coverage::html(&programs, &d)
    {
        eprintln!("zcov: {}: {e}", d.display());
        return ExitCode::from(8);
    }
    ExitCode::SUCCESS
}

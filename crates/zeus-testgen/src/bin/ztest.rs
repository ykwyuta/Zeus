//! `ztest` - runs the test cases `ztestgen` made and compares the
//! results with the expected ones.
//!
//! ```text
//! ztest [-I DIR]... [-L DIR]... [--coverage DIR] SOURCE.cbl CASES-DIR
//! ```
//!
//! `--coverage DIR` records coverage (see `zcov`). The exit status is 1
//! when a case differs.

use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;

use zeus_cobol::front::FrontOptions;
use zeus_testgen::case::{read_dir, replay};
use zeus_testgen::engine::Options;

fn usage() -> ExitCode {
    eprintln!("usage: ztest [-I DIR]... [-L DIR]... [--coverage DIR] SOURCE.cbl CASES-DIR");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut copy = Vec::new();
    let mut opts = Options::default();
    let mut pos = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "-I" => match it.next() {
                Some(d) => copy.push(PathBuf::from(d)),
                None => return usage(),
            },
            "-L" => match it.next() {
                Some(d) => opts.search.push(PathBuf::from(d)),
                None => return usage(),
            },
            "--coverage" => match it.next() {
                Some(d) => zeus_cobol::coverage::set_dir(std::path::Path::new(&d)),
                None => return usage(),
            },
            s if s.starts_with('-') => return usage(),
            s => pos.push(PathBuf::from(s)),
        }
    }
    let [source, dir] = &pos[..] else {
        return usage();
    };
    let fo = FrontOptions {
        copy_dirs: copy,
        ..Default::default()
    };
    let module = match zeus_cobol::compile_file(source, &fo) {
        Ok(m) => Rc::new(m),
        Err(d) => {
            eprintln!("ztest: {d}");
            return ExitCode::from(8);
        }
    };
    let cases = match read_dir(dir) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ztest: {e}");
            return ExitCode::from(8);
        }
    };
    let mut failed = 0;
    for (name, c) in &cases {
        match replay(&module, c, &opts) {
            Ok(d) if d.is_empty() => println!("{name}: ok"),
            Ok(d) => {
                failed += 1;
                println!("{name}: FAILED");
                for x in d {
                    println!("  {x}");
                }
            }
            Err(e) => {
                failed += 1;
                println!("{name}: {e}");
            }
        }
    }
    println!("ztest: {} cases, {failed} failed", cases.len());
    if failed > 0 {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

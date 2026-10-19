//! `ztestgen` - test cases covering both ways of every branch of a
//! COBOL program (C1), found by concolic execution and a solver.
//!
//! ```text
//! ztestgen [-I DIR]... [-L DIR]... [--program NAME] [--runs N] [--time SECONDS]
//!          [--limit INSTRUCTIONS] [--no-smt] [--threads N] [--stub PROGRAM]...
//!          [--seeds DIR]... [--out DIR] SOURCE.cbl
//! ```
//!
//! The inputs are the program's USING parameters and the lines it
//! ACCEPTs. The cases go to DIR (default `tests`) as `case-NNN.json`
//! with `report.txt`: the branch directions covered and why the others
//! are not. `ztest` runs the cases again. `--seeds` gives initial inputs:
//! case files (earlier cases to start from, or inputs written by hand
//! with only `fields`, by item name).

use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;

use zeus_cobol::front::FrontOptions;
use zeus_testgen::case::{case_file, line_of};
use zeus_testgen::engine::{Gap, Options, generate};

fn usage() -> ExitCode {
    eprintln!(
        "usage: ztestgen [-I DIR]... [-L DIR]... [--program NAME] [--runs N] [--time SECONDS] [--limit N] [--no-smt] [--threads N] [--stub PROGRAM]... [--seeds DIR]... [--out DIR] SOURCE.cbl"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut copy = Vec::new();
    let mut opts = Options::default();
    let mut out = PathBuf::from("tests");
    let mut program = None;
    let mut source = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut val = || it.next();
        match a.as_str() {
            "-I" => match val() {
                Some(d) => copy.push(PathBuf::from(d)),
                None => return usage(),
            },
            "-L" => match val() {
                Some(d) => opts.search.push(PathBuf::from(d)),
                None => return usage(),
            },
            "--program" => program = val().map(|p| p.to_ascii_uppercase()),
            "--out" | "-o" => match val() {
                Some(d) => out = d.into(),
                None => return usage(),
            },
            "--no-smt" => opts.smt = false,
            "--stub" => match val() {
                Some(p) => {
                    opts.stub.insert(p.to_ascii_uppercase());
                }
                None => return usage(),
            },
            "--threads" => match val().and_then(|n| n.parse().ok()) {
                Some(n) => opts.threads = n,
                None => return usage(),
            },
            "--seeds" => match val() {
                Some(d) => match zeus_testgen::case::read_dir(std::path::Path::new(&d)) {
                    Ok(cs) => opts.seeds.extend(cs.into_iter().map(|(_, c)| c)),
                    Err(e) => {
                        eprintln!("ztestgen: {e}");
                        return ExitCode::from(8);
                    }
                },
                None => return usage(),
            },
            "--runs" => match val().and_then(|n| n.parse().ok()) {
                Some(n) => opts.max_runs = n,
                None => return usage(),
            },
            "--time" => match val().and_then(|n| n.parse().ok()) {
                Some(n) => opts.time = Duration::from_secs(n),
                None => return usage(),
            },
            "--limit" => match val().and_then(|n| n.parse().ok()) {
                Some(n) => opts.limit = n,
                None => return usage(),
            },
            s if s.starts_with('-') => return usage(),
            s => source = Some(PathBuf::from(s)),
        }
    }
    let Some(source) = source else { return usage() };
    let fo = FrontOptions {
        copy_dirs: copy,
        ..Default::default()
    };
    let module = match zeus_cobol::compile_file(&source, &fo) {
        Ok(m) => Rc::new(m),
        Err(d) => {
            eprintln!("ztestgen: {d}");
            return ExitCode::from(8);
        }
    };
    let program = program.unwrap_or_else(|| module.programs[0].name.clone());
    let report = match generate(&module, &program, &opts) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("ztestgen: {e}");
            return ExitCode::from(8);
        }
    };
    if let Err(e) = std::fs::create_dir_all(&out) {
        eprintln!("ztestgen: {}: {e}", out.display());
        return ExitCode::from(8);
    }
    for (k, c) in report.cases.iter().enumerate() {
        let f = case_file(&module, &program, &report.fields, &report.files, c);
        let path = out.join(format!("case-{:03}.json", k + 1));
        let text = serde_json::to_string_pretty(&f).unwrap_or_default();
        if let Err(e) = std::fs::write(&path, text + "\n") {
            eprintln!("ztestgen: {}: {e}", path.display());
            return ExitCode::from(8);
        }
    }
    let total = report.all.len();
    let infeasible = report
        .gaps
        .iter()
        .filter(|(_, g)| matches!(g, Gap::Infeasible | Gap::Impossible))
        .count();
    let covered = total - report.gaps.len();
    let pct = |a: usize, b: usize| {
        if b == 0 {
            100.0
        } else {
            100.0 * a as f64 / b as f64
        }
    };
    let mut text = format!(
        "program {program}: {} runs, {} cases\nC1: {covered} of {total} branch directions ({:.1}%), {:.1}% of the feasible ({infeasible} infeasible)\n",
        report.runs,
        report.cases.len(),
        pct(covered, total),
        pct(covered, total - infeasible),
    );
    text.push_str(if report.smt {
        "solver: built-in search, then Z3
"
    } else {
        "solver: built-in search
"
    });
    if !report.stubbed.is_empty() {
        let names: Vec<&str> = report.stubbed.iter().map(|s| s.as_str()).collect();
        text.push_str(&format!(
            "stubs (programs called that ran as stubs): {}\n",
            names.join(", ")
        ));
    }
    if !report.gaps.is_empty() {
        text.push_str("\nnot covered:\n");
        for ((p, pc, j), g) in &report.gaps {
            let why = match g {
                Gap::NotReached => "not reached",
                Gap::NotSymbolic => "condition does not depend on the inputs followed",
                Gap::Infeasible => "infeasible (no inputs on the paths tried)",
                Gap::NotFound => "no inputs found",
                Gap::Impossible => "impossible (static analysis: the values of flags and numbers)",
            };
            let line = line_of(&module, p, *pc).map_or("?".into(), |l| l.to_string());
            text.push_str(&format!(
                "  {p} line {line} (instruction {pc}) {}: {why}\n",
                if *j { "jump" } else { "next" }
            ));
        }
    }
    let _ = std::fs::write(out.join("report.txt"), &text);
    print!("{text}");
    ExitCode::SUCCESS
}

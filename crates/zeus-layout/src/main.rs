//! `zlayout` - fixed-length layouts of the data COBOL programs exchange,
//! as copybooks.
//!
//! ```text
//! zlayout [-I DIR]... [--bms DIR]... [--out DIR] [--kinds K,...] [--values] [--no-offsets]
//!         [--keep-base] [--max-layouts N] [--no-check] [--source-encoding E] [--ccsid N] PROGRAM.cbl...
//! ```
//!
//! `--bms DIR`: BMS mapsets under DIR give the symbolic maps programs COPY.
//! For each program: its files and cards (FD records, READ INTO, WRITE
//! FROM, ACCEPT), MQ message buffers, its parameters and the arguments of
//! the programs it calls, one copybook per layout (`PROGRAM.KIND.RESOURCE
//! [.LAYOUT].cpy`) in the output directory (default `layouts`), with the
//! index `layouts.csv`. Each copybook is compiled and its positions
//! compared with the program's (`--no-check`: not). The exit status is 4
//! when a check fails, 8 when a program cannot be read.

use std::path::PathBuf;
use std::process::ExitCode;
use zeus_cobol::front::SourceEncoding;

use zeus_layout::{Options, analyze_file, write};

fn usage() -> ExitCode {
    eprintln!(
        "usage: zlayout [-I DIR]... [--bms DIR]... [--out DIR] [--kinds K,...] [--values] [--no-offsets] [--keep-base] [--max-layouts N] [--no-check] [--source-encoding E] [--ccsid N] PROGRAM.cbl..."
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut opts = Options::default();
    let mut out = PathBuf::from("layouts");
    let mut sources = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "-I" | "--lib" => match it.next() {
                Some(d) => opts.copy_dirs.push(d.into()),
                None => return usage(),
            },
            "--out" | "-o" => match it.next() {
                Some(d) => out = d.into(),
                None => return usage(),
            },
            "--bms" => match it.next() {
                Some(d) => opts.bms_dirs.push(d.into()),
                None => return usage(),
            },
            "--kinds" => match it.next() {
                Some(k) => {
                    opts.kinds = k
                        .split(',')
                        .map(|s| s.trim().to_ascii_uppercase())
                        .collect()
                }
                None => return usage(),
            },
            "--max-layouts" => match it.next().and_then(|n| n.parse().ok()) {
                Some(n) => opts.max_layouts = n,
                None => return usage(),
            },
            "--source-encoding" => match it.next().and_then(|e| SourceEncoding::parse(&e)) {
                Some(e) => opts.source_encoding = e,
                None => return usage(),
            },
            "--ccsid" => match it.next().and_then(|c| c.parse().ok()) {
                Some(c) if zeus_cobol::codepage::set_active(c) => {}
                _ => return usage(),
            },
            "--values" => opts.values = true,
            "--no-offsets" => opts.offsets = false,
            "--keep-base" => opts.keep_base = true,
            "--no-check" => opts.check = false,
            s if s.starts_with("-I") && s.len() > 2 => opts.copy_dirs.push(s[2..].into()),
            s if s.starts_with('-') => return usage(),
            s => sources.push(PathBuf::from(s)),
        }
    }
    if sources.is_empty() {
        return usage();
    }
    let _maps = match zeus_layout::symbolic_maps(&mut opts) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("zlayout: symbolic maps: {e}");
            return ExitCode::from(8);
        }
    };
    let mut all = Vec::new();
    let mut status = ExitCode::SUCCESS;
    for s in &sources {
        match analyze_file(s, &opts) {
            Ok(l) => all.extend(l),
            Err(e) => {
                eprintln!("zlayout: {e}");
                status = ExitCode::from(8);
            }
        }
    }
    if let Err(e) = write(&all, &out) {
        eprintln!("zlayout: {}: {e}", out.display());
        return ExitCode::from(8);
    }
    let failed: Vec<_> = all
        .iter()
        .filter_map(|l| match &l.check {
            Some(Err(e)) => Some((l, e)),
            _ => None,
        })
        .collect();
    for (l, e) in &failed {
        eprintln!("zlayout: {}: check failed: {e}", l.file_name);
    }
    println!(
        "zlayout: {} layouts of {} programs written to {}{}",
        all.len(),
        all.iter()
            .map(|l| &l.program)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        out.display(),
        if failed.is_empty() {
            String::new()
        } else {
            format!(", {} checks failed", failed.len())
        }
    );
    if !failed.is_empty() && status == ExitCode::SUCCESS {
        status = ExitCode::from(4);
    }
    status
}

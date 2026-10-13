//! `zdeps` - dependency analysis of COBOL, PL/I and JCL sources.
//!
//! ```text
//! zdeps analyze [--lib DIR]... [--out DIR] [--no-compile] [--bom] ROOT...
//! zdeps masters [--out DIR] [--bom]
//! ```
//!
//! `analyze` follows the references of the root files (or of every
//! source file under the root directories) recursively, looking names up
//! in the library directories, and writes to the output directory
//! (default `deps`) the masters and the results as CSV: nodes.csv,
//! edges.csv, node_attributes.csv, edge_attributes.csv,
//! edge_locations.csv and missing_assets.csv. COBOL and PL/I programs are
//! checked with the Zeus compilers unless `--no-compile` is given.
//! `--bom` starts the files with a UTF-8 byte order mark (for
//! spreadsheet programs). The exit status is 4 when assets are missing.

use std::path::PathBuf;
use std::process::ExitCode;

use zeus_deps::{graph, report};

fn usage() -> ExitCode {
    eprintln!("usage: zdeps analyze [--lib DIR]... [--out DIR] [--no-compile] [--bom] ROOT...");
    eprintln!("       zdeps masters [--out DIR] [--bom]");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(cmd) = args.first().cloned() else {
        return usage();
    };
    let mut opts = graph::Options {
        compile: true,
        ..Default::default()
    };
    let mut out = PathBuf::from("deps");
    let mut bom = false;
    let mut it = args[1..].iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--lib" | "-I" | "-L" => match it.next() {
                Some(d) => opts.libs.push(PathBuf::from(d)),
                None => return usage(),
            },
            "--out" | "-o" => match it.next() {
                Some(d) => out = PathBuf::from(d),
                None => return usage(),
            },
            "--no-compile" => opts.compile = false,
            "--bom" => bom = true,
            s if s.starts_with('-') => return usage(),
            s => opts.roots.push(PathBuf::from(s)),
        }
    }
    if let Err(e) = report::write_masters(&out, bom) {
        eprintln!("zdeps: {}: {e}", out.display());
        return ExitCode::from(8);
    }
    match cmd.as_str() {
        "masters" => ExitCode::SUCCESS,
        "analyze" => {
            if opts.roots.is_empty() {
                return usage();
            }
            for r in &opts.roots {
                if !r.exists() {
                    eprintln!("zdeps: {}: not found", r.display());
                    return ExitCode::from(8);
                }
            }
            let g = graph::analyze(&opts);
            let base = std::env::current_dir().unwrap_or_default();
            if let Err(e) = report::write_results(&g, &out, &base, bom) {
                eprintln!("zdeps: {}: {e}", out.display());
                return ExitCode::from(8);
            }
            let files = g.nodes.iter().filter(|n| n.status == "FILE").count();
            let missing = g.nodes.iter().filter(|n| n.status == "MISSING").count();
            let dynamic = g.nodes.iter().filter(|n| n.status == "DYNAMIC").count();
            let errors = g
                .nodes
                .iter()
                .filter(|n| {
                    n.attrs
                        .iter()
                        .any(|(k, v)| k.ends_with("_status") && v == "ERROR")
                })
                .count();
            println!(
                "zdeps: {} nodes ({files} files, {missing} missing, {dynamic} dynamic), {} edges, {errors} files with errors; written to {}",
                g.nodes.len(),
                g.edges.len(),
                out.display()
            );
            if missing > 0 {
                ExitCode::from(4)
            } else {
                ExitCode::SUCCESS
            }
        }
        _ => usage(),
    }
}

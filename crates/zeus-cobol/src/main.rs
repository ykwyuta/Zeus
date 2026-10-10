//! `zcobc` - the Zeus COBOL compiler driver.
//!
//! The compiler itself is not implemented yet (see docs/proposal/0003). This
//! driver already fixes the command-line contract that the CCVS85 harness
//! (`zeus-ccvs`) uses, so the harness can report the compiler's results from
//! day one:
//!
//! ```text
//! zcobc compile [-I <dir>]... [--subprogram] -o <artifact> <source>
//! zcobc run <artifact>
//! ```
//!
//! Both subcommands currently fail with exit status 1.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("compile") | Some("run") => {
            eprintln!("zcobc: not implemented yet");
            ExitCode::from(1)
        }
        Some("--version") => {
            println!("zcobc {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: zcobc compile [-I <dir>]... [--subprogram] -o <artifact> <source>");
            eprintln!("       zcobc run <artifact>");
            ExitCode::from(2)
        }
    }
}

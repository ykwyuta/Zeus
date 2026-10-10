//! `zeus-hercdiff` - records Hercules results for decimal instructions.
//!
//! ```text
//! zeus-hercdiff run --count 2000 --seed 1 --out work/decimal-oracle.json
//! ```

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Result, bail};
use clap::Parser;

use zeus_hercdiff::cases::{self, Op, Rng};
use zeus_hercdiff::hercules::Hercules;
use zeus_hercdiff::image;
use zeus_hercdiff::{OracleRun, Record};

#[derive(Parser)]
#[command(
    version,
    about = "Record Hercules results for packed-decimal instructions"
)]
enum Cli {
    /// Generate cases, run them on Hercules and write the results.
    Run {
        /// Cases per instruction.
        #[arg(long, default_value_t = 200)]
        count: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
        /// Enable the decimal-overflow program mask (interrupt instead of CC3).
        #[arg(long)]
        decimal_overflow_interrupt: bool,
        #[arg(long, default_value = "hercules")]
        hercules: PathBuf,
        #[arg(long, default_value = "work/hercules")]
        work: PathBuf,
        #[arg(long, default_value_t = 2)]
        run_seconds: u32,
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    match real_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("zeus-hercdiff: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn real_main() -> Result<()> {
    let Cli::Run {
        count,
        seed,
        decimal_overflow_interrupt,
        hercules,
        work,
        run_seconds,
        out,
    } = Cli::parse();
    if count == 0 {
        bail!("--count must be positive");
    }
    let mut rng = Rng::new(seed);
    let cases: Vec<_> = Op::ALL
        .iter()
        .flat_map(|&op| (0..count).map(move |_| op))
        .map(|op| cases::generate(&mut rng, op))
        .collect();

    let herc = Hercules {
        binary: hercules,
        run_seconds,
    };
    let core = image::build(&cases, decimal_overflow_interrupt);
    if !herc.available() {
        bail!("{} not found", herc.binary.display());
    }
    let (dump, version) = herc.run(&work, &core, image::dump_end(cases.len()))?;
    let observed = image::decode(&cases, &dump)?;

    let records: Vec<Record> = cases
        .into_iter()
        .zip(observed)
        .map(|(case, result)| Record {
            instruction: image::instruction(&case).to_vec(),
            case,
            result,
        })
        .collect();

    // Per-instruction summary: condition codes and interruptions seen.
    let mut summary: BTreeMap<&str, BTreeMap<String, usize>> = BTreeMap::new();
    for r in &records {
        let key = match (r.result.cc, r.result.interruption) {
            (Some(cc), _) => format!("cc{cc}"),
            (None, Some(code)) => format!("pic{code:04X}"),
            (None, None) => "none".to_string(),
        };
        *summary
            .entry(r.case.op.mnemonic())
            .or_default()
            .entry(key)
            .or_default() += 1;
    }
    for (op, outcomes) in &summary {
        let parts: Vec<String> = outcomes.iter().map(|(k, n)| format!("{k}={n}")).collect();
        println!("{op:<5} {}", parts.join(" "));
    }

    let run = OracleRun {
        oracle: version,
        architecture: "ESA/390".to_string(),
        seed,
        decimal_overflow_interrupt,
        records,
    };
    fs::write(&out, serde_json::to_string_pretty(&run)? + "\n")?;
    println!("{} records written to {}", run.records.len(), out.display());
    Ok(())
}

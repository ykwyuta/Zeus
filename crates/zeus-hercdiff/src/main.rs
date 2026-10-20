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
use zeus_hercdiff::{OracleRun, Record};
use zeus_hercdiff::{emulate, image, zhvs, zpvs};

#[derive(Parser)]
#[command(
    version,
    after_help = include_str!("../../../docs/help/zeus-hercdiff.txt"),
    about = "Record Hercules results for packed-decimal instructions and validation suites"
)]
enum Cli {
    /// Generate cases, run them on Hercules and write the results.
    #[command(after_help = include_str!("../../../docs/help/zeus-hercdiff.txt"))]
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
        /// Also compare the results with zeus-decimal (exit 1 on mismatch).
        #[arg(long)]
        compare: bool,
    },
    /// Compare a recorded oracle run with zeus-decimal.
    #[command(after_help = include_str!("../../../docs/help/zeus-hercdiff.txt"))]
    Compare {
        #[arg(long)]
        oracle: PathBuf,
        /// Show at most this many mismatches.
        #[arg(long, default_value_t = 20)]
        show: usize,
    },
    /// Assemble, link and run ZHVS (HLASM Validation Suite) programs on bare-metal Hercules (z/Arch).
    #[command(after_help = include_str!("../../../docs/help/zeus-hercdiff.txt"))]
    Zhvs {
        /// Path to the `zhvs` suite directory.
        #[arg(long, default_value = "zhvs")]
        suite: PathBuf,
        #[arg(long, default_value = "hercules")]
        hercules: PathBuf,
        #[arg(long, default_value = "work/hercules-zhvs")]
        work: PathBuf,
        #[arg(long, default_value_t = 2)]
        run_seconds: u32,
        /// Comma-separated list of programs to run (default: all 28 bare-metal programs).
        #[arg(long, value_delimiter = ',')]
        programs: Vec<String>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Compile, link and run ZPVS (PL/I Validation Suite) programs on MVS 3.8j (PGM=IEMAA) under Hercules.
    #[command(after_help = include_str!("../../../docs/help/zeus-hercdiff.txt"))]
    Zpvs {
        /// Path to the `zpvs` suite directory.
        #[arg(long, default_value = "zpvs")]
        suite: PathBuf,
        /// Directory containing MVS 3.8j `.cckd` DASD images (`mvsres.140.cckd`, etc.).
        #[arg(long)]
        mvs_dasd: PathBuf,
        #[arg(long, default_value = "hercules")]
        hercules: PathBuf,
        #[arg(long, default_value = "work/hercules-zpvs")]
        work: PathBuf,
        #[arg(long, default_value_t = 6)]
        run_seconds: u32,
        /// Comma-separated list of programs to run (default: all PL/I (F)-compatible programs).
        #[arg(long, value_delimiter = ',')]
        programs: Vec<String>,
        #[arg(long)]
        out: PathBuf,
    },
}

fn main() -> ExitCode {
    match real_main() {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("zeus-hercdiff: {e:#}");
            ExitCode::from(2)
        }
    }
}

/// Prints mismatches between the oracle and zeus-decimal; true if none.
fn report_compare(run: &OracleRun, show: usize) -> bool {
    let bad = emulate::mismatches(&run.records);
    for (r, ours) in bad.iter().take(show) {
        println!(
            "MISMATCH {} {} op1={} op2={}{}",
            r.case.op.mnemonic(),
            cases::hex::encode(&r.instruction),
            cases::hex::encode(&r.case.op1),
            cases::hex::encode(&r.case.op2),
            r.case
                .srp
                .map_or(String::new(), |(s, i)| format!(" shift={s:02X} round={i}"))
        );
        println!("  oracle: {:?}", r.result);
        println!("  zeus:   {ours:?}");
    }
    println!(
        "compared {} records with zeus-decimal: {} mismatches",
        run.records.len(),
        bad.len()
    );
    bad.is_empty()
}

fn real_main() -> Result<bool> {
    let (count, seed, decimal_overflow_interrupt, hercules, work, run_seconds, out, compare) =
        match Cli::parse() {
            Cli::Compare { oracle, show } => {
                let run: OracleRun = serde_json::from_str(&fs::read_to_string(&oracle)?)?;
                return Ok(report_compare(&run, show));
            }
            Cli::Zhvs {
                suite,
                hercules,
                work,
                run_seconds,
                programs,
                out,
            } => {
                let herc = Hercules {
                    binary: hercules,
                    run_seconds,
                };
                if !herc.available() {
                    bail!("{} not found", herc.binary.display());
                }
                let res = zhvs::run_zhvs(&suite, &herc, &work, &programs)?;
                fs::write(&out, serde_json::to_string_pretty(&res)? + "\n")?;
                println!(
                    "ZHVS: {}/{} programs completed, {}/{} checks passed -> {}",
                    res.completed_programs,
                    res.total_programs,
                    res.passed_checks,
                    res.total_checks,
                    out.display()
                );
                return Ok(res.completed_programs == res.total_programs);
            }
            Cli::Zpvs {
                suite,
                mvs_dasd,
                hercules,
                work,
                run_seconds,
                programs,
                out,
            } => {
                let herc = Hercules {
                    binary: hercules,
                    run_seconds,
                };
                if !herc.available() {
                    bail!("{} not found", herc.binary.display());
                }
                let res = zpvs::run_zpvs(&suite, &mvs_dasd, &herc, &work, &programs)?;
                fs::write(&out, serde_json::to_string_pretty(&res)? + "\n")?;
                println!(
                    "ZPVS: {} programs, {}/{} checks passed -> {}",
                    res.total_programs,
                    res.passed_checks,
                    res.total_checks,
                    out.display()
                );
                return Ok(res.passed_checks > 0);
            }
            Cli::Run {
                count,
                seed,
                decimal_overflow_interrupt,
                hercules,
                work,
                run_seconds,
                out,
                compare,
            } => (
                count,
                seed,
                decimal_overflow_interrupt,
                hercules,
                work,
                run_seconds,
                out,
                compare,
            ),
        };
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
    if compare {
        if decimal_overflow_interrupt {
            bail!("--compare supports only runs with the decimal-overflow mask off");
        }
        return Ok(report_compare(&run, 20));
    }
    Ok(true)
}

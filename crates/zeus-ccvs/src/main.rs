//! `zeus-ccvs` - NIST CCVS85 harness.
//!
//! ```text
//! zeus-ccvs extract  --newcob newcob.val --config ccvs85/exec85.conf --out work/suite \
//!                    [--overlay ccvs85/exec85-zeus.conf] [--verify ccvs85/extract-manifest.txt]
//! zeus-ccvs run      --suite work/suite --toolchain gnucobol|zeus --out results.json
//! zeus-ccvs report   --results results.json [--baseline baseline.json] [--programs]
//! zeus-ccvs manifest --out manifest.txt <EXEC85 newcob.tmp files>...
//! ```

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Duration;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};

use zeus_ccvs::{exec85, extract, population, runner, summary, toolchain::Toolchain};

#[derive(Parser)]
#[command(version, about = "NIST CCVS85 COBOL-85 test suite harness")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Clone, Copy, ValueEnum)]
enum ToolchainKind {
    Gnucobol,
    Zeus,
    /// zcobc compile --native: native executables with machine code
    ZeusNative,
}

#[derive(Subcommand)]
enum Cmd {
    /// Extract the test programs from the population file.
    Extract {
        #[arg(long)]
        newcob: PathBuf,
        /// EXEC85 control-card file (X-card values).
        #[arg(long, default_value = "ccvs85/exec85.conf")]
        config: PathBuf,
        /// X-card overlays applied after the configuration (for example
        /// the native collating sequence of an EBCDIC implementation).
        #[arg(long)]
        overlay: Vec<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        /// Check the extraction against a reference manifest.
        #[arg(long)]
        verify: Option<PathBuf>,
    },
    /// Compile and run the extracted suite.
    Run {
        #[arg(long)]
        suite: PathBuf,
        #[arg(long, default_value = "work/run")]
        work: PathBuf,
        #[arg(long, value_enum)]
        toolchain: ToolchainKind,
        /// Path of GnuCOBOL's `cobc`.
        #[arg(long)]
        cobc: Option<PathBuf>,
        /// Path of `zcobc` (default: next to this executable).
        #[arg(long)]
        zcobc: Option<PathBuf>,
        /// Comma-separated modules (default: all present in the suite).
        #[arg(long, value_delimiter = ',')]
        modules: Vec<String>,
        /// Comma-separated programs to run (default: all).
        #[arg(long, value_delimiter = ',')]
        programs: Vec<String>,
        #[arg(long, default_value_t = 4)]
        jobs: usize,
        /// Per-step timeout in seconds.
        #[arg(long, default_value_t = 120)]
        timeout: u64,
        #[arg(long)]
        out: PathBuf,
    },
    /// Print a summary of a results file.
    Report {
        #[arg(long)]
        results: PathBuf,
        /// Results to compare against (e.g. the GnuCOBOL baseline).
        #[arg(long)]
        baseline: Option<PathBuf>,
        /// Also list every program.
        #[arg(long)]
        programs: bool,
    },
    /// Build a reference manifest from EXEC85 output files.
    Manifest {
        #[arg(long)]
        out: PathBuf,
        files: Vec<PathBuf>,
    },
}

fn main() -> ExitCode {
    match real_main() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("zeus-ccvs: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn read(path: &PathBuf) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    // newcob.val is ASCII; tolerate stray bytes.
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn load_results(path: &PathBuf) -> Result<runner::SuiteResult> {
    serde_json::from_str(&read(path)?).with_context(|| format!("parsing {}", path.display()))
}

fn real_main() -> Result<ExitCode> {
    match Cli::parse().cmd {
        Cmd::Extract {
            newcob,
            config,
            overlay,
            out,
            verify,
        } => {
            let members = population::parse(&read(&newcob)?)?;
            let mut cfg = exec85::Config::parse(&read(&config)?)?;
            for o in &overlay {
                cfg.overlay(&read(o)?)?;
            }
            let extracted = extract::transform(&members, &cfg);
            extract::write_tree(&extracted, &out)?;
            println!(
                "extracted {} members into {}",
                extracted.len(),
                out.display()
            );
            if let Some(manifest) = verify {
                let reference = extract::parse_manifest(&read(&manifest)?)?;
                let diffs = extract::verify(&extract::manifest_of(&extracted), &reference);
                if !diffs.is_empty() {
                    for d in &diffs {
                        eprintln!("{d}");
                    }
                    eprintln!("{} differences against {}", diffs.len(), manifest.display());
                    return Ok(ExitCode::from(1));
                }
                println!(
                    "identical to reference manifest ({} members)",
                    reference.len()
                );
            }
        }
        Cmd::Run {
            suite,
            work,
            toolchain,
            cobc,
            zcobc,
            modules,
            programs,
            jobs,
            timeout,
            out,
        } => {
            let tc = match toolchain {
                ToolchainKind::Gnucobol => Toolchain::gnucobol(cobc),
                ToolchainKind::Zeus | ToolchainKind::ZeusNative => {
                    let zcobc = match zcobc {
                        Some(p) => p,
                        None => std::env::current_exe()?.with_file_name("zcobc"),
                    };
                    let zcobc = zcobc.canonicalize().context("locating zcobc")?;
                    if matches!(toolchain, ToolchainKind::Zeus) {
                        Toolchain::zeus(zcobc)
                    } else {
                        Toolchain::zeus_native(zcobc)
                    }
                }
            };
            let modules = if modules.is_empty() {
                runner::MODULES
                    .iter()
                    .filter(|m| suite.join(m).is_dir())
                    .map(|m| m.to_string())
                    .collect()
            } else {
                modules
            };
            let opts = runner::RunOptions {
                suite: suite.canonicalize().context("locating suite")?,
                work: work.join(tc.name()),
                modules,
                programs,
                jobs,
                timeout: Duration::from_secs(timeout),
            };
            let result = runner::run(&tc, &opts)?;
            fs::write(&out, serde_json::to_string_pretty(&result)? + "\n")?;
            print!("{}", summary::markdown(&result, None));
        }
        Cmd::Report {
            results,
            baseline,
            programs,
        } => {
            let current = load_results(&results)?;
            let base = baseline.as_ref().map(load_results).transpose()?;
            print!("{}", summary::markdown(&current, base.as_ref()));
            if programs {
                println!("\n```");
                print!("{}", summary::program_report(&current));
                println!("```");
            }
        }
        Cmd::Manifest { out, files } => {
            let texts = files.iter().map(read).collect::<Result<Vec<_>>>()?;
            let manifest = extract::manifest_from_exec85_output(&texts)?;
            fs::write(&out, extract::format_manifest(&manifest))?;
            println!("{} members written to {}", manifest.len(), out.display());
        }
    }
    Ok(ExitCode::SUCCESS)
}

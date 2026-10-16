//! Compiles and runs the extracted suite with a toolchain.
//!
//! For every selected module the extracted directory is copied to a fresh
//! work directory. Called subprograms (`lib/`) are compiled first; then the
//! main (`.CBL`) and follow-on (`.SUB`) programs are processed in name order,
//! so a follow-on program sees the files its parent left behind. Files named
//! `XXXXX*` (the suite's data files) are removed before each main program.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::jcl::{DdKind, parse_dd_allocations};
use crate::manifest::{ExpectKind, Manifest, ProgramConfig};
use crate::report::{self, Counts};
use crate::special::special;
use crate::toolchain::Toolchain;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum Outcome {
    /// The program ran and its report was evaluated.
    Ran {
        counts: Counts,
    },
    /// Compile-time test: compiled successfully, not run.
    Compiled,
    CompileError,
    ExecuteError {
        code: Option<i32>,
    },
    Timeout,
}

impl Outcome {
    pub fn counts(&self) -> Counts {
        match self {
            Outcome::Ran { counts } => *counts,
            _ => Counts::default(),
        }
    }

    /// The program ran (or compiled, for compile-time tests) without a
    /// failing test.
    pub fn ok(&self) -> bool {
        match self {
            Outcome::Ran { counts } => counts.fail == 0,
            Outcome::Compiled => true,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramResult {
    pub program: String,
    #[serde(flatten)]
    pub outcome: Outcome,
    pub seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleResult {
    pub module: String,
    pub programs: Vec<ProgramResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuiteResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suite: Option<String>,
    pub toolchain: String,
    pub version: String,
    pub modules: Vec<ModuleResult>,
}

#[derive(Debug, Clone)]
pub struct RunOptions {
    /// Extracted suite (output of `zeus-ccvs extract`) or ZCVS directory.
    pub suite: PathBuf,
    /// Scratch directory for compilation and execution.
    pub work: PathBuf,
    pub modules: Vec<String>,
    /// Restrict to these programs (all when empty).
    pub programs: Vec<String>,
    pub jobs: usize,
    pub timeout: Duration,
}

pub fn run(tc: &Toolchain, opts: &RunOptions) -> Result<SuiteResult> {
    let manifest = Manifest::load(&opts.suite)?;
    fs::create_dir_all(&opts.work)?;
    copy_dir(&opts.suite.join("copy"), &opts.work.join("copy"))?;
    let copyalt_src = opts.suite.join("copyalt");
    if copyalt_src.is_dir() {
        copy_dir(&copyalt_src, &opts.work.join("copyalt"))?;
        if !opts.work.join("COPYALT").exists() {
            let _ = copy_dir(&copyalt_src, &opts.work.join("COPYALT"));
        }
    }
    let queue = Mutex::new(opts.modules.clone());
    let results = Mutex::new(Vec::new());
    let errors = Mutex::new(Vec::new());
    std::thread::scope(|s| {
        for _ in 0..opts.jobs.max(1) {
            s.spawn(|| {
                loop {
                    let Some(module) = queue.lock().unwrap().pop() else {
                        break;
                    };
                    match run_module(tc, opts, manifest.as_ref(), &module) {
                        Ok(r) => results.lock().unwrap().push(r),
                        Err(e) => errors.lock().unwrap().push(format!("{module}: {e:#}")),
                    }
                }
            });
        }
    });
    let errors = errors.into_inner().unwrap();
    if !errors.is_empty() {
        anyhow::bail!("{}", errors.join("\n"));
    }
    let mut modules = results.into_inner().unwrap();
    modules.sort_by_key(|m: &ModuleResult| order(&m.module));
    Ok(SuiteResult {
        suite: manifest.and_then(|m| (!m.name.is_empty()).then_some(m.name)),
        toolchain: tc.name().to_string(),
        version: tc.version(),
        modules,
    })
}

/// Canonical CCVS85 module order for reports.
pub const MODULES: &[&str] = &[
    "NC", "SM", "IC", "SQ", "RL", "IX", "ST", "IF", "SG", "OB", "DB", "RW", "CM",
];

/// Canonical ZCVS module order for reports.
pub const ZCVS_MODULES: &[&str] = &["ZD", "ZA", "ZN", "ZP", "ZF", "ZS", "ZE", "ZX"];

/// Combined canonical module order across CCVS85 and ZCVS.
pub const ALL_MODULES: &[&str] = &[
    "NC", "SM", "IC", "SQ", "RL", "IX", "ST", "IF", "SG", "OB", "DB", "RW", "CM", "ZD", "ZA", "ZN",
    "ZP", "ZF", "ZS", "ZE", "ZX",
];

pub fn order(module: &str) -> usize {
    ALL_MODULES
        .iter()
        .position(|m| *m == module)
        .unwrap_or(ALL_MODULES.len())
}

fn run_module(
    tc: &Toolchain,
    opts: &RunOptions,
    manifest: Option<&Manifest>,
    module: &str,
) -> Result<ModuleResult> {
    let src = opts.suite.join(module);
    let dir = opts.work.join(module);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    copy_dir(&src, &dir).with_context(|| format!("module {module} not found in suite"))?;
    let dir = dir.canonicalize()?;
    let ibm_std = manifest.is_some();

    // Called subprograms first.
    let lib = dir.join("lib");
    if lib.is_dir() {
        for source in sorted_files(&lib, &["CBL"])? {
            let program = stem(&source);
            let rel = Path::new("lib").join(source.file_name().unwrap());
            let mut cmd = tc.compile_subprogram_with(&program, &rel, ibm_std);
            let log = dir.join(format!("{program}.compile.log"));
            run_logged(&mut cmd, &dir, &log, opts.timeout)?;
        }
    }

    let mut programs = Vec::new();
    for source in sorted_files(&dir, &["CBL", "SUB"])? {
        let program = stem(&source);
        if !opts.programs.is_empty() && !opts.programs.contains(&program) {
            continue;
        }
        let cfg = manifest
            .and_then(|m| m.programs.get(&program))
            .cloned()
            .unwrap_or_default();
        let start = Instant::now();
        let outcome = run_program(tc, opts, &dir, module, &source, &program, &cfg, ibm_std)?;
        programs.push(ProgramResult {
            program,
            outcome,
            seconds: (start.elapsed().as_secs_f64() * 1000.0).round() / 1000.0,
        });
    }
    Ok(ModuleResult {
        module: module.to_string(),
        programs,
    })
}

#[allow(clippy::too_many_arguments)]
fn run_program(
    tc: &Toolchain,
    opts: &RunOptions,
    dir: &Path,
    module: &str,
    source: &Path,
    program: &str,
    cfg: &ProgramConfig,
    ibm_std: bool,
) -> Result<Outcome> {
    let sp = special(program);
    for a in tc.artifacts(program) {
        let _ = fs::remove_file(dir.join(a));
    }
    let rel = Path::new(source.file_name().unwrap());
    let mut compile = tc.compile_main_with(program, rel, sp.needs, cfg.ccsid, ibm_std);
    let log = dir.join(format!("{program}.compile.log"));
    let compile_status = run_logged(&mut compile, dir, &log, opts.timeout)?;
    if cfg.expect == ExpectKind::CompileError {
        return match compile_status {
            None => Ok(Outcome::Timeout),
            Some(status) if status.success() => {
                // Expected a compile error, but compilation succeeded!
                Ok(Outcome::Ran {
                    counts: Counts {
                        total: 1,
                        fail: 1,
                        ..Counts::default()
                    },
                })
            }
            Some(_) => {
                let diag_ok = match (&cfg.diagnostic_contains, tc) {
                    (Some(needle), Toolchain::Zeus { .. } | Toolchain::ZeusNative { .. }) => {
                        let text = fs::read_to_string(&log).unwrap_or_default();
                        text.to_ascii_lowercase()
                            .contains(&needle.to_ascii_lowercase())
                    }
                    _ => true,
                };
                if diag_ok {
                    Ok(Outcome::Ran {
                        counts: Counts {
                            total: 1,
                            pass: 1,
                            ..Counts::default()
                        },
                    })
                } else {
                    Ok(Outcome::Ran {
                        counts: Counts {
                            total: 1,
                            fail: 1,
                            ..Counts::default()
                        },
                    })
                }
            }
        };
    }

    match compile_status {
        Some(status) if status.success() => {}
        Some(_) => return Ok(Outcome::CompileError),
        None => return Ok(Outcome::Timeout),
    }
    if sp.compile_only || cfg.expect == ExpectKind::CompileOnly {
        return Ok(Outcome::Compiled);
    }

    if source.extension().is_some_and(|e| e == "CBL") {
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("XXXXX"))
            {
                if path.is_dir() {
                    fs::remove_dir_all(&path)?;
                } else {
                    fs::remove_file(&path)?;
                }
            }
        }
    }

    let report_file = dir.join(format!("{program}.log"));
    let _ = fs::remove_file(&report_file);
    let mut cmd = tc.run_in(dir, program, cfg.parm.as_deref());
    cmd.env("DD_REPORT", &report_file);
    tc.debug_env(&mut cmd, module == "DB" && program != "DB102A");
    if module == "RW" {
        cmd.env("DD_XXXXX049", dir.join(format!("{program}.rep")));
    }
    if let Some(jcl_name) = &cfg.jcl {
        let jcl_path = dir.join(jcl_name);
        let jcl_text =
            fs::read_to_string(&jcl_path).with_context(|| format!("reading {}", jcl_path.display()))?;
        for dd in parse_dd_allocations(&jcl_text) {
            let ext = match dd.kind {
                DdKind::Tape => "aws",
                DdKind::Dataset => "dat",
            };
            let dd_path = dir.join(format!("{}.{ext}", dd.ddname));
            let _ = fs::remove_file(&dd_path);
            cmd.env(format!("DD_{}", dd.ddname), &dd_path);
        }
    }
    let stdin_data = match sp.stdin {
        Some(bytes) => Some(bytes.to_vec()),
        None => {
            let dat = dir.join(format!("{program}.DAT"));
            dat.exists().then(|| fs::read(&dat)).transpose()?
        }
    };
    let stdout_file = dir.join(format!("{program}.out"));
    let status = run_with_input(&mut cmd, dir, stdin_data, &stdout_file, opts.timeout)?;
    let Some(status) = status else {
        return Ok(Outcome::Timeout);
    };
    if let Some(expected_rc) = cfg.expect_exit_code {
        if status.code() != Some(expected_rc) {
            return Ok(Outcome::ExecuteError {
                code: status.code(),
            });
        }
    } else if !status.success() && !sp.may_fail {
        return Ok(Outcome::ExecuteError {
            code: status.code(),
        });
    }
    let counts = if sp.no_output {
        Counts {
            total: 1,
            pass: 1,
            ..Counts::default()
        }
    } else {
        let report = fs::read(&report_file).unwrap_or_default();
        let stdout = fs::read(&stdout_file).unwrap_or_default();
        report::evaluate(sp.style, &report, &stdout)
    };
    Ok(Outcome::Ran { counts })
}

/// Runs `cmd` in `dir` with combined output to `log`. `None` on timeout.
fn run_logged(
    cmd: &mut Command,
    dir: &Path,
    log: &Path,
    timeout: Duration,
) -> Result<Option<std::process::ExitStatus>> {
    run_with_input(cmd, dir, None, log, timeout)
}

fn run_with_input(
    cmd: &mut Command,
    dir: &Path,
    input: Option<Vec<u8>>,
    output: &Path,
    timeout: Duration,
) -> Result<Option<std::process::ExitStatus>> {
    let out = fs::File::create(output)?;
    let err = out.try_clone()?;
    cmd.current_dir(dir)
        .stdout(out)
        .stderr(err)
        .stdin(if input.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    let mut child = cmd
        .spawn()
        .with_context(|| format!("starting {:?}", cmd.get_program()))?;
    if let Some(data) = input {
        let mut stdin = child.stdin.take().unwrap();
        // The program may exit without reading all of its input.
        let _ = stdin.write_all(&data);
    }
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn sorted_files(dir: &Path, exts: &[&str]) -> Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| exts.contains(&e))
        })
        .collect();
    files.sort();
    Ok(files)
}

fn stem(path: &Path) -> String {
    path.file_stem().unwrap().to_string_lossy().into_owned()
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    if !from.is_dir() {
        anyhow::bail!("{} is not a directory", from.display());
    }
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

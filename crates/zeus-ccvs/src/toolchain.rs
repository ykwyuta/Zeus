//! COBOL toolchains the suite can be run with.
//!
//! * `GnuCobol` - the comparison oracle (`cobc`), never part of Zeus itself.
//! * `Zeus` - the Zeus compiler driver `zcobc` (docs/proposal/0003).
//! * `ZeusNative` - `zcobc compile --native`: the main programs as native
//!   executables with machine code (docs/proposal/0009); called
//!   subprograms are modules as for `Zeus`.
//!
//! Both are invoked from inside a module directory (e.g. `work/NC`), with the
//! COPY library in `../copy`. Files are mapped to DD names through
//! `DD_<name>` environment variables, which both toolchains honour.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::special::Need;

#[derive(Debug, Clone)]
pub enum Toolchain {
    GnuCobol {
        cobc: PathBuf,
    },
    Zeus {
        zcobc: PathBuf,
    },
    ZeusNative {
        zcobc: PathBuf,
    },
    Zpli {
        compiler: PathBuf,
        native: bool,
    },
    Zas {
        compiler: PathBuf,
        format: AsmFormat,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum AsmFormat {
    Zeus,
    Obj,
    Goff,
}

impl Toolchain {
    pub fn source_extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Zpli { .. } => &["PLI"],
            Self::Zas { .. } => &["ASM"],
            _ => &["CBL", "SUB"],
        }
    }

    pub fn gnucobol(cobc: Option<PathBuf>) -> Self {
        Toolchain::GnuCobol {
            cobc: cobc.unwrap_or_else(|| PathBuf::from("cobc")),
        }
    }

    pub fn zeus(zcobc: PathBuf) -> Self {
        Toolchain::Zeus { zcobc }
    }

    pub fn zeus_native(zcobc: PathBuf) -> Self {
        Toolchain::ZeusNative { zcobc }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Toolchain::GnuCobol { .. } => "gnucobol",
            Toolchain::Zeus { .. } => "zeus",
            Toolchain::ZeusNative { .. } => "zeus-native",
            Self::Zpli { native: false, .. } => "zpli",
            Self::Zpli { native: true, .. } => "zpli-native",
            Self::Zas {
                format: AsmFormat::Zeus,
                ..
            } => "zas",
            Self::Zas {
                format: AsmFormat::Obj,
                ..
            } => "zas-obj",
            Self::Zas {
                format: AsmFormat::Goff,
                ..
            } => "zas-goff",
        }
    }

    /// Version string of the toolchain, for the results file.
    pub fn version(&self) -> String {
        let out = match self {
            Toolchain::GnuCobol { cobc } => Command::new(cobc).arg("--version").output(),
            Toolchain::Zeus { zcobc } | Toolchain::ZeusNative { zcobc } => {
                Command::new(zcobc).arg("--version").output()
            }
            Self::Zpli { compiler, .. } | Self::Zas { compiler, .. } => {
                Command::new(compiler).arg("--version").output()
            }
        };
        out.ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.lines().next().map(str::to_string))
            .unwrap_or_else(|| "unknown".to_string())
    }

    /// Command compiling the main program `source` (relative to the module
    /// directory) into an executable artifact named after `program`.
    pub fn compile_main(&self, program: &str, source: &Path, needs: &[Need]) -> Command {
        self.compile_main_with(program, source, needs, None, false)
    }

    /// Command compiling the main program with optional CCSID and IBM dialect
    /// selection (used when `manifest.toml` is present).
    pub fn compile_main_with(
        &self,
        program: &str,
        source: &Path,
        needs: &[Need],
        ccsid: Option<u32>,
        ibm_std: bool,
    ) -> Command {
        let exe = format!("{program}{}", std::env::consts::EXE_SUFFIX);
        match self {
            Self::Zpli { compiler, native } => {
                let mut cmd = Command::new(compiler);
                cmd.arg("compile");
                if *native {
                    cmd.arg("--native");
                }
                if let Some(cp) = ccsid {
                    cmd.args(["--ccsid", &cp.to_string()]);
                }
                cmd.args(["-I", "../copy", "-I", ".", "-o"])
                    .arg(if *native {
                        exe
                    } else {
                        format!("{program}.zo")
                    })
                    .arg(source);
                cmd
            }
            Self::Zas { compiler, format } => {
                let mut cmd = Command::new(compiler);
                let format = match format {
                    AsmFormat::Zeus => "zeus",
                    AsmFormat::Obj => "obj",
                    AsmFormat::Goff => "goff",
                };
                cmd.args([
                    "compile",
                    "-I",
                    "../copy",
                    "-I",
                    ".",
                    "--output-format",
                    format,
                    "-o",
                ])
                .arg(format!("{program}.zo"))
                .arg(source);
                cmd
            }
            Toolchain::GnuCobol { cobc } => {
                let mut cmd = gnucobol_base(cobc, ibm_std);
                cmd.arg("-x");
                gnucobol_needs(&mut cmd, program, needs);
                cmd.arg("-o").arg(&exe).arg(source);
                cmd
            }
            Toolchain::Zeus { zcobc } => {
                let mut cmd = Command::new(zcobc);
                cmd.arg("compile");
                if needs.contains(&Need::DebuggingLines) {
                    cmd.arg("--debugging-lines");
                }
                if let Some(cp) = ccsid {
                    cmd.arg("--ccsid").arg(cp.to_string());
                }
                cmd.args(["-I", "../copy", "-I", "..", "-o"])
                    .arg(format!("{program}.zo"))
                    .arg(source);
                cmd
            }
            Toolchain::ZeusNative { zcobc } => {
                let mut cmd = Command::new(zcobc);
                cmd.args(["compile", "--native"]);
                if needs.contains(&Need::DebuggingLines) {
                    cmd.arg("--debugging-lines");
                }
                if let Some(cp) = ccsid {
                    cmd.arg("--ccsid").arg(cp.to_string());
                }
                cmd.args(["-I", "../copy", "-I", "..", "-o"])
                    .arg(&exe)
                    .arg(source);
                cmd
            }
        }
    }

    /// Command compiling the called subprogram `source` into a module that
    /// the runtime loads by program-id from the module directory.
    pub fn compile_subprogram(&self, program: &str, source: &Path) -> Command {
        self.compile_subprogram_with(program, source, false)
    }

    /// Command compiling the called subprogram with optional IBM dialect mode.
    pub fn compile_subprogram_with(&self, program: &str, source: &Path, ibm_std: bool) -> Command {
        if let Self::Zpli { compiler, .. } | Self::Zas { compiler, .. } = self {
            let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("");
            let driver = match ext {
                "CBL" => "zcobc",
                "ASM" => "zas",
                _ => "zpli",
            };
            let mut cmd = Command::new(
                compiler.with_file_name(format!("{driver}{}", std::env::consts::EXE_SUFFIX)),
            );
            cmd.arg("compile");
            if ext == "CBL" {
                cmd.arg("--subprogram");
            }
            cmd.args(["-I", "../copy", "-I", ".", "-o"])
                .arg(format!("{program}.zo"))
                .arg(source);
            return cmd;
        }
        match self {
            Toolchain::GnuCobol { cobc } => {
                let mut cmd = gnucobol_base(cobc, ibm_std);
                cmd.arg("-m").arg(source);
                cmd
            }
            Toolchain::Zeus { zcobc } | Toolchain::ZeusNative { zcobc } => {
                let mut cmd = Command::new(zcobc);
                cmd.args(["compile", "--subprogram", "-I", "../copy", "-I", "..", "-o"])
                    .arg(format!("{program}.zo"))
                    .arg(source);
                cmd
            }
            Self::Zpli { .. } | Self::Zas { .. } => unreachable!(),
        }
    }

    /// Command running the compiled program.
    pub fn run(&self, program: &str) -> Command {
        self.run_with(program, None)
    }

    /// Command running the compiled program with optional JCL `PARM=` text.
    pub fn run_with(&self, program: &str, parm: Option<&str>) -> Command {
        self.run_in(Path::new("."), program, parm)
    }

    /// Command running the compiled program inside `dir` with optional JCL `PARM=` text.
    pub fn run_in(&self, dir: &Path, program: &str, parm: Option<&str>) -> Command {
        let exe = dir.join(format!("{program}{}", std::env::consts::EXE_SUFFIX));
        match self {
            Self::Zpli { compiler, native } => {
                let mut cmd = if *native {
                    Command::new(&exe)
                } else {
                    let mut cmd = Command::new(compiler);
                    cmd.arg("run").arg(format!("{program}.zo"));
                    cmd
                };
                if let Some(p) = parm {
                    cmd.args(["--parm", p]);
                }
                cmd
            }
            Self::Zas { compiler, .. } => {
                // The shared PL/I run-unit driver installs the cross-language
                // linker, including assembler -> PL/I callbacks.
                let mut cmd = Command::new(
                    compiler.with_file_name(format!("zpli{}", std::env::consts::EXE_SUFFIX)),
                );
                cmd.arg("run-hlasm")
                    .arg(format!("{program}.zo"))
                    .args(["-L", "."]);
                if let Some(p) = parm {
                    cmd.args(["--parm", p]);
                }
                cmd
            }
            Toolchain::GnuCobol { .. } => {
                let mut cmd = Command::new(&exe);
                if let Some(p) = parm {
                    cmd.arg(p);
                }
                // Switch settings expected by the suite (X-51 / X-52).
                cmd.env("COB_SWITCH_1", "ON")
                    .env("COB_SWITCH_2", "OFF")
                    .env("COB_DISABLE_WARNINGS", "Y")
                    .env("COB_SET_DEBUG", "N");
                cmd
            }
            Toolchain::Zeus { zcobc } => {
                let mut cmd = Command::new(zcobc);
                cmd.arg("run").arg(format!("{program}.zo"));
                if let Some(p) = parm {
                    cmd.args(["--parm", p]);
                }
                cmd.env("ZEUS_SWITCH_1", "ON").env("ZEUS_SWITCH_2", "OFF");
                cmd
            }
            Toolchain::ZeusNative { .. } => {
                let mut cmd = Command::new(&exe);
                if let Some(p) = parm {
                    cmd.args(["--parm", p]);
                }
                cmd.env("ZEUS_SWITCH_1", "ON").env("ZEUS_SWITCH_2", "OFF");
                cmd
            }
        }
    }

    /// Enables the run-time debugging switch (module DB).
    pub fn debug_env(&self, cmd: &mut Command, on: bool) {
        let v = if on { "Y" } else { "N" };
        match self {
            Toolchain::GnuCobol { .. } => cmd.env("COB_SET_DEBUG", v),
            Toolchain::Zeus { .. } | Toolchain::ZeusNative { .. } => cmd.env("ZEUS_DEBUG", v),
            Self::Zpli { .. } | Self::Zas { .. } => cmd,
        };
    }

    /// Files a compilation leaves behind for `program` (removed on recompile).
    pub fn artifacts(&self, program: &str) -> Vec<String> {
        let exe = format!("{program}{}", std::env::consts::EXE_SUFFIX);
        match self {
            Toolchain::GnuCobol { .. } => vec![
                program.to_string(),
                exe,
                format!("{program}.so"),
                format!("{program}.dll"),
            ],
            Toolchain::Zeus { .. } => vec![format!("{program}.zo")],
            Toolchain::ZeusNative { .. } => {
                vec![program.to_string(), exe, format!("{program}.zo")]
            }
            Self::Zpli { native: true, .. } => vec![exe, format!("{program}.zo")],
            Self::Zpli { .. } | Self::Zas { .. } => vec![format!("{program}.zo")],
        }
    }
}

fn gnucobol_base(cobc: &Path, ibm_std: bool) -> Command {
    let mut cmd = Command::new(cobc);
    let std_flag = if ibm_std { "-std=ibm" } else { "-std=cobol85" };
    cmd.args([std_flag, "--debug", "-I", "../copy", "-I", ".."]);
    cmd
}

fn gnucobol_needs(cmd: &mut Command, program: &str, needs: &[Need]) {
    for need in needs {
        match need {
            Need::Listing => {
                cmd.arg("-t").arg(format!("{program}.lst"));
            }
            Need::FlagObsolete => {
                cmd.arg("-Wobsolete");
            }
            Need::DebuggingLines => {
                cmd.arg("-fdebugging-line");
            }
            Need::AllowGotoSection => {
                cmd.arg("-Wno-goto-section");
            }
            Need::NoDataIncompatibleCheck => {
                cmd.arg("-fno-ec=data-incompatible");
            }
        }
    }
}

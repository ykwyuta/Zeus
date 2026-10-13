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
    GnuCobol { cobc: PathBuf },
    Zeus { zcobc: PathBuf },
    ZeusNative { zcobc: PathBuf },
}

impl Toolchain {
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
        }
    }

    /// Version string of the toolchain, for the results file.
    pub fn version(&self) -> String {
        let out = match self {
            Toolchain::GnuCobol { cobc } => Command::new(cobc).arg("--version").output(),
            Toolchain::Zeus { zcobc } | Toolchain::ZeusNative { zcobc } => {
                Command::new(zcobc).arg("--version").output()
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
        match self {
            Toolchain::GnuCobol { cobc } => {
                let mut cmd = gnucobol_base(cobc);
                cmd.arg("-x");
                gnucobol_needs(&mut cmd, program, needs);
                cmd.arg("-o").arg(program).arg(source);
                cmd
            }
            Toolchain::Zeus { zcobc } => {
                let mut cmd = Command::new(zcobc);
                cmd.arg("compile");
                if needs.contains(&Need::DebuggingLines) {
                    cmd.arg("--debugging-lines");
                }
                cmd.args(["-I", "../copy", "-o"])
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
                cmd.args(["-I", "../copy", "-o"]).arg(program).arg(source);
                cmd
            }
        }
    }

    /// Command compiling the called subprogram `source` into a module that
    /// the runtime loads by program-id from the module directory.
    pub fn compile_subprogram(&self, program: &str, source: &Path) -> Command {
        match self {
            Toolchain::GnuCobol { cobc } => {
                let mut cmd = gnucobol_base(cobc);
                cmd.arg("-m").arg(source);
                cmd
            }
            Toolchain::Zeus { zcobc } | Toolchain::ZeusNative { zcobc } => {
                let mut cmd = Command::new(zcobc);
                cmd.args(["compile", "--subprogram", "-I", "../copy", "-o"])
                    .arg(format!("{program}.zo"))
                    .arg(source);
                cmd
            }
        }
    }

    /// Command running the compiled program.
    pub fn run(&self, program: &str) -> Command {
        match self {
            Toolchain::GnuCobol { .. } => {
                let mut cmd = Command::new(format!("./{program}"));
                // Switch settings expected by the suite (X-51 / X-52).
                cmd.env("COB_SWITCH_1", "ON")
                    .env("COB_SWITCH_2", "OFF")
                    .env("COB_DISABLE_WARNINGS", "Y")
                    .env("COB_SET_DEBUG", "N");
                cmd
            }
            Toolchain::Zeus { zcobc } => {
                let mut cmd = Command::new(zcobc);
                cmd.arg("run")
                    .arg(format!("{program}.zo"))
                    .env("ZEUS_SWITCH_1", "ON")
                    .env("ZEUS_SWITCH_2", "OFF");
                cmd
            }
            Toolchain::ZeusNative { .. } => {
                let mut cmd = Command::new(format!("./{program}"));
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
        };
    }

    /// Files a compilation leaves behind for `program` (removed on recompile).
    pub fn artifacts(&self, program: &str) -> Vec<String> {
        match self {
            Toolchain::GnuCobol { .. } => vec![program.to_string(), format!("{program}.so")],
            Toolchain::Zeus { .. } => vec![format!("{program}.zo")],
            Toolchain::ZeusNative { .. } => vec![program.to_string(), format!("{program}.zo")],
        }
    }
}

fn gnucobol_base(cobc: &Path) -> Command {
    let mut cmd = Command::new(cobc);
    cmd.args(["-std=cobol85", "--debug", "-I", "../copy"]);
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

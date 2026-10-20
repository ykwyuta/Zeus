//! Runs a storage image on Hercules (as an external program) and returns a
//! dump of storage. Hercules is used strictly as a black-box oracle; see
//! CONTRIBUTING.md.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};

pub struct Hercules {
    pub binary: PathBuf,
    /// Seconds to let the program run before the storage is saved.
    pub run_seconds: u32,
}

impl Hercules {
    /// Whether the binary can be found (an explicit path, or on `PATH`).
    pub fn available(&self) -> bool {
        if self.binary.components().count() > 1 {
            return self.binary.is_file();
        }
        std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|d| d.join(&self.binary).is_file())
        })
    }

    /// IPLs `image` in ESA/390 mode and returns storage `0..=dump_end`
    /// together with the Hercules version line from the log.
    pub fn run(&self, dir: &Path, image: &[u8], dump_end: usize) -> Result<(Vec<u8>, String)> {
        self.run_with_arch(dir, image, dump_end, "ESA/390")
    }

    /// IPLs `image` in `archmode` (`ESA/390` or `z/Arch`) and returns storage
    /// `0..=dump_end` together with the Hercules version line from the log.
    pub fn run_with_arch(
        &self,
        dir: &Path,
        image: &[u8],
        dump_end: usize,
        archmode: &str,
    ) -> Result<(Vec<u8>, String)> {
        fs::create_dir_all(dir)?;
        let mainsize_mb = (image.len().max(dump_end + 1)).div_ceil(1 << 20) + 1;
        fs::write(dir.join("core.bin"), image)?;
        fs::write(dir.join("core.ins"), "core.bin 0x00000000\n")?;
        fs::write(
            dir.join("hercules.cnf"),
            format!(
                "ARCHMODE {archmode}\nMAINSIZE {mainsize_mb}\nNUMCPU 1\n\
                 CPUSERIAL 000001\nCPUMODEL 9672\n\
                 # Hercules requires at least one device\n000E 1403 printer.txt\n"
            ),
        )?;
        let _ = fs::remove_file(dir.join("dump.bin"));
        fs::write(
            dir.join("hercules.rc"),
            format!(
                "ipl core.ins\npause {}\nsavecore dump.bin 0 {dump_end:X}\nquit\n",
                self.run_seconds
            ),
        )?;
        let log = fs::File::create(dir.join("hercules.log"))?;
        let mut child = Command::new(&self.binary)
            .args(["-f", "hercules.cnf", "-d"])
            .env("HERCULES_RC", "hercules.rc")
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(log.try_clone()?)
            .stderr(log)
            .spawn()
            .with_context(|| format!("starting {}", self.binary.display()))?;
        let limit = Duration::from_secs(u64::from(self.run_seconds) + 60);
        let start = Instant::now();
        while child.try_wait()?.is_none() {
            if start.elapsed() > limit {
                let _ = child.kill();
                let _ = child.wait();
                bail!(
                    "Hercules did not terminate; see {}",
                    dir.join("hercules.log").display()
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let dump = fs::read(dir.join("dump.bin")).with_context(|| {
            format!(
                "no storage dump written; see {}",
                dir.join("hercules.log").display()
            )
        })?;
        let log = fs::read_to_string(dir.join("hercules.log")).unwrap_or_default();
        let version = log
            .lines()
            .find(|l| l.starts_with("Hercules Version"))
            .unwrap_or("Hercules (unknown version)")
            .trim()
            .to_string();
        Ok((dump, version))
    }
}

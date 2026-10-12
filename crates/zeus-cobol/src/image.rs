//! Native executables: a copy of the Zeus run-time executable (`zeus-rt`,
//! built for the platform that runs the program) with the program's load
//! modules appended. At start-up the run time finds them at the end of
//! its own file and runs the first one.
//!
//! The appended data: for each member its name (u16 length and bytes)
//! and its module (u64 length and bytes), then the number of members
//! (u32), the length of all that (u64) and the magic `ZEUSIMG1`. Integers
//! are little-endian.

use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"ZEUSIMG1";

/// The modules of an executable: (member name, module file contents);
/// the first is the main program.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Image {
    pub members: Vec<(String, Vec<u8>)>,
}

impl Image {
    /// The appended form.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, data) in &self.members {
            out.extend((name.len() as u16).to_le_bytes());
            out.extend(name.as_bytes());
            out.extend((data.len() as u64).to_le_bytes());
            out.extend(data);
        }
        out.extend((self.members.len() as u32).to_le_bytes());
        let len = out.len() as u64;
        out.extend(len.to_le_bytes());
        out.extend(MAGIC);
        out
    }

    /// The image at the end of an executable's contents, if any.
    pub fn from_executable(file: &[u8]) -> Option<Image> {
        let n = file.len();
        if n < 16 || &file[n - 8..] != MAGIC {
            return None;
        }
        let len = u64::from_le_bytes(file[n - 16..n - 8].try_into().ok()?) as usize;
        let start = (n - 16).checked_sub(len)?;
        let data = &file[start..n - 16];
        let count = u32::from_le_bytes(data.get(len.checked_sub(4)?..)?.try_into().ok()?);
        let mut at = 0;
        let mut members = Vec::new();
        for _ in 0..count {
            let nl = u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?) as usize;
            at += 2;
            let name = String::from_utf8(data.get(at..at + nl)?.to_vec()).ok()?;
            at += nl;
            let dl = u64::from_le_bytes(data.get(at..at + 8)?.try_into().ok()?) as usize;
            at += 8;
            members.push((name, data.get(at..at + dl)?.to_vec()));
            at += dl;
        }
        Some(Image { members })
    }

    /// The image of the running executable, if it has one.
    pub fn of_current_exe() -> Option<Image> {
        let exe = std::env::current_exe().ok()?;
        Image::from_executable(&std::fs::read(exe).ok()?)
    }

    /// Writes executable `out`: the run time `runtime` and this image.
    pub fn write_executable(&self, runtime: &Path, out: &Path) -> std::io::Result<()> {
        let mut data = std::fs::read(runtime)?;
        // a run time that is itself an executable with an image
        if let Some(old) = Image::from_executable(&data) {
            data.truncate(data.len() - old.to_bytes().len());
        }
        data.extend(self.to_bytes());
        std::fs::write(out, data)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(out, std::fs::Permissions::from_mode(0o755))?;
        }
        Ok(())
    }
}

/// The file name of the run time for a target triple (None: this
/// platform).
fn runtime_name(target: Option<&str>) -> &'static str {
    let windows = match target {
        Some(t) => t.contains("windows"),
        None => cfg!(windows),
    };
    if windows { "zeus-rt.exe" } else { "zeus-rt" }
}

/// Finds the run time executable for `target` (a Rust target triple such
/// as `aarch64-unknown-linux-gnu`; None: the platform the compiler runs
/// on): ZEUS_RUNTIME (a file), `$ZEUS_RUNTIME_DIR/<target>/zeus-rt`, or
/// next to the compiler (for another target, in the cargo layout
/// `target/<target>/release/zeus-rt`).
pub fn find_runtime(target: Option<&str>) -> Result<PathBuf, String> {
    if let Some(p) = std::env::var_os("ZEUS_RUNTIME") {
        return Ok(PathBuf::from(p));
    }
    let name = runtime_name(target);
    let mut tried = Vec::new();
    if let Some(dir) = std::env::var_os("ZEUS_RUNTIME_DIR") {
        let dir = PathBuf::from(dir);
        tried.push(match target {
            Some(t) => dir.join(t).join(name),
            None => dir.join(name),
        });
    }
    if let Some(bin) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        match target {
            None => tried.push(bin.join(name)),
            Some(t) => {
                // target/release/zcobc -> target/<t>/release/zeus-rt
                let profile = bin.file_name().map(|p| p.to_os_string());
                if let (Some(root), Some(profile)) = (bin.parent(), profile) {
                    tried.push(root.join(t).join(profile).join(name));
                }
                tried.push(bin.join(t).join(name));
            }
        }
    }
    tried.iter().find(|p| p.is_file()).cloned().ok_or_else(|| {
        let hint = match target {
            Some(t) => format!("cargo build --release -p zeus-native --target {t}"),
            None => "cargo build --release -p zeus-native".into(),
        };
        format!(
            "the Zeus run time ({name}) was not found (looked at {}); build it with `{hint}` or set ZEUS_RUNTIME",
            tried
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

/// The member name of a module file: its file name without `.zo`, in
/// capitals.
pub fn member_name(path: &Path) -> String {
    let stem = match path.extension() {
        Some(e) if e.eq_ignore_ascii_case("zo") => path.file_stem(),
        _ => path.file_name(),
    };
    stem.map(|s| s.to_string_lossy().to_ascii_uppercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_round_trip() {
        let img = Image {
            members: vec![
                ("MAIN".into(), b"{\"x\":1}".to_vec()),
                ("SUB".into(), vec![0, 1, 2, 3]),
            ],
        };
        let mut exe = b"\x7fELF...the run time...".to_vec();
        assert_eq!(Image::from_executable(&exe), None);
        exe.extend(img.to_bytes());
        assert_eq!(Image::from_executable(&exe), Some(img));
        assert_eq!(member_name(Path::new("/x/sub1.zo")), "SUB1");
    }
}

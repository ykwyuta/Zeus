//! DD allocation: maps DD names (the names programs use for their files)
//! to host paths.
//!
//! Resolution order for a name: explicit allocations ([`DdTable::set`]),
//! the allocation file, the environment variable `DD_<name>`. The
//! allocation file is named by `ZEUS_DD_CONFIG` and has one `NAME=path`
//! per line (`#` starts a comment); relative paths are relative to the
//! file's directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct DdTable {
    explicit: HashMap<String, PathBuf>,
    config: HashMap<String, PathBuf>,
}

impl DdTable {
    /// A table with the allocation file named by `ZEUS_DD_CONFIG`, if any.
    pub fn from_env() -> std::io::Result<DdTable> {
        let mut t = DdTable::default();
        if let Ok(p) = std::env::var("ZEUS_DD_CONFIG") {
            t.load_config(Path::new(&p))?;
        }
        Ok(t)
    }

    /// Reads an allocation file.
    pub fn load_config(&mut self, path: &Path) -> std::io::Result<()> {
        let text = std::fs::read_to_string(path)?;
        let base = path.parent().unwrap_or(Path::new("."));
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            if let Some((name, p)) = line.split_once('=') {
                let p = Path::new(p.trim());
                let p = if p.is_relative() {
                    base.join(p)
                } else {
                    p.to_path_buf()
                };
                self.config.insert(name.trim().to_ascii_uppercase(), p);
            }
        }
        Ok(())
    }

    /// Allocates `name` to `path`.
    pub fn set(&mut self, name: &str, path: PathBuf) {
        self.explicit.insert(name.to_ascii_uppercase(), path);
    }

    /// Names allocated explicitly or by the allocation file, sorted.
    pub fn names(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .explicit
            .keys()
            .chain(self.config.keys())
            .cloned()
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// The path allocated to the first of `names` that is allocated.
    pub fn resolve(&self, names: &[&str]) -> Option<PathBuf> {
        for n in names {
            let u = n.to_ascii_uppercase();
            if let Some(p) = self.explicit.get(&u).or_else(|| self.config.get(&u)) {
                return Some(p.clone());
            }
        }
        for n in names {
            for key in [format!("DD_{n}"), format!("DD_{}", n.to_ascii_uppercase())] {
                if let Ok(p) = std::env::var(&key) {
                    return Some(PathBuf::from(p));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_then_config() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = dir.path().join("dd.conf");
        std::fs::write(
            &cfg,
            "# allocations\nCUSTFILE = data/cust.dat\nOUT=/tmp/out\n",
        )
        .unwrap();
        let mut t = DdTable::default();
        t.load_config(&cfg).unwrap();
        assert_eq!(
            t.resolve(&["custfile"]),
            Some(dir.path().join("data/cust.dat"))
        );
        t.set("OUT", PathBuf::from("/x"));
        assert_eq!(t.resolve(&["NOPE", "OUT"]), Some(PathBuf::from("/x")));
        assert_eq!(t.resolve(&["ZEUS-NOT-ALLOCATED-NAME"]), None);
    }
}

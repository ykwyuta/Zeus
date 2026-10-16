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

use crate::format::RecordFormat;

/// A DD allocation: the host file and, when known from the catalog or the
/// DD statement, the record format of the data set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Allocation {
    pub path: PathBuf,
    pub format: Option<RecordFormat>,
}

impl Allocation {
    pub fn new(path: PathBuf) -> Allocation {
        Allocation { path, format: None }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DdTable {
    explicit: HashMap<String, Allocation>,
    config: HashMap<String, PathBuf>,
    io: HashMap<String, IoAttributes>,
}

#[derive(Debug, Clone, Default)]
pub struct IoAttributes {
    pub buffers: Option<u32>,
    pub block_size: Option<u32>,
    pub tape: bool,
    /// One-based data set sequence within the tape volume.
    pub file_sequence: Option<usize>,
}

impl DdTable {
    pub fn set_io(&mut self, name: &str, attributes: IoAttributes) {
        self.io.insert(name.to_ascii_uppercase(), attributes);
    }

    pub fn io_attributes(&self, names: &[&str]) -> IoAttributes {
        names
            .iter()
            .find_map(|n| self.io.get(&n.to_ascii_uppercase()))
            .cloned()
            .unwrap_or_default()
    }
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
        self.allocate(name, Allocation::new(path));
    }

    pub fn allocate(&mut self, name: &str, a: Allocation) {
        self.explicit.insert(name.to_ascii_uppercase(), a);
    }

    /// Removes all allocations made with [`DdTable::set`] /
    /// [`DdTable::allocate`].
    pub fn clear(&mut self) {
        self.explicit.clear();
        self.io.clear();
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
        self.allocation(names).map(|a| a.path)
    }

    /// The allocation of the first of `names` that is allocated.
    pub fn allocation(&self, names: &[&str]) -> Option<Allocation> {
        for n in names {
            let u = n.to_ascii_uppercase();
            if let Some(a) = self.explicit.get(&u) {
                return Some(a.clone());
            }
            if let Some(p) = self.config.get(&u) {
                return Some(Allocation::new(p.clone()));
            }
        }
        for n in names {
            for key in [format!("DD_{n}"), format!("DD_{}", n.to_ascii_uppercase())] {
                if let Ok(p) = std::env::var(&key) {
                    return Some(Allocation::new(PathBuf::from(p)));
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

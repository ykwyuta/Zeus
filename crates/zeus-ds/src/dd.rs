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

/// The external name in IBM [label-][S-|AS-]name assignment syntax.
/// Host paths and Zeus names outside that syntax retain their original meaning.
pub fn assignment_ddname(assignment: &str) -> Option<String> {
    if assignment.contains(['/', '\\', ':', '.']) {
        return None;
    }
    let name = assignment.rsplit('-').next()?;
    if !(1..=8).contains(&name.len())
        || !name.as_bytes()[0].is_ascii_alphabetic()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"@$#".contains(&b))
    {
        return None;
    }
    Some(name.to_ascii_uppercase())
}

pub fn same_assignment(left: &str, right: &str) -> bool {
    assignment_ddname(left).unwrap_or_else(|| left.into())
        == assignment_ddname(right).unwrap_or_else(|| right.into())
}

fn allocation_names(names: &[&str]) -> Vec<String> {
    let mut result = Vec::new();
    for name in names {
        let raw = name.to_ascii_uppercase();
        result.push(raw.clone());
        if let Some(ddname) = assignment_ddname(name)
            && ddname != raw
        {
            result.push(ddname);
        }
    }
    result
}

/// A DD allocation: the host file and, when known from the catalog or the
/// DD statement, the record format of the data set.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Allocation {
    pub path: PathBuf,
    pub format: Option<RecordFormat>,
}

impl Allocation {
    pub fn new(path: PathBuf) -> Allocation {
        Allocation { path, format: None }
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct DdTable {
    explicit: HashMap<String, Allocation>,
    config: HashMap<String, PathBuf>,
    io: HashMap<String, IoAttributes>,
    #[serde(default)]
    dynamic: HashMap<String, crate::dynamic::DynamicAllocation>,
    #[serde(default)]
    file_environment: HashMap<String, String>,
    #[serde(default)]
    dynamic_catalog: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct IoAttributes {
    /// External dataset name used for standard tape label identification.
    #[serde(default)]
    pub dataset_name: Option<String>,
    /// RERUN: DISP=MOD retains numbered checkpoint entries in one archive.
    #[serde(default)]
    pub checkpoint_multiple: bool,
    pub buffers: Option<u32>,
    pub block_size: Option<u32>,
    /// Upper bound for system-determined tape blocks (DD BLKSZLIM).
    #[serde(default)]
    pub block_size_limit: Option<u32>,
    #[serde(default)]
    pub expiration: Option<crate::tape::Expiration>,
    #[serde(default)]
    pub retention_days: Option<u32>,
    /// Virtual tape security level: 0, 1 (all access), or 3 (write access).
    #[serde(default)]
    pub tape_security: u8,
    pub tape: bool,
    /// One-based data set sequence within the tape volume.
    pub file_sequence: Option<usize>,
    /// Complete ordered host paths of the virtual volume set, if explicit.
    pub volumes: Vec<PathBuf>,
    /// Standard-label volume serials. Missing values use ZEUS01, ZEUS02, ...
    pub volume_serials: Vec<String>,
    /// Simulated maximum data bytes per volume (labels are separate).
    pub volume_capacity: Option<usize>,
    /// DD LABEL SL/NL; None leaves label selection to the FD.
    pub standard_labels: Option<bool>,
}

impl DdTable {
    pub fn set_dynamic_catalog(&mut self, root: PathBuf) {
        self.dynamic_catalog = Some(root);
    }

    /// Run-unit-local override for an allocation environment variable.
    pub fn set_file_environment(&mut self, name: &str, value: String) {
        self.file_environment
            .insert(name.to_ascii_uppercase(), value);
    }

    /// Evaluate the unprefixed ASSIGN environment variable at each OPEN.
    pub fn refresh_dynamic(&mut self, names: &[&str]) -> crate::Result<()> {
        if self.static_allocation(names).is_some() {
            return Ok(());
        }
        let Some(name) = names.first().and_then(|n| assignment_ddname(n)) else {
            return Ok(());
        };
        let value = self
            .file_environment
            .get(&name)
            .cloned()
            .or_else(|| std::env::var(&name).ok());
        self.dynamic.remove(&name);
        if let Some(value) = value {
            let allocation = crate::dynamic::allocate(&value, self.dynamic_catalog.as_deref())?;
            self.dynamic.insert(name, allocation);
        }
        Ok(())
    }

    pub fn clear_dynamic(&mut self) {
        self.dynamic.clear();
    }

    /// Freeze allocations (including environment/config fallbacks) and resolve
    /// relative media paths for a run image resumed in a different directory.
    pub fn anchor(&mut self, base: &Path, names: &[String]) {
        for name in names {
            if let Some(mut allocation) = self.static_allocation(&[name]) {
                if allocation.path.is_relative() {
                    allocation.path = base.join(&allocation.path);
                }
                self.explicit.insert(name.to_ascii_uppercase(), allocation);
            }
        }
        for allocation in self.explicit.values_mut() {
            if allocation.path.is_relative() {
                allocation.path = base.join(&allocation.path);
            }
        }
        for path in self.config.values_mut() {
            if path.is_relative() {
                *path = base.join(&*path);
            }
        }
        for attributes in self.io.values_mut() {
            for path in &mut attributes.volumes {
                if path.is_relative() {
                    *path = base.join(&*path);
                }
            }
        }
        for allocation in self.dynamic.values_mut() {
            if allocation.allocation.path.is_relative() {
                allocation.allocation.path = base.join(&allocation.allocation.path);
            }
        }
        if let Some(root) = &mut self.dynamic_catalog
            && root.is_relative()
        {
            *root = base.join(&*root);
        }
    }
    pub fn set_io(&mut self, name: &str, attributes: IoAttributes) {
        self.io.insert(name.to_ascii_uppercase(), attributes);
    }

    pub fn io_attributes(&self, names: &[&str]) -> IoAttributes {
        allocation_names(names)
            .iter()
            .find_map(|n| self.io.get(n))
            .cloned()
            .unwrap_or_else(|| IoAttributes {
                dataset_name: allocation_names(names)
                    .iter()
                    .find_map(|n| self.dynamic.get(n).and_then(|a| a.dataset.clone())),
                ..IoAttributes::default()
            })
    }
    /// A table with the allocation file named by `ZEUS_DD_CONFIG`, if any.
    pub fn from_env() -> std::io::Result<DdTable> {
        let mut t = DdTable::default();
        t.dynamic_catalog = Some(
            std::env::var_os("ZEUS_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("zeus"))
                .join("catalog"),
        );
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
        self.dynamic.clear();
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
        self.static_allocation(names).or_else(|| {
            allocation_names(names)
                .iter()
                .find_map(|n| self.dynamic.get(n).map(|a| a.allocation.clone()))
        })
    }

    fn static_allocation(&self, names: &[&str]) -> Option<Allocation> {
        let names = allocation_names(names);
        for n in &names {
            if let Some(a) = self.explicit.get(n) {
                return Some(a.clone());
            }
            if let Some(p) = self.config.get(n) {
                return Some(Allocation::new(p.clone()));
            }
        }
        for n in &names {
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
    fn assignment_labels_resolve_to_ddnames_without_changing_explicit_host_aliases() {
        let mut dd = DdTable::default();
        dd.set("INPUT", PathBuf::from("input.dat"));
        dd.set_io(
            "INPUT",
            IoAttributes {
                buffers: Some(3),
                ..Default::default()
            },
        );
        for name in [
            "INPUT",
            "S-INPUT",
            "DISK-S-INPUT",
            "VSAM-AS-INPUT",
            "disk-input",
        ] {
            assert_eq!(dd.resolve(&[name]), Some(PathBuf::from("input.dat")));
            assert_eq!(dd.io_attributes(&[name]).buffers, Some(3));
        }
        dd.set("DISK-S-INPUT", PathBuf::from("alias.dat"));
        assert_eq!(
            dd.resolve(&["DISK-S-INPUT"]),
            Some(PathBuf::from("alias.dat"))
        );
        for path in [
            "C:\\dir-file",
            "/dir/file-name",
            "file-name.dat",
            "S-TOOLONGDD",
        ] {
            assert_eq!(assignment_ddname(path), None);
        }
        assert_eq!(assignment_ddname("LABEL-A$#1"), Some("A$#1".into()));
    }

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

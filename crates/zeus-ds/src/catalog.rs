//! The catalog: data set names and their attributes.
//!
//! A catalog lives in a host directory: `catalog.json` holds the entries
//! and `datasets/` the data (one file per data set, one directory per
//! partitioned data set with a file per member).
//!
//! Entries follow the z/OS catalog: non-VSAM data sets (sequential or
//! partitioned), VSAM clusters (key-sequenced, entry-sequenced, relative
//! record), alternate indexes and paths, and generation data groups whose
//! generations are non-VSAM entries named `base.GnnnnV00`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::format::RecordFormat;
use crate::{DsError, Result};

/// Record format of a non-VSAM data set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Recfm {
    F,
    Fb,
    Fba,
    V,
    Vb,
    Vba,
    U,
}

impl Recfm {
    pub fn parse(s: &str) -> Option<Recfm> {
        Some(match s.to_ascii_uppercase().as_str() {
            "F" | "FS" => Recfm::F,
            "FB" | "FBS" => Recfm::Fb,
            "FBA" | "FA" => Recfm::Fba,
            "V" | "VS" => Recfm::V,
            "VB" | "VBS" => Recfm::Vb,
            "VBA" | "VA" => Recfm::Vba,
            "U" => Recfm::U,
            _ => return None,
        })
    }

    pub fn format(self, lrecl: u32) -> RecordFormat {
        match self {
            Recfm::F | Recfm::Fb | Recfm::Fba => RecordFormat::Fixed(lrecl),
            _ => RecordFormat::Variable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VsamOrg {
    Ksds,
    Esds,
    Rrds,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntryKind {
    /// Sequential (PS) data set.
    Sequential {
        recfm: Recfm,
        lrecl: u32,
    },
    /// Partitioned (PO) data set: a directory of members.
    Partitioned {
        recfm: Recfm,
        lrecl: u32,
    },
    Cluster {
        org: VsamOrg,
        /// KEYS(length offset) of a KSDS.
        keys: Option<(u32, u32)>,
        /// RECORDSIZE(average maximum).
        record_size: (u32, u32),
        /// Alternate indexes related to the cluster.
        aixs: Vec<String>,
        /// LOG(NONE | UNDO | ALL): whether CICS backs out changes.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        log: Option<String>,
    },
    AlternateIndex {
        relate: String,
        keys: (u32, u32),
        unique: bool,
        upgrade: bool,
    },
    Path {
        entry: String,
    },
    Gdg {
        limit: u32,
        scratch: bool,
        /// Absolute generation numbers, oldest first.
        generations: Vec<u32>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub name: String,
    pub kind: EntryKind,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Stored {
    entries: BTreeMap<String, Entry>,
}

#[derive(Debug)]
pub struct Catalog {
    root: PathBuf,
    entries: BTreeMap<String, Entry>,
}

fn io(e: std::io::Error, p: &Path) -> DsError {
    DsError::Io(format!("{}: {e}", p.display()))
}

/// Name of generation `g` of GDG `base`.
pub fn generation_name(base: &str, g: u32) -> String {
    format!("{base}.G{g:04}V00")
}

impl Catalog {
    /// Opens (or creates) the catalog in `root`.
    pub fn open(root: &Path) -> Result<Catalog> {
        std::fs::create_dir_all(root.join("datasets")).map_err(|e| io(e, root))?;
        let file = root.join("catalog.json");
        let entries = match std::fs::read(&file) {
            Ok(d) => {
                serde_json::from_slice::<Stored>(&d)
                    .map_err(|e| DsError::Damaged(format!("{}: {e}", file.display())))?
                    .entries
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(e) => return Err(io(e, &file)),
        };
        Ok(Catalog {
            root: root.to_path_buf(),
            entries,
        })
    }

    pub fn save(&self) -> Result<()> {
        let file = self.root.join("catalog.json");
        let data = serde_json::to_vec_pretty(&Stored {
            entries: self.entries.clone(),
        })
        .map_err(|e| DsError::Io(e.to_string()))?;
        std::fs::write(&file, data).map_err(|e| io(e, &file))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn get(&self, name: &str) -> Option<&Entry> {
        self.entries.get(&name.to_ascii_uppercase())
    }

    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.values()
    }

    /// Host file (or directory) of a data set.
    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join("datasets").join(name.to_ascii_uppercase())
    }

    /// The data set holding the data of `name`: a path or alternate index
    /// resolves to its base cluster.
    pub fn base_of(&self, name: &str) -> Option<&Entry> {
        match &self.get(name)?.kind {
            EntryKind::Path { entry } => self.base_of(entry),
            EntryKind::AlternateIndex { relate, .. } => self.base_of(relate),
            _ => self.get(name),
        }
    }

    /// Adds an entry; fails when the name is catalogued.
    pub fn define(&mut self, entry: Entry) -> Result<()> {
        let name = entry.name.to_ascii_uppercase();
        if self.entries.contains_key(&name) {
            return Err(DsError::DuplicateKey);
        }
        if let EntryKind::AlternateIndex { relate, .. } = &entry.kind {
            match self.entries.get_mut(&relate.to_ascii_uppercase()) {
                Some(Entry {
                    kind: EntryKind::Cluster { aixs, .. },
                    ..
                }) => aixs.push(name.clone()),
                _ => return Err(DsError::NotFound),
            }
        }
        if matches!(entry.kind, EntryKind::Partitioned { .. }) {
            std::fs::create_dir_all(self.path(&name)).map_err(|e| io(e, &self.path(&name)))?;
        }
        self.entries.insert(name.clone(), Entry { name, ..entry });
        Ok(())
    }

    /// Replaces the attributes of an entry, keeping its data.
    pub fn delete_entry_only(&mut self, name: &str) -> Result<Entry> {
        self.entries
            .remove(&name.to_ascii_uppercase())
            .ok_or(DsError::NotFound)
    }

    /// Removes an entry and its data (a cluster with its alternate
    /// indexes and paths).
    pub fn delete(&mut self, name: &str) -> Result<Entry> {
        let name = name.to_ascii_uppercase();
        let entry = self.entries.remove(&name).ok_or(DsError::NotFound)?;
        match &entry.kind {
            EntryKind::Cluster { aixs, .. } => {
                for a in aixs.clone() {
                    let _ = self.delete(&a);
                }
            }
            EntryKind::AlternateIndex { relate, .. } => {
                if let Some(Entry {
                    kind: EntryKind::Cluster { aixs, .. },
                    ..
                }) = self.entries.get_mut(relate)
                {
                    aixs.retain(|a| *a != name);
                }
                let paths: Vec<String> = self
                    .entries
                    .values()
                    .filter(|e| matches!(&e.kind, EntryKind::Path { entry } if *entry == name))
                    .map(|e| e.name.clone())
                    .collect();
                for p in paths {
                    self.entries.remove(&p);
                }
            }
            EntryKind::Gdg { generations, .. } => {
                for g in generations.clone() {
                    let _ = self.delete(&generation_name(&name, g));
                }
            }
            _ => {}
        }
        let p = self.path(&name);
        if p.is_dir() {
            let _ = std::fs::remove_dir_all(&p);
        } else if p.exists() {
            let _ = std::fs::remove_file(&p);
        }
        Ok(entry)
    }

    /// Resolves a relative generation `base(n)`: Some(name) of generation
    /// current+n (n <= 0) of an existing generation, or of a new one
    /// (n > 0). `pending` holds generations created earlier in the same
    /// job, keyed by relative number.
    pub fn generation(&self, base: &str, rel: i32) -> Option<(String, u32)> {
        let EntryKind::Gdg { generations, .. } = &self.get(base)?.kind else {
            return None;
        };
        let current = generations.last().copied().unwrap_or(0) as i64;
        let g = current + rel as i64;
        if g < 1 {
            return None;
        }
        if rel <= 0 && !generations.contains(&(g as u32)) {
            return None;
        }
        Some((
            generation_name(&base.to_ascii_uppercase(), g as u32),
            g as u32,
        ))
    }

    /// Catalogues generation `g` of GDG `base` with attributes `kind`,
    /// rolling off the oldest generation beyond LIMIT.
    pub fn add_generation(&mut self, base: &str, g: u32, kind: EntryKind) -> Result<String> {
        let base = base.to_ascii_uppercase();
        let name = generation_name(&base, g);
        let roll = match self.entries.get_mut(&base) {
            Some(Entry {
                kind: EntryKind::Gdg {
                    limit, generations, ..
                },
                ..
            }) => {
                generations.push(g);
                generations.sort_unstable();
                if generations.len() > *limit as usize {
                    Some(generations.remove(0))
                } else {
                    None
                }
            }
            _ => return Err(DsError::NotFound),
        };
        self.entries.insert(
            name.clone(),
            Entry {
                name: name.clone(),
                kind,
            },
        );
        if let Some(old) = roll {
            let _ = self.delete(&generation_name(&base, old));
        }
        Ok(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn define_delete_and_generations() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = Catalog::open(dir.path()).unwrap();
        c.define(Entry {
            name: "a.ksds".into(),
            kind: EntryKind::Cluster {
                org: VsamOrg::Ksds,
                keys: Some((11, 0)),
                record_size: (300, 300),
                aixs: vec![],
                log: None,
            },
        })
        .unwrap();
        c.define(Entry {
            name: "A.AIX".into(),
            kind: EntryKind::AlternateIndex {
                relate: "A.KSDS".into(),
                keys: (11, 16),
                unique: false,
                upgrade: true,
            },
        })
        .unwrap();
        c.define(Entry {
            name: "A.PATH".into(),
            kind: EntryKind::Path {
                entry: "A.AIX".into(),
            },
        })
        .unwrap();
        assert_eq!(c.base_of("a.path").unwrap().name, "A.KSDS");
        c.save().unwrap();
        let mut c = Catalog::open(dir.path()).unwrap();
        c.delete("A.KSDS").unwrap();
        assert!(c.get("A.AIX").is_none() && c.get("A.PATH").is_none());

        c.define(Entry {
            name: "G".into(),
            kind: EntryKind::Gdg {
                limit: 2,
                scratch: true,
                generations: vec![],
            },
        })
        .unwrap();
        assert!(c.generation("G", 0).is_none());
        let ps = EntryKind::Sequential {
            recfm: Recfm::Fb,
            lrecl: 80,
        };
        for _ in 0..3 {
            let (_, g) = c.generation("G", 1).unwrap();
            c.add_generation("G", g, ps.clone()).unwrap();
        }
        assert_eq!(c.generation("G", 0).unwrap().0, "G.G0003V00");
        assert_eq!(c.generation("G", -1).unwrap().0, "G.G0002V00");
        assert!(c.get("G.G0001V00").is_none(), "rolled off");
    }
}

/// How records are written to a data set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    /// Replace the content.
    Replace,
    /// Add to the existing records (MOD, REPRO into a non-empty data set).
    Append,
}

impl Catalog {
    /// Pad byte of fixed-length records (EBCDIC blank).
    const PAD: u8 = 0x40;

    fn keyed(&self, keys: (u32, u32)) -> crate::keyed::KeyedDataset {
        crate::keyed::KeyedDataset::new(
            vec![crate::keyed::KeySpec {
                offset: keys.1 as usize,
                len: keys.0 as usize,
                duplicates: false,
            }],
            Self::PAD,
        )
    }

    /// Record format of a non-VSAM data set or the variable format of VSAM
    /// data.
    pub fn record_format(&self, name: &str) -> Option<RecordFormat> {
        match &self.base_of(name)?.kind {
            EntryKind::Sequential { recfm, lrecl } | EntryKind::Partitioned { recfm, lrecl } => {
                Some(recfm.format(*lrecl))
            }
            EntryKind::Cluster { .. } => Some(RecordFormat::Variable),
            _ => None,
        }
    }

    /// Keyed access to a KSDS or to a path over one: the host file of the
    /// base cluster, the key specifications of the cluster (primary key,
    /// then the key of each alternate index), which of them `name` uses and
    /// the maximum record length.
    /// LOG of the cluster a data set (cluster, path) belongs to: Some(true)
    /// for UNDO / ALL, Some(false) for NONE, None when not given.
    pub fn recoverable(&self, name: &str) -> Option<bool> {
        match &self.base_of(name)?.kind {
            EntryKind::Cluster { log: Some(l), .. } => Some(l != "NONE"),
            _ => None,
        }
    }

    pub fn keyed_access(
        &self,
        name: &str,
    ) -> Result<(PathBuf, Vec<crate::keyed::KeySpec>, usize, usize)> {
        let entry = self.get(name).ok_or(DsError::NotFound)?;
        let base = self.base_of(name).ok_or(DsError::NotFound)?;
        let EntryKind::Cluster {
            org: VsamOrg::Ksds,
            keys: Some(keys),
            record_size,
            aixs,
            ..
        } = &base.kind
        else {
            return Err(DsError::Damaged(format!("{name} is not a KSDS")));
        };
        let mut specs = vec![crate::keyed::KeySpec {
            offset: keys.1 as usize,
            len: keys.0 as usize,
            duplicates: false,
        }];
        let via = match &entry.kind {
            EntryKind::Path { entry } => Some(entry.to_ascii_uppercase()),
            _ => None,
        };
        let mut which = 0;
        for aix in aixs {
            if let Some(EntryKind::AlternateIndex { keys, unique, .. }) =
                self.get(aix).map(|e| &e.kind)
            {
                specs.push(crate::keyed::KeySpec {
                    offset: keys.1 as usize,
                    len: keys.0 as usize,
                    duplicates: !unique,
                });
                if via.as_deref() == Some(aix.to_ascii_uppercase().as_str()) {
                    which = specs.len() - 1;
                }
            }
        }
        Ok((self.path(&base.name), specs, which, record_size.1 as usize))
    }

    /// Reads all records of a catalogued data set (a KSDS in key order,
    /// through a path in the order of the alternate key).
    pub fn read_records(&self, name: &str) -> Result<Vec<Vec<u8>>> {
        let entry = self.get(name).ok_or(DsError::NotFound)?;
        let base = self.base_of(name).ok_or(DsError::NotFound)?;
        let path = self.path(&base.name);
        match &base.kind {
            EntryKind::Sequential { recfm, lrecl } => {
                Ok(
                    crate::seq::SequentialDataset::load(&path, recfm.format(*lrecl), Self::PAD)?
                        .map(|d| d.records)
                        .unwrap_or_default(),
                )
            }
            EntryKind::Cluster { org, keys, .. } => match org {
                VsamOrg::Ksds => {
                    let keys = keys.ok_or_else(|| DsError::Damaged("KSDS without KEYS".into()))?;
                    let mut specs = vec![crate::keyed::KeySpec {
                        offset: keys.1 as usize,
                        len: keys.0 as usize,
                        duplicates: false,
                    }];
                    // reading through a path: the alternate key order
                    let alt = match &entry.kind {
                        EntryKind::Path { entry } => match self.get(entry).map(|e| &e.kind) {
                            Some(EntryKind::AlternateIndex { keys, unique, .. }) => {
                                specs.push(crate::keyed::KeySpec {
                                    offset: keys.1 as usize,
                                    len: keys.0 as usize,
                                    duplicates: !unique,
                                });
                                true
                            }
                            _ => false,
                        },
                        _ => false,
                    };
                    let Some(d) = crate::keyed::KeyedDataset::load(&path, specs, Self::PAD)? else {
                        return Ok(Vec::new());
                    };
                    let which = usize::from(alt);
                    let mut out = Vec::new();
                    let mut f = d.next(which, None, false);
                    while let Some(x) = f {
                        out.push(x.record.clone());
                        f = d.next(which, Some(&x.position), false);
                    }
                    Ok(out)
                }
                VsamOrg::Esds => Ok(crate::seq::SequentialDataset::load(
                    &path,
                    RecordFormat::Variable,
                    Self::PAD,
                )?
                .map(|d| d.records)
                .unwrap_or_default()),
                VsamOrg::Rrds => {
                    let Some(d) = crate::relative::RelativeDataset::load(&path)? else {
                        return Ok(Vec::new());
                    };
                    let mut out = Vec::new();
                    let mut n = 1;
                    while let Some((k, r)) = d.next_from(n) {
                        out.push(r.to_vec());
                        n = k + 1;
                    }
                    Ok(out)
                }
            },
            _ => Err(DsError::Damaged(format!("{name} holds no records"))),
        }
    }

    /// Writes records to a catalogued data set. Returns the number of
    /// records rejected (duplicate keys of a KSDS).
    pub fn write_records(&self, name: &str, recs: &[Vec<u8>], mode: WriteMode) -> Result<usize> {
        let base = self.base_of(name).ok_or(DsError::NotFound)?;
        let path = self.path(&base.name);
        match &base.kind {
            EntryKind::Sequential { recfm, lrecl } => {
                let fmt = recfm.format(*lrecl);
                let mut d = match mode {
                    WriteMode::Append => {
                        crate::seq::SequentialDataset::load(&path, fmt, Self::PAD)?
                    }
                    WriteMode::Replace => None,
                }
                .unwrap_or_else(|| crate::seq::SequentialDataset::new(fmt, Self::PAD));
                d.records.extend(recs.iter().cloned());
                d.store(&path)?;
                Ok(0)
            }
            EntryKind::Cluster { org, keys, .. } => match org {
                VsamOrg::Ksds => {
                    let keys = keys.ok_or_else(|| DsError::Damaged("KSDS without KEYS".into()))?;
                    let mut d = match mode {
                        WriteMode::Append => crate::keyed::KeyedDataset::load(
                            &path,
                            self.keyed(keys).keys().to_vec(),
                            Self::PAD,
                        )?,
                        WriteMode::Replace => None,
                    }
                    .unwrap_or_else(|| self.keyed(keys));
                    let mut rejected = 0;
                    for r in recs {
                        if d.insert(r.clone()).is_err() {
                            rejected += 1;
                        }
                    }
                    d.store(&path)?;
                    Ok(rejected)
                }
                VsamOrg::Esds => {
                    let mut d = match mode {
                        WriteMode::Append => crate::seq::SequentialDataset::load(
                            &path,
                            RecordFormat::Variable,
                            Self::PAD,
                        )?,
                        WriteMode::Replace => None,
                    }
                    .unwrap_or_else(|| {
                        crate::seq::SequentialDataset::new(RecordFormat::Variable, Self::PAD)
                    });
                    d.records.extend(recs.iter().cloned());
                    d.store(&path)?;
                    Ok(0)
                }
                VsamOrg::Rrds => {
                    let mut d = match mode {
                        WriteMode::Append => crate::relative::RelativeDataset::load(&path)?,
                        WriteMode::Replace => None,
                    }
                    .unwrap_or_default();
                    for r in recs {
                        d.append(r.clone());
                    }
                    d.store(&path)?;
                    Ok(0)
                }
            },
            _ => Err(DsError::Damaged(format!("{name} cannot hold records"))),
        }
    }
}

#[cfg(test)]
mod record_tests {
    use super::*;

    #[test]
    fn repro_style_copy() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = Catalog::open(dir.path()).unwrap();
        c.define(Entry {
            name: "PS".into(),
            kind: EntryKind::Sequential {
                recfm: Recfm::Fb,
                lrecl: 4,
            },
        })
        .unwrap();
        c.define(Entry {
            name: "KS".into(),
            kind: EntryKind::Cluster {
                org: VsamOrg::Ksds,
                keys: Some((2, 0)),
                record_size: (4, 4),
                aixs: vec![],
                log: None,
            },
        })
        .unwrap();
        c.write_records(
            "PS",
            &[b"02AB".to_vec(), b"01CD".to_vec(), b"01XX".to_vec()],
            WriteMode::Replace,
        )
        .unwrap();
        let recs = c.read_records("PS").unwrap();
        assert_eq!(c.write_records("KS", &recs, WriteMode::Replace).unwrap(), 1);
        assert_eq!(
            c.read_records("KS").unwrap(),
            vec![b"01CD".to_vec(), b"02AB".to_vec()]
        );
    }
}

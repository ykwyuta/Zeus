//! Environment allocation syntax, implemented from the COBOL ASSIGN contract.
use std::path::{Path, PathBuf};

use crate::catalog::{Catalog, Entry, EntryKind, Recfm};
use crate::dd::Allocation;
use crate::format::RecordFormat;
use crate::{DsError, Result};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct DynamicAllocation {
    pub allocation: Allocation,
    pub dataset: Option<String>,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub append: bool,
    #[serde(default)]
    release: Option<Release>,
}

#[derive(Debug, Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
enum Disposition {
    #[default]
    Keep,
    Delete,
    Catalog,
    Uncatalog,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Release {
    catalog_root: PathBuf,
    entry: Entry,
    member: bool,
    disposition: Disposition,
}

impl DynamicAllocation {
    pub fn anchor(&mut self, base: &Path) {
        if self.allocation.path.is_relative() {
            self.allocation.path = base.join(&self.allocation.path);
        }
        if let Some(release) = &mut self.release
            && release.catalog_root.is_relative()
        {
            release.catalog_root = base.join(&release.catalog_root);
        }
    }

    pub fn release(&self) -> Result<()> {
        let Some(release) = &self.release else {
            return Ok(());
        };
        let mut catalog = Catalog::open(&release.catalog_root)?;
        match release.disposition {
            Disposition::Keep => {}
            Disposition::Catalog => {
                if let Some(existing) = catalog.get(&release.entry.name) {
                    if existing.kind != release.entry.kind {
                        return Err(invalid("catalog entry changed during allocation"));
                    }
                } else {
                    catalog.define(release.entry.clone())?;
                }
                catalog.save()?;
            }
            Disposition::Uncatalog => {
                if release.member {
                    return Err(invalid("cannot uncatalog an individual PDS member"));
                }
                if catalog.get(&release.entry.name).is_some() {
                    catalog.delete_entry_only(&release.entry.name)?;
                    catalog.save()?;
                }
            }
            Disposition::Delete => {
                if release.member || catalog.get(&release.entry.name).is_none() {
                    match std::fs::remove_file(&self.allocation.path) {
                        Ok(()) => {}
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => return Err(invalid(&error.to_string())),
                    }
                } else {
                    catalog.delete(&release.entry.name)?;
                    catalog.save()?;
                }
            }
        }
        Ok(())
    }
}

fn invalid(message: &str) -> DsError {
    DsError::Io(format!("dynamic allocation: {message}"))
}

pub(crate) fn allocate(
    value: &str,
    catalog_root: Option<&Path>,
    model: Option<(RecordFormat, u32)>,
) -> Result<DynamicAllocation> {
    let value = value.trim();
    if let Some(path) = value
        .strip_prefix("PATH(")
        .and_then(|s| s.strip_suffix(')'))
    {
        if path.is_empty()
            || path.chars().any(char::is_whitespace)
            || path.contains(['(', ')'])
            || !Path::new(path).is_absolute()
        {
            return Err(invalid(
                "PATH requires an absolute host path without embedded blanks",
            ));
        }
        return Ok(DynamicAllocation {
            allocation: Allocation::new(PathBuf::from(path)),
            dataset: None,
            source: value.into(),
            append: false,
            release: None,
        });
    }
    let source = value
        .strip_prefix("DSN(")
        .ok_or_else(|| invalid("expected PATH(...) or DSN(...)"))?;
    let mut depth = 1;
    let close = source
        .char_indices()
        .find_map(|(i, c)| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            (depth == 0).then_some(i)
        })
        .ok_or_else(|| invalid("unterminated DSN"))?;
    let inside = &source[..close];
    let (name, inside_member) = if let Some((name, member)) = inside.split_once('(') {
        (
            name,
            Some(
                member
                    .strip_suffix(')')
                    .ok_or_else(|| invalid("invalid member"))?,
            ),
        )
    } else {
        (inside, None)
    };
    let mut tail = &source[close + 1..];
    let member = if tail.starts_with('(') {
        let end = tail
            .find(')')
            .ok_or_else(|| invalid("unterminated member"))?;
        let member = &tail[1..end];
        tail = &tail[end + 1..];
        if inside_member.is_some() {
            return Err(invalid("member specified twice"));
        }
        Some(member)
    } else {
        inside_member
    };
    let component = |s: &str| {
        !s.is_empty()
            && s.len() <= 8
            && (s.as_bytes()[0].is_ascii_uppercase() || b"@$#".contains(&s.as_bytes()[0]))
            && s.bytes()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b"@$#".contains(&b))
    };
    if name.len() > 44 || !name.split('.').all(component) || member.is_some_and(|m| !component(m)) {
        return Err(invalid("invalid fully qualified dataset/member name"));
    }
    if !tail.is_empty() && !tail.starts_with([',', ' ']) {
        return Err(invalid("DSN options require a comma or blanks"));
    }
    let options: Vec<_> = tail
        .split(|c: char| c == ',' || c.is_ascii_whitespace())
        .filter(|s| !s.is_empty())
        .collect();
    let mut status = None;
    let mut disposition = None;
    for option in options {
        if matches!(option, "NEW" | "OLD" | "SHR" | "MOD") {
            if status.replace(option).is_some() {
                return Err(invalid("conflicting dataset dispositions"));
            }
        } else {
            let end = match option {
                "KEEP" => Disposition::Keep,
                "DELETE" => Disposition::Delete,
                "CATALOG" => Disposition::Catalog,
                "UNCATALOG" => Disposition::Uncatalog,
                _ => {
                    return Err(invalid(&format!(
                        "unsupported dataset allocation option {option}"
                    )));
                }
            };
            if disposition.replace(end).is_some() {
                return Err(invalid("conflicting release dispositions"));
            }
        }
    }
    let status = status.unwrap_or("OLD");
    let disposition = disposition.unwrap_or_default();
    let root = catalog_root.ok_or_else(|| invalid("no dynamic dataset catalog configured"))?;
    let catalog = Catalog::open(root)?;
    let existing = catalog.base_of(name).cloned();
    if status == "NEW" && existing.is_some() && member.is_none() {
        return Err(invalid("NEW dataset already exists"));
    }
    let new = existing.is_none() && matches!(status, "NEW" | "MOD");
    let entry = if let Some(entry) = existing {
        entry
    } else if new && member.is_none() {
        let (format, lrecl) =
            model.ok_or_else(|| invalid("a new dataset requires a sequential FD"))?;
        let recfm = match format {
            RecordFormat::Fixed(_) => Recfm::F,
            RecordFormat::Variable => Recfm::V,
            RecordFormat::Text => {
                return Err(invalid("DSN does not allocate a line-sequential file"));
            }
        };
        Entry {
            name: name.into(),
            kind: EntryKind::Sequential { recfm, lrecl },
        }
    } else {
        return Err(invalid("dataset is not cataloged"));
    };
    let mut path = catalog.path(&entry.name);
    if let Some(member) = member {
        if !matches!(entry.kind, EntryKind::Partitioned { .. }) {
            return Err(invalid("member requires a partitioned dataset"));
        }
        path.push(member);
    } else if matches!(
        entry.kind,
        EntryKind::Partitioned { .. } | EntryKind::Gdg { .. }
    ) {
        return Err(invalid("a partitioned dataset requires a member"));
    }
    if matches!(
        entry.kind,
        EntryKind::Cluster { .. } | EntryKind::Path { .. } | EntryKind::AlternateIndex { .. }
    ) && (!matches!(status, "OLD" | "SHR") || !matches!(disposition, Disposition::Keep))
    {
        return Err(invalid(
            "VSAM environment allocation permits only OLD or SHR",
        ));
    }
    if new || member.is_some() && matches!(status, "NEW" | "MOD") && !path.exists() {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| invalid(&error.to_string()))?;
    } else if status == "NEW" {
        return Err(invalid("NEW member already exists"));
    } else if !path.is_file() {
        return Err(invalid("dataset/member does not exist"));
    }
    Ok(DynamicAllocation {
        allocation: Allocation {
            path,
            format: catalog.record_format(name).or(model.map(|m| m.0)),
        },
        dataset: Some(name.into()),
        source: value.into(),
        append: status == "MOD",
        release: Some(Release {
            catalog_root: root.to_path_buf(),
            entry,
            member: member.is_some(),
            disposition,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Entry, Recfm};

    #[test]
    fn new_and_release_dispositions_update_the_catalog_and_exact_dataset() {
        for end in ["KEEP", "CATALOG", "UNCATALOG", "DELETE"] {
            let dir = tempfile::tempdir().unwrap();
            let allocation = allocate(
                &format!("DSN(APP.NEW),NEW,{end}"),
                Some(dir.path()),
                Some((RecordFormat::Fixed(10), 10)),
            )
            .unwrap();
            std::fs::write(&allocation.allocation.path, b"ABCDEFGHIJ").unwrap();
            assert!(Catalog::open(dir.path()).unwrap().get("APP.NEW").is_none());
            allocation.release().unwrap();
            let catalog = Catalog::open(dir.path()).unwrap();
            assert_eq!(catalog.get("APP.NEW").is_some(), end == "CATALOG");
            assert_eq!(allocation.allocation.path.exists(), end != "DELETE");
            if end == "CATALOG" {
                assert_eq!(
                    catalog.record_format("APP.NEW"),
                    Some(RecordFormat::Fixed(10))
                );
                assert!(
                    allocate(
                        "DSN(APP.NEW),NEW",
                        Some(dir.path()),
                        Some((RecordFormat::Fixed(10), 10))
                    )
                    .is_err()
                );
                assert_eq!(
                    std::fs::read(&allocation.allocation.path).unwrap(),
                    b"ABCDEFGHIJ"
                );
                let append = allocate("DSN(APP.NEW),MOD", Some(dir.path()), None).unwrap();
                assert!(append.append);
                let remove = allocate("DSN(APP.NEW),OLD,DELETE", Some(dir.path()), None).unwrap();
                remove.release().unwrap();
                assert!(!allocation.allocation.path.exists());
                assert!(Catalog::open(dir.path()).unwrap().get("APP.NEW").is_none());
            }
        }
    }

    #[test]
    fn member_syntax_and_release_preserve_the_enclosing_pds() {
        let dir = tempfile::tempdir().unwrap();
        let mut catalog = Catalog::open(dir.path()).unwrap();
        catalog
            .define(Entry {
                name: "APP.LIB".into(),
                kind: EntryKind::Partitioned {
                    recfm: Recfm::Fb,
                    lrecl: 80,
                },
            })
            .unwrap();
        catalog.save().unwrap();
        let member = allocate("DSN(APP.LIB(MEMBER)),NEW,KEEP", Some(dir.path()), None).unwrap();
        member.release().unwrap();
        let same = allocate("DSN(APP.LIB)(MEMBER),OLD,DELETE", Some(dir.path()), None).unwrap();
        assert_eq!(same.allocation.path, member.allocation.path);
        same.release().unwrap();
        assert!(!same.allocation.path.exists());
        assert!(catalog.path("APP.LIB").is_dir());
        assert!(Catalog::open(dir.path()).unwrap().get("APP.LIB").is_some());
        assert!(
            allocate(
                "DSN(APP.NEW(MEMBER)),NEW",
                Some(dir.path()),
                Some((RecordFormat::Fixed(10), 10))
            )
            .is_err()
        );
    }

    #[test]
    fn paths_and_cataloged_datasets_resolve_without_treating_options_as_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("CaseSensitive");
        let value = format!(" PATH({}) ", path.display());
        assert_eq!(allocate(&value, None, None).unwrap().allocation.path, path);
        let mut catalog = Catalog::open(dir.path()).unwrap();
        catalog
            .define(Entry {
                name: "APP.INPUT".into(),
                kind: EntryKind::Sequential {
                    recfm: Recfm::Fb,
                    lrecl: 10,
                },
            })
            .unwrap();
        std::fs::write(catalog.path("APP.INPUT"), b"ABCDEFGHIJ").unwrap();
        catalog.save().unwrap();
        let allocation = allocate("DSN(APP.INPUT),SHR", Some(dir.path()), None).unwrap();
        assert_eq!(allocation.allocation.path, catalog.path("APP.INPUT"));
        assert_eq!(
            allocation.allocation.format,
            Some(crate::format::RecordFormat::Fixed(10))
        );
        for bad in [
            "",
            "PATH(relative)",
            "PATH(/some path)",
            "path(/tmp/file)",
            "DSN(&&TMP),OLD",
            "DSN(APP.INPUT)SHR",
            "DSN(APP.INPUT),BOGUS",
            "DSN(APP.MISSING),OLD",
        ] {
            assert!(allocate(bad, Some(dir.path()), None).is_err(), "{bad}");
        }
    }
}

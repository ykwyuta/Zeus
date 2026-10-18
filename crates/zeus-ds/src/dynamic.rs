//! Environment allocation syntax, implemented from the COBOL ASSIGN contract.
use std::path::{Path, PathBuf};

use crate::catalog::{Catalog, EntryKind};
use crate::dd::Allocation;
use crate::{DsError, Result};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct DynamicAllocation {
    pub allocation: Allocation,
    pub dataset: Option<String>,
}

fn invalid(message: &str) -> DsError {
    DsError::Io(format!("dynamic allocation: {message}"))
}

pub(crate) fn allocate(value: &str, catalog_root: Option<&Path>) -> Result<DynamicAllocation> {
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
        });
    }
    let source = value
        .strip_prefix("DSN(")
        .ok_or_else(|| invalid("expected PATH(...) or DSN(...)"))?;
    let close = source
        .find(')')
        .ok_or_else(|| invalid("unterminated DSN"))?;
    let name = &source[..close];
    let mut tail = &source[close + 1..];
    let member = if tail.starts_with('(') {
        let end = tail
            .find(')')
            .ok_or_else(|| invalid("unterminated member"))?;
        let member = &tail[1..end];
        tail = &tail[end + 1..];
        Some(member)
    } else {
        None
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
    if options.len() > 1
        || options
            .first()
            .is_some_and(|s| !matches!(*s, "OLD" | "SHR"))
    {
        return Err(invalid(
            "this allocation requires a cataloged dataset and OLD or SHR",
        ));
    }
    let root = catalog_root.ok_or_else(|| invalid("no dynamic dataset catalog configured"))?;
    let catalog = Catalog::open(root)?;
    let entry = catalog
        .base_of(name)
        .ok_or_else(|| invalid("dataset is not cataloged"))?;
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
    if !path.is_file() {
        return Err(invalid("dataset/member does not exist"));
    }
    Ok(DynamicAllocation {
        allocation: Allocation {
            path,
            format: catalog.record_format(name),
        },
        dataset: Some(name.into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{Entry, Recfm};

    #[test]
    fn paths_and_cataloged_datasets_resolve_without_treating_options_as_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("CaseSensitive");
        let value = format!(" PATH({}) ", path.display());
        assert_eq!(allocate(&value, None).unwrap().allocation.path, path);
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
        let allocation = allocate("DSN(APP.INPUT),SHR", Some(dir.path())).unwrap();
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
            assert!(allocate(bad, Some(dir.path())).is_err(), "{bad}");
        }
    }
}

//! The basis ledger (`basis.json`) of a validation suite.
//!
//! Every check states why its expected value is right:
//!
//! - `spec`: the published IBM documentation determines the value; a failure
//!   on z/OS means the check is wrong.
//! - `zos`: the documentation does not determine the value; the expectation
//!   is the current Zeus behavior and stays provisional until a z/OS run
//!   confirms or corrects it.
//! - `zeus`: the check depends on a Zeus-only selection and cannot run on
//!   z/OS unchanged.
//!
//! A program entry gives the default; `tests` overrides single checks.
//! `zos` and `zeus` need a `note` that says what must be confirmed or adapted.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, serde::Deserialize)]
pub struct TestBasis {
    pub basis: String,
    pub note: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ProgramBasis {
    pub basis: String,
    pub note: Option<String>,
    #[serde(default)]
    pub tests: BTreeMap<String, TestBasis>,
}

fn check_kind(what: &str, basis: &str, note: &Option<String>) {
    assert!(
        matches!(basis, "spec" | "zos" | "zeus"),
        "{what}: unknown basis {basis}"
    );
    if basis != "spec" {
        assert!(
            note.as_deref().is_some_and(|n| !n.trim().is_empty()),
            "{what}: basis {basis} needs a note"
        );
    }
}

/// Checks that the ledger names exactly the programs of the coverage
/// inventory and only checks that exist; returns the count per basis.
pub fn verify(
    suite: &str,
    ledger: &str,
    coverage: &BTreeMap<String, Vec<String>>,
) -> BTreeMap<String, usize> {
    let ledger: BTreeMap<String, ProgramBasis> =
        serde_json::from_str(ledger).unwrap_or_else(|e| panic!("parse {suite}/basis.json: {e}"));
    let programs: BTreeSet<_> = coverage.keys().collect();
    let based: BTreeSet<_> = ledger.keys().collect();
    assert_eq!(
        programs, based,
        "{suite}/basis.json must name every program"
    );
    let mut counts = BTreeMap::new();
    for (program, entry) in &ledger {
        check_kind(program, &entry.basis, &entry.note);
        let tests = &coverage[program];
        for (id, test) in &entry.tests {
            assert!(
                tests.contains(id),
                "{suite}/basis.json: {program} has no check {id}"
            );
            check_kind(&format!("{program} {id}"), &test.basis, &test.note);
        }
        for id in tests {
            let basis = entry.tests.get(id).map_or(&entry.basis, |t| &t.basis);
            *counts.entry(basis.clone()).or_insert(0) += 1;
        }
        if tests.is_empty() {
            *counts.entry(entry.basis.clone()).or_insert(0) += 1;
        }
    }
    counts
}

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
//!
//! Every check cites the published documents (`docs/sources.json`) that
//! decide its value, or, for `zos`, the place that leaves it open, in `refs`: `"DOC: heading"` or
//! `"DOC: heading | quotation"`. A check's refs are its own, else its
//! program's. `tools/basis-refs.py check` confirms the headings and the
//! quotations against the documents themselves, which the repository does
//! not hold.

use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, serde::Deserialize)]
pub struct TestBasis {
    pub basis: String,
    pub note: Option<String>,
    #[serde(default)]
    pub refs: Vec<String>,
}

#[derive(Debug, serde::Deserialize)]
pub struct ProgramBasis {
    pub basis: String,
    pub note: Option<String>,
    #[serde(default)]
    pub refs: Vec<String>,
    #[serde(default)]
    pub tests: BTreeMap<String, TestBasis>,
}

/// The document identifiers of `docs/sources.json`.
fn documents() -> BTreeSet<String> {
    let sources: BTreeMap<String, serde_json::Value> =
        serde_json::from_str(include_str!("../../../../docs/sources.json"))
            .expect("parse docs/sources.json");
    sources.into_keys().collect()
}

fn check_refs(what: &str, refs: &[String], docs: &BTreeSet<String>) {
    for r in refs {
        let (doc, rest) = r
            .split_once(':')
            .unwrap_or_else(|| panic!("{what}: reference without a document: {r}"));
        assert!(
            docs.contains(doc.trim()),
            "{what}: {doc} is not in docs/sources.json"
        );
        let heading = rest.split('|').next().unwrap_or("").trim();
        assert!(
            !heading.is_empty(),
            "{what}: reference without a heading: {r}"
        );
    }
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
    let docs = documents();
    let mut counts = BTreeMap::new();
    for (program, entry) in &ledger {
        check_kind(program, &entry.basis, &entry.note);
        check_refs(program, &entry.refs, &docs);
        let tests = &coverage[program];
        for (id, test) in &entry.tests {
            assert!(
                tests.contains(id),
                "{suite}/basis.json: {program} has no check {id}"
            );
            check_kind(&format!("{program} {id}"), &test.basis, &test.note);
            check_refs(&format!("{program} {id}"), &test.refs, &docs);
        }
        for id in tests {
            let test = entry.tests.get(id);
            let basis = test.map_or(&entry.basis, |t| &t.basis);
            *counts.entry(basis.clone()).or_insert(0) += 1;
            let cited = test.is_some_and(|t| !t.refs.is_empty()) || !entry.refs.is_empty();
            assert!(
                cited,
                "{suite}/basis.json: {program} {id} has no reference to a published document"
            );
        }
        if tests.is_empty() {
            *counts.entry(entry.basis.clone()).or_insert(0) += 1;
            assert!(
                !entry.refs.is_empty(),
                "{suite}/basis.json: {program} has no reference to a published document"
            );
        }
    }
    counts
}

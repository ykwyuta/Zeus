//! Coverage of a PL/I program: `zpli --coverage`, read back with
//! `zeus_cobol::coverage` (lines, IF branches, procedures).

use std::path::Path;
use std::process::Command;

#[test]
fn pli_coverage() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/coverage.pli");
    let dir = tempfile::tempdir().unwrap();
    let data = dir.path().join("cov");
    let out = Command::new(env!("CARGO_BIN_EXE_zpli"))
        .arg("--coverage")
        .arg(&data)
        .arg(&src)
        .env_remove("ZEUS_COVERAGE")
        .output()
        .unwrap();
    assert!(out.status.success());
    let programs = zeus_cobol::coverage::load_dir(&data).unwrap();
    let [p] = programs.as_slice() else {
        panic!("{programs:?}")
    };
    assert_eq!((p.language.as_str(), p.name.as_str()), ("PL/I", "PCOV"));
    let lines: Vec<(u32, u64)> = p.lines.iter().map(|l| (l.line, l.count)).collect();
    // declarations and the procedure statement are not executable
    assert_eq!(
        lines,
        [(3, 1), (4, 5), (5, 2), (7, 3), (9, 1), (10, 1), (12, 0)]
    );
    let branches: Vec<(u32, u64, u64)> = p
        .branches
        .iter()
        .map(|b| (b.line, b.taken, b.not_taken))
        .collect();
    // IF: THEN taken / ELSE taken
    assert_eq!(branches, [(4, 2, 3), (9, 0, 1)]);
    let fns: Vec<(&str, u64)> = p
        .functions
        .iter()
        .map(|f| (f.name.as_str(), f.count))
        .collect();
    assert_eq!(fns, [("PCOV", 1), ("NEVER", 0)]);
}

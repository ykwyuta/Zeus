//! `ztestgen` on a program with EVALUATE, 88 levels, string and numeric
//! comparisons, ON SIZE ERROR, an ACCEPTed line and a loop: every
//! feasible branch direction covered, the infeasible one found, and the
//! cases replayed by `ztest` with the same coverage measured.

use std::path::Path;
use std::process::Command;

#[test]
fn covers_the_sample() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/SAMPLE.cbl");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("cases");
    let o = Command::new(env!("CARGO_BIN_EXE_ztestgen"))
        .arg("--out")
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(
        o.status.success(),
        "{text}{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        text.contains("C1: 17 of 18 branch directions")
            && text.contains("100.0% of the feasible (1 infeasible)"),
        "{text}"
    );
    // the infeasible direction: NOT ON SIZE ERROR after LK-AMOUNT > 1000.50
    assert!(
        text.contains("SAMPLE line 44 (instruction ") && text.contains(") jump: infeasible"),
        "{text}"
    );
    let report = std::fs::read_to_string(out.join("report.txt")).unwrap();
    assert_eq!(report, text);
    // a case for the user ADMIN001
    let cases: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    assert!(
        cases
            .iter()
            .any(|c| c.contains("\"LK-USER\": \"ADMIN001\"")),
        "{cases:?}"
    );
    assert!(cases.iter().any(|c| c.contains("\"LOOP\"")), "{cases:?}");
    // replayed with coverage: the same branches
    let cov = tmp.path().join("cov");
    let o = Command::new(env!("CARGO_BIN_EXE_ztest"))
        .arg("--coverage")
        .arg(&cov)
        .arg(&src)
        .arg(&out)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{text}");
    assert!(text.contains(", 0 failed"), "{text}");
    let progs = zeus_cobol::coverage::load_dir(&cov).unwrap();
    let (_, branches, _) = zeus_cobol::coverage::summary(&progs[0]);
    assert_eq!((branches.hit, branches.total), (17, 18));
}

#[test]
fn a_changed_program_fails_its_cases() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/SAMPLE.cbl");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("cases");
    let o = Command::new(env!("CARGO_BIN_EXE_ztestgen"))
        .arg("--out")
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(o.status.success());
    let changed = tmp.path().join("SAMPLE.cbl");
    let text = std::fs::read_to_string(&src)
        .unwrap()
        .replace("'DENIED'", "'REFUSED'");
    std::fs::write(&changed, text).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_ztest"))
        .arg(&changed)
        .arg(&out)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(1), "{text}");
    assert!(text.contains("1 failed"), "{text}");
}

/// A program reading a sequential file to its end and writing a report
/// file: the records and their number are inputs (AT END, FILE STATUS
/// '10'), the records written are expected results.
#[test]
fn covers_a_file_program() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs/FILEPGM.cbl");
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("cases");
    let o = Command::new(env!("CARGO_BIN_EXE_ztestgen"))
        .arg("--out")
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{text}");
    assert!(
        text.contains("C1: 16 of 16 branch directions (100.0%)"),
        "{text}"
    );
    let cases: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    // an empty input file, a large debit reported, the overdrawn balance
    assert!(
        cases.iter().any(|c| c.contains("\"records\": []")),
        "{cases:?}"
    );
    assert!(
        cases.iter().any(|c| c.contains("\"TR-TYPE\": \"D\"")),
        "{cases:?}"
    );
    assert!(
        cases.iter().any(|c| c.contains("\"return_code\": 4")),
        "{cases:?}"
    );
    // the report file is compared on replay
    let o = Command::new(env!("CARGO_BIN_EXE_ztest"))
        .arg(&src)
        .arg(&out)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert!(o.status.success(), "{text}");
    let changed = tmp.path().join("FILEPGM.cbl");
    let text = std::fs::read_to_string(&src)
        .unwrap()
        .replace("'LARGE DEBIT'", "'BIG DEBIT'");
    std::fs::write(&changed, text).unwrap();
    let o = Command::new(env!("CARGO_BIN_EXE_ztest"))
        .arg(&changed)
        .arg(&out)
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&o.stdout);
    assert_eq!(o.status.code(), Some(1), "{text}");
    assert!(text.contains("file RPT-FILE: records"), "{text}");
}

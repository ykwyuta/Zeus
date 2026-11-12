//! The z/OS results tool (`tools/zos-results.py`) must read what comes back
//! from z/OS the same way the runner reads Zeus reports, and must sort each
//! failure by its basis. A tool that misreads a report would turn a z/OS run
//! into wrong conclusions about Zeus, so the tool is checked here.

use std::fs;
use std::path::Path;
use std::process::Command;

fn tool() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/zos-results.py")
}

fn ebcdic_records(lines: &[&str]) -> Vec<u8> {
    // cp037 for the characters the report uses
    let mut out = Vec::new();
    for line in lines {
        let mut record = vec![0x40u8; 120];
        for (i, c) in line.bytes().enumerate() {
            record[i] = match c {
                b' ' => 0x40,
                b'A'..=b'I' => 0xc1 + (c - b'A'),
                b'J'..=b'R' => 0xd1 + (c - b'J'),
                b'S'..=b'Z' => 0xe2 + (c - b'S'),
                b'0'..=b'9' => 0xf0 + (c - b'0'),
                b'-' => 0x60,
                b'*' => 0x5c,
                _ => panic!("unmapped {c}"),
            };
        }
        out.extend(record);
    }
    out
}

#[test]
fn zos_results_sorts_failures_by_basis() {
    let tmp = tempfile::tempdir().unwrap();
    let reports = tmp.path().join("reports");
    fs::create_dir(&reports).unwrap();
    // raw dataset: fixed 120-byte EBCDIC records
    fs::write(
        reports.join("HS105A"),
        ebcdic_records(&["PASS  TP-BAD-SIGN-CC", "FAIL* TP-BAD-DIGIT-CC"]),
    )
    .unwrap();
    // text download
    fs::write(
        reports.join("HD107A.txt"),
        "FAIL* E-TENTH\nPASS  F-FRACTION\nPASS  FS-SCALE\n",
    )
    .unwrap();
    let steps = tmp.path().join("steps.txt");
    fs::write(
        &steps,
        "PROGRAM COMPILE-RC COMPLETION\nHS105A 0 0\nHE101N 4 -\n",
    )
    .unwrap();
    let results = tmp.path().join("r.json");
    let status = Command::new("python3")
        .arg(tool())
        .args(["record", "--suite", "zhvs", "--reports"])
        .arg(&reports)
        .arg("--steps")
        .arg(&steps)
        .args(["--env", "compiler=test", "--date", "2026-11-12", "--out"])
        .arg(&results)
        .status()
        .expect("run python3");
    assert!(status.success());
    let recorded: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&results).unwrap()).unwrap();
    let hs = &recorded["programs"]["HS105A"]["tests"];
    assert_eq!(hs["TP-BAD-SIGN-CC"], "PASS");
    assert_eq!(hs["TP-BAD-DIGIT-CC"], "FAIL");
    let hd = &recorded["programs"]["HD107A"]["tests"];
    assert_eq!(hd["E-TENTH"], "FAIL");
    assert_eq!(hd["D-TENTH"], "MISSING");

    let out = Command::new("python3")
        .arg(tool())
        .args(["compare", "--suite", "zhvs", "--results"])
        .arg(&results)
        .output()
        .expect("run python3");
    assert_eq!(
        out.status.code(),
        Some(1),
        "failures must give a nonzero exit"
    );
    let text = String::from_utf8(out.stdout).unwrap();
    let section = |head: &str| {
        let start = text
            .find(head)
            .unwrap_or_else(|| panic!("no {head} in\n{text}"));
        let rest = &text[start..];
        rest[..rest.find("\n\n").unwrap_or(rest.len())].to_string()
    };
    // TP's condition code is fixed by Principles of Operation: the check is wrong
    assert!(section("spec FAIL").contains("HS105A TP-BAD-DIGIT-CC"));
    // E'0.1' rounding is the current Zeus value: Zeus differs from z/OS
    assert!(section("zos FAIL").contains("HD107A E-TENTH"));
    // a manifest exit code means the step must not end with RC 0
    assert!(section("STEP").contains("HS105A"));
    // a negative check must be rejected (RC 8 or more)
    assert!(section("STEP").contains("HE101N"));
}

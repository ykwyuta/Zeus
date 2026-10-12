//! NATIONAL data (UTF-16): N / NX literals, moves to and from
//! alphanumeric and DBCS items, NATIONAL-OF / DISPLAY-OF, comparisons,
//! LENGTH / BYTE-LENGTH, reference modification in characters and ACCEPT.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[test]
fn national_data() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/national.cbl");
    let dir = tempfile::tempdir().unwrap();
    let zo = dir.path().join("NAT1.zo");
    let st = Command::new(env!("CARGO_BIN_EXE_zcobc"))
        .args(["compile", "--ccsid", "939", "-o"])
        .arg(&zo)
        .arg(&src)
        .status()
        .unwrap();
    assert!(st.success());
    let mut child = Command::new(env!("CARGO_BIN_EXE_zcobc"))
        .arg("run")
        .arg(&zo)
        .env_remove("ZEUS_CCSID")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all("名古屋\n".as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    assert_eq!(
        lines,
        [
            "東京    |",
            "AB",
            "ABC   |",
            "東京              |",
            "大阪\u{3000}\u{3000}  |",
            "6",
            "12",
            "日本    |",
            "日本    |",
            "EQUAL",
            "GREATER",
            "SPACES",
            "京",
            "IN=名古屋       |",
        ]
    );
}

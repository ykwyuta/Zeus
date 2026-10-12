//! Japanese EBCDIC (CCSIDs 930, 939, 1390, 1399): mixed literals (SO /
//! SI), DBCS items and literals (PIC G / N, G"..." / N"..."), DISPLAY and
//! ACCEPT through zcobc (each run is a process with its code page), and
//! DBCS user-defined words.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[test]
fn japanese_code_pages() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/japanese.cbl");
    let dir = tempfile::tempdir().unwrap();
    let zo = dir.path().join("JP1.zo");
    for ccsid in ["930", "939", "1390", "1399"] {
        let st = Command::new(env!("CARGO_BIN_EXE_zcobc"))
            .args(["compile", "--ccsid", ccsid, "-o"])
            .arg(&zo)
            .arg(&src)
            .status()
            .unwrap();
        assert!(st.success());
        // the run takes the module's code page
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
            .write_all("東京タワー\n".as_bytes())
            .unwrap();
        let out = child.wait_with_output().unwrap();
        let text = String::from_utf8(out.stdout).unwrap();
        let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
        assert_eq!(
            lines,
            [
                "コード変換 ABC 日本語",
                "漢字データ",
                "山田  様",
                "12",
                "入力=東京タワー",
                "合計=00042"
            ],
            "CCSID {ccsid}"
        );
    }
}

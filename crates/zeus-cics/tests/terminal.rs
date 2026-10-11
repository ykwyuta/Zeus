//! Terminal facilities through zcics: START to a printer (now and after an
//! interval) with RETRIEVE, the alternate screen size, structured fields
//! and RECEIVE BUFFER (programs and script in tests/terminal).

use std::path::Path;
use std::process::Command;

use zeus_cobol::front::FrontOptions;

#[test]
fn terminal_facilities() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/terminal");
    let dir = tempfile::tempdir().unwrap();
    let loadlib = dir.path().join("loadlib");
    std::fs::create_dir(&loadlib).unwrap();
    for p in ["ZPRT", "ZPR2P", "ZALT", "ZSF", "ZBUF"] {
        let module =
            zeus_cobol::compile_file(&src.join(format!("{p}.cbl")), &FrontOptions::default())
                .unwrap();
        zeus_cobol::save_module(&module, &loadlib.join(format!("{p}.zo"))).unwrap();
    }
    let out = Command::new(env!("CARGO_BIN_EXE_zcics"))
        .arg("--home")
        .arg(dir.path())
        .arg("--csd")
        .arg(src.join("TEST.CSD"))
        .arg("--loadlib")
        .arg(&loadlib)
        .args(["--model", "5", "--printer", "PRT1"])
        .arg(src.join("script.txt"))
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{stdout}{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // the Read Partition Query went out as a Write Structured Field
    assert!(stdout.contains("< F3000501FF02"), "{stdout}");
    // both STARTs printed at PRT1 (the second after its interval)
    let printed = std::fs::read_to_string(dir.path().join("print/PRT1.txt")).unwrap();
    assert_eq!(
        printed,
        "ZPR2: HELLO PRINTER        SD\n\u{c}ZPR2: PRINTED LATER        SD\n\u{c}"
    );
}

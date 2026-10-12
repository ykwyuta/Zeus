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
    for p in ["ZPRT", "ZPR2P", "ZALT", "ZSF", "ZBUF", "ZSA"] {
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

/// A Japanese BMS map (CCSID 939): double-byte INITIAL values, a SOSI
/// field the operator types kanji into, mixed data in the reply.
#[test]
fn japanese_screen() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/japanese");
    let dir = tempfile::tempdir().unwrap();
    let (maps, copy, loadlib) = (
        dir.path().join("maps"),
        dir.path().join("copy"),
        dir.path().join("loadlib"),
    );
    for d in [&maps, &copy, &loadlib] {
        std::fs::create_dir(d).unwrap();
    }
    let run = |cmd: &mut Command| {
        let out = cmd.env("ZEUS_CCSID", "939").output().unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(out.status.success(), "{text}");
        text
    };
    let bin = |name: &str| Path::new(env!("CARGO_BIN_EXE_zcics")).with_file_name(name);
    run(Command::new(bin("zbms"))
        .arg(src.join("JPMAP.bms"))
        .arg("--copy")
        .arg(copy.join("JPMAP.cpy"))
        .arg("--map")
        .arg(maps.join("JPMAP.json")));
    run(Command::new(bin("zcobc"))
        .args(["compile", "--cics", "-I"])
        .arg(&copy)
        .arg("-o")
        .arg(loadlib.join("JPGM.zo"))
        .arg(src.join("JPGM.cbl")));
    run(Command::new(env!("CARGO_BIN_EXE_zcics"))
        .arg("--home")
        .arg(dir.path())
        .arg("--csd")
        .arg(src.join("TEST.CSD"))
        .arg("--loadlib")
        .arg(&loadlib)
        .arg("--mapsets")
        .arg(&maps)
        .arg(src.join("script.txt")));
}

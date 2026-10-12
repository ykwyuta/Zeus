//! PL/I CICS programs (tests/pli): a pseudo-conversational transaction
//! with a BMS map, a VSAM file and a COMMAREA, which LINKs to a COBOL
//! program that LINKs to a PL/I program and CALLs a PL/I procedure, all on
//! the task's storage.

use std::path::Path;
use std::process::Command;

use zeus_ds::catalog::{Catalog, Entry, EntryKind, VsamOrg};

#[test]
fn pli_transaction() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pli");
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
        let out = cmd.output().unwrap();
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
        .arg(src.join("PLMAP.bms"))
        .arg("--pli")
        .arg("--copy")
        .arg(copy.join("PLMAP.pli"))
        .arg("--map")
        .arg(maps.join("PLMAP.json")));
    for p in ["PLGR", "PLUPPER", "PLLEN", "PLRET"] {
        run(Command::new(bin("zpli"))
            .args(["compile", "-I"])
            .arg(&copy)
            .arg("-o")
            .arg(loadlib.join(format!("{p}.zo")))
            .arg(src.join(format!("{p}.pli"))));
    }
    run(Command::new(bin("zcobc"))
        .args(["compile", "--cics", "-o"])
        .arg(loadlib.join("CBGREET.zo"))
        .arg(src.join("CBGREET.cbl")));
    let mut catalog = Catalog::open(&dir.path().join("catalog")).unwrap();
    catalog
        .define(Entry {
            name: "ZTEST.NAMES".into(),
            kind: EntryKind::Cluster {
                org: VsamOrg::Ksds,
                keys: Some((10, 0)),
                record_size: (30, 30),
                aixs: vec![],
                log: None,
            },
        })
        .unwrap();
    catalog.save().unwrap();
    let out = run(Command::new(env!("CARGO_BIN_EXE_zcics"))
        .arg("--home")
        .arg(dir.path())
        .arg("--csd")
        .arg(src.join("TEST.CSD"))
        .arg("--loadlib")
        .arg(&loadlib)
        .arg("--mapsets")
        .arg(&maps)
        .arg(src.join("script.txt")));
    assert!(!out.contains("abended"), "{out}");
}

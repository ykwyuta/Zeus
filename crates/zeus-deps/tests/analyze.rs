//! `zdeps analyze` on a small COBOL / PL/I / JCL application.

use std::fs;
use std::path::Path;
use std::process::Command;

fn write(dir: &Path, name: &str, text: &str) {
    let p = dir.join(name);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn rows(dir: &Path, name: &str) -> Vec<Vec<String>> {
    let text = fs::read_to_string(dir.join(name)).unwrap();
    text.lines()
        .skip(1)
        .map(|l| l.split(',').map(str::to_string).collect())
        .collect()
}

#[test]
fn analyze_writes_the_csvs() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tmp.path().join("app");
    write(
        &app,
        "jcl/RUNJOB.jcl",
        "//RUNJOB   JOB (ACCT),'TEST'
//STEP1    EXEC PGM=MAINPGM
//STEP2    EXEC PGM=PLIPGM
//STEP3    EXEC PGM=IEFBR14
//STEP4    EXEC PGM=NOSUCH
",
    );
    write(
        &app,
        "cbl/MAINPGM.cbl",
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAINPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       COPY CUSTREC.
       COPY NOCOPY.
       COPY MS1.
       PROCEDURE DIVISION.
           CALL WS-SUB
           CALL 'SUBPGM'
           CALL WS-UNKNOWN
           GOBACK.
",
    );
    write(
        &app,
        "bms/MS1.bms",
        "MS1      DFHMSD TYPE=&SYSPARM,LANG=COBOL,MODE=INOUT,STORAGE=AUTO
M1       DFHMDI SIZE=(24,80)
F1       DFHMDF POS=(1,1),LENGTH=5
         DFHMSD TYPE=FINAL
         END
",
    );
    write(
        &app,
        "cpy/CUSTREC.cpy",
        "       01 WS-SUB PIC X(8) VALUE 'SUBPGM'.\n",
    );
    write(
        &app,
        "cbl/SUBPGM.cbl",
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. SUBPGM.
       PROCEDURE DIVISION.
           GOBACK.
",
    );
    write(
        &app,
        "pli/PLIPGM.pli",
        " PLIPGM: PROC OPTIONS(MAIN);
   %INCLUDE PLIINC;
   PUT SKIP LIST('HI');
 END PLIPGM;
",
    );
    write(&app, "pli/PLIINC.inc", " DCL X FIXED BIN(31) INIT(1);\n");
    let out = tmp.path().join("out");
    let st = Command::new(env!("CARGO_BIN_EXE_zdeps"))
        .args(["analyze", "--no-compile", "--out"])
        .arg(&out)
        .arg(app.join("jcl/RUNJOB.jcl"))
        .arg("--lib")
        .arg(&app)
        .status()
        .unwrap();
    // missing assets make the exit code 4
    assert_eq!(st.code(), Some(4));
    for f in [
        "nodes.csv",
        "edges.csv",
        "node_attributes.csv",
        "edge_attributes.csv",
        "edge_locations.csv",
        "missing_assets.csv",
    ] {
        assert!(out.join(f).exists(), "{f}");
    }
    let nodes = rows(&out, "nodes.csv");
    let node = |name: &str| {
        nodes
            .iter()
            .find(|r| r[2] == name)
            .unwrap_or_else(|| panic!("node {name}"))
            .clone()
    };
    assert_eq!(node("RUNJOB")[1], "JCL_JOB");
    assert_eq!(node("MAINPGM")[1], "COBOL_PROGRAM");
    assert_eq!(node("CUSTREC")[1], "COBOL_COPYBOOK");
    assert_eq!(node("SUBPGM")[3], "FILE");
    assert_eq!(node("PLIINC")[1], "PLI_INCLUDE");
    assert_eq!(node("IEFBR14")[3], "SYSTEM");
    assert_eq!(node("NOSUCH")[3], "MISSING");
    assert_eq!(node("NOCOPY")[3], "MISSING");
    assert_eq!(node("WS-UNKNOWN")[3], "DYNAMIC");

    let edges = rows(&out, "edges.csv");
    let main_id = &node("MAINPGM")[0];
    let sub_id = &node("SUBPGM")[0];
    // CALL WS-SUB (VALUE in the copybook) and CALL 'SUBPGM' share one edge
    let call = edges
        .iter()
        .find(|e| e[1] == "CALL" && &e[2] == main_id && &e[4] == sub_id)
        .expect("CALL edge");
    assert_eq!(call[7], "2");
    let locs = rows(&out, "edge_locations.csv");
    let mut at: Vec<(String, String)> = locs
        .iter()
        .filter(|l| l[0] == call[0])
        .map(|l| (l[4].clone(), l[5].clone()))
        .collect();
    at.sort();
    assert_eq!(at, [("10".into(), "17".into()), ("9".into(), "17".into())]);
    let attrs = rows(&out, "edge_attributes.csv");
    assert!(
        attrs
            .iter()
            .any(|a| a[0] == call[0] && a[2] == "via" && a[3] == "VALUE")
    );
    let nattrs = rows(&out, "node_attributes.csv");
    assert!(
        nattrs
            .iter()
            .any(|a| &a[0] == main_id && a[2] == "compile_status" && a[3] == "NOT_CHECKED")
    );

    // the symbolic map of a mapset
    let ms1 = node("MS1");
    assert_eq!(ms1[1], "BMS_MAPSET");
    let copy = edges
        .iter()
        .find(|e| e[1] == "COPY" && &e[2] == main_id && e[4] == ms1[0])
        .expect("COPY MS1");
    assert!(
        attrs
            .iter()
            .any(|a| a[0] == copy[0] && a[2] == "via" && a[3] == "BMS")
    );
    // every edge joins node types its master allows
    for e in &edges {
        let t = zeus_deps::master::EDGE_TYPES
            .iter()
            .find(|t| t.code == e[1])
            .unwrap();
        let ty = |id: &str| nodes.iter().find(|n| n[0] == id).unwrap()[1].clone();
        assert!(t.from.contains(&ty(&e[2]).as_str()), "{e:?}");
        assert!(t.to.contains(&ty(&e[4]).as_str()), "{e:?}");
    }

    let missing = rows(&out, "missing_assets.csv");
    let mut names: Vec<(String, String)> = missing
        .iter()
        .map(|m| (m[0].clone(), m[2].clone()))
        .collect();
    names.sort();
    names.dedup();
    assert_eq!(
        names,
        [
            ("NOCOPY".into(), "NOT_FOUND".into()),
            ("NOSUCH".into(), "NOT_FOUND".into()),
            ("WS-UNKNOWN".into(), "DYNAMIC".into()),
        ]
    );
}

#[test]
fn masters() {
    let tmp = tempfile::tempdir().unwrap();
    let st = Command::new(env!("CARGO_BIN_EXE_zdeps"))
        .args(["masters", "--out"])
        .arg(tmp.path())
        .status()
        .unwrap();
    assert!(st.success());
    let types = rows(tmp.path(), "node_types.csv");
    assert!(types.iter().any(|r| r[0] == "COBOL_PROGRAM"));
    let edges = rows(tmp.path(), "edge_types.csv");
    assert!(edges.iter().any(|r| r[0] == "CICS_LINK"));
    for f in [
        "node_attribute_types.csv",
        "edge_attribute_types.csv",
        "system_assets.csv",
    ] {
        assert!(tmp.path().join(f).exists(), "{f}");
    }
}

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
        "unsupported_syntax.csv",
        "unsupported_syntax_locations.csv",
        "compile_diagnostics.csv",
    ] {
        assert!(out.join(f).exists(), "{f}");
    }
    let nodes = rows(&out, "nodes.csv");
    for file in [
        "unsupported_syntax.csv",
        "unsupported_syntax_locations.csv",
        "compile_diagnostics.csv",
    ] {
        assert!(rows(&out, file).is_empty(), "--no-compile: {file}");
    }
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

#[test]
fn unsupported_syntax_reports_each_use_and_distinguishes_compile_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tmp.path().join("app");
    write(&app, "OPS.cpy", "           SEND 'COPY MESSAGE'.\n");
    for (name, body) in [
        ("A", "           COPY OPS.\n           SEND 'SECOND'.\n"),
        ("B", "           COPY OPS.\n           RECEIVE MESSAGE.\n"),
        ("BROKEN", "           MOVE 1 TO UNDEFINED-NAME.\n"),
    ] {
        write(
            &app,
            &format!("{name}.cbl"),
            &format!(
                "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. {name}.\n       PROCEDURE DIVISION.\n{body}           GOBACK.\n"
            ),
        );
    }
    write(
        &app,
        "P.pli",
        " P: PROC OPTIONS(MAIN);\n %INCLUDE PIO;\n END P;\n",
    );
    write(&app, "PIO.inc", " PUT BOGUS;\n");
    let out = tmp.path().join("out");
    let result = Command::new(env!("CARGO_BIN_EXE_zdeps"))
        .args(["analyze", "--bom", "--out"])
        .arg(&out)
        .arg(&app)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        String::from_utf8_lossy(&result.stdout)
            .contains("3 unsupported syntax kinds, 5 occurrences")
    );
    let summary = rows(&out, "unsupported_syntax.csv");
    assert_eq!(summary.len(), 3, "{summary:?}");
    let send = summary.iter().find(|r| r[2] == "SEND").unwrap();
    assert_eq!(&send[1..], ["COBOL", "SEND", "RUNTIME", "3", "2", "2"]);
    let locations = rows(&out, "unsupported_syntax_locations.csv");
    let send_uses: Vec<_> = locations.iter().filter(|r| r[0] == send[0]).collect();
    assert_eq!(send_uses.len(), 3);
    let copy_uses: Vec<_> = send_uses
        .iter()
        .filter(|r| r[7].ends_with("OPS.cpy"))
        .collect();
    assert_eq!(copy_uses.len(), 2);
    assert!(
        copy_uses
            .iter()
            .all(|r| r[8] == "1" && r[9] == "SEND 'COPY MESSAGE'.")
    );
    assert!(
        send_uses
            .iter()
            .any(|r| r[2] == "A" && r[8] == "5" && r[7].ends_with("A.cbl"))
    );
    let pli = locations.iter().find(|r| r[4] == "PL/I").unwrap();
    assert_eq!(pli[5], "PUT option BOGUS is not supported");
    assert_eq!(pli[6], "COMPILE");
    assert!(pli[7].ends_with("PIO.inc"));
    assert_eq!(pli[8], "1");
    assert_eq!(pli[9], "PUT BOGUS;");
    let diagnostics = rows(&out, "compile_diagnostics.csv");
    assert_eq!(diagnostics.len(), 6, "{diagnostics:?}");
    assert!(
        diagnostics
            .iter()
            .any(|r| r[1] == "BROKEN" && r[4] == "ERROR")
    );
    assert!(!locations.iter().any(|r| r[2] == "BROKEN"));
    for file in [
        "unsupported_syntax.csv",
        "unsupported_syntax_locations.csv",
        "compile_diagnostics.csv",
    ] {
        assert!(
            fs::read(out.join(file))
                .unwrap()
                .starts_with(b"\xef\xbb\xbf")
        );
    }
}

//! `zlayout` on a small program: files with several layouts and their
//! record types, a card, parameters, call arguments and a query result.

use std::fs;
use std::path::Path;
use std::process::Command;

const PROGRAM: &str = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. LAYTEST.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT IN-FILE ASSIGN TO UT-S-INFILE.
           SELECT CARDS ASSIGN TO SYSIN.
       DATA DIVISION.
       FILE SECTION.
       FD  IN-FILE.
       01  IN-REC                    PIC X(40).
       FD  CARDS.
       01  CARD-REC                  PIC X(80).
       WORKING-STORAGE SECTION.
       COPY RECS.
       01  WS-PARM                   PIC X(10).
       01  WS-NAME                   PIC X(20).
       01  WS-AMT                    PIC S9(7)V99 COMP-3.
       01  WS-AMT-IND                PIC S9(4) COMP.
       LINKAGE SECTION.
       01  LK-AREA.
           05  LK-CODE               PIC X(2).
           05  LK-DATE               PIC X(8).
           05  LK-DATE-R REDEFINES LK-DATE.
               10  LK-YYYY           PIC 9(4).
               10  LK-MM             PIC 9(2).
               10  LK-DD             PIC 9(2).
       PROCEDURE DIVISION USING LK-AREA.
       MAIN-PARA.
           OPEN INPUT IN-FILE CARDS.
           READ IN-FILE INTO TX-RECORD.
           EVALUATE TX-TYPE
               WHEN 'H'
                   PERFORM HEADER-PARA
               WHEN 'D'
                   MOVE TX-AMOUNT TO WS-AMT
           END-EVALUATE.
           READ CARDS.
           ACCEPT WS-PARM.
           CALL 'SUBPGM' USING WS-PARM TX-RECORD.
           EXEC SQL
               SELECT NAME, AMOUNT INTO :WS-NAME, :WS-AMT :WS-AMT-IND
                 FROM T WHERE ID = :WS-PARM
           END-EXEC.
           GOBACK.
       HEADER-PARA.
           DISPLAY TX-RUN-DATE.
";

const RECS: &str = "       01  TX-RECORD.
           05  TX-TYPE               PIC X.
               88  TX-HEADER         VALUE 'H'.
           05  TX-BODY               PIC X(39).
           05  TX-HEAD REDEFINES TX-BODY.
               10  TX-RUN-DATE       PIC X(8).
               10  FILLER            PIC X(31).
           05  TX-DETAIL REDEFINES TX-BODY.
               10  TX-AMOUNT         PIC S9(7)V99 COMP-3.
               10  TX-TEXT           PIC X(34).
";

fn read(dir: &Path, name: &str) -> String {
    fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn layouts_of_a_program() {
    let tmp = tempfile::tempdir().unwrap();
    let src = tmp.path().join("LAYTEST.cbl");
    fs::write(&src, PROGRAM).unwrap();
    fs::write(tmp.path().join("RECS.cpy"), RECS).unwrap();
    let out = tmp.path().join("out");
    let o = Command::new(env!("CARGO_BIN_EXE_zlayout"))
        .arg("--out")
        .arg(&out)
        .arg(&src)
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let mut files: Vec<String> = fs::read_dir(&out)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(
        files,
        [
            "LAYTEST.CALL.SUBPGM.1.cpy",
            "LAYTEST.CALL.SUBPGM.2.DETAIL.cpy",
            "LAYTEST.CALL.SUBPGM.2.HEAD.cpy",
            "LAYTEST.CARD.SYSIN.CARD-REC.cpy",
            "LAYTEST.CARD.SYSIN.WS-PARM.cpy",
            "LAYTEST.FILE.INFILE.IN-REC.cpy",
            "LAYTEST.FILE.INFILE.TX-RECORD.DETAIL.cpy",
            "LAYTEST.FILE.INFILE.TX-RECORD.HEAD.cpy",
            "LAYTEST.PARM.1.cpy",
            "LAYTEST.SQLRESULT.SELECT.MAIN-PARA.cpy",
            "layouts.csv",
        ]
    );
    // the header layout: chosen by TX-TYPE = 'H', no REDEFINES left
    let head = read(&out, "LAYTEST.FILE.INFILE.TX-RECORD.HEAD.cpy");
    assert!(head.contains("* SELECTED  : TX-TYPE = 'H'"), "{head}");
    assert!(!head.contains("REDEFINES"), "{head}");
    assert!(head.contains("88  TX-HEADER"), "{head}");
    let line = head.lines().find(|l| l.contains("TX-RUN-DATE")).unwrap();
    assert!(line.ends_with("00002"), "{line}");
    let detail = read(&out, "LAYTEST.FILE.INFILE.TX-RECORD.DETAIL.cpy");
    assert!(detail.contains("* SELECTED  : TX-TYPE = 'D'"), "{detail}");
    let line = detail.lines().find(|l| l.contains("TX-TEXT")).unwrap();
    assert!(line.ends_with("00007"), "{line}");
    // a parameter: the date in its parts (the PIC X(8) it redefines is
    // a plain container)
    let p = read(&out, "LAYTEST.PARM.1.cpy");
    let body: String = p.lines().filter(|l| !l.starts_with("      *")).collect();
    assert!(
        body.contains("10  LK-MM") && !body.contains("LK-DATE "),
        "{p}"
    );
    // the query result: the host variables and the indicator as one record
    let q = read(&out, "LAYTEST.SQLRESULT.SELECT.MAIN-PARA.cpy");
    let body: Vec<&str> = q.lines().filter(|l| !l.starts_with("      *")).collect();
    assert!(body[0].starts_with("       01  SELECT-ROW."), "{q}");
    assert!(
        body[1].contains("05  WS-NAME") && body[1].ends_with("00001"),
        "{q}"
    );
    assert!(
        body[2].contains("05  WS-AMT ") && body[2].ends_with("00021"),
        "{q}"
    );
    assert!(
        body[3].contains("05  WS-AMT-IND") && body[3].ends_with("00026"),
        "{q}"
    );
    let csv = read(&out, "layouts.csv");
    assert!(csv.lines().skip(1).all(|l| l.ends_with(",OK")), "{csv}");
}

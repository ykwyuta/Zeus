//! End-to-end tests: compile a COBOL program and run it.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use zeus_cobol::front::FrontOptions;
use zeus_cobol::vm::Machine;

#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Compiles `body` (lines in area A/B, columns 8-72) and returns what it
/// DISPLAYs.
fn run(body: &str, debug: bool) -> String {
    run_with(body, debug, &[])
}

fn run_with(body: &str, debug: bool, dds: &[(&str, std::path::PathBuf)]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("T.cbl");
    let text: String = body.lines().map(|l| format!("       {l}\n")).collect();
    std::fs::write(&path, text).unwrap();
    let module = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap();
    let out = Capture::default();
    let mut m = Machine::new(vec![]);
    m.set_stdout(Box::new(out.clone()));
    m.set_debug(debug);
    for (n, p) in dds {
        m.set_dd(n, p.clone());
    }
    m.run_main(module).unwrap();
    String::from_utf8(out.0.borrow().clone()).unwrap()
}

#[test]
fn independent_segment_is_reset_on_entry() {
    let out = run(
        "IDENTIFICATION DIVISION.
PROGRAM-ID. SEG.
PROCEDURE DIVISION.
MAIN SECTION.
M-1.
    PERFORM IND.
    PERFORM IND.
    PERFORM FIX.
    PERFORM FIX.
    STOP RUN.
IND SECTION 60.
I-1.
    GO TO I-3.
I-2.
    DISPLAY \"ALTERED\".
    GO TO I-4.
I-3.
    DISPLAY \"INITIAL\".
    ALTER I-1 TO PROCEED TO I-2.
I-4.
    EXIT.
FIX SECTION 10.
F-1.
    GO TO F-3.
F-2.
    DISPLAY \"ALTERED\".
    GO TO F-4.
F-3.
    DISPLAY \"INITIAL\".
    ALTER F-1 TO PROCEED TO F-2.
F-4.
    EXIT.",
        false,
    );
    // the independent segment starts over; the fixed one keeps its state
    assert_eq!(out, "INITIAL\nINITIAL\nINITIAL\nALTERED\n");
}

#[test]
fn debugging_section_on_procedure() {
    let src = "IDENTIFICATION DIVISION.
PROGRAM-ID. DBG.
ENVIRONMENT DIVISION.
CONFIGURATION SECTION.
SOURCE-COMPUTER. ZEUS WITH DEBUGGING MODE.
PROCEDURE DIVISION.
DECLARATIVES.
D-1 SECTION.
    USE FOR DEBUGGING ON P-2.
D-1-A.
    DISPLAY DEBUG-NAME(1:3) \"/\" DEBUG-CONTENTS(1:12).
END DECLARATIVES.
MAIN SECTION.
P-1.
    PERFORM P-2.
P-2.
    EXIT.
P-3.
    STOP RUN.";
    assert_eq!(run(src, true), "P-2/PERFORM LOOP\nP-2/FALL THROUGH\n");
    // object-time switch off: the debugging section does not run
    assert_eq!(run(src, false), "");
}

#[test]
fn report_writer_control_breaks_and_sums() {
    let dir = tempfile::tempdir().unwrap();
    let rep = dir.path().join("rep.txt");
    let src = "IDENTIFICATION DIVISION.
PROGRAM-ID. RPT.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT RFILE ASSIGN TO \"RWTEST1\".
DATA DIVISION.
FILE SECTION.
FD  RFILE REPORT IS SALES.
WORKING-STORAGE SECTION.
01  REGION PIC X(4).
01  AMOUNT PIC 999.
REPORT SECTION.
RD  SALES CONTROLS ARE FINAL REGION
    PAGE LIMIT 20 HEADING 1 FIRST DETAIL 3
    LAST DETAIL 18 FOOTING 19.
01  TYPE RH LINE 1.
    03 COLUMN 1 PIC X(5) VALUE \"SALES\".
01  TYPE PH LINE 2.
    03 COLUMN 1 PIC X(4) VALUE \"PAGE\".
    03 COLUMN 6 PIC 9 SOURCE PAGE-COUNTER.
01  TYPE CH REGION LINE PLUS 1.
    03 COLUMN 1 PIC X(4) SOURCE REGION.
01  DL TYPE DE LINE PLUS 1.
    03 COLUMN 3 PIC X(4) SOURCE REGION GROUP INDICATE.
    03 COLUMN 8 PIC ZZ9 SOURCE AMOUNT.
01  TYPE CF REGION LINE PLUS 1.
    03 COLUMN 1 PIC X(5) VALUE \"TOTAL\".
    03 RTOT COLUMN 8 PIC ZZZ9 SUM AMOUNT.
01  TYPE CF FINAL LINE PLUS 2.
    03 COLUMN 1 PIC X(5) VALUE \"GRAND\".
    03 COLUMN 8 PIC ZZZ9 SUM RTOT.
PROCEDURE DIVISION.
    OPEN OUTPUT RFILE.
    INITIATE SALES.
    MOVE \"EAST\" TO REGION. MOVE 10 TO AMOUNT. GENERATE DL.
    MOVE 20 TO AMOUNT. GENERATE DL.
    MOVE \"WEST\" TO REGION. MOVE 5 TO AMOUNT. GENERATE DL.
    TERMINATE SALES.
    CLOSE RFILE.
    STOP RUN.";
    run_with(src, false, &[("RWTEST1", rep.clone())]);
    let out = std::fs::read_to_string(&rep).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(
        lines,
        [
            "SALES",
            "PAGE 1",
            "EAST",
            "  EAST  10",
            "        20",
            "TOTAL    30",
            "WEST",
            "  WEST   5",
            "TOTAL     5",
            "",
            "GRAND    35",
        ]
    );
}

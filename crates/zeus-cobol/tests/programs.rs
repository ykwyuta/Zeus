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
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("T.cbl");
    let text: String = body.lines().map(|l| format!("       {l}\n")).collect();
    std::fs::write(&path, text).unwrap();
    let module = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap();
    let out = Capture::default();
    let mut m = Machine::new(vec![]);
    m.set_stdout(Box::new(out.clone()));
    m.set_debug(debug);
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

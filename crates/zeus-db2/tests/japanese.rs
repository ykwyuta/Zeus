//! Japanese data (CCSID 939) through Db2: mixed data (PIC X with SO / SI)
//! and DBCS data (PIC G) as host variables, stored in a Unicode database
//! and read back. Runs against ZEUS_DB2 (skipped when it is not set); a
//! process of its own, as the code page is chosen per process.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use zeus_cobol::front::FrontOptions;
use zeus_cobol::vm::Machine;
use zeus_db2::Db2Host;

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

const PROGRAM: &str = "IDENTIFICATION DIVISION.
PROGRAM-ID. SQLJP.
DATA DIVISION.
WORKING-STORAGE SECTION.
    EXEC SQL INCLUDE SQLCA END-EXEC.
01 WS-ID       PIC S9(9) COMP.
01 WS-NAME     PIC X(30).
01 WS-KANJI    PIC G(8).
01 WS-COUNT    PIC S9(9) COMP.
PROCEDURE DIVISION.
    EXEC SQL DROP TABLE ZEUSTEST.JP1 END-EXEC.
    EXEC SQL CREATE TABLE ZEUSTEST.JP1 (ID INTEGER NOT NULL,
        NAME VARCHAR(60), KANJI VARCHAR(40)) END-EXEC.
    MOVE 1 TO WS-ID.
    MOVE 'ヤマダ 山田太郎' TO WS-NAME.
    MOVE G'東京都千代田区' TO WS-KANJI.
    EXEC SQL INSERT INTO ZEUSTEST.JP1
        VALUES (:WS-ID, :WS-NAME, :WS-KANJI) END-EXEC.
    DISPLAY 'INSERT ' SQLCODE.
    MOVE SPACES TO WS-NAME.
    MOVE SPACES TO WS-KANJI.
    EXEC SQL SELECT NAME, KANJI INTO :WS-NAME, :WS-KANJI
        FROM ZEUSTEST.JP1 WHERE ID = 1 END-EXEC.
    DISPLAY 'SELECT ' SQLCODE.
    DISPLAY WS-NAME.
    DISPLAY WS-KANJI.
    EXEC SQL SELECT COUNT(*) INTO :WS-COUNT FROM ZEUSTEST.JP1
        WHERE KANJI LIKE '東京%' END-EXEC.
    DISPLAY 'LIKE ' WS-COUNT.
    STOP RUN.";

#[test]
fn japanese_through_db2() {
    if std::env::var_os("ZEUS_DB2").is_none() {
        eprintln!("ZEUS_DB2 not set; skipped");
        return;
    }
    assert!(zeus_cobol::codepage::set_active(939));
    let host = Rc::new(Db2Host::from_env().unwrap());
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("T.cbl");
    let text: String = PROGRAM.lines().map(|l| format!("       {l}\n")).collect();
    std::fs::write(&path, text).unwrap();
    let module = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap();
    let out = Capture::default();
    let mut m = Machine::new(vec![]);
    m.set_stdout(Box::new(out.clone()));
    m.set_exec_host(host.clone());
    m.run_main(module).unwrap();
    host.end_uow(true).unwrap();
    let text = String::from_utf8(out.0.borrow().clone()).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    assert_eq!(
        lines,
        [
            "INSERT 00000000{",
            "SELECT 00000000{",
            "ヤマダ 山田太郎",
            "東京都千代田区",
            "LIKE 00000000A"
        ]
    );
}

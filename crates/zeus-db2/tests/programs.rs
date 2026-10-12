//! COBOL programs with embedded SQL against the Db2 server named by
//! ZEUS_DB2 (tools/db2ce.sh); skipped when it is not set.

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

fn run(host: &Rc<Db2Host>, body: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("T.cbl");
    let text: String = body.lines().map(|l| format!("       {l}\n")).collect();
    std::fs::write(&path, text).unwrap();
    let module = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap();
    let out = Capture::default();
    let mut m = Machine::new(vec![]);
    m.set_stdout(Box::new(out.clone()));
    m.set_exec_host(host.clone());
    m.run_main(module).unwrap();
    host.end_uow(true).unwrap();
    String::from_utf8(out.0.borrow().clone()).unwrap()
}

#[test]
fn embedded_sql() {
    let Some(host) = Db2Host::from_env() else {
        eprintln!("ZEUS_DB2 not set; skipped");
        return;
    };
    let host = Rc::new(host);
    let out = run(
        &host,
        "IDENTIFICATION DIVISION.
PROGRAM-ID. SQLT1.
DATA DIVISION.
WORKING-STORAGE SECTION.
    EXEC SQL INCLUDE SQLCA END-EXEC.
01 WS-ID       PIC S9(9) COMP.
01 WS-NAME.
   49 WS-NAME-LEN  PIC S9(4) COMP.
   49 WS-NAME-TEXT PIC X(20).
01 WS-AMT      PIC S9(7)V99 COMP-3.
01 WS-CD       PIC X(3).
01 WS-CD-IND   PIC S9(4) COMP.
01 WS-COUNT    PIC S9(9) COMP.
01 WS-NEXT     PIC S9(9) COMP.
01 WS-OUT      PIC -(7)9.99.
    EXEC SQL DECLARE C1 CURSOR FOR
        SELECT ID, NAME, AMT, CD FROM ZEUSTEST.PGM1
         WHERE ID >= :WS-ID ORDER BY ID
    END-EXEC.
PROCEDURE DIVISION.
    EXEC SQL DROP TABLE ZEUSTEST.PGM1 END-EXEC.
    EXEC SQL CREATE TABLE ZEUSTEST.PGM1 (ID INTEGER NOT NULL
        PRIMARY KEY, NAME VARCHAR(20), AMT DECIMAL(9,2),
        CD CHAR(3)) END-EXEC.
    DISPLAY 'CREATE ' SQLCODE.
    PERFORM VARYING WS-ID FROM 1 BY 1 UNTIL WS-ID > 3
        MOVE 5 TO WS-NAME-LEN
        MOVE 'O''HAR' TO WS-NAME-TEXT
        COMPUTE WS-AMT = WS-ID * 10.5
        MOVE 'ABC' TO WS-CD
        MOVE 0 TO WS-CD-IND
        IF WS-ID = 2 MOVE -1 TO WS-CD-IND END-IF
        EXEC SQL INSERT INTO ZEUSTEST.PGM1 VALUES
            (:WS-ID, :WS-NAME, :WS-AMT, :WS-CD:WS-CD-IND)
        END-EXEC
    END-PERFORM.
    EXEC SQL INSERT INTO ZEUSTEST.PGM1 (ID) VALUES (1) END-EXEC.
    DISPLAY 'DUP ' SQLCODE ' ' SQLSTATE.
    EXEC SQL SELECT COUNT(*), SUM(AMT) INTO :WS-COUNT, :WS-AMT
        FROM ZEUSTEST.PGM1 END-EXEC.
    MOVE WS-AMT TO WS-OUT.
    DISPLAY 'COUNT ' WS-COUNT ' SUM ' WS-OUT.
    MOVE 2 TO WS-ID.
    EXEC SQL OPEN C1 END-EXEC.
    PERFORM UNTIL SQLCODE NOT = 0
        EXEC SQL FETCH C1 INTO :WS-ID, :WS-NAME, :WS-AMT,
            :WS-CD:WS-CD-IND END-EXEC
        IF SQLCODE = 0
            MOVE WS-AMT TO WS-OUT
            DISPLAY WS-ID ' ' WS-NAME-TEXT(1:WS-NAME-LEN) ' '
                WS-OUT ' ' WS-CD ' ' WS-CD-IND
        END-IF
    END-PERFORM.
    DISPLAY 'END ' SQLCODE.
    EXEC SQL CLOSE C1 END-EXEC.
    EXEC SQL SELECT ID INTO :WS-ID FROM ZEUSTEST.PGM1
        WHERE ID = 99 END-EXEC.
    DISPLAY 'NOTFOUND ' SQLCODE.
    EXEC SQL SET :WS-NEXT = 41 + 1 END-EXEC.
    DISPLAY 'SET ' WS-NEXT.
    STOP RUN.",
    );
    // DISPLAY of signed binary items shows the sign in the last digit, as
    // on z/OS ({ = +0, L = -3, J = -1, ...)
    assert_eq!(
        out,
        "CREATE 00000000{\n\
         DUP 00000080L 23505\n\
         COUNT 00000000C SUM       63.00\n\
         00000000B O'HAR       21.00 ABC 000J\n\
         00000000C O'HAR       31.50 ABC 000{\n\
         END 00000010{\n\
         NOTFOUND 00000010{\n\
         SET 00000004B\n"
    );
}

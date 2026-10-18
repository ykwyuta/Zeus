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
fn whenever_is_lexical_and_updates_sqlca_before_branching() {
    // An unopened cursor reports -501 without requiring a database server.
    let host = Rc::new(Db2Host::new(None, None));
    let out = run(
        &host,
        "ID DIVISION.
PROGRAM-ID. WHENTEST.
DATA DIVISION.
WORKING-STORAGE SECTION.
EXEC SQL INCLUDE SQLCA END-EXEC.
01 V PIC S9(9) COMP.
EXEC SQL WHENEVER SQLERROR GOTO BAD END-EXEC.
PROCEDURE DIVISION.
    GO TO CHECK-SQL.
UNREACHED.
    EXEC SQL WHENEVER SQLERROR GO TO :HANDLE-ERROR END-EXEC.
CHECK-SQL.
    EXEC SQL FETCH MISSING INTO :V END-EXEC.
    DISPLAY 'FAILED TO BRANCH'.
    STOP RUN.
HANDLE-ERROR.
    IF SQLCODE = -501 DISPLAY 'ERROR' END-IF.
    EXEC SQL WHENEVER SQLERROR CONTINUE END-EXEC.
    EXEC SQL FETCH MISSING INTO :V END-EXEC.
    DISPLAY 'CONTINUE'.
    STOP RUN.
BAD.
    DISPLAY 'WRONG SCOPE'.
    STOP RUN.",
    );
    assert_eq!(out, "ERROR\nCONTINUE\n");
}

#[test]
fn whenever_in_declaratives_precedes_the_main_body() {
    let host = Rc::new(Db2Host::new(None, None));
    let out = run(
        &host,
        "ID DIVISION.
PROGRAM-ID. DECLWHEN.
DATA DIVISION.
WORKING-STORAGE SECTION.
EXEC SQL INCLUDE SQLCA END-EXEC.
01 V PIC S9(9) COMP.
PROCEDURE DIVISION.
DECLARATIVES.
IO-HANDLER SECTION.
    USE AFTER STANDARD ERROR PROCEDURE ON INPUT.
    EXEC SQL WHENEVER SQLERROR GO TO HANDLE-SQL END-EXEC.
END DECLARATIVES.
MAIN SECTION.
    EXEC SQL FETCH MISSING INTO :V END-EXEC.
    DISPLAY 'WRONG SCOPE'.
    STOP RUN.
HANDLE-SQL.
    DISPLAY 'DECLARATIVE SCOPE'.
    STOP RUN.",
    );
    assert_eq!(out, "DECLARATIVE SCOPE\n");
}

#[test]
fn whenever_database_warning_and_not_found() {
    let Some(host) = Db2Host::from_env() else {
        return;
    };
    let out = run(
        &Rc::new(host),
        "ID DIVISION.
PROGRAM-ID. DBWHEN.
DATA DIVISION.
WORKING-STORAGE SECTION.
EXEC SQL INCLUDE SQLCA END-EXEC.
01 V PIC X(2).
01 IND PIC S9(4) COMP.
PROCEDURE DIVISION.
    EXEC SQL WHENEVER SQLWARNING GO TO WARN-HANDLER END-EXEC.
    EXEC SQL VALUES ('ABCDE') INTO :V:IND END-EXEC.
    DISPLAY 'NO WARNING'.
    STOP RUN.
WARN-HANDLER.
    IF SQLCODE = 0 AND SQLWARN1 = 'W' AND IND = 5
        AND V = 'AB' DISPLAY 'TRUNCATED' END-IF.
    EXEC SQL WHENEVER NOT FOUND GOTO NO-ROWS END-EXEC.
    EXEC SQL SELECT IBMREQD INTO :V FROM SYSIBM.SYSDUMMY1
        WHERE 1 = 0 END-EXEC.
    DISPLAY 'NO NOTFOUND'.
    STOP RUN.
NO-ROWS.
    IF SQLCODE = 100 DISPLAY 'NOTFOUND' END-IF.
    STOP RUN.",
    );
    assert_eq!(out, "TRUNCATED\nNOTFOUND\n");
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
    // Positive binary values use ordinary digits; negative values retain
    // the trailing sign overpunch. COMP-5 uses the full storage width.
    assert_eq!(
        out,
        "CREATE 0000000000\n\
         DUP 000000080L 23505\n\
         COUNT 000000003 SUM       63.00\n\
         000000002 O'HAR       21.00 ABC 000J\n\
         000000003 O'HAR       31.50 ABC 0000\n\
         END 0000000100\n\
         NOTFOUND 0000000100\n\
         SET 000000042\n"
    );
}

/// Dynamic SQL: EXECUTE IMMEDIATE, PREPARE / EXECUTE USING, a cursor for
/// a prepared query (OPEN USING), DESCRIBE into the SQLDA and FETCH USING
/// DESCRIPTOR, COMP-2 host variables.
#[test]
fn dynamic_sql() {
    let Some(host) = Db2Host::from_env() else {
        eprintln!("ZEUS_DB2 not set; skipped");
        return;
    };
    let host = Rc::new(host);
    let out = run(
        &host,
        "IDENTIFICATION DIVISION.
PROGRAM-ID. SQLDYN.
DATA DIVISION.
WORKING-STORAGE SECTION.
    EXEC SQL INCLUDE SQLCA END-EXEC.
    EXEC SQL INCLUDE SQLDA END-EXEC.
01 WS-STMT.
   49 WS-STMT-LEN  PIC S9(4) COMP.
   49 WS-STMT-TEXT PIC X(100).
01 WS-ID       PIC S9(9) COMP.
01 WS-LOW      PIC S9(9) COMP.
01 WS-RATE     COMP-2.
01 WS-CD       PIC X(3).
01 WS-IND      PIC S9(4) COMP OCCURS 3.
01 WS-OUT      PIC -(4)9.99.
PROCEDURE DIVISION.
    EXEC SQL DROP TABLE ZEUSTEST.DYN1 END-EXEC.
    STRING 'CREATE TABLE ZEUSTEST.DYN1 (ID INTEGER NOT NULL,'
        ' RATE FLOAT, CD CHAR(3))' DELIMITED BY SIZE
        INTO WS-STMT-TEXT.
    MOVE 73 TO WS-STMT-LEN.
    EXEC SQL EXECUTE IMMEDIATE :WS-STMT END-EXEC.
    DISPLAY 'CREATE ' SQLCODE.
    MOVE 'INSERT INTO ZEUSTEST.DYN1 VALUES (?, ?, ?)'
        TO WS-STMT-TEXT.
    MOVE 42 TO WS-STMT-LEN.
    EXEC SQL PREPARE S1 FROM :WS-STMT END-EXEC.
    PERFORM VARYING WS-ID FROM 1 BY 1 UNTIL WS-ID > 3
        COMPUTE WS-RATE = WS-ID * 2.25
        MOVE 'XYZ' TO WS-CD
        EXEC SQL EXECUTE S1 USING :WS-ID, :WS-RATE, :WS-CD
        END-EXEC
    END-PERFORM.
    DISPLAY 'INSERT ' SQLCODE.
    MOVE 'SELECT ID, RATE, CD FROM ZEUSTEST.DYN1 WHERE ID > ?'
        TO WS-STMT-TEXT.
    MOVE 51 TO WS-STMT-LEN.
    EXEC SQL PREPARE S2 FROM :WS-STMT END-EXEC.
    EXEC SQL DECLARE C2 CURSOR FOR S2 END-EXEC.
    MOVE 3 TO SQLN.
    EXEC SQL DESCRIBE S2 INTO :SQLDA END-EXEC.
    DISPLAY 'DESCRIBE ' SQLCODE ' ' SQLD.
    SET SQLDATA(1) TO ADDRESS OF WS-ID.
    SET SQLDATA(2) TO ADDRESS OF WS-RATE.
    SET SQLDATA(3) TO ADDRESS OF WS-CD.
    SET SQLIND(1) TO ADDRESS OF WS-IND(1).
    SET SQLIND(2) TO ADDRESS OF WS-IND(2).
    SET SQLIND(3) TO ADDRESS OF WS-IND(3).
    MOVE 1 TO WS-LOW.
    EXEC SQL OPEN C2 USING :WS-LOW END-EXEC.
    PERFORM UNTIL SQLCODE NOT = 0
        EXEC SQL FETCH C2 USING DESCRIPTOR :SQLDA END-EXEC
        IF SQLCODE = 0
            MOVE WS-RATE TO WS-OUT
            DISPLAY WS-ID ' ' WS-OUT ' ' WS-CD
        END-IF
    END-PERFORM.
    EXEC SQL CLOSE C2 END-EXEC.
    EXEC SQL EXECUTE S3 END-EXEC.
    DISPLAY 'S3 ' SQLCODE.
    STOP RUN.",
    );
    let mut lines: Vec<&str> = out.lines().collect();
    lines[3..5].sort();
    assert_eq!(
        lines,
        [
            "CREATE 0000000000",
            "INSERT 0000000000",
            "DESCRIBE 0000000000 00003",
            "000000002     4.50 XYZ",
            "000000003     6.75 XYZ",
            "S3 000000051M",
        ]
    );
}

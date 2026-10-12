//! Jobs through the JCL engine: a COBOL program compiled (IGYCRCTL),
//! bound (IEWL) and run in one job; IF/THEN/ELSE/ENDIF on step return
//! codes and abends; INCLUDE groups from a JCLLIB library.

use zeus_ds::catalog::{Catalog, Entry, EntryKind, Recfm};
use zeus_jcl::job::{Env, Outcome, StepResult, run_job};

fn env(dir: &std::path::Path) -> Env {
    let mut catalog = Catalog::open(&dir.join("catalog")).unwrap();
    for (name, recfm, lrecl) in [
        ("ZEUS.LOADLIB", Recfm::U, 0),
        ("ZEUS.PROCLIB", Recfm::Fb, 80),
    ] {
        catalog
            .define(Entry {
                name: name.into(),
                kind: EntryKind::Partitioned { recfm, lrecl },
            })
            .unwrap();
        std::fs::create_dir_all(catalog.path(name)).unwrap();
    }
    std::fs::write(
        catalog.path("ZEUS.PROCLIB").join("CHECKS"),
        "//OK      EXEC PGM=IEFBR14\n//OK2     EXEC PGM=IEFBR14\n",
    )
    .unwrap();
    Env {
        catalog,
        spool: dir.join("spool"),
        proclibs: Vec::new(),
        loadlibs: Vec::new(),
    }
}

fn outcomes(r: &[StepResult]) -> Vec<(String, Outcome)> {
    r.iter()
        .map(|s| {
            let name = match &s.proc_step {
                Some(p) => format!("{}.{p}", s.step),
                None => s.step.clone(),
            };
            (name, s.outcome.clone())
        })
        .collect()
}

const JOB: &str = "\
//COMPGO   JOB (ACCT),'COMPILE AND GO'
//         JCLLIB ORDER=ZEUS.PROCLIB
//COMP     EXEC PGM=IGYCRCTL
//SYSPRINT DD SYSOUT=*
//SYSLIN   DD DSN=&&LOADSET,DISP=(MOD,PASS)
//SYSIN    DD *
       IDENTIFICATION DIVISION.
       PROGRAM-ID. HELLO.
       PROCEDURE DIVISION.
           DISPLAY 'HELLO FROM A COMPILED STEP'
           MOVE 4 TO RETURN-CODE
           GOBACK.
/*
//LKED     EXEC PGM=IEWL
//SYSPRINT DD SYSOUT=*
//SYSLIN   DD DSN=&&LOADSET,DISP=(OLD,DELETE)
//SYSLMOD  DD DSN=ZEUS.LOADLIB(HELLO),DISP=SHR
//BOUND    IF (LKED.RC = 0) THEN
//GO       EXEC PGM=HELLO
//STEPLIB  DD DSN=ZEUS.LOADLIB,DISP=SHR
//SYSOUT   DD SYSOUT=*
//         ENDIF
//CHK      IF (GO.RC = 4 &
//             RC <= 4) THEN
//         INCLUDE MEMBER=CHECKS
//         ELSE
//BAD      EXEC PGM=IEFBR14
//         ENDIF
//NOGO     IF ABEND | GO.RC > 4 THEN
//NOPE     EXEC PGM=IEFBR14
//         ENDIF
";

#[test]
fn compile_bind_and_conditions() {
    let dir = tempfile::tempdir().unwrap();
    let mut env = env(dir.path());
    let mut log = Vec::new();
    let r = run_job(&mut env, JOB, &mut log).unwrap();
    let log = String::from_utf8_lossy(&log);
    assert_eq!(
        outcomes(&r),
        [
            ("COMP".into(), Outcome::Rc(0)),
            ("LKED".into(), Outcome::Rc(0)),
            ("GO".into(), Outcome::Rc(4)),
            ("OK".into(), Outcome::Rc(0)),
            ("OK2".into(), Outcome::Rc(0)),
            ("BAD".into(), Outcome::Bypassed),
            ("NOPE".into(), Outcome::Bypassed),
        ],
        "{log}"
    );
    let sysout = std::fs::read_dir(dir.path().join("spool/COMPGO"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("GO.SYSOUT"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect::<String>();
    assert!(sysout.contains("HELLO FROM A COMPILED STEP"), "{sysout}");
}

const ABEND: &str = "\
//ABENDS   JOB
//S1       EXEC PGM=IEFBR14
//S2       EXEC PGM=NOSUCH
//S3       EXEC PGM=IEFBR14
//RECOVER  IF S2.ABENDCC = S806 THEN
//FIX      EXEC PGM=IEFBR14
//         ELSE
//NOFIX    EXEC PGM=IEFBR14
//         ENDIF
//OKCHK    IF (RC = 0 & ¬ABEND) THEN
//S4       EXEC PGM=IEFBR14
//         ENDIF
//BADCOMP  EXEC PGM=IGYCRCTL
//SYSIN    DD *
       IDENTIFICATION DIVISION.
       PROGRAM-ID. BROKEN.
       PROCEDURE DIVISION.
           MOVE.
/*
";

#[test]
fn abends_and_compile_errors() {
    let dir = tempfile::tempdir().unwrap();
    let mut env = env(dir.path());
    let mut log = Vec::new();
    let r = run_job(&mut env, ABEND, &mut log).unwrap();
    let log = String::from_utf8_lossy(&log);
    assert_eq!(
        outcomes(&r),
        [
            ("S1".into(), Outcome::Rc(0)),
            ("S2".into(), Outcome::Abend("S806".into())),
            ("S3".into(), Outcome::Bypassed),
            ("FIX".into(), Outcome::Rc(0)),
            ("NOFIX".into(), Outcome::Bypassed),
            ("S4".into(), Outcome::Bypassed),
            ("BADCOMP".into(), Outcome::Bypassed),
        ],
        "{log}"
    );
    // without the abend, the compile error gives RC 12
    let r = run_job(
        &mut env,
        &ABEND.replace("PGM=NOSUCH", "PGM=IEFBR14"),
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(r.last().unwrap().outcome, Outcome::Rc(12));
    assert_eq!(r[4].outcome, Outcome::Rc(0), "NOFIX runs");
}

const PLI_JOB: &str = "\
//PLIJOB   JOB (ACCT),'PL/I'
//DEFINE   EXEC PGM=IDCAMS
//SYSPRINT DD SYSOUT=*
//SYSIN    DD *
  DEFINE CLUSTER (NAME(ZEUS.ACCOUNTS) INDEXED -
         KEYS(5 0) RECORDSIZE(40 40))
/*
//COMP     EXEC PGM=IBMZPLI
//SYSPRINT DD SYSOUT=*
//SYSLIN   DD DSN=&&OBJ,DISP=(MOD,PASS)
//SYSIN    DD *
 ACCTS: PROC(PARM) OPTIONS(MAIN);
   DCL PARM CHAR(100) VAR;
   DCL TRANS FILE RECORD INPUT;
   DCL ACCTS FILE RECORD KEYED UPDATE;
   DCL 1 TRAN, 2 ID CHAR(5), 2 AMT PIC 'S9999V99', 2 F CHAR(69);
   DCL 1 ACCT, 2 ID CHAR(5), 2 BAL FIXED DEC(9,2), 2 NAME CHAR(30);
   DCL EOF BIT(1) INIT('0'B);
   DCL (NEW, UPD) FIXED BIN(31) INIT(0);
   ON ENDFILE(TRANS) EOF = '1'B;
   PUT SKIP LIST('RUN', PARM);
   READ FILE(TRANS) INTO(TRAN);
   DO WHILE (¬EOF);
     ON KEY(ACCTS) BEGIN;
       ACCT.ID = TRAN.ID;
       ACCT.BAL = AMT;
       ACCT.NAME = 'NEW ACCOUNT';
       WRITE FILE(ACCTS) FROM(ACCT) KEYFROM(TRAN.ID);
       NEW = NEW + 1;
       GO TO NEXT;
     END;
     READ FILE(ACCTS) INTO(ACCT) KEY(TRAN.ID);
     ACCT.BAL = ACCT.BAL + AMT;
     REWRITE FILE(ACCTS) FROM(ACCT) KEY(TRAN.ID);
     UPD = UPD + 1;
 NEXT:
     READ FILE(TRANS) INTO(TRAN);
   END;
   CLOSE FILE(ACCTS);
   PUT SKIP EDIT('NEW', NEW, 'UPDATED', UPD)(A, F(3), X(1), A, F(3));
   OPEN FILE(ACCTS) INPUT SEQUENTIAL;
   EOF = '0'B;
   ON ENDFILE(ACCTS) EOF = '1'B;
   READ FILE(ACCTS) INTO(ACCT);
   DO WHILE (¬EOF);
     PUT SKIP EDIT(ACCT.ID, ACCT.BAL)(A, F(12,2));
     READ FILE(ACCTS) INTO(ACCT);
   END;
   IF UPD > 0 THEN CALL PLIRETC(4);
 END ACCTS;
/*
//LKED     EXEC PGM=IEWL
//SYSLIN   DD DSN=&&OBJ,DISP=(OLD,DELETE)
//SYSLMOD  DD DSN=ZEUS.LOADLIB(ACCTS),DISP=SHR
//GO       EXEC PGM=ACCTS,PARM='/NIGHTLY'
//STEPLIB  DD DSN=ZEUS.LOADLIB,DISP=SHR
//SYSPRINT DD SYSOUT=*
//ACCTS    DD DSN=ZEUS.ACCOUNTS,DISP=SHR
//TRANS    DD *
00001+001000
00002+000550
00001-000025
/*
";

#[test]
fn pli_compile_and_run() {
    let dir = tempfile::tempdir().unwrap();
    let mut env = env(dir.path());
    let mut log = Vec::new();
    let r = run_job(&mut env, PLI_JOB, &mut log).unwrap();
    let log = String::from_utf8_lossy(&log);
    let spool = std::fs::read_dir(dir.path().join("spool/PLIJOB"))
        .unwrap()
        .flatten()
        .map(|e| {
            (
                e.file_name().to_string_lossy().into_owned(),
                std::fs::read_to_string(e.path()).unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    let sysprint = |step: &str| {
        spool
            .iter()
            .filter(|(n, _)| n.contains(&format!("{step}.SYSPRINT")))
            .map(|(_, t)| t.clone())
            .collect::<String>()
    };
    assert_eq!(
        outcomes(&r),
        [
            ("DEFINE".into(), Outcome::Rc(0)),
            ("COMP".into(), Outcome::Rc(0)),
            ("LKED".into(), Outcome::Rc(0)),
            ("GO".into(), Outcome::Rc(4)),
        ],
        "{log}\n{}",
        sysprint("COMP")
    );
    let out = sysprint("GO");
    let lines: Vec<&str> = out.lines().map(str::trim_end).collect();
    assert_eq!(
        lines,
        [
            "RUN                     NIGHTLY",
            "NEW  2 UPDATED  1",
            "00001        9.75",
            "00002        5.50",
        ],
        "{out}"
    );
}

/// Compiles `src` with `compiler` and binds it as `name`.
fn compile_step(name: &str, compiler: &str, src: &str) -> String {
    format!(
        "//C{name:<7} EXEC PGM={compiler}\n//SYSPRINT DD SYSOUT=*\n//SYSLIN   DD DSN=&&O{name},DISP=(MOD,PASS)\n//SYSIN    DD *\n{src}/*\n//L{name:<7} EXEC PGM=IEWL\n//SYSLIN   DD DSN=&&O{name},DISP=(OLD,DELETE)\n//SYSLMOD  DD DSN=ZEUS.LOADLIB({name}),DISP=SHR\n"
    )
}

const CSUB: &str = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. CSUB.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 CNT PIC 9(3) VALUE 0.
       LINKAGE SECTION.
       01 N PIC S9(9) COMP.
       01 T PIC X(10).
       PROCEDURE DIVISION USING N T.
           ADD 1 TO CNT.
           ADD 100 TO N.
           DISPLAY 'CSUB ' CNT ' ' T.
           MOVE 'COBOL' TO T.
           MOVE 3 TO RETURN-CODE.
           GOBACK.
";

const PSUB: &str = " PSUB: PROC(N, T) RETURNS(FIXED BIN(31));
   DCL N FIXED BIN(31), T CHAR(10);
   DCL CSUB ENTRY OPTIONS(COBOL);
   DCL CALLS FIXED BIN(31) STATIC INIT(0);
   CALLS = CALLS + 1;
   PUT SKIP LIST('PSUB', N, T);
   N = N * 3;
   CALL CSUB(N, T);
   CALL PLIRETC(2);
   RETURN(CALLS);
 END PSUB;
";

const CMAIN: &str = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. CMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 N PIC S9(9) COMP VALUE 7.
       01 T PIC X(10) VALUE 'HELLO'.
       PROCEDURE DIVISION.
           CALL 'PSUB' USING N T.
           DISPLAY 'BACK ' N ' ' T ' RC ' RETURN-CODE.
           GOBACK.
";

const PMAIN: &str = " PMAIN: PROC OPTIONS(MAIN);
   DCL PSUB ENTRY(FIXED BIN(31), CHAR(10)) RETURNS(FIXED BIN(31));
   DCL CSUB ENTRY OPTIONS(COBOL);
   DCL N FIXED BIN(31) INIT(1);
   DCL T CHAR(10) INIT('PLI');
   DCL K FIXED BIN(31);
   K = PSUB(N, T);
   K = PSUB(N, T);
   PUT SKIP LIST(N, T, K);
   CALL CSUB(N, T);
   PUT SKIP LIST(N, T, PLIRETV());
   CALL PLIRETC(0);
 END PMAIN;
";

#[test]
fn calls_between_languages() {
    let dir = tempfile::tempdir().unwrap();
    let mut env = env(dir.path());
    let mut job = String::from("//MIXJOB   JOB (ACCT),'MIXED'\n");
    for (n, c, s) in [
        ("CSUB", "IGYCRCTL", CSUB),
        ("PSUB", "IBMZPLI", PSUB),
        ("CMAIN", "IGYCRCTL", CMAIN),
        ("PMAIN", "IBMZPLI", PMAIN),
    ] {
        job.push_str(&compile_step(n, c, s));
    }
    for p in ["CMAIN", "PMAIN"] {
        job.push_str(&format!(
            "//R{p:<7} EXEC PGM={p}\n//STEPLIB  DD DSN=ZEUS.LOADLIB,DISP=SHR\n//SYSPRINT DD SYSOUT=*\n//SYSOUT   DD SYSOUT=*\n"
        ));
    }
    let mut log = Vec::new();
    let r = run_job(&mut env, &job, &mut log).unwrap();
    let log = String::from_utf8_lossy(&log);
    let spool = |step: &str, dd: &str| {
        std::fs::read_dir(dir.path().join("spool/MIXJOB"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(&format!("{step}.{dd}")))
            .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
            .collect::<String>()
    };
    let rcs: Vec<(String, Outcome)> = outcomes(&r).into_iter().filter(|(n, _)| n.starts_with('R')).collect();
    assert_eq!(
        rcs,
        [("RCMAIN".into(), Outcome::Rc(2)), ("RPMAIN".into(), Outcome::Rc(0))],
        "{log}"
    );
    let lines = |s: String| s.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect::<Vec<_>>();
    assert_eq!(lines(spool("RCMAIN", "SYSPRINT")), ["PSUB 7 HELLO"]);
    assert_eq!(
        lines(spool("RCMAIN", "SYSOUT")),
        ["CSUB 001 HELLO", "BACK 00000012A COBOL RC 000B"]
    );
    assert_eq!(
        lines(spool("RPMAIN", "SYSPRINT")),
        ["PSUB 1 PLI", "PSUB 103 COBOL", "409 COBOL 2", "509 COBOL 3"]
    );
    assert_eq!(lines(spool("RPMAIN", "SYSOUT")), ["CSUB 001 PLI", "CSUB 002 COBOL", "CSUB 003 COBOL"]);
}

const MQPROG: &str = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MQPROG.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 QMGR            PIC X(48) VALUE SPACES.
       01 HCONN           PIC S9(9) BINARY VALUE 0.
       01 HOBJ            PIC S9(9) BINARY VALUE 0.
       01 OPTIONS         PIC S9(9) BINARY.
       01 CC              PIC S9(9) BINARY.
       01 RC              PIC S9(9) BINARY.
       01 BUFLEN          PIC S9(9) BINARY.
       01 DATALEN         PIC S9(9) BINARY.
       01 BUFFER          PIC X(100).
       01 I               PIC 9.
       01 N               PIC 9 VALUE 0.
       01 OD.
       COPY CMQODV.
       01 MD.
       COPY CMQMDV.
       01 PMO.
       COPY CMQPMOV.
       01 GMO.
       COPY CMQGMOV.
       01 MQC.
       COPY CMQV.
       PROCEDURE DIVISION.
           CALL 'MQCONN' USING QMGR HCONN CC RC
           DISPLAY 'CONN ' RC
           MOVE 'DEV.QUEUE.3' TO MQOD-OBJECTNAME
           COMPUTE OPTIONS = MQOO-OUTPUT + MQOO-INPUT-SHARED
           CALL 'MQOPEN' USING HCONN OD OPTIONS HOBJ CC RC
           DISPLAY 'OPEN ' RC
      *    EMPTY THE QUEUE
           PERFORM GET-ONE UNTIL RC NOT = 0
           MOVE 0 TO N
      *    A UNIT OF WORK BACKED OUT, THEN ONE COMMITTED
           PERFORM PUT-ONE VARYING I FROM 1 BY 1 UNTIL I > 3
           CALL 'MQBACK' USING HCONN CC RC
           PERFORM PUT-ONE VARYING I FROM 4 BY 1 UNTIL I > 5
           CALL 'MQCMIT' USING HCONN CC RC
           PERFORM GET-ONE UNTIL RC NOT = 0
           DISPLAY 'LAST ' RC ' COUNT ' N
           CALL 'MQCLOSE' USING HCONN HOBJ MQCO-NONE CC RC
           CALL 'MQDISC' USING HCONN CC RC
           DISPLAY 'DISC ' RC
           GOBACK.
       PUT-ONE.
           MOVE MQMI-NONE TO MQMD-MSGID
           MOVE MQFMT-STRING TO MQMD-FORMAT
           MOVE SPACES TO BUFFER
           STRING 'MESSAGE ' I DELIMITED BY SIZE INTO BUFFER
           MOVE 9 TO BUFLEN
           MOVE MQPMO-SYNCPOINT TO MQPMO-OPTIONS
           CALL 'MQPUT' USING HCONN HOBJ MD PMO BUFLEN BUFFER CC RC.
       GET-ONE.
           MOVE MQMI-NONE TO MQMD-MSGID
           MOVE MQCI-NONE TO MQMD-CORRELID
           MOVE MQCCSI-Q-MGR TO MQMD-CODEDCHARSETID
           MOVE MQENC-NATIVE TO MQMD-ENCODING
           COMPUTE MQGMO-OPTIONS = MQGMO-NO-WAIT + MQGMO-CONVERT
           MOVE 100 TO BUFLEN
           CALL 'MQGET' USING HCONN HOBJ MD GMO BUFLEN BUFFER
                              DATALEN CC RC
           IF RC = 0
               ADD 1 TO N
               DISPLAY 'GOT ' BUFFER(1:DATALEN) ' ' MQMD-FORMAT
           END-IF.
";

/// MQI calls on the queue manager of ZEUS_MQ through the IBM MQ client
/// library (skipped without ZEUS_MQ). The queue DEV.QUEUE.3 is emptied
/// first.
#[test]
fn mq_calls() {
    if std::env::var_os("ZEUS_MQ").is_none() {
        eprintln!("ZEUS_MQ is not set: MQ test skipped");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut env = env(dir.path());
    let mut job = String::from("//MQJOB    JOB (ACCT),'MQ'\n");
    job.push_str(&compile_step("MQPROG", "IGYCRCTL", MQPROG));
    job.push_str("//RUN      EXEC PGM=MQPROG\n//STEPLIB  DD DSN=ZEUS.LOADLIB,DISP=SHR\n//SYSOUT   DD SYSOUT=*\n");
    let mut log = Vec::new();
    let r = run_job(&mut env, &job, &mut log).unwrap();
    let log = String::from_utf8_lossy(&log);
    let out: String = std::fs::read_dir(dir.path().join("spool/MQJOB"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("RUN.SYSOUT"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
        .collect();
    assert_eq!(outcomes(&r).last(), Some(&("RUN".into(), Outcome::Rc(0))), "{log}\n{out}");
    let lines: Vec<&str> = out.lines().map(str::trim_end).collect();
    assert_eq!(
        lines,
        [
            "CONN 00000000{",
            "OPEN 00000000{",
            "GOT MESSAGE 4 MQSTR",
            "GOT MESSAGE 5 MQSTR",
            "LAST 00000203C COUNT 2",
            "DISC 00000000{",
        ],
        "{out}"
    );
}

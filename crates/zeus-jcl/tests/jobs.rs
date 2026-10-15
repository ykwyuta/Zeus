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

#[test]
fn concurrent_compiler_steps_keep_sources_separate() {
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    std::thread::scope(|scope| {
        for worker in 0..8 {
            let barrier = barrier.clone();
            scope.spawn(move || {
                let invalid = worker % 2 != 0;
                let job = if invalid {
                    JOB.replace("MOVE 4 TO RETURN-CODE", "THIS IS NOT COBOL")
                } else {
                    JOB.to_string()
                };
                barrier.wait();
                for _ in 0..8 {
                    let dir = tempfile::tempdir().unwrap();
                    let mut env = env(dir.path());
                    let mut log = Vec::new();
                    let results = run_job(&mut env, &job, &mut log).unwrap();
                    let expected = if invalid { 12 } else { 0 };
                    assert_eq!(
                        results[0].outcome,
                        Outcome::Rc(expected),
                        "worker {worker}: {}",
                        String::from_utf8_lossy(&log)
                    );
                    if !invalid {
                        assert_eq!(results[2].outcome, Outcome::Rc(4));
                    }
                }
            });
        }
    });
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
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .contains(&format!("{step}.{dd}"))
            })
            .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
            .collect::<String>()
    };
    let rcs: Vec<(String, Outcome)> = outcomes(&r)
        .into_iter()
        .filter(|(n, _)| n.starts_with('R'))
        .collect();
    assert_eq!(
        rcs,
        [
            ("RCMAIN".into(), Outcome::Rc(2)),
            ("RPMAIN".into(), Outcome::Rc(0))
        ],
        "{log}"
    );
    let lines = |s: String| {
        s.lines()
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
    };
    assert_eq!(lines(spool("RCMAIN", "SYSPRINT")), ["PSUB 7 HELLO"]);
    assert_eq!(
        lines(spool("RCMAIN", "SYSOUT")),
        ["CSUB 001 HELLO", "BACK 00000012A COBOL RC 000B"]
    );
    assert_eq!(
        lines(spool("RPMAIN", "SYSPRINT")),
        ["PSUB 1 PLI", "PSUB 103 COBOL", "409 COBOL 2", "509 COBOL 3"]
    );
    assert_eq!(
        lines(spool("RPMAIN", "SYSOUT")),
        ["CSUB 001 PLI", "CSUB 002 COBOL", "CSUB 003 COBOL"]
    );
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
    assert_eq!(
        outcomes(&r).last(),
        Some(&("RUN".into(), Outcome::Rc(0))),
        "{log}\n{out}"
    );
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

/// The MQI from PL/I: the include members CMQP, CMQEPP and the
/// structures, on DEV.QUEUE.1.
const MQPLI: &str = " MQPLI: PROC OPTIONS(MAIN);
   %INCLUDE CMQP;
   %INCLUDE CMQEPP;
   %INCLUDE CMQODP;
   %INCLUDE CMQMDP;
   %INCLUDE CMQPMOP;
   %INCLUDE CMQGMOP;
   %INCLUDE CMQCBDP;
   %INCLUDE CMQCTLOP;
   DCL QMGR CHAR(48) INIT('');
   DCL (HCONN, HOBJ, OPTS, CC, RC, BUFLEN, DATALEN) FIXED BIN(31);
   DCL CLOSEOPTS FIXED BIN(31) INIT(MQCO_NONE);
   DCL BUFFER CHAR(100);
   DCL (I, N) FIXED BIN(31) INIT(0);
   DCL SELS(1) FIXED BIN(31) INIT(MQIA_CURRENT_Q_DEPTH);
   DCL INTS(1) FIXED BIN(31);
   DCL (ONE, ZERO) FIXED BIN(31);
   DCL CHARS CHAR(1);

   HCONN = 0;
   CALL MQCONN(QMGR, HCONN, CC, RC);
   PUT SKIP LIST('CONN', RC);
   MQOD.OBJECTNAME = 'DEV.QUEUE.1';
   OPTS = MQOO_OUTPUT + MQOO_INPUT_SHARED + MQOO_INQUIRE;
   CALL MQOPEN(HCONN, MQOD, OPTS, HOBJ, CC, RC);
   PUT SKIP LIST('OPEN', RC);
   /* empty the queue */
   CALL GETONE;
   DO WHILE (RC = 0);
     CALL GETONE;
   END;
   N = 0;
   DO I = 1 TO 3;
     MQMD.MSGID = MQMI_NONE;
     MQMD.FORMAT = MQFMT_STRING;
     BUFFER = 'PLI MESSAGE ' || CHAR(I);
     BUFLEN = LENGTH(TRIM(BUFFER));
     MQPMO.OPTIONS = MQPMO_SYNCPOINT;
     CALL MQPUT(HCONN, HOBJ, MQMD, MQPMO, BUFLEN, BUFFER, CC, RC);
   END;
   CALL MQCMIT(HCONN, CC, RC);
   ONE = 1;
   ZERO = 0;
   CALL MQINQ(HCONN, HOBJ, ONE, SELS, ONE, INTS, ZERO, CHARS, CC, RC);
   PUT SKIP LIST('DEPTH', INTS(1), RC);
   CALL GETONE;
   DO WHILE (RC = 0);
     PUT SKIP LIST(SUBSTR(BUFFER, 1, DATALEN), MQMD.FORMAT);
     N = N + 1;
     CALL GETONE;
   END;
   PUT SKIP LIST('LAST', RC, 'COUNT', N);
   /* a message consumer: the PL/I procedure MQPLICB */
   DO I = 1 TO 2;
     MQMD.MSGID = MQMI_NONE;
     MQMD.FORMAT = MQFMT_STRING;
     BUFFER = 'ASYNC ' || TRIM(CHAR(I));
     BUFLEN = LENGTH(TRIM(BUFFER));
     MQPMO.OPTIONS = MQPMO_NO_SYNCPOINT;
     CALL MQPUT(HCONN, HOBJ, MQMD, MQPMO, BUFLEN, BUFFER, CC, RC);
   END;
   MQCBD.CALLBACKNAME = 'MQPLICB';
   MQCBD.OPTIONS = MQCBDO_STOP_CALL;
   MQMD.MSGID = MQMI_NONE;
   MQMD.CORRELID = MQCI_NONE;
   MQGMO.OPTIONS = MQGMO_WAIT + MQGMO_CONVERT;
   MQGMO.WAITINTERVAL = 3000;
   OPTS = MQOP_REGISTER;
   CALL MQCB(HCONN, OPTS, MQCBD, HOBJ, MQMD, MQGMO, CC, RC);
   PUT SKIP LIST('CB', RC);
   OPTS = MQOP_START_WAIT;
   CALL MQCTL(HCONN, OPTS, MQCTLO, CC, RC);
   PUT SKIP LIST('CTL', RC);
   CALL MQCLOSE(HCONN, HOBJ, CLOSEOPTS, CC, RC);
   CALL MQDISC(HCONN, CC, RC);
   PUT SKIP LIST('DISC', RC);

 GETONE: PROC;
   MQMD.MSGID = MQMI_NONE;
   MQMD.CORRELID = MQCI_NONE;
   MQMD.CODEDCHARSETID = MQCCSI_Q_MGR;
   MQMD.ENCODING = MQENC_NATIVE;
   MQGMO.OPTIONS = MQGMO_NO_WAIT + MQGMO_CONVERT;
   BUFLEN = 100;
   BUFFER = '';
   CALL MQGET(HCONN, HOBJ, MQMD, MQGMO, BUFLEN, BUFFER, DATALEN,
              CC, RC);
 END GETONE;
 END MQPLI;
";

const MQPLICB: &str = " MQPLICB: PROC(HCONN, MD, GMO, BUF, CBC);
   %INCLUDE CMQP;
   %INCLUDE CMQEPP;
   %INCLUDE CMQMDP;
   %INCLUDE CMQGMOP;
   %INCLUDE CMQCBCP;
   %INCLUDE CMQCTLOP;
   DCL HCONN FIXED BIN(31);
   DCL 1 MD LIKE MQMD;
   DCL 1 GMO LIKE MQGMO;
   DCL BUF CHAR(100);
   DCL 1 CBC LIKE MQCBC;
   DCL (OP, CC, RC) FIXED BIN(31);
   OP = MQOP_STOP;
   SELECT (CBC.CALLTYPE);
     WHEN (MQCBCT_MSG_REMOVED) DO;
       PUT SKIP LIST('CALLBACK', SUBSTR(BUF, 1, CBC.DATALENGTH),
         MD.FORMAT);
       IF SUBSTR(BUF, 1, 7) = 'ASYNC 2' THEN
         CALL MQCTL(HCONN, OP, MQCTLO, CC, RC);
     END;
     WHEN (MQCBCT_STOP_CALL) PUT SKIP LIST('STOPPED');
     OTHERWISE DO;
       PUT SKIP LIST('EVENT', CBC.REASON);
       CALL MQCTL(HCONN, OP, MQCTLO, CC, RC);
     END;
   END;
 END MQPLICB;
";

/// MQI calls from a PL/I program (skipped without ZEUS_MQ).
#[test]
fn mq_calls_from_pli() {
    if std::env::var_os("ZEUS_MQ").is_none() {
        eprintln!("ZEUS_MQ is not set: MQ test skipped");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut env = env(dir.path());
    let mut job = String::from("//MQPJOB   JOB (ACCT),'MQ'\n");
    job.push_str(&compile_step("MQPLI", "IBMZPLI", MQPLI));
    job.push_str(&compile_step("MQPLICB", "IBMZPLI", MQPLICB));
    job.push_str("//RUN      EXEC PGM=MQPLI\n//STEPLIB  DD DSN=ZEUS.LOADLIB,DISP=SHR\n//SYSPRINT DD SYSOUT=*\n//SYSOUT   DD SYSOUT=*\n");
    let mut log = Vec::new();
    let r = run_job(&mut env, &job, &mut log).unwrap();
    let log = String::from_utf8_lossy(&log);
    let out: String = std::fs::read_dir(dir.path().join("spool/MQPJOB"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("RUN."))
        .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
        .collect();
    assert_eq!(
        outcomes(&r).last(),
        Some(&("RUN".into(), Outcome::Rc(0))),
        "{log}\n{out}"
    );
    let lines: Vec<String> = out
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    assert_eq!(
        lines,
        [
            "CONN 0",
            "OPEN 0",
            "DEPTH 3 0",
            "PLI MESSAGE 1 MQSTR",
            "PLI MESSAGE 2 MQSTR",
            "PLI MESSAGE 3 MQSTR",
            "LAST 2033 COUNT 3",
            // the callee's lines come before the caller's pending line
            "CALLBACK ASYNC 1 MQSTR",
            "CALLBACK ASYNC 2 MQSTR",
            "STOPPED",
            "CB 0",
            "CTL 0",
            "DISC 0",
        ],
        "{out}"
    );
}

const MQPS: &str = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MQPS.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 QMGR            PIC X(48) VALUE SPACES.
       01 HCONN           PIC S9(9) BINARY VALUE 0.
       01 HOBJ            PIC S9(9) BINARY VALUE 0.
       01 HSUB            PIC S9(9) BINARY VALUE 0.
       01 HPUB            PIC S9(9) BINARY VALUE 0.
       01 HQ              PIC S9(9) BINARY VALUE 0.
       01 HMSG1           PIC S9(18) BINARY VALUE 0.
       01 HMSG2           PIC S9(18) BINARY VALUE 0.
       01 OPTIONS         PIC S9(9) BINARY.
       01 CC              PIC S9(9) BINARY.
       01 RC              PIC S9(9) BINARY.
       01 BUFLEN          PIC S9(9) BINARY.
       01 DATALEN         PIC S9(9) BINARY.
       01 BUFFER          PIC X(100).
       01 TOPIC           PIC X(4) VALUE 'dev/'.
       01 TYPE-STRING     PIC S9(9) BINARY.
       01 TYPE-INT32      PIC S9(9) BINARY.
       01 VLEN            PIC S9(9) BINARY.
       01 ONE             PIC S9(9) BINARY VALUE 1.
       01 ZERO-LEN        PIC S9(9) BINARY VALUE 0.
       01 PTYPE           PIC S9(9) BINARY.
       01 PLEN            PIC S9(9) BINARY.
       01 PSTR            PIC X(20).
       01 PINT            PIC S9(9) BINARY.
       01 PNAME1          PIC X(6) VALUE 'Colour'.
       01 PNAME2          PIC X(5) VALUE 'Count'.
       01 PNAME.
          05 PN-VSPTR     POINTER.
          05 PN-VSOFFSET  PIC S9(9) BINARY VALUE 0.
          05 PN-VSBUFSIZE PIC S9(9) BINARY VALUE 0.
          05 PN-VSLENGTH  PIC S9(9) BINARY.
          05 PN-VSCCSID   PIC S9(9) BINARY VALUE -3.
       01 SELS            PIC S9(9) BINARY.
       01 IATTR           PIC S9(9) BINARY.
       01 CATTR           PIC X(1).
       01 I               PIC 9.
       01 OD.
       COPY CMQODV.
       01 MD.
       COPY CMQMDV.
       01 PMO.
       COPY CMQPMOV.
       01 GMO.
       COPY CMQGMOV.
       01 SD.
       COPY CMQSDV.
       01 CMHO.
       COPY CMQCMHOV.
       01 DMHO.
       COPY CMQDMHOV.
       01 SMPO.
       COPY CMQSMPOV.
       01 IMPO.
       COPY CMQIMPOV.
       01 PD.
       COPY CMQPDV.
       01 CBD.
       COPY CMQCBDV.
       01 CTLO.
       COPY CMQCTLOV.
       01 MQC.
       COPY CMQV.
       PROCEDURE DIVISION.
           CALL 'MQCONN' USING QMGR HCONN CC RC
           DISPLAY 'CONN ' RC
      *    A SUBSCRIPTION TO A TOPIC, WITH A MANAGED QUEUE
           SET MQSD-OBJECTSTRING-VSPTR TO ADDRESS OF TOPIC
           MOVE 4 TO MQSD-OBJECTSTRING-VSLENGTH
           COMPUTE MQSD-OPTIONS = MQSO-CREATE + MQSO-NON-DURABLE
                                + MQSO-MANAGED
           CALL 'MQSUB' USING HCONN SD HOBJ HSUB CC RC
           DISPLAY 'SUB ' RC
      *    A PUBLICATION WITH TWO MESSAGE PROPERTIES
           MOVE MQOT-TOPIC TO MQOD-OBJECTTYPE
           MOVE MQOD-VERSION-4 TO MQOD-VERSION
           SET MQOD-OBJECTSTRING-VSPTR TO ADDRESS OF TOPIC
           MOVE 4 TO MQOD-OBJECTSTRING-VSLENGTH
           CALL 'MQOPEN' USING HCONN OD MQOO-OUTPUT HPUB CC RC
           DISPLAY 'OPEN TOPIC ' RC
           CALL 'MQCRTMH' USING HCONN CMHO HMSG1 CC RC
           DISPLAY 'CRTMH ' RC
           SET PN-VSPTR TO ADDRESS OF PNAME1
           MOVE 6 TO PN-VSLENGTH
           MOVE 'Blue' TO PSTR
           MOVE MQTYPE-STRING TO TYPE-STRING
           MOVE MQTYPE-INT32 TO TYPE-INT32
           MOVE 4 TO VLEN
           CALL 'MQSETMP' USING HCONN HMSG1 SMPO PNAME PD
                TYPE-STRING VLEN PSTR CC RC
           DISPLAY 'SETMP ' RC
           SET PN-VSPTR TO ADDRESS OF PNAME2
           MOVE 5 TO PN-VSLENGTH
           MOVE 42 TO PINT
           CALL 'MQSETMP' USING HCONN HMSG1 SMPO PNAME PD
                TYPE-INT32 VLEN PINT CC RC
           MOVE MQPMO-VERSION-3 TO MQPMO-VERSION
           MOVE HMSG1 TO MQPMO-ORIGINALMSGHANDLE
           MOVE MQFMT-STRING TO MQMD-FORMAT
           MOVE 'PUBLISHED' TO BUFFER
           MOVE 9 TO BUFLEN
           CALL 'MQPUT' USING HCONN HPUB MD PMO BUFLEN BUFFER CC RC
           DISPLAY 'PUT ' RC
      *    THE PUBLICATION FROM THE SUBSCRIPTION'S QUEUE
           CALL 'MQCRTMH' USING HCONN CMHO HMSG2 CC RC
           MOVE MQGMO-VERSION-4 TO MQGMO-VERSION
           COMPUTE MQGMO-OPTIONS = MQGMO-WAIT + MQGMO-CONVERT
                                 + MQGMO-PROPERTIES-IN-HANDLE
           MOVE 5000 TO MQGMO-WAITINTERVAL
           MOVE HMSG2 TO MQGMO-MSGHANDLE
           PERFORM RESET-MD
           MOVE 100 TO BUFLEN
           MOVE SPACES TO BUFFER
           CALL 'MQGET' USING HCONN HOBJ MD GMO BUFLEN BUFFER
                              DATALEN CC RC
           DISPLAY 'GOT ' RC ' ' BUFFER(1:DATALEN)
           SET PN-VSPTR TO ADDRESS OF PNAME1
           MOVE 6 TO PN-VSLENGTH
           MOVE SPACES TO PSTR
           MOVE MQTYPE-AS-SET TO PTYPE
           MOVE 20 TO VLEN
           CALL 'MQINQMP' USING HCONN HMSG2 IMPO PNAME PD PTYPE
                VLEN PSTR PLEN CC RC
           DISPLAY 'COLOUR ' RC ' ' PSTR(1:PLEN) ' ' PTYPE
           SET PN-VSPTR TO ADDRESS OF PNAME2
           MOVE 5 TO PN-VSLENGTH
           MOVE 0 TO PINT
           MOVE 4 TO VLEN
           CALL 'MQINQMP' USING HCONN HMSG2 IMPO PNAME PD PTYPE
                VLEN PINT PLEN CC RC
           DISPLAY 'COUNT ' RC ' ' PINT ' ' PLEN
           CALL 'MQDLTMH' USING HCONN HMSG1 DMHO CC RC
           CALL 'MQDLTMH' USING HCONN HMSG2 DMHO CC RC
           DISPLAY 'DLTMH ' RC
           CALL 'MQCLOSE' USING HCONN HPUB MQCO-NONE CC RC
           CALL 'MQCLOSE' USING HCONN HSUB MQCO-NONE CC RC
           DISPLAY 'CLOSE SUB ' RC
           CALL 'MQCLOSE' USING HCONN HOBJ MQCO-NONE CC RC
      *    A MESSAGE CONSUMER CALLED BACK BY MQCTL
           MOVE MQOT-Q TO MQOD-OBJECTTYPE
           MOVE MQOD-VERSION-1 TO MQOD-VERSION
           MOVE 'DEV.QUEUE.2' TO MQOD-OBJECTNAME
           COMPUTE OPTIONS = MQOO-OUTPUT + MQOO-INPUT-SHARED
           CALL 'MQOPEN' USING HCONN OD OPTIONS HQ CC RC
           MOVE MQGMO-VERSION-1 TO MQGMO-VERSION
           MOVE MQGMO-NO-WAIT TO MQGMO-OPTIONS
           PERFORM UNTIL RC NOT = 0
               PERFORM RESET-MD
               MOVE 100 TO BUFLEN
               CALL 'MQGET' USING HCONN HQ MD GMO BUFLEN BUFFER
                                  DATALEN CC RC
           END-PERFORM
           MOVE MQPMO-VERSION-1 TO MQPMO-VERSION
           PERFORM VARYING I FROM 1 BY 1 UNTIL I > 2
               PERFORM RESET-MD
               MOVE MQFMT-STRING TO MQMD-FORMAT
               MOVE SPACES TO BUFFER
               STRING 'ASYNC ' I DELIMITED BY SIZE INTO BUFFER
               MOVE 7 TO BUFLEN
               CALL 'MQPUT' USING HCONN HQ MD PMO BUFLEN BUFFER CC RC
           END-PERFORM
           MOVE 'MQCBPGM' TO MQCBD-CALLBACKNAME
           COMPUTE MQCBD-OPTIONS = MQCBDO-STOP-CALL
           PERFORM RESET-MD
           COMPUTE MQGMO-OPTIONS = MQGMO-WAIT + MQGMO-CONVERT
           MOVE 3000 TO MQGMO-WAITINTERVAL
           CALL 'MQCB' USING HCONN MQOP-REGISTER CBD HQ MD GMO CC RC
           DISPLAY 'CB ' RC
           CALL 'MQCTL' USING HCONN MQOP-START-WAIT CTLO CC RC
           DISPLAY 'CTL ' RC
           CALL 'MQCB' USING HCONN MQOP-DEREGISTER CBD HQ MD GMO CC RC
           DISPLAY 'DEREGISTER ' RC
      *    MQSET OF A QUEUE NOT OPENED FOR SET (2040)
           MOVE MQIA-INHIBIT-PUT TO SELS
           MOVE MQQA-PUT-ALLOWED TO IATTR
           CALL 'MQSET' USING HCONN HQ ONE SELS ONE IATTR ZERO-LEN
                CATTR CC RC
           DISPLAY 'SET ' RC
           CALL 'MQCLOSE' USING HCONN HQ MQCO-NONE CC RC
           CALL 'MQDISC' USING HCONN CC RC
           DISPLAY 'DISC ' RC
           GOBACK.
       RESET-MD.
           MOVE MQMI-NONE TO MQMD-MSGID
           MOVE MQCI-NONE TO MQMD-CORRELID
           MOVE MQCCSI-Q-MGR TO MQMD-CODEDCHARSETID
           MOVE MQENC-NATIVE TO MQMD-ENCODING.
";

const MQCBPGM: &str = "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MQCBPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 N               PIC 9 VALUE 0.
       01 CC              PIC S9(9) BINARY.
       01 RC              PIC S9(9) BINARY.
       01 CTLO.
       COPY CMQCTLOV.
       01 MQC.
       COPY CMQV.
       LINKAGE SECTION.
       01 HCONN           PIC S9(9) BINARY.
       01 MD.
       COPY CMQMDL.
       01 GMO.
       COPY CMQGMOL.
       01 BUFFER          PIC X(100).
       01 CBC.
       COPY CMQCBCL.
       PROCEDURE DIVISION USING HCONN MD GMO BUFFER CBC.
           EVALUATE MQCBC-CALLTYPE
           WHEN MQCBCT-MSG-REMOVED
               ADD 1 TO N
               DISPLAY 'CALLBACK ' BUFFER(1:MQCBC-DATALENGTH) ' '
                       MQMD-FORMAT
               IF N = 2
                   CALL 'MQCTL' USING HCONN MQOP-STOP CTLO CC RC
               END-IF
           WHEN MQCBCT-STOP-CALL
               DISPLAY 'STOPPED AFTER ' N
           WHEN OTHER
               DISPLAY 'EVENT ' MQCBC-REASON
               CALL 'MQCTL' USING HCONN MQOP-STOP CTLO CC RC
           END-EVALUATE
           GOBACK.
";

/// Publish/subscribe, message handles and properties, a message consumer
/// called by MQCTL, and MQSET on the queue manager of ZEUS_MQ (skipped
/// without it).
#[test]
fn mq_pubsub_properties_and_callbacks() {
    if std::env::var_os("ZEUS_MQ").is_none() {
        eprintln!("ZEUS_MQ is not set: MQ test skipped");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut env = env(dir.path());
    let mut job = String::from("//MQPSJOB  JOB (ACCT),'MQ'\n");
    job.push_str(&compile_step("MQCBPGM", "IGYCRCTL", MQCBPGM));
    job.push_str(&compile_step("MQPS", "IGYCRCTL", MQPS));
    job.push_str("//RUN      EXEC PGM=MQPS\n//STEPLIB  DD DSN=ZEUS.LOADLIB,DISP=SHR\n//SYSOUT   DD SYSOUT=*\n");
    let mut log = Vec::new();
    let r = run_job(&mut env, &job, &mut log).unwrap();
    let log = String::from_utf8_lossy(&log);
    let out: String = std::fs::read_dir(dir.path().join("spool/MQPSJOB"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().contains("RUN.SYSOUT"))
        .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
        .collect();
    assert_eq!(
        outcomes(&r).last(),
        Some(&("RUN".into(), Outcome::Rc(0))),
        "{log}\\n{out}"
    );
    let lines: Vec<&str> = out.lines().map(str::trim_end).collect();
    assert_eq!(
        lines,
        [
            "CONN 00000000{",
            "SUB 00000000{",
            "OPEN TOPIC 00000000{",
            "CRTMH 00000000{",
            "SETMP 00000000{",
            "PUT 00000000{",
            "GOT 00000000{ PUBLISHED",
            "COLOUR 00000000{ Blue 00000102D",
            "COUNT 00000000{ 00000004B 00000000D",
            "DLTMH 00000000{",
            "CLOSE SUB 00000000{",
            "CB 00000000{",
            "CALLBACK ASYNC 1 MQSTR",
            "CALLBACK ASYNC 2 MQSTR",
            "STOPPED AFTER 2",
            "CTL 00000000{",
            "DEREGISTER 00000000{",
            "SET 00000204{",
            "DISC 00000000{",
        ],
        "{out}"
    );
}

const SHOPDBD: &str = "         DBD   NAME=SHOPDB,ACCESS=(HDAM,OSAM),RMNAME=(DFSHDC40,5,10)
         DATASET DD1=SHOPDD
         SEGM  NAME=CUSTOMER,PARENT=0,BYTES=30
         FIELD NAME=(CUSTNO,SEQ,U),BYTES=5,START=1,TYPE=C
         SEGM  NAME=ORDERS,PARENT=CUSTOMER,BYTES=20
         FIELD NAME=(ORDNO,SEQ,U),BYTES=4,START=1,TYPE=C
         DBDGEN
         FINISH
         END
";

const SHOPPSB: &str = "         PCB   TYPE=DB,DBDNAME=SHOPDB,PROCOPT=A,KEYLEN=9
         SENSEG NAME=CUSTOMER,PARENT=0
         SENSEG NAME=ORDERS,PARENT=CUSTOMER
         PSBGEN LANG=COBOL,PSBNAME=SHOPPSB,CMPAT=YES
         END
";

/// Data division items of the IMS test programs.
const SHOPDATA: &str = "       01 CUST-REC.
          05 CUST-NO    PIC X(5).
          05 CUST-NAME  PIC X(25).
       01 ORD-REC.
          05 ORD-NO     PIC X(4).
          05 ORD-ITEM   PIC X(16).
       01 SSA-CUST.
          05 FILLER     PIC X(9) VALUE 'CUSTOMER('.
          05 FILLER     PIC X(10) VALUE 'CUSTNO  = '.
          05 SSA-KEY    PIC X(5).
          05 FILLER     PIC X VALUE ')'.
       01 SSA-CUST-U    PIC X(9) VALUE 'CUSTOMER '.
       01 SSA-ORD-U     PIC X(9) VALUE 'ORDERS   '.
       01 F-ISRT        PIC X(4) VALUE 'ISRT'.
       01 F-GU          PIC X(4) VALUE 'GU  '.
       01 F-GHU         PIC X(4) VALUE 'GHU '.
       01 F-GN          PIC X(4) VALUE 'GN  '.
       01 F-REPL        PIC X(4) VALUE 'REPL'.
       01 F-DLET        PIC X(4) VALUE 'DLET'.
       LINKAGE SECTION.
       01 IOPCB         PIC X(48).
       01 DBPCB.
          05 PCB-DBD    PIC X(8).
          05 PCB-LEVEL  PIC X(2).
          05 PCB-STATUS PIC X(2).
          05 PCB-PROCOPT PIC X(4).
          05 FILLER     PIC S9(5) COMP.
          05 PCB-SEGM   PIC X(8).
          05 PCB-KFBL   PIC S9(5) COMP.
          05 PCB-NSENS  PIC S9(5) COMP.
          05 PCB-KFB    PIC X(9).
";

/// DL/I calls: inserts (with a duplicate), qualified GU, GHU and REPL,
/// a missing key, DLET of a customer with its orders, and GN through
/// the database.
fn shopload() -> String {
    format!(
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. SHOPLOAD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
{SHOPDATA}       PROCEDURE DIVISION USING IOPCB DBPCB.
           DISPLAY 'DBD ' PCB-DBD ' PROCOPT ' PCB-PROCOPT
           MOVE 'C0001ALICE' TO CUST-REC
           CALL 'CBLTDLI' USING F-ISRT DBPCB CUST-REC SSA-CUST-U
           MOVE 'C0001' TO SSA-KEY
           MOVE '0001BOOK' TO ORD-REC
           CALL 'CBLTDLI' USING F-ISRT DBPCB ORD-REC SSA-CUST SSA-ORD-U
           MOVE '0002PEN' TO ORD-REC
           CALL 'CBLTDLI' USING F-ISRT DBPCB ORD-REC SSA-CUST SSA-ORD-U
           MOVE 'C0002BOB' TO CUST-REC
           CALL 'CBLTDLI' USING F-ISRT DBPCB CUST-REC SSA-CUST-U
           MOVE 'C0002' TO SSA-KEY
           MOVE '0001LAMP' TO ORD-REC
           CALL 'CBLTDLI' USING F-ISRT DBPCB ORD-REC SSA-CUST SSA-ORD-U
           MOVE 'C0003CAROL' TO CUST-REC
           CALL 'CBLTDLI' USING F-ISRT DBPCB CUST-REC SSA-CUST-U
           MOVE 'C0001DUPLICATE' TO CUST-REC
           CALL 'CBLTDLI' USING F-ISRT DBPCB CUST-REC SSA-CUST-U
           DISPLAY 'ISRT DUPLICATE ' PCB-STATUS
           MOVE 'C0002' TO SSA-KEY
           CALL 'CBLTDLI' USING F-GHU DBPCB CUST-REC SSA-CUST
           DISPLAY 'GHU ' PCB-STATUS ' ' CUST-NAME ' ' PCB-SEGM
                   ' ' PCB-LEVEL ' ' PCB-KFBL ' ' PCB-KFB(1:5)
           MOVE 'ROBERT' TO CUST-NAME
           CALL 'CBLTDLI' USING F-REPL DBPCB CUST-REC
           DISPLAY 'REPL ' PCB-STATUS
           MOVE 'C0009' TO SSA-KEY
           CALL 'CBLTDLI' USING F-GU DBPCB CUST-REC SSA-CUST
           DISPLAY 'GU C0009 ' PCB-STATUS
           MOVE 'C0003' TO SSA-KEY
           CALL 'CBLTDLI' USING F-GHU DBPCB CUST-REC SSA-CUST
           CALL 'CBLTDLI' USING F-DLET DBPCB CUST-REC
           DISPLAY 'DLET ' PCB-STATUS
           MOVE 'C0001' TO SSA-KEY
           CALL 'CBLTDLI' USING F-GU DBPCB CUST-REC SSA-CUST
      *    GA: a segment of a higher level (the next customer)
           PERFORM UNTIL PCB-STATUS NOT = SPACES AND NOT = 'GA'
               DISPLAY 'GN ' PCB-STATUS ' ' PCB-SEGM ' ' PCB-LEVEL ' '
                       PCB-KFB(1:PCB-KFBL)
               MOVE SPACES TO ORD-REC
               CALL 'CBLTDLI' USING F-GN DBPCB ORD-REC
           END-PERFORM
           DISPLAY 'END ' PCB-STATUS
           GOBACK.
"
    )
}

/// EXEC DLI in a BMP: GU with WHERE, GNP, then an insert that the abend
/// backs out.
fn shopread() -> String {
    format!(
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. SHOPREAD.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-KEY        PIC X(5) VALUE 'C0002'.
       01 ABCODE        PIC S9(9) BINARY VALUE 123.
       01 TIMING        PIC S9(9) BINARY VALUE 0.
{SHOPDATA}       PROCEDURE DIVISION USING IOPCB DBPCB.
           EXEC DLI GU USING PCB(2) SEGMENT(CUSTOMER)
                WHERE (CUSTNO = WS-KEY) INTO(CUST-REC)
           END-EXEC
           DISPLAY 'GU ' DIBSTAT ' ' CUST-REC ' ' DIBSEGM ' ' DIBSEGLV
           EXEC DLI GNP USING PCB(2) SEGMENT(ORDERS) INTO(ORD-REC)
           END-EXEC
           DISPLAY 'GNP ' DIBSTAT ' ' ORD-REC ' ' DIBSEGLV
           EXEC DLI GNP USING PCB(2) SEGMENT(ORDERS) INTO(ORD-REC)
           END-EXEC
           DISPLAY 'GNP ' DIBSTAT
           MOVE 'C0007LOST' TO CUST-REC
           EXEC DLI ISRT USING PCB(2) SEGMENT(CUSTOMER) FROM(CUST-REC)
           END-EXEC
           DISPLAY 'ISRT ' DIBSTAT
           CALL 'CEE3ABD' USING ABCODE TIMING
           GOBACK.
"
    )
}

/// The insert of the abended step is not in the database.
fn shopchk() -> String {
    format!(
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. SHOPCHK.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
{SHOPDATA}       PROCEDURE DIVISION USING IOPCB DBPCB.
           MOVE 'C0007' TO SSA-KEY
           CALL 'CBLTDLI' USING F-GU DBPCB CUST-REC SSA-CUST
           DISPLAY 'GU C0007 ' PCB-STATUS
           MOVE 'C0002' TO SSA-KEY
           CALL 'CBLTDLI' USING F-GU DBPCB CUST-REC SSA-CUST
           DISPLAY 'GU C0002 ' PCB-STATUS ' ' CUST-NAME
           GOBACK.
"
    )
}

/// SSAs with command codes and Boolean qualifications, and a symbolic
/// checkpoint.
fn shopmore() -> String {
    format!(
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. SHOPMORE.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 SSA-PATH.
          05 FILLER     PIC X(11) VALUE 'CUSTOMER*D('.
          05 FILLER     PIC X(10) VALUE 'CUSTNO  = '.
          05 FILLER     PIC X(6) VALUE 'C0001)'.
       01 SSA-ORD-GE.
          05 FILLER     PIC X(9) VALUE 'ORDERS  ('.
          05 FILLER     PIC X(10) VALUE 'ORDNO   >='.
          05 FILLER     PIC X(5) VALUE '0002)'.
       01 PATH-AREA.
          05 P-CUST     PIC X(30).
          05 P-ORD      PIC X(20).
       01 SSA-OR.
          05 FILLER     PIC X(9) VALUE 'CUSTOMER('.
          05 FILLER     PIC X(10) VALUE 'CUSTNO  = '.
          05 FILLER     PIC X(6) VALUE 'C0009+'.
          05 FILLER     PIC X(10) VALUE 'CUSTNO  = '.
          05 FILLER     PIC X(6) VALUE 'C0002)'.
       01 SSA-AND.
          05 FILLER     PIC X(9) VALUE 'CUSTOMER('.
          05 FILLER     PIC X(10) VALUE 'CUSTNO  GT'.
          05 FILLER     PIC X(6) VALUE 'C0000&'.
          05 FILLER     PIC X(10) VALUE 'CUSTNO  < '.
          05 FILLER     PIC X(6) VALUE 'C0002)'.
       01 SSA-LAST      PIC X(11) VALUE 'ORDERS  *L '.
       01 F-CHKP        PIC X(4) VALUE 'CHKP'.
       01 IO-LEN        PIC S9(9) COMP VALUE 12.
       01 CHKP-ID       PIC X(12) VALUE 'SHOP0001'.
       01 SAVE-LEN      PIC S9(9) COMP VALUE 10.
       01 SAVED         PIC X(10) VALUE 'COUNT=0042'.
{SHOPDATA}       PROCEDURE DIVISION USING IOPCB DBPCB.
           CALL 'CBLTDLI' USING F-GU DBPCB PATH-AREA SSA-PATH SSA-ORD-GE
           DISPLAY 'PATH ' PCB-STATUS ' ' P-CUST(1:10) ' ' P-ORD(1:8)
                   PCB-SEGM
           CALL 'CBLTDLI' USING F-GU DBPCB CUST-REC SSA-OR
           DISPLAY 'OR ' PCB-STATUS ' ' CUST-NO
           CALL 'CBLTDLI' USING F-GU DBPCB CUST-REC SSA-AND
           DISPLAY 'AND ' PCB-STATUS ' ' CUST-NO
           MOVE 'C0001' TO SSA-KEY
           CALL 'CBLTDLI' USING F-GU DBPCB ORD-REC SSA-CUST SSA-LAST
           DISPLAY 'LAST ' PCB-STATUS ' ' ORD-NO
           CALL 'CBLTDLI' USING F-CHKP IOPCB IO-LEN CHKP-ID SAVE-LEN
                SAVED
           DISPLAY 'CHKP ' IOPCB(11:2)
           GOBACK.
"
    )
}

/// XRST: a normal start, then a restart from the symbolic checkpoint.
fn shoprst() -> String {
    format!(
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. SHOPRST.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 F-XRST        PIC X(4) VALUE 'XRST'.
       01 IO-LEN        PIC S9(9) COMP VALUE 12.
       01 RST-ID        PIC X(12) VALUE SPACES.
       01 SAVE-LEN      PIC S9(9) COMP VALUE 10.
       01 SAVED         PIC X(10) VALUE 'FRESH'.
{SHOPDATA}       PROCEDURE DIVISION USING IOPCB DBPCB.
           CALL 'CBLTDLI' USING F-XRST IOPCB IO-LEN RST-ID SAVE-LEN
                SAVED
           DISPLAY 'START ' IOPCB(11:2) ' ' SAVED
           MOVE 'SHOP0001' TO RST-ID
           CALL 'CBLTDLI' USING F-XRST IOPCB IO-LEN RST-ID SAVE-LEN
                SAVED
           DISPLAY 'XRST ' IOPCB(11:2) ' ' RST-ID(1:8) ' ' SAVED
           GOBACK.
"
    )
}

/// IMS batch regions (DFSRRC00): DL/I calls and EXEC DLI on a database
/// defined by DBD and PSB members of the IMS DD libraries, committed at
/// the end of a step and backed out when it abends.
#[test]
fn ims_batch_regions() {
    let dir = tempfile::tempdir().unwrap();
    let mut env = env(dir.path());
    for (lib, member, text) in [
        ("ZEUS.DBDLIB", "SHOPDB", SHOPDBD),
        ("ZEUS.PSBLIB", "SHOPPSB", SHOPPSB),
    ] {
        env.catalog
            .define(Entry {
                name: lib.into(),
                kind: EntryKind::Partitioned {
                    recfm: Recfm::Fb,
                    lrecl: 80,
                },
            })
            .unwrap();
        let d = env.catalog.path(lib);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join(member), text).unwrap();
    }
    let mut job = String::from("//IMSJOB   JOB (ACCT),'IMS'\n");
    for (n, src) in [
        ("SHOPLOAD", shopload()),
        ("SHOPREAD", shopread()),
        ("SHOPCHK", shopchk()),
        ("SHOPMORE", shopmore()),
        ("SHOPRST", shoprst()),
    ] {
        job.push_str(&compile_step(n, "IGYCRCTL", &src));
    }
    // CHECK runs after the abend of READ (COND=EVEN)
    for (step, region, pgm, cond) in [
        ("LOAD", "DLI", "SHOPLOAD", ""),
        ("READ", "BMP", "SHOPREAD", ""),
        ("CHECK", "DBB", "SHOPCHK", ",COND=EVEN"),
        ("MORE", "DLI", "SHOPMORE", ",COND=EVEN"),
        ("RESTART", "DLI", "SHOPRST", ",COND=EVEN"),
    ] {
        job.push_str(&format!(
            "//{step:<8} EXEC PGM=DFSRRC00,PARM='{region},{pgm},SHOPPSB'{cond}\n//STEPLIB  DD DSN=ZEUS.LOADLIB,DISP=SHR\n//IMS      DD DSN=ZEUS.PSBLIB,DISP=SHR\n//         DD DSN=ZEUS.DBDLIB,DISP=SHR\n//SYSOUT   DD SYSOUT=*\n"
        ));
    }
    let mut log = Vec::new();
    let r = run_job(&mut env, &job, &mut log).unwrap();
    let log = String::from_utf8_lossy(&log);
    let out = |step: &str| -> Vec<String> {
        std::fs::read_dir(dir.path().join("spool/IMSJOB"))
            .unwrap()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .contains(&format!("{step}.SYSOUT"))
            })
            .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
            .collect::<String>()
            .lines()
            .map(|l| l.trim_end().to_string())
            .collect()
    };
    let steps: Vec<(String, Outcome)> = outcomes(&r).into_iter().skip(10).collect();
    assert_eq!(
        steps,
        [
            ("LOAD".into(), Outcome::Rc(0)),
            ("READ".into(), Outcome::Abend("U0123".into())),
            ("CHECK".into(), Outcome::Rc(0)),
            ("MORE".into(), Outcome::Rc(0)),
            ("RESTART".into(), Outcome::Rc(0)),
        ],
        "{log}"
    );
    assert_eq!(
        out("LOAD"),
        [
            "DBD SHOPDB   PROCOPT A",
            "ISRT DUPLICATE II",
            "GHU    BOB                       CUSTOMER 01 0000E C0002",
            "REPL",
            "GU C0009 GE",
            "DLET",
            "GN    CUSTOMER 01 C0001",
            "GN    ORDERS   02 C00010001",
            "GN    ORDERS   02 C00010002",
            "GN GA CUSTOMER 01 C0002",
            "GN    ORDERS   02 C00020001",
            "END GB",
        ]
    );
    assert_eq!(
        out("READ"),
        [
            "GU    C0002ROBERT                    CUSTOMER 01",
            "GNP    0001LAMP             02",
            "GNP GE",
            "ISRT",
        ]
    );
    assert_eq!(out("CHECK"), ["GU C0007 GE", "GU C0002    ROBERT"]);
    assert_eq!(
        out("MORE"),
        [
            "PATH    C0001ALICE 0002PEN ORDERS",
            "OR    C0002",
            "AND    C0001",
            "LAST    0002",
            "CHKP",
        ]
    );
    assert_eq!(
        out("RESTART"),
        ["START    FRESH", "XRST    SHOP0001 COUNT=0042"]
    );
}

/// The Db2 utilities on the server of ZEUS_DB2 (skipped without it):
/// LOAD of delimited records (DSNUTILB), DSNTIAUL's unload with its
/// LOAD statements, and LOAD of the unloaded records with them.
#[test]
fn db2_unload_and_load() {
    if std::env::var_os("ZEUS_DB2").is_none() {
        eprintln!("ZEUS_DB2 is not set: Db2 utility test skipped");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let mut env = env(dir.path());
    let job = "//UTLJOB   JOB (ACCT),'DB2 UTIL'
//DDL      EXEC PGM=IKJEFT01
//SYSTSPRT DD SYSOUT=*
//SYSPRINT DD SYSOUT=*
//SYSTSIN  DD *
  DSN SYSTEM(DB2)
  RUN PROGRAM(DSNTIAD) PLAN(DSNTIAD) PARMS('RC0')
  END
/*
//SYSIN    DD *
  DROP TABLE ZEUSTEST.UTL1;
/*
//CREATE   EXEC PGM=IKJEFT01
//SYSTSPRT DD SYSOUT=*
//SYSPRINT DD SYSOUT=*
//SYSTSIN  DD *
  DSN SYSTEM(DB2)
  RUN PROGRAM(DSNTIAD) PLAN(DSNTIAD)
  END
/*
//SYSIN    DD *
  CREATE TABLE ZEUSTEST.UTL1 (ID BIGINT NOT NULL,
    NAME CHAR(5), AMT DECIMAL(7,2), TS TIMESTAMP);
/*
//LOAD1    EXEC PGM=DSNUTILB,PARM='DB2'
//SYSPRINT DD SYSOUT=*
//SYSIN    DD *
  LOAD DATA REPLACE
  FORMAT DELIMITED COLDEL ',' CHARDEL '\"' DECPT '.'
  INTO TABLE ZEUSTEST.UTL1
  (ID BIGINT, NAME CHAR, AMT DECIMAL, TS TIMESTAMP EXTERNAL)
/*
//SYSREC   DD *
1,\"ALPHA\",10.50,\"2003-07-23 17:56:00.64\"
2,,-2.25,\"2003-07-23 17:56:01\"
3,\"GAM\"\"A\",,
/*
//UNLOAD   EXEC PGM=IKJEFT01
//SYSTSPRT DD SYSOUT=*
//SYSPRINT DD SYSOUT=*
//SYSREC00 DD DSN=ZEUS.UTL.UNLOAD,DISP=(NEW,CATLG)
//SYSPUNCH DD DSN=ZEUS.UTL.PUNCH,DISP=(NEW,CATLG),RECFM=FB,LRECL=80
//SYSTSIN  DD *
  DSN SYSTEM(DB2)
  RUN PROGRAM(DSNTIAUL) PLAN(DSNTIAUL) PARMS('SQL')
  END
/*
//SYSIN    DD *
  SELECT ID, NAME, AMT FROM ZEUSTEST.UTL1 ORDER BY ID;
/*
//RELOAD   EXEC PGM=DSNUTILB,PARM='DB2'
//SYSPRINT DD SYSOUT=*
//SYSIN    DD DSN=ZEUS.UTL.PUNCH,DISP=SHR
//SYSREC00 DD DSN=ZEUS.UTL.UNLOAD,DISP=SHR
//CHECK    EXEC PGM=IKJEFT01
//SYSTSPRT DD SYSOUT=*
//SYSPRINT DD SYSOUT=*
//SYSTSIN  DD *
  DSN SYSTEM(DB2)
  RUN PROGRAM(DSNTEP2) PLAN(DSNTEP2)
  END
/*
//SYSIN    DD *
  SELECT COUNT(*), COUNT(NAME), SUM(AMT) FROM ZEUSTEST.UTL1;
/*
";
    let mut log = Vec::new();
    let r = run_job(&mut env, job, &mut log).unwrap();
    let log = String::from_utf8_lossy(&log);
    let spool: String = {
        let mut files: Vec<_> = std::fs::read_dir(dir.path().join("spool/UTLJOB"))
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .collect();
        files.sort();
        files
            .iter()
            .map(|p| std::fs::read_to_string(p).unwrap_or_default())
            .collect()
    };
    let rcs: Vec<(String, Outcome)> = outcomes(&r);
    for (step, o) in &rcs[1..] {
        assert_eq!(*o, Outcome::Rc(0), "{step}\n{log}\n{spool}");
    }
    // the unloaded records: ID BIGINT, NAME CHAR(5) and its indicator,
    // AMT DECIMAL(7,2) and its indicator
    let recs = env.catalog.read_records("ZEUS.UTL.UNLOAD").unwrap();
    assert_eq!(recs.len(), 3, "{spool}");
    assert_eq!(recs[0].len(), 8 + 6 + 5);
    assert_eq!(&recs[0][..8], &1i64.to_be_bytes());
    assert_eq!(&recs[0][8..13], &zeus_cobol::ebcdic::encode("ALPHA")[..]);
    assert_eq!(recs[0][13], 0);
    assert_eq!(&recs[0][14..18], &[0x00, 0x01, 0x05, 0x0C]);
    assert_eq!(recs[1][13], b'?' ^ 0x50, "NAME of row 2 is NULL");
    assert_eq!(&recs[1][14..18], &[0x00, 0x00, 0x22, 0x5D]);
    assert_eq!(&recs[2][8..13], &zeus_cobol::ebcdic::encode("GAM\"A")[..]);
    assert_eq!(recs[2][18], b'?' ^ 0x50, "AMT of row 3 is NULL");
    // reloaded with the LOAD statements DSNTIAUL wrote: 6 rows
    assert!(spool.contains("| 6 | 4 | 16.50"), "{spool}");
}

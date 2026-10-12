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

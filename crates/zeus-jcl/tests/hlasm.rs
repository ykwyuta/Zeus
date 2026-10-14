use zeus_ds::catalog::{Catalog, Entry, EntryKind, Recfm};
use zeus_jcl::job::{Env, Outcome};

#[test]
fn assembler_macros_receive_actual_jcl_job_step_sysparm_and_dd_context() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = Catalog::open(&dir.path().join("catalog")).unwrap();
    let mut env = Env {
        catalog,
        spool: dir.path().join("spool"),
        loadlibs: Vec::new(),
        proclibs: Vec::new(),
    };
    let job = "//CTXJOB JOB\n//ASM EXEC PGM=ASMA90,PARM='OBJECT,SYSPARM(Mixed,Case),LIST'\n//SYSPRINT DD SYSOUT=*\n//SYSLIN DD DSN=&&OBJ,DISP=(NEW,DELETE)\n//SYSIN DD *\n MACRO\n CONTEXT\n AIF ('&SYSJOB' EQ 'CTXJOB' AND '&SYSSTEP' EQ 'ASM').ONE\n MNOTE 8,'job or step context'\n.ONE AIF ('&SYSPARM' EQ 'Mixed,Case').TWO\n MNOTE 8,'sysparm context'\n.TWO AIF (K'&SYSLIN_DSN GT 0 AND K'&SYSIN_DSN GT 0).OK\n MNOTE 8,'data set context'\n.OK ANOP\n MEND\nTEST CSECT\n CONTEXT\n LA 15,0\n BR 14\n END\n/*\n";
    let mut log = Vec::new();
    let result = zeus_jcl::job::run_job(&mut env, job, &mut log).unwrap();
    assert_eq!(
        result[0].outcome,
        Outcome::Rc(0),
        "{}",
        String::from_utf8_lossy(&log)
    );
}

#[test]
fn pli_reads_assembler_heap_after_return_and_frees_it_in_another_call_and_step() {
    let dir = tempfile::tempdir().unwrap();
    let load = dir.path().join("load");
    std::fs::create_dir(&load).unwrap();
    for (name, body) in [
        (
            "TAKE",
            " L 2,0(1)\n GETMAIN RU,LV=8,SP=3\n ST 1,0(2)\n MVI 0(1),C'A'\n MVI 1(1),C'B'\n MVI 2(1),C'C'\n LA 15,0\n BR 14",
        ),
        (
            "RELEASE",
            " L 2,0(1)\n L 3,0(2)\n FREEMAIN RU,LV=8,A=(3),SP=3\n LA 15,0\n BR 14",
        ),
    ] {
        let source = dir.path().join(format!("{name}.asm"));
        std::fs::write(&source, format!("{name} CSECT\n{body}\n END\n")).unwrap();
        let module = zeus_cobol::hlasm::compile_file(&source, &[]).unwrap();
        zeus_cobol::hlasm::save_module(&module, &load.join(format!("{name}.zo"))).unwrap();
    }
    let mut catalog = Catalog::open(&dir.path().join("catalog")).unwrap();
    catalog
        .define(Entry {
            name: "APP.LOAD".into(),
            kind: EntryKind::Partitioned {
                recfm: Recfm::U,
                lrecl: 0,
            },
        })
        .unwrap();
    let mut env = Env {
        catalog,
        spool: dir.path().join("spool"),
        loadlibs: vec![load],
        proclibs: Vec::new(),
    };
    let job = "//HEAPJOB JOB\n//PLI EXEC PGM=IBMZPLI\n//SYSPRINT DD SYSOUT=*\n//SYSLIN DD DSN=&&OBJ,DISP=(NEW,PASS)\n//SYSIN DD *\n PMAIN: PROC OPTIONS(MAIN);\n DCL TAKE ENTRY OPTIONS(ASM RETCODE);\n DCL RELEASE ENTRY OPTIONS(ASM RETCODE);\n DCL P POINTER INIT(NULL());\n DCL TEXT CHAR(3) BASED(P);\n DCL I FIXED BIN(31);\n CALL PLIRETC(0);\n DO I = 1 TO 3;\n CALL TAKE(P);\n IF TEXT ^= 'ABC' THEN CALL PLIRETC(99);\n CALL RELEASE(P);\n END;\n END PMAIN;\n/*\n//BIND EXEC PGM=IEWL\n//SYSPRINT DD SYSOUT=*\n//SYSLIN DD DSN=&&OBJ,DISP=(OLD,DELETE)\n//SYSLMOD DD DSN=APP.LOAD(PMAIN),DISP=SHR\n//RUN1 EXEC PGM=PMAIN\n//STEPLIB DD DSN=APP.LOAD,DISP=SHR\n//SYSPRINT DD SYSOUT=*\n//RUN2 EXEC PGM=PMAIN\n//STEPLIB DD DSN=APP.LOAD,DISP=SHR\n//SYSPRINT DD SYSOUT=*\n";
    let mut log = Vec::new();
    let results = zeus_jcl::job::run_job(&mut env, job, &mut log).unwrap();
    assert_eq!(results.len(), 4);
    for result in results {
        assert_eq!(
            result.outcome,
            Outcome::Rc(0),
            "{result:?}: {}",
            String::from_utf8_lossy(&log)
        );
    }
}

#[test]
fn assemble_bind_and_execute_hlasm_cobol_and_pli_in_jcl() {
    let dir = tempfile::tempdir().unwrap();
    let mut catalog = Catalog::open(&dir.path().join("catalog")).unwrap();
    catalog
        .define(Entry {
            name: "APP.LOAD".into(),
            kind: EntryKind::Partitioned {
                recfm: Recfm::U,
                lrecl: 0,
            },
        })
        .unwrap();
    let mut env = Env {
        catalog,
        spool: dir.path().join("spool"),
        loadlibs: Vec::new(),
        proclibs: Vec::new(),
    };
    let job = "//ASMJOB JOB
//ASM EXEC PGM=ASMA90
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&OBJ,DISP=(NEW,PASS)
//SYSIN DD *
ASMADD   CSECT
         MNOTE 4,'warning retains object'
         USING *,15
         L 2,0(1)
         L 3,4(1)
         L 4,0(2)
         AHI 4,7
         ST 4,0(3)
         LA 15,9
         BR 14
         END
/*
//BIND EXEC PGM=IEWL
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&OBJ,DISP=(OLD,DELETE)
//SYSLMOD DD DSN=APP.LOAD(ASMADD),DISP=SHR
//CBL EXEC PGM=IGYCRCTL
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&CBL,DISP=(NEW,PASS)
//SYSIN DD *
       IDENTIFICATION DIVISION.
       PROGRAM-ID. CBLMAIN.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC S9(9) COMP VALUE 5.
       01 B PIC S9(9) COMP VALUE 0.
       PROCEDURE DIVISION.
           CALL 'ASMADD' USING A B
           IF B = 12 AND RETURN-CODE = 9
             MOVE 0 TO RETURN-CODE
           ELSE MOVE 99 TO RETURN-CODE END-IF
           STOP RUN.
/*
//BINDC EXEC PGM=IEWL
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&CBL,DISP=(OLD,DELETE)
//SYSLMOD DD DSN=APP.LOAD(CBLMAIN),DISP=SHR
//RUNC EXEC PGM=CBLMAIN
//STEPLIB DD DSN=APP.LOAD,DISP=SHR
//SYSOUT DD SYSOUT=*
//PLI EXEC PGM=IBMZPLI
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&PLI,DISP=(NEW,PASS)
//SYSIN DD *
 PMAIN: PROC OPTIONS(MAIN);
 DCL ASMADD ENTRY OPTIONS(ASM RETCODE);
 DCL A FIXED BIN(31) INIT(5), B FIXED BIN(31) INIT(0);
 CALL ASMADD(A,B);
 IF B = 12 THEN CALL PLIRETC(0); ELSE CALL PLIRETC(99);
 END PMAIN;
/*
//BINDP EXEC PGM=IEWL
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&PLI,DISP=(OLD,DELETE)
//SYSLMOD DD DSN=APP.LOAD(PMAIN),DISP=SHR
//RUNP EXEC PGM=PMAIN
//STEPLIB DD DSN=APP.LOAD,DISP=SHR
//SYSPRINT DD SYSOUT=*
//ASMM EXEC PGM=ASMAHL
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&MAIN,DISP=(NEW,PASS)
//SYSIN DD *
ASMMAIN CSECT
         USING *,15
         L 2,0(1)
         CLI 2(2),C'X'
         BE OK
         LA 15,99
         BR 14
OK       LA 15,4
         BR 14
         END
/*
//BINDM EXEC PGM=IEWL
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&MAIN,DISP=(OLD,DELETE)
//SYSLMOD DD DSN=APP.LOAD(ASMMAIN),DISP=SHR
//RUNM EXEC PGM=ASMMAIN,PARM='XYZ'
//STEPLIB DD DSN=APP.LOAD,DISP=SHR
";
    let mut log = Vec::new();
    let results = zeus_jcl::job::run_job(&mut env, job, &mut log).unwrap();
    let logs = std::fs::read_dir(dir.path().join("spool/ASMJOB"))
        .unwrap()
        .flatten()
        .map(|f| std::fs::read_to_string(f.path()).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n");
    for step in results {
        assert_eq!(
            step.outcome,
            Outcome::Rc(if matches!(step.step.as_str(), "RUNM" | "ASM") {
                4
            } else {
                0
            }),
            "{}: {}\n{logs}",
            step.step,
            String::from_utf8_lossy(&log)
        );
    }
}

#[test]
fn binder_links_concatenated_objects_and_searches_syslib() {
    let dir = tempfile::tempdir().unwrap();
    let mut catalog = Catalog::open(&dir.path().join("catalog")).unwrap();
    catalog
        .define(Entry {
            name: "APP.LOAD".into(),
            kind: EntryKind::Partitioned {
                recfm: Recfm::U,
                lrecl: 0,
            },
        })
        .unwrap();
    let mut env = Env {
        catalog,
        spool: dir.path().join("spool"),
        loadlibs: vec![],
        proclibs: vec![],
    };
    let job = "//LINKJOB JOB
//ASMS EXEC PGM=ASMA90
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&SUB,DISP=(NEW,PASS)
//SYSIN DD *
SUB CSECT
BEGIN EQU SUB
RESULT EQU 0,33,C'F',33,GR32
ARGREG EQU 2,,,,GR32
&RC SETA I'=FDS(30)E(1)'0E-1'
         AIF (S'=FDS(30)E(1)'0E-1' EQ 30).LITERAL
         MNOTE 8,'unexpected literal attributes'
.LITERAL ANOP
         AIF (D'ARGREG AND T'RESULT EQ 'F').CHECK
         MNOTE 8,'unexpected ordinary attributes'
.CHECK ANOP
         AIF (SYSATTRA('ARGREG') EQ 'GR32').OK
         MNOTE 8,'unexpected assembler type'
.OK ANOP
         L ARGREG,0(1)
FINISH CLI 2(ARGREG),C'X'
         BCR 7,14
         LA 15,&RC
         BR 14
         ENTRY BEGIN,FINISH
         END
/*
//BINDS EXEC PGM=IEWL
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&SUB,DISP=(OLD,PASS)
//SYSLMOD DD DSN=APP.LOAD(SUB),DISP=SHR
//ASMM EXEC PGM=ASMA90
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&MAIN,DISP=(NEW,PASS)
//SYSIN DD *
MAIN CSECT
TARGET EQU SUB+SPAN
SPAN EQU FINISH-BEGIN-4
         USING *,15
         LR 10,14
         L 15,=A(TARGET)
         BRASL 14,TARGET
         LR 14,10
         BR 14
         EXTRN SUB,BEGIN,FINISH
         END
/*
//LINKCAT EXEC PGM=IEWL
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&MAIN,DISP=(OLD,PASS)
//       DD DSN=&&SUB,DISP=(OLD,PASS)
//SYSLMOD DD DSN=APP.LOAD(ROOTCAT),DISP=SHR
//LINKLIB EXEC PGM=IEWL
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&MAIN,DISP=(OLD,PASS)
//SYSLIB DD DSN=APP.LOAD,DISP=SHR
//SYSLMOD DD DSN=APP.LOAD(ROOTLIB),DISP=SHR
//RUNCAT EXEC PGM=ROOTCAT,PARM='XYZ'
//STEPLIB DD DSN=APP.LOAD,DISP=SHR
//RUNLIB EXEC PGM=ROOTLIB,PARM='XYZ'
//STEPLIB DD DSN=APP.LOAD,DISP=SHR
//BADLINK EXEC PGM=IEWL
//SYSPRINT DD SYSOUT=*
//SYSLIN DD DSN=&&MAIN,DISP=(OLD,DELETE)
//SYSLMOD DD DSN=APP.LOAD(BROKEN),DISP=SHR
";
    let mut log = Vec::new();
    let steps = zeus_jcl::job::run_job(&mut env, job, &mut log).unwrap();
    for step in steps {
        let expected = match step.step.as_str() {
            "RUNCAT" | "RUNLIB" => 33,
            "BADLINK" => 12,
            _ => 0,
        };
        assert_eq!(
            step.outcome,
            Outcome::Rc(expected),
            "{}: {}",
            step.step,
            String::from_utf8_lossy(&log)
        );
    }
}

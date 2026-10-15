use zeus_cobol::hlasm::{self, AssemblyDataSet, AssemblyOptions};

#[test]
fn assembly_context_supplies_real_identity_options_date_and_dataset_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("context.asm");
    std::fs::write(&path, " MACRO\n CONTEXT\n MNOTE 0,'&SYSIN_DSN|&SYSIN_MEMBER|&SYSLIN_DSN|&SYSLIB_DSN'\n MEND\n AIF ('&SYSASM' EQ 'ZEUS' AND '&SYSDATC' EQ '19700101').DATE\n MNOTE 8,'assembler/date'\n.DATE AIF ('&SYSDATE' EQ '01/01/70' AND '&SYSTIME' EQ '00.00').JOB\n MNOTE 8,'date/time'\n.JOB AIF ('&SYSJOB' EQ 'JOB1' AND '&SYSSTEP' EQ 'ASM').PARM\n MNOTE 8,'job/step'\n.PARM AIF ('&SYSPARM' EQ 'Mixed,Case' AND T'&SYSOPT_DBCS EQ 'N').OPT\n MNOTE 8,'sysparm/type'\n.OPT AIF (&SYSOPT_CODEPAGE EQ 37 AND NOT &SYSOPT_RENT).OK\n MNOTE 8,'options'\n.OK ANOP\nTEST CSECT\n CONTEXT\n LA 15,0\n BR 14\n END\n").unwrap();
    let mut options = AssemblyOptions {
        sysparm: "Mixed,Case".into(),
        job: Some("JOB1".into()),
        step: Some("ASM".into()),
        started: std::time::UNIX_EPOCH,
        ..Default::default()
    };
    options.data_sets.insert(
        "SYSIN".into(),
        AssemblyDataSet {
            name: "APP.SOURCE".into(),
            member: "MAIN".into(),
            volume: "VOL001".into(),
        },
    );
    options.data_sets.insert(
        "SYSLIN".into(),
        AssemblyDataSet {
            name: "APP.OBJECT".into(),
            ..Default::default()
        },
    );
    let report = hlasm::compile_report_with_options(&path, &[], &options);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    assert_eq!(report.diagnostics[0].message, "APP.SOURCE|MAIN|APP.OBJECT|");
}

#[test]
fn macro_local_context_is_captured_at_entry_while_inner_calls_see_new_sections() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("context.asm");
    std::fs::write(&path, " MACRO\n INNER\n MNOTE 0,'&SYSECT|&SYSLOC|&SYSSTYP'\n MEND\n MACRO\n OUTER\n MNOTE 0,'&SYSECT|&SYSLOC|&SYSSTYP'\nAREA LOCTR\n INNER\n MNOTE 0,'&SYSECT|&SYSLOC|&SYSSTYP'\n MEND\nTEST CSECT\n OUTER\n LOCTR\n LA 15,0\n BR 14\n END\n").unwrap();
    let report = hlasm::compile_report(&path, &[]);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        ["TEST|TEST|CSECT", "TEST|AREA|CSECT", "TEST|TEST|CSECT"]
    );
}

#[test]
fn library_member_and_sequence_fields_are_preserved_through_nested_calls() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    std::fs::write(
        dir.path().join("TRACE.mac"),
        " MACRO\n TRACE\n MNOTE 0,'&SYSLIB_MEMBER|&SYSSEQF'\n MEND\n",
    )
    .unwrap();
    let call = format!("{:<72}SEQ00001", " TRACE");
    std::fs::write(
        &path,
        format!("TEST CSECT\n{call}\n LA 15,0\n BR 14\n END\n"),
    )
    .unwrap();
    let report = hlasm::compile_report(&path, &[]);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    assert_eq!(report.diagnostics[0].message, "TRACE|SEQ00001");
}

#[test]
fn cli_sysparm_preserves_values_that_look_like_help_options() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("main.asm");
    let object = dir.path().join("main.zo");
    std::fs::write(&source, " AIF ('&SYSPARM' EQ '--help').OK\n MNOTE 8,'sysparm was lost'\n.OK ANOP\nTEST CSECT\n LA 15,0\n BR 14\n END\n").unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["compile", "--sysparm", "--help", "-o"])
        .arg(&object)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(object.exists());
}

#[test]
fn macro_library_context_identifies_the_library_that_supplied_the_member() {
    let dir = tempfile::tempdir().unwrap();
    let lib = dir.path().join("second");
    std::fs::create_dir(&lib).unwrap();
    let source = dir.path().join("main.asm");
    std::fs::write(
        lib.join("TRACE.mac"),
        " MACRO\n TRACE\n MNOTE 0,'&SYSLIB_DSN|&SYSLIB_MEMBER'\n MEND\n",
    )
    .unwrap();
    std::fs::write(&source, "TEST CSECT\n TRACE\n LA 15,0\n BR 14\n END\n").unwrap();
    let mut options = AssemblyOptions::default();
    options.data_sets.insert(
        "SYSLIB".into(),
        AssemblyDataSet {
            name: "APP.FIRST".into(),
            ..Default::default()
        },
    );
    options.library_data_sets.insert(
        lib.clone(),
        AssemblyDataSet {
            name: "APP.SECOND".into(),
            ..Default::default()
        },
    );
    let report = hlasm::compile_report_with_options(&source, &[lib], &options);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    assert_eq!(report.diagnostics[0].message, "APP.SECOND|TRACE");
}

#[test]
fn fatal_operand_diagnostics_include_the_operation_at_the_reported_location() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.asm");
    std::fs::write(&path, "TEST CSECT\n LR 99,1\n END\n").unwrap();
    let report = hlasm::compile_report(&path, &[]);
    assert_eq!(report.diagnostics[0].operation, "LR");
    assert_eq!(report.diagnostics[0].line, 2);
    assert!(!report.complete);
    std::fs::write(
        &path,
        " MACRO\n BAD\n MVI 0(0),256\n MEND\nTEST CSECT\n BAD\n END\n",
    )
    .unwrap();
    let report = hlasm::compile_report(&path, &[]);
    assert_eq!(report.diagnostics[0].operation, "MVI");
    assert_eq!(report.diagnostics[0].line, 6);
}

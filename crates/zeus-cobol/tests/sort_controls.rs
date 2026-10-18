use zeus_cobol::{front::FrontOptions, vm::Machine};

fn compile(source: &str) -> zeus_cobol::zir::Module {
    try_compile(source).unwrap()
}

fn try_compile(source: &str) -> Result<zeus_cobol::zir::Module, zeus_cobol::diag::Diag> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.cbl");
    assert!(source.lines().all(|l| l.len() <= 65));
    std::fs::write(
        &path,
        source
            .lines()
            .map(|l| format!("       {l}\n"))
            .collect::<String>(),
    )
    .unwrap();
    zeus_cobol::compile_file(&path, &FrontOptions::default())
}

fn source(setup: &str) -> String {
    format!(
        "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT S ASSIGN TO SORTWORK.\nSELECT O ASSIGN TO RESULT.\nDATA DIVISION.\nFILE SECTION.\nSD S.\n01 R.\n02 K PIC X.\n02 V PIC 9.\nFD O.\n01 OUT-REC PIC XX.\nPROCEDURE DIVISION.\nIF SORT-CONTROL NOT = 'IGZSRTCD' MOVE 1 TO RETURN-CODE\n GOBACK END-IF\nIF SORT-MESSAGE NOT = 'SYSOUT' MOVE 2 TO RETURN-CODE\n GOBACK END-IF\n{setup}\nSORT S ON ASCENDING KEY K\n INPUT PROCEDURE FILL-RECORDS GIVING O\nGOBACK.\nFILL-RECORDS.\nMOVE 'B1' TO R\nRELEASE R\nMOVE 'A2' TO R\nRELEASE R\nMOVE 'B3' TO R\nRELEASE R\nMOVE 'A4' TO R\nRELEASE R."
    )
}

#[test]
fn merge_checks_each_input_order_and_reports_missing_inputs() {
    let src = "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT S ASSIGN SORTWORK.\nSELECT I1 ASSIGN FIRST FILE STATUS FS.\nSELECT I2 ASSIGN SECOND FILE STATUS FS.\nSELECT O ASSIGN RESULT.\nDATA DIVISION.\nFILE SECTION.\nSD S.\n01 R.\n02 K PIC X.\n02 V PIC X.\nFD I1.\n01 R1 PIC XX.\nFD I2.\n01 R2 PIC XX.\nFD O.\n01 RO PIC XX.\nWORKING-STORAGE SECTION.\n01 FS PIC XX.\nPROCEDURE DIVISION.\nMERGE S ON ASCENDING KEY K USING I1 I2 GIVING O\nIF SORT-RETURN NOT = EXPECTED MOVE 9 TO RETURN-CODE END-IF\nGOBACK.";
    for (input, expected) in [(Some("B2D4"), 0), (Some("D4B2"), 16), (None, 16)] {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first");
        let second = dir.path().join("second");
        let result = dir.path().join("result");
        std::fs::write(&first, zeus_cobol::ebcdic::encode("A1C3")).unwrap();
        if let Some(input) = input {
            std::fs::write(&second, zeus_cobol::ebcdic::encode(input)).unwrap();
        }
        let mut m = Machine::new(vec![]);
        m.set_stdout(Box::new(std::io::sink()));
        m.set_dd("FIRST", first);
        m.set_dd("SECOND", second);
        m.set_dd("RESULT", result.clone());
        assert_eq!(
            m.run_main(compile(&src.replace("EXPECTED", &expected.to_string())))
                .unwrap(),
            0
        );
        assert_eq!(result.exists(), expected == 0);
        if expected == 0 {
            assert_eq!(
                std::fs::read(result).unwrap(),
                zeus_cobol::ebcdic::encode("A1B2C3D4")
            );
        }
    }
}

#[test]
fn national_keys_use_national_order_with_an_alphanumeric_collation() {
    let src = source("")
        .replace("ENVIRONMENT DIVISION.\n","ENVIRONMENT DIVISION.\nCONFIGURATION SECTION.\nSPECIAL-NAMES. ALPHABET RV IS 256 THRU 1.\n")
        .replace("02 K PIC X.","02 K PIC N.")
        .replace("02 V PIC 9.","02 V PIC X.")
        .replace("01 OUT-REC PIC XX.","01 OUT-REC PIC X(3).")
        .replace("ASCENDING KEY K\n","ASCENDING KEY K COLLATING SEQUENCE RV\n");
    let (header, _) = src.split_once("FILL-RECORDS.\n").unwrap();
    let src = format!(
        "{header}FILL-RECORDS.\nMOVE N'Z' TO K\nMOVE 'Z' TO V\nRELEASE R\nMOVE N'A' TO K\nMOVE 'A' TO V\nRELEASE R\nMOVE N'M' TO K\nMOVE 'M' TO V\nRELEASE R."
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("result");
    let mut m = Machine::new(vec![]);
    m.set_dd("RESULT", path.clone());
    assert_eq!(m.run_main(compile(&src)).unwrap(), 0);
    let records = std::fs::read(path).unwrap();
    assert_eq!(
        records.chunks_exact(3).map(|r| r[2]).collect::<Vec<_>>(),
        zeus_cobol::ebcdic::encode("AMZ")
    );
}

#[test]
fn floating_keys_keep_the_full_hfp_range_during_spill_and_merge() {
    let src = source("MOVE 1 TO SORT-CORE-SIZE")
        .replace("02 K PIC X.", "02 K USAGE COMP-2.")
        .replace("02 V PIC 9.", "02 V PIC X.")
        .replace("01 OUT-REC PIC XX.", "01 OUT-REC PIC X(9).");
    let (header, _) = src.split_once("FILL-RECORDS.\n").unwrap();
    let src = format!(
        "{header}FILL-RECORDS.\nMOVE 1.0E+60 TO K\nMOVE 'C' TO V\nRELEASE R\nMOVE -1.0E+60 TO K\nMOVE 'A' TO V\nRELEASE R\nMOVE 1.0E+59 TO K\nMOVE 'B' TO V\nRELEASE R."
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("result");
    let mut m = Machine::new(vec![]);
    m.set_dd("RESULT", path.clone());
    assert_eq!(m.run_main(compile(&src)).unwrap(), 0);
    let records = std::fs::read(path).unwrap();
    assert_eq!(
        records.chunks_exact(9).map(|r| r[8]).collect::<Vec<_>>(),
        zeus_cobol::ebcdic::encode("ABC")
    );
}

#[test]
fn sort_diagnoses_wrong_records_keys_and_execution_phases() {
    let base = source("");
    for bad in [
        base.replace("KEY K", "KEY OUT-REC"),
        base.replace("02 K PIC X.", "02 K OCCURS 2 PIC X."),
        base.replace("SD S.", "FD S."),
        base.replace("RELEASE R", "RELEASE OUT-REC"),
        base.replace("PROGRAM-ID. T.", "PROGRAM-ID. T RECURSIVE.\nCBL THREAD"),
    ] {
        assert!(try_compile(&bad).is_err());
    }
    let outside = base.replace(
        "SORT S ON ASCENDING KEY K\n INPUT PROCEDURE FILL-RECORDS GIVING O",
        "RELEASE R",
    );
    assert!(
        Machine::new(vec![])
            .run_main(compile(&outside))
            .unwrap_err()
            .0
            .contains("input procedure")
    );
    let invalid_phase = base.replace(
        "MOVE 'B1' TO R\nRELEASE R",
        "RETURN S AT END CONTINUE END-RETURN",
    );
    assert!(
        Machine::new(vec![])
            .run_main(compile(&invalid_phase))
            .unwrap_err()
            .0
            .contains("output procedure")
    );
}

#[test]
fn exact_file_size_terminates_on_mismatch_but_estimates_do_not() {
    for (size, status) in [
        ("4", 0),
        ("1", 16),
        ("0", 16),
        ("E1", 0),
        ("U1", 0),
        ("1000000000000000000000000000", 16),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let control = dir.path().join("control");
        let result = dir.path().join("result");
        let messages = dir.path().join("messages");
        std::fs::write(
            &control,
            format!(" OPTION FILSZ={size},MAINSIZE=1K,RESINV=1M,MSGDDN=SORTMSG\n"),
        )
        .unwrap();
        let source = source("").replace("GOBACK.\nFILL-RECORDS.",&format!("IF SORT-RETURN NOT = {status} MOVE 9 TO RETURN-CODE\nEND-IF\nGOBACK.\nFILL-RECORDS."));
        let mut m = Machine::new(vec![]);
        m.set_dd("IGZSRTCD", control);
        m.set_dd("RESULT", result.clone());
        m.set_dd("SORTMSG", messages.clone());
        assert_eq!(m.run_main(compile(&source)).unwrap(), 0, "{size}");
        assert_eq!(result.exists(), status == 0, "{size}");
        if status == 16 {
            assert!(
                std::fs::read_to_string(messages)
                    .unwrap()
                    .contains("ICE047A")
            );
        }
    }
}

#[test]
fn checkpoint_control_dd_restarts_sort_without_repeating_the_input() {
    for (keyword, rerun) in [("CKPT", false), ("CHKPT", false), ("CKPT", true)] {
        let dir = tempfile::tempdir().unwrap();
        let checkpoint = dir.path().join("sort.ckpt");
        let control = dir.path().join("control");
        let result = dir.path().join("result");
        std::fs::write(&control, format!(" OPTION {keyword},MAINSIZE=1\n")).unwrap();
        let mut source = source("").replace(
            "GOBACK.\nFILL-RECORDS.",
            "CALL 'CRASH'\nGOBACK.\nFILL-RECORDS.\nCALL 'COUNT'",
        );
        if rerun {
            source = source.replace(
                "DATA DIVISION.",
                "I-O-CONTROL.\nRERUN ON COBCKPT.\nDATA DIVISION.",
            );
        }
        let mut first = Machine::new(vec![]);
        first.set_stdout(Box::new(std::io::sink()));
        first.set_dd("IGZSRTCD", control);
        first.set_dd("SORTCKPT", checkpoint.clone());
        let cobol_checkpoint = dir.path().join("cobol.ckpt");
        first.set_dd("COBCKPT", cobol_checkpoint.clone());
        first.set_dd("RESULT", result.clone());
        first.add_native("COUNT", |_, _| Ok(0));
        first.add_native("CRASH", |_, _| Err("interrupted".into()));
        assert!(
            first
                .run_main(compile(&source))
                .unwrap_err()
                .0
                .contains("interrupted")
        );
        assert!(first.checkpoint_files().contains(&checkpoint));
        assert_eq!(cobol_checkpoint.exists(), rerun);
        let mut resumed = Machine::new(vec![]);
        resumed.set_stdout(Box::new(std::io::sink()));
        resumed.add_native("COUNT", |_, _| Err("input was repeated".into()));
        resumed.add_native("CRASH", |_, _| Ok(0));
        assert_eq!(resumed.restart_checkpoint(&checkpoint).unwrap(), 0);
        assert_eq!(
            std::fs::read(result).unwrap(),
            zeus_cobol::ebcdic::encode("A2A4B1B3")
        );
    }
}

#[test]
fn cancellation_on_release_leaves_the_input_procedure_and_skips_giving() {
    let source = source("")
        .replace(
            "MOVE 'B1' TO R\nRELEASE R",
            "MOVE 16 TO SORT-RETURN\nRELEASE R\nMOVE 9 TO RETURN-CODE",
        )
        .replace(
            "GOBACK.\nFILL-RECORDS.",
            "IF SORT-RETURN NOT = 16 MOVE 8 TO RETURN-CODE END-IF\nGOBACK.\nFILL-RECORDS.",
        );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("result");
    let mut m = Machine::new(vec![]);
    m.set_dd("RESULT", path.clone());
    assert_eq!(m.run_main(compile(&source)).unwrap(), 0);
    assert!(!path.exists());
}

#[test]
fn cancellation_on_return_leaves_the_output_procedure() {
    let source = source("")
        .replace(
            "INPUT PROCEDURE FILL-RECORDS GIVING O",
            "INPUT PROCEDURE FILL-RECORDS\n OUTPUT PROCEDURE EMIT-RECORDS",
        )
        .replace(
            "GOBACK.\nFILL-RECORDS.",
            "IF SORT-RETURN NOT = 16 MOVE 8 TO RETURN-CODE END-IF\nGOBACK.\nFILL-RECORDS.",
        );
    let source = format!(
        "{source}\nEMIT-RECORDS.\nMOVE 16 TO SORT-RETURN\nRETURN S AT END MOVE 7 TO RETURN-CODE END-RETURN\nMOVE 9 TO RETURN-CODE."
    );
    assert_eq!(Machine::new(vec![]).run_main(compile(&source)).unwrap(), 0);
}

#[test]
fn spill_sort_preserves_duplicates_and_estimate_is_not_a_limit() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("result");
    let module = compile(&source(
        "MOVE 16 TO SORT-RETURN\nMOVE 1 TO SORT-CORE-SIZE SORT-FILE-SIZE\nMOVE 2 TO SORT-MODE-SIZE",
    ));
    assert_eq!(module.compilation_return_code(), 4);
    let mut machine = Machine::new(vec![]);
    machine.set_dd("RESULT", path.clone());
    assert_eq!(machine.run_main(module).unwrap(), 0);
    assert_eq!(
        std::fs::read(path).unwrap(),
        zeus_cobol::ebcdic::encode("A2A4B1B3")
    );
}

#[test]
fn ebcdic_control_dd_overrides_registers_and_routes_messages() {
    let dir = tempfile::tempdir().unwrap();
    let control = dir.path().join("control");
    let messages = dir.path().join("messages");
    let result = dir.path().join("result");
    let last = "OPTION MSGDDN=SORT";
    let split = format!("{:<width$}{last}*", "C", width = 71 - last.len());
    let cards = [
        "CARD OPTION MAINSIZE=1,FILSZ=E1, remarks".to_string(),
        "* comment between continuation cards".into(),
        "               SMS=2".into(),
        split,
        "               MSG".into(),
        " SMS=2".into(),
    ];
    let mut bytes = Vec::new();
    for (index, text) in cards.iter().enumerate() {
        let mut card = zeus_cobol::ebcdic::encode(text);
        card.resize(80, 0x40);
        card[72..80].copy_from_slice(&zeus_cobol::ebcdic::encode(&format!("{index:08}")));
        bytes.extend_from_slice(&card);
    }
    std::fs::write(&control, bytes).unwrap();
    let mut machine = Machine::new(vec![]);
    machine.allocate(
        "CUSTOM",
        zeus_ds::dd::Allocation {
            path: control,
            format: Some(zeus_ds::format::RecordFormat::Fixed(80)),
        },
    );
    machine.set_dd("SORTMSG", messages.clone());
    machine.set_dd("RESULT", result.clone());
    assert_eq!(machine.run_main(compile(&source("MOVE 'CUSTOM' TO SORT-CONTROL\nMOVE 1000 TO SORT-CORE-SIZE\nMOVE 'MISSING' TO SORT-MESSAGE"))).unwrap(), 0);
    assert_eq!(
        std::fs::read(result).unwrap(),
        zeus_cobol::ebcdic::encode("A2A4B1B3")
    );
    let messages = std::fs::read_to_string(messages).unwrap();
    for register in [
        "SORT-CORE-SIZE",
        "SORT-FILE-SIZE",
        "SORT-MODE-SIZE",
        "SORT-MESSAGE",
    ] {
        assert!(messages.contains(register), "{messages}");
    }
}

#[test]
fn nested_program_uses_outer_global_registers() {
    let source = "ID DIVISION.\nPROGRAM-ID. OUTER-P.\nPROCEDURE DIVISION.\nMOVE 123 TO SORT-CORE-SIZE\nCALL 'INNER-P'\nIF SORT-CORE-SIZE NOT = 456 MOVE 1 TO RETURN-CODE END-IF\nGOBACK.\nID DIVISION.\nPROGRAM-ID. INNER-P.\nPROCEDURE DIVISION.\nIF SORT-CORE-SIZE NOT = 123 MOVE 1 TO RETURN-CODE\n GOBACK END-IF\nMOVE 456 TO SORT-CORE-SIZE\nGOBACK.\nEND PROGRAM INNER-P.\nEND PROGRAM OUTER-P.";
    assert_eq!(Machine::new(vec![]).run_main(compile(source)).unwrap(), 0);
}

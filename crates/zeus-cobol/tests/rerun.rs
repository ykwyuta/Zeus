use std::{cell::RefCell, io::Write, rc::Rc};
use zeus_cobol::{front::FrontOptions, vm::Machine};

#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn assignment_prefixes_resolve_for_data_checkpoint_and_restart() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO DISK-S-OUTFILE.\nI-O-CONTROL.\nRERUN ON DISK-S-CKPT EVERY 1 RECORDS OF F.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC X.\nPROCEDURE DIVISION.\nOPEN OUTPUT F\nWRITE R FROM 'A'\nCALL 'CRASH'\nWRITE R FROM 'B'\nCLOSE F\nSTOP RUN.";
    assert!(compile(&source.replace("DISK-S-CKPT", "OTHER-S-OUTFILE")).is_err());
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("state.ckpt");
    let data = dir.path().join("output.dat");
    let mut first = Machine::new(vec![]);
    first.set_dd("OUTFILE", data.clone());
    first.set_dd("CKPT", checkpoint.clone());
    first.add_native("CRASH", |_, _| Err("interrupted".into()));
    assert!(
        first
            .run_main(compile(source).unwrap())
            .unwrap_err()
            .0
            .contains("interrupted")
    );
    let mut resumed = Machine::new(vec![]);
    resumed.add_native("CRASH", |_, _| Ok(0));
    assert_eq!(resumed.restart_checkpoint(&checkpoint).unwrap(), 0);
    assert_eq!(
        std::fs::read(data).unwrap(),
        zeus_cobol::ebcdic::encode("AB")
    );
}

#[test]
fn restart_preserves_static_bindings_and_cancelled_dynamic_storage() {
    for (option, directive, expected) in [
        ("DYNAM", "", b"8\n".as_slice()),
        ("NODYNAM", "", b"2\n".as_slice()),
        ("NODYNAM", ">>CALLINTERFACE DYNAMIC", b"8\n".as_slice()),
        ("DYNAM", ">>CALLINTERFACE STATIC", b"2\n".as_slice()),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let sub = |initial| {
            compile(&format!("ID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9 VALUE {initial}.\nPROCEDURE DIVISION.\nADD 1 TO N\nDISPLAY N\nGOBACK." )).unwrap()
        };
        let path = dir.path().join("SUB.zo");
        std::fs::write(&path, zeus_cobol::module_bytes(&sub(0)).unwrap()).unwrap();
        let source = format!(
            "CBL {option}\nID DIVISION.\nPROGRAM-ID. MAIN.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO OUTFILE.\nI-O-CONTROL.\nRERUN ON CKPT EVERY 1 RECORDS OF F.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC X.\nPROCEDURE DIVISION.\nCALL 'SUB'\nCANCEL 'SUB'\nOPEN OUTPUT F\nWRITE R FROM 'A'\nCALL 'CRASH'\nCALL 'SUB'\nCANCEL 'SUB'\nCLOSE F\nSTOP RUN."
        );
        let checkpoint = dir.path().join("step.ckpt");
        let source = source.replace(
            "PROCEDURE DIVISION.\n",
            &format!("PROCEDURE DIVISION.\n{directive}\n"),
        );
        let mut first = Machine::new(vec![dir.path().to_path_buf()]);
        first.set_stdout(Box::new(std::io::sink()));
        first.set_dd("OUTFILE", dir.path().join("out.dat"));
        first.set_dd("CKPT", checkpoint.clone());
        first.add_native("CRASH", |_, _| Err("interrupted".into()));
        assert!(
            first
                .run_main(compile(&source).unwrap())
                .unwrap_err()
                .0
                .contains("interrupted")
        );
        std::fs::write(path, zeus_cobol::module_bytes(&sub(7)).unwrap()).unwrap();
        let output = Capture::default();
        let mut resumed = Machine::new(vec![]);
        resumed.set_stdout(Box::new(output.clone()));
        resumed.add_native("CRASH", |_, _| Ok(0));
        resumed.restart_checkpoint(&checkpoint).unwrap();
        assert_eq!(&*output.0.borrow(), expected, "{option}");
    }
}
fn compile(source: &str) -> Result<zeus_cobol::zir::Module, zeus_cobol::diag::Diag> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    assert!(source.lines().all(|line| line.len() <= 65));
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

#[test]
fn multiple_checkpoints_keep_each_identifier_and_restart_an_earlier_image() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO INFILE.\nI-O-CONTROL.\nRERUN ON CKPT EVERY 2 RECORDS OF F.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC XX.\nPROCEDURE DIVISION.\nOPEN INPUT F\nREAD F\nMOVE 9 TO RETURN-CODE\nREAD F\nIF RETURN-CODE NOT = 0 DISPLAY 'BAD-STATUS' END-IF\nDISPLAY R\nREAD F\nDISPLAY R\nMOVE 9 TO RETURN-CODE\nREAD F\nCALL 'CRASH'\nIF RETURN-CODE NOT = 0 DISPLAY 'BAD-STATUS' END-IF\nDISPLAY R\nREAD F\nDISPLAY R\nCLOSE F\nSTOP RUN.";
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("history.ckpt");
    let input = dir.path().join("input.dat");
    std::fs::write(&input, zeus_cobol::ebcdic::encode("AABBCCDDEE")).unwrap();
    let mut first = Machine::new(vec![]);
    first.set_stdout(Box::new(std::io::sink()));
    first.set_dd("INFILE", input);
    first.set_dd("CKPT", checkpoint.clone());
    first.set_dd_io(
        "CKPT",
        zeus_ds::dd::IoAttributes {
            checkpoint_multiple: true,
            ..Default::default()
        },
    );
    first.add_native("CRASH", |_, _| Err("interrupted".into()));
    assert!(
        first
            .run_main(compile(source).unwrap())
            .unwrap_err()
            .0
            .contains("interrupted")
    );
    assert_eq!(Machine::checkpoint_ids(&checkpoint).unwrap(), vec![1, 2]);
    for (id, expected) in [(2, "DD\nEE\n"), (1, "BB\nCC\nDD\nEE\n")] {
        let capture = Capture::default();
        let mut resumed = Machine::new(vec![]);
        resumed.set_stdout(Box::new(capture.clone()));
        resumed.add_native("CRASH", |_, _| Ok(0));
        assert_eq!(
            resumed
                .restart_checkpoint_at(&checkpoint, Some(id))
                .unwrap(),
            0
        );
        assert_eq!(&*capture.0.borrow(), expected.as_bytes());
    }
    assert_eq!(Machine::checkpoint_ids(&checkpoint).unwrap(), vec![1, 2, 3]);
    let cli = std::process::Command::new(env!("CARGO_BIN_EXE_zcobc"))
        .arg("checkpoints")
        .arg(&checkpoint)
        .output()
        .unwrap();
    assert!(cli.status.success());
    assert_eq!(cli.stdout, b"1\n2\n3\n");
    let mut missing = Machine::new(vec![]);
    assert!(
        missing
            .restart_checkpoint_at(&checkpoint, Some(99))
            .unwrap_err()
            .0
            .contains("identifier not found")
    );
    let damaged = dir.path().join("damaged.ckpt");
    let mut bytes = std::fs::read(&checkpoint).unwrap();
    bytes.pop();
    std::fs::write(&damaged, bytes).unwrap();
    assert!(Machine::checkpoint_ids(&damaged).is_err());
    assert!(Machine::new(vec![]).restart_checkpoint(&damaged).is_err());
}

#[test]
fn checkpoint_resumes_nested_call_perform_input_output_and_returning() {
    let source = "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 ANSWER PIC 99.\nPROCEDURE DIVISION.\nCALL 'WORK' RETURNING ANSWER\nDISPLAY ANSWER\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. WORK.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO INFILE.\nSELECT O ASSIGN TO OUTFILE.\nI-O-CONTROL.\nRERUN ON CKPT EVERY 2 RECORDS OF F.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 IR PIC XX.\nFD O.\n01 OR-REC PIC XX.\nLOCAL-STORAGE SECTION.\n01 N PIC 99 VALUE 0.\n01 DONE PIC 9 VALUE 0.\n01 TXT PIC XXX.\nLINKAGE SECTION.\n01 RESULT-N PIC 99.\nPROCEDURE DIVISION RETURNING RESULT-N.\nACCEPT TXT\nOPEN INPUT F OUTPUT O\nPERFORM STEP-REC UNTIL DONE = 1\nMOVE N TO RESULT-N\nCLOSE F O\nACCEPT TXT\nDISPLAY TXT\nGOBACK.\nSTEP-REC.\nREAD F\nAT END MOVE 1 TO DONE\nNOT AT END\nADD 1 TO N\nWRITE OR-REC FROM IR\nIF N = 3 CALL 'CRASH' END-IF\nEND-READ.\nEND PROGRAM WORK.";
    let module = compile(source).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("checkpoint.json");
    let output = dir.path().join("output.dat");
    let input = dir.path().join("input.dat");
    std::fs::write(&input, zeus_cobol::ebcdic::encode("AABBCCDDEE")).unwrap();
    let mut first = Machine::new(vec![]);
    first.set_dd("INFILE", input);
    first.set_dd("OUTFILE", output.clone());
    first.set_dd("CKPT", checkpoint.clone());
    first.set_stdin(vec!["one".into(), "two".into()]);
    first.add_native("CRASH", |_, _| Err("intentional interruption".into()));
    let error = first.run_main(module).unwrap_err();
    assert!(error.0.contains("intentional interruption"), "{error}");
    assert_eq!(first.checkpoint_files(), &[checkpoint.clone()]);
    drop(first);
    let capture = Capture::default();
    let mut resumed = Machine::new(vec![]);
    resumed.set_stdout(Box::new(capture.clone()));
    resumed.add_native("CRASH", |_, _| Ok(0));
    assert_eq!(resumed.restart_checkpoint(&checkpoint).unwrap(), 0);
    assert_eq!(
        std::fs::read(output).unwrap(),
        zeus_cobol::ebcdic::encode("AABBCCDDEE")
    );
    assert_eq!(&*capture.0.borrow(), b"two\n05\n");
}

#[test]
fn rerun_rejects_invalid_intervals_and_external_or_recursive_files() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO INFILE.\nI-O-CONTROL.\nRERUN ON CKPT EVERY 2 RECORDS OF F.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC XX.\nPROCEDURE DIVISION.\nSTOP RUN.";
    for bad in [
        source.replace("EVERY 2", "EVERY 0"),
        source.replace("EVERY 2", "EVERY 16777216"),
        source.replace("FD F.", "FD F EXTERNAL."),
        source.replace("PROGRAM-ID. T.", "PROGRAM-ID. T RECURSIVE."),
        source.replace("ON CKPT", "ON INFILE"),
        source.replace("OF F.", "OF F\nRERUN ON OTHER EVERY 2 RECORDS OF F."),
        source.replace("OF F.", "OF F\nRERUN ON OTHER EVERY 3 RECORDS OF F."),
        source.replace("OF F.", "OF F\nRERUN ON OTHER 3 RECORDS OF F."),
    ] {
        assert!(compile(&bad).is_err());
    }
    compile(&source.replace("EVERY 2", "EVERY 16777215")).unwrap();
}

#[test]
fn rerun_optional_keywords_preserve_real_checkpoint_and_restart() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO OUTFILE.\nI-O-CONTROL.\nRERUN ON CKPT EVERY 1 RECORDS OF F.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC X.\nPROCEDURE DIVISION.\nOPEN OUTPUT F\nWRITE R FROM 'A'\nCALL 'CRASH'\nCLOSE F\nDISPLAY R\nSTOP RUN.";
    for clause in ["RERUN CKPT 1 RECORDS F", "RERUN ON CKPT 1 RECORDS OF F"] {
        let dir = tempfile::tempdir().unwrap();
        let checkpoint = dir.path().join("step.ckpt");
        let output = dir.path().join("out.dat");
        let source = source.replace("RERUN ON CKPT EVERY 1 RECORDS OF F", clause);
        let mut first = Machine::new(vec![]);
        first.set_dd("OUTFILE", output.clone());
        first.set_dd("CKPT", checkpoint.clone());
        first.add_native("CRASH", |_, _| Err("interrupted".into()));
        assert!(
            first
                .run_main(compile(&source).unwrap())
                .unwrap_err()
                .0
                .contains("interrupted")
        );
        let capture = Capture::default();
        let mut resumed = Machine::new(vec![]);
        resumed.set_stdout(Box::new(capture.clone()));
        resumed.add_native("CRASH", |_, _| Ok(0));
        assert_eq!(resumed.restart_checkpoint(&checkpoint).unwrap(), 0);
        assert_eq!(&*capture.0.borrow(), b"A\n");
        assert_eq!(
            std::fs::read(output).unwrap(),
            zeus_cobol::ebcdic::encode("A")
        );
    }
    // A record interval and a volume interval may coexist for the same file.
    compile(&source.replace("OF F.", "OF F\nRERUN OTHER END UNIT F.")).unwrap();
}

#[test]
fn end_of_volume_checkpoints_resume_fixed_and_spanned_tape_reads() {
    for (mode, block, capacity, standard) in [
        ("F", 20, 20, true),
        ("S", 13, 13, true),
        ("F", 20, 20, false),
        ("S", 13, 13, false),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let tape = dir.path().join("data.aws");
        let checkpoint = dir.path().join("volume.json");
        let base = format!(
            "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO TAPEFILE.\nDATA DIVISION.\nFILE SECTION.\nFD F RECORDING MODE {mode}\nBLOCK CONTAINS {block} CHARACTERS.\n01 R PIC X(10).\nWORKING-STORAGE SECTION.\n01 N PIC 99 VALUE 0.\n01 DONE PIC 9 VALUE 0.\nPROCEDURE DIVISION.\n"
        );
        let base = if standard {
            base
        } else {
            base.replace("BLOCK CONTAINS", "LABEL RECORDS OMITTED\nBLOCK CONTAINS")
        };
        let output = format!(
            "{base}OPEN OUTPUT F\nWRITE R FROM 'AAAAAAAAAA'\nWRITE R FROM 'BBBBBBBBBB'\nWRITE R FROM 'CCCCCCCCCC'\nWRITE R FROM 'DDDDDDDDDD'\nWRITE R FROM 'EEEEEEEEEE'\nCLOSE F\nSTOP RUN."
        );
        let mut writer = Machine::new(vec![]);
        writer.set_dd("TAPEFILE", tape.clone());
        writer.set_dd_io(
            "TAPEFILE",
            zeus_ds::dd::IoAttributes {
                volume_capacity: Some(capacity),
                ..Default::default()
            },
        );
        writer.run_main(compile(&output).unwrap()).unwrap();
        let input = format!("{base}OPEN INPUT F\nPERFORM UNTIL DONE = 1\nREAD F AT END MOVE 1 TO DONE\nNOT AT END ADD 1 TO N\nIF N = 3 CALL 'CRASH' END-IF\nEND-READ\nEND-PERFORM\nCLOSE F\nDISPLAY N\nSTOP RUN.").replace("DATA DIVISION.", "I-O-CONTROL.\nRERUN ON CKPT EVERY END OF UNIT OF F.\nDATA DIVISION.");
        let mut reader = Machine::new(vec![]);
        reader.set_dd("TAPEFILE", tape);
        reader.set_dd("CKPT", checkpoint.clone());
        reader.add_native("CRASH", |_, _| Err("interrupted".into()));
        assert!(
            reader
                .run_main(compile(&input).unwrap())
                .unwrap_err()
                .0
                .contains("interrupted")
        );
        let capture = Capture::default();
        let mut resumed = Machine::new(vec![]);
        resumed.add_native("CRASH", |_, _| Ok(0));
        resumed.set_stdout(Box::new(capture.clone()));
        assert_eq!(resumed.restart_checkpoint(&checkpoint).unwrap(), 0);
        assert_eq!(&*capture.0.borrow(), b"05\n");
    }
}

#[test]
fn close_reel_checkpoint_restores_pending_tape_sections_and_allows_writes() {
    for standard in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let tape = dir.path().join("output.aws");
        let checkpoint = dir.path().join("reel.json");
        let source = format!(
            "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO TAPEFILE.\nI-O-CONTROL.\nRERUN ON CKPT EVERY END OF UNIT OF F.\nDATA DIVISION.\nFILE SECTION.\nFD F RECORDING MODE F\nLABEL RECORDS {}\nBLOCK CONTAINS 20 CHARACTERS.\n01 R PIC X(10).\nPROCEDURE DIVISION.\nOPEN OUTPUT F\nWRITE R FROM 'AAAAAAAAAA'\nCLOSE F REEL\nCALL 'CRASH'\nWRITE R FROM 'BBBBBBBBBB'\nCLOSE F WITH NO REWIND\nOPEN OUTPUT F WITH NO REWIND\nWRITE R FROM 'CCCCCCCCCC'\nCLOSE F\nSTOP RUN.",
            if standard { "STANDARD" } else { "OMITTED" }
        );
        let mut first = Machine::new(vec![]);
        first.set_dd("TAPEFILE", tape.clone());
        first.set_dd("CKPT", checkpoint.clone());
        first.add_native("CRASH", |_, _| Err("interrupted after reel switch".into()));
        assert!(
            first
                .run_main(compile(&source).unwrap())
                .unwrap_err()
                .0
                .contains("interrupted")
        );
        let mut resumed = Machine::new(vec![]);
        resumed.add_native("CRASH", |_, _| Ok(0));
        assert_eq!(resumed.restart_checkpoint(&checkpoint).unwrap(), 0);
        let first = zeus_ds::tape::decode(&std::fs::read(tape).unwrap()).unwrap();
        let second =
            zeus_ds::tape::decode(&std::fs::read(dir.path().join("output.vol002.aws")).unwrap())
                .unwrap();
        assert_eq!(
            zeus_ds::tape::file_with_labels(&first, 0, standard)
                .unwrap()
                .blocks,
            vec![zeus_cobol::ebcdic::encode("AAAAAAAAAA")]
        );
        let file = zeus_ds::tape::file_with_labels(&second, 0, standard).unwrap();
        assert_eq!(file.blocks, vec![zeus_cobol::ebcdic::encode("BBBBBBBBBB")]);
        let next = zeus_ds::tape::file_with_labels(&second, file.next, standard).unwrap();
        assert_eq!(next.blocks, vec![zeus_cobol::ebcdic::encode("CCCCCCCCCC")]);
    }
}

#[test]
fn sort_checkpoint_resumes_before_output_without_repeating_input() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT S ASSIGN TO SORTWK.\nSELECT O ASSIGN TO OUTFILE.\nI-O-CONTROL.\nRERUN ON SORTCKPT.\nDATA DIVISION.\nFILE SECTION.\nSD S.\n01 SR PIC XX.\nFD O.\n01 OR-REC PIC XX.\nWORKING-STORAGE SECTION.\n01 DONE PIC 9 VALUE 0.\nPROCEDURE DIVISION.\nSORT S ASCENDING KEY SR\nINPUT PROCEDURE FEED OUTPUT PROCEDURE DRAIN\nDISPLAY 'DONE'\nSTOP RUN.\nFEED.\nCALL 'FEED-ONCE'\nRELEASE SR FROM 'CC'\nRELEASE SR FROM 'AA'\nRELEASE SR FROM 'BB'.\nDRAIN.\nCALL 'CRASH'\nOPEN OUTPUT O\nPERFORM UNTIL DONE = 1\nRETURN S AT END MOVE 1 TO DONE\nNOT AT END WRITE OR-REC FROM SR END-RETURN\nEND-PERFORM\nCLOSE O.";
    let dir = tempfile::tempdir().unwrap();
    let checkpoint = dir.path().join("sort.json");
    let output = dir.path().join("out.dat");
    let mut first = Machine::new(vec![dir.path().to_path_buf()]);
    first.set_dd("SORTCKPT", checkpoint.clone());
    first.set_dd("OUTFILE", output.clone());
    first.add_native("FEED-ONCE", |_, _| Ok(0));
    first.add_native("CRASH", |_, _| Err("interrupted sort output".into()));
    let error = first.run_main(compile(source).unwrap()).unwrap_err();
    assert!(error.0.contains("interrupted sort output"), "{error}");
    let capture = Capture::default();
    let mut resumed = Machine::new(vec![]);
    resumed.set_stdout(Box::new(capture.clone()));
    resumed.add_native("CRASH", |_, _| Ok(0));
    resumed.add_native("FEED-ONCE", |_, _| Err("input was repeated".into()));
    resumed.restart_checkpoint(&checkpoint).unwrap();
    assert_eq!(&*capture.0.borrow(), b"DONE\n");
    assert_eq!(
        std::fs::read(output).unwrap(),
        zeus_cobol::ebcdic::encode("AABBCC")
    );
    let sub = compile("ID DIVISION.\nPROGRAM-ID. CRASH.\nPROCEDURE DIVISION.\nGOBACK.").unwrap();
    std::fs::write(
        dir.path().join("CRASH.zo"),
        zeus_cobol::module_bytes(&sub).unwrap(),
    )
    .unwrap();
    let command = std::process::Command::new(env!("CARGO_BIN_EXE_zcobc"))
        .arg("restart")
        .arg(&checkpoint)
        .output()
        .unwrap();
    assert!(
        command.status.success(),
        "{}",
        String::from_utf8_lossy(&command.stderr)
    );
    assert_eq!(command.stdout, b"DONE\n");
}

#[test]
fn checkpoint_dd_cannot_overwrite_a_data_file_through_a_path_alias() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO DATAFILE.\nI-O-CONTROL.\nRERUN ON CKPT EVERY 1 RECORDS OF F.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC XX.\nPROCEDURE DIVISION.\nOPEN OUTPUT F\nWRITE R FROM 'AB'\nCLOSE F\nSTOP RUN.";
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.dat");
    std::fs::write(&path, b"original data").unwrap();
    let mut machine = Machine::new(vec![]);
    machine.set_dd("DATAFILE", path.clone());
    machine.set_dd("CKPT", dir.path().join(".").join("data.dat"));
    let error = machine.run_main(compile(source).unwrap()).unwrap_err();
    assert!(error.0.contains("cannot share a data-file path"), "{error}");
    assert_eq!(std::fs::read(path).unwrap(), b"original data");
}

#[test]
fn cli_restart_anchors_relative_datasets_and_load_libraries() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO OUTFILE.\nI-O-CONTROL.\nRERUN ON CKPT EVERY 1 RECORDS OF F.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC XX.\nPROCEDURE DIVISION.\nOPEN OUTPUT F\nWRITE R FROM 'AA'\nCALL 'FINISH'\nWRITE R FROM 'BB'\nCLOSE F\nSTOP RUN.";
    let original = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::write(
        original.path().join("main.zo"),
        zeus_cobol::module_bytes(&compile(source).unwrap()).unwrap(),
    )
    .unwrap();
    let failed = std::process::Command::new(env!("CARGO_BIN_EXE_zcobc"))
        .current_dir(original.path())
        .args([
            "run",
            "--dd",
            "OUTFILE=out.dat",
            "--dd",
            "CKPT=step.ckpt",
            "main.zo",
        ])
        .output()
        .unwrap();
    assert!(!failed.status.success());
    let sub = compile("ID DIVISION.\nPROGRAM-ID. FINISH.\nPROCEDURE DIVISION.\nGOBACK.").unwrap();
    std::fs::write(
        original.path().join("FINISH.zo"),
        zeus_cobol::module_bytes(&sub).unwrap(),
    )
    .unwrap();
    let resumed = std::process::Command::new(env!("CARGO_BIN_EXE_zcobc"))
        .current_dir(elsewhere.path())
        .arg("restart")
        .arg(original.path().join("step.ckpt"))
        .output()
        .unwrap();
    assert!(
        resumed.status.success(),
        "{}",
        String::from_utf8_lossy(&resumed.stderr)
    );
    assert_eq!(
        std::fs::read(original.path().join("out.dat")).unwrap(),
        zeus_cobol::ebcdic::encode("AABB")
    );
    assert!(!elsewhere.path().join("out.dat").exists());
}

#[test]
fn cli_restart_preserves_program_codepage_and_explicit_runtime_override() {
    let source = "CBL CP(1140) SSRANGE\nID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO OUTFILE.\nI-O-CONTROL.\nRERUN ON CKPT EVERY 1 RECORDS OF F.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC X.\nPROCEDURE DIVISION.\nOPEN OUTPUT F\nWRITE R FROM 'A'\nCALL 'FINISH'\nIF FUNCTION NATIONAL-OF(X'9F') = N'€'\nDISPLAY 'CODEPAGE' ELSE DISPLAY 'OVERRIDE' END-IF\nCLOSE F\nDISPLAY R(2:1)\nSTOP RUN.";
    for override_page in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("main.zo"),
            zeus_cobol::module_bytes(&compile(source).unwrap()).unwrap(),
        )
        .unwrap();
        let mut first = std::process::Command::new(env!("CARGO_BIN_EXE_zcobc"));
        first.current_dir(dir.path()).arg("run");
        if override_page {
            first.args(["--ccsid", "37"]);
        }
        let result = first
            .args([
                "--dd",
                "OUTFILE=out.dat",
                "--dd",
                "CKPT=step.ckpt",
                "main.zo",
            ])
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&result.stderr).contains("program FINISH not found"));
        let sub =
            compile("ID DIVISION.\nPROGRAM-ID. FINISH.\nPROCEDURE DIVISION.\nGOBACK.").unwrap();
        std::fs::write(
            dir.path().join("FINISH.zo"),
            zeus_cobol::module_bytes(&sub).unwrap(),
        )
        .unwrap();
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_zcobc"))
            .current_dir(dir.path())
            .args(["restart", "step.ckpt"])
            .output()
            .unwrap();
        assert_eq!(
            result.stdout,
            if override_page {
                b"OVERRIDE\n".as_slice()
            } else {
                b"CODEPAGE\n".as_slice()
            }
        );
        assert!(String::from_utf8_lossy(&result.stderr).contains("SSRANGE"));
    }
}

use std::{cell::RefCell, io::Write, rc::Rc};
use zeus_cobol::{front::FrontOptions, vm::Machine, zir::Module};
use zeus_ds::{
    dd::IoAttributes,
    tape::{self, Entry},
};

#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn compile(src: &str) -> Result<Module, zeus_cobol::diag::Diag> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("P.cbl");
    assert!(src.lines().all(|l| l.len() <= 65));
    std::fs::write(
        &path,
        src.lines()
            .map(|l| format!("       {l}\n"))
            .collect::<String>(),
    )
    .unwrap();
    zeus_cobol::compile_file(&path, &FrontOptions::default())
}
fn run(src: &str) -> String {
    let out = Capture::default();
    let mut vm = Machine::new(vec![]);
    vm.set_stdout(Box::new(out.clone()));
    vm.run_main(compile(src).unwrap()).unwrap();
    String::from_utf8(out.0.borrow().clone()).unwrap()
}
fn source(fd: &str, select: &str, procedure: &str) -> String {
    format!(
        "ID DIVISION.\nPROGRAM-ID. P.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO OUTFILE {select}.\nDATA DIVISION.\nFILE SECTION.\nFD F {fd}.\n01 R PIC X(10).\nWORKING-STORAGE SECTION.\n01 N PIC 99 VALUE 10.\nPROCEDURE DIVISION.\n{procedure}\nSTOP RUN."
    )
}
fn file_vm(src: &str, path: &std::path::Path, io: IoAttributes) -> Machine {
    let mut vm = Machine::new(vec![]);
    vm.set_dd("OUTFILE", path.to_owned());
    vm.set_dd_io("OUTFILE", io);
    vm.run_main(compile(src).unwrap()).unwrap();
    vm
}

#[test]
fn recursive_calls_share_ws_but_isolate_locals_linkage_and_performs() {
    let src = "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 99 VALUE 4.\n01 ANSWER PIC 9(4).\nPROCEDURE DIVISION.\nCALL 'FACT' USING N ANSWER\nDISPLAY ANSWER\nCALL 'FACT' USING N ANSWER\nDISPLAY ANSWER\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. FACT RECURSIVE.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 CALLS PIC 99 VALUE 0.\nLOCAL-STORAGE SECTION.\n01 SAVE-N PIC 99 VALUE 0.\n01 CHILD-N PIC 99 VALUE 0.\n01 CHILD-RESULT PIC 9(4) VALUE 0.\nLINKAGE SECTION.\n01 ARG-N PIC 99.\n01 ARG-RESULT PIC 9(4).\nPROCEDURE DIVISION USING ARG-N ARG-RESULT.\nADD 1 TO CALLS\nMOVE ARG-N TO SAVE-N\nPERFORM CALCULATE\nIF ARG-N = 4 DISPLAY CALLS END-IF\nGOBACK.\nCALCULATE.\nIF ARG-N = 0\nMOVE 1 TO ARG-RESULT\nELSE\nCOMPUTE CHILD-N = ARG-N - 1\nCALL 'FACT' USING CHILD-N CHILD-RESULT\nCOMPUTE ARG-RESULT = SAVE-N * CHILD-RESULT\nEND-IF.\nEND PROGRAM FACT.";
    assert_eq!(run(src), "05\n0024\n10\n0024\n");
    let bad = src.replace("FACT RECURSIVE", "FACT");
    assert!(
        Machine::new(vec![])
            .run_main(compile(&bad).unwrap())
            .unwrap_err()
            .to_string()
            .contains("recursive CALL")
    );
}

#[test]
fn recursive_attribute_constraints_are_diagnosed() {
    let src = "ID DIVISION.\nPROGRAM-ID. P RECURSIVE.\nPROCEDURE DIVISION.\nSTOP RUN.\nID DIVISION.\nPROGRAM-ID. CHILD.\nPROCEDURE DIVISION.\nGOBACK.\nEND PROGRAM CHILD.\nEND PROGRAM P.";
    assert!(compile(src).unwrap_err().message.contains("RECURSIVE"));
    assert!(
        compile(
            &src.replace("P RECURSIVE", "P")
                .replace("PROGRAM-ID. CHILD.", "PROGRAM-ID. CHILD RECURSIVE.")
        )
        .is_err()
    );
}

#[test]
fn recursive_reentry_through_an_external_language_frame() {
    fn bridge(vm: &mut Machine, args: &[Option<usize>]) -> Result<i32, String> {
        let end = vm
            .call_program(
                "R",
                &[zeus_cobol::interlang::XRef {
                    addr: args[0],
                    len: 2,
                    ..Default::default()
                }],
            )?
            .unwrap();
        Ok(end.rc)
    }
    let src = "ID DIVISION.\nPROGRAM-ID. P.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 99 VALUE 3.\nPROCEDURE DIVISION.\nCALL 'R' USING N\nSTOP RUN.\nEND PROGRAM P.\nID DIVISION.\nPROGRAM-ID. R RECURSIVE.\nDATA DIVISION.\nLOCAL-STORAGE SECTION.\n01 SAVE-N PIC 99 VALUE 0.\n01 CHILD-N PIC 99 VALUE 0.\nLINKAGE SECTION.\n01 N PIC 99.\nPROCEDURE DIVISION USING N.\nMOVE N TO SAVE-N\nIF N > 0\nCOMPUTE CHILD-N = N - 1\nCALL 'BRIDGE' USING CHILD-N\nEND-IF\nDISPLAY SAVE-N\nGOBACK.\nEND PROGRAM R.";
    let module = compile(src).unwrap();
    let module = serde_json::from_slice(&serde_json::to_vec(&module).unwrap()).unwrap();
    let out = Capture::default();
    let mut vm = Machine::new(vec![]);
    vm.set_stdout(Box::new(out.clone()));
    vm.add_native_machine("BRIDGE", bridge);
    vm.run_main(module).unwrap();
    assert_eq!(
        String::from_utf8(out.0.borrow().clone()).unwrap(),
        "00\n01\n02\n03\n"
    );
}

#[test]
fn reserve_overrides_dd_bufno_and_buffers_survive_until_close() {
    let dir = tempfile::tempdir().unwrap();
    for (reserve, dd, expected) in [
        ("RESERVE 3 AREAS", Some(5), 3),
        ("", Some(5), 5),
        ("", None, 2),
    ] {
        let src = source(
            "BLOCK CONTAINS 20 CHARACTERS",
            reserve,
            "OPEN OUTPUT F\nWRITE R FROM 'ABCDEFGHIJ'\nCLOSE F",
        );
        let vm = file_vm(
            &src,
            &dir.path().join("out"),
            IoAttributes {
                buffers: dd,
                ..Default::default()
            },
        );
        assert_eq!(
            vm.opened_buffer_counts(),
            &[("OUTFILE".into(), expected, 20)]
        );
    }
    assert!(
        compile(&source(
            "",
            "ORGANIZATION LINE SEQUENTIAL\nRESERVE 1 AREA",
            "GOBACK"
        ))
        .is_err()
    );
}

#[test]
fn spanned_records_roundtrip_as_physical_blocks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.dat");
    let src = source(
        "RECORDING MODE S BLOCK CONTAINS 12 CHARACTERS",
        "",
        "OPEN OUTPUT F\nWRITE R FROM 'ABCDEFGHIJ'\nCLOSE F",
    );
    file_vm(&src, &path, Default::default());
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(&bytes[..8], b"\0\x0c\0\0\0\x08\x01\0");
    assert_eq!(bytes.len(), 34);
    let input = source(
        "RECORDING MODE S BLOCK CONTAINS 12 CHARACTERS",
        "",
        "OPEN INPUT F\nREAD F\nIF R NOT = 'ABCDEFGHIJ' MOVE 9 TO RETURN-CODE END-IF\nCLOSE F",
    );
    let mut vm = Machine::new(vec![]);
    vm.set_dd("OUTFILE", path);
    assert_eq!(vm.run_main(compile(&input).unwrap()).unwrap(), 0);
}

#[test]
fn undefined_records_keep_exact_block_lengths_on_aws_tape() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("u.aws");
    let fd = "RECORDING MODE U\nRECORD VARYING FROM 1 TO 10 DEPENDING N\nLABEL RECORDS OMITTED";
    let src = source(
        fd,
        "",
        "OPEN OUTPUT F\nMOVE 2 TO N\nWRITE R FROM 'AB'\nMOVE 5 TO N\nWRITE R FROM 'CDEFG'\nCLOSE F",
    );
    file_vm(&src, &path, Default::default());
    let entries = tape::decode(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(
        entries,
        vec![
            Entry::Block(vec![0xc1, 0xc2]),
            Entry::Block(vec![0xc3, 0xc4, 0xc5, 0xc6, 0xc7]),
            Entry::Mark,
            Entry::Mark
        ]
    );
    let input = source(
        fd,
        "",
        "OPEN INPUT F\nREAD F\nIF N NOT = 2 MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF N NOT = 5 MOVE 9 TO RETURN-CODE END-IF\nCLOSE F",
    );
    let mut vm = Machine::new(vec![]);
    vm.set_dd("OUTFILE", path);
    assert_eq!(vm.run_main(compile(&input).unwrap()).unwrap(), 0);
}

#[test]
fn tape_marks_no_rewind_reversed_and_standard_labels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f.aws");
    let fd = "RECORDING MODE F BLOCK CONTAINS 20 CHARACTERS\nLABEL RECORDS STANDARD";
    let src = source(
        fd,
        "",
        "OPEN OUTPUT F\nWRITE R FROM 'ABCDEFGHIJ'\nWRITE R FROM 'KLMNOPQRST'\nCLOSE F WITH NO REWIND\nOPEN OUTPUT F WITH NO REWIND\nWRITE R FROM 'UVWXYZ1234'\nCLOSE F\nOPEN INPUT F REVERSED\nREAD F\nIF R NOT = 'KLMNOPQRST' MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'ABCDEFGHIJ' MOVE 9 TO RETURN-CODE END-IF\nCLOSE F WITH NO REWIND\nOPEN INPUT F WITH NO REWIND\nREAD F\nIF R NOT = 'UVWXYZ1234' MOVE 9 TO RETURN-CODE END-IF\nCLOSE F",
    );
    let mut vm = Machine::new(vec![]);
    vm.set_dd("OUTFILE", path.clone());
    assert_eq!(vm.run_main(compile(&src).unwrap()).unwrap(), 0);
    let entries = tape::decode(&std::fs::read(&path).unwrap()).unwrap();
    let Entry::Block(vol) = &entries[0] else {
        panic!()
    };
    assert_eq!(&vol[..10], &zeus_cobol::ebcdic::encode("VOL1ZEUS01"));
    assert_eq!(tape::file(&entries, 0).unwrap().blocks.len(), 1);
    let next = tape::file(&entries, 0).unwrap().next;
    assert_eq!(tape::file(&entries, next).unwrap().blocks.len(), 1);
    let Entry::Block(header) = &entries[next] else {
        panic!()
    };
    assert_eq!(&header[31..35], &zeus_cobol::ebcdic::encode("0002"));
    let input = source(
        fd,
        "",
        "OPEN INPUT F\nREAD F\nIF R NOT = 'UVWXYZ1234' MOVE 9 TO RETURN-CODE END-IF\nCLOSE F",
    );
    let mut vm = Machine::new(vec![]);
    vm.set_dd("OUTFILE", path);
    vm.set_dd_io(
        "OUTFILE",
        IoAttributes {
            file_sequence: Some(2),
            ..Default::default()
        },
    );
    assert_eq!(vm.run_main(compile(&input).unwrap()).unwrap(), 0);
}

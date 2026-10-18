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

#[test]
fn cataloged_dataset_environment_uses_catalog_record_attributes() {
    use zeus_ds::catalog::{Catalog, Entry as CatalogEntry, EntryKind, Recfm};
    let dir = tempfile::tempdir().unwrap();
    let mut catalog = Catalog::open(dir.path()).unwrap();
    catalog
        .define(CatalogEntry {
            name: "APP.INPUT".into(),
            kind: EntryKind::Sequential {
                recfm: Recfm::Fb,
                lrecl: 10,
            },
        })
        .unwrap();
    std::fs::write(
        catalog.path("APP.INPUT"),
        zeus_cobol::ebcdic::encode("ABCDEFGHIJ"),
    )
    .unwrap();
    catalog.save().unwrap();
    let mut vm = Machine::new(vec![]);
    vm.set_dynamic_catalog(dir.path().to_path_buf());
    vm.set_file_environment("OUTFILE", "DSN(APP.INPUT),SHR".into());
    assert_eq!(vm.run_main(compile(&source("", "", "OPEN INPUT F\nREAD F\nIF R NOT = 'ABCDEFGHIJ' MOVE 1 TO RETURN-CODE END-IF\nCLOSE F")).unwrap()).unwrap(), 0);
}

#[test]
fn allocation_environment_is_reevaluated_at_open_and_dd_takes_precedence() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.dat");
    let second = dir.path().join("second.dat");
    let mut vm = Machine::new(vec![]);
    vm.set_file_environment("OUTFILE", format!("PATH({})", first.display()));
    vm.add_native_machine("CHANGE", |machine, args| {
        let address = args[0].unwrap();
        let value = zeus_cobol::ebcdic::decode_latin1(&machine.storage()[address..address + 512]);
        let value = String::from_utf8(value).unwrap().trim_end().to_string();
        machine.set_file_environment("OUTFILE", value);
        Ok(0)
    });
    let program = source("", "", "OPEN OUTPUT F\nWRITE R FROM 'ABCDEFGHIJ'\nCLOSE F\nCALL 'CHANGE' USING NEXT-PATH\nOPEN OUTPUT F\nWRITE R FROM 'KLMNOPQRST'\nCLOSE F")
        .replace("PROCEDURE DIVISION.", "01 NEXT-PATH PIC X(512).\nPROCEDURE DIVISION.\nACCEPT NEXT-PATH");
    vm.set_stdin(vec![format!("PATH({})", second.display())]);
    assert_eq!(vm.run_main(compile(&program).unwrap()).unwrap(), 0);
    assert_eq!(
        std::fs::read(&first).unwrap(),
        zeus_cobol::ebcdic::encode("ABCDEFGHIJ")
    );
    assert_eq!(
        std::fs::read(&second).unwrap(),
        zeus_cobol::ebcdic::encode("KLMNOPQRST")
    );
    for invalid in ["", "not-an-allocation", "PATH(relative)", "DSN(&&TEMP),OLD"] {
        let mut vm = Machine::new(vec![]);
        vm.set_file_environment("OUTFILE", invalid.into());
        let program = source(
            "",
            "FILE STATUS N",
            "OPEN INPUT F\nIF N NOT = 98 MOVE 1 TO RETURN-CODE END-IF",
        );
        assert_eq!(
            vm.run_main(compile(&program).unwrap()).unwrap(),
            0,
            "{invalid}"
        );
    }
    let mut vm = Machine::new(vec![]);
    vm.set_dd("OUTFILE", first);
    vm.set_file_environment("OUTFILE", "invalid".into());
    assert_eq!(vm.run_main(compile(&source("", "", "OPEN INPUT F\nREAD F\nIF R NOT = 'ABCDEFGHIJ' MOVE 1 TO RETURN-CODE END-IF\nCLOSE F")).unwrap()).unwrap(), 0);
}

#[test]
fn protection_on_a_continuation_volume_is_checked_before_reading() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.aws");
    let second = dir.path().join("second.aws");
    let io = IoAttributes {
        volumes: vec![first.clone(), second.clone()],
        volume_capacity: Some(20),
        ..Default::default()
    };
    let fd = "RECORDING MODE F LABEL RECORDS STANDARD\nBLOCK CONTAINS 20 CHARACTERS";
    file_vm(
        &source(
            fd,
            "",
            "OPEN OUTPUT F\nPERFORM 4 TIMES WRITE R FROM 'ABCDEFGHIJ' END-PERFORM\nCLOSE F",
        ),
        &first,
        io.clone(),
    );
    let mut entries = tape::decode(&std::fs::read(&second).unwrap()).unwrap();
    for entry in &mut entries {
        if let Entry::Block(b) = entry
            && b.starts_with(&zeus_cobol::ebcdic::encode("HDR1"))
        {
            b[53] = zeus_cobol::ebcdic::encode("1")[0];
        }
    }
    std::fs::write(second, tape::encode(&entries).unwrap()).unwrap();
    file_vm(
        &source(
            fd,
            "FILE STATUS N",
            "OPEN INPUT F\nIF N NOT = 37 MOVE 1 TO RETURN-CODE END-IF",
        ),
        &first,
        io.clone(),
    );
    let mut vm = Machine::new(vec![]);
    vm.set_dd("OUTFILE", first);
    vm.set_dd_io("OUTFILE", io);
    vm.set_tape_authorizer(|request| !request.writing && request.security == 1);
    assert_eq!(
        vm.run_main(
            compile(&source(
                fd,
                "",
                "OPEN INPUT F\nPERFORM 4 TIMES READ F END-PERFORM\nCLOSE F"
            ))
            .unwrap()
        )
        .unwrap(),
        0
    );
}

#[test]
fn tape_retention_and_password_levels_require_host_authorization() {
    for (security, expiration) in [
        (0, Some("2155/365")),
        (0, Some("99365")),
        (1, None),
        (3, None),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("protected.aws");
        let fd = "RECORDING MODE F LABEL RECORDS STANDARD";
        file_vm(
            &source(fd, "", "OPEN OUTPUT F\nWRITE R FROM 'ABCDEFGHIJ'\nCLOSE F"),
            &path,
            IoAttributes {
                tape_security: security,
                expiration: expiration.map(|v| tape::Expiration::from_jcl(v).unwrap()),
                ..Default::default()
            },
        );
        let original = std::fs::read(&path).unwrap();
        let input = if security == 1 {
            "OPEN INPUT F\nIF N NOT = 37 MOVE 1 TO RETURN-CODE END-IF"
        } else {
            "OPEN INPUT F\nIF N NOT = 00 MOVE 1 TO RETURN-CODE END-IF\nCLOSE F"
        };
        file_vm(
            &source(fd, "FILE STATUS N", input),
            &path,
            IoAttributes::default(),
        );
        file_vm(
            &source(
                fd,
                "FILE STATUS N",
                "OPEN OUTPUT F\nIF N NOT = 37 MOVE 1 TO RETURN-CODE END-IF",
            ),
            &path,
            IoAttributes::default(),
        );
        assert_eq!(std::fs::read(&path).unwrap(), original);
        let mut vm = Machine::new(vec![]);
        vm.set_dd("OUTFILE", path.clone());
        vm.set_tape_authorizer(move |request| {
            assert_eq!(request.dataset, "OUTFILE");
            assert!(request.writing);
            assert!(request.unexpired || request.security != 0);
            true
        });
        assert_eq!(
            vm.run_main(
                compile(&source(
                    fd,
                    "",
                    "OPEN OUTPUT F\nWRITE R FROM 'REPLACED!!'\nCLOSE F"
                ))
                .unwrap()
            )
            .unwrap(),
            0
        );
        assert_ne!(std::fs::read(path).unwrap(), original);
    }
}

#[test]
fn tape_retention_period_is_written_and_capped_at_2155() {
    for days in [0, 1, 93000] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("retention.aws");
        file_vm(
            &source(
                "LABEL RECORDS STANDARD",
                "",
                "OPEN OUTPUT F\nWRITE R\nCLOSE F",
            ),
            &path,
            IoAttributes {
                retention_days: Some(days),
                ..Default::default()
            },
        );
        let entries = tape::decode(&std::fs::read(path).unwrap()).unwrap();
        let Entry::Block(header) = &entries[1] else {
            panic!("HDR1 missing")
        };
        let header = zeus_cobol::ebcdic::decode_latin1(header);
        let expiration = tape::Expiration::from_label(&header[47..53])
            .unwrap()
            .unwrap();
        if days == 93000 {
            assert_eq!(
                expiration,
                tape::Expiration::Date {
                    year: 2155,
                    day: 365
                }
            );
        }
    }
}

#[test]
fn vse_style_tape_without_label_two_uses_fd_and_physical_boundaries() {
    for mode in ['F', 'V', 'S', 'U'] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("vse.aws");
        let fd = format!(
            "RECORDING MODE {mode} LABEL RECORDS STANDARD{}",
            if mode == 'U' {
                ""
            } else {
                "\nBLOCK CONTAINS 40 CHARACTERS"
            }
        );
        file_vm(
            &source(
                &fd,
                "",
                "OPEN OUTPUT F\nWRITE R FROM 'ABCDEFGHIJ'\nWRITE R FROM 'KLMNOPQRST'\nCLOSE F",
            ),
            &path,
            IoAttributes::default(),
        );
        let mut entries = tape::decode(&std::fs::read(&path).unwrap()).unwrap();
        entries.retain(|entry| {
            !matches!(entry, Entry::Block(b)
            if b.starts_with(&zeus_cobol::ebcdic::encode("HDR2"))
                || b.starts_with(&zeus_cobol::ebcdic::encode("EOF2")))
        });
        std::fs::write(&path, tape::encode(&entries).unwrap()).unwrap();
        let fd = if mode == 'S' {
            fd
        } else {
            format!("RECORDING MODE {mode} LABEL RECORDS STANDARD")
        };
        file_vm(
            &source(
                &fd,
                "",
                "OPEN INPUT F\nREAD F\nIF R NOT = 'ABCDEFGHIJ' MOVE 1 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'KLMNOPQRST' MOVE 2 TO RETURN-CODE END-IF\nCLOSE F",
            ),
            &path,
            IoAttributes::default(),
        );
    }
}

#[test]
fn large_tape_blocks_round_trip_merge_labels_and_reject_io() {
    for mode in ['F', 'V'] {
        for automatic in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("large.aws");
            let block = if automatic { 0 } else { 65536 };
            let fd = format!(
                "RECORDING MODE {mode} LABEL RECORDS STANDARD\nBLOCK CONTAINS {block} CHARACTERS"
            );
            let output = source(
                &fd,
                "",
                "OPEN OUTPUT F\nPERFORM 2200 TIMES WRITE R FROM 'ABCDEFGHIJ' END-PERFORM\nCLOSE F",
            )
            .replace("PIC X(10)", "PIC X(32)");
            file_vm(
                &output,
                &path,
                IoAttributes {
                    block_size_limit: automatic.then_some(65536),
                    ..Default::default()
                },
            );
            let entries = tape::decode(&std::fs::read(&path).unwrap()).unwrap();
            let file = tape::file(&entries, 0).unwrap();
            assert_eq!(file.blocks.len(), 2);
            assert!(file.blocks[0].len() > 32760);
            let Entry::Block(header) = &entries[2] else {
                panic!("HDR2 missing")
            };
            let header = zeus_cobol::ebcdic::decode_latin1(header);
            assert_eq!(&header[5..10], b"00000");
            assert_eq!(&header[70..80], b"0000065536");
            let input_fd = format!("RECORDING MODE {mode} LABEL RECORDS STANDARD");
            let input = source(&input_fd, "", "OPEN INPUT F\nPERFORM 2200 TIMES\nREAD F\nIF R(1:10) NOT = 'ABCDEFGHIJ'\nMOVE 1 TO RETURN-CODE END-IF\nEND-PERFORM\nCLOSE F").replace("PIC X(10)", "PIC X(32)");
            file_vm(&input, &path, IoAttributes::default());
            let io = source(
                &input_fd,
                "FILE STATUS N",
                "OPEN I-O F\nIF N NOT = 37 MOVE 1 TO RETURN-CODE END-IF",
            )
            .replace("PIC X(10)", "PIC X(32)");
            file_vm(&io, &path, IoAttributes::default());
        }
    }
}

#[test]
fn large_blocks_on_direct_access_report_status_ninety() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("disk.dat");
    let src = source(
        "RECORDING MODE F BLOCK CONTAINS 32770 CHARACTERS",
        "FILE STATUS N",
        "OPEN OUTPUT F\nIF N NOT = 90 MOVE 1 TO RETURN-CODE END-IF",
    );
    file_vm(&src, &path, IoAttributes::default());
    assert!(!path.exists());
}

#[test]
fn extended_dataset_sequence_on_the_first_mounted_label_controls_positioning() {
    for sequence in [10000, 65535] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("extended.aws");
        let fd = "RECORDING MODE F LABEL RECORDS STANDARD";
        file_vm(
            &source(fd, "", "OPEN OUTPUT F\nWRITE R FROM 'ABCDEFGHIJ'\nCLOSE F"),
            &path,
            IoAttributes::default(),
        );
        let mut entries = tape::decode(&std::fs::read(&path).unwrap()).unwrap();
        for entry in &mut entries {
            if let Entry::Block(b) = entry
                && (b.starts_with(&zeus_cobol::ebcdic::encode("HDR1"))
                    || b.starts_with(&zeus_cobol::ebcdic::encode("EOF1")))
            {
                tape::write_data_sequence(b, sequence).unwrap();
            }
        }
        std::fs::write(&path, tape::encode(&entries).unwrap()).unwrap();
        for requested in [sequence, 1] {
            file_vm(
                &source(
                    fd,
                    "",
                    "OPEN INPUT F\nREAD F\nIF R NOT = 'ABCDEFGHIJ' MOVE 1 TO RETURN-CODE END-IF\nCLOSE F",
                ),
                &path,
                IoAttributes {
                    file_sequence: Some(requested),
                    ..Default::default()
                },
            );
        }
    }
}

#[test]
fn unspecified_and_automatic_tape_blocks_merge_hdr2_and_preserve_extend_layout() {
    for block in ["", "BLOCK CONTAINS 0 CHARACTERS"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("merge.aws");
        file_vm(
            &source(
                "RECORDING MODE F LABEL RECORDS STANDARD\nBLOCK CONTAINS 20 CHARACTERS",
                "",
                "OPEN OUTPUT F\nWRITE R FROM 'ABCDEFGHIJ'\nWRITE R FROM 'KLMNOPQRST'\nCLOSE F",
            ),
            &path,
            IoAttributes::default(),
        );
        let fd = format!("RECORDING MODE F LABEL RECORDS STANDARD\n{block}");
        file_vm(
            &source(
                &fd,
                "",
                "OPEN INPUT F\nREAD F\nIF R NOT = 'ABCDEFGHIJ' MOVE 1 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'KLMNOPQRST' MOVE 2 TO RETURN-CODE END-IF\nCLOSE F\nOPEN EXTEND F\nWRITE R FROM 'UVWXYZ1234'\nCLOSE F",
            ),
            &path,
            IoAttributes::default(),
        );
        let entries = tape::decode(&std::fs::read(path).unwrap()).unwrap();
        let file = tape::file_with_labels(&entries, 0, true).unwrap();
        assert_eq!(
            file.blocks.iter().map(Vec::len).collect::<Vec<_>>(),
            vec![20, 10]
        );
    }
}

#[test]
fn tape_volume_order_diagnostics_are_nonfatal_but_requested_serials_are_checked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("set.aws");
    let attrs = IoAttributes {
        tape: true,
        volume_capacity: Some(10),
        ..Default::default()
    };
    let fd = "RECORDING MODE F LABEL RECORDS STANDARD\nBLOCK CONTAINS 10 CHARACTERS";
    file_vm(
        &source(
            fd,
            "",
            "OPEN OUTPUT F\nWRITE R FROM 'ABCDEFGHIJ'\nWRITE R FROM 'KLMNOPQRST'\nCLOSE F",
        ),
        &path,
        attrs.clone(),
    );
    let second = dir.path().join("set.vol002.aws");
    let mut entries = tape::decode(&std::fs::read(&second).unwrap()).unwrap();
    for entry in &mut entries {
        if let Entry::Block(b) = entry {
            if b.starts_with(&zeus_cobol::ebcdic::encode("HDR1"))
                || b.starts_with(&zeus_cobol::ebcdic::encode("EOF1"))
            {
                b[21..31].copy_from_slice(&zeus_cobol::ebcdic::encode("OTHERV0009"));
                b[35..41].fill(0xff); // Generation/version are not input checks.
            }
            if b.starts_with(&zeus_cobol::ebcdic::encode("VOL1")) {
                b[20] = 0xff;
            }
        }
    }
    std::fs::write(second, tape::encode(&entries).unwrap()).unwrap();
    let output = Capture::default();
    let mut vm = Machine::new(vec![]);
    vm.set_stdout(Box::new(output.clone()));
    vm.set_dd("OUTFILE", path.clone());
    vm.set_dd_io(
        "OUTFILE",
        IoAttributes {
            volume_serials: vec!["ZEUS01".into(), "ZEUS02".into()],
            ..attrs.clone()
        },
    );
    let input = source(
        fd,
        "",
        "OPEN INPUT F\nREAD F\nIF R NOT = 'ABCDEFGHIJ' MOVE 1 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'KLMNOPQRST' MOVE 2 TO RETURN-CODE END-IF\nCLOSE F",
    );
    assert_eq!(vm.run_main(compile(&input).unwrap()).unwrap(), 0);
    assert!(
        String::from_utf8(output.0.borrow().clone())
            .unwrap()
            .contains("IEC709I")
    );
    let mut vm = Machine::new(vec![]);
    vm.set_dd("OUTFILE", path);
    vm.set_dd_io(
        "OUTFILE",
        IoAttributes {
            volume_serials: vec!["WRONG".into()],
            ..attrs
        },
    );
    let input = source(
        fd,
        "FILE STATUS N",
        "OPEN INPUT F\nIF N NOT = 39 MOVE 3 TO RETURN-CODE END-IF",
    );
    assert_eq!(vm.run_main(compile(&input).unwrap()).unwrap(), 0);
}

#[test]
fn reserve_checks_the_documented_buffer_limit() {
    let src = source("RECORDING MODE F", "RESERVE 255 AREAS", "STOP RUN");
    compile(&src).unwrap();
    assert!(
        compile(&src.replace("RESERVE 255", "RESERVE 256"))
            .unwrap_err()
            .message
            .contains("255")
    );
}

#[test]
fn standard_tape_labels_use_external_dataset_tail_and_undefined_zero_length() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("labels.aws");
    let dataset = "LONG.PREFIX.TEST.UNDEFINED";
    let attrs = IoAttributes {
        dataset_name: Some(dataset.into()),
        tape: true,
        block_size: Some(20),
        ..IoAttributes::default()
    };
    let src = source("RECORDING MODE U LABEL RECORDS STANDARD", "FILE STATUS FS", "OPEN OUTPUT F\nWRITE R FROM 'ABC'\nCLOSE F\nOPEN INPUT F\nREAD F\nIF R NOT = 'ABC' MOVE 1 TO RETURN-CODE END-IF\nCLOSE F").replace("01 N PIC 99 VALUE 10.","01 N PIC 99 VALUE 10.\n01 FS PIC XX.");
    let mut vm = Machine::new(vec![]);
    vm.set_dd("OUTFILE", path.clone());
    vm.set_dd_io("OUTFILE", attrs.clone());
    assert_eq!(vm.run_main(compile(&src).unwrap()).unwrap(), 0);
    let entries = tape::decode(&std::fs::read(&path).unwrap()).unwrap();
    for entry in &entries {
        if let Entry::Block(b) = entry {
            let id = zeus_cobol::ebcdic::decode(&b[..b.len().min(4)]);
            if id == "HDR1" || id == "EOF1" {
                assert_eq!(
                    zeus_cobol::ebcdic::decode(&b[4..21]),
                    &dataset[dataset.len() - 17..]
                );
                assert_eq!(b[35..41], [0x40; 6]);
            }
            if id == "HDR2" || id == "EOF2" {
                assert_eq!(zeus_cobol::ebcdic::decode(&b[10..15]), "00000");
            }
        }
    }
    let mut vm = Machine::new(vec![]);
    vm.set_dd("OUTFILE", path);
    vm.set_dd_io(
        "OUTFILE",
        IoAttributes {
            dataset_name: Some("WRONG.DATASET".into()),
            ..attrs
        },
    );
    let src = source(
        "RECORDING MODE U LABEL RECORDS STANDARD",
        "FILE STATUS FS",
        "OPEN INPUT F\nIF FS NOT = '39' MOVE 1 TO RETURN-CODE END-IF",
    )
    .replace(
        "01 N PIC 99 VALUE 10.",
        "01 N PIC 99 VALUE 10.\n01 FS PIC XX.",
    );
    assert_eq!(vm.run_main(compile(&src).unwrap()).unwrap(), 0);
}

#[test]
fn allocated_empty_tape_is_initialized_only_by_open_output() {
    for labels in ["STANDARD", "OMITTED"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.aws");
        std::fs::write(&path, []).unwrap();
        let source = format!(
            "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO OUTFILE FILE STATUS FS.\nDATA DIVISION.\nFILE SECTION.\nFD F RECORDING MODE F LABEL RECORDS {labels}.\n01 R PIC XX.\nWORKING-STORAGE SECTION.\n01 FS PIC XX.\nPROCEDURE DIVISION.\nOPEN INPUT F\nDISPLAY FS\nOPEN OUTPUT F\nWRITE R FROM 'AB'\nCLOSE F\nOPEN INPUT F\nREAD F\nDISPLAY R\nCLOSE F\nSTOP RUN."
        );
        let output = Capture::default();
        let mut vm = Machine::new(vec![]);
        vm.set_dd("OUTFILE", path);
        vm.set_stdout(Box::new(output.clone()));
        vm.run_main(compile(&source).unwrap()).unwrap();
        assert_eq!(&*output.0.borrow(), b"39\nAB\n", "{labels}");
    }
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
    assert_eq!(vm.run_main(compile(src).unwrap()).unwrap(), 0);
    vm
}

#[test]
fn multiple_datasets_share_capacity_without_empty_continuation_sections() {
    for standard in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("files.aws");
        let fd = format!(
            "{}\nRECORDING MODE F BLOCK CONTAINS 20 CHARACTERS",
            if standard {
                "LABEL RECORDS STANDARD"
            } else {
                "LABEL RECORDS OMITTED"
            }
        );
        let output = source(
            &fd,
            "",
            "OPEN OUTPUT F\nWRITE R FROM 'AAAAAAAAAA'\nWRITE R FROM 'BBBBBBBBBB'\nWRITE R FROM 'CCCCCCCCCC'\nCLOSE F WITH NO REWIND\nOPEN OUTPUT F WITH NO REWIND\nWRITE R FROM 'DDDDDDDDDD'\nWRITE R FROM 'EEEEEEEEEE'\nWRITE R FROM 'FFFFFFFFFF'\nWRITE R FROM 'GGGGGGGGGG'\nWRITE R FROM 'HHHHHHHHHH'\nCLOSE F",
        );
        file_vm(
            &output,
            &path,
            IoAttributes {
                volume_capacity: Some(20),
                ..Default::default()
            },
        );
        let mut volumes = vec![];
        for number in 1..=5 {
            let volume = if number == 1 {
                path.clone()
            } else {
                dir.path().join(format!("files.vol{number:03}.aws"))
            };
            volumes.push(tape::decode(&std::fs::read(volume).unwrap()).unwrap());
        }
        let second = tape::file_with_labels(&volumes[1], 0, standard).unwrap();
        assert_eq!(second.blocks.len(), 1);
        assert!(!second.continued);
        assert_eq!(volumes[1].get(second.next), Some(&Entry::Mark));
        if standard {
            let Entry::Block(header) = &volumes[2][1] else {
                panic!()
            };
            assert_eq!(&header[27..31], zeus_cobol::ebcdic::encode("0001"));
            assert_eq!(&header[31..35], zeus_cobol::ebcdic::encode("0002"));
        }
        let input = source(
            &fd,
            "",
            "OPEN INPUT F\nREAD F\nIF R NOT = 'AAAAAAAAAA' MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'BBBBBBBBBB' MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'CCCCCCCCCC' MOVE 9 TO RETURN-CODE END-IF\nREAD F AT END CONTINUE END-READ\nCLOSE F WITH NO REWIND\nOPEN INPUT F WITH NO REWIND\nREAD F\nIF R NOT = 'DDDDDDDDDD' MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'EEEEEEEEEE' MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'FFFFFFFFFF' MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'GGGGGGGGGG' MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'HHHHHHHHHH' MOVE 9 TO RETURN-CODE END-IF\nREAD F AT END CONTINUE END-READ\nCLOSE F",
        );
        file_vm(&input, &path, Default::default());
    }
}

#[test]
fn appending_a_dataset_preserves_prior_reel_labels() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("labels.aws");
    let program = source("LABEL RECORDS UL\nRECORDING MODE F BLOCK CONTAINS 20 CHARACTERS", "",
        "MOVE 0 TO N\nOPEN OUTPUT F\nPERFORM 5 TIMES WRITE R FROM 'AAAAAAAAAA' END-PERFORM\nIF N NOT = 22 MOVE 9 TO RETURN-CODE END-IF\nCLOSE F WITH NO REWIND\nOPEN OUTPUT F WITH NO REWIND\nPERFORM 5 TIMES WRITE R FROM 'BBBBBBBBBB' END-PERFORM\nIF N NOT = 44 MOVE 9 TO RETURN-CODE END-IF\nCLOSE F")
        .replace("01 R PIC X(10).", "01 R PIC X(10).\n01 UL PIC X(80).")
        .replace("PROCEDURE DIVISION.\n", "PROCEDURE DIVISION.\nDECLARATIVES.\nH SECTION.\nUSE AFTER STANDARD BEGINNING REEL LABEL PROCEDURE ON OUTPUT.\nHP.\nMOVE SPACES TO UL\nMOVE 'UHL1' TO UL(1:4)\nADD 1 TO N.\nT SECTION.\nUSE AFTER STANDARD ENDING REEL LABEL PROCEDURE ON OUTPUT.\nTP.\nMOVE SPACES TO UL\nMOVE 'UTL1' TO UL(1:4)\nADD 10 TO N.\nEND DECLARATIVES.\nMAIN SECTION.\n");
    file_vm(
        &program,
        &path,
        IoAttributes {
            volume_capacity: Some(20),
            ..Default::default()
        },
    );
    for number in 1..=6 {
        let volume = if number == 1 {
            path.clone()
        } else {
            dir.path().join(format!("labels.vol{number:03}.aws"))
        };
        let entries = tape::decode(&std::fs::read(volume).unwrap()).unwrap();
        let file = tape::file_with_labels(&entries, 0, true).unwrap();
        assert_eq!(file.continued, matches!(number, 1 | 2 | 4 | 5));
        let has = |id: &str| {
            entries.iter().any(
                |e| matches!(e, Entry::Block(b) if b.starts_with(&zeus_cobol::ebcdic::encode(id))),
            )
        };
        assert_eq!(has("UTL1"), file.continued, "volume {number}");
        assert_eq!(
            has("UHL1"),
            matches!(number, 2 | 3 | 5 | 6),
            "volume {number}"
        );
    }
}

#[test]
fn close_reel_flushes_partial_blocks_and_keeps_files_open() {
    for standard in [false, true] {
        for capacity in [None, Some(20)] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("switch.aws");
            let fd = format!(
                "{}\nRECORDING MODE F BLOCK CONTAINS 20 CHARACTERS",
                if standard {
                    "LABEL RECORDS STANDARD"
                } else {
                    "LABEL RECORDS OMITTED"
                }
            );
            let output = source(
                &fd,
                "FILE STATUS N",
                "OPEN OUTPUT F\nWRITE R FROM 'AAAAAAAAAA'\nCLOSE F REEL WITH NO REWIND\nIF N NOT = 0 MOVE 9 TO RETURN-CODE END-IF\nWRITE R FROM 'BBBBBBBBBB'\nCLOSE F UNIT FOR REMOVAL\nCLOSE F REEL\nWRITE R FROM 'CCCCCCCCCC'\nCLOSE F",
            );
            file_vm(
                &output,
                &path,
                IoAttributes {
                    volume_capacity: capacity,
                    ..Default::default()
                },
            );
            for (number, count) in [(1, 1), (2, 1), (3, 0), (4, 1)] {
                let volume = if number == 1 {
                    path.clone()
                } else {
                    dir.path().join(format!("switch.vol{number:03}.aws"))
                };
                let entries = tape::decode(&std::fs::read(volume).unwrap()).unwrap();
                let file = tape::file_with_labels(&entries, 0, standard).unwrap();
                assert_eq!(file.blocks.len(), count);
                assert!(file.blocks.iter().all(|b| b.len() == 10));
                if standard {
                    assert_eq!(file.continued, number != 4);
                }
            }
            let input = source(
                &fd,
                "FILE STATUS N",
                "OPEN INPUT F\nREAD F\nIF R NOT = 'AAAAAAAAAA' MOVE 9 TO RETURN-CODE END-IF\nCLOSE F REEL\nIF N NOT = 0 MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'BBBBBBBBBB' MOVE 9 TO RETURN-CODE END-IF\nCLOSE F UNIT FOR REMOVAL\nREAD F\nIF R NOT = 'CCCCCCCCCC' MOVE 9 TO RETURN-CODE END-IF\nCLOSE F UNIT\nREAD F AT END CONTINUE END-READ\nIF N NOT = 10 MOVE 9 TO RETURN-CODE END-IF\nCLOSE F",
            );
            file_vm(&input, &path, Default::default());
        }
    }
}

#[test]
fn compiler_awo_and_block0_change_physical_output() {
    for (option, blocks) in [("AWO", 1), ("NOAWO", 2), ("AWO NOAWO", 2)] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("awo.aws");
        let program = format!(
            "CBL {option}\n{}",
            source(
                "RECORDING MODE V\nRECORD VARYING FROM 1 TO 10 DEPENDING N\nBLOCK CONTAINS 32 CHARACTERS",
                "",
                "MOVE 2 TO N\nOPEN OUTPUT F\nPERFORM 4 TIMES WRITE R FROM 'AB' END-PERFORM\nCLOSE F"
            )
        );
        file_vm(&program, &path, Default::default());
        let entries = tape::decode(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(
            tape::file_with_labels(&entries, 0, true)
                .unwrap()
                .blocks
                .len(),
            blocks,
            "{option}"
        );
    }
    for (option, blocks) in [("BLOCK0", 1), ("NOBLOCK0", 3), ("BLOCK0 NOBLOCK0", 3)] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("block.aws");
        let program = format!(
            "CBL {option}\n{}",
            source(
                "RECORDING MODE F",
                "",
                "OPEN OUTPUT F\nPERFORM 3 TIMES WRITE R FROM 'ABCDEFGHIJ' END-PERFORM\nCLOSE F"
            )
        );
        file_vm(&program, &path, Default::default());
        let entries = tape::decode(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(
            tape::file_with_labels(&entries, 0, true)
                .unwrap()
                .blocks
                .len(),
            blocks,
            "{option}"
        );
    }
    let module = compile(&format!(
        "CBL BLOCK0\n{}",
        source("RECORDING MODE U", "", "STOP RUN")
    ))
    .unwrap();
    assert_eq!(module.programs[0].files[0].block_size, None);
}

#[test]
fn reversed_rejects_multireel_files_but_accepts_single_reel_files_in_a_volume_set() {
    for standard in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("reverse.aws");
        let fd = format!(
            "LABEL RECORDS {}\nRECORDING MODE F BLOCK CONTAINS 20 CHARACTERS",
            if standard { "STANDARD" } else { "OMITTED" }
        );
        let writer = source(
            &fd,
            "",
            "OPEN OUTPUT F\nPERFORM 3 TIMES WRITE R FROM 'AAAAAAAAAA' END-PERFORM\nCLOSE F",
        );
        file_vm(
            &writer,
            &path,
            IoAttributes {
                volume_capacity: Some(20),
                ..Default::default()
            },
        );
        let mut vm = Machine::new(vec![]);
        vm.set_dd("OUTFILE", path.clone());
        let reader = source(&fd, "", "OPEN INPUT F REVERSED\nREAD F\nCLOSE F");
        assert!(
            vm.run_main(compile(&reader).unwrap())
                .unwrap_err()
                .0
                .contains("single-reel")
        );
        let writer = source(
            &fd,
            "",
            "OPEN OUTPUT F\nWRITE R FROM 'AAAAAAAAAA'\nWRITE R FROM 'BBBBBBBBBB'\nCLOSE F WITH NO REWIND\nOPEN OUTPUT F WITH NO REWIND\nWRITE R FROM 'CCCCCCCCCC'\nWRITE R FROM 'DDDDDDDDDD'\nCLOSE F",
        );
        file_vm(
            &writer,
            &path,
            IoAttributes {
                volume_capacity: Some(20),
                ..Default::default()
            },
        );
        for (sequence, last) in [(1, "BBBBBBBBBB"), (2, "DDDDDDDDDD")] {
            let reader = source(
                &fd,
                "",
                &format!(
                    "OPEN INPUT F REVERSED\nREAD F\nIF R NOT = '{last}' MOVE 9 TO RETURN-CODE END-IF\nCLOSE F"
                ),
            );
            file_vm(
                &reader,
                &path,
                IoAttributes {
                    file_sequence: Some(sequence),
                    ..Default::default()
                },
            );
        }
    }
}

#[test]
fn tape_volume_boundaries_are_transparent_to_logical_reads() {
    for (fd, capacity) in [
        ("RECORDING MODE F BLOCK CONTAINS 20 CHARACTERS", 20),
        ("RECORDING MODE S BLOCK CONTAINS 13 CHARACTERS", 13),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("multi.aws");
        let output = source(
            fd,
            "",
            "OPEN OUTPUT F\nWRITE R FROM 'ABCDEFGHIJ'\nWRITE R FROM 'KLMNOPQRST'\nWRITE R FROM 'UVWXYZ1234'\nWRITE R FROM '567890ABCD'\nWRITE R FROM 'EFGHIJKLMN'\nCLOSE F",
        );
        file_vm(
            &output,
            &path,
            IoAttributes {
                volume_capacity: Some(capacity),
                ..Default::default()
            },
        );
        let first = tape::decode(&std::fs::read(&path).unwrap()).unwrap();
        assert!(tape::file_with_labels(&first, 0, true).unwrap().continued);
        let second =
            tape::decode(&std::fs::read(dir.path().join("multi.vol002.aws")).unwrap()).unwrap();
        assert!(
            matches!(&second[0], Entry::Block(b) if b.starts_with(&zeus_cobol::ebcdic::encode("VOL1ZEUS02")))
        );
        let input = source(
            fd,
            "",
            "OPEN INPUT F\nREAD F\nIF R NOT = 'ABCDEFGHIJ' MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'KLMNOPQRST' MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'UVWXYZ1234' MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = '567890ABCD' MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'EFGHIJKLMN' MOVE 9 TO RETURN-CODE END-IF\nCLOSE F",
        );
        file_vm(&input, &path, Default::default());
        std::fs::remove_file(dir.path().join("multi.vol002.aws")).unwrap();
        let input = source(
            fd,
            "FILE STATUS N",
            "OPEN INPUT F\nIF N NOT = 39 MOVE 9 TO RETURN-CODE END-IF",
        );
        file_vm(&input, &path, Default::default());
    }
}

#[test]
fn tape_open_checks_dd_labels_and_header_record_attributes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("attrs.aws");
    let fd = "RECORDING MODE F BLOCK CONTAINS 20 CHARACTERS";
    file_vm(
        &source(fd, "", "OPEN OUTPUT F\nWRITE R FROM 'ABCDEFGHIJ'\nCLOSE F"),
        &path,
        Default::default(),
    );
    let input = source(
        fd,
        "FILE STATUS N",
        "OPEN INPUT F\nIF N NOT = 39 MOVE 9 TO RETURN-CODE END-IF",
    );
    file_vm(
        &input,
        &path,
        IoAttributes {
            standard_labels: Some(false),
            ..Default::default()
        },
    );
    let mut entries = tape::decode(&std::fs::read(&path).unwrap()).unwrap();
    if let Entry::Block(header) = &mut entries[2] {
        header[10..15].copy_from_slice(&zeus_cobol::ebcdic::encode("00011"));
    }
    std::fs::write(&path, tape::encode(&entries).unwrap()).unwrap();
    file_vm(&input, &path, Default::default());
}

#[test]
fn tape_trailer_counts_are_checked_and_eov_text_in_payload_is_not_a_label() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("payload.aws");
    let output = source(
        "RECORDING MODE F",
        "",
        "OPEN OUTPUT F\nWRITE R FROM 'EOV1-data'\nCLOSE F",
    )
    .replace("PIC X(10)", "PIC X(80)");
    file_vm(&output, &path, Default::default());
    let input = source(
        "RECORDING MODE F",
        "",
        "OPEN INPUT F\nREAD F\nIF R(1:4) NOT = 'EOV1' MOVE 9 TO RETURN-CODE END-IF\nCLOSE F",
    )
    .replace("PIC X(10)", "PIC X(80)");
    file_vm(&input, &path, Default::default());
    let mut entries = tape::decode(&std::fs::read(&path).unwrap()).unwrap();
    let trailer = entries
        .iter_mut()
        .find_map(|entry| match entry {
            Entry::Block(b) if b.starts_with(&zeus_cobol::ebcdic::encode("EOF1")) => Some(b),
            _ => None,
        })
        .unwrap();
    trailer[54..60].copy_from_slice(&zeus_cobol::ebcdic::encode("000002"));
    std::fs::write(&path, tape::encode(&entries).unwrap()).unwrap();
    let input = source(
        "RECORDING MODE F",
        "FILE STATUS N",
        "OPEN INPUT F\nIF N NOT = 39 MOVE 9 TO RETURN-CODE END-IF",
    )
    .replace("PIC X(10)", "PIC X(80)");
    file_vm(&input, &path, Default::default());
}

#[test]
fn user_labels_have_separate_areas_and_more_labels_reenters_the_declarative() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("labels.aws");
    let fd = "LABEL RECORDS UL RECORDING MODE F";
    let with_labels = |src: String, declarations: &str| {
        src
        .replace("01 R PIC X(10).", "01 R PIC X(10).\n01 UL PIC X(80).")
        .replace("PROCEDURE DIVISION.\n", &format!("PROCEDURE DIVISION.\nDECLARATIVES.\n{declarations}\nEND DECLARATIVES.\nMAIN SECTION.\n"))
    };
    let output = with_labels(
        source(
            fd,
            "",
            "MOVE 0 TO N\nOPEN OUTPUT F\nIF N NOT = 2 MOVE 9 TO RETURN-CODE END-IF\nWRITE R FROM 'ABCDEFGHIJ'\nCLOSE F",
        ),
        "H SECTION.\nUSE AFTER STANDARD BEGINNING FILE LABEL PROCEDURE ON OUTPUT.\nHP.\nMOVE SPACES TO UL\nMOVE 'UHL1' TO UL(1:4)\nIF N = 1 MOVE 'UHL2' TO UL(1:4) END-IF\nADD 1 TO N\nIF N < 2 GO TO MORE-LABELS END-IF.\nT SECTION.\nUSE AFTER STANDARD ENDING FILE LABEL PROCEDURE ON OUTPUT.\nTP.\nMOVE SPACES TO UL\nMOVE 'UTL1' TO UL(1:4).",
    );
    file_vm(&output, &path, Default::default());
    let entries = tape::decode(&std::fs::read(&path).unwrap()).unwrap();
    let file = tape::file_with_labels(&entries, 0, true).unwrap();
    assert_eq!(file.blocks, vec![zeus_cobol::ebcdic::encode("ABCDEFGHIJ")]);
    assert!(
        matches!(&entries[3], Entry::Block(b) if b.starts_with(&zeus_cobol::ebcdic::encode("UHL1")))
    );
    assert!(
        matches!(&entries[4], Entry::Block(b) if b.starts_with(&zeus_cobol::ebcdic::encode("UHL2")))
    );
    let input = with_labels(
        source(
            fd,
            "",
            "MOVE 0 TO N\nOPEN INPUT F\nIF N NOT = 2 MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF R NOT = 'ABCDEFGHIJ' MOVE 9 TO RETURN-CODE END-IF\nREAD F AT END CONTINUE END-READ\nIF N NOT = 12 MOVE 9 TO RETURN-CODE END-IF\nCLOSE F\nIF N NOT = 12 MOVE 9 TO RETURN-CODE END-IF",
        ),
        "H SECTION.\nUSE AFTER STANDARD BEGINNING FILE LABEL PROCEDURE ON INPUT.\nHP.\nIF UL(1:3) NOT = 'UHL' MOVE 9 TO RETURN-CODE END-IF\nADD 1 TO N\nGO TO MORE-LABELS.\nT SECTION.\nUSE AFTER STANDARD ENDING FILE LABEL PROCEDURE ON INPUT.\nTP.\nIF UL(1:4) NOT = 'UTL1' MOVE 9 TO RETURN-CODE END-IF\nADD 10 TO N.",
    );
    file_vm(&input, &path, Default::default());
}

#[test]
fn reel_labels_run_at_each_continuation_and_are_stored_on_the_physical_volume() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reel.aws");
    let labels = |src: String, mode: &str| {
        src
        .replace("01 R PIC X(10).", "01 R PIC X(10).\n01 UL PIC X(80).")
        .replace("PROCEDURE DIVISION.\n", &format!("PROCEDURE DIVISION.\nDECLARATIVES.\nH SECTION.\nUSE AFTER STANDARD BEGINNING REEL LABEL PROCEDURE ON {mode}.\nHP.\nMOVE SPACES TO UL\nMOVE 'UHL1' TO UL(1:4)\nADD 1 TO N.\nT SECTION.\nUSE AFTER STANDARD ENDING UNIT LABEL PROCEDURE ON {mode}.\nTP.\nMOVE SPACES TO UL\nMOVE 'UTL1' TO UL(1:4)\nADD 10 TO N.\nEND DECLARATIVES.\nMAIN SECTION.\n"))
    };
    let fd = "LABEL RECORDS UL\nRECORDING MODE F BLOCK CONTAINS 20 CHARACTERS";
    let output = labels(
        source(
            fd,
            "",
            "MOVE 0 TO N\nOPEN OUTPUT F\nIF N NOT = 0 MOVE 9 TO RETURN-CODE END-IF\nWRITE R FROM 'AAAAAAAAAA'\nWRITE R FROM 'BBBBBBBBBB'\nIF N NOT = 0 MOVE 9 TO RETURN-CODE END-IF\nWRITE R FROM 'CCCCCCCCCC'\nIF N NOT = 11 MOVE 9 TO RETURN-CODE END-IF\nWRITE R FROM 'DDDDDDDDDD'\nWRITE R FROM 'EEEEEEEEEE'\nIF N NOT = 22 MOVE 9 TO RETURN-CODE END-IF\nCLOSE F",
        ),
        "OUTPUT",
    );
    file_vm(
        &output,
        &path,
        IoAttributes {
            volume_capacity: Some(20),
            ..Default::default()
        },
    );
    let first = tape::decode(&std::fs::read(&path).unwrap()).unwrap();
    assert!(first.iter().any(
        |e| matches!(e, Entry::Block(b) if b.starts_with(&zeus_cobol::ebcdic::encode("UTL1")))
    ));
    let second = tape::decode(&std::fs::read(dir.path().join("reel.vol002.aws")).unwrap()).unwrap();
    assert!(second.iter().any(
        |e| matches!(e, Entry::Block(b) if b.starts_with(&zeus_cobol::ebcdic::encode("UHL1")))
    ));
    let input = labels(
        source(
            fd,
            "",
            "MOVE 0 TO N\nOPEN INPUT F\nREAD F\nREAD F\nIF N NOT = 0 MOVE 9 TO RETURN-CODE END-IF\nREAD F\nIF N NOT = 11 MOVE 9 TO RETURN-CODE END-IF\nREAD F\nREAD F\nIF N NOT = 22 MOVE 9 TO RETURN-CODE END-IF\nCLOSE F\nIF N NOT = 22 MOVE 9 TO RETURN-CODE END-IF",
        ),
        "INPUT",
    );
    file_vm(&input, &path, Default::default());
}

#[test]
fn mode_label_declaratives_resolve_identical_common_items_across_files() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO FIRST-TAPE.\nSELECT G ASSIGN TO SECOND-TAPE.\nDATA DIVISION.\nFILE SECTION.\nFD F LABEL RECORDS LF RECORDING MODE F.\n01 RF PIC X.\n01 LF.\n05 LABEL-ID PIC X(4).\n05 LABEL-TEXT PIC X(76).\nFD G LABEL RECORDS LG RECORDING MODE F.\n01 RG PIC X.\n01 LG.\n05 LABEL-ID PIC X(4).\n05 LABEL-TEXT PIC X(76).\nPROCEDURE DIVISION.\nDECLARATIVES.\nH SECTION.\nUSE AFTER STANDARD BEGINNING FILE LABEL PROCEDURE ON OUTPUT.\nHP.\nMOVE 'UHL1' TO LABEL-ID\nMOVE ALL 'A' TO LABEL-TEXT.\nEND DECLARATIVES.\nMAIN SECTION.\nOPEN OUTPUT F G\nWRITE RF FROM 'X'\nWRITE RG FROM 'Y'\nCLOSE F G\nSTOP RUN.";
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.aws");
    let b = dir.path().join("b.aws");
    let mut machine = Machine::new(vec![]);
    machine.set_dd("FIRST-TAPE", a.clone());
    machine.set_dd("SECOND-TAPE", b.clone());
    machine.run_main(compile(source).unwrap()).unwrap();
    for path in [a, b] {
        let entries = tape::decode(&std::fs::read(path).unwrap()).unwrap();
        let Entry::Block(label) = &entries[3] else {
            panic!();
        };
        assert_eq!(&label[..4], zeus_cobol::ebcdic::encode("UHL1"));
        assert_eq!(&label[4..], zeus_cobol::ebcdic::encode(&"A".repeat(76)));
    }
    let bad = source.replacen(
        "05 LABEL-TEXT PIC X(76).",
        "05 LABEL-TEXT PIC X(75).\n05 FILLER PIC X.",
        1,
    );
    assert!(compile(&bad).unwrap_err().message.contains("ambiguous"));
    let duplicate = source.replace("END DECLARATIVES.", "J SECTION.\nUSE AFTER STANDARD BEGINNING FILE LABEL PROCEDURE ON OUTPUT.\nJP.\nCONTINUE.\nEND DECLARATIVES.");
    assert!(
        compile(&duplicate)
            .unwrap_err()
            .message
            .contains("overlapping LABEL")
    );
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

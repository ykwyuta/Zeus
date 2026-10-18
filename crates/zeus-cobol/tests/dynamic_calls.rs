use std::{cell::RefCell, io::Write, path::PathBuf, rc::Rc};
use zeus_cobol::{front::FrontOptions, vm::Machine, zir::Module};

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
fn compile_result(source: &str) -> Result<Module, zeus_cobol::diag::Diag> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.cbl");
    assert!(source.lines().all(|line| line.chars().count() <= 65));
    std::fs::write(
        &path,
        source
            .lines()
            .map(|line| format!("       {line}\n"))
            .collect::<String>(),
    )
    .unwrap();
    zeus_cobol::compile_file(&path, &FrontOptions::default())
}
fn compile(source: &str) -> Module {
    compile_result(source).unwrap()
}
fn object(path: &std::path::Path, module: &Module) {
    std::fs::write(path, zeus_cobol::module_bytes(module).unwrap()).unwrap();
}

thread_local! {
    static REPLACEMENT: RefCell<Option<(PathBuf, Vec<u8>)>> = const { RefCell::new(None) };
    static SIZES: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
}

fn subprogram(tag: &str, start: u8, entry: bool) -> Module {
    let entry = if entry { "GOBACK.\nENTRY 'ALT'.\n" } else { "" };
    compile(&format!(
        "ID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 COUNTER PIC 9 VALUE {start}.\n01 SHARED-COUNT EXTERNAL PIC 99.\nPROCEDURE DIVISION.\n{entry}IF SHARED-COUNT IS NOT NUMERIC\nMOVE 0 TO SHARED-COUNT END-IF\nADD 1 TO COUNTER SHARED-COUNT\nDISPLAY '{tag}' COUNTER '/' SHARED-COUNT\nGOBACK."
    ))
}

#[test]
fn dynam_and_call_identifier_reload_cancelled_objects_and_entry_aliases() {
    for (option, call, cancel, entry, dynamic) in [
        ("NODYNAM", "'SUB'", "SUB", false, false),
        ("DYNAM", "'SUB'", "SUB", false, true),
        ("NODYN", "NAME-FIELD", "SUB", false, true),
        ("DYN", "'ALT'", "ALT", true, true),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("object.zo");
        object(&path, &subprogram("OLD", 0, entry));
        REPLACEMENT.with(|slot| {
            *slot.borrow_mut() = Some((
                path,
                zeus_cobol::module_bytes(&subprogram("NEW", 7, entry)).unwrap(),
            ))
        });
        let source = format!(
            "CBL {option}\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 NAME-FIELD PIC XXX VALUE 'SUB'.\nPROCEDURE DIVISION.\nCALL {call}\nCALL 'REPLACE'\nCANCEL '{cancel}'\nCANCEL '{cancel}'\nCALL {call}\nSTOP RUN."
        );
        let module = compile(&source);
        let module =
            zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
        let output = Capture::default();
        let mut vm = Machine::new(vec![dir.path().to_path_buf()]);
        vm.set_stdout(Box::new(output.clone()));
        vm.add_native("REPLACE", |_, _| {
            REPLACEMENT.with(|slot| {
                let slot = slot.borrow();
                let (path, bytes) = slot.as_ref().unwrap();
                std::fs::write(path, bytes).map_err(|e| e.to_string())
            })?;
            Ok(0)
        });
        vm.run_main(module).unwrap();
        let expected = if dynamic {
            "OLD1/01\nNEW8/02\n"
        } else {
            "OLD1/01\nOLD2/02\n"
        };
        assert_eq!(
            String::from_utf8(output.0.borrow().clone()).unwrap(),
            expected,
            "{option}: {call}"
        );
        REPLACEMENT.with(|slot| slot.borrow_mut().take());
    }
}

#[test]
fn static_dependencies_are_bound_before_first_call() {
    for (option, directive, expected) in [
        ("NODYNAM", "", "OLD1/01\n"),
        ("DYNAM", "", "NEW8/01\n"),
        ("DYNAM", ">>CALLINTERFACE STATIC", "OLD1/01\n"),
        ("NODYNAM", ">>CALLINTERFACE DYNAMIC", "NEW8/01\n"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("SUB.zo");
        object(&path, &subprogram("OLD", 0, false));
        REPLACEMENT.with(|slot| {
            *slot.borrow_mut() = Some((
                path,
                zeus_cobol::module_bytes(&subprogram("NEW", 7, false)).unwrap(),
            ))
        });
        let source = format!(
            "CBL {option}\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\n{directive}\nCALL 'REPLACE'\nCALL 'SUB'\nSTOP RUN."
        );
        let output = Capture::default();
        let mut vm = Machine::new(vec![dir.path().to_path_buf()]);
        vm.set_stdout(Box::new(output.clone()));
        vm.add_native("REPLACE", |_, _| {
            REPLACEMENT.with(|slot| {
                let slot = slot.borrow();
                let (path, bytes) = slot.as_ref().unwrap();
                std::fs::write(path, bytes).map_err(|e| e.to_string())
            })?;
            Ok(0)
        });
        vm.run_main(compile(&source)).unwrap();
        assert_eq!(&*output.0.borrow(), expected.as_bytes());
    }
}

#[test]
fn cancel_resets_contained_programs_and_reuses_private_storage() {
    let source = "CBL DYNAM\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nCALL 'SUB'\nCALL 'SUB'\nCANCEL 'SUB'\nCALL 'SUB'\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nPROCEDURE DIVISION.\nCALL 'CHILD'\nGOBACK.\nID DIVISION.\nPROGRAM-ID. CHILD.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9 VALUE 1.\nPROCEDURE DIVISION.\nDISPLAY N\nADD 1 TO N\nGOBACK.\nEND PROGRAM CHILD.\nEND PROGRAM SUB.";
    let output = Capture::default();
    let mut vm = Machine::new(vec![]);
    vm.set_stdout(Box::new(output.clone()));
    vm.run_main(compile(source)).unwrap();
    assert_eq!(&*output.0.borrow(), b"1\n2\n1\n");

    // INITIAL also cancels the state of contained programs after a return,
    // even though these calls are statically bound.
    let initial_source = source
        .replace("CBL DYNAM", "CBL NODYNAM")
        .replace("PROGRAM-ID. SUB.", "PROGRAM-ID. SUB INITIAL.");
    let output = Capture::default();
    let mut vm = Machine::new(vec![]);
    vm.set_stdout(Box::new(output.clone()));
    vm.run_main(compile(&initial_source)).unwrap();
    assert_eq!(&*output.0.borrow(), b"1\n1\n1\n");

    let dir = tempfile::tempdir().unwrap();
    object(&dir.path().join("SUB.zo"), &subprogram("", 0, false));
    let source = "CBL DYNAM\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 I PIC 999.\nPROCEDURE DIVISION.\nPERFORM VARYING I FROM 1 BY 1 UNTIL I > 100\nCALL 'SUB'\nCANCEL 'SUB'\nCALL 'SIZE'\nEND-PERFORM\nSTOP RUN.";
    SIZES.with(|sizes| sizes.borrow_mut().clear());
    let mut vm = Machine::new(vec![dir.path().to_path_buf()]);
    vm.set_stdout(Box::new(std::io::sink()));
    vm.add_native("SIZE", |memory, _| {
        SIZES.with(|sizes| sizes.borrow_mut().push(memory.len()));
        Ok(0)
    });
    vm.run_main(compile(source)).unwrap();
    SIZES.with(|sizes| {
        let sizes = sizes.borrow();
        assert_eq!(sizes.len(), 100);
        assert!(sizes.last().unwrap() <= &(sizes[0] + 8), "{sizes:?}");
    });
}

#[test]
fn cancel_of_an_active_ancestor_terminates_the_run_unit() {
    let source = "CBL DYNAM\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nCALL 'SUB'\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nPROCEDURE DIVISION.\nCANCEL 'MAIN'\nGOBACK.";
    let error = Machine::new(vec![]).run_main(compile(source)).unwrap_err();
    assert!(error.0.contains("active program"), "{error}");
}

#[test]
fn dynam_rejects_explicit_and_implicit_cics_translation() {
    for options in ["DYNAM CICS", "DYNAM"] {
        let source = format!(
            "CBL {options}\nID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nEXEC CICS RETURN END-EXEC."
        );
        assert!(
            compile_result(&source)
                .unwrap_err()
                .message
                .contains("DYNAM")
        );
    }
}

#[test]
fn set_entry_literal_uses_the_dynam_binding_mode() {
    for (option, expected) in [
        ("NODYNAM", "OLD1/01\nOLD2/02\n"),
        ("DYNAM", "OLD1/01\nOLD1/02\n"),
    ] {
        let dir = tempfile::tempdir().unwrap();
        object(&dir.path().join("SUB.zo"), &subprogram("OLD", 0, false));
        let source = format!(
            "CBL {option}\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 P PROCEDURE-POINTER.\nPROCEDURE DIVISION.\nSET P TO ENTRY 'SUB'\nCALL P\nCANCEL 'SUB'\nSET P TO ENTRY 'SUB'\nCALL P\nSTOP RUN."
        );
        let output = Capture::default();
        let mut vm = Machine::new(vec![dir.path().to_path_buf()]);
        vm.set_stdout(Box::new(output.clone()));
        vm.run_main(compile(&source)).unwrap();
        assert_eq!(&*output.0.borrow(), expected.as_bytes());
    }
}

#[test]
fn cancel_closes_internal_files_and_resets_a_composite_dynamic_object() {
    let dir = tempfile::tempdir().unwrap();
    let source = "ID DIVISION.\nPROGRAM-ID. SUB.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO OUTFILE.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC X.\nWORKING-STORAGE SECTION.\n01 FIRST-TIME PIC 9 VALUE 1.\nPROCEDURE DIVISION.\nIF FIRST-TIME = 1\nOPEN OUTPUT F MOVE 0 TO FIRST-TIME END-IF\nWRITE R FROM 'A'\nCALL 'HELPER'\nGOBACK.\nEND PROGRAM SUB.\nID DIVISION.\nPROGRAM-ID. HELPER.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9 VALUE 1.\nPROCEDURE DIVISION.\nDISPLAY N\nADD 1 TO N\nGOBACK.\nEND PROGRAM HELPER.";
    object(&dir.path().join("SUB.zo"), &compile(source));
    let file = dir.path().join("output.dat");
    REPLACEMENT.with(|slot| *slot.borrow_mut() = Some((file.clone(), vec![])));
    let source = "CBL DYNAM\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nCALL 'SUB'\nCALL 'SUB'\nCANCEL 'SUB'\nCALL 'CHECKFILE'\nCALL 'SUB'\nCANCEL 'SUB'\nSTOP RUN.";
    let output = Capture::default();
    let mut vm = Machine::new(vec![dir.path().to_path_buf()]);
    vm.set_dd("OUTFILE", file.clone());
    vm.set_stdout(Box::new(output.clone()));
    vm.add_native("CHECKFILE", |_, _| {
        REPLACEMENT.with(|slot| {
            let slot = slot.borrow();
            let (path, _) = slot.as_ref().unwrap();
            assert_eq!(
                std::fs::read(path).unwrap(),
                zeus_cobol::ebcdic::encode("AA")
            );
        });
        Ok(0)
    });
    vm.run_main(compile(source)).unwrap();
    assert_eq!(&*output.0.borrow(), b"1\n2\n1\n");
    assert_eq!(
        std::fs::read(file).unwrap(),
        zeus_cobol::ebcdic::encode("A")
    );
}

#[test]
fn implicit_cancel_close_does_not_execute_label_declaratives() {
    for explicit in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let close = if explicit { "CLOSE F\n" } else { "" };
        let source = format!(
            "ID DIVISION.\nPROGRAM-ID. SUB.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO OUTFILE.\nDATA DIVISION.\nFILE SECTION.\nFD F LABEL RECORDS UL RECORDING MODE F.\n01 R PIC X.\n01 UL PIC X(80).\nPROCEDURE DIVISION.\nDECLARATIVES.\nT SECTION.\nUSE AFTER STANDARD ENDING FILE LABEL PROCEDURE ON OUTPUT.\nTP.\nDISPLAY 'LABEL'\nMOVE SPACES TO UL\nMOVE 'UTL1' TO UL(1:4).\nEND DECLARATIVES.\nMAIN SECTION.\nOPEN OUTPUT F\nWRITE R FROM 'A'\n{close}GOBACK."
        );
        object(&dir.path().join("SUB.zo"), &compile(&source));
        let output = Capture::default();
        let mut vm = Machine::new(vec![dir.path().to_path_buf()]);
        vm.set_dd("OUTFILE", dir.path().join("out.aws"));
        vm.set_stdout(Box::new(output.clone()));
        vm.run_main(compile("CBL DYNAM\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nCALL 'SUB'\nCANCEL 'SUB'\nSTOP RUN.")).unwrap();
        assert_eq!(
            &*output.0.borrow(),
            if explicit {
                b"LABEL\n".as_slice()
            } else {
                b"".as_slice()
            }
        );
    }
}

//! CALLINTERFACE is applied after COPY/REPLACE, to individual statements.
use std::{cell::RefCell, io::Write, path::Path, rc::Rc};
use zeus_cobol::{
    front::FrontOptions,
    vm::Machine,
    zir::{Instr, Module},
};

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
fn source(path: &Path, text: &str) {
    assert!(text.lines().all(|line| line.chars().count() <= 65));
    std::fs::write(
        path,
        text.lines()
            .map(|line| format!("       {line}\n"))
            .collect::<String>(),
    )
    .unwrap();
}
fn compile(text: &str) -> Result<Module, zeus_cobol::diag::Diag> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.cbl");
    source(&path, text);
    zeus_cobol::compile_file(&path, &FrontOptions::default())
}
fn counter(name: &str) -> Module {
    compile(&format!("ID DIVISION.\nPROGRAM-ID. {name}.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9 VALUE 0.\nPROCEDURE DIVISION.\nADD 1 TO N\nDISPLAY N\nGOBACK.")).unwrap()
}

#[test]
fn call_and_cancel_validate_program_name_categories() {
    for declaration in [
        "01 TARGET PIC XXX.",
        "01 TARGET PIC AAA.",
        "01 TARGET PIC 999.",
        "01 TARGET.\n 05 PART-NAME PIC XXX.",
    ] {
        for statement in ["CALL TARGET", "CANCEL TARGET"] {
            compile(&format!("ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{declaration}\nPROCEDURE DIVISION.\n{statement}\nSTOP RUN.")).unwrap();
        }
    }
    for declaration in [
        "01 TARGET PIC 999 COMP.",
        "01 TARGET PIC 999 COMP-3.",
        "01 TARGET PIC NNN.",
        "01 TARGET USAGE POINTER.",
        "01 TARGET USAGE INDEX.",
        "01 TARGET COMP-2.",
        "01 TARGET PIC ZZ9.",
    ] {
        for statement in ["CALL TARGET", "CANCEL TARGET"] {
            let error = compile(&format!("ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{declaration}\nPROCEDURE DIVISION.\n{statement}\nSTOP RUN.")).unwrap_err();
            assert!(
                error.message.contains("target requires"),
                "{declaration}/{statement}: {error}"
            );
        }
    }
    for target in ["123", "N'ABC'", "SPACE", "NULL"] {
        for verb in ["CALL", "CANCEL"] {
            assert!(compile(&format!("ID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\n{verb} {target}\nSTOP RUN.")).is_err(), "{verb} {target}");
        }
    }
    for usage in ["PROCEDURE-POINTER", "FUNCTION-POINTER"] {
        let text = format!(
            "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 TARGET USAGE {usage}.\nPROCEDURE DIVISION.\nCALL TARGET\nSTOP RUN."
        );
        compile(&text).unwrap();
        assert!(compile(&text.replace("CALL TARGET", "CANCEL TARGET")).is_err());
    }
}

#[test]
fn dll_exports_sidefiles_entry_storage_and_cancel_residency() {
    for options in ["CBL DLL", "CBL NODLL"] {
        let dir = tempfile::tempdir().unwrap();
        let target = compile("CBL EXPORTALL\nID DIVISION.\nPROGRAM-ID. PRIMARY.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 99 VALUE 0.\nPROCEDURE DIVISION.\nADD 1 TO N\nDISPLAY N\nGOBACK.\nENTRY 'ALTERNATE'.\nADD 10 TO N\nDISPLAY N\nGOBACK.").unwrap();
        zeus_cobol::save_module(&target, &dir.path().join("BUNDLE.zo")).unwrap();
        let side = std::fs::read_to_string(dir.path().join("BUNDLE.zo.x")).unwrap();
        assert!(side.contains("PRIMARY") && side.contains("ALTERNATE"));
        let directive = if options == "CBL DLL" {
            ""
        } else {
            ">>CALLINT DLL\n"
        };
        let main = compile(&format!("{options}\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 P USAGE PROCEDURE-POINTER.\nPROCEDURE DIVISION.\n{directive}CALL 'PRIMARY'\nCANCEL 'PRIMARY'\nCALL 'ALTERNATE'\nSET P TO ENTRY 'PRIMARY'\nCALL P\nCALL 'ALTERNATE'\nSTOP RUN.")).unwrap();
        let main =
            zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&main).unwrap()).unwrap();
        let out = Capture::default();
        let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
        machine.set_stdout(Box::new(out.clone()));
        machine.run_main(main).unwrap();
        assert_eq!(&*out.0.borrow(), b"01\n11\n12\n22\n");
    }
}

#[test]
fn by_value_uses_encoded_character_widths_and_checks_computed_slices() {
    let module = compile("CBL CP(273)\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 TEXT-VALUE PIC XX VALUE 'Zü'.\n01 EDITED-VALUE PIC ZZ9 VALUE 123.\n01 N PIC 9 VALUE 1.\nPROCEDURE DIVISION.\nCALL 'SUB' USING BY VALUE 'ü'\nCALL 'SUB' USING BY VALUE X'D0'\nCALL 'SUB' USING BY VALUE TEXT-VALUE(2:N)\nCALL 'SUB' USING BY VALUE TEXT-VALUE(2:)\nCALL 'SUB' USING BY VALUE EDITED-VALUE(1:1)\nCALL 'SUB' USING BY VALUE SHIFT-IN\nCALL 'SUB' USING BY VALUE SHIFT-OUT\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nLINKAGE SECTION.\n01 C PIC X.\nPROCEDURE DIVISION USING BY VALUE C.\nIF C = X'0F' DISPLAY 'IN'\nELSE IF C = X'0E' DISPLAY 'OUT'\nELSE DISPLAY C END-IF END-IF\nGOBACK.\nEND PROGRAM SUB.").unwrap();
    let out = Capture::default();
    let mut machine = Machine::new(vec![]);
    machine.set_stdout(Box::new(out.clone()));
    machine.run_main(module).unwrap();
    assert_eq!(&*out.0.borrow(), "ü\nü\nü\nü\n1\nIN\nOUT\n".as_bytes());
    let bad = compile("ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 X PIC XX VALUE 'AB'.\n01 N PIC 9 VALUE 2.\nPROCEDURE DIVISION.\nCALL 'SUB' USING BY VALUE X(1:N)\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nLINKAGE SECTION.\n01 C PIC X.\nPROCEDURE DIVISION USING BY VALUE C.\nGOBACK.\nEND PROGRAM SUB.").unwrap();
    assert!(
        Machine::new(vec![])
            .run_main(bad)
            .unwrap_err()
            .to_string()
            .contains("computed width")
    );
}

#[test]
fn binder_combines_private_units_and_preserves_nested_metadata() {
    let mut main = compile("CBL DLL CP(37)\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nDISPLAY FUNCTION WHEN-COMPILED\nCALL 'PRIVATE'\nSTOP RUN.").unwrap();
    main.compiled_at = Some("2011010203040500+0000".into());
    let mut sub = compile("CBL CP(273)\nID DIVISION.\nPROGRAM-ID. PRIVATE.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 G GLOBAL PIC X VALUE 'ü'.\nPROCEDURE DIVISION.\nDISPLAY G\nCALL 'CHILD'\nDISPLAY FUNCTION WHEN-COMPILED\nGOBACK.\nID DIVISION.\nPROGRAM-ID. CHILD.\nPROCEDURE DIVISION.\nDISPLAY G\nGOBACK.\nEND PROGRAM CHILD.\nEND PROGRAM PRIVATE.").unwrap();
    sub.compiled_at = Some("2022020304050600+0000".into());
    let module = zeus_cobol::link::modules(vec![main, sub]).unwrap();
    assert_eq!(module.programs.len(), 3);
    assert_eq!(module.programs[2].parent, Some(1));
    let out = Capture::default();
    let mut machine = Machine::new(vec![]);
    machine.set_stdout(Box::new(out.clone()));
    machine.run_main(module).unwrap();
    assert_eq!(
        &*out.0.borrow(),
        "2011010203040500+0000\nü\nü\n2022020304050600+0000\n".as_bytes()
    );
    assert!(
        zeus_cobol::link::modules(vec![counter("DUP"), counter("DUP")])
            .unwrap_err()
            .contains("duplicate bound")
    );
}

#[test]
fn entry_pointer_remembers_its_binding_when_the_directive_changes() {
    let dir = tempfile::tempdir().unwrap();
    save(&dir.path().join("PRIVATE.zo"), &counter("PRIVATE"));
    let main = compile("CBL DLL\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 P USAGE PROCEDURE-POINTER.\nPROCEDURE DIVISION.\n>>CALLINT STATIC\nSET P TO ENTRY 'PRIVATE'\n>>CALLINT DLL\nCALL P\nCANCEL 'PRIVATE'\nCALL P\nSTOP RUN.").unwrap();
    let out = Capture::default();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.set_stdout(Box::new(out.clone()));
    machine.run_main(main).unwrap();
    assert_eq!(&*out.0.borrow(), b"1\n2\n");
}

#[test]
fn dll_does_not_expose_private_symbols_and_reports_missing_exports() {
    let dir = tempfile::tempdir().unwrap();
    zeus_cobol::save_module(&counter("PRIVATE"), &dir.path().join("PRIVATE.zo")).unwrap();
    let main = compile("CBL DLL\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 NAME-TEXT PIC X(7) VALUE 'PRIVATE'.\nPROCEDURE DIVISION.\nCALL NAME-TEXT ON EXCEPTION DISPLAY 'HIDDEN' END-CALL\nSTOP RUN.").unwrap();
    let out = Capture::default();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.set_stdout(Box::new(out.clone()));
    machine.run_main(main).unwrap();
    assert_eq!(&*out.0.borrow(), b"HIDDEN\n");
    assert!(
        compile("CBL DLL DYNAM\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nSTOP RUN.")
            .unwrap_err()
            .message
            .contains("NODYNAM")
    );
    let main = compile(
        "CBL DLL\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nCALL 'PRIVATE'\nSTOP RUN.",
    )
    .unwrap();
    assert!(
        Machine::new(vec![dir.path().to_path_buf()])
            .run_main(main)
            .unwrap_err()
            .to_string()
            .contains("DLL export PRIVATE")
    );
}
fn save(path: &Path, module: &Module) {
    std::fs::write(path, zeus_cobol::module_bytes(module).unwrap()).unwrap();
}

#[test]
fn callinterface_overrides_each_call_and_bare_directive_restores_defaults() {
    for (option, directive, expected) in [
        ("NODYNAM", "DYNAMIC", b"1\n1\n1\n2\n".as_slice()),
        ("DYNAM", "STATIC", b"1\n2\n1\n1\n".as_slice()),
    ] {
        let dir = tempfile::tempdir().unwrap();
        save(&dir.path().join("SUB.zo"), &counter("SUB"));
        save(&dir.path().join("OTHER.zo"), &counter("OTHER"));
        let text = format!(
            "CBL {option}\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\n>>CALLINTERFACE {directive}\nCALL 'SUB'\nCANCEL 'SUB'\nCALL 'SUB'\n>>CALLINT\nCALL 'OTHER'\nCANCEL 'OTHER'\nCALL 'OTHER'\nSTOP RUN."
        );
        let module = compile(&text).unwrap();
        let module =
            zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
        let out = Capture::default();
        let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
        machine.set_stdout(Box::new(out.clone()));
        machine.run_main(module).unwrap();
        assert_eq!(&*out.0.borrow(), expected, "{option}/{directive}");
    }
}

#[test]
fn set_entry_binding_follows_directive_and_identifier_calls_remain_dynamic() {
    for (directive, expected) in [
        ("STATIC", b"1\n2\n".as_slice()),
        ("DYNAMIC", b"1\n1\n".as_slice()),
    ] {
        let dir = tempfile::tempdir().unwrap();
        save(&dir.path().join("SUB.zo"), &counter("SUB"));
        let text = format!(
            "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 P USAGE PROCEDURE-POINTER.\n01 NAME-FIELD PIC XXX VALUE 'SUB'.\nPROCEDURE DIVISION.\n>>CALLINT {directive}\nSET P TO ENTRY 'SUB'\nCALL P\nCANCEL 'SUB'\nSET P TO ENTRY 'SUB'\nCALL P\nSTOP RUN."
        );
        let out = Capture::default();
        let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
        machine.set_stdout(Box::new(out.clone()));
        machine.run_main(compile(&text).unwrap()).unwrap();
        assert_eq!(&*out.0.borrow(), expected);
        // STATIC cannot turn CALL identifier into a static literal CALL.
        let text = text
            .replace("SET P TO ENTRY 'SUB'\n", "")
            .replace("CALL P", "CALL NAME-FIELD");
        let out = Capture::default();
        let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
        machine.set_stdout(Box::new(out.clone()));
        machine.run_main(compile(&text).unwrap()).unwrap();
        assert_eq!(&*out.0.borrow(), b"1\n1\n");
    }
}

#[test]
fn copy_order_replacement_protection_inactive_branches_and_unit_scope() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    source(
        &dir.path().join("MODE.cpy"),
        ">>CALLINTERFACE STATIC\nCALL 'SUB'",
    );
    source(
        &path,
        "CBL DYNAM\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nREPLACE ==STATIC== BY ==DYNAMIC==.\nCOPY MODE REPLACING ==SUB== BY ==OTHER==.\n>>IF FALSE\n>>CALLINTERFACE INVALID\n>>END-IF\nCALL 'OTHER'\n>>CALLINT DYNAMIC\nCALL 'DYN'\n>>CALLINTERFACE\nCALL 'DEFAULT'\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. NEXT-PROG.\nPROCEDURE DIVISION.\nCALL 'LAST'\nSTOP RUN.",
    );
    let module = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap();
    let modes: Vec<_> = module.programs[0]
        .code
        .iter()
        .filter_map(|instruction| match instruction {
            Instr::Call { interface, .. } => Some(*interface),
            _ => None,
        })
        .collect();
    assert_eq!(
        modes,
        vec![
            Some(zeus_cobol::zir::CallInterface::Static),
            Some(zeus_cobol::zir::CallInterface::Static),
            Some(zeus_cobol::zir::CallInterface::Dynamic),
            None
        ]
    );
    assert!(module.programs[1].code.iter().any(|instruction| matches!(
        instruction,
        Instr::Call {
            interface: None,
            ..
        }
    )));
    // A directive can stand before a paragraph or at the end of a unit.
    compile("ID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\n>>CALLINT STATIC\nMAIN-PARA.\nSTOP RUN.\n>>CALLINT DYNAMIC\nEND PROGRAM T.").unwrap();
    compile("ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO INFILE.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC X.\nPROCEDURE DIVISION.\n>>CALLINT STATIC\nDECLARATIVES.\n>>CALLINT DYNAMIC\nERR SECTION.\n>>CALLINT STATIC\nUSE AFTER ERROR ON F.\nERR-PARA.\nCONTINUE.\n>>CALLINT DYNAMIC\nEND DECLARATIVES.\nMAIN SECTION.\nSTOP RUN.").unwrap();
    // Other pseudo-text characters retain ordinary COPY/REPLACE behavior.
    compile("ID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nREPLACE == >> == BY ==CONTINUE==.\nSTOP RUN.").unwrap();
}

#[test]
fn conflicting_literal_calls_use_the_first_convention_and_preserve_warning() {
    for (first, later, expected) in [
        ("DYNAMIC", "STATIC", b"1\n1\n".as_slice()),
        ("STATIC", "DYNAMIC", b"1\n2\n".as_slice()),
    ] {
        let dir = tempfile::tempdir().unwrap();
        save(&dir.path().join("SUB.zo"), &counter("SUB"));
        let text = format!(
            "ID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\n>>CALLINT {first}\nCALL 'SUB'\nCANCEL 'SUB'\n>>CALLINT {later}\nCALL 'sub '\nSTOP RUN."
        );
        let module = compile(&text).unwrap();
        assert_eq!(module.compilation_return_code(), 4);
        assert_eq!(module.diagnostics.len(), 1);
        assert!(module.diagnostics[0].message.contains(first));
        assert_eq!(module.diagnostics[0].pos.line, 8);
        let module =
            zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
        assert_eq!(module.compilation_return_code(), 4);
        let out = Capture::default();
        let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
        machine.set_stdout(Box::new(out.clone()));
        machine.run_main(module).unwrap();
        assert_eq!(&*out.0.borrow(), expected);
    }
}

#[test]
fn cli_emits_warning_artifact_and_listing_with_return_code_four() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    let artifact = dir.path().join("main.zo");
    let listing = dir.path().join("main.lst");
    source(
        &path,
        "ID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\n>>CALLINT DYNAMIC\nCALL 'MISSING' ON EXCEPTION CONTINUE END-CALL\n>>CALLINT STATIC\nCALL 'MISSING' ON EXCEPTION CONTINUE END-CALL\nSTOP RUN.",
    );
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zcobc"))
        .arg("compile")
        .arg("--listing")
        .arg(&listing)
        .arg("-o")
        .arg(&artifact)
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("conflicting CALLINTERFACE"));
    let listing = std::fs::read_to_string(listing).unwrap();
    assert!(listing.contains("W: conflicting CALLINTERFACE"));
    assert!(listing.contains("END OF COMPILATION; RC=4"));
    let module = zeus_cobol::load_module(&artifact).unwrap();
    assert_eq!(module.compilation_return_code(), 4);
    Machine::new(vec![]).run_main(module).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zcobc"))
        .arg("check")
        .arg(path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(4));
    // Older modules have neither the warning list nor per-CALL overrides.
    let mut old = serde_json::to_value(counter("OLD")).unwrap();
    old.as_object_mut().unwrap().remove("diagnostics");
    let old = zeus_cobol::load_module_bytes(&serde_json::to_vec(&old).unwrap()).unwrap();
    assert_eq!(old.compilation_return_code(), 0);
}

#[test]
fn invalid_callinterface_placement_and_operands_are_diagnosed() {
    let body = "ID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nCALL 'MISSING' ON EXCEPTION CONTINUE END-CALL\nSTOP RUN.";
    for text in [
        body.replace(
            "PROCEDURE DIVISION.",
            ">>CALLINT STATIC\nPROCEDURE DIVISION.",
        ),
        body.replace("CALL 'MISSING'", "CALL\n>>CALLINT STATIC\n'MISSING'"),
        body.replace("STOP RUN.", "STOP >>CALLINT STATIC\nRUN."),
        body.replace("STOP RUN.", ">>CALLINT STATIC DYNAMIC\nSTOP RUN."),
        body.replace("STOP RUN.", ">>CALLINT STATIC.\nSTOP RUN."),
        body.replace(
            "STOP RUN.",
            "REPLACE ==X\n>>CALLINT STATIC\n== BY ==Y==.\nSTOP RUN.",
        ),
        body.replace("STOP RUN.", "COPY\n>>CALLINT STATIC\nMEMBER.\nSTOP RUN."),
        body.replace(
            "STOP RUN.",
            "DISPLAY 'unfinished\n>>CALLINT STATIC\n'\nSTOP RUN.",
        ),
    ] {
        assert!(compile(&text).is_err(), "{text}");
    }
    let module =
        compile(&body.replace("CALL 'MISSING'", ">>CALLINT DYNAMIC\nCALL 'MISSING'")).unwrap();
    Machine::new(vec![]).run_main(module).unwrap();
    let cics = body
        .replace("ID DIVISION.", "CBL CICS\nID DIVISION.")
        .replace("CALL 'MISSING'", ">>CALLINT DYNAMIC\nCALL 'MISSING'");
    // The CICS restriction is on the global DYNAM option. Per-call
    // CALLINTERFACE DYNAMIC is explicitly supported by IBM under CICS.
    compile(&cics).unwrap();
}

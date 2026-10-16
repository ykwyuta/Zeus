use std::{cell::RefCell, io::Write, rc::Rc};
use zeus_cobol::{front::FrontOptions, vm::Machine, zir::Module};

#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn compile(src: &str) -> Result<Module, zeus_cobol::diag::Diag> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("C.cbl");
    assert!(src.lines().all(|l| l.chars().count() <= 65));
    std::fs::write(
        &path,
        src.lines()
            .map(|l| format!("       {l}\n"))
            .collect::<String>(),
    )
    .unwrap();
    zeus_cobol::compile_file(&path, &FrontOptions::default())
}
fn run_module(module: Module) -> String {
    let mut vm = Machine::new(vec![]);
    let out = Capture::default();
    vm.set_stdout(Box::new(out.clone()));
    vm.run_main(module).unwrap();
    String::from_utf8(out.0.borrow().clone()).unwrap()
}
fn run(src: &str) -> String {
    run_module(compile(src).unwrap())
}

#[test]
fn compiled_time_is_persisted_and_register_has_its_own_format() {
    let src = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 STAMP PIC X(16).\nPROCEDURE DIVISION.\nMOVE WHEN-COMPILED TO STAMP\nDISPLAY FUNCTION WHEN-COMPILED\nDISPLAY STAMP\nSTOP RUN.";
    let mut module = compile(src).unwrap();
    assert_eq!(module.compiled_at.as_ref().unwrap().len(), 21);
    module.compiled_at = Some("2000010203040567+0000".into());
    let saved = zeus_cobol::module_bytes(&module).unwrap();
    assert_eq!(
        run_module(zeus_cobol::load_module_bytes(&saved).unwrap()),
        "2000010203040567+0000\n01/02/0003.04.05\n"
    );
}

#[test]
fn returning_program_result_preserves_callers_return_code_and_exception_target() {
    let src = "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 ANSWER PIC S9(9) COMP-5 VALUE 0.\nPROCEDURE DIVISION.\nMOVE 7 TO RETURN-CODE\nCALL 'SUB' RETURNING ANSWER\nDISPLAY ANSWER '/' RETURN-CODE\nCALL 'MISSING' RETURNING ANSWER ON EXCEPTION CONTINUE\nEND-CALL\nDISPLAY ANSWER '/' RETURN-CODE\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nLINKAGE SECTION.\n01 RESULT PIC S9(9) COMP-5.\nPROCEDURE DIVISION RETURNING RESULT.\nMOVE 123456 TO RESULT\nMOVE 9 TO RETURN-CODE\nGOBACK.\nEND PROGRAM SUB.";
    let output = run(src);
    let lines: Vec<_> = output.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], lines[1]);
    assert_eq!(lines[0], "0000123456/0007");
}

#[test]
fn returning_character_result_and_metadata_survive_serialization() {
    let src = "ID DIVISION.\nPROGRAM-ID. P.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 RESULT PIC X(4).\nPROCEDURE DIVISION.\nCALL 'F' RETURNING RESULT\nDISPLAY RESULT\nSTOP RUN.\nEND PROGRAM P.\nID DIVISION.\nPROGRAM-ID. F.\nDATA DIVISION.\nLINKAGE SECTION.\n01 RESULT PIC X(4).\nPROCEDURE DIVISION RETURNING RESULT.\nMOVE 'DONE' TO RESULT\nGOBACK.\nEND PROGRAM F.";
    let module = compile(src).unwrap();
    let module =
        zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
    assert_eq!(run_module(module), "DONE\n");
}

#[test]
fn trunc_modes_apply_to_value_move_display_and_arithmetic() {
    let src = "CBL TRUNC(MODE)\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 B PIC 9(4) BINARY VALUE 12345.\n01 C PIC S99 BINARY.\n01 N PIC 9(4) COMP-5 VALUE 12345.\nPROCEDURE DIVISION.\nDISPLAY B '/' N\nMOVE 123451 TO C\nDISPLAY C\nMOVE 32767 TO C\nADD 1 TO C ON SIZE ERROR DISPLAY 'OVERFLOW' END-ADD\nSTOP RUN.";
    assert_eq!(run(&src.replace("MODE", "STD")), "2345/12345\n51\n");
    assert_eq!(
        run(&src.replace("MODE", "BIN")),
        "12345/12345\n0762J\nOVERFLOW\n"
    );
    let valid = "CBL TRUNC(MODE)\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 B PIC 9(4) BINARY.\nPROCEDURE DIVISION.\nMOVE 1234 TO B\nADD 1 TO B\nDISPLAY B\nSTOP RUN.";
    assert_eq!(
        run(&valid.replace("MODE", "STD")),
        run(&valid.replace("MODE", "OPT"))
    );
}

fn vsam_source(org: &str, key: &str, data: &str, actions: &str) -> String {
    format!(
        "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO TESTFILE\nORGANIZATION {org} ACCESS DYNAMIC\n{key}\nFILE STATUS FS DETAIL.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 REC.\n05 K PIC X.\n05 A PIC X.\nWORKING-STORAGE SECTION.\n01 FS PIC XX.\n01 DETAIL.\n05 VR PIC 9(4) COMP-5.\n05 VC PIC 9(4) COMP-5.\n05 VF PIC 9(4) COMP-5.\n{data}\nPROCEDURE DIVISION.\n{actions}\nSTOP RUN."
    )
}

#[test]
fn vsam_extended_status_reports_logical_errors_and_clears_local_errors() {
    for (org, key, data, setup) in [
        ("INDEXED", "RECORD KEY K", "01 RK PIC 9.", "MOVE 'A' TO K"),
        (
            "RELATIVE",
            "RELATIVE KEY RK",
            "01 RK PIC 9.",
            "MOVE 1 TO RK",
        ),
    ] {
        let actions = format!(
            "OPEN OUTPUT F\nCLOSE F\nOPEN I-O F\n{setup}\nWRITE REC\nWRITE REC\nDISPLAY FS '/' VR '/' VC '/' VF\nCLOSE F\nREAD F\nDISPLAY FS '/' VR '/' VC '/' VF"
        );
        let mut module = compile(&vsam_source(org, key, data, &actions)).unwrap();
        let dir = tempfile::tempdir().unwrap();
        module.programs[0].files[0].assign =
            dir.path().join("test.dat").to_string_lossy().into_owned();
        let module =
            zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
        assert_eq!(
            run_module(module),
            "22/00008/00000/00008\n47/00000/00000/00000\n"
        );
    }
}

#[test]
fn vsam_extended_status_requires_a_valid_group() {
    let src = vsam_source("INDEXED", "RECORD KEY K", "", "CONTINUE");
    for bad in [
        src.replace("ORGANIZATION INDEXED", "ORGANIZATION SEQUENTIAL"),
        src.replace("05 VF PIC 9(4) COMP-5.", "05 VF PIC 9(9) COMP-5."),
        src.replace("01 DETAIL.", "01 DETAIL PIC X(6).\n01 UNUSED."),
    ] {
        assert!(compile(&bad).is_err());
    }
}

#[test]
fn national_decimal_and_numeric_editing_use_utf16_storage() {
    let src = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC S9(3)V99 NATIONAL SIGN LEADING SEPARATE.\n01 E PIC ZZ9.99 NATIONAL.\n01 D PIC S9(3)V99 SIGN LEADING SEPARATE.\nPROCEDURE DIVISION.\nMOVE -12.34 TO N\nADD 2 TO N\nIF N IS NUMERIC DISPLAY 'NUMERIC' END-IF\nMOVE N TO D\nDISPLAY D\nMOVE N TO E\nDISPLAY E\nMOVE E TO D\nDISPLAY D\nMOVE N'99' TO N(2:2)\nDISPLAY N\nSTOP RUN.";
    let m = compile(src).unwrap();
    let desc = m.programs[0].descs.iter().find(|d| d.name == "N").unwrap();
    assert_eq!(desc.len, 12);
    assert_eq!(run_module(m), "NUMERIC\n-01034\n 10.34\n+01034\n-99034\n");
}

#[test]
fn national_groups_and_national_editing_preserve_unicode_and_character_boundaries() {
    let src = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 G GROUP-USAGE NATIONAL.\n05 INNER-G.\n10 A PIC N(2).\n10 N PIC 99.\n01 E PIC NBN/0N.\n01 COPY-G PIC N(4).\nPROCEDURE DIVISION.\nMOVE N'AB12' TO G\nIF N = 12 DISPLAY 'NUMBER' END-IF\nMOVE G TO COPY-G\nIF COPY-G = N'AB12' DISPLAY 'GROUP' END-IF\nMOVE N'日月火' TO E\nIF E = N'日 月/0火' DISPLAY 'EDITED' END-IF\nMOVE E(1:3) TO COPY-G\nIF COPY-G = N'日 月 ' DISPLAY 'REFMOD' END-IF\nINITIALIZE G\nIF A = N'  ' AND N = 0 DISPLAY 'INITIAL' END-IF\nSTOP RUN.";
    assert_eq!(run(src), "NUMBER\nGROUP\nEDITED\nREFMOD\nINITIAL\n");
}

#[test]
fn invalid_national_definitions_are_diagnosed() {
    for entries in [
        "01 N PIC S999 NATIONAL.",
        "01 G GROUP-USAGE NATIONAL.\n05 N PIC 9 BINARY.",
        "01 G GROUP-USAGE NATIONAL USAGE NATIONAL.\n05 N PIC 9.",
        "01 N PIC N GROUP-USAGE NATIONAL.",
        "01 N PIC XXX NATIONAL.",
    ] {
        let src = format!(
            "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{entries}\nPROCEDURE DIVISION.\nSTOP RUN."
        );
        assert!(compile(&src).is_err(), "{entries}");
    }
}

#[test]
fn procedure_and_function_pointers_support_entry_copy_null_and_indirect_calls() {
    let src = "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 PP USAGE PROCEDURE-POINTER VALUE NULL.\n01 FP USAGE FUNCTION-POINTER VALUE NULL.\n01 NAME PIC X(3) VALUE 'SUB'.\n01 ANSWER PIC 999.\nPROCEDURE DIVISION.\nIF PP = NULL DISPLAY 'NULL' END-IF\nSET PP TO ENTRY NAME\nSET FP TO PP\nCALL FP RETURNING ANSWER\nDISPLAY ANSWER\nSET PP TO NULL\nIF PP = NULL DISPLAY 'CLEARED' END-IF\nCALL FP RETURNING ANSWER\nDISPLAY ANSWER\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 COUNTER PIC 999 VALUE 0.\nLINKAGE SECTION.\n01 RESULT PIC 999.\nPROCEDURE DIVISION RETURNING RESULT.\nADD 1 TO COUNTER\nMOVE COUNTER TO RESULT\nGOBACK.\nEND PROGRAM SUB.";
    let module = compile(src).unwrap();
    let descs = &module.programs[0].descs;
    assert_eq!(descs.iter().find(|d| d.name == "PP").unwrap().len, 8);
    assert_eq!(descs.iter().find(|d| d.name == "FP").unwrap().len, 4);
    assert_eq!(run_module(module), "NULL\n001\nCLEARED\n002\n");
}

#[test]
fn invalid_pointer_values_and_null_calls_fail() {
    for entry in [
        "01 P PROCEDURE-POINTER VALUE 1.",
        "01 P FUNCTION-POINTER VALUE 'SUB'.",
    ] {
        assert!(compile(&format!("ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{entry}\nPROCEDURE DIVISION.\nSTOP RUN.")).is_err());
    }
    let m = compile("ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 P FUNCTION-POINTER VALUE NULL.\nPROCEDURE DIVISION.\nCALL P\nSTOP RUN.").unwrap();
    let error = Machine::new(vec![]).run_main(m).unwrap_err();
    assert!(error.0.contains("null or invalid"));
}

#[test]
fn returning_survives_an_exception_handled_inside_the_callee() {
    let src = "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 ANSWER PIC 999.\nPROCEDURE DIVISION.\nCALL 'SUB' RETURNING ANSWER\nDISPLAY ANSWER\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nLINKAGE SECTION.\n01 RESULT PIC 999.\nPROCEDURE DIVISION RETURNING RESULT.\nMOVE 123 TO RESULT\nCALL 'MISSING' ON EXCEPTION CONTINUE END-CALL\nGOBACK.\nEND PROGRAM SUB.";
    assert_eq!(run(src), "123\n");
}

#[test]
fn recursive_programs_reject_ibm_prohibited_constructs() {
    for body in ["GO TO.", "ALTER P TO PROCEED TO Q."] {
        let src = format!(
            "ID DIVISION.\nPROGRAM-ID. T RECURSIVE.\nPROCEDURE DIVISION.\n{body}\nP.\nGO TO Q.\nQ.\nGOBACK."
        );
        let error = compile(&src).unwrap_err();
        assert!(error.message.contains("RECURSIVE"), "{error}");
    }
    let src = "ID DIVISION.\nPROGRAM-ID. T RECURSIVE.\nENVIRONMENT DIVISION.\nCONFIGURATION SECTION.\nOBJECT-COMPUTER. IBM SEGMENT-LIMIT 49.\nPROCEDURE DIVISION.\nGOBACK.";
    assert!(compile(src).unwrap_err().message.contains("RECURSIVE"));
}

#[test]
fn returning_rejects_non_linkage_and_entry_combinations() {
    let src = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nLINKAGE SECTION.\n01 RESULT PIC 999.\nPROCEDURE DIVISION RETURNING RESULT.\nENTRY 'ALT'.\nGOBACK.";
    assert!(compile(src).unwrap_err().message.contains("ENTRY"));
    let src = src
        .replace("LINKAGE SECTION.", "WORKING-STORAGE SECTION.")
        .replace("ENTRY 'ALT'.\n", "");
    assert!(compile(&src).unwrap_err().message.contains("LINKAGE"));
}

#[test]
fn foreign_returning_uses_function_value_and_preserves_return_code() {
    struct Foreign;
    impl zeus_cobol::interlang::Linker for Foreign {
        fn call(
            &self,
            name: &str,
            sys: Machine,
            _: &[zeus_cobol::interlang::XRef],
        ) -> (Machine, Option<Result<zeus_cobol::interlang::XEnd, String>>) {
            assert_eq!(name, "FOREIGN");
            (
                sys,
                Some(Ok(zeus_cobol::interlang::XEnd {
                    rc: 31,
                    ret: Some(12345u16.to_be_bytes().to_vec()),
                    stop: false,
                })),
            )
        }
    }
    let m = compile("ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 ANSWER PIC 9(4) COMP-5.\nPROCEDURE DIVISION.\nMOVE 7 TO RETURN-CODE\nCALL 'FOREIGN' RETURNING ANSWER\nDISPLAY ANSWER '/' RETURN-CODE\nSTOP RUN.").unwrap();
    let mut vm = Machine::new(vec![]);
    let out = Capture::default();
    vm.set_stdout(Box::new(out.clone()));
    vm.set_linker(Rc::new(Foreign));
    vm.run_main(m).unwrap();
    assert_eq!(&*out.0.borrow(), b"12345/0007\n");
}

#[test]
fn externally_called_cobol_exposes_the_returning_representation() {
    let m = compile("ID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nLINKAGE SECTION.\n01 RESULT PIC 9(4) COMP-5.\nPROCEDURE DIVISION RETURNING RESULT.\nMOVE 12345 TO RESULT\nGOBACK.").unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("SUB.zo"),
        zeus_cobol::module_bytes(&m).unwrap(),
    )
    .unwrap();
    let mut vm = Machine::new(vec![dir.path().to_path_buf()]);
    let result = vm.call_program("SUB", &[]).unwrap().unwrap();
    assert_eq!(result.ret.unwrap(), 12345u16.to_be_bytes());
}

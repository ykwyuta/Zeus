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

fn range_source(entries: &str, statements: &str) -> String {
    format!(
        "CBL SSRANGE\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{entries}\nPROCEDURE DIVISION.\n{statements}\nSTOP RUN."
    )
}

#[test]
fn codepage_controls_literals_conversions_and_separately_compiled_calls() {
    let host = zeus_cobol::codepage::active().ccsid;
    for (ccsid, literal) in [(1140, "€"), (939, "日本"), (37, "¤")] {
        let source = format!(
            "CBL CODEPAGE({ccsid})\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC X(8) VALUE '{literal}'.\nPROCEDURE DIVISION.\nIF FUNCTION TRIM(FUNCTION NATIONAL-OF(A))\n= N'{literal}' DISPLAY 'OK' END-IF\nSTOP RUN."
        );
        let module = compile(&source).unwrap();
        assert_eq!(module.ccsid, ccsid);
        let descriptor = module.programs[0]
            .descs
            .iter()
            .find(|d| d.name == "A")
            .unwrap();
        let expected = zeus_cobol::codepage::get(ccsid).unwrap().encode(literal);
        assert_eq!(
            &module.programs[0].static_init
                [descriptor.offset as usize..descriptor.offset as usize + expected.len()],
            &expected
        );
        assert_eq!(run_module(module), "OK\n");
        assert_eq!(zeus_cobol::codepage::active().ccsid, host);
    }
    let main = compile("CBL CP(37)\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nCALL 'SUB'\nIF FUNCTION NATIONAL-OF(X'9F') = N'¤'\nDISPLAY 'MAIN' END-IF\nSTOP RUN.").unwrap();
    let sub = compile("CBL CP(1140)\nID DIVISION.\nPROGRAM-ID. SUB.\nPROCEDURE DIVISION.\nIF FUNCTION NATIONAL-OF(X'9F') = N'€'\nDISPLAY 'SUB' END-IF\nGOBACK.").unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("SUB.zo"),
        zeus_cobol::module_bytes(&sub).unwrap(),
    )
    .unwrap();
    let output = Capture::default();
    let mut vm = Machine::new(vec![dir.path().to_path_buf()]);
    vm.set_stdout(Box::new(output.clone()));
    vm.run_main(main).unwrap();
    assert_eq!(&*output.0.borrow(), b"SUB\nMAIN\n");
    let module = compile("CBL CP(1140)\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nLINKAGE SECTION.\n01 ARGS.\n05 SIZE-FIELD PIC 9(4) COMP-5.\n05 TEXT-FIELD PIC X.\nPROCEDURE DIVISION USING ARGS.\nIF FUNCTION NATIONAL-OF(TEXT-FIELD) = N'€'\nDISPLAY 'PARM' END-IF\nSTOP RUN.").unwrap();
    let output = Capture::default();
    let mut vm = Machine::new(vec![]);
    vm.set_step(zeus_cobol::vm::StepInfo {
        parm: "€".into(),
        ..Default::default()
    });
    vm.set_stdout(Box::new(output.clone()));
    vm.run_main(module).unwrap();
    assert_eq!(&*output.0.borrow(), b"PARM\n");
    for option in ["CP(99999)", "CODEPAGE('939')", "CP"] {
        let source =
            format!("CBL {option}\nID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nSTOP RUN.");
        assert!(compile(&source).is_err(), "{option}");
        assert_eq!(zeus_cobol::codepage::active().ccsid, host);
    }
}

#[test]
fn national_string_functions_preserve_unicode_type_and_surrogate_pairs() {
    let source = range_source(
        "01 N PIC N(4) VALUE N'日😀a'.",
        "IF FUNCTION REVERSE(N) = N'a😀日'\nDISPLAY 'REVERSE' END-IF\nIF FUNCTION UPPER-CASE(N) = N'日😀A'\nDISPLAY 'UPPER' END-IF\nIF FUNCTION LOWER-CASE(N'İΣÉ') = N'iσé'\nDISPLAY 'LOWER' END-IF\nIF FUNCTION UPPER-CASE(N'ßᾀ') = N'ßᾈ'\nDISPLAY 'SIMPLE' END-IF\nIF FUNCTION TRIM(N' 日 ') = N'日'\nDISPLAY 'TRIM' END-IF\nIF FUNCTION MAX(N'日', N'月') = N'月'\nDISPLAY 'MAX' END-IF\nIF FUNCTION MIN(N'日', N'日 ') = N'日'\nDISPLAY 'MIN' END-IF\nIF FUNCTION ORD-MAX(N'日', N'月') = 2\nDISPLAY 'ORD' END-IF\nIF FUNCTION REVERSE(N)(1:1) = N'a'\nDISPLAY 'SLICE' END-IF",
    );
    let module = compile(&source).unwrap();
    let module =
        zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
    assert_eq!(
        run_module(module),
        "REVERSE\nUPPER\nLOWER\nSIMPLE\nTRIM\nMAX\nMIN\nORD\nSLICE\n"
    );
    let error = Machine::new(vec![])
        .run_main(
            compile(&range_source(
                "01 N PIC N(2).",
                "DISPLAY FUNCTION REVERSE(N)(2:2)",
            ))
            .unwrap(),
        )
        .unwrap_err();
    assert!(error.0.contains("SSRANGE"));
    for function in ["MAX", "MIN", "ORD-MAX", "ORD-MIN"] {
        let source = range_source("", &format!("DISPLAY FUNCTION {function}(N'A', 'B')"));
        assert!(compile(&source).is_err(), "{function}");
    }
}

#[test]
fn ssrange_checks_effective_table_addresses_and_survives_serialization() {
    let declarations =
        "01 G.\n05 ROW OCCURS 2.\n10 CELL PIC X OCCURS 3.\n01 AFTER-G PIC X VALUE 'Z'.";
    let source = range_source(declarations, "MOVE 'Q' TO CELL(2, 1)\nDISPLAY CELL(1, 4)");
    let module = compile(&source).unwrap();
    assert!(module.programs[0].ssrange);
    let module =
        zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
    assert_eq!(run_module(module), "Q\n");
    for reference in ["CELL(1, 0)", "CELL(2, 4)"] {
        let source = range_source(declarations, &format!("DISPLAY {reference}"));
        let error = Machine::new(vec![])
            .run_main(compile(&source).unwrap())
            .unwrap_err();
        assert!(error.0.contains("SSRANGE"), "{error}");
    }
    let source = range_source(declarations, "MOVE 'Z' TO CELL(2, 4)\nDISPLAY CELL(2, 4)");
    let mut vm = Machine::new(vec![]);
    vm.set_range_checking(false);
    let output = Capture::default();
    vm.set_stdout(Box::new(output.clone()));
    vm.run_main(compile(&source).unwrap()).unwrap();
    assert_eq!(&*output.0.borrow(), b"Z\n");
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("range.zo");
    std::fs::write(
        &path,
        zeus_cobol::module_bytes(&compile(&source).unwrap()).unwrap(),
    )
    .unwrap();
    for (mode, success) in [("ON", false), ("OFF", true)] {
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_zcobc"))
            .args(["run", "--check-range", mode])
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(
            result.status.success(),
            success,
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    assert_eq!(run(&source.replace("CBL SSRANGE", "CBL NOSSRANGE")), "Z\n");
}

#[test]
fn ssrange_global_table_boundaries_use_the_defining_program_storage() {
    let source = "CBL SSRANGE\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9 VALUE 1.\n01 G GLOBAL.\n05 A PIC X OCCURS 0 TO 3 DEPENDING ON N.\n05 T OCCURS 2.\n10 B PIC X.\nPROCEDURE DIVISION.\nMOVE 'Q' TO B(2)\nCALL 'SUB'\nSTOP RUN.\nID DIVISION.\nPROGRAM-ID. SUB.\nPROCEDURE DIVISION.\nDISPLAY B(2)\nDISPLAY B(3)\nGOBACK.\nEND PROGRAM SUB.\nEND PROGRAM MAIN.";
    let mut vm = Machine::new(vec![]);
    let output = Capture::default();
    vm.set_stdout(Box::new(output.clone()));
    let error = vm.run_main(compile(source).unwrap()).unwrap_err();
    assert_eq!(&*output.0.borrow(), b"Q\n");
    assert!(error.0.contains("SSRANGE"), "{error}");
}

#[test]
fn ssrange_checks_reference_modification_in_character_units() {
    let entries = "01 A PIC XXX VALUE 'ABC'.\n01 N PIC N(3) VALUE N'日月火'.";
    assert_eq!(
        run(&range_source(
            entries,
            "IF N(2:2) = N'月火' DISPLAY A(2:2) END-IF"
        )),
        "BC\n"
    );
    for reference in [
        "A(0:1)",
        "A(2:3)",
        "A(1:0)",
        "N(3:2)",
        "FUNCTION NATIONAL-OF('ABC')(3:2)",
    ] {
        let source = range_source(entries, &format!("DISPLAY {reference}"));
        let error = Machine::new(vec![])
            .run_main(compile(&source).unwrap())
            .unwrap_err();
        assert!(error.0.contains("SSRANGE"), "{reference}: {error}");
    }
}

#[test]
fn ssrange_checks_variable_lengths_and_all_occurrence_counts() {
    let entries = "01 N PIC S9 VALUE 4.\n01 G.\n05 A PIC 9 OCCURS 0 TO 3 DEPENDING ON N.";
    for statement in [
        "DISPLAY G",
        "MOVE 0 TO N\nDISPLAY G",
        "MOVE -1 TO N\nDISPLAY G",
        "DISPLAY FUNCTION SUM(A(ALL))",
        "MOVE 0 TO N\nDISPLAY FUNCTION SUM(A(ALL))",
    ] {
        let source = range_source(entries, statement);
        let error = Machine::new(vec![])
            .run_main(compile(&source).unwrap())
            .unwrap_err();
        assert!(error.0.contains("SSRANGE"), "{statement}: {error}");
    }
    // Ordinary table references use the maximum table region, even when
    // the current ODO count is smaller.
    assert_eq!(
        run(&range_source(
            entries,
            "MOVE 1 TO N\nMOVE 7 TO A(3)\nDISPLAY A(3)"
        )),
        "7\n"
    );
}

#[test]
fn complex_odo_recomputes_following_offsets_lengths_and_outer_stride() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9 VALUE 2.\n01 M PIC 9 VALUE 1.\n01 G.\n05 A OCCURS 0 TO 4 DEPENDING ON N PIC X.\n05 TAIL PIC XX VALUE 'OK'.\n05 ROWS OCCURS 2.\n10 HEAD PIC X VALUE 'H'.\n10 B OCCURS 0 TO 3 DEPENDING ON M PIC X.\n05 LAST-FIELD PIC X VALUE 'Z'.\nPROCEDURE DIVISION.\nDISPLAY TAIL HEAD(2) LAST-FIELD\nDISPLAY LENGTH OF G\nMOVE 'a' TO A(1)\nMOVE 'b' TO A(2)\nMOVE 'c' TO B(1, 1)\nMOVE 'd' TO B(2, 1)\nDISPLAY G\nMOVE 1 TO N\nMOVE 2 TO M\nMOVE 'XY' TO TAIL\nMOVE '1' TO HEAD(1)\nMOVE '2' TO HEAD(2)\nMOVE 'p' TO B(1, 1)\nMOVE 'q' TO B(1, 2)\nMOVE 'r' TO B(2, 1)\nMOVE 's' TO B(2, 2)\nMOVE '!' TO LAST-FIELD\nDISPLAY G\nDISPLAY LENGTH OF G\nSTOP RUN.";
    let module = compile(source).unwrap();
    let module =
        zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
    assert_eq!(
        run_module(module),
        "OKHZ\n000000009\nabOKHcHdZ\naXY1pq2rs!\n000000010\n"
    );
}

#[test]
fn complex_odo_global_references_evaluate_counts_in_the_owner() {
    let source = "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9 VALUE 1.\n01 G GLOBAL.\n05 A OCCURS 0 TO 4 DEPENDING ON N PIC X.\n05 TAIL PIC XX VALUE 'OK'.\nPROCEDURE DIVISION.\nCALL 'SUB'\nMOVE 3 TO N\nMOVE 'HI' TO TAIL\nCALL 'SUB'\nSTOP RUN.\nID DIVISION.\nPROGRAM-ID. SUB.\nPROCEDURE DIVISION.\nDISPLAY TAIL\nGOBACK.\nEND PROGRAM SUB.\nEND PROGRAM MAIN.";
    assert_eq!(run(source), "OK\nHI\n");
}

#[test]
fn complex_odo_rejects_variably_located_count_objects() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9 VALUE 1.\n01 G.\n05 A OCCURS 0 TO 3 DEPENDING ON N PIC X.\n05 M PIC 9.\n05 B OCCURS 0 TO 3 DEPENDING ON M PIC X.\nPROCEDURE DIVISION.\nSTOP RUN.";
    assert!(
        compile(source)
            .unwrap_err()
            .message
            .contains("variably located")
    );
}

#[test]
fn statistical_functions_keep_extended_precision_and_use_active_occurrences() {
    let source = "CBL ARITH(EXTEND)\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9 VALUE 2.\n01 G.\n05 A OCCURS 1 TO 3 DEPENDING ON N PIC 9(31).\n01 R PIC 9.9(30).\n01 S PIC 9(31).\nPROCEDURE DIVISION.\nMOVE 1000000000000000000000000000001 TO A(1)\nMOVE 1000000000000000000000000000003 TO A(2)\nCOMPUTE S = FUNCTION SUM(A(ALL))\nDISPLAY S\nCOMPUTE R ROUNDED = FUNCTION VARIANCE(A(ALL))\nDISPLAY R\nCOMPUTE R ROUNDED = FUNCTION STANDARD-DEVIATION(A(ALL))\nDISPLAY R\nCOMPUTE R ROUNDED = FUNCTION MEAN(0 1 1)\nDISPLAY R\nCOMPUTE R ROUNDED = FUNCTION ANNUITY(0 4)\nDISPLAY R\nCOMPUTE R ROUNDED = FUNCTION PRESENT-VALUE(1 1 1)\nDISPLAY R\nSTOP RUN.";
    assert_eq!(
        run(source),
        "2000000000000000000000000000004\n1.000000000000000000000000000000\n1.000000000000000000000000000000\n0.666666666666666666666666666667\n0.250000000000000000000000000000\n0.750000000000000000000000000000\n"
    );
}

#[test]
fn same_sort_area_rejects_shared_participants_and_open_peers() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT S ASSIGN TO SORTWK.\nSELECT F ASSIGN TO INFILE.\nI-O-CONTROL.\nSAME SORT AREA FOR S F.\nDATA DIVISION.\nFILE SECTION.\nSD S.\n01 SR PIC X.\nFD F.\n01 FR PIC X.\nPROCEDURE DIVISION.\nSORT S ASCENDING KEY SR USING F GIVING F\nSTOP RUN.";
    assert!(
        compile(source)
            .unwrap_err()
            .message
            .contains("SAME SORT AREA")
    );
    let source = source.replace(
        "SORT S ASCENDING KEY SR USING F GIVING F",
        "OPEN OUTPUT F\nSORT S ASCENDING KEY SR\nINPUT PROCEDURE P OUTPUT PROCEDURE Q",
    );
    let source = format!("{source}\nP.\nCONTINUE.\nQ.\nCONTINUE.");
    let module = compile(&source).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut vm = Machine::new(vec![]);
    vm.set_dd("INFILE", dir.path().join("in.dat"));
    assert!(
        vm.run_main(module)
            .unwrap_err()
            .0
            .contains("SAME SORT AREA")
    );
    let source = source
        .replace("OPEN OUTPUT F\n", "")
        .replace("P.\nCONTINUE.", "P.\nOPEN OUTPUT F.");
    let mut vm = Machine::new(vec![]);
    vm.set_dd("INFILE", dir.path().join("in.dat"));
    assert!(
        vm.run_main(compile(&source).unwrap())
            .unwrap_err()
            .0
            .contains("SAME SORT AREA")
    );
}

#[test]
fn inactive_source_settings_do_not_enable_debugging_or_decimal_comma() {
    let source = "CBL DEFINE(Target='a)b')\nID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nCONFIGURATION SECTION.\n>>IF Target IS DEFINED\n>>DEFINE Target AS PARAMETER\n>>END-IF\n>>IF FALSE\nSOURCE-COMPUTER. IBM WITH DEBUGGING MODE.\nSPECIAL-NAMES. DECIMAL-POINT IS COMMA.\n>>END-IF\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC 9V9 VALUE 1.5.\nPROCEDURE DIVISION.\n>>IF Target = 'a)b'\nDISPLAY A\n>>END-IF\nSTOP RUN.";
    // Parameter names are introduced only by DEFINE AS PARAMETER.
    let source = source.replace(">>IF Target IS DEFINED\n", "").replace(
        ">>DEFINE Target AS PARAMETER\n>>END-IF",
        ">>DEFINE Target AS PARAMETER",
    );
    assert_eq!(run(&source), "15\n");
}

#[test]
fn group_move_uses_maximum_size_when_it_contains_the_odo_object() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 G.\n05 N PIC 9 VALUE 1.\n05 A OCCURS 0 TO 3 DEPENDING ON N PIC X.\n05 TAIL PIC X.\nPROCEDURE DIVISION.\nMOVE '2xyZ!' TO G\nDISPLAY G\nDISPLAY TAIL\nSTOP RUN.";
    assert_eq!(run(source), "2xyZ\nZ\n");
}

#[test]
fn complex_odo_retains_the_extent_of_an_earlier_redefinition() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9 VALUE 0.\n01 G.\n05 A PIC X.\n05 BIG-FIELD REDEFINES A PIC X(4).\n05 B OCCURS 0 TO 3 DEPENDING ON N PIC X.\n05 TAIL PIC X VALUE 'Z'.\nPROCEDURE DIVISION.\nDISPLAY LENGTH OF G\nDISPLAY TAIL\nMOVE 3 TO N\nMOVE '!' TO TAIL\nDISPLAY LENGTH OF G\nDISPLAY TAIL\nSTOP RUN.";
    assert_eq!(run(source), "000000004\nZ\n000000005\n!\n");
}

#[test]
fn group_returning_compares_subordinate_types_and_ignores_field_names() {
    let source = "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 ANSWER.\n05 A PIC XX.\n05 B PIC 99.\nPROCEDURE DIVISION.\nCALL 'SUB' RETURNING ANSWER\nDISPLAY ANSWER\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nLINKAGE SECTION.\n01 RESULT-G.\n05 C PIC XX.\n05 D PIC 99.\nPROCEDURE DIVISION RETURNING RESULT-G.\nMOVE 'OK' TO C\nMOVE 12 TO D\nGOBACK.\nEND PROGRAM SUB.";
    assert_eq!(run(source), "OK12\n");
    let bad = source.replace("05 D PIC 99.", "05 D PIC XX.");
    assert!(
        Machine::new(vec![])
            .run_main(compile(&bad).unwrap())
            .unwrap_err()
            .0
            .contains("RETURNING")
    );
}

#[test]
fn data_clauses_reject_invalid_categories_instead_of_ignoring_them() {
    for entry in [
        "01 A PIC 99 JUSTIFIED.",
        "01 A PIC Z9 JUSTIFIED.",
        "01 A JUSTIFIED.\n05 B PIC X.",
        "01 A PIC XX BLANK WHEN ZERO.",
        "01 A PIC S99 BLANK WHEN ZERO.",
        "01 A PIC **9 BLANK WHEN ZERO.",
        "01 A PIC 99 COMP BLANK WHEN ZERO.",
        "01 A PIC 99 SIGN LEADING.",
        "01 A SIGN LEADING.\n05 B PIC X.",
        "01 A COMP-2 PIC 99.",
        "01 A USAGE POINTER PIC 99.",
        "01 A PIC XX COMP-3.",
    ] {
        let source = format!(
            "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{entry}\nPROCEDURE DIVISION.\nSTOP RUN."
        );
        assert!(compile(&source).is_err(), "{entry}");
    }
}

#[test]
fn external_file_declarations_must_agree_across_programs() {
    let source = "ID DIVISION.\nPROGRAM-ID. MAIN.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO INFILE.\nDATA DIVISION.\nFILE SECTION.\nFD F EXTERNAL.\n01 REC-A PIC XX.\nPROCEDURE DIVISION.\nCALL 'SUB'\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT OPTIONAL F ASSIGN TO INFILE.\nDATA DIVISION.\nFILE SECTION.\nFD F EXTERNAL.\n01 REC-B PIC XX.\nPROCEDURE DIVISION.\nOPEN INPUT F\nCLOSE F\nGOBACK.\nEND PROGRAM SUB.";
    assert!(
        Machine::new(vec![])
            .run_main(compile(source).unwrap())
            .unwrap_err()
            .0
            .contains("declarations disagree")
    );
    // Record names can differ, while OPTIONAL must agree.
    let source = source.replace("SELECT F ASSIGN", "SELECT OPTIONAL F ASSIGN");
    assert_eq!(
        Machine::new(vec![])
            .run_main(compile(&source).unwrap())
            .unwrap(),
        0
    );
}

#[test]
fn arithmetic_modes_bound_operands_and_preserve_extended_float_precision() {
    for (mode, expected) in [("COMPAT", "0\n"), ("EXTEND", "1\n")] {
        let source = format!(
            "CBL ARITH({mode})\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A COMP-2.\n01 R PIC 9.\nPROCEDURE DIVISION.\nMOVE 1.0E+30 TO A\nCOMPUTE R = (A + 1) - A\nDISPLAY R\nSTOP RUN."
        );
        let module = compile(&source).unwrap();
        let module =
            zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
        assert_eq!(run_module(module), expected, "{mode}");
    }
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 R PIC 9(31).\nPROCEDURE DIVISION.\nSTOP RUN.";
    assert!(compile(source).is_err());
    compile(&format!("CBL ARITH(EXTEND)\n{source}")).unwrap();
}

#[test]
fn intrinsic_all_subscripts_keep_fixed_indices_and_active_odo_counts() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9 VALUE 2.\n01 K PIC 9 VALUE 2.\n01 G.\n05 ROWS OCCURS 2.\n10 COL OCCURS 0 TO 3 DEPENDING ON N PIC 99.\n01 WORDS.\n05 WORD-ROW OCCURS 2.\n10 TXT OCCURS 2 PIC XX.\n01 R PIC 99.\nPROCEDURE DIVISION.\nMOVE 1 TO COL(1 1)\nMOVE 2 TO COL(1 2)\nMOVE 4 TO COL(2 1)\nMOVE 5 TO COL(2 2)\nCOMPUTE R = FUNCTION SUM(COL(K ALL))\nDISPLAY R\nCOMPUTE R = FUNCTION SUM(COL(ALL 2))\nDISPLAY R\nCOMPUTE R = FUNCTION SUM(COL(ALL ALL))\nDISPLAY R\nMOVE 'AA' TO TXT(1 1)\nMOVE 'BA' TO TXT(1 2)\nMOVE 'ZA' TO TXT(2 1)\nMOVE 'QA' TO TXT(2 2)\nDISPLAY FUNCTION MAX(TXT(2 ALL))\nCOMPUTE R = FUNCTION ORD-MIN(TXT(ALL 2))\nDISPLAY R\nSTOP RUN.";
    let module = compile(source).unwrap();
    let module =
        zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
    assert_eq!(run_module(module), "09\n07\n12\nZA\n01\n");
    let invalid = source.replace("01 K PIC 9 VALUE 2.", "01 K PIC 9 VALUE 0.");
    assert!(
        Machine::new(vec![])
            .run_main(compile(&invalid).unwrap())
            .unwrap_err()
            .0
            .contains("subscript out of bounds")
    );
    assert!(compile(&source.replace("COL(K ALL)", "COL(ALL)")).is_err());
}

#[test]
fn intrinsic_functions_diagnose_arity_category_and_scalar_all_constraints() {
    for expression in [
        "SQRT(1 2)",
        "CURRENT-DATE(1)",
        "CHAR()",
        "ABS('TEXT')",
        "MIN(1 'X')",
        "MOD(1)",
        "RANDOM(1 2)",
        "NUMVAL('1' '2')",
        "UPPER-CASE('AB' 'CD')",
        "YEAR-TO-YYYY(1 2 3)",
        "SUM(1)(1:1)",
        "SQRT(A(ALL))",
    ] {
        let source = format!(
            "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 G.\n05 A OCCURS 2 PIC 99.\nPROCEDURE DIVISION.\nDISPLAY FUNCTION {expression}\nSTOP RUN."
        );
        assert!(compile(&source).is_err(), "{expression}");
    }
}

#[test]
fn random_defaults_to_seed_zero_and_restarts_a_repeatable_open_interval_sequence() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A COMP-2.\n01 B COMP-2.\n01 C COMP-2.\nPROCEDURE DIVISION.\nCOMPUTE A = FUNCTION RANDOM\nCOMPUTE B = FUNCTION RANDOM(0)\nIF A = B DISPLAY 'DEFAULT' END-IF\nCOMPUTE B = FUNCTION RANDOM\nCOMPUTE C = FUNCTION RANDOM(0)\nCOMPUTE C = FUNCTION RANDOM\nIF B = C DISPLAY 'SEQUENCE' END-IF\nPERFORM 100 TIMES\nCOMPUTE A = FUNCTION RANDOM\nIF A NOT > 0 OR A NOT < 1\nMOVE 9 TO RETURN-CODE END-IF\nEND-PERFORM\nDISPLAY 'BOUNDS'\nSTOP RUN.";
    assert_eq!(run(source), "DEFAULT\nSEQUENCE\nBOUNDS\n");
    let mut machine = Machine::new(vec![]);
    machine.set_stdout(Box::new(std::io::sink()));
    assert_eq!(machine.run_main(compile(source).unwrap()).unwrap(), 0);
    for seed in ["-1", "1.5"] {
        let invalid = source.replace("RANDOM(0)", &format!("RANDOM({seed})"));
        assert!(
            Machine::new(vec![])
                .run_main(compile(&invalid).unwrap())
                .unwrap_err()
                .0
                .contains("nonnegative integer")
        );
    }
}

#[test]
fn random_preserves_existing_large_seed_sequences() {
    // A legacy Zeus sequence, not an assertion about IBM's random algorithm.
    // The integer projection avoids depending on insignificant HFP bits.
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC 9(9).\n01 B PIC 9(9).\nPROCEDURE DIVISION.\nCOMPUTE A = FUNCTION RANDOM(1234567890123456) * 1000000000\nCOMPUTE B = FUNCTION RANDOM(1234567890123456) * 1000000000\nIF A = B DISPLAY 'RESTART' END-IF\nDISPLAY A\nSTOP RUN.";
    assert_eq!(run(source), "RESTART\n936200262\n");
}

#[test]
fn numeric_text_functions_honor_national_currency_decimal_comma_and_arithmetic() {
    let source = "CBL ARITH(EXTEND)\nID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nCONFIGURATION SECTION.\nSPECIAL-NAMES.\nDECIMAL-POINT IS COMMA\nCURRENCY SIGN IS '@'.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 I PIC 9(31).\n01 R PIC S9(4)V9.\nPROCEDURE DIVISION.\nCOMPUTE I =\nFUNCTION NUMVAL('1234567890123456789012345678901')\nDISPLAY I\nIF FUNCTION NUMVAL(N'1,5') = 1,5\nDISPLAY 'NATIONAL' END-IF\nCOMPUTE R = FUNCTION NUMVAL-C(N'EUR1.234,5CR' N'EUR')\nIF R = -1234,5 DISPLAY 'CURRENCY' END-IF\nIF FUNCTION NUMVAL-C('@12,5') = 12,5\nDISPLAY 'DEFAULT-CURRENCY' END-IF\nIF FUNCTION TEST-NUMVAL('12A') = 3\nDISPLAY 'POSITION' END-IF\nIF FUNCTION TEST-NUMVAL('1.5') = 2\nDISPLAY 'COMMA' END-IF\nIF FUNCTION TEST-NUMVAL-C('eur12' 'EUR') = 1\nDISPLAY 'CASE' END-IF\nSTOP RUN.";
    let module = compile(source).unwrap();
    let module =
        zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
    assert_eq!(
        run_module(module),
        "1234567890123456789012345678901\nNATIONAL\nCURRENCY\nDEFAULT-CURRENCY\nPOSITION\nCOMMA\nCASE\n"
    );
    for expression in [
        "FUNCTION NUMVAL('1234567890123456789')",
        "FUNCTION NUMVAL-C('12' N'EUR')",
        "FUNCTION NUMVAL-C('12' 'E1')",
        "FUNCTION NUMVAL('1A2')",
    ] {
        let invalid = format!(
            "ID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nDISPLAY {expression}\nSTOP RUN."
        );
        assert!(
            Machine::new(vec![])
                .run_main(compile(&invalid).unwrap())
                .is_err()
        );
    }
}

#[test]
fn apost_and_quote_options_control_figuratives_in_values_and_executable_code() {
    let body = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC XXX VALUE QUOTES.\n01 B PIC XXX.\n01 N PIC N(2) VALUE QUOTE.\nPROCEDURE DIVISION.\nMOVE QUOTES TO B\nIF A = B AND B = QUOTES\nDISPLAY A '/' B '/' N\nEND-IF\nSTOP RUN.";
    for (option, quote) in [
        ("APOST", '\''),
        ("QUOTE", '"'),
        ("APOST QUOTE", '"'),
        ("QUOTE APOST", '\''),
    ] {
        let module = compile(&format!("CBL {option}\n{body}")).unwrap();
        let module =
            zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
        assert_eq!(
            run_module(module),
            format!(
                "{0}/{0}/{1}\n",
                quote.to_string().repeat(3),
                quote.to_string().repeat(2)
            )
        );
    }
    assert_eq!(run(body), "\"\"\"/\"\"\"/\"\"\n");
}

#[test]
fn integer_intrinsics_preserve_large_hfp_arguments_and_signed_remainders() {
    let source = "CBL ARITH(EXTEND)\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A COMP-2.\n01 B COMP-2.\n01 R PIC S9V9.\nPROCEDURE DIVISION.\nCOMPUTE A = 2.0E0 ** 200\nCOMPUTE B = FUNCTION INTEGER(A)\nIF A = B DISPLAY 'INTEGER' END-IF\nCOMPUTE B = FUNCTION INTEGER-PART(A)\nIF A = B DISPLAY 'PART' END-IF\nCOMPUTE R = FUNCTION MOD(A 3)\nIF R = 1 DISPLAY 'MOD' END-IF\nCOMPUTE R = FUNCTION MOD(-A 3)\nIF R = 2 DISPLAY 'NEGATIVE-MOD' END-IF\nCOMPUTE R = FUNCTION REM(-A 3)\nIF R = -1 DISPLAY 'REM' END-IF\nCOMPUTE R = FUNCTION REM(-11.5 5)\nIF R = -1.5 DISPLAY 'DECIMAL-REM' END-IF\nCOMPUTE R = FUNCTION INTEGER(-2.75)\nIF R = -3 DISPLAY 'FLOOR' END-IF\nCOMPUTE R = FUNCTION INTEGER-PART(-2.75)\nIF R = -2 DISPLAY 'TRUNCATE' END-IF\nIF FUNCTION ORD-MAX(A 1 A) = 1 DISPLAY 'MAX' END-IF\nIF FUNCTION ORD-MIN(A 1 A) = 2 DISPLAY 'MIN' END-IF\nSTOP RUN.";
    assert_eq!(
        run(source),
        "INTEGER\nPART\nMOD\nNEGATIVE-MOD\nREM\nDECIMAL-REM\nFLOOR\nTRUNCATE\nMAX\nMIN\n"
    );
}

#[test]
fn factorial_validates_arithmetic_mode_and_integral_argument() {
    let source = "CBL ARITH(EXTEND)\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 R PIC 9(31).\nPROCEDURE DIVISION.\nCOMPUTE R = FUNCTION FACTORIAL(29)\nDISPLAY R\nSTOP RUN.";
    assert_eq!(run(source), "8841761993739701954543616000000\n");
    for (mode, value) in [
        ("COMPAT", "29"),
        ("EXTEND", "30"),
        ("EXTEND", "-1"),
        ("EXTEND", "2.5"),
    ] {
        let bad = source
            .replace("ARITH(EXTEND)", &format!("ARITH({mode})"))
            .replace("9(31)", "9(18)")
            .replace("FACTORIAL(29)", &format!("FACTORIAL({value})"));
        let error = Machine::new(vec![])
            .run_main(compile(&bad).unwrap())
            .unwrap_err();
        assert!(error.0.contains("FACTORIAL argument"), "{error}");
    }
}

#[test]
fn hfp_addition_and_subtraction_discard_bits_below_the_hex_guard_digit() {
    for (mode, exponent) in [("COMPAT", 30), ("EXTEND", 50)] {
        let source = format!(
            "CBL ARITH({mode})\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 R PIC 9.\nPROCEDURE DIVISION.\nCOMPUTE R = 1 - 1.0E-{exponent}\nDISPLAY R\nCOMPUTE R = 1 + -1.0E-{exponent}\nDISPLAY R\nSTOP RUN."
        );
        assert_eq!(run(&source), "1\n1\n");
    }
}

#[test]
fn justified_national_and_dbcs_moves_pad_and_truncate_on_character_boundaries() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC N(4) JUSTIFIED VALUE N'AB'.\n01 D PIC G(3) DISPLAY-1 JUSTIFIED.\nPROCEDURE DIVISION.\nIF N = N'AB  ' DISPLAY 'VALUE' END-IF\nMOVE N'AB' TO N\nIF N = N'  AB' DISPLAY 'PAD' END-IF\nMOVE N'123456' TO N\nIF N = N'3456' DISPLAY 'TRUNCATE' END-IF\nMOVE N'XY' TO N(1:2)\nIF N = N'XY56' DISPLAY 'REFMOD' END-IF\nMOVE X'42434445' TO D\nIF D = X'404042434445' DISPLAY 'DBCS-PAD' END-IF\nMOVE X'464748494A4B4C4D' TO D\nIF D = X'48494A4B4C4D' DISPLAY 'DBCS-TRUNCATE' END-IF\nSTOP RUN.";
    assert_eq!(
        run(source),
        "VALUE\nPAD\nTRUNCATE\nREFMOD\nDBCS-PAD\nDBCS-TRUNCATE\n"
    );
}

#[test]
fn extended_functions_and_fractional_powers_retain_more_than_f64_precision() {
    let source = "CBL ARITH(EXTEND)\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 R PIC 9.9(30).\n01 Q PIC 99V99.\n01 M PIC 9V99.\nPROCEDURE DIVISION.\nCOMPUTE R ROUNDED = FUNCTION SQRT(2)\nDISPLAY R\nCOMPUTE R ROUNDED = 2 ** 0.5\nDISPLAY R\nCOMPUTE R ROUNDED = FUNCTION LOG(2)\nDISPLAY R\nCOMPUTE R ROUNDED = 2 ** -1\nDISPLAY R\nDIVIDE 5 BY 3 GIVING Q ROUNDED REMAINDER M\nDISPLAY Q '/' M\nSTOP RUN.";
    assert_eq!(
        run(source),
        "1.414213562373095048801688724210\n1.414213562373095048801688724210\n0.693147180559945309417232121458\n0.500000000000000000000000000000\n0167/002\n"
    );
}

#[test]
fn numproc_and_numcls_apply_the_documented_numeric_class_signs() {
    for (option, raw, expected) in [
        ("NUMPROC(PFD)", "F1C2", "Y"),
        ("NUMPROC(PFD)", "F1F2", "N"),
        ("NUMPROC(PFD)", "F0F0", "Y"),
        ("NUMPROC(NOPFD)", "F1F2", "Y"),
        ("NUMPROC(MIG)", "F1A2", "N"),
        ("NUMPROC(MIG),NUMCLS(ALT)", "F1A2", "Y"),
        ("NUMPROC(PFD),NUMCLS(ALT)", "F1A2", "N"),
    ] {
        let source = format!(
            "CBL {option}\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 RAW PIC XX VALUE X'{raw}'.\n01 N REDEFINES RAW PIC S99.\nPROCEDURE DIVISION.\nIF N IS NUMERIC DISPLAY 'Y' ELSE DISPLAY 'N' END-IF\nSTOP RUN."
        );
        assert_eq!(run(&source), format!("{expected}\n"), "{option}/{raw}");
    }
}

#[test]
fn external_floats_support_display_national_implied_points_and_arithmetic() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 F PIC +99.99E+99.\n01 N PIC -9V999E-99 USAGE NATIONAL SIGN LEADING.\n01 R PIC 9(6).\n01 B COMP-2.\nPROCEDURE DIVISION.\nMOVE 1234 TO F N\nDISPLAY F '/' N\nCOMPUTE R = F + N\nDISPLAY R\nIF F IS NUMERIC AND N IS NUMERIC DISPLAY 'Y' END-IF\nMOVE -0.00125 TO F N\nDISPLAY F '/' N\nINITIALIZE F\nDISPLAY F\nMOVE 2.5 TO B\nDISPLAY B\nSTOP RUN.";
    let module = compile(source).unwrap();
    let module =
        zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
    assert_eq!(
        run_module(module),
        "+12.34E+02/ 1234E 03\n002468\nY\n-12.50E-04/-1250E-03\n+00.00E+00\n 2.5000000000000000E+00\n"
    );
    for declaration in [
        "PIC +9.9E+99 VALUE 1",
        "PIC +9.9E+99 COMP",
        "PIC +9.9E+99 JUSTIFIED",
        "PIC +9.9E+99 BLANK WHEN ZERO",
        "PIC +99E+99",
        "PIC +9.9E+999",
    ] {
        assert!(compile(&format!("ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 F {declaration}.\nPROCEDURE DIVISION.\nSTOP RUN.")).is_err(), "{declaration}");
    }
}

#[test]
fn duplicate_exported_entry_names_are_diagnosed() {
    let source = "ID DIVISION.\nPROGRAM-ID. A.\nPROCEDURE DIVISION.\nGOBACK.\nENTRY 'B'.\nGOBACK.\nEND PROGRAM A.\nID DIVISION.\nPROGRAM-ID. B.\nPROCEDURE DIVISION.\nGOBACK.\nEND PROGRAM B.";
    assert!(compile(source).unwrap_err().message.contains("duplicate"));
}

#[test]
fn by_value_preserves_binary_float_and_character_representations() {
    let source = "CBL ARITH(EXTEND)\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 BIG PIC 9(18) COMP-5 VALUE 18446744073709551615.\n01 FL COMP-2 VALUE 2.5.\n01 RESULT PIC 99.\nPROCEDURE DIVISION.\nCALL 'SUB' USING BY VALUE BIG FL\n'A' N'界' BY REFERENCE RESULT\nDISPLAY RESULT '/' BIG\nCALL 'ALT' USING BY VALUE BIG BY REFERENCE RESULT\nDISPLAY RESULT '/' BIG\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nLINKAGE SECTION.\n01 X PIC 9(18) COMP-5.\n01 Y COMP-2.\n01 C PIC X.\n01 N PIC N.\n01 R PIC 99.\nPROCEDURE DIVISION USING BY VALUE X Y C N BY REFERENCE R.\nIF X = 18446744073709551615 AND Y = 2.5\nAND C = 'A' AND N = N'界' MOVE 1 TO R END-IF\nMOVE 0 TO X\nGOBACK.\nENTRY 'ALT' USING BY VALUE X BY REFERENCE R.\nIF X = 18446744073709551615 MOVE 2 TO R END-IF\nMOVE 0 TO X\nGOBACK.\nEND PROGRAM SUB.";
    assert_eq!(
        run(source),
        "01/18446744073709551615\n02/18446744073709551615\n"
    );
    let mismatch = source.replace(
        "CALL 'ALT' USING BY VALUE BIG",
        "CALL 'ALT' USING BY REFERENCE BIG",
    );
    assert!(
        Machine::new(vec![])
            .run_main(compile(&mismatch).unwrap())
            .unwrap_err()
            .0
            .contains("modes must agree")
    );
}

#[test]
fn conditional_compilation_selects_before_lexing_and_honors_parameters() {
    let source = "CBL DEFINE(Target='prod')\n>>DEFINE Target AS PARAMETER\n>>DEFINE Version AS 2 + 3\n>>DEFINE Enabled AS B'1'\nID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\n>>IF Target IS DEFINED AND Target = 'prod' AND Enabled\nDISPLAY 'parameter'\n>>ELSE\nthis is invalid COBOL @ 'unterminated\n>>END-IF\n>>EVALUATE Version\n>>WHEN 1\nINVALID @\n>>WHEN 4 THRU 6\nDISPLAY 'range'\n>>WHEN OTHER\nINVALID @\n>>END-EVALUATE\n>>DEFINE Enabled OFF\n>>IF Enabled IS NOT DEFINED\nDISPLAY 'off'\n>>END-IF\n>>DEFINE Version AS 7 OVERRIDE\n>>EVALUATE TRUE\n>>WHEN Version = 7\nDISPLAY 'override'\n>>WHEN OTHER\nINVALID @\n>>END-EVALUATE\nSTOP RUN.";
    assert_eq!(run(source), "parameter\nrange\noff\noverride\n");
    assert!(compile(&source.replace("AS 7 OVERRIDE", "AS 7")).is_err());
    assert!(compile(&source.replace(">>END-IF", ">>ELSE")).is_err());
}

#[test]
fn conditional_definitions_in_copy_members_affect_later_source_conditions() {
    let dir = tempfile::tempdir().unwrap();
    let member = "       >>DEFINE From-Copy AS 1\n       >>IF From-Copy = 1\n       01 X PIC 9 VALUE 7.\n       >>ELSE\n       INVALID @\n       >>END-IF\n";
    std::fs::write(dir.path().join("DEFS.cpy"), member).unwrap();
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\nCOPY DEFS.\nPROCEDURE DIVISION.\n>>IF From-Copy IS DEFINED\nDISPLAY X\n>>ELSE\nINVALID @\n>>END-IF\nSTOP RUN.";
    let path = dir.path().join("MAIN.cbl");
    std::fs::write(
        &path,
        source
            .lines()
            .map(|l| format!("       {l}\n"))
            .collect::<String>(),
    )
    .unwrap();
    assert_eq!(
        run_module(zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap()),
        "7\n"
    );
}

#[test]
fn conditional_directives_count_as_evaluate_bodies_and_validate_operands() {
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\n>>DEFINE Result AS 0\n>>EVALUATE 1\n>>WHEN 1\n>>DEFINE Result AS 1 OVERRIDE\n>>WHEN 2\n>>DEFINE Result AS 2 OVERRIDE\n>>END-EVALUATE\n>>IF Result = 1\nDISPLAY 'selected'\n>>ELSE\nINVALID @\n>>END-IF\nSTOP RUN.";
    assert_eq!(run(source), "selected\n");
    for directive in [
        ">>DEFINE Result OFF EXTRA",
        ">>ELSE EXTRA",
        ">>END-IF EXTRA",
    ] {
        let bad = if directive.contains("OFF") {
            source.replace(">>DEFINE Result AS 0", directive)
        } else if directive.contains("ELSE") {
            source.replace(">>ELSE", directive)
        } else {
            source.replace(">>END-IF", directive)
        };
        assert!(compile(&bad).is_err());
    }
}

#[test]
fn copy_replacing_preserves_directives_and_rejects_nested_replacement_chains() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("MAIN.cbl");
    let source = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\nCOPY DEFS REPLACING ==:TAG:== BY ==NEW==.\nPROCEDURE DIVISION.\nDISPLAY NEW-N\nSTOP RUN.";
    std::fs::write(
        &path,
        source
            .lines()
            .map(|line| format!("       {line}\n"))
            .collect::<String>(),
    )
    .unwrap();
    std::fs::write(dir.path().join("DEFS.cpy"), "       >>DEFINE Label AS ':TAG:'\n       >>IF Label = ':TAG:'\n       01 :TAG:-N PIC 9 VALUE 7.\n       >>ELSE\n       INVALID @\n       >>END-IF\n").unwrap();
    assert_eq!(
        run_module(zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap()),
        "7\n"
    );
    std::fs::write(dir.path().join("INNER.cpy"), "       01 OLD PIC 9.\n").unwrap();
    for member in [
        "       COPY INNER REPLACING ==OLD== BY ==NEW-N==.\n",
        "       >>IF TRUE\n       COPY INNER REPLACING ==OLD== BY ==NEW-N==.\n       >>END-IF\n",
    ] {
        std::fs::write(dir.path().join("DEFS.cpy"), member).unwrap();
        let error = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap_err();
        assert!(error.message.contains("nested COPY chain"), "{error:?}");
    }
}

#[test]
fn alternate_entries_start_at_their_own_statement_and_bind_their_own_arguments() {
    let src = "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC 999 VALUE 1.\n01 B PIC 999 VALUE 2.\n01 P FUNCTION-POINTER.\nPROCEDURE DIVISION.\nCALL 'SUB' USING A\nCALL 'ALT' USING B\nSET P TO ENTRY 'OTHER'\nCALL P USING A B\nDISPLAY A '/' B\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nLINKAGE SECTION.\n01 X PIC 999.\n01 Y PIC 999.\nPROCEDURE DIVISION USING X.\nADD 1 TO X\nGOBACK.\nENTRY 'ALT' USING Y.\nADD 10 TO Y\nGOBACK.\nENTRY 'OTHER' USING Y X.\nADD 100 TO X\nADD 20 TO Y\nGOBACK.\nEND PROGRAM SUB.";
    let module = compile(src).unwrap();
    let module =
        zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&module).unwrap()).unwrap();
    assert_eq!(run_module(module), "022/112\n");
}

#[test]
fn exported_entry_is_resolved_from_a_differently_named_object() {
    let module = compile("ID DIVISION.\nPROGRAM-ID. SUB.\nPROCEDURE DIVISION.\nDISPLAY 'WRONG'\nGOBACK.\nENTRY 'ALT'.\nDISPLAY 'ALT'\nGOBACK.").unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("SUB.zo"),
        zeus_cobol::module_bytes(&module).unwrap(),
    )
    .unwrap();
    let mut vm = Machine::new(vec![dir.path().to_path_buf()]);
    let output = Capture::default();
    vm.set_stdout(Box::new(output.clone()));
    vm.call_program("ALT", &[]).unwrap().unwrap();
    assert_eq!(&*output.0.borrow(), b"ALT\n");
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

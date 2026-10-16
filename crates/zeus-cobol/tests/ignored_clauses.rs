//! Specifications and inventory: docs/proposal/0015-cobol-ignored-clauses.md.
use zeus_cobol::{
    front::{self, FileTable, FrontOptions},
    vm::Machine,
};

fn compile(src: &str) -> Result<zeus_cobol::zir::Module, zeus_cobol::diag::Diag> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("TEST.cbl");
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

fn file_source(fd: &str) -> String {
    format!(
        "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO OUTFILE.\nDATA DIVISION.\nFILE SECTION.\nFD F {fd}.\n01 R PIC X(3).\nPROCEDURE DIVISION.\nOPEN OUTPUT F\nMOVE 'ABC' TO R\nWRITE R\nCLOSE F\nSTOP RUN."
    )
}

#[test]
fn recording_mode_controls_disk_format_independently_of_length() {
    for (clause, expected) in [
        ("RECORDING MODE F", &b"\xc1\xc2\xc3"[..]),
        ("RECORDING MODE V", &b"\x00\x07\x00\x00\xc1\xc2\xc3"[..]),
        (
            "RECORD IS VARYING FROM 3 TO 3",
            &b"\x00\x07\x00\x00\xc1\xc2\xc3"[..],
        ),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.dat");
        let mut module = compile(&file_source(clause)).unwrap();
        module.programs[0].files[0].assign = path.to_string_lossy().into_owned();
        Machine::new(vec![]).run_main(module).unwrap();
        assert_eq!(std::fs::read(path).unwrap(), expected, "{clause}");
    }
}

#[test]
fn recording_mode_validation_and_unsupported_block_formats() {
    for (clause, message) in [
        ("RECORDING MODE BAD", "must be F, V, U or S"),
        ("RECORDING MODE S", "requires BLOCK CONTAINS"),
        (
            "RECORDING MODE U BLOCK CONTAINS 3 CHARACTERS",
            "cannot specify BLOCK",
        ),
    ] {
        assert!(
            compile(&file_source(clause))
                .unwrap_err()
                .message
                .contains(message)
        );
    }
}

#[test]
fn source_options_apply_before_lexing_and_preserve_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("opts.cbl");
    for (line, default, national, cics) in [
        ("000100 CBL NSYMBOL(DBCS),CICS", false, false, true),
        ("       PROCESS NS(NAT),NOCICS", true, true, false),
        (
            "       CBL NS(DBCS),NS(NAT),CICS,NOCICS",
            false,
            true,
            false,
        ),
        ("       CBL LIST,MAP,SOURCE", true, false, true),
    ] {
        std::fs::write(&path, format!("{line}\n       DISPLAY N'AB'.\n")).unwrap();
        let opts = FrontOptions {
            nsymbol_dbcs: default,
            cics: true,
            ..Default::default()
        };
        let (tokens, translated) =
            front::source_tokens(&path, &opts, &mut FileTable::default()).unwrap();
        assert_eq!(translated, cics);
        assert_eq!(
            tokens
                .iter()
                .any(|t| matches!(t.tok, front::lexer::Tok::National(_))),
            national
        );
    }
}

#[test]
fn source_options_change_picture_category_and_unknown_settings_fail() {
    for (opt, expected) in [("NS(DBCS)", false), ("NS(NAT)", true)] {
        let m = compile(&format!("CBL {opt}\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC N(2).\nPROCEDURE DIVISION.\nSTOP RUN.")).unwrap();
        assert_eq!(
            format!("{:?}", m.programs[0].descs).contains("National"),
            expected
        );
    }
    for opt in [
        "TRUNC(BAD)",
        "ARITH(EXTEND)",
        "NUMPROC(PFD)",
        "NS(BAD)",
        "NS(DBCS",
        "CICS(BOGUS)",
    ] {
        assert!(
            compile(&format!(
                "CBL {opt}\nID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nSTOP RUN."
            ))
            .is_err(),
            "{opt}"
        );
    }
}

#[test]
fn computer_no_effect_clauses_are_checked() {
    let source = |clauses: &str| {
        format!(
            "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nCONFIGURATION SECTION.\n{clauses}\nPROCEDURE DIVISION.\nSTOP RUN."
        )
    };
    compile(&source("SOURCE-COMPUTER. IBM WITH DEBUGGING MODE.\nOBJECT-COMPUTER. IBM MEMORY SIZE 4096 WORDS\nSEGMENT-LIMIT IS 49.")).unwrap();
    for clause in [
        "MEMORY SIZE BAD WORDS",
        "MEMORY SIZE 3 BANANAS",
        "SEGMENT-LIMIT 0",
        "SEGMENT-LIMIT 50",
        "GARBAGE",
    ] {
        assert!(compile(&source(&format!("OBJECT-COMPUTER. IBM {clause}."))).is_err());
    }
}

#[test]
fn unimplemented_io_controls_and_open_options_are_not_swallowed() {
    {
        let clause = "RERUN ON F EVERY 10 RECORDS";
        let src = file_source("").replace(
            "DATA DIVISION.",
            &format!("I-O-CONTROL.\n{clause}.\nDATA DIVISION."),
        );
        assert!(compile(&src).is_err(), "{clause}");
    }
    for opt in ["REVERSED", "WITH NO REWIND"] {
        let src = file_source("").replace("OPEN OUTPUT F", &format!("OPEN INPUT F {opt}"));
        assert!(compile(&src).is_ok(), "{opt}");
    }
}

#[test]
fn code_set_translates_record_payload_but_preserves_rdw() {
    for alphabet in ["STANDARD-1", "STANDARD-2", "NATIVE", "EBCDIC"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ascii.dat");
        let src = file_source("RECORDING MODE V CODE-SET CS")
            .replace("INPUT-OUTPUT SECTION.", &format!("CONFIGURATION SECTION.\nSPECIAL-NAMES. ALPHABET CS IS {alphabet}.\nINPUT-OUTPUT SECTION."));
        let mut module = compile(&src).unwrap();
        module.programs[0].files[0].assign = path.to_string_lossy().into_owned();
        // Ensure object serialization retains the source settings.
        let bytes = serde_json::to_vec(&module).unwrap();
        let module = serde_json::from_slice(&bytes).unwrap();
        Machine::new(vec![]).run_main(module).unwrap();
        let expected = if alphabet.starts_with("STANDARD") {
            &b"\0\x07\0\0ABC"[..]
        } else {
            &b"\0\x07\0\0\xc1\xc2\xc3"[..]
        };
        assert_eq!(std::fs::read(&path).unwrap(), expected);
        let input = src.replace(
            "OPEN OUTPUT F\nMOVE 'ABC' TO R\nWRITE R",
            "OPEN INPUT F\nREAD F\nIF R NOT = 'ABC' MOVE 9 TO RETURN-CODE END-IF",
        );
        let mut module = compile(&input).unwrap();
        module.programs[0].files[0].assign = path.to_string_lossy().into_owned();
        assert_eq!(Machine::new(vec![]).run_main(module).unwrap(), 0);
    }
}

#[test]
fn code_set_checks_alphabet_and_record_usage() {
    let src = file_source("CODE-SET CS").replace(
        "INPUT-OUTPUT SECTION.",
        "CONFIGURATION SECTION.\nSPECIAL-NAMES. ALPHABET CS IS STANDARD-1.\nINPUT-OUTPUT SECTION.",
    );
    for pic in ["PIC S9(3)", "PIC 9(3) COMP", "PIC N(3)"] {
        assert!(
            compile(&src.replace("PIC X(3)", pic))
                .unwrap_err()
                .message
                .contains("CODE-SET requires")
        );
    }
    compile(&src.replace("PIC X(3)", "PIC S9(3) SIGN TRAILING SEPARATE")).unwrap();
    compile(&src.replace(
        "01 R PIC X(3).",
        "01 R SIGN TRAILING SEPARATE.\n05 N PIC S9(3).",
    ))
    .unwrap();
    assert!(
        compile(&file_source("CODE-SET MISSING"))
            .unwrap_err()
            .message
            .contains("not defined")
    );
}

#[test]
fn same_area_shares_vsam_record_storage_and_checks_open_exclusion() {
    let src = "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO FOUT ORGANIZATION RELATIVE.\nSELECT G ASSIGN TO GOUT ORGANIZATION RELATIVE.\nI-O-CONTROL. SAME AREA FOR F G.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 RF PIC X(3).\nFD G.\n01 RG PIC X(5).\nPROCEDURE DIVISION.\nOPEN OUTPUT F\nOPEN OUTPUT G\nSTOP RUN.";
    let mut module = compile(src).unwrap();
    let code = &module.programs[0];
    assert_eq!(
        code.descs[code.files[0].record as usize].offset,
        code.descs[code.files[1].record as usize].offset
    );
    let dir = tempfile::tempdir().unwrap();
    for (i, f) in module.programs[0].files.iter_mut().enumerate() {
        f.assign = dir
            .path()
            .join(format!("{i}.dat"))
            .to_string_lossy()
            .into_owned();
    }
    assert!(
        Machine::new(vec![])
            .run_main(module)
            .unwrap_err()
            .to_string()
            .contains("SAME AREA")
    );
    let qsam = src.replace("ORGANIZATION RELATIVE", "ORGANIZATION SEQUENTIAL");
    let m = compile(&qsam).unwrap();
    assert!(m.programs[0].files.iter().all(|f| f.same_area.is_empty()));
}

#[test]
fn no_effect_padding_clause_still_checks_its_operand() {
    for value in ["' '", "N' '"] {
        let src = file_source("").replace(
            "ASSIGN TO OUTFILE.",
            &format!("ASSIGN TO OUTFILE PADDING CHARACTER {value}."),
        );
        compile(&src).unwrap();
    }
    for value in ["'AB'", "123", "N'AB'"] {
        let src = file_source("").replace(
            "ASSIGN TO OUTFILE.",
            &format!("ASSIGN TO OUTFILE PADDING CHARACTER {value}."),
        );
        assert!(
            compile(&src)
                .unwrap_err()
                .message
                .contains("PADDING CHARACTER")
        );
    }
}

#[test]
fn user_labels_and_screen_are_diagnosed() {
    assert!(
        compile(&file_source("LABEL RECORDS R"))
            .unwrap_err()
            .message
            .contains("tape label")
    );
    let src = "ID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nSCREEN SECTION.\n01 S.\nPROCEDURE DIVISION.\nSTOP RUN.";
    assert!(compile(src).unwrap_err().message.contains("SCREEN SECTION"));
}

#[test]
fn documentation_clauses_keep_valid_source_accepted() {
    let src = file_source("LABEL RECORDS STANDARD DATA RECORDS R").replace(
        "DATA DIVISION.",
        "I-O-CONTROL.\nMULTIPLE FILE TAPE CONTAINS F POSITION 1.\nDATA DIVISION.",
    );
    compile(&src).unwrap();
    let bad = src.replace("POSITION 1", "POSITION WRONG");
    assert!(compile(&bad).is_err());
}

#[test]
fn global_file_code_set_uses_its_declaring_environment() {
    let src = "ID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nCONFIGURATION SECTION.\nSPECIAL-NAMES. ALPHABET CS IS STANDARD-1.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO OUTFILE.\nDATA DIVISION.\nFILE SECTION.\nFD F GLOBAL CODE-SET CS.\n01 R PIC X(3).\nPROCEDURE DIVISION.\nSTOP RUN.\nID DIVISION.\nPROGRAM-ID. CHILD.\nPROCEDURE DIVISION.\nGOBACK.\nEND PROGRAM CHILD.\nEND PROGRAM T.";
    let m = compile(src).unwrap();
    assert!(m.programs.iter().all(|p| p.files[0].code_set_ascii));
}

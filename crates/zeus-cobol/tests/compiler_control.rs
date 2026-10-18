use std::{cell::RefCell, io::Write, path::Path, rc::Rc};
use zeus_cobol::{front::FrontOptions, vm::Machine};

#[test]
fn sqlims_requires_its_explicit_compiler_option() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sqlims.cbl");
    for position in ["DATA", "PROCEDURE"] {
        let (declarations, statements) = if position == "DATA" {
            ("EXEC SQLIMS BEGIN DECLARE SECTION END-EXEC", "CONTINUE")
        } else {
            ("01 N PIC 9.", "EXEC SQLIMS COMMIT END-EXEC")
        };
        source(
            &path,
            &format!(
                "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{declarations}\nPROCEDURE DIVISION.\n{statements}\nSTOP RUN."
            ),
        );
        let error = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap_err();
        assert!(error.message.contains("requires SQLIMS"), "{error}");
    }
}

#[test]
fn sqlims_validates_declarations_and_effective_lp_option() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sqlims-options.cbl");
    let frame = |data: &str, body: &str| {
        format!(
            "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n{data}\nPROCEDURE DIVISION.\n{body}\nGOBACK."
        )
    };
    for (data, body, expected) in [
        (
            "EXEC SQLIMS BEGIN DECLARE SECTION END-EXEC",
            "CONTINUE",
            "END DECLARE SECTION",
        ),
        (
            "EXEC SQLIMS PREPARE S FROM :V END-EXEC",
            "CONTINUE",
            "DATA DIVISION",
        ),
        (
            "01 V PIC X(80).",
            "EXEC SQLIMS PREPARE S FROM :V END-EXEC",
            "SQLIMSCA",
        ),
        (
            "01 SQLIMSCA PIC X(136).\n01 V PIC X(80).",
            "EXEC SQLIMS PREPARE S FROM :V END-EXEC",
            "224 bytes",
        ),
        (
            "",
            "EXEC SQLIMS BEGIN DECLARE SECTION END-EXEC",
            "DATA DIVISION",
        ),
        (
            "",
            "EXEC SQLIMS DECLARE C CURSOR FOR SELECT * FROM T END-EXEC",
            "statement-name",
        ),
    ] {
        source(&path, &frame(data, body));
        let error = zeus_cobol::compile_file(
            &path,
            &FrontOptions {
                compiler_options: "SQLIMS".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(error.message.contains(expected), "{error}");
    }
    source(&path, &frame("", "EXEC SQLIMS CLOSE C END-EXEC"));
    let error = zeus_cobol::compile_file(
        &path,
        &FrontOptions {
            compiler_options: "SQLIMS LP(64)".into(),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(error.message.contains("LP(32)"), "{error}");
    source(
        &path,
        "ID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\n>>IF NOT IGY-SQLIMS\nTHIS MUST NOT BE COMPILED\n>>END-IF\nGOBACK.",
    );
    zeus_cobol::compile_file(
        &path,
        &FrontOptions {
            compiler_options: "SQLIMS".into(),
            ..Default::default()
        },
    )
    .unwrap();
}

#[test]
fn sqlims_cursor_declarations_have_lexical_scope_and_unique_statements() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sqlims-scope.cbl");
    for (body, expected) in [
        (
            "EXEC SQLIMS OPEN C END-EXEC\nEXEC SQLIMS DECLARE C CURSOR FOR S END-EXEC",
            "declared before use",
        ),
        (
            "EXEC SQLIMS DECLARE C CURSOR FOR S END-EXEC\nEXEC SQLIMS DECLARE D CURSOR FOR S END-EXEC",
            "already declared",
        ),
        ("EXEC SQLIMS PREPARE S FROM :SQL-TEXT END-EXEC", "reserved"),
    ] {
        source(
            &path,
            &format!(
                "ID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\nEXEC SQLIMS INCLUDE SQLIMSCA END-EXEC.\n01 SQL-TEXT PIC X(80).\nPROCEDURE DIVISION.\n{body}\nGOBACK."
            ),
        );
        let error = zeus_cobol::compile_file(
            &path,
            &FrontOptions {
                compiler_options: "SQLIMS".into(),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(error.message.contains(expected), "{error}");
    }
}

#[test]
fn flag_levels_filter_warning_summaries_and_source_embedding_without_changing_rc() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("flag.cbl");
    source(
        &path,
        "ID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\n>>CALLINT DYNAMIC\nCALL 'MISSING' ON EXCEPTION CONTINUE END-CALL\n>>CALLINT STATIC\nCALL 'MISSING' ON EXCEPTION CONTINUE END-CALL\nSTOP RUN.",
    );
    for (options, count, embedded) in [
        ("FLAG(I,I)", 2, true),
        ("F(W,W)", 2, true),
        ("FLAG(W)", 1, false),
        ("FLAG(I,S)", 1, false),
        ("FLAG(I,U)", 1, false),
        ("FLAG(E)", 0, false),
        ("NOFLAG", 0, false),
        ("NOF", 0, false),
        ("NOSOURCE FLAG(W)", 1, false),
    ] {
        let (module, listing) = zeus_cobol::compile_file_with_listing(
            &path,
            &FrontOptions {
                compiler_options: options.into(),
                ..Default::default()
            },
        );
        let module = module.unwrap();
        assert_eq!(module.compilation_return_code(), 4, "{options}");
        assert_eq!(module.diagnostics.len(), 1);
        assert_eq!(
            listing.matches("W: conflicting CALLINTERFACE").count(),
            count,
            "{options}\n{listing}"
        );
        assert_eq!(listing.contains("==0001:000007==>"), embedded, "{options}");
        assert!(listing.contains("END OF COMPILATION; RC=4"));
    }
}

#[test]
fn flag_keeps_option_errors_visible_and_copy_errors_out_of_source_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("flag.cbl");
    let text = "ID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nMOVE 1 TO MISSING\nSTOP RUN.";
    source(&path, text);
    let options = |text: &str| FrontOptions {
        compiler_options: text.into(),
        ..Default::default()
    };
    let (result, listing) = zeus_cobol::compile_file_with_listing(&path, &options("NOFLAG"));
    assert_eq!(result.unwrap_err().severity.return_code(), 12);
    assert!(!listing.contains("\nDIAGNOSTICS\n"));
    assert!(!listing.contains("==0001:"));
    assert!(listing.contains("END OF COMPILATION; RC=12"));
    for setting in [
        "NOFLAG BOGUS",
        "FLAG(S,W)",
        "FLAG(W,)",
        "FLAG(W,W,I)",
        "NOSOURCE FLAG(I,W)",
    ] {
        let (result, listing) = zeus_cobol::compile_file_with_listing(&path, &options(setting));
        assert!(result.is_err(), "{setting}");
        assert!(listing.contains("\nDIAGNOSTICS\n"), "{setting}\n{listing}");
        assert!(!listing.contains("==0001:"));
    }
    source(&path, &text.replace("MOVE 1 TO MISSING", "COPY ABSENT."));
    let (result, listing) = zeus_cobol::compile_file_with_listing(&path, &options("FLAG(I,I)"));
    assert!(result.unwrap_err().message.contains("COPY"));
    assert!(listing.contains("\nDIAGNOSTICS\n"));
    assert!(!listing.contains("==0001:"));
}

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
fn run(module: zeus_cobol::zir::Module) -> Vec<u8> {
    let out = Capture::default();
    let mut vm = Machine::new(vec![]);
    vm.set_stdout(Box::new(out.clone()));
    vm.run_main(module).unwrap();
    out.0.borrow().clone()
}

#[test]
fn predefined_variables_follow_effective_options_and_copy_expansion() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    source(
        &dir.path().join("FLAGS.cpy"),
        ">>IF IGY-DYNAM = B'1' AND DYNAM\nDISPLAY 'DYNAM'\n>>END-IF\n>>IF IGY-OPTIMIZE = 2 AND OPTIMIZE = 2\nDISPLAY 'OPT'\n>>END-IF\n>>IF IGY-SQL AND SQL\nDISPLAY 'SQL'\n>>END-IF",
    );
    source(
        &path,
        "CBL DYNAM OPT(FULL) SQL LP(32)\nID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nCOPY FLAGS.\n>>IF NOT IGY-CICS AND NOT IGY-DLL AND NOT IGY-THREAD\nDISPLAY 'FLAGS'\n>>END-IF\n>>IF IGY-LP = 32 AND NOT IGY-SQLIMS\nDISPLAY 'LP'\n>>END-IF\n>>IF IGY-COMPILER-VRM = COMPILER-VRM\nDISPLAY 'VERSION'\n>>END-IF\nSTOP RUN.",
    );
    let module = zeus_cobol::compile_file(
        &path,
        &FrontOptions {
            compiler_options: "NODYNAM NOOPT NOSQL".into(),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(module.optimize, 2);
    assert!(module.programs[0].dynam);
    assert_eq!(run(module), b"DYNAM\nOPT\nSQL\nFLAGS\nLP\nVERSION\n");
    source(
        &path,
        "ID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\n>>IF NOT IGY-DYNAM AND IGY-OPTIMIZE = 0\nDISPLAY 'DEFAULT'\n>>END-IF\nSTOP RUN.",
    );
    assert_eq!(
        run(zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap()),
        b"DEFAULT\n"
    );
    source(
        &path,
        "CBL CICS\nID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\n>>IF IGY-CICS AND CICS\nDISPLAY 'CICS'\n>>END-IF\nSTOP RUN.",
    );
    assert_eq!(
        run(zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap()),
        b"CICS\n"
    );
}

#[test]
fn predefined_names_cannot_be_changed_or_removed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    for directive in [
        ">>DEFINE IGY-DYNAM AS B'1' OVERRIDE",
        ">>DEFINE DYNAM OFF",
        ">>DEFINE IGY-PRIVATE 1",
        ">>DEFINE COMPILER-VRM AS PARAMETER",
    ] {
        source(
            &path,
            &format!("ID DIVISION.\nPROGRAM-ID. T.\n{directive}\nPROCEDURE DIVISION.\nSTOP RUN."),
        );
        let error = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap_err();
        assert!(error.message.contains("read-only"), "{error}");
    }
    source(
        &path,
        "CBL DEFINE(IGY-CICS=B'1')\nID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nSTOP RUN.",
    );
    assert!(
        zeus_cobol::compile_file(&path, &FrontOptions::default())
            .unwrap_err()
            .message
            .contains("read-only")
    );
}

#[test]
fn listings_show_copy_replacements_storage_resolved_references_and_code() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    source(
        &dir.path().join("FIELDS.cpy"),
        "01 OLD-NAME PIC 99 VALUE 1.\n01 UNUSED PIC X.",
    );
    source(
        &path,
        "CBL SOURCE MAP LIST XREF(FULL) VBREF\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\nCOPY FIELDS REPLACING ==OLD-NAME== BY ==COUNTER==.\nPROCEDURE DIVISION.\nADD 1 TO COUNTER\nDISPLAY COUNTER\n>>IF B'0'\nSUBTRACT 1 FROM COUNTER\n>>END-IF\nSTOP RUN.",
    );
    let (module, listing) = zeus_cobol::compile_file_with_listing(&path, &FrontOptions::default());
    assert_eq!(run(module.unwrap()), b"02\n");
    for text in [
        "FIELDS.cpy",
        "EXPANDED ACTIVE TOKENS",
        "01 COUNTER",
        "DATA MAP",
        "COUNTER Static",
        "ZIR CODE LIST",
        "INITIAL PRIVATE STORAGE",
        "DATA CROSS-REFERENCE",
        "COUNTER DEF=2:1",
        "UNUSED DEF=2:2",
        "VERB CROSS-REFERENCE",
        "ADD 1:8",
        "DISPLAY 1:9",
    ] {
        assert!(listing.contains(text), "missing {text}\n{listing}");
    }
    assert!(
        !listing
            .split("VERB CROSS-REFERENCE")
            .nth(1)
            .unwrap()
            .contains("SUBTRACT")
    );

    source(
        &path,
        "CBL NOSOURCE NOMAP OFFSET XREF(SHORT) NOVBREF\nID DIVISION.\nPROGRAM-ID. T.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 UNUSED PIC X.\nPROCEDURE DIVISION.\nSTOP RUN.",
    );
    let (module, listing) = zeus_cobol::compile_file_with_listing(&path, &FrontOptions::default());
    module.unwrap();
    assert!(listing.contains("OFFSET TABLE"));
    for suppressed in [
        "EXPANDED ACTIVE TOKENS",
        "DATA MAP",
        "ZIR CODE LIST",
        "UNUSED",
        "VERB CROSS-REFERENCE",
    ] {
        assert!(
            !listing.contains(suppressed),
            "unexpected {suppressed}\n{listing}"
        );
    }
}

#[test]
fn listing_conflicts_respect_option_levels_and_errors_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    source(
        &path,
        "CBL LIST\nID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nSTOP RUN.",
    );
    let opts = FrontOptions {
        compiler_options: "OFFSET".into(),
        ..Default::default()
    };
    let (module, listing) = zeus_cobol::compile_file_with_listing(&path, &opts);
    module.unwrap();
    assert!(listing.contains("ZIR CODE LIST"));
    assert!(!listing.contains("OFFSET TABLE"));
    source(
        &path,
        "CBL LIST OFFSET\nID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nSTOP RUN.",
    );
    let (module, listing) = zeus_cobol::compile_file_with_listing(&path, &FrontOptions::default());
    assert!(module.unwrap_err().message.contains("mutually exclusive"));
    assert!(listing.contains("DIAGNOSTICS"));
    assert!(listing.contains("RC=12"));
    let (_, listing) = zeus_cobol::compile_file_with_listing(
        &path,
        &FrontOptions {
            compiler_options: "NOSOURCE".into(),
            ..Default::default()
        },
    );
    assert!(!listing.contains("       PROGRAM-ID"));
}

#[test]
fn cli_writes_listings_on_success_and_on_compilation_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    let listing = dir.path().join("main.lst");
    let artifact = dir.path().join("main.zo");
    source(
        &path,
        "ID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nSTOP RUN.",
    );
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_zcobc"))
        .args([
            "compile",
            "--options",
            "OPT(2),LIST,NOSOURCE,NOXREF",
            "--listing",
        ])
        .arg(&listing)
        .arg("-o")
        .arg(&artifact)
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        std::fs::read_to_string(&listing)
            .unwrap()
            .contains("ZIR CODE LIST")
    );
    assert_eq!(zeus_cobol::load_module(&artifact).unwrap().optimize, 2);
    let previous = std::fs::read(&path).unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_zcobc"))
        .args(["compile", "--listing"])
        .arg(&path)
        .arg(&path)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(std::fs::read(&path).unwrap(), previous);
    source(
        &path,
        "ID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nTHIS IS INVALID.\nSTOP RUN.",
    );
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_zcobc"))
        .args(["compile", "--listing"])
        .arg(&listing)
        .arg(&path)
        .output()
        .unwrap();
    assert!(!result.status.success());
    let listing = std::fs::read_to_string(listing).unwrap();
    assert!(listing.contains("DIAGNOSTICS"));
    assert!(listing.contains("RC=12"));
}

#[test]
fn explicit_nosql_disables_translation_but_default_auto_mode_is_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    source(
        &path,
        "CBL NOSQL\nID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nEXEC SQL COMMIT END-EXEC\nSTOP RUN.",
    );
    assert!(
        zeus_cobol::compile_file(&path, &FrontOptions::default())
            .unwrap_err()
            .message
            .contains("NOSQL")
    );
    source(
        &path,
        "ID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\nEXEC SQL COMMIT END-EXEC\nSTOP RUN.",
    );
    zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap();
}

#[test]
fn cross_references_resolve_procedure_and_file_targets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    source(
        &path,
        "CBL NOSOURCE XREF\nID DIVISION.\nPROGRAM-ID. T.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO OUTFILE.\nDATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC X.\nPROCEDURE DIVISION.\nPERFORM WRITER\nSTOP RUN.\nWRITER.\nOPEN OUTPUT F\nWRITE R FROM 'X'\nCLOSE F.",
    );
    let (module, listing) = zeus_cobol::compile_file_with_listing(&path, &FrontOptions::default());
    module.unwrap();
    let procedures = listing
        .split("PROCEDURE CROSS-REFERENCE")
        .nth(1)
        .unwrap()
        .split("FILE CROSS-REFERENCE")
        .next()
        .unwrap();
    assert!(procedures.contains("WRITER"));
    assert!(procedures.contains("DEF=1:15 1:13"), "{procedures}");
    assert!(
        listing.contains("F ASSIGN=OUTFILE 1:16 1:17 1:18"),
        "{listing}"
    );
}

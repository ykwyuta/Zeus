use zeus_cobol::hlasm;
fn assemble(text: &str, mut options: hlasm::AssemblyOptions) -> hlasm::AssemblyReport {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("process.asm");
    std::fs::write(&path, text).unwrap();
    options.collect_listing = true;
    hlasm::compile_report_with_options(&path, &[], &options)
}
const BODY: &str = "MAIN CSECT\n PRINT ON\n BR 14\nVALUE DC CL16'ABCDEFGHIJKLMNOP'\n END\n";

#[test]
fn initial_process_options_change_listing_without_changing_macro_statement_numbers_or_modules() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    std::fs::write(&path,"*PROCESS PCONTROL(ON,DATA) trailing remarks are not options\n*PROCESS OPTABLE(ZEUS)\nMAIN CSECT\n PRINT OFF,NODATA\n BR 14\nVALUE DC CL16'ABCDEFGHIJKLMNOP'\n END\n").unwrap();
    let plain = hlasm::compile_report(&path, &[]);
    assert!(plain.module.is_some(), "{:?}", plain.diagnostics);
    assert_eq!(plain.return_code(), 0);
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert_eq!(
        serde_json::to_value(plain.module).unwrap(),
        serde_json::to_value(&report.module).unwrap()
    );
    let listing = report.listing.unwrap();
    assert_eq!(listing.process_statements.len(), 2);
    assert_eq!(listing.process_statements[0].options, "PCONTROL(ON,DATA)");
    assert_eq!(listing.process_statements[0].line, 1);
    assert_eq!(listing.statements[0].location.statement, 1);
    assert_eq!(listing.statements[0].location.line, 3);
    assert!(listing.statements.iter().all(|s| s.operation != "*PROCESS"));
    assert!(listing.pcontrol["ON"] && listing.pcontrol["DATA"]);
    let text = listing.text();
    assert!(text.contains("*PROCESS PCONTROL(ON,DATA)"));
    assert!(text.contains("C1C2C3C4C5C6C7C8C9D1D2D3D4D5D6D7"));
    let back: hlasm::AssemblyListing =
        serde_json::from_value(serde_json::to_value(&listing).unwrap()).unwrap();
    assert_eq!(back.text(), text);
}

#[test]
fn process_pcontrol_precedence_and_duplicate_warnings_match_source_and_invocation_levels() {
    for (source, invocation, specified, expected, rc) in [
        ("PCONTROL(OFF)", "", false, Some(false), 0),
        ("PCONTROL(OFF)", "ON", false, Some(true), 4),
        ("PCONTROL(OFF)", "NOPCONTROL", true, None, 4),
        ("OVERRIDE(PCONTROL(OFF))", "ON", true, Some(false), 4),
        (
            "PCONTROL(OFF)\n*PROCESS PCONTROL(ON)",
            "",
            false,
            Some(true),
            4,
        ),
        (
            "OVERRIDE(PCONTROL(OFF))\n*PROCESS PCONTROL(ON)",
            "ON",
            true,
            Some(false),
            4,
        ),
        ("PCONTROL(OFF,ON)", "", false, Some(true), 4),
        ("PCONTROL(OFF)\n*PROCESS NOPCONTROL", "", false, None, 4),
    ] {
        let report = assemble(
            &format!("*PROCESS {source}\n{BODY}"),
            hlasm::AssemblyOptions {
                pcontrol: hlasm::parse_pcontrol(invocation).unwrap(),
                pcontrol_specified: specified,
                ..Default::default()
            },
        );
        assert!(
            report.module.is_some(),
            "{source}: {:?}",
            report.diagnostics
        );
        assert_eq!(
            report.return_code(),
            rc,
            "{source}: {:?}",
            report.diagnostics
        );
        let listing = report.listing.unwrap();
        assert_eq!(listing.pcontrol.get("ON").copied(), expected, "{source}");
        assert_eq!(listing.text().is_empty(), expected == Some(false));
        if source.starts_with("OVERRIDE") {
            assert!(listing.process_statements[0].override_options);
        }
    }
}

#[test]
fn process_is_initial_primary_input_only_with_ten_record_limit_and_exact_keyword() {
    for prefix in [
        "* an ordinary comment\n",
        "\n",
        "*PROCESSX unrelated comment\n",
        "MAIN CSECT\n",
    ] {
        let body = if prefix.starts_with("MAIN") {
            " BR 14\n END\n"
        } else {
            BODY
        };
        let report = assemble(
            &format!("{prefix}*PROCESS SYSPARM(ignored)\n{body}"),
            Default::default(),
        );
        assert!(
            report.module.is_some(),
            "{prefix:?}: {:?}",
            report.diagnostics
        );
        assert!(report.listing.unwrap().process_statements.is_empty());
    }
    let report = assemble(
        &format!(
            "{}*PROCESS SYSPARM(ignored-eleventh)\n{BODY}",
            "*PROCESS OPTABLE(ZEUS)\n".repeat(10)
        ),
        Default::default(),
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    assert_eq!(report.listing.unwrap().process_statements.len(), 10);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    std::fs::write(&path, "MAIN CSECT\n COPY CODE\n END\n").unwrap();
    std::fs::write(
        dir.path().join("CODE.cpy"),
        "*PROCESS PCONTROL(OFF),SYSPARM(ignored)\n BR 14\n",
    )
    .unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[dir.path().to_path_buf()],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let listing = report.listing.unwrap();
    assert!(listing.process_statements.is_empty());
    assert!(listing.pcontrol.is_empty());
    assert_eq!(listing.source_files.len(), 2);
}

#[test]
fn process_reports_restricted_unsupported_and_malformed_options_without_ignoring_them() {
    let report = assemble(
        &format!("*PROCESS SYSPARM('a b'),NOADATA,NOLIST,GOFF,UNKNOWN\n{BODY}"),
        Default::default(),
    );
    assert!(report.module.is_none());
    assert!(!report.complete);
    assert_eq!(report.diagnostics.len(), 5, "{:?}", report.diagnostics);
    assert!(
        report
            .diagnostics
            .iter()
            .all(|d| d.line == 1 && d.operation == "*PROCESS" && d.severity == 12)
    );
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|d| d.message.contains("not permitted"))
            .count(),
        4
    );
    assert!(
        report.listing.unwrap().process_statements[0]
            .options
            .contains("'a b'")
    );
    for option in [
        "",
        "PCONTROL(ON",
        "OVERRIDE(PCONTROL(ON)),PCONTROL(OFF)",
        "PCONTROL(BOGUS)",
    ] {
        let report = assemble(&format!("*PROCESS {option}\n{BODY}"), Default::default());
        assert!(report.module.is_none(), "{option}: accepted");
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.line == 1 && d.severity == 12)
        );
    }
}

#[test]
fn process_observes_ictl_columns_and_never_consumes_a_continuation_record() {
    let report = assemble(
        " ICTL 2,60,10\n *PROCESS PCONTROL(ON)\n MAIN CSECT\n  PRINT OFF\n  BR 14\n  END\n",
        Default::default(),
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let listing = report.listing.unwrap();
    assert_eq!(listing.process_statements[0].line, 2);
    assert!(listing.pcontrol["ON"]);
    let report = assemble(
        &format!("{:<71}+\n{BODY}", "*PROCESS PCONTROL(ON)"),
        hlasm::AssemblyOptions {
            source_format: hlasm::SourceFormat::Fixed,
            ..Default::default()
        },
    );
    assert!(report.module.is_none());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.category == "PROCESS_CONTINUATION")
    );
    assert!(
        report
            .listing
            .unwrap()
            .statements
            .iter()
            .any(|s| s.label == "MAIN")
    );
}

#[test]
fn cli_explicit_nopcontrol_overrides_process_but_source_override_wins() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    let text = dir.path().join("main.lst");
    for (option, shown) in [("PCONTROL(OFF)", true), ("OVERRIDE(PCONTROL(OFF))", false)] {
        std::fs::write(&path, format!("*PROCESS {option}\n{BODY}")).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
            .args(["check", "--pcontrol", "NOPCONTROL", "--listing"])
            .arg(&text)
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(4),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            std::fs::read_to_string(&text).unwrap().contains("BR 14"),
            shown
        );
    }
}

#[test]
fn process_optable_initial_and_current_macro_values_follow_acontrol_stack() {
    let report = assemble(
        "*PROCESS OP(uni,nolist)\n MACRO\n TRACE\n MNOTE 0,'&SYSOPT_OPTABLE|&SYSOPT_CURR_OPTABLE'\n MEND\nMAIN CSECT\n TRACE\n PUSH ACONTROL\n ACONTROL OPTABLE(ZEUS)\n TRACE\n POP ACONTROL\n TRACE\n BR 14\n END\n",
        Default::default(),
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .map(|d| d.message.as_str())
            .collect::<Vec<_>>(),
        ["UNI|UNI", "UNI|ZEUS", "UNI|UNI"]
    );
    assert_eq!(report.listing.unwrap().optable.as_deref(), Some("UNI"));
}

#[test]
fn optable_and_pcontrol_have_independent_precedence_and_invalid_invocation_is_not_hidden() {
    let report = assemble(
        &format!("*PROCESS PCONTROL(OFF),OPTABLE(UNI)\n{BODY}"),
        hlasm::AssemblyOptions {
            pcontrol: hlasm::parse_pcontrol("ON").unwrap(),
            optable: Some("ZEUS".into()),
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .filter(|d| d.category == "OPTION_IGNORED")
            .count(),
        2
    );
    let listing = report.listing.unwrap();
    assert!(listing.pcontrol["ON"]);
    assert_eq!(listing.optable.as_deref(), Some("ZEUS"));
    let report = assemble(
        &format!("*PROCESS OVERRIDE(OPTABLE(UNI))\n*PROCESS PCONTROL(OFF)\n{BODY}"),
        hlasm::AssemblyOptions {
            pcontrol: hlasm::parse_pcontrol("ON").unwrap(),
            optable: Some("ZEUS".into()),
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let listing = report.listing.unwrap();
    assert!(listing.pcontrol["ON"]);
    assert_eq!(listing.optable.as_deref(), Some("UNI"));
    for options in [
        hlasm::AssemblyOptions {
            optable: Some("ESA".into()),
            ..Default::default()
        },
        hlasm::AssemblyOptions {
            pcontrol: std::collections::BTreeMap::from([("BAD".into(), true)]),
            ..Default::default()
        },
    ] {
        let report = assemble(
            &format!("*PROCESS OVERRIDE(OPTABLE(UNI),PCONTROL(ON))\n{BODY}"),
            options,
        );
        assert!(report.module.is_none());
        assert_eq!(report.return_code(), 12);
    }
    let report = assemble(
        &format!("*PROCESS OPTABLE(UNI,NOLIST)\n{BODY}"),
        hlasm::AssemblyOptions {
            optable: Some("uni,nolist".into()),
            ..Default::default()
        },
    );
    assert_eq!(report.return_code(), 0, "{:?}", report.diagnostics);
}

#[test]
fn optable_cli_selects_real_context_and_help_values_are_not_consumed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    let json = dir.path().join("listing.json");
    std::fs::write(
        &path,
        " MNOTE 0,'&SYSOPT_OPTABLE|&SYSOPT_CURR_OPTABLE'\nMAIN CSECT\n BR 14\n END\n",
    )
    .unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["check", "--optable", "uni", "--listing-json"])
        .arg(&json)
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("UNI|UNI"));
    let listing: hlasm::AssemblyListing =
        serde_json::from_slice(&std::fs::read(&json).unwrap()).unwrap();
    assert_eq!(listing.optable.as_deref(), Some("UNI"));
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["check", "--optable", "--help"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(12));
    assert!(String::from_utf8_lossy(&output.stderr).contains("OPTABLE"));
    assert!(hlasm::parse_optable("UNI,LIST").is_err());
    assert!(hlasm::parse_optable("ESA").is_err());
}

#[test]
fn initial_process_uses_decoded_ebcdic_records_and_preserves_original_byte_columns() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("process-ebcdic.asm");
    let cp = zeus_cobol::codepage::get(939).unwrap();
    let lines = [
        "*PROCESS PCONTROL(ON) Japanese remarks",
        "MAIN CSECT",
        " PRINT OFF",
        " BR 14",
        " END",
    ];
    let bytes: Vec<_> = lines
        .iter()
        .flat_map(|line| {
            let mut record = cp.encode(line);
            record.resize(80, 0x40);
            record
        })
        .collect();
    std::fs::write(&path, bytes).unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            source_encoding: hlasm::SourceEncoding::Ebcdic {
                ccsid: 939,
                record_length: 80,
            },
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let listing = report.listing.unwrap();
    assert!(listing.pcontrol["ON"]);
    assert_eq!(listing.process_statements[0].line, 1);
    assert_eq!(listing.source_files[0].records[0].columns, 80);
    assert_eq!(listing.statements[0].location.line, 2);
    assert!(listing.text().contains("BR 14"));
}

#[test]
fn compat_macrocase_has_independent_process_precedence_and_acontrol_can_change_it() {
    let body = " MACRO\n PROBE &V\n MNOTE 0,'&V'\n MEND\nMAIN CSECT\n PROBE mixedCase\n ACONTROL COMPAT(NOMACROCASE)\n PROBE mixedCase\n BR 14\n END\n";
    for (process, invocation, expected, rc) in [
        ("", None, false, 0),
        ("*PROCESS COMPAT(MACROCASE)\n", None, true, 0),
        ("*PROCESS COMPAT(MACROCASE)\n", Some(false), false, 4),
        (
            "*PROCESS OVERRIDE(COMPAT(MACROCASE))\n",
            Some(false),
            true,
            4,
        ),
        ("*PROCESS CPAT(MC,NOMC)\n", None, false, 4),
    ] {
        let report = assemble(
            &format!("{process}{body}"),
            hlasm::AssemblyOptions {
                macrocase: invocation,
                ..Default::default()
            },
        );
        assert!(
            report.module.is_some(),
            "{process}: {:?}",
            report.diagnostics
        );
        assert_eq!(report.return_code(), rc);
        let messages: Vec<_> = report
            .diagnostics
            .iter()
            .filter(|d| d.operation == "MNOTE")
            .map(|d| d.message.as_str())
            .collect();
        assert_eq!(
            messages,
            [
                if expected { "MIXEDCASE" } else { "mixedCase" },
                "mixedCase"
            ]
        );
        let listing = report.listing.unwrap();
        assert_eq!(listing.macrocase.unwrap_or(false), expected);
    }
}

#[test]
fn compat_profile_parser_cli_and_source_reject_unsupported_or_conflicting_suboptions() {
    assert!(hlasm::parse_compat("macrocase,MC").unwrap());
    assert!(!hlasm::parse_compat("NOMC").unwrap());
    for bad in ["", "CASE", "MACROCASE,NOMACROCASE", "MC,", "--help"] {
        assert!(hlasm::parse_compat(bad).is_err(), "{bad}");
    }
    for bad in ["COMPAT(CASE)", "COMPAT()", "CPAT(MC,UNKNOWN)"] {
        let report = assemble(&format!("*PROCESS {bad}\n{BODY}"), Default::default());
        assert!(report.module.is_none());
        assert!(!report.complete);
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.category == "UNSUPPORTED_OPTION")
        );
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("compat.asm");
    let listing = dir.path().join("compat.json");
    std::fs::write(&path, BODY).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["check", "--compat", "MC", "--listing-json"])
        .arg(&listing)
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let parsed: hlasm::AssemblyListing =
        serde_json::from_slice(&std::fs::read(listing).unwrap()).unwrap();
    assert_eq!(parsed.macrocase, Some(true));
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["check", "--compat", "--help"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(12));
    assert!(String::from_utf8_lossy(&output.stderr).contains("COMPAT"));
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["run", "--compat", "MC"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(12));
    assert!(String::from_utf8_lossy(&output.stderr).contains("available for compile/check"));
}

use zeus_cobol::hlasm;

fn report(controls: &str) -> hlasm::AssemblyReport {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pcontrol.asm");
    std::fs::write(&path," MACRO\n OUTER\n INNER\n MEND\n MACRO\n INNER\n PRINT OFF,NOGEN,NODATA,NOMCALL,NOMSOURCE,NOUHEAD\nSECRET DC CL16'ABCDEFGHIJKLMNOP'\n MEND\nMAIN CSECT\n USING MAIN,15\n BR 14\n PUSH PRINT\n PRINT OFF,NOGEN,NODATA,NOMCALL,NOMSOURCE,NOUHEAD\n OUTER\n POP PRINT\n PRINT ON,GEN,DATA,MCALL,MSOURCE,UHEAD\n END\n").unwrap();
    let plain = hlasm::compile_report(&path, &[]);
    assert!(plain.module.is_some(), "{:?}", plain.diagnostics);
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            pcontrol: hlasm::parse_pcontrol(controls).unwrap(),
            ..Default::default()
        },
    );
    assert_eq!(
        serde_json::to_value(plain.module).unwrap(),
        serde_json::to_value(&report.module).unwrap()
    );
    report
}

#[test]
fn pcontrol_locks_named_options_across_print_stack_and_nested_macros() {
    let report = report("ON,GEN,DATA,MCALL,MSOURCE,UHEAD");
    let listing = report.listing.unwrap();
    for row in &listing.statements {
        for option in ["ON", "GEN", "DATA", "MCALL", "MSOURCE", "UHEAD"] {
            assert_eq!(
                row.print.get(option),
                Some(&true),
                "{} {option}",
                row.operation
            );
        }
    }
    assert!(
        listing
            .macro_calls
            .iter()
            .all(|c| c.print["GEN"] && c.print["MCALL"])
    );
    let text = listing.text();
    assert!(text.contains("SECRET DC"));
    assert!(text.contains("C1C2C3C4C5C6C7C8C9D1D2D3D4D5D6D7"));
    assert!(text.contains("MACRO  INNER"));
    let back: hlasm::AssemblyListing =
        serde_json::from_value(serde_json::to_value(&listing).unwrap()).unwrap();
    assert_eq!(back.text(), text);
}

#[test]
fn pcontrol_off_suppresses_all_expanded_text_without_discarding_json_or_xref() {
    let listing = report("OFF").listing.unwrap();
    assert!(listing.text().is_empty());
    assert!(!listing.statements.is_empty());
    assert_eq!(listing.macro_calls.len(), 2);
    assert!(listing.cross_reference().contains("SECRET"));
    assert!(listing.original_source_text().contains("SECRET DC"));
    let listing = report("ON,NOGEN").listing.unwrap();
    assert!(listing.statements.iter().all(|s| !s.print["GEN"]));
    assert!(!listing.text().contains("SECRET DC"));
    let listing = report("ON,GEN,NOMSOURCE,DATA").listing.unwrap();
    assert!(!listing.text().contains("SECRET DC"));
    assert!(listing.text().contains("C1C2C3C4C5C6C7C8C9D1D2D3D4D5D6D7"));
}

#[test]
fn pcontrol_parser_and_api_reject_invalid_profiles_and_nopcontrol_honors_print() {
    assert!(hlasm::parse_pcontrol("NOPCONTROL").unwrap().is_empty());
    assert!(hlasm::parse_pcontrol("").unwrap().is_empty());
    assert!(!hlasm::parse_pcontrol("on,nodata").unwrap()["DATA"]);
    for invalid in [
        "ON,OFF",
        "NOGEN,GEN",
        "UNKNOWN",
        "NOPRINT",
        "ON,",
        "NOPCONTROL,ON",
    ] {
        assert!(hlasm::parse_pcontrol(invalid).is_err(), "{invalid}");
    }
    let listing = report("NOPCONTROL").listing.unwrap();
    assert!(listing.pcontrol.is_empty());
    assert!(listing.statements[0].print["UHEAD"]);
    assert!(!listing.text().contains("SECRET DC"));
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    std::fs::write(&path, "MAIN CSECT\n BR 14\n END\n").unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            pcontrol: std::collections::BTreeMap::from([("UNKNOWN".into(), true)]),
            ..Default::default()
        },
    );
    assert!(report.module.is_none());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.message.contains("invalid PCONTROL key"))
    );
}

#[test]
fn cli_pcontrol_values_are_not_help_requests_and_compile_check_emit_locked_listings() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("main.asm");
    let listing = dir.path().join("main.lst");
    std::fs::write(&source, "MAIN CSECT\n PRINT OFF\n BR 14\n END\n").unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["check", "--pcontrol", "ON", "--listing"])
        .arg(&listing)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(std::fs::read_to_string(&listing).unwrap().contains("BR 14"));
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["check", "--pcontrol", "--help"])
        .arg(&source)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(12));
    assert!(String::from_utf8_lossy(&output.stderr).contains("PCONTROL"));
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["run", "--pcontrol", "ON"])
        .arg(&source)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(12));
    assert!(String::from_utf8_lossy(&output.stderr).contains("available for compile/check"));
}

#[test]
fn nogen_skips_control_prefixes_but_uses_only_the_first_object_statement_in_each_call() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("first-object.asm");
    std::fs::write(&path," MACRO\n DATAONLY\n PRINT DATA,NOPRINT\n&X SETA 1\nZERO DS 0D\nFIELD DC CL16'ABCDEFGHIJKLMNOP'\n MEND\n MACRO\n CODEFIRST\n PRINT DATA,NOPRINT\n BR 14\nHIDDEN DC CL16'QRSTUVWXYZ012345'\n MEND\nMAIN CSECT\n BR 14\n DATAONLY\n CODEFIRST\n END\n").unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            pcontrol: hlasm::parse_pcontrol("ON,NOGEN,DATA").unwrap(),
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let listing = report.listing.unwrap();
    let text = listing.text();
    assert!(text.contains("C1C2C3C4C5C6C7C8"));
    assert!(!text.contains("C1C2C3C4C5C6C7C8C9"));
    assert!(!text.contains("D8D9E2E3E4E5E6E7"));
    assert!(!text.contains("FIELD DC"));
    assert!(!text.contains("HIDDEN DC"));
}

#[test]
fn nogen_macro_first_object_can_be_a_literal_pool_without_claiming_later_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pool-first.asm");
    std::fs::write(&path, " MACRO\n POOLONLY\n LTORG\nLATER DC CL16'QRSTUVWXYZ012345'\n MEND\n MACRO\n EMPTYPOOL\n LTORG\n MEND\nMAIN CSECT\n USING MAIN,15\n L 2,=CL16'ABCDEFGHIJKLMNOP'\n BR 14\n POOLONLY\n EMPTYPOOL\n END\n").unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            pcontrol: hlasm::parse_pcontrol("ON,NOGEN,DATA").unwrap(),
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let listing = report.listing.unwrap();
    assert_eq!(listing.literals.len(), 1);
    let text = listing.text();
    assert_eq!(text.matches("C1C2C3C4C5C6C7C8").count(), 1);
    assert!(!text.contains("C1C2C3C4C5C6C7C8C9"));
    assert!(!text.contains("D8D9E2E3E4E5E6E7"));
    assert!(!text.contains("LITERAL"));
    assert!(text.contains("MACRO  EMPTYPOOL"));
}

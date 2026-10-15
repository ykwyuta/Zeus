use zeus_cobol::hlasm;

fn report(source: &str) -> hlasm::AssemblyReport {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("diagnostics.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_report(&path, &[])
}

#[test]
fn warnings_and_comments_preserve_modules_and_all_mnotes_are_reported() {
    let r = report(
        "MAIN CSECT\n         MNOTE 'notice'\n         MNOTE ,'default'\n         MNOTE 4,'warning'\n         MNOTE *,'comment'\n         BR 14\n         END\n",
    );
    assert!(r.complete);
    assert!(r.module.is_some());
    assert_eq!(r.return_code(), 4);
    assert_eq!(
        r.diagnostics.iter().map(|d| d.severity).collect::<Vec<_>>(),
        [0, 1, 4, 0]
    );
    assert_eq!(r.diagnostics[1].line, 3);
}

#[test]
fn errors_collect_following_messages_but_never_emit_a_module() {
    let r = report(
        "MAIN CSECT\n         MNOTE 8,'bad'\n         MNOTE 12,'worse'\n         BR 14\n         END\n",
    );
    assert!(r.complete);
    assert!(r.module.is_none());
    assert_eq!(r.return_code(), 12);
    assert_eq!(r.diagnostics.len(), 2);
}

#[test]
fn unresolved_operations_are_collected_and_flag_the_report_incomplete() {
    let r = report(
        "MAIN CSECT\n         MISSING1 2,3\n         MISSING2 A\n         MNOTE 4,'still scanning'\n         BR 14\n         END\n",
    );
    assert!(!r.complete);
    assert!(r.module.is_none());
    assert_eq!(
        r.diagnostics
            .iter()
            .filter(|d| d.category == "UNRESOLVED_OPERATION")
            .map(|d| (d.operation.as_str(), d.line))
            .collect::<Vec<_>>(),
        [("MISSING1", 2), ("MISSING2", 3)]
    );
}

#[test]
fn fatal_syntax_keeps_prior_diagnostics_and_marks_unfinished_reports() {
    let r = report(
        "MAIN CSECT\n         MNOTE 4,'before error'\n         L 99,BAD\n         BR 14\n         END\n",
    );
    assert!(!r.complete);
    assert!(r.module.is_none());
    assert_eq!(r.diagnostics.len(), 2);
    for severity in [-1, 256] {
        assert!(
            report(&format!(
                "MAIN CSECT\n         MNOTE {severity},'bad'\n         END\n"
            ))
            .module
            .is_none()
        );
    }
}

#[test]
fn mnote_severity_symbols_track_nested_macro_returns() {
    let r = report(
        "         MACRO\n         INNER\n         MNOTE 4,'inner'\n         MEND\n         MACRO\n         OUTER\n         MNOTE 1,'outer'\n         INNER\n         AIF (&SYSM_SEV EQ 4 AND &SYSM_HSEV EQ 4).OK\n         MNOTE 8,'severity state wrong'\n.OK ANOP\n         MEND\n         OUTER\n         AIF (&SYSM_SEV EQ 4 AND &SYSM_HSEV EQ 4).DONE\n         MNOTE 8,'returned severity wrong'\n.DONE ANOP\nMAIN CSECT\n         BR 14\n         END\n",
    );
    assert!(r.module.is_some(), "{:?}", r.diagnostics);
    assert_eq!(r.return_code(), 4);
    assert_eq!(r.diagnostics.len(), 2);
}

#[test]
fn cli_outputs_warnings_json_and_rc_while_preserving_successful_module() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("notes.asm");
    let output = dir.path().join("notes.zo");
    let json = dir.path().join("notes.json");
    std::fs::write(
        &input,
        "MAIN CSECT\n         MNOTE 4,'warning'\n         BR 14\n         END\n",
    )
    .unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .arg("compile")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--diagnostics")
        .arg(&json)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(4));
    assert!(output.is_file());
    assert!(String::from_utf8_lossy(&result.stderr).contains("warning"));
    let r: hlasm::AssemblyReport = serde_json::from_slice(&std::fs::read(json).unwrap()).unwrap();
    assert!(r.complete);
    assert!(r.module.is_some());
}

#[test]
fn missing_copy_members_and_unsupported_operand_forms_have_distinct_categories() {
    for (body, operation, category) in [
        ("COPY ABSENT_MEMBER", "COPY", "MISSING_MEMBER"),
        (
            "STORAGE OBTAIN,LENGTH=8,KEY=3",
            "STORAGE",
            "UNSUPPORTED_FORM",
        ),
    ] {
        let report = report(&format!(
            "MAIN CSECT\n         {body}\n         BR 14\n         END\n"
        ));
        assert!(report.module.is_none());
        assert!(!report.complete);
        assert_eq!(report.diagnostics[0].operation, operation);
        assert_eq!(report.diagnostics[0].category, category);
        assert_eq!(report.diagnostics[0].line, 2);
    }
}

#[test]
fn binary_constant_conversion_warnings_preserve_modules_and_source_locations() {
    let r = report(
        "MAIN CSECT\n         BR 14\nA DC EB'.1'\nB DC DB'1E1000'\nC DC LB'1E-10000R6'\nD DC EB'1.5',DB'0',LB'(INF)'\n         END\n",
    );
    assert!(r.complete && r.module.is_some(), "{:?}", r.diagnostics);
    assert_eq!(r.return_code(), 4);
    assert_eq!(r.diagnostics.len(), 3);
    for (d, line, reason) in r.diagnostics.iter().zip([3, 4, 5]).map(|(d, line)| {
        (
            d,
            line,
            match line {
                3 => "exactly",
                4 => "largest",
                _ => "smallest",
            },
        )
    }) {
        assert_eq!(
            (
                d.line,
                d.severity,
                d.category.as_str(),
                d.operation.as_str()
            ),
            (line, 4, "FLOAT_CONVERSION", "DC")
        );
        assert!(d.message.contains(reason), "{d}");
    }
}

#[test]
fn binary_scale_modifiers_are_ignored_with_warning_instead_of_rejecting_constants() {
    let r = report(
        "MAIN CSECT\n         BR 14\nA DC EBS1'1.5'\nB DC DBS(-100)'2'\nC DC LBS(EXP)'3'\nEXP EQU 100\n         ENTRY A,B,C\n         END\n",
    );
    assert!(r.complete && r.module.is_some(), "{:?}", r.diagnostics);
    assert_eq!(r.diagnostics.len(), 3);
    assert!(
        r.diagnostics
            .iter()
            .all(|d| d.message.contains("scale modifier is ignored"))
    );
    let module = r.module.unwrap();
    for (name, raw) in [
        ("A", "3FC00000"),
        ("B", "4000000000000000"),
        ("C", "40008000000000000000000000000000"),
    ] {
        let at = module.entries.get(name).copied().unwrap_or_else(|| {
            // Only explicitly exported labels become Module entries.
            panic!("missing exported {name}")
        }) as usize;
        let bytes = (0..raw.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(&module.storage[at..at + bytes.len()], bytes);
    }
}

#[test]
fn conversion_warnings_are_not_duplicated_by_layout_retries_or_duplication_factors() {
    let r = report(
        "MAIN CSECT\n         USING MAIN,15\n         LE 0,=EB'.1'\nALTERNATE LOCTR\nA DC 3EB'.2,.3'\nMAIN LOCTR\n         BR 14\n         END\n",
    );
    assert!(r.complete && r.module.is_some(), "{:?}", r.diagnostics);
    assert_eq!(r.diagnostics.len(), 3);
    assert_eq!(
        r.diagnostics.iter().map(|d| d.line).collect::<Vec<_>>(),
        [3, 5, 5]
    );
    assert!(r.diagnostics[1].message.contains("nominal 1"));
    assert!(r.diagnostics[2].message.contains("nominal 2"));
}

#[test]
fn reserved_fields_zero_duplication_and_special_values_do_not_warn_about_rounding() {
    let r = report(
        "MAIN CSECT\n         BR 14\nA DS EB'.1'\nB DC 0DB'.1'\nC DC EB'(INF),(SNAN),(NAN),-0'\nD DC EBL.9'1'\n         END\n",
    );
    assert!(r.complete && r.module.is_some(), "{:?}", r.diagnostics);
    assert_eq!(r.return_code(), 0);
    assert!(r.diagnostics.is_empty());
    let r = report("MAIN CSECT\n         BR 14\nA DC EBL.9'1.5'\n         END\n");
    assert_eq!(r.return_code(), 4);
    assert_eq!(r.diagnostics.len(), 1);
}

#[test]
fn cli_emits_binary_conversion_warnings_in_diagnostics_and_listing() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("conversion.asm");
    let output = dir.path().join("conversion.zo");
    let json = dir.path().join("conversion.json");
    let listing = dir.path().join("conversion.lst");
    std::fs::write(
        &input,
        "MAIN CSECT\n         BR 14\nA DC EB'.1'\n         END\n",
    )
    .unwrap();
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .arg("compile")
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .arg("--diagnostics")
        .arg(&json)
        .arg("--listing")
        .arg(&listing)
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(4));
    assert!(output.is_file());
    let r: hlasm::AssemblyReport = serde_json::from_slice(&std::fs::read(json).unwrap()).unwrap();
    assert_eq!(r.diagnostics.len(), 1);
    assert_eq!(r.diagnostics[0].line, 3);
    assert!(
        std::fs::read_to_string(listing)
            .unwrap()
            .contains("FLOAT_CONVERSION severity 4")
    );
}

#[test]
fn conversion_warnings_before_an_assembly_failure_are_preserved() {
    let r = report("MAIN CSECT\nA DC EB'.1'\n         L 99,A\n         END\n");
    assert!(!r.complete && r.module.is_none());
    assert_eq!(r.diagnostics.len(), 2);
    assert_eq!(r.diagnostics[0].category, "FLOAT_CONVERSION");
    assert_eq!(r.diagnostics[0].line, 2);
    assert_eq!(r.diagnostics[1].severity, 12);
}

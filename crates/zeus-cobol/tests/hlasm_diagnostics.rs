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

use zeus_cobol::hlasm::{self, AssemblyOptions, SourceEncoding};

fn compile(text: &str) -> hlasm::AssemblyReport {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("input.asm");
    std::fs::write(&path, text).unwrap();
    hlasm::compile_report(&path, &[])
}

fn run(report: hlasm::AssemblyReport) -> i32 {
    let module = report
        .module
        .as_ref()
        .unwrap_or_else(|| panic!("{:?}", report.diagnostics));
    let mut mem = module.storage_at(0).unwrap();
    hlasm::execute(module, &mut mem, 0, &[]).unwrap()
}

#[test]
fn input_columns_and_disabled_continuation_allow_identification_before_source() {
    let text = " ICTL 9,80\n00000001TEST CSECT\n00000002 LA 15,7\n00000003 BR 14\n00000004 END\n";
    assert_eq!(run(compile(text)), 7);
    assert_eq!(
        run(compile("\u{feff}TEST CSECT\n LA 15,3\n BR 14\n END\n")),
        3
    );
}

#[test]
fn custom_continuation_columns_preserve_quoted_spaces_and_data() {
    let first = format!("{:<50}X", "         DC C'HELLO");
    let text = format!(
        " ICTL 3,50,12\n  TEST CSECT\n   LA 15,9\n   BR 14\n{first}\n           WORLD'\n   END\n"
    );
    let report = compile(&text);
    let m = report
        .module
        .as_ref()
        .unwrap_or_else(|| panic!("{:?}", report.diagnostics));
    assert!(
        m.storage
            .windows(9)
            .any(|s| s == zeus_cobol::ebcdic::encode("HELLO    "))
    );
    assert_eq!(run(report), 9);
}

#[test]
fn malformed_input_controls_and_continuations_are_errors() {
    for source in [
        " ICTL 0,71,16\n",
        " ICTL 40,41,16\n",
        " ICTL 1,71,1\n",
        " ICTL 1,81\n",
        " ICTL 1,71,16,20\n",
        " ICTL A,71\n",
        "TEST CSECT\n ICTL 9,80\n",
        " ICTL 1,71\n ICTL 1,71\n",
        " ISEQ 0,80\n",
        " ISEQ 74,72\n",
        " ISEQ 72\n",
        " ISEQ 1,80,80\n",
    ] {
        let report = compile(source);
        assert!(report.module.is_none(), "{source}");
        assert!(!report.complete, "{source}");
    }
    let source = format!("TEST CSECT\n{:<71}X\n BR 14\n", " LA 15,0");
    assert!(
        compile(&source).diagnostics[0]
            .message
            .contains("column 16")
    );
}

fn numbered(text: &str, sequence: &str) -> String {
    format!("{text:<72}{sequence:<8}\n")
}

#[test]
fn sequence_warnings_include_comments_continuations_and_the_ending_iseq() {
    let text = " ISEQ 73,80\n".to_owned()
        + &numbered("TEST CSECT", "00000002")
        + &numbered("* comment", "00000001")
        + &numbered(" LA 15,0", "")
        + &numbered(" BR 14", "00000002")
        + &numbered(" ISEQ", "00000002")
        + &numbered(" END", "00000000");
    let report = compile(&text);
    assert!(report.complete);
    assert_eq!(report.return_code(), 4);
    assert_eq!(
        report
            .diagnostics
            .iter()
            .map(|d| (d.line, d.category.as_str()))
            .collect::<Vec<_>>(),
        [(3, "SEQUENCE"), (6, "SEQUENCE")]
    );
    assert_eq!(run(report), 0);
    let first = format!("{:<71}X00000001\n", " DC C'A");
    let cont = format!("{:<15}{:<56} 00000001\n", "", "B'");
    let report = compile(&(" ISEQ 73,80\nTEST CSECT\n".to_owned() + &first + &cont + " END\n"));
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.line == 4 && d.category == "SEQUENCE")
    );
}

#[test]
fn copy_members_use_standard_columns_and_cannot_set_input_controls() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    std::fs::write(
        &path,
        " ICTL 9,80\n00000001TEST CSECT\n00000002 COPY BODY\n00000003 END\n",
    )
    .unwrap();
    let member = dir.path().join("BODY");
    std::fs::write(&member, " LA 15,6\n BR 14\n").unwrap();
    assert_eq!(run(hlasm::compile_report(&path, &[])), 6);
    std::fs::write(&member, " ISEQ 73,80\n").unwrap();
    let report = hlasm::compile_report(&path, &[]);
    assert!(report.module.is_none());
    assert!(
        report
            .diagnostics
            .last()
            .unwrap()
            .message
            .contains("library macro members")
    );
}

fn ebcdic_records(ccsid: u32, lines: &[&str]) -> Vec<u8> {
    let cp = zeus_cobol::codepage::get(ccsid).unwrap();
    lines
        .iter()
        .flat_map(|s| {
            let mut b = cp.encode(s);
            assert!(b.len() <= 80);
            b.resize(80, 0x40);
            b
        })
        .collect()
}

#[test]
fn fixed_ebcdic_sources_and_library_members_use_the_selected_codepage() {
    for ccsid in zeus_cobol::codepage::CCSIDS {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("source.asm");
        std::fs::write(
            &path,
            ebcdic_records(ccsid, &["TEST CSECT", " COPY BODY", " END"]),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("BODY"),
            ebcdic_records(ccsid, &[" LA 15,5", " BR 14"]),
        )
        .unwrap();
        let options = AssemblyOptions {
            source_encoding: SourceEncoding::Ebcdic {
                ccsid,
                record_length: 80,
            },
            ..Default::default()
        };
        assert_eq!(
            run(hlasm::compile_report_with_options(&path, &[], &options)),
            5
        );
    }
}

#[test]
fn mixed_dbcs_input_counts_byte_columns_and_reports_broken_records() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("source.asm");
    // The DBCS comment takes more bytes than Unicode characters. The byte in
    // column 72 is blank, and identification after it must never become source.
    let mut bytes = ebcdic_records(
        939,
        &["TEST CSECT", "* 日本語", " LA 15,4", " BR 14", " END"],
    );
    let options = AssemblyOptions {
        source_encoding: SourceEncoding::Ebcdic {
            ccsid: 939,
            record_length: 80,
        },
        ..Default::default()
    };
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(
        run(hlasm::compile_report_with_options(&path, &[], &options)),
        4
    );
    bytes[90] = 0x0e;
    std::fs::write(&path, &bytes).unwrap();
    let report = hlasm::compile_report_with_options(&path, &[], &options);
    assert!(report.module.is_none());
    assert_eq!(report.diagnostics[0].line, 2);
    std::fs::write(&path, &bytes[..bytes.len() - 1]).unwrap();
    let report = hlasm::compile_report_with_options(&path, &[], &options);
    assert!(
        report.diagnostics[0]
            .message
            .contains("complete fixed records")
    );
}

#[test]
fn encoding_options_reject_unknown_pages_and_bad_record_lengths() {
    assert!("utf8".parse::<SourceEncoding>().is_ok());
    assert!("ebcdic:939:80".parse::<SourceEncoding>().is_ok());
    for text in [
        "ebcdic",
        "ebcdic:999:80",
        "ebcdic:37:0",
        "ebcdic:37:79",
        "ebcdic:37:32761",
        "utf16",
    ] {
        assert!(text.parse::<SourceEncoding>().is_err(), "{text}");
    }
}

#[test]
fn continuation_preserves_identification_before_a_custom_begin_column() {
    let first = format!("00000003{:<42}X", " DC C'ABC");
    let next = format!("00000004       {}", "DEF'");
    let source = format!(
        " ICTL 9,50,16\n00000001 ISEQ 1,8\n00000002TEST CSECT\n{first}\n{next}\n00000005CODE LA 15,0\n00000006 BR 14\n00000007 END CODE\n"
    );
    let report = compile(&source);
    assert!(report.complete, "{:?}", report.diagnostics);
    assert_eq!(report.return_code(), 0, "{:?}", report.diagnostics);
    assert!(
        report
            .module
            .as_ref()
            .unwrap()
            .storage
            .windows(3)
            .any(|s| s == zeus_cobol::ebcdic::encode("DEF"))
    );
}

#[test]
fn mixed_source_characters_with_two_unicode_codepoints_keep_their_two_byte_columns() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("two.asm");
    let source = ebcdic_records(
        1390,
        &[
            "TEST CSECT",
            " LA 15,0",
            " BR 14",
            "DATA DC CU'æ\u{300}'",
            " END",
        ],
    );
    std::fs::write(&path, source).unwrap();
    let options = AssemblyOptions {
        source_encoding: SourceEncoding::Ebcdic {
            ccsid: 1390,
            record_length: 80,
        },
        ..Default::default()
    };
    let report = hlasm::compile_report_with_options(&path, &[], &options);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    assert_eq!(&report.module.unwrap().storage[6..10], &[0, 0xe6, 3, 0]);
}

#[test]
fn source_format_auto_preserves_extended_remarks_and_fixed_checks_column_72() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("remarks.asm");
    let statement = format!(
        "{:<32}{}",
        " LA 15,3", "a long remark whose words continue beyond column seventy two"
    );
    let text = format!("TEST CSECT\n{statement}\n BR 14\n END\n");
    std::fs::write(&path, &text).unwrap();
    assert_eq!(run(hlasm::compile_report(&path, &[])), 3);
    let options = AssemblyOptions {
        source_format: hlasm::SourceFormat::Fixed,
        ..Default::default()
    };
    let report = hlasm::compile_report_with_options(&path, &[], &options);
    assert!(report.module.is_none());
    assert!(report.diagnostics[0].message.contains("column 16"));
    std::fs::write(&path, format!(" ICTL 1,71,16\n{text}")).unwrap();
    assert!(hlasm::compile_report(&path, &[]).module.is_none());
    std::fs::write(&path, &text).unwrap();
    for (format, success) in [
        ("auto", true),
        ("fixed", false),
        ("free", false),
        ("--help", false),
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
            .args(["check", "--source-format", format])
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(
            output.status.success(),
            success,
            "{format}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn fixed_records_longer_than_80_columns_still_use_the_continuation_column() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wide-records.asm");
    let records = [
        "TEST CSECT".to_owned(),
        " LA 15,0".into(),
        " BR 14".into(),
        format!("{:<71}X00000001", "DATA DC C'A"),
        format!("{:<15}B'", ""),
        " END".into(),
    ];
    let mut input = Vec::new();
    for record in records {
        let mut bytes = zeus_cobol::ebcdic::encode(&record);
        bytes.resize(100, 0x40);
        input.extend(bytes);
    }
    std::fs::write(&path, input).unwrap();
    for format in [hlasm::SourceFormat::Auto, hlasm::SourceFormat::Fixed] {
        let options = AssemblyOptions {
            source_encoding: SourceEncoding::Ebcdic {
                ccsid: 37,
                record_length: 100,
            },
            source_format: format,
            ..Default::default()
        };
        let report = hlasm::compile_report_with_options(&path, &[], &options);
        let module = report
            .module
            .unwrap_or_else(|| panic!("{:?}", report.diagnostics));
        let expected = zeus_cobol::ebcdic::encode(&format!("A{:60}B", ""));
        assert!(
            module
                .storage
                .windows(expected.len())
                .any(|w| w == expected)
        );
    }
}

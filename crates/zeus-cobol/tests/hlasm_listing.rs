use zeus_cobol::hlasm;

#[test]
fn listing_tracks_final_layout_data_dummy_symbols_and_original_literals() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    std::fs::write(&path,"MAIN CSECT\n USING MAIN,15\n L 2,VALUE\n L 3,=F'9'\n B DONE\nVALUE DC F'7'\nTEXT DC C'VALUE'\nALIAS EQU VALUE+4\nLEN EQU L'VALUE\nDONE BR 14\nMAP DSECT\nFIELD DS D\n END MAIN\n").unwrap();
    let options = hlasm::AssemblyOptions {
        collect_listing: true,
        ..Default::default()
    };
    let report = hlasm::compile_report_with_options(&path, &[], &options);
    assert!(report.complete, "{:?}", report.diagnostics);
    let details = report.listing.as_ref().unwrap();
    let module = report.module.as_ref().unwrap();
    let value = &details.symbols["VALUE"];
    assert_eq!(value.length, 4);
    assert_eq!(value.definitions.len(), 1);
    assert_eq!(
        value.references.iter().map(|r| r.line).collect::<Vec<_>>(),
        [3, 8, 9]
    );
    let row = details
        .statements
        .iter()
        .find(|s| s.label == "VALUE")
        .unwrap();
    assert_eq!(row.data, 7u32.to_be_bytes());
    assert_eq!(row.offset as i64, value.value);
    assert_eq!(
        &module.storage[row.offset as usize..row.offset as usize + 4],
        row.data
    );
    assert_eq!(details.symbols["FIELD"].section.as_deref(), Some("MAP"));
    assert_eq!(details.symbols["FIELD"].length, 8);
    assert!(details.statements.iter().any(|s| s.operands == "3,=F'9'"));
    assert!(
        details
            .statements
            .iter()
            .filter(|s| s.operation == "L")
            .all(|s| s.data.is_empty())
    );
    let bytes = serde_json::to_vec(details).unwrap();
    let back: hlasm::AssemblyListing = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(back.symbols["VALUE"].references, value.references);
    assert!(back.cross_reference().contains("main.asm:3#"));
}

#[test]
fn listing_preserves_library_member_locations_and_does_not_change_modules() {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main.asm");
    let member = dir.path().join("DATA.asm");
    std::fs::write(&member, "VALUE DC F'27'\n").unwrap();
    std::fs::write(
        &main,
        "MAIN CSECT\n USING MAIN,15\n L 15,VALUE\n BR 14\n COPY DATA\n END\n",
    )
    .unwrap();
    let plain = hlasm::compile_report(&main, &[dir.path().to_path_buf()]);
    let report = hlasm::compile_report_with_options(
        &main,
        &[dir.path().to_path_buf()],
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
    assert!(
        listing.symbols["VALUE"].definitions[0]
            .file
            .ends_with("DATA.asm")
    );
    assert!(
        listing.symbols["VALUE"].references[0]
            .file
            .ends_with("main.asm")
    );
}

#[test]
fn cli_writes_embedded_help_source_listing_xref_and_json() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("main.asm");
    std::fs::write(&source, "MAIN CSECT\n BR 14\nVALUE DC F'7'\n END\n").unwrap();
    let listing = dir.path().join("main.lst");
    let xref = dir.path().join("main.xref");
    let json = dir.path().join("main.json");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .arg("check")
        .arg("--listing")
        .arg(&listing)
        .arg("--xref")
        .arg(&xref)
        .arg("--listing-json")
        .arg(&json)
        .arg(&source)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        std::fs::read_to_string(listing)
            .unwrap()
            .contains("00000007")
    );
    assert!(std::fs::read_to_string(xref).unwrap().contains("VALUE"));
    let details: hlasm::AssemblyListing =
        serde_json::from_slice(&std::fs::read(json).unwrap()).unwrap();
    assert_eq!(details.symbols["VALUE"].value, 4);
    let help = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(help.status.success());
    assert!(
        String::from_utf8(help.stdout)
            .unwrap()
            .contains("--listing")
    );
}

#[test]
fn constant_type_names_and_quoted_text_do_not_become_symbol_references() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("refs.asm");
    std::fs::write(&path,"MAIN CSECT\n USING MAIN,15\n L 2,=A(B)\n BR 14\nA DC F'0'\nB DC F'0'\nADDRESS DC A(B)\nTEXT DC C'A B'\n EXTRN EXTERNAL\nOTHER DC V(EXTERNAL)\n END\n").unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert!(report.complete, "{:?}", report.diagnostics);
    let listing = report.listing.unwrap();
    assert!(listing.symbols["A"].references.is_empty());
    assert_eq!(
        listing.symbols["B"]
            .references
            .iter()
            .map(|r| r.line)
            .collect::<Vec<_>>(),
        [3, 7]
    );
    assert_eq!(listing.symbols["EXTERNAL"].definitions.len(), 1);
    assert_eq!(
        listing.symbols["EXTERNAL"]
            .references
            .iter()
            .map(|r| r.line)
            .collect::<Vec<_>>(),
        [10]
    );
}

#[test]
fn listing_honors_print_status_stack_data_limits_and_page_controls() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("print.asm");
    std::fs::write(&path," TITLE 'A synthetic listing'\nMAIN CSECT\n BR 14\n PRINT OFF\nHIDDEN DC C'never shown'\n PRINT ON,DATA,NOPRINT\n PUSH PRINT,NOPRINT\n PRINT NODATA,NOPRINT\nSHORT DC CL16'ABCDEFGH12345678'\n POP PRINT,NOPRINT\nFULL DC CL16'IJKLMNOP87654321'\n SPACE 2\n EJECT\n EJECT\n TITLE 'Second heading'\nTAIL DC F'7'\n END\n").unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert!(report.complete, "{:?}", report.diagnostics);
    let listing = report.listing.unwrap();
    let text = listing.text();
    assert!(text.contains("A synthetic listing"));
    assert!(text.contains("Second heading"));
    assert_eq!(text.matches("A synthetic listing").count(), 1);
    assert_eq!(text.matches("Second heading").count(), 1);
    assert!(!text.contains("never shown"));
    assert!(!text.contains("NOPRINT"));
    assert!(!text.contains("F1F2F3F4F5F6F7F8"));
    assert!(text.contains("F8F7F6F5F4F3F2F1"));
    assert!(!text.contains("\u{c}\u{c}"));
    assert!(listing.statements.iter().any(|s| s.label == "HIDDEN")); // JSON retains suppressed rows.
}

#[test]
fn listing_distinguishes_macro_calls_generated_rows_and_source_suppression() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("macro-print.asm");
    std::fs::write(&path," MACRO\n&L MAKE &V\n&L DC CL12'&V'\n MEND\nMAIN CSECT\n BR 14\n PRINT NOGEN,NOPRINT\nHIDDEN MAKE ABCDEFGHIJKL\n PRINT GEN,NOMSOURCE,DATA,NOPRINT\nQUIET MAKE MNOPQRSTUVWX\n PRINT MSOURCE,NOPRINT\nSHOWN MAKE YZ0123456789\n END\n").unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert!(report.complete, "{:?}", report.diagnostics);
    let listing = report.listing.unwrap();
    let text = listing.text();
    assert!(text.contains("MACRO HIDDEN MAKE ABCDEFGHIJKL"));
    assert!(!text.contains("HIDDEN DC"));
    assert!(text.contains("MACRO QUIET MAKE MNOPQRSTUVWX"));
    assert!(!text.contains("QUIET DC"));
    assert!(text.contains("SHOWN DC CL12'YZ0123456789'"));
    let row = listing
        .statements
        .iter()
        .find(|r| r.label == "HIDDEN")
        .unwrap();
    assert_eq!(row.macro_depth, 1);
    assert_eq!(row.macro_calls[0].depth, 0);
    assert!(
        row.macro_calls[0]
            .definition_file
            .ends_with("macro-print.asm")
    );
}

#[test]
fn invalid_spacing_and_eject_operands_are_diagnosed_even_without_listing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.asm");
    for instruction in ["SPACE -1", "SPACE MAIN", "EJECT 1", "PRINT NOPRINT"] {
        std::fs::write(&path, format!("MAIN CSECT\n BR 14\n {instruction}\n END\n")).unwrap();
        assert!(hlasm::compile_file(&path, &[]).is_err(), "{instruction}");
    }
}

#[test]
fn uhead_uses_resolved_usings_and_conditional_eject_observes_remaining_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pages.asm");
    std::fs::write(&path,"MAIN CSECT\n USING MAIN,15\n PRINT UHEAD,NOPRINT\n TITLE 'Active registers'\n BR 14\n CEJECT 1\nA DC F'1'\n CEJECT 60\nB DC F'2'\n DROP 15\n TITLE 'No registers'\nC DC F'3'\n END\n").unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert!(report.complete, "{:?}", report.diagnostics);
    let listing = report.listing.unwrap();
    let text = listing.text();
    assert!(text.contains("R15=IMAGE+0"), "{text}");
    assert!(text.contains("Active USINGs: none"), "{text}");
    let a = listing.statements.iter().find(|r| r.label == "A").unwrap();
    assert_eq!(a.usings.len(), 1);
    assert_eq!(a.usings[0].register, 15);
    let c = listing.statements.iter().find(|r| r.label == "C").unwrap();
    assert!(c.usings.is_empty());
    assert_eq!(text.matches('\u{c}').count(), 3);
}

#[test]
fn empty_nested_suppressed_and_trailing_macro_calls_remain_in_invocation_order() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.asm");
    std::fs::write(&path," MACRO\n&L EMPTY\n MEXIT\n MEND\n MACRO\n OUTER\n EMPTY\n MEND\nMAIN CSECT\n BR 14\n PRINT MCALL,GEN,NOPRINT\nFIRST EMPTY\n OUTER\n PRINT OFF,NOPRINT\nHIDDEN EMPTY\n PRINT ON,NOPRINT\nLAST EMPTY\n").unwrap();
    let options = hlasm::AssemblyOptions {
        collect_listing: true,
        ..Default::default()
    };
    let report = hlasm::compile_report_with_options(&path, &[], &options);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let plain = hlasm::compile_report(&path, &[]);
    assert_eq!(
        hlasm::module_bytes(report.module.as_ref().unwrap()).unwrap(),
        hlasm::module_bytes(plain.module.as_ref().unwrap()).unwrap()
    );
    let listing = report.listing.unwrap();
    assert_eq!(
        listing
            .macro_calls
            .iter()
            .map(|c| (c.label.as_str(), c.operation.as_str(), c.depth, c.generated))
            .collect::<Vec<_>>(),
        [
            ("FIRST", "EMPTY", 0, 0),
            ("", "OUTER", 0, 0),
            ("", "EMPTY", 1, 0),
            ("HIDDEN", "EMPTY", 0, 0),
            ("LAST", "EMPTY", 0, 0)
        ]
    );
    assert!(!listing.symbols.contains_key("FIRST"));
    let text = listing.text();
    assert_eq!(text.matches("      MACRO ").count(), 4);
    assert!(text.contains("MACRO FIRST EMPTY"));
    assert!(text.contains("MACRO LAST EMPTY"));
    assert!(!text.contains("HIDDEN EMPTY"));
    let restored: hlasm::AssemblyListing =
        serde_json::from_slice(&serde_json::to_vec(&listing).unwrap()).unwrap();
    assert_eq!(restored.text(), text);
}

#[test]
fn macro_call_visibility_uses_the_callers_print_state_before_the_body_changes_it() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("quiet.asm");
    std::fs::write(&path," MACRO\n QUIET\n PRINT OFF,NOPRINT\nHIDDEN DC C'body hidden'\n PRINT ON,NOPRINT\n MEND\nMAIN CSECT\n BR 14\n QUIET\n END\n").unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let listing = report.listing.unwrap();
    assert_eq!(listing.macro_calls.len(), 1);
    assert_eq!(listing.macro_calls[0].generated, 3);
    let text = listing.text();
    assert!(text.contains("MACRO  QUIET"));
    assert!(!text.contains("body hidden"));
}

#[test]
fn empty_nogen_macro_does_not_claim_the_next_unrelated_data_rows_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nogen-empty.asm");
    std::fs::write(&path," MACRO\n EMPTY\n MEND\nMAIN CSECT\n BR 14\n PRINT NOGEN,NOPRINT\n EMPTY\nDATA DC X'AABBCCDD'\n END\n").unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let text = report.listing.unwrap().text();
    assert_eq!(text.matches("AABBCCDD").count(), 2); // source nominal plus the single data display
}

#[test]
fn expanded_rows_keep_original_macro_definition_files_and_lines() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    let member = dir.path().join("MAKE.asm");
    std::fs::write(&member, " MACRO\n&L MAKE\n&L DC F'7'\n MEND\n").unwrap();
    std::fs::write(&path, "MAIN CSECT\n BR 14\nVALUE MAKE\n END\n").unwrap();
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
    let row = listing
        .statements
        .iter()
        .find(|s| s.label == "VALUE")
        .unwrap();
    assert_eq!(row.location.file, path.display().to_string());
    assert_eq!(row.location.line, 3);
    let definition = row.definition.as_ref().unwrap();
    assert_eq!(definition.file, member.display().to_string());
    assert_eq!(definition.line, 3);
    assert_eq!(listing.macro_calls[0].definition_line, 2);
    assert_eq!(
        listing.macro_calls[0].definition_file,
        member.display().to_string()
    );
    assert!(
        listing
            .statements
            .iter()
            .find(|s| s.label == "MAIN")
            .unwrap()
            .definition
            .is_none()
    );
    assert!(
        listing
            .text()
            .contains(&format!("definition {}:3", member.display()))
    );
}

#[test]
fn original_records_preserve_comments_definitions_continuations_and_read_members() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    let source = "* opening comment\n\nMAIN CSECT\n BR 14\n PRINT OFF\n COPY DATA\n CALLER\n END\n* after END\n";
    let continuation = format!(
        "{:<71}+00000001\n{:<71} 00000002\n",
        "VALUE DC C'A',", "               C'B'"
    );
    std::fs::write(&path, source).unwrap();
    std::fs::write(
        dir.path().join("DATA.asm"),
        format!("* copy comment\n{continuation}"),
    )
    .unwrap();
    let macro_text = "* macro comment\n MACRO\n CALLER\n.* definition comment\n ANOP\n MEND\n";
    std::fs::write(dir.path().join("CALLER.mac"), macro_text).unwrap();
    let dirs = [dir.path().to_path_buf()];
    let plain = hlasm::compile_report(&path, &dirs);
    let report = hlasm::compile_report_with_options(
        &path,
        &dirs,
        &hlasm::AssemblyOptions {
            collect_listing: true,
            source_format: hlasm::SourceFormat::Fixed,
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    assert_eq!(
        serde_json::to_value(plain.module).unwrap(),
        serde_json::to_value(&report.module).unwrap()
    );
    let listing = report.listing.unwrap();
    assert_eq!(listing.source_files.len(), 3);
    let main = &listing.source_files[0];
    assert!(!main.library);
    assert_eq!(main.format, "fixed");
    assert_eq!(
        main.records
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>(),
        source.lines().collect::<Vec<_>>()
    );
    assert_eq!(main.records[1].columns, 0);
    assert_eq!(main.records.last().unwrap().line, 9);
    let copy = &listing.source_files[1];
    assert!(copy.library && copy.file.ends_with("DATA.asm"));
    assert!(copy.records[1].text.ends_with("+00000001"));
    assert_eq!(copy.records[2].columns, 80);
    let library = &listing.source_files[2];
    assert!(library.file.ends_with("CALLER.mac"));
    assert_eq!(library.records[3].text, ".* definition comment");
    assert!(listing.original_source_text().contains("* copy comment"));
    assert!(!listing.text().contains("* copy comment"));
    let json = serde_json::to_vec(&listing).unwrap();
    let back: hlasm::AssemblyListing = serde_json::from_slice(&json).unwrap();
    assert_eq!(back.original_source_text(), listing.original_source_text());
}

#[test]
fn original_records_keep_ebcdic_byte_columns_and_dbcs_positions() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mixed.asm");
    let cp = zeus_cobol::codepage::get(939).unwrap();
    let lines = ["* 日本語", "MAIN CSECT", " BR 14", " END"];
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
    let file = &listing.source_files[0];
    assert_eq!(file.encoding, "ebcdic:939:80");
    let comment = &file.records[0];
    assert_eq!(comment.columns, 80);
    assert!(comment.text.starts_with("* 日本語"));
    assert_eq!(&comment.positions[..6], &[1, 2, 4, 6, 8, 11]);
    assert_eq!(comment.positions.len(), comment.text.chars().count());
    assert!(file.records[1].positions.is_empty());
}

#[test]
fn cli_original_source_archive_is_available_after_parse_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.asm");
    let archive = dir.path().join("original.txt");
    let json = dir.path().join("listing.json");
    let original = format!("* original failure\n{:<71}+\n", " BROKEN");
    std::fs::write(&path, &original).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["check", "--source-format", "fixed", "--source-listing"])
        .arg(&archive)
        .arg("--listing-json")
        .arg(&json)
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(12));
    let text = std::fs::read_to_string(&archive).unwrap();
    assert!(text.contains("* original failure"));
    assert!(text.contains("missing continuation record"));
    let listing: hlasm::AssemblyListing =
        serde_json::from_slice(&std::fs::read(json).unwrap()).unwrap();
    assert_eq!(
        listing.source_files[0].records[1].text,
        original.lines().nth(1).unwrap()
    );
    let wrong_command = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .arg("run")
        .arg("--source-listing")
        .arg(&archive)
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(wrong_command.status.code(), Some(12));
    assert!(String::from_utf8_lossy(&wrong_command.stderr).contains("available for compile/check"));
    // A help-looking value is a path, not a request to display help.
    let literal_help = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .current_dir(dir.path())
        .args(["check", "--source-listing", "--help"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(literal_help.status.code(), Some(12));
    assert!(dir.path().join("--help").is_file());
}

#[test]
fn literal_pool_listing_keeps_final_layout_bytes_references_and_pool_identity() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("literal-pools.asm");
    let source = "MAIN CSECT\n USING MAIN,15\n L 2,=F'7'\n L 3,=F'7'\n L 4,=A(*)\n B NEXT\n LTORG\nNEXT L 5,=F'7'\n L 6,=3F'9'\n BR 14\n END\n";
    std::fs::write(&path, source).unwrap();
    let plain = hlasm::compile_report(&path, &[]);
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    assert_eq!(
        serde_json::to_value(plain.module).unwrap(),
        serde_json::to_value(&report.module).unwrap()
    );
    let module = report.module.unwrap();
    let listing = report.listing.unwrap();
    assert_eq!(listing.literals.len(), 4);
    for literal in &listing.literals {
        assert_eq!(
            literal.data,
            module.storage
                [literal.offset as usize..literal.offset as usize + literal.length as usize]
        );
        assert_eq!(
            i64::from(literal.offset),
            listing.symbols[&literal.name].value
        );
    }
    let sevens: Vec<_> = listing
        .literals
        .iter()
        .filter(|l| l.operand == "=F'7'")
        .collect();
    assert_eq!(sevens.len(), 2);
    assert_eq!(sevens[0].pool.line, 7);
    assert_eq!(sevens[1].pool.line, 11);
    assert_ne!(sevens[0].offset, sevens[1].offset);
    assert_eq!(
        sevens[0]
            .references
            .iter()
            .map(|r| r.line)
            .collect::<Vec<_>>(),
        [3, 4]
    );
    assert_eq!(sevens[1].references[0].line, 8);
    let address = listing
        .literals
        .iter()
        .find(|l| l.operand == "=A(*)")
        .unwrap();
    assert_eq!(address.data, module.code[2].offset.to_be_bytes());
    let repeated = listing
        .literals
        .iter()
        .find(|l| l.operand == "=3F'9'")
        .unwrap();
    assert_eq!(repeated.length, 12);
    assert_eq!(repeated.data, [0, 0, 0, 9].repeat(3));
    let text = listing.text();
    assert_eq!(text.matches("LITERAL").count(), 4);
    assert!(text.find("LITERAL").unwrap() < text.find("NEXT L 5").unwrap());
    assert!(
        listing
            .cross_reference()
            .contains("LITERAL OFFSET LENGTH POOL REFERENCES")
    );
    let back: hlasm::AssemblyListing =
        serde_json::from_value(serde_json::to_value(&listing).unwrap()).unwrap();
    assert_eq!(back.text(), text);
}

#[test]
fn literal_pool_listing_observes_pool_print_and_data_controls() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pool-print.asm");
    std::fs::write(&path, "MAIN CSECT\n USING MAIN,15\n L 2,=F'123'\n B NEXT\n PRINT OFF\n LTORG\n PRINT ON,DATA\nNEXT L 3,=3F'9'\n BR 14\n END\n").unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let listing = report.listing.unwrap();
    assert_eq!(listing.literals.len(), 2);
    let text = listing.text();
    assert_eq!(text.matches("LITERAL").count(), 1);
    assert!(!text.contains("0000007B"));
    assert!(text.contains("000000090000000900000009"));
    assert!(listing.cross_reference().contains("=F'123'"));
}

#[test]
fn conditional_assembly_models_keep_original_and_expanded_text_and_nogen_limits_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("models.asm");
    std::fs::write(&path, "&N SETA 0\n&TEXT SETC 'ABCDEFGHIJKLMNOP'\nMAIN CSECT\n BR 14\n.LOOP ANOP\n&N SETA &N+1\nITEM&N DC CL16'&TEXT'\n AIF (&N LT 2).LOOP\nPLAIN DC C'&&'\n END\n").unwrap();
    let plain = hlasm::compile_report(&path, &[]);
    assert!(plain.module.is_some(), "{:?}", plain.diagnostics);
    for generated_option in ["GEN", "NOGEN"] {
        let report = hlasm::compile_report_with_options(
            &path,
            &[],
            &hlasm::AssemblyOptions {
                collect_listing: true,
                pcontrol: hlasm::parse_pcontrol(&format!("ON,DATA,{generated_option}")).unwrap(),
                ..Default::default()
            },
        );
        assert_eq!(
            serde_json::to_value(&plain.module).unwrap(),
            serde_json::to_value(&report.module).unwrap()
        );
        let listing = report.listing.unwrap();
        let models: Vec<_> = listing
            .statements
            .iter()
            .filter(|row| row.model.is_some())
            .collect();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].label, "ITEM1");
        assert_eq!(models[1].label, "ITEM2");
        assert_eq!(models[0].model.as_ref().unwrap().label, "ITEM&N");
        assert_eq!(models[0].model.as_ref().unwrap().operands, "CL16'&TEXT'");
        assert_eq!(models[0].location.line, models[1].location.line);
        assert_ne!(models[0].location.statement, models[1].location.statement);
        let text = listing.text();
        assert_eq!(text.matches("MODEL ITEM&N DC CL16'&TEXT'").count(), 2);
        assert!(text.contains("PLAIN DC"));
        if generated_option == "GEN" {
            assert!(text.contains("ITEM1 DC CL16'ABCDEFGHIJKLMNOP'"));
            assert!(text.contains("C1C2C3C4C5C6C7C8C9D1D2D3D4D5D6D7"));
        } else {
            assert!(!text.contains("ITEM1 DC"));
            assert_eq!(text.matches("C1C2C3C4C5C6C7C8").count(), 2);
            assert!(!text.contains("C1C2C3C4C5C6C7C8C9"));
        }
        let back: hlasm::AssemblyListing =
            serde_json::from_value(serde_json::to_value(&listing).unwrap()).unwrap();
        assert_eq!(back.text(), text);
    }
}

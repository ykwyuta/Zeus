use zeus_cobol::hlasm;

fn run(source: &str) -> i32 {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("operations.asm");
    std::fs::write(&path, source).unwrap();
    let module = hlasm::compile_file(&path, &[]).unwrap();
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128).unwrap());
    hlasm::execute(&module, &mut memory, 128, &[]).unwrap()
}

#[test]
fn opsyn_copies_machine_and_conditional_operations_at_definition() {
    assert_eq!(
        run(
            "ADD OPSYN AHI\nOLD OPSYN ADD\nADD OPSYN SR\nIF OPSYN AIF\n         IF (O'OLD EQ 'O' AND O'ADD EQ 'O').GOOD\n         MNOTE 8,'operation type wrong'\n.GOOD ANOP\nMAIN CSECT\n         LGHI 15,2\n         OLD 15,7\n         BR 14\n         END\n"
        ),
        9
    );
}

#[test]
fn deleted_operations_have_undefined_attributes_and_can_be_restored() {
    assert_eq!(
        run(
            "KEEP OPSYN LGHI\nLGHI OPSYN\n         AIF (O'LGHI EQ 'U').OK\n         MNOTE 8,'deleted operation type wrong'\n.OK ANOP\nLGHI OPSYN KEEP\nMAIN CSECT\n         LGHI 15,7\n         BR 14\n         END\n"
        ),
        7
    );
}

#[test]
fn nested_macro_definitions_and_macro_snapshots_expand_correctly() {
    assert_eq!(
        run(
            "         MACRO\n         OUTER\n         MACRO\n         INNER &N\n         LGHI 15,&N\n         MEND\n         MEND\n         OUTER\nCOPYIN OPSYN INNER\nINNER OPSYN\n         AIF (O'COPYIN EQ 'M' AND O'INNER EQ 'U').OK\n         MNOTE 8,'macro operation type wrong'\n.OK ANOP\nMAIN CSECT\n         COPYIN 11\n         BR 14\n         END\n"
        ),
        11
    );
}

#[test]
fn nested_definitions_shield_inner_variables_from_outer_parameter_substitution() {
    assert_eq!(
        run(
            " MACRO\n OUTER &N\n LCLA &V\n&V SETA 7\n MACRO\n INNER &N\n LCLA &V\n&V SETA 3\n LA 15,&V+&N\n MEND\n LA 2,&V\n INNER &N\n AR 15,2\n MEND\nMAIN CSECT\n OUTER 11\n BR 14\n END\n"
        ),
        21
    );
}

#[test]
fn cnop_emits_executable_padding_and_consecutive_zero_length_cnops_stay_distinct() {
    assert_eq!(
        run(
            "MAIN CSECT\n         LGHI 15,3\n         CNOP 4,8\n         CNOP 0,16\n         AHI 15,5\n         BR 14\n         END\n"
        ),
        8
    );
}

#[test]
fn push_pop_using_restores_base_selection() {
    assert_eq!(
        run(
            "MAIN CSECT\n         USING MAIN,12\n         LR 12,15\n         PUSH USING\n         DROP 12\n         USING MAIN,11\n         LR 11,15\n         L 3,VALUE\n         POP USING\n         L 15,VALUE\n         BR 14\nVALUE DC F'17'\n         END\n"
        ),
        17
    );
}

#[test]
fn macros_can_override_instructions_and_conditional_operations_with_saved_aliases() {
    assert_eq!(
        run(
            "OLDLOAD OPSYN LGHI\nOLDSET OPSYN SETA\n MACRO\n LGHI &R,&N\n OLDLOAD &R,&N+1\n MEND\n MACRO\n SETA &R,&N\n OLDLOAD &R,&N+2\n MEND\nMAIN CSECT\n LGHI 2,10\n SETA 3,20\n AR 2,3\n LR 15,2\n BR 14\n END\n"
        ),
        33
    );
}

#[test]
fn macro_header_and_trailer_aliases_keep_sequence_symbols_in_their_own_scopes() {
    assert_eq!(
        run(
            "BEGIN OPSYN MACRO\nFINISH OPSYN MEND\n BEGIN\n INNER\n AGO .DONE\n.DONE LGHI 15,29\n FINISH\n AGO .DONE\n MNOTE 8,'wrong outer branch'\n.DONE ANOP\nMAIN CSECT\n INNER\n BR 14\n END\n"
        ),
        29
    );
}

#[test]
fn independent_push_pop_status_classes_allow_combined_requests_and_detect_underflow() {
    assert_eq!(
        run(
            "MAIN CSECT\n USING MAIN,12\n LR 12,15\n PUSH USING,PRINT,ACONTROL,NOPRINT\n PRINT OFF,NOGEN,NODATA\n ACONTROL OPTABLE(ZEUS)\n DROP 12\n POP PRINT,NOPRINT\n POP USING,ACONTROL\n L 15,VALUE\n BR 14\nVALUE DC F'23'\n END\n"
        ),
        23
    );
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("control.asm");
    for (body, expected) in [
        (" POP PRINT", "POP PRINT without"),
        (" POP ACONTROL", "POP ACONTROL without"),
        (" PUSH PRINT,PRINT", "duplicate"),
        (" PUSH NOPRINT", "at least one"),
        (" PRINT UNKNOWN", "unsupported PRINT"),
    ] {
        std::fs::write(
            &path,
            format!("MAIN CSECT\n{body}\n LA 15,0\n BR 14\n END\n"),
        )
        .unwrap();
        assert!(
            hlasm::compile_file(&path, &[])
                .unwrap_err()
                .contains(expected)
        );
    }
}

#[test]
fn character_pairing_dequote_and_dc_length_functions() {
    assert_eq!(
        run(
            "&TEXT SETC DEQUOTE('''value''')\n&VALUE SETC DCVAL('a''''b')\n&LEN SETA DCLEN('a''''b')\n         AIF ('&TEXT' EQ 'value' AND &LEN EQ 3).NEXT\n         MNOTE 8,'text or length failed'\n.NEXT AIF ('&VALUE' EQ 'a''b').OK\n         MNOTE 8,'string functions failed'\n.OK ANOP\nMAIN CSECT\n         LGHI 15,&LEN\n         BR 14\n         END\n"
        ),
        3
    );
}

#[test]
fn cnop_alignment_survives_linking_serialization_and_loading() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("align.asm");
    std::fs::write(&file, "ALIGNED CSECT\n         LGHI 15,6\n         CNOP 0,32\nHERE BR 14\n         ENTRY HERE\n         END\n").unwrap();
    let module = hlasm::compile_file(&file, &[]).unwrap();
    assert_eq!(module.alignment, 32);
    assert_eq!(module.entries["HERE"] % 32, 0);
    let bytes = hlasm::module_bytes(&module).unwrap();
    let module = hlasm::load_module(&bytes).unwrap();
    let linked = hlasm::link_modules(&[module]).unwrap();
    assert_eq!(linked.alignment, 32);
    assert!(linked.storage_at(136).is_err());
    let mut machine = zeus_cobol::vm::Machine::new(vec![]);
    assert_eq!(machine.run_hlasm_main(linked).unwrap(), 6);
}

#[test]
fn acontrol_afpr_tracks_each_statement_and_restores_nested_status_across_members() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("afpr.asm");
    std::fs::write(dir.path().join("ENABLE.cpy"), " ACONTROL AFPR\n LDR 9,1\n").unwrap();
    std::fs::write(&path, "MAIN CSECT\n USING MAIN,15\n ACONTROL NOAFPR\n LDR 2,6\n PUSH ACONTROL,PRINT,NOPRINT\n COPY ENABLE\n PUSH ACONTROL\n ACONTROL NOAFPR,NOPRINT\n LXDB 4,=DB'1'\n POP ACONTROL\n LDR 15,11\n POP ACONTROL,PRINT\n LDR 0,6\n CFDBR 3,5,0\n LDGR 0,3\n LGDR 3,0\n EFPC 3\n SFPC 3\n EEDTR 3,2\n IEDTR 0,2,3\n RRDTR 0,2,3,8\n BR 14\n END\n").unwrap();
    let dirs = [dir.path().to_path_buf()];
    let plain = hlasm::compile_report(&path, &dirs);
    assert!(plain.module.is_some(), "{:?}", plain.diagnostics);
    let report = hlasm::compile_report_with_options(
        &path,
        &dirs,
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert_eq!(
        serde_json::to_value(plain.module).unwrap(),
        serde_json::to_value(report.module).unwrap()
    );
    // POP has restored NOAFPR despite changes in an included member.
    let source = std::fs::read_to_string(&path)
        .unwrap()
        .replace(" LDR 0,6", " LDR 1,6");
    std::fs::write(&path, source).unwrap();
    let report = hlasm::compile_report(&path, &dirs);
    assert!(report.module.is_none());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.line == 13 && d.message.contains("NOAFPR")),
        "{:?}",
        report.diagnostics
    );
}

#[test]
fn acontrol_noafpr_restricts_all_floating_roles_but_keeps_gprs_and_masks_unrestricted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("afpr-roles.asm");
    for instruction in [
        "LDR 1,0",
        "LDR 0,9",
        "ADBR 0,9",
        "MADBR 0,2,9",
        "MADB 0,9,=DB'1'",
        "LDEB 9,=EB'1'",
        "LXDB 8,=DB'1'",
        "LXR 8,0",
        "LDGR 9,3",
        "LGDR 3,9",
        "TBEDR 0,0,9",
        "CPSDR 0,2,9",
        "MAY 0,9,=D'1'",
        "STD 9,OUT",
        "TDCDT 9,0",
        "SLDT 0,9,1",
        "CDPT 9,OUT(1),8",
    ] {
        let source = format!(
            "MAIN CSECT\n USING MAIN,15\n ACONTROL NOAFPR\n {instruction}\n BR 14\nOUT DS 4D\n END\n"
        );
        std::fs::write(&path, &source).unwrap();
        let report = hlasm::compile_report(&path, &[]);
        assert!(report.module.is_none(), "{instruction}: NOAFPR accepted");
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.line == 4 && d.message.contains("NOAFPR")),
            "{instruction}: {:?}",
            report.diagnostics
        );
        std::fs::write(&path, source.replace("ACONTROL NOAFPR", "ACONTROL AFPR")).unwrap();
        let report = hlasm::compile_report(&path, &[]);
        assert!(
            report.module.is_some(),
            "{instruction}: AFPR rejected: {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn acontrol_macrocase_changes_only_macro_operand_values_and_restores_with_stack() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("macrocase.asm");
    let text = " MACRO\n PROBE &VALUE,&KEY=defaultCase\n MNOTE 0,'VALUE=&VALUE|KEY=&KEY|LIST=&SYSLIST(1)'\n MEND\nMAIN CSECT\n ACONTROL COMPAT(MACROCASE)\n PROBE mixedCase,KEY=otherCase\n PROBE 'quotedCase',KEY='quotedKey'\n PROBE C'characterCase'\n PUSH ACONTROL\n ACONTROL CPAT(NOMC)\n PROBE mixedCase\n POP ACONTROL\n PROBE mixedCase\n BR 14\n END\n";
    std::fs::write(&path, text).unwrap();
    let report = hlasm::compile_report_with_options(
        &path,
        &[],
        &hlasm::AssemblyOptions {
            collect_listing: true,
            ..Default::default()
        },
    );
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let messages: Vec<_> = report
        .diagnostics
        .iter()
        .filter(|d| d.operation == "MNOTE")
        .map(|d| d.message.as_str())
        .collect();
    assert_eq!(
        messages,
        [
            "VALUE=MIXEDCASE|KEY=OTHERCASE|LIST=MIXEDCASE",
            "VALUE='quotedCase'|KEY='quotedKey'|LIST='quotedCase'",
            "VALUE=C'CHARACTERCASE'|KEY=defaultCase|LIST=C'CHARACTERCASE'",
            "VALUE=mixedCase|KEY=defaultCase|LIST=mixedCase",
            "VALUE=MIXEDCASE|KEY=defaultCase|LIST=MIXEDCASE"
        ]
    );
    let listing = report.listing.unwrap();
    assert!(
        listing
            .original_source_text()
            .contains("PROBE mixedCase,KEY=otherCase")
    );
    assert_eq!(listing.macro_calls[0].operands, "mixedCase,KEY=otherCase");
}

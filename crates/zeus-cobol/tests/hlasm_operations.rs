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

use zeus_cobol::hlasm;

fn execute(body: &str, data: &str) -> Result<(i32, hlasm::Module, Vec<u8>), String> {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("strings.asm");
    std::fs::write(&file, format!("MAIN CSECT\n         USING MAIN,12\n         LR 12,15\n{body}\n         BR 14\n{data}\n         END\n")).unwrap();
    let module = hlasm::compile_file(&file, &[])?;
    let mut storage = vec![0; 128];
    storage.extend(module.storage_at(128)?);
    let result = hlasm::execute(&module, &mut storage, 128, &[])?;
    Ok((result, module, storage))
}

#[test]
fn extended_long_move_and_compare_resume_after_partial_completion() {
    let body = "         LARL 4,OUT\n         LGFI 5,5000\n         LGFI 6,2147483647\n         LGHI 7,0\nMOVE MVCLE 4,6,90\n         BRC 1,MOVE\n         LGFI 2,5000\n         SR 4,2\n         LR 8,4\n         LGFI 5,5000\n         LGHI 7,0\nCMP CLCLE 4,6,90\n         BRC 1,CMP\n         BRC 8,EQUAL\n         LGHI 15,99\n         BR 14\nEQUAL CLI 0(8),90\n         BRC 7,FAIL\n         LARL 8,OUT+4998\n         CLI 1(8),90\n         BRC 7,FAIL\n         LR 15,5\n         BR 14\nFAIL LGHI 15,98";
    assert_eq!(execute(body, "OUT DS 5000C").unwrap().0, 0);
}

#[test]
fn terminated_move_search_and_compare_preserve_completion_registers() {
    let body = "         LGHI 0,0\n         LA 2,OUT\n         LA 3,SRC\n         LR 6,3\n         MVST 2,3\n         BRC 4,MOVED\n         LGHI 15,99\n         BR 14\nMOVED CR 3,6\n         BRC 7,FAIL\n         LA 4,OUT\n         SR 2,4\n         CHI 2,3\n         BRC 7,FAIL\n         LA 2,OUT\n         CLST 2,3\n         BRC 7,FAIL\n         LGHI 0,194\n         LA 2,OUT+4\n         LA 3,OUT\n         SRST 2,3\n         BRC 4,FOUND\n         LGHI 15,97\n         BR 14\nFOUND LA 4,OUT+1\n         SR 2,4\n         LR 15,2\n         BR 14\nFAIL LGHI 15,98";
    // Use CH against a literal rather than depending on a newer CHI alias.
    let body = body.replace("CHI 2,3", "CH 2,=H'3'");
    assert_eq!(
        execute(&body, "SRC DC C'ABC',X'00'\nOUT DS CL4").unwrap().0,
        0
    );
}

#[test]
fn terminated_compare_treats_the_terminator_as_low_and_checks_reserved_bits() {
    assert_eq!(execute("         LGHI 0,240\n         LA 2,A\n         LA 3,B\n         CLST 2,3\n         BRC 4,LOW\n         LGHI 15,99\n         BR 14\nLOW LGHI 15,1", "A DC C'A0'\nB DC C'AB0'").unwrap().0, 1);
    for op in ["MVST", "CLST", "SRST"] {
        assert!(
            execute(&format!("         LGHI 0,256\n         {op} 2,3"), "")
                .unwrap_err()
                .contains("reserved bits")
        );
    }
}

#[test]
fn translate_and_test_preserves_source_and_reports_address_function_and_cc() {
    for (op, operand) in [("TRT", "INPUT(3)"), ("TRTR", "INPUT+2(3)")] {
        let body = format!(
            "         MVI TABLE+2,7\n         LGFI 2,X'12340000'\n         {op} {operand},TABLE\n         BRC 4,FOUND\n         LGHI 15,99\n         BR 14\nFOUND LA 3,INPUT+1\n         SR 1,3\n         LR 15,2\n         OR 15,1"
        );
        assert_eq!(
            execute(&body, "INPUT DC X'010203'\nTABLE DC 256X'00'")
                .unwrap()
                .0,
            0x12340007
        );
    }
}

#[test]
fn translate_and_test_no_match_leaves_registers_and_last_match_sets_cc_two() {
    for (op, operand, table_index) in [("TRT", "INPUT(3)", 3), ("TRTR", "INPUT+2(3)", 1)] {
        let body = format!(
            "         LGHI 1,6\n         LGHI 2,7\n         {op} {operand},TABLE\n         BRC 8,ZERO\n         LGHI 15,90\n         BR 14\nZERO AR 1,2\n         AHI 1,-13\n         BRC 8,SAVED\n         LGHI 15,92\n         BR 14\nSAVED MVI TABLE+{table_index},9\n         {op} {operand},TABLE\n         BRC 2,LAST\n         LGHI 15,91\n         BR 14\nLAST LR 15,2"
        );
        assert_eq!(
            execute(&body, "INPUT DC X'010203'\nTABLE DC 256X'00'")
                .unwrap()
                .0,
            9
        );
    }
}

#[test]
fn decimal_shift_and_round_supports_positive_negative_and_extreme_shifts() {
    for (nominal, length, shift, rounding, result) in [
        ("-12345", 3, 63, 5, -1235),
        ("12", 3, 2, 0, 1200),
        ("9", 1, 32, 9, 0),
        ("0", 3, 31, 0, 0),
        ("-1", 2, 0, 9, -1),
    ] {
        let body = format!(
            "         SRP VALUE({length}),{shift},{rounding}\n         ZAP OUT,VALUE\n         CVB 15,OUT"
        );
        assert_eq!(
            execute(
                &body,
                &format!("VALUE DC PL{length}'{nominal}'\nOUT DS PL8")
            )
            .unwrap()
            .0,
            result
        );
    }
}

#[test]
fn decimal_shift_overflow_keeps_negative_sign_even_when_truncated_to_zero() {
    let (_, module, memory) = execute("         SRP VALUE(2),3,0\n         BRC 1,OVERFLOW\n         LGHI 15,99\n         BR 14\nOVERFLOW LGHI 15,0", "VALUE DC PL2'-100'\n         ENTRY VALUE").unwrap();
    let at = 128 + module.entries["VALUE"] as usize;
    assert_eq!(&memory[at..at + 2], [0, 0x0d]);
}

#[test]
fn invalid_decimal_operands_and_rounding_digits_are_rejected() {
    assert!(
        execute("         SRP VALUE(2),0,0", "VALUE DC X'1A2C'")
            .unwrap_err()
            .contains("packed decimal digit")
    );
    assert!(
        execute("         SRP VALUE(2),0,10", "VALUE DC P'1'")
            .unwrap_err()
            .contains("rounding digit")
    );
}

#[test]
fn move_long_pads_and_compare_long_updates_pairs_and_condition_codes() {
    let body = "         LA 2,OUT\n         LGHI 3,5\n         LA 4,INPUT\n         LGFI 5,X'40000003'\n         MVCL 2,4\n         BRC 2,PADDED\n         LGHI 15,99\n         BR 14\nPADDED LA 2,OUT\n         LGHI 3,5\n         LA 4,INPUT\n         LGFI 5,X'40000003'\n         CLCL 2,4\n         BRC 8,EQUAL\n         LGHI 15,98\n         BR 14\nEQUAL LR 15,3";
    assert_eq!(execute(body, "INPUT DC C'ABC'\nOUT DS CL5").unwrap().0, 0);
}

#[test]
fn move_long_detects_destructive_overlap_without_changing_data() {
    let body = "         LA 2,INPUT+1\n         LGHI 3,3\n         LA 4,INPUT\n         LGHI 5,3\n         MVCL 2,4\n         BRC 1,OVERLAP\n         LGHI 15,99\n         BR 14\nOVERLAP CLC INPUT(5),=C'ABCDE'\n         BRC 8,UNCHANGED\n         LGHI 15,98\n         BR 14\nUNCHANGED LGHI 15,0";
    assert_eq!(execute(body, "INPUT DC C'ABCDE'").unwrap().0, 0);
}

#[test]
fn compare_long_stops_at_first_difference_and_zero_lengths_ignore_addresses() {
    let body = "         LA 2,INPUT\n         LGHI 3,3\n         LA 4,OTHER\n         LGHI 5,3\n         CLCL 2,4\n         BRC 4,LOW\n         LGHI 15,99\n         BR 14\nLOW AHI 3,-2\n         LGHI 15,0\n         OR 15,3\n         LGFI 2,2147483647\n         LGFI 4,2147483647\n         LGHI 3,0\n         LGHI 5,0\n         MVCL 2,4\n         CLCL 2,4";
    assert_eq!(
        execute(body, "INPUT DC C'ABC'\nOTHER DC C'AXC'").unwrap().0,
        0
    );
    assert!(
        execute("         MVCL 3,4", "")
            .unwrap_err()
            .contains("even register pairs")
    );
}

#[test]
fn edit_suppresses_zeroes_preserves_negative_messages_and_sets_cc() {
    for (value, expected, mask) in [
        ("12", "4040F1F240", 2),
        ("-12", "4040F1F260", 4),
        ("0", "4040404040", 8),
    ] {
        let body = format!(
            "         ED OUT(5),VALUE\n         BRC {mask},SIGN\n         LGHI 15,99\n         BR 14\nSIGN CLC OUT(5),EXPECTED\n         BRC 8,MATCHED\n         LGHI 15,98\n         BR 14\nMATCHED LGHI 15,0"
        );
        assert_eq!(
            execute(
                &body,
                &format!("OUT DC X'4020202060'\nVALUE DC PL2'{value}'\nEXPECTED DC X'{expected}'")
            )
            .unwrap()
            .0,
            0
        );
    }
}

#[test]
fn edit_significance_starter_and_field_separator_control_each_field() {
    let body = "         ED OUT(7),VALUE\n         CLC OUT(7),EXPECTED\n         BRC 8,MATCHED\n         LGHI 15,99\n         BR 14\nMATCHED LGHI 15,0";
    assert_eq!(
        execute(
            body,
            "OUT DC X'402021204B2020'\nVALUE DC PL3'26'\nEXPECTED DC X'404040F04BF2F6'"
        )
        .unwrap()
        .0,
        0
    );
    let body = "         ED OUT(8),VALUE\n         BRC 4,NEGATIVE\n         LGHI 15,99\n         BR 14\nNEGATIVE CLC OUT(8),EXPECTED\n         BRC 8,MATCHED\n         LGHI 15,98\n         BR 14\nMATCHED LGHI 15,0";
    assert_eq!(execute(body, "OUT DC X'4020202022202020'\nVALUE DC PL2'123',PL2'-45'\nEXPECTED DC X'40F1F2F34040F4F5'").unwrap().0, 0);
}

#[test]
fn edit_and_mark_reports_first_significant_digit_and_zero_preserves_register() {
    assert_eq!(
        execute(
            "         EDMK OUT(4),VALUE\n         LA 3,OUT+2\n         SR 1,3\n         LR 15,1",
            "OUT DC X'40202020'\nVALUE DC PL2'12'"
        )
        .unwrap()
        .0,
        0
    );
    assert_eq!(
        execute(
            "         LGHI 1,88\n         EDMK OUT(4),VALUE\n         LR 15,1",
            "OUT DC X'40202020'\nVALUE DC PL2'0'"
        )
        .unwrap()
        .0,
        88
    );
    assert!(
        execute(
            "         ED OUT(4),VALUE",
            "OUT DC X'40202020'\nVALUE DC X'A12C'"
        )
        .unwrap_err()
        .contains("edit digit")
    );
    assert_eq!(
        execute(
            "         ED OUT(1),4095(0)\n         LGHI 15,0",
            "OUT DC C' '"
        )
        .unwrap()
        .0,
        0
    );
}

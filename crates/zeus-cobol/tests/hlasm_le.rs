use zeus_cobol::{hlasm, vm::Machine};

fn compile(source: &str) -> hlasm::Module {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("le.asm");
    std::fs::write(&file, source).unwrap();
    hlasm::compile_file(&file, &[]).unwrap()
}

#[test]
fn assembler_links_date_service_and_loads_an_entry_with_fixed_arguments() {
    let module = compile(
        "MAIN CSECT\n USING MAIN,15\n LR 12,14\n LINK EP=CEEDAYS,PARAM=(DATE,PIC,DAY,FC),VL=1\n LARL 15,MAIN\n CLC FC(12),=XL12'0'\n BNE BAD\n LOAD EP=CEEDATE\n LR 6,0\n LARL 15,MAIN\n LA 1,PARMS\n LR 15,6\n BASR 14,15\n LARL 15,MAIN\n CLC OUTPUT(10),DATE+2\n BNE BAD\n CLC FC(12),=XL12'0'\n BNE BAD\n DELETE EP=CEEDATE\n LARL 15,MAIN\n XR 15,15\n BR 12\nBAD LA 15,99\n BR 12\nDATE DC H'10',C'2000-03-01'\nPIC DC H'10',C'YYYY-MM-DD'\nDAY DS F\nOUTPUT DS CL80\nFC DS 3F\nPARMS DC A(DAY,PIC,OUTPUT,FC)\n END\n",
    );
    let mut machine = Machine::new(vec![]);
    assert_eq!(machine.run_hlasm_main(module).unwrap(), 0);
}

#[test]
fn amode64_calls_le_through_explicit_four_byte_lists() {
    let module = compile(
        "MAIN CSECT\n AMODE 64\n USING MAIN,15\n STG 14,RETURN\n LINKX EP=CEEDAYS,PARAM=(DATE,PIC,DAY,FC),PLIST4=YES\n LARL 15,MAIN\n CLC FC(12),=XL12'0'\n BNE BAD\n L 3,DAY\n C 3,=F'1'\n BNE BAD\n LG 14,RETURN\n XR 15,15\n BR 14\nBAD LG 14,RETURN\n LA 15,99\n BR 14\nDATE DC H'8',C'15821015'\nPIC DC H'8',C'YYYYMMDD'\nDAY DS F\nFC DS 3F\nRETURN DS AD\n END\n",
    );
    assert_eq!(Machine::new(vec![]).run_hlasm_main(module).unwrap(), 0);
    let module = compile(
        "MAIN CSECT\n AMODE 64\n USING MAIN,15\n LINKX EP=CEEGMT,PARAM=(DAY,SECONDS,FC),PLIST8=YES\n BR 14\nDAY DS F\nSECONDS DS D\nFC DS 3F\n END\n",
    );
    let error = Machine::new(vec![]).run_hlasm_main(module).unwrap_err();
    assert!(
        error.contains("four-byte AMODE 31 parameter list"),
        "{error}"
    );
}

#[test]
fn malformed_le_string_and_output_areas_report_errors_without_panicking() {
    for source in [
        "MAIN CSECT\n USING MAIN,15\n LINK EP=CEEDAYS,PARAM=(DATE,PIC,DAY,FC)\n BR 14\nDATE DC X'FFFF'\nPIC DC H'8',C'YYYYMMDD'\nDAY DS F\nFC DS 3F\n END\n",
        "MAIN CSECT\n USING MAIN,15\n LINK EP=CEEDATE,PARAM=(DAY,PIC,BAD,FC)\n BR 14\nDAY DC F'1'\nPIC DC H'8',C'YYYYMMDD'\nFC DS 3F\nBAD EQU *+300\n END\n",
    ] {
        let module = compile(source);
        let error = Machine::new(vec![]).run_hlasm_main(module).unwrap_err();
        assert!(error.contains("outside run-unit storage"), "{error}");
    }
}

#[test]
fn assembler_le_abend_propagates_original_code() {
    let module = compile(
        "MAIN CSECT\n USING MAIN,15\n LINK EP=CEE3ABD,PARAM=(CODE,TIMING)\n BR 14\nCODE DC F'1234'\nTIMING DC F'0'\n END\n",
    );
    let error = Machine::new(vec![]).run_hlasm_main(module).unwrap_err();
    assert_eq!(error, "\0abend U1234");
}

#[test]
fn construct_condition_token_preserves_abi_fields_and_supports_both_token_cases() {
    for (c1, case, severity, control, facility, expected) in [
        ("0000", 1, 0, 1, "CEE", "0000FEDC41C3C5C5F1234567"),
        ("0003", 1, 3, 1, "CEE", "0003FEDC59C3C5C5F1234567"),
        ("0004", 1, 4, 0, "Z9q", "0004FEDC60E9F998F1234567"),
        ("BEEF", 2, 3, 0, "Z9q", "BEEFFEDC98E9F998F1234567"),
        ("BEEF", 2, 4, 1, "CEE", "BEEFFEDCA1C3C5C5F1234567"),
    ] {
        let module = compile(&format!(
            "MAIN CSECT\n USING MAIN,15\n LR 12,14\n LINK EP=CEENCOD,PARAM=(C1,C2,CASE,SEV,CTL,FAC,ISI,TOK,FC)\n LARL 15,MAIN\n CLC TOK(12),WANT\n BNE BAD\n CLC FC(12),=XL12'0'\n BNE BAD\n XR 15,15\n BR 12\nBAD LA 15,99\n BR 12\nC1 DC X'{c1}'\nC2 DC X'FEDC'\nCASE DC H'{case}'\nSEV DC H'{severity}'\nCTL DC H'{control}'\nFAC DC C'{facility}'\nISI DC X'F1234567'\nTOK DC XL12'AA'\nFC DC XL12'BB'\nWANT DC X'{expected}'\n END\n"
        ));
        assert_eq!(
            Machine::new(vec![]).run_hlasm_main(module).unwrap(),
            0,
            "case {case} severity {severity}"
        );
    }
}

#[test]
fn construct_condition_reports_input_errors_and_keeps_failed_output_unchanged() {
    for (case, severity, control, facility, message, feedback_severity, flags) in [
        (0, 3, 1, "CEE", 401, 3, "59"),
        (3, 3, 1, "CEE", 401, 3, "59"),
        (1, 3, 2, "CEE", 402, 3, "59"),
        (1, -1, 1, "CEE", 403, 3, "59"),
        (1, 5, 1, "CEE", 403, 3, "59"),
        (1, 3, 0, "ABC", 452, 3, "59"),
        (1, 3, 0, "0AZ", 452, 3, "59"),
        (1, 3, 0, "Z?Z", 404, 1, "49"),
    ] {
        let check = if feedback_severity == 1 {
            " CLC TOK+5(3),FAC\n BNE BAD\n"
        } else {
            " CLC TOK(12),KEEP\n BNE BAD\n"
        };
        let module = compile(&format!(
            "MAIN CSECT\n USING MAIN,15\n LR 12,14\n LINK EP=CEENCOD,PARAM=(C1,C2,CASE,SEV,CTL,FAC,ISI,TOK,FC)\n LARL 15,MAIN\n CLC FC(12),WANT\n BNE BAD\n{check} XR 15,15\n BR 12\nBAD LA 15,99\n BR 12\nC1 DC H'3'\nC2 DC H'765'\nCASE DC H'{case}'\nSEV DC H'{severity}'\nCTL DC H'{control}'\nFAC DC C'{facility}'\nISI DC F'0'\nTOK DC X'112233445566778899AABBCC'\nKEEP DC X'112233445566778899AABBCC'\nFC DS 3F\nWANT DC X'{feedback_severity:04X}{message:04X}{flags}C3C5C500000000'\n END\n"
        ));
        assert_eq!(
            Machine::new(vec![]).run_hlasm_main(module).unwrap(),
            0,
            "error {message}"
        );
    }
}

#[test]
fn loaded_condition_constructor_accepts_omitted_feedback_and_reads_rsect_inputs() {
    let module = compile(
        "MAIN CSECT\n USING MAIN,15\n LR 12,14\n LOAD EP=CEENCOD\n LR 6,0\n LARL 15,MAIN\n LA 1,PARMS\n LR 15,6\n BASR 14,15\n LARL 15,MAIN\n CLC TOK(12),WANT\n BNE BAD\n DELETE EP=CEENCOD\n XR 15,15\n BR 12\nBAD LA 15,99\n BR 12\nTOK DS 3F\nPARMS DC A(C1,C2,CASE,SEV,CTL,FAC,ISI,TOK,0)\nCONST RSECT\nC1 DC H'2'\nC2 DC H'765'\nCASE DC H'1'\nSEV DC H'2'\nCTL DC H'0'\nFAC DC C'Z9q'\nISI DC F'-1'\nWANT DC X'000202FD50E9F998FFFFFFFF'\n END MAIN\n",
    );
    assert_eq!(Machine::new(vec![]).run_hlasm_main(module).unwrap(), 0);
}

#[test]
fn condition_constructor_without_feedback_uses_default_severe_condition_action() {
    let module = compile(
        "MAIN CSECT\n USING MAIN,15\n LINK EP=CEENCOD,PARAM=(C1,C2,CASE,SEV,CTL,FAC,ISI,TOK)\n BR 14\nC1 DC H'0'\nC2 DC H'765'\nCASE DC H'0'\nSEV DC H'0'\nCTL DC H'0'\nFAC DC C'Z9q'\nISI DC F'0'\nTOK DS 3F\n END\n",
    );
    assert_eq!(
        Machine::new(vec![]).run_hlasm_main(module).unwrap_err(),
        "\0abend U4038"
    );
}

#[test]
fn cobol_constructs_conditions_directly_and_through_an_assembler_loaded_entry() {
    let dir = tempfile::tempdir().unwrap();
    let wrapper = compile(
        "WRAPPER CSECT\n LR 12,14\n LR 2,1\n LOAD EP=CEENCOD\n LR 6,0\n LR 1,2\n BASR 14,6\n BR 12\n END\n",
    );
    hlasm::save_module(&wrapper, &dir.path().join("WRAPPER.zo")).unwrap();
    for target in ["CEENCOD", "WRAPPER"] {
        let path = dir.path().join("token.cbl");
        let text = format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. TOKENTEST.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 IDONE PIC S9(4) COMP VALUE 2.\n01 IDTWO PIC S9(4) COMP VALUE 931.\n01 STYLE PIC S9(4) COMP VALUE 1.\n01 LEVELNUM PIC S9(4) COMP VALUE 2.\n01 OWNERFLAG PIC S9(4) COMP VALUE 0.\n01 OWNERNAME PIC X(3) VALUE 'Z4m'.\n01 INSTANCEVAL PIC S9(9) COMP VALUE -1.\n01 TOKENOUT.\n  05 TOKENLEVEL PIC S9(4) COMP.\n  05 TOKENNUMBER PIC S9(4) COMP.\n  05 TOKENFLAGS PIC X.\n  05 TOKENOWNER PIC X(3).\n  05 TOKENINSTANCE PIC S9(9) COMP.\n01 STATUSOUT PIC X(12).\nPROCEDURE DIVISION.\n    CALL '{target}' USING IDONE IDTWO STYLE LEVELNUM\n        OWNERFLAG OWNERNAME INSTANCEVAL TOKENOUT STATUSOUT\n    IF TOKENLEVEL = 2 AND TOKENNUMBER = 931 AND\n       TOKENFLAGS = X'50' AND TOKENOWNER = 'Z4m' AND\n       TOKENINSTANCE = -1 AND STATUSOUT = LOW-VALUES\n       MOVE 0 TO RETURN-CODE\n    ELSE MOVE 99 TO RETURN-CODE END-IF\n    STOP RUN.\n"
        );
        let text: String = text
            .lines()
            .map(|line| format!("       {line}\n"))
            .collect();
        std::fs::write(&path, text).unwrap();
        let module = zeus_cobol::compile_file(&path, &Default::default()).unwrap();
        assert_eq!(
            Machine::new(vec![dir.path().to_path_buf()])
                .run_main(module)
                .unwrap(),
            0,
            "{target}"
        );
    }
}

#[test]
fn condition_constructor_rejects_protected_output_before_mutating_feedback() {
    let module = compile(
        "MAIN CSECT\n USING MAIN,15\n LINK EP=CEENCOD,PARAM=(C1,C2,CASE,SEV,CTL,FAC,ISI,TOK,FC)\n BR 14\nFC DS 3F\nCONST RSECT\nC1 DC H'1'\nC2 DC H'765'\nCASE DC H'1'\nSEV DC H'1'\nCTL DC H'0'\nFAC DC C'Z9q'\nISI DC F'0'\nTOK DS 3F\n END MAIN\n",
    );
    let error = Machine::new(vec![]).run_hlasm_main(module).unwrap_err();
    assert!(error.contains("storage protection"), "{error}");
}

#[test]
fn condition_handlers_run_in_lifo_order_receive_saved_tokens_and_unregister() {
    let module = compile(
        "MAIN CSECT\n USING MAIN,15\n ST 14,RET\n LINK EP=CEEHDLR,PARAM=(H1,T1,FC)\n LARL 15,MAIN\n LINK EP=CEEHDLR,PARAM=(H2,T2,FC)\n LARL 15,MAIN\n LINK EP=CEESGL,PARAM=(COND,Q,FC)\n LARL 15,MAIN\n CLC FC(12),=XL12'0'\n BNE BAD\n L 3,COUNT\n C 3,=F'21'\n BNE BAD\n LINK EP=CEEHDLU,PARAM=(H2,FC)\n LARL 15,MAIN\n CLC FC(12),=XL12'0'\n BNE BAD\n XC COUNT(4),COUNT\n LINK EP=CEESGL,PARAM=(COND,Q,FC)\n LARL 15,MAIN\n L 3,COUNT\n C 3,=F'1'\n BNE BAD\n L 14,RET\n XR 15,15\n BR 14\nBAD L 14,RET\n LA 15,99\n BR 14\nH1 DC A(FIRST)\nH2 DC A(SECOND)\nT1 DC F'1'\nT2 DC F'2'\nCOUNT DC F'0'\nCOND DC X'0003006358E9E9E900000000'\nQ DC F'0'\nFC DS 3F\nRET DS F\n DROP 15\nFIRST DS 0H\n USING FIRST,15\n L 2,4(1)\n L 3,0(2)\n LARL 4,COUNT\n L 5,0(4)\n MHI 5,10\n AR 5,3\n ST 5,0(4)\n L 2,8(1)\n LHI 3,10\n ST 3,0(2)\n BR 14\n DROP 15\nSECOND DS 0H\n USING SECOND,15\n L 2,4(1)\n L 3,0(2)\n LARL 4,COUNT\n ST 3,0(4)\n L 2,8(1)\n LHI 3,20\n ST 3,0(2)\n BR 14\n END MAIN\n",
    );
    assert_eq!(Machine::new(vec![]).run_hlasm_main(module).unwrap(), 0);
}

#[test]
fn handler_registration_dies_with_nested_hlasm_invocation() {
    let dir = tempfile::tempdir().unwrap();
    let child = compile(
        "CHILD CSECT\n USING CHILD,15\n ST 14,RET\n LINK EP=CEEHDLR,PARAM=(H,T,FC)\n LARL 15,CHILD\n L 14,RET\n BR 14\nH DC A(HAND)\nT DC F'9'\nFC DS 3F\nRET DS F\n DROP 15\nHAND DS 0H\n USING HAND,15\n L 2,8(1)\n LHI 3,10\n ST 3,0(2)\n BR 14\n END\n",
    );
    std::fs::write(
        dir.path().join("CHILD.zo"),
        hlasm::module_bytes(&child).unwrap(),
    )
    .unwrap();
    let main = compile(
        "MAIN CSECT\n USING MAIN,15\n ST 14,RET\n LINK EP=CHILD\n LARL 15,MAIN\n LINK EP=CEESGL,PARAM=(COND,Q,FC)\n LARL 15,MAIN\n CLC FC(8),COND\n BNE BAD\n L 14,RET\n XR 15,15\n BR 14\nBAD L 14,RET\n LA 15,99\n BR 14\nCOND DC X'0003006358E9E9E900000000'\nQ DC F'0'\nFC DS 3F\nRET DS F\n END\n",
    );
    assert_eq!(
        Machine::new(vec![dir.path().to_path_buf()])
            .run_hlasm_main(main)
            .unwrap(),
        0
    );
}

#[test]
fn decompose_condition_service_preserves_raw_fields_and_rejects_reserved_flags() {
    for (token, expected, fc) in [
        (
            "BEEFFEDCA1C3C5C5F1234567",
            "BEEFFEDC000200040001C3C5C5F1234567",
            "000000000000000000000000",
        ),
        (
            "000000000000000000000000",
            "0000000000000000000000000000000000",
            "000000000000000000000000",
        ),
        (
            "BEEFFEDCA7C3C5C5F1234567",
            "00AA00AA00AA00AA00AA0000AA000000AA",
            "0003006659C3C5C500000000",
        ),
    ] {
        let module = compile(&format!(
            "MAIN CSECT\n USING MAIN,15\n LR 12,14\n LINK EP=CEEDCOD,PARAM=(TOK,C1,C2,CASE,SEV,CTL,FAC,ISI,FC)\n LARL 15,MAIN\n CLC OUT(17),WANT\n BNE BAD\n CLC FC(12),WFC\n BNE BAD\n XR 15,15\n BR 12\nBAD LA 15,99\n BR 12\nTOK DC X'{token}'\nOUT DS 0H\nC1 DC XL2'AA'\nC2 DC XL2'AA'\nCASE DC XL2'AA'\nSEV DC XL2'AA'\nCTL DC XL2'AA'\nFAC DC XL3'AA'\nISI DC XL4'AA'\nFC DS 3F\nWANT DC X'{expected}'\nWFC DC X'{fc}'\n END\n"
        ));
        // The seven decomposed fields occupy 17 bytes (no implicit fullword alignment).
        assert_eq!(
            Machine::new(vec![]).run_hlasm_main(module).unwrap(),
            0,
            "{token}"
        );
    }
}

#[test]
fn cobol_handler_is_called_through_loaded_entry_and_released_on_return() {
    let dir = tempfile::tempdir().unwrap();
    let getter = compile(
        "GETPTR CSECT\n LR 12,14\n LR 3,1\n LOAD EP=CBLHAND\n L 2,0(3)\n ST 0,0(2)\n XR 15,15\n BR 12\n END\n",
    );
    hlasm::save_module(&getter, &dir.path().join("GETPTR.zo")).unwrap();
    let text = "IDENTIFICATION DIVISION.
PROGRAM-ID. DRIVER.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 ROUTINE PIC X(4).
01 USERTOKEN PIC S9(9) COMP VALUE 731.
01 CONDTOK PIC X(12) VALUE X'0003006358E9E9E900000000'.
01 FC PIC X(12).
PROCEDURE DIVISION.
 CALL 'GETPTR' USING ROUTINE
 CALL 'REGISTER' USING ROUTINE USERTOKEN FC
 CALL 'CEESGL' USING CONDTOK OMITTED FC
 IF FC = LOW-VALUES MOVE 91 TO RETURN-CODE STOP RUN END-IF
 CALL 'CEEHDLR' USING ROUTINE USERTOKEN FC
 CALL 'CEESGL' USING CONDTOK OMITTED FC
 IF FC NOT = LOW-VALUES\n MOVE 92 TO RETURN-CODE STOP RUN END-IF
 CALL 'CEEHDLU' USING ROUTINE FC
 CALL 'CEESGL' USING CONDTOK OMITTED FC
 IF FC = LOW-VALUES MOVE 93 TO RETURN-CODE\n ELSE MOVE 0 TO RETURN-CODE END-IF
 STOP RUN.
END PROGRAM DRIVER.
IDENTIFICATION DIVISION.
PROGRAM-ID. REGISTER.
DATA DIVISION.
LINKAGE SECTION.
01 ROUTINE PIC X(4).
01 USERTOKEN PIC S9(9) COMP.
01 FC PIC X(12).
PROCEDURE DIVISION USING ROUTINE USERTOKEN FC.
 CALL 'CEEHDLR' USING ROUTINE USERTOKEN FC
 GOBACK.
END PROGRAM REGISTER.
IDENTIFICATION DIVISION.
PROGRAM-ID. CBLHAND.
DATA DIVISION.
LINKAGE SECTION.
01 CONDTOK PIC X(12).
01 USERTOKEN PIC S9(9) COMP.
01 ACTIONCODE PIC S9(9) COMP.
01 NEWCOND PIC X(12).
PROCEDURE DIVISION USING CONDTOK USERTOKEN\n ACTIONCODE NEWCOND.
 IF USERTOKEN = 731 MOVE 10 TO ACTIONCODE\n ELSE MOVE 20 TO ACTIONCODE END-IF
 GOBACK.
END PROGRAM CBLHAND.
";
    let text: String = text
        .lines()
        .map(|line| format!("       {line}\n"))
        .collect();
    let path = dir.path().join("condition.cbl");
    std::fs::write(&path, text).unwrap();
    let module = zeus_cobol::compile_file(&path, &Default::default()).unwrap();
    let mut handler = module.clone();
    handler.programs.retain(|p| p.name == "CBLHAND");
    zeus_cobol::save_module(&handler, &dir.path().join("CBLHAND.zo")).unwrap();
    assert_eq!(
        Machine::new(vec![dir.path().to_path_buf()])
            .run_main(module)
            .unwrap(),
        0
    );
}

#[test]
fn promoted_conditions_restart_at_handle_or_resume_frame_and_equivalent_conditions_percolate() {
    for action in [32, 33] {
        let source = format!(
            "MAIN CSECT\n USING MAIN,15\n LR 12,14\n LINK EP=CEEHDLR,PARAM=(H,T,FC)\n LARL 15,MAIN\n LINK EP=CEESGL,PARAM=(COND,Q,FC)\n LARL 15,MAIN\n CLC FC(12),=XL12'0'\n BNE BAD\n XR 15,15\n BR 12\nBAD LA 15,99\n BR 12\nH DC A(HAND)\nT DC F'0'\nQ DC F'0'\nFC DS 3F\nCOND DC X'0003006358E9E9E900000000'\n DROP 15\nHAND DS 0H\n USING HAND,15\n L 2,0(1)\n L 3,8(1)\n CLI 3(2),99\n BNE RESUME\n L 4,12(1)\n MVC 0(12,4),NEXTCOND\n LHI 5,{action}\n ST 5,0(3)\n BR 14\nRESUME LHI 5,10\n ST 5,0(3)\n BR 14\nNEXTCOND DC X'0003006458E9E9E900000000'\n END MAIN\n"
        );
        assert_eq!(
            Machine::new(vec![])
                .run_hlasm_main(compile(&source))
                .unwrap(),
            0,
            "{action}"
        );
    }
    let source = "MAIN CSECT\n USING MAIN,15\n LR 12,14\n LINK EP=CEEHDLR,PARAM=(H,T,FC)\n LARL 15,MAIN\n LINK EP=CEESGL,PARAM=(COND,Q,FC)\n LARL 15,MAIN\n CLC FC(8),COND\n BNE BAD\n XR 15,15\n BR 12\nBAD LA 15,99\n BR 12\nH DC A(HAND)\nT DC F'0'\nQ DC F'0'\nFC DS 3F\nCOND DC X'0003006358E9E9E900000000'\n DROP 15\nHAND DS 0H\n L 2,0(1)\n L 3,8(1)\n L 4,12(1)\n MVC 0(12,4),0(2)\n LHI 5,32\n ST 5,0(3)\n BR 14\n END MAIN\n";
    assert_eq!(
        Machine::new(vec![])
            .run_hlasm_main(compile(source))
            .unwrap(),
        0
    );
}

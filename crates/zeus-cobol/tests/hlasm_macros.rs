use zeus_cobol::hlasm;

fn compile(source: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("macro.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[])
}

fn run(source: &str) -> i32 {
    let m = compile(source).unwrap();
    let mut mem = vec![0; 128];
    mem.extend(m.storage_at(128).unwrap());
    hlasm::execute(&m, &mut mem, 128, &[]).unwrap()
}

#[test]
fn representation_conversion_matrix_preserves_bits_width_and_signed_decimal() {
    for (expression, expected) in [
        ("B2X('1')", "1"),
        ("B2X('000000001')", "001"),
        ("B2C('11110001')", "1"),
        ("X2C('F1F2')", "12"),
        ("X2B('a1')", "10100001"),
        ("C2X('AB')", "C1C2"),
        ("C2B('1')", "11110001"),
        ("D2X('-1')", "FFFFFFFF"),
        ("D2B('1')", "00000000000000000000000000000001"),
        ("C2X(D2C('-2147483648'))", "80000000"),
        ("B2D('11111111111111111111111111111111')", "-1"),
        ("C2D('A')", "+193"),
        ("X2D('80000000')", "-2147483648"),
        ("B2D('')", "+0"),
        ("D2B('')", ""),
        ("X2C('')", ""),
        ("C2X(B2C('1'))", "01"),
        ("C2X(X2C('123'))", "0123"),
    ] {
        let source = format!(
            "&C SETC {expression}\n         AIF ('&C' EQ '{expected}').OK\n         MNOTE 8,'conversion failed'\n.OK ANOP\nTEST CSECT\n         LA 15,0\n         BR 14\n         END\n"
        );
        assert_eq!(run(&source), 0, "{expression}");
    }
    for expression in [
        "B2X('2')",
        "X2C(' G')",
        "X2D('100000000')",
        "D2X('2147483648')",
        "D2C('')",
        "C2D('ABCDE')",
    ] {
        assert!(
            compile(&format!("&C SETC {expression}\nTEST CSECT\n END\n")).is_err(),
            "{expression}"
        );
    }
}

#[test]
fn logical_expression_invocation_forms_work_with_strings_search_and_substrings() {
    assert_eq!(
        run(
            "&C SETC (UPPER 'aBc').(LOWER 'DeF')\n&B SETC (BYTE 193)\n&S SETC (SIGNED -27)\n&D SETC (DOUBLE 'a''b')\n&N SETA ('abcdef' INDEX 'cd')+('abc' FIND 'xzbc')\n AIF ('&C' EQ 'ABCdef' AND '&B' EQ 'A').ONE\n MNOTE 8,'logical character form'\n.ONE AIF ('&S' EQ '-27' AND K'&D EQ 4 AND &N EQ 5).OK\n MNOTE 8,'logical conversion or search form'\n.OK ANOP\nTEST CSECT\n LA 15,&N\n BR 14\n END\n"
        ),
        5
    );
}

#[test]
fn boolean_xor_nonzero_terms_and_character_comparison_follow_macro_rules() {
    assert_eq!(
        run(
            "&B SETB (1 OR 0 XOR 1)\n&A SETA (1 OR 0 XOR 1)\n&C SETC UPPER('ab')LOWER('CD')\n AIF (NOT &B AND &A EQ 0 AND ('ZZ' LT 'AAA')).ONE\n MNOTE 8,'operator priority or character relation'\n.ONE AIF ((27) AND (1 XOR NOT 1) AND '&C' EQ 'ABcd').OK\n MNOTE 8,'arithmetic boolean or juxtaposition'\n.OK ANOP\nTEST CSECT\n LA 15,1\n BR 14\n END\n"
        ),
        1
    );
}

#[test]
fn set_arrays_support_lists_omissions_growth_and_arithmetic_subscripts() {
    assert_eq!(
        run("         LCLA &A(1),&I
         LCLB &B(2)
         LCLC &C(3)
&A(1)    SETA 10,20,30
&A(2)    SETA ,40
&I       SETA 1
&B(1)    SETB (1 EQ 1),(0)
&C(1)    SETC 'one','two','three'
&C(&I+1) SETC 'xy'
         AIF (&B(1) AND NOT &B(2)).GOOD
         AGO .BAD
.GOOD    AIF (N'&A EQ 3 AND K'&C(2) EQ 2).PASS
.BAD     MNOTE 8,'array check failed'
.PASS    ANOP
TEST CSECT
         USING *,15
         CLC TEXT(2),=C'xy'
         BNE FAIL
         LA 15,&A(2)+&A(3)
         BR 14
FAIL     LA 15,99
         BR 14
TEXT     DC C'&C(2)'
         END
"),
        60
    );
}

#[test]
fn global_arrays_survive_nested_calls_while_locals_reset() {
    assert_eq!(
        run("         MACRO
         BUMP &R
         GBLA &A(1)
         LCLA &L(2)
&A(2)    SETA &A(1)+1
&A(1)    SETA &A(2)
&L(2)    SETA &L(2)+1
         LA &R,&A(1)+&L(2)
         MEND
TEST CSECT
         BUMP 2
         BUMP 3
         AR 2,3
         LR 15,2
         BR 14
         END
"),
        5
    );
}

#[test]
fn syslist_nested_sublists_label_parameter_and_attributes() {
    assert_eq!(
        run("         MACRO
&LABEL   PICK &LIST
         AIF (N'&LIST EQ 2 AND N'&SYSLIST EQ 2).CHECK
         MNOTE 8,'wrong argument count'
.CHECK   AIF ('&SYSLIST(0)' EQ 'LAB').GOOD
         MNOTE 8,'missing label'
.GOOD    ANOP
&A       SETA &LIST(2,1)+&SYSLIST(2,2)
&LABEL   LA 15,&A
         MEND
TEST CSECT
LAB      PICK (3,(4,5)),(6,7)
         BR 14
         END
"),
        11
    );
    assert_eq!(
        run("         MACRO
         VARARGS &KEY=0
&I       SETA N'&SYSLIST
         LA 15,&SYSLIST(&I)
         MEND
TEST CSECT
         VARARGS 1,,4,KEY=7
         BR 14
         END
"),
        4
    );
}

#[test]
fn macro_nesting_stack_and_next_generated_statement() {
    assert_eq!(
        run("         MACRO
         INNER
         AIF (&SYSNEST EQ 2).NAMES
         MNOTE 8,'wrong nesting level'
.NAMES   AIF ('&SYSMAC' EQ 'INNER' AND '&SYSMAC(1)' EQ 'OUTER').OPEN
         MNOTE 8,'wrong macro names'
.OPEN    AIF ('&SYSMAC(&SYSNEST)' EQ 'OPEN CODE').GOOD
         MNOTE 8,'missing open code'
.GOOD    LA 15,42
         MEND
         MACRO
         OUTER
         INNER
         MEND
TEST CSECT
         OUTER
         BR 14
         END
"),
        42
    );
    assert_eq!(
        run(
            "TEST CSECT\n         USING *,15\n&NEXT SETA &SYSSTMT\n         LA 15,&NEXT\n         BR 14\n         END\n"
        ),
        3
    );
}

#[test]
fn character_expressions_substring_concat_duplication_and_escaped_values() {
    assert_eq!(
        run("         LCLC &S,&Q,&AMP
&S       SETC 'abcdef'(2,3).(2)'XYZ'(2,*)
&Q       SETC 'O''Neil'
&AMP     SETC '&&'
         AIF ('&S' EQ 'bcdYZYZ' AND '&S'(2,2) EQ 'cd').GOOD
         MNOTE 8,'incorrect string expression'
.GOOD    ANOP
TEST CSECT
         USING *,15
         CLC QUOTE(6),=C'O''Neil'
         BNE FAIL
         CLC AMP(1),=X'50'
         BNE FAIL
         LA 15,K'&S
         BR 14
FAIL     LA 15,99
         BR 14
QUOTE DC C'&Q'
AMP DC C'&AMP'
         END
"),
        7
    );
    assert_eq!(
        run(
            "&NEG SETA -12\n&S SETC '&NEG'\n         AIF ('&S' EQ '12').OK\n         MNOTE 8,'signed character substitution'\n.OK ANOP\nTEST CSECT\n         LA 15,1\n         BR 14\n         END\n"
        ),
        1
    );
}

#[test]
fn created_set_symbols_support_indirection_and_distinct_parameter_names() {
    assert_eq!(
        run("&KEY SETC 'COUNT'
&(&KEY) SETA 5
&COUNT SETA &COUNT+1
&(&KEY.S)(2) SETC 'abc'
         AIF ('&(&KEY.S)(2)' EQ 'abc').OK
         MNOTE 8,'created array'
.OK ANOP
TEST CSECT
         LA 15,&COUNT
         BR 14
         END
"),
        6
    );
    assert_eq!(
        run(
            "         MACRO\n         SHADOW &P\n&(P) SETA 7\n         LA 15,&P+&(P)\n         MEND\nTEST CSECT\n         SHADOW 99\n         BR 14\n         END\n"
        ),
        106
    );
}

#[test]
fn invalid_macro_operations_report_errors_instead_of_panicking_or_truncating() {
    for (source, error) in [
        ("         LCLA &A(2)\n&A(0) SETA 1", "assignment target"),
        ("&A SETA 2147483648", "32-bit"),
        ("&A(1000001) SETA 1", "1,000,000"),
        ("&C SETC 'abc'(0,2)", "start must be positive"),
        ("&C SETC (5000)'X'", "duplication factor"),
        ("&C SETC 'A'+'B'", "concatenation"),
        ("         LCLA &SYSNEST", "read-only"),
        ("&N SETA &SYSNEST(1)", "scalar system symbol"),
        ("&A SETA 1\n&A SETC 'X'", "symbol type"),
    ] {
        let source =
            format!("{source}\nTEST CSECT\n         LA 15,0\n         BR 14\n         END\n");
        let message = compile(&source).unwrap_err();
        assert!(message.contains(error), "expected {error}: {message}");
    }
}

#[test]
fn representation_search_and_character_functions_work_in_nested_expressions() {
    assert_eq!(
        run("&A SETA X2A('FFFFFFFF')+2
&B SETA B2A('11111111111111111111111111111110')
&C SETA C2A('A')
&D SETA D2A('-12')
&S SETC UPPER('ab').LOWER('CD').A2X(-1)
&T SETC A2D(5).SIGNED(-2)
&E SETA INDEX('&S','cd')
&F SETA FIND('abcde','xd')
&ROUND SETC A2C(C2A('ABCD'))
&BYTE SETC BYTE(193)
&DUP SETC DOUBLE('a&&b''c')
         AIF ('&S' eq 'ABcdFFFFFFFF').TEXT
         MNOTE 8,'case/conversion functions'
.TEXT AIF ('&T' EQ '+5-2' AND '&ROUND' EQ 'ABCD').ROUND
         MNOTE 8,'round trip conversion'
.ROUND AIF ('&BYTE' EQ 'A' AND K'&DUP EQ 7).VALID
         MNOTE 8,'byte/double functions'
.VALID AIF (ISBIN('101') AND ISDEC('12')).HEX
         MNOTE 8,'validity functions'
.HEX AIF (ISHEX('AFF') AND ISSYM('A1')).PASS
         MNOTE 8,'validity functions'
.PASS ANOP
TEST CSECT
         LA 15,&A+&B+&C+&D+&E+&F
         BR 14
         END
"),
        187
    );
    for (source, message) in [
        ("&N SETA X2A('ZZ')", "digits"),
        (
            "&N SETA B2A('111111111111111111111111111111111')",
            "32 bits",
        ),
        ("&N SETA ISBIN('')", "nonempty"),
        ("&N SETA C2A('ABCDE')", "four bytes"),
        (
            "&C SETC UNKNOWN('abc')",
            "unsupported character macro function",
        ),
    ] {
        let source = format!("{source}\nTEST CSECT\n         BR 14\n         END\n");
        assert!(compile(&source).unwrap_err().contains(message));
    }
}

#[test]
fn computed_ago_extended_aif_and_actr_control_expansion() {
    assert_eq!(
        run("&INDEX SETA 2
         ACTR 4
         AGO (&INDEX).FIRST,.SECOND,.THIRD
.FIRST MNOTE 8,'wrong computed target'
.THIRD MNOTE 8,'wrong computed target'
.SECOND AIF (0).BAD,(1).GOOD
.BAD MNOTE 8,'wrong extended condition'
.GOOD AGO (0).BAD
TEST CSECT
         LA 15,42
         BR 14
         END
"),
        42
    );
    let message = compile("         ACTR 2\n.LOOP ANOP\n         AGO .LOOP\nTEST CSECT\n         BR 14\n         END\n").unwrap_err();
    assert!(message.contains("ACTR conditional branch count exhausted"));
    assert!(
        compile("         MEXIT\nTEST CSECT\n         BR 14\n         END\n")
            .unwrap_err()
            .contains("outside a macro")
    );
}

#[test]
fn arithmetic_bit_functions_and_self_defining_terms_have_32_bit_semantics() {
    assert_eq!(
        run("&A SETA (10 AND 3)
&B SETA (1 OR 8)
&C SETA (15 XOR 3)
&D SETA (1 SLL 5)
&E SETA (X'FFFFFFFF' SRL 31)
&F SETA (-2 SRA 63)
&G SETA (NOT 10)
&H SETA X'FFFFFFFF'+1
         AIF (&H EQ 0).GOOD
         MNOTE 8,'unsigned instead of signed self-defining term'
.GOOD ANOP
TEST CSECT
         LA 15,&A+&B+&C+&D+&E+&F+&G
         BR 14
         END
"),
        44
    );
}

#[test]
fn substitution_preserves_argument_boundaries_and_end_stops_expansion() {
    assert_eq!(
        run("         MACRO
         ONE &P
         AIF (N'&SYSLIST EQ 1 AND K'&P EQ 3).GOOD
         MNOTE 8,'substituted text changed argument boundaries'
.GOOD LA 15,42
         MEND
&TEXT SETC 'A,B'
TEST CSECT
         ONE &TEXT
         BR 14
         END
         MNOTE 8,'must not process statements after END'
"),
        42
    );
    for (prototype, message) in [
        ("BAD &P,&P", "duplicate macro parameter"),
        ("BAD &SYSNEST", "reserved macro parameter"),
    ] {
        let source = format!(
            "         MACRO\n         {prototype}\n         MEND\nTEST CSECT\n         BR 14\n         END\n"
        );
        assert!(compile(&source).unwrap_err().contains(message));
    }
    let source = "         MACRO\n         BAD &K=0\n         MEND\nTEST CSECT\n         BAD K=1,K=2\n         BR 14\n         END\n";
    assert!(compile(source).unwrap_err().contains("duplicate keyword"));
    assert_eq!(
        run(
            "         MACRO\n         ONE &P\n         AIF ('&P' EQ 'K=1').PASS\n         MNOTE 8,'value became keyword'\n.PASS LA 15,7\n         MEND\n&TEXT SETC 'K=1'\nTEST CSECT\n         ONE &TEXT\n         BR 14\n         END\n"
        ),
        7
    );
}

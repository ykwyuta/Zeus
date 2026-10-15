use zeus_cobol::hlasm;

fn execute(body: &str, data: &str) -> Result<i32, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test.asm");
    std::fs::write(&path,format!("TEST CSECT\n         USING *,15\n         LR 12,15\n         USING TEST,12\n         DROP 15\n{body}\n         BR 14\n{data}\n         END\n")).unwrap();
    let module = hlasm::compile_file(&path, &[])?;
    // Relocated load origin makes accidental absolute addresses fail.
    let base = 128;
    let mut mem = vec![0; base];
    mem.extend(&module.storage);
    hlasm::execute(&module, &mut mem, base, &[])
}

#[test]
fn arithmetic_logical_halfword_and_byte_instructions() {
    let cases = [
        (
            "         L 2,ONE\n         A 2,TWO\n         S 2,ONE\n         LR 15,2",
            2,
        ),
        (
            "         LH 2,NEG\n         AH 2,POS\n         SH 2,NEG\n         LR 15,2",
            9,
        ),
        (
            "         L 2,ONE\n         IC 2,BYTE\n         STC 2,DEST\n         XR 2,2\n         IC 2,DEST\n         LR 15,2",
            171,
        ),
        (
            "         L 2,BITS\n         N 2,MASK\n         O 2,ONE\n         X 2,TWO\n         LR 15,2",
            3,
        ),
        (
            "         L 2,ONE\n         SLL 2,5\n         SRL 2,2\n         LR 15,2",
            8,
        ),
        ("         LH 2,NEG\n         SRA 2,63\n         LR 15,2", -1),
        (
            "         LH 2,NEG\n         LPR 2,2\n         LCR 2,2\n         LNR 2,2\n         LR 15,2",
            -5,
        ),
        (
            "         L 3,VALUE\n         M 2,TWO\n         D 2,THREE\n         LR 15,3",
            8,
        ),
        (
            "         L 3,VALUE\n         MH 3,POS\n         LR 15,3",
            108,
        ),
        (
            "         LH 2,NEG\n         STH 2,HALF\n         LH 15,HALF",
            -5,
        ),
    ];
    let data = "ONE DC F'1'\nTWO DC F'2'\nTHREE DC F'3'\nVALUE DC F'12'\nBITS DC X'FF00'\nMASK DC X'0000FFFF'\nNEG DC H'-5'\nPOS DC H'9'\nBYTE DC X'AB'\nDEST DS X\nHALF DS H";
    for (body, expected) in cases {
        assert_eq!(execute(body, data).unwrap(), expected, "{body}");
    }
}

#[test]
fn condition_codes_masks_and_link_register_branches() {
    let cases = [
        "         L 2,MAX\n         AL 2,ONE\n         BC 2,OK", // carry and zero -> 2
        "         L 2,ZERO\n         SL 2,ONE\n         BC 4,OK", // borrow and nonzero -> 1
        "         L 2,MAX\n         CL 2,ONE\n         BH OK",
        "         L 2,MAX\n         C 2,ONE\n         BL OK",
        "         L 2,POSMAX\n         A 2,ONE\n         BO OK",
        "         TM BYTE,X'F0'\n         BC 1,OK",
        "         NI BYTE,X'00'\n         BZ OK",
    ];
    for body in cases {
        assert_eq!(execute(&format!("{body}\n         LA 15,99\n         BR 14\nOK LA 15,7"),"MAX DC X'FFFFFFFF'\nPOSMAX DC X'7FFFFFFF'\nONE DC F'1'\nZERO DC F'0'\nBYTE DC X'F1'").unwrap(),7,"{body}");
    }
    assert_eq!(execute("         LR 10,14\n         BAS 14,SUB\n         LR 14,10\n         B FINISH\nSUB LA 15,42\n         BR 14\nFINISH LR 15,15","").unwrap(),42);
}

#[test]
fn packed_arithmetic_conversion_and_zoned_data() {
    let data = "A DC PL8'123'\nB DC PL3'-23'\nC DC PL2'5'\nOUT DS PL8\nBIN DS D\nZONE DC Z'12345'\nZOUT DS ZL5\nPBUF DS PL3";
    let data = data.replace("BIN DS D", "BIN DS PL8");
    let cases = [
        (
            "         ZAP OUT,A\n         AP OUT,B\n         CVB 15,OUT",
            100,
        ),
        (
            "         ZAP OUT,A\n         SP OUT,B\n         CVB 15,OUT",
            146,
        ),
        (
            "         ZAP OUT,A\n         MP OUT,C\n         CVB 15,OUT",
            615,
        ),
        (
            "         ZAP OUT,A\n         DP OUT,C\n         ZAP BIN,OUT(6)\n         CVB 15,BIN",
            24,
        ),
        (
            "         LA 2,456\n         CVD 2,OUT\n         CVB 15,OUT",
            456,
        ),
        (
            "         PACK PBUF,ZONE\n         UNPK ZOUT,PBUF\n         PACK OUT,ZOUT\n         CVB 15,OUT",
            12345,
        ),
        (
            "         CP A,B\n         BH OK\n         LA 15,99\n         BR 14\nOK LA 15,8",
            8,
        ),
    ];
    for (body, n) in cases {
        assert_eq!(execute(body, &data).unwrap(), n, "{body}");
    }
    assert!(
        execute(
            "         ZAP OUT,A\n         DP OUT,ZERO",
            "A DC PL8'12'\nOUT DS PL8\nZERO DC P'0'"
        )
        .unwrap_err()
        .contains("divide")
    );
}

#[test]
fn expressions_constants_attributes_and_multiple_using_registers() {
    let data = "R EQU NEXT+1\nNEXT EQU 1\nBEGIN DC F'10,20,30'\nAFTER EQU *\nSIZE EQU AFTER-BEGIN\nBYTE DC BL1'10101010'\nSMALL DC FL1'255'\nPACKED DC P'12345'\nZONE DC Z'-456'";
    assert_eq!(
        execute(
            "         LA R,(SIZE*2+X'10')/2\n         AHI R,L'BEGIN\n         LR 15,R",
            data
        )
        .unwrap(),
        24
    );
    assert_eq!(
        execute(
            "         XR 2,2\n         IC 2,SMALL\n         LR 15,2",
            data
        )
        .unwrap(),
        255
    );
    assert_eq!(
        execute(
            "         XR 2,2\n         IC 2,BYTE\n         LR 15,2",
            data
        )
        .unwrap(),
        170
    );
    let body = "         LA 11,4095(12)\n         LA 11,1(11)\n         USING TEST,12,11\n         L 15,FAR";
    assert_eq!(execute(body, "PAD DS 4200C\nFAR DC F'67'").unwrap(), 67);
    assert!(
        execute("         LA 15,1/0", "")
            .unwrap_err()
            .contains("division by zero")
    );
}

#[test]
fn conditional_macros_keywords_defaults_labels_and_unique_symbols() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("macro.asm");
    let source = "         MACRO
&NAME    INCR &R,&COUNT=3,&SKIP=NO
         LCLA &I
&I       SETA 0
         AIF ('&SKIP' EQ 'YES').DONE
&NAME    LA &R,0
.LOOP    ANOP
&I       SETA &I+1
         AHI &R,2
         AIF (&I LT &COUNT).LOOP
         AGO .EXIT
.DONE    ANOP
         LA &R,9
.EXIT    MEXIT
         MEND
TEST CSECT
         USING *,15
START    INCR 2,COUNT=4
         LR 3,2
         INCR 2,SKIP=YES
         AR 3,2
         LR 15,3
         BR 14
         END
";
    std::fs::write(&path, source).unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    let mut mem = m.storage.clone();
    assert_eq!(hlasm::execute(&m, &mut mem, 0, &[]).unwrap(), 17);
}

#[test]
fn continuation_records_combine_operand_lists() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("continued.asm");
    let first = format!("{:<71}X", "VALUES DC F'1',");
    std::fs::write(&path,format!("TEST CSECT\n         USING *,15\n         L 15,VALUES+4\n         BR 14\n{first}\n               F'42'\n         END\n")).unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    let mut mem = m.storage.clone();
    assert_eq!(hlasm::execute(&m, &mut mem, 0, &[]).unwrap(), 42);
}

#[test]
fn control_sections_address_constants_entries_and_relocation_survive_calls() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ROOT.asm");
    let source = "ROOT CSECT
         USING *,15
         L 2,PTR
         L 15,0(2)
         BR 14
PTR      DC A(DATA)
SECOND   CSECT
         USING *,15
         L 2,=A(DATA)
         L 15,0(2)
         BR 14
         ENTRY SECOND
         LTORG
DATA     DC F'73'
         END ROOT
";
    std::fs::write(&path, source).unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    let origin = 256;
    let mut mem = vec![0; origin];
    mem.extend(m.storage_at(origin).unwrap());
    assert_eq!(hlasm::execute(&m, &mut mem, origin, &[]).unwrap(), 73);
    let mut second = m.clone();
    second.entry = m.entries["SECOND"];
    assert_eq!(hlasm::execute(&second, &mut mem, origin, &[]).unwrap(), 73);
    hlasm::save_module(&m, &dir.path().join("ROOT.zo")).unwrap();
    let mut machine = zeus_cobol::vm::Machine::new(vec![dir.path().to_path_buf()]);
    machine.getmain(128, 0);
    assert_eq!(machine.call_hlasm("ROOT", &[]).unwrap().unwrap(), 73);
    let size = machine.storage().len();
    assert_eq!(machine.call_hlasm("SECOND", &[]).unwrap().unwrap(), 73);
    assert_eq!(machine.storage().len(), size);
    let source = dir.path().join("cancel.cbl");
    std::fs::write(&source,"       IDENTIFICATION DIVISION.\n       PROGRAM-ID. CANCELTEST.\n       PROCEDURE DIVISION.\n           CALL 'ROOT'\n           CANCEL 'ROOT'\n           CALL 'ROOT'\n           IF RETURN-CODE = 73\n             MOVE 0 TO RETURN-CODE\n           ELSE MOVE 99 TO RETURN-CODE END-IF\n           STOP RUN.\n").unwrap();
    let main = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
    assert_eq!(machine.run_main(main).unwrap(), 0);
}

#[test]
fn macro_global_state_local_scopes_and_logical_precedence() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("global.asm");
    let source = "         MACRO
         NUMBER &R
         GBLA &SEQ
         LCLA &LOCAL
&SEQ     SETA &SEQ+1
&LOCAL   SETA &LOCAL+1
         LA &R,&SEQ
         AHI &R,&LOCAL
         MEND
TEST CSECT
         USING *,15
         NUMBER 2
         NUMBER 3
         AR 2,3
         AIF (0 AND 1 OR 1).GOOD
         LA 15,99
         BR 14
.GOOD    ANOP
         LR 15,2
         BR 14
         END
";
    std::fs::write(&path, source).unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    let mut mem = m.storage.clone();
    assert_eq!(hlasm::execute(&m, &mut mem, 0, &[]).unwrap(), 5);
}

#[test]
fn unique_macro_labels_attributes_with_comments_and_expansion_limits() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("unique.asm");
    let source = "         MACRO
         ONCE &R
LABEL&SYSNDX LA &R,3
         MEND
TEST CSECT
         USING *,15
         ONCE 2
         ONCE 3
         AR 2,3
         AHI 2,L'FIELD       attribute followed by a comment
         LR 15,2
         BR 14
FIELD    DS CL7
         END
";
    std::fs::write(&path, source).unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    let mut mem = m.storage.clone();
    assert_eq!(hlasm::execute(&m, &mut mem, 0, &[]).unwrap(), 13);
    std::fs::write(
        &path,
        "TEST CSECT\n.LOOP ANOP\n         AGO .LOOP\n         END\n",
    )
    .unwrap();
    assert!(
        hlasm::compile_file(&path, &[])
            .unwrap_err()
            .contains("conditional assembly limit")
    );
}

#[test]
fn decimal_overflow_divide_exceptions_and_invalid_digits() {
    assert_eq!(
        execute(
            "         AP A,B\n         BO OK\n         LA 15,99\n         BR 14\nOK LA 15,4",
            "A DC PL1'9'\nB DC PL1'1'"
        )
        .unwrap(),
        4
    );
    assert!(
        execute("         CVB 15,BAD", "BAD DC X'00000000000000AC'")
            .unwrap_err()
            .contains("invalid packed decimal digit")
    );
    assert!(
        execute(
            "         L 2,HIGH\n         L 3,LOW\n         D 2,DIVISOR",
            "HIGH DC F'1'\nLOW DC F'0'\nDIVISOR DC F'1'"
        )
        .unwrap_err()
        .contains("divide exception")
    );
    assert!(
        execute(
            "         L 3,LOW\n         D 2,DIVISOR",
            "LOW DC F'123'\nDIVISOR DC F'0'"
        )
        .unwrap_err()
        .contains("divide exception")
    );
}

#[test]
fn ltorg_places_separate_pools_and_asterisk_literals_capture_use_location() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pools.asm");
    std::fs::write(&path,"TEST CSECT\n         USING *,15\n         L 2,=F'17'\n         L 3,=F'17'\n         B NEXT\nPOOL1 LTORG\nNEXT L 4,=F'17'\nHERE L 5,=A(*)\n         LA 6,HERE\n         CR 5,6\n         BNE FAIL\n         AR 2,3\n         AR 2,4\n         LR 15,2\n         BR 14\nFAIL LA 15,99\n         BR 14\nPOOL2 LTORG\n         END\n").unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    let first = &m.code[0].address.as_ref().unwrap();
    let duplicate = &m.code[1].address.as_ref().unwrap();
    let second = &m.code[3].address.as_ref().unwrap();
    assert_eq!(first.displacement, duplicate.displacement);
    assert_ne!(first.displacement, second.displacement);
    let mut mem = vec![0; 128];
    mem.extend(m.storage_at(128).unwrap());
    assert_eq!(hlasm::execute(&m, &mut mem, 128, &[]).unwrap(), 51);
    std::fs::write(&path,"TEST CSECT\n         USING *,15\n         L 2,=F'17'\n         LTORG\n         BR 14\n         END\n").unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    let mut mem = m.storage_at(0).unwrap();
    assert!(
        hlasm::execute(&m, &mut mem, 0, &[])
            .unwrap_err()
            .contains("entered data")
    );
}

#[test]
fn org_redefines_data_preserves_skipped_bytes_and_restores_high_water() {
    let body = "         LA 2,SOURCE\n         TR SOURCE(3),TABLE\n         CLC SOURCE(3),EXPECT\n         BNE FAIL\n         LA 15,0\n         BR 14\nFAIL LA 15,99";
    let data = "TABLE DC 256X'00'\n         ORG TABLE+C'A'\n         DC X'010203'\n         ORG TABLE+C'Z'\n         DC X'FF'\n         ORG TABLE+C'A'\nALIAS DS 3X\n         ORG\nSOURCE DC C'ABC'\nEXPECT DC X'010203'";
    assert_eq!(execute(body, data).unwrap(), 0);
    assert_eq!(
        execute(
            "         LA 15,AFTER-TABLE",
            "TABLE DS 32X\n         ORG TABLE+1\n         DC X'FF'\n         ORG ,8,-2\nAFTER EQU *"
        )
        .unwrap(),
        30
    );
    assert!(
        execute("         LA 15,0", "         ORG TEST-1")
            .unwrap_err()
            .contains("outside current section")
    );
    assert!(
        execute("         LA 15,0", "         ORG *,3")
            .unwrap_err()
            .contains("power of two")
    );
}

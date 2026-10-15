use zeus_cobol::hlasm;

fn compile(source: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("conditional.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[])
}

fn run(source: &str) -> i32 {
    let module = compile(source).unwrap();
    let mut storage = vec![0; 128];
    storage.extend(module.storage_at(128).unwrap());
    hlasm::execute(&module, &mut storage, 128, &[]).unwrap()
}

#[test]
fn conditional_attributes_read_generated_storage_instruction_and_equ_definitions() {
    assert_eq!(
        run(
            "MAIN CSECT\n         J RUN\nFIELD DC CL7'ABCDEFG'\nTWO LR 2,2\nFOUR LGHI 3,0\nSIX LARL 4,FIELD\nVIEW EQU FIELD+1,3,C'C'\n&LEN SETA L'VIEW+L'FIELD\n&TYPE SETC T'VIEW\n         AIF (L'TWO EQ 2 AND L'FOUR EQ 4 AND L'SIX EQ 6).CHECK\n         MNOTE 8,'instruction lengths wrong'\n.CHECK AIF ('&TYPE' EQ 'C' AND T'FIELD EQ 'C' AND D'VIEW).GOOD\n         MNOTE 8,'generated attributes wrong'\n.GOOD ANOP\nRUN LGHI 15,&LEN\n         BR 14\n         END\n"
        ),
        10
    );
}

#[test]
fn attributes_accept_macro_parameters_sublists_syslist_and_setc_array_elements() {
    assert_eq!(
        run(
            "         MACRO\n         CHECK &P\n&L SETA L'&P\n&T SETC T'&SYSLIST(1)\n         AIF (&L EQ 7 AND '&T' EQ 'C').GOOD\n         MNOTE 8,'parameter attributes wrong'\n.GOOD ANOP\n         MEND\nMAIN CSECT\n         J RUN\nFIELD DC CL7'ABCDEFG'\n         CHECK FIELD+2\n         CHECK (FIELD,OTHER)\n         LCLC &N(2)\n&N(1) SETC 'FIELD'\n&L SETA L'&N(1)\n&D SETA D'&N(1)\n&I SETA 3\n&B SETB (1)\n         AIF (T'&I EQ 'N' AND T'&B EQ 'N' AND &D).GOOD\n         MNOTE 8,'SET attributes wrong'\n.GOOD ANOP\nRUN LGHI 15,&L\n         BR 14\n         END\n"
        ),
        7
    );
}

#[test]
fn defined_attribute_guards_repeated_generation_without_exposing_skipped_or_later_labels() {
    assert_eq!(
        run(
            "         MACRO\n         EMIT\n         LGHI 2,0\n         MEND\nMAIN CSECT\nMARK EMIT\n         AIF (D'MARK AND L'MARK EQ 4 AND T'MARK EQ 'I').GOOD\n         MNOTE 8,'macro call label attributes missing'\n.GOOD ANOP\n         LGHI 15,4\n         BR 14\n         END\n"
        ),
        4
    );
    assert_eq!(
        run(
            "&I SETA 0\n&BEFORE SETA D'ONE\n.LOOP AIF (D'ONE).NEXT\nONE EQU 5\n.NEXT ANOP\n&I SETA &I+1\n         AIF (&I LT 2).LOOP\n         AGO .SKIP\nSKIPPED EQU 9\n.SKIP ANOP\n         AIF (D'ONE AND NOT D'SKIPPED AND NOT D'LATER).CHECK\n         MNOTE 8,'defined source order wrong'\n.CHECK AIF (&BEFORE EQ 0 AND &I EQ 2).GOOD\n         MNOTE 8,'defined guard failed'\n.GOOD ANOP\nMAIN CSECT\n         LGHI 15,19\n         BR 14\nLATER DC F'0'\n         END\n"
        ),
        19
    );
    compile("MAIN CSECT\n         BR 14\n         EXTRN STRONG\n         WXTRN WEAK\nFORWARD EQU LATER\n         AIF (D'STRONG AND D'WEAK AND D'FORWARD).GOOD\n         MNOTE 8,'external or forward EQU is not defined'\n.GOOD ANOP\nLATER DC F'0'\n         END\n").unwrap();
}

#[test]
fn conditional_character_types_do_not_change_ordinary_numeric_type_or_length_expressions() {
    let module = compile("MAIN CSECT\n         BR 14\nFIELD DC FL3'7'\nWEIRD EQU FIELD,1,C'&&'\n&TYPE SETC T'FIELD\n&W SETC T'WEIRD\n&TEXT SETC 'L''FIELD'\n         AIF ('&TYPE' EQ 'G' AND '&W' EQ '&&').CHECK\n         MNOTE 8,'type character escaping wrong'\n.CHECK AIF ('&TEXT' EQ 'L''FIELD').GOOD\n         MNOTE 8,'quoted attribute text changed'\n.GOOD ANOP\nRESULT DC A(T'FIELD,L'FIELD)\n         ENTRY RESULT\n         END\n").unwrap();
    let at = module.entries["RESULT"] as usize;
    assert_eq!(
        &module.storage[at..at + 4],
        &u32::from(zeus_cobol::ebcdic::encode("G")[0]).to_be_bytes()
    );
    assert_eq!(&module.storage[at + 4..at + 8], &3u32.to_be_bytes());
}

#[test]
fn copy_generated_attributes_are_visible_in_macro_bodies() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("fields.cpy"), "FIELD DC CL4'ABCD'\n").unwrap();
    let source = "         MACRO\n         CHECK &P\n         AIF (D'&P AND L'&P EQ 4 AND T'&P EQ 'C').GOOD\n         MNOTE 8,'COPY attributes unavailable'\n.GOOD ANOP\n         MEND\nMAIN CSECT\n         J RUN\n         COPY fields\n         CHECK FIELD\nRUN LGHI 15,24\n         BR 14\n         END\n";
    let path = dir.path().join("conditional.asm");
    std::fs::write(&path, source).unwrap();
    let module = hlasm::compile_file(&path, &[dir.path().to_path_buf()]).unwrap();
    let mut storage = vec![0; 128];
    storage.extend(module.storage_at(128).unwrap());
    assert_eq!(hlasm::execute(&module, &mut storage, 128, &[]).unwrap(), 24);
}

#[test]
fn unavailable_lengths_invalid_set_references_and_literal_attributes_are_diagnosed() {
    for (body, diagnostic) in [
        ("&L SETA L'MISSING", "not available after lookahead"),
        ("VALUE EQU 3\n&L SETA L'VALUE", "invalid for type U"),
        ("&I SETA 3\n&D SETA D'&I", "SETA or SETB"),
        (
            "&P SETC 'FIELD+1'\n&D SETA D'&P",
            "represent an ordinary symbol",
        ),
        (
            "&L SETA L'=E'1.2.3'",
            "floating-point nominal requires decimal digits",
        ),
        (
            "&P SETC '=E''1.2.3'''\n&T SETC T'&P",
            "floating-point nominal requires decimal digits",
        ),
        (
            "FIELD DC CL4'ABCD'\nVIEW EQU FIELD,L'FIELD,C'C'\n&T SETC T'VIEW",
            "attributes of VIEW are unavailable",
        ),
    ] {
        let error = compile(&format!(
            "MAIN CSECT\n         BR 14\n{body}\n         END\n"
        ))
        .unwrap_err();
        assert!(error.contains(diagnostic), "{body}: {error}");
        assert!(error.contains("conditional.asm:"), "{error}");
    }
}

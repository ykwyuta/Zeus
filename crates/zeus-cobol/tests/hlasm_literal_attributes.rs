use zeus_cobol::hlasm;

fn compile(source: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("literal-attrs.asm");
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
fn literal_attributes_define_metadata_while_defined_attribute_only_observes_it() {
    assert_eq!(
        run(
            "&BEFORE SETA D'=F'29'\n&AGAIN SETA D'=F'29'\n&L SETA L'=F'29'\n&AFTER SETA D'=F'29'\n&T SETC T'=FL3'29'\n         AIF (&BEFORE EQ 0 AND &AGAIN EQ 0 AND &AFTER EQ 1).CHECK\n         MNOTE 8,'literal defined state wrong'\n.CHECK AIF ('&T' EQ 'G' AND L'=3F'29' EQ 4).GOOD\n         MNOTE 8,'literal attributes wrong'\n.GOOD ANOP\nMAIN CSECT\n         USING MAIN,15\n         L 3,=F'29'\n         LGHI 15,&L+&AFTER\n         BR 14\n         END\n"
        ),
        5
    );
}

#[test]
fn attribute_only_literals_do_not_emit_storage_or_request_external_libraries() {
    let plain = compile("MAIN CSECT\n         LGHI 15,4\n         BR 14\n         END\n").unwrap();
    let attributed = compile("&LEN SETA L'=V(MISSING)\n&TYPE SETC T'=A(UNKNOWN+4)\n         AIF (D'=V(MISSING) AND '&TYPE' EQ 'A').GOOD\n         MNOTE 8,'address literal metadata wrong'\n.GOOD ANOP\nMAIN CSECT\n         LGHI 15,&LEN\n         BR 14\n         END\n").unwrap();
    assert_eq!(plain.storage, attributed.storage);
    assert!(attributed.externals.is_empty());
    assert!(attributed.relocations.is_empty());
}

#[test]
fn literal_attributes_accept_macro_parameters_setc_arrays_and_escaped_nominal_values() {
    assert_eq!(
        run(
            "         MACRO\n         CHECK &P\n&LEN SETA L'&P\n&TYPE SETC T'&SYSLIST(1)\n         AIF (&LEN EQ 7 AND '&TYPE' EQ 'C').GOOD\n         MNOTE 8,'parameter literal attributes wrong'\n.GOOD ANOP\n         MEND\n         CHECK =C'a''b,(*)'\n         LCLC &LIT(1)\n&LIT(1) SETC '=F''7'''\n&LEN SETA L'&LIT(1)\n&DEF SETA D'&LIT(1)\n&PAYLOAD SETC 'a''&&B'\n&CHARLEN SETA L'=C'&PAYLOAD'\n         AIF (&DEF EQ 1 AND &CHARLEN EQ 4).GOOD\n         MNOTE 8,'literal substitution or definition wrong'\n.GOOD ANOP\nMAIN CSECT\n         LGHI 15,&LEN+&CHARLEN\n         BR 14\n         END\n"
        ),
        8
    );
}

#[test]
fn ordinary_literal_attributes_are_numeric_and_do_not_allocate_literal_pools() {
    let module = compile("MAIN CSECT\n         BR 14\nWIDTH EQU L'=FL3'7'\nRESULT DC A(L'=F'1,2',T'=FL3'7',WIDTH,L'=A(MISSING))\n         AIF (D'=FL3'7' AND D'=F'1,2').GOOD\n         MNOTE 8,'ordinary literal attribute did not define metadata'\n.GOOD ANOP\n         ENTRY RESULT\n         END\n").unwrap();
    let at = module.entries["RESULT"] as usize;
    let values: Vec<_> = module.storage[at..at + 16]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_be_bytes(*b))
        .collect();
    assert_eq!(
        values,
        vec![4, u32::from(zeus_cobol::ebcdic::encode("G")[0]), 3, 4]
    );
    assert!(module.relocations.is_empty());
    assert!(module.externals.is_empty());
}

#[test]
fn location_literals_have_statement_scope_but_character_stars_and_products_do_not() {
    assert_eq!(
        run(
            "&L SETA L'=A(*)+D'=A(*)\n&NEXT SETA D'=A(*)\n&C SETA L'=C'*'\n&D SETA D'=C'*'\n&P SETA L'=A(2*3)\n&PD SETA D'=A(2*3)\n         AIF (&L EQ 5 AND &NEXT EQ 0 AND &D EQ 1 AND &PD EQ 1).GOOD\n         MNOTE 8,'literal location identity wrong'\n.GOOD ANOP\nMAIN CSECT\n         LGHI 15,&C+&P\n         BR 14\n         END\n"
        ),
        5
    );
    let module = compile("MAIN CSECT\n         USING MAIN,15\n         L 2,=C'*'\n         L 3,=C'*'\n         L 4,=A(*)\n         L 5,=A(*)\n         LGHI 15,0\n         BR 14\n         END\n").unwrap();
    assert_eq!(
        module.code[0].address.as_ref().unwrap().displacement,
        module.code[1].address.as_ref().unwrap().displacement
    );
    let storage = module.storage_at(128).unwrap();
    for index in [2, 3] {
        let at = module.code[index].address.as_ref().unwrap().displacement as usize;
        assert_eq!(
            u32::from_be_bytes(storage[at..at + 4].try_into().unwrap()),
            128 + module.code[index].offset
        );
    }
}

#[test]
fn literal_definition_follows_generated_machine_instructions_copy_and_conditional_branches() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("uses.cpy"), "         L 3,=F'11'\n").unwrap();
    let source = "&BEFORE SETA D'=F'11'\nMAIN CSECT\n         USING MAIN,15\n         COPY uses\n         AGO .SKIP\n         L 4,=F'99'\n.SKIP ANOP\n         AIF (&BEFORE EQ 0 AND D'=F'11' AND NOT D'=F'99').GOOD\n         MNOTE 8,'machine literal source order wrong'\n.GOOD ANOP\n         LTORG\n         AIF (D'=F'11' AND NOT D'=X'0B').DONE\n         MNOTE 8,'literal specifications incorrectly merged'\n.DONE ANOP\n         END\n";
    let path = dir.path().join("literal-attrs.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[dir.path().to_path_buf()]).unwrap();
}

#[test]
fn invalid_literal_attributes_report_source_errors_and_enforce_storage_limits() {
    for (body, diagnostic) in [
        ("&L SETA L'=C'unclosed", "unterminated literal"),
        ("&L SETA L'=A(1", "unclosed literal address"),
        (
            "&T SETC T'=E'1.2.3'",
            "floating-point nominal requires decimal digits",
        ),
        ("&L SETA L'=2000000F'1'", "exceeds 1 MiB"),
        (
            "&P SETC '=F''1''oops'\n&D SETA D'&P",
            "unexpected literal suffix",
        ),
    ] {
        let error = compile(&format!(
            "MAIN CSECT\n         BR 14\n{body}\n         END\n"
        ))
        .unwrap_err();
        assert!(error.contains(diagnostic), "{body}: {error}");
        assert!(error.contains("literal-attrs.asm:"), "{error}");
    }
}

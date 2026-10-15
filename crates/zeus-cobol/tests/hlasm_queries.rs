use zeus_cobol::hlasm;

fn compile(source: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("queries.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[])
}

fn run(source: &str) -> i32 {
    let module = compile(source).unwrap();
    let module = hlasm::load_module(&hlasm::module_bytes(&module).unwrap()).unwrap();
    let mut storage = vec![0; 128];
    storage.extend(module.storage_at(128).unwrap());
    let length = storage.len();
    let rc = hlasm::execute(&module, &mut storage, 128, &[]).unwrap();
    assert_eq!(storage.len(), length);
    rc
}

#[test]
fn queries_return_program_bytes_and_register_types_in_character_and_logical_expressions() {
    assert_eq!(
        run(
            "WORK EQU 3,,,C'Tag7',GR32\nCHOICE EQU 0,,,C'WORK'\n&TAG SETC SYSATTRP('work')\n&TYPE SETC SYSATTRA(UPPER('work'))\n&NEST SETC SYSATTRA(SYSATTRP('CHOICE'))\n         AIF ('&TAG' EQ 'Tag7' AND '&TYPE' EQ 'GR32').CHECK\n         MNOTE 8,'attribute values wrong'\n.CHECK AIF ('&NEST' EQ 'GR32').GOOD\n         MNOTE 8,'nested query wrong'\n.GOOD ANOP\nMAIN CSECT\n         LGHI 15,42\n         BR 14\n         END\n"
        ),
        42
    );
}

#[test]
fn queries_distinguish_null_from_four_zero_bytes_and_preserve_quoted_character_data() {
    assert_eq!(
        run(
            "ZERO EQU 0,,,0\nNONE EQU 0\nQUOTED EQU 0,,,C'a''&&B'\n&Z SETC SYSATTRP('ZERO')\n&N SETC SYSATTRP('NONE')\n&X SETC A2X(C2A(SYSATTRP('ZERO')))\n         AIF (K'&Z EQ 4 AND K'&N EQ 0).CHECK\n         MNOTE 8,'null and zero collapsed'\n.CHECK AIF ('&X' EQ '00000000').BYTES\n         MNOTE 8,'zero bytes changed'\n.BYTES AIF (C2A(SYSATTRP('QUOTED')) EQ C'a''&&B').GOOD\n         MNOTE 8,'quoted bytes changed'\n.GOOD ANOP\nMAIN CSECT\n         LGHI 15,17\n         BR 14\n         END\n"
        ),
        17
    );
}

#[test]
fn queries_follow_generated_source_order_alias_defaults_and_skipped_definitions() {
    assert_eq!(
        run(
            "&BEFORE SETC SYSATTRA('LATER')\nLATER EQU 2,,,C'Abcd',GR64\nALIAS EQU LATER\n         AGO .SKIP\nSKIPPED EQU 3,,,,FPR\n.SKIP ANOP\n&AFTER SETC SYSATTRA('LATER')\n&A SETC SYSATTRA('ALIAS')\n&P SETC SYSATTRP('ALIAS')\n&S SETC SYSATTRA('SKIPPED')\n&EMPTY SETC SYSATTRP('')\n         AIF (K'&BEFORE EQ 0 AND K'&A EQ 0).CHECK\n         MNOTE 8,'source order or alias wrong'\n.CHECK AIF (K'&P EQ 0 AND K'&S EQ 0 AND K'&EMPTY EQ 0).LAST\n         MNOTE 8,'absent attribute wrong'\n.LAST AIF ('&AFTER' EQ 'GR64').GOOD\n         MNOTE 8,'defined type unavailable'\n.GOOD ANOP\nMAIN CSECT\n         LGHI 15,23\n         BR 14\n         END\n"
        ),
        23
    );
}

#[test]
fn queries_use_constant_equ_dependencies_even_when_the_equ_value_is_forward() {
    assert_eq!(
        run(
            "COUNT EQU NEXT+1\nNEXT EQU 16\nFORWARD EQU LATER,,,COUNT+1,GR32\n&V SETA C2A(SYSATTRP('FORWARD'))\n&T SETC SYSATTRA('FORWARD')\n         AIF (&V EQ 18 AND '&T' EQ 'GR32').GOOD\n         MNOTE 8,'forward value hid known attributes'\n.GOOD ANOP\nMAIN CSECT\n         LGHI 15,&V\n         BR 14\nLATER DC F'0'\n         END\n"
        ),
        18
    );
}

#[test]
fn copy_macro_parameters_and_set_arrays_share_generated_attribute_definitions() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("defs.cpy"), "SOURCE EQU 3,,,C'Copy',GR32\n").unwrap();
    let source = "         COPY defs\n         MACRO\n         DEFINE &NAME,&KIND\n&NAME EQU 4,,,C'Made',&KIND\n         MEND\n         MACRO\n         CHECK &NAME\n         LCLC &A(2)\n&A(1) SETC SYSATTRA('&NAME'),SYSATTRP('&NAME')\n         AIF ('&A(1)' EQ 'GR64' AND '&A(2)' EQ 'Made').GOOD\n         MNOTE 8,'macro queries failed'\n.GOOD ANOP\n         MEND\n         DEFINE TARGET,GR64\n         CHECK TARGET\n         AIF (SYSATTRA('SOURCE') EQ 'GR32').GOOD\n         MNOTE 8,'COPY attributes missing'\n.GOOD ANOP\nMAIN CSECT\n         LGHI 15,31\n         BR 14\n         END\n";
    let path = dir.path().join("queries.asm");
    std::fs::write(&path, source).unwrap();
    let module = hlasm::compile_file(&path, &[dir.path().to_path_buf()]).unwrap();
    let mut storage = vec![0; 128];
    storage.extend(module.storage_at(128).unwrap());
    assert_eq!(hlasm::execute(&module, &mut storage, 128, &[]).unwrap(), 31);
}

#[test]
fn storage_assembler_types_include_extensions_and_query_text_inside_literals_stays_literal() {
    let module = compile("MAIN CSECT\n         BR 14\nSMALL DC F'1'\nBIG DS FD\nTEXT DC C'SYSATTRA(''SMALL'')'\n&A SETC SYSATTRA('SMALL')\n&B SETC SYSATTRA('BIG')\n&P SETC SYSATTRP('SMALL')\n         AIF ('&A' EQ 'F' AND '&B' EQ 'FD').CHECK\n         MNOTE 8,'storage types wrong'\n.CHECK AIF (K'&P EQ 0).GOOD\n         MNOTE 8,'unassigned storage program type wrong'\n.GOOD ANOP\n         ENTRY TEXT\n         END\n").unwrap();
    let at = module.entries["TEXT"] as usize;
    let expected = zeus_cobol::ebcdic::encode("SYSATTRA('SMALL')");
    assert_eq!(&module.storage[at..at + expected.len()], expected);
}

#[test]
fn invalid_queries_and_layout_dependent_attributes_report_source_diagnostics() {
    for (body, diagnostic) in [
        (
            "&A SETC SYSATTRA('A','B')",
            "requires one character argument",
        ),
        ("&A SETC SYSATTRP(17)", "requires a character expression"),
        ("&A SETC SYSATTRA('A'", "unclosed macro parenthesis"),
        (
            "VALUE EQU MAIN,,,(*-MAIN),GR32\n&A SETC SYSATTRP('VALUE')",
            "location counter is not available",
        ),
    ] {
        let error = compile(&format!(
            "MAIN CSECT\n         BR 14\n{body}\n         END\n"
        ))
        .unwrap_err();
        assert!(error.contains(diagnostic), "{error}");
        assert!(error.contains("queries.asm:"), "{error}");
    }
}

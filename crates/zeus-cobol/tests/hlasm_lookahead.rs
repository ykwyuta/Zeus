use zeus_cobol::hlasm;

#[test]
fn forward_attributes_scan_source_and_copy_without_defining_or_generating_symbols() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    std::fs::write(
        dir.path().join("FIELDS.mac"),
        "VALUE DC FL4S2'1.25'\nTEXT DC CL7'ABC'\n",
    )
    .unwrap();
    let source = "&MEM SETC 'FIELDS'\n MACRO\n QUERY\n AIF (NOT D'VALUE AND L'VALUE EQ 4).ONE\n MNOTE 8,'defined or length lookahead'\n.ONE AIF (T'VALUE EQ 'G' AND S'VALUE EQ 2 AND I'VALUE EQ 29).TWO\n MNOTE 8,'type or numeric lookahead'\n.TWO AIF (L'TEXT EQ 7 AND NOT D'TEXT).OK\n MNOTE 8,'copy lookahead'\n.OK ANOP\n MEND\nTEST CSECT\n QUERY\n USING TEST,15\n LA 15,L'TEXT\n BR 14\n COPY &MEM\n END TEST\n";
    std::fs::write(&path, source).unwrap();
    let report = hlasm::compile_report(&path, &[]);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let module = report.module.unwrap();
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128).unwrap());
    assert_eq!(hlasm::execute(&module, &mut memory, 128, &[]).unwrap(), 7);
}

#[test]
fn lookahead_skips_macro_definitions_and_does_not_substitute_open_code_model_fields() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    std::fs::write(&path, "&N SETA L'LATER\n AIF (T'GENERATED EQ 'U' AND NOT D'LATER).OK\n MNOTE 8,'lookahead generated a definition'\n.OK ANOP\n MACRO\n DEFINE\nGENERATED DC CL2'AB'\n MEND\nTEST CSECT\n LA 15,&N\n BR 14\nLATER DC CL9'A'\n END TEST\n").unwrap();
    let report = hlasm::compile_report(&path, &[]);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
}

#[test]
fn forward_external_attributes_are_found_in_declaration_operands() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("external.asm");
    std::fs::write(&source, " AIF (T'FUTURE EQ 'T' AND T'OPTIONAL EQ '$' AND NOT D'FUTURE).OK\n MNOTE 8,'external lookahead'\n.OK ANOP\nTEST CSECT\n LA 15,0\n BR 14\n EXTRN FUTURE\n WXTRN OPTIONAL\nVALUE DC A(T'FUTURE-C'T')\n ENTRY VALUE\n END TEST\n").unwrap();
    let report = hlasm::compile_report(&source, &[]);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let module = report.module.unwrap();
    let at = module.entries["VALUE"] as usize;
    assert_eq!(&module.storage[at..at + 4], &[0, 0, 0, 0]);
}

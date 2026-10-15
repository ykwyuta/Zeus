use zeus_cobol::hlasm;

fn compile(source: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("modifier-exprs.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[])
}

fn words(module: &hlasm::Module, name: &str, count: usize) -> Vec<i32> {
    let at = module.entries[name] as usize;
    module.storage[at..at + count * 4]
        .chunks_exact(4)
        .map(|b| i32::from_be_bytes(b.try_into().unwrap()))
        .collect()
}

#[test]
fn length_scale_and_exponent_accept_nested_signed_expressions_and_forward_equ() {
    let module = compile("MAIN CSECT\n         BR 14\nDATA DC 2FL(WIDTH)S+(SHIFT*2)E-(POWER)'1.25,-1.25'\nWIDTH EQU 2+2\nSHIFT EQU 2\nPOWER EQU 1\nATTR DC A(L'DATA,S'DATA,I'DATA)\n         ENTRY DATA,ATTR\n         END\n").unwrap();
    assert_eq!(words(&module, "DATA", 4), [2, -2, 2, -2]);
    assert_eq!(words(&module, "ATTR", 3), [4, 4, 27]);
}

#[test]
fn modifiers_accept_self_defining_terms_and_literal_attributes_inside_parentheses() {
    let module = compile("MAIN CSECT\n         BR 14\nDATA DC FL(L'=F'0')S(C')'-C')'+2)E(X'01'-1)'1.25'\nATTR DC A(L'=FL(2+2)S-(1+1)'20',S'DATA,I'DATA)\n         ENTRY DATA,ATTR\n         END\n").unwrap();
    assert_eq!(words(&module, "DATA", 1), [5]);
    assert_eq!(words(&module, "ATTR", 3), [4, 2, 29]);
}

#[test]
fn conditional_attributes_and_macro_arguments_use_already_known_equ_values() {
    let module = compile("WIDTH EQU 4\nSHIFT EQU 3\n         MACRO\n         CHECK &P\n         AIF (L'&P EQ 4 AND S'&P EQ 3).NEXT\n         MNOTE 8,'expression literal attributes wrong'\n.NEXT AIF (I'&SYSLIST(1) EQ 28).OK\n         MNOTE 8,'integer attribute wrong'\n.OK ANOP\n         MEND\n         CHECK =FL(WIDTH)S(SHIFT)E-(1)'15E1'\n&I SETA I'=FL(WIDTH)S(SHIFT)E-(1)'15E1'\n         AIF (D'=FL(WIDTH)S(SHIFT)E-(1)'15E1').DONE\n         MNOTE 8,'literal definition wrong'\n.DONE ANOP\nMAIN CSECT\n         LGHI 15,&I\n         BR 14\n         END\n").unwrap();
    let mut storage = vec![0; 128];
    storage.extend(module.storage_at(128).unwrap());
    assert_eq!(hlasm::execute(&module, &mut storage, 128, &[]).unwrap(), 28);
    let plain = compile("MAIN CSECT\n         LGHI 15,28\n         BR 14\n         END\n").unwrap();
    assert_eq!(module.storage, plain.storage);
}

#[test]
fn generated_symbol_attributes_and_runtime_literal_pool_use_expression_values() {
    let module = compile("WIDTH EQU 4\nSHIFT EQU 2\nDATA DC FL(WIDTH)S(SHIFT)'1.25'\n         AIF (L'DATA EQ 4 AND S'DATA EQ 2 AND I'DATA EQ 29).OK\n         MNOTE 8,'data attributes wrong'\n.OK ANOP\nMAIN CSECT\n         USING MAIN,15\n         L 3,=FL(WIDTH)S(SHIFT)'1.25'\n         LH 4,=HS-(SHIFT)E(1)'2'\n         LR 15,3\n         AR 15,4\n         BR 14\n         END MAIN\n").unwrap();
    let mut storage = vec![0; 128];
    storage.extend(module.storage_at(128).unwrap());
    assert_eq!(hlasm::execute(&module, &mut storage, 128, &[]).unwrap(), 10);
}

#[test]
fn unavailable_forward_literal_values_and_relocatable_or_layout_expressions_fail() {
    for source in [
        "MAIN CSECT\n         L 2,=FS(SHIFT)'1'\nSHIFT EQU 2\n         END\n",
        "&S SETA S'=FS(SHIFT)'1'\nSHIFT EQU 2\nMAIN CSECT\n         END\n",
        "MAIN CSECT\nDATA DC FS(MAIN)'1'\n         END\n",
        "MAIN CSECT\nDATA DC FS(*-MAIN)'1'\n         END\n",
    ] {
        assert!(compile(source).is_err(), "{source}");
    }
}

#[test]
fn malformed_or_out_of_range_expression_modifiers_keep_source_diagnostics() {
    for spec in [
        "FL(-1)'1'",
        "FS(347)'0'",
        "FE(-86)'0'",
        "FS(1/0)'1'",
        "FS(UNKNOWN)'1'",
        "FS(1+)'1'",
        "FS(1'1'",
        "FS(2147483648)'0'",
        "FS(2)L(4)'1'",
        "0FL(2097152)'0'",
    ] {
        let error = compile(&format!("MAIN CSECT\nDATA DC {spec}\n         END\n")).unwrap_err();
        assert!(error.contains("modifier-exprs.asm:2:"), "{spec}: {error}");
    }
}

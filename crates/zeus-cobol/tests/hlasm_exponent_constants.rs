use zeus_cobol::hlasm;

fn compile(source: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("exponents.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[])
}

fn words(module: &hlasm::Module, label: &str, count: usize) -> Vec<i32> {
    let at = module.entries[label] as usize;
    module.storage[at..at + count * 4]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| i32::from_be_bytes(*b))
        .collect()
}

#[test]
fn exponent_modifiers_combine_with_each_nominal_and_preserve_numeric_attributes() {
    let module = compile("MAIN CSECT\n         BR 14\nDATA DC 2FS2E+1'1.25,-1.25,15E-1'\n         DC F'2E3,25E-1,-25E-1'\nATTR DC A(L'DATA,S'DATA,I'DATA)\n         ENTRY DATA,ATTR\n         END\n").unwrap();
    assert_eq!(
        words(&module, "DATA", 9),
        [50, -50, 60, 50, -50, 60, 2000, 3, -3]
    );
    assert_eq!(words(&module, "ATTR", 3), [4, 2, 29]);
}

#[test]
fn decimal_exponents_use_exact_digits_across_large_intermediates_and_rounding() {
    let module = compile("MAIN CSECT\n         BR 14\nDATA DC FDE75'9.223372036854775807E-57'\n         DC FDE75'-9.223372036854775808E-57'\n         DC FDS100E-30'1'\n         DC FDE-85'1'\n         DC FDE75'0'\n         DC FDE-18'1499999999999999999'\n         ENTRY DATA\n         END\n").unwrap();
    let at = module.entries["DATA"] as usize;
    let values: Vec<_> = module.storage[at..at + 48]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| i64::from_be_bytes(*b))
        .collect();
    assert_eq!(values, [i64::MAX, i64::MIN, 1, 0, 0, 1]);
}

#[test]
fn exponent_literals_execute_and_macro_parameter_attributes_do_not_allocate_data() {
    let module = compile(
        "         MACRO\n         CHECK &P\n         AIF (S'&P EQ 3 AND I'&SYSLIST(1) EQ 28).OK\n         MNOTE 8,'exponent literal attributes wrong'\n.OK ANOP\n         MEND\n         CHECK =FS3E-1'15E1'\nMAIN CSECT\n         USING MAIN,15\n         L 3,=FS3E-1'15E1'\n         LH 4,=HE1'1'\n         LR 15,3\n         AR 15,4\n         BR 14\n         END\n",
    );
    let module = module.unwrap();
    let mut storage = vec![0; 128];
    storage.extend(module.storage_at(128).unwrap());
    assert_eq!(
        hlasm::execute(&module, &mut storage, 128, &[]).unwrap(),
        130
    );
    let plain = compile("MAIN CSECT\n         BR 14\n         END\n").unwrap();
    let attributes = compile("&I SETA I'=FS3E-1'15E1'\n         AIF (&I EQ 28 AND D'=FS3E-1'15E1').OK\n         MNOTE 8,'exponent metadata wrong'\n.OK ANOP\nMAIN CSECT\n         BR 14\n         END\n").unwrap();
    assert_eq!(plain.storage, attributes.storage);
}

#[test]
fn explicit_lengths_halfwords_and_negative_binary_scales_work_with_exponents() {
    let module = compile("MAIN CSECT\n         BR 14\nDATA DC 2HL3S-1E2'2.5,-2.5'\n         DS FS-2E75\nATTR DC A(L'DATA,S'DATA,I'DATA)\n         ENTRY DATA,ATTR\n         END\n").unwrap();
    let at = module.entries["DATA"] as usize;
    assert_eq!(
        &module.storage[at..at + 12],
        [0, 0, 125, 255, 255, 131, 0, 0, 125, 255, 255, 131]
    );
    assert_eq!(words(&module, "ATTR", 3), [3, -1, 24]);
}

#[test]
fn malformed_out_of_order_and_out_of_range_exponents_are_diagnosed() {
    for spec in [
        "HSE1'1'",
        "FE76'0'",
        "FE-86'0'",
        "FE75'1E1'",
        "FE-85'0E-1'",
        "F'1E'",
        "F'E1'",
        "F'1E+-1'",
        "F'1E1E1'",
        "FE1S2'1'",
        "FE1L4'1'",
        "FE(1/0)'1'",
        "FE2147483648'0'",
        "F'1E2147483648'",
        "FDE75'1'",
        "F'2147483648E0'",
        "ZE1'1'",
    ] {
        let error = compile(&format!("MAIN CSECT\nDATA DC {spec}\n         END\n")).unwrap_err();
        assert!(error.contains("exponents.asm:2:"), "{spec}: {error}");
    }
}

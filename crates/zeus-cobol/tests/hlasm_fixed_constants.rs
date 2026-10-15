use zeus_cobol::hlasm;

fn compile(source: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixed.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[])
}

#[test]
fn fixed_scales_decimal_values_rounding_and_attributes() {
    let module = compile("MAIN CSECT\n         BR 14\nDATA DC FS2'1.25,-1.25,1.125,-1.125'\n         DC HS-2'10,-10,9,-9'\n         DC F'1.499999999999999999999,1.5,-1.5'\nATTR DC A(S'DATA,I'DATA,S'=FDS-187'0',I'=FDS346'0')\n         ENTRY DATA,ATTR\n         END\n").unwrap();
    let at = module.entries["DATA"] as usize;
    let fullwords: Vec<_> = module.storage[at..at + 16]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| i32::from_be_bytes(*b))
        .collect();
    assert_eq!(fullwords, [5, -5, 5, -5]);
    let halfwords: Vec<_> = module.storage[at + 16..at + 24]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| i16::from_be_bytes(*b))
        .collect();
    assert_eq!(halfwords, [3, -3, 2, -2]);
    let rounded: Vec<_> = module.storage[at + 24..at + 36]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| i32::from_be_bytes(*b))
        .collect();
    assert_eq!(rounded, [1, 2, -2]);
    let at = module.entries["ATTR"] as usize;
    let attributes: Vec<_> = module.storage[at..at + 16]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| i32::from_be_bytes(*b))
        .collect();
    assert_eq!(attributes, [2, 29, -187, -283]);
}

#[test]
fn exact_conversion_handles_large_intermediates_and_signed_boundaries() {
    let module = compile("MAIN CSECT\n         BR 14\nDATA DC FD'9223372036854775807,-9223372036854775808'\n         DC FDS-64'18446744073709551616'\n         DC FDS100'.000000000000000000000000000001'\n         DC FDS346'0'\n         DC FDS-187'1'\n         ENTRY DATA\n         END\n").unwrap();
    let at = module.entries["DATA"] as usize;
    let values: Vec<_> = module.storage[at..at + 48]
        .as_chunks::<8>()
        .0
        .iter()
        .map(|b| i64::from_be_bytes(*b))
        .collect();
    assert_eq!(values, [i64::MAX, i64::MIN, 1, 1, 0, 0]);
}

#[test]
fn scaled_literals_execute_and_conditional_attributes_follow_macro_arguments() {
    let module = compile("         MACRO\n         CHECK &VALUE\n&S SETA S'&VALUE\n&I SETA I'&SYSLIST(1)\n         AIF (&S EQ -2 AND &I EQ 33).GOOD\n         MNOTE 8,'scaled attributes wrong'\n.GOOD ANOP\n         MEND\n         CHECK =FS-2'20'\nMAIN CSECT\n         USING MAIN,15\n         L 3,=FS8'1.25'\n         A 3,=FS-2'20'\n         LH 4,=HS+1'-.5'\n         AR 3,4\n         LR 15,3\n         BR 14\n         END\n").unwrap();
    let mut storage = vec![0; 128];
    storage.extend(module.storage_at(128).unwrap());
    assert_eq!(
        hlasm::execute(&module, &mut storage, 128, &[]).unwrap(),
        324
    );
}

#[test]
fn explicit_lengths_scaling_lists_and_duplication_preserve_layout() {
    let module = compile("MAIN CSECT\n         BR 14\nDATA DC 2FL3S+4'1.5,-1.5'\n         DC HL4S-1'65536'\n         DS HS3\nATTR DC A(L'DATA,S'DATA,I'DATA,T'DATA)\n         ENTRY DATA,ATTR\n         END\n").unwrap();
    let at = module.entries["DATA"] as usize;
    assert_eq!(
        &module.storage[at..at + 16],
        [
            0, 0, 24, 255, 255, 232, 0, 0, 24, 255, 255, 232, 0, 0, 128, 0
        ]
    );
    let at = module.entries["ATTR"] as usize;
    let values: Vec<_> = module.storage[at..at + 16]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| i32::from_be_bytes(*b))
        .collect();
    assert_eq!(
        values,
        [3, 4, 19, i32::from(zeus_cobol::ebcdic::encode("G")[0])]
    );
}

#[test]
fn invalid_scales_nominals_overflow_and_remaining_modifiers_fail() {
    for spec in [
        "FS347'0'",
        "HS-188'0'",
        "FLS2'1'",
        "FS1L4'1'",
        "FS(2/0)'1'",
        "FS2E76'1'",
        "F'1E-86'",
        "F'--1'",
        "H'.'",
        "FD'1.2.3'",
        "H'32767.5'",
        "F'2147483647.5'",
        "FD'9223372036854775807.5'",
        "FDS346'1'",
    ] {
        let error = compile(&format!("MAIN CSECT\nDATA DC {spec}\n         END\n")).unwrap_err();
        assert!(error.contains("fixed.asm:2:"), "{spec}: {error}");
    }
}

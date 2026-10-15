use zeus_cobol::hlasm;

fn compile(source: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("numeric-attrs.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[])
}

fn bytes<'a>(module: &'a hlasm::Module, name: &str, length: usize) -> &'a [u8] {
    let at = module.entries[name] as usize;
    &module.storage[at..at + length]
}

#[test]
fn ordinary_numeric_attributes_match_decimal_storage_and_equ_resets() {
    let module = compile("MAIN CSECT\n         BR 14\nFULL DC F'1'\nHALF DS H\nWIDE DC FD'2'\nSHORT DC FL3'3'\nPACK DC P'-12.34'\nZONE DC Z'+.50'\nALIAS EQU PACK\nVALUES DC A(S'FULL,I'FULL,I'HALF,I'WIDE,I'SHORT)\n         DC A(S'PACK,I'PACK,S'ZONE,I'ZONE,S'ALIAS,I'ALIAS)\n         ENTRY PACK,ZONE,VALUES\n         END\n").unwrap();
    assert_eq!(bytes(&module, "PACK", 3), [0x01, 0x23, 0x4d]);
    assert_eq!(bytes(&module, "ZONE", 2), [0xf5, 0xc0]);
    let values: Vec<_> = bytes(&module, "VALUES", 44)
        .chunks_exact(4)
        .map(|b| i32::from_be_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(values, [0, 31, 15, 63, 23, 2, 3, 2, 0, 0, 0]);
}

#[test]
fn decimal_lists_and_duplication_take_attributes_from_first_nominal() {
    let module = compile("MAIN CSECT\n         BR 14\nPACK DC 2P'1.2,345.67'\nZONE DC ZL4'-1.25'\nVALUES DC A(L'PACK,S'PACK,I'PACK,L'ZONE,S'ZONE,I'ZONE)\n         ENTRY PACK,ZONE,VALUES\n         END\n").unwrap();
    assert_eq!(
        bytes(&module, "PACK", 10),
        [0x01, 0x2c, 0x34, 0x56, 0x7c, 0x01, 0x2c, 0x34, 0x56, 0x7c]
    );
    assert_eq!(bytes(&module, "ZONE", 4), [0xf0, 0xf1, 0xf2, 0xd5]);
    let values: Vec<_> = bytes(&module, "VALUES", 24)
        .chunks_exact(4)
        .map(|b| i32::from_be_bytes(b.try_into().unwrap()))
        .collect();
    assert_eq!(values, [2, 1, 2, 4, 2, 2]);
}

#[test]
fn conditional_numeric_attributes_accept_symbols_parameters_and_setc_arrays() {
    let module = compile("PACK DC PL5'12.34'\nALIAS EQU PACK,9,C'F'\n         MACRO\n         CHECK &P\n&S SETA S'&P\n&I SETA I'&SYSLIST(1)\n         AIF (&S EQ 2 AND &I EQ 7).GOOD\n         MNOTE 8,'parameter attributes wrong'\n.GOOD ANOP\n         MEND\n         CHECK PACK\n         LCLC &ITEM(1)\n&ITEM(1) SETC '=ZL6''123.45'''\n&S SETA S'&ITEM(1)\n&I SETA I'&ITEM(1)\n         AIF (&S EQ 2 AND &I EQ 4 AND S'ALIAS EQ 0).OK\n         MNOTE 8,'array or EQU attributes wrong'\n.OK ANOP\nMAIN CSECT\n         LGHI 15,I'ALIAS+&I\n         BR 14\n         END MAIN\n").unwrap();
    let mut storage = vec![0; 128];
    storage.extend(module.storage_at(128).unwrap());
    assert_eq!(hlasm::execute(&module, &mut storage, 128, &[]).unwrap(), 4);
}

#[test]
fn numeric_literal_attributes_define_metadata_without_emitting_pool_storage() {
    let plain = compile("MAIN CSECT\n         LGHI 15,7\n         BR 14\n         END\n").unwrap();
    let attributed = compile("&BEFORE SETA D'=PL5'12.34'\n&I SETA I'=PL5'12.34'\n         AIF (&BEFORE EQ 0 AND D'=PL5'12.34').GOOD\n         MNOTE 8,'numeric literal definition wrong'\n.GOOD ANOP\nMAIN CSECT\n         LGHI 15,&I+S'=FD'0'\n         BR 14\n         END\n").unwrap();
    assert_eq!(plain.storage, attributed.storage);
    let mut storage = vec![0; 128];
    storage.extend(attributed.storage_at(128).unwrap());
    assert_eq!(
        hlasm::execute(&attributed, &mut storage, 128, &[]).unwrap(),
        7
    );
    assert!(attributed.relocations.is_empty());
}

#[test]
fn unsupported_attributes_modifiers_and_invalid_decimals_are_diagnosed() {
    for declaration in ["C'x'", "A(0)", "X'01'"] {
        for kind in ['S', 'I'] {
            assert!(
                compile(&format!(
                    "&V SETA {kind}'={declaration}\nMAIN CSECT\n         END\n"
                ))
                .is_err()
            );
        }
    }
    for declaration in ["FS347'1'", "HS-188'2'", "P'1.2.3'", "Z'.'", "P'1a.2'"] {
        assert!(
            compile(&format!(
                "MAIN CSECT\nVALUE DC {declaration}\n         END\n"
            ))
            .is_err(),
            "{declaration}"
        );
    }
    assert!(compile("&V SETA I'LATER\nMAIN CSECT\nLATER DC P'1'\n         END\n").is_err());
    assert!(compile("MAIN CSECT\nLABEL BR 14\nVALUE DC A(S'LABEL)\n         END\n").is_err());
}

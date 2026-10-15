use zeus_cobol::hlasm;

fn assemble(data: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("constants.asm");
    std::fs::write(
        &path,
        format!("TEST CSECT\n LA 15,0\n BR 14\n{data}\n END\n"),
    )
    .unwrap();
    hlasm::compile_file(&path, &[])
}

#[test]
fn duplication_and_length_expressions_accept_forward_absolute_equ_values() {
    let m = assemble("DATA DC (N+1)XL(M*2)'AB'\nN EQU 2\nM EQU 1").unwrap();
    assert_eq!(&m.storage[6..12], &[0, 0xab, 0, 0xab, 0, 0xab]);
    let m = assemble("DATA DC (N)A(TEST)\nN EQU 3").unwrap();
    assert_eq!(m.relocations.len(), 3);
    let image = m.storage_at(128).unwrap();
    assert_eq!(&image[8..20], &[0, 0, 0, 128, 0, 0, 0, 128, 0, 0, 0, 128]);
    for text in [
        "DATA DC (-1)F'0'",
        "DATA DC (TEST)F'0'",
        "DATA DS XL(-1)",
        "DATA DS (2000000)X",
        "DATA DS XL(2000000)",
    ] {
        assert!(assemble(text).is_err(), "{text}");
    }
}

#[test]
fn character_encodings_have_distinct_bytes_padding_truncation_and_lengths() {
    let m = assemble("A DC CA'Abé'\nB DC CAL5'Q'\nC DC CE'Ab'\nD DC CU'AZ日'\nE DC CUL8'Q'\nF DC CUL2'XYZ'\nG DS CU\nH DS CUL6").unwrap();
    let mut expected = vec![b'A', b'b', 0xe9, b'Q', 0x20, 0x20, 0x20, 0x20];
    expected.extend(zeus_cobol::ebcdic::encode("Ab"));
    expected.extend([
        0, 0x41, 0, 0x5a, 0x65, 0xe5, 0, 0x51, 0, 0x20, 0, 0x20, 0, 0x20, 0, 0x58,
    ]);
    expected.extend([0; 8]);
    assert_eq!(&m.storage[6..6 + expected.len()], expected);
    assert!(m.storage[6 + expected.len()..].iter().all(|b| *b == 0));
    assert!(assemble("A DC CA'日'").unwrap_err().contains("CCSID 819"));
    assert!(
        assemble("A DC CUL3'X'")
            .unwrap_err()
            .contains("multiple of two")
    );
    assert!(assemble("A DS CUL3").is_err());
}

#[test]
fn expression_literals_and_macro_attributes_share_declaration_parsing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("literal.asm");
    std::fs::write(&path, "N EQU 2\n AIF (L'=(N+1)CAL(2+1)'A' EQ 3).OK\n MNOTE 8,'literal length'\n.OK ANOP\nTEST CSECT\n USING TEST,15\n LA 2,=(N+1)CAL(2+1)'A'\n LA 15,0\n BR 14\nDATA DC (N+1)CU'AB'\n END\n").unwrap();
    let report = hlasm::compile_report(&path, &[]);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    assert!(
        report
            .module
            .unwrap()
            .storage
            .windows(9)
            .any(|s| s == b"A  A  A  ")
    );
    assert!(assemble("ZERO DC 0F").is_ok());
    assert!(assemble(" LA 2,=0F'1'").is_err());
}

#[test]
fn graphic_constants_use_pure_dbcs_without_shifts_and_pad_with_dbcs_spaces() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("graphic.asm");
    let object = dir.path().join("graphic.zo");
    std::fs::write(
        &path,
        "TEST CSECT\n LA 15,0\n BR 14\nDATA DC 2GL6'日本'\nRES DS G\n END\n",
    )
    .unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .args(["compile", "-o"])
        .arg(&object)
        .arg(&path)
        .env("ZEUS_CCSID", "939")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let m = hlasm::load_module(&std::fs::read(object).unwrap()).unwrap();
    let cp = zeus_cobol::codepage::get(939).unwrap();
    let mut expected = cp.encode_dbcs("日本");
    expected.extend([0x40, 0x40]);
    let mut expected = expected.repeat(2);
    expected.extend([0, 0]);
    assert_eq!(&m.storage[6..6 + expected.len()], expected);
    assert!(m.storage[6 + expected.len()..].iter().all(|b| *b == 0));
}

#[test]
fn bit_fields_pack_across_operands_nominals_and_duplication_before_byte_alignment() {
    let m = assemble("DATA DC BL.3'101',XL.5'1B',FL.12'-1',BL.4'0101'\nDUP DC 3BL.3'101'\nMULT DC XL.4'1,2,3'\nDEC DC PL.12'123',ZL.4'7'\nSPACE DS (3)BL.(2+1)\nADDR DC BL.3'1',XL.2'3',A(TEST)\n").unwrap();
    // Adjacent bit operands share a byte. Each declaration ends at a byte
    // boundary, and the A constant starts at its ordinary fullword alignment.
    // DATA is 24 bits (3 bytes), not four; DUP starts immediately after it.
    assert_eq!(&m.storage[6..9], &[0xbb, 0xff, 0xf5]);
    assert_eq!(&m.storage[9..11], &[0xb6, 0x80]);
    assert_eq!(&m.storage[11..13], &[0x12, 0x30]);
    assert_eq!(&m.storage[13..15], &[0x23, 0xc7]);
    assert_eq!(&m.storage[15..17], &[0, 0]);
    assert_eq!(m.storage[17], 0x38);
    assert!(m.relocations.iter().any(|r| r.offset == 20));
    assert_eq!(&m.storage_at(128).unwrap()[20..24], &[0, 0, 0, 128]);
}

#[test]
fn character_bit_fields_pad_and_truncate_at_the_high_order_end() {
    let m = assemble("DATA DC CAL.4'AB',CAL.12'Q',CL.12'A'\nNEG DC FL.12'-1'\n").unwrap();
    assert_eq!(&m.storage[6..10], &[0x45, 0x12, 0xc1, 0x40]);
    assert_eq!(&m.storage[10..12], &[0xff, 0xf0]);
    for data in [
        "DATA DC BL.0'1'",
        "DATA DC BL.2049'1'",
        "DATA DC CUL.16'X'",
        "DATA DC GL.8'X'",
        "DATA DC VL.4(TEST)",
        "DATA DC FL.65'1'",
        "DATA DC (1000000)BL.64'1'",
    ] {
        assert!(assemble(data).is_err(), "{data}");
    }
}

#[test]
fn byte_nominal_lists_and_reserved_lists_apply_duplication_to_the_whole_list() {
    let m = assemble("DATA DC 2XL2'1,2,3',2BL4'1,10'\nRES DS 2F'1,2,3'\n").unwrap();
    assert_eq!(&m.storage[6..18], &[0, 1, 0, 2, 0, 3, 0, 1, 0, 2, 0, 3]);
    assert_eq!(
        &m.storage[18..34],
        &[0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 2]
    );
    assert!(m.storage[36..60].iter().all(|b| *b == 0));
}

#[test]
fn address_widths_and_doublewords_relocate_after_linking_and_loading() {
    let m = assemble("ADDR DC Y(TEST),AL2(TEST+2),AL3(TEST+4),AD(TEST+6)\nEXT DC VD(TARGET),VL3(TARGET)\n EXTRN TARGET").unwrap();
    assert_eq!(m.zeus_hlasm, 7);
    assert_eq!(
        m.relocations
            .iter()
            .map(|r| (r.offset, r.width))
            .collect::<Vec<_>>(),
        [(6, 2), (8, 2), (10, 3), (16, 8), (24, 8), (32, 3)]
    );
    let m = hlasm::load_module(&hlasm::module_bytes(&m).unwrap()).unwrap();
    let mut target = assemble("").unwrap();
    target.name = "TARGET".into();
    target.entries.clear();
    target.entries.insert("TARGET".into(), 0);
    let linked = hlasm::link_modules(&[m, target]).unwrap();
    assert_eq!(linked.zeus_hlasm, 7);
    let image = linked.storage_at(128).unwrap();
    assert_eq!(&image[6..8], &128u16.to_be_bytes());
    assert_eq!(&image[8..10], &130u16.to_be_bytes());
    assert_eq!(&image[10..13], &[0, 0, 132]);
    assert_eq!(&image[16..24], &134u64.to_be_bytes());
    let external = u64::from(linked.entries["TARGET"]) + 128;
    assert_eq!(&image[24..32], &external.to_be_bytes());
    assert_eq!(&image[32..35], &external.to_be_bytes()[5..]);
    let mut old = linked.clone();
    old.zeus_hlasm = 6;
    assert!(hlasm::load_module(&hlasm::module_bytes(&old).unwrap()).is_err());
}

#[test]
fn address_absolute_values_sign_extension_reservation_and_location_advance() {
    let m = assemble("A DC AL8(-1),AD(4294967296),Y(-1),AL1(257)\nB DC 3AL1(*-B)\nC DS AD,Y,VL3\n")
        .unwrap();
    assert_eq!(&m.storage[6..14], &[0xff; 8]);
    assert_eq!(&m.storage[16..24], &4294967296u64.to_be_bytes());
    assert_eq!(&m.storage[24..26], &[0xff; 2]);
    assert_eq!(m.storage[26], 1);
    assert_eq!(&m.storage[27..30], &[0, 1, 2]);
    assert!(m.storage[32..45].iter().all(|b| *b == 0));
    for source in [
        "A DC AL1(TEST)",
        "A DC AL5(TEST)",
        "A DC YL1(TEST)",
        "A DC ADL1(TEST)",
        "A DC VL2(TEST)",
        "A DC VDL5(TEST)",
        "A DC AL0(0)",
        "A DC YL3(0)",
        "A DC AD",
    ] {
        assert!(assemble(source).is_err(), "{source}");
    }
}

#[test]
fn absolute_bit_address_fields_pack_and_relocatable_operands_are_rejected() {
    let m =
        assemble("DATA DC AL.4(3,4),YL.4(-1),AL.4(2)\nWIDE DC AL.128(-1)\nRES DS YL.3\n").unwrap();
    assert_eq!(&m.storage[6..8], &[0x34, 0xf2]);
    assert_eq!(&m.storage[8..24], &[0xff; 16]);
    assert_eq!(m.storage[24], 0);
    assert!(m.relocations.is_empty());
    for source in [
        "DATA DC AL.8(TEST)",
        "DATA DC YL.16(TEST)",
        "DATA DC AL.129(1)",
        "DATA DC YL.17(1)",
    ] {
        assert!(assemble(source).is_err(), "{source}");
    }
}

#[test]
fn base_displacement_constants_use_declared_bindings_and_signed_long_displacement() {
    let m = assemble(
        " USING TEST,12\nA DC S(TEST,4095(3)),SY(-2(3),524287(15),-524288(1))\nB DS 2S,2SY",
    )
    .unwrap();
    assert_eq!(
        &m.storage[6..19],
        &[
            0xc0, 0, 0x3f, 0xff, 0x3f, 0xfe, 0xff, 0xff, 0xff, 0x7f, 0x10, 0, 0x80
        ]
    );
    assert!(m.relocations.is_empty());
    for text in [
        "A DC S(-1(3))",
        "A DC S(4096(3))",
        "A DC SY(524288(3))",
        "A DC SY(-524289(3))",
        "A DC SL3(0)",
        "A DC SYL2(0)",
        "A DC S(1(0,3))",
        "A DC S(TEST)",
        "A DC S",
    ] {
        assert!(assemble(text).is_err(), "{text}");
    }
}

#[test]
fn base_constants_advance_location_and_literals_bind_at_their_pool() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("base.asm");
    std::fs::write(&path, "TEST CSECT\n USING TEST,12\n LA 2,=2S(*)\n DROP 12\n USING TEST,11\n LTORG\n DROP 11\n USING TEST,10\n LA 3,=SY(TEST-1)\n LA 15,0\n BR 14\nDATA DC 2S(*,*)\n END\n").unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    assert_eq!(&m.storage[8..12], &[0xb0, 0, 0xb0, 0]);
    let data = m.code.last().unwrap().offset as usize + 2;
    assert_eq!(
        &m.storage[data..data + 8],
        &[
            0xa0,
            data as u8,
            0xa0,
            (data + 2) as u8,
            0xa0,
            (data + 4) as u8,
            0xa0,
            (data + 6) as u8
        ]
    );
    assert!(m.storage.windows(3).any(|w| w == [0xaf, 0xff, 0xff]));
}

#[test]
fn external_dummy_offsets_lengths_and_total_merge_across_modules() {
    let a = assemble("A DXD CL3\nB DXD F\nOFF DC Q(A,B,C),QD(B),QY(C)\nLEN DC J(A,B,C),JD(B)\nTOTAL CXD\nC DSECT\nFIELD DS 3H").unwrap();
    let bytes = a.storage_at(0).unwrap();
    // A=0..3, B=4..8, C=8..14. DSECT length is its full extent.
    let quantities = &a.bindings.quantities;
    let get = |i: usize| {
        let q = &quantities[i];
        &bytes[q.offset as usize..q.offset as usize + q.width as usize]
    };
    assert_eq!(get(0), [0, 0, 0, 0]);
    assert_eq!(get(1), [0, 0, 0, 4]);
    assert_eq!(get(2), [0, 0, 0, 8]);
    assert_eq!(get(3), [0, 0, 0, 0, 0, 0, 0, 4]);
    assert_eq!(get(4), [0, 8, 0]);
    assert_eq!(get(5), [0, 0, 0, 3]);
    assert_eq!(get(6), [0, 0, 0, 4]);
    assert_eq!(get(7), [0, 0, 0, 6]);
    assert_eq!(get(8), [0, 0, 0, 0, 0, 0, 0, 4]);
    assert_eq!(get(9), [0, 0, 0, 14]);
    let b = assemble("A DXD 3FD\nB DXD 2FD").unwrap();
    let mut b = b;
    b.name = "OTHER".into();
    b.entries.clear();
    b.entries.insert("OTHER".into(), b.entry);
    let linked = hlasm::link_modules(&[a.clone(), b]).unwrap();
    let bytes = linked.storage_at(8192).unwrap();
    assert_eq!(
        &bytes[quantities[1].offset as usize..quantities[1].offset as usize + 4],
        &[0, 0, 0, 24]
    );
    assert_eq!(
        &bytes[quantities[2].offset as usize..quantities[2].offset as usize + 4],
        &[0, 0, 0, 40]
    );
    assert_eq!(
        &bytes[quantities[9].offset as usize..quantities[9].offset as usize + 4],
        &[0, 0, 0, 46]
    );
    assert_eq!(
        hlasm::load_module(&hlasm::module_bytes(&linked).unwrap())
            .unwrap()
            .storage_at(8192)
            .unwrap(),
        bytes
    );
}

#[test]
fn section_quantities_diagnose_unresolved_malformed_and_overflowing_values() {
    for text in [
        "A DC QL5(X)",
        "A DC QYL4(X)",
        "A DC JL1(X)",
        "A DC Q(X+1)",
        "A DC J",
        "A DXD F\nA DXD H",
        "SIZE CXD 1",
    ] {
        assert!(assemble(text).is_err(), "{text}");
    }
    let m = assemble("OFF DC Q(MISSING)").unwrap();
    assert!(
        m.storage_at(0)
            .unwrap_err()
            .contains("unresolved external dummy")
    );
    assert!(hlasm::link_modules(&[m]).is_err());
    let m = assemble("A DXD CL524288\nB DXD F\nOFF DC QY(B)").unwrap();
    assert!(m.storage_at(0).unwrap_err().contains("signed 20-bit"));
    let m =
        assemble("QTY DC QL1(A),QL2(B)\nA DXD CL257\nB DXD F\nUNUSED DSECT\nX DS CL1000").unwrap();
    let b = m.storage_at(0).unwrap();
    let q = &m.bindings.quantities[1];
    assert_eq!(&b[q.offset as usize..q.offset as usize + 2], &[1, 4]);
}

#[test]
fn dc_ds_annotations_preserve_bytes_and_expose_program_and_assembler_types() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("annotations.asm");
    std::fs::write(&path, "TEST CSECT\nA DC FDP(C'TEST')'17'\nB DS XP(C'ZERO')L.3\nC DC F'1',HP(C'NEXT')'2'\nD DC QDP(C'OFFS')(EXT)\nEXT DXD F\n AIF (SYSATTRP('A') EQ 'TEST' AND SYSATTRA('A') EQ 'FD').A\n MNOTE 8,'A annotation'\n.A ANOP\n AIF (SYSATTRP('B') EQ 'ZERO' AND SYSATTRA('B') EQ 'X').B\n MNOTE 8,'B annotation'\n.B ANOP\n AIF (SYSATTRP('C') EQ 'NEXT' AND SYSATTRA('C') EQ 'F').C\n MNOTE 8,'first annotated operand'\n.C ANOP\n AIF (SYSATTRP('D') EQ 'OFFS' AND SYSATTRA('D') EQ 'QD').D\n MNOTE 8,'QD annotation'\n.D ANOP\n USING TEST,12\nSTART LA 2,=FP(7)'9'\n LA 15,0\n BR 14\n END START\n").unwrap();
    let report = hlasm::compile_report(&path, &[]);
    assert!(report.module.is_some(), "{:?}", report.diagnostics);
    let m = report.module.unwrap();
    assert_eq!(&m.storage[..8], &17u64.to_be_bytes());
    for value in ["4294967296", "-2147483649", "", "TEST"] {
        assert!(
            assemble(&format!("DATA DC FP({value})'1'")).is_err(),
            "{value}"
        );
    }
}

#[test]
fn psect_constants_use_explicit_association_and_relocate_across_link_origins() {
    let m = assemble(
        "REF DC R(TEST),RD(TEST),RL3(TEST)\nTEST XATTR PSECT(AREA),LINKAGE(OS)\nAREA DS 4F",
    )
    .unwrap();
    let target = m.bindings.psects[0].offset;
    for origin in [0, 4096, 65536] {
        let bytes = m.storage_at(origin).unwrap();
        for q in &m.bindings.quantities {
            assert_eq!(
                &bytes[q.offset as usize..q.offset as usize + q.width as usize],
                &u64::from(target + origin as u32).to_be_bytes()[8 - q.width as usize..]
            );
        }
    }
    let mut prefix = assemble("PAD DS CL35").unwrap();
    prefix.name = "PREFIX".into();
    prefix.entries.clear();
    prefix.entries.insert("PREFIX".into(), 0);
    let linked = hlasm::link_modules(&[prefix, m.clone()]).unwrap();
    let target = linked.bindings.psects[0].offset;
    let bytes = linked.storage_at(1024).unwrap();
    let q = &linked.bindings.quantities[0];
    assert_eq!(
        &bytes[q.offset as usize..q.offset as usize + 4],
        &(target + 1024).to_be_bytes()
    );
    assert!(m.storage_at(0x1000000).unwrap_err().contains("24 bits"));
    for text in [
        "REF DC R(TEST)",
        "REF DC RL2(TEST)",
        "REF DC RDL5(TEST)",
        "REF DC R(TEST+1)",
    ] {
        if let Ok(m) = assemble(text) {
            assert!(m.storage_at(0).is_err(), "{text}");
        }
    }
}

#[test]
fn quantities_pull_library_members_and_reject_malformed_module_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("external.asm");
    std::fs::write(&source, "OTHER CSECT\n LA 15,0\n BR 14\nEXT DXD 7F\n END\n").unwrap();
    let external = hlasm::compile_file(&source, &[]).unwrap();
    std::fs::write(
        dir.path().join("EXT.zo"),
        hlasm::module_bytes(&external).unwrap(),
    )
    .unwrap();
    let main = assemble("REF DC Q(EXT),J(EXT)\nTOTAL CXD").unwrap();
    let linked = hlasm::link_with_libraries(vec![main.clone()], &[dir.path().into()]).unwrap();
    let bytes = linked.storage_at(0).unwrap();
    for q in &linked.bindings.quantities[1..] {
        assert_eq!(
            &bytes[q.offset as usize..q.offset as usize + 4],
            &28u32.to_be_bytes()
        );
    }
    let mut bad = main.clone();
    bad.bindings.quantities[0].width = 0;
    assert!(hlasm::load_module(&hlasm::module_bytes(&bad).unwrap()).is_err());
    let mut bad = main;
    bad.zeus_hlasm = 8;
    assert!(
        hlasm::load_module(&hlasm::module_bytes(&bad).unwrap())
            .unwrap_err()
            .contains("version 9")
    );
}

#[test]
fn classes_separate_readonly_code_and_writable_psect_storage() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("classes.asm");
    std::fs::write(&path,"TEST RSECT\n USING TEST,12\n LR 12,15\n MVI AREA,7\n LA 15,0\n BR 14\nWORK CATTR NOTEXECUTABLE,ALIGN(4),RMODE(31)\nAREA DC F'0'\nREF DC R(TEST)\nTEST XATTR PSECT(AREA)\nTEST RSECT\n BR 14\nWORK CATTR\nTAIL DS H\n END TEST\n").unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    assert_eq!(m.alignment, 16);
    assert_eq!(m.rmode, 31);
    let target = m.bindings.psects[0].offset;
    assert_eq!(target % 16, 0);
    assert!(!m.read_only.iter().any(|r| r.contains(&target)));
    let mut memory = m.storage_at(0).unwrap();
    hlasm::execute(&m, &mut memory, 0, &[]).unwrap();
    assert_eq!(memory[target as usize], 7);
    for (source, message) in [
        (
            "TEST CSECT\n BR 14\nDATA CATTR NOTEXECUTABLE\n BR 14\n END TEST",
            "NOTEXECUTABLE",
        ),
        (
            "TEST RSECT\n BR 14\nTEST XATTR PSECT(TEST)\n END TEST",
            "PSECT",
        ),
        (
            "TEST CSECT\n BR 14\nDATA CATTR ALIGN(5)\n END TEST",
            "ALIGN",
        ),
        (
            "TEST CSECT\n BR 14\nDATA CATTR\nDATA CATTR READONLY\n END TEST",
            "continuation",
        ),
    ] {
        std::fs::write(&path, source).unwrap();
        assert!(
            hlasm::compile_file(&path, &[])
                .unwrap_err()
                .contains(message),
            "{source}"
        );
    }
}

#[test]
fn noload_class_parts_merge_by_name_and_resolve_priority_ordered_offsets() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("parts.asm");
    std::fs::write(&path,"TEST CSECT\n LA 15,0\n BR 14\nOFF DC Q(PARTA,PARTB,PRIVATE),QY(PARTA)\nLEN DC J(PARTA,PARTB,PRIVATE)\nTOTAL CXD\nPRIVATE CATTR NOLOAD,NOTEXECUTABLE,ALIGN(2),PART(PARTA),PRIORITY(1)\nA DS CL3\nPRIVATE CATTR PART(PARTB),PRIORITY(2)\nB DS CL5\n END TEST\n").unwrap();
    let a = hlasm::compile_file(&path, &[]).unwrap();
    let get = |module: &hlasm::Module| {
        let bytes = module.storage_at(1024).unwrap();
        module
            .bindings
            .quantities
            .iter()
            .map(|q| bytes[q.offset as usize..q.offset as usize + q.width as usize].to_vec())
            .collect::<Vec<_>>()
    };
    let q = get(&a);
    assert_eq!(q[0], [0, 0, 0, 8]);
    assert_eq!(q[1], [0, 0, 0, 0]);
    assert_eq!(q[2], [0, 0, 0, 0]);
    assert_eq!(q[3], [0, 8, 0]);
    assert_eq!(q[4], [0, 0, 0, 3]);
    assert_eq!(q[5], [0, 0, 0, 5]);
    assert_eq!(q[6], [0, 0, 0, 11]);
    assert_eq!(q[7], [0, 0, 0, 0]);
    std::fs::write(&path,"OTHER CSECT\n LA 15,0\n BR 14\nPRIVATE CATTR NOLOAD,NOTEXECUTABLE,ALIGN(2),PART(PARTA)\nA DS CL10\n END OTHER\n").unwrap();
    let b = hlasm::compile_file(&path, &[]).unwrap();
    let linked = hlasm::link_modules(&[a, b]).unwrap();
    let q = get(&linked);
    assert_eq!(q[0], [0, 0, 0, 8]);
    assert_eq!(q[4], [0, 0, 0, 10]);
    assert_eq!(q[6], [0, 0, 0, 18]);
    for attrs in [
        "PART(X)",
        "FILL(256),PART(X),NOLOAD",
        "PRIORITY(2147483648),PART(X),NOLOAD",
        "PRIORITY(1)",
        "FILL(1)",
    ] {
        std::fs::write(
            &path,
            format!("TEST CSECT\n BR 14\nPRIVATE CATTR {attrs}\n END TEST"),
        )
        .unwrap();
        assert!(hlasm::compile_file(&path, &[]).is_err(), "{attrs}");
    }
}

#[test]
fn address_and_graphic_type_attributes_match_implied_and_explicit_lengths() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("types.asm");
    let declarations = [
        ("A", "DS", "J(EXT)", "A"),
        ("B", "DS", "JD(EXT)", "A"),
        ("C", "DS", "R(EXT)", "V"),
        ("D", "DS", "RD(EXT)", "V"),
        ("E", "DS", "S(0)", "S"),
        ("F", "DS", "SY(0)", "S"),
        ("G", "DS", "Q(EXT)", "Q"),
        ("H", "DS", "QD(EXT)", "Q"),
        ("I", "DS", "QY(EXT)", "Q"),
        ("J", "DS", "SL2(0)", "R"),
        ("K", "DS", "SYL3(0)", "R"),
        ("L", "DS", "QL4(EXT)", "R"),
        ("M", "DS", "YL2", "R"),
        ("N", "DS", "JDL8(EXT)", "R"),
        ("O", "DS", "G", "@"),
        ("P", "CXD", "", "A"),
        ("EXT", "DXD", "F", "J"),
    ];
    let mut source = "TEST CSECT\n LA 15,0\n BR 14\n".to_string();
    for (name, op, decl, _) in &declarations {
        source += &format!("{name} {op} {decl}\n");
    }
    for (index, (name, _, _, kind)) in declarations.iter().enumerate() {
        source += &format!(
            " AIF (T'{name} EQ '{kind}').OK{index}\n MNOTE 8,'{name} type'\n.OK{index} ANOP\n"
        );
    }
    source += " END TEST\n";
    std::fs::write(&path, source).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_zas"))
        .arg("check")
        .arg(&path)
        .env("ZEUS_CCSID", "939")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn q_constants_for_class_labels_follow_element_concatenation_and_part_merging() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("class-label.asm");
    std::fs::write(&path,"FIRST CSECT\n BR 14\nOFF DC Q(ITEM),QY(ITEM),J(PRIVATE)\nPRIVATE CATTR NOTEXECUTABLE,ALIGN(2),RMODE(31)\n DS CL1\nITEM DC XL2'1122'\n ENTRY ITEM\nFIRST XATTR PSECT(ITEM)\n END FIRST\n").unwrap();
    let first = hlasm::compile_file(&path, &[]).unwrap();
    std::fs::write(&path,"SECOND CSECT\n BR 14\nOFF DC Q(ITEM2),QY(ITEM2),J(PRIVATE),R(FIRST)\nPRIVATE CATTR NOTEXECUTABLE,ALIGN(2),RMODE(31)\n DS CL2\nITEM2 DC XL3'A1B2C3'\n ENTRY ITEM2\n END SECOND\n").unwrap();
    let second = hlasm::compile_file(&path, &[]).unwrap();
    let linked = hlasm::link_modules(&[first, second]).unwrap();
    let bytes = linked.storage_at(4096).unwrap();
    let get = |i: usize| {
        let q = &linked.bindings.quantities[i];
        &bytes[q.offset as usize..q.offset as usize + q.width as usize]
    };
    assert_eq!(get(0), [0, 0, 0, 1]);
    assert_eq!(get(1), [0, 1, 0]);
    let start = linked.entries["ITEM"] - 1;
    let end = linked.entries["ITEM2"] + 3;
    let displacement = linked.entries["ITEM2"] - start;
    assert_eq!(get(2), (end - start).to_be_bytes());
    assert_eq!(get(3), displacement.to_be_bytes());
    assert_eq!(
        get(4),
        [
            ((displacement >> 8) & 15) as u8,
            displacement as u8,
            (displacement >> 12) as u8
        ]
    );
    assert_eq!(get(5), (end - start).to_be_bytes());
    let psect = u32::from_be_bytes(get(6).try_into().unwrap());
    assert_eq!(psect, 4096 + start);
    assert_eq!(
        &bytes[(psect + displacement - 4096) as usize..(psect + displacement - 4096 + 3) as usize],
        &[0xa1, 0xb2, 0xc3]
    );
    let linked = hlasm::load_module(&hlasm::module_bytes(&linked).unwrap()).unwrap();
    assert_eq!(linked.storage_at(4096).unwrap(), bytes);
    std::fs::write(&path,"TEST CSECT\n BR 14\nOFF DC Q(INNER),J(PRIVATE)\nPRIVATE CATTR NOLOAD,NOTEXECUTABLE,PART(PART1),PRIORITY(1)\n DS CL1\nINNER DS CL2\nPRIVATE CATTR PART(PART2),PRIORITY(2)\n DS CL5\n END TEST\n").unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    let bytes = m.storage_at(0).unwrap();
    let q = &m.bindings.quantities[0];
    assert_eq!(
        &bytes[q.offset as usize..q.offset as usize + 4],
        &9u32.to_be_bytes()
    );
}

#[test]
fn class_part_identifier_namespaces_do_not_collide_with_embedded_at_signs() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("class-names.asm");
    std::fs::write(&path,"MAIN CSECT\n BR 14\nINFO DC Q(C,B@C,MAIN),J(A@B,A,MAIN)\nA@B CATTR NOLOAD,PART(C)\n DS CL3\nA CATTR NOLOAD,PART(B@C)\n DS CL7\nMAIN CATTR NOLOAD\n DS CL5\n END MAIN\n").unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    let bytes = m.storage_at(0).unwrap();
    for (index, value) in [0u32, 0, 0, 3, 7, 5].iter().enumerate() {
        let q = &m.bindings.quantities[index];
        assert_eq!(
            &bytes[q.offset as usize..q.offset as usize + 4],
            &value.to_be_bytes()
        );
    }
}

#[test]
fn external_psect_labels_resolve_to_their_class_start_and_q_reaches_the_real_field() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("root.asm");
    std::fs::write(&path,"ROOT RSECT\n BR 14\n EXTRN INTERIOR\nTABLES CATTR NOTEXECUTABLE,RMODE(31)\nREF DC R(ROOT),Q(INTERIOR),J(PRIVATE)\nROOT XATTR PSECT(INTERIOR)\n END ROOT\n").unwrap();
    let root = hlasm::compile_file(&path, &[]).unwrap();
    std::fs::write(&path,"OTHER CSECT\n BR 14\nPRIVATE CATTR NOTEXECUTABLE,ALIGN(4),RMODE(31)\nAREA DC F'0'\nINTERIOR DC F'27'\n ENTRY INTERIOR\n END OTHER\n").unwrap();
    let target = hlasm::compile_file(&path, &[]).unwrap();
    std::fs::write(
        dir.path().join("INTERIOR.zo"),
        hlasm::module_bytes(&target).unwrap(),
    )
    .unwrap();
    let linked = hlasm::link_with_libraries(vec![root], &[dir.path().into()]).unwrap();
    for base in [0, 4096, 65536] {
        let bytes = linked.storage_at(base).unwrap();
        let value = |i: usize| {
            let q = &linked.bindings.quantities[i];
            u32::from_be_bytes(
                bytes[q.offset as usize..q.offset as usize + 4]
                    .try_into()
                    .unwrap(),
            )
        };
        assert_eq!(
            value(0) + value(1),
            base as u32 + linked.entries["INTERIOR"]
        );
        assert_eq!(value(1), 4);
        assert_eq!(value(2), 8);
        let field = (value(0) + value(1) - base as u32) as usize;
        assert_eq!(&bytes[field..field + 4], &27u32.to_be_bytes());
    }
}

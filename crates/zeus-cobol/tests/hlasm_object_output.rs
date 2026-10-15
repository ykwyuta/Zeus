use zeus_cobol::{hlasm, vm::Machine};

fn compile(source: &str) -> hlasm::Module {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("output.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[]).unwrap()
}
fn roundtrip(module: &hlasm::Module) -> hlasm::Module {
    let bytes = hlasm::object_bytes(module).unwrap();
    assert!(bytes.len().is_multiple_of(80));
    assert_eq!(&bytes[..4], &[2, 0xc5, 0xe2, 0xc4]);
    assert_eq!(hlasm::object_bytes(module).unwrap(), bytes);
    let loaded = hlasm::load_module(&bytes).unwrap();
    assert!(loaded.code.is_empty());
    assert_eq!(loaded.storage.len(), module.storage.len());
    assert_eq!(loaded.entry, module.entry);
    loaded
}
fn execute(module: &hlasm::Module, base: usize) -> (i32, Vec<u8>) {
    let mut mem = vec![0; base];
    mem.extend(module.storage_at(base).unwrap());
    let rc = hlasm::execute(module, &mut mem, base, &[]).unwrap();
    (rc, mem)
}

#[test]
fn source_emits_architectural_instructions_and_exact_obj_record_fields() {
    let mut source = compile("SMALL CSECT\n LA 15,37\n BR 14\n END\n");
    source.storage.truncate(6);
    let bytes = hlasm::object_bytes(&source).unwrap();
    assert_eq!(bytes.len(), 240);
    assert_eq!(&bytes[10..16], &[0, 16, 0x40, 0x40, 0, 1]);
    assert_eq!(
        &bytes[16..24],
        &[0xe2, 0xd4, 0xc1, 0xd3, 0xd3, 0x40, 0x40, 0x40]
    );
    assert_eq!(&bytes[24..32], &[0, 0, 0, 0, 6, 0, 0, 6]);
    assert_eq!(&bytes[80..84], &[2, 0xe3, 0xe7, 0xe3]);
    assert_eq!(&bytes[90..96], &[0, 6, 0x40, 0x40, 0, 1]);
    // Independent known architectural byte sequence, not Zeus templates.
    assert_eq!(&bytes[96..102], &[0x41, 0xf0, 0, 37, 7, 0xfe]);
    assert_eq!(&bytes[160..164], &[2, 0xc5, 0xd5, 0xc4]);
    for base in [0, 4096, 2 << 20] {
        assert_eq!(execute(&roundtrip(&source), base).0, 37);
    }
}

#[test]
fn integer_branch_string_decimal_and_wide_encodings_execute_after_reloading() {
    let source = compile(
        "WORK CSECT\n USING WORK,15\n LR 12,14\n LA 2,3\n XR 3,3\nLOOP AR 3,2\n BRCT 2,LOOP\n CHI 3,6\n JNE BAD\n MVC OUT(4),INPUT\n CLC OUT(4),INPUT\n BNE BAD\n MVI OUT+1,C'Z'\n CLI OUT+1,C'Z'\n BNE BAD\n ZAP SUM(3),NUM(2)\n AP SUM(3),NUM(2)\n CP SUM(3),WANT(3)\n BNE BAD\n LGHI 4,17\n LGHI 5,5\n AGR 4,5\n CGHI 4,22\n JNE BAD\n LA 15,23\n BR 12\nBAD LA 15,99\n BR 12\nINPUT DC C'ABCD'\nOUT DS CL4\nNUM DC PL2'12'\nSUM DS PL3\nWANT DC PL3'24'\n END\n",
    );
    let object = roundtrip(&source);
    for base in [0, 4096, 2 << 20] {
        assert_eq!(execute(&source, base).0, 23);
        assert_eq!(execute(&object, base).0, 23);
    }
}

#[test]
fn hfp_and_signed_long_displacement_encodings_preserve_results() {
    let source = compile(
        "FORMATS CSECT\n USING FORMATS,15\n LR 12,14\n LD 0,FIRST\n AD 0,SECOND\n STD 0,RESULT\n CLC RESULT(8),WANT\n BNE BAD\n LER 2,0\n AER 2,2\n LA 2,BUFFER+8\n LGHI 3,-7\n STG 3,-8(2)\n LG 4,-8(2)\n CGHI 4,-7\n JNE BAD\n LGFI 5,123456\n AGFI 5,-123400\n CGFI 5,56\n JNE BAD\n LA 15,27\n BR 12\nBAD LA 15,99\n BR 12\nFIRST DC D'1.5'\nSECOND DC D'2.5'\nRESULT DS D\nWANT DC D'4'\nBUFFER DS D\n END\n",
    );
    let object = roundtrip(&source);
    for base in [0, 4096, 2 << 20] {
        assert_eq!(execute(&source, base).0, 27);
        assert_eq!(execute(&object, base).0, 27);
    }
}

#[test]
fn architectural_input_is_retained_and_reemitted_without_decoding_data_as_code() {
    let source = compile("RAW CSECT\n LA 15,33\n BR 14\n DC X'FFFFFFFFFF'\n END\n");
    let first = roundtrip(&source);
    let second = roundtrip(&first);
    assert_eq!(first.storage, second.storage);
    assert_eq!(execute(&second, 4096).0, 33);
}

#[test]
fn large_external_dictionary_crosses_the_ebcdic_blank_identifier_boundary() {
    let mut source = compile("MANY CSECT\n BR 14\n END\n");
    for n in 0..16_500 {
        source.externals.insert(format!("E{n:07}"), n % 2 == 0);
    }
    for count in [16_446, 16_447, 16_500] {
        let mut boundary = source.clone();
        boundary
            .externals
            .retain(|name, _| name[1..].parse::<usize>().unwrap() < count);
        let object = roundtrip(&boundary);
        assert_eq!(boundary.externals, object.externals);
    }
}

#[test]
fn entry_labels_text_record_boundaries_and_internal_addresses_survive_output() {
    let source = compile(
        "LABELS CSECT\n USING LABELS,15\n ENTRY SECOND\n LR 12,14\n LARL 2,TARGET\n L 3,PTR\n CR 2,3\n JNE BAD\n LA 15,11\n BR 12\nBAD LA 15,99\n BR 12\nSECOND LA 15,21\n BR 14\nPAD DC 80C'X'\nPTR DC A(TARGET)\nTARGET DC F'7'\n END SECOND\n",
    );
    let object = roundtrip(&source);
    assert_eq!(object.entries, source.entries);
    for base in [0, 4096, 2 << 20] {
        let expected = source.storage_at(base).unwrap();
        let actual = object.storage_at(base).unwrap();
        let at = source.relocations[0].offset as usize;
        assert_eq!(&actual[at..at + 4], &expected[at..at + 4]);
        assert_eq!(execute(&object, base).0, 21);
    }
    let mut first = object.clone();
    first.entry = 0;
    assert_eq!(execute(&first, 4096).0, 11);
}

#[test]
fn address_widths_external_weak_and_signed_terms_survive_binding() {
    let source = compile(
        "REFS CSECT\n EXTRN VALUE\n WXTRN ABSENT\n BR 14\n DC AL2(*),AL2(*),AL3(*),AL4(*),ADL5(*)\n DC ADL6(*),ADL7(*),AD(*)\n DC A(VALUE-REFS+7),AD(VALUE+3),A(ABSENT+5)\n END\n",
    );
    let mut source = source;
    source.zeus_hlasm = source.zeus_hlasm.max(7);
    source.relocations[0].width = 1;
    // Many repeated terms also exercise multiple RLD records.
    let expression = source.relocations.last_mut().unwrap();
    expression.expression = Some(hlasm::RelocationExpression {
        addend: -3,
        local_coefficient: -2,
        terms: vec![hlasm::RelativeTerm {
            symbol: "ABSENT".into(),
            coefficient: 3,
            weak: true,
        }],
    });
    expression.target = 0;
    expression.symbol = None;
    expression.addend = 0;
    expression.weak = false;
    let object = roundtrip(&source);
    assert_eq!(object.externals, source.externals);
    let value = compile("VALUE CSECT\n BR 14\n DC F'17'\n END\n");
    let original = hlasm::link_modules(&[source, value.clone()]).unwrap();
    let reloaded = hlasm::link_modules(&[object, value]).unwrap();
    for base in [0, 4096, 2 << 20] {
        let a = original.storage_at(base).unwrap();
        let b = reloaded.storage_at(base).unwrap();
        for r in &original.relocations {
            let at = r.offset as usize;
            assert_eq!(&a[at..at + r.width as usize], &b[at..at + r.width as usize]);
        }
    }
}

#[test]
fn amode24_31_64_rmode_and_uniform_rsect_are_retained() {
    for amode in [24, 31, 64] {
        for rmode in [24, 31] {
            let mut source = compile(&format!(
                "MODES RSECT\n AMODE {amode}\n RMODE {}\n LA 15,19\n BR 14\n END\n",
                if rmode == 24 { "24" } else { "ANY" }
            ));
            source.storage.truncate(source.read_only[0].end as usize);
            let object = roundtrip(&source);
            assert_eq!(object.amode, amode);
            assert_eq!(object.rmode, rmode);
            assert_eq!(object.read_only, source.read_only);
            assert_eq!(execute(&object, 4096).0, 19);
        }
    }
}

#[test]
fn emitted_obj_is_callable_with_shared_parameters_and_reusable_storage() {
    let dir = tempfile::tempdir().unwrap();
    let source = compile(
        "ADDVALUE CSECT\n L 2,0(1)\n L 3,4(1)\n L 4,0(2)\n AHI 4,7\n ST 4,0(3)\n LA 15,9\n BR 14\n END\n",
    );
    std::fs::write(
        dir.path().join("ADDVALUE.obj"),
        hlasm::object_bytes(&source).unwrap(),
    )
    .unwrap();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    let a = machine.getmain(8, 0);
    machine.storage_mut()[a..a + 4].copy_from_slice(&5i32.to_be_bytes());
    assert_eq!(
        machine
            .call_hlasm("ADDVALUE", &[Some(a), Some(a + 4)])
            .unwrap(),
        Ok(9)
    );
    assert_eq!(&machine.storage()[a + 4..a + 8], &12i32.to_be_bytes());
    let mark = machine.storage().len();
    for _ in 0..20 {
        assert_eq!(
            machine
                .call_hlasm("ADDVALUE", &[Some(a), Some(a + 4)])
                .unwrap(),
            Ok(9)
        );
        assert_eq!(machine.storage().len(), mark);
    }
}

#[test]
fn unsupported_metadata_and_instructions_are_diagnosed() {
    let mut private = compile("PRIVATE CSECT\n LA 15,1\nNEXT BR 14\n END NEXT\n");
    private.entries.clear();
    assert!(
        hlasm::object_bytes(&private)
            .unwrap_err()
            .contains("offset zero")
    );
    for (source, message) in [
        ("LONGMODULENAME CSECT\n BR 14\n END\n", "external name"),
        (
            "M CSECT\n LINK EP=CALLEE\n BR 14\n END\n",
            "macro expansion",
        ),
        (
            "M CSECT\n MVST 2,4\n BR 14\n END\n",
            "encoding is not implemented",
        ),
        (
            "M CSECT\n EXTRN CALLEE\n BRASL 14,CALLEE\n END\n",
            "linked before",
        ),
        (
            "M CSECT\n BR 14\nOTHER RSECT\n DC F'0'\n END M\n",
            "partially read-only",
        ),
        (
            "M CSECT\n BR 14\nOTHER CSECT\n AMODE 64\n BR 14\n END M\n",
            "mixed entry",
        ),
    ] {
        let error = hlasm::object_bytes(&compile(source)).unwrap_err();
        assert!(error.contains(message), "{error}");
    }
    let mut malformed = compile("M CSECT\n LA 15,1\n BR 14\n END\n");
    malformed.code[0].address.as_mut().unwrap().displacement = 4096;
    assert!(
        hlasm::object_bytes(&malformed)
            .unwrap_err()
            .contains("12-bit")
    );
    malformed.relocations.push(hlasm::Relocation {
        offset: 0,
        width: 4,
        target: 0,
        symbol: None,
        addend: 0,
        weak: false,
        expression: None,
    });
    malformed.code.clear();
    malformed.relocations.push(malformed.relocations[0].clone());
    assert!(
        hlasm::object_bytes(&malformed)
            .unwrap_err()
            .contains("overlapping")
    );
}

#[test]
fn cli_compiles_and_links_obj_and_preserves_existing_file_on_encoding_failure() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("simple.asm");
    std::fs::write(&path, "SIMPLE CSECT\n LA 15,37\n BR 14\n END\n").unwrap();
    let zas = env!("CARGO_BIN_EXE_zas");
    let compile = std::process::Command::new(zas)
        .args(["compile", "--output-format", "obj"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        compile.status.success(),
        "{}",
        String::from_utf8_lossy(&compile.stderr)
    );
    let object = path.with_extension("obj");
    let linked = dir.path().join("linked.obj");
    let link = std::process::Command::new(zas)
        .args(["link", "--output-format", "obj", "-o"])
        .arg(&linked)
        .arg(&object)
        .output()
        .unwrap();
    assert!(
        link.status.success(),
        "{}",
        String::from_utf8_lossy(&link.stderr)
    );
    assert_eq!(
        std::process::Command::new(zas)
            .arg("run")
            .arg(&linked)
            .status()
            .unwrap()
            .code(),
        Some(37)
    );
    std::fs::write(&path, "SIMPLE CSECT\n MVST 2,4\n BR 14\n END\n").unwrap();
    let before = std::fs::read(&object).unwrap();
    let failed = std::process::Command::new(zas)
        .args(["compile", "--output-format", "obj"])
        .arg(&path)
        .output()
        .unwrap();
    assert_eq!(failed.status.code(), Some(12));
    assert!(String::from_utf8_lossy(&failed.stderr).contains("OBJ output"));
    assert_eq!(std::fs::read(object).unwrap(), before);
    for command in ["check", "run"] {
        let output = std::process::Command::new(zas)
            .args([command, "--output-format", "obj"])
            .arg(&path)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(12));
        assert!(String::from_utf8_lossy(&output.stderr).contains("available for compile/link"));
    }
}

#[test]
fn floating_and_long_storage_fields_match_independent_architectural_bytes() {
    for (instruction, hex) in [
        ("ADBR 1,2", "B31A0012"),
        ("MAEBR 0,1,2", "B30E0012"),
        ("DIDBR 1,2,3,4", "B35B2413"),
        ("CFDBRA 3,6,0,4", "B3996430"),
        ("ADTR 0,2,4", "B3D24002"),
        ("QADTR 0,2,4,8", "B3F52804"),
        ("CPSDR 0,2,4", "B3722004"),
        ("CXFTR 4,3,6,0", "B9593046"),
        ("FIER 1,2", "B3770012"),
        ("SQDR 1,2", "B2440012"),
        ("LZER 3", "B3740030"),
        ("SFPC 3", "B3840030"),
        ("EFPC 4", "B38C0040"),
        ("IPM 5", "B2220050"),
        ("AEB 1,837(3,2)", "ED132345000A"),
        ("MAEB 0,2,837(3,4)", "ED234345000E"),
        ("SLDT 0,2,837(4)", "ED2043450040"),
        ("TCEB 1,837(2)", "ED1023450010"),
        ("CPDT 0,837(9,2),8", "ED08234508AC"),
        ("LFPC 837(2)", "B29D2345"),
        ("STFPC 837(2)", "B29C2345"),
        ("SRNMT 7", "B2B90007"),
        ("STMG 8,9,-8(2)", "EB892FF8FF24"),
        ("LMG 8,9,-8(2)", "EB892FF8FF04"),
        ("MVIY -8(2),42", "EB2A2FF8FF52"),
        ("STOCG 3,-8(2),4", "EB342FF8FFE3"),
    ] {
        let source = compile(&format!("BYTES CSECT\n {instruction}\n BR 14\n END\n"));
        let expected: Vec<_> = (0..hex.len())
            .step_by(2)
            .map(|n| u8::from_str_radix(&hex[n..n + 2], 16).unwrap())
            .collect();
        for bytes in [
            hlasm::object_bytes(&source).unwrap(),
            hlasm::goff_bytes(&source).unwrap(),
        ] {
            let loaded = hlasm::load_module(&bytes).unwrap();
            assert_eq!(&loaded.storage[..expected.len()], expected, "{instruction}");
        }
    }
}

#[test]
fn partial_immediate_opcodes_match_architecture_and_run_in_obj_and_goff() {
    let groups: &[(u8, u8, &[&str])] = &[
        (
            0xa5,
            0,
            &[
                "IIHH", "IIHL", "IILH", "IILL", "NIHH", "NIHL", "NILH", "NILL", "OIHH", "OIHL",
                "OILH", "OILL", "LLIHH", "LLIHL", "LLILH", "LLILL",
            ],
        ),
        (0xa7, 0, &["TMLH", "TMLL", "TMHH", "TMHL"]),
        (
            0xc0,
            6,
            &[
                "XIHF", "XILF", "IIHF", "IILF", "NIHF", "NILF", "OIHF", "OILF", "LLIHF", "LLILF",
            ],
        ),
    ];
    for &(prefix, start, names) in groups {
        for (n, name) in names.iter().enumerate() {
            let immediate = if prefix == 0xc0 { "305419896" } else { "4660" };
            let golden = compile(&format!("IMM CSECT\n {name} 2,{immediate}\n BR 14\n END\n"));
            let mut expected = vec![prefix, 0x20 | (start + n as u8)];
            if prefix == 0xc0 {
                expected.extend([0x12, 0x34, 0x56, 0x78]);
            } else {
                expected.extend([0x12, 0x34]);
            }
            let loaded = roundtrip(&golden);
            assert_eq!(&loaded.storage[..expected.len()], &expected, "{name}");
            let source = compile(&format!(
                "IMM CSECT\n USING IMM,15\n LGHI 2,-1\n {name} 2,{immediate}\n STG 2,RESULT\n IPM 3\n ST 3,FLAGS\n LA 15,17\n BR 14\nRESULT DS D\nFLAGS DS F\n END\n"
            ));
            let result = source.storage.len() - 12;
            for bytes in [
                hlasm::object_bytes(&source).unwrap(),
                hlasm::goff_bytes(&source).unwrap(),
            ] {
                let loaded = hlasm::load_module(&bytes).unwrap();
                for base in [0, 4096, 2 << 20] {
                    let (rc, expected) = execute(&source, base);
                    let (actual_rc, actual) = execute(&loaded, base);
                    assert_eq!(actual_rc, rc, "{name}");
                    assert_eq!(
                        &actual[base + result..],
                        &expected[base + result..],
                        "{name}"
                    );
                }
            }
        }
    }
}

#[test]
fn rie_rotate_and_conditional_immediates_keep_masks_and_control_bits() {
    for (op, sub) in [
        ("RISBG", 0x55),
        ("RISBGN", 0x59),
        ("RISBHG", 0x5d),
        ("RISBLG", 0x51),
        ("RNSBG", 0x54),
        ("ROSBG", 0x56),
        ("RXSBG", 0x57),
    ] {
        let source = compile(&format!("ROT CSECT\n {op} 2,3,8,159,4\n BR 14\n END\n"));
        assert_eq!(
            &roundtrip(&source).storage[..6],
            &[0xec, 0x23, 8, 159, 4, sub]
        );
    }
    for (op, sub) in [("LOCHI", 0x42), ("LOCGHI", 0x46), ("LOCHHI", 0x4e)] {
        let source = compile(&format!("COND CSECT\n {op} 2,-7,8\n BR 14\n END\n"));
        assert_eq!(
            &roundtrip(&source).storage[..6],
            &[0xec, 0x28, 0xff, 0xf9, 0, sub]
        );
    }
    let source = compile(
        "BITS CSECT\n USING BITS,15\n LGHI 2,-1\n LGHI 3,4660\n RISBGZ 2,3,48,63\n CGHI 2,4660\n JNE BAD\n LOCGHIE 2,-7\n CGHI 2,-7\n JNE BAD\n RISBGN 2,3,48,191,0\n LOCHINE 2,99\n CGHI 2,4660\n JNE BAD\n LA 15,43\n BR 14\nBAD LA 15,99\n BR 14\n END\n",
    );
    for bytes in [
        hlasm::object_bytes(&source).unwrap(),
        hlasm::goff_bytes(&source).unwrap(),
    ] {
        let loaded = hlasm::load_module(&bytes).unwrap();
        for base in [0, 4096, 2 << 20] {
            assert_eq!(execute(&source, base).0, 43);
            assert_eq!(execute(&loaded, base).0, 43);
        }
    }
}

#[test]
fn rrf_logic_selection_and_popcnt_have_independent_byte_and_execution_oracles() {
    let logic = [
        ("NRK", 0xf4),
        ("NGRK", 0xe4),
        ("ORK", 0xf6),
        ("OGRK", 0xe6),
        ("XRK", 0xf7),
        ("XGRK", 0xe7),
        ("NNRK", 0x74),
        ("NNGRK", 0x64),
        ("NORK", 0x76),
        ("NOGRK", 0x66),
        ("NXRK", 0x77),
        ("NXGRK", 0x67),
        ("NCRK", 0xf5),
        ("NCGRK", 0xe5),
        ("OCRK", 0x75),
        ("OCGRK", 0x65),
    ];
    for (op, sub) in logic {
        let source = compile(&format!("LOGIC CSECT\n {op} 2,3,4\n BR 14\n END\n"));
        assert_eq!(&roundtrip(&source).storage[..4], &[0xb9, sub, 0x40, 0x23]);
    }
    for (op, sub) in [("SELR", 0xf0), ("SELGR", 0xe3), ("SELFHR", 0xc0)] {
        let source = compile(&format!("SELECT CSECT\n {op} 2,3,4,8\n BR 14\n END\n"));
        assert_eq!(&roundtrip(&source).storage[..4], &[0xb9, sub, 0x48, 0x23]);
    }
    for (op, sub) in [
        ("LOCR", 0xf2),
        ("LOCGR", 0xe2),
        ("LOCFHR", 0xe0),
        ("POPCNT", 0xe1),
    ] {
        let source = compile(&format!("REG CSECT\n {op} 2,3,8\n BR 14\n END\n"));
        assert_eq!(&roundtrip(&source).storage[..4], &[0xb9, sub, 0x80, 0x23]);
    }
    let operations = logic.iter().map(|(op, _)| format!("{op} 2,3,4")).chain(
        [
            "SELR 2,3,4,8",
            "SELGR 2,3,4,7",
            "SELFHR 2,3,4,8",
            "LOCR 2,3,8",
            "LOCGR 2,3,7",
            "LOCFHR 2,3,8",
            "POPCNT 2,3,8",
            "POPCNT 2,3,0",
        ]
        .map(String::from),
    );
    for operation in operations {
        let source = compile(&format!(
            "EXEC CSECT\n USING EXEC,15\n LGHI 2,-1\n LGHI 3,4660\n LGHI 4,255\n CR 3,3\n {operation}\n STG 2,RESULT\n IPM 5\n ST 5,FLAGS\n LA 15,19\n BR 14\nRESULT DS D\nFLAGS DS F\n END\n"
        ));
        let result = source.storage.len() - 12;
        for bytes in [
            hlasm::object_bytes(&source).unwrap(),
            hlasm::goff_bytes(&source).unwrap(),
        ] {
            let loaded = hlasm::load_module(&bytes).unwrap();
            for base in [0, 4096] {
                let expected = execute(&source, base);
                let actual = execute(&loaded, base);
                assert_eq!(actual.0, 19);
                assert_eq!(
                    &actual.1[base + result..],
                    &expected.1[base + result..],
                    "{operation}"
                );
            }
        }
    }
}

#[test]
fn extended_rxy_fields_encode_signed_displacements_and_index_registers() {
    for (op, sub) in [
        ("CVBY", 0x06),
        ("CVDY", 0x26),
        ("LRVG", 0x0f),
        ("DSGF", 0x1d),
        ("LRV", 0x1e),
        ("LRVH", 0x1f),
        ("STRVG", 0x2f),
        ("STRV", 0x3e),
        ("STRVH", 0x3f),
        ("NY", 0x54),
        ("CLY", 0x55),
        ("OY", 0x56),
        ("XY", 0x57),
        ("ALY", 0x5e),
        ("SLY", 0x5f),
        ("STHY", 0x70),
        ("STCY", 0x72),
        ("ICY", 0x73),
        ("LB", 0x76),
        ("LGB", 0x77),
        ("LHY", 0x78),
        ("CHY", 0x79),
        ("AHY", 0x7a),
        ("SHY", 0x7b),
        ("MLG", 0x86),
        ("DLG", 0x87),
        ("ALCG", 0x88),
        ("SLBG", 0x89),
        ("LLGC", 0x90),
        ("LLGH", 0x91),
        ("LLC", 0x94),
        ("LLH", 0x95),
        ("ML", 0x96),
        ("DL", 0x97),
        ("ALC", 0x98),
        ("SLB", 0x99),
    ] {
        let source = compile(&format!("LONG CSECT\n {op} 2,-1(3,4)\n BR 14\n END\n"));
        assert_eq!(
            &roundtrip(&source).storage[..6],
            &[0xe3, 0x23, 0x4f, 0xff, 0xff, sub],
            "{op}"
        );
    }
    let source = compile(
        "REV CSECT\n USING REV,15\n LA 2,BUFFER+8\n LGFI 3,305419896\n STRV 3,-8(2)\n LRV 4,-8(2)\n CR 3,4\n JNE BAD\n NY 4,MASK\n CLY 4,WANT\n JNE BAD\n LB 5,BYTE\n CHI 5,-1\n JNE BAD\n LLC 5,BYTE\n CHI 5,255\n JNE BAD\n LLH 6,HALF\n LLILF 7,65534\n CR 6,7\n JNE BAD\n LHY 6,HALF\n AHI 6,7\n STHY 6,HALF\n CHY 6,POSITIVE\n JNE BAD\n LA 15,29\n BR 14\nBAD LA 15,99\n BR 14\nMASK DC X'00FF00FF'\nWANT DC X'00340078'\nBYTE DC X'FF'\nHALF DC H'-2'\nPOSITIVE DC H'5'\nBUFFER DS D\n END\n",
    );
    for bytes in [
        hlasm::object_bytes(&source).unwrap(),
        hlasm::goff_bytes(&source).unwrap(),
    ] {
        let loaded = hlasm::load_module(&bytes).unwrap();
        for base in [0, 4096] {
            assert_eq!(execute(&source, base).0, 29);
            assert_eq!(execute(&loaded, base).0, 29);
        }
    }
}

#[test]
fn logical_loads_preserve_upper_word_and_cc_and_decimal_y_forms_round_trip() {
    let source = compile(
        "LOW CSECT\n USING LOW,15\n LGHI 2,-1\n LA 4,1\n LTR 4,4\n LLC 2,BYTE\n STG 2,VALUE\n IPM 3\n ST 3,FLAGS\n CLC VALUE(8),WANTC\n JNE BAD\n CLC FLAGS(4),WANTCC\n JNE BAD\n LTR 4,4\n LLH 2,HALF\n STG 2,VALUE\n IPM 3\n ST 3,FLAGS\n CLC VALUE(8),WANTH\n JNE BAD\n CLC FLAGS(4),WANTCC\n JNE BAD\n LGHI 5,-12345\n CVDY 5,DECIMAL\n CVBY 6,DECIMAL\n CR 5,6\n JNE BAD\n LA 15,31\n BR 14\nBAD LA 15,99\n BR 14\nBYTE DC X'FF'\nHALF DC X'FFFE'\nWANTC DC X'FFFFFFFF000000FF'\nWANTH DC X'FFFFFFFF0000FFFE'\nWANTCC DC X'20000000'\nVALUE DS D\nFLAGS DS F\nDECIMAL DS D\n END\n",
    );
    for bytes in [
        hlasm::object_bytes(&source).unwrap(),
        hlasm::goff_bytes(&source).unwrap(),
    ] {
        let loaded = hlasm::load_module(&bytes).unwrap();
        for base in [0, 4096, 2 << 20] {
            assert_eq!(execute(&source, base).0, 31);
            assert_eq!(execute(&loaded, base).0, 31);
        }
    }
}

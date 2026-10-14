use zeus_cobol::hlasm;

fn compile(body: &str, setup: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("condition.asm");
    let body = body
        .lines()
        .map(|s| {
            if let Some(label) = s.strip_prefix('@') {
                label.to_string()
            } else {
                format!(" {s}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path,format!("TEST CSECT\n USING TEST,12\n LR 12,15\n L 10,0(1)\n LA 11,64(10)\n LMG 2,5,0(10)\n{setup}\n{body}\n BC 8,ZERO\n BC 4,ONE\n BC 2,TWO\n LA 15,3\n B SAVE\nZERO LA 15,0\n B SAVE\nONE LA 15,1\n B SAVE\nTWO LA 15,2\nSAVE STMG 2,5,0(10)\n BR 14\n END\n")).unwrap();
    hlasm::compile_file(&path, &[])
}

fn run(body: &str, cc: u8) -> Result<([u64; 4], Vec<u8>), String> {
    let setup = match cc {
        0 => " CR 7,7",
        1 => " LGHI 7,-1\n LTR 7,7",
        2 => " LA 7,1\n LTR 7,7",
        _ => " LGFI 7,2147483647\n AHI 7,1",
    };
    let module = compile(body, setup)?;
    let module = hlasm::load_module(&hlasm::module_bytes(&module)?)?;
    let regs: [u64; 4] = [0xaabbccdd11223344, 0, 0x5566778899aabbcc, 0];
    let mut mem = vec![0; 128];
    for (n, r) in regs.iter().enumerate() {
        mem[n * 8..n * 8 + 8].copy_from_slice(&r.to_be_bytes());
    }
    mem[64..72].copy_from_slice(&0x0102030405060708u64.to_be_bytes());
    mem.extend(module.storage_at(128)?);
    assert_eq!(
        hlasm::execute(&module, &mut mem, 128, &[Some(0)])?,
        i32::from(cc),
        "CC changed for {body}"
    );
    Ok((
        std::array::from_fn(|n| u64::from_be_bytes(mem[n * 8..n * 8 + 8].try_into().unwrap())),
        mem[64..72].to_vec(),
    ))
}

#[test]
fn conditional_loads_and_immediates_select_all_cc_masks_and_preserve_other_halves() {
    for cc in 0..4 {
        for mask in [0u8, 1, 2, 4, 8, 15] {
            for (op, operand, expected) in [
                ("LOC", "0(11)", 0xaabbccdd01020304),
                ("LOCG", "0(11)", 0x0102030405060708),
                ("LOCFH", "0(11)", 0x0102030411223344),
                ("LOCR", "4", 0xaabbccdd99aabbcc),
                ("LOCGR", "4", 0x5566778899aabbcc),
                ("LOCFHR", "4", 0x5566778811223344),
                ("LOCHI", "-1", 0xaabbccddffffffff),
                ("LOCGHI", "-1", u64::MAX),
                ("LOCHHI", "-1", 0xffffffff11223344),
            ] {
                let (regs, data) = run(&format!("{op} 2,{operand},{mask}"), cc).unwrap();
                assert_eq!(
                    regs[0],
                    if mask & (8 >> cc) != 0 {
                        expected
                    } else {
                        0xaabbccdd11223344
                    },
                    "{op} cc={cc} mask={mask}"
                );
                assert_eq!(regs[2], 0x5566778899aabbcc);
                assert_eq!(data, 0x0102030405060708u64.to_be_bytes());
            }
        }
    }
}

#[test]
fn conditional_stores_use_low_high_or_full_register_fields() {
    for cc in 0..4 {
        for (op, expected) in [
            ("STOC", 0x1122334405060708u64),
            ("STOCFH", 0xaabbccdd05060708),
            ("STOCG", 0xaabbccdd11223344),
        ] {
            let (regs, data) = run(&format!("{op} 2,0(11),8"), cc).unwrap();
            assert_eq!(regs[0], 0xaabbccdd11223344);
            assert_eq!(
                data,
                if cc == 0 {
                    expected
                } else {
                    0x0102030405060708
                }
                .to_be_bytes()
            );
        }
    }
}

#[test]
fn extended_condition_mnemonics_use_the_same_masks_as_base_forms() {
    for (suffix, mask) in [
        ("E", 8),
        ("Z", 8),
        ("NE", 7),
        ("NZ", 7),
        ("O", 1),
        ("NO", 14),
        ("H", 2),
        ("P", 2),
        ("NH", 13),
        ("NP", 13),
        ("L", 4),
        ("M", 4),
        ("NL", 11),
        ("NM", 11),
    ] {
        for cc in 0..4 {
            for (root, operand) in [
                ("LOC", "0(11)"),
                ("LOCR", "4"),
                ("LOCGHI", "1"),
                ("STOCFH", "0(11)"),
            ] {
                assert_eq!(
                    run(&format!("{root}{suffix} 2,{operand}"), cc).unwrap(),
                    run(&format!("{root} 2,{operand},{mask}"), cc).unwrap(),
                    "{root}{suffix} cc={cc}"
                );
            }
        }
    }
}

#[test]
fn execute_overlays_memory_and_immediate_masks_and_selects_conditional_rrf_opcodes() {
    for (body, expected) in [
        (
            "LA 6,40\nEX 6,TARGET\nB DONE\n@TARGET LOC 0,0(11),0\n@DONE BCR 0,0",
            0xaabbccdd01020304,
        ),
        (
            "LA 6,40\nEX 6,TARGET\nB DONE\n@TARGET LOCHI 0,-1,0\n@DONE BCR 0,0",
            0xaabbccddffffffff,
        ),
        (
            "LA 6,16\nEX 6,TARGET\nB DONE\n@TARGET LOCGR 2,4,8\n@DONE BCR 0,0",
            0xaabbccdd99aabbcc,
        ),
        (
            "LA 6,8\nEX 6,TARGET\nB DONE\n@TARGET LOCNE 2,0(11)\n@DONE BCR 0,0",
            0xaabbccdd01020304,
        ),
    ] {
        assert_eq!(run(body, 0).unwrap().0[0], expected);
    }
}

#[test]
fn invalid_masks_immediates_indices_and_unmapped_operands_are_rejected() {
    for body in [
        "LOC 2,0(11),16",
        "LOC 2,0(3,11),8",
        "LOCR 2,4,-1",
        "LOCHI 2,32768,8",
        "LOCHI 2,-32769,8",
        "LOCRE 2,4,8",
        "LOC 2,524288(11),8",
    ] {
        assert!(compile(body, " CR 7,7").is_err(), "{body}");
    }
    for body in ["LA 11,4000\nLOC 2,0(11),0", "LA 11,4000\nSTOCG 2,0(11),0"] {
        assert!(run(body, 0).is_err(), "{body}");
    }
}

#[test]
fn protected_storage_checks_include_identical_and_unselected_store_attempts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("readonly.asm");
    for op in [
        "STOC 2,VALUE,8",
        "STOC 2,VALUE,0",
        "CS 2,3,VALUE",
        "ST 2,VALUE",
        "STH 2,VALUE",
        "STC 2,VALUE",
        "MVI VALUE,0",
        "NI VALUE,255",
        "OI VALUE,0",
        "XI VALUE,0",
        "MVC VALUE(4),VALUE",
        "NC VALUE(4),VALUE",
        "OC VALUE(4),VALUE",
        "XC VALUE(4),VALUE",
        "STCM 2,15,VALUE",
        "STRV 2,VALUE",
        "STG 2,VALUE",
        "STMG 2,2,VALUE",
    ] {
        std::fs::write(&path,format!("TEST CSECT\n USING TEST,12\n LR 12,15\n XR 2,2\n {op}\n BR 14\nRO RSECT\nVALUE DC 2F'0'\n END\n")).unwrap();
        let module = hlasm::compile_file(&path, &[]).unwrap();
        let mut mem = module.storage_at(0).unwrap();
        let before = mem.clone();
        assert!(
            hlasm::execute(&module, &mut mem, 0, &[])
                .unwrap_err()
                .contains("protection exception"),
            "{op}"
        );
        assert_eq!(mem, before);
    }
}

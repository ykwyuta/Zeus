use zeus_cobol::hlasm;

fn compile(body: &str, data: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("relative.asm");
    let body = body
        .lines()
        .map(|line| {
            if let Some(label) = line.strip_prefix('@') {
                label.to_owned()
            } else {
                format!("         {}", line.trim())
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, format!("REL CSECT\n         LR 12,15\n         USING REL,12\n         L 10,0(1)\n         LG 2,INITIAL\n{body}\n         STMG 2,5,0(10)\n         LGHI 15,0\n         BR 14\nINITIAL DC XL8'AABBCCDD11223344'\n{data}\n         END REL\n")).unwrap();
    hlasm::compile_file(&path, &[])
}

fn execute(module: &hlasm::Module, base: usize) -> Result<[u64; 4], String> {
    let module = hlasm::load_module(&hlasm::module_bytes(module).unwrap()).unwrap();
    let mut mem = vec![0; base];
    mem.extend(module.storage_at(base).unwrap());
    let before = mem.len();
    assert_eq!(hlasm::execute(&module, &mut mem, base, &[Some(16)])?, 0);
    assert_eq!(mem.len(), before);
    Ok(std::array::from_fn(|n| {
        u64::from_be_bytes(mem[16 + n * 8..24 + n * 8].try_into().unwrap())
    }))
}

fn run(body: &str, data: &str) -> [u64; 4] {
    execute(&compile(body, data).unwrap(), 128).unwrap()
}

fn incoming_cc(cc: u8) -> &'static str {
    match cc {
        0 => "XR 5,5",
        1 => "LGHI 5,1\nNR 5,5",
        2 => "LGHI 5,1\nLTR 5,5",
        _ => "LGHI 5,-1\nALR 5,5",
    }
}

#[test]
fn relative_conditions_and_jump_aliases_cover_every_mask_and_preserve_cc() {
    for op in ["BRC", "BRCL"] {
        for mask in 0..=15 {
            for cc in 0..=3 {
                let result = run(
                    &format!(
                        "{}\n{op} {mask},TAKEN\nLGHI 2,1\nJG CHECK\n@TAKEN LGHI 2,2\n@CHECK LGHI 4,99\nBRC {},DONE\nJG ENDCC\n@DONE LGHI 4,1\n@ENDCC NOPR 0",
                        incoming_cc(cc),
                        8 >> cc
                    ),
                    "",
                );
                assert_eq!(
                    result[0],
                    if mask & (8 >> cc) != 0 { 2 } else { 1 },
                    "{op} mask {mask} CC {cc}"
                );
                assert_eq!(result[2], 1, "{op}: changed CC");
            }
        }
    }
    for prefix in ["J", "JG"] {
        for (suffix, mask) in [
            ("", 15),
            ("E", 8),
            ("Z", 8),
            ("NE", 7),
            ("NZ", 7),
            ("H", 2),
            ("P", 2),
            ("L", 4),
            ("M", 4),
            ("NH", 13),
            ("NP", 13),
            ("NL", 11),
            ("NM", 11),
            ("O", 1),
            ("NO", 14),
            ("NOP", 0),
        ] {
            for cc in 0..=3 {
                let op = format!("{prefix}{suffix}");
                let result = run(
                    &format!(
                        "{}\n{op} TAKEN\nLGHI 2,1\nJG DONE\n@TAKEN LGHI 2,2\n@DONE NOPR 0",
                        incoming_cc(cc)
                    ),
                    "",
                );
                assert_eq!(
                    result[0],
                    if mask & (8 >> cc) != 0 { 2 } else { 1 },
                    "{op} CC {cc}"
                );
            }
        }
    }
}

#[test]
fn relative_calls_save_next_address_mode_bit_and_upper_word_for_forward_and_backward_targets() {
    for op in ["BRAS", "BRASL"] {
        for backward in [false, true] {
            let prefix = if backward {
                "JG CALL\n@SUB BR 2\n@CALL NOPR 0\n"
            } else {
                ""
            };
            let suffix = if backward { "" } else { "@SUB BR 2\n" };
            let module = compile(&format!("{prefix}{op} 2,SUB\n@AFTER LARL 3,AFTER\nLGR 5,2\nNILF 2,X'7FFFFFFF'\nLLGFR 2,2\nSLGR 2,3\nJG DONE\n{suffix}@DONE NOPR 0"), "").unwrap();
            let call = module.code.iter().find(|i| i.op == op).unwrap();
            let next = 128 + u64::from(call.offset) + if op == "BRAS" { 4 } else { 6 };
            let result = execute(&module, 128).unwrap();
            assert_eq!(result[0], 0, "{op}: wrong return address");
            assert_eq!(result[1], next);
            assert_eq!(
                result[3],
                0xaabbccdd80000000 | next,
                "{op}: link format/upper word"
            );
        }
    }
}

#[test]
fn relative_counts_use_low_full_and_high_register_widths_with_wraparound() {
    for (op, setup, remaining) in [
        ("BRCT", "IILF 2,3", 0xaabbccdd00000000),
        ("BRCTG", "LGHI 2,3", 0),
        ("BRCTH", "IIHF 2,3", 0x11223344),
    ] {
        let result = run(
            &format!(
                "{setup}\nLGHI 3,0\n@LOOP AHI 3,1\n{op} 2,LOOP\nLGHI 4,99\nBRC 2,OK\nJG DONE\n@OK LGHI 4,1\n@DONE NOPR 0"
            ),
            "",
        );
        assert_eq!(result[0], remaining, "{op}: wrong register width");
        assert_eq!(result[1], 3);
        assert_eq!(result[2], 1, "{op}: changed CC");
    }
    for (op, setup, expected) in [
        ("BRCT", "IILF 2,0", 0xaabbccddffffffff),
        ("BRCTG", "LGHI 2,0", u64::MAX),
        ("BRCTH", "IIHF 2,0", 0xffffffff11223344),
    ] {
        let result = run(
            &format!("{setup}\n{op} 2,TAKEN\nLGHI 3,99\nJG DONE\n@TAKEN LGHI 3,1\n@DONE NOPR 0"),
            "",
        );
        assert_eq!(result[0], expected, "{op}: zero count must wrap");
        assert_eq!(result[1], 1);
    }
}

#[test]
fn register_and_storage_count_branches_support_suppression_aliases_and_full_width() {
    for op in ["BCTR", "BCTGR", "BCTG"] {
        let target = if op == "BCTG" { "0(6)" } else { "6" };
        let result = run(
            &format!("LGHI 2,3\nLGHI 3,0\nLARL 6,LOOP\n@LOOP AHI 3,1\n{op} 2,{target}"),
            "",
        );
        assert_eq!(&result[..2], &[0, 3], "{op}");
    }
    assert_eq!(run("BCTR 2,0", "")[0], 0xaabbccdd11223343);
    assert_eq!(run("BCTGR 2,0", "")[0], 0xaabbccdd11223343);
    assert_eq!(run("LGHI 2,0\nBCTGR 2,0", "")[0], u64::MAX);
    assert_eq!(run("LGHI 2,1\nBCTG 2,-1(0)", "")[0], 0);
    for op in ["BCTR", "BCTGR"] {
        let result = run(
            &format!(
                "LARL 2,TAKEN\n{op} 2,2\nLGHI 3,99\nJG DONE\n@TAKEN LGHI 3,1\n@DONE LARL 5,TAKEN"
            ),
            "",
        );
        assert_eq!(
            result[0] & 0x7fffffff,
            result[3] - 1,
            "{op}: target must be read before decrement"
        );
        assert_eq!(result[1], 1);
    }
}

#[test]
fn relative_address_loads_preserve_upper_word_and_do_not_access_storage() {
    let module = compile(
        "LARL 2,DATA\nLARL 3,DATA\nL 3,0(3)",
        "GAP DS 70000X\nDATA DC F'123'",
    )
    .unwrap();
    let ins = module.code.iter().find(|i| i.op == "LARL").unwrap();
    for base in [128, 1024] {
        let result = execute(&module, base).unwrap();
        let expected =
            base as u64 + u64::from(ins.offset) + ins.address.as_ref().unwrap().displacement as u64;
        assert_eq!(result[0], 0xaabbccdd00000000 | expected);
        assert_eq!(result[1], 123);
    }
    let module = compile("LARL 2,*+4294967294", "").unwrap();
    let ins = module.code.iter().find(|i| i.op == "LARL").unwrap();
    assert_eq!(
        execute(&module, 128).unwrap()[0],
        0xaabbccdd00000000 | ((128 + u64::from(ins.offset) + 4294967294) & 0x7fffffff)
    );
    assert_eq!(run("JG FAR\nDS 70000X\n@FAR LGHI 2,7", "")[0], 7);
}

#[test]
fn relative_targets_relocate_with_linked_images_and_can_cross_control_sections() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.asm");
    std::fs::write(&path, "MAIN CSECT\n         USING MAIN,15\n         LR 9,14\n         L 15,=V(REL)\n         BASR 14,15\n         LR 14,9\n         BR 14\n         END\n").unwrap();
    let main = hlasm::compile_file(&path, &[]).unwrap();
    let sub = compile(
        "BRAS 6,SUB\nJG DONE\n@SUB LARL 3,DATA\nL 2,0(3)\nBR 6\n@DONE NOPR 0",
        "OTHER CSECT\nDATA DC F'123'",
    )
    .unwrap();
    let linked = hlasm::link_modules(&[main, sub]).unwrap();
    for base in [128, 1024] {
        assert_eq!(execute(&linked, base).unwrap()[0], 0xaabbccdd0000007b);
    }
}

#[test]
fn relative_encodings_validate_alignment_ranges_sections_and_external_references() {
    for (op, prefix) in [
        ("BRC", "0,"),
        ("BRAS", "2,"),
        ("BRCT", "2,"),
        ("BRCTG", "2,"),
        ("JNOP", ""),
    ] {
        for distance in ["+65534", "-65536"] {
            compile(&format!("{op} {prefix}*{distance}"), "").unwrap();
        }
        for distance in ["+65536", "-65538"] {
            assert!(
                compile(&format!("{op} {prefix}*{distance}"), "")
                    .unwrap_err()
                    .contains("halfword displacement out of range")
            );
        }
    }
    for op in ["BRCL", "BRASL", "BRCTH", "LARL"] {
        let first = if op == "BRCL" { 0 } else { 2 };
        for distance in ["+4294967294", "-4294967296"] {
            compile(&format!("{op} {first},*{distance}"), "").unwrap();
        }
        assert!(
            compile(&format!("{op} {first},*+4294967296"), "")
                .unwrap_err()
                .contains("halfword displacement out of range")
        );
    }
    assert!(
        compile("J *+1", "")
            .unwrap_err()
            .contains("halfword aligned")
    );
    assert!(
        compile("BRC 16,*", "")
            .unwrap_err()
            .contains("invalid branch mask")
    );
    assert!(
        compile("J 2", "")
            .unwrap_err()
            .contains("assembled control sections")
    );
    assert!(
        compile("LARL 2,FIELD", "AREA DSECT\nFIELD DS F")
            .unwrap_err()
            .contains("assembled control sections")
    );
    let external = compile("EXTRN EXT\nBRASL 2,EXT", "").unwrap();
    assert!(external.code.iter().any(|i| i.relative_external.is_some()));
}

#[test]
fn relative_branch_errors_and_malformed_operands_reclaim_frames() {
    let mut modules = vec![compile("J DATA", "DATA DC F'1'").unwrap()];
    let mut odd = compile("J *+2", "").unwrap();
    odd.code
        .iter_mut()
        .find(|i| i.op == "J")
        .unwrap()
        .address
        .as_mut()
        .unwrap()
        .displacement = 1;
    modules.push(odd);
    let mut invalid = compile("BRCL 0,*", "").unwrap();
    invalid
        .code
        .iter_mut()
        .find(|i| i.op == "BRCL")
        .unwrap()
        .immediate = 16;
    modules.push(invalid);
    let mut indexed = compile("JNOP *", "").unwrap();
    indexed
        .code
        .iter_mut()
        .find(|i| i.op == "JNOP")
        .unwrap()
        .address
        .as_mut()
        .unwrap()
        .index = 1;
    modules.push(indexed);
    for module in modules {
        let mut mem = vec![0; 128];
        mem.extend(module.storage_at(128).unwrap());
        let before = mem.clone();
        assert!(hlasm::execute(&module, &mut mem, 128, &[Some(16)]).is_err());
        assert_eq!(mem, before);
    }
    run("JNOP *+65534\nJGNOP *-4294967296", "");
}

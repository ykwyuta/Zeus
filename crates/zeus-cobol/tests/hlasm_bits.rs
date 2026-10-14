use zeus_cobol::hlasm;

fn compile(body: &str, data: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bits.asm");
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
    std::fs::write(&path, format!("BITS CSECT\n         USING BITS,12\n         LR 12,15\n         L 10,0(1)\n{body}\n         STMG 2,5,0(10)\n         LGHI 15,0\n         BR 14\n{data}\n         END\n")).unwrap();
    hlasm::compile_file(&path, &[])
}

fn run(body: &str, data: &str) -> [u64; 4] {
    let module = compile(body, data).unwrap();
    let module = hlasm::load_module(&hlasm::module_bytes(&module).unwrap()).unwrap();
    let mut mem = vec![0; 128];
    mem.extend(module.storage_at(128).unwrap());
    let before = mem.len();
    assert_eq!(
        hlasm::execute(&module, &mut mem, 128, &[Some(16)]).unwrap(),
        0
    );
    assert_eq!(mem.len(), before);
    std::array::from_fn(|n| u64::from_be_bytes(mem[16 + n * 8..24 + n * 8].try_into().unwrap()))
}

fn check_cc(body: &str, cc: u8) -> String {
    format!(
        "{body}\nLGHI 4,99\nBC {},OK\nB DONE\n@OK LGHI 4,1\n@DONE NOPR 0",
        8 >> cc
    )
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
fn carry_and_borrow_use_the_cc_high_bit_for_all_input_codes_and_operand_widths() {
    for wide in [false, true] {
        let max = if wide { u64::MAX } else { u64::from(u32::MAX) };
        // Explicit boundary expectations include the zero-result/borrow case.
        let additions = [
            (0, 0, false, 0, 0),
            (0, 0, true, 1, 1),
            (max, 0, true, 0, 2),
            (max, 1, true, 1, 3),
            (0, max, true, 0, 2),
            (max, max, true, max, 3),
            (max, 1, false, 0, 2),
            (1, 1, false, 2, 1),
        ];
        let subtractions = [
            (0, 0, false, max, 1),
            (0, max, false, 0, 0),
            (0, 0, true, 0, 2),
            (max, max, false, max, 1),
            (max, max, true, 0, 2),
            (1, 0, false, 0, 2),
            (1, 0, true, 1, 3),
            (0, 1, true, max, 1),
            (max, 0, false, max - 1, 3),
        ];
        for subtract in [false, true] {
            let cases = if subtract {
                &subtractions[..]
            } else {
                &additions[..]
            };
            for register in [false, true] {
                let op = match (wide, subtract, register) {
                    (false, false, false) => "ALC",
                    (false, false, true) => "ALCR",
                    (true, false, false) => "ALCG",
                    (true, false, true) => "ALCGR",
                    (false, true, false) => "SLB",
                    (false, true, true) => "SLBR",
                    (true, true, false) => "SLBG",
                    (true, true, true) => "SLBGR",
                };
                for &(a, b, carry, expected, cc) in cases {
                    for previous in 0..=3 {
                        if (previous >= 2) != carry {
                            continue;
                        }
                        let full_a = if wide { a } else { 0x1122334400000000 | a };
                        let full_b = if wide { b } else { 0x5566778800000000 | b };
                        let operand = if register {
                            "3"
                        } else if wide {
                            "INPUTB"
                        } else {
                            "B32"
                        };
                        let body = check_cc(
                            &format!(
                                "LG 2,INPUTA\nLG 3,INPUTB\n{}\n{op} 2,{operand}",
                                incoming_cc(previous)
                            ),
                            cc,
                        );
                        let result = run(
                            &body,
                            &format!(
                                "GAP DS 5000X\nINPUTA DC XL8'{full_a:016X}'\nINPUTB DC XL8'{full_b:016X}'\nB32 DC XL4'{:08X}'",
                                b as u32
                            ),
                        );
                        assert_eq!(
                            result[0],
                            if wide {
                                expected
                            } else {
                                0x1122334400000000 | expected
                            },
                            "{op}: {a} {b} previous CC {previous}"
                        );
                        assert_eq!(result[1], full_b, "{op}: source changed");
                        assert_eq!(
                            result[2], 1,
                            "{op}: wrong CC for {a} {b} previous {previous}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn carry_chains_propagate_across_128_bits_and_allow_aliased_operands() {
    let result = run(
        "LGHI 2,-1\nLGHI 3,-1\nLGHI 5,1\nALGR 3,5\nALCG 2,=FD'0'",
        "",
    );
    assert_eq!(&result[..2], &[0, 0]);
    let result = run("LGHI 2,0\nLGHI 3,0\nLGHI 5,1\nSLGR 3,5\nSLBG 2,=FD'0'", "");
    assert_eq!(&result[..2], &[u64::MAX, u64::MAX]);
    assert_eq!(run("LGHI 2,-1\nXR 5,5\nALCGR 2,2", "")[0], u64::MAX - 1);
    assert_eq!(run("LGHI 2,7\nLGHI 5,1\nLTR 5,5\nSLBGR 2,2", "")[0], 0);
    assert_eq!(
        run("LGHI 2,7\nXR 5,5\nSLBR 2,2", "")[0],
        u64::from(u32::MAX)
    );
}

#[test]
fn immediate_insert_load_and_logical_operations_select_the_correct_register_fields() {
    let cases = [
        ("IIHH", "X'ABCD'", 0xabcd334455667788),
        ("IIHL", "X'ABCD'", 0x1122abcd55667788),
        ("IILH", "X'ABCD'", 0x11223344abcd7788),
        ("IILL", "X'ABCD'", 0x112233445566abcd),
        ("IIHF", "X'89ABCDEF'", 0x89abcdef55667788),
        ("IILF", "X'89ABCDEF'", 0x1122334489abcdef),
        ("LLIHH", "X'ABCD'", 0xabcd000000000000),
        ("LLIHL", "X'ABCD'", 0x0000abcd00000000),
        ("LLILH", "X'ABCD'", 0x00000000abcd0000),
        ("LLILL", "X'ABCD'", 0x000000000000abcd),
        ("LLIHF", "X'89ABCDEF'", 0x89abcdef00000000),
        ("LLILF", "X'89ABCDEF'", 0x0000000089abcdef),
        ("NIHH", "X'F0F0'", 0x1020334455667788),
        ("NIHL", "X'F0F0'", 0x1122304055667788),
        ("NILH", "X'F0F0'", 0x1122334450607788),
        ("NILL", "X'F0F0'", 0x1122334455667080),
        ("NIHF", "X'F0F0F0F0'", 0x1020304055667788),
        ("NILF", "X'F0F0F0F0'", 0x1122334450607080),
        ("OIHH", "X'0F0F'", 0x1f2f334455667788),
        ("OIHL", "X'0F0F'", 0x11223f4f55667788),
        ("OILH", "X'0F0F'", 0x112233445f6f7788),
        ("OILL", "X'0F0F'", 0x1122334455667f8f),
        ("OIHF", "X'0F0F0F0F'", 0x1f2f3f4f55667788),
        ("OILF", "X'0F0F0F0F'", 0x112233445f6f7f8f),
        ("XIHF", "X'11223344'", 0x0000000055667788),
        ("XILF", "X'55667788'", 0x1122334400000000),
    ];
    for (op, immediate, expected) in cases {
        let body = format!("LG 2,VALUE\n{op} 2,{immediate}");
        assert_eq!(
            run(&body, "VALUE DC XL8'1122334455667788'")[0],
            expected,
            "{op}"
        );
        let module = compile(&body, "VALUE DC XL8'1122334455667788'").unwrap();
        let n = module.code.iter().position(|i| i.op == op).unwrap();
        assert_eq!(
            module.code[n + 1].offset - module.code[n].offset,
            if op.ends_with('F') { 6 } else { 4 }
        );
    }
    assert_eq!(
        run(
            "LLIHH 2,X'FEDC'\nIIHL 2,X'BA98'\nIILH 2,X'7654'\nIILL 2,X'3210'",
            ""
        )[0],
        0xfedcba9876543210
    );
    assert_eq!(
        run("LLIHF 2,X'FEDCBA98'\nIILF 2,X'76543210'", "")[0],
        0xfedcba9876543210
    );
}

#[test]
fn logical_immediate_condition_codes_depend_only_on_the_modified_field() {
    for (suffix, shift, mask) in [
        ("HH", 48, 0xffff),
        ("HL", 32, 0xffff),
        ("LH", 16, 0xffff),
        ("LL", 0, 0xffff),
        ("HF", 32, 0xffffffff),
        ("LF", 0, 0xffffffff),
    ] {
        for prefix in ["NI", "OI"] {
            let op = format!("{prefix}{suffix}");
            let original = u64::MAX ^ (mask << shift);
            for (immediate, cc, expected) in [
                (0, 0, original),
                (
                    1,
                    if prefix == "OI" { 1 } else { 0 },
                    if prefix == "OI" {
                        original | (1 << shift)
                    } else {
                        original
                    },
                ),
            ] {
                let result = run(
                    &check_cc(&format!("LG 2,VALUE\n{op} 2,{immediate}"), cc),
                    &format!("VALUE DC XL8'{original:016X}'"),
                );
                assert_eq!(result[0], expected, "{op}");
                assert_eq!(result[2], 1, "{op}: wrong field CC");
            }
        }
    }
    for (op, value, immediate) in [
        ("XIHF", 0x1122334455667788u64, 0x11223344u32),
        ("XILF", 0x1122334455667788, 0x55667788),
    ] {
        let result = run(
            &check_cc(&format!("LG 2,VALUE\n{op} 2,{immediate}"), 0),
            &format!("VALUE DC XL8'{value:016X}'"),
        );
        assert_ne!(result[0], 0);
        assert_eq!(result[2], 1);
    }
}

#[test]
fn insert_and_logical_load_immediates_preserve_all_incoming_condition_codes() {
    for op in [
        "IIHH", "IIHL", "IILH", "IILL", "IIHF", "IILF", "LLIHH", "LLIHL", "LLILH", "LLILL",
        "LLIHF", "LLILF",
    ] {
        for cc in 0..=3 {
            let result = run(
                &check_cc(&format!("{}\n{op} 2,123", incoming_cc(cc)), cc),
                "",
            );
            assert_eq!(result[2], 1, "{op}: incoming CC {cc}");
        }
    }
}

#[test]
fn register_test_masks_cover_all_codes_and_use_the_leftmost_selected_bit() {
    for (op, shift) in [("TMHH", 48), ("TMHL", 32), ("TMLH", 16), ("TMLL", 0)] {
        for (field, mask, cc) in [
            (0u64, 3, 0),
            (1, 3, 1),
            (2, 3, 2),
            (3, 3, 3),
            (0x2000, 0x6000, 1),
            (0x4000, 0x6000, 2),
            (0xffff, 0, 0),
            (0x0100, 0x0101, 2),
            (0x0001, 0x0101, 1),
            (0xffff, 0xffff, 3),
        ] {
            let value = !(0xffffu64 << shift) | (field << shift);
            let result = run(
                &check_cc(&format!("LG 2,VALUE\n{op} 2,{mask}"), cc),
                &format!("VALUE DC XL8'{value:016X}'"),
            );
            assert_eq!(result[0], value, "{op}: changed the register");
            assert_eq!(result[2], 1, "{op}: field={field:x} mask={mask:x}");
        }
    }
}

#[test]
fn immediate_bit_patterns_ranges_and_absolute_expressions_are_checked() {
    for op in ["IIHH", "LLILL", "NIHL", "OILH", "TMLL"] {
        compile(&format!("{op} 2,65535"), "").unwrap();
        for immediate in ["65536", "-1", "X'10000'"] {
            assert!(
                compile(&format!("{op} 2,{immediate}"), "")
                    .unwrap_err()
                    .contains("invalid unsigned immediate")
            );
        }
    }
    for op in ["IIHF", "LLILF", "NIHF", "OILF", "XIHF"] {
        compile(&format!("{op} 2,4294967295"), "").unwrap();
        compile(&format!("{op} 2,-1"), "").unwrap();
        for immediate in ["4294967296", "-2147483649"] {
            assert!(
                compile(&format!("{op} 2,{immediate}"), "")
                    .unwrap_err()
                    .contains("invalid unsigned immediate")
            );
        }
    }
    assert!(
        compile("IILL 2,DATA", "DATA DC H'1'")
            .unwrap_err()
            .contains("immediate must be absolute")
    );
    assert_eq!(run("LLILL 2,(2+3)*7", "")[0], 35);
    assert_eq!(run("LLILF 2,X'FFFFFFFF'", "")[0], u64::from(u32::MAX));
    assert_eq!(run("LLILF 2,-1", "")[0], u64::from(u32::MAX));
    assert_eq!(run("LLIHF 2,-2147483648", "")[0], 1 << 63);
    assert_eq!(run("LLILL 2,L'VALUE", "VALUE DC FD'1'")[0], 8);
}

#[test]
fn carry_memory_access_and_malformed_immediates_report_errors_and_reclaim_frames() {
    for op in ["ALC", "ALCG", "SLB", "SLBG"] {
        let module = compile(&format!("{op} 2,524287(0)"), "").unwrap();
        let mut mem = vec![0; 128];
        mem.extend(module.storage_at(128).unwrap());
        let before = mem.clone();
        assert!(
            hlasm::execute(&module, &mut mem, 128, &[Some(16)])
                .unwrap_err()
                .contains("outside run-unit storage")
        );
        assert_eq!(mem, before);
        assert!(
            compile(&format!("{op} 2,-524289(4)"), "")
                .unwrap_err()
                .contains("displacement out of range")
        );
    }
    let mut module = compile("IILL 2,0", "").unwrap();
    module
        .code
        .iter_mut()
        .find(|i| i.op == "IILL")
        .unwrap()
        .immediate = 65536;
    let mut mem = vec![0; 128];
    mem.extend(module.storage_at(128).unwrap());
    let before = mem.clone();
    assert!(
        hlasm::execute(&module, &mut mem, 128, &[Some(16)])
            .unwrap_err()
            .contains("invalid unsigned immediate")
    );
    assert_eq!(mem, before);
}

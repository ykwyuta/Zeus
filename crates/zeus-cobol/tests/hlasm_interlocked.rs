use zeus_cobol::hlasm;

fn compile(body: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("interlocked.asm");
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
    std::fs::write(&path, format!("TEST CSECT\n USING TEST,12\n LR 12,15\n L 10,0(1)\n LA 11,64(10)\n LMG 2,7,0(10)\n{body}\n BC 8,ZERO\n BC 4,ONE\n BC 2,TWO\n LA 15,3\n B SAVE\nZERO LA 15,0\n B SAVE\nONE LA 15,1\n B SAVE\nTWO LA 15,2\nSAVE STMG 2,7,0(10)\n BR 14\n END\n")).unwrap();
    hlasm::compile_file(&path, &[])
}

fn run(body: &str, registers: [u64; 6], data: &[u8]) -> Result<(i32, [u64; 6], Vec<u8>), String> {
    let module = compile(body)?;
    let module = hlasm::load_module(&hlasm::module_bytes(&module)?)?;
    let mut mem = vec![0u8; 128];
    for (n, v) in registers.iter().enumerate() {
        mem[n * 8..n * 8 + 8].copy_from_slice(&v.to_be_bytes());
    }
    mem[64..64 + data.len()].copy_from_slice(data);
    mem.extend(module.storage_at(128)?);
    let before = mem.len();
    let rc = hlasm::execute(&module, &mut mem, 128, &[Some(0)])?;
    assert_eq!(mem.len(), before);
    Ok((
        rc,
        std::array::from_fn(|n| u64::from_be_bytes(mem[n * 8..n * 8 + 8].try_into().unwrap())),
        mem[64..80].to_vec(),
    ))
}

#[test]
fn compare_and_swap_updates_only_the_selected_width_and_preserves_upper_words() {
    for op in ["CS", "CSY", "CSG", "CDS", "CDSY", "CDSG"] {
        let double = op.starts_with("CDS");
        let wide = matches!(op, "CSG" | "CDSG");
        let width = if op == "CDSG" {
            16
        } else if double || wide {
            8
        } else {
            4
        };
        let regs: [u64; 6] = [
            0xaabbccdd11223344,
            0x9988776655667788,
            0x0102030412345678,
            0x0506070823456789,
            0,
            0,
        ];
        let expected = if wide {
            regs[0].to_be_bytes().to_vec()
        } else if double {
            [0x11223344u32.to_be_bytes(), 0x55667788u32.to_be_bytes()].concat()
        } else {
            0x11223344u32.to_be_bytes().to_vec()
        };
        let expected = if op == "CDSG" {
            [expected, regs[1].to_be_bytes().to_vec()].concat()
        } else {
            expected
        };
        let replacement = if wide {
            regs[2].to_be_bytes().to_vec()
        } else if double {
            [0x12345678u32.to_be_bytes(), 0x23456789u32.to_be_bytes()].concat()
        } else {
            0x12345678u32.to_be_bytes().to_vec()
        };
        let replacement = if op == "CDSG" {
            [replacement, regs[3].to_be_bytes().to_vec()].concat()
        } else {
            replacement
        };
        let (cc, out, data) = run(&format!("{op} 2,4,0(11)"), regs, &expected).unwrap();
        assert_eq!(cc, 0, "{op}");
        assert_eq!(out, regs, "{op}");
        assert_eq!(&data[..width], replacement, "{op}");
        let (cc, out, data) = run(&format!("{op} 2,4,0(11)"), regs, &[0x55; 16]).unwrap();
        assert_eq!(cc, 1, "{op}");
        assert_eq!(data, [0x55; 16]);
        assert_eq!(
            out[0],
            if wide {
                0x5555555555555555
            } else {
                0xaabbccdd55555555
            },
            "{op}"
        );
        assert_eq!(
            out[1],
            if op == "CDSG" {
                0x5555555555555555
            } else if double {
                0x9988776655555555
            } else {
                regs[1]
            },
            "{op}"
        );
        assert_eq!(out[2..], regs[2..]);
    }
}

#[test]
fn interlocked_fetch_operations_return_the_original_value_and_set_result_cc() {
    for (op, input, arg, expected, cc) in [
        ("LAA", 7, 5, 12u32, 2),
        ("LAA", 0x7fffffff, 1, 0x80000000, 3),
        ("LAA", 0xffffffff, 0, 0xffffffff, 1),
        ("LAA", 1, 0xffffffff, 0, 0),
        ("LAAL", 0xffffffff, 1, 0, 2),
        ("LAAL", 0xffffffff, 2, 1, 3),
        ("LAN", 7, 8, 0, 0),
        ("LAO", 7, 8, 15, 1),
        ("LAX", 7, 3, 4, 1),
    ] {
        let regs = [0xaabbccdd00000000, 0, arg, 0, 0, 0];
        let (result_cc, out, data) = run(
            &format!("{op} 2,4,0(11)"),
            regs,
            &(input as u32).to_be_bytes(),
        )
        .unwrap();
        assert_eq!(result_cc, cc, "{op}");
        assert_eq!(out[0], 0xaabbccdd00000000 | input, "{op}");
        assert_eq!(out[2], arg);
        assert_eq!(&data[..4], expected.to_be_bytes(), "{op}");
    }
    for (op, input, arg, expected, cc) in [
        ("LAAG", 0x7fffffffffffffff, 1, 0x8000000000000000, 3),
        ("LAAG", u64::MAX, 0, u64::MAX, 1),
        ("LAALG", u64::MAX, 1, 0, 2),
        ("LAALG", u64::MAX, 2, 1, 3),
        (
            "LANG",
            0xf000000000000000,
            0x1000000000000000,
            0x1000000000000000,
            1,
        ),
        ("LAOG", 0xf000000000000000, 3, 0xf000000000000003, 1),
        ("LAXG", u64::MAX, u64::MAX, 0, 0),
    ] {
        let (result_cc, out, data) = run(
            &format!("{op} 2,4,0(11)"),
            [0, 0, arg, 0, 0, 0],
            &input.to_be_bytes(),
        )
        .unwrap();
        assert_eq!(result_cc, cc, "{op}");
        assert_eq!(out[0], input, "{op}");
        assert_eq!(&data[..8], expected.to_be_bytes(), "{op}");
    }
}

#[test]
fn fetch_and_compare_operations_capture_overlapping_registers_before_update() {
    let (cc, out, data) = run("LAA 2,2,0(11)", [9, 0, 0, 0, 0, 0], &5u32.to_be_bytes()).unwrap();
    assert_eq!((cc, out[0]), (2, 5));
    assert_eq!(&data[..4], 14u32.to_be_bytes());
    let (cc, out, data) = run(
        "CDS 2,2,0(11)",
        [9, 7, 0, 0, 0, 0],
        &[0, 0, 0, 9, 0, 0, 0, 7],
    )
    .unwrap();
    assert_eq!((cc, out[0], out[1]), (0, 9, 7));
    assert_eq!(&data[..8], &[0, 0, 0, 9, 0, 0, 0, 7]);
}

#[test]
fn test_and_set_uses_the_high_bit_and_execute_ignores_its_second_byte() {
    for (byte, cc) in [(0, 0), (0x7f, 0), (0x80, 1), (0xff, 1)] {
        let (result_cc, _, data) = run("TS 0(11)", [0; 6], &[byte]).unwrap();
        assert_eq!(result_cc, cc);
        assert_eq!(data[0], 0xff);
    }
    let (cc, _, data) = run(
        "LA 2,255\nEX 2,TARGET\nB DONE\n@TARGET TS 0(11)\n@DONE BCR 0,0",
        [0; 6],
        &[0],
    )
    .unwrap();
    assert_eq!(cc, 0);
    assert_eq!(data[0], 0xff);
}

#[test]
fn long_displacements_and_execute_register_overlays_are_honored() {
    let (cc, _, data) = run(
        "LA 11,72(10)\nCSY 2,4,-8(11)",
        [5, 0, 7, 0, 0, 0],
        &5u32.to_be_bytes(),
    )
    .unwrap();
    assert_eq!(cc, 0);
    assert_eq!(&data[..4], 7u32.to_be_bytes());
    let (cc, out, data) = run(
        "LA 6,36\nEX 6,TARGET\nB DONE\n@TARGET CS 0,0,0(11)\n@DONE BCR 0,0",
        [5, 0, 7, 0, 0, 0],
        &5u32.to_be_bytes(),
    )
    .unwrap();
    assert_eq!(cc, 0);
    assert_eq!(out[0], 5);
    assert_eq!(&data[..4], 7u32.to_be_bytes());
}

#[test]
fn invalid_pairs_alignment_indices_and_bounds_do_not_modify_storage() {
    for body in [
        "CDS 3,4,0(11)",
        "CDSG 2,5,0(11)",
        "CS 2,4,0(3,11)",
        "CS 2,4,-1(11)",
        "CS 2,4,4096(11)",
        "CSY 2,4,524288(11)",
    ] {
        assert!(compile(body).is_err(), "{body}");
    }
    for body in [
        "CS 2,4,1(11)",
        "CDS 2,4,4(11)",
        "CSG 2,4,4(11)",
        "CDSG 2,4,8(11)",
        "LAAG 2,4,4(11)",
        "LA 11,4000\nCS 2,4,0(11)",
    ] {
        assert!(run(body, [0; 6], &[0; 16]).is_err(), "{body}");
    }
    let m = compile("CS 2,4,0(11)").unwrap();
    let mut mem = vec![0; 128];
    mem.extend(m.storage_at(128).unwrap());
    let before = mem.len();
    let mut bad = m.clone();
    bad.code
        .iter_mut()
        .find(|i| i.op == "CS")
        .unwrap()
        .registers[0] = 17;
    assert!(hlasm::execute(&bad, &mut mem, 128, &[Some(0)]).is_err());
    assert_eq!(mem.len(), before);
}

#[test]
fn load_pair_disjoint_captures_both_addresses_before_overlapping_register_updates() {
    let data = [
        0x1122334455667788u64.to_be_bytes(),
        0x99aabbccddeeff00u64.to_be_bytes(),
    ]
    .concat();
    let registers = [0xaabbccdd00000040, 0x9988776600000048, 0, 0, 0, 0];
    let (cc, regs, after) = run("LPD 2,0(2),0(3)", registers, &data).unwrap();
    assert_eq!(cc, 0);
    assert_eq!(regs[0], 0xaabbccdd11223344);
    assert_eq!(regs[1], 0x9988776699aabbcc);
    assert_eq!(after, data);
    let (cc, regs, after) = run("LPDG 2,0(2),0(3)", registers, &data).unwrap();
    assert_eq!(cc, 0);
    assert_eq!(&regs[..2], &[0x1122334455667788, 0x99aabbccddeeff00]);
    assert_eq!(after, data);
    let (_, regs, _) = run("LPD 2,0(11),0(11)", registers, &data).unwrap();
    assert_eq!(regs[0] as u32, regs[1] as u32);
}

#[test]
fn load_pair_disjoint_execute_can_select_the_wide_ssf_opcode() {
    let data = [
        0x1122334455667788u64.to_be_bytes(),
        0x99aabbccddeeff00u64.to_be_bytes(),
    ]
    .concat();
    let (cc, regs, after) = run(
        "LGHI 6,1\nEX 6,TARGET\nB DONE\n@TARGET LPD 4,0(11),8(11)\n@DONE NOPR 0",
        [0; 6],
        &data,
    )
    .unwrap();
    assert_eq!(cc, 0);
    assert_eq!(&regs[2..4], &[0x1122334455667788, 0x99aabbccddeeff00]);
    assert_eq!(after, data);
    for source in [
        "LPD 3,0(11),4(11)",
        "LPDG 15,0(11),8(11)",
        "LPD 2,0(2,11),4(11)",
        "LPD 2,4096(11),4(11)",
    ] {
        assert!(compile(source).is_err(), "{source}");
    }
    for source in [
        "LPD 2,1(11),4(11)",
        "LPDG 2,0(11),4(11)",
        "LPDG 2,0(11),4095",
    ] {
        assert!(run(source, [0; 6], &data).is_err(), "{source}");
    }
}

fn run_csst(
    body: &str,
    registers: [u64; 6],
    first: [u8; 16],
    parameters: [u8; 32],
) -> Result<(i32, [u64; 6], Vec<u8>), String> {
    let m = compile(body)?;
    let mut memory = vec![0u8; 256];
    for (index, value) in registers.iter().enumerate() {
        memory[index * 8..index * 8 + 8].copy_from_slice(&value.to_be_bytes());
    }
    memory[64..80].copy_from_slice(&first);
    memory[80..96].fill(0x55);
    memory[96..128].copy_from_slice(&parameters);
    memory.extend(m.storage_at(256)?);
    let before = memory.len();
    let rc = hlasm::execute(&m, &mut memory, 256, &[Some(0)])?;
    assert_eq!(memory.len(), before);
    Ok((
        rc,
        std::array::from_fn(|n| u64::from_be_bytes(memory[n * 8..n * 8 + 8].try_into().unwrap())),
        memory[64..128].to_vec(),
    ))
}

#[test]
fn compare_swap_and_store_supports_every_compare_and_store_width() {
    let first = [
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0,
    ];
    let parameters = std::array::from_fn(|n| 0xa0 + n as u8);
    for function in 0..3 {
        for characteristic in 0..5 {
            let width = 4 << function;
            let count = 1 << characteristic;
            for equal in [true, false] {
                let mut registers = [0; 6];
                registers[0] = if function == 0 {
                    0xaabbccdd00000000
                        | u64::from(u32::from_be_bytes(first[..4].try_into().unwrap()))
                } else {
                    u64::from_be_bytes(first[..8].try_into().unwrap())
                };
                registers[1] = u64::from_be_bytes(first[8..].try_into().unwrap());
                if !equal {
                    registers[0] ^= 1;
                }
                let body = format!(
                    "LGHI 0,{}\nLA 1,111\nCSST 64,80,2",
                    characteristic * 256 + function
                );
                let (cc, regs, after) = run_csst(&body, registers, first, parameters).unwrap();
                assert_eq!(cc, if equal { 0 } else { 1 });
                if equal {
                    assert_eq!(&after[..width], &parameters[..width]);
                    assert_eq!(&after[width..16], &first[width..]);
                    assert_eq!(&after[16..16 + count], &parameters[16..16 + count]);
                    assert_eq!(&after[16 + count..32], &vec![0x55; 16 - count]);
                    assert_eq!(regs, registers);
                } else {
                    assert_eq!(&after[..16], &first);
                    assert_eq!(&after[16..32], &[0x55; 16]);
                    assert_eq!(
                        regs[0],
                        if function == 0 {
                            0xaabbccdd00000000
                                | u64::from(u32::from_be_bytes(first[..4].try_into().unwrap()))
                        } else {
                            u64::from_be_bytes(first[..8].try_into().unwrap())
                        }
                    );
                    if function == 2 {
                        assert_eq!(regs[1], u64::from_be_bytes(first[8..].try_into().unwrap()));
                    }
                }
                assert_eq!(&after[32..], &parameters);
            }
        }
    }
}

#[test]
fn compare_swap_and_store_captures_parameter_aliases_and_orders_overlapping_stores() {
    let first = [0x11; 16];
    let parameters = std::array::from_fn(|n| 0xa0 + n as u8);
    let registers = [0x11111111, 0, 0, 0, 0, 0];
    let (_, _, after) = run_csst(
        "LGHI 0,1024\nLA 1,96\nCSST 64,64,2",
        registers,
        first,
        parameters,
    )
    .unwrap();
    assert_eq!(&after[..16], &parameters[16..32]);
    let registers = [
        u64::from(u32::from_be_bytes(parameters[16..20].try_into().unwrap())),
        0,
        0,
        0,
        0,
        0,
    ];
    let (_, _, after) = run_csst(
        "LGHI 0,1024\nLA 1,96\nCSST 112,80,2",
        registers,
        first,
        parameters,
    )
    .unwrap();
    assert_eq!(&after[16..32], &parameters[16..32]);
    assert_eq!(&after[48..52], &parameters[..4]);
}

#[test]
fn compare_swap_and_store_rejects_invalid_functions_alignment_and_register_pairs() {
    for body in [
        "LGHI 0,3\nLA 1,96\nCSST 64,80,2",
        "LGHI 0,1280\nLA 1,96\nCSST 64,80,2",
        "LGHI 0,2\nLA 1,96\nCSST 64,80,3",
        "LGHI 0,0\nLA 1,96\nCSST 65,80,2",
        "LGHI 0,1024\nLA 1,96\nCSST 64,81,2",
        "LGHI 0,0\nIILF 1,2147418112\nCSST 64,80,2",
    ] {
        assert!(run_csst(body, [0; 6], [0; 16], [0; 32]).is_err(), "{body}");
    }
    assert!(compile("CSST 0(2,11),0(11),2").is_err());
}

#[test]
fn compare_swap_and_store_protection_failure_suppresses_both_stores() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("csst-protection.asm");
    std::fs::write(&path,"TEST CSECT\n USING TEST,12\n LR 12,15\n LA 1,PARAM\n LGHI 0,0\n XR 2,2\n LARL 4,RO\n USING RO,4\n CSST VALUE,READONLY,2\n BR 14\nVALUE DC F'0'\n CNOP 0,16\nPARAM DC F'7',3F'0'\n DC X'00'\nRO RSECT\nREADONLY DC X'00'\n END TEST\n").unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    let mut memory = m.storage_at(0).unwrap();
    let before = memory.clone();
    assert!(
        hlasm::execute(&m, &mut memory, 0, &[])
            .unwrap_err()
            .contains("protection exception")
    );
    assert_eq!(memory, before);
}

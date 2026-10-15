use zeus_cobol::hlasm;

fn compile(body: &str, data: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("masked.asm");
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
    std::fs::write(&path, format!("MASKED CSECT\n         USING MASKED,12\n         LR 12,15\n         L 10,0(1)\n         LG 2,INITIAL\n{body}\n         STMG 2,5,0(10)\n         LGHI 15,0\n         BR 14\nINITIAL DC XL8'0123456789ABCDEF'\n{data}\n         END\n")).unwrap();
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

const PACKED: [&[u8]; 16] = [
    &[],
    &[0xef],
    &[0xcd],
    &[0xcd, 0xef],
    &[0xab],
    &[0xab, 0xef],
    &[0xab, 0xcd],
    &[0xab, 0xcd, 0xef],
    &[0x89],
    &[0x89, 0xef],
    &[0x89, 0xcd],
    &[0x89, 0xcd, 0xef],
    &[0x89, 0xab],
    &[0x89, 0xab, 0xef],
    &[0x89, 0xab, 0xcd],
    &[0x89, 0xab, 0xcd, 0xef],
];

fn data_prefix(op: &str) -> &'static str {
    if op.ends_with(['Y', 'H']) {
        "GAP DS 5000X\n"
    } else {
        ""
    }
}

#[test]
fn insert_masks_cover_all_sixteen_byte_combinations_and_both_register_halves() {
    let expected = [
        0x89abcdefu32,
        0x89abcd10,
        0x89ab10ef,
        0x89ab1020,
        0x8910cdef,
        0x8910cd20,
        0x891020ef,
        0x89102030,
        0x10abcdef,
        0x10abcd20,
        0x10ab20ef,
        0x10ab2030,
        0x1020cdef,
        0x1020cd30,
        0x102030ef,
        0x10203040,
    ];
    for op in ["ICM", "ICMY", "ICMH"] {
        for (mask, &word) in expected.iter().enumerate() {
            let initial = if op == "ICMH" {
                "IIHF 2,X'89ABCDEF'\nIILF 2,X'01234567'\n"
            } else {
                ""
            };
            let body = check_cc(
                &format!("{initial}{op} 2,{mask},DATA"),
                if mask == 0 { 0 } else { 2 },
            );
            let data = format!("{}DATA DC X'10203040'", data_prefix(op));
            let result = run(&body, &data);
            assert_eq!(
                result[0],
                if op == "ICMH" {
                    (u64::from(word) << 32) | 0x01234567
                } else {
                    0x0123456700000000 | u64::from(word)
                },
                "{op} mask {mask}"
            );
            assert_eq!(result[2], 1, "{op} mask {mask}: wrong CC");
            let module = compile(&body, &data).unwrap();
            let n = module.code.iter().position(|i| i.op == op).unwrap();
            assert_eq!(
                module.code[n + 1].offset - module.code[n].offset,
                if op == "ICM" { 4 } else { 6 }
            );
        }
    }
}

#[test]
fn insert_condition_codes_ignore_preserved_bytes_and_use_first_storage_byte_sign() {
    for op in ["ICM", "ICMY", "ICMH"] {
        for (bytes, cc) in [
            ("00000000", 0),
            ("80000000", 1),
            ("00800000", 2),
            ("7F000000", 2),
        ] {
            let result = run(
                &check_cc(&format!("{op} 2,5,DATA"), cc),
                &format!("DATA DC X'{bytes}'"),
            );
            assert_eq!(result[2], 1, "{op}: {bytes}");
        }
    }
    assert_eq!(
        run(
            "LR 2,12\nAHI 2,DATA-MASKED\nICM 2,15,0(2)",
            "DATA DC X'10203040'"
        )[0],
        0x0123456710203040
    );
}

#[test]
fn stores_pack_selected_bytes_contiguously_without_changing_register_or_cc() {
    for op in ["STCM", "STCMY", "STCMH"] {
        for (mask, bytes) in PACKED.iter().enumerate() {
            let initial = if op == "STCMH" {
                "IIHF 2,X'89ABCDEF'\nIILF 2,X'01234567'\n"
            } else {
                ""
            };
            let body = check_cc(
                &format!("{initial}LGHI 5,1\nLTR 5,5\n{op} 2,{mask},DATA\nLG 3,DATA"),
                2,
            );
            let result = run(
                &body,
                &format!("{}DATA DC XL8'AAAAAAAAAAAAAAAA'", data_prefix(op)),
            );
            let mut expected = [0xaa; 8];
            expected[..bytes.len()].copy_from_slice(bytes);
            assert_eq!(result[1], u64::from_be_bytes(expected), "{op} mask {mask}");
            assert_eq!(
                result[0],
                if op == "STCMH" {
                    0x89abcdef01234567
                } else {
                    0x0123456789abcdef
                }
            );
            assert_eq!(result[2], 1, "{op} mask {mask}: changed CC");
        }
    }
}

#[test]
fn compare_uses_unsigned_packed_byte_order_for_all_masks_and_preserves_registers() {
    for op in ["CLM", "CLMY", "CLMH"] {
        for (mask, bytes) in PACKED.iter().enumerate() {
            let initial = if op == "CLMH" {
                "IIHF 2,X'89ABCDEF'\nIILF 2,X'01234567'\n"
            } else {
                ""
            };
            for comparison in [0i8, -1, 1] {
                if bytes.is_empty() && comparison != 0 {
                    continue;
                }
                let mut memory = [0u8; 4];
                memory[..bytes.len()].copy_from_slice(bytes);
                if !bytes.is_empty() {
                    memory[0] = (i16::from(memory[0]) + i16::from(comparison)) as u8;
                }
                let hex = memory
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<String>();
                let result = run(
                    &check_cc(
                        &format!("{initial}{op} 2,{mask},DATA"),
                        if comparison < 0 {
                            2
                        } else if comparison > 0 {
                            1
                        } else {
                            0
                        },
                    ),
                    &format!("{}DATA DC X'{hex}'", data_prefix(op)),
                );
                assert_eq!(
                    result[0],
                    if op == "CLMH" {
                        0x89abcdef01234567
                    } else {
                        0x0123456789abcdef
                    }
                );
                assert_eq!(result[2], 1, "{op} mask {mask} compare {comparison}");
            }
        }
    }
}

#[test]
fn masked_accesses_read_only_selected_count_and_zero_masks_still_check_one_byte() {
    for op in [
        "ICM", "ICMY", "ICMH", "STCM", "STCMY", "STCMH", "CLM", "CLMY", "CLMH",
    ] {
        // One argument means R1+3 is the final accessible byte of the frame.
        // A one-byte mask must not read or write an entire fullword here.
        run(&format!("LR 6,1\nAHI 6,3\n{op} 2,1,0(6)"), "");
        for (offset, mask) in [(4, 0), (3, 3)] {
            let module =
                compile(&format!("LR 6,1\nAHI 6,{offset}\n{op} 2,{mask},0(6)"), "").unwrap();
            let mut mem = vec![0; 128];
            mem.extend(module.storage_at(128).unwrap());
            let before = mem.clone();
            assert!(
                hlasm::execute(&module, &mut mem, 128, &[Some(16)])
                    .unwrap_err()
                    .contains("outside run-unit storage"),
                "{op} offset {offset} mask {mask}"
            );
            assert_eq!(mem, before);
        }
    }
}

#[test]
fn masked_operand_formats_and_persisted_masks_are_validated() {
    for op in [
        "ICM", "ICMY", "ICMH", "STCM", "STCMY", "STCMH", "CLM", "CLMY", "CLMH",
    ] {
        for mask in ["-1", "16", "256", "DATA"] {
            assert!(
                compile(&format!("{op} 2,{mask},0(6)"), "DATA DS X")
                    .unwrap_err()
                    .contains("four-bit byte mask")
            );
        }
        assert!(
            compile(&format!("{op} 2,1,0(3,6)"), "")
                .unwrap_err()
                .contains("index not permitted")
        );
        if op.ends_with(['Y', 'H']) {
            for displacement in ["524287", "-524288"] {
                compile(&format!("{op} 2,1,{displacement}(6)"), "").unwrap();
            }
            assert!(
                compile(&format!("{op} 2,1,524288(6)"), "")
                    .unwrap_err()
                    .contains("displacement out of range")
            );
        } else {
            assert!(
                compile(&format!("{op} 2,1,4096(6)"), "")
                    .unwrap_err()
                    .contains("displacement out of range")
            );
        }
    }
    let mut module = compile("ICM 2,0,0(6)", "").unwrap();
    module
        .code
        .iter_mut()
        .find(|i| i.op == "ICM")
        .unwrap()
        .immediate = 16;
    let mut mem = vec![0; 128];
    mem.extend(module.storage_at(128).unwrap());
    let before = mem.clone();
    assert!(
        hlasm::execute(&module, &mut mem, 128, &[Some(16)])
            .unwrap_err()
            .contains("four-bit byte mask")
    );
    assert_eq!(mem, before);
}

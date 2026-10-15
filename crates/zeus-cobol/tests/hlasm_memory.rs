use zeus_cobol::hlasm;

fn compile(body: &str, data: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("memory.asm");
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
    std::fs::write(&path, format!("MEMORY CSECT\n         USING MEMORY,12\n         LR 12,15\n         L 10,0(1)\n         LG 2,UPPER\n{body}\n         STMG 2,3,0(10)\n         LGHI 15,0\n         BR 14\nUPPER DC XL8'AABBCCDD11223344'\n{data}\n         END\n")).unwrap();
    hlasm::compile_file(&path, &[])
}

fn run(body: &str, data: &str) -> [u64; 2] {
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

const FAR: &str = "GAP DS 5000X\nVALUE DC F'12'\nPOS DC F'5'\nNEG DC F'-3'\nHALF DC H'-3'\nBYTE DC X'AB'\nPACKED DC PL8'-123456'\nSAVE DS 4F";

#[test]
fn long_displacement_register_memory_operations_use_low_words_and_real_six_byte_sizes() {
    let cases: &[(&str, u32)] = &[
        ("LY 2,VALUE", 12),
        ("STY 2,SAVE\nLY 3,SAVE\nLR 2,3", 0x11223344),
        ("LHY 2,HALF", (-3i32) as u32),
        ("STHY 2,SAVE\nLHY 2,SAVE", 0x3344),
        ("ICY 2,BYTE", 0x112233ab),
        ("STCY 2,SAVE\nXR 2,2\nICY 2,SAVE", 0x44),
        ("LY 2,VALUE\nAY 2,POS", 17),
        ("LY 2,VALUE\nSY 2,POS", 7),
        ("LY 2,VALUE\nALY 2,POS", 17),
        ("LY 2,VALUE\nSLY 2,POS", 7),
        ("LY 2,VALUE\nNY 2,POS", 4),
        ("LY 2,VALUE\nOY 2,POS", 13),
        ("LY 2,VALUE\nXY 2,POS", 9),
        ("LY 2,VALUE\nAHY 2,HALF", 9),
        ("LY 2,VALUE\nSHY 2,HALF", 15),
        ("CVBY 2,PACKED", (-123456i32) as u32),
        ("LY 2,VALUE\nCVDY 2,SAVE\nXR 2,2\nCVBY 2,SAVE", 12),
    ];
    for (body, expected) in cases {
        assert_eq!(
            run(body, FAR)[0],
            0xaabbccdd00000000 | u64::from(*expected),
            "{body}"
        );
        let module = compile(body, FAR).unwrap();
        for (n, ins) in module
            .code
            .iter()
            .enumerate()
            .filter(|(_, i)| i.op.ends_with('Y'))
        {
            assert_eq!(module.code[n + 1].offset - ins.offset, 6, "{} size", ins.op);
            assert!(ins.address.as_ref().unwrap().displacement > 4095);
        }
    }
}

#[test]
fn long_displacement_signed_unsigned_and_halfword_compares_and_overflow_codes() {
    let cases = [
        "LGHI 2,-1\nCY 2,VALUE\nBL OK",
        "LGHI 2,-1\nCLY 2,VALUE\nBH OK",
        "LGHI 2,-3\nCHY 2,HALF\nBE OK",
        "LGFI 2,2147483647\nAY 2,POS\nBO OK",
        "LGFI 2,-2147483648\nSY 2,POS\nBO OK",
        "LGHI 2,-1\nALY 2,ONE\nBC 2,OK",
        "LGHI 2,0\nSLY 2,ONE\nBC 4,OK",
        "LGHI 2,0\nNY 2,POS\nBE OK",
        "LGHI 2,0\nOY 2,POS\nBC 4,OK",
        "LGHI 2,5\nXY 2,POS\nBE OK",
    ];
    for body in cases {
        assert_eq!(
            run(
                &format!("{body}\nLGHI 2,99\nB DONE\n@OK LGHI 2,1\n@DONE NOPR 0"),
                &format!("{FAR}\nONE DC F'1'")
            )[0],
            1,
            "{body}"
        );
    }
}

#[test]
fn long_displacement_byte_immediates_modify_storage_and_report_conditions() {
    for (body, expected) in [
        ("MVIY BYTE,X'55'\nXR 2,2\nICY 2,BYTE", 0x55),
        ("NIY BYTE,X'0F'\nXR 2,2\nICY 2,BYTE", 0x0b),
        ("OIY BYTE,X'50'\nXR 2,2\nICY 2,BYTE", 0xfb),
        ("XIY BYTE,X'FF'\nXR 2,2\nICY 2,BYTE", 0x54),
    ] {
        assert_eq!(run(body, FAR)[0], 0xaabbccdd00000000 | expected, "{body}");
    }
    for body in [
        "CLIY BYTE,X'AB'\nBE OK",
        "TMY BYTE,X'F0'\nBC 4,OK",
        "TMY BYTE,X'80'\nBC 1,OK",
        "TMY BYTE,0\nBE OK",
        "NIY BYTE,0\nBE OK",
        "OIY BYTE,1\nBC 4,OK",
        "XIY BYTE,X'AB'\nBE OK",
    ] {
        assert_eq!(
            run(
                &format!("{body}\nLGHI 2,99\nB DONE\n@OK LGHI 2,1\n@DONE NOPR 0"),
                FAR
            )[0],
            1,
            "{body}"
        );
    }
}

#[test]
fn long_displacement_multiple_register_access_wraps_and_captures_original_base() {
    assert_eq!(
        run(
            "LY 2,VALUE\nLY 3,NEG\nSTMY 2,3,SAVE\nXR 2,2\nLMY 2,3,SAVE",
            FAR
        ),
        [0xaabbccdd0000000c, 0x00000000fffffffd]
    );
    assert_eq!(
        run("LR 2,12\nAHI 2,VALUE-MEMORY\nLMY 2,3,0(2)", FAR),
        [0xaabbccdd0000000c, 5]
    );
    assert_eq!(
        run(
            "LGHI 0,7\nSTMY 14,1,SAVE\nLGHI 0,99\nLMY 14,1,SAVE\nLGR 2,0",
            FAR
        )[0],
        7
    );
}

#[test]
fn long_displacements_support_negative_indexed_addresses_and_enforce_format_limits() {
    assert_eq!(
        run("LR 4,12\nAHI 4,VALUE-MEMORY+16\nLY 2,-16(4)", FAR)[0],
        0xaabbccdd0000000c
    );
    assert_eq!(
        run("LR 4,12\nAHI 4,VALUE-MEMORY-1\nLGHI 5,1\nLY 2,0(5,4)", FAR)[0],
        0xaabbccdd0000000c
    );
    for op in ["LY", "STY", "MVIY", "CLIY", "NIY", "OIY", "XIY", "TMY"] {
        let operands = if op.ends_with("IY") || op == "TMY" {
            "0(4,5),1"
        } else {
            "2,524288(4)"
        };
        let error = compile(&format!("{op} {operands}"), "").unwrap_err();
        assert!(
            error.contains("index not permitted") || error.contains("displacement out of range"),
            "{error}"
        );
    }
    for operand in ["524287(4)", "-524288(4)"] {
        compile(&format!("LY 2,{operand}"), "").unwrap();
    }
    for operand in ["524288(4)", "-524289(4)"] {
        assert!(
            compile(&format!("LY 2,{operand}"), "")
                .unwrap_err()
                .contains("displacement out of range")
        );
    }
    assert!(
        compile("L 2,4096(4)", "")
            .unwrap_err()
            .contains("displacement out of range")
    );
    assert!(
        compile("LMY 2,3,0(4,5)", "")
            .unwrap_err()
            .contains("index not permitted")
    );
    assert!(
        compile("MVIY 0(4),256", "")
            .unwrap_err()
            .contains("invalid byte immediate")
    );
}

#[test]
fn reversed_loads_preserve_unmodified_bits_and_reverse_only_their_operand_width() {
    let data = "GAP DS 5000X\nBYTES DC XL8'0123456789ABCDEF'";
    assert_eq!(run("LRVH 2,BYTES", data)[0], 0xaabbccdd11222301);
    assert_eq!(run("LRV 2,BYTES", data)[0], 0xaabbccdd67452301);
    assert_eq!(run("LRVG 2,BYTES", data)[0], 0xefcdab8967452301);
    assert_eq!(
        run("LG 3,SOURCE\nLRVR 2,3", "SOURCE DC XL8'0123456789ABCDEF'")[0],
        0xaabbccddefcdab89
    );
    assert_eq!(
        run("LG 3,SOURCE\nLRVGR 2,3", "SOURCE DC XL8'0123456789ABCDEF'")[0],
        0xefcdab8967452301
    );
    assert_eq!(run("LRVR 2,2", "")[0], 0xaabbccdd44332211);
    assert_eq!(run("LRVGR 2,2", "")[0], 0x44332211ddccbbaa);
}

#[test]
fn reversed_stores_write_exact_width_and_keep_adjacent_sentinel_bytes() {
    let data = "GAP DS 5000X\nDEST DC XL8'AAAAAAAAAAAAAAAA'";
    for (op, expected) in [("STRVH", 0xaa4433aaaaaaaaaa), ("STRV", 0xaa44332211aaaaaa)] {
        assert_eq!(
            run(&format!("{op} 2,DEST+1\nLG 2,DEST"), data)[0],
            expected,
            "{op}"
        );
    }
    assert_eq!(run("STRVG 2,DEST\nLG 2,DEST", data)[0], 0x44332211ddccbbaa);
    for (store, load, expected) in [
        ("STRVH", "LRVH", 0xaabbccdd11223344),
        ("STRV", "LRV", 0xaabbccdd11223344),
        ("STRVG", "LRVG", 0xaabbccdd11223344),
    ] {
        assert_eq!(
            run(&format!("{store} 2,DEST\n{load} 2,DEST"), data)[0],
            expected
        );
    }
}

#[test]
fn signed_byte_loads_extend_to_the_correct_register_width_without_changing_cc() {
    let data = "GAP DS 5000X\nNEG DC X'80'\nPOS DC X'7F'";
    assert_eq!(run("LB 2,NEG", data)[0], 0xaabbccddffffff80);
    assert_eq!(run("LGB 2,NEG", data)[0], (-128i64) as u64);
    assert_eq!(run("LB 2,POS", data)[0], 0xaabbccdd0000007f);
    assert_eq!(run("LGB 2,POS", data)[0], 127);
    assert_eq!(run("LGHI 3,128\nLBR 2,3", "")[0], 0xaabbccddffffff80);
    assert_eq!(run("LGHI 3,128\nLGBR 2,3", "")[0], (-128i64) as u64);
    assert_eq!(run("LBR 2,2", "")[0], 0xaabbccdd00000044);
    assert_eq!(run("LGBR 2,2", "")[0], 0x44);
    for op in [
        "LRVH 2,NEG",
        "LRV 2,NEG",
        "LRVG 2,NEG",
        "LRVR 2,2",
        "LRVGR 2,2",
        "STRVH 2,DEST",
        "STRV 2,DEST",
        "STRVG 2,DEST",
        "LB 2,NEG",
        "LGB 2,NEG",
        "LBR 2,2",
        "LGBR 2,2",
        "LY 2,VALUE",
        "STY 2,DEST",
        "LHY 2,NEG",
        "STHY 2,DEST",
        "ICY 2,NEG",
        "STCY 2,DEST",
        "MVIY DEST,1",
        "LMY 2,3,VALUE",
        "STMY 2,3,DEST",
        "CVDY 2,DEST",
        "CVBY 2,PACKED",
    ] {
        assert_eq!(
            run(
                &format!(
                    "LGHI 3,1\nCGHI 3,0\n{op}\nBH OK\nLGHI 2,99\nB DONE\n@OK LGHI 2,1\n@DONE NOPR 0"
                ),
                "GAP DS 5000X\nVALUE DC F'12,5'\nNEG DC XL8'8000000000000000'\nPACKED DC PL8'123'\nDEST DS 8X"
            )[0],
            1,
            "{op}: changed CC"
        );
    }
}

#[test]
fn long_and_reversed_memory_bounds_errors_preserve_storage_and_reclaim_frames() {
    for body in [
        "LY 2,524287(0)",
        "STY 2,524287(0)",
        "MVIY 524287(0),1",
        "LMY 2,3,524287(0)",
        "STMY 2,3,524287(0)",
        "LRVH 2,524287(0)",
        "LRV 2,524287(0)",
        "LRVG 2,524287(0)",
        "STRVH 2,524287(0)",
        "STRV 2,524287(0)",
        "STRVG 2,524287(0)",
        "LB 2,524287(0)",
        "LGB 2,524287(0)",
    ] {
        let module = compile(body, "").unwrap();
        let mut mem = vec![0; 128];
        mem.extend(module.storage_at(128).unwrap());
        let before = mem.clone();
        assert!(
            hlasm::execute(&module, &mut mem, 128, &[Some(16)])
                .unwrap_err()
                .contains("outside run-unit storage"),
            "{body}"
        );
        assert_eq!(mem, before, "{body}: memory changed on failure");
    }
}

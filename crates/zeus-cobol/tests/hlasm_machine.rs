use zeus_cobol::hlasm;

fn bytes(text: &str) -> Vec<u8> {
    let s = text.split_whitespace().collect::<String>();
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
        .collect()
}
fn source(text: &str) -> hlasm::Module {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.asm");
    std::fs::write(&path, text).unwrap();
    hlasm::compile_file(&path, &[]).unwrap()
}
fn machine(name: &str, image: Vec<u8>) -> hlasm::Module {
    let mut m = source("MAIN CSECT\n         BR 14\n         END\n");
    m.name = name.into();
    m.entries.clear();
    m.entries.insert(name.into(), 0);
    m.zeus_hlasm = 11;
    m.code.clear();
    m.storage = image;
    m.machine_code = std::iter::once(0..m.storage.len() as u32).collect();
    hlasm::load_module(&hlasm::module_bytes(&m).unwrap()).unwrap()
}
fn execute(m: &hlasm::Module) -> (i32, Vec<u8>) {
    let mut memory = vec![0; 4096];
    memory.extend(m.storage_at(4096).unwrap());
    let length = memory.len();
    let result = hlasm::execute(m, &mut memory, 4096, &[]).unwrap();
    assert_eq!(memory.len(), length);
    (result, memory)
}

#[test]
fn architectural_rr_rx_fetch_skips_data_and_uses_real_instruction_lengths() {
    // Own fixture: BASR 12,0; L 1,22(12); A 1,26(12); ST 1,30(12);
    // LR 15,1; BCR 15,14, followed by aligned fullwords 7,9 and output.
    let mut image = bytes("0DC0 5810C016 5A10C01A 5010C01E 18F1 07FE");
    image.resize(24, 0);
    image.extend(bytes("00000007 00000009 00000000"));
    let (rc, mem) = execute(&machine("ADD", image));
    assert_eq!(rc, 16);
    assert_eq!(&mem[4096 + 32..4096 + 36], 16u32.to_be_bytes());
}

#[test]
fn architectural_text_is_fetched_after_self_modification() {
    // Set MVC's length byte before execution. Its source and destination are
    // data in the same imported section and must never be decoded eagerly.
    let mut image = bytes("41000004 4200F009 D200F020F018 41F00000 07FE");
    image.resize(24, 0);
    image.extend(b"ABCDEFGH");
    image.resize(40, 0);
    let (rc, mem) = execute(&machine("COPY", image));
    assert_eq!(rc, 0);
    assert_eq!(&mem[4096 + 32..4096 + 40], b"ABCDE\0\0\0");
}

#[test]
fn execute_overlays_architectural_bytes_without_modifying_storage() {
    // LA 2,4; EX 2,20(15); LA 15,0; BCR 15,14; padding;
    // MVC 40(1,15),32(15). EX temporarily changes length from 1 to 5.
    let mut image = bytes("41200004 4420F014 41F00000 07FE");
    image.resize(20, 0);
    image.extend(bytes("D200F028F020"));
    image.resize(32, 0);
    image.extend(b"ABCDEFGH");
    image.resize(48, 0);
    let (_, mem) = execute(&machine("EXCOPY", image));
    assert_eq!(&mem[4096 + 40..4096 + 48], b"ABCDE\0\0\0");
    assert_eq!(mem[4096 + 21], 0);
}

#[test]
fn architectural_pc_relative_branches_and_immediates_keep_signed_displacements() {
    // LGHI 1,3; AHI 1,-1; BRCT 1,-2 halfwords; LR 15,1; BCR 15,14.
    // BRCT decrements once more, so use a loop containing a neutral LA.
    let image = bytes("A7190003 41200000 A716FFFE 18F1 07FE");
    let (rc, _) = execute(&machine("LOOP", image));
    assert_eq!(rc, 0);
}

#[test]
fn machine_and_source_modules_link_and_branch_in_both_directions() {
    let caller = source(
        "MAIN CSECT\n         USING MAIN,15\n         LR 12,14\n         L 3,=A(SUB)\n         BASR 14,3\n         LR 14,12\n         BR 14\n         EXTRN SUB\n         END\n",
    );
    // LA 15,23; BCR 15,14.
    let callee = machine("SUB", bytes("41F00017 07FE"));
    let linked = hlasm::link_modules(&[caller, callee]).unwrap();
    assert_eq!(linked.zeus_hlasm, 11);
    assert!(!linked.machine_code.is_empty());
    assert_eq!(execute(&linked).0, 23);
    // Reverse direction: an architectural RX load obtains a relocated source
    // entry, BASR branches to its decoded instruction map, and returns here.
    let callee = source("SUB CSECT\n         LA 15,29\n         BR 14\n         END\n");
    let mut caller = machine(
        "MAIN",
        bytes("18CE 5830F014 0DE3 18EC 07FE 0000000000000000 00000000"),
    );
    caller.storage.resize(24, 0);
    caller.relocations.push(hlasm::Relocation {
        offset: 20,
        width: 4,
        target: 0,
        symbol: Some("SUB".into()),
        addend: 0,
        weak: false,
        expression: None,
    });
    caller.externals.insert("SUB".into(), false);
    let linked = hlasm::link_modules(&[caller, callee]).unwrap();
    assert_eq!(execute(&linked).0, 29);
}

#[test]
fn malformed_ranges_and_invalid_fetches_fail_without_leaking_invocation_storage() {
    let mut m = machine("BAD", bytes("FFFF00000000"));
    for (ranges, version) in [
        (std::iter::once(0..6).collect(), 10),
        (std::iter::once(6..6).collect(), 11),
        (std::iter::once(0..7).collect(), 11),
        (vec![0..4, 2..6], 11),
    ] {
        m.machine_code = ranges;
        m.zeus_hlasm = version;
        assert!(hlasm::load_module(&hlasm::module_bytes(&m).unwrap()).is_err());
    }
    for image in [bytes("FFFF00000000"), bytes("58F0")] {
        let m = machine("BAD", image);
        let mut mem = vec![0; 4096];
        mem.extend(m.storage_at(4096).unwrap());
        let mark = mem.len();
        let error = hlasm::execute(&m, &mut mem, 4096, &[]).unwrap_err();
        assert!(
            error.contains("unsupported IBM") || error.contains("boundary"),
            "{error}"
        );
        assert_eq!(mem.len(), mark);
    }
}

#[test]
fn architectural_wide_register_and_signed_long_displacement_memory_fields() {
    // LGR 8,15 preserves the entry base, LGHI 2,-7; STG 2,72(8);
    // LG 3,72(8); AGR 3,2; STG 3,64(8); LG 4,-8(8) uses a negative
    // twenty-bit displacement into the caller's mapped storage.
    let mut image = bytes(
        "B904008F A729FFF9 E32080480024 E33080480004 B9080032 E33080400024 E3408FF8FF04 41F00000 07FE",
    );
    image.resize(80, 0);
    let mut module = machine("WIDE", image);
    module.amode = 64;
    let (_, mem) = execute(&module);
    assert_eq!(&mem[4096 + 64..4096 + 72], (-14i64).to_be_bytes());
    assert_eq!(&mem[4096 + 72..4096 + 80], (-7i64).to_be_bytes());
}

#[test]
fn architectural_float_rrd_and_rrf_register_positions_are_distinct() {
    // F0=1, F2=1.5, F4=2.25; MADBR 0,2,4 computes 1+1.5*2.25.
    // CFDBR 5,5,0 truncates that result into GPR5. The integer conversion's
    // control nibble is not a register, unlike the fused destination nibble.
    let mut image =
        bytes("6800F030 6820F038 6840F040 B31E0024 6000F048 B3995050 5050F050 41F00000 07FE");
    image.resize(48, 0);
    image.extend(bytes("3FF0000000000000 3FF8000000000000 4002000000000000"));
    image.resize(88, 0);
    let (_, mem) = execute(&machine("FLOAT", image));
    assert_eq!(&mem[4096 + 72..4096 + 80], bytes("4011800000000000"));
    assert_eq!(&mem[4096 + 80..4096 + 84], 4u32.to_be_bytes());
    // ADTR 0,2,4 uses RRF-a: destination in byte3 high, source3 in
    // byte2 high. QADTR 8,0,6,8 uses RRF-b with a different layout.
    let mut image = bytes("6820F030 6840F038 6860F040 B3D24002 B3F50886 6080F048 41F00000 07FE");
    image.resize(48, 0);
    image.extend(bytes("2238000000000015 2238000000000025 2230000000000001"));
    image.resize(80, 0);
    let (_, mem) = execute(&machine("DECIMAL", image));
    // Decimal inputs are integer 15 and 25, requested quantum is .01.
    assert_eq!(&mem[4096 + 72..4096 + 80], bytes("2230000000001000"));
}

#[test]
fn architectural_float_memory_formats_use_distinct_fused_registers_and_index() {
    // R8=base, R9=8. RXF MADB has R3=2 in byte 1 and destination F0 in
    // byte 4; indexed input is at 72+8. RXE ADB uses byte 1 as F0 instead.
    let mut image = bytes(
        "B904008F 41900008 68008040 68208048 ED298048001E ED008048001A 60008058 ED0002000011 B2220010 8810001C 50108060 41F00000 07FE",
    );
    image.resize(64, 0);
    image.extend(bytes("3FF0000000000000 3FF8000000000000 4002000000000000"));
    image.resize(104, 0);
    let (_, mem) = execute(&machine("RXFLOAT", image));
    assert_eq!(&mem[4096 + 88..4096 + 96], bytes("4017800000000000"));
    assert_eq!(&mem[4096 + 96..4096 + 100], 1u32.to_be_bytes());
    // HFP RXE load-lengthened retains the source fraction exactly.
    let mut image = bytes("ED00F0200024 6000F028 41F00000 07FE");
    image.resize(32, 0);
    image.extend(bytes("41180000"));
    image.resize(48, 0);
    let (_, mem) = execute(&machine("RXHEX", image));
    assert_eq!(&mem[4096 + 40..4096 + 48], bytes("4118000000000000"));
}

#[test]
fn architectural_decimal_rsl_length_mask_and_rxf_shift_fields_execute() {
    // CDPT F2,64(4,R8),8 converts signed packed +12345. SLDT F0,F2,1
    // multiplies its coefficient by ten without altering the quantum.
    // CPDT F0,80(5,R8),8 writes a signed five-byte packed result.
    let mut image = bytes(
        "B904008F ED03804028AE ED2000010040 60008048 ED04805008AC ED000FFF0054 B2220010 8810001C 50108058 41F00000 07FE",
    );
    image.resize(64, 0);
    image.extend(bytes("0012345C"));
    image.resize(96, 0);
    let (_, mem) = execute(&machine("RXDECIMAL", image));
    assert_eq!(&mem[4096 + 80..4096 + 85], bytes("000123450C"));
    assert_eq!(&mem[4096 + 88..4096 + 92], 1u32.to_be_bytes());
    // Packed input has a zero quantum; shift leaves that exponent unchanged.
    let expected = source("MAIN CSECT\n BR 14\nVALUE DC DD'123450'\n ENTRY VALUE\n END\n");
    let at = expected.entries["VALUE"] as usize;
    assert_eq!(&mem[4096 + 72..4096 + 80], &expected.storage[at..at + 8]);
}

#[test]
fn architectural_rsy_save_restore_shift_and_negative_displacement_execute() {
    // Preserve entry base in R8; rotate and shift into distinct destinations.
    // STMG/LMG save and restore a pair of full-width general registers.
    let mut image = bytes(
        "B904008F A729FFF9 EB320001000D EB430001001C EB2380A00024 A7290000 A7390000 EB2380A00004 E33080800024 E34080880024 EB558FF8FF04 E35080900024 41F00000 07FE",
    );
    image.resize(176, 0);
    let mut module = machine("RSYSAVE", image);
    module.amode = 64;
    let (_, mem) = execute(&module);
    assert_eq!(&mem[4096 + 128..4096 + 136], (-14i64).to_be_bytes());
    assert_eq!(&mem[4096 + 136..4096 + 144], (-27i64).to_be_bytes());
    assert_eq!(&mem[4096 + 144..4096 + 152], [0; 8]);
    assert_eq!(&mem[4096 + 160..4096 + 168], (-7i64).to_be_bytes());
}

#[test]
fn architectural_siy_masked_conditional_and_atomic_fields_are_distinct() {
    let mut image = bytes(
        "B904008F EBAB80800052 EB0F80800054 EB8080810056 EBFF80810057 EB1F80800081 EB1F8088002D A7290003 EB32809000F8 50308094 A7490009 EB48809800F3 EB48809000F2 E34080A00024 41F00000 07FE",
    );
    image.resize(128, 0);
    image.extend(bytes("00000000 00000000 00000000 00000000 00000005"));
    image.resize(168, 0);
    let (_, mem) = execute(&machine("RSYMASK", image));
    assert_eq!(&mem[4096 + 128..4096 + 132], bytes("0B7F0000"));
    assert_eq!(&mem[4096 + 136..4096 + 140], bytes("0B7F0000"));
    assert_eq!(&mem[4096 + 144..4096 + 148], 8u32.to_be_bytes());
    assert_eq!(&mem[4096 + 148..4096 + 152], 5u32.to_be_bytes());
    // LAA result is positive (CC2). Mask 8 disables both STOC and LOC.
    assert_eq!(&mem[4096 + 152..4096 + 156], [0; 4]);
    assert_eq!(&mem[4096 + 160..4096 + 168], 9u64.to_be_bytes());
}

#[test]
fn architectural_binary_memory_lengthening_and_extended_multiply_execute() {
    let mut image = bytes(
        "ED00F0400004 6000F050 ED40F0480005 6040F058 6060F060 6800F048 ED00F0480007 6000F068 6020F070 41F00000 07FE",
    );
    image.resize(64, 0);
    image.extend(bytes("3FC00000 00000000 3FF0000000000001"));
    image.resize(120, 0);
    let (_, mem) = execute(&machine("RXWIDEN", image));
    assert_eq!(&mem[4096 + 80..4096 + 88], bytes("3FF8000000000000"));
    assert_eq!(
        &mem[4096 + 88..4096 + 104],
        bytes("3FFF000000000000 1000000000000000")
    );
    assert_eq!(
        &mem[4096 + 104..4096 + 120],
        bytes("3FFF000000000000 2000000000000100")
    );
}

#[test]
fn halfword_immediates_preserve_high_halves_cc_and_execute_register_overlay() {
    let m = source(
        "MAIN CSECT\n LR 12,15\n USING MAIN,12\n LG 2,=X'1122334455667788'\n LHI 2,-32768\n STG 2,OUT\n CHI 2,0\n MHI 2,-32768\n STG 2,OUT+8\n IPM 3\n SRL 3,28\n ST 3,OUT+16\n L 3,=X'08000000'\n SPM 3\n MHI 2,4\n ST 2,OUT+20\n LA 3,80\n EX 3,TEMPLATE\n MHI 5,3\n CHI 5,-21\n BNE BAD\n ST 5,OUT+24\n LHI 15,0\n B DONE\nBAD LHI 15,99\n B DONE\nTEMPLATE LHI 0,-7\nDONE BR 14\nOUT DS 4D\n ENTRY OUT\n END\n",
    );
    let (rc, mem) = execute(&m);
    assert_eq!(rc, 0);
    let at = 4096 + m.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 8], bytes("11223344FFFF8000"));
    assert_eq!(&mem[at + 8..at + 16], bytes("1122334440000000"));
    assert_eq!(&mem[at + 16..at + 20], 1u32.to_be_bytes());
    assert_eq!(&mem[at + 20..at + 24], [0; 4]);
    assert_eq!(&mem[at + 24..at + 28], (-21i32).to_be_bytes());
}

#[test]
fn architectural_halfword_immediates_sign_extend_and_keep_load_multiply_cc() {
    let mut image = bytes(
        "A7288000 A72C8000 A72E0000 A738FFFF A73C0000 B2220010 8810001C 5020F040 5010F044 A7F80000 07FE",
    );
    image.resize(72, 0);
    let (_, mem) = execute(&machine("RIHALF", image));
    assert_eq!(&mem[4096 + 64..4096 + 68], 0x40000000u32.to_be_bytes());
    assert_eq!(&mem[4096 + 68..4096 + 72], 2u32.to_be_bytes());
}

#[test]
fn halfword_immediate_source_operands_require_signed_absolute_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("immediate.asm");
    for op in ["LHI", "MHI", "CHI", "AHI", "LGHI", "AGHI", "CGHI", "MGHI"] {
        for (operand, valid) in [
            ("-32768", true),
            ("32767", true),
            ("32768", false),
            ("-32769", false),
            ("MAIN", false),
            ("MAIN-MAIN+7", true),
        ] {
            std::fs::write(
                &path,
                format!("MAIN CSECT\n {op} 2,{operand}\n BR 14\n END\n"),
            )
            .unwrap();
            let report = hlasm::compile_report(&path, &[]);
            assert_eq!(
                report.module.is_some(),
                valid,
                "{op} {operand}: {:?}",
                report.diagnostics
            );
        }
    }
}

#[test]
fn architectural_ril_full_immediates_high_word_loops_and_execute_relative_long() {
    let mut image = bytes(
        "B904008F C021FFFFFFF9 C2280000000A C22C00000003 E32080800024 B2220010 8810001C 50108088 E34080900004 C22800000001 CC46FFFFFFFD E34080980024 E32080A00024 41300004",
    );
    let ex = image.len();
    image.extend(bytes("C63000000000 A7F80000 07FE"));
    let target = image.len();
    image.extend(bytes("D20080B080A8"));
    let relative = ((target as i32 - ex as i32) / 2).to_be_bytes();
    image[ex + 2..ex + 6].copy_from_slice(&relative);
    image.resize(144, 0);
    image.extend(bytes("0000000212345678"));
    image.resize(168, 0);
    image.extend(b"ABCDEFGH");
    image.resize(184, 0);
    let (_, mem) = execute(&machine("RILFORMS", image));
    assert_eq!(&mem[4096 + 128..4096 + 136], 3u64.to_be_bytes());
    assert_eq!(&mem[4096 + 136..4096 + 140], 0u32.to_be_bytes());
    assert_eq!(&mem[4096 + 152..4096 + 160], 0x12345678u64.to_be_bytes());
    assert_eq!(&mem[4096 + 160..4096 + 168], 5u64.to_be_bytes());
    assert_eq!(&mem[4096 + 176..4096 + 184], b"ABCDE\0\0\0");
}

#[test]
fn fullword_immediate_source_operands_require_signed_absolute_values() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("full-immediate.asm");
    for op in ["LGFI", "AGFI", "CGFI"] {
        for (operand, valid) in [
            ("-2147483648", true),
            ("2147483647", true),
            ("2147483648", false),
            ("-2147483649", false),
            ("MAIN+7", false),
            ("MAIN-MAIN+7", true),
        ] {
            std::fs::write(
                &path,
                format!("MAIN CSECT\n {op} 2,{operand}\n BR 14\n END\n"),
            )
            .unwrap();
            let report = hlasm::compile_report(&path, &[]);
            assert_eq!(
                report.module.is_some(),
                valid,
                "{op} {operand}: {:?}",
                report.diagnostics
            );
        }
    }
}

#[test]
fn architectural_divide_to_integer_decodes_rrf_b_registers_and_rounding_mask() {
    // Own bytes: LE 0,128(15); LE 2,132(15); DIEBR 0,4,2,4;
    // STE 0,136(15); STE 4,140(15); IPM 1; ST 1,144(15); XR 15,15; BR 14.
    let mut image =
        bytes("7800F080 7820F084 B3534402 7000F088 7040F08C B2220010 5010F090 17FF 07FE");
    image.resize(128, 0);
    image.extend(bytes("40E00000 40000000 00000000 00000000 00000000"));
    let (rc, mem) = execute(&machine("DIVINT", image));
    assert_eq!(rc, 0);
    assert_eq!(&mem[4096 + 136..4096 + 144], bytes("BF800000 40800000"));
    assert_eq!(mem[4096 + 144] >> 4, 0);
}

#[test]
fn architectural_bfp_reserved_rrf_modifiers_fail_without_leaking_invocation_storage() {
    for fields in ["41", "48", "20"] {
        let m = machine("BADMOD", bytes(&format!("B344{fields}01 07FE")));
        let mut memory = vec![0; 4096];
        memory.extend(m.storage_at(4096).unwrap());
        let mark = memory.len();
        let error = hlasm::execute(&m, &mut memory, 4096, &[]).unwrap_err();
        assert!(
            error.contains("modifier") || error.contains("rounding"),
            "{fields}: {error}"
        );
        assert_eq!(memory.len(), mark);
    }
}

use zeus_cobol::hlasm;

fn compile(spec: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("float.asm");
    std::fs::write(
        &file,
        format!("MAIN CSECT\n         BR 14\nDATA DC {spec}\n         ENTRY DATA\n         END\n"),
    )
    .unwrap();
    hlasm::compile_file(&file, &[])
}

fn data(spec: &str) -> Vec<u8> {
    let module = compile(spec).unwrap();
    let at = module.entries["DATA"] as usize;
    module.storage[at..module.storage.len() - 256].to_vec()
}

#[test]
fn hexadecimal_short_long_extended_and_quadword_alignment() {
    assert_eq!(data("E'1,-12.5,0'"), hex("41100000 C1C80000 00000000"));
    assert_eq!(data("DH'.1'"), hex("401999999999999A"));
    assert_eq!(data("LH'.1'"), hex("4019999999999999 329999999999999A"));
    let module = compile("LQ'1'").unwrap();
    assert_eq!(module.entries["DATA"] % 16, 0);
    assert_eq!(data("ES1'1'"), hex("42010000"));
}

#[test]
fn binary_formats_keep_full_extended_precision_and_signed_zero() {
    assert_eq!(data("EB'1,-0,.1'"), hex("3F800000 80000000 3DCCCCCD"));
    assert_eq!(data("DB'.1'"), hex("3FB999999999999A"));
    assert_eq!(data("LB'.1'"), hex("3FFB999999999999999999999999999A"));
    assert_eq!(
        data("LB'1.0000000000000000000000000000000002'"),
        hex("3FFF0000000000000000000000000001")
    );
}

#[test]
fn decimal_encodes_dpd_and_preserves_quantum() {
    assert_eq!(data("ED'1'"), hex("22500001"));
    assert_eq!(data("DD'1'"), hex("2238000000000001"));
    assert_eq!(data("LD'1'"), hex("22080000000000000000000000000001"));
    assert_eq!(data("ED'1.00'"), hex("22300080"));
    assert_eq!(data("ED'888'"), hex("2250006E"));
    assert_eq!(data("ED'-0'"), hex("A2500000"));
}

#[test]
fn binary_ties_and_directed_rounding_are_exact() {
    assert_eq!(data("EB'1.000000059604644775390625R4'"), hex("3F800000"));
    assert_eq!(data("EB'1.000000059604644775390625R1'"), hex("3F800001"));
    assert_eq!(data("EB'1.000000059604644775390626R4'"), hex("3F800001"));
    assert_eq!(data("EB'-1.00000000000000000001R6'"), hex("BF800000"));
    assert_eq!(data("EB'-1.00000000000000000001R7'"), hex("BF800001"));
    assert_eq!(
        data("DB'1.00000000000000011102230246251565404236316680908203125R4'"),
        hex("3FF0000000000000")
    );
}

#[test]
fn subnormals_overflows_and_extreme_exponents() {
    assert_eq!(data("EB'1E-100R6,1E-100R4'"), hex("00000001 00000000"));
    assert_eq!(data("EB'1E100R5,1E100R4'"), hex("7F7FFFFF 7F800000"));
    assert_eq!(data("DB'1E-2147483648R6'"), hex("0000000000000001"));
    assert_eq!(
        data("LD'1E-2147483648R10'"),
        hex("00000000000000000000000000000001")
    );
    assert_eq!(data("ED'1E2147483647'"), hex("78000000"));
}

#[test]
fn decimal_rounding_modes_and_carry() {
    assert_eq!(
        data("ED'1.2345665R8,1.2345665R12,1.2345665R13'"),
        hex("25F4D2E6 25F4D2E7 25F4D2E6")
    );
    assert_eq!(data("ED'9999999.5'"), hex("26600000"));
}

#[test]
fn special_values_and_reservation_layout() {
    assert_eq!(
        data("EB'(INF)',DB'(QNAN)',LB'(SNAN)'"),
        hex("7F800000 7FFC000000000000 7FFF4000000000000000000000000000")
    );
    assert_eq!(
        data("ED'(INF)',DD'(SNAN)',LD'(DMIN)'"),
        hex("78000000 7E00000000000000 00000000000000000000000000000001")
    );
    assert_eq!(data("2DBL4'1'"), hex("3FF00000 3FF00000"));
    assert_eq!(data("EBL.16'1',XL.4'F',BL.4'1010'"), hex("3F80FA"));
}

#[test]
fn malformed_constants_are_source_located_errors() {
    for spec in [
        "EB'1R3'",
        "ED'1R7'",
        "EDL3'1'",
        "LH'1.2.3'",
        "EB'1E1E1'",
        "EB'(INVALID)'",
        "EQL4'1'",
    ] {
        let error = compile(spec).unwrap_err();
        assert!(error.contains("float.asm:3:"), "{spec}: {error}");
    }
}

#[test]
fn floating_attributes_literals_duplication_and_bit_lists() {
    let module = program(
        "         LE 0,=EB'1.25'\n         STE 0,OUT\n",
        "A DC ES2'1'\nB DC LS10'1'\nC DC DB'1'\nOUT DC A(0,S'A,I'A,S'B,I'B,S'C,I'C,T'C)\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    let values: Vec<_> = memory[at..at + 32]
        .chunks_exact(4)
        .map(|v| u32::from_be_bytes(v.try_into().unwrap()))
        .collect();
    assert_eq!(
        values,
        [
            0x3fa00000,
            2,
            4,
            10,
            18,
            0,
            0,
            u32::from(zeus_cobol::ebcdic::encode("D")[0])
        ]
    );
    assert_eq!(data("2EBL.12'1,2',BL.4'1111'"), hex("3F84003F8400F0"));
}

#[test]
fn loaded_entry_branches_share_fprs_and_fpc_with_caller() {
    let dir = tempfile::tempdir().unwrap();
    let callee_path = dir.path().join("FPADD.asm");
    std::fs::write(
        &callee_path,
        "FPADD CSECT\n USING FPADD,15\n ADB 0,=DB'2'\n SRNM 3\n BR 14\n END\n",
    )
    .unwrap();
    let callee = hlasm::compile_file(&callee_path, &[]).unwrap();
    hlasm::save_module(&callee, &dir.path().join("FPADD.zo")).unwrap();
    let caller_path = dir.path().join("FPMAIN.asm");
    std::fs::write(&caller_path,"FPMAIN CSECT\n LR 12,15\n USING FPMAIN,12\n ST 14,RET\n LE 0,=EB'1'\n LDEBR 0,0\n LOAD EP=FPADD\n LR 15,0\n BASR 14,15\n STD 0,OUT\n EFPC 3\n N 3,=F'7'\n C 3,=F'3'\n BNE BAD\n LD 4,=DB'3'\n CDBR 0,4\n BNE BAD\n XR 15,15\n B DONE\nBAD LA 15,99\nDONE L 14,RET\n BR 14\nRET DS F\nOUT DS D\n END\n").unwrap();
    let caller = hlasm::compile_file(&caller_path, &[]).unwrap();
    let mut machine = zeus_cobol::vm::Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.run_hlasm_main(caller).unwrap(), 0);
}

fn hex(text: &str) -> Vec<u8> {
    let text = text.replace(' ', "");
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}

fn program(body: &str, declarations: &str) -> hlasm::Module {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fp-run.asm");
    std::fs::write(&path,format!("MAIN CSECT\n         USING MAIN,15\n{body}         XR 15,15\n         BR 14\n{declarations}         ENTRY OUT\n         END\n")).unwrap();
    let module = hlasm::compile_file(&path, &[]).unwrap();
    // Exercise the same serialized representation used by library and JCL loads.
    hlasm::load_module(&serde_json::to_vec(&module).unwrap()).unwrap()
}
fn run(module: &hlasm::Module) -> Vec<u8> {
    let mut memory = vec![0; 4096];
    memory.extend(module.storage_at(4096).unwrap());
    assert_eq!(hlasm::execute(module, &mut memory, 4096, &[]).unwrap(), 0);
    // Every successful arithmetic fixture also traverses architectural output,
    // loader, decoder and execution. Compare the complete user data image.
    for format in ["goff", "obj"] {
        if format == "obj" && module.alignment > 8 {
            continue;
        }
        let bytes = if format == "goff" {
            hlasm::goff_bytes(module)
        } else {
            hlasm::object_bytes(module)
        }
        .unwrap_or_else(|e| panic!("{format}: {e}"));
        let loaded = hlasm::load_module(&bytes).unwrap();
        let mut actual = vec![0; 4096];
        actual.extend(loaded.storage_at(4096).unwrap());
        assert_eq!(
            hlasm::execute(&loaded, &mut actual, 4096, &[]).unwrap(),
            0,
            "{format}"
        );
        let out = 4096 + module.entries["OUT"] as usize;
        assert_eq!(
            &actual[out..4096 + module.storage.len()],
            &memory[out..4096 + module.storage.len()],
            "{format}"
        );
    }
    memory
}

#[test]
fn binary_arithmetic_memory_and_register_forms_execute_exactly() {
    let module = program(
        "         LD 0,=DB'1.5'\n         LDR 1,0\n         ADB 0,=DB'2.25'\n         MDBR 0,1\n         DDB 0,=DB'1.5'\n         SDB 0,=DB'.25'\n         STD 0,OUT\n         CDB 0,=DB'3.5'\n         BE OK\n         L 0,4095(14)\nOK       LE 4,=EB'2'\n         AEBR 4,4\n         STE 4,OUT+8\n",
        "OUT DS D,EB\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 12], hex("400C000000000000 40800000"));
}

#[test]
fn extended_register_pairs_alias_and_preserve_full_precision() {
    let module = program(
        "         LD 0,NUMBER\n         LD 2,NUMBER+8\n         LXR 4,0\n         AXBR 4,4\n         STD 4,OUT\n         STD 6,OUT+8\n         LDXBR 8,4\n         STD 8,OUT+16\n",
        "NUMBER DC LB'.1'\nOUT DS 3D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 24],
        hex("3FFC999999999999999999999999999A 3FC999999999999A")
    );
}

#[test]
fn square_root_and_format_conversion_use_exact_rounding() {
    let module = program(
        "         LD 0,=DB'2'\n         SQDBR 0,0\n         STD 0,OUT\n         LEDBR 4,0\n         STE 4,OUT+8\n         LXDBR 8,0\n         SQXBR 8,8\n         STD 8,OUT+16\n         STD 10,OUT+24\n",
        "OUT DS 4D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 12], hex("3FF6A09E667F3BCD 3FB504F3"));
    // The extended root is computed from the rounded long operand, not host f64.
    assert_ne!(&memory[at + 24..at + 32], &[0; 8]);
}

#[test]
fn short_writes_preserve_low_half_and_sign_loads_preserve_signaling_nan() {
    let module = program(
        "         LD 4,=X'0123456789ABCDEF'\n         LE 4,=EB'1.5'\n         STD 4,OUT\n         LE 0,=EB'(SNAN)'\n         LNEBR 4,0\n         STE 4,OUT+8\n         STFPC OUT+12\n",
        "OUT DS 2D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 16],
        hex("3FC0000089ABCDEF FFA00000 00000000")
    );
}

#[test]
fn fpc_rounding_flags_and_unmasked_nan_division() {
    let module = program(
        "         LD 0,=DB'1'\n         DDB 0,=DB'3'\n         STFPC OUT\n         SRNM 2\n         LD 0,=DB'1'\n         DDB 0,=DB'3'\n         STD 0,OUT+8\n         LZER 4\n         DEBR 4,4\n         STE 4,OUT+16\n         STFPC OUT+20\n",
        "OUT DS 3D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 4], hex("00080000"));
    assert_eq!(
        &memory[at + 8..at + 24],
        hex("3FD5555555555556 7FC00000 00880002")
    );
}

#[test]
fn invalid_operation_trap_suppresses_following_store_and_reclaims_frame() {
    let module = program(
        "         LFPC =X'80000000'\n         LZER 0\n         DEBR 0,0\n         STE 0,OUT\n",
        "OUT DC X'AABBCCDD'\n",
    );
    let mut memory = vec![0; 4096];
    memory.extend(module.storage_at(4096).unwrap());
    let mark = memory.len();
    let error = hlasm::execute(&module, &mut memory, 4096, &[]).unwrap_err();
    assert!(error.contains("DXC=80"), "{error}");
    assert_eq!(memory.len(), mark);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 4], hex("AABBCCDD"));
}

#[test]
fn binary_random_finite_add_subtract_multiply_divide_match_independent_host_oracle() {
    let mut seed = 0x9467_12abu32;
    let mut body = String::new();
    let mut expected = Vec::new();
    for i in 0..80 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let a = f32::from_bits((seed & 0x807f_ffff) | ((120 + (seed >> 24) % 16) << 23));
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let b = f32::from_bits((seed & 0x807f_ffff) | ((120 + (seed >> 24) % 16) << 23));
        let (op, value) = match i % 4 {
            0 => ("AEB", a + b),
            1 => ("SEB", a - b),
            2 => ("MEEB", a * b),
            _ => ("DEB", a / b),
        };
        body.push_str(&format!(
            "         LE 0,=X'{:08X}'\n         {op} 0,=X'{:08X}'\n         STE 0,OUT+{}\n",
            a.to_bits(),
            b.to_bits(),
            i * 4
        ));
        expected.extend(value.to_bits().to_be_bytes());
    }
    let module = program(&body, "OUT DS 80F\n");
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + expected.len()], expected);
}

#[test]
fn hexadecimal_arithmetic_and_short_to_long_multiplication() {
    let module = program(
        "         LE 0,=E'1.5'\n         MER 0,0\n         AD 0,=D'2.25'\n         DD 0,=D'1.5'\n         SD 0,=D'.5'\n         STD 0,OUT\n         CD 0,=D'2.5'\n         BE OK\n         L 0,4095(14)\nOK       LE 4,=E'1.5'\n         MEER 4,4\n         STE 4,OUT+8\n",
        "OUT DS D,E\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 12], hex("4128000000000000 41240000"));
}

#[test]
fn hexadecimal_addition_uses_one_guard_digit_before_normalization() {
    let module = program(
        "         LE 0,=E'1'\n         SE 0,=X'3A100000'\n         STE 0,OUT\n         LE 0,=E'1'\n         SE 0,=X'3B100000'\n         STE 0,OUT+4\n         LE 0,=X'41010000'\n         AUR 0,0\n         STE 0,OUT+8\n         LE 0,=X'41010000'\n         AER 0,0\n         STE 0,OUT+12\n",
        "OUT DS 4E\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 16],
        hex("41100000 40FFFFFF 41020000 40200000")
    );
}

#[test]
fn hexadecimal_extended_pairs_rounding_and_exponent_underflow() {
    let module = program(
        "         LD 0,=D'1.5'\n         MXDR 0,0\n         STD 0,OUT\n         STD 2,OUT+8\n         LDXR 4,0\n         STD 4,OUT+16\n         LE 8,=X'00100000'\n         DE 8,=E'16'\n         STE 8,OUT+24\n",
        "OUT DS 4D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 28],
        hex("4124000000000000 3300000000000000 4124000000000000 00000000")
    );
}

#[test]
fn cobol_comp2_arguments_share_hfp_storage_with_assembler() {
    let dir = tempfile::tempdir().unwrap();
    let asm_path = dir.path().join("HFPADD.asm");
    std::fs::write(&asm_path,"HFPADD CSECT\n USING HFPADD,15\n L 2,0(1)\n LD 0,0(2)\n AD 0,=D'2.25'\n STD 0,0(2)\n XR 15,15\n BR 14\n END\n").unwrap();
    let asm = hlasm::compile_file(&asm_path, &[]).unwrap();
    hlasm::save_module(&asm, &dir.path().join("HFPADD.zo")).unwrap();
    let cobol_path = dir.path().join("main.cbl");
    std::fs::write(&cobol_path,"       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAINPGM.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 A COMP-2 VALUE 2.5.\n       PROCEDURE DIVISION.\n           CALL 'HFPADD' USING A\n           IF A = 4.75 MOVE 0 TO RETURN-CODE\n           ELSE MOVE 99 TO RETURN-CODE END-IF\n           STOP RUN.\n").unwrap();
    let module = zeus_cobol::compile_file(&cobol_path, &Default::default()).unwrap();
    let mut machine = zeus_cobol::vm::Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.run_main(module).unwrap(), 0);
}

#[test]
fn signed_integer_conversions_and_short_to_long_binary_product() {
    let module = program(
        "         L 3,=F'-16777217'\n         CEFBR 0,3\n         STE 0,OUT\n         CDFBR 0,3\n         STD 0,OUT+8\n         LG 3,=FD'9223372036854775807'\n         CXGBR 4,3\n         STD 4,OUT+16\n         STD 6,OUT+24\n         LE 0,=EB'1.5'\n         MDEBR 0,0\n         STD 0,OUT+32\n",
        "OUT DS 5D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 4], hex("CB800000"));
    assert_eq!(
        &memory[at + 8..at + 40],
        hex("C170000010000000 403DFFFFFFFFFFFFFFFC000000000000 4002000000000000")
    );
}

#[test]
fn fused_binary_operations_round_once_and_execute_decodes_register_positions() {
    let module = program(
        "         LE 0,=EB'-1'\n         LE 1,=EB'1.00000011920928955078125'\n         LE 2,=EB'.99999988079071044921875'\n         MAEBR 0,1,2\n         STE 0,OUT\n         LE 0,=EB'-1'\n         MAEB 0,1,=EB'.99999988079071044921875'\n         STE 0,OUT+4\n         LE 0,=EB'-1'\n         LA 3,4\n         EX 3,TEMPLATE\n         STE 0,OUT+8\n         B DONE\nTEMPLATE AEBR 1,2\nDONE EQU *\n",
        "OUT DS 3E\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 12], hex("A8800000 A8800000 A8800000"));
}

#[test]
fn special_signed_values_match_documented_hlasm_fraction_patterns() {
    assert_eq!(
        data("EB'(SNAN),(QNAN),(NAN),-(INF)'"),
        hex("7FA00000 7FE00000 7FC00000 FF800000")
    );
    assert_eq!(data("DH'-0'"), hex("8000000000000000"));
    assert_eq!(data("D'-0'"), hex("0000000000000000"));
    assert_eq!(data("LH'1'"), hex("4110000000000000 3300000000000000"));
    assert_eq!(data("LL8'1'"), hex("4110000000000000"));
    for spec in [
        "EBL.10'(INF)'",
        "E'1R4'",
        "D'1E76'",
        "EHL.11'1'",
        "LHS28'1'",
    ] {
        assert!(compile(spec).is_err(), "{spec}");
    }
}

#[test]
fn integer_conversion_rounding_suppression_condition_codes_and_register_halves() {
    let module = program(
        "         LE 0,=EB'1.5'\n         LG 3,=X'1234567800000000'\n         CFEBR 3,4,0\n         STG 3,OUT\n         CFEBRA 4,5,0,4\n         ST 4,OUT+8\n         LE 0,=EB'-.25'\n         CFEBRA 4,5,0,4\n         BC 4,NEGATIVE\n         L 0,4095(14)\nNEGATIVE ST 4,OUT+12\n         LD 0,=D'-2.5'\n         CFDR 4,0,0\n         ST 4,OUT+16\n         CFDR 4,1,0\n         ST 4,OUT+20\n",
        "OUT DS 3D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 24],
        hex("1234567800000002 00000001 00000000 FFFFFFFE FFFFFFFD")
    );
}

#[test]
fn logical_integer_conversion_covers_full_u64_range_and_invalid_sources() {
    let module = program(
        "         LG 3,=X'FFFFFFFFFFFFFFFF'\n         CXLGBR 4,0,3,0\n         CLGXBR 5,5,4,0\n         STG 5,OUT\n         LE 0,=EB'-1'\n         CLFEBR 4,5,0,0\n         ST 4,OUT+8\n         STFPC OUT+12\n         XR 4,4\n         SFPC 4\n         LD 0,=DB'(NAN)'\n         CFDBRA 4,0,0,4\n         ST 4,OUT+16\n         STFPC OUT+20\n",
        "OUT DS 3D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 24],
        hex("FFFFFFFFFFFFFFFF 00000000 00880000 80000000 00800000")
    );
}

#[test]
fn decimal_arithmetic_keeps_exact_decimal_values_and_preferred_quantum() {
    let module = program(
        "         LD 2,=DD'.1'\n         LD 4,=DD'.2'\n         ADTR 0,2,4\n         STD 0,OUT\n         LD 2,=DD'1.00'\n         LD 4,=DD'2.0'\n         ADTR 0,2,4\n         STD 0,OUT+8\n         LD 2,=DD'1.20'\n         MDTR 0,2,4\n         STD 0,OUT+16\n         LD 2,=DD'1'\n         LD 4,=DD'2'\n         DDTRA 0,2,4,4\n         STD 0,OUT+24\n         STFPC OUT+32\n",
        "OUT DS 5D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 36],
        hex("2234000000000003 2230000000000180 222C000000000A00 2234000000000005 00000000")
    );
}

#[test]
fn extended_decimal_addition_preserves_thirty_four_digits() {
    let module = program(
        "         LD 0,A\n         LD 2,A+8\n         LD 4,B\n         LD 6,B+8\n         AXTR 8,0,4\n         STD 8,OUT\n         STD 10,OUT+8\n         LD 0,EXPECT\n         LD 2,EXPECT+8\n         CXTR 8,0\n         BE OK\n         L 0,4095(14)\nOK EQU *\n",
        "A DC LD'1000000000000000000000000000000000'\nB DC LD'1'\nEXPECT DC LD'1000000000000000000000000000000001'\nOUT DS 2D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 16],
        data("LD'1000000000000000000000000000000001'")
    );
}

#[test]
fn decimal_exceptions_nan_canonicalization_and_quantum_flags() {
    let module = program(
        "         LD 2,=DD'(MAX)'\n         LD 4,=DD'10'\n         MDTR 0,2,4\n         STD 0,OUT\n         STFPC OUT+8\n         XR 3,3\n         SFPC 3\n         LD 2,=DD'(SNAN)'\n         LTDTR 0,2\n         STD 0,OUT+16\n         STFPC OUT+24\n         XR 3,3\n         SFPC 3\n         LD 2,=DD'1'\n         LD 4,=DD'3'\n         DDTRA 0,2,4,4\n         STFPC OUT+28\n",
        "OUT DS 4D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 12], hex("7800000000000000 00280000"));
    assert_eq!(
        &memory[at + 16..at + 32],
        hex("7C00000000000000 00800000 000C0000")
    );
}

#[test]
fn common_fp_transfers_sign_changes_and_decimal_rounding_mode() {
    let module = program(
        "         LG 3,=X'FEDCBA9876543210'\n         LDGR 0,3\n         LPDFR 4,0\n         LGDR 5,4\n         STG 5,OUT\n         LCDFR 4,4\n         LGDR 5,4\n         STG 5,OUT+8\n         SRNMT 2\n         LD 2,=DD'1'\n         LD 4,=DD'3'\n         DDTR 0,2,4\n         STD 0,OUT+16\n         STFPC OUT+24\n",
        "OUT DS 4D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 16],
        hex("7EDCBA9876543210 FEDCBA9876543210")
    );
    assert_eq!(&memory[at + 16..at + 24], data("DD'.3333333333333334'"));
    assert_eq!(&memory[at + 24..at + 28], hex("00080020"));
}

#[test]
fn program_mask_insert_extract_preserves_high_register_half() {
    let module = program(
        "         L 3,=X'FF000000'\n         SPM 3\n         LG 4,=X'12345678FFFFFFFF'\n         IPM 4\n         STG 4,OUT\n         LA 3,0\n         SPM 3\n         IPM 4\n         STG 4,OUT+8\n",
        "OUT DS 2D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 16],
        hex("123456783FFFFFFF 1234567800FFFFFF")
    );
}

#[test]
fn program_masks_control_hfp_fixed_and_decimal_exceptions() {
    for (mask, body, message) in [
        (
            2,
            "         LE 0,=X'00100000'\n         DE 0,=E'16'\n",
            "HFP exponent-underflow",
        ),
        (
            1,
            "         LE 0,=E'1'\n         SER 0,0\n",
            "HFP significance",
        ),
        (
            8,
            "         L 4,=F'2147483647'\n         AHI 4,1\n",
            "fixed-point-overflow",
        ),
        (4, "         AP SMALL,=P'1'\n", "decimal-overflow"),
    ] {
        let source = format!(
            "         L 3,=X'{:02X}000000'\n         SPM 3\n{body}",
            mask
        );
        let module = program(&source, "SMALL DC PL1'9'\nOUT DS D\n");
        let mut memory = vec![0; 4096];
        memory.extend(module.storage_at(4096).unwrap());
        let error = hlasm::execute(&module, &mut memory, 4096, &[]).unwrap_err();
        assert!(error.contains(message), "{error}");
        // Clearing the mask gives the architectural default completed result.
        let module = program(
            &format!("         XR 3,3\n         SPM 3\n{body}"),
            "SMALL DC PL1'9'\nOUT DS D\n",
        );
        run(&module);
    }
}

#[test]
fn link_preserves_fp_registers_control_and_program_mask() {
    let dir = tempfile::tempdir().unwrap();
    for (name, source) in [
        (
            "FPCHILD",
            "FPCHILD CSECT\n USING FPCHILD,15\n ADB 0,=DB'2.25'\n SRNMT 2\n L 3,=X'01000000'\n SPM 3\n XR 15,15\n BR 14\n END\n",
        ),
        (
            "FPMAIN",
            "FPMAIN CSECT\n USING FPMAIN,15\n LR 12,14\n LD 0,=DB'1.5'\n LINK EP=FPCHILD\n LARL 15,FPMAIN\n USING FPMAIN,15\n CDB 0,=DB'3.75'\n BNE FAIL\n EFPC 3\n C 3,=F'32'\n BNE FAIL\n IPM 3\n SRL 3,24\n C 3,=F'1'\n BNE FAIL\n XR 15,15\n BR 12\nFAIL LA 15,99\n BR 12\n END\n",
        ),
    ] {
        let file = dir.path().join(format!("{name}.asm"));
        std::fs::write(&file, source).unwrap();
        let module = hlasm::compile_file(&file, &[]).unwrap();
        hlasm::save_module(&module, &dir.path().join(format!("{name}.zo"))).unwrap();
    }
    let main = hlasm::load_module(&std::fs::read(dir.path().join("FPMAIN.zo")).unwrap()).unwrap();
    let mut machine = zeus_cobol::vm::Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.run_hlasm_main(main).unwrap(), 0);
}

#[test]
fn decimal_lengthening_and_rounding_preserve_quantum_and_register_halves() {
    let module = program(
        "         LE 2,=ED'1.00'\n         LDETR 0,2,0\n         STD 0,OUT\n         LXDTR 4,0,0\n         STD 4,OUT+8\n         STD 6,OUT+16\n         LDXTR 8,8,4,0\n         STD 8,OUT+24\n         LD 10,=X'0000000012345678'\n         LEDTR 10,8,8,0\n         STD 10,OUT+32\n         LD 2,=DD'1.2345675'\n         LEDTR 0,8,2,0\n         STE 0,OUT+40\n         STFPC OUT+44\n",
        "OUT DS 6D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 8], data("DD'1.00'"));
    assert_eq!(&memory[at + 8..at + 24], data("LD'1.00'"));
    assert_eq!(&memory[at + 24..at + 32], data("DD'1.00'"));
    assert_eq!(&memory[at + 32..at + 36], data("ED'1.00'"));
    assert_eq!(&memory[at + 36..at + 40], hex("12345678"));
    assert_eq!(&memory[at + 40..at + 44], data("ED'1.234568'"));
    assert_eq!(&memory[at + 44..at + 48], hex("00080000"));
}

#[test]
fn decimal_rounding_can_suppress_inexact_without_changing_rounding() {
    let module = program(
        "         LD 2,=DD'1.2345675'\n         LEDTR 0,9,2,4\n         STE 0,OUT\n         STFPC OUT+4\n",
        "OUT DS D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 4], data("ED'1.234567'"));
    assert_eq!(&memory[at + 4..at + 8], &[0; 4]);
}

#[test]
fn decimal_integer_conversions_preserve_full_u64_and_general_register_halves() {
    let module = program(
        "         LG 3,=X'FFFFFFFFFFFFFFFF'\n         CXLGTR 4,0,3,0\n         STD 4,OUT\n         STD 6,OUT+8\n         CLGXTR 5,9,4,0\n         STG 5,OUT+16\n         LG 3,=X'12345678FFFFFF85'\n         CDFTR 0,0,3,0\n         STD 0,OUT+24\n         LG 5,=X'1122334400000000'\n         CFDTR 5,9,0,0\n         STG 5,OUT+32\n         LG 3,=X'8000000000000000'\n         CXGTR 4,3\n         CGXTR 5,9,4\n         STG 5,OUT+40\n",
        "OUT DS 6D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 16], data("LD'18446744073709551615'"));
    assert_eq!(&memory[at + 16..at + 24], hex("FFFFFFFFFFFFFFFF"));
    assert_eq!(&memory[at + 24..at + 32], data("DD'-123'"));
    assert_eq!(
        &memory[at + 32..at + 48],
        hex("11223344FFFFFF85 8000000000000000")
    );
}

#[test]
fn decimal_integer_rounding_uses_decimal_prepare_and_fpc_mode() {
    let module = program(
        "         SRNMT 2\n         LD 0,=DD'2.2'\n         CGDTR 3,0,0\n         STG 3,OUT\n         CGDTR 3,15,0\n         STG 3,OUT+8\n         LD 0,=DD'5.1'\n         CGDTRA 3,15,0,4\n         STG 3,OUT+16\n         LG 3,=X'FFFFFFFFFFFFFFFF'\n         CDLGTR 0,9,3,4\n         STD 0,OUT+24\n",
        "OUT DS 4D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 24],
        hex("0000000000000003 0000000000000002 0000000000000006")
    );
    assert_eq!(&memory[at + 24..at + 32], data("DD'18446744073709550000'"));
}

#[test]
fn decimal_integer_conversion_all_forms_and_execute_opcode_selection() {
    for (from, to, modifiers) in [
        ("CDGTR", "CGDTR", false),
        ("CXGTR", "CGXTR", false),
        ("CDGTRA", "CGDTRA", true),
        ("CXGTRA", "CGXTRA", true),
        ("CDFTR", "CFDTR", true),
        ("CXFTR", "CFXTR", true),
        ("CDLGTR", "CLGDTR", true),
        ("CXLGTR", "CLGXTR", true),
        ("CDLFTR", "CLFDTR", true),
        ("CXLFTR", "CLFXTR", true),
    ] {
        let from_args = if modifiers { "4,0,3,0" } else { "4,3" };
        let to_args = if modifiers { "5,9,4,0" } else { "5,9,4" };
        let module = program(
            &format!(
                "         LGHI 3,123\n         {from} {from_args}\n         {to} {to_args}\n         STG 5,OUT\n"
            ),
            "OUT DS D\n",
        );
        let memory = run(&module);
        let at = 4096 + module.entries["OUT"] as usize;
        assert_eq!(&memory[at..at + 8], &123u64.to_be_bytes(), "{from}/{to}");
    }
    // B951 -> B959 changes long DFP to extended DFP without changing RRF fields.
    let module = program(
        "         LGHI 3,123\n         LA 6,8\n         EX 6,TEMPLATE\n         STD 4,OUT\n         STD 6,OUT+8\n         B DONE\nTEMPLATE CDFTR 4,0,3,0\nDONE EQU *\n",
        "OUT DS 2D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 16], data("LD'123'"));
}

#[test]
fn decimal_load_integer_keeps_preferred_quantum_signed_zero_and_cc() {
    let module = program(
        "         LD 2,=DD'2.50'\n         XR 3,3\n         CR 3,3\n         FIDTR 0,8,2,0\n         BNE BAD\n         STD 0,OUT\n         LD 2,=DD'2.00'\n         FIDTR 0,8,2,0\n         STD 0,OUT+8\n         LD 2,=DD'-.2'\n         FIDTR 0,9,2,4\n         STD 0,OUT+16\n         LD 2,=DD'2E20'\n         FIDTR 0,8,2,0\n         STD 0,OUT+24\n         LD 4,WIDE\n         LD 6,WIDE+8\n         FIXTR 8,12,4,0\n         STD 8,OUT+32\n         STD 10,OUT+40\n         B DONE\nBAD L 0,4095(14)\nDONE EQU *\n",
        "WIDE DC LD'123456789012345678901234567890.5'\nOUT DS 6D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 8], data("DD'2'"));
    assert_eq!(&memory[at + 8..at + 16], data("DD'2'"));
    assert_eq!(&memory[at + 16..at + 24], data("DD'-0'"));
    assert_eq!(&memory[at + 24..at + 32], data("DD'2E20'"));
    assert_eq!(
        &memory[at + 32..at + 48],
        data("LD'123456789012345678901234567891'")
    );
}

#[test]
fn decimal_load_integer_quantum_exception_is_independent_of_inexact_suppression() {
    let module = program(
        "         LD 2,=DD'2.00'\n         FIDTR 0,4,2,4\n         STD 0,OUT\n         STFPC OUT+8\n",
        "OUT DS 2D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 8], data("DD'2'"));
    assert_eq!(&memory[at + 8..at + 12], hex("00040000"));
}

#[test]
fn decimal_exponent_significance_and_insertion_preserve_digits_and_flags() {
    let module = program(
        "         L 3,=X'FC000000'\n         SFPC 3\n         LD 2,=DD'1.00'\n         EEDTR 3,2\n         STG 3,OUT\n         ESDTR 3,2\n         STG 3,OUT+8\n         LGHI 3,400\n         LD 2,=DD'123.45'\n         IEDTR 0,2,3\n         STD 0,OUT+16\n         LD 2,=X'FE00000000000042'\n         EEDTR 3,2\n         STG 3,OUT+24\n         LGHI 3,398\n         IEDTR 0,2,3\n         STD 0,OUT+32\n         LGHI 3,-1\n         IEDTR 0,2,3\n         STD 0,OUT+40\n         LGHI 3,-3\n         IEDTR 0,2,3\n         STD 0,OUT+48\n         LGHI 3,768\n         IEDTR 0,2,3\n         STD 0,OUT+56\n         STFPC OUT+64\n",
        "OUT DS 9D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 16],
        hex("000000000000018C 0000000000000003")
    );
    assert_eq!(&memory[at + 16..at + 24], data("DD'12345E2'"));
    assert_eq!(&memory[at + 24..at + 32], &(-3i64).to_be_bytes());
    assert_eq!(&memory[at + 32..at + 40], data("DD'-42'"));
    assert_eq!(
        &memory[at + 40..at + 64],
        hex("F800000000000042 FE00000000000042 FC00000000000042")
    );
    assert_eq!(&memory[at + 64..at + 68], hex("FC000000"));
}

#[test]
fn extended_decimal_digit_operations_alias_safely_and_execute_extracts_significance() {
    let module = program(
        "         LD 4,NUMBER\n         LD 6,NUMBER+8\n         EEXTR 3,4\n         STG 3,OUT\n         ESXTR 3,4\n         STG 3,OUT+8\n         LGHI 3,6176\n         IEXTR 4,4,3\n         STD 4,OUT+16\n         STD 6,OUT+24\n         LD 2,=DD'1.00'\n         LA 5,2\n         EX 5,TEMPLATE\n         STG 3,OUT+32\n         B DONE\nTEMPLATE EEDTR 3,2\nDONE EQU *\n",
        "NUMBER DC LD'12345678901234567890123456789012.34'\nOUT DS 5D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 8], &6174u64.to_be_bytes());
    assert_eq!(&memory[at + 8..at + 16], &34u64.to_be_bytes());
    assert_eq!(
        &memory[at + 16..at + 32],
        data("LD'1234567890123456789012345678901234'")
    );
    assert_eq!(&memory[at + 32..at + 40], &3u64.to_be_bytes());
}

#[test]
fn decimal_division_quantum_rules_distinguish_infinity_from_exact_finite_division() {
    let module = program(
        "         LD 2,=DD'-1'\n         LD 4,=DD'(INF)'\n         DDTRA 0,2,4,4\n         STD 0,OUT\n         STFPC OUT+8\n         XR 3,3\n         SFPC 3\n         LD 2,=DD'1'\n         LD 4,=DD'2'\n         DDTRA 0,2,4,4\n         STD 0,OUT+16\n         STFPC OUT+24\n         LD 2,=DD'0E369'\n         LD 4,=DD'1E-398'\n         DDTRA 0,2,4,4\n         STD 0,OUT+32\n         STFPC OUT+40\n",
        "OUT DS 6D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 8], hex("8000000000000000"));
    assert_eq!(&memory[at + 8..at + 12], hex("00040000"));
    assert_eq!(&memory[at + 16..at + 24], data("DD'.5'"));
    assert_eq!(&memory[at + 24..at + 28], &[0; 4]);
    assert_eq!(&memory[at + 32..at + 40], data("DD'0E369'"));
    assert_eq!(&memory[at + 40..at + 44], hex("00040000"));
}

#[test]
fn decimal_quantize_and_reround_keep_requested_quantum_and_condition_code() {
    let module = program(
        "         LD 2,=DD'1.00'\n         LD 4,=DD'1.235'\n         XR 3,3\n         CR 3,3\n         QADTR 0,4,2,8\n         BNE BAD\n         STD 0,OUT\n         LGHI 3,3\n         LD 4,=DD'999.5'\n         RRDTR 0,4,3,8\n         STD 0,OUT+8\n         LGHI 3,2\n         LD 4,=DD'100.00'\n         RRDTR 0,4,3,8\n         STD 0,OUT+16\n         LGHI 3,0\n         RRDTR 0,4,3,8\n         STD 0,OUT+24\n         LD 4,=DD'-.001'\n         LD 2,=DD'1.00'\n         QADTR 0,4,2,9\n         STD 0,OUT+32\n         B DONE\nBAD L 0,4095(14)\nDONE EQU *\n",
        "OUT DS 5D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    for (index, expected) in [
        "DD'1.24'",
        "DD'100E1'",
        "DD'10E1'",
        "DD'100.00'",
        "DD'-0.00'",
    ]
    .iter()
    .enumerate()
    {
        assert_eq!(&memory[at + index * 8..at + index * 8 + 8], data(expected));
    }
}

#[test]
fn decimal_quantize_rejects_unrepresentable_quantum_and_mixed_infinity() {
    let module = program(
        "         LD 2,=DD'1E-20'\n         LD 4,=DD'1'\n         QADTR 0,4,2,4\n         STD 0,OUT\n         STFPC OUT+8\n         XR 3,3\n         SFPC 3\n         LD 2,=DD'(INF)'\n         QADTR 0,4,2,8\n         STD 0,OUT+16\n         LD 4,=DD'-(INF)'\n         QADTR 0,4,2,8\n         STD 0,OUT+24\n         LGHI 3,1\n         LD 4,=DD'(MAX)'\n         RRDTR 0,4,3,8\n         STD 0,OUT+32\n",
        "OUT DS 5D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 12], hex("7C00000000000000 00800000"));
    assert_eq!(
        &memory[at + 16..at + 40],
        hex("7C00000000000000 F800000000000000 7C00000000000000")
    );
}

#[test]
fn extended_quantize_and_reround_decode_distinct_register_positions_under_execute() {
    let module = program(
        "         LD 4,NUMBER\n         LD 6,NUMBER+8\n         LD 8,QUANTUM\n         LD 10,QUANTUM+8\n         QAXTR 0,4,8,8\n         STD 0,OUT\n         STD 2,OUT+8\n         LG 3,=X'ABCDEF0000000003'\n         RRXTR 0,4,3,8\n         STD 0,OUT+16\n         STD 2,OUT+24\n         LD 4,=DD'123.456'\n         LA 5,2\n         EX 5,TEMPLATE\n         STD 0,OUT+32\n         B DONE\nTEMPLATE QADTR 0,4,3,8\nDONE EQU *\n",
        "NUMBER DC LD'123456789012345678901234567890.1234'\nQUANTUM DC LD'1.00'\nOUT DS 5D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 16],
        data("LD'123456789012345678901234567890.12'")
    );
    assert_eq!(&memory[at + 16..at + 32], data("LD'123E27'"));
    // EX OR 2 selects RRDTR (B3F7) from QADTR (B3F5).
    assert_eq!(&memory[at + 32..at + 40], data("DD'123'"));
}

#[test]
fn hexadecimal_half_and_square_root_use_normalized_rounded_results() {
    let module = program(
        "         LE 2,=E'5'\n         HER 0,2\n         STE 0,OUT\n         LD 2,=D'5'\n         HDR 0,2\n         STD 0,OUT+8\n         LE 2,=E'2'\n         SQER 0,2\n         STE 0,OUT+16\n         LD 2,=D'2'\n         SQDR 0,2\n         STD 0,OUT+24\n         SQE 0,=E'2.25'\n         STE 0,OUT+32\n         SQD 0,=D'6.25'\n         STD 0,OUT+40\n         LD 4,EXTENDED\n         LD 6,EXTENDED+8\n         SQXR 8,4\n         STD 8,OUT+48\n         STD 10,OUT+56\n         LE 2,=X'00000001'\n         SQER 0,2\n         STE 0,OUT+64\n         LE 2,=X'FF000000'\n         SQER 0,2\n         STE 0,OUT+68\n",
        "EXTENDED DC LH'2.25'\nOUT DS 9D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 4], data("E'2.5'"));
    assert_eq!(&memory[at + 8..at + 16], data("D'2.5'"));
    assert_eq!(&memory[at + 16..at + 20], hex("4116A09E"));
    assert_eq!(&memory[at + 24..at + 32], hex("4116A09E667F3BCD"));
    assert_eq!(&memory[at + 32..at + 36], data("E'1.5'"));
    assert_eq!(&memory[at + 40..at + 48], data("D'2.5'"));
    assert_eq!(&memory[at + 48..at + 64], data("LH'1.5'"));
    assert_eq!(&memory[at + 64..at + 72], hex("1E100000 00000000"));
}

#[test]
fn hexadecimal_square_root_exception_and_execute_select_short_format() {
    let module = program(
        "         LE 2,=E'2.25'\n         LA 3,1\n         EX 3,TEMPLATE\n         STE 0,OUT\n         B DONE\nTEMPLATE SQDR 0,2\nDONE EQU *\n",
        "OUT DS D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 4], data("E'1.5'"));
    let module = program("         SQD 0,=D'-1'\n", "OUT DS D\n");
    let mut memory = vec![0; 4096];
    memory.extend(module.storage_at(4096).unwrap());
    let mark = memory.len();
    assert!(
        hlasm::execute(&module, &mut memory, 4096, &[])
            .unwrap_err()
            .contains("HFP square-root exception")
    );
    assert_eq!(memory.len(), mark);
}

#[test]
fn nominal_exponents_are_unbounded_and_combine_before_range_reduction() {
    assert_eq!(
        data("DB'1E999999999999999999999999999'"),
        hex("7FF0000000000000")
    );
    assert_eq!(
        data("EB'1E-999999999999999999999999999R6'"),
        hex("00000001")
    );
    assert_eq!(
        data("LD'0E999999999999999999999999999'"),
        data("LD'0E2147483647'")
    );
    assert_eq!(data("DBE(-2147483648)'1E2147483648'"), data("DB'1'"));
    assert_eq!(data("DE(-2147483648)'1E2147483648'"), data("D'1'"));
}

#[test]
fn hexadecimal_lengthening_preserves_unnormalized_characteristics_and_signed_zero() {
    let module = program(
        "         LE 2,=X'41001000'\n         LDER 0,2\n         STD 0,OUT\n         LXER 4,2\n         STD 4,OUT+8\n         STD 6,OUT+16\n         LXDR 8,0\n         STD 8,OUT+24\n         STD 10,OUT+32\n         LE 2,=X'FF000000'\n         LDER 0,2\n         STD 0,OUT+40\n         LXER 4,2\n         STD 4,OUT+48\n         STD 6,OUT+56\n         LDE 0,=E'1.5'\n         STD 0,OUT+64\n         LXD 4,=D'1.5'\n         STD 4,OUT+72\n         STD 6,OUT+80\n         LXE 8,=E'1.5'\n         STD 8,OUT+88\n         STD 10,OUT+96\n",
        "OUT DS 13D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &memory[at..at + 40],
        hex("4100100000000000 4100100000000000 3300000000000000 4100100000000000 3300000000000000")
    );
    assert_eq!(
        &memory[at + 40..at + 64],
        hex("FF00000000000000 8000000000000000 8000000000000000")
    );
    assert_eq!(&memory[at + 64..at + 72], data("D'1.5'"));
    assert_eq!(&memory[at + 72..at + 88], data("LH'1.5'"));
    assert_eq!(&memory[at + 88..at + 104], data("LH'1.5'"));
}

#[test]
fn hexadecimal_load_integer_normalizes_truncates_and_returns_positive_true_zero() {
    let module = program(
        "         LE 2,=E'-2.75'\n         FIER 0,2\n         STE 0,OUT\n         LD 2,=X'C100100000000000'\n         FIDR 0,2\n         STD 0,OUT+8\n         LD 4,EXTENDED\n         LD 6,EXTENDED+8\n         FIXR 8,4\n         STD 8,OUT+16\n         STD 10,OUT+24\n         LE 2,=E'1E60'\n         FIER 0,2\n         STE 0,OUT+32\n",
        "EXTENDED DC LH'1234567890.75'\nOUT DS 5D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 4], data("E'-2'"));
    assert_eq!(&memory[at + 8..at + 16], &[0; 8]);
    assert_eq!(&memory[at + 16..at + 32], data("LH'1234567890'"));
    assert_eq!(&memory[at + 32..at + 36], data("E'1E60'"));
}

#[test]
fn hexadecimal_fused_operations_keep_exact_product_and_use_rrd_rxf_positions() {
    let module = program(
        "         LE 0,=E'-1'\n         LE 2,=E'1.00000095367431640625'\n         LE 4,=E'.99999904632568359375'\n         MAER 0,2,4\n         STE 0,OUT\n         LE 0,=E'1'\n         MSER 0,2,4\n         STE 0,OUT+4\n         LE 0,=E'1'\n         LA 3,1\n         EX 3,TEMPLATE\n         STE 0,OUT+8\n         LE 0,=E'-1'\n         MAE 0,2,=E'.99999904632568359375'\n         STE 0,OUT+12\n         LE 0,=E'1'\n         MSE 0,2,=E'.99999904632568359375'\n         STE 0,OUT+16\n         LD 0,=D'-1'\n         LD 2,=D'1.5'\n         LD 4,=D'2.25'\n         MADR 0,2,4\n         STD 0,OUT+24\n         LD 0,=D'1'\n         MSDR 0,2,4\n         STD 0,OUT+32\n         LD 0,=D'-1'\n         MAD 0,2,=D'2.25'\n         STD 0,OUT+40\n         LD 0,=D'1'\n         MSD 0,2,=D'2.25'\n         STD 0,OUT+48\n         B DONE\nTEMPLATE MAER 0,2,4\nDONE EQU *\n",
        "OUT DS 7D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    for n in 0..5 {
        assert_eq!(&memory[at + n * 4..at + n * 4 + 4], hex("B7100000"));
    }
    for n in 0..4 {
        assert_eq!(&memory[at + 24 + n * 8..at + 32 + n * 8], data("D'2.375'"));
    }
}

#[test]
fn hexadecimal_unnormalized_multiply_add_covers_full_high_low_register_and_memory_forms() {
    for (op, add, part, memory) in [
        ("MYR", false, 0, false),
        ("MYHR", false, 1, false),
        ("MYLR", false, 2, false),
        ("MY", false, 0, true),
        ("MYH", false, 1, true),
        ("MYL", false, 2, true),
        ("MAYR", true, 0, false),
        ("MAYHR", true, 1, false),
        ("MAYLR", true, 2, false),
        ("MAY", true, 0, true),
        ("MAYH", true, 1, true),
        ("MAYL", true, 2, true),
    ] {
        let operand = if memory { "=D'1'" } else { "4" };
        let module = program(
            &format!(
                "         LD 0,=D'1'\n         LD 2,=D'1'\n         LD 4,=D'1'\n         {op} 0,2,{operand}\n         STD 0,OUT\n         STD 2,OUT+8\n"
            ),
            "OUT DS 2D\n",
        );
        let mem = run(&module);
        let at = 4096 + module.entries["OUT"] as usize;
        let hi = if add {
            hex("4202000000000000")
        } else {
            hex("4201000000000000")
        };
        let lo = hex("3400000000000000");
        match part {
            0 => {
                assert_eq!(&mem[at..at + 8], hi, "{op}");
                assert_eq!(&mem[at + 8..at + 16], lo, "{op}");
            }
            1 => assert_eq!(&mem[at..at + 8], hi, "{op}"),
            _ => assert_eq!(&mem[at..at + 8], lo, "{op}"),
        }
    }
}

#[test]
fn hexadecimal_unnormalized_exponents_zero_signs_pair_aliases_and_execute_fields() {
    let module = program(
        "         L 3,=X'0F000000'\n         SPM 3\n         LD 2,=D'1'\n         LD 4,=D'1'\n         LD 6,=D'1'\n         MAYR 2,4,6\n         STD 0,OUT\n         STD 2,OUT+8\n         LD 2,=X'7FFFFFFFFFFFFFFF'\n         MYR 0,2,2\n         STD 0,OUT+16\n         STD 2,OUT+24\n         LD 2,=X'C100000000000000'\n         LD 4,=X'4100000000000000'\n         MYR 0,2,4\n         STD 0,OUT+32\n         STD 2,OUT+40\n         LD 0,=X'C100000000000000'\n         LD 2,=X'C100000000000000'\n         MAYR 0,2,4\n         STD 0,OUT+48\n         STD 2,OUT+56\n         LD 0,=D'1'\n         LD 2,=D'1'\n         LD 4,=D'1'\n         LA 3,1\n         EX 3,TEMPLATE\n         STD 0,OUT+64\n         LD 0,=D'1'\n         LD 2,=D'3'\n         LD 6,=D'2'\n         LA 3,64\n         EX 3,MEMORY\n         STD 0,OUT+72\n         B DONE\nTEMPLATE MAYR 0,2,4\nMEMORY MAY 0,2,=D'1'\nDONE EQU *\n",
        "OUT DS 10D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &mem[at..at + 80],
        hex(
            "4202000000000000 3400000000000000 3EFFFFFFFFFFFFFE 3000000000000001 C200000000000000 B400000000000000 4200000000000000 3400000000000000 4201000000000000 4203000000000000"
        )
    );
}

#[test]
fn invalid_decimal_integer_conversion_does_not_recognize_inexact() {
    let module = program(
        "         L 3,=X'08000000'\n         SFPC 3\n         LD 0,=DD'(NAN)'\n         CGDTRA 4,8,0,0\n         STG 4,OUT\n         STFPC OUT+8\n         LD 0,=DD'1E100'\n         CLGDTR 4,8,0,0\n         STG 4,OUT+16\n         STFPC OUT+24\n",
        "OUT DS 4D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 12], hex("8000000000000000 08800000"));
    assert_eq!(&mem[at + 16..at + 28], hex("FFFFFFFFFFFFFFFF 08800000"));
}

#[test]
fn decimal_register_packed_conversions_use_significands_not_numeric_exponents() {
    let module = program(
        "         LG 3,=X'123456789012345D'\n         CDSTR 0,3\n         STD 0,OUT\n         CSDTR 5,0,1\n         STG 5,OUT+8\n         LG 3,=X'9876543210123456'\n         CDUTR 0,3\n         STD 0,OUT+16\n         CUDTR 5,0\n         STG 5,OUT+24\n         LD 0,=DD'9876543210123456E-100'\n         CUDTR 5,0\n         STG 5,OUT+32\n         LD 0,=DD'1234567890123456'\n         CSDTR 5,0,0\n         STG 5,OUT+40\n         LD 0,=DD'12'\n         EX 0,TEMPLATE\n         STG 5,OUT+48\n         B DONE\nTEMPLATE CSDTR 5,0,1\nDONE EQU *\n",
        "OUT DS 7D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 8], data("DD'-123456789012345'"));
    assert_eq!(&mem[at + 8..at + 16], hex("123456789012345D"));
    assert_eq!(&mem[at + 16..at + 24], data("DD'9876543210123456'"));
    assert_eq!(
        &mem[at + 24..at + 56],
        hex("9876543210123456 9876543210123456 234567890123456C 000000000000012F")
    );
}

#[test]
fn extended_register_packed_conversions_keep_full_register_pairs() {
    let module = program(
        "         LG 6,=X'1234567890123456'\n         LG 7,=X'7890123456789012'\n         CXUTR 4,6\n         STD 4,OUT\n         STD 6,OUT+8\n         CUXTR 8,4\n         STMG 8,9,OUT+16\n         LG 6,=X'1234567890123456'\n         LG 7,=X'789012345678912D'\n         CXSTR 4,6\n         STD 4,OUT+32\n         STD 6,OUT+40\n         CSXTR 8,4,0\n         STMG 8,9,OUT+48\n",
        "OUT DS 8D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &mem[at..at + 16],
        data("LD'12345678901234567890123456789012'")
    );
    assert_eq!(
        &mem[at + 16..at + 32],
        hex("1234567890123456 7890123456789012")
    );
    assert_eq!(
        &mem[at + 32..at + 48],
        data("LD'-1234567890123456789012345678912'")
    );
    assert_eq!(
        &mem[at + 48..at + 64],
        hex("1234567890123456 789012345678912D")
    );
}

#[test]
fn register_packed_special_values_do_not_signal_ieee_and_bad_digits_are_data_errors() {
    let module = program(
        "         L 3,=X'FC000000'\n         SFPC 3\n         LD 0,=X'FE00000000000042'\n         CUDTR 5,0\n         STG 5,OUT\n         CSDTR 5,0,0\n         STG 5,OUT+8\n         STFPC OUT+16\n         LG 3,=X'000000000000000B'\n         CDSTR 0,3\n         STD 0,OUT+24\n",
        "OUT DS 4D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(
        &mem[at..at + 20],
        hex("0000000000000042 000000000000042D FC000000")
    );
    assert_eq!(&mem[at + 24..at + 32], data("DD'-0'"));
    for (op, bits) in [("CDSTR", "0000000000000019"), ("CDUTR", "000000000000000A")] {
        let module = program(
            &format!("         LG 3,=X'{bits}'\n         {op} 0,3\n"),
            "OUT DS D\n",
        );
        let mut mem = vec![0; 4096];
        mem.extend(module.storage_at(4096).unwrap());
        assert!(
            hlasm::execute(&module, &mut mem, 4096, &[])
                .unwrap_err()
                .contains("general-operand data exception")
        );
    }
}

#[test]
fn decimal_memory_packed_conversions_validate_lengths_sign_controls_and_full_precision() {
    let module = program(
        "         CDPT 0,SIGNED(4),8\n         STD 0,OUT\n         CPDT 0,OUT+8(4),8\n         CDPT 0,IGNORE(2),9\n         STD 0,OUT+16\n         CDPT 0,UNSIGNED(9),0\n         STD 0,OUT+24\n         CXPT 4,WIDE(18),8\n         STD 4,OUT+32\n         STD 6,OUT+40\n         CPXT 4,OUT+48(18),8\n         LA 3,4\n         EX 3,TEMPLATE\n         STD 0,OUT+72\n         B DONE\nTEMPLATE CDPT 0,LONGER(1),8\nDONE EQU *\n",
        "SIGNED DC PL4'-1234567'\nIGNORE DC X'1230'\nUNSIGNED DC X'009876543210123456'\nWIDE DC PL18'1234567890123456789012345678901234'\nLONGER DC PL5'123456789'\nOUT DS 10D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 8], data("DD'-1234567'"));
    assert_eq!(&mem[at + 8..at + 12], hex("1234567D"));
    assert_eq!(&mem[at + 16..at + 24], data("DD'123'"));
    assert_eq!(&mem[at + 24..at + 32], data("DD'9876543210123456'"));
    assert_eq!(
        &mem[at + 32..at + 48],
        data("LD'1234567890123456789012345678901234'")
    );
    assert_eq!(
        &mem[at + 48..at + 66],
        hex("01234567890123456789012345678901234C")
    );
    assert_eq!(&mem[at + 72..at + 80], data("DD'123456789'"));
}

#[test]
fn decimal_memory_packed_output_truncation_cc_and_force_positive_zero() {
    let module = program(
        "         LD 0,=DD'12345E-20'\n         CPDT 0,OUT(2),8\n         IPM 3\n         ST 3,OUT+4\n         LD 0,=DD'-0.0'\n         CPDT 0,OUT+8(1),10\n         CPDT 0,OUT+9(1),11\n         LD 0,=X'FE00000000000042'\n         CPDT 0,OUT+10(2),8\n         IPM 3\n         ST 3,OUT+12\n         STFPC OUT+16\n",
        "OUT DS 3D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 2], hex("345C"));
    assert_eq!(&mem[at + 4..at + 8], hex("30000000"));
    assert_eq!(&mem[at + 8..at + 12], hex("0D0F 042D"));
    assert_eq!(&mem[at + 12..at + 20], hex("30000000 00000000"));
    let module = program(
        "         L 3,=X'04000000'\n         SPM 3\n         LD 0,=DD'-1000'\n         CPDT 0,OUT(1),9\n",
        "OUT DS D\n",
    );
    let mut mem = vec![0; 4096];
    mem.extend(module.storage_at(4096).unwrap());
    let mark = mem.len();
    assert!(
        hlasm::execute(&module, &mut mem, 4096, &[])
            .unwrap_err()
            .contains("decimal-overflow exception")
    );
    assert_eq!(mem[4096 + module.entries["OUT"] as usize], 0x0c);
    assert_eq!(mem.len(), mark);
}

#[test]
fn packed_memory_nonzero_unused_digits_and_invalid_signs_are_data_errors() {
    for (value, length, mask) in [("010000000000000000", 9, 0), ("1230", 2, 8), ("12AC", 2, 8)] {
        let module = program(
            &format!("         CDPT 0,INPUT({length}),{mask}\n"),
            &format!("INPUT DC X'{value}'\nOUT DS D\n"),
        );
        let mut mem = vec![0; 4096];
        mem.extend(module.storage_at(4096).unwrap());
        assert!(
            hlasm::execute(&module, &mut mem, 4096, &[])
                .unwrap_err()
                .contains("general-operand data exception")
        );
    }
}

#[test]
fn decimal_zoned_conversions_use_zone_and_sign_controls_at_both_precisions() {
    let module = program(
        "         CDZT 0,SIGNED(4),8\n         STD 0,OUT\n         CZDT 0,OUT+8(4),8\n         CDZT 0,ASCII(4),0\n         STD 0,OUT+16\n         CZDT 0,OUT+24(4),4\n         CDZT 0,ZONES(4),0\n         STD 0,OUT+32\n         CXZT 4,WIDE(34),0\n         STD 4,OUT+40\n         STD 6,OUT+48\n         CZXT 4,OUT+56(34),0\n",
        "SIGNED DC X'F1F2F3D4'\nASCII DC X'31323334'\nZONES DC X'A1B2C3F4'\nWIDE DC C'1234567890123456789012345678901234'\nOUT DS 12D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 8], data("DD'-1234'"));
    assert_eq!(&mem[at + 8..at + 12], hex("F1F2F3D4"));
    assert_eq!(&mem[at + 16..at + 24], data("DD'1234'"));
    assert_eq!(&mem[at + 24..at + 28], b"1234");
    assert_eq!(&mem[at + 32..at + 40], data("DD'1234'"));
    assert_eq!(
        &mem[at + 40..at + 56],
        data("LD'1234567890123456789012345678901234'")
    );
    assert_eq!(
        &mem[at + 56..at + 90],
        zeus_cobol::ebcdic::encode("1234567890123456789012345678901234")
    );
}

#[test]
fn decimal_zoned_output_truncation_force_plus_and_input_sign_validation() {
    let module = program(
        "         LD 0,=DD'-1000'\n         CZDT 0,OUT(3),11\n         IPM 3\n         ST 3,OUT+4\n         LD 0,=DD'-0'\n         CZDT 0,OUT+8(1),8\n         CZDT 0,OUT+9(1),15\n         LD 0,=X'FE00000000000042'\n         CZDT 0,OUT+10(2),8\n         STFPC OUT+12\n",
        "OUT DS 2D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 3], hex("F0F0F0"));
    assert_eq!(&mem[at + 4..at + 8], hex("30000000"));
    assert_eq!(&mem[at + 8..at + 16], hex("D0 F0 F4D2 00000000"));
    for (value, mask) in [("31323334", 8), ("31323334", 9), ("F1F2FA", 0)] {
        let length = value.len() / 2;
        let module = program(
            &format!("         CDZT 0,INPUT({length}),{mask}\n"),
            &format!("INPUT DC X'{value}'\nOUT DS D\n"),
        );
        let mut mem = vec![0; 4096];
        mem.extend(module.storage_at(4096).unwrap());
        assert!(
            hlasm::execute(&module, &mut mem, 4096, &[])
                .unwrap_err()
                .contains("general-operand data exception")
        );
    }
}

#[test]
fn binary_load_integer_supports_rounding_masks_signed_zero_and_extended_precision() {
    let module = program(
        "         LE 2,=EB'1.5'\n         FIEBR 0,4,2\n         STE 0,OUT\n         FIEBRA 0,5,2,4\n         STE 0,OUT+4\n         LD 2,=DB'-.25'\n         FIDBR 0,5,2\n         STD 0,OUT+8\n         LE 2,=EB'2.2'\n         FIEBRA 0,3,2,4\n         STE 0,OUT+16\n         LD 4,EXTENDED\n         LD 6,EXTENDED+8\n         FIXBR 8,1,4\n         STD 8,OUT+24\n         STD 10,OUT+32\n         FIXBRA 8,4,4,4\n         STD 8,OUT+40\n         STD 10,OUT+48\n         LD 2,=DB'1.5'\n         FIDBRA 0,4,2,4\n         STD 0,OUT+56\n",
        "EXTENDED DC LB'123456789012345678901234567890.5'\nOUT DS 8D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 8], hex("40000000 3F800000"));
    assert_eq!(&mem[at + 8..at + 16], data("DB'-0'"));
    assert_eq!(&mem[at + 16..at + 20], data("EB'3'"));
    assert_eq!(
        &mem[at + 24..at + 40],
        data("LB'123456789012345678901234567891'")
    );
    assert_eq!(
        &mem[at + 40..at + 56],
        data("LB'123456789012345678901234567890'")
    );
    assert_eq!(&mem[at + 56..at + 64], data("DB'2'"));
}

#[test]
fn binary_load_rounded_advanced_forms_keep_rounding_and_suppression_through_execute() {
    let module = program(
        "         L 3,=X'08000000'\n         SFPC 3\n         LD 2,=DB'1.00000001'\n         EX 0,TEMPLATE\n         STE 0,OUT\n         STFPC OUT+4\n         LD 4,EXTENDED\n         LD 6,EXTENDED+8\n         LDXBRA 8,6,4,4\n         STD 8,OUT+8\n         LEXBRA 8,3,4,4\n         STE 8,OUT+16\n         B DONE\nTEMPLATE LEDBRA 0,6,2,4\nDONE EQU *\n",
        "EXTENDED DC LB'1.0000000000000000000000000000000002'\nOUT DS 3D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 8], hex("3F800001 08000000"));
    assert_eq!(&mem[at + 8..at + 16], hex("3FF0000000000001"));
    assert_eq!(&mem[at + 16..at + 20], hex("3F800001"));
}

#[test]
fn binary_hex_radix_conversions_report_cc_without_ieee_exceptions() {
    let module = program(
        "         L 3,=X'FC000000'\n         SFPC 3\n         LE 2,=EB'1.5'\n         THDER 0,2\n         STD 0,OUT\n         IPM 3\n         ST 3,OUT+8\n         LD 2,=DB'-1E-300'\n         THDR 0,2\n         STD 0,OUT+16\n         IPM 3\n         ST 3,OUT+24\n         LD 2,=DB'-(SNAN)'\n         THDR 0,2\n         STD 0,OUT+32\n         STFPC OUT+40\n         LD 2,=D'1.5'\n         TBDR 0,0,2\n         STD 0,OUT+48\n         LD 0,=X'0000000012345678'\n         LD 2,=X'0000000000000001'\n         TBEDR 0,6,2\n         STE 0,OUT+56\n         STD 0,OUT+64\n         LD 2,=X'7FFFFFFFFFFFFFFF'\n         TBEDR 0,3,2\n         STE 0,OUT+72\n         TBEDR 0,1,2\n         STE 0,OUT+76\n         TBEDR 0,0,2\n         STE 0,OUT+80\n         STFPC OUT+84\n         TBDR 0,0,2\n         STD 0,OUT+88\n",
        "OUT DS 12D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 12], hex("4118000000000000 20000000"));
    assert_eq!(&mem[at + 16..at + 28], hex("8000000000000000 10000000"));
    assert_eq!(&mem[at + 32..at + 44], hex("7FFFFFFFFFFFFFFF FC000000"));
    assert_eq!(&mem[at + 48..at + 56], data("DB'1.5'"));
    assert_eq!(&mem[at + 56..at + 60], hex("00000001"));
    assert_eq!(
        &mem[at + 64..at + 96],
        hex("0000000112345678 7F7FFFFF 7F800000 7F7FFFFF FC000000 4FAFFFFFFFFFFFFF")
    );
}

#[test]
fn copy_sign_is_radix_independent_and_execute_preserves_three_register_positions() {
    let module = program(
        "         LD 2,=X'8000000000000000'\n         LD 4,=X'7FF0000000000042'\n         L 3,=X'FC000000'\n         SFPC 3\n         CR 3,3\n         EX 0,TEMPLATE\n         BNE BAD\n         STD 0,OUT\n         STFPC OUT+8\n         B DONE\nBAD L 0,4095(14)\nTEMPLATE CPSDR 0,2,4\nDONE EQU *\n",
        "OUT DS 2D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 12], hex("FFF0000000000042 FC000000"));
}

#[test]
fn prepare_rounding_overflow_delivers_largest_finite_binary_and_decimal_results() {
    assert_eq!(data("ED'1E999999999999999999999R15'"), data("ED'(MAX)'"));
    let module = program(
        "         SRNMB 7\n         LE 0,=EB'(MAX)'\n         MEEB 0,=EB'2'\n         STE 0,OUT\n         STFPC OUT+4\n         XR 3,3\n         SFPC 3\n         SRNMT 7\n         LD 2,=DD'(MAX)'\n         LD 4,=DD'10'\n         MDTR 0,2,4\n         STD 0,OUT+8\n         STFPC OUT+16\n",
        "OUT DS 3D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 8], hex("7F7FFFFF 00280007"));
    assert_eq!(&mem[at + 8..at + 16], data("DD'(MAX)'"));
    assert_eq!(&mem[at + 16..at + 20], hex("00280070"));
}

#[test]
fn data_classification_covers_all_sign_class_bits_and_all_binary_decimal_widths() {
    for (op, kind, width, decimal) in [
        ("TCEB", "EB", 4, false),
        ("TCDB", "DB", 8, false),
        ("TCXB", "LB", 16, false),
        ("TDCET", "ED", 4, true),
        ("TDCDT", "DD", 8, true),
        ("TDCXT", "LD", 16, true),
    ] {
        let classes = if decimal {
            ["0", "(DMIN)", "1", "(INF)", "(NAN)", "(SNAN)"]
        } else {
            ["0", "1", "(DMIN)", "(INF)", "(NAN)", "(SNAN)"]
        };
        let mut body = String::from("         L 3,=X'FC000000'\n         SFPC 3\n");
        let mut declarations = String::new();
        for (class, value) in classes.iter().enumerate() {
            for sign in 0..2 {
                let n = class * 2 + sign;
                let nominal = format!("{}{value}", if sign == 1 { "-" } else { "" });
                declarations.push_str(&format!("V{n} DC {kind}'{nominal}'\n"));
                body.push_str(&format!(
                    "         {} 0,V{n}\n",
                    if width == 4 { "LE" } else { "LD" }
                ));
                if width == 16 {
                    body.push_str(&format!("         LD 2,V{n}+8\n"));
                }
                let mask = 1u32 << (11 - n);
                body.push_str(&format!("         {op} 0,{mask}\n         BC 4,Y{n}\n         L 0,4095(14)\nY{n} {op} 0,{}\n         BE N{n}\n         L 0,4095(14)\nN{n} EQU *\n",4095^mask));
            }
        }
        body.push_str("         STFPC OUT\n");
        declarations.push_str("OUT DS F\n");
        let module = program(&body, &declarations);
        let mem = run(&module);
        let at = 4096 + module.entries["OUT"] as usize;
        assert_eq!(&mem[at..at + 4], hex("FC000000"), "{op}");
    }
}

#[test]
fn decimal_group_masks_cover_safe_extreme_full_precision_and_special_values() {
    for (op, kind, width, maximum, full) in [
        ("TDGET", "ED", 4, 90, "1234567"),
        ("TDGDT", "DD", 8, 369, "1234567890123456"),
        (
            "TDGXT",
            "LD",
            16,
            6111,
            "1234567890123456789012345678901234",
        ),
    ] {
        let values = [
            "0".to_string(),
            format!("0E{maximum}"),
            format!("1E{maximum}"),
            "1".to_string(),
            full.to_string(),
            "(SNAN)".to_string(),
        ];
        let mut body = String::new();
        let mut declarations = String::new();
        for (group, value) in values.iter().enumerate() {
            for sign in 0..2 {
                let n = group * 2 + sign;
                let mask = 1u32 << (11 - n);
                declarations.push_str(&format!(
                    "V{n} DC {kind}'{}{value}'\n",
                    if sign == 1 { "-" } else { "" }
                ));
                body.push_str(&format!(
                    "         {} 0,V{n}\n",
                    if width == 4 { "LE" } else { "LD" }
                ));
                if width == 16 {
                    body.push_str(&format!("         LD 2,V{n}+8\n"));
                }
                body.push_str(&format!("         {op} 0,{mask}\n         BC 4,N{n}\n         L 0,4095(14)\nN{n} EQU *\n"));
            }
        }
        declarations.push_str("OUT DS F\n");
        run(&program(&body, &declarations));
    }
}

#[test]
fn classification_and_digit_shifts_ignore_high_mask_address_bits_in_amode64() {
    let module = program(
        "         SAM64\n         LG 2,=X'8000000000000800'\n         LE 0,=EB'0'\n         TCEB 0,0(2)\n         BC 4,OK\n         L 0,4095(14)\nOK LD 4,=DD'1.23'\n         LG 2,=X'8000000000000001'\n         SLDT 0,4,0(2)\n         STD 0,OUT\n         SAM31\n",
        "OUT DS D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 8], data("DD'12.30'"));
}

#[test]
fn decimal_biased_exponent_compare_ignores_sign_and_does_not_signal_nan() {
    for (left, right, cc) in [
        ("-1.0", "100", 1),
        ("0E2", "1E1", 2),
        ("-(INF)", "(INF)", 0),
        ("(SNAN)", "(NAN)", 0),
        ("(INF)", "1", 3),
    ] {
        for (op, kind, width) in [("CEDTR", "DD", 8), ("CEXTR", "LD", 16)] {
            let mut body = String::from(
                "         L 3,=X'FC000000'\n         SFPC 3\n         LD 0,A\n         LD 4,B\n",
            );
            if width == 16 {
                body.push_str("         LD 2,A+8\n         LD 6,B+8\n");
            }
            body.push_str(&format!(
                "         {op} 0,4\n         BC {},OK\n         L 0,4095(14)\nOK STFPC OUT\n",
                8 >> cc
            ));
            let module = program(
                &body,
                &format!("A DC {kind}'{left}'\nB DC {kind}'{right}'\nOUT DS F\n"),
            );
            let mem = run(&module);
            let at = 4096 + module.entries["OUT"] as usize;
            assert_eq!(&mem[at..at + 4], hex("FC000000"));
        }
    }
}

#[test]
fn decimal_digit_shifts_keep_quantum_sign_and_special_encodings_without_exceptions() {
    let module = program(
        "         LD 2,=DD'123.45'\n         SLDT 0,2,2\n         STD 0,OUT\n         SRDT 0,2,2\n         STD 0,OUT+8\n         SLDT 0,2,63\n         STD 0,OUT+16\n         LD 2,=X'FE00000000000042'\n         SRDT 0,2,1\n         STD 0,OUT+24\n         SLDT 0,2,2\n         STD 0,OUT+32\n         LD 2,=X'FA00000000000042'\n         SRDT 0,2,0\n         STD 0,OUT+40\n         LD 4,EXTENDED\n         LD 6,EXTENDED+8\n         SLXT 8,4,2\n         STD 8,OUT+48\n         STD 10,OUT+56\n         SRXT 4,4,2\n         STD 4,OUT+64\n         STD 6,OUT+72\n",
        "EXTENDED DC LD'1234567890123456789012345678901234'\nOUT DS 10D\n",
    );
    let mem = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 8], data("DD'12345.00'"));
    assert_eq!(&mem[at + 8..at + 16], data("DD'1.23'"));
    assert_eq!(&mem[at + 16..at + 24], data("DD'0.00'"));
    assert_eq!(
        &mem[at + 24..at + 48],
        hex("FE00000000000004 FE00000000001100 F800000000000042")
    );
    assert_eq!(
        &mem[at + 48..at + 64],
        data("LD'3456789012345678901234567890123400'")
    );
    assert_eq!(
        &mem[at + 64..at + 80],
        data("LD'12345678901234567890123456789012'")
    );
}

#[test]
fn pfpo_all_fifty_four_cross_radix_functions_execute_after_module_round_trip() {
    let formats = [
        (0, "EH", 4),
        (1, "DH", 8),
        (2, "LH", 16),
        (5, "EB", 4),
        (6, "DB", 8),
        (7, "LB", 16),
        (8, "ED", 4),
        (9, "DD", 8),
        (10, "LD", 16),
    ];
    let mut functions = 0;
    for (source, input, input_width) in formats {
        for (target, output, output_width) in formats {
            if (source <= 2 && target <= 2)
                || ((5..=7).contains(&source) && (5..=7).contains(&target))
                || (source >= 8 && target >= 8)
            {
                continue;
            }
            // All three representations hold these values exactly. For DFP
            // targets, DPQC=1 selects the source constant's natural quantum.
            for value in ["1.5", "-1.5", "0", "-0"] {
                let control = 0x01000008u32
                    | ((target as u32) << 16)
                    | ((source as u32) << 8)
                    | if target >= 8 { 0x10 } else { 0 };
                let mut body = format!(
                    "         LG 1,=X'89ABCDEF12345678'\n         LD 0,=X'0123456789ABCDEF'\n         {} 4,INPUT\n",
                    if input_width == 4 { "LE" } else { "LD" }
                );
                if input_width == 16 {
                    body.push_str("         LD 6,INPUT+8\n");
                }
                body.push_str(&format!("         L 0,=X'{control:08X}'\n         EX 0,TEMPLATE\n         BE OK\n         L 0,4095(14)\nOK STD 0,OUT\n"));
                if output_width == 16 {
                    body.push_str("         STD 2,OUT+8\n");
                }
                body.push_str("         STG 1,OUT+16\n         STFPC OUT+24\n         B FINISH\nTEMPLATE PFPO\nFINISH EQU *\n");
                let module = program(&body, &format!("INPUT DC {input}'{value}'\nOUT DS 4D\n"));
                let mem = run(&module);
                let at = 4096 + module.entries["OUT"] as usize;
                assert_eq!(
                    &mem[at..at + output_width],
                    data(&format!("{output}'{value}'")),
                    "{input} -> {output}, {value}"
                );
                if output_width == 4 {
                    assert_eq!(&mem[at + 4..at + 8], hex("89ABCDEF"));
                }
                assert_eq!(&mem[at + 16..at + 28], hex("89ABCDEF00000000 00000000"));
            }
            functions += 1;
        }
    }
    assert_eq!(functions, 54);
}

#[test]
fn pfpo_rounding_uses_explicit_decimal_and_binary_rounding_selectors() {
    // Decimal128 holds the exact midpoint above BFP32 1.0.
    for (controls, fpc, expected) in [
        (8, 0, "3F800000"),
        (12, 0, "3F800001"),
        (13, 0, "3F800000"),
        (15, 0, "3F800001"),
        (0, 0x20, "3F800001"),
        (1, 2, "3F800001"),
    ] {
        let control = 0x01050A00u32 | controls;
        let module = program(
            &format!(
                "         LD 4,INPUT\n         LD 6,INPUT+8\n         L 3,=X'{fpc:08X}'\n         SFPC 3\n         L 0,=X'{control:08X}'\n         PFPO\n         BC 4,OK\n         L 0,4095(14)\nOK STE 0,OUT\n         STFPC OUT+4\n"
            ),
            "INPUT DC LD'1.000000059604644775390625'\nOUT DS D\n",
        );
        let mem = run(&module);
        let at = 4096 + module.entries["OUT"] as usize;
        assert_eq!(&mem[at..at + 4], hex(expected));
        assert_eq!(&mem[at + 4..at + 8], (fpc | 0x00080000u32).to_be_bytes());
    }
}

#[test]
fn binary_memory_lengthening_and_long_to_extended_products_keep_all_bits() {
    let module = program(
        "         LDEB 0,=EB'1.5'\n         STD 0,OUT\n         LXEB 4,=EB'1.5'\n         STD 4,OUT+8\n         STD 6,OUT+16\n         LXDB 8,=X'3FF0000000000001'\n         STD 8,OUT+24\n         STD 10,OUT+32\n         LD 0,=X'3FF0000000000001'\n         MXDB 0,=X'3FF0000000000001'\n         STD 0,OUT+40\n         STD 2,OUT+48\n         LD 4,=X'3FF0000000000001'\n         LD 8,=X'3FF0000000000001'\n         MXDBR 4,8\n         STD 4,OUT+56\n         STD 6,OUT+64\n",
        "OUT DS 9D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 8], hex("3FF8000000000000"));
    assert_eq!(
        &memory[at + 8..at + 24],
        hex("3FFF800000000000 0000000000000000")
    );
    assert_eq!(
        &memory[at + 24..at + 40],
        hex("3FFF000000000000 1000000000000000")
    );
    assert_eq!(
        &memory[at + 40..at + 56],
        hex("3FFF000000000000 2000000000000100")
    );
    assert_eq!(&memory[at + 56..at + 72], &memory[at + 40..at + 56]);
}

#[test]
fn binary_memory_lengthening_preserves_special_values_and_validates_pairs() {
    let module = program(
        "         LDEB 0,=X'80000000'\n         STD 0,OUT\n         LXEB 4,=X'FF800000'\n         STD 4,OUT+8\n         STD 6,OUT+16\n         LXDB 8,=X'7FF8000000000042'\n         STD 8,OUT+24\n         STD 10,OUT+32\n         LDEB 0,=X'7F800001'\n         STD 0,OUT+40\n         STFPC OUT+48\n",
        "OUT DS 7D\n",
    );
    let memory = run(&module);
    let at = 4096 + module.entries["OUT"] as usize;
    assert_eq!(&memory[at..at + 8], hex("8000000000000000"));
    assert_eq!(
        &memory[at + 8..at + 24],
        hex("FFFF000000000000 0000000000000000")
    );
    assert_eq!(
        &memory[at + 24..at + 40],
        hex("7FFF800000000004 2000000000000000")
    );
    assert_eq!(&memory[at + 40..at + 48], hex("7FF8000020000000"));
    assert_ne!(memory[at + 49] & 0x80, 0);
    for op in ["LXDB", "LXEB", "MXDB"] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid-pair.asm");
        std::fs::write(
            &path,
            format!("MAIN CSECT\n USING MAIN,15\n {op} 2,=DB'1'\n BR 14\n END\n"),
        )
        .unwrap();
        let report = hlasm::compile_report(&path, &[]);
        assert!(
            report.module.is_none(),
            "{op}: invalid extended pair accepted"
        );
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.message.contains("extended floating-point register")),
            "{op}: {:?}",
            report.diagnostics
        );
    }
}

#[test]
fn divide_to_integer_uses_final_quotient_rounding_and_preserves_divisor() {
    for (mask, quotient, remainder) in [
        (0, "4010000000000000", "BFF0000000000000"),
        (1, "4010000000000000", "BFF0000000000000"),
        (3, "4008000000000000", "3FF0000000000000"),
        (4, "4010000000000000", "BFF0000000000000"),
        (5, "4008000000000000", "3FF0000000000000"),
        (6, "4010000000000000", "BFF0000000000000"),
        (7, "4008000000000000", "3FF0000000000000"),
    ] {
        let m = program(
            &format!(
                " LD 0,=DB'7'\n LD 2,=DB'2'\n DIDBR 0,4,2,{mask}\n STD 0,OUT\n STD 4,OUT+8\n STD 2,OUT+16\n IPM 1\n ST 1,OUT+24\n STFPC OUT+28\n"
            ),
            "OUT DS 4D\n",
        );
        let mem = run(&m);
        let at = 4096 + m.entries["OUT"] as usize;
        assert_eq!(
            &mem[at..at + 24],
            hex(&format!("{remainder} {quotient} 4000000000000000")),
            "mask {mask}"
        );
        assert_eq!(mem[at + 24] >> 4, 0, "mask {mask}");
        assert_eq!(
            &mem[at + 28..at + 32],
            [0; 4],
            "integer rounding does not set inexact; mask {mask}"
        );
    }
}

#[test]
fn divide_to_integer_partial_final_and_scaled_overflow_results_have_distinct_cc() {
    for (dividend, divisor, rem, quo, cc) in [
        ("4B800000", "3F400000", "3F800000", "4BAAAAAA", 2),
        ("4B800000", "3F000000", "00000000", "4C000000", 0),
        ("7F000000", "00000001", "00000000", "69800000", 1),
        ("80000000", "40000000", "80000000", "80000000", 0),
        ("3F800000", "FF800000", "3F800000", "80000000", 0),
    ] {
        let m = program(
            " LE 0,A\n LE 2,B\n DIEBR 0,4,2,4\n STE 0,OUT\n STE 4,OUT+4\n IPM 1\n ST 1,OUT+8\n STFPC OUT+12\n",
            &format!("A DC X'{dividend}'\nB DC X'{divisor}'\nOUT DS 2D\n"),
        );
        let mem = run(&m);
        let at = 4096 + m.entries["OUT"] as usize;
        assert_eq!(
            &mem[at..at + 8],
            hex(&format!("{rem} {quo}")),
            "{dividend}/{divisor}"
        );
        assert_eq!(mem[at + 8] >> 4, cc, "{dividend}/{divisor}");
        assert_eq!(&mem[at + 12..at + 16], [0; 4]);
    }
}

#[test]
fn divide_to_integer_special_operands_propagate_nan_and_invalid_without_divide_by_zero() {
    for (a, b, result, flag) in [
        ("3F800000", "00000000", "7FC00000", 0x80),
        ("7F800000", "3F800000", "7FC00000", 0x80),
        ("7FC12345", "3F800000", "7FC12345", 0),
        ("7FC12345", "7F812345", "7FC12345", 0x80),
        ("7F823456", "7F812345", "7FC23456", 0x80),
    ] {
        let m = program(
            " LE 0,A\n LE 2,B\n DIEBR 0,4,2,5\n STE 0,OUT\n STE 4,OUT+4\n IPM 1\n ST 1,OUT+8\n STFPC OUT+12\n",
            &format!("A DC X'{a}'\nB DC X'{b}'\nOUT DS 2D\n"),
        );
        let mem = run(&m);
        let at = 4096 + m.entries["OUT"] as usize;
        assert_eq!(&mem[at..at + 8], hex(&format!("{result} {result}")));
        assert_eq!(mem[at + 8] >> 4, 1);
        assert_eq!(&mem[at + 12..at + 16], &[0, flag, 0, 0]);
    }
}

#[test]
fn divide_to_integer_source_rejects_overlapping_registers_and_reserved_modifiers() {
    for instruction in [
        "DIEBR 0,0,2,4",
        "DIDBR 0,2,2,4",
        "DIEBR 0,4,0,4",
        "DIDBR 0,4,2,2",
        "DIEBR 0,4,2,8",
        "DIDBR 0,4,2",
        "DIEBR 0,4,2,4,0",
    ] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.asm");
        std::fs::write(&path, format!("MAIN CSECT\n {instruction}\n BR 14\n END\n")).unwrap();
        assert!(hlasm::compile_file(&path, &[]).is_err(), "{instruction}");
    }
}

#[test]
fn divide_to_integer_execute_opcode_overlay_preserves_rrf_b_operands() {
    let m = program(
        " LD 0,=DB'7'\n LD 2,=DB'2'\n LA 1,8\n EX 1,TEMPLATE\n B AFTER\nTEMPLATE DIEBR 0,4,2,4\nAFTER STD 0,OUT\n STD 4,OUT+8\n",
        "OUT DS 2D\n",
    );
    let mem = run(&m);
    let at = 4096 + m.entries["OUT"] as usize;
    assert_eq!(&mem[at..at + 16], hex("BFF0000000000000 4010000000000000"));
    let template = m.code.iter().find(|i| i.op == "DIEBR").unwrap();
    assert_eq!(mem[4096 + template.offset as usize + 1], 0x53);
}

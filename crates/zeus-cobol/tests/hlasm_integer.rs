use zeus_cobol::hlasm;

fn compile(body: &str, data: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("integer.asm");
    let body = body
        .lines()
        .map(|line| {
            if line.starts_with('@') {
                line.trim_start_matches('@').to_owned()
            } else {
                format!("         {}", line.trim())
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&path, format!("INTEGER CSECT\n         USING INTEGER,12\n         LR 12,15\n         L 10,0(1)\n{body}\n         STMG 2,5,0(10)\n         LGHI 15,0\n         BR 14\n{data}\n         END\n")).unwrap();
    hlasm::compile_file(&path, &[])
}

fn run(body: &str, data: &str) -> [u64; 4] {
    let module = compile(body, data).unwrap();
    // Cover persisted modules and a relocated image as well as compilation.
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

#[test]
fn shifts_and_rotates_handle_width_boundaries_distinct_sources_and_base_counts() {
    let cases: &[(&str, u64, u64)] = &[
        ("SLLG 2,3,32", 4294967298, 8589934592),
        ("SRLG 2,3,32", 4294967298, 1),
        ("SLLG 2,3,63", 3, 1 << 63),
        ("SRLG 2,3,63", u64::MAX, 1),
        ("SLLG 2,3,64", 3, 3),
        ("SRLG 2,3,0", 3, 3),
        ("SRAG 2,3,1", (-5i64) as u64, (-3i64) as u64),
        ("SRAG 2,3,63", (-5i64) as u64, u64::MAX),
        ("SRAG 2,3,63", i64::MAX as u64, 0),
        ("SLAG 2,3,1", (-5i64) as u64, (-10i64) as u64),
        ("SLAG 2,3,63", u64::MAX, 1 << 63),
        ("RLLG 2,3,32", 0x1122334455667788, 0x5566778811223344),
        ("RLLG 2,3,-1", 3, (1 << 63) | 1),
        ("RLLG 2,3,64", 3, 3),
        ("SLLG 2,3,0(2)", 3, 6), // destination is also count base
        ("SLLG 2,3,-524287", 3, 6),
    ];
    for (body, source, expected) in cases {
        let result = run(
            &format!("LGHI 2,65\nLG 3,SOURCE\n{body}"),
            &format!("SOURCE DC XL8'{source:016X}'"),
        );
        assert_eq!(result[0], *expected, "{body}");
        assert_eq!(result[1], *source, "{body}: source changed");
    }
    assert_eq!(
        run(
            "LG 2,VALUE\nLG 3,SOURCE\nRLL 2,3,8",
            "VALUE DC XL8'AABBCCDD00000000'\nSOURCE DC XL8'1122334455667788'"
        )[0],
        0xaabbccdd66778855
    );
    assert_eq!(run("LGHI 3,3\nLGHI 4,-1\nSLLG 2,3,0(4)", "")[0], 1 << 63);
    assert_eq!(run("LGHI 2,3\nRLLG 2,2,1", "")[0], 6);
}

#[test]
fn double_register_shifts_preserve_both_upper_words() {
    let data = "EVEN DC XL8'AABBCCDD11223344'\nODD DC XL8'EEFF001155667788'";
    let result = run("LG 2,EVEN\nLG 3,ODD\nSLDL 2,4", data);
    assert_eq!(&result[..2], &[0xaabbccdd12233445, 0xeeff001156677880]);
    let result = run("LG 2,EVEN\nLG 3,ODD\nSRDL 2,36", data);
    assert_eq!(&result[..2], &[0xaabbccdd00000000, 0xeeff001101122334]);
    let data = "EVEN DC XL8'AABBCCDDFFFFFFFF'\nODD DC XL8'EEFF0011FFFFFFFB'";
    let result = run("LG 2,EVEN\nLG 3,ODD\nSRDA 2,1", data);
    assert_eq!(&result[..2], &[0xaabbccddffffffff, 0xeeff0011fffffffd]);
    let result = run("LG 2,EVEN\nLG 3,ODD\nSLDA 2,1", data);
    assert_eq!(&result[..2], &[0xaabbccddffffffff, 0xeeff0011fffffff6]);
    assert_eq!(
        run("LGHI 2,0\nLGHI 3,1\nSLDL 2,63\nSRDL 2,63", "")[..2],
        [0, 1]
    );
}

#[test]
fn logical_multiplication_returns_full_products_and_preserves_condition_code() {
    for op in ["MLGR 2,3", "MLG 2,ONES"] {
        let result = run(
            &format!("LGHI 2,99\nLG 3,ONES\n{op}"),
            "ONES DC XL8'FFFFFFFFFFFFFFFF'",
        );
        assert_eq!(&result[..2], &[u64::MAX - 1, 1], "{op}");
    }
    for op in ["MLR 2,3", "ML 2,ONES"] {
        let result = run(
            &format!("LG 2,EVEN\nLG 3,ODD\n{op}"),
            "EVEN DC XL8'AABBCCDD12345678'\nODD DC XL8'EEFF0011FFFFFFFF'\nONES DC X'FFFFFFFF'",
        );
        assert_eq!(
            &result[..2],
            &[0xaabbccddfffffffe, 0xeeff001100000001],
            "{op}"
        );
    }
}

#[test]
fn unsigned_division_handles_double_length_dividends_and_aliased_divisors() {
    for op in ["DLGR 2,4", "DLG 2,DIVISOR"] {
        let result = run(
            &format!("LGHI 2,1\nLGHI 3,10\nLGHI 4,3\n{op}"),
            "DIVISOR DC FD'3'",
        );
        assert_eq!(&result[..2], &[2, 6148914691236517208], "{op}");
    }
    let result = run("LGHI 2,1\nLGHI 3,-1\nDLG 2,=FD'2'", "");
    assert_eq!(&result[..2], &[1, u64::MAX]);
    let result = run("LGHI 2,1\nLGHI 3,3\nDLGR 2,3", "");
    assert_eq!(&result[..2], &[1, 6148914691236517206]);
    for op in ["DLR 2,4", "DL 2,DIVISOR"] {
        let result = run(
            &format!("LG 2,EVEN\nLG 3,ODD\nLGHI 4,3\n{op}"),
            "EVEN DC XL8'AABBCCDD00000001'\nODD DC XL8'EEFF00110000000A'\nDIVISOR DC F'3'",
        );
        assert_eq!(
            &result[..2],
            &[0xaabbccdd00000002, 0xeeff001155555558],
            "{op}"
        );
    }
}

#[test]
fn signed_single_division_uses_only_the_odd_dividend_and_sign_extends_fullwords() {
    for op in ["DSGR 2,4", "DSG 2,DIVISOR"] {
        let result = run(
            &format!("LGHI 2,99\nLGHI 3,-17\nLGHI 4,5\n{op}"),
            "DIVISOR DC FD'5'",
        );
        assert_eq!(&result[..2], &[(-2i64) as u64, (-3i64) as u64], "{op}");
    }
    for op in ["DSGFR 2,4", "DSGF 2,DIVISOR"] {
        let result = run(
            &format!("LGHI 2,99\nLGHI 3,17\nLG 4,REGVALUE\n{op}"),
            "DIVISOR DC F'-5'\nREGVALUE DC XL8'12345678FFFFFFFB'",
        );
        assert_eq!(&result[..2], &[2, (-3i64) as u64], "{op}");
    }
    let result = run("LGHI 2,5\nLGHI 3,-17\nDSGR 2,2", "");
    assert_eq!(&result[..2], &[(-2i64) as u64, (-3i64) as u64]);
    let result = run("LG 3,MIN\nDSG 2,=FD'1'", "MIN DC FD'-9223372036854775808'");
    assert_eq!(&result[..2], &[0, 1 << 63]);
}

#[test]
fn arithmetic_shift_condition_codes_and_unchanged_logical_multiply_divide_codes() {
    let cases = [
        "LGHI 3,1\nSLAG 2,3,63\nBO OK",
        "LGHI 3,-1\nSLAG 2,3,63\nBM OK",
        "LG 3,MIN\nSLAG 2,3,1\nBO OK",
        "LGHI 3,1\nSRAG 2,3,63\nBE OK",
        "LGHI 3,4\nSRAG 2,3,1\nBP OK",
        "LGHI 2,0\nLGHI 3,1\nSLDA 2,63\nBO OK",
        "LGHI 2,-1\nLGHI 3,-1\nSLDA 2,63\nBM OK",
        "LGHI 2,0\nLGHI 3,1\nSRDA 2,1\nBE OK",
        "LGHI 2,-1\nLGHI 3,-5\nSRDA 2,1\nBM OK",
    ];
    for body in cases {
        assert_eq!(
            run(
                &format!("{body}\nLGHI 2,99\nB DONE\n@OK LGHI 2,1\n@DONE NOPR 0"),
                "MIN DC FD'-9223372036854775808'"
            )[0],
            1,
            "{body}"
        );
    }
    for op in [
        "SLLG 2,3,1",
        "SRLG 2,3,1",
        "RLL 2,3,1",
        "RLLG 2,3,1",
        "SLDL 2,1",
        "SRDL 2,1",
        "MLR 2,4",
        "MLGR 2,4",
        "ML 2,WORD",
        "MLG 2,DWORD",
        "DLR 2,4",
        "DLGR 2,4",
        "DL 2,WORD",
        "DLG 2,DWORD",
        "DSGR 2,4",
        "DSGFR 2,4",
        "DSG 2,DWORD",
        "DSGF 2,WORD",
    ] {
        assert_eq!(
            run(
                &format!(
                    "LGHI 2,0\nLGHI 3,1\nLGHI 4,3\nCGHI 4,3\n{op}\nBE OK\nLGHI 2,99\nB DONE\n@OK LGHI 2,1\n@DONE NOPR 0"
                ),
                "WORD DC F'3'\nDWORD DC FD'3'"
            )[0],
            1,
            "{op}: changed CC"
        );
    }
}

#[test]
fn invalid_pairs_counts_and_divide_exceptions_are_diagnosed_and_reclaim_frames() {
    for op in ["MLR", "MLGR", "DLR", "DLGR", "DSGR", "DSGFR"] {
        assert!(
            compile(&format!("{op} 3,4"), "")
                .unwrap_err()
                .contains("even register pair")
        );
    }
    for op in [
        "ML", "MLG", "DL", "DLG", "DSG", "DSGF", "SLDL", "SRDL", "SLDA", "SRDA",
    ] {
        assert!(
            compile(&format!("{op} 3,0(4)"), "")
                .unwrap_err()
                .contains("even register pair")
        );
    }
    assert!(
        compile("SLLG 2,3,0(4,5)", "")
            .unwrap_err()
            .contains("index not permitted")
    );
    assert!(
        compile("SLDL 2,4096", "")
            .unwrap_err()
            .contains("displacement out of range")
    );
    assert!(
        compile("RLLG 2,3,-524289", "")
            .unwrap_err()
            .contains("displacement out of range")
    );
    for body in [
        "LGHI 3,1\nLGHI 4,0\nDSGR 2,4",
        "LGHI 3,1\nLGHI 4,0\nDLGR 2,4",
        "LGHI 3,1\nDLR 2,4",
        "LG 3,MIN\nLGHI 4,-1\nDSGR 2,4",
        "LG 3,MIN\nLGHI 4,-1\nDSGFR 2,4",
        "LGHI 2,3\nLGHI 3,1\nLGHI 4,3\nDLGR 2,4",
        "LGHI 2,3\nLGHI 3,1\nLGHI 4,3\nDLR 2,4",
    ] {
        let module = compile(body, "MIN DC FD'-9223372036854775808'").unwrap();
        let mut mem = vec![0; 128];
        mem.extend(module.storage_at(128).unwrap());
        let before = mem.clone();
        assert!(
            hlasm::execute(&module, &mut mem, 128, &[Some(16)])
                .unwrap_err()
                .contains("fixed-point divide exception"),
            "{body}"
        );
        assert_eq!(mem, before);
    }
}

#[test]
fn using_prefers_nonnegative_displacements_before_nearer_negative_ones() {
    let module = compile("USING INTEGER+256,11\nLG 2,INTEGER+248", "").unwrap();
    let load = module
        .code
        .iter()
        .find(|i| i.op == "LG")
        .unwrap()
        .address
        .as_ref()
        .unwrap();
    assert_eq!((load.base, load.displacement), (12, 248));
    let module = compile(
        "DROP 12\nUSING INTEGER+256,11\nUSING INTEGER+512,9\nLG 2,INTEGER+248",
        "",
    )
    .unwrap();
    let load = module
        .code
        .iter()
        .find(|i| i.op == "LG")
        .unwrap()
        .address
        .as_ref()
        .unwrap();
    assert_eq!((load.base, load.displacement), (11, -8));
}

#[test]
fn cobol_calls_shift_and_divide_with_shared_eight_byte_binary_fields() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("ASMDIV.asm");
    std::fs::write(&source, "ASMDIV CSECT\n         USING ASMDIV,15\n         L 6,0(1)\n         L 7,4(1)\n         L 8,8(1)\n         LG 3,0(6)\n         SLLG 3,3,1\n         DSG 2,=FD'5'\n         STG 3,0(7)\n         STG 2,0(8)\n         LGHI 15,0\n         BR 14\n         END\n").unwrap();
    let module = hlasm::compile_file(&source, &[]).unwrap();
    hlasm::save_module(&module, &dir.path().join("ASMDIV.zo")).unwrap();
    let source = dir.path().join("main.cbl");
    std::fs::write(
        &source,
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAINPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC S9(18) COMP VALUE 4294967298.
       01 B PIC S9(18) COMP VALUE 0.
       01 C PIC S9(18) COMP VALUE 0.
       PROCEDURE DIVISION.
           CALL 'ASMDIV' USING A B C
           IF A = 4294967298 AND B = 1717986919 AND C = 1
             MOVE 0 TO RETURN-CODE
           ELSE MOVE 99 TO RETURN-CODE END-IF
           STOP RUN.
",
    )
    .unwrap();
    let module = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
    let mut machine = zeus_cobol::vm::Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.run_main(module).unwrap(), 0);
}

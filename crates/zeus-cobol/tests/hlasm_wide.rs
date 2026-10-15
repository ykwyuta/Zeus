use zeus_cobol::hlasm;

fn compile(body: &str, data: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wide.asm");
    std::fs::write(&path, format!("WIDE CSECT\n         USING WIDE,12\n         LR 12,15\n         L 10,0(1)\n{body}\n         STG 2,0(10)\n         LGHI 15,0\n         BR 14\n{data}\n         END\n")).unwrap();
    hlasm::compile_file(&path, &[])
}

fn run(body: &str, data: &str) -> u64 {
    let module = compile(body, data).unwrap();
    let mut mem = vec![0; 128];
    mem.extend(module.storage_at(128).unwrap());
    let before = mem.len();
    assert_eq!(
        hlasm::execute(&module, &mut mem, 128, &[Some(16)]).unwrap(),
        0
    );
    assert_eq!(mem.len(), before);
    u64::from_be_bytes(mem[16..24].try_into().unwrap())
}

const DATA: &str = "BIG DC FD'4294967298'\nNEG DC FD'-3'\nMAX DC FD'9223372036854775807'\nONES DC XL8'FFFFFFFFFFFFFFFF'\nWORD DC F'-7'\nHALF DC H'-5'\nBYTE DC X'AB'\nDEST DS FD";

#[test]
fn wide_integer_load_store_arithmetic_and_logical_operations() {
    let cases: &[(&str, u64)] = &[
        ("LG 2,BIG", 4294967298),
        ("LGF 2,WORD", (-7i64) as u64),
        ("LLGF 2,WORD", u64::from((-7i32) as u32)),
        ("LGH 2,HALF", (-5i64) as u64),
        ("LLGH 2,HALF", 65531),
        ("LLGC 2,BYTE", 171),
        ("LG 3,BIG\n LGR 2,3", 4294967298),
        ("LGHI 3,-2\n LGFR 2,3", (-2i64) as u64),
        ("LGHI 3,-2\n LLGFR 2,3", 4294967294),
        ("LG 2,BIG\n AG 2,NEG", 4294967295),
        ("LG 2,BIG\n SG 2,NEG", 4294967301),
        ("LG 2,BIG\n LGHI 3,-3\n AGR 2,3", 4294967295),
        ("LG 2,BIG\n LGHI 3,-3\n SGR 2,3", 4294967301),
        ("LG 2,BIG\n AGF 2,WORD", 4294967291),
        ("LG 2,BIG\n SGF 2,WORD", 4294967305),
        ("LG 2,BIG\n LGHI 3,-7\n AGFR 2,3", 4294967291),
        ("LG 2,BIG\n LGHI 3,-7\n SGFR 2,3", 4294967305),
        ("LG 2,BIG\n AGHI 2,-2", 4294967296),
        ("LG 2,BIG\n AGFI 2,-2147483648", 2147483650),
        ("LGFI 2,-2147483648", (-2147483648i64) as u64),
        ("LG 2,NEG\n LCGR 2,2", 3),
        ("LG 2,NEG\n LPGR 2,2", 3),
        ("LGHI 2,3\n LNGR 2,2", (-3i64) as u64),
        ("LG 2,BIG\n NG 2,ONES", 4294967298),
        ("LG 2,BIG\n OG 2,ONES", u64::MAX),
        ("LG 2,BIG\n XG 2,BIG", 0),
        ("LG 2,BIG\n NGR 2,2", 4294967298),
        ("LG 2,BIG\n OGR 2,2", 4294967298),
        ("LG 2,BIG\n XGR 2,2", 0),
        (
            "LG 2,BIG\n MSG 2,NEG",
            4294967298u64.wrapping_mul((-3i64) as u64),
        ),
        (
            "LG 2,BIG\n LGHI 3,-3\n MSGR 2,3",
            4294967298u64.wrapping_mul((-3i64) as u64),
        ),
        ("LG 2,BIG\n MGHI 2,2", 8589934596),
        ("LG 2,BIG\n STG 2,DEST\n LGHI 2,0\n LG 2,DEST", 4294967298),
    ];
    for (body, expected) in cases {
        let body = body
            .lines()
            .map(|l| format!("         {}", l.trim()))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(run(&body, DATA), *expected, "{body}");
    }
}

#[test]
fn mixed_width_instructions_preserve_upper_register_halves() {
    for (body, low) in [
        ("LR 2,3", 7),
        ("L 2,WORD", (-7i32) as u32),
        ("LH 2,HALF", (-5i32) as u32),
        ("AHI 2,3", 5),
        ("AR 2,3", 9),
        ("XR 2,2", 0),
        ("LM 2,2,WORD", (-7i32) as u32),
        ("IC 2,BYTE", 171),
        ("LA 2,7", 7),
    ] {
        assert_eq!(
            run(
                &format!("         LG 2,BIG\n         LGHI 3,7\n         {body}"),
                DATA
            ),
            (1u64 << 32) | u64::from(low),
            "{body}"
        );
    }
}

#[test]
fn wide_condition_codes_include_overflow_carry_and_borrow() {
    let cases = [
        "LG 2,MAX\n AGHI 2,1\n BO OK",
        "LG 2,ONES\n ALG 2,=FD'1'\n BC 2,OK",
        "LG 2,ONES\n LGHI 3,2\n ALGR 2,3\n BC 1,OK",
        "LGHI 2,0\n SLG 2,=FD'1'\n BC 4,OK",
        "LGHI 2,1\n LGHI 3,1\n SLGR 2,3\n BC 2,OK",
        "LGHI 2,0\n SG 2,=FD'1'\n BM OK",
        "LGHI 2,-1\n CG 2,MAX\n BL OK",
        "LGHI 2,-1\n CLG 2,MAX\n BH OK",
        "LGHI 2,-7\n CGF 2,WORD\n BE OK",
        "LGHI 2,-7\n LGHI 3,-7\n CGFR 2,3\n BE OK",
        "LGHI 2,-7\n CGHI 2,-7\n BE OK",
        "LGHI 2,-7\n CGFI 2,-7\n BE OK",
        "LGHI 2,1\n LGHI 3,2\n CGR 2,3\n BL OK",
        "LGHI 2,-1\n LGHI 3,2\n CLGR 2,3\n BH OK",
        "LGHI 2,-1\n LTGR 2,2\n BM OK",
        "LG 2,MIN\n LCGR 2,2\n BO OK",
        "LG 2,MIN\n LPGR 2,2\n BO OK",
        "LGHI 2,1\n XGR 2,2\n BE OK",
        "LGHI 2,1\n OGR 2,2\n BC 4,OK",
    ];
    for body in cases {
        let body = body
            .lines()
            .map(|l| format!("         {}", l.trim()))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            run(
                &format!("{body}\n         LGHI 2,99\n         B DONE\nOK LGHI 2,1\nDONE NOPR 0"),
                &format!("{DATA}\nMIN DC FD'-9223372036854775808'")
            ),
            1,
            "{body}"
        );
    }
}

#[test]
fn doubleword_constants_lengths_lists_alignment_and_multiple_register_storage() {
    let data =
        "PAD DC X'AA'\nPAIR DC FD'4294967298,-3'\nSAVE DS 2FD\nEMPTY DS 0FD\nLENGTH EQU L'EMPTY";
    assert_eq!(
        run(
            "         LMG 2,3,PAIR\n         STMG 2,3,SAVE\n         LG 2,SAVE+8",
            data
        ),
        (-3i64) as u64
    );
    assert_eq!(run("         LGHI 2,LENGTH", data), 8);
    assert_eq!(
        run(
            "         LA 2,PAIR\n         LMG 2,3,0(2)\n         LGR 2,3",
            data
        ),
        (-3i64) as u64
    );
    assert_eq!(
        run(
            "         LG 0,PAIR\n         STMG 14,1,SAVE\n         LGHI 0,0\n         LMG 14,1,SAVE\n         LGR 2,0",
            "PAIR DC FD'4294967298'\nSAVE DS 4FD"
        ),
        4294967298
    );
    assert_eq!(run("         LGF 2,SHORT", "SHORT DC FDL4'-1'"), u64::MAX);

    assert_eq!(
        run(
            "         LG 0,PAIR\n         LG 1,PAIR+8\n         STMG 0,1,SAVE\n         LMG 2,3,SAVE\n         LGR 2,3",
            data
        ),
        (-3i64) as u64
    );
}

#[test]
fn long_displacements_are_checked_without_relaxing_legacy_rx_addresses() {
    assert_eq!(
        run("         LG 2,FAR", "GAP DS 5000X\nFAR DC FD'4294967298'"),
        4294967298
    );
    assert_eq!(
        run(
            "         LA 4,DEST\n         LG 2,-8(4)",
            "VALUE DC FD'99'\nDEST DS FD"
        ),
        99
    );
    assert!(
        compile("         LG 2,524288(4)", "")
            .unwrap_err()
            .contains("displacement out of range")
    );
    assert!(
        compile("         LG 2,-524289(4)", "")
            .unwrap_err()
            .contains("displacement out of range")
    );
    assert!(
        compile("         L 2,4096(4)", "")
            .unwrap_err()
            .contains("displacement out of range")
    );
    assert!(
        compile("         LG 2,VALUE", "VALUE DC FDS347'2'")
            .unwrap_err()
            .contains("binary scale must be in")
    );
    assert!(
        compile("         LGHI 2,32768", "")
            .unwrap_err()
            .contains("signed immediate")
    );
    assert!(
        compile("         LGFI 2,2147483648", "")
            .unwrap_err()
            .contains("signed immediate")
    );
}

#[test]
fn wide_bounds_errors_reclaim_temporary_call_storage() {
    let module = compile("         LG 2,524287(0)", "").unwrap();
    let mut mem = vec![0; 128];
    mem.extend(module.storage_at(128).unwrap());
    let before = mem.clone();
    assert!(
        hlasm::execute(&module, &mut mem, 128, &[Some(16)])
            .unwrap_err()
            .contains("outside run-unit storage")
    );
    assert_eq!(mem, before);
}

#[test]
fn machine_calls_share_doubleword_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("WIDEADD.asm");
    std::fs::write(&path, "WIDEADD CSECT\n         USING WIDEADD,15\n         L 2,0(1)\n         LG 3,0(2)\n         AGHI 3,1\n         STG 3,0(2)\n         LGHI 15,0\n         BR 14\n         END\n").unwrap();
    let module = hlasm::compile_file(&path, &[]).unwrap();
    hlasm::save_module(&module, &dir.path().join("WIDEADD.zo")).unwrap();
    let mut machine = zeus_cobol::vm::Machine::new(vec![dir.path().to_path_buf()]);
    let addr = machine.getmain(8, 0);
    machine.storage_mut()[addr..addr + 8].copy_from_slice(&4294967296u64.to_be_bytes());
    for _ in 0..3 {
        assert_eq!(
            machine
                .call_hlasm("WIDEADD", &[Some(addr)])
                .unwrap()
                .unwrap(),
            0
        );
    }
    assert_eq!(
        &machine.storage_mut()[addr..addr + 8],
        &4294967299u64.to_be_bytes()
    );
}

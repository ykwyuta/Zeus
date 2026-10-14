use zeus_cobol::hlasm;

fn compile(source: &str) -> Result<hlasm::Module, String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("layout.asm");
    std::fs::write(&path, source).unwrap();
    hlasm::compile_file(&path, &[])
}

fn run(source: &str) -> i32 {
    let module = compile(source).unwrap();
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128).unwrap());
    hlasm::execute(&module, &mut memory, 128, &[]).unwrap()
}

#[test]
fn control_sections_reopen_without_interleaving_storage_or_resetting_counters() {
    assert_eq!(
        run(
            "TEST CSECT\n USING *,15\n L 2,VALUE\n B NEXT\nOTHER CSECT\nOTHERDATA DC F'99'\nTEST CSECT\nNEXT A 2,VALUE\n LR 15,2\n BR 14\nVALUE DC F'21'\n END TEST\n"
        ),
        42
    );
}

#[test]
fn location_counters_reopen_and_default_counter_keeps_executable_flow() {
    assert_eq!(
        run(
            "TEST CSECT\n USING TEST,15\n L 2,VALUE\nDATA LOCTR\nVALUE DC F'21'\n LOCTR\n A 2,VALUE2\n LR 15,2\n BR 14\nDATA LOCTR\nVALUE2 DC F'22'\n END TEST\n"
        ),
        43
    );
}

#[test]
fn named_counters_resume_their_own_sections_and_control_sections_resume_the_last_counter() {
    assert_eq!(
        run(
            "TEST CSECT\n USING TEST,15\n L 2,V1\n A 2,V2\n LR 15,2\n BR 14\nDATA LOCTR\nV1 DC F'17'\nOTHER CSECT\n DS 3C\nTEST CSECT\nV2 DC F'19'\nOTHER CSECT\nDATA LOCTR\nV3 DC F'23'\n END TEST\n"
        ),
        36
    );
}

#[test]
fn location_counter_boundaries_do_not_add_implicit_alignment() {
    let module = compile("TEST CSECT\n LA 15,0\n BR 14\n DC C'X'\nTAIL LOCTR\nFIRST DC C'Y'\nWORD DC F'7'\n ENTRY TAIL,FIRST,WORD\n END TEST\n").unwrap();
    assert_eq!(module.entries["TAIL"], 7);
    assert_eq!(module.entries["FIRST"], 7);
    assert_eq!(module.entries["WORD"], 8);
    assert_eq!(&module.storage[7..12], &[0xE8, 0, 0, 0, 7]);
}

#[test]
fn location_counter_relocation_literals_and_cross_counter_equ_use_final_addresses() {
    assert_eq!(
        run(
            "TEST CSECT\n USING TEST,15\n L 3,=A(POINTER)\n L 4,0(3)\n L 15,0(4)\n BR 14\nDATA LOCTR\nPOINTER DC A(VALUE)\nDIST EQU VALUE-TEST\n LOCTR\n LTORG\nDATA LOCTR\nVALUE DC F'37'\n END TEST\n"
        ),
        37
    );
}

#[test]
fn dummy_location_counters_share_one_dummy_section_without_allocating_storage() {
    assert_eq!(
        run(
            "REC DSECT\nF1 DS F\nPART LOCTR\nF2 DS F\n LOCTR\nF3 DS F\nREC DSECT\nF4 DS F\nTEST CSECT\n USING TEST,15\n LA 3,DATA\n USING REC,3\n L 2,F1\n A 2,F3\n A 2,F4\n A 2,F2\n LR 15,2\n BR 14\nDATA DC F'1',F'2',F'3',F'4'\n END TEST\n"
        ),
        10
    );
}

#[test]
fn layout_names_cannot_hide_ordinary_symbols_or_counters_in_other_sections() {
    for source in [
        "TEST CSECT\nA DC F'1'\nA CSECT\n END\n",
        "TEST CSECT\nA LOCTR\nOTHER CSECT\nA CSECT\n END\n",
    ] {
        assert!(compile(source).is_err());
    }
}

#[test]
fn named_using_keeps_independent_register_domains_and_drop_register_leaves_named_domain() {
    assert_eq!(
        run(
            "REC DSECT\nFIELD DS F\nTEST CSECT\n USING TEST,15\n LA 3,DATA\n LA 4,DATA+4\n USING REC,3\nQ USING REC,4\n L 2,FIELD\n DROP 4\n A 2,Q.FIELD\n DROP Q\n LR 15,2\n BR 14\nDATA DC F'13',F'17'\n END TEST\n"
        ),
        30
    );
    assert!(
        compile("REC DSECT\nFIELD DS F\nTEST CSECT\nQ USING REC,3\n L 2,FIELD\n END\n")
            .unwrap_err()
            .contains("no USING")
    );
    assert!(
        compile("REC DSECT\nFIELD DS F\nTEST CSECT\nQ USING REC,3\n DROP Q\n L 2,Q.FIELD\n END\n")
            .unwrap_err()
            .contains("no USING")
    );
}

#[test]
fn qualified_symbol_attributes_ignore_using_qualifiers_in_both_expression_languages() {
    assert_eq!(
        run(
            "REC DSECT\nFIELD DS CL4\n MACRO\n CHECK &P\n AIF (L'&P EQ 4 AND T'&P EQ 'C' AND D'&P).OK\n MNOTE 8,'qualified parameter attribute'\n.OK ANOP\n MEND\n&LEN SETA L'Q.FIELD\n AIF (T'Q.FIELD EQ 'C' AND D'Q.FIELD).GOOD\n MNOTE 8,'qualified ordinary attribute'\n.GOOD ANOP\nTEST CSECT\n USING TEST,12\n LR 12,15\n LA 3,DATA\nQ USING REC,3\n CHECK Q.FIELD\n MVC OUT(L'Q.FIELD),Q.FIELD\n CLC OUT(&LEN),DATA\n BNE FAIL\n LA 15,0\n BR 14\nFAIL LA 15,99\n BR 14\nDATA DC C'ABCD'\nOUT DS CL4\n END TEST\n"
        ),
        0
    );
}

#[test]
fn dependent_using_translates_offsets_and_keeps_root_ownership_through_chains() {
    assert_eq!(
        run(
            "REC DSECT\nHEAD DS F\nBODY DS F\nSUB DSECT\nVALUE DS F\nTEST CSECT\n USING TEST,15\n LA 3,DATA\nROOT USING REC,3\nFIRST USING SUB,ROOT.BODY\nSECOND USING SUB,FIRST.VALUE\n DROP FIRST\n L 2,SECOND.VALUE\n LR 15,2\n BR 14\nDATA DC F'11',F'39'\n END TEST\n"
        ),
        39
    );
    assert!(compile("REC DSECT\nBODY DS F\nSUB DSECT\nVALUE DS F\nTEST CSECT\n USING REC,3\nQ USING SUB,BODY\n DROP 3\n L 2,Q.VALUE\n END\n").unwrap_err().contains("no USING"));
}

#[test]
fn using_end_limits_and_push_pop_restore_all_named_and_dependent_domains() {
    assert_eq!(
        run(
            "REC DSECT\nHEAD DS F\nBODY DS F\nSUB DSECT\nVALUE DS F\nTEST CSECT\n USING TEST,15\n LA 3,DATA\nQ USING (REC,REC+8),3\nD USING SUB,Q.BODY\n PUSH USING\n DROP Q\n POP USING\n L 15,D.VALUE\n BR 14\nDATA DC F'2',F'41'\n END TEST\n"
        ),
        41
    );
    assert!(
        compile(
            "REC DSECT\nHEAD DS F\nBODY DS F\nTEST CSECT\n USING (REC,REC+4),3\n L 2,BODY\n END\n"
        )
        .unwrap_err()
        .contains("no USING")
    );
}

#[test]
fn readonly_sections_survive_serialization_linking_and_reject_storage_writes() {
    let module = compile("TEST RSECT\n USING TEST,15\n MVI VALUE,1\n LA 15,0\n BR 14\nVALUE DC X'00'\n ENTRY VALUE\n END TEST\n").unwrap();
    assert_eq!(module.zeus_hlasm, 6);
    let module = hlasm::load_module(&hlasm::module_bytes(&module).unwrap()).unwrap();
    let prefix = compile("PREFIX CSECT\n LA 15,0\n BR 14\n END PREFIX\n").unwrap();
    let linked = hlasm::link_modules(&[prefix, module.clone()]).unwrap();
    assert_eq!(linked.zeus_hlasm, 6);
    assert!(linked.read_only[0].start > 0);
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128).unwrap());
    let before = memory.clone();
    assert!(
        hlasm::execute(&module, &mut memory, 128, &[])
            .unwrap_err()
            .contains("storage protection exception")
    );
    assert_eq!(memory, before);
    for ranges in [
        std::iter::once(0..u32::MAX).collect(),
        std::iter::once(std::ops::Range { start: 4, end: 2 }).collect(),
        vec![0..4, 2..6],
    ] {
        let mut invalid = module.clone();
        invalid.read_only = ranges;
        assert!(
            hlasm::load_module(&hlasm::module_bytes(&invalid).unwrap())
                .unwrap_err()
                .contains("RSECT storage range")
        );
    }
    let mut invalid = module;
    invalid.zeus_hlasm = 5;
    assert!(
        hlasm::load_module(&hlasm::module_bytes(&invalid).unwrap())
            .unwrap_err()
            .contains("version 6")
    );
}

#[test]
fn run_unit_protection_also_covers_writes_from_other_assembler_modules() {
    let readonly =
        compile("CONST RSECT\n LA 15,0\n BR 14\nVALUE DC X'AB'\n ENTRY VALUE\n END CONST\n")
            .unwrap();
    let writer =
        compile("WRITE CSECT\n L 2,0(1)\n L 2,0(2)\n MVI 0(2),1\n LA 15,0\n BR 14\n END WRITE\n")
            .unwrap();
    let mut memory = vec![0; 128];
    memory.extend(readonly.storage_at(128).unwrap());
    let writer_base = memory.len().next_multiple_of(8);
    memory.resize(writer_base, 0);
    memory.extend(writer.storage_at(writer_base).unwrap());
    let pointer = 128 + readonly.entries["VALUE"];
    memory[16..20].copy_from_slice(&pointer.to_be_bytes());
    let mut storage = hlasm::DynamicStorage::default();
    hlasm::execute_with_storage(&readonly, &mut memory, 128, &[], &mut storage).unwrap();
    let before = memory.clone();
    assert!(
        hlasm::execute_with_storage(&writer, &mut memory, writer_base, &[Some(16)], &mut storage)
            .unwrap_err()
            .contains("storage protection exception")
    );
    assert_eq!(memory, before);
}

#[test]
fn residency_and_section_type_attributes_survive_serialization_and_linking() {
    let module = compile(
        "TEST CSECT\nTEST AMODE ANY31\nTEST RMODE 24\n LA 15,T'TEST-C'J'\n BR 14\n END TEST\n",
    )
    .unwrap();
    assert_eq!(module.rmode, 24);
    assert_eq!(module.zeus_hlasm, 6);
    let module = hlasm::load_module(&hlasm::module_bytes(&module).unwrap()).unwrap();
    assert!(
        module
            .storage_at(1 << 24)
            .unwrap_err()
            .contains("below 16 MiB")
    );
    let prefix = compile("PREFIX CSECT\n LA 15,0\n BR 14\n END PREFIX\n").unwrap();
    let linked = hlasm::link_modules(&[prefix, module]).unwrap();
    assert_eq!(linked.rmode, 24);
    assert_eq!(linked.zeus_hlasm, 6);
    assert!(linked.storage_at(1 << 24).is_err());
}

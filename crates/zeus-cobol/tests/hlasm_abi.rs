use zeus_cobol::{hlasm, vm::Machine};

fn compile(source: &str) -> hlasm::Module {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("abi.asm");
    std::fs::write(&path, source).unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    hlasm::load_module(&hlasm::module_bytes(&m).unwrap()).unwrap()
}
#[test]
fn amode64_uses_eight_byte_parameters_and_a_full_f4sa_save_area() {
    let module = compile(
        "WIDE CSECT\n AMODE 64\n USING WIDE,15\n STMG 14,12,8(13)\n LG 2,0(1)\n LG 3,8(1)\n LG 4,0(2)\n AGHI 4,7\n STG 4,0(3)\n LG 14,8(13)\n LGHI 15,9\n BR 14\n END\n",
    );
    assert_eq!(module.amode, 64);
    assert_eq!(module.zeus_hlasm, 10);
    let mut memory = vec![0; 128];
    memory[..8].copy_from_slice(&10_000_000_000u64.to_be_bytes());
    memory.extend(module.storage_at(128).unwrap());
    let mark = memory.len();
    for n in 1..=3 {
        assert_eq!(
            hlasm::execute(&module, &mut memory, 128, &[Some(0), Some(8)]).unwrap(),
            9
        );
        assert_eq!(&memory[8..16], &(10_000_000_000u64 + 7).to_be_bytes());
        assert_eq!(memory.len(), mark, "call {n}");
    }
}
#[test]
fn amode64_does_not_alias_high_addresses_to_low_storage() {
    for bits in [
        0x0000000080000000u64,
        0x0000000100000000,
        0x8000000000000000,
    ] {
        let module = compile(&format!(
            "WIDE CSECT\n AMODE 64\n USING WIDE,15\n LG 2,=X'{bits:016X}'\n LG 3,0(2)\n BR 14\n END\n"
        ));
        let mut memory = vec![0; 128];
        memory.extend(module.storage_at(128).unwrap());
        let mark = memory.len();
        let error = hlasm::execute(&module, &mut memory, 128, &[]).unwrap_err();
        assert!(
            error.contains("outside run-unit storage"),
            "{bits:016X}: {error}"
        );
        assert_eq!(memory.len(), mark);
    }
}
#[test]
fn amode64_address_generation_and_relative_calls_write_complete_registers() {
    let module = compile(
        "WIDE CSECT\n AMODE 64\n LGR 12,15\n USING WIDE,12\n STG 14,RET\n LG 2,=X'1122334400000000'\n LARL 2,VALUE\n LG 3,0(2)\n LA 2,VALUE\n LG 4,0(2)\n CGR 3,4\n BNE BAD\n BRASL 14,SUB\n B DONE\nBAD LGHI 15,99\nDONE LG 14,RET\n BR 14\nSUB LGHI 15,27\n BR 14\nRET DS AD\nVALUE DC FD'10000000000'\n END\n",
    );
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128).unwrap());
    assert_eq!(hlasm::execute(&module, &mut memory, 128, &[]).unwrap(), 27);
}
#[test]
fn cobol_calls_amode64_with_original_shared_addresses() {
    let dir = tempfile::tempdir().unwrap();
    let module = compile(
        "WIDE CSECT\n AMODE 64\n USING WIDE,15\n LG 2,0(1)\n LG 3,0(2)\n AGHI 3,7\n STG 3,0(2)\n XR 15,15\n BR 14\n END\n",
    );
    hlasm::save_module(&module, &dir.path().join("WIDE.zo")).unwrap();
    let path = dir.path().join("main.cbl");
    std::fs::write(&path,"       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAINPGM.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 A PIC S9(18) COMP VALUE 10000000000.\n       PROCEDURE DIVISION.\n           CALL 'WIDE' USING A\n           IF A = 10000000007 MOVE 0 TO RETURN-CODE\n           ELSE MOVE 99 TO RETURN-CODE END-IF\n           STOP RUN.\n").unwrap();
    let program = zeus_cobol::compile_file(&path, &Default::default()).unwrap();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.run_main(program).unwrap(), 0);
}
#[test]
fn exec_parm_keeps_the_zos_four_byte_list_even_for_amode64() {
    let module = compile(
        "MAIN CSECT\n AMODE 64\n USING MAIN,15\n L 2,0(1)\n N 2,=X'7FFFFFFF'\n LH 3,0(2)\n LR 15,3\n BR 14\n END\n",
    );
    let mut machine = Machine::new(vec![]);
    machine.set_step(zeus_cobol::vm::StepInfo {
        parm: "ABCD".into(),
        ..Default::default()
    });
    assert_eq!(machine.run_hlasm_main(module).unwrap(), 4);
}
#[test]
fn entry_modes_survive_mixed_control_sections_linking_and_serialization() {
    let mixed = compile("ONE CSECT\n AMODE 31\n BR 14\nTWO CSECT\n AMODE 64\n BR 14\n END\n");
    assert_eq!(mixed.entry_mode(mixed.entries["ONE"]), 31);
    assert_eq!(mixed.entry_mode(mixed.entries["TWO"]), 64);
    let other = compile("THREE CSECT\n AMODE 64\n BR 14\n END\n");
    let linked = hlasm::link_modules(&[mixed, other]).unwrap();
    let linked = hlasm::load_module(&hlasm::module_bytes(&linked).unwrap()).unwrap();
    assert_eq!(linked.entry_mode(linked.entries["ONE"]), 31);
    assert_eq!(linked.entry_mode(linked.entries["TWO"]), 64);
    assert_eq!(linked.entry_mode(linked.entries["THREE"]), 64);
}

#[test]
fn bassm_bsm_switch_both_directions_and_capture_aliased_target_before_link() {
    let module = compile(
        "MAIN CSECT\n AMODE 31\n LR 12,15\n USING MAIN,12\n ST 14,RET\n L 2,=A(WIDE+1)\n BASSM 2,2\n C 15,=F'27'\n BNE BAD\n LA 15,0\n B DONE\nBAD LA 15,99\nDONE L 14,RET\n BR 14\nRET DS F\nWIDE CSECT\n AMODE 64\n LGHI 15,27\n BSM 0,2\n END\n",
    );
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128).unwrap());
    assert_eq!(hlasm::execute(&module, &mut memory, 128, &[]).unwrap(), 0);
    let module = compile(
        "MAIN CSECT\n AMODE 64\n LGR 12,15\n USING MAIN,12\n STG 14,RET\n LLGF 2,=A(NARROW+2147483648)\n BASSM 14,2\n CGHI 15,19\n BNE BAD\n LGHI 15,0\n B DONE\nBAD LGHI 15,99\nDONE LG 14,RET\n BR 14\nRET DS AD\nNARROW CSECT\n AMODE 31\n LA 15,19\n BSM 0,14\n END\n",
    );
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128).unwrap());
    assert_eq!(hlasm::execute(&module, &mut memory, 128, &[]).unwrap(), 0);
}

#[test]
fn sam_instructions_change_addressing_without_changing_register_bits() {
    let module = compile(
        "MAIN CSECT\n USING MAIN,15\n LG 2,=X'1234567800000080'\n SAM64\n SAM31\n LG 3,=X'1234567800000080'\n CGR 2,3\n BNE BAD\n SAM24\n SAM31\n XR 15,15\n BR 14\nBAD LA 15,99\n BR 14\n END\n",
    );
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128).unwrap());
    assert_eq!(hlasm::execute(&module, &mut memory, 128, &[]).unwrap(), 0);
}

#[test]
fn loaded_amode64_entry_returns_through_mode_tagged_link() {
    let dir = tempfile::tempdir().unwrap();
    let module = compile("WIDE CSECT\n AMODE 64\n LGHI 15,27\n BSM 0,14\n END\n");
    hlasm::save_module(&module, &dir.path().join("WIDE.zo")).unwrap();
    let caller = compile(
        "MAIN CSECT\n LR 12,15\n USING MAIN,12\n ST 14,RET\n LOAD EP=WIDE\n LR 8,0\n BASSM 14,8\n C 15,=F'27'\n BNE BAD\n DELETE EP=WIDE\n B DONE\nBAD LA 15,99\nDONE L 14,RET\n BR 14\nRET DS F\n END\n",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.run_hlasm_main(caller).unwrap(), 0);
}

fn install(dir: &std::path::Path, name: &str, source: &str) {
    hlasm::save_module(&compile(source), &dir.join(format!("{name}.zo"))).unwrap();
}
#[test]
fn linkx_preserves_explicit_parameter_width_across_callee_modes() {
    let dir = tempfile::tempdir().unwrap();
    install(
        dir.path(),
        "WIDE",
        "WIDE CSECT\n AMODE 64\n LG 2,0(1)\n LG 3,0(2)\n AGHI 3,7\n STG 3,0(2)\n LGHI 15,17\n BR 14\n END\n",
    );
    install(
        dir.path(),
        "NARROW",
        "NARROW CSECT\n L 2,0(1)\n L 3,0(2)\n AHI 3,11\n ST 3,0(2)\n LA 15,19\n BR 14\n END\n",
    );
    install(
        dir.path(),
        "ROOT31",
        "ROOT31 CSECT\n LR 12,15\n USING ROOT31,12\n ST 14,RET\n L 10,0(1)\n LINKX EP=WIDE,PARAM=((10)),PLIST8=YES,AMODE64OK=YES\n L 14,RET\n BR 14\nRET DS F\n END\n",
    );
    install(
        dir.path(),
        "ROOT64",
        "ROOT64 CSECT\n AMODE 64\n LGR 12,15\n USING ROOT64,12\n STG 14,RET\n LG 10,0(1)\n LINKX EP=NARROW,PARAM=((10)),PLIST4=YES,VL=1\n LG 14,RET\n BR 14\nRET DS AD\n END\n",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    let arg = machine.getmain(8, 0);
    machine.storage_mut()[arg..arg + 8].copy_from_slice(&10_000_000_000u64.to_be_bytes());
    assert_eq!(
        machine.call_hlasm("ROOT31", &[Some(arg)]).unwrap().unwrap(),
        17
    );
    assert_eq!(
        &machine.storage()[arg..arg + 8],
        &10_000_000_007u64.to_be_bytes()
    );
    machine.storage_mut()[arg..arg + 4].copy_from_slice(&5u32.to_be_bytes());
    assert_eq!(
        machine.call_hlasm("ROOT64", &[Some(arg)]).unwrap().unwrap(),
        19
    );
    assert_eq!(&machine.storage()[arg..arg + 4], &16u32.to_be_bytes());
}
#[test]
fn inherited_wide_parameter_lists_call_cobol_without_terminators() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("cb.cbl");
    std::fs::write(&path,"       IDENTIFICATION DIVISION.\n       PROGRAM-ID. CBADD.\n       DATA DIVISION.\n       LINKAGE SECTION.\n       01 A PIC S9(18) COMP.\n       PROCEDURE DIVISION USING A.\n           ADD 7 TO A\n           MOVE 9 TO RETURN-CODE\n           GOBACK.\n").unwrap();
    let cb = zeus_cobol::compile_file(&path, &Default::default()).unwrap();
    zeus_cobol::save_module(&cb, &dir.path().join("CBADD.zo")).unwrap();
    install(
        dir.path(),
        "ROOT",
        "ROOT CSECT\n AMODE 64\n LGR 12,15\n USING ROOT,12\n STG 14,RET\n LINKX EP=CBADD\n LG 14,RET\n BR 14\nRET DS AD\n END\n",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    let arg = machine.getmain(8, 0);
    machine.storage_mut()[arg..arg + 8].copy_from_slice(&10_000_000_000u64.to_be_bytes());
    assert_eq!(
        machine.call_hlasm("ROOT", &[Some(arg)]).unwrap().unwrap(),
        9
    );
    assert_eq!(
        &machine.storage()[arg..arg + 8],
        &10_000_000_007u64.to_be_bytes()
    );
}
#[test]
fn linkx_rejects_missing_amode64_permission_and_invalid_list_combinations() {
    let dir = tempfile::tempdir().unwrap();
    install(dir.path(), "WIDE", "WIDE CSECT\n AMODE 64\n BR 14\n END\n");
    install(
        dir.path(),
        "ROOT",
        "ROOT CSECT\n USING ROOT,15\n LINKX EP=WIDE,PARAM=(),PLIST8=YES\n BR 14\n END\n",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    for _ in 0..2 {
        assert!(
            machine
                .call_hlasm("ROOT", &[])
                .unwrap()
                .unwrap_err()
                .contains("AMODE64OK=YES")
        );
    }
    let mark = machine.storage().len();
    assert!(machine.call_hlasm("ROOT", &[]).unwrap().is_err());
    assert_eq!(machine.storage().len(), mark);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.asm");
    for form in [
        "LINKX EP=WIDE,PARAM=(),PLIST8=YES,VL=1",
        "LINKX EP=WIDE,PLIST8=YES,PLIST4=NO",
        "LINK EP=WIDE,PLIST8=YES",
    ] {
        std::fs::write(&path, format!("MAIN CSECT\n {form}\n BR 14\n END\n")).unwrap();
        assert!(hlasm::compile_file(&path, &[]).is_err(), "{form}");
    }
}

#[test]
fn amode64_supervisor_services_reject_high_addresses_before_truncation() {
    for operation in [
        "FREEMAIN RU,LV=8,A=(2)",
        "POST (2)",
        "WAIT ECB=(2)",
        "WTO TEXT=(2)",
    ] {
        let module = compile(&format!(
            "WIDE CSECT\n AMODE 64\n USING WIDE,15\n LG 2,=X'0000000100000080'\n {operation}\n BR 14\n END\n"
        ));
        let mut memory = vec![0; 128];
        memory.extend(module.storage_at(128).unwrap());
        let mark = memory.len();
        let error = hlasm::execute(&module, &mut memory, 128, &[]).unwrap_err();
        assert!(
            error.contains("service parameter address must be below 2 GB"),
            "{operation}: {error}"
        );
        assert_eq!(memory.len(), mark);
    }
}

#[test]
fn getmain_in_amode64_returns_a_complete_pointer_and_can_release_it() {
    let module = compile(
        "WIDE CSECT\n AMODE 64\n USING WIDE,15\n LG 1,=X'FFFFFFFF00000000'\n GETMAIN RU,LV=8\n LGR 2,1\n LGHI 3,123\n STG 3,0(2)\n LG 4,0(2)\n FREEMAIN RU,LV=8,A=(2)\n LGR 15,4\n BR 14\n END\n",
    );
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128).unwrap());
    let mark = memory.len();
    assert_eq!(hlasm::execute(&module, &mut memory, 128, &[]).unwrap(), 123);
    assert_eq!(memory.len(), mark);
}

#[test]
fn amode64_mvcle_updates_full_length_after_partial_completion() {
    let module = compile(
        "WIDE CSECT\n AMODE 64\n USING WIDE,15\n LARL 2,BUFFER\n LG 3,=X'0000000100000002'\n LARL 4,SOURCE\n LGHI 5,2\n MVCLE 2,4,90\n LARL 7,RESULT\n STG 3,0(7)\n STG 5,8(7)\n IPM 6\n ST 6,16(7)\n XR 15,15\n BR 14\nSOURCE DC X'1122'\nBUFFER DS 4096C\nRESULT DS 3D\n ENTRY BUFFER,RESULT\n END\n",
    );
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128).unwrap());
    assert_eq!(hlasm::execute(&module, &mut memory, 128, &[]).unwrap(), 0);
    let at = 128 + module.entries["RESULT"] as usize;
    assert_eq!(&memory[at..at + 8], &0x00000000fffff002u64.to_be_bytes());
    assert_eq!(&memory[at + 8..at + 16], &[0; 8]);
    assert_eq!(&memory[at + 16..at + 20], &0x30000000u32.to_be_bytes());
    let at = 128 + module.entries["BUFFER"] as usize;
    assert_eq!(&memory[at..at + 2], &[0x11, 0x22]);
    assert!(memory[at + 2..at + 4096].iter().all(|b| *b == 90));
}

#[test]
fn amode64_implicit_string_addresses_do_not_alias_the_low_word() {
    let module = compile(
        "WIDE CSECT\n AMODE 64\n USING WIDE,15\n LGHI 0,0\n LG 2,=X'0000000100000080'\n LARL 3,SOURCE\n MVST 2,3\n BR 14\nSOURCE DC X'1100'\n END\n",
    );
    let mut memory = vec![0; 128];
    memory.extend(module.storage_at(128).unwrap());
    assert!(
        hlasm::execute(&module, &mut memory, 128, &[])
            .unwrap_err()
            .contains("outside run-unit storage")
    );
}

#[test]
fn class_entries_inherit_the_owning_control_sections_address_mode() {
    let module = compile(
        "MAIN CSECT\n AMODE 64\n BR 14\nCODE CATTR ALIGN(2)\nINNER LGHI 15,17\n BR 14\n ENTRY INNER\nOTHER CSECT\n AMODE 24\n BR 14\nMORE CATTR ALIGN(2)\nNARROW LA 15,19\n BR 14\n ENTRY NARROW\n END MAIN\n",
    );
    assert_eq!(module.entry_mode(module.entries["INNER"]), 64);
    assert_eq!(module.entry_mode(module.entries["NARROW"]), 24);
}

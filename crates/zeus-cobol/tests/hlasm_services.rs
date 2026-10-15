use std::path::Path;
use zeus_cobol::{hlasm, vm::Machine};

fn assemble(dir: &Path, name: &str, body: &str) -> hlasm::Module {
    let path = dir.join(format!("{name}.asm"));
    std::fs::write(
        &path,
        format!("{name} CSECT\n USING {name},12\n LR 12,15\n{body}\n END\n"),
    )
    .unwrap();
    let m = hlasm::compile_file(&path, &[]).unwrap();
    hlasm::save_module(&m, &dir.join(format!("{name}.zo"))).unwrap();
    m
}

fn cobol(dir: &Path, name: &str, body: &str) {
    let path = dir.join(format!("{name}.cbl"));
    std::fs::write(&path, format!("       IDENTIFICATION DIVISION.\n       PROGRAM-ID. {name}.\n       DATA DIVISION.\n       LINKAGE SECTION.\n       01 ARG PIC S9(9) COMP.\n       PROCEDURE DIVISION USING ARG.\n           {body}\n")).unwrap();
    let m = zeus_cobol::compile_file(&path, &Default::default()).unwrap();
    zeus_cobol::save_module(&m, &dir.join(format!("{name}.zo"))).unwrap();
}

#[test]
fn assembler_links_cobol_and_assembler_with_original_shared_addresses() {
    let dir = tempfile::tempdir().unwrap();
    cobol(
        dir.path(),
        "CBADD",
        "ADD 7 TO ARG\n           MOVE 9 TO RETURN-CODE\n           GOBACK.",
    );
    assemble(
        dir.path(),
        "ASADD",
        " L 2,0(1)\n L 3,0(2)\n AHI 3,11\n ST 3,0(2)\n LA 15,13\n BR 14",
    );
    assemble(
        dir.path(),
        "DRIVER",
        " ST 14,RET\n LINK EP=CBADD\n ST 15,CODE1\n LINK EP=ASADD\n C 15,=F'13'\n BNE BAD\n L 15,CODE1\n C 15,=F'9'\n BNE BAD\n LA 15,0\n B DONE\nBAD LA 15,99\nDONE L 14,RET\n BR 14\nRET DS F\nCODE1 DS F",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    let arg = machine.getmain(4, 0);
    machine.storage_mut()[arg..arg + 4].copy_from_slice(&5u32.to_be_bytes());
    for n in 1..=3 {
        assert_eq!(
            machine.call_hlasm("DRIVER", &[Some(arg)]).unwrap().unwrap(),
            0
        );
        assert_eq!(
            &machine.storage()[arg..arg + 4],
            &(5u32 + 18 * n).to_be_bytes()
        );
    }
}

#[test]
fn named_and_register_parameters_and_eploc_preserve_caller_registers() {
    let dir = tempfile::tempdir().unwrap();
    cobol(
        dir.path(),
        "CBADD",
        "ADD 7 TO ARG\n           MOVE 9 TO RETURN-CODE\n           GOBACK.",
    );
    assemble(
        dir.path(),
        "DRIVER",
        " ST 14,RET\n L 10,0(1)\n LINK EP=CBADD,PARAM=((10)),VL=1\n LINK EPLOC=NAME,PARAM=(VALUE)\n L 3,VALUE\n ST 3,0(10)\n L 14,RET\n BR 14\nRET DS F\nNAME DC CL8'CBADD'\nVALUE DC F'23'",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    let arg = machine.getmain(4, 0);
    assert_eq!(
        machine.call_hlasm("DRIVER", &[Some(arg)]).unwrap().unwrap(),
        9
    );
    assert_eq!(&machine.storage()[arg..arg + 4], &30u32.to_be_bytes());
}

#[test]
fn parameter_list_variable_bit_and_tail_transfer_are_observable() {
    let dir = tempfile::tempdir().unwrap();
    assemble(
        dir.path(),
        "CHECK",
        " L 2,0(1)\n L 15,0(2)\n LTR 2,2\n BP BAD\n BR 14\nBAD LA 15,99\n BR 14",
    );
    assemble(
        dir.path(),
        "MARKED",
        " XCTL EP=CHECK,PARAM=(VALUE),VL=1\n LA 15,88\n BR 14\nVALUE DC F'7'",
    );
    assemble(
        dir.path(),
        "FIXED",
        " XCTL EP=CHECK,PARAM=(VALUE)\n LA 15,88\n BR 14\nVALUE DC F'7'",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.call_hlasm("MARKED", &[]).unwrap().unwrap(), 7);
    assert_eq!(machine.call_hlasm("FIXED", &[]).unwrap().unwrap(), 99);
}

#[test]
fn failed_service_calls_restore_machine_and_reclaim_invocation_frames() {
    let dir = tempfile::tempdir().unwrap();
    assemble(dir.path(), "BAD", " LINK EP=ABSENT\n BR 14");
    assemble(dir.path(), "GOOD", " LA 15,6\n BR 14");
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    assert!(
        machine
            .call_hlasm("BAD", &[])
            .unwrap()
            .unwrap_err()
            .contains("module entry ABSENT not found")
    );
    let retained = machine.storage().len();
    for _ in 0..3 {
        assert!(machine.call_hlasm("BAD", &[]).unwrap().is_err());
        assert_eq!(machine.storage().len(), retained);
    }
    assert_eq!(machine.call_hlasm("GOOD", &[]).unwrap().unwrap(), 6);
}

#[test]
fn explicit_service_forms_reject_unsupported_keywords_and_conflicting_names() {
    let dir = tempfile::tempdir().unwrap();
    for body in [
        " LINK EP=A,EPLOC=B",
        " LINK EP=A,EP=B",
        " LINK EP=A,VL=1",
        " LINK EP=A,PARAM=((1))",
        " LINK EP=A,DCB=B",
        " LINK EP=TOOLONGNAME",
        " LINK PARAM=(X)",
    ] {
        let path = dir.path().join("bad.asm");
        std::fs::write(&path, format!("TEST CSECT\n{body}\n END\n")).unwrap();
        assert!(hlasm::compile_file(&path, &[]).is_err(), "{body}");
    }
}

#[test]
fn load_counts_aliases_register_entry_calls_and_delete_reload_are_consistent() {
    let dir = tempfile::tempdir().unwrap();
    assemble(
        dir.path(),
        "COUNTER",
        " ENTRY ALT\nALT L 2,TOTAL\n AHI 2,1\n ST 2,TOTAL\n LR 15,2\n BR 14\nTOTAL DC F'0'",
    );
    let driver = assemble(
        dir.path(),
        "DRIVER",
        " ST 14,RET\n LR 9,12\n LOAD EP=COUNTER\n ST 1,SIZE\n LR 8,0\n LOAD EP=ALT\n LR 15,8\n BASR 14,8\n LR 12,9\n C 15,=F'1'\n BNE BAD\n DELETE EP=ALT\n LR 15,8\n BASR 14,8\n LR 12,9\n C 15,=F'2'\n BNE BAD\n DELETE EP=COUNTER\n LTR 15,15\n BNE BAD\n DELETE EP=COUNTER\n C 15,=F'4'\n BNE BAD\n LOAD EP=COUNTER\n LR 8,0\n LR 15,8\n BASR 14,8\n LR 12,9\n C 15,=F'1'\n BNE BAD\n DELETE EP=COUNTER\n B DONE\nBAD LA 15,99\nDONE L 14,RET\n BR 14\nRET DS F\nSIZE DS F",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.run_hlasm_main(driver).unwrap(), 0);
    let retained = machine.storage().len();
    for _ in 0..5 {
        assert_eq!(machine.call_hlasm("DRIVER", &[]).unwrap().unwrap(), 0);
        assert_eq!(machine.storage().len(), retained);
    }
}

#[test]
fn deleting_an_active_loaded_image_defers_reclamation_until_return() {
    let dir = tempfile::tempdir().unwrap();
    assemble(dir.path(), "SELF", " DELETE EP=SELF\n LA 15,23\n BR 14");
    assemble(
        dir.path(),
        "DRIVER",
        " ST 14,RET\n LR 9,12\n LOAD EP=SELF\n LR 8,0\n LR 15,8\n BASR 14,8\n LR 12,9\n C 15,=F'23'\n BNE BAD\n DELETE EP=SELF\n B DONE\nBAD LA 15,99\nDONE L 14,RET\n BR 14\nRET DS F",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.call_hlasm("DRIVER", &[]).unwrap().unwrap(), 4);
    let retained = machine.storage().len();
    for _ in 0..5 {
        assert_eq!(machine.call_hlasm("DRIVER", &[]).unwrap().unwrap(), 4);
        assert_eq!(machine.storage().len(), retained);
    }
}

#[test]
fn link_responsibility_ends_on_return_and_a_deleted_entry_cannot_be_called() {
    let dir = tempfile::tempdir().unwrap();
    assemble(
        dir.path(),
        "COUNTER",
        " L 2,TOTAL\n AHI 2,1\n ST 2,TOTAL\n LR 15,2\n BR 14\nTOTAL DC F'0'",
    );
    assemble(
        dir.path(),
        "DRIVER",
        " ST 14,RET\n LINK EP=COUNTER\n C 15,=F'1'\n BNE BAD\n LINK EP=COUNTER\n C 15,=F'1'\n BNE BAD\n LA 15,0\n B DONE\nBAD LA 15,99\nDONE L 14,RET\n BR 14\nRET DS F",
    );
    assemble(
        dir.path(),
        "STALE",
        " LOAD EP=COUNTER\n LR 8,0\n DELETE EP=COUNTER\n LR 15,8\n BASR 14,8\n BR 14",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.call_hlasm("DRIVER", &[]).unwrap().unwrap(), 0);
    assert!(
        machine
            .call_hlasm("STALE", &[])
            .unwrap()
            .unwrap_err()
            .contains("loaded entry")
    );
}

#[test]
fn loaded_cobol_entries_accept_fixed_parameter_lists_and_release_dispatch_slots() {
    let dir = tempfile::tempdir().unwrap();
    cobol(
        dir.path(),
        "CBADD",
        "ADD 7 TO ARG\n           MOVE 9 TO RETURN-CODE\n           GOBACK.",
    );
    assemble(
        dir.path(),
        "DRIVER",
        " ST 14,RET\n ST 1,ARGS\n LOAD EP=CBADD\n LR 8,0\n L 1,ARGS\n L 2,0(1)\n ST 2,FIXED\n NI FIXED,X'7F'\n LA 1,FIXED\n LR 15,8\n BASR 14,15\n ST 15,CODE\n DELETE EP=CBADD\n L 15,CODE\n L 14,RET\n BR 14\nRET DS F\nARGS DS F\nFIXED DS F\nCODE DS F",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    let arg = machine.getmain(4, 0);
    assert_eq!(
        machine.call_hlasm("DRIVER", &[Some(arg)]).unwrap().unwrap(),
        9
    );
    assert_eq!(&machine.storage()[arg..arg + 4], &7u32.to_be_bytes());
    let retained = machine.storage().len();
    for n in 2..=5 {
        assert_eq!(
            machine.call_hlasm("DRIVER", &[Some(arg)]).unwrap().unwrap(),
            9
        );
        assert_eq!(&machine.storage()[arg..arg + 4], &(7u32 * n).to_be_bytes());
        assert_eq!(machine.storage().len(), retained);
    }
}

#[test]
fn fixed_inherited_parameter_lists_survive_nested_link_and_tail_transfer() {
    let dir = tempfile::tempdir().unwrap();
    assemble(
        dir.path(),
        "CHECK",
        " L 2,0(1)\n LTR 2,2\n BM BAD\n L 15,0(2)\n BR 14\nBAD LA 15,99\n BR 14",
    );
    assemble(dir.path(), "MIDDLE", " XCTL EP=CHECK");
    assemble(
        dir.path(),
        "DRIVER",
        " ST 14,RET\n LINK EP=MIDDLE,PARAM=(VALUE)\n L 14,RET\n BR 14\nRET DS F\nVALUE DC F'23'",
    );
    let mut machine = Machine::new(vec![dir.path().into()]);
    assert_eq!(machine.call_hlasm("DRIVER", &[]).unwrap().unwrap(), 23);
}

#[test]
fn loaded_psect_and_class_offsets_reach_external_storage_in_the_same_run_unit() {
    let dir = tempfile::tempdir().unwrap();
    let target = assemble(
        dir.path(),
        "OTHER",
        " BR 14\nPRIVATE CATTR NOTEXECUTABLE,ALIGN(4),RMODE(31)\nAREA DC F'0'\nINTERIOR DC F'27'\n ENTRY INTERIOR",
    );
    hlasm::save_module(&target, &dir.path().join("INTERIOR.zo")).unwrap();
    assemble(
        dir.path(),
        "ROOT",
        " L 10,0(1)\n L 2,RREF\n A 2,QOFF\n L 3,0(2)\n ST 3,0(10)\n L 15,JSIZE\n BR 14\n EXTRN INTERIOR\nTABLES CATTR NOTEXECUTABLE,RMODE(31)\nRREF DC R(ROOT)\nQOFF DC Q(INTERIOR)\nJSIZE DC J(PRIVATE)\nROOT XATTR PSECT(INTERIOR)",
    );
    let mut machine = Machine::new(vec![dir.path().into()]);
    let output = machine.getmain(4, 0);
    for _ in 0..2 {
        assert_eq!(
            machine
                .call_hlasm("ROOT", &[Some(output)])
                .unwrap()
                .unwrap(),
            8
        );
        assert_eq!(&machine.storage()[output..output + 4], &27u32.to_be_bytes());
    }
}

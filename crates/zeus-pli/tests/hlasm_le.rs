//! Direct LE calls retain the shared run unit and PL/I activation lifetimes.
use zeus_cobol::{hlasm, vm::Machine as RunUnit};
use zeus_pli::vm::{Machine, RunEnd, RunEnv};
fn handler(m: &mut RunUnit, args: &[Option<usize>]) -> Result<i32, String> {
    let token = args[1].ok_or("missing token")?;
    let result = args[2].ok_or("missing result")?;
    let value = i32::from_be_bytes(m.storage()[token..token + 4].try_into().unwrap());
    m.storage_mut()[result..result + 4]
        .copy_from_slice(&(if value == 731 { 10i32 } else { 20 }).to_be_bytes());
    Ok(0)
}
#[test]
fn pli_calls_le_directly_and_unregisters_handlers_when_the_registering_procedure_returns() {
    let dir = tempfile::tempdir().unwrap();
    let asm = dir.path().join("getter.asm");
    std::fs::write(&asm,"GETPTR CSECT\n LR 12,14\n LR 3,1\n LOAD EP=PHDLR\n L 2,0(3)\n ST 0,0(2)\n XR 15,15\n BR 12\n END\n").unwrap();
    let getter = hlasm::compile_file(&asm, &[]).unwrap();
    hlasm::save_module(&getter, &dir.path().join("GETPTR.zo")).unwrap();
    let path = dir.path().join("driver.pli");
    std::fs::write(&path,"DRIVER: PROC OPTIONS(MAIN);\n DCL (GETPTR,CEEHDLR,CEEHDLU,CEESGL) ENTRY EXTERNAL;\n DCL ROUTINE FIXED BIN(31);\n DCL TOKEN FIXED BIN(31) INIT(731);\n DCL CT CHAR(12) INIT('0003006358E9E9E900000000'X);\n DCL FC CHAR(12);\n DCL Q FIXED BIN(31) INIT(0);\n CALL GETPTR(ROUTINE);\n CALL REGISTER;\n CALL CEESGL(CT,Q,FC);\n IF FC='000000000000000000000000'X THEN DO;\n CALL PLIRETC(91); STOP;\n END;\n CALL CEEHDLR(ROUTINE,TOKEN,FC);\n CALL CEESGL(CT,Q,FC);\n IF FC^='000000000000000000000000'X THEN DO;\n CALL PLIRETC(92); STOP;\n END;\n CALL CEEHDLU(ROUTINE,FC);\n CALL CEESGL(CT,Q,FC);\n IF FC='000000000000000000000000'X THEN DO;\n CALL PLIRETC(93); STOP;\n END;\n REGISTER: PROC;\n CALL CEEHDLR(ROUTINE,TOKEN,FC);\n END REGISTER;\n END DRIVER;\n").unwrap();
    let (rc, compiled) = zeus_pli::compile_file(&path, &Default::default());
    let module = compiled
        .module
        .unwrap_or_else(|| panic!("{rc}: {:?}", compiled.messages));
    let mut sys = RunUnit::new(vec![dir.path().to_path_buf()]);
    sys.add_native_machine("PHDLR", handler);
    let env = RunEnv {
        dds: Default::default(),
        parm: String::new(),
        stdout: Box::new(Vec::<u8>::new()),
        msgout: Box::new(Vec::<u8>::new()),
        stdin: None,
        stdin_reader: None,
        linker: None,
        exec_host: None,
    };
    let mut machine = Machine::with_storage(module, env, sys);
    assert_eq!(machine.run(), RunEnd::Rc(0));
}

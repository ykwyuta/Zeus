use std::path::Path;
use zeus_cobol::{hlasm, vm::Machine};

fn assemble(dir: &Path, name: &str, source: &str) -> hlasm::Module {
    let path = dir.join(format!("{name}.asm"));
    std::fs::write(&path, source).unwrap();
    let m = hlasm::compile_file(&path, &[dir.to_path_buf()]).unwrap();
    hlasm::save_module(&m, &dir.join(format!("{name}.zo"))).unwrap();
    m
}

const ADD: &str = "ASMADD   CSECT
         USING *,15
         L     2,0(1)
         L     3,4(1)
         L     4,0(2)
         AHI   4,7
         ST    4,0(3)
         LA    15,9
         BR    14
         END
";

#[test]
fn cobol_call_updates_shared_storage_and_return_code() {
    let dir = tempfile::tempdir().unwrap();
    assemble(dir.path(), "ASMADD", ADD);
    let src = dir.path().join("main.cbl");
    std::fs::write(
        &src,
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAINPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC S9(9) COMP VALUE 5.
       01 B PIC S9(9) COMP VALUE 0.
       PROCEDURE DIVISION.
           CALL 'ASMADD' USING A B
           IF B = 12 AND RETURN-CODE = 9
             MOVE 0 TO RETURN-CODE
           ELSE MOVE 99 TO RETURN-CODE END-IF
           STOP RUN.
",
    )
    .unwrap();
    let module = zeus_cobol::compile_file(&src, &Default::default()).unwrap();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.add_native("ASMADD", |_, _| {
        Err("must use assembled module, not fallback".into())
    });
    assert_eq!(machine.run_main(module).unwrap(), 0);
}

#[test]
fn assembler_call_uses_original_addresses_and_reclaims_call_frames() {
    let dir = tempfile::tempdir().unwrap();
    assemble(dir.path(), "ASMADD", ADD);
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    let a = machine.getmain(8, 0);
    machine.storage_mut()[a..a + 4].copy_from_slice(&5i32.to_be_bytes());
    assert_eq!(
        machine
            .call_hlasm("ASMADD", &[Some(a), Some(a + 4)])
            .unwrap()
            .unwrap(),
        9
    );
    assert_eq!(&machine.storage()[a + 4..a + 8], &12i32.to_be_bytes());
    let retained = machine.storage().len();
    for _ in 0..100 {
        assert_eq!(
            machine
                .call_hlasm("ASMADD", &[Some(a), Some(a)])
                .unwrap()
                .unwrap(),
            9
        );
        assert_eq!(machine.storage().len(), retained);
    }
    assert_eq!(
        i32::from_be_bytes(machine.storage()[a..a + 4].try_into().unwrap()),
        705
    );
}

#[test]
fn bounds_errors_are_reported_and_call_frames_reclaimed() {
    let dir = tempfile::tempdir().unwrap();
    let m = assemble(
        dir.path(),
        "BAD",
        "BAD CSECT
         L 2,0(1)
         L 3,0(2)
         L 4,0(3)
         BR 14
         END
",
    );
    let mut mem = 0x7fff_0000u32.to_be_bytes().to_vec();
    // Keep the load origin valid so the test reaches the bad indirect pointer.
    mem.resize(8, 0);
    let base = mem.len();
    mem.extend(&m.storage);
    let before = mem.len();
    assert!(
        hlasm::execute(&m, &mut mem, base, &[Some(0)])
            .unwrap_err()
            .contains("outside run-unit storage")
    );
    assert_eq!(mem.len(), before);
    let path = dir.path().join("unsupported.asm");
    std::fs::write(&path, "BAD CSECT\n         NOTREAL 1,2\n         END\n").unwrap();
    assert!(
        hlasm::compile_file(&path, &[])
            .unwrap_err()
            .contains("unsupported instruction")
    );
}

#[test]
#[ignore = "requires pinned CardDemo corpus: tools/fetch-carddemo.sh"]
fn carddemo_sources_compile_and_execute_without_native_replacements() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.cache/carddemo/app");
    let dirs = vec![corpus.join("maclib")];
    let mut mem = vec![0u8; 80];
    for (input, expected) in [
        ("120230405            1", "2023-04-05"),
        ("22023-04-05          2", "20230405"),
    ] {
        let mut bytes = zeus_cobol::ebcdic::encode(input);
        bytes.resize(80, 0x40);
        mem[..80].copy_from_slice(&bytes);
        let module = hlasm::compile_file(&corpus.join("asm/COBDATFT.asm"), &dirs).unwrap();
        let base = mem.len();
        mem.extend(module.storage_at(base).unwrap());
        assert_eq!(
            hlasm::execute(&module, &mut mem, base, &[Some(0)]).unwrap(),
            0
        );
        assert_eq!(
            zeus_cobol::ebcdic::decode(&mem[22..22 + expected.len()]),
            expected
        );
        mem.truncate(80);
    }
    let module = hlasm::compile_file(&corpus.join("asm/MVSWAIT.asm"), &dirs).unwrap();
    let mut mem = vec![0; 8];
    let base = mem.len();
    mem.extend(module.storage_at(base).unwrap());
    assert_eq!(
        hlasm::execute(&module, &mut mem, base, &[Some(0)]).unwrap(),
        0
    );
}

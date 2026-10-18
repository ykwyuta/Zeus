use zeus_cobol::{front::FrontOptions, vm::threaded::ThreadedRunUnit};

fn compile(dir: &std::path::Path, name: &str, text: &str) {
    let path = dir.join(format!("{name}.cbl"));
    std::fs::write(
        &path,
        text.lines()
            .map(|line| format!("       {line}\n"))
            .collect::<String>(),
    )
    .unwrap();
    let module = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap();
    zeus_cobol::save_module(&module, &dir.join(format!("{name}.zo"))).unwrap();
}

#[test]
fn host_threads_share_ws_and_receive_separate_locals_and_reference_results() {
    let dir = tempfile::tempdir().unwrap();
    compile(
        dir.path(),
        "COUNTER",
        "CBL THREAD RENT\nID DIVISION.\nPROGRAM-ID. COUNTER IS RECURSIVE.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 COUNTED PIC 9(4) VALUE 0.\nLOCAL-STORAGE SECTION.\n01 LOCAL-NUMBER PIC 99 VALUE 9.\nLINKAGE SECTION.\n01 RESULT-NUMBER PIC 9(4).\nPROCEDURE DIVISION USING RESULT-NUMBER.\n>>IF IGY-THREAD\nADD 1 TO COUNTED\nMOVE COUNTED TO RESULT-NUMBER\nADD 1 TO LOCAL-NUMBER\nDISPLAY LOCAL-NUMBER\n>>END-IF\nGOBACK.",
    );
    let unit = ThreadedRunUnit::spawn(vec![dir.path().to_path_buf()], |_| {}).unwrap();
    let handles: Vec<_> = (0..8)
        .map(|_| {
            let unit = unit.clone();
            std::thread::spawn(move || {
                (0..20)
                    .map(|_| {
                        let result = unit.call("COUNTER", vec![Some(vec![0xF0; 4])]).unwrap();
                        assert_eq!(result.output, b"10\n");
                        assert!(!result.end.stop);
                        zeus_cobol::codepage::get(37)
                            .unwrap()
                            .decode(result.arguments[0].as_ref().unwrap())
                            .parse::<u32>()
                            .unwrap()
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let mut counts = handles
        .into_iter()
        .flat_map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();
    counts.sort_unstable();
    assert_eq!(counts, (1..=160).collect::<Vec<_>>());
    // An independent run unit has independent WS and EXTERNAL state.
    let other = ThreadedRunUnit::spawn(vec![dir.path().to_path_buf()], |_| {}).unwrap();
    let first = other.call("COUNTER", vec![Some(vec![0xF0; 4])]).unwrap();
    assert_eq!(first.arguments[0], Some(vec![0xF0, 0xF0, 0xF0, 0xF1]));
}

#[test]
fn recursive_special_registers_belong_to_each_invocation() {
    let dir = tempfile::tempdir().unwrap();
    compile(
        dir.path(),
        "LEVELS",
        "CBL THREAD\nID DIVISION.\nPROGRAM-ID. LEVELS IS RECURSIVE.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 DEPTH-NUMBER PIC 9 VALUE 0.\nPROCEDURE DIVISION.\nADD 1 TO DEPTH-NUMBER\nMOVE DEPTH-NUMBER TO TALLY\nIF DEPTH-NUMBER < 3 CALL 'LEVELS' END-CALL END-IF\nDISPLAY TALLY\nGOBACK.",
    );
    let unit = ThreadedRunUnit::spawn(vec![dir.path().to_path_buf()], |_| {}).unwrap();
    assert_eq!(
        unit.call("LEVELS", vec![]).unwrap().output,
        b"00003\n00002\n00001\n"
    );
}

#[test]
fn local_storage_indexes_belong_to_each_active_invocation() {
    let dir = tempfile::tempdir().unwrap();
    compile(
        dir.path(),
        "LOCALIX",
        "CBL THREAD\nID DIVISION.\nPROGRAM-ID. LOCALIX IS RECURSIVE.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 DEPTH-NUMBER PIC 9 VALUE 0.\nLOCAL-STORAGE SECTION.\n01 TAB.\n05 ELEM OCCURS 2 INDEXED BY IX PIC X VALUE 'A'.\nPROCEDURE DIVISION.\nADD 1 TO DEPTH-NUMBER\nSET IX TO 1\nMOVE 'B' TO ELEM(2)\nIF DEPTH-NUMBER < 2 CALL 'LOCALIX' END-CALL END-IF\nDISPLAY ELEM(IX)\nSET IX TO 2\nGOBACK.",
    );
    let unit = ThreadedRunUnit::spawn(vec![dir.path().to_path_buf()], |_| {}).unwrap();
    assert_eq!(unit.call("LOCALIX", vec![]).unwrap().output, b"A\nA\n");
    assert_eq!(unit.call("LOCALIX", vec![]).unwrap().output, b"A\n");
}

#[test]
fn finishing_a_shared_run_unit_closes_and_flushes_open_files() {
    let dir = tempfile::tempdir().unwrap();
    compile(
        dir.path(),
        "FILES",
        "CBL THREAD\nID DIVISION.\nPROGRAM-ID. FILES IS RECURSIVE.\nENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\nSELECT F ASSIGN TO OUTFILE.\nDATA DIVISION.\nFILE SECTION.\nFD F RECORDING MODE F LABEL RECORDS OMITTED.\n01 R PIC XX.\nPROCEDURE DIVISION.\nOPEN OUTPUT F\nWRITE R FROM 'OK'\nGOBACK.",
    );
    let output = dir.path().join("out.bin");
    let target = output.clone();
    let unit = ThreadedRunUnit::spawn(vec![dir.path().to_path_buf()], move |m| {
        m.set_dd("OUTFILE", target)
    })
    .unwrap();
    unit.call("FILES", vec![]).unwrap();
    unit.finish().unwrap();
    unit.finish().unwrap();
    assert_eq!(std::fs::read(&output).unwrap(), [0xD6, 0xD2]);
    assert!(unit.call("FILES", vec![]).is_err());
    unit.finish().unwrap();
}

#[test]
fn thread_requirements_and_run_unit_termination_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bad.cbl");
    std::fs::write(&path, "       CBL THREAD\n       ID DIVISION.\n       PROGRAM-ID. BAD.\n       PROCEDURE DIVISION.\n           GOBACK.\n").unwrap();
    assert!(
        zeus_cobol::compile_file(&path, &FrontOptions::default())
            .unwrap_err()
            .message
            .contains("THREAD requires")
    );
    compile(
        dir.path(),
        "PLAIN",
        "ID DIVISION.\nPROGRAM-ID. PLAIN.\nPROCEDURE DIVISION.\nGOBACK.",
    );
    let unit = ThreadedRunUnit::spawn(vec![dir.path().to_path_buf()], |_| {}).unwrap();
    assert!(
        unit.call("PLAIN", vec![])
            .unwrap_err()
            .contains("requires THREAD")
    );
    compile(
        dir.path(),
        "FINISHER",
        "CBL THREAD\nID DIVISION.\nPROGRAM-ID. FINISHER IS RECURSIVE.\nPROCEDURE DIVISION.\nMOVE 7 TO RETURN-CODE\nSTOP RUN.",
    );
    let unit = ThreadedRunUnit::spawn(vec![dir.path().to_path_buf()], |_| {}).unwrap();
    let end = unit.call("FINISHER", vec![]).unwrap().end;
    assert_eq!(end.rc, 7);
    assert!(end.stop);
    assert!(unit.call("FINISHER", vec![]).is_err());
}

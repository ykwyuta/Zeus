use std::path::Path;
use zeus_cobol::{hlasm, vm::Machine};

fn assemble(dir: &Path, name: &str, body: &str) {
    let path = dir.join(format!("{name}.asm"));
    std::fs::write(
        &path,
        format!("{name} CSECT\n USING {name},12\n LR 12,15\n{body}\n END\n"),
    )
    .unwrap();
    let module = hlasm::compile_file(&path, &[]).unwrap();
    hlasm::save_module(&module, &dir.join(format!("{name}.zo"))).unwrap();
}

#[test]
fn post_wait_preserve_cc_registers_and_completion_code() {
    let dir = tempfile::tempdir().unwrap();
    assemble(
        dir.path(),
        "EVENT",
        " L 10,0(1)\n LA 15,19\n XR 3,3\n POST (10),X'3FFFFFFF'\n WAIT ECB=(10)\n BE OK\n LA 15,99\nOK BR 14",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    let ecb = machine.getmain(4, 0);
    assert_eq!(
        machine.call_hlasm("EVENT", &[Some(ecb)]).unwrap().unwrap(),
        19
    );
    assert_eq!(
        &machine.storage()[ecb..ecb + 4],
        &0x7fff_ffffu32.to_be_bytes()
    );
}

#[test]
fn cooperative_wait_dispatches_only_enough_queued_producer_completions() {
    let dir = tempfile::tempdir().unwrap();
    assemble(
        dir.path(),
        "ONE",
        " L 10,0(1)\n WAIT 1,ECBLIST=8(10)\n LA 15,0\n BR 14",
    );
    assemble(
        dir.path(),
        "TWO",
        " L 10,0(1)\n LA 2,2\n WAIT (2),ECBLIST=8(10)\n LA 15,0\n BR 14",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    let base = machine.getmain(24, 0);
    machine.storage_mut()[base + 8..base + 12].copy_from_slice(&(base as u32).to_be_bytes());
    machine.storage_mut()[base + 12..base + 16]
        .copy_from_slice(&((base + 4) as u32 | 0x8000_0000).to_be_bytes());
    machine.schedule_hlasm_post(base + 4, 9).unwrap();
    machine.schedule_hlasm_post(base + 20, 7).unwrap();
    machine.schedule_hlasm_post(base, 11).unwrap();
    assert_eq!(
        machine.call_hlasm("ONE", &[Some(base)]).unwrap().unwrap(),
        0
    );
    assert_eq!(
        &machine.storage()[base..base + 8],
        &[0, 0, 0, 0, 0x40, 0, 0, 9]
    );
    assert_eq!(&machine.storage()[base + 20..base + 24], &[0; 4]);
    assert_eq!(
        machine.call_hlasm("TWO", &[Some(base)]).unwrap().unwrap(),
        0
    );
    assert_eq!(
        &machine.storage()[base..base + 8],
        &[0x40, 0, 0, 11, 0x40, 0, 0, 9]
    );
    assert_eq!(&machine.storage()[base + 20..base + 24], &[0x40, 0, 0, 7]);
}

#[test]
fn waiting_without_a_producer_reports_deadlock_and_cleans_wait_bits_and_frames() {
    let dir = tempfile::tempdir().unwrap();
    assemble(dir.path(), "WAITING", " L 10,0(1)\n WAIT ECB=(10)\n BR 14");
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    let ecb = machine.getmain(4, 0);
    assert!(
        machine
            .call_hlasm("WAITING", &[Some(ecb)])
            .unwrap()
            .unwrap_err()
            .contains("WAIT deadlock")
    );
    let size = machine.storage().len();
    for _ in 0..3 {
        assert!(
            machine
                .call_hlasm("WAITING", &[Some(ecb)])
                .unwrap()
                .is_err()
        );
        assert_eq!(machine.storage().len(), size);
        assert_eq!(&machine.storage()[ecb..ecb + 4], &[0; 4]);
    }
    machine.schedule_hlasm_post(ecb, 0).unwrap();
    assert!(machine.call_hlasm("WAITING", &[Some(ecb)]).unwrap().is_ok());
}

#[derive(Clone)]
struct Output(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);
impl std::io::Write for Output {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn wto_writes_literal_and_length_prefixed_text_and_preserves_routing_metadata() {
    let dir = tempfile::tempdir().unwrap();
    assemble(
        dir.path(),
        "MESSAGE",
        " WTO 'Worker started',ROUTCDE=(1,10),DESC=(2)\n WTO TEXT=MSG\n LA 15,0\n BR 14\nMSG DC AL2(L'TEXT)\nTEXT DC C'It''s done'",
    );
    let output = Output(Default::default());
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.set_stdout(Box::new(output.clone()));
    assert_eq!(machine.call_hlasm("MESSAGE", &[]).unwrap().unwrap(), 0);
    assert_eq!(&*output.0.lock().unwrap(), b"Worker started\nIt's done\n");
    assert_eq!(machine.operator_messages()[0].routes, [1, 10]);
    assert_eq!(machine.operator_messages()[0].descriptors, [2]);
    assert_eq!(machine.operator_messages()[1].text, "It's done");
}

#[test]
fn invalid_ecbs_lists_completion_codes_and_text_lengths_are_errors() {
    let dir = tempfile::tempdir().unwrap();
    for (name, body) in [
        ("ALIGN", " L 10,0(1)\n POST 1(10)\n BR 14"),
        (
            "RANGE",
            " L 10,0(1)\n LA 3,4\n SLL 3,28\n POST (10),(3)\n BR 14",
        ),
        (
            "DUP",
            " WAIT 2,ECBLIST=LIST\n BR 14\nLIST DC A(ECB),A(ECB+X'80000000')\nECB DS F",
        ),
        ("COUNT", " LA 2,2\n WAIT (2),ECB=ECB\n BR 14\nECB DS F"),
        ("EMPTY", " WTO ''\n BR 14"),
        ("LONG", " WTO TEXT=MSG\n BR 14\nMSG DC H'127',CL127'X'"),
    ] {
        assemble(dir.path(), name, body);
        let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
        let ecb = machine.getmain(4, 0);
        let error = machine.call_hlasm(name, &[Some(ecb)]).unwrap().unwrap_err();
        assert!(
            error.contains(match name {
                "ALIGN" => "aligned",
                "RANGE" => "30 bits",
                "DUP" => "list or count",
                "COUNT" => "event count",
                _ => "1..126",
            }),
            "{name}: {error}"
        );
    }
    let mut machine = Machine::new(vec![]);
    assert!(machine.schedule_hlasm_post(1, 0).is_err());
    assert!(machine.schedule_hlasm_post(0, 0x4000_0000).is_err());
}

use std::path::Path;
use zeus_cobol::{hlasm, vm::Machine};
use zeus_ds::format::RecordFormat;

fn assemble(dir: &Path, name: &str, source: &str) -> hlasm::Module {
    let path = dir.join(format!("{name}.asm"));
    std::fs::write(
        &path,
        format!("{name} CSECT\n USING {name},12\n LR 12,15\n{source}\n END\n"),
    )
    .unwrap();
    let module = hlasm::compile_file(&path, &[]).unwrap();
    let module = hlasm::load_module(&hlasm::module_bytes(&module).unwrap()).unwrap();
    hlasm::save_module(&module, &dir.join(format!("{name}.zo"))).unwrap();
    module
}

const MOVE: &str = " XR 6,6\n OPEN (IN,INPUT,OUT,OUTPUT)\n LTR 15,15\n BNE DONE\nLOOP GET IN,AREA\n PUT OUT,AREA\n AHI 6,1\n B LOOP\nEOF CLOSE (IN,,OUT)\n LR 15,6\nDONE BR 14\nIN DCB DDNAME=INFILE,MACRF=GM,RECFM=FB,LRECL=4,EODAD=EOF\nOUT DCB DDNAME=OUTFILE,MACRF=PM,RECFM=FB,LRECL=4\nAREA DS CL4";

#[test]
fn qsam_move_mode_copies_fixed_records_and_branches_to_eodad() {
    let dir = tempfile::tempdir().unwrap();
    let module = assemble(dir.path(), "COPYREC", MOVE);
    assert_eq!(module.zeus_hlasm, 8);
    assert_eq!(module.dcbs.len(), 2);
    let input = dir.path().join("input");
    let output = dir.path().join("output");
    let records = zeus_cobol::ebcdic::encode("ABCD1234WXYZ");
    std::fs::write(&input, &records).unwrap();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.set_dd("INFILE", input);
    machine.set_dd("OUTFILE", output.clone());
    assert_eq!(machine.run_hlasm_main(module).unwrap(), 3);
    assert_eq!(std::fs::read(output).unwrap(), records);
}

#[test]
fn qsam_variable_locate_mode_copies_rdw_records_and_flushes_the_last_put_on_close() {
    let dir = tempfile::tempdir().unwrap();
    let module = assemble(
        dir.path(),
        "COPYVAR",
        " XR 6,6\n OPEN (IN,INPUT,OUT,OUTPUT)\nLOOP GET IN\n LR 8,1\n PUT OUT\n LH 2,0(8)\n BCTR 2,0\n EX 2,COPY\n AHI 6,1\n B LOOP\nCOPY MVC 0(1,1),0(8)\nEOF CLOSE (IN,,OUT)\n FREEPOOL IN\n FREEPOOL OUT\n LR 15,6\n BR 14\nIN DCB DDNAME=INFILE,MACRF=GL,RECFM=VB,LRECL=20,EODAD=EOF\nOUT DCB DDNAME=OUTFILE,MACRF=PL,RECFM=VB,LRECL=20",
    );
    let input = dir.path().join("input");
    let output = dir.path().join("output");
    let mut records = vec![];
    for value in ["A", "LONG RECORD", "XYZ"] {
        zeus_ds::format::encode_into(
            RecordFormat::Variable,
            &zeus_cobol::ebcdic::encode(value),
            0x40,
            &mut records,
        );
    }
    std::fs::write(&input, &records).unwrap();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.set_dd("INFILE", input);
    machine.set_dd("OUTFILE", output.clone());
    assert_eq!(machine.run_hlasm_main(module).unwrap(), 3);
    assert_eq!(std::fs::read(output).unwrap(), records);
}

#[test]
fn dd_format_overrides_dcb_lengths_and_text_datasets_transcode_records() {
    let dir = tempfile::tempdir().unwrap();
    let module = assemble(dir.path(), "COPYREC", MOVE);
    let input = dir.path().join("input");
    let output = dir.path().join("output");
    let records = zeus_cobol::ebcdic::encode("ABCD1234");
    std::fs::write(&input, &records).unwrap();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    for (name, path) in [("INFILE", input.clone()), ("OUTFILE", output.clone())] {
        machine.allocate(
            name,
            zeus_ds::dd::Allocation {
                path,
                format: Some(RecordFormat::Fixed(2)),
            },
        );
    }
    assert_eq!(machine.run_hlasm_main(module.clone()).unwrap(), 4);
    assert_eq!(std::fs::read(&output).unwrap(), records);
    std::fs::write(&input, "AB\nCD\n").unwrap();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    for (name, path) in [("INFILE", input), ("OUTFILE", output.clone())] {
        machine.allocate(
            name,
            zeus_ds::dd::Allocation {
                path,
                format: Some(RecordFormat::Text),
            },
        );
    }
    assert_eq!(machine.run_hlasm_main(module).unwrap(), 2);
    assert_eq!(std::fs::read_to_string(output).unwrap(), "AB  \nCD  \n");
}

#[test]
fn open_dcb_and_locate_buffer_survive_return_and_teardown_flushes_the_pending_record() {
    let dir = tempfile::tempdir().unwrap();
    let module = assemble(
        dir.path(),
        "WRITER",
        " CLI STATE,0\n BNE AGAIN\n OPEN (OUT,OUTPUT)\n MVI STATE,1\nAGAIN PUT OUT\n MVC 0(4,1),=C'DATA'\n LA 15,0\n BR 14\nOUT DCB DDNAME=OUTFILE,MACRF=PL,RECFM=F,LRECL=4\nSTATE DC X'00'",
    );
    let output = dir.path().join("output");
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.set_dd("OUTFILE", output.clone());
    assert_eq!(machine.call_hlasm("WRITER", &[]).unwrap().unwrap(), 0);
    let size = machine.storage().len();
    for _ in 0..3 {
        assert_eq!(machine.call_hlasm("WRITER", &[]).unwrap().unwrap(), 0);
        assert_eq!(machine.storage().len(), size);
    }
    assert_eq!(std::fs::read(&output).unwrap().len(), 12);
    machine.finish_assembler_io().unwrap();
    assert_eq!(
        std::fs::read(&output).unwrap(),
        zeus_cobol::ebcdic::encode("DATADATADATADATA")
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.set_dd("OUTFILE", output.clone());
    assert_eq!(machine.run_hlasm_main(module).unwrap(), 0);
    assert_eq!(
        std::fs::read(&output).unwrap(),
        zeus_cobol::ebcdic::encode("DATA")
    );
}

#[test]
fn an_open_copied_dcb_pins_its_heap_storage_until_close() {
    let dir = tempfile::tempdir().unwrap();
    let module = assemble(
        dir.path(),
        "PINNED",
        " GETMAIN RU,LV=96,SP=3\n LR 10,1\n MVC 0(96,10),TEMPLATE\n OPEN ((10),OUTPUT)\n FREEMAIN RC,LV=96,A=(10),SP=3\n C 15,=F'4'\n BNE BAD\n STORAGE RELEASE,SP=3,COND=YES\n C 15,=F'4'\n BNE BAD\n CLOSE ((10))\n FREEMAIN RU,LV=96,A=(10),SP=3\n LA 15,0\n BR 14\nBAD LA 15,99\n BR 14\nTEMPLATE DCB DDNAME=OUTFILE,MACRF=PM,RECFM=F,LRECL=4",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.set_dd("OUTFILE", dir.path().join("output"));
    assert_eq!(machine.run_hlasm_main(module).unwrap(), 0);
}

#[test]
fn missing_dd_returns_open_failure_and_invalid_records_and_modes_report_errors() {
    let dir = tempfile::tempdir().unwrap();
    let module = assemble(
        dir.path(),
        "MISSING",
        " OPEN (OUT,OUTPUT)\n BR 14\nOUT DCB DDNAME=OUTFILE,MACRF=PM,RECFM=F,LRECL=4",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    assert_eq!(machine.run_hlasm_main(module).unwrap(), 8);
    let module = assemble(dir.path(), "SHORT", MOVE);
    let input = dir.path().join("input");
    std::fs::write(&input, [0; 3]).unwrap();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.set_dd("INFILE", input);
    machine.set_dd("OUTFILE", dir.path().join("output"));
    assert!(
        machine
            .run_hlasm_main(module)
            .unwrap_err()
            .contains("incomplete fixed-length")
    );
    let module = assemble(
        dir.path(),
        "BADRDW",
        " OPEN (OUT,OUTPUT)\n PUT OUT,REC\n BR 14\nOUT DCB DDNAME=OUTFILE,MACRF=PM,RECFM=V,LRECL=20\nREC DC H'2',H'0'",
    );
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.set_dd("OUTFILE", dir.path().join("output"));
    assert!(machine.run_hlasm_main(module).unwrap_err().contains("RDW"));
}

#[test]
fn dcb_serialization_and_linking_preserve_offsets_and_exit_targets() {
    let dir = tempfile::tempdir().unwrap();
    let dcb = assemble(dir.path(), "IO", MOVE);
    let first = assemble(dir.path(), "FIRST", " LA 15,0\n BR 14");
    let linked = hlasm::link_modules(&[first, dcb.clone()]).unwrap();
    assert_eq!(linked.zeus_hlasm, 8);
    assert!(linked.dcbs[0].offset > dcb.dcbs[0].offset);
    let bytes = linked.storage_at(128).unwrap();
    let def = &linked.dcbs[0];
    assert_eq!(
        &bytes[def.offset as usize + 40..def.offset as usize + 48],
        zeus_cobol::ebcdic::encode("INFILE  ")
    );
    assert_eq!(
        &bytes[def.offset as usize + 33..def.offset as usize + 36],
        &(128 + def.end.unwrap()).to_be_bytes()[1..]
    );
    let mut old = dcb;
    old.zeus_hlasm = 7;
    assert!(hlasm::load_module(&hlasm::module_bytes(&old).unwrap()).is_err());
}

use std::{cell::RefCell, io::Write, path::Path, rc::Rc};
use zeus_cobol::{front::FrontOptions, vm::Machine, zir::Module};
#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn compile(dir: &Path, name: &str, text: &str) -> Module {
    let path = dir.join(format!("{name}.cbl"));
    std::fs::write(
        &path,
        text.lines()
            .map(|line| {
                assert!(line.len() <= 65);
                format!("       {line}\n")
            })
            .collect::<String>(),
    )
    .unwrap();
    zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap()
}
#[test]
fn lp64_discards_cics_option_and_rejects_unsupported_control_flow() {
    let dir = tempfile::tempdir().unwrap();
    let module = compile(
        dir.path(),
        "T",
        "CBL LP(64) CICS DYNAM\nID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\n>>IF IGY-CICS\nMOVE 9 TO RETURN-CODE\n>>ELSE\nMOVE 0 TO RETURN-CODE\n>>END-IF\nGOBACK.",
    );
    assert_eq!(Machine::new(vec![]).run_main(module).unwrap(), 0);
    for body in [
        "EXEC CICS RETURN END-EXEC.",
        "GO TO.",
        "ALTER P TO PROCEED TO Q.\nGOBACK.\nP.\nGO TO Q.\nQ.\nGOBACK.",
    ] {
        let source =
            format!("CBL LP(64)\nID DIVISION.\nPROGRAM-ID. T.\nPROCEDURE DIVISION.\n{body}");
        let path = dir.path().join("bad.cbl");
        std::fs::write(
            &path,
            source
                .lines()
                .map(|l| format!("       {l}\n"))
                .collect::<String>(),
        )
        .unwrap();
        let error = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap_err();
        assert!(error.message.contains("LP(64)"), "{error}");
    }
}

#[test]
fn lp64_pointer_index_layout_and_by_value_dll_linkage() {
    let dir = tempfile::tempdir().unwrap();
    let sub = compile(
        dir.path(),
        "SUB",
        "CBL LP(64) EXPORTALL\nID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nLINKAGE SECTION.\n01 ARG-POINTER USAGE POINTER.\n01 REFERRED PIC XX.\nPROCEDURE DIVISION USING BY VALUE ARG-POINTER.\nSET ADDRESS OF REFERRED TO ARG-POINTER\nMOVE 'HI' TO REFERRED\nGOBACK.",
    );
    zeus_cobol::save_module(&sub, &dir.path().join("SUB.zo")).unwrap();
    let main = compile(
        dir.path(),
        "MAIN",
        "CBL LP(64) THREAD\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 P USAGE POINTER.\n01 P32 USAGE POINTER-32.\n01 FP USAGE FUNCTION-POINTER.\n01 PP USAGE PROCEDURE-POINTER.\n01 IX USAGE INDEX.\n01 V PIC XX VALUE 'NO'.\n01 TAB.\n05 ELEM OCCURS 2 INDEXED BY TI PIC X.\nPROCEDURE DIVISION.\n>>IF IGY-LP = 64 AND NOT IGY-THREAD\nIF LENGTH OF P = 8 AND LENGTH OF P32 = 4\nAND LENGTH OF FP = 8 AND LENGTH OF PP = 8\nAND LENGTH OF IX = 8 DISPLAY 'WIDTHS' END-IF\n>>END-IF\nSET TI TO 2\nMOVE 'Z' TO ELEM(TI)\nDISPLAY ELEM(2)\nSET P TO ADDRESS OF V\nSET P32 TO P\nSET FP TO ENTRY 'SUB'\nCALL FP USING BY VALUE P\nDISPLAY V\nDISPLAY LENGTH OF V\nSTOP RUN.",
    );
    assert!(main.programs[0].dll);
    assert!(!main.programs[0].thread);
    let ti = main.programs[0]
        .descs
        .iter()
        .find(|d| d.name == "TI")
        .unwrap();
    assert_eq!(ti.len, 8);
    let main = zeus_cobol::load_module_bytes(&zeus_cobol::module_bytes(&main).unwrap()).unwrap();
    let out = Capture::default();
    let mut machine = Machine::new(vec![dir.path().to_path_buf()]);
    machine.set_stdout(Box::new(out.clone()));
    machine.run_main(main).unwrap();
    assert_eq!(&*out.0.borrow(), b"WIDTHS\nZ\nHI\n000000000000000002\n");
}
#[test]
fn length_register_arguments_use_the_selected_binary_width() {
    let dir = tempfile::tempdir().unwrap();
    for (lp, digits) in [(32, 9), (64, 18)] {
        let text = format!(
            "CBL LP({lp})\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 TEXT-VALUE PIC XX.\nPROCEDURE DIVISION.\nCALL 'SUB' USING BY CONTENT LENGTH OF TEXT-VALUE\nCALL 'VALUE-ENTRY' USING BY VALUE LENGTH OF TEXT-VALUE\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nLINKAGE SECTION.\n01 COUNT-VALUE PIC 9({digits}) BINARY.\nPROCEDURE DIVISION USING COUNT-VALUE.\nIF COUNT-VALUE = 2 DISPLAY 'CONTENT' END-IF\nMOVE 99 TO COUNT-VALUE\nGOBACK.\nENTRY 'VALUE-ENTRY' USING BY VALUE COUNT-VALUE.\nIF COUNT-VALUE = 2 DISPLAY 'VALUE' END-IF\nMOVE 99 TO COUNT-VALUE\nGOBACK.\nEND PROGRAM SUB."
        );
        let module = compile(dir.path(), "MAIN", &text);
        let out = Capture::default();
        let mut machine = Machine::new(vec![]);
        machine.set_stdout(Box::new(out.clone()));
        machine.run_main(module).unwrap();
        assert_eq!(&*out.0.borrow(), b"CONTENT\nVALUE\n", "LP({lp})");
    }
}

#[test]
fn dll_linkage_rejects_different_address_modes() {
    let dir = tempfile::tempdir().unwrap();
    let sub = compile(
        dir.path(),
        "SUB",
        "CBL LP(32) EXPORTALL\nID DIVISION.\nPROGRAM-ID. SUB.\nPROCEDURE DIVISION.\nGOBACK.",
    );
    zeus_cobol::save_module(&sub, &dir.path().join("SUB.zo")).unwrap();
    let main = compile(
        dir.path(),
        "MAIN",
        "CBL LP(64)\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nCALL 'SUB'\nSTOP RUN.",
    );
    let error = Machine::new(vec![dir.path().to_path_buf()])
        .run_main(main)
        .unwrap_err();
    assert!(error.to_string().contains("same LP"), "{error}");
}

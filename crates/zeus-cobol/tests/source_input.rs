//! External source representations and byte-based reference-format columns.
use std::{cell::RefCell, io::Write, path::Path, rc::Rc};
use zeus_cobol::{
    front::{FrontOptions, SourceEncoding},
    vm::Machine,
    zir::Module,
};

#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Vec<u8>>>);
impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn record(body: &str, indicator: char, ccsid: u32) -> Vec<u8> {
    let page = zeus_cobol::codepage::get(ccsid).unwrap();
    let mut bytes = page.encode(&format!("      {indicator}{body}"));
    assert!(bytes.len() <= 72, "{body}");
    bytes.resize(72, 0x40);
    bytes.extend(page.encode("BAD'CODE")); // Identification area, not program text.
    assert_eq!(bytes.len(), 80);
    bytes
}
fn write(path: &Path, text: &str, ccsid: u32, encoding: SourceEncoding) {
    let bytes: Vec<_> = text
        .lines()
        .flat_map(|body| {
            let mut bytes = record(body, ' ', ccsid);
            if encoding == SourceEncoding::Ebcdic {
                bytes.push(0x15);
            }
            bytes
        })
        .collect();
    std::fs::write(path, bytes).unwrap();
}
fn run_bytes(module: Module) -> Vec<u8> {
    let output = Capture::default();
    let mut machine = Machine::new(vec![]);
    machine.set_stdout(Box::new(output.clone()));
    machine.run_main(module).unwrap();
    output.0.borrow().clone()
}
fn run(module: Module) -> String {
    String::from_utf8(run_bytes(module)).unwrap()
}

#[test]
fn ebcdic_sources_use_source_codepage_and_preserve_byte_columns_in_all_pages() {
    for (ccsid, value) in [
        (37, "café"),
        (1140, "€"),
        (930, "日本ｶ"),
        (939, "日本"),
        (1390, "日本"),
        (1399, "日本"),
    ] {
        for encoding in [SourceEncoding::Ebcdic, SourceEncoding::Ebcdic80] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("main.cbl");
            let text = format!(
                "CBL CP({ccsid}) DEFINE(Target='{value}')\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\n>>DEFINE Target AS PARAMETER\n>>IF Target = '{value}'\nDISPLAY '{value}'\n>>ELSE\nDISPLAY 'WRONG'\n>>END-IF\nSTOP RUN."
            );
            write(&path, &text, ccsid, encoding);
            let (result, listing) = zeus_cobol::compile_file_with_listing(
                &path,
                &FrontOptions {
                    source_encoding: encoding,
                    ..Default::default()
                },
            );
            let module = result.unwrap_or_else(|e| panic!("{ccsid} {encoding:?}: {e}"));
            assert_eq!(module.ccsid, ccsid);
            assert!(!listing.contains('\u{FDD0}'));
            assert!(listing.contains(value));
            let expected = if ccsid == 37 {
                format!("{value}\n").chars().map(|c| c as u8).collect()
            } else {
                format!("{value}\n").into_bytes()
            };
            assert_eq!(run_bytes(module), expected);
        }
    }
}

#[test]
fn call_names_are_decoded_in_each_programs_selected_codepage() {
    for ccsid in zeus_cobol::codepage::CCSIDS {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.cbl");
        let text = format!(
            "CBL CP({ccsid})\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nCALL 'SUB@'\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. 'SUB@'.\nPROCEDURE DIVISION.\nDISPLAY 'OK'\nGOBACK.\nEND PROGRAM 'SUB@'."
        );
        write(&path, &text, ccsid, SourceEncoding::Ebcdic80);
        let module = zeus_cobol::compile_file(
            &path,
            &FrontOptions {
                source_encoding: SourceEncoding::Ebcdic80,
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{ccsid}: {e}"));
        assert_eq!(run_bytes(module), b"OK\n", "CCSID {ccsid}");
    }
}

#[test]
fn all_supported_codepages_compile_external_cards_and_national_conversion() {
    for ccsid in zeus_cobol::codepage::CCSIDS {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.cbl");
        let text = format!(
            "CBL CP({ccsid})\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 TEXT-VALUE PIC XXX VALUE 'ABC'.\n01 NATIONAL-VALUE PIC N(3).\nPROCEDURE DIVISION.\nMOVE FUNCTION NATIONAL-OF(TEXT-VALUE) TO NATIONAL-VALUE\nDISPLAY FUNCTION DISPLAY-OF(NATIONAL-VALUE)\nSTOP RUN."
        );
        write(&path, &text, ccsid, SourceEncoding::Ebcdic80);
        let module = zeus_cobol::compile_file(
            &path,
            &FrontOptions {
                source_encoding: SourceEncoding::Ebcdic80,
                ..Default::default()
            },
        )
        .unwrap_or_else(|error| panic!("CCSID {ccsid}: {error}"));
        assert_eq!(run_bytes(module), b"ABC\n", "CCSID {ccsid}");
    }
}

#[test]
fn ebcdic_record_separators_accept_host_and_ebcdic_newlines() {
    for separator in [vec![0x15], vec![0x25], vec![0x0A], vec![0x0D, 0x0A]] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.cbl");
        let mut bytes = Vec::new();
        for line in [
            "ID DIVISION.",
            "PROGRAM-ID. MAIN.",
            "PROCEDURE DIVISION.",
            "DISPLAY 'OK'",
            "STOP RUN.",
        ] {
            bytes.extend(record(line, ' ', 37));
            bytes.extend_from_slice(&separator);
        }
        std::fs::write(&path, bytes).unwrap();
        let options = FrontOptions {
            source_encoding: SourceEncoding::Ebcdic,
            ..Default::default()
        };
        assert_eq!(
            run(zeus_cobol::compile_file(&path, &options).unwrap()),
            "OK\n"
        );
    }
}

#[test]
fn copy_directives_replacement_and_dbcs_names_use_the_selected_input_encoding() {
    for encoding in [SourceEncoding::Ebcdic, SourceEncoding::Ebcdic80] {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("main.cbl");
        write(
            &dir.path().join("DECL.cpy"),
            ">>DEFINE Chosen AS '日本'\n01 RAW PIC X(6) VALUE '日本'.\n01 GFIELD PIC G(2) VALUE G'日本'.",
            939,
            encoding,
        );
        write(
            &path,
            "CBL CP(939)\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\nCOPY DECL REPLACING ==RAW== BY ==日本==.\nPROCEDURE DIVISION.\n>>IF Chosen = '日本'\nDISPLAY 日本\nDISPLAY GFIELD\n>>END-IF\nSTOP RUN.",
            939,
            encoding,
        );
        let (module, listing) = zeus_cobol::compile_file_with_listing(
            &path,
            &FrontOptions {
                source_encoding: encoding,
                ..Default::default()
            },
        );
        assert!(listing.contains("DECL.cpy"));
        assert!(listing.contains("DISPLAY 日本"));
        assert!(!listing.contains('\u{FDD0}'));
        assert_eq!(run(module.unwrap()), "日本\n日本\n");
    }
}

#[test]
fn dbcs_continued_literals_use_original_byte_padding_and_indicators() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("continued.cbl");
    let mut bytes = Vec::new();
    for line in [
        "CBL CP(939)",
        "ID DIVISION.",
        "PROGRAM-ID. MAIN.",
        "PROCEDURE DIVISION.",
    ] {
        bytes.extend(record(line, ' ', 939));
    }
    let body = "    DISPLAY '日";
    let padding = 72
        - zeus_cobol::codepage::get(939)
            .unwrap()
            .encode(&format!("       {body}"))
            .len();
    bytes.extend(record(body, ' ', 939));
    bytes.extend(record("    '本'", '-', 939));
    bytes.extend(record("STOP RUN.", ' ', 939));
    std::fs::write(&path, bytes).unwrap();
    let module = zeus_cobol::compile_file(
        &path,
        &FrontOptions {
            source_encoding: SourceEncoding::Ebcdic80,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(run(module), format!("日{}本\n", " ".repeat(padding)));
}

#[test]
fn unicode_copy_overrides_and_utf8_bom_preserve_unicode_without_lossy_decoding() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    let copy = dir.path().join("TEXT.cpy");
    std::fs::write(&copy, "       DISPLAY '日本'\n").unwrap();
    write(
        &path,
        "CBL CP(939)\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nCOPY TEXT.\nSTOP RUN.",
        939,
        SourceEncoding::Ebcdic80,
    );
    let mut options = FrontOptions {
        source_encoding: SourceEncoding::Ebcdic80,
        ..Default::default()
    };
    options
        .copy_encodings
        .insert(copy.canonicalize().unwrap(), SourceEncoding::Utf8);
    assert_eq!(
        run(zeus_cobol::compile_file(&path, &options).unwrap()),
        "日本\n"
    );
    write(
        &path,
        "CBL CP(939)\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\nCOPY SQLCA.\nPROCEDURE DIVISION.\nIF SQLCODE = 0 DISPLAY 'OK' END-IF\nSTOP RUN.",
        939,
        SourceEncoding::Ebcdic80,
    );
    assert_eq!(
        run(zeus_cobol::compile_file(&path, &options).unwrap()),
        "OK\n"
    );
    std::fs::write(&path, "\u{FEFF}       ID DIVISION.\n       PROGRAM-ID. MAIN.\n       PROCEDURE DIVISION.\n       DISPLAY 'OK'\n       STOP RUN.\n").unwrap();
    assert_eq!(
        run(zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap()),
        "OK\n"
    );
    std::fs::write(&path, [0xFF]).unwrap();
    let error = zeus_cobol::compile_file(&path, &FrontOptions::default()).unwrap_err();
    assert!(error.message.contains("invalid UTF-8"));
    assert_eq!(error.file.as_deref(), path.to_str());
}

#[test]
fn invalid_ebcdic_records_and_copy_data_report_their_actual_file_and_line() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    let options = FrontOptions {
        source_encoding: SourceEncoding::Ebcdic80,
        compiler_options: "CP(939)".into(),
        ..Default::default()
    };
    std::fs::write(&path, [0x40; 79]).unwrap();
    assert!(
        zeus_cobol::compile_file(&path, &options)
            .unwrap_err()
            .message
            .contains("multiple of 80")
    );
    write(
        &path,
        "ID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nCOPY BAD.\nSTOP RUN.",
        939,
        SourceEncoding::Ebcdic80,
    );
    let copy = dir.path().join("BAD.cpy");
    for malformed in [vec![0x0E, 0x42], vec![0x0F], vec![0x0E, 0x01, 0x01, 0x0F]] {
        let mut bytes = vec![0x40; 80];
        bytes[7..7 + malformed.len()].copy_from_slice(&malformed);
        std::fs::write(&copy, bytes).unwrap();
        let error = zeus_cobol::compile_file(&path, &options).unwrap_err();
        assert!(error.message.contains("invalid EBCDIC"), "{error}");
        assert_eq!(error.pos.line, 1);
        assert_eq!(error.file.as_deref(), copy.to_str());
    }
}

#[test]
fn cli_compiles_checks_and_lists_ebcdic_input_with_explicit_record_format() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.cbl");
    let listing = dir.path().join("main.lst");
    let module = dir.path().join("main.zo");
    write(
        &path,
        "CBL CP(1140)\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nDISPLAY '€'\nSTOP RUN.",
        1140,
        SourceEncoding::Ebcdic80,
    );
    let cli = env!("CARGO_BIN_EXE_zcobc");
    let out = std::process::Command::new(cli)
        .args(["compile", "--source-encoding", "ebcdic80", "--listing"])
        .arg(&listing)
        .arg("-o")
        .arg(&module)
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        std::fs::read_to_string(listing)
            .unwrap()
            .contains("DISPLAY '€'")
    );
    assert_eq!(run(zeus_cobol::load_module(&module).unwrap()), "€\n");
    for command in ["check", "inventory"] {
        let out = std::process::Command::new(cli)
            .args([command, "--source-encoding", "ebcdic80"])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

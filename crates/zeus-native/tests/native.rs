//! Native executables: the run time (`zeus-rt`) with modules appended runs
//! them on its own. A COBOL main program calling a PL/I procedure (PARM,
//! ACCEPT, the return code), a PL/I main program reading a DD, and COBOL
//! machine code giving the interpreter's results.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use zeus_cobol::image::Image;

fn programs() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs")
}

#[test]
fn dll_calls_share_entry_storage_in_native_and_interpreted_executables() {
    let dir = tempfile::tempdir().unwrap();
    for lp64 in [false, true] {
        let mut members = Vec::new();
        for (name, text) in [
            (
                "MAIN",
                "CBL DLL\nID DIVISION.\nPROGRAM-ID. MAIN.\nPROCEDURE DIVISION.\nCALL 'PRIMARY'\nCANCEL 'PRIMARY'\nCALL 'ALTENTRY'\nCALL 'PRIMARY'\nSTOP RUN.",
            ),
            (
                "PRIMARY",
                "CBL EXPORTALL\nID DIVISION.\nPROGRAM-ID. PRIMARY.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 99 VALUE 0.\nPROCEDURE DIVISION.\nADD 1 TO N\nDISPLAY N\nGOBACK.\nENTRY 'ALTENTRY'.\nADD 10 TO N\nDISPLAY N\nGOBACK.",
            ),
        ] {
            let text = if lp64 {
                format!("CBL LP(64)\n{text}")
            } else {
                text.into()
            };
            let path = dir.path().join(format!("{name}.cbl"));
            std::fs::write(
                &path,
                text.lines()
                    .map(|line| format!("       {line}\n"))
                    .collect::<String>(),
            )
            .unwrap();
            let module = zeus_cobol::compile_file(&path, &Default::default()).unwrap();
            members.push((name.into(), zeus_cobol::module_bytes(&module).unwrap()));
        }
        let exe = executable(
            dir.path(),
            if lp64 { "dll64" } else { "dll32" },
            &Image { members },
        );
        for interpret in [false, true] {
            assert_eq!(run_env(&exe, &[], "", interpret), ("01\n11\n12".into(), 0));
        }
    }
}

#[test]
fn ebcdic_dbcs_source_runs_as_native_and_interpreted_executable() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("main.cbl");
    let page = zeus_cobol::codepage::get(939).unwrap();
    let text = "CBL CP(939)\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 TEXT-FIELD PIC X(6) VALUE '日本'.\nPROCEDURE DIVISION.\nDISPLAY TEXT-FIELD\nSTOP RUN.";
    let cards: Vec<_> = text
        .lines()
        .flat_map(|line| {
            let mut record = page.encode(&format!("       {line}"));
            assert!(record.len() <= 72);
            record.resize(80, 0x40);
            record
        })
        .collect();
    std::fs::write(&source, cards).unwrap();
    let module = zeus_cobol::compile_file(
        &source,
        &zeus_cobol::front::FrontOptions {
            source_encoding: zeus_cobol::front::SourceEncoding::Ebcdic80,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        !zeus_cobol::native::compile(&module, &zeus_cobol::native::host_triple())
            .unwrap()
            .programs
            .is_empty()
    );
    let image = Image {
        members: vec![("MAIN".into(), zeus_cobol::module_bytes(&module).unwrap())],
    };
    let exe = executable(dir.path(), "ebcdic-input", &image);
    for interpret in [false, true] {
        assert_eq!(run_env(&exe, &[], "", interpret), ("日本".into(), 0));
    }
}

#[test]
fn optimize_levels_preserve_results_in_generated_executables() {
    let dir = tempfile::tempdir().unwrap();
    for level in 0..=2 {
        let source = dir.path().join("opt.cbl");
        let text = format!(
            "CBL OPT({level})\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 I PIC 9(4) COMP-5.\n01 TOTAL PIC 9(9) COMP-5 VALUE 0.\nPROCEDURE DIVISION.\nPERFORM VARYING I FROM 1 BY 1 UNTIL I > 10\nCOMPUTE TOTAL = TOTAL + I * I\nEND-PERFORM\nDISPLAY TOTAL\nSTOP RUN."
        );
        std::fs::write(
            &source,
            text.lines()
                .map(|line| format!("       {line}\n"))
                .collect::<String>(),
        )
        .unwrap();
        let module = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
        assert_eq!(module.optimize, level);
        assert!(
            !zeus_cobol::native::compile(&module, &zeus_cobol::native::host_triple())
                .unwrap()
                .programs
                .is_empty()
        );
        let image = Image {
            members: vec![("MAIN".into(), zeus_cobol::module_bytes(&module).unwrap())],
        };
        let exe = executable(dir.path(), &format!("opt{level}"), &image);
        for interpret in [false, true] {
            assert_eq!(run_env(&exe, &[], "", interpret), ("0000000385".into(), 0));
        }
    }
}

#[test]
fn dynam_cancel_and_static_call_state_match_in_standalone_executables() {
    let dir = tempfile::tempdir().unwrap();
    let sub_path = dir.path().join("sub.cbl");
    let sub_source = "ID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 N PIC 9 VALUE 0.\nPROCEDURE DIVISION.\nADD 1 TO N\nDISPLAY N\nGOBACK.";
    std::fs::write(
        &sub_path,
        sub_source
            .lines()
            .map(|line| format!("       {line}\n"))
            .collect::<String>(),
    )
    .unwrap();
    let sub = zeus_cobol::compile_file(&sub_path, &Default::default()).unwrap();
    for (option, directive, pointer, expected) in [
        ("DYNAM", "", false, "1\n1"),
        ("NODYNAM", "", false, "1\n2"),
        ("NODYNAM", ">>CALLINTERFACE DYNAMIC", false, "1\n1"),
        ("DYNAM", ">>CALLINTERFACE STATIC", false, "1\n2"),
        ("DYNAM", "", true, "1\n1"),
        ("NODYNAM", "", true, "1\n2"),
        ("NODYNAM", ">>CALLINTERFACE DYNAMIC", true, "1\n1"),
        ("DYNAM", ">>CALLINTERFACE STATIC", true, "1\n2"),
    ] {
        let main_path = dir.path().join("main.cbl");
        let main_source = format!(
            "CBL {option}\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 P USAGE PROCEDURE-POINTER.\nPROCEDURE DIVISION.\nCALL 'SUB'\nCANCEL 'SUB'\nCALL 'SUB'\nSTOP RUN."
        );
        let main_source = main_source.replace(
            "PROCEDURE DIVISION.\n",
            &format!("PROCEDURE DIVISION.\n{directive}\n"),
        );
        let main_source = if pointer {
            main_source.replace("CALL 'SUB'", "SET P TO ENTRY 'SUB'\nCALL P")
        } else {
            main_source
        };
        std::fs::write(
            &main_path,
            main_source
                .lines()
                .map(|line| format!("       {line}\n"))
                .collect::<String>(),
        )
        .unwrap();
        let main = zeus_cobol::compile_file(&main_path, &Default::default()).unwrap();
        let image = Image {
            members: vec![
                ("MAIN".into(), zeus_cobol::module_bytes(&main).unwrap()),
                ("SUB".into(), zeus_cobol::module_bytes(&sub).unwrap()),
            ],
        };
        let exe = executable(dir.path(), &format!("{option}-{pointer}"), &image);
        for interpret in [false, true] {
            assert_eq!(run_env(&exe, &[], "", interpret), (expected.into(), 0));
        }
    }
}

#[test]
fn ssrange_in_standalone_executables_preserves_national_functions_and_checks() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("range.cbl");
    let text = "CBL SSRANGE\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC X OCCURS 2.\nPROCEDURE DIVISION.\nIF FUNCTION REVERSE(N'日😀a') = N'a😀日'\nDISPLAY 'UNICODE' END-IF\nDISPLAY A(3)\nSTOP RUN.";
    std::fs::write(
        &source,
        text.lines()
            .map(|line| format!("       {line}\n"))
            .collect::<String>(),
    )
    .unwrap();
    let module = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
    assert!(
        zeus_cobol::native::compile(&module, &zeus_cobol::native::host_triple())
            .unwrap()
            .programs
            .is_empty()
    );
    let image = Image {
        members: vec![("MAIN".into(), zeus_cobol::module_bytes(&module).unwrap())],
    };
    let exe = executable(dir.path(), "range", &image);
    for interpret in [false, true] {
        let mut command = Command::new(&exe);
        if interpret {
            command.env("ZEUS_INTERPRET", "1");
        } else {
            command.env_remove("ZEUS_INTERPRET");
        }
        let result = command.output().unwrap();
        assert!(!result.status.success());
        assert_eq!(result.stdout, b"UNICODE\n");
        assert!(String::from_utf8_lossy(&result.stderr).contains("SSRANGE"));
    }
}

#[test]
fn apost_figuratives_match_in_native_and_interpreted_executables() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("apost.cbl");
    let text = "CBL APOST\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC XXX VALUE QUOTES.\n01 B PIC XXX.\nPROCEDURE DIVISION.\nMOVE QUOTES TO B\nIF A = B AND B = QUOTES DISPLAY A B\nELSE MOVE 9 TO RETURN-CODE END-IF\nSTOP RUN.";
    std::fs::write(
        &source,
        text.lines()
            .map(|line| format!("       {line}\n"))
            .collect::<String>(),
    )
    .unwrap();
    let module = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
    assert!(zeus_cobol::native::compile(&module, &zeus_cobol::native::host_triple()).is_ok());
    let image = Image {
        members: vec![("MAIN".into(), zeus_cobol::module_bytes(&module).unwrap())],
    };
    let exe = executable(dir.path(), "apost", &image);
    assert_eq!(run_env(&exe, &[], "", false), ("''''''".into(), 0));
    assert_eq!(run_env(&exe, &[], "", true), ("''''''".into(), 0));
}

#[test]
fn returning_national_trunc_and_entry_pointers_match_interpreted_execution() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("compat.cbl");
    let text = "CBL TRUNC(BIN)\nID DIVISION.\nPROGRAM-ID. MAIN.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 B PIC 9(4) BINARY VALUE 12345.\n01 P PROCEDURE-POINTER.\n01 F FUNCTION-POINTER.\n01 NAT-G GROUP-USAGE NATIONAL.\n05 A PIC NN.\n05 N PIC 99.\nPROCEDURE DIVISION.\nDISPLAY B\nMOVE N'AB12' TO NAT-G\nADD 1 TO N\nDISPLAY NAT-G\nSET P TO ENTRY 'SUB'\nSET F TO P\nCALL F RETURNING B\nADD 1 TO B\nDISPLAY B\nDISPLAY FUNCTION WHEN-COMPILED\nSTOP RUN.\nEND PROGRAM MAIN.\nID DIVISION.\nPROGRAM-ID. SUB.\nDATA DIVISION.\nLINKAGE SECTION.\n01 RESULT PIC 9(4) COMP-5.\nPROCEDURE DIVISION RETURNING RESULT.\nMOVE 54321 TO RESULT\nGOBACK.\nEND PROGRAM SUB.";
    std::fs::write(
        &source,
        text.lines()
            .map(|s| format!("       {s}\n"))
            .collect::<String>(),
    )
    .unwrap();
    let mut module = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
    module.compiled_at = Some("2000010203040567+0000".into());
    let image = Image {
        members: vec![("MAIN".into(), zeus_cobol::module_bytes(&module).unwrap())],
    };
    let exe = executable(dir.path(), "compat", &image);
    let native = run_env(&exe, &[], "", false);
    assert_eq!(native, run_env(&exe, &[], "", true));
    assert_eq!(
        native,
        ("12345\nAB13\n54322\n2000010203040567+0000".into(), 0)
    );
}

fn pli(name: &str) -> (String, Vec<u8>) {
    let (rc, c) = zeus_pli::compile_file(
        &programs().join(format!("{name}.pli")),
        &zeus_pli::Options::default(),
    );
    let m = c
        .module
        .unwrap_or_else(|| panic!("{name}: rc {rc}: {:?}", c.messages));
    (m.name.clone(), zeus_pli::module_bytes(&m).unwrap())
}

fn executable(dir: &Path, name: &str, image: &Image) -> PathBuf {
    let exe = dir.join(name);
    let mut image = image.clone();
    image
        .add_machine_code(&zeus_cobol::native::host_triple())
        .unwrap();
    image
        .write_executable(Path::new(env!("CARGO_BIN_EXE_zeus-rt")), &exe)
        .unwrap();
    exe
}

fn run(exe: &Path, args: &[&str], stdin: &str) -> (String, i32) {
    run_env(exe, args, stdin, false)
}

fn run_env(exe: &Path, args: &[&str], stdin: &str, interpret: bool) -> (String, i32) {
    let mut cmd = Command::new(exe);
    if interpret {
        cmd.env("ZEUS_INTERPRET", "1");
    } else {
        cmd.env_remove("ZEUS_INTERPRET");
    }
    let mut child = cmd
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n");
    (text, out.status.code().unwrap_or(-1))
}

#[test]
fn native_cobol_calls_embedded_hlasm_with_shared_storage() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("asmadd.asm");
    std::fs::write(&source, "ASMADD CSECT\n         USING *,15\n         LR 10,14\n         L 15,=V(ASMLEAF)\n         BASR 14,15\n         LR 14,10\n         BR 14\n         END\n").unwrap();
    let asm = zeus_cobol::hlasm::compile_file(&source, &[]).unwrap();
    let leaf_source = dir.path().join("leaf.asm");
    std::fs::write(&leaf_source, "ASMLEAF CSECT\n         USING *,15\n         L 2,0(1)\n         L 4,=A(INCR)\n         L 3,0(2)\n         A 3,0(4)\n         ST 3,0(2)\n         LA 15,9\n         BR 14\nINCR DC F'7'\n         END\n").unwrap();
    let leaf = zeus_cobol::hlasm::compile_file(&leaf_source, &[]).unwrap();
    let source = dir.path().join("main.cbl");
    std::fs::write(&source, "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAINPGM.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 A PIC S9(9) COMP VALUE 5.\n       PROCEDURE DIVISION.\n           CALL 'ASMADD' USING A\n           IF A = 12 AND RETURN-CODE = 9\n             MOVE 0 TO RETURN-CODE\n           ELSE MOVE 99 TO RETURN-CODE END-IF\n           STOP RUN.\n").unwrap();
    let cobol = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
    let image = Image {
        members: vec![
            ("MAINPGM".into(), zeus_cobol::module_bytes(&cobol).unwrap()),
            (
                "ASMADD".into(),
                zeus_cobol::hlasm::module_bytes(&asm).unwrap(),
            ),
            (
                "ASMLEAF".into(),
                zeus_cobol::hlasm::module_bytes(&leaf).unwrap(),
            ),
        ],
    };
    let exe = executable(dir.path(), "shared-asm", &image);
    assert_eq!(run(&exe, &[], "").1, 0);
    assert_eq!(run_env(&exe, &[], "", true).1, 0);
}

#[test]
fn assembler_links_back_to_native_cobol_and_pli_with_shared_arguments() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bridge.asm");
    std::fs::write(&path, "BRIDGE RSECT\n USING BRIDGE,12\n LR 12,15\n L 11,PREF\n C 11,=A(RET)\n BNE EARLY\n ST 14,RET\n ST 1,ARGS\n LINK EP=CBADD\n L 1,ARGS\n LINK EP=PSUB\n L 14,RET\n BR 14\nEARLY LA 15,99\n BR 14\nWORK CATTR NOTEXECUTABLE,ALIGN(4),RMODE(31)\nRET DS F\nARGS DS F\nPREF DC RP(C'CTX!')(BRIDGE)\nBRIDGE XATTR PSECT(RET)\n END BRIDGE\n").unwrap();
    let asm = zeus_cobol::hlasm::compile_file(&path, &[]).unwrap();
    let path = dir.path().join("cbadd.cbl");
    std::fs::write(&path, "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. CBADD.\n       DATA DIVISION.\n       LINKAGE SECTION.\n       01 ARG PIC S9(9) COMP.\n       PROCEDURE DIVISION USING ARG.\n           ADD 7 TO ARG\n           MOVE 9 TO RETURN-CODE\n           GOBACK.\n").unwrap();
    let sub = zeus_cobol::compile_file(&path, &Default::default()).unwrap();
    let path = dir.path().join("psub.pli");
    std::fs::write(&path, " PSUB: PROC(V) OPTIONS(ASM);\n DCL V FIXED BIN(31);\n V=V+11;\n CALL PLIRETC(17);\n END PSUB;\n").unwrap();
    let (rc, compiled) = zeus_pli::compile_file(&path, &Default::default());
    let pli = compiled
        .module
        .unwrap_or_else(|| panic!("rc {rc}: {:?}", compiled.messages));
    assert!(
        pli.procs.iter().any(|p| p.name == "PSUB" && p.external),
        "{:?}",
        pli.procs
    );
    zeus_pli::load_module_bytes(&zeus_pli::module_bytes(&pli).unwrap()).unwrap();
    let path = dir.path().join("main.cbl");
    std::fs::write(&path, "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAINPGM.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 ARG PIC S9(9) COMP VALUE 5.\n       PROCEDURE DIVISION.\n           CALL 'BRIDGE' USING ARG\n           IF ARG = 23 AND RETURN-CODE = 17\n             MOVE 0 TO RETURN-CODE\n           ELSE MOVE 99 TO RETURN-CODE END-IF\n           STOP RUN.\n").unwrap();
    let main = zeus_cobol::compile_file(&path, &Default::default()).unwrap();
    let image = Image {
        members: vec![
            ("MAINPGM".into(), zeus_cobol::module_bytes(&main).unwrap()),
            ("CBADD".into(), zeus_cobol::module_bytes(&sub).unwrap()),
            ("PSUB".into(), zeus_pli::module_bytes(&pli).unwrap()),
            (
                "BRIDGE".into(),
                zeus_cobol::hlasm::module_bytes(&asm).unwrap(),
            ),
        ],
    };
    let exe = executable(dir.path(), "asm-reverse-call", &image);
    assert_eq!(run(&exe, &[], "").1, 0);
    assert_eq!(run_env(&exe, &[], "", true).1, 0);
    let path = dir.path().join("loaded.asm");
    std::fs::write(&path, "BRIDGE CSECT\n USING BRIDGE,12\n LR 12,15\n ST 14,RET\n ST 1,ARGS\n LINK EP=CBADD\n LOAD EP=PSUB\n LR 8,0\n L 1,ARGS\n L 2,0(1)\n ST 2,FIXED\n NI FIXED,X'7F'\n LA 1,FIXED\n LR 15,8\n BASR 14,15\n ST 15,CODE\n DELETE EP=PSUB\n L 15,CODE\n L 14,RET\n BR 14\nRET DS F\nARGS DS F\nFIXED DS F\nCODE DS F\n END\n").unwrap();
    let loaded = zeus_cobol::hlasm::compile_file(&path, &[]).unwrap();
    let mut image = image;
    image.members.last_mut().unwrap().1 = zeus_cobol::hlasm::module_bytes(&loaded).unwrap();
    let exe = executable(dir.path(), "asm-loaded-pli", &image);
    assert_eq!(run(&exe, &[], "").1, 0);
    assert_eq!(run_env(&exe, &[], "", true).1, 0);
}

#[test]
fn native_cobol_calls_assembler_record_io_with_the_same_dds() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("reader.asm");
    std::fs::write(&path,"READER CSECT\n USING READER,12\n LR 12,15\n L 10,0(1)\n XR 6,6\n OPEN (IN,INPUT)\nLOOP GET IN\n AHI 6,1\n B LOOP\nEOF CLOSE (IN)\n ST 6,0(10)\n LA 15,0\n BR 14\nIN DCB DDNAME=INFILE,MACRF=GL,RECFM=F,LRECL=4,EODAD=EOF\n END\n").unwrap();
    let asm = zeus_cobol::hlasm::compile_file(&path, &[]).unwrap();
    let path = dir.path().join("main.cbl");
    std::fs::write(&path,"       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAINPGM.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 CNT PIC S9(9) COMP VALUE 0.\n       PROCEDURE DIVISION.\n           CALL 'READER' USING CNT\n           IF CNT = 3 AND RETURN-CODE = 0\n             MOVE 0 TO RETURN-CODE\n           ELSE MOVE 99 TO RETURN-CODE END-IF\n           STOP RUN.\n").unwrap();
    let main = zeus_cobol::compile_file(&path, &Default::default()).unwrap();
    let image = Image {
        members: vec![
            ("MAINPGM".into(), zeus_cobol::module_bytes(&main).unwrap()),
            (
                "READER".into(),
                zeus_cobol::hlasm::module_bytes(&asm).unwrap(),
            ),
        ],
    };
    let input = dir.path().join("input");
    std::fs::write(&input, zeus_cobol::ebcdic::encode("ABCD1234WXYZ")).unwrap();
    let dd = format!("INFILE={}", input.display());
    let exe = executable(dir.path(), "assembler-io", &image);
    assert_eq!(run(&exe, &["--dd", &dd], "").1, 0);
    assert_eq!(run_env(&exe, &["--dd", &dd], "", true).1, 0);
}

#[test]
fn native_cobol_calls_hlasm_with_reversed_bytes_and_long_displacements() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("asmrev.asm");
    std::fs::write(&source, "ASMREV CSECT\n         USING ASMREV,15\n         L 2,0(1)\n         L 3,4(1)\n         L 6,8(1)\n         LRVG 4,0(2)\n         STG 4,0(3)\n         LY 5,DELTA\n         STY 5,0(6)\n         LGHI 15,0\n         BR 14\nGAP DS 5000X\nDELTA DC F'7'\n         END\n").unwrap();
    let asm = zeus_cobol::hlasm::compile_file(&source, &[]).unwrap();
    let source = dir.path().join("main.cbl");
    std::fs::write(
        &source,
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAINPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC X(8) VALUE 'ABCDEFGH'.
       01 B PIC X(8) VALUE SPACES.
       01 N PIC S9(9) COMP VALUE 0.
       PROCEDURE DIVISION.
           CALL 'ASMREV' USING A B N
           IF A = 'ABCDEFGH' AND B = 'HGFEDCBA' AND N = 7
             MOVE 0 TO RETURN-CODE
           ELSE MOVE 99 TO RETURN-CODE END-IF
           STOP RUN.
",
    )
    .unwrap();
    let cobol = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
    let image = Image {
        members: vec![
            ("MAINPGM".into(), zeus_cobol::module_bytes(&cobol).unwrap()),
            (
                "ASMREV".into(),
                zeus_cobol::hlasm::module_bytes(&asm).unwrap(),
            ),
        ],
    };
    let exe = executable(dir.path(), "reverse-asm", &image);
    assert_eq!(run(&exe, &[], "").1, 0);
    assert_eq!(run_env(&exe, &[], "", true).1, 0);
}

#[test]
fn native_cobol_calls_hlasm_with_carry_and_partial_register_immediates() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("asmbits.asm");
    std::fs::write(&source, "ASMBITS CSECT\n         L 2,0(1)\n         LG 4,0(2)\n         LLILL 3,1\n         ALR 4,3\n         ALCGR 4,3\n         IIHF 4,1\n         STG 4,0(2)\n         LGHI 15,0\n         BR 14\n         END\n").unwrap();
    let asm = zeus_cobol::hlasm::compile_file(&source, &[]).unwrap();
    let source = dir.path().join("main.cbl");
    std::fs::write(
        &source,
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAINPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC S9(18) COMP VALUE 4294967295.
       PROCEDURE DIVISION.
           CALL 'ASMBITS' USING A
           IF A = 4294967298 AND RETURN-CODE = 0
             MOVE 0 TO RETURN-CODE
           ELSE MOVE 99 TO RETURN-CODE END-IF
           STOP RUN.
",
    )
    .unwrap();
    let cobol = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
    let image = Image {
        members: vec![
            ("MAINPGM".into(), zeus_cobol::module_bytes(&cobol).unwrap()),
            (
                "ASMBITS".into(),
                zeus_cobol::hlasm::module_bytes(&asm).unwrap(),
            ),
        ],
    };
    let exe = executable(dir.path(), "carry-asm", &image);
    assert_eq!(run(&exe, &[], "").1, 0);
    assert_eq!(run_env(&exe, &[], "", true).1, 0);
}

#[test]
fn native_cobol_calls_hlasm_with_masked_bytes_in_both_register_halves() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("asmmask.asm");
    std::fs::write(&source, "ASMMASK CSECT\n         USING ASMMASK,12\n         LR 12,15\n         L 2,0(1)\n         L 3,4(1)\n         LLILL 5,0\n         ICM 5,5,0(2)\n         STCM 5,5,0(3)\n         ICMH 5,10,0(2)\n         STCMH 5,10,2(3)\n         CLM 5,5,0(2)\n         BNE FAIL\n         CLMH 5,10,0(2)\n         BNE FAIL\n         LGHI 15,0\n         BR 14\nFAIL LGHI 15,99\n         BR 14\n         END\n").unwrap();
    let asm = zeus_cobol::hlasm::compile_file(&source, &[]).unwrap();
    let source = dir.path().join("main.cbl");
    std::fs::write(
        &source,
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAINPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC X(2) VALUE 'AB'.
       01 B PIC X(4) VALUE SPACES.
       PROCEDURE DIVISION.
           CALL 'ASMMASK' USING A B
           IF A = 'AB' AND B = 'ABAB' AND RETURN-CODE = 0
             MOVE 0 TO RETURN-CODE
           ELSE MOVE 99 TO RETURN-CODE END-IF
           STOP RUN.
",
    )
    .unwrap();
    let cobol = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
    let image = Image {
        members: vec![
            ("MAINPGM".into(), zeus_cobol::module_bytes(&cobol).unwrap()),
            (
                "ASMMASK".into(),
                zeus_cobol::hlasm::module_bytes(&asm).unwrap(),
            ),
        ],
    };
    let exe = executable(dir.path(), "masked-asm", &image);
    assert_eq!(run(&exe, &[], "").1, 0);
    assert_eq!(run_env(&exe, &[], "", true).1, 0);
}

#[test]
fn native_cobol_calls_hlasm_with_relative_calls_loops_and_far_data() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("asmrel.asm");
    std::fs::write(&source, "ASMREL CSECT\n         L 10,0(1)\n         L 3,0(10)\n         LARL 12,INCR\n         LGHI 2,3\nLOOP BRASL 6,ADDONE\n         BRCTG 2,LOOP\n         ST 3,0(10)\n         LGHI 15,0\n         BR 14\nADDONE A 3,0(12)\n         BR 6\nGAP DS 70000X\nINCR DC F'7'\n         END\n").unwrap();
    let asm = zeus_cobol::hlasm::compile_file(&source, &[]).unwrap();
    let source = dir.path().join("main.cbl");
    std::fs::write(
        &source,
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAINPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC S9(9) COMP VALUE 5.
       PROCEDURE DIVISION.
           CALL 'ASMREL' USING A
           IF A = 26 AND RETURN-CODE = 0
             MOVE 0 TO RETURN-CODE
           ELSE MOVE 99 TO RETURN-CODE END-IF
           STOP RUN.
",
    )
    .unwrap();
    let cobol = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
    let image = Image {
        members: vec![
            ("MAINPGM".into(), zeus_cobol::module_bytes(&cobol).unwrap()),
            (
                "ASMREL".into(),
                zeus_cobol::hlasm::module_bytes(&asm).unwrap(),
            ),
        ],
    };
    let exe = executable(dir.path(), "relative-asm", &image);
    assert_eq!(run(&exe, &[], "").1, 0);
    assert_eq!(run_env(&exe, &[], "", true).1, 0);
}

#[test]
fn native_cobol_calls_hlasm_with_complex_equ_adcon_and_relative_linkage() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("asmcross.asm");
    std::fs::write(&source, "ASMCROSS CSECT\nTARGET EQU ASMLEAF+SPAN,4,C'A'\nSPAN EQU FINISH-BEGIN-4\n         USING ASMCROSS,15\n         LR 9,14\n         L 15,=A(TARGET)\n         BRASL 14,TARGET\n         LR 14,9\n         BR 14\n         EXTRN ASMLEAF,BEGIN,FINISH\n         END\n").unwrap();
    let caller = zeus_cobol::hlasm::compile_file(&source, &[]).unwrap();
    assert_eq!(caller.zeus_hlasm, 4);
    let source = dir.path().join("asmleaf.asm");
    std::fs::write(&source, "ASMLEAF CSECT\nBEGIN EQU ASMLEAF\nARGREG EQU 2,,,,GR32\nWORK EQU 3,,,,GR32\nDELTA EQU 0,7,C'F',X'00000007',GR32\n&INC SETA I'=FS(24)E(-2)'0E+2'+C2A(SYSATTRP('DELTA'))-7\n         AIF (S'=FS(24)E(-2)'0E+2' EQ 24).LITERAL\n         MNOTE 8,'unexpected literal attributes'\n.LITERAL ANOP\n         AIF (D'WORK AND T'DELTA EQ 'F').CHECK\n         MNOTE 8,'unexpected ordinary attributes'\n.CHECK AIF (SYSATTRA('WORK') EQ 'GR32').OK\n         MNOTE 8,'unexpected assembler type'\n.OK ANOP\n         L ARGREG,0(1)\nFINISH L WORK,0(ARGREG)\n         AHI WORK,&INC\n         ST WORK,0(ARGREG)\n         LGHI 15,T'DELTA-C'F'+9\n         BR 14\n         ENTRY BEGIN,FINISH\n         END\n").unwrap();
    let callee = zeus_cobol::hlasm::compile_file(&source, &[]).unwrap();
    let source = dir.path().join("main.cbl");
    std::fs::write(
        &source,
        "       IDENTIFICATION DIVISION.
       PROGRAM-ID. MAINPGM.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 A PIC S9(9) COMP VALUE 5.
       PROCEDURE DIVISION.
           CALL 'ASMCROSS' USING A
           IF A = 12 AND RETURN-CODE = 9
             MOVE 0 TO RETURN-CODE
           ELSE MOVE 99 TO RETURN-CODE END-IF
           STOP RUN.
",
    )
    .unwrap();
    let cobol = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
    let image = Image {
        members: vec![
            ("MAINPGM".into(), zeus_cobol::module_bytes(&cobol).unwrap()),
            (
                "ASMCROSS".into(),
                zeus_cobol::hlasm::module_bytes(&caller).unwrap(),
            ),
            (
                "ASMLEAF".into(),
                zeus_cobol::hlasm::module_bytes(&callee).unwrap(),
            ),
        ],
    };
    let exe = executable(dir.path(), "external-relative-asm", &image);
    assert_eq!(run(&exe, &[], "").1, 0);
    assert_eq!(run_env(&exe, &[], "", true).1, 0);
}

#[test]
fn cobol_main_with_pli_subprogram() {
    let dir = tempfile::tempdir().unwrap();
    let module = zeus_cobol::compile_file(
        &programs().join("hello.cbl"),
        &zeus_cobol::front::FrontOptions::default(),
    )
    .unwrap();
    let image = Image {
        members: vec![
            ("HELLO".into(), zeus_cobol::module_bytes(&module).unwrap()),
            pli("psq"),
        ],
    };
    let exe = executable(dir.path(), "hello", &image);
    // run from another directory: the modules come from the executable
    let (out, rc) = run(&exe, &["WORLD"], "typed line\n");
    assert_eq!(rc, 3, "{out}");
    assert_eq!(
        out,
        "PARM=WORLD\nIN PL/I 25\nN=000000025 T=PL/I\nIN=typed line"
    );
}

#[test]
fn native_cobol_reads_assembler_allocated_storage_and_frees_it_later() {
    let dir = tempfile::tempdir().unwrap();
    let mut members = Vec::new();
    for (name, body) in [
        (
            "TAKE",
            "         L 2,0(1)\n         GETMAIN RU,LV=32\n         ST 1,0(2)\n         MVC 0(3,1),=C'ABC'",
        ),
        (
            "FREE",
            "         L 2,0(1)\n         L 3,0(2)\n         FREEMAIN RU,LV=32,A=(3)",
        ),
    ] {
        let source = dir.path().join(format!("{name}.asm"));
        std::fs::write(&source, format!("{name} CSECT\n         USING {name},12\n         LR 12,15\n{body}\n         LGHI 15,0\n         BR 14\n         END\n")).unwrap();
        let module = zeus_cobol::hlasm::compile_file(&source, &[]).unwrap();
        members.push((
            name.into(),
            zeus_cobol::hlasm::module_bytes(&module).unwrap(),
        ));
    }
    let source = dir.path().join("main.cbl");
    std::fs::write(&source, "       IDENTIFICATION DIVISION.\n       PROGRAM-ID. MAINPGM.\n       DATA DIVISION.\n       WORKING-STORAGE SECTION.\n       01 P USAGE POINTER.\n       01 RESULT-CODE PIC 9(4) COMP VALUE 99.\n       LINKAGE SECTION.\n       01 HEAP-DATA PIC X(3).\n       PROCEDURE DIVISION.\n           CALL 'TAKE' USING P\n           SET ADDRESS OF HEAP-DATA TO P\n           IF HEAP-DATA = 'ABC' MOVE 0 TO RESULT-CODE END-IF\n           CALL 'FREE' USING P\n           MOVE RESULT-CODE TO RETURN-CODE\n           STOP RUN.\n").unwrap();
    let module = zeus_cobol::compile_file(&source, &Default::default()).unwrap();
    members.insert(
        0,
        ("MAINPGM".into(), zeus_cobol::module_bytes(&module).unwrap()),
    );
    let exe = executable(dir.path(), "dynamic-asm", &Image { members });
    assert_eq!(run(&exe, &[], "").1, 0);
    assert_eq!(run_env(&exe, &[], "", true).1, 0);
}

#[test]
fn pli_main_with_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let exe = executable(
        dir.path(),
        "rep",
        &Image {
            members: vec![pli("rep")],
        },
    );
    // two fixed 80-byte EBCDIC records
    let data: Vec<u8> = ["FIRST LINE", "SECOND"]
        .iter()
        .flat_map(|s| {
            let mut r = zeus_cobol::ebcdic::encode(s);
            r.resize(80, 0x40);
            r
        })
        .collect();
    let input = dir.path().join("in.dat");
    std::fs::write(&input, data).unwrap();
    let dd = format!("IN={}", input.display());
    let (out, rc) = run(&exe, &["--dd", &dd, "--parm", "/NIGHTLY"], "");
    assert_eq!(rc, 2, "{out}");
    assert_eq!(out, "PARM NIGHTLY\nFIRST LINE\nSECOND\nRECORDS 2");
    // the run time alone has no program
    let (_, rc) = run(Path::new(env!("CARGO_BIN_EXE_zeus-rt")), &[], "");
    assert_eq!(rc, 2);
}

#[test]
fn machine_code_matches_the_interpreter() {
    let dir = tempfile::tempdir().unwrap();
    let module = zeus_cobol::compile_file(
        &programs().join("arith.cbl"),
        &zeus_cobol::front::FrontOptions::default(),
    )
    .unwrap();
    let image = Image {
        members: vec![("ARITH".into(), zeus_cobol::module_bytes(&module).unwrap())],
    };
    let exe = executable(dir.path(), "arith", &image);
    let (native, rc) = run_env(&exe, &[], "", false);
    assert_eq!(rc, 0, "{native}");
    let (interp, _) = run_env(&exe, &[], "", true);
    assert_eq!(native, interp);
    assert_eq!(
        native,
        // P = 0, J = 8, R = -33, N = 4 (negative signs as overpunches); the
        // size errors truncate (H = 004, R = 8765); L leaves 64 bits
        "P=0000000 J=0008 R=003L N=000000004\n\
         U=1234 Q=54321\n\
         H=004 R=8765\n\
         L=999999999999999999\n\
         V=STAR W=START JR= XY\n\
         OK"
    );
}

#[test]
fn display_integers_and_justified_moves_match_the_interpreter() {
    let dir = tempfile::tempdir().unwrap();
    let module =
        zeus_cobol::compile_file(&programs().join("display.cbl"), &Default::default()).unwrap();
    let image = Image {
        members: vec![("ZONED".into(), zeus_cobol::module_bytes(&module).unwrap())],
    };
    let exe = executable(dir.path(), "zoned", &image);
    let native = run_env(&exe, &[], "", false);
    let interpreted = run_env(&exe, &[], "", true);
    assert_eq!(native, interpreted);
    assert_eq!(native.1, 0, "{}", native.0);
    assert!(!native.0.contains("BAD"), "{}", native.0);
    assert!(native.0.contains("[DEF][ ABCDEF]"), "{}", native.0);
    assert!(native.0.contains("[GER][ XY]"), "{}", native.0);
    assert!(native.0.contains("SIZE ERROR"), "{}", native.0);
}

#[test]
fn text_comparisons_constants_and_signs_match_the_interpreter() {
    let module =
        zeus_cobol::compile_file(&programs().join("text.cbl"), &Default::default()).unwrap();
    for order in ["ebcdic", "reverse", "equal"] {
        let mut module = module.clone();
        if order != "ebcdic" {
            let p = &mut module.programs[0];
            let mut weights: Vec<u16> = (0..256).map(|b| 255 - b).collect();
            if order == "equal" {
                weights[0xC2] = weights[0xC1];
            }
            p.collating = Some(weights);
            p.high_value = 0;
            p.low_value = 255;
        }
        let dir = tempfile::tempdir().unwrap();
        let image = Image {
            members: vec![(
                "TEXT-NATIVE".into(),
                zeus_cobol::module_bytes(&module).unwrap(),
            )],
        };
        let exe = executable(dir.path(), "text", &image);
        let native = run_env(&exe, &[], "", false);
        let interpreted = run_env(&exe, &[], "", true);
        assert_eq!(native, interpreted, "{order}");
        assert_eq!(native.1, 0, "{order}: {}", native.0);
        let expected = match order {
            "ebcdic" => "NE\nLT\nLE\nNOT GT\nNOT GE\nPREFIX LT\nDONE",
            "reverse" => "NE\nNOT LT\nNOT LE\nGT\nGE\nPREFIX NOT LT\nDONE",
            _ => "EQ\nNOT LT\nLE\nNOT GT\nGE\nPREFIX LT\nDONE",
        };
        assert_eq!(native.0, expected, "{order}");
    }
}

#[test]
fn tables_and_fixed_reference_modification_match_the_interpreter() {
    let dir = tempfile::tempdir().unwrap();
    let module =
        zeus_cobol::compile_file(&programs().join("table.cbl"), &Default::default()).unwrap();
    let image = Image {
        members: vec![(
            "TABLE-NATIVE".into(),
            zeus_cobol::module_bytes(&module).unwrap(),
        )],
    };
    let exe = executable(dir.path(), "table", &image);
    let native = run_env(&exe, &[], "", false);
    let interpreted = run_env(&exe, &[], "", true);
    assert_eq!(native, interpreted);
    assert_eq!(native.1, 0, "{}", native.0);
    assert_eq!(native.0, "TOTAL=000042\n[BCXYZ ]\n[AABCDF]\nDONE");
}

#[test]
fn dynamic_text_references_match_the_interpreter() {
    let initial =
        zeus_cobol::compile_file(&programs().join("dynamic-text.cbl"), &Default::default())
            .unwrap();
    for reverse in [false, true] {
        let mut module = initial.clone();
        if reverse {
            module.programs[0].collating = Some((0..256).map(|b| 255 - b).collect());
        }
        let dir = tempfile::tempdir().unwrap();
        let image = Image {
            members: vec![(
                "DYNAMIC-TEXT".into(),
                zeus_cobol::module_bytes(&module).unwrap(),
            )],
        };
        let exe = executable(dir.path(), "dynamic-text", &image);
        let native = run_env(&exe, &[], "", false);
        let interpreted = run_env(&exe, &[], "", true);
        assert_eq!(native, interpreted, "reverse={reverse}");
        assert_eq!(native.1, 0, "{}", native.0);
        let rel = if reverse { "NOT LT" } else { "LT" };
        assert_eq!(
            native.0,
            format!("[ BCD]\n[DUVW ]\n{rel}\n[ABCCDFGH]\nDONE")
        );
    }
}

#[test]
fn generated_executable_includes_help_without_running_the_program() {
    let dir = tempfile::tempdir().unwrap();
    let module =
        zeus_cobol::compile_file(&programs().join("arith.cbl"), &Default::default()).unwrap();
    let image = Image {
        members: vec![("ARITH".into(), zeus_cobol::module_bytes(&module).unwrap())],
    };
    let exe = executable(dir.path(), "help-program", &image);
    for flag in ["--help", "-h"] {
        let result = Command::new(&exe)
            .arg(flag)
            .current_dir(dir.path())
            .output()
            .unwrap();
        assert!(result.status.success());
        assert!(result.stderr.is_empty());
        let help = String::from_utf8(result.stdout).unwrap();
        assert!(help.contains("--dd NAME=PATH"));
        assert!(help.contains("EXAMPLES"));
        assert!(help.contains("EXIT STATUS"));
        assert!(
            !help.contains("P=000000"),
            "main program ran while requesting help"
        );
    }
}

#[test]
fn native_and_interpreted_cobol_use_the_same_condition_token_service() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("token.cbl");
    let text = "IDENTIFICATION DIVISION.\nPROGRAM-ID. TOKENTEST.\nDATA DIVISION.\nWORKING-STORAGE SECTION.\n01 IDONE PIC S9(4) COMP VALUE 2.\n01 IDTWO PIC S9(4) COMP VALUE 931.\n01 STYLE PIC S9(4) COMP VALUE 1.\n01 LEVELNUM PIC S9(4) COMP VALUE 2.\n01 OWNERFLAG PIC S9(4) COMP VALUE 0.\n01 OWNERNAME PIC X(3) VALUE 'Z4m'.\n01 INSTANCEVAL PIC S9(9) COMP VALUE -1.\n01 TOKENOUT PIC X(12).\n01 STATUSOUT PIC X(12).\nPROCEDURE DIVISION.\n    CALL 'CEENCOD' USING IDONE IDTWO STYLE LEVELNUM\n        OWNERFLAG OWNERNAME INSTANCEVAL TOKENOUT STATUSOUT\n    IF TOKENOUT = X'000203A350E9F494FFFFFFFF' AND\n       STATUSOUT = LOW-VALUES\n       MOVE 0 TO RETURN-CODE\n    ELSE MOVE 99 TO RETURN-CODE END-IF\n    STOP RUN.\n";
    let text: String = text
        .lines()
        .map(|line| format!("       {line}\n"))
        .collect();
    std::fs::write(&path, text).unwrap();
    let module = zeus_cobol::compile_file(&path, &Default::default()).unwrap();
    let image = Image {
        members: vec![(
            "TOKENTEST".into(),
            zeus_cobol::module_bytes(&module).unwrap(),
        )],
    };
    let exe = executable(dir.path(), "condition-token", &image);
    assert_eq!(run(&exe, &[], "").1, 0);
    assert_eq!(run_env(&exe, &[], "", true).1, 0);
}

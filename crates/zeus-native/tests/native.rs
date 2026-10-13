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
        "PARM=WORLD\nIN PL/I 25\nN=00000002E T=PL/I\nIN=typed line"
    );
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
        // P = 0, J = 8, R = -33, N = 4 (zoned signs as overpunches); the
        // size errors truncate (H = 004, R = 8765); L leaves 64 bits
        "P=000000{ J=000H R=003L N=00000000D\n\
         U=1234 Q=54321\n\
         H=00D R=876E\n\
         L=99999999999999999I\n\
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
    assert_eq!(native.0, "TOTAL=00004B\n[BCXYZ ]\n[AABCDF]\nDONE");
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

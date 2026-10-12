//! Native executables: the run time (`zeus-rt`) with modules appended runs
//! them on its own. A COBOL main program calling a PL/I procedure (PARM,
//! ACCEPT, the return code), and a PL/I main program reading a DD.

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
    image
        .write_executable(Path::new(env!("CARGO_BIN_EXE_zeus-rt")), &exe)
        .unwrap();
    exe
}

fn run(exe: &Path, args: &[&str], stdin: &str) -> (String, i32) {
    let mut child = Command::new(exe)
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

//! PL/I programs run through the compiler and interpreter, compared with
//! their expected SYSPRINT output (tests/programs/*.out) and return code.

use std::cell::RefCell;
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use zeus_ds::format::RecordFormat;
use zeus_pli::vm::{DdInfo, RunEnd, RunEnv};

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

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs")
}

/// Compiles and runs a program; its SYSPRINT, messages and end.
fn run(name: &str, dds: HashMap<String, DdInfo>, stdin: &str) -> (String, String, RunEnd) {
    let src = dir().join(format!("{name}.pli"));
    let (rc, c) = zeus_pli::compile_file(&src, &zeus_pli::Options::default());
    let module = c
        .module
        .unwrap_or_else(|| panic!("{name}: rc {rc}: {:?}", c.messages));
    // through the serialised form, as a load module
    let module = zeus_pli::load_module_bytes(&zeus_pli::module_bytes(&module).unwrap()).unwrap();
    let out = Capture::default();
    let msg = Capture::default();
    let env = RunEnv {
        dds,
        parm: String::new(),
        stdout: Box::new(out.clone()),
        msgout: Box::new(msg.clone()),
        stdin: Some(stdin.lines().map(str::to_string).collect()),
        stdin_reader: None,
    };
    let end = zeus_pli::run(module, env);
    let o = String::from_utf8(out.0.borrow().clone()).unwrap();
    let m = String::from_utf8(msg.0.borrow().clone()).unwrap();
    (o, m, end)
}

fn expected(name: &str) -> String {
    std::fs::read_to_string(dir().join(format!("{name}.out"))).unwrap()
}

#[test]
fn basics() {
    let (out, msg, end) = run("basics", HashMap::new(), "");
    assert_eq!(end, RunEnd::Rc(0), "{msg}");
    assert_eq!(out, expected("basics"));
}

#[test]
fn list_output() {
    let (out, _, end) = run("list", HashMap::new(), "");
    assert_eq!(end, RunEnd::Rc(0));
    assert_eq!(out, expected("list"));
}

#[test]
fn conditions() {
    let (out, msg, end) = run("conditions", HashMap::new(), "");
    assert_eq!(end, RunEnd::Abend("U4038".into()));
    assert_eq!(out, expected("conditions"));
    assert!(
        msg.contains("ONCODE=600 The CONVERSION condition was raised"),
        "{msg}"
    );
    assert!(
        msg.contains("ONCODE=3 The ERROR condition was raised"),
        "{msg}"
    );
}

#[test]
fn files() {
    let tmp = tempfile::tempdir().unwrap();
    let outfile = tmp.path().join("out.dat");
    let mut dds = HashMap::new();
    dds.insert(
        "INFILE".to_string(),
        DdInfo {
            path: dir().join("files.dat"),
            format: Some(RecordFormat::Fixed(80)),
            keys: None,
            sysout: false,
        },
    );
    dds.insert(
        "OUTFILE".to_string(),
        DdInfo {
            path: outfile.clone(),
            format: Some(RecordFormat::Fixed(80)),
            keys: None,
            sysout: false,
        },
    );
    let input = std::fs::read_to_string(dir().join("files.in")).unwrap();
    let (out, msg, end) = run("files", dds, &input);
    assert_eq!(end, RunEnd::Rc(4), "{msg}");
    assert_eq!(out, expected("files"));
    // the records written: REST replaced by 'SEEN' (EBCDIC)
    let data = std::fs::read(&outfile).unwrap();
    assert_eq!(data.len(), 160);
    assert_eq!(&data[12..16], &[0xE2, 0xC5, 0xC5, 0xD5]);
}

#[test]
fn preprocessor() {
    let (out, msg, end) = run("macros", HashMap::new(), "");
    assert_eq!(end, RunEnd::Rc(0), "{msg}");
    assert_eq!(out, expected("macros"));
}

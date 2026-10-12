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
    run_with(name, dds, stdin, None)
}

fn run_with(
    name: &str,
    dds: HashMap<String, DdInfo>,
    stdin: &str,
    exec_host: Option<Rc<dyn zeus_cobol::vm::exec::ExecHost>>,
) -> (String, String, RunEnd) {
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
        linker: None,
        exec_host,
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

#[test]
fn controlled_and_refer() {
    let (out, msg, end) = run("storage", HashMap::new(), "");
    assert_eq!(end, RunEnd::Rc(0), "{msg}");
    assert_eq!(out, expected("storage"));
}

/// EXEC SQL on the Db2 server named by ZEUS_DB2 (skipped without it).
#[test]
fn embedded_sql() {
    let Some(h) = zeus_db2::Db2Host::from_env() else {
        eprintln!("ZEUS_DB2 is not set: EXEC SQL test skipped");
        return;
    };
    let h = Rc::new(h);
    let (out, msg, end) = run_with("sql", HashMap::new(), "", Some(h.clone()));
    let _ = h.end_uow(true);
    assert_eq!(end, RunEnd::Rc(0), "{msg}");
    assert_eq!(out, expected("sql"));
}

/// ENTRY variables: assigned procedures, arrays of them, CALL through
/// them, comparison, and ENTRY parameters.
#[test]
fn entry_variables() {
    let (out, msg, end) = run("entries", HashMap::new(), "");
    assert_eq!(out, expected("entries"), "{msg}");
    assert_eq!(end, RunEnd::Rc(0));
}

/// FETCH and RELEASE of a separately compiled procedure: its STATIC
/// storage lasts until RELEASE; FETCH TITLE names the load module; a
/// module that cannot be loaded raises ERROR.
#[test]
fn fetch_and_release() {
    let lib = tempfile::tempdir().unwrap();
    let (rc, c) = zeus_pli::compile_file(&dir().join("suba.pli"), &zeus_pli::Options::default());
    let sub = c
        .module
        .unwrap_or_else(|| panic!("suba: rc {rc}: {:?}", c.messages));
    std::fs::write(
        lib.path().join("SUBA"),
        zeus_pli::module_bytes(&sub).unwrap(),
    )
    .unwrap();
    let (rc, c) = zeus_pli::compile_file(&dir().join("fetch.pli"), &zeus_pli::Options::default());
    let module = c
        .module
        .unwrap_or_else(|| panic!("fetch: rc {rc}: {:?}", c.messages));
    let out = Capture::default();
    let msg = Capture::default();
    let env_of = {
        let (out, msg) = (out.clone(), msg.clone());
        move || RunEnv {
            dds: HashMap::new(),
            parm: String::new(),
            stdout: Box::new(out.clone()),
            msgout: Box::new(msg.clone()),
            stdin: None,
            stdin_reader: None,
            linker: None,
            exec_host: None,
        }
    };
    let linker =
        zeus_pli::link::ModuleLinker::new(vec![lib.path().to_path_buf()], Box::new(env_of.clone()));
    let mut env = env_of();
    env.linker = Some(linker.clone());
    let end = zeus_pli::run(module, env);
    let o = String::from_utf8(out.0.borrow().clone()).unwrap();
    let m = String::from_utf8(msg.0.borrow().clone()).unwrap();
    let lines: Vec<&str> = o.lines().map(str::trim_end).collect();
    let squeeze = |l: &str| l.split_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        lines.iter().map(|l| squeeze(l)).collect::<Vec<_>>(),
        [
            "SUBA ONE 1",
            "SUBA TWO 2",
            "SUBA AGAIN 1",
            "SUBA TITLE 2",
            "ERROR 9251",
            "END"
        ],
        "{m}"
    );
    assert_eq!(end, RunEnd::Rc(0));
}

/// AREA and OFFSET (ALLOCATE IN, POINTER, OFFSET, EMPTY, area assignment,
/// the AREA condition), iSUB defining, a REFER member in the middle of a
/// structure, and CONTROLLED parameters.
#[test]
fn areas_isub_refer_and_controlled_parameters() {
    for name in ["area", "isub", "refer", "ctl"] {
        let (out, msg, end) = run(name, HashMap::new(), "");
        assert_eq!(end, RunEnd::Rc(0), "{name}: {msg}");
        assert_eq!(out, expected(name), "{name}");
    }
}

/// GENERIC, ORDINAL (with UPTHRU/DOWNTHRU), DEFAULT, floating-point
/// pictures, DEFINE STRUCTURE / ALIAS with HANDLE and NEW, GRAPHIC and
/// WIDECHAR, and COMPLEX.
#[test]
fn types_and_type_definitions() {
    for name in [
        "generic", "default", "floatpic", "handle", "wide", "complex",
    ] {
        let (out, msg, end) = run(name, HashMap::new(), "");
        assert_eq!(end, RunEnd::Rc(0), "{name}: {msg}");
        assert_eq!(out, expected(name), "{name}");
    }
}

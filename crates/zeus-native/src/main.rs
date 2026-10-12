//! `zeus-rt` - the run time of native executables. `zcobc compile
//! --native` and `zpli compile --native` copy this executable (built for
//! the target platform) and append the program's modules
//! ([`zeus_cobol::image`]); started, it runs the first of them.
//!
//! ```text
//! PROGRAM [--dd NAME=PATH]... [--parm TEXT | TEXT] [--ccsid N]
//! ```
//!
//! The main program may be COBOL or PL/I; the other modules (and the
//! `.zo` files in the executable's directory) are its subprograms. Files:
//! `--dd` (see zeus_ds::dd); SYSPRINT and DISPLAY go to standard output,
//! SYSIN and ACCEPT come from standard input. EXEC SQL runs on ZEUS_DB2,
//! the MQI on ZEUS_MQ. The exit status is the return code; 12 after an
//! abend (16 for PL/I), 16 after a run-time error.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use zeus_cobol::image::Image;
use zeus_pli::vm::{DdInfo, RunEnd, RunEnv};

struct Args {
    dds: Vec<(String, PathBuf)>,
    parm: String,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut a = Args {
        dds: Vec::new(),
        parm: String::new(),
    };
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--dd" => {
                let v = it.next().ok_or("--dd needs NAME=PATH")?;
                let (n, p) = v.split_once('=').ok_or("--dd needs NAME=PATH")?;
                a.dds.push((n.to_ascii_uppercase(), PathBuf::from(p)));
            }
            "--parm" => a.parm = it.next().ok_or("--parm needs a value")?.clone(),
            "--ccsid" => {
                let c: u32 = it
                    .next()
                    .and_then(|c| c.parse().ok())
                    .ok_or("--ccsid needs a number")?;
                if !zeus_cobol::codepage::set_active(c) {
                    return Err(format!("CCSID {c} is not supported"));
                }
            }
            s if s.starts_with("--") => return Err(format!("unknown option {s}")),
            s => a.parm = s.to_string(),
        }
    }
    Ok(a)
}

/// The modules written to a directory of their own for the run (the
/// load library of the run unit); removed at the end.
struct Library(PathBuf);

impl Library {
    fn new(image: &Image) -> std::io::Result<Library> {
        let dir = std::env::temp_dir().join(format!("zeus-rt-{}", std::process::id()));
        std::fs::create_dir_all(&dir)?;
        for (name, data) in &image.members {
            std::fs::write(dir.join(format!("{name}.zo")), data)?;
        }
        Ok(Library(dir))
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn main() -> ExitCode {
    let Some(image) = Image::of_current_exe().filter(|i| !i.members.is_empty()) else {
        eprintln!(
            "zeus-rt: this is the Zeus run time without a program; make an executable with `zcobc compile --native` or `zpli compile --native`"
        );
        return ExitCode::from(2);
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args = match parse_args(&args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("zeus-rt: {e}");
            eprintln!("usage: PROGRAM [--dd NAME=PATH]... [--parm TEXT | TEXT] [--ccsid N]");
            return ExitCode::from(2);
        }
    };
    let lib = match Library::new(&image) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("zeus-rt: cannot write the modules: {e}");
            return ExitCode::from(16);
        }
    };
    let mut libs = vec![lib.0.clone()];
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        libs.push(dir);
    }
    let (name, main) = &image.members[0];
    let code = run(name, main, &args, libs);
    drop(lib);
    code
}

fn run(name: &str, main: &[u8], args: &Args, libs: Vec<PathBuf>) -> ExitCode {
    let pli = zeus_pli::is_module(main);
    let explicit_ccsid = std::env::var_os("ZEUS_CCSID").is_some();
    // EXEC SQL on ZEUS_DB2, the MQI on ZEUS_MQ
    let db2 = zeus_db2::Db2Host::from_env().map(Rc::new);
    let host = |d: &Option<Rc<zeus_db2::Db2Host>>| {
        d.clone()
            .map(|h| h as Rc<dyn zeus_cobol::vm::exec::ExecHost>)
    };
    let dds: HashMap<String, DdInfo> = args
        .dds
        .iter()
        .map(|(n, p)| {
            (
                n.clone(),
                DdInfo {
                    path: p.clone(),
                    format: None,
                    keys: None,
                    sysout: true,
                    relative: false,
                },
            )
        })
        .collect();
    // the run unit: the COBOL programs of the library, with the DDs and
    // Db2; PL/I modules run in its storage
    let mut sys = zeus_cobol::vm::Machine::new(libs.clone());
    for (n, p) in &args.dds {
        sys.allocate(
            n,
            zeus_ds::dd::Allocation {
                path: p.clone(),
                format: None,
            },
        );
    }
    sys.set_stdout(Box::new(std::io::stdout()));
    if let Some(h) = &db2 {
        sys.set_exec_host(h.clone());
    }
    zeus_mq::register(&mut sys);
    let (sub_dds, sub_db2) = (dds.clone(), host(&db2));
    let linker = zeus_pli::link::ModuleLinker::new(
        libs,
        Box::new(move || RunEnv {
            dds: sub_dds.clone(),
            parm: String::new(),
            stdout: Box::new(std::io::stdout()),
            msgout: Box::new(std::io::stderr()),
            stdin: Some(Vec::new()),
            stdin_reader: None,
            linker: None,
            exec_host: sub_db2.clone(),
        }),
    );
    sys.set_linker(linker.clone());
    let (ok, code) = if pli {
        let module = match zeus_pli::load_module_bytes(main) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("zeus-rt: {name}: {e}");
                return ExitCode::from(16);
            }
        };
        if !explicit_ccsid {
            zeus_cobol::codepage::set_active(module.ccsid);
        }
        let env = RunEnv {
            dds,
            parm: args.parm.clone(),
            stdout: Box::new(std::io::stdout()),
            msgout: Box::new(std::io::stderr()),
            stdin: None,
            stdin_reader: Some(Box::new(std::io::stdin())),
            linker: Some(linker.clone()),
            exec_host: host(&db2),
        };
        let (end, mut sys) = zeus_pli::run_in(module, env, sys);
        linker.finish();
        sys.finish();
        match end {
            RunEnd::Rc(rc) => (true, rc.clamp(0, 255) as u8),
            RunEnd::Abend(code) => {
                eprintln!("{name}: abnormal end {code}");
                (false, 16)
            }
        }
    } else {
        let module = match zeus_cobol::load_module_bytes(main) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("zeus-rt: {name}: {e}");
                return ExitCode::from(16);
            }
        };
        if !explicit_ccsid {
            zeus_cobol::codepage::set_active(module.ccsid);
        }
        sys.set_step(zeus_cobol::vm::StepInfo {
            parm: args.parm.clone(),
            ..Default::default()
        });
        let r = sys.run_main(module);
        linker.finish();
        sys.finish();
        match r {
            Ok(code) => (true, (code & 0xFF) as u8),
            Err(e) => match e.abend() {
                Some(code) => {
                    eprintln!("{name}: ABEND {code}");
                    (false, 12)
                }
                None => {
                    eprintln!("{name}: run-time error: {e}");
                    (false, 16)
                }
            },
        }
    };
    // the units of work end with the run: committed, or backed out
    zeus_mq::end_run(ok);
    if let Some(h) = &db2 {
        let _ = h.end_uow(ok);
    }
    ExitCode::from(code)
}

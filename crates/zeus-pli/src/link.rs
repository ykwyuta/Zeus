//! Calls between separately loaded modules of a run unit: PL/I modules
//! found in the load library directories. COBOL programs run in the run
//! unit's COBOL machine, whose storage is lent to the PL/I modules.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use zeus_cobol::interlang::{Linker, XEnd, XRef};

use crate::vm::{Machine, RunEnv};

/// Makes the run-time environment of a newly loaded PL/I module.
pub type EnvFactory = Box<dyn Fn() -> RunEnv>;

/// Finds external procedures in the load library: PL/I modules (loaded
/// on first use, each keeping its static storage in the run unit).
pub struct ModuleLinker {
    me: Weak<ModuleLinker>,
    dirs: Vec<PathBuf>,
    pli_env: EnvFactory,
    plis: RefCell<Vec<(PathBuf, Rc<RefCell<Machine>>)>>,
}

impl ModuleLinker {
    pub fn new(dirs: Vec<PathBuf>, pli_env: EnvFactory) -> Rc<ModuleLinker> {
        Rc::new_cyclic(|me| ModuleLinker {
            me: me.clone(),
            dirs,
            pli_env,
            plis: RefCell::new(Vec::new()),
        })
    }

    /// Ends the run unit: closes the files of the loaded modules.
    pub fn finish(&self) {
        for (_, m) in self.plis.borrow().iter() {
            if let Ok(mut m) = m.try_borrow_mut() {
                m.finish();
            }
        }
    }

    /// The PL/I module in file `path` (loaded once).
    fn pli_module(&self, path: &PathBuf) -> Option<Rc<RefCell<Machine>>> {
        if let Some((_, m)) = self.plis.borrow().iter().find(|(p, _)| p == path) {
            return Some(m.clone());
        }
        let data = std::fs::read(path).ok()?;
        if !crate::is_module(&data) {
            return None;
        }
        let module = crate::load_module_bytes(&data).ok()?;
        let mut env = (self.pli_env)();
        env.linker = self.me.upgrade().map(|l| l as Rc<dyn Linker>);
        let m = Rc::new(RefCell::new(Machine::module(module, env)));
        self.plis.borrow_mut().push((path.clone(), m.clone()));
        Some(m)
    }

    /// The module with entry `name`: loaded already, or a module file
    /// named like it.
    fn find(&self, name: &str) -> Option<Rc<RefCell<Machine>>> {
        let loaded: Vec<Rc<RefCell<Machine>>> =
            self.plis.borrow().iter().map(|(_, m)| m.clone()).collect();
        for m in loaded {
            if m.try_borrow().map(|x| x.has_entry(name)).unwrap_or(false) {
                return Some(m);
            }
        }
        for d in &self.dirs {
            for f in [
                name.to_string(),
                format!("{name}.zo"),
                name.to_ascii_lowercase(),
            ] {
                let path = d.join(&f);
                if path.is_file()
                    && let Some(m) = self.pli_module(&path)
                    && m.try_borrow().map(|x| x.has_entry(name)).unwrap_or(true)
                {
                    return Some(m);
                }
            }
        }
        None
    }
}

impl Linker for ModuleLinker {
    fn call(
        &self,
        name: &str,
        sys: zeus_cobol::vm::Machine,
        args: &[XRef],
    ) -> (zeus_cobol::vm::Machine, Option<Result<XEnd, String>>) {
        let name = name.to_ascii_uppercase();
        let Some(m) = self.find(&name) else {
            return (sys, None);
        };
        let Ok(mut m) = m.try_borrow_mut() else {
            return (
                sys,
                Some(Err(format!("recursive call of {name} in an active module"))),
            );
        };
        m.attach(sys);
        let r = m.call_entry(&name, args).transpose();
        (m.detach(), r)
    }
}

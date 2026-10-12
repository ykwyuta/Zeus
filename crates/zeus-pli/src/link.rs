//! Calls between separately loaded modules: PL/I modules found in the
//! load library directories, and COBOL programs.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::{Rc, Weak};

use zeus_cobol::interlang::{Linker, XArg, XEnd};

use crate::vm::{Machine, RunEnv};

/// Makes the run-time environment of a newly loaded PL/I module.
pub type EnvFactory = Box<dyn Fn() -> RunEnv>;
/// Makes the COBOL run time for COBOL programs called from PL/I.
pub type CobolFactory = Box<dyn Fn() -> zeus_cobol::vm::Machine>;

/// Finds external procedures in the load library: PL/I modules (loaded
/// on first use, each kept in its own machine so that its STATIC storage
/// persists) and COBOL programs.
pub struct ModuleLinker {
    me: Weak<ModuleLinker>,
    dirs: Vec<PathBuf>,
    pli_env: EnvFactory,
    cobol: Option<CobolFactory>,
    plis: RefCell<Vec<(PathBuf, Rc<RefCell<Machine>>)>>,
    cobol_machine: RefCell<Option<Rc<RefCell<zeus_cobol::vm::Machine>>>>,
}

impl ModuleLinker {
    pub fn new(dirs: Vec<PathBuf>, pli_env: EnvFactory, cobol: Option<CobolFactory>) -> Rc<ModuleLinker> {
        Rc::new_cyclic(|me| ModuleLinker {
            me: me.clone(),
            dirs,
            pli_env,
            cobol,
            plis: RefCell::new(Vec::new()),
            cobol_machine: RefCell::new(None),
        })
    }

    /// Ends the run unit: closes the files of the loaded modules.
    pub fn finish(&self) {
        for (_, m) in self.plis.borrow().iter() {
            if let Ok(mut m) = m.try_borrow_mut() {
                m.finish();
            }
        }
        if let Some(c) = self.cobol_machine.borrow().as_ref()
            && let Ok(mut c) = c.try_borrow_mut()
        {
            c.finish();
        }
    }

    fn call_pli(m: &Rc<RefCell<Machine>>, name: &str, args: &mut [XArg]) -> Option<Result<XEnd, String>> {
        match m.try_borrow_mut() {
            Ok(mut mm) => mm.call_entry(name, args).transpose(),
            Err(_) => Some(Err(format!("recursive call of {name} in an active module"))),
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
        let m = Rc::new(RefCell::new(Machine::new(module, env)));
        self.plis.borrow_mut().push((path.clone(), m.clone()));
        Some(m)
    }
}

impl Linker for ModuleLinker {
    fn call(&self, name: &str, args: &mut [XArg]) -> Option<Result<XEnd, String>> {
        let name = name.to_ascii_uppercase();
        // entries of the modules already loaded
        let loaded: Vec<Rc<RefCell<Machine>>> = self.plis.borrow().iter().map(|(_, m)| m.clone()).collect();
        for m in &loaded {
            if m.try_borrow().map(|x| x.has_entry(&name)).unwrap_or(false) {
                return Self::call_pli(m, &name, args);
            }
        }
        // a PL/I module named like the entry
        for d in &self.dirs {
            for f in [name.clone(), format!("{name}.zo"), name.to_ascii_lowercase()] {
                let path = d.join(&f);
                if path.is_file()
                    && let Some(m) = self.pli_module(&path)
                    && m.try_borrow().map(|x| x.has_entry(&name)).unwrap_or(true)
                {
                    return Self::call_pli(&m, &name, args);
                }
            }
        }
        // a COBOL program
        let factory = self.cobol.as_ref()?;
        let c = self
            .cobol_machine
            .borrow_mut()
            .get_or_insert_with(|| {
                let mut c = factory();
                if let Some(l) = self.me.upgrade() {
                    c.set_linker(l);
                }
                Rc::new(RefCell::new(c))
            })
            .clone();
        let Ok(mut c) = c.try_borrow_mut() else {
            return Some(Err(format!("{name}: a COBOL program is active (recursive call between languages)")));
        };
        c.call_program(&name, args).transpose()
    }
}

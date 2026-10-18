//! EXEC CICS / SQL / DLI statements.
//!
//! The VM evaluates the operands of an EXEC statement and hands them to
//! the host interface installed with [`Machine::set_exec_host`] (the CICS,
//! Db2 or IMS implementation). The host works on storage through the
//! machine and tells it how to continue: next statement, a procedure
//! (HANDLE CONDITION) or return from the program (RETURN, XCTL).

use std::rc::Rc;

use zeus_decimal::Dec;

use super::{Machine, RResult, RunError};
use crate::zir::{DataType, ExecKind, ExecVal, ProgramCode};

/// A data area operand: address and length in storage, and its type.
#[derive(Debug, Clone)]
pub struct Area {
    pub addr: usize,
    pub len: usize,
    pub ty: DataType,
}

/// An evaluated option argument.
#[derive(Debug, Clone)]
pub enum ExecArg {
    Area(Area),
    /// Alphanumeric literal or value (EBCDIC).
    Alnum(Vec<u8>),
    /// Numeric literal or arithmetic expression.
    Num(Dec),
    /// `ADDRESS OF record`: its current address; `linkage` is the record's
    /// base slot when the host may set it (SET option).
    Pointer {
        addr: usize,
        linkage: Option<u32>,
    },
    /// A procedure (instruction index), for HANDLE CONDITION and the like.
    Label(u32),
    Name(String),
}

/// An EXEC statement with its operands evaluated.
#[derive(Debug, Clone)]
pub struct ExecCall {
    pub kind: ExecKind,
    pub command: String,
    pub line: u32,
    /// Name of the program executing the statement.
    pub program: String,
    /// Instance of the program (for [`Machine::set_linkage`]).
    pub instance: usize,
    pub options: Vec<(String, Option<ExecArg>)>,
    pub sql: Option<String>,
    pub host: Vec<(Area, Option<Area>)>,
    /// Which host variables are VARCHAR structures (halfword length and
    /// text).
    pub host_varchar: Vec<bool>,
}

impl ExecCall {
    /// Whether option `name` is present.
    pub fn has(&self, name: &str) -> bool {
        self.options.iter().any(|(n, _)| n == name)
    }

    /// Argument of option `name` (None when absent or without argument).
    pub fn arg(&self, name: &str) -> Option<&ExecArg> {
        self.options
            .iter()
            .find(|(n, _)| n == name)
            .and_then(|(_, a)| a.as_ref())
    }
}

/// How execution continues after an EXEC statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecOutcome {
    Continue,
    /// Continue at an instruction of the program (a HANDLE label).
    Goto(u32),
    /// Return from the program, as GOBACK.
    Return,
}

/// The CICS / Db2 / IMS implementation running EXEC statements.
pub trait ExecHost {
    fn exec(&self, m: &mut Machine, call: &ExecCall) -> RResult<ExecOutcome>;
}

/// How a program run with [`Machine::run_program`] ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramEnd {
    /// GOBACK / EXIT PROGRAM, or a RETURN of the host interface.
    Returned,
    StopRun(i32),
}

impl Machine {
    /// Allocates `len` bytes of storage set to `init` (GETMAIN).
    pub fn getmain(&mut self, len: usize, init: u8) -> usize {
        self.alloc(&vec![init; len])
    }

    /// Runs the separately compiled program `name` (found in the search
    /// directories) as called by the host, with the addresses of its USING
    /// parameters. Each run starts with the program in its initial state.
    /// Returns None when the program does not exist.
    pub fn run_program(
        &mut self,
        name: &str,
        args: &[Option<usize>],
    ) -> RResult<Option<ProgramEnd>> {
        let name = name.trim().to_ascii_uppercase();
        let Some(i) = self.find_program(&name) else {
            return Ok(None);
        };
        if self.instances[i].active {
            return Err(RunError(format!("program {name} is already active")));
        }
        if self.instances[i].initialized {
            self.reset(i);
        }
        self.instances[i].initialized = true;
        let using = self.instances[i].code.using.clone();
        for (k, slot) in using.iter().enumerate() {
            self.instances[i].linkage[*slot as usize] = args.get(k).copied().flatten();
        }
        self.instances[i].active = true;
        self.instances[i].perform.clear();
        let r = self.run_from(i, true);
        self.instances[i].active = false;
        match r {
            Ok(super::Exit::Returned) => Ok(Some(ProgramEnd::Returned)),
            Ok(super::Exit::StopRun(c)) => Ok(Some(ProgramEnd::StopRun(c))),
            Err(e) if e.0 == super::STOP_RUN => Ok(Some(ProgramEnd::StopRun(self.return_code))),
            Err(e) => {
                // the programs still active ended with the error
                for inst in &mut self.instances {
                    inst.active = false;
                }
                Err(e)
            }
        }
    }

    /// Installs the interface that runs EXEC statements.
    pub fn set_exec_host(&mut self, host: Rc<dyn ExecHost>) {
        self.exec_host = Some(host);
    }

    /// Storage of the run unit.
    pub fn storage(&self) -> &[u8] {
        &self.mem
    }

    pub fn storage_mut(&mut self) -> &mut [u8] {
        &mut self.mem
    }

    /// Sets the base address of a LINKAGE SECTION record of a program
    /// instance (SET(ADDRESS OF record) options).
    pub fn set_linkage(&mut self, instance: usize, slot: u32, addr: usize) -> RResult<()> {
        let inst = self
            .instances
            .get_mut(instance)
            .ok_or_else(|| RunError("no such program instance".into()))?;
        match inst.linkage.get_mut(slot as usize) {
            Some(l) => {
                *l = Some(addr);
                Ok(())
            }
            None => Err(RunError("no such linkage record".into())),
        }
    }

    pub(super) fn exec(&mut self, cur: usize, code: &ProgramCode, n: u32) -> RResult<ExecOutcome> {
        let def = &code.execs[n as usize];
        let mut options = Vec::with_capacity(def.options.len());
        for o in &def.options {
            let arg = match &o.arg {
                None => None,
                Some(v) => Some(self.exec_arg(cur, code, v)?),
            };
            options.push((o.name.clone(), arg));
        }
        let mut host = Vec::with_capacity(def.host.len());
        for (v, i) in &def.host {
            let v = self.exec_area(cur, v)?;
            let i = i.as_ref().map(|i| self.exec_area(cur, i)).transpose()?;
            host.push((v, i));
        }
        let call = ExecCall {
            kind: def.kind,
            command: def.command.clone(),
            line: def.line,
            program: code.name.clone(),
            instance: cur,
            options,
            sql: def.sql.clone(),
            host,
            host_varchar: def.host_varchar.clone(),
        };
        let Some(h) = self.exec_host.clone() else {
            return Err(RunError(format!(
                "{}: EXEC {:?} {} at line {} needs a {} environment",
                code.name,
                def.kind,
                def.command,
                def.line,
                match def.kind {
                    ExecKind::Cics => "CICS",
                    ExecKind::Sql => "Db2",
                    ExecKind::SqlIms => "IMS SQL",
                    ExecKind::Dli => "IMS",
                }
            )));
        };
        h.exec(self, &call)
    }

    fn exec_area(&mut self, cur: usize, loc: &crate::zir::Loc) -> RResult<Area> {
        let (addr, len) = self.addr(cur, loc)?;
        Ok(Area {
            addr,
            len,
            ty: self.loc_type(cur, loc),
        })
    }

    fn exec_arg(&mut self, cur: usize, code: &ProgramCode, v: &ExecVal) -> RResult<ExecArg> {
        Ok(match v {
            ExecVal::Area(loc) => ExecArg::Area(self.exec_area(cur, loc)?),
            ExecVal::Address(loc) => {
                let linkage = match code.descs[loc.desc as usize].base {
                    crate::zir::Base::Linkage(n) if code.descs[loc.desc as usize].offset == 0 => {
                        Some(n)
                    }
                    _ => None,
                };
                let addr = self.addr(cur, loc).map(|(a, _)| a).unwrap_or(0);
                ExecArg::Pointer { addr, linkage }
            }
            ExecVal::Value(src) => {
                let val = self.eval_src(cur, src)?;
                match &val {
                    super::eval::Val::NumLit(..) | super::eval::Val::Num(_) => {
                        ExecArg::Num(self.val_num(&val))
                    }
                    super::eval::Val::Item(DataType::Numeric(_), _) => {
                        ExecArg::Num(self.val_num(&val))
                    }
                    _ => ExecArg::Alnum(self.alnum_bytes(cur, &val, 0)?),
                }
            }
            ExecVal::Label(l) => ExecArg::Label(*l),
            ExecVal::Name(s) => ExecArg::Name(s.clone()),
        })
    }
}

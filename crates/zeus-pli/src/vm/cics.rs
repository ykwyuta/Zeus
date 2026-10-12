//! EXEC CICS: the options are evaluated into areas of the run unit's
//! storage and values, and run by the CICS host of the run unit (the task).
//! A CICS program is run with [`Machine::run_cics`]: `DFHEIPTR` addresses
//! the EXEC interface block and the main procedure's parameter points to
//! the COMMAREA (NULL without one).

use zeus_cobol::vm::exec::{Area, ExecArg, ExecCall, ExecOutcome};
use zeus_cobol::zir::{DataType, ExecKind, NumType, NumUsage, SignPos};
use zeus_decimal::Dec;

use super::*;

impl Machine {
    /// The storage and type of a variable named in an option.
    fn cics_area(&mut self, r: &Ref, act: usize) -> R<Area> {
        let var = &self.module.vars[r.var as usize];
        let scalar = var.ty.is_some() && !r.subs.iter().any(|s| s.is_none());
        if !scalar {
            // a structure or array: its storage as a group
            let (bit, _) = self.element_or_start(r, act)?;
            let len = self.aggregate_bytes(r, act)?.len();
            return Ok(Area {
                addr: (bit / 8) as usize,
                len,
                ty: DataType::Group,
            });
        }
        let loc = self.locate(r, act)?;
        let addr = (loc.bit / 8) as usize;
        let len = loc.ty.storage(true).0 as usize;
        let num = |usage, digits, scale| {
            DataType::Numeric(NumType {
                usage,
                digits,
                scale,
                signed: true,
                sign: SignPos::Trailing,
            })
        };
        let ty = match &loc.ty {
            Ty::Fixed {
                dec: true, p, q, ..
            } => num(NumUsage::Packed, u32::from(*p), i32::from(*q)),
            Ty::Fixed { dec: false, .. } => {
                let digits = match len {
                    1 => 2,
                    2 => 4,
                    4 => 9,
                    _ => 18,
                };
                num(NumUsage::Native, digits, 0)
            }
            Ty::Pointer | Ty::Offset(_) => DataType::Pointer,
            Ty::Char {
                var: Varying::Varying,
                ..
            } => {
                // the current value
                let b = self.read(addr as u32, 2)?;
                let n = u16::from_be_bytes([b[0], b[1]]) as usize;
                return Ok(Area {
                    addr: addr + 2,
                    len: n.min(len.saturating_sub(2)),
                    ty: DataType::Alphanumeric { just: false },
                });
            }
            _ => DataType::Alphanumeric { just: false },
        };
        Ok(Area { addr, len, ty })
    }

    fn cics_arg(&mut self, o: &ExecOperand, act: usize) -> R<ExecArg> {
        Ok(match o {
            ExecOperand::Ref(r) => ExecArg::Area(self.cics_area(r, act)?),
            ExecOperand::Label(l) => ExecArg::Label(*l),
            ExecOperand::Name(n) => ExecArg::Name(n.clone()),
            ExecOperand::Value(e) => match self.eval(e, act)? {
                Value::Char(b) => ExecArg::Alnum(b),
                Value::Ptr(p) => ExecArg::Pointer {
                    addr: p as usize,
                    linkage: None,
                },
                Value::Float(f) => ExecArg::Num(Dec::from_i64(f as i64)),
                v => ExecArg::Num(Dec::from_i64(v.as_i64())),
            },
        })
    }

    pub(super) fn exec_cics(&mut self, x: &ExecStmt, act: usize) -> R<()> {
        let Some(h) = self.exec_host() else {
            return err(format!("EXEC CICS {} needs a CICS region", x.command));
        };
        let mut options = Vec::with_capacity(x.options.len());
        for (name, o) in &x.options {
            let arg = match o {
                Some(o) => Some(self.cics_arg(o, act)?),
                None => None,
            };
            options.push((name.clone(), arg));
        }
        let call = ExecCall {
            kind: ExecKind::Cics,
            command: x.command.clone(),
            line: CUR_LINE.with(|l| l.get()),
            program: self.module.name.clone(),
            instance: self.cics_instance,
            options,
            sql: None,
            host: Vec::new(),
            host_varchar: Vec::new(),
        };
        let r = h.exec(self.sys(), &call);
        match r {
            Ok(ExecOutcome::Continue) => Ok(()),
            Ok(ExecOutcome::Goto(l)) => {
                let target = self.act_of(act, self.module.labels[l as usize].block)?;
                Err(Intr::Goto(l, target))
            }
            Ok(ExecOutcome::Return) => Err(Intr::CicsReturn),
            Err(e) => Err(Intr::Fatal(e.0)),
        }
    }

    /// Runs the main procedure as a CICS program: `eib` is the address of
    /// the EXEC interface block, `commarea` that of the COMMAREA.
    pub fn run_cics(&mut self, eib: usize, commarea: Option<usize>) -> R<()> {
        let m = self.module.clone();
        let eiptr = m.vars.iter().position(|v| v.name == "DFHEIPTR");
        self.run_with_pointers(&[commarea], |me, a| {
            if let Some(v) = eiptr {
                let r = Ref {
                    var: v as VarId,
                    subs: Vec::new(),
                    locator: None,
                };
                let loc = me.locate(&r, a)?;
                me.write((loc.bit / 8) as u32, &(eib as u32).to_be_bytes())?;
            }
            Ok(())
        })
    }

    /// Runs the main procedure with storage addresses for its parameters
    /// (an IMS program's PCBs, a CICS program's COMMAREA): a POINTER
    /// parameter gets the address (NULL for None), another is the storage
    /// at the address. `init` runs in the main procedure's activation
    /// before its body.
    pub fn run_with_pointers(
        &mut self,
        addrs: &[Option<usize>],
        init: impl FnOnce(&mut Machine, usize) -> R<()>,
    ) -> R<()> {
        let m = self.module.clone();
        let Some(main) = m.main.or(if m.procs.is_empty() { None } else { Some(0) }) else {
            return err("the program has no procedure");
        };
        if !self.started {
            self.started = true;
            self.static_init()?;
        }
        let proc = &m.procs[main as usize];
        let mut bindings = Vec::new();
        for (k, &pv) in proc.params.iter().enumerate() {
            let addr = addrs.get(k).copied().flatten();
            let pointer = matches!(m.vars[pv as usize].ty, Some(Ty::Pointer));
            let bit = if pointer || addr.is_none() {
                let cell = self.heap_alloc(4);
                let p = addr.map_or(NULL, |a| a as u32);
                self.write(cell, &p.to_be_bytes())?;
                u64::from(cell) * 8
            } else {
                addr.unwrap_or(0) as u64 * 8
            };
            bindings.push(Some(Binding {
                bit,
                dims: Vec::new(),
                len: None,
            }));
        }
        let saved = (CUR_ENABLED.with(|c| c.get()), CUR_LINE.with(|c| c.get()));
        CUR_ENABLED.with(|c| c.set(cond::DEFAULT));
        let a = self.activate(proc.block, None, bindings, proc.returns.clone())?;
        let r = (|| -> R<()> {
            init(self, a)?;
            let body = m.blocks[proc.block as usize].body.clone();
            match self.run_list(&body, a) {
                Ok(()) | Err(Intr::Return(_)) | Err(Intr::CicsReturn) | Err(Intr::Stop) => Ok(()),
                Err(e) => Err(e),
            }
        })();
        self.deactivate(a);
        CUR_ENABLED.with(|c| c.set(saved.0));
        CUR_LINE.with(|c| c.set(saved.1));
        self.complete_lines();
        let _ = self.env.stdout.flush();
        r
    }

    /// How a CICS program ended: an abend of the task (from the host)
    /// comes back as it came, ERROR as abend APLI, other errors as their
    /// message.
    pub fn cics_end(&mut self, r: R<()>) -> Result<(), String> {
        match r {
            Ok(()) => Ok(()),
            Err(Intr::Fatal(m)) if m.starts_with(ABEND) => Err(m),
            Err(Intr::Fatal(m)) => Err(format!("{m} (line {})", CUR_LINE.with(|l| l.get()))),
            Err(Intr::Terminate) => Err(format!("{ABEND}APLI")),
            Err(e) => Err(format!("unexpected {e:?}")),
        }
    }
}

//! EXEC SQL: the host variables are copied into storage of the COBOL run
//! time, whose Db2 interface runs the statement, and copied back.

use zeus_cobol::vm::exec::{Area, ExecArg, ExecCall};
use zeus_cobol::zir::{DataType, ExecKind, NumType, NumUsage, SignPos};

use super::*;

/// The COBOL data type of a host variable, and whether it is VARCHAR.
fn data_type(ty: &Ty) -> Option<(DataType, bool)> {
    let num = |usage, digits, scale| {
        DataType::Numeric(NumType {
            usage,
            digits,
            scale,
            signed: true,
            sign: SignPos::Trailing,
        })
    };
    Some(match ty {
        Ty::Char {
            var: Varying::Fixed,
            ..
        } => (DataType::Alphanumeric { just: false }, false),
        Ty::Char {
            var: Varying::Varying,
            ..
        } => (DataType::Alphanumeric { just: false }, true),
        Ty::Fixed {
            dec: true, p, q, ..
        } => (num(NumUsage::Packed, u32::from(*p), i32::from(*q)), false),
        Ty::Fixed {
            dec: false, q: 0, ..
        } => {
            let digits = match ty.storage(true).0 {
                2 => 4,
                4 => 9,
                8 => 18,
                _ => return None,
            };
            (num(NumUsage::Native, digits, 0), false)
        }
        _ => return None,
    })
}

impl Machine {
    /// The scalar host variable or area of `r` in the run unit's storage.
    pub(super) fn exec_area(&mut self, r: &Ref, act: usize) -> R<(Area, bool)> {
        let loc = self.locate(r, act)?;
        let name = &self.module.vars[r.var as usize].name;
        let Some((ty, vc)) = data_type(&loc.ty) else {
            return err(format!("{name} cannot be a host variable"));
        };
        if loc.bit % 8 != 0 {
            return err(format!("{name} is not on a byte boundary"));
        }
        Ok((
            Area {
                addr: (loc.bit / 8) as usize,
                len: loc.ty.storage(true).0 as usize,
                ty,
            },
            vc,
        ))
    }

    /// The EXEC interface: the run time's (Db2), else the run unit's.
    pub(super) fn exec_host(&mut self) -> Option<std::rc::Rc<dyn zeus_cobol::vm::exec::ExecHost>> {
        self.env
            .exec_host
            .clone()
            .or_else(|| self.sys().exec_host())
    }

    pub(super) fn exec_stmt(&mut self, x: &ExecStmt, act: usize) -> R<()> {
        if x.kind == "CICS" {
            return self.exec_cics(x, act);
        }
        let Some(h) = self.exec_host() else {
            return err(format!(
                "EXEC {} needs a Db2 environment (ZEUS_DB2)",
                x.kind
            ));
        };
        // the host variables are in the run unit's storage
        let mut host = Vec::new();
        let mut varchar = Vec::new();
        for (v, i) in &x.host {
            let (va, vc) = self.exec_area(v, act)?;
            let ia = match i {
                Some(i) => Some(self.exec_area(i, act)?.0),
                None => None,
            };
            varchar.push(vc);
            host.push((va, ia));
        }
        let mut options = Vec::new();
        if let Some(q) = &x.query {
            options.push(("QUERY".to_string(), Some(ExecArg::Name(q.clone()))));
        }
        let mut ca = None;
        if let Some(r) = &x.sqlca {
            let (bit, _) = self.element_or_start(r, act)?;
            let ar = Area {
                addr: (bit / 8) as usize,
                len: 136,
                ty: DataType::Group,
            };
            options.push(("SQLCA".to_string(), Some(ExecArg::Area(ar.clone()))));
            ca = Some(ar.addr as u32);
        }
        let call = ExecCall {
            kind: ExecKind::Sql,
            command: x
                .text
                .split_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_uppercase(),
            line: CUR_LINE.with(|l| l.get()),
            program: self.module.name.clone(),
            instance: 0,
            options,
            sql: Some(x.text.clone()),
            host,
            host_varchar: varchar,
        };
        h.exec(self.sys(), &call).map_err(|e| Intr::Fatal(e.0))?;
        // WHENEVER
        if let Some(addr) = ca
            && !x.whenever.is_empty()
        {
            let b = self.read(addr, 136)?;
            let code = i32::from_be_bytes([b[12], b[13], b[14], b[15]]);
            let k = if code < 0 {
                0
            } else if code == 100 {
                2
            } else if code > 0 || b[120] == 0xE6 {
                1
            } else {
                return Ok(());
            };
            if let Some((_, e)) = x.whenever.iter().find(|(w, _)| *w == k) {
                return self.exec(&Stmt::GoTo(e.clone()), act);
            }
        }
        Ok(())
    }
}

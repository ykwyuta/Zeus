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
        Ty::Char { var: Varying::Fixed, .. } => (DataType::Alphanumeric { just: false }, false),
        Ty::Char { var: Varying::Varying, .. } => (DataType::Alphanumeric { just: false }, true),
        Ty::Fixed { dec: true, p, q, .. } => (num(NumUsage::Packed, u32::from(*p), i32::from(*q)), false),
        Ty::Fixed { dec: false, q: 0, .. } => {
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
    pub(super) fn exec_stmt(&mut self, x: &ExecStmt, act: usize) -> R<()> {
        let Some(h) = self.env.exec_host.clone() else {
            return err(format!("EXEC {} needs a Db2 environment (ZEUS_DB2)", x.kind));
        };
        let mut shim = zeus_cobol::vm::Machine::new(Vec::new());
        // host variables: (address here, area there)
        let mut places: Vec<(u32, Area)> = Vec::new();
        let mut host = Vec::new();
        let mut varchar = Vec::new();
        let mut area = |me: &mut Machine, shim: &mut zeus_cobol::vm::Machine, r: &Ref| -> R<(Area, bool)> {
            let loc = me.locate(r, act)?;
            let name = &me.module.vars[r.var as usize].name;
            let Some((ty, vc)) = data_type(&loc.ty) else {
                return err(format!("{name} cannot be a host variable"));
            };
            let n = loc.ty.storage(true).0 as usize;
            let addr = (loc.bit / 8) as u32;
            let a = shim.getmain(n, 0);
            shim.storage_mut()[a..a + n].copy_from_slice(&me.read(addr, n)?);
            let ar = Area { addr: a, len: n, ty };
            places.push((addr, ar.clone()));
            Ok((ar, vc))
        };
        for (v, i) in &x.host {
            let (va, vc) = area(self, &mut shim, v)?;
            let ia = match i {
                Some(i) => Some(area(self, &mut shim, i)?.0),
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
            let addr = (bit / 8) as u32;
            let a = shim.getmain(136, 0);
            shim.storage_mut()[a..a + 136].copy_from_slice(&self.read(addr, 136)?);
            let ar = Area {
                addr: a,
                len: 136,
                ty: DataType::Group,
            };
            options.push(("SQLCA".to_string(), Some(ExecArg::Area(ar.clone()))));
            ca = Some((addr, ar));
        }
        let call = ExecCall {
            kind: ExecKind::Sql,
            command: x.text.split_whitespace().next().unwrap_or("").to_ascii_uppercase(),
            line: CUR_LINE.with(|l| l.get()),
            program: self.module.name.clone(),
            instance: 0,
            options,
            sql: Some(x.text.clone()),
            host,
            host_varchar: varchar,
        };
        h.exec(&mut shim, &call).map_err(|e| Intr::Fatal(e.0))?;
        for (addr, a) in places.iter().chain(ca.iter()) {
            let b = shim.storage()[a.addr..a.addr + a.len].to_vec();
            self.write(*addr, &b)?;
        }
        // WHENEVER
        if let Some((addr, _)) = ca
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

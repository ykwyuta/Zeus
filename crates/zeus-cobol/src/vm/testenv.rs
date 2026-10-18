//! The test environment of a run (test generation, `ztestgen` / `ztest`):
//! I/O statements made to fail, and stubs for the programs called that
//! are not there.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::{Machine, RResult};
use crate::zir::OpenMode;

/// The kinds of I/O statements that can be made to fail.
pub const FAULT_KINDS: [&str; 7] = [
    "OPEN", "READ", "WRITE", "REWRITE", "DELETE", "START", "CLOSE",
];

/// I/O statements made to fail: (file, statement kind, occurrence from
/// 1). Only files with FILE STATUS fail: the statement does nothing and
/// the status is 35 (OPEN INPUT / I-O) or 30.
#[derive(Debug, Default)]
pub struct IoFaults {
    pub fail: BTreeSet<(String, String, u32)>,
    pub(super) counts: HashMap<(String, &'static str), u32>,
    /// The last statement that could fail: (file, kind, occurrence, failed).
    pub(super) last: Option<(String, &'static str, u32, bool)>,
}

/// Programs called that are not there run as stubs: the `n`-th call of a
/// program sets its argument `k` (BY REFERENCE, from 1) to the bytes
/// `outputs[(program, n, k)]` (a leading part) and returns with
/// RETURN-CODE 0.
#[derive(Debug, Default)]
pub struct CallStubs {
    pub outputs: BTreeMap<(String, u32, u32), Vec<u8>>,
    /// The stubbed calls: (program, call number, argument lengths).
    pub calls: Vec<(String, u32, Vec<usize>)>,
    pub(super) counts: HashMap<String, u32>,
}

/// The bytes of an argument a stub's output follows (at most).
pub const STUB_BYTES: usize = 256;

impl Machine {
    pub fn set_io_faults(&mut self, fail: BTreeSet<(String, String, u32)>) {
        self.faults = Some(Box::new(IoFaults {
            fail,
            ..Default::default()
        }));
    }

    pub fn set_call_stubs(&mut self, outputs: BTreeMap<(String, u32, u32), Vec<u8>>) {
        self.stubs = Some(Box::new(CallStubs {
            outputs,
            ..Default::default()
        }));
    }

    /// The stubbed calls of the run.
    pub fn stub_calls(&self) -> Vec<(String, u32, Vec<usize>)> {
        self.stubs.as_ref().map_or(Vec::new(), |s| s.calls.clone())
    }

    /// Counts an I/O statement on file `f`; true when it is made to fail
    /// (its status is set).
    pub(super) fn io_fault(
        &mut self,
        cur: usize,
        f: u32,
        kind: &'static str,
        mode: Option<OpenMode>,
    ) -> RResult<bool> {
        let Some(faults) = self.faults.as_mut() else {
            return Ok(false);
        };
        let def = &self.instances[cur].code.files[f as usize];
        if def.status.is_none() {
            faults.last = None;
            return Ok(false);
        }
        let name = def.name.clone();
        let n = faults.counts.entry((name.clone(), kind)).or_insert(0);
        *n += 1;
        let n = *n;
        let fail = faults.fail.contains(&(name.clone(), kind.to_string(), n));
        faults.last = Some((name, kind, n, fail));
        if fail {
            let code = match mode {
                Some(OpenMode::Input | OpenMode::InputOutput) => "35",
                _ => "30",
            };
            let def = def.clone();
            self.fault_status(cur, &def, code)?;
        }
        Ok(fail)
    }

    /// Takes the last statement that could fail (for concolic execution).
    pub(super) fn take_fault_point(&mut self) -> Option<(String, &'static str, u32, bool)> {
        self.faults.as_mut()?.last.take()
    }

    /// A stub for program `name` when stubs are on: sets its arguments.
    pub(super) fn call_stub(
        &mut self,
        cur: usize,
        name: &str,
        args: &[Option<(usize, usize)>],
    ) -> RResult<bool> {
        let Some(stubs) = self.stubs.as_mut() else {
            return Ok(false);
        };
        let n = stubs.counts.entry(name.to_string()).or_insert(0);
        *n += 1;
        let n = *n;
        stubs.calls.push((
            name.to_string(),
            n,
            args.iter().map(|a| a.map_or(0, |a| a.1)).collect(),
        ));
        for (k, a) in args.iter().enumerate() {
            let Some((addr, len)) = *a else { continue };
            if let Some(out) = stubs.outputs.get(&(name.to_string(), n, k as u32 + 1)) {
                let l = out.len().min(len).min(STUB_BYTES);
                self.mem[addr..addr + l].copy_from_slice(&out[..l]);
            }
        }
        self.return_code = 0;
        self.store_rc(cur);
        if self.sym.is_some() {
            self.sym_stub(name, n, args);
        }
        Ok(true)
    }
}

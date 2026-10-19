//! The test environment of a run (test generation, `ztestgen` / `ztest`):
//! I/O statements made to fail, stubs for the programs called that are
//! not there, and the outputs of LE date and time services as inputs.

use std::collections::{BTreeMap, HashMap};

use super::{Machine, RResult};
use crate::zir::OpenMode;

/// The kinds of I/O statements that can be made to fail.
pub const FAULT_KINDS: [&str; 7] = [
    "OPEN", "READ", "WRITE", "REWRITE", "DELETE", "START", "CLOSE",
];

/// I/O statements made to fail: (file, statement kind, occurrence from
/// one) -> how. Only files with FILE STATUS fail: the statement does
/// nothing and the status is 35 (OPEN INPUT / I-O) or 30 with `FAULT`, or
/// '9' and a binary byte as VSAM logical errors give it with `FAULT_VSAM`.
#[derive(Debug, Default)]
pub struct IoFaults {
    pub fail: BTreeMap<(String, String, u32), u8>,
    pub(super) counts: HashMap<(String, &'static str), u32>,
    /// The last statement that could fail: (file, kind, occurrence, how
    /// it failed or 0).
    pub(super) last: Option<(String, &'static str, u32, u8)>,
}

/// A statement made to fail with status 35 or 30.
pub const FAULT: u8 = 1;
/// A statement made to fail with status '9' and X'10' (not numeric).
pub const FAULT_VSAM: u8 = 2;
/// The status of `FAULT_VSAM` (EBCDIC).
pub const VSAM_STATUS: [u8; 2] = [0xF9, 0x10];

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

/// The output arguments (index, length) of the LE services whose results
/// are inputs in test runs: the service runs, then the `n`-th call's
/// argument `k` is set from the stub outputs `(service, n, k)` when there
/// are some, and follows variables like a stub's.
fn le_outputs(name: &str) -> &'static [(usize, usize)] {
    match name {
        "CEEDAYS" => &[(2, 4), (3, 12)],
        "CEESECS" => &[(2, 8), (3, 12)],
        "CEEDATE" | "CEEDATM" => &[(2, 80), (3, 12)],
        "CEELOCT" => &[(0, 4), (1, 8), (2, 17), (3, 12)],
        "CEEGMT" => &[(0, 4), (1, 8), (2, 12)],
        _ => &[],
    }
}

impl Machine {
    pub fn set_io_faults(&mut self, fail: BTreeMap<(String, String, u32), u8>) {
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
        let how = faults
            .fail
            .get(&(name.clone(), kind.to_string(), n))
            .copied()
            .unwrap_or(0);
        faults.last = Some((name, kind, n, how));
        if how == FAULT_VSAM {
            let def = def.clone();
            self.fault_status(cur, &def, "90")?;
            if let Some(loc) = &def.status {
                let (a, l) = self.addr(cur, loc)?;
                if l >= 2 {
                    self.mem[a + 1] = VSAM_STATUS[1];
                }
            }
        } else if how != 0 {
            let code = match mode {
                Some(OpenMode::Input | OpenMode::InputOutput) => "35",
                _ => "30",
            };
            let def = def.clone();
            self.fault_status(cur, &def, code)?;
        }
        Ok(how != 0)
    }

    /// Takes the last statement that could fail (for concolic execution).
    pub(super) fn take_fault_point(&mut self) -> Option<(String, &'static str, u32, u8)> {
        self.faults.as_mut()?.last.take()
    }

    /// After LE service `name` ran in a test run: its outputs from the stub
    /// outputs, if any, and following variables.
    pub(super) fn le_test_outputs(&mut self, name: &str, args: &[Option<usize>]) {
        let outs = le_outputs(name);
        let Some(stubs) = self.stubs.as_mut() else {
            return;
        };
        if outs.is_empty() {
            return;
        }
        let n = stubs.counts.entry(name.to_string()).or_insert(0);
        *n += 1;
        let n = *n;
        let mut sized = vec![None; args.len()];
        for &(k, len) in outs {
            let Some(Some(addr)) = args.get(k).copied() else {
                continue;
            };
            if addr + len > self.mem.len() {
                continue;
            }
            if let Some(out) = stubs.outputs.get(&(name.to_string(), n, k as u32 + 1)) {
                let l = out.len().min(len);
                self.mem[addr..addr + l].copy_from_slice(&out[..l]);
            }
            sized[k] = Some((addr, len));
        }
        if self.sym.is_some() {
            self.sym_stub(name, n, &sized);
        }
    }

    /// A DL/I call (CBLTDLI, AIBTDLI...: function, PCB, I/O area, SSAs)
    /// when stubs are on: the status code of the PCB (bytes 11-12) and the
    /// I/O area of a get call are what the `n`-th call of the function
    /// `F` sets, as a stub's arguments 2 and 3 of program `CBLTDLI.F`. The
    /// status is spaces, from the second call of a GN / GNP on GB / GE.
    fn dli_call_stub(&mut self, name: &str, args: &[Option<(usize, usize)>]) -> bool {
        let Some(Some((fa, fl))) = args.first().copied() else {
            return false;
        };
        let Some(Some((pcb, pl))) = args.get(1).copied() else {
            return false;
        };
        if pl < 12 {
            return false;
        }
        let func = crate::ebcdic::decode(&self.mem[fa..fa + fl.min(4)])
            .trim()
            .to_string();
        let Some(stubs) = self.stubs.as_mut() else {
            return false;
        };
        let prog = format!("{name}.{func}");
        let n = stubs.counts.entry(prog.clone()).or_insert(0);
        *n += 1;
        let n = *n;
        stubs.calls.push((
            prog.clone(),
            n,
            args.iter().map(|a| a.map_or(0, |a| a.1)).collect(),
        ));
        let status = match stubs.outputs.get(&(prog.clone(), n, 2)) {
            Some(out) if out.len() >= 12 => [out[10], out[11]],
            _ => {
                let s = match func.as_str() {
                    "GN" | "GHN" if n > 1 => "GB",
                    "GNP" | "GHNP" if n > 1 => "GE",
                    _ => "  ",
                };
                let b = crate::ebcdic::encode(s);
                [b[0], b[1]]
            }
        };
        self.mem[pcb + 10..pcb + 12].copy_from_slice(&status);
        let get = func.starts_with('G');
        let io = args.get(2).copied().flatten().filter(|_| get);
        if let Some((addr, len)) = io
            && let Some(out) = stubs.outputs.get(&(prog.clone(), n, 3))
        {
            let l = out.len().min(len).min(STUB_BYTES);
            self.mem[addr..addr + l].copy_from_slice(&out[..l]);
        }
        if self.sym.is_some() {
            // the status bytes as the leading bytes 11-12 of argument 2
            let mut outs = vec![None, None, io];
            outs.truncate(args.len());
            self.sym_stub(&prog, n, &outs);
            if !self.sym.as_ref().is_some_and(|st| st.full()) {
                self.sym_dli_status(&prog, n, pcb);
            }
        }
        true
    }

    /// A stub for program `name` when stubs are on: sets its arguments.
    pub(super) fn call_stub(
        &mut self,
        cur: usize,
        name: &str,
        args: &[Option<(usize, usize)>],
    ) -> RResult<bool> {
        if matches!(name, "CBLTDLI" | "AIBTDLI" | "CEETDLI" | "AERTDLI")
            && self.dli_call_stub(name, args)
        {
            self.return_code = 0;
            self.store_rc(cur);
            return Ok(true);
        }
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

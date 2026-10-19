//! Test case files (JSON) and their replay.
//!
//! ```json
//! { "program": "PGM",
//!   "params": [ { "hex": "C1C2...", "fields": { "NAME": "value", ... } } ],
//!   "accept": [ "line", ... ],
//!   "files": [ { "name": "IN-FILE", "records": [ { "hex": "...", "fields": { ... } } ] } ],
//!   "faults": [ { "file": "IN-FILE", "statement": "OPEN", "occurrence": 1 },
//!               { "file": "IN-FILE", "statement": "READ", "occurrence": 2, "status": "F910" } ],
//!   "stubs": [ { "program": "SUBPGM", "call": 1, "argument": 1, "hex": "..." } ],
//!   "exec": { "PGM.120.1.RESP": { "num": 13 }, "PGM.120.1.INTO.ACCT-ID": { "hex": "F0F1" } },
//!   "expect": { "return_code": 0, "display": [ "..." ], "params": [ "C1C2..." ],
//!               "error": null, "files": [ { "name": "OUT-FILE", "records": [ "..." ] } ],
//!               "exec": [ "CICS READ FILE('ACCTDAT') RIDFLD=F0F1" ] },
//!   "covers": [ { "program": "PGM", "line": 42, "pc": 17, "jumped": true } ] }
//! ```
//!
//! The parameters are their storage in hexadecimal (EBCDIC); `fields`
//! shows the input items' values for reading. `files` are the records of
//! the files the program reads; `expect.files` those of the files it
//! writes. `faults` are I/O statements made to fail (the n-th OPEN, READ,
//! ... of a file: status 35 or 30, or with `status` F910, '9' and a binary
//! byte as VSAM logical errors give it); `stubs` what the stubs of the
//! programs called that are not there set in their arguments (leading
//! bytes); `expect.stubbed` names those programs. `exec` is what EXEC
//! CICS / SQL statements give the program (the n-th execution of the
//! statement at a line: RESP, INTO items, SQLCODE, host variables);
//! `expect.exec` the statements executed with what they sent out. The
//! expected values are
//! what the program did when the case was made (a regression test).

use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use zeus_cobol::vm::testenv;
use zeus_cobol::zir::Module;

use crate::engine::{
    Case, Field, FileSpec, Inputs, Options, default_inputs, default_record, field_text, file_specs,
    run,
};
use crate::stubhost::Out;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Param {
    /// The storage (may be left out in seeds: then `fields` give it).
    #[serde(default)]
    pub hex: String,
    #[serde(default)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Expect {
    pub return_code: i32,
    pub display: Vec<String>,
    pub params: Vec<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub files: Vec<OutFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stubbed: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exec: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Fault {
    pub file: String,
    pub statement: String,
    pub occurrence: u32,
    /// The status when not 35 / 30 (hexadecimal).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Stub {
    pub program: String,
    pub call: u32,
    pub argument: u32,
    #[serde(default)]
    pub hex: String,
    /// The bytes as text (EBCDIC) instead of `hex`, in seeds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// The records of an input file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InFile {
    pub name: String,
    pub records: Vec<Param>,
}

/// The records written to an output file (hexadecimal).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutFile {
    pub name: String,
    pub records: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cover {
    pub program: String,
    pub line: Option<u32>,
    pub pc: u32,
    pub jumped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CaseFile {
    #[serde(default)]
    pub program: String,
    #[serde(default)]
    pub params: Vec<Param>,
    #[serde(default)]
    pub accept: Vec<String>,
    #[serde(default)]
    pub files: Vec<InFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faults: Vec<Fault>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stubs: Vec<Stub>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub exec: BTreeMap<String, Out>,
    /// The time of day the program saw (UTC, `YYYY-MM-DD HH:MM:SS`; left
    /// out: 2025-01-01 09:00:00).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<String>,
    /// Left out in seeds.
    #[serde(default)]
    pub expect: Expect,
    #[serde(default)]
    pub covers: Vec<Cover>,
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

pub fn unhex(s: &str) -> Result<Vec<u8>, String> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) {
        return Err("odd number of hex digits".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

/// The source line of an instruction.
pub fn line_of(module: &Module, program: &str, pc: u32) -> Option<u32> {
    let p = module.programs.iter().find(|p| p.name == program)?;
    let s = *p.instr_stmt.get(pc as usize)?;
    p.stmt_pos.get(s as usize).map(|&(_, l)| l)
}

fn out_files(files: &[(String, Vec<Vec<u8>>)]) -> Vec<OutFile> {
    files
        .iter()
        .map(|(n, r)| OutFile {
            name: n.clone(),
            records: r.iter().map(|b| hex(b)).collect(),
        })
        .collect()
}

fn lines(s: &str) -> Vec<String> {
    s.lines().map(|l| l.trim_end().to_string()).collect()
}

/// The file of a generated case.
pub fn case_file(
    module: &Module,
    program: &str,
    fields: &[Field],
    specs: &[FileSpec],
    c: &Case,
) -> CaseFile {
    let params = c
        .inputs
        .params
        .iter()
        .enumerate()
        .map(|(k, b)| {
            let mut m = serde_json::Map::new();
            for f in fields.iter().filter(|f| f.param == k) {
                m.insert(f.name.clone(), serde_json::Value::String(field_text(f, b)));
            }
            Param {
                hex: hex(b),
                fields: m,
            }
        })
        .collect();
    let files = specs
        .iter()
        .filter(|f| f.input)
        .zip(&c.inputs.files)
        .map(|(f, recs)| InFile {
            name: f.name.clone(),
            records: recs
                .iter()
                .map(|r| {
                    let mut m = serde_json::Map::new();
                    for rf in &f.fields {
                        let fd = Field {
                            param: 0,
                            name: rf.name.clone(),
                            offset: rf.offset,
                            len: rf.len,
                            num: rf.num,
                        };
                        m.insert(
                            rf.name.clone(),
                            serde_json::Value::String(field_text(&fd, r)),
                        );
                    }
                    Param {
                        hex: hex(r),
                        fields: m,
                    }
                })
                .collect(),
        })
        .collect();
    CaseFile {
        program: program.to_string(),
        params,
        accept: c.inputs.accept.clone(),
        files,
        faults: c
            .inputs
            .faults
            .iter()
            .map(|((f, k, n), how)| Fault {
                file: f.clone(),
                statement: k.clone(),
                occurrence: *n,
                status: (*how == testenv::FAULT_VSAM).then(|| hex(&testenv::VSAM_STATUS)),
            })
            .collect(),
        stubs: c
            .inputs
            .stubs
            .iter()
            .map(|((p, n, k), b)| Stub {
                program: p.clone(),
                call: *n,
                argument: *k,
                hex: hex(b),
                text: None,
            })
            .collect(),
        exec: c.inputs.exec.clone(),
        clock: c.inputs.clock.map(clock_text),
        expect: Expect {
            return_code: c.out.return_code,
            display: lines(&c.out.display),
            params: c.out.params.iter().map(|b| hex(b)).collect(),
            error: c.out.error.clone(),
            files: out_files(&c.out.files),
            stubbed: c.out.stubbed.iter().cloned().collect(),
            exec: c.out.exec.clone(),
        },
        covers: c
            .covers
            .iter()
            .map(|(p, pc, j)| Cover {
                program: p.clone(),
                line: line_of(module, p, *pc),
                pc: *pc,
                jumped: *j,
            })
            .collect(),
    }
}

/// Seconds since 1970-01-01 UTC as `YYYY-MM-DD HH:MM:SS`.
pub fn clock_text(t: u64) -> String {
    let days = (t / 86_400) as i64;
    let s = t % 86_400;
    // the civil date of a day number (H. Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        s / 3600,
        s % 3600 / 60,
        s % 60
    )
}

/// `YYYY-MM-DD HH:MM:SS` (UTC) as seconds since 1970-01-01.
pub fn clock_value(text: &str) -> Result<u64, String> {
    let bad = || format!("bad clock {text} (YYYY-MM-DD HH:MM:SS)");
    let (date, time) = text
        .trim()
        .split_once(' ')
        .unwrap_or((text.trim(), "00:00:00"));
    let n: Vec<i64> = date
        .split('-')
        .chain(time.split(':'))
        .map(|x| x.parse().map_err(|_| bad()))
        .collect::<Result<_, _>>()?;
    let [y, m, d, hh, mm, ss] = n[..] else {
        return Err(bad());
    };
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hh * 3600 + mm * 60 + ss).map_err(|_| bad())
}

/// Sets an item from its value in a case file (`fields`): a number at the
/// item's scale, else text (EBCDIC, padded with spaces).
fn set_field(f: &Field, bytes: &mut [u8], value: &serde_json::Value) -> Result<(), String> {
    let text = match value {
        serde_json::Value::String(s) => s.clone(),
        v => v.to_string(),
    };
    let Some(b) = bytes.get_mut(f.offset..f.offset + f.len) else {
        return Err(format!("{}: outside the data", f.name));
    };
    match &f.num {
        Some(t) => {
            let t2 = text.trim();
            let (neg, digits) = match t2.strip_prefix('-') {
                Some(d) => (true, d),
                None => (false, t2.strip_prefix('+').unwrap_or(t2)),
            };
            let (int, frac) = digits.split_once('.').unwrap_or((digits, ""));
            let scale = t.scale.max(0) as usize;
            let frac: String = frac
                .chars()
                .chain(std::iter::repeat('0'))
                .take(scale)
                .collect();
            let all = format!("{int}{frac}");
            let mut x: i128 = if all.is_empty() {
                0
            } else {
                all.parse()
                    .map_err(|_| format!("{}: not a number: {text}", f.name))?
            };
            if neg {
                x = -x;
            }
            zeus_cobol::vm::data::store_raw(t, b, x);
        }
        None => {
            let e = zeus_cobol::ebcdic::encode(&text);
            for (i, x) in b.iter_mut().enumerate() {
                *x = e.get(i).copied().unwrap_or(0x40);
            }
        }
    }
    Ok(())
}

/// The bytes of a parameter or record from a case file: its storage
/// (`hex`) if given, else `base`, then each item of `fields` set.
fn seed_bytes(p: &Param, base: Vec<u8>, items: &[Field]) -> Result<Vec<u8>, String> {
    let mut b = base;
    if !p.hex.is_empty() {
        let h = unhex(&p.hex)?;
        let n = b.len().min(h.len());
        b[..n].copy_from_slice(&h[..n]);
    }
    for (name, value) in &p.fields {
        match items.iter().find(|f| f.name.eq_ignore_ascii_case(name)) {
            Some(f) => set_field(f, &mut b, value)?,
            None => return Err(format!("no input item {name}")),
        }
    }
    Ok(b)
}

/// The inputs of a case file used as a seed (an initial input): items not
/// given are zero (numbers) or spaces, the expected results are ignored.
pub fn seed_inputs(
    case: &CaseFile,
    fields: &[Field],
    lens: &[usize],
    specs: &[FileSpec],
) -> Result<Inputs, String> {
    let mut inputs = default_inputs(fields, lens);
    for (k, p) in case.params.iter().enumerate().take(lens.len()) {
        let items: Vec<Field> = fields
            .iter()
            .filter(|f| f.param == k)
            .map(|f| Field {
                param: 0,
                ..f.clone()
            })
            .collect();
        inputs.params[k] = seed_bytes(p, inputs.params[k].clone(), &items)?;
    }
    inputs.accept = case.accept.clone();
    for f in specs.iter().filter(|f| f.input) {
        let items: Vec<Field> = f
            .fields
            .iter()
            .map(|rf| Field {
                param: 0,
                name: rf.name.clone(),
                offset: rf.offset,
                len: rf.len,
                num: rf.num,
            })
            .collect();
        let recs = match case
            .files
            .iter()
            .find(|x| x.name.eq_ignore_ascii_case(&f.name))
        {
            Some(x) => x
                .records
                .iter()
                .map(|r| seed_bytes(r, default_record(f), &items))
                .collect::<Result<Vec<_>, _>>()?,
            None => vec![default_record(f)],
        };
        inputs.files.push(recs);
    }
    inputs.faults = case
        .faults
        .iter()
        .map(|f| {
            let how = if f.status.is_some() {
                testenv::FAULT_VSAM
            } else {
                testenv::FAULT
            };
            ((f.file.clone(), f.statement.clone(), f.occurrence), how)
        })
        .collect();
    inputs.stubs = case
        .stubs
        .iter()
        .map(|s| {
            let b = match &s.text {
                Some(t) => zeus_cobol::ebcdic::encode(t),
                None => unhex(&s.hex)?,
            };
            Ok(((s.program.clone(), s.call, s.argument), b))
        })
        .collect::<Result<_, String>>()?;
    inputs.exec = case.exec.clone();
    inputs.clock = case.clock.as_deref().map(clock_value).transpose()?;
    Ok(inputs)
}

/// Runs a case again (without concolic execution): the differences from
/// its expected results.
pub fn replay(module: &Rc<Module>, case: &CaseFile, opts: &Options) -> Result<Vec<String>, String> {
    let inputs = Inputs {
        params: case
            .params
            .iter()
            .map(|p| unhex(&p.hex))
            .collect::<Result<_, _>>()?,
        accept: case.accept.clone(),
        files: file_specs(module, &opts.search)
            .iter()
            .filter(|f| f.input)
            .map(|f| {
                case.files
                    .iter()
                    .find(|x| x.name == f.name)
                    .map_or(Ok(Vec::new()), |x| {
                        x.records.iter().map(|r| unhex(&r.hex)).collect()
                    })
            })
            .collect::<Result<_, _>>()?,
        faults: case
            .faults
            .iter()
            .map(|f| {
                let how = if f.status.is_some() {
                    testenv::FAULT_VSAM
                } else {
                    testenv::FAULT
                };
                ((f.file.clone(), f.statement.clone(), f.occurrence), how)
            })
            .collect(),
        stubs: case
            .stubs
            .iter()
            .map(|s| Ok(((s.program.clone(), s.call, s.argument), unhex(&s.hex)?)))
            .collect::<Result<_, String>>()?,
        exec: case.exec.clone(),
        clock: case.clock.as_deref().map(clock_value).transpose()?,
    };
    // the programs that ran as stubs run as stubs again
    let mut opts = opts.clone();
    opts.stub.extend(case.expect.stubbed.iter().cloned());
    let out = run(module, &case.program, &[], &inputs, &opts, false);
    let mut diffs = Vec::new();
    let e = &case.expect;
    if out.return_code != e.return_code {
        diffs.push(format!(
            "return code {} instead of {}",
            out.return_code, e.return_code
        ));
    }
    if out.error != e.error {
        diffs.push(format!("error {:?} instead of {:?}", out.error, e.error));
    }
    let disp = lines(&out.display);
    if disp != e.display {
        diffs.push(format!("display {disp:?} instead of {:?}", e.display));
    }
    for (k, (got, want)) in out.params.iter().map(|b| hex(b)).zip(&e.params).enumerate() {
        if &got != want {
            diffs.push(format!("parameter {} {got} instead of {want}", k + 1));
        }
    }
    if out.exec != e.exec {
        diffs.push(format!("EXEC {:?} instead of {:?}", out.exec, e.exec));
    }
    let got = out_files(&out.files);
    for w in &e.files {
        match got.iter().find(|g| g.name == w.name) {
            Some(g) if g.records == w.records => {}
            Some(g) => diffs.push(format!(
                "file {}: records {:?} instead of {:?}",
                w.name, g.records, w.records
            )),
            None => diffs.push(format!("file {} not written", w.name)),
        }
    }
    Ok(diffs)
}

/// Reads the case files of a directory (`*.json`), in name order.
pub fn read_dir(dir: &Path) -> Result<Vec<(String, CaseFile)>, String> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let data = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let c: CaseFile =
                serde_json::from_slice(&data).map_err(|e| format!("{}: {e}", p.display()))?;
            Ok((p.file_name().unwrap().to_string_lossy().into_owned(), c))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_round_trip() {
        for (t, s) in [
            (1_735_722_000, "2025-01-01 09:00:00"),
            (1_709_208_000, "2024-02-29 12:00:00"),
            (946_684_799, "1999-12-31 23:59:59"),
        ] {
            assert_eq!(clock_text(t), s);
            assert_eq!(clock_value(s), Ok(t));
        }
        assert!(clock_value("2025-13").is_err());
    }
}

//! Test case files (JSON) and their replay.
//!
//! ```json
//! { "program": "PGM",
//!   "params": [ { "hex": "C1C2...", "fields": { "NAME": "value", ... } } ],
//!   "accept": [ "line", ... ],
//!   "files": [ { "name": "IN-FILE", "records": [ { "hex": "...", "fields": { ... } } ] } ],
//!   "faults": [ { "file": "IN-FILE", "statement": "OPEN", "occurrence": 1 } ],
//!   "stubs": [ { "program": "SUBPGM", "call": 1, "argument": 1, "hex": "..." } ],
//!   "expect": { "return_code": 0, "display": [ "..." ], "params": [ "C1C2..." ],
//!               "error": null, "files": [ { "name": "OUT-FILE", "records": [ "..." ] } ] },
//!   "covers": [ { "program": "PGM", "line": 42, "pc": 17, "jumped": true } ] }
//! ```
//!
//! The parameters are their storage in hexadecimal (EBCDIC); `fields`
//! shows the input items' values for reading. `files` are the records of
//! the files the program reads; `expect.files` those of the files it
//! writes. `faults` are I/O statements made to fail (the n-th OPEN, READ,
//! ... of a file: status 35 or 30); `stubs` what the stubs of the
//! programs called that are not there set in their arguments (leading
//! bytes); `expect.stubbed` names those programs. The expected values are
//! what the program did when the case was made (a regression test).

use std::path::Path;
use std::rc::Rc;

use serde::{Deserialize, Serialize};
use zeus_cobol::zir::Module;

use crate::engine::{Case, Field, FileSpec, Inputs, Options, field_text, file_specs, run};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Param {
    pub hex: String,
    #[serde(default)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Expect {
    pub return_code: i32,
    pub display: Vec<String>,
    pub params: Vec<String>,
    pub error: Option<String>,
    #[serde(default)]
    pub files: Vec<OutFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stubbed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Fault {
    pub file: String,
    pub statement: String,
    pub occurrence: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Stub {
    pub program: String,
    pub call: u32,
    pub argument: u32,
    pub hex: String,
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
    pub program: String,
    pub params: Vec<Param>,
    #[serde(default)]
    pub accept: Vec<String>,
    #[serde(default)]
    pub files: Vec<InFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub faults: Vec<Fault>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stubs: Vec<Stub>,
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
            .map(|(f, k, n)| Fault {
                file: f.clone(),
                statement: k.clone(),
                occurrence: *n,
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
            })
            .collect(),
        expect: Expect {
            return_code: c.out.return_code,
            display: lines(&c.out.display),
            params: c.out.params.iter().map(|b| hex(b)).collect(),
            error: c.out.error.clone(),
            files: out_files(&c.out.files),
            stubbed: c.out.stubbed.iter().cloned().collect(),
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
            .map(|f| (f.file.clone(), f.statement.clone(), f.occurrence))
            .collect(),
        stubs: case
            .stubs
            .iter()
            .map(|s| Ok(((s.program.clone(), s.call, s.argument), unhex(&s.hex)?)))
            .collect::<Result<_, String>>()?,
    };
    let out = run(module, &case.program, &[], &inputs, opts, false);
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

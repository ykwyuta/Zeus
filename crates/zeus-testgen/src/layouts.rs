//! The layouts of the data in a program's test cases: the copybooks
//! zeus-layout writes (`zlayout`, one per layout, with `layouts.csv`) and
//! `data.csv`, which part of a case file each one describes.
//!
//! ```text
//! data,direction,record,layouts
//! params[1],input/output,DFHCOMMAREA,PGM.COMMAREA.IN.cpy
//! files[IN-FILE],input,IN-REC,PGM.FILE.INFILE.cpy
//! expect.files[OUT-FILE],output,OUT-REC,PGM.FILE.OUTFILE.cpy
//! exec[PGM.43.*.INTO],input,WS-ACCT,PGM.FILE.ACCTDAT.cpy
//! stubs[SUBPGM.1],input/output,WS-PARM,PGM.CALL.SUBPGM.1.cpy
//! ```
//!
//! `data` names the case data as in the case files: a parameter by its
//! index in `params`, a file by name, the `exec` keys of a statement (any
//! execution `*`; the items of a group area follow after a further `.`),
//! what a statement sends (`expect.exec`), the arguments of a stub
//! (program and argument number), the ACCEPTed lines. `direction` says
//! whether the case gives it to the program (input), checks it after the
//! run (output), or both. `layouts` are the copybooks of its record
//! (several: one per REDEFINES alternative, or the work areas a file is
//! read into). A group record no interface of zeus-layout has (a
//! terminal's input, a small COMMAREA) gets a layout of kind RECORD.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use zeus_cobol::zir::{Base, CallArgMode, ExecKind, ExecVal, Instr, Module, ProgramCode, Src};
use zeus_layout::Layout;

use crate::engine::FileSpec;
use crate::stubhost::{receives, sql_outputs};

/// A part of the case files and the layouts of its record.
#[derive(Debug, Clone, PartialEq)]
pub struct DataRow {
    pub data: String,
    pub direction: &'static str,
    /// The record (level-01 item) holding it.
    pub record: String,
    /// Copybook file names (in the layouts directory).
    pub layouts: Vec<String>,
}

/// How the layouts of a part of the cases are found.
#[derive(Debug, Clone)]
enum Find {
    /// Layouts of these kinds (any: empty) whose record is the part's.
    Record(&'static [&'static str]),
    /// The layouts of a file (FILE, CARD) by DD name.
    File(String),
    /// The query result of the SQL statement at a line, else the record.
    Sql(u32, String),
    /// The layouts of a CALL argument (resource `PROGRAM.N`), else an MQ
    /// buffer.
    Call(String),
    /// The cards ACCEPTed.
    Cards,
}

#[derive(Debug, Clone)]
struct Part {
    data: String,
    direction: &'static str,
    record: String,
    find: Find,
}

/// The level-01 item containing a descriptor: the largest one over it
/// with the same storage (the first of the largest: a group before its
/// items).
fn record_of(code: &ProgramCode, desc: u32) -> Option<String> {
    let d = code.descs.get(desc as usize)?;
    let end = d.offset + d.len;
    largest(
        code.descs
            .iter()
            .filter(|x| x.base == d.base && x.offset <= d.offset && x.offset + x.len >= end),
    )
}

fn largest<'a>(it: impl Iterator<Item = &'a zeus_cobol::zir::Desc>) -> Option<String> {
    let mut best: Option<&zeus_cobol::zir::Desc> = None;
    for d in it {
        if best.is_none_or(|b| d.len > b.len) {
            best = Some(d);
        }
    }
    best.map(|d| d.name.clone())
}

/// The record of a USING parameter (its storage).
fn param_record(code: &ProgramCode, slot: u32) -> Option<String> {
    largest(
        code.descs
            .iter()
            .filter(|d| d.base == Base::Linkage(slot) && d.offset == 0),
    )
}

/// The item passed as an argument (by reference or content).
fn arg_desc(a: &CallArgMode) -> Option<u32> {
    match a {
        CallArgMode::Reference(l) | CallArgMode::Content(Src::Loc(l), _) => Some(l.desc),
        _ => None,
    }
}

/// The parts of the program's cases that have a record.
fn parts(module: &Module, program: &str, files: &[FileSpec]) -> Vec<Part> {
    let mut out: Vec<Part> = Vec::new();
    let Some(code) = module.programs.iter().find(|p| p.name == program) else {
        return out;
    };
    let mut push = |data: String, direction, record: String, find| {
        if !out.iter().any(|p| p.data == data) {
            out.push(Part {
                data,
                direction,
                record,
                find,
            });
        }
    };
    for (k, &slot) in code.using.iter().enumerate() {
        // the EIB: CICS's own block
        if let Some(rec) = param_record(code, slot).filter(|r| r != "DFHEIBLK") {
            push(
                format!("params[{k}]"),
                "input/output",
                rec,
                Find::Record(&["PARM", "COMMAREA", "RECORD"]),
            );
        }
    }
    for f in files {
        let direction = match (f.input, f.output) {
            (true, true) => "input/output",
            (true, false) => "input",
            _ => "output",
        };
        if f.input {
            push(
                format!("files[{}]", f.name),
                direction,
                String::new(),
                Find::File(f.assign.clone()),
            );
        }
        if f.output {
            push(
                format!("expect.files[{}]", f.name),
                direction,
                String::new(),
                Find::File(f.assign.clone()),
            );
        }
    }
    for e in &code.execs {
        let key = |opt: &str| format!("exec[{program}.{}.*.{opt}]", e.line);
        match e.kind {
            ExecKind::Cics => {
                for o in &e.options {
                    let Some(ExecVal::Area(loc)) = &o.arg else {
                        continue;
                    };
                    let Some(rec) = record_of(code, loc.desc).filter(|r| r != "DFHEIBLK") else {
                        continue;
                    };
                    let first = e.options.first().map_or("", |o| o.name.as_str());
                    if receives(&e.command, first, &o.name) {
                        push(key(&o.name), "input", rec, Find::Record(&[]));
                    } else {
                        push(
                            format!("expect.exec[{} CICS {} {}]", e.line, e.command, o.name),
                            "output",
                            rec,
                            Find::Record(&[]),
                        );
                    }
                }
            }
            ExecKind::Sql | ExecKind::SqlIms => {
                let outs = sql_outputs(e.sql.as_deref().unwrap_or(""), e.host.len());
                for (k, (loc, _)) in e.host.iter().enumerate() {
                    let Some(rec) = record_of(code, loc.desc) else {
                        continue;
                    };
                    let item = code
                        .descs
                        .get(loc.desc as usize)
                        .map_or(String::new(), |d| d.name.clone());
                    if outs.get(k).copied().unwrap_or(false) {
                        push(
                            key(&format!("H{}", k + 1)),
                            "input",
                            rec,
                            Find::Sql(e.line, item),
                        );
                    } else {
                        push(
                            format!("expect.exec[{} SQL {} H{}]", e.line, e.command, k + 1),
                            "output",
                            rec,
                            Find::Record(&[]),
                        );
                    }
                }
            }
            ExecKind::Dli => {}
        }
    }
    // the arguments of the programs called (stubs when not there)
    let mut seen = BTreeSet::new();
    for i in &code.code {
        let Instr::Call {
            program: Src::Alnum(b),
            args,
            ..
        } = i
        else {
            continue;
        };
        let name = zeus_cobol::ebcdic::decode(b).trim().to_ascii_uppercase();
        for (k, a) in args.iter().enumerate() {
            let Some(desc) = arg_desc(a) else { continue };
            if !seen.insert((name.clone(), k)) {
                continue;
            }
            let Some(rec) = record_of(code, desc) else {
                continue;
            };
            push(
                format!("stubs[{name}.{}]", k + 1),
                "input/output",
                rec,
                Find::Call(format!("{name}.{}", k + 1)),
            );
        }
    }
    push("accept".into(), "input", String::new(), Find::Cards);
    out
}

/// The group records of the program's cases (for
/// `zeus_layout::Options::records`: an elementary item's value is in the
/// case itself).
pub fn records(module: &Module, program: &str, files: &[FileSpec]) -> Vec<String> {
    let Some(code) = module.programs.iter().find(|p| p.name == program) else {
        return Vec::new();
    };
    let group = |name: &str| {
        code.descs
            .iter()
            .any(|d| d.name == name && matches!(d.ty, zeus_cobol::zir::DataType::Group))
    };
    let mut v: Vec<String> = parts(module, program, files)
        .into_iter()
        .map(|p| p.record)
        .filter(|r| !r.is_empty() && group(r))
        .collect();
    v.sort();
    v.dedup();
    v
}

/// The layouts of the program's data (`zlayout` with the same COPY and
/// BMS directories, and RECORD layouts for `records`), checked against
/// the program's positions.
pub fn analyze(
    source: &Path,
    copy_dirs: &[PathBuf],
    bms_dirs: &[PathBuf],
    source_encoding: zeus_cobol::front::SourceEncoding,
    records: Vec<String>,
) -> Result<Vec<Layout>, String> {
    let mut opts = zeus_layout::Options {
        copy_dirs: copy_dirs.to_vec(),
        bms_dirs: bms_dirs.to_vec(),
        records,
        source_encoding,
        ..Default::default()
    };
    let _maps = zeus_layout::symbolic_maps(&mut opts).map_err(|e| e.to_string())?;
    zeus_layout::analyze_file(source, &opts)
}

/// The items of a copybook: (level, name), in order.
fn entries(text: &str) -> Vec<(u32, String)> {
    text.lines()
        .filter(|l| l.as_bytes().get(6).is_some_and(|c| *c != b'*'))
        .filter_map(|l| {
            let body = l.get(7..l.len().min(72))?;
            let mut w = body.split_whitespace();
            let level = w.next()?.parse().ok()?;
            let name = w.next()?.trim_end_matches('.').to_string();
            Some((level, name))
        })
        .collect()
}

/// A test on a layout and its items.
type Pick<'a> = dyn Fn(&Layout, &[(u32, String)]) -> bool + 'a;

/// Which part of the cases each layout describes.
pub fn data_map(
    module: &Module,
    program: &str,
    files: &[FileSpec],
    layouts: &[Layout],
) -> Vec<DataRow> {
    let mine: Vec<(&Layout, Vec<(u32, String)>)> = layouts
        .iter()
        .filter(|l| l.program == program)
        .map(|l| (l, entries(&l.text)))
        .collect();
    let record_of = |l: &Layout| -> String {
        mine.iter()
            .find(|(x, _)| std::ptr::eq(*x, l))
            .and_then(|(_, e)| e.first().map(|(_, n)| n.clone()))
            .unwrap_or_default()
    };
    let select = |f: &Pick| -> Vec<&Layout> {
        mine.iter()
            .filter(|(l, e)| f(l, e))
            .map(|(l, _)| *l)
            .collect()
    };
    let by_record = |rec: &str, kinds: &[&str]| {
        select(&|l, e| {
            (kinds.is_empty() || kinds.contains(&l.kind))
                && e.first().is_some_and(|(_, n)| n == rec)
        })
    };
    let mut rows = Vec::new();
    for p in parts(module, program, files) {
        let found: Vec<&Layout> = match &p.find {
            Find::Record(kinds) => by_record(&p.record, kinds),
            Find::File(dd) => {
                select(&|l, _| matches!(l.kind, "FILE" | "CARD") && l.resource == *dd)
            }
            Find::Sql(line, item) => {
                let at = format!("(LINE {line})");
                let r = select(&|l, e| {
                    l.kind == "SQLRESULT"
                        && l.text.lines().any(|t| t.contains(&at))
                        && e.iter().any(|(_, n)| n == item)
                });
                if r.is_empty() {
                    by_record(&p.record, &[])
                } else {
                    r
                }
            }
            Find::Call(res) => {
                let r = select(&|l, _| l.kind == "CALL" && l.resource == *res);
                if r.is_empty() {
                    by_record(&p.record, &["MQ"])
                } else {
                    r
                }
            }
            Find::Cards => select(&|l, _| l.kind == "CARD"),
        };
        if found.is_empty() {
            continue;
        }
        let record = if p.record.is_empty() {
            record_of(found[0])
        } else {
            p.record.clone()
        };
        rows.push(DataRow {
            data: p.data,
            direction: p.direction,
            record,
            layouts: found.iter().map(|l| l.file_name.clone()).collect(),
        });
    }
    rows
}

fn csv(fields: &[&str]) -> String {
    fields
        .iter()
        .map(|f| {
            if f.contains([',', '"', '\n']) {
                format!("\"{}\"", f.replace('"', "\"\""))
            } else {
                f.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Writes the copybooks and `layouts.csv` (zeus-layout) and `data.csv`
/// to `dir`.
pub fn write(layouts: &[Layout], rows: &[DataRow], dir: &Path) -> std::io::Result<()> {
    zeus_layout::write(layouts, dir)?;
    let mut text = String::from("data,direction,record,layouts\r\n");
    for r in rows {
        text.push_str(&csv(&[
            &r.data,
            r.direction,
            &r.record,
            &r.layouts.join(" "),
        ]));
        text.push_str("\r\n");
    }
    std::fs::write(dir.join("data.csv"), text)
}

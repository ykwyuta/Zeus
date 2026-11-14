//! JCL DD allocation parser for ZCVS `.JCL` sidecars and `export-jcl` generator
//! for z/OS (`IGYCRCTL` + `IEWL` + GO) execution.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::manifest::{ExpectKind, Manifest, ProgramConfig};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DdKind {
    /// Virtual tape (`UNIT=3480`/`3490`/`3590`/`3420`/`TAPE`), mapped to `<dd>.aws`.
    Tape,
    /// Disk dataset (`DSN=...`), mapped to `<dd>.dat`.
    Dataset,
    /// A z/OS UNIX file (`PATH='...'`), mapped to `<dd>.txt`; exported with
    /// the same path.
    Path(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DdAllocation {
    pub ddname: String,
    pub kind: DdKind,
}

/// The JCL statements without `//`, continuation lines joined.
fn logical_lines(jcl_text: &str) -> Vec<String> {
    let mut logical_lines: Vec<String> = Vec::new();
    for raw in jcl_text.lines() {
        let code = if raw.len() > 72 { &raw[..72] } else { raw };
        let trimmed = code.trim_end();
        if !trimmed.starts_with("//") || trimmed.starts_with("//*") {
            continue;
        }
        let body = &trimmed[2..];
        // a line without a name continues the statement before it when
        // that ends with a comma; otherwise it is a statement without a
        // name (kept with its leading blank)
        match logical_lines.last_mut() {
            Some(prev) if body.starts_with(' ') && prev.ends_with(',') => {
                prev.push_str(body.trim_start());
            }
            _ => logical_lines.push(body.to_string()),
        }
    }
    logical_lines
}

/// The exported JCL symbols (z/OS MVS JCL Reference, EXPORT and SET): the
/// values given by SET statements to the names in an EXPORT SYMLIST
/// (`SYMLIST=*` exports every symbol). A value may be quoted ('' is a
/// quote); an empty value is a symbol set to the null string. Only what the
/// suites use: SET symbols, not those of PROC or EXEC.
pub fn exported_symbols(jcl_text: &str) -> Vec<(String, String)> {
    let mut values: Vec<(String, String)> = Vec::new();
    let mut exported: Vec<String> = Vec::new();
    let mut all = false;
    for stmt in logical_lines(jcl_text) {
        // [name] operation operands: a statement without a name starts
        // with a blank
        let rest = if stmt.starts_with(' ') {
            stmt.as_str()
        } else {
            stmt.split_once(' ').map_or("", |(_, r)| r)
        };
        let rest = rest.trim_start();
        let (op, operands) = rest.split_once(' ').unwrap_or((rest, ""));
        let operands = operands.trim();
        match op.to_ascii_uppercase().as_str() {
            "SET" => {
                let mut rest = operands;
                while let Some((name, after)) = rest.split_once('=') {
                    let name = name.trim().to_ascii_uppercase();
                    let (value, tail) = if let Some(q) = after.strip_prefix('\'') {
                        let mut v = String::new();
                        let mut it = q.char_indices().peekable();
                        let mut end = q.len();
                        while let Some((i, c)) = it.next() {
                            if c == '\'' {
                                if it.peek().is_some_and(|(_, d)| *d == '\'') {
                                    v.push('\'');
                                    it.next();
                                    continue;
                                }
                                end = i + 1;
                                break;
                            }
                            v.push(c);
                        }
                        (v, &q[end..])
                    } else {
                        let end = after.find([',', ' ']).unwrap_or(after.len());
                        (after[..end].to_string(), &after[end..])
                    };
                    values.retain(|(n, _)| *n != name);
                    values.push((name, value));
                    rest = match tail.strip_prefix(',') {
                        Some(t) => t,
                        None => break,
                    };
                }
            }
            "EXPORT" => {
                let u = operands.to_ascii_uppercase();
                if let Some(list) = u.strip_prefix("SYMLIST=") {
                    let list = list.split_whitespace().next().unwrap_or("");
                    let list = list.trim_start_matches('(').trim_end_matches(')');
                    for n in list.split(',').filter(|n| !n.is_empty()) {
                        if n == "*" {
                            all = true;
                        } else {
                            exported.push(n.to_string());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    values.retain(|(n, _)| all || exported.contains(n));
    values
}

/// The job of a JCL sidecar as GETSYSWORD sees it: JOBNAME, ACTINFO (the
/// accounting information without its parentheses and apostrophes),
/// JOBCLASS, MSGCLASS, and STEPNAME (the step that runs `program`).
pub fn job_words(jcl_text: &str, program: &str) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for stmt in logical_lines(jcl_text) {
        if stmt.starts_with(' ') {
            continue;
        }
        let Some((name, rest)) = stmt.split_once(' ') else {
            continue;
        };
        let rest = rest.trim_start();
        let (op, operands) = rest.split_once(' ').unwrap_or((rest, ""));
        let operands = operands.trim();
        match op.to_ascii_uppercase().as_str() {
            "JOB" => {
                out.push(("NAME", name.to_ascii_uppercase()));
                // the accounting information: the first positional operand
                let acct = if let Some(r) = operands.strip_prefix('(') {
                    let mut depth = 1;
                    let mut quoted = false;
                    let mut end = r.len();
                    for (i, c) in r.char_indices() {
                        match c {
                            '\'' => quoted = !quoted,
                            '(' if !quoted => depth += 1,
                            ')' if !quoted => {
                                depth -= 1;
                                if depth == 0 {
                                    end = i;
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                    r[..end].to_string()
                } else {
                    operands.split(',').next().unwrap_or("").to_string()
                };
                if !acct.contains('=') {
                    out.push(("ACTINFO", acct.replace('\'', "")));
                }
                for (key, var) in [("CLASS=", "CLASS"), ("MSGCLASS=", "MSGCLASS")] {
                    let found = operands.split(',').find_map(|p| {
                        p.trim().strip_prefix(key).map(str::to_string)
                    });
                    if let Some(v) = found {
                        out.push((var, v));
                    }
                }
            }
            "EXEC" => {
                let pgm = operands
                    .split(',')
                    .find_map(|p| p.trim().strip_prefix("PGM="))
                    .unwrap_or("");
                if pgm.eq_ignore_ascii_case(program) {
                    out.push(("STEPNAME", name.to_ascii_uppercase()));
                }
            }
            _ => {}
        }
    }
    out
}

/// Parses `//<DDNAME> DD ...` statements from a JCL sidecar file, joining
/// continuation lines and classifying tape vs disk allocations.
pub fn parse_dd_allocations(jcl_text: &str) -> Vec<DdAllocation> {
    let logical_lines = logical_lines(jcl_text);

    let mut out = Vec::new();
    for stmt in logical_lines {
        let mut parts = stmt.split_whitespace();
        let Some(ddname) = parts.next() else {
            continue;
        };
        let Some(op) = parts.next() else {
            continue;
        };
        if !op.eq_ignore_ascii_case("DD") {
            continue;
        }
        if matches!(
            ddname.to_ascii_uppercase().as_str(),
            "STEPLIB" | "JOBLIB" | "SYSPRINT" | "SYSOUT" | "CEEDUMP" | "SYSUDUMP" | "REPORT"
        ) {
            continue;
        }
        let rest: String = parts.collect::<Vec<_>>().join(" ").to_ascii_uppercase();
        if rest.contains("SYSOUT=") || rest.contains("DUMMY") {
            continue;
        }
        let is_tape = [
            "UNIT=3480",
            "UNIT=3490",
            "UNIT=3590",
            "UNIT=3420",
            "UNIT=TAPE",
        ]
        .iter()
        .any(|u| rest.contains(u));
        let path = rest
            .split_once("PATH='")
            .and_then(|(_, r)| r.split_once('\''))
            .map(|(p, _)| p.to_string());
        let kind = if let Some(p) = path {
            // the original spelling (the parsed text is upper-cased)
            let original = stmt
                .split_once("PATH='")
                .and_then(|(_, r)| r.split_once('\''))
                .map_or(p, |(p, _)| p.to_string());
            DdKind::Path(original)
        } else if is_tape {
            DdKind::Tape
        } else {
            DdKind::Dataset
        };
        if !out.iter().any(|d: &DdAllocation| d.ddname == ddname) {
            out.push(DdAllocation {
                ddname: ddname.to_ascii_uppercase(),
                kind,
            });
        }
    }
    out
}

#[derive(Debug, Default, Clone)]
pub struct ExportSummary {
    pub copy_members: usize,
    pub source_members: usize,
    pub jcl_members: usize,
}

/// Exports a suite (`zcvs/`) into a z/OS-ready directory layout (`COPY/`,
/// `SRCLIB/`, `JCL/`) using `hlq` as the dataset high-level qualifier.
pub fn export_jcl(
    suite: &Path,
    out: &Path,
    hlq: &str,
    selected_modules: &[String],
) -> Result<ExportSummary> {
    let manifest = Manifest::load(suite)?.unwrap_or_default();
    let modules: Vec<String> = if selected_modules.is_empty() {
        let all = manifest.all_modules();
        if all.is_empty() {
            crate::runner::ALL_MODULES
                .iter()
                .filter(|m| suite.join(m).is_dir())
                .map(|m| (*m).to_string())
                .collect()
        } else {
            all.into_iter().filter(|m| suite.join(m).is_dir()).collect()
        }
    } else {
        selected_modules.to_vec()
    };

    let copy_dir = out.join("COPY");
    let srclib_dir = out.join("SRCLIB");
    let jcl_dir = out.join("JCL");
    fs::create_dir_all(&copy_dir)?;
    fs::create_dir_all(&srclib_dir)?;
    fs::create_dir_all(&jcl_dir)?;

    let mut summary = ExportSummary::default();
    let source_ext = match manifest.language.as_str() {
        "pli" => "PLI",
        "hlasm" => "ASM",
        _ => "CBL",
    };
    let all_name = if manifest.name.is_empty() {
        "ZCVSALL".to_string()
    } else {
        format!("{}ALL", manifest.name)
    };

    // 1. Export COPY members from `copy/`.
    let suite_copy = suite.join("copy");
    if suite_copy.is_dir() {
        for entry in sorted_entries(&suite_copy, &["CBL", "CPY", "PLI", "mac"])? {
            let member = stem_upper(&entry)?;
            let content = format_fixed80(&fs::read_to_string(&entry)?)?;
            let ext = entry.extension().unwrap().to_string_lossy();
            fs::write(copy_dir.join(format!("{member}.{ext}")), content)?;
            summary.copy_members += 1;
        }
    }

    // 2. Export SRCLIB members and per-module JCL jobs.
    let mut all_jcl = render_job_card(&all_name, "LANGUAGE VALIDATION SUITE");
    let mut common_steps = String::new();
    let common_lib = suite.join("lib");
    if common_lib.is_dir() {
        for sub in sorted_entries(&common_lib, &["CBL", "PLI", "ASM"])? {
            let member = stem_upper(&sub)?;
            let ext = sub.extension().unwrap().to_string_lossy();
            fs::write(
                srclib_dir.join(format!("{member}.{ext}")),
                format_fixed80(&fs::read_to_string(&sub)?)?,
            )?;
            summary.source_members += 1;
            common_steps.push_str(&render_other_steps(
                hlq,
                &member,
                &ext,
                &ProgramConfig::default(),
                true,
                &[],
            ));
        }
        all_jcl.push_str(&common_steps);
    }
    for module in &modules {
        let mod_dir = suite.join(module);
        if !mod_dir.is_dir() {
            bail!("module {module} not found in {}", suite.display());
        }
        let job_name = format!("{module}JOB");
        let mut mod_jcl = render_job_card(&job_name, &format!("ZCVS MODULE {module}"));
        mod_jcl.push_str(&common_steps);

        // Subprograms in `lib/` first.
        let lib_dir = mod_dir.join("lib");
        if lib_dir.is_dir() {
            for sub in sorted_entries(&lib_dir, &["CBL", "PLI", "ASM"])? {
                let member = stem_upper(&sub)?;
                let content = format_fixed80(&fs::read_to_string(&sub)?)?;
                let ext = sub.extension().unwrap().to_string_lossy();
                fs::write(srclib_dir.join(format!("{member}.{ext}")), content)?;
                summary.source_members += 1;

                let step = if ext == "CBL" {
                    render_subprogram_steps(hlq, &member)
                } else {
                    render_other_steps(
                        hlq,
                        &member,
                        &ext,
                        &ProgramConfig {
                            expect: ExpectKind::CompileOnly,
                            ..Default::default()
                        },
                        true,
                        &[],
                    )
                };
                mod_jcl.push_str(&step);
                all_jcl.push_str(&step);
            }
        }

        // Main programs in module directory.
        let extensions: &[&str] = if source_ext == "CBL" {
            &["CBL", "SUB"]
        } else {
            std::slice::from_ref(&source_ext)
        };
        for src in sorted_entries(&mod_dir, extensions)? {
            let member = stem_upper(&src)?;
            let content = format_fixed80(&fs::read_to_string(&src)?)?;
            fs::write(srclib_dir.join(format!("{member}.{source_ext}")), content)?;
            summary.source_members += 1;

            let cfg = manifest.programs.get(&member).cloned().unwrap_or_default();
            let (extra_dds, symbols) = if let Some(jcl_file) = &cfg.jcl {
                let jcl_path = mod_dir.join(jcl_file);
                let text = fs::read_to_string(&jcl_path)
                    .with_context(|| format!("reading {}", jcl_path.display()))?;
                (parse_dd_allocations(&text), exported_symbols(&text))
            } else {
                (Vec::new(), Vec::new())
            };
            let mut step = if source_ext == "CBL" {
                render_program_steps(hlq, &member, &cfg, &extra_dds)
            } else {
                render_other_steps(hlq, &member, source_ext, &cfg, false, &extra_dds)
            };
            // exported JCL symbols: EXPORT and SET before the run step
            let go = format!("//G{member:<7} EXEC");
            if !symbols.is_empty()
                && let Some(at) = step.find(&go)
            {
                step.insert_str(at, &render_symbols(&member, &symbols));
            }
            mod_jcl.push_str(&step);
            all_jcl.push_str(&step);
        }

        fs::write(jcl_dir.join(format!("{job_name}.JCL")), &mod_jcl)?;
        summary.jcl_members += 1;
    }

    fs::write(jcl_dir.join(format!("{all_name}.JCL")), &all_jcl)?;
    summary.jcl_members += 1;
    Ok(summary)
}

/// EXPORT SYMLIST and SET statements for the exported JCL symbols of a
/// program; a value that is empty or not a plain word is quoted.
fn render_symbols(member: &str, symbols: &[(String, String)]) -> String {
    let names: Vec<&str> = symbols.iter().map(|(n, _)| n.as_str()).collect();
    let mut s = format!("//X{member:<7} EXPORT SYMLIST=({})\n", names.join(","));
    // one SET statement (names are at most 8 characters), one symbol per
    // line, continued after a comma
    for (k, (n, v)) in symbols.iter().enumerate() {
        let plain = v.chars().all(|c| c.is_ascii_alphanumeric() || "@#$.".contains(c));
        let value = if plain {
            v.clone()
        } else {
            format!("'{}'", v.replace('\'', "''"))
        };
        let head = if k == 0 {
            format!("//S{member:<7} SET ")
        } else {
            "//             ".to_string()
        };
        let comma = if k + 1 < symbols.len() { "," } else { "" };
        let _ = writeln!(s, "{head}{n}={value}{comma}");
    }
    s
}

fn render_other_steps(
    hlq: &str,
    member: &str,
    ext: &str,
    cfg: &ProgramConfig,
    helper: bool,
    extra_dds: &[DdAllocation],
) -> String {
    let (compiler, options, library) = if ext == "PLI" {
        ("IBMZPLI", "OBJECT,MARGINS(2,72)", "IBMZPLI.SIBMZCMP")
    } else {
        ("ASMA90", "OBJECT", "ASM.SASMMOD1")
    };
    let mut s =
        format!("//* PROGRAM {member}\n//C{member:<7} EXEC PGM={compiler},PARM='{options}'\n");
    s.push_str(&format!(
        "//STEPLIB  DD DSN={library},DISP=SHR\n//SYSLIB   DD DSN={hlq}.COPY,DISP=SHR\n"
    ));
    if ext == "ASM" {
        s.push_str("//         DD DSN=SYS1.MACLIB,DISP=SHR\n");
    }
    s.push_str(&format!("//SYSIN    DD DSN={hlq}.SRCLIB({member}),DISP=SHR\n//SYSPRINT DD SYSOUT=*\n//SYSUT1   DD UNIT=SYSDA,SPACE=(CYL,(1,1))\n"));
    if cfg.expect == ExpectKind::CompileError {
        s.push_str(&format!("//SYSLIN   DD DUMMY\n//V{member:<7} EXEC PGM=IDCAMS,COND=(8,LE,C{member})\n//SYSPRINT DD SYSOUT=*\n//SYSIN    DD *\n SET MAXCC = 16\n/*\n"));
        return s;
    }
    s.push_str(&format!("//SYSLIN   DD DSN=&&OBJ{member},DISP=(MOD,PASS),UNIT=SYSDA,\n//            SPACE=(TRK,(5,5))\n"));
    if cfg.expect == ExpectKind::CompileOnly && !helper {
        return s;
    }
    s.push_str(&format!("//L{member:<7} EXEC PGM=IEWL,PARM='LIST,XREF',COND=(4,LT,C{member})\n//SYSLIB   DD DSN=CEE.SCEELKED,DISP=SHR\n//         DD DSN={hlq}.LOAD,DISP=SHR\n//SYSLIN   DD DSN=&&OBJ{member},DISP=(OLD,DELETE)\n//SYSLMOD  DD DSN={hlq}.LOAD({member}),DISP=SHR\n//SYSPRINT DD SYSOUT=*\n//SYSUT1   DD UNIT=SYSDA,SPACE=(CYL,(1,1))\n"));
    if helper {
        return s;
    }
    let parm = cfg
        .parm
        .as_ref()
        .map(|p| format!(",PARM='{p}'"))
        .unwrap_or_default();
    s.push_str(&format!("//G{member:<7} EXEC PGM={member}{parm},COND=(4,LT,L{member})\n//STEPLIB  DD DSN={hlq}.LOAD,DISP=SHR\n//         DD DSN=CEE.SCEERUN,DISP=SHR\n//REPORT   DD DSN={hlq}.REPORT({member}),DISP=SHR\n//SYSPRINT DD SYSOUT=*\n"));
    for dd in extra_dds {
        if let DdKind::Path(path) = &dd.kind {
            write_path_dd(&mut s, &dd.ddname, path);
            continue;
        }
        let unit = if dd.kind == DdKind::Tape {
            "3480"
        } else {
            "SYSDA"
        };
        let _ = writeln!(s, "//{:<8} DD DSN={hlq}.{member}.{},", dd.ddname, dd.ddname);
        let _ = writeln!(
            s,
            "//            UNIT={unit},SPACE=(TRK,(5,5)),DISP=(NEW,DELETE)"
        );
    }
    s
}

/// A z/OS UNIX file DD: created for read and write, deleted at the end of
/// the step, text data (line-sequential files).
fn write_path_dd(s: &mut String, ddname: &str, path: &str) {
    let _ = writeln!(s, "//{ddname:<8} DD PATH='{path}',");
    let _ = writeln!(
        s,
        "//            PATHOPTS=(ORDWR,OCREAT),PATHMODE=(SIRUSR,SIWUSR),"
    );
    let _ = writeln!(s, "//            PATHDISP=(DELETE,DELETE),FILEDATA=TEXT");
}

fn stem_upper(path: &Path) -> Result<String> {
    let name = path
        .file_stem()
        .and_then(|s| s.to_str())
        .with_context(|| format!("invalid file name {}", path.display()))?
        .to_ascii_uppercase();
    if name.is_empty() || name.len() > 8 {
        bail!("member name '{name}' exceeds 8-character PDS member limit");
    }
    Ok(name)
}

fn sorted_entries(dir: &Path, exts: &[&str]) -> Result<Vec<std::path::PathBuf>> {
    let mut files: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| exts.iter().any(|x| x.eq_ignore_ascii_case(e)))
        })
        .collect();
    files.sort();
    Ok(files)
}

/// Width of a source line as a z/OS record: a character outside Latin-1
/// is a double-byte character (two bytes), and each run of them is
/// enclosed in shift-out and shift-in (one byte each), as a mixed EBCDIC
/// code page (930, 939, 1390, 1399) stores it.
pub fn record_width(line: &str) -> usize {
    let mut width = 0;
    let mut dbcs = false;
    for c in line.chars() {
        let double = u32::from(c) > 0xFF;
        if double && !dbcs {
            width += 1;
        } else if !double && dbcs {
            width += 1;
        }
        dbcs = double;
        width += if double { 2 } else { 1 };
    }
    if dbcs {
        width += 1;
    }
    width
}

/// Pads each line to an 80-byte record, measured as the record will be on
/// z/OS (`record_width`), not in host characters.
fn format_fixed80(text: &str) -> Result<String> {
    let mut out = String::new();
    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim_end_matches('\r');
        let cols = record_width(trimmed);
        if cols > 80 {
            bail!("line {} exceeds 80 columns ({cols} bytes as a record)", idx + 1);
        }
        let pad = 80usize.saturating_sub(cols);
        let _ = writeln!(out, "{trimmed}{:pad$}", "");
    }
    Ok(out)
}

fn render_job_card(jobname: &str, desc: &str) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "//{jobname:<8} JOB (ZCVS),'{desc}',CLASS=A,MSGCLASS=X,");
    let _ = writeln!(s, "//         MSGLEVEL=(1,1),NOTIFY=&SYSUID");
    let _ = writeln!(
        s,
        "//*-------------------------------------------------------------------*"
    );
    s
}

fn render_subprogram_steps(hlq: &str, member: &str) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "//* SUBPROGRAM {member}");
    let _ = writeln!(
        s,
        "//C{member:<7} EXEC PGM=IGYCRCTL,PARM='RENT,NODYNAM,LIB'"
    );
    let _ = writeln!(s, "//STEPLIB  DD DSN=IGY.V6R4M0.SIGYCOMP,DISP=SHR");
    let _ = writeln!(s, "//SYSLIB   DD DSN={hlq}.COPY,DISP=SHR");
    let _ = writeln!(s, "//SYSIN    DD DSN={hlq}.SRCLIB({member}),DISP=SHR");
    let _ = writeln!(
        s,
        "//SYSLIN   DD DSN=&&OBJ{member},DISP=(MOD,PASS),UNIT=SYSDA,"
    );
    let _ = writeln!(s, "//            SPACE=(TRK,(5,5))");
    let _ = writeln!(s, "//SYSPRINT DD SYSOUT=*");
    for n in 1..=7 {
        let _ = writeln!(s, "//SYSUT{n}   DD UNIT=SYSDA,SPACE=(CYL,(1,1))");
    }
    let _ = writeln!(
        s,
        "//L{member:<7} EXEC PGM=IEWL,PARM='LIST,XREF,LET,RENT',COND=(4,LT,C{member})"
    );
    let _ = writeln!(s, "//SYSLIB   DD DSN=CEE.SCEELKED,DISP=SHR");
    let _ = writeln!(s, "//SYSLIN   DD DSN=&&OBJ{member},DISP=(OLD,DELETE)");
    let _ = writeln!(s, "//SYSLMOD  DD DSN={hlq}.LOAD({member}),DISP=SHR");
    let _ = writeln!(s, "//SYSPRINT DD SYSOUT=*");
    let _ = writeln!(s, "//SYSUT1   DD UNIT=SYSDA,SPACE=(CYL,(1,1))");
    s
}

fn render_program_steps(
    hlq: &str,
    member: &str,
    cfg: &ProgramConfig,
    extra_dds: &[DdAllocation],
) -> String {
    let mut s = String::new();
    let mut parms = vec!["RENT".to_string(), "NODYNAM".to_string(), "LIB".to_string()];
    if let Some(ccsid) = cfg.ccsid {
        parms.push(format!("CODEPAGE({ccsid})"));
    }
    let parm_str = parms.join(",");
    let _ = writeln!(s, "//* PROGRAM {member}");
    let _ = writeln!(s, "//C{member:<7} EXEC PGM=IGYCRCTL,PARM='{parm_str}'");
    let _ = writeln!(s, "//STEPLIB  DD DSN=IGY.V6R4M0.SIGYCOMP,DISP=SHR");
    let _ = writeln!(s, "//SYSLIB   DD DSN={hlq}.COPY,DISP=SHR");
    let _ = writeln!(s, "//SYSIN    DD DSN={hlq}.SRCLIB({member}),DISP=SHR");
    if cfg.expect == ExpectKind::CompileError {
        let _ = writeln!(s, "//SYSLIN   DD DUMMY");
        let _ = writeln!(s, "//SYSPRINT DD SYSOUT=*");
        for n in 1..=7 {
            let _ = writeln!(s, "//SYSUT{n}   DD UNIT=SYSDA,SPACE=(CYL,(1,1))");
        }
        return s;
    }
    let _ = writeln!(
        s,
        "//SYSLIN   DD DSN=&&OBJ{member},DISP=(MOD,PASS),UNIT=SYSDA,"
    );
    let _ = writeln!(s, "//            SPACE=(TRK,(5,5))");
    let _ = writeln!(s, "//SYSPRINT DD SYSOUT=*");
    for n in 1..=7 {
        let _ = writeln!(s, "//SYSUT{n}   DD UNIT=SYSDA,SPACE=(CYL,(1,1))");
    }
    if let Some(rc) = cfg.expect_compile_rc {
        // ends the job with RC 16 unless the compile gave exactly `rc`
        let _ = writeln!(s, "//V{member:<7} EXEC PGM=IDCAMS,COND=({rc},EQ,C{member})");
        let _ = writeln!(s, "//SYSPRINT DD SYSOUT=*");
        let _ = writeln!(s, "//SYSIN    DD *");
        let _ = writeln!(s, " SET MAXCC = 16");
        let _ = writeln!(s, "/*");
    }
    let _ = writeln!(
        s,
        "//L{member:<7} EXEC PGM=IEWL,PARM='LIST,XREF,LET,RENT',COND=(4,LT,C{member})"
    );
    let _ = writeln!(s, "//SYSLIB   DD DSN=CEE.SCEELKED,DISP=SHR");
    let _ = writeln!(s, "//         DD DSN={hlq}.LOAD,DISP=SHR");
    let _ = writeln!(s, "//SYSLIN   DD DSN=&&OBJ{member},DISP=(OLD,DELETE)");
    let _ = writeln!(s, "//SYSLMOD  DD DSN={hlq}.LOAD({member}),DISP=SHR");
    let _ = writeln!(s, "//SYSPRINT DD SYSOUT=*");
    let _ = writeln!(s, "//SYSUT1   DD UNIT=SYSDA,SPACE=(CYL,(1,1))");

    if cfg.expect == ExpectKind::CompileOnly {
        return s;
    }

    if let Some(run_parm) = &cfg.parm {
        let _ = writeln!(
            s,
            "//G{member:<7} EXEC PGM={member},PARM='{run_parm}',COND=(4,LT,L{member})"
        );
    } else {
        let _ = writeln!(s, "//G{member:<7} EXEC PGM={member},COND=(4,LT,L{member})");
    }
    let _ = writeln!(s, "//STEPLIB  DD DSN={hlq}.LOAD,DISP=SHR");
    let _ = writeln!(s, "//         DD DSN=CEE.SCEERUN,DISP=SHR");
    let _ = writeln!(s, "//REPORT   DD DSN={hlq}.REPORT({member}),DISP=SHR");
    let _ = writeln!(s, "//SYSOUT   DD SYSOUT=*");
    for dd in extra_dds {
        match &dd.kind {
            DdKind::Tape => {
                let _ = writeln!(
                    s,
                    "//{:<8} DD DSN={hlq}.{member}.{},UNIT=3480,DISP=(NEW,PASS)",
                    dd.ddname, dd.ddname
                );
            }
            DdKind::Dataset => {
                let _ = writeln!(s, "//{:<8} DD DSN={hlq}.{member}.{},", dd.ddname, dd.ddname);
                let _ = writeln!(
                    s,
                    "//            UNIT=SYSDA,SPACE=(TRK,(5,5)),DISP=(NEW,DELETE)"
                );
            }
            DdKind::Path(path) => write_path_dd(&mut s, &dd.ddname, path),
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_words_of_the_job_card() {
        let jcl = "//JOB44   JOB   (D548-8686,'12/8/85',PGMBIN),CLASS=B,\n\
                   //  MSGCLASS=X\n//GO EXEC PGM=GETSYM\n";
        assert_eq!(
            job_words(jcl, "GETSYM"),
            vec![
                ("NAME", "JOB44".to_string()),
                ("ACTINFO", "D548-8686,12/8/85,PGMBIN".to_string()),
                ("CLASS", "B".to_string()),
                ("MSGCLASS", "X".to_string()),
                ("STEPNAME", "GO".to_string()),
            ]
        );
    }

    #[test]
    fn exported_jcl_symbols() {
        let jcl = "//J JOB\n//EX1 EXPORT SYMLIST=(S1,L1,E)\n\
                   //SET1 SET S1=STEWART,L1='LA GUARDIA'\n\
                   //         SET E=,H=HIDDEN\n//S EXEC PGM=X\n";
        assert_eq!(
            exported_symbols(jcl),
            vec![
                ("S1".to_string(), "STEWART".to_string()),
                ("L1".to_string(), "LA GUARDIA".to_string()),
                ("E".to_string(), String::new()),
            ]
        );
    }

    #[test]
    fn parse_tape_and_disk_dds() {
        let jcl = "\
//ZF201M   JOB (ZCVS),'TAPE TEST',CLASS=A,MSGCLASS=X
//STEP1    EXEC PGM=ZF201M
//STEPLIB  DD DSN=USER.ZCVS.LOAD,DISP=SHR
//TAPE1    DD DSN=USER.ZCVS.TAPE1,UNIT=3480,VOL=SER=TAPE01,
//            DISP=(NEW,PASS),DCB=(RECFM=FB,LRECL=80,BLKSIZE=800)
//TAPE2    DD DSN=USER.ZCVS.TAPE2,UNIT=3480,VOL=SER=TAPE02,
//            DISP=(NEW,PASS),DCB=(RECFM=FB,LRECL=80,BLKSIZE=800)
//WORKDS   DD DSN=USER.ZCVS.WORK,UNIT=SYSDA,DISP=(NEW,DELETE)
//REPORT   DD SYSOUT=*
";
        let dds = parse_dd_allocations(jcl);
        assert_eq!(
            dds,
            vec![
                DdAllocation {
                    ddname: "TAPE1".to_string(),
                    kind: DdKind::Tape,
                },
                DdAllocation {
                    ddname: "TAPE2".to_string(),
                    kind: DdKind::Tape,
                },
                DdAllocation {
                    ddname: "WORKDS".to_string(),
                    kind: DdKind::Dataset,
                },
            ]
        );
    }

    #[test]
    fn record_width_counts_double_byte_characters_and_shift_codes() {
        assert_eq!(record_width("G1 DC C'A'"), 10);
        // 'Käfer' is single-byte in an EBCDIC code page
        assert_eq!(record_width("Käfer"), 5);
        // G'日' is G ' SO 2 bytes SI ': 7 bytes
        assert_eq!(record_width("G'日'"), 7);
        assert_eq!(record_width("山田太郎"), 10);
        assert_eq!(record_width("日X日"), 9);
    }
}

//! Compilers and the binder as step programs.
//!
//! * `IGYCRCTL` (COBOL) and `IBMZPLI` / `IEL1AA` (PL/I) compile the source read from
//!   SYSIN; COPY members and %INCLUDE files come from the SYSLIB
//!   libraries. The module is written to SYSLIN (Zeus has no object
//!   decks: the "object" is the executable module). Messages go to
//!   SYSPRINT; the return code is 0, or 12 when the compilation failed.
//!   PARM options: `CICS`, `NSYMBOL(DBCS)` (COBOL); others are accepted.
//! * `IEWL`, `IEWBLINK`, `HEWL`, `HEWLH096` (binder) copy the module of
//!   SYSLIN into the SYSLMOD library: the member named in the DSN, or by
//!   a `NAME member(R)` control statement in SYSLIN or SYSIN.

use std::io::Write;
use std::path::{Path, PathBuf};

use crate::job::{DdKind, ProgramResult, StepCtx};

/// Writes the library members of SYSLIB as host text files into `dir`
/// (the compilers read COPY members and include files as text).
fn syslib(ctx: &StepCtx, dir: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let Some(dd) = ctx.dd("SYSLIB") else {
        return dirs;
    };
    for (n, d) in std::iter::once(dd).chain(dd.concat.iter()).enumerate() {
        if !d.path.is_dir() {
            continue;
        }
        let out = dir.join(format!("lib{n}"));
        if std::fs::create_dir_all(&out).is_err() {
            continue;
        }
        if let Ok(rd) = std::fs::read_dir(&d.path) {
            for e in rd.flatten() {
                let Ok(data) = std::fs::read(e.path()) else {
                    continue;
                };
                let text = crate::job::member_text(&data);
                let _ = std::fs::write(out.join(e.file_name()), text);
            }
        }
        dirs.push(out);
    }
    dirs
}

fn options(parm: &str) -> Vec<String> {
    parm.split([',', ' '])
        .map(|o| o.trim().to_ascii_uppercase())
        .filter(|o| !o.is_empty())
        .collect()
}

/// Source text of SYSIN and the source name for messages.
fn source(ctx: &StepCtx) -> Result<(String, String), String> {
    let lines = ctx.read_text("SYSIN")?;
    let name = match ctx.dd("SYSIN").map(|d| &d.kind) {
        Some(DdKind::Dataset { dsn, member }) => match member {
            Some(m) => format!("{dsn}({m})"),
            None => dsn.clone(),
        },
        _ => "SYSIN".into(),
    };
    let mut text = lines.join("\n");
    text.push('\n');
    Ok((name, text))
}

/// Writes a module to SYSLIN (the file of the data set).
fn write_syslin(ctx: &StepCtx, data: &[u8]) -> Result<(), String> {
    let Some(dd) = ctx.dd("SYSLIN") else {
        return Ok(());
    };
    match dd.kind {
        DdKind::Dataset { .. } => {
            if let Some(p) = dd.path.parent() {
                std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            std::fs::write(&dd.path, data).map_err(|e| e.to_string())
        }
        _ => Ok(()),
    }
}

pub fn cobol(ctx: &mut StepCtx) -> ProgramResult {
    let mut print = ctx
        .sysout("SYSPRINT")
        .map_err(crate::job::Outcome::JclError)?;
    let opts = options(&ctx.parm);
    let tmp = tempdir()?;
    let (name, text) = match source(ctx) {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(print, "IGYOS4001-U  SYSIN: {e}");
            return Ok(16);
        }
    };
    let src = tmp.path().join(source_file(&name, "cbl"));
    std::fs::write(&src, text).map_err(|e| crate::job::Outcome::JclError(e.to_string()))?;
    let fo = zeus_cobol::front::FrontOptions {
        copy_dirs: syslib(ctx, tmp.path()),
        cics: opts.iter().any(|o| o == "CICS" || o.starts_with("CICS(")),
        nsymbol_dbcs: opts.iter().any(|o| o == "NSYMBOL(DBCS)" || o == "NS(DBCS)"),
        ..Default::default()
    };
    let _ = writeln!(print, "ZEUS COBOL COMPILER (IGYCRCTL)  SOURCE {name}");
    let rc = match zeus_cobol::compile_file(&src, &fo) {
        Ok(module) => {
            let out = tmp.path().join("module.zo");
            zeus_cobol::save_module(&module, &out)
                .map_err(|e| crate::job::Outcome::JclError(e.to_string()))?;
            let data = std::fs::read(&out).unwrap_or_default();
            if let Err(e) = write_syslin(ctx, &data) {
                let _ = writeln!(print, "IGYOS4002-U  SYSLIN: {e}");
                return Ok(16);
            }
            let _ = writeln!(print, "END OF COMPILATION, HIGHEST SEVERITY 0");
            0
        }
        Err(d) => {
            let _ = writeln!(print, "{d}");
            let _ = writeln!(print, "END OF COMPILATION, HIGHEST SEVERITY 12");
            12
        }
    };
    Ok(rc)
}

pub fn pli(ctx: &mut StepCtx) -> ProgramResult {
    let mut print = ctx
        .sysout("SYSPRINT")
        .map_err(crate::job::Outcome::JclError)?;
    let tmp = tempdir()?;
    let (name, text) = match source(ctx) {
        Ok(s) => s,
        Err(e) => {
            let _ = writeln!(print, "ZEUS PL/I: SYSIN: {e}");
            return Ok(16);
        }
    };
    let src = tmp.path().join(source_file(&name, "pli"));
    std::fs::write(&src, text).map_err(|e| crate::job::Outcome::JclError(e.to_string()))?;
    let opts = zeus_pli::Options {
        include_dirs: syslib(ctx, tmp.path()),
        ..Default::default()
    };
    let _ = writeln!(print, "ZEUS PL/I COMPILER (IBMZPLI)  SOURCE {name}");
    let (rc, compiled) = zeus_pli::compile_file(&src, &opts);
    for m in &compiled.messages {
        let _ = writeln!(print, "{m}");
    }
    if let Some(m) = compiled.module {
        let written = zeus_pli::module_bytes(&m)
            .map_err(|e| e.to_string())
            .and_then(|d| write_syslin(ctx, &d));
        if let Err(e) = written {
            let _ = writeln!(print, "ZEUS PL/I: SYSLIN: {e}");
            return Ok(16);
        }
    }
    let _ = writeln!(print, "END OF COMPILATION, RETURN CODE {rc}");
    Ok(rc)
}

/// The binder: the module of SYSLIN into the SYSLMOD library.
pub fn bind(ctx: &mut StepCtx) -> ProgramResult {
    let mut print = ctx
        .sysout("SYSPRINT")
        .map_err(crate::job::Outcome::JclError)?;
    let Some(lin) = ctx.dd("SYSLIN") else {
        let _ = writeln!(print, "IEW2201E SYSLIN NOT ALLOCATED");
        return Ok(12);
    };
    let data = std::iter::once(lin)
        .chain(lin.concat.iter())
        .filter_map(|d| std::fs::read(&d.path).ok())
        .find(|d| d.first() == Some(&b'{'));
    // (COBOL and PL/I modules are both JSON documents)
    let Some(data) = data else {
        let _ = writeln!(print, "IEW2202E NO MODULE IN SYSLIN");
        return Ok(12);
    };
    // NAME member(R) in SYSIN
    let named = ctx.read_text("SYSIN").ok().and_then(|lines| {
        lines.iter().find_map(|l| {
            let l = l.trim();
            let rest = l.strip_prefix("NAME ")?;
            let m = rest.trim().split(['(', ' ']).next()?.to_ascii_uppercase();
            (!m.is_empty()).then_some(m)
        })
    });
    let Some(lmod) = ctx.dd("SYSLMOD") else {
        let _ = writeln!(print, "IEW2203E SYSLMOD NOT ALLOCATED");
        return Ok(12);
    };
    let path = match (&lmod.kind, named) {
        (
            DdKind::Dataset {
                member: Some(_), ..
            },
            None,
        ) => lmod.path.clone(),
        (
            DdKind::Dataset {
                member: Some(_), ..
            },
            Some(m),
        ) => lmod.path.parent().map(|p| p.join(&m)).unwrap_or_default(),
        (DdKind::Dataset { member: None, .. }, Some(m)) => lmod.path.join(m),
        _ => {
            let _ = writeln!(print, "IEW2204E NO MEMBER NAME FOR SYSLMOD");
            return Ok(12);
        }
    };
    if let Some(p) = path.parent() {
        if p.is_file() {
            let _ = std::fs::remove_file(p);
        }
        std::fs::create_dir_all(p).map_err(|e| crate::job::Outcome::JclError(e.to_string()))?;
    }
    std::fs::write(&path, &data).map_err(|e| crate::job::Outcome::JclError(e.to_string()))?;
    let member = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let _ = writeln!(print, "IEW2008I MODULE {member} SAVED");
    Ok(0)
}

fn source_file(name: &str, ext: &str) -> String {
    let base: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    format!("{base}.{ext}")
}

fn tempdir() -> Result<tempfile::TempDir, crate::job::Outcome> {
    // Exclusive creation avoids clock-resolution collisions between parallel
    // compiler steps. TempDir also cleans up on every early return.
    tempfile::Builder::new()
        .prefix("zeus-compile-")
        .tempdir()
        .map_err(|e| crate::job::Outcome::JclError(e.to_string()))
}

//! `EXEC PGM=DFSRRC00`: IMS batch regions.
//!
//! PARM='DLI|DBB|BMP,program,psb': the program runs with the PSB
//! scheduled and gets its PCB masks (the I/O PCB first for BMP regions and
//! CMPAT=YES PSBs). PARM='ULR,DFSURGL0,dbd' reloads a database from the
//! unload in DD DFSUINPT. The DBD and PSB sources are the members of the
//! IMS DD concatenation (DBDLIB and PSBLIB); the databases are in one
//! store: ZEUS_IMS_WAL, or `ims/ims.wal` next to the catalog.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use crate::expand::Step;
use crate::job::{Outcome, ProgramResult, StepCtx};

fn fail(ctx: &StepCtx, msg: &str, code: &str) -> ProgramResult {
    if let Ok(mut f) = ctx.sysout("SYSPRINT") {
        let _ = writeln!(f, "ZEUS IMS: {msg}");
    }
    Err(Outcome::Abend(code.into()))
}

/// The IMS system of a step: the library of its IMS DD and the store.
fn system(ctx: &StepCtx) -> Result<zeus_ims::runtime::Ims, String> {
    let dirs: Vec<PathBuf> = match ctx.dd("IMS") {
        Some(dd) => std::iter::once(dd)
            .chain(dd.concat.iter())
            .map(|d| d.path.clone())
            .collect(),
        None => std::env::var("ZEUS_IMS_LIB")
            .map(|v| v.split(':').map(PathBuf::from).collect())
            .map_err(|_| "no IMS DD (PSBLIB and DBDLIB) for the step".to_string())?,
    };
    let library = zeus_ims::runtime::Library::from_dirs(&dirs)?;
    let wal = std::env::var_os("ZEUS_IMS_WAL")
        .map(PathBuf::from)
        .unwrap_or_else(|| ctx.env.catalog.root().join("ims").join("ims.wal"));
    zeus_ims::runtime::Ims::open(&wal, library)
}

pub(crate) fn run(ctx: &mut StepCtx, libs: &[PathBuf], job: &str, step: &Step) -> ProgramResult {
    let parm = ctx
        .parm
        .trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .to_string();
    let parts: Vec<String> = parm
        .split(',')
        .map(|p| p.trim().to_ascii_uppercase())
        .collect();
    let region = parts.first().cloned().unwrap_or_default();
    let program = parts.get(1).cloned().unwrap_or_default();
    let psb = parts
        .get(2)
        .filter(|p| !p.is_empty())
        .cloned()
        .unwrap_or_else(|| program.clone());
    let ims = match system(ctx) {
        Ok(i) => i,
        Err(e) => return fail(ctx, &e, "U0428"),
    };
    match region.as_str() {
        "DLI" | "DBB" | "BMP" => {}
        "ULR" if program == "DFSURGL0" => {
            let Some(input) = ctx.dd("DFSUINPT").map(|d| d.path.clone()) else {
                return fail(ctx, "DFSURGL0 needs DD DFSUINPT", "U0428");
            };
            let data = std::fs::read(&input).unwrap_or_default();
            return match zeus_ims::runtime::reload(&ims, &psb, &data) {
                Ok(n) => {
                    if let Ok(mut f) = ctx.sysout("SYSPRINT") {
                        let _ = writeln!(f, "ZEUS IMS: DFSURGL0 reloaded {n} segments of {psb}");
                    }
                    Ok(0)
                }
                Err(e) => fail(ctx, &format!("DFSURGL0 {psb}: {e}"), "U0359"),
            };
        }
        other => {
            return fail(
                ctx,
                &format!("region type {other} ({program}) is not supported"),
                "U0101",
            );
        }
    }
    let io_pcb = region == "BMP" || ims.library.psbs.get(&psb).is_some_and(|p| p.cmpat);
    let dds: HashMap<String, PathBuf> = ctx
        .dds
        .iter()
        .map(|d| (d.name.clone(), d.path.clone()))
        .collect();
    let sched = match zeus_ims::runtime::Schedule::new(&ims, &psb, io_pcb, &dds) {
        Ok(s) => s,
        Err(e) => return fail(ctx, &e, "U0456"),
    };
    crate::job::run_program(ctx, &program, libs, job, step, Some(sched))
}

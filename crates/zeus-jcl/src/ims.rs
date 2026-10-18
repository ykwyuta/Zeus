//! `EXEC PGM=DFSRRC00`: IMS batch and message processing regions, and the
//! transaction definitions of `EXEC PGM=CSLUSPOC`.
//!
//! PARM='MSG': a message processing region. Each input message queued for
//! a defined transaction (CSLUSPOC `CREATE TRAN NAME(t) SET(PGM(p))`; the
//! PSB is the program's name) runs its program with the I/O PCB first;
//! the program gets the message with GU and inserts its output to the
//! source LTERM, whose queue `zjob ims receive` reads. The region ends when
//! no message is left (WFI=N). A program that ends abnormally leaves its
//! input message queued and stops its transaction for the rest of the
//! region (return code 8). The message queue is `ims/tmq` next to the
//! catalog.
//!
//! PARM='DLI|DBB|BMP,program,psb': the program runs with the PSB
//! scheduled and gets its PCB masks (the I/O PCB first for BMP regions and
//! CMPAT=YES PSBs). PARM='ULU,DFSURGU0,dbd' unloads a database to DD
//! DFSUROUT; PARM='ULR,DFSURGL0,dbd' reloads a database from the
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
        // the prereorganization utility: the Zeus store needs no control
        // data set
        "ULU" if program == "DFSURPR0" => {
            if let Ok(mut f) = ctx.sysout("SYSPRINT") {
                let _ = writeln!(f, "ZEUS IMS: DFSURPR0 has nothing to do");
            }
            return Ok(0);
        }
        // the HD reorganization unload: the database to DD DFSUROUT
        "ULU" if program == "DFSURGU0" => {
            let Some(out) = ctx.dd("DFSUROUT").map(|d| d.path.clone()) else {
                return fail(ctx, "DFSURGU0 needs DD DFSUROUT", "U0428");
            };
            return match zeus_ims::runtime::unload(&ims, &psb) {
                Ok((data, n)) => {
                    if let Err(e) = std::fs::write(&out, data) {
                        return fail(ctx, &format!("DFSURGU0 {psb}: {e}"), "U0359");
                    }
                    if let Ok(mut f) = ctx.sysout("SYSPRINT") {
                        let _ = writeln!(f, "ZEUS IMS: DFSURGU0 unloaded {n} segments of {psb}");
                    }
                    Ok(0)
                }
                Err(e) => fail(ctx, &format!("DFSURGU0 {psb}: {e}"), "U0359"),
            };
        }
        "MSG" => return message_region(ctx, libs, job, step, &ims),
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
    let mut sched = match zeus_ims::runtime::Schedule::new(&ims, &psb, io_pcb, &dds) {
        Ok(s) => s,
        Err(e) => return fail(ctx, &e, "U0456"),
    };
    if region == "BMP" {
        sched.enable_sqlims();
    }
    crate::job::run_program(ctx, &program, libs, job, step, Some(sched))
}

// ---- message processing ---------------------------------------------------

/// The file of the transaction definitions (transaction, program).
fn definitions_path(root: &std::path::Path) -> PathBuf {
    root.join("ims").join("transactions")
}

/// The IMS message queue next to the catalog.
pub fn message_queue(root: &std::path::Path) -> Result<zeus_ims::tm::Queue, String> {
    zeus_ims::tm::Queue::open(root.join("ims").join("tmq"))
}

/// The defined transactions: code -> program.
pub fn transactions(root: &std::path::Path) -> Vec<(String, String)> {
    std::fs::read_to_string(definitions_path(root))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut w = l.split_whitespace();
            Some((w.next()?.to_string(), w.next()?.to_string()))
        })
        .collect()
}

/// An 8-byte EBCDIC name.
pub fn name8(s: &str) -> [u8; 8] {
    let mut b = [zeus_cobol::ebcdic::SPACE; 8];
    for (d, c) in b.iter_mut().zip(s.to_ascii_uppercase().bytes()) {
        *d = zeus_cobol::ebcdic::from_latin1(c);
    }
    b
}

fn text8(b: &[u8; 8]) -> String {
    zeus_cobol::ebcdic::decode_latin1(b)
        .iter()
        .map(|&c| c as char)
        .collect::<String>()
        .trim()
        .to_string()
}

/// `EXEC PGM=CSLUSPOC`: the type-2 commands of SYSIN. CREATE TRAN defines
/// a transaction (kept next to the catalog); the other commands (CREATE
/// DB / PGM, START) have nothing to do.
pub(crate) fn spoc(ctx: &mut StepCtx) -> ProgramResult {
    let input = ctx.read_text("SYSIN").unwrap_or_default();
    let mut out = ctx
        .sysout("SYSPRINT")
        .map_err(|e| Outcome::JclError(e.to_string()))?;
    // commands: '+' or '-' at the end continues on the next line
    let mut cmds = Vec::new();
    let mut cur = String::new();
    for l in &input {
        let t: String = l.chars().take(72).collect();
        let t = t.trim();
        if let Some(rest) = t.strip_suffix('+').or_else(|| t.strip_suffix('-')) {
            cur.push_str(rest);
            cur.push(' ');
        } else {
            cur.push_str(t);
            if !cur.trim().is_empty() {
                cmds.push(cur.trim().to_string());
            }
            cur.clear();
        }
    }
    let root = ctx.env.catalog.root().to_path_buf();
    let mut defs = transactions(&root);
    let value = |cmd: &str, key: &str| -> Option<String> {
        let up = cmd.to_ascii_uppercase();
        let i = up.find(&format!("{key}("))? + key.len() + 1;
        let j = up[i..].find(')')? + i;
        Some(up[i..j].trim().to_string())
    };
    for cmd in cmds {
        let up = cmd.to_ascii_uppercase();
        let words: Vec<&str> = up.split_whitespace().collect();
        if words.len() >= 2 && words[0] == "CREATE" && words[1] == "TRAN" {
            let (Some(t), Some(p)) = (value(&cmd, "NAME"), value(&cmd, "PGM")) else {
                let _ = writeln!(out, "{cmd}\n  ZEUS: NAME and PGM are needed");
                continue;
            };
            defs.retain(|d| d.0 != t);
            defs.push((t.clone(), p.clone()));
            let _ = writeln!(out, "{cmd}\n  ZEUS: transaction {t} runs program {p}");
        } else {
            let _ = writeln!(out, "{cmd}\n  ZEUS: nothing to do");
        }
    }
    let path = definitions_path(&root);
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let text: String = defs.iter().map(|(t, p)| format!("{t} {p}\n")).collect();
    std::fs::write(&path, text).map_err(|e| Outcome::JclError(e.to_string()))?;
    Ok(0)
}

/// A message processing region (PARM='MSG'): runs the queued input
/// messages of the defined transactions.
fn message_region(
    ctx: &mut StepCtx,
    libs: &[PathBuf],
    job: &str,
    step: &Step,
    ims: &zeus_ims::runtime::Ims,
) -> ProgramResult {
    let root = ctx.env.catalog.root().to_path_buf();
    let queue = match message_queue(&root) {
        Ok(q) => q,
        Err(e) => return fail(ctx, &format!("message queue: {e}"), "U0428"),
    };
    let owner = zeus_ims::tm::process_owner();
    let dds: HashMap<String, PathBuf> = ctx
        .dds
        .iter()
        .map(|d| (d.name.clone(), d.path.clone()))
        .collect();
    let mut rc = 0;
    let mut count = 0;
    let mut stopped = std::collections::HashSet::new();
    loop {
        let mut ran = false;
        for (tran, program) in transactions(&root) {
            if stopped.contains(&tran) {
                continue;
            }
            let Ok(Some(msg)) = queue.get(name8(&tran), owner) else {
                continue;
            };
            ran = true;
            count += 1;
            let input = zeus_ims::runtime::TmMessage {
                destination: tran.clone(),
                source: text8(&msg.source),
                segments: zeus_ims::runtime::TmMessage::split(&msg.data),
            };
            let mut sched = match zeus_ims::runtime::Schedule::new(ims, &program, true, &dds) {
                Ok(s) => s,
                Err(e) => {
                    let _ = queue.rollback(owner, &[msg.id]);
                    return fail(ctx, &e, "U0456");
                }
            };
            sched.tm = Some(zeus_ims::runtime::Tm::new(vec![input], "ZEUSUSER"));
            sched.enable_sqlims();
            if let Ok(mut f) = ctx.sysout("SYSPRINT") {
                let _ = writeln!(f, "ZEUS IMS: transaction {tran} (program {program})");
            }
            let r = crate::job::run_program(ctx, &program, libs, job, step, Some(sched));
            let outputs = zeus_ims::runtime::take_outputs();
            match r {
                Ok(c) => {
                    rc = rc.max(c);
                    let out: Vec<([u8; 8], [u8; 8], Vec<u8>)> = outputs
                        .iter()
                        .map(|m| (name8(&m.destination), name8(&tran), m.data()))
                        .collect();
                    if let Err(e) = queue.complete(owner, &[msg.id], &out) {
                        return fail(ctx, &format!("message queue: {e}"), "U0428");
                    }
                }
                Err(e) => {
                    // the input message stays queued and the transaction is
                    // stopped (for this region); the others go on
                    let _ = queue.rollback(owner, &[msg.id]);
                    let why = match &e {
                        Outcome::Abend(c) => format!("abend {c}"),
                        other => format!("{other:?}"),
                    };
                    if let Ok(mut f) = ctx.sysout("SYSPRINT") {
                        let _ = writeln!(
                            f,
                            "ZEUS IMS: transaction {tran} ended abnormally ({why}); it is stopped"
                        );
                    }
                    stopped.insert(tran.clone());
                    rc = rc.max(8);
                }
            }
        }
        if !ran {
            break;
        }
    }
    if let Ok(mut f) = ctx.sysout("SYSPRINT") {
        let _ = writeln!(f, "ZEUS IMS: {count} messages processed");
    }
    Ok(rc)
}

//! TSO in batch (IKJEFT01, IKJEFT1A, IKJEFT1B) with the DSN command
//! processor of Db2: `DSN SYSTEM(...)`, `RUN PROGRAM(p) [PLAN(..)]
//! [LIB('dsn')] [PARMS('..')]`, `BIND`/`FREE` (nothing to do: Zeus runs
//! SQL dynamically) and `END`.
//!
//! The Db2 sample programs DSNTIAD, DSNTEP2 and DSNTEP4 run the SQL
//! statements of SYSIN; other programs are COBOL programs of the step's
//! libraries. The Db2 server is the one named by ZEUS_DB2.

use std::io::Write;

use crate::job::{Outcome, ProgramResult, StepCtx};

/// SYSTSIN commands: comments removed, `-` / `+` continuations joined.
fn commands(lines: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for l in lines {
        let mut l: String = l.chars().take(72).collect();
        if let Some(p) = l.find("/*") {
            l.truncate(p);
        }
        let t = l.trim_end();
        if let Some(rest) = t.strip_suffix('-').or_else(|| t.strip_suffix('+')) {
            cur.push_str(rest);
            cur.push(' ');
            continue;
        }
        cur.push_str(t);
        let c = cur.trim().to_string();
        if !c.is_empty() {
            out.push(c);
        }
        cur.clear();
    }
    out
}

/// The value of `KEY(value)` in a command (quotes removed).
fn operand(cmd: &str, key: &str) -> Option<String> {
    let up = cmd.to_ascii_uppercase();
    let at = up.find(&format!("{key}("))?;
    let rest = &cmd[at + key.len() + 1..];
    let end = rest.rfind(')').map(|e| {
        // the first ')' outside quotes
        let mut q = false;
        rest.char_indices()
            .find(|&(_, c)| {
                if c == '\'' {
                    q = !q;
                }
                c == ')' && !q
            })
            .map_or(e, |(i, _)| i)
    })?;
    Some(rest[..end].trim().trim_matches('\'').to_string())
}

pub fn run(
    program: &str,
    ctx: &mut StepCtx,
    libs: &[std::path::PathBuf],
    job: &str,
    step: &crate::expand::Step,
) -> ProgramResult {
    let input = ctx.read_text("SYSTSIN").unwrap_or_default();
    let mut out = ctx
        .sysout("SYSTSPRT")
        .map_err(|e| Outcome::JclError(e.to_string()))?;
    let mut rc = 0;
    let _ = writeln!(out, "{program}: TSO commands");
    for cmd in commands(&input) {
        let _ = writeln!(out, "READY\n{cmd}");
        let verb = cmd
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        match verb.as_str() {
            "DSN" | "END" | "FREE" | "BIND" | "REBIND" => {}
            "RUN" => {
                let Some(p) = operand(&cmd, "PROGRAM") else {
                    let _ = writeln!(out, "DSNE118E PROGRAM NOT SPECIFIED");
                    rc = rc.max(12);
                    continue;
                };
                let p = p.to_ascii_uppercase();
                let r = match p.as_str() {
                    "DSNTIAD" | "DSNTEP2" | "DSNTEP4" => {
                        let rc0 = operand(&cmd, "PARMS")
                            .is_some_and(|x| x.to_ascii_uppercase().contains("RC0"));
                        sql_script(ctx, &p, rc0)
                    }
                    _ => {
                        let mut libs = libs.to_vec();
                        if let Some(lib) = operand(&cmd, "LIB") {
                            libs.push(ctx.env.catalog.path(&lib));
                        }
                        if let Some(parm) = operand(&cmd, "PARMS") {
                            ctx.parm = parm;
                        }
                        crate::job::run_cobol(ctx, &p, &libs, job, step)
                    }
                };
                let c = r?;
                let _ = writeln!(out, "DSN RUN {p} RC={c:04}");
                rc = rc.max(c);
            }
            other => {
                let _ = writeln!(out, "IKJ56500I COMMAND {other} NOT FOUND");
                rc = rc.max(12);
            }
        }
    }
    Ok(rc)
}

/// DSNTIAD / DSNTEP2 / DSNTEP4: the SQL statements of SYSIN. With
/// PARMS('RC0') warnings leave the return code at 0.
fn sql_script(ctx: &mut StepCtx, program: &str, rc0: bool) -> ProgramResult {
    let mut out = ctx
        .sysout("SYSPRINT")
        .map_err(|e| Outcome::JclError(e.to_string()))?;
    let Some(db2) = zeus_db2::Db2Host::from_env() else {
        let _ = writeln!(out, "{program}: no Db2 server configured (ZEUS_DB2)");
        return Ok(12);
    };
    let text: String = ctx
        .read_text("SYSIN")
        .unwrap_or_default()
        .iter()
        .map(|l| {
            let l: String = l.chars().take(72).collect();
            format!("{l}\n")
        })
        .collect();
    let mut rc = 0;
    for stmt in zeus_db2::dialect::split_script(&text) {
        let _ = writeln!(out, "{program}: {stmt}");
        let up = stmt.to_ascii_uppercase();
        let r = if up.starts_with("COMMIT") {
            db2.end_uow(true).map(Some)
        } else if up.starts_with("ROLLBACK") {
            db2.end_uow(false).map(Some)
        } else {
            db2.run_script_statement(&stmt)
        };
        match r {
            Ok(None) => {
                let _ = writeln!(out, "  (not needed on Db2 for Linux; skipped)");
            }
            Ok(Some(ca)) | Err(ca) => {
                let _ = writeln!(
                    out,
                    "  SQLCODE = {}, SQLSTATE = {} {}",
                    ca.sqlcode,
                    ca.sqlstate,
                    ca.tokens.join(" ")
                );
                if ca.sqlcode < 0 {
                    rc = rc.max(8);
                } else if ca.sqlcode > 0 && ca.sqlcode != 100 && !rc0 {
                    rc = rc.max(4);
                }
            }
        }
    }
    let _ = db2.end_uow(rc < 8);
    Ok(rc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tso_commands() {
        let c = commands(&[
            "/* comment */".into(),
            "  DSN SYSTEM(DAZ1)".into(),
            "  RUN PROGRAM(DSNTEP4) -".into(),
            "  PLAN(DSNTEP4) -".into(),
            "  PARMS('/ALIGN(LHS) MIXED')".into(),
            " END".into(),
        ]);
        assert_eq!(c.len(), 3);
        assert_eq!(operand(&c[1], "PROGRAM").as_deref(), Some("DSNTEP4"));
        assert_eq!(
            operand(&c[1], "PARMS").as_deref(),
            Some("/ALIGN(LHS) MIXED")
        );
    }
}

//! System utilities run as step programs.

mod compile;
pub mod idcams;
mod iebgener;
pub mod sort;
pub mod tso;

use std::io::Write;

use crate::job::{ProgramResult, StepCtx};

/// Runs `program` when it is a utility; None for other programs.
pub fn run(program: &str, ctx: &mut StepCtx) -> Option<ProgramResult> {
    Some(match program {
        "IEFBR14" => Ok(0),
        "IEBGENER" | "ICEGENER" => iebgener::run(ctx),
        "IDCAMS" => idcams::run(ctx),
        "SORT" | "ICEMAN" | "DFSORT" | "SYNCSORT" => sort::run(ctx),
        "IGYCRCTL" => compile::cobol(ctx),
        "IBMZPLI" | "IEL1AA" | "IEL0AA" => compile::pli(ctx),
        "IEWL" | "IEWBLINK" | "HEWL" | "HEWLH096" | "IEWLF128" => compile::bind(ctx),
        // Operator and network facilities of the z/OS installation; the
        // CardDemo batch jobs use them to close / open CICS files.
        "SDSF" | "FTP" => {
            if let Ok(mut f) = ctx.sysout("ISFOUT").or_else(|_| ctx.sysout("SYSPRINT")) {
                let _ = writeln!(f, "ZEUS: {program} is not available; step ignored");
            }
            Ok(0)
        }
        _ => return None,
    })
}

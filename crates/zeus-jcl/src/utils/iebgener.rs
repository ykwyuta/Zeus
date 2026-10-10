//! IEBGENER: copies SYSUT1 to SYSUT2 (no editing control statements).

use std::io::Write;

use zeus_ds::catalog::WriteMode;

use crate::job::{Outcome, ProgramResult, StepCtx};

pub fn run(ctx: &mut StepCtx) -> ProgramResult {
    let mut print = ctx.sysout("SYSPRINT").map_err(Outcome::JclError)?;
    let _ = writeln!(print, "DATA SET UTILITY - GENERATE");
    let recs = match ctx.read("SYSUT1") {
        Ok(r) => r,
        Err(e) => {
            let _ = writeln!(print, "IEB311I {e}");
            return Ok(12);
        }
    };
    ctx.inherit_attributes("SYSUT2", "SYSUT1", &recs);
    match ctx.write("SYSUT2", &recs, WriteMode::Replace) {
        Ok(_) => {
            let _ = writeln!(print, "PROCESSING ENDED AT EOD ({} RECORDS)", recs.len());
            Ok(0)
        }
        Err(e) => {
            let _ = writeln!(print, "IEB311I {e}");
            Ok(12)
        }
    }
}

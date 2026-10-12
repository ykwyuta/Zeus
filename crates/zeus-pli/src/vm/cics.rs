//! EXEC CICS of PL/I programs: the commands run on the CICS interface of
//! the run unit (the task's COBOL machine), with their operands in the
//! run unit's storage.

use super::*;

impl Machine {
    pub(super) fn exec_cics(&mut self, x: &ExecStmt, _act: usize) -> R<()> {
        err(format!("EXEC CICS {} is not supported yet", x.text))
    }
}

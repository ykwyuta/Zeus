//! CICS for Zeus: runs CICS transactions written in COBOL.
//!
//! A [`region::Region`] holds the resource definitions (from a CSD), the
//! data sets (through the zeus-ds catalog), mapsets, temporary storage and
//! transient data queues. A [`region::Terminal`] is a 3270 display
//! ([`screen::Screen`]); when its operator presses an attention key the
//! region starts the transaction named on the screen or by the previous
//! RETURN TRANSID, and [`task`] runs its programs on the zcobc VM,
//! executing their EXEC CICS commands.

pub mod bms;
pub mod csd;
pub mod indfile;
pub mod region;
pub mod screen;
pub mod task;
pub mod time;
pub mod tn3270;

//! Zeus IMS DB: hierarchical databases defined by DBD and PSB sources,
//! DL/I calls (GU, GN, GNP and their hold forms, ISRT, REPL, DLET) through
//! PCBs, units of work with a checksummed write-ahead log, GSAM and the
//! IMS TM message queue.
//!
//! The database core comes from the `imsdb` crate of gnucobol-zosplus
//! (by the same owner); the run-time glue for the Zeus COBOL and PL/I
//! run times is in `runtime`.

pub mod call_adapter;
pub mod catalog;
pub mod csv_load;
pub mod dli;
pub mod gsam;
pub mod runtime;
pub mod store;
pub mod tm;
pub mod unload;

pub use catalog::{Dbd, Psb};
pub use store::{Database, Record, StoreError};

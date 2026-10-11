//! Db2 for Zeus: the embedded SQL of COBOL programs (EXEC SQL) runs on a
//! Db2 server (Db2 Community Edition) through the DRDA client of
//! `zeus-drda`, with Db2 for z/OS SQL adapted where Db2 for Linux differs.

// SQLCA errors are this crate's error type
#![allow(clippy::result_large_err)]

pub mod dialect;
pub mod host;

pub use host::Db2Host;

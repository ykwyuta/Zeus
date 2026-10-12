//! A client of the Distributed Relational Database Architecture (DRDA,
//! The Open Group), the protocol Db2 servers speak, written from the
//! published DRDA specification. Zeus uses it to run the embedded SQL of
//! COBOL programs on Db2 (Db2 Community Edition).

pub mod client;
pub mod ddm;
pub mod sql;

pub use client::{Connection, Error, Params};

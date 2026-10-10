//! Zeus JCL engine.
//!
//! Runs z/OS batch jobs: parses the JCL ([`parse`]), expands catalogued
//! and in-stream procedures with their symbolic parameters ([`expand`]),
//! allocates the data sets of each step through the catalog of `zeus-ds`
//! and runs the step's program ([`job`]): a COBOL program compiled by
//! zcobc, or one of the system utilities ([`utils`]) IEFBR14, IEBGENER,
//! IDCAMS and SORT.

pub mod expand;
pub mod job;
pub mod native;
pub mod parse;
pub mod utils;

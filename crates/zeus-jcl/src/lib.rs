//! Zeus JCL engine.
//!
//! Runs z/OS batch jobs: parses the JCL ([`parse`]), expands catalogued
//! and in-stream procedures with their symbolic parameters ([`expand`]),
//! allocates the data sets of each step through the catalog of `zeus-ds`
//! and runs the step's program ([`job`]): a COBOL program compiled by
//! zcobc, or one of the system utilities ([`utils`]) IEFBR14, IEBGENER,
//! IDCAMS, SORT, the COBOL compiler IGYCRCTL and the binder IEWL. Steps are
//! selected by COND and by IF/THEN/ELSE/ENDIF constructs ([`cond`]);
//! INCLUDE groups come from the JCLLIB libraries.

pub mod cond;
pub mod expand;
pub mod ims;
pub mod job;
pub mod native;
pub mod parse;
pub mod utils;

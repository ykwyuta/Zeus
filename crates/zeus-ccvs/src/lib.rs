//! Tooling for the NIST CCVS85 COBOL-85 test suite: extraction of the test
//! programs from the population file, compilation and execution with a
//! COBOL toolchain (GnuCOBOL as the comparison oracle, or `zcobc`), parsing of
//! the test reports and summaries.
//!
//! See docs/proposal/0003-cobol-compiler.md section 5.2 for the pass criteria.

pub mod exec85;
pub mod extract;
pub mod jcl;
pub mod manifest;
pub mod population;
pub mod report;
pub mod runner;
pub mod special;
pub mod summary;
pub mod toolchain;

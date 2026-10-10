//! Oracle runs of packed-decimal instructions on Hercules.
//!
//! Generated cases (AP, SP, ZAP, CP, MP, DP, SRP, MVO, PACK, UNPK) are packed
//! into one IPL-able ESA/390 storage image, executed on Hercules, and the
//! resulting operand fields, condition codes and program interruption codes
//! are recorded. In Phase 1 these records are the expected results for
//! `zeus-decimal` (docs/proposal/0003 section 3, item 2).

pub mod cases;
pub mod emulate;
pub mod hercules;
pub mod image;

use serde::{Deserialize, Serialize};

use cases::Case;
use image::Observed;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    #[serde(flatten)]
    pub case: Case,
    #[serde(with = "cases::hex")]
    pub instruction: Vec<u8>,
    pub result: Observed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleRun {
    pub oracle: String,
    pub architecture: String,
    pub seed: u64,
    pub decimal_overflow_interrupt: bool,
    pub records: Vec<Record>,
}

//! Decimal arithmetic for Zeus.
//!
//! * [`Dec`] - an exact fixed-point decimal value (128-bit coefficient and a
//!   decimal scale) used for COBOL arithmetic.
//! * [`packed`], [`zoned`], [`binary`] - the storage formats of numeric data
//!   (USAGE PACKED-DECIMAL / DISPLAY / BINARY) with EBCDIC zones.
//! * [`hfp`] - IBM hexadecimal floating point (COMP-1 / COMP-2, FLOAT).
//! * [`insn`] - the semantics of the z/Architecture decimal instructions
//!   (AP, SP, ZAP, CP, MP, DP, SRP, MVO, PACK, UNPK), verified against
//!   Hercules (`zeus-hercdiff`).
//!
//! All formats follow the Principles of Operation (SA22-7832): sign codes
//! A, C, E, F are plus and B, D are minus; the preferred signs are C and D.

pub mod binary;
mod dec;
pub mod hfp;
pub mod insn;
pub mod packed;
pub mod zoned;

pub use dec::{Dec, Rounding};

/// Preferred sign codes.
pub const SIGN_PLUS: u8 = 0xC;
pub const SIGN_MINUS: u8 = 0xD;
/// Sign code of unsigned fields.
pub const SIGN_UNSIGNED: u8 = 0xF;

/// Whether a sign nibble denotes a negative value (B or D).
pub fn sign_is_negative(sign: u8) -> bool {
    sign == 0xB || sign == 0xD
}

/// Whether a nibble is a valid sign code (A-F).
pub fn sign_is_valid(sign: u8) -> bool {
    sign >= 0xA
}

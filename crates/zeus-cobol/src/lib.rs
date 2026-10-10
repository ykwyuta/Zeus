//! The Zeus COBOL compiler (Enterprise COBOL 4.2 dialect) and its runtime.
//!
//! Pipeline: source reading and COPY/REPLACE ([`front`]), semantic analysis
//! and storage layout ([`sema`]), code generation to the ZIR instruction
//! set, and execution by the ZIR interpreter ([`vm`]).

pub mod diag;
pub mod ebcdic;
pub mod front;

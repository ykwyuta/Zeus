//! Zeus data set layer.
//!
//! Data sets hold EBCDIC records as z/OS does; this crate knows nothing
//! about COBOL. It provides:
//!
//! * [`catalog`]: data set names and attributes (non-VSAM, VSAM clusters,
//!   alternate indexes, paths, generation data groups);
//! * [`dd`]: allocation of DD names to host files;
//! * [`format`]: the on-disk record formats (fixed length, variable
//!   length with a record descriptor word, host text lines);
//! * [`seq`]: sequential data sets;
//! * [`relative`]: relative record data sets (numbered slots, as VSAM
//!   RRDS);
//! * [`keyed`]: key-sequenced data sets with alternate indexes (as VSAM
//!   KSDS with AIX), keys given by offset and length in the record.
//!
//! Data sets are loaded into memory when opened and written back when
//! closed; the in-memory structures are the ones a later persistent
//! implementation keeps.

pub mod catalog;
pub mod dd;
pub mod format;
pub mod keyed;
pub mod physical;
pub mod relative;
pub mod seq;
pub mod tape;

use std::fmt;

/// Data set errors. Programming-language run times map them to their own
/// status codes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DsError {
    /// No record with the key / record number.
    NotFound,
    /// A record with the primary key or record number already exists.
    DuplicateKey,
    /// The record would duplicate an alternate key defined as unique.
    DuplicateAlternateKey,
    /// Record number 0 or outside the data set.
    InvalidRecordNumber,
    /// The data set content does not follow its record format.
    Damaged(String),
    /// Host I/O error.
    Io(String),
}

impl fmt::Display for DsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DsError::NotFound => f.write_str("record not found"),
            DsError::DuplicateKey => f.write_str("duplicate key"),
            DsError::DuplicateAlternateKey => f.write_str("duplicate alternate key"),
            DsError::InvalidRecordNumber => f.write_str("invalid record number"),
            DsError::Damaged(s) => write!(f, "data set damaged: {s}"),
            DsError::Io(s) => write!(f, "I/O error: {s}"),
        }
    }
}

impl std::error::Error for DsError {}

pub type Result<T> = std::result::Result<T, DsError>;

/// Relation of a positioning request (START).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rel {
    Eq,
    Gt,
    Ge,
}

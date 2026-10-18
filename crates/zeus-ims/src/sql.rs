//! IMS COBOL dynamic SQL, implemented from the IMS Application Programming
//! APIs reference, chapter 8. This is deliberately separate from Db2 SQL.

mod engine;
pub(crate) mod host;
pub(crate) mod parser;

pub use engine::{Column, SqlResult, SqlValue};

/// An SQL error with the IMS SQLSTATE. Numeric codes are supplied only where
/// the published IMS code reference establishes their meaning.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SqlError {
    pub code: i32,
    pub state: &'static str,
    pub message: String,
}

impl SqlError {
    pub(crate) fn new(state: &'static str, message: impl Into<String>) -> Self {
        Self {
            // -1 is a Zeus diagnostic when no IMS numeric mapping has been
            // established; never substitute a Db2 SQLCODE for an IMS code.
            code: match state {
                "42803" => -122,
                "42601" => -199,
                _ => -1,
            },
            state,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for SqlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SQLIMS {} {}: {}", self.code, self.state, self.message)
    }
}

impl std::error::Error for SqlError {}

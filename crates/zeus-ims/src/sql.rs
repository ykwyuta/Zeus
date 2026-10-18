//! IMS COBOL dynamic SQL, implemented from the IMS Application Programming
//! APIs reference, chapter 8. This is deliberately separate from Db2 SQL.

mod datetime;
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
    pub(crate) fn with_code(mut self, code: i32) -> Self {
        self.code = code;
        self
    }

    pub(crate) fn new(state: &'static str, message: impl Into<String>) -> Self {
        Self {
            // -1 is a Zeus diagnostic when no IMS numeric mapping has been
            // established; never substitute a Db2 SQLCODE for an IMS code.
            code: match state {
                "42803" => -122,
                "42601" => -199,
                "42602" => -113,
                "42603" => -10,
                "42604" => -103,
                "42612" => -84,
                "42617" => -198,
                "42622" => -107,
                "42702" => -203,
                "42704" => -204,
                "42703" => -205,
                "42802" => -117,
                "42811" => -158,
                "42895" => -301,
                "42806" => -303,
                "42821" => -408,
                "07001" => -313,
                "07003" => -518,
                "07005" => -517,
                "22001" => -404,
                "22002" => -305,
                "22003" => -413,
                "22018" => -420,
                "23502" => -407,
                "23503" => -530,
                "23504" => -531,
                "23600" => -8010,
                "23601" => -8011,
                "23602" => -8012,
                "23603" => -8013,
                "23605" => -8015,
                "23609" => -8019,
                "23610" => -8020,
                "23611" => -8021,
                "23612" => -8022,
                "23613" => -8023,
                "23614" => -8024,
                "23615" => -8025,
                "24506" => -519,
                "26501" => -514,
                "42000" => -350,
                "42201" => -8001,
                "54001" => -101,
                "54011" => -680,
                "58030" => -8004,
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

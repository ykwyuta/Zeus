//! Compiler diagnostics, modelled on IBM severities: I (0), W (4), E (8),
//! S (12), U (16). The highest severity gives the compiler's return code.

use std::fmt;

use crate::front::source::Pos;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Severe,
}

impl Severity {
    pub fn return_code(self) -> i32 {
        match self {
            Severity::Info => 0,
            Severity::Warning => 4,
            Severity::Error => 8,
            Severity::Severe => 12,
        }
    }

    fn letter(self) -> char {
        match self {
            Severity::Info => 'I',
            Severity::Warning => 'W',
            Severity::Error => 'E',
            Severity::Severe => 'S',
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub pos: Pos,
    pub severity: Severity,
    pub message: String,
    /// Source file name, filled in when reported.
    pub file: Option<String>,
}

impl Diag {
    pub fn error(pos: Pos, message: impl Into<String>) -> Diag {
        Diag {
            pos,
            severity: Severity::Severe,
            message: message.into(),
            file: None,
        }
    }

    pub fn warning(pos: Pos, message: impl Into<String>) -> Diag {
        Diag {
            pos,
            severity: Severity::Warning,
            message: message.into(),
            file: None,
        }
    }
}

impl fmt::Display for Diag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let file = self.file.as_deref().unwrap_or("");
        write!(
            f,
            "{file}:{}: {}: {}",
            self.pos.line,
            self.severity.letter(),
            self.message
        )
    }
}

impl std::error::Error for Diag {}

pub type Result<T> = std::result::Result<T, Diag>;

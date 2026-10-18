//! Compiler diagnostics, modelled on IBM severities: I (0), W (4), E (8),
//! S (12), U (16). The highest severity gives the compiler's return code.

use std::fmt;

use crate::front::source::Pos;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Severe,
    Unrecoverable,
}

impl Severity {
    pub fn return_code(self) -> i32 {
        match self {
            Severity::Info => 0,
            Severity::Warning => 4,
            Severity::Error => 8,
            Severity::Severe => 12,
            Severity::Unrecoverable => 16,
        }
    }

    fn letter(self) -> char {
        match self {
            Severity::Info => 'I',
            Severity::Warning => 'W',
            Severity::Error => 'E',
            Severity::Severe => 'S',
            Severity::Unrecoverable => 'U',
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Phase {
    #[default]
    Source,
    Options,
    Preprocess,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Diag {
    #[serde(default)]
    pub phase: Phase,
    pub pos: Pos,
    pub severity: Severity,
    pub message: String,
    /// Source file name, filled in when reported.
    pub file: Option<String>,
}

impl Diag {
    pub fn error(pos: Pos, message: impl Into<String>) -> Diag {
        Diag {
            phase: Phase::Source,
            pos,
            severity: Severity::Severe,
            message: message.into(),
            file: None,
        }
    }

    pub fn warning(pos: Pos, message: impl Into<String>) -> Diag {
        Diag {
            phase: Phase::Source,
            pos,
            severity: Severity::Warning,
            message: message.into(),
            file: None,
        }
    }

    pub fn during(mut self, phase: Phase) -> Self {
        self.phase = phase;
        self
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

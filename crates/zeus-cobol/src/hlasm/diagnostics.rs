use super::{Module, assemble, macros};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub file: String,
    pub line: usize,
    pub operation: String,
    pub severity: u8,
    pub category: String,
    pub message: String,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}: {} severity {}: {}",
            self.file,
            self.line,
            self.category,
            self.severity,
            if matches!(
                self.category.as_str(),
                "ERROR" | "MISSING_MEMBER" | "UNSUPPORTED_FORM"
            ) && !self.operation.is_empty()
            {
                format!("{}: {}", self.operation, self.message)
            } else {
                self.message.clone()
            }
        )
    }
}

impl Diagnostic {
    pub(super) fn at(
        s: &assemble::Statement,
        severity: u8,
        category: &str,
        message: String,
    ) -> Self {
        Self {
            file: s.file.clone(),
            line: s.line,
            operation: s.op.clone(),
            severity,
            category: category.into(),
            message,
        }
    }

    fn failure(path: &Path, message: String) -> Self {
        let parsed = message.split_once(": ").and_then(|(prefix, body)| {
            let (file, line) = prefix.rsplit_once(':')?;
            Some((file.to_string(), line.parse().ok()?, body.to_string()))
        });
        let (file, line, message) =
            parsed.unwrap_or_else(|| (path.display().to_string(), 0, message));
        let (operation, message) = if let Some((op, body)) = message
            .split_once(": ")
            .filter(|(op, _)| *op == op.to_ascii_uppercase() && assemble::symbol_name(op))
        {
            (op.to_string(), body.to_string())
        } else {
            (
                assemble::statements(Path::new(&file))
                    .ok()
                    .and_then(|lines| lines.into_iter().find(|s| s.line == line))
                    .map_or_else(String::new, |s| s.op),
                message,
            )
        };
        let category =
            if operation == "COPY" && message.contains("member") && message.contains("not found") {
                "MISSING_MEMBER"
            } else if message.contains("unsupported ") || message.contains("not supported") {
                "UNSUPPORTED_FORM"
            } else {
                "ERROR"
            };
        Self {
            file,
            line,
            operation,
            severity: 12,
            category: category.into(),
            message,
        }
    }
}

/// A successful module may have informational/warning diagnostics. `complete`
/// is false when parsing, expansion or assembly stopped or could not be trusted.
#[derive(Debug, Serialize, Deserialize)]
pub struct AssemblyReport {
    pub module: Option<Module>,
    pub diagnostics: Vec<Diagnostic>,
    pub complete: bool,
}

impl AssemblyReport {
    pub fn return_code(&self) -> u8 {
        self.diagnostics
            .iter()
            .map(|d| d.severity)
            .max()
            .unwrap_or(0)
    }
}

pub fn compile_report(path: &Path, dirs: &[PathBuf]) -> AssemblyReport {
    compile_report_with_options(path, dirs, &super::AssemblyOptions::default())
}

pub fn compile_report_with_options(
    path: &Path,
    dirs: &[PathBuf],
    options: &super::AssemblyOptions,
) -> AssemblyReport {
    let mut report = AssemblyReport {
        module: None,
        diagnostics: vec![],
        complete: false,
    };
    let result = super::source::statements(path, options, false, &mut report.diagnostics)
        .and_then(|input| macros::expand(input, dirs, &mut report.diagnostics, options))
        .and_then(assemble::compile_statements);
    match result {
        Ok(module) => {
            report.complete = !report
                .diagnostics
                .iter()
                .any(|d| d.category == "UNRESOLVED_OPERATION");
            if report.return_code() < 8 && report.complete {
                report.module = Some(module);
            }
        }
        Err(error) => report.diagnostics.push(Diagnostic::failure(path, error)),
    }
    report
}

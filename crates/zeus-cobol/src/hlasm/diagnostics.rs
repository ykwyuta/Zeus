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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listing: Option<super::AssemblyListing>,
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
        listing: options
            .collect_listing
            .then(super::AssemblyListing::default),
        module: None,
        diagnostics: vec![],
        complete: false,
    };
    let timer = std::time::Instant::now();
    let mut effective = options.clone();
    let mut process_statements = Vec::new();
    let mut calls = Vec::new();
    let mut sources = Vec::new();
    let mut copies = Vec::new();
    let result = super::source::statements(
        path,
        options,
        false,
        &mut report.diagnostics,
        Some(&mut sources),
    )
    .and_then(|input| {
        super::options::validate(options)?;
        let input = super::options::apply_process(
            input,
            &mut effective,
            &mut report.diagnostics,
            &mut process_statements,
        );
        macros::expand(
            input,
            dirs,
            &mut report.diagnostics,
            &effective,
            &mut calls,
            &mut sources,
            &mut copies,
        )
    })
    .and_then(|lines| {
        assemble::compile_statements(lines, report.listing.as_mut(), &mut report.diagnostics)
    });
    if let Some(listing) = &mut report.listing {
        listing.elapsed_micros = timer.elapsed().as_micros().min(u128::from(u64::MAX)) as u64;
        listing.optable = effective.optable;
        listing.macrocase = effective.macrocase;
        listing.pcontrol = effective.pcontrol;
        listing.process_statements = process_statements;
        listing.macro_calls = calls;
        listing.copy_calls = copies;
        listing.source_files = sources;
    }
    match result {
        Ok(module) => {
            report.complete = !report.diagnostics.iter().any(|d| {
                matches!(
                    d.category.as_str(),
                    "UNRESOLVED_OPERATION" | "UNSUPPORTED_OPTION"
                )
            });
            if report.return_code() < 8 && report.complete {
                report.module = Some(module);
            }
        }
        Err(error) => report.diagnostics.push(Diagnostic::failure(path, error)),
    }
    report
}

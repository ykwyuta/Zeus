//! Listing option profiles shared by invocation interfaces and source controls.
use std::collections::BTreeMap;
mod process;
pub use process::ProcessStatement;
pub(super) use process::{apply_process, scan_process};

pub(super) fn print_option(option: &str) -> Option<(&str, bool)> {
    Some(match option {
        "ON" => ("ON", true),
        "OFF" => ("ON", false),
        "GEN" | "DATA" | "MCALL" | "MSOURCE" | "UHEAD" => (option, true),
        "NOGEN" | "NODATA" | "NOMCALL" | "NOMSOURCE" | "NOUHEAD" => (&option[2..], false),
        _ => return None,
    })
}

/// PCONTROL's named PRINT overrides. Empty/NOPCONTROL honors source PRINT.
pub fn parse_pcontrol(text: &str) -> Result<BTreeMap<String, bool>, String> {
    let text = text.to_ascii_uppercase();
    if text.is_empty() || text == "NOPCONTROL" {
        return Ok(BTreeMap::new());
    }
    let mut result = BTreeMap::new();
    for arg in super::assemble::operands(&text) {
        let (name, value) =
            print_option(&arg).ok_or_else(|| format!("unsupported PCONTROL option {arg}"))?;
        if result
            .insert(name.to_string(), value)
            .is_some_and(|previous| previous != value)
        {
            return Err(format!("conflicting PCONTROL option {arg}"));
        }
    }
    Ok(result)
}

/// Implemented instruction-name union, with optional suppression of the table list.
pub fn parse_optable(text: &str) -> Result<String, String> {
    let text = text.to_ascii_uppercase();
    let args = super::assemble::operands(&text);
    if args.len() > 2 || args.get(1).is_some_and(|arg| arg != "NOLIST") {
        return Err(
            "OPTABLE accepts UNI/ZEUS with optional NOLIST; table LIST output is not yet supported"
                .into(),
        );
    }
    match args.first().map(String::as_str) {
        Some(name @ ("UNI" | "ZEUS")) => Ok(name.into()),
        _ => Err(format!(
            "unsupported OPTABLE profile {text}; implemented source tables are UNI and ZEUS"
        )),
    }
}

pub(super) fn validate(options: &super::AssemblyOptions) -> Result<(), String> {
    for key in options.pcontrol.keys() {
        if !matches!(
            key.as_str(),
            "ON" | "GEN" | "DATA" | "MCALL" | "MSOURCE" | "UHEAD"
        ) {
            return Err(format!("invalid PCONTROL key {key}"));
        }
    }
    if let Some(table) = &options.optable {
        parse_optable(table)?;
    }
    Ok(())
}

pub(super) fn macrocase_option(text: &str) -> Option<bool> {
    match text {
        "MACROCASE" | "MC" => Some(true),
        "NOMACROCASE" | "NOMC" => Some(false),
        _ => None,
    }
}
/// Implemented COMPAT profile; unsupported compatibility modes are errors.
pub fn parse_compat(text: &str) -> Result<bool, String> {
    let mut value = None;
    for option in super::assemble::operands(&text.to_ascii_uppercase()) {
        let next = macrocase_option(&option)
            .ok_or_else(|| format!("unsupported COMPAT option {option}"))?;
        if value.is_some_and(|old| old != next) {
            return Err("conflicting COMPAT MACROCASE options".into());
        }
        value = Some(next);
    }
    value.ok_or_else(|| "COMPAT requires at least one supported suboption".into())
}

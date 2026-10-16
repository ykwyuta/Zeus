//! Source compiler options: implement supported settings, diagnose the rest.
use super::{FrontOptions, Trunc};
use crate::diag::{Diag, Result};

pub(super) fn parse(text: &str, defaults: &FrontOptions) -> Result<(bool, bool, Trunc)> {
    let mut cics = defaults.cics;
    let mut dbcs = defaults.nsymbol_dbcs;
    let mut trunc = defaults.trunc;
    let mut rest = text.trim();
    while !rest.is_empty() {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
        if rest.is_empty() {
            break;
        }
        let end = rest
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
            .unwrap_or(rest.len());
        let name = &rest[..end];
        if name.is_empty() {
            return Err(Diag::error(
                Default::default(),
                "invalid CBL/PROCESS option",
            ));
        }
        rest = rest[end..].trim_start();
        let arg = if rest.starts_with('(') {
            let end = rest
                .find(')')
                .ok_or_else(|| Diag::error(Default::default(), "unclosed compiler option"))?;
            let arg = rest[1..end].trim();
            rest = &rest[end + 1..];
            Some(arg)
        } else {
            None
        };
        match (name, arg) {
            ("CICS", None) => cics = true,
            ("CICS", Some(arg)) if cics_options(arg) => cics = true,
            ("NOCICS", None) => cics = false,
            // SQL is translated by the integrated EXEC SQL front end. Literal
            // CALLs already reuse the program bound by the Zeus module loader.
            // Code is immutable and each Machine owns its data (RENT).
            ("SQL" | "NODYNAM" | "RENT", None) => {}
            ("FLAG", Some("I" | "W" | "E" | "S" | "U")) => {}
            ("NSYMBOL" | "NS", Some("DBCS")) => dbcs = true,
            ("NSYMBOL" | "NS", Some("NATIONAL" | "NAT")) => dbcs = false,
            ("TRUNC", Some("STD")) => trunc = Trunc::Std,
            ("TRUNC", Some("OPT")) => trunc = Trunc::Opt,
            ("TRUNC", Some("BIN")) => trunc = Trunc::Bin,
            // Listing-only options do not change the executable semantics.
            (
                "LIST" | "NOLIST" | "SOURCE" | "NOSOURCE" | "MAP" | "NOMAP" | "OFFSET" | "NOOFFSET"
                | "XREF" | "NOXREF" | "VBREF" | "NOVBREF",
                None,
            ) => {}
            _ => {
                return Err(Diag::error(
                    Default::default(),
                    format!(
                        "unsupported or invalid CBL/PROCESS option: {name}{}",
                        arg.map_or(String::new(), |a| format!("({a})"))
                    ),
                ));
            }
        }
    }
    Ok((cics, dbcs, trunc))
}

fn cics_options(arg: &str) -> bool {
    let Some(options) = arg.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) else {
        return false;
    };
    !options.is_empty()
        && options
            .split(',')
            .all(|option| matches!(option.trim(), "SP" | "EDF" | "DLI"))
}

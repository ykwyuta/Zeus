//! Initial primary-input *PROCESS options, kept out of macro statement numbering.
//! Precedence and restricted options follow the public HLASM option tables.
use crate::hlasm::{
    AssemblyOptions, Diagnostic,
    assemble::{Statement, operands},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStatement {
    pub file: String,
    pub line: usize,
    pub options: String,
    pub override_options: bool,
}

pub(in crate::hlasm) fn scan_process(area: &str) -> Option<String> {
    if !area.get(..8)?.eq_ignore_ascii_case("*PROCESS") {
        return None;
    }
    let tail = &area[8..];
    if !tail.is_empty() && !tail.starts_with(char::is_whitespace) {
        return None;
    }
    let tail = tail.trim_start();
    let mut quoted = false;
    let end = tail
        .char_indices()
        .find_map(|(at, c)| {
            if c == '\'' {
                quoted = !quoted;
            }
            (c.is_whitespace() && !quoted).then_some(at)
        })
        .unwrap_or(tail.len());
    Some(tail[..end].into())
}
fn message(
    s: &Statement,
    diagnostics: &mut Vec<Diagnostic>,
    severity: u8,
    category: &str,
    message: impl Into<String>,
) {
    diagnostics.push(Diagnostic {
        file: s.file.clone(),
        line: s.line,
        operation: "*PROCESS".into(),
        severity,
        category: category.into(),
        message: message.into(),
    });
}
fn balanced(text: &str) -> bool {
    let (mut depth, mut quote) = (0usize, false);
    for c in text.chars() {
        if c == '\'' {
            quote = !quote;
        }
        if !quote {
            match c {
                '(' => depth += 1,
                ')' => {
                    if depth == 0 {
                        return false;
                    }
                    depth -= 1;
                }
                _ => {}
            }
        }
    }
    depth == 0 && !quote
}
fn profile(
    text: &str,
    s: &Statement,
    diagnostics: &mut Vec<Diagnostic>,
) -> Result<BTreeMap<String, bool>, String> {
    let mut result = BTreeMap::new();
    for arg in operands(text) {
        let (key, value) = super::print_option(&arg)
            .ok_or_else(|| format!("unsupported PCONTROL option {arg}"))?;
        if result
            .insert(key.to_string(), value)
            .is_some_and(|old| old != value)
        {
            message(
                s,
                diagnostics,
                4,
                "OPTION_CONFLICT",
                format!("conflicting PCONTROL {key}; the last value is used"),
            );
        }
    }
    Ok(result)
}
fn restricted(name: &str) -> bool {
    let root = name.split('(').next().unwrap_or(name);
    let root = root.strip_prefix("NO").unwrap_or(root);
    matches!(
        root,
        "ADATA"
            | "LANGUAGE"
            | "SYSPARM"
            | "ASA"
            | "LINECOUNT"
            | "TERM"
            | "DECK"
            | "LIST"
            | "TRANSLATE"
            | "EXIT"
            | "OBJECT"
            | "XOBJECT"
            | "GOFF"
            | "SIZE"
    )
}

struct Selection<T> {
    value: T,
    precedence: u8,
}
impl<T: PartialEq> Selection<T> {
    fn apply(
        &mut self,
        value: T,
        priority: u8,
        name: &str,
        s: &Statement,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        let different = value != self.value;
        if priority <= self.precedence {
            if different && (self.precedence < 6 || priority == 2) {
                message(
                    s,
                    diagnostics,
                    4,
                    "OPTION_CONFLICT",
                    format!("{name} replaces a conflicting option in effect"),
                );
            }
            self.value = value;
            self.precedence = priority;
        } else if different && self.precedence != 2 {
            message(
                s,
                diagnostics,
                4,
                "OPTION_IGNORED",
                format!("source {name} is ignored in favor of the invocation option"),
            );
        }
    }
}
pub(in crate::hlasm) fn apply_process(
    input: Vec<Statement>,
    options: &mut AssemblyOptions,
    diagnostics: &mut Vec<Diagnostic>,
    records: &mut Vec<ProcessStatement>,
) -> Vec<Statement> {
    // Lower numbers have higher precedence. Invocation is 4; ordinary source
    // is 5; source OVERRIDE is 2. Empty invocation can explicitly mean NOPCONTROL.
    let mut pcontrol = Selection {
        value: options.pcontrol.clone(),
        precedence: if options.pcontrol_specified || !options.pcontrol.is_empty() {
            4
        } else {
            6
        },
    };
    let mut optable = Selection {
        value: super::parse_optable(options.optable.as_deref().unwrap_or("ZEUS"))
            .expect("invocation options were validated before applying *PROCESS"),
        precedence: if options.optable.is_some() { 4 } else { 6 },
    };
    let mut macrocase = Selection {
        value: options.macrocase.unwrap_or(false),
        precedence: if options.macrocase.is_some() { 4 } else { 6 },
    };
    let mut result = Vec::with_capacity(input.len());
    for s in input {
        if s.op != "*PROCESS" {
            result.push(s);
            continue;
        }
        let text = s.args.to_ascii_uppercase();
        let override_options = text.starts_with("OVERRIDE(");
        records.push(ProcessStatement {
            file: s.file.clone(),
            line: s.line,
            options: s.args.clone(),
            override_options,
        });
        if !balanced(&text) {
            message(
                &s,
                diagnostics,
                12,
                "ASSEMBLER_OPTION",
                "unbalanced *PROCESS option parentheses or apostrophes",
            );
            continue;
        }
        let args = operands(&text);
        let text = if override_options {
            if args.len() != 1 || !text.ends_with(')') {
                message(
                    &s,
                    diagnostics,
                    12,
                    "ASSEMBLER_OPTION",
                    "OVERRIDE must be the first and only *PROCESS keyword",
                );
                continue;
            }
            &text[9..text.len() - 1]
        } else {
            &text
        };
        for arg in operands(text) {
            if restricted(&arg) {
                message(
                    &s,
                    diagnostics,
                    12,
                    "ASSEMBLER_OPTION",
                    format!("option {arg} is not permitted on *PROCESS"),
                );
                continue;
            }
            let value = if arg == "NOPCONTROL" {
                Some(Ok(BTreeMap::new()))
            } else {
                arg.strip_prefix("PCONTROL(")
                    .and_then(|v| v.strip_suffix(')'))
                    .map(|v| profile(v, &s, diagnostics))
            };
            let priority = if override_options { 2 } else { 5 };
            if let Some(value) = value {
                match value {
                    Ok(value) => pcontrol.apply(value, priority, "PCONTROL", &s, diagnostics),
                    Err(error) => message(&s, diagnostics, 12, "ASSEMBLER_OPTION", error),
                }
            } else if let Some(table) = arg
                .strip_prefix("OPTABLE(")
                .or_else(|| arg.strip_prefix("OP("))
                .and_then(|v| v.strip_suffix(')'))
            {
                match super::parse_optable(table) {
                    Ok(value) => optable.apply(value, priority, "OPTABLE", &s, diagnostics),
                    Err(error) => message(&s, diagnostics, 12, "UNSUPPORTED_OPTION", error),
                }
            } else if let Some(compat) = arg
                .strip_prefix("COMPAT(")
                .or_else(|| arg.strip_prefix("CPAT("))
                .and_then(|v| v.strip_suffix(')'))
            {
                let mut value = None;
                for suboption in operands(compat) {
                    match super::macrocase_option(&suboption) {
                        Some(next) => {
                            if value.is_some_and(|old| old != next) {
                                message(
                                    &s,
                                    diagnostics,
                                    4,
                                    "OPTION_CONFLICT",
                                    "conflicting COMPAT MACROCASE options; the last value is used",
                                );
                            }
                            value = Some(next);
                        }
                        None => message(
                            &s,
                            diagnostics,
                            12,
                            "UNSUPPORTED_OPTION",
                            format!("unsupported COMPAT option {suboption}"),
                        ),
                    }
                }
                if let Some(value) = value {
                    macrocase.apply(value, priority, "COMPAT(MACROCASE)", &s, diagnostics);
                }
            } else if arg.starts_with("OVERRIDE(") {
                message(
                    &s,
                    diagnostics,
                    12,
                    "ASSEMBLER_OPTION",
                    "OVERRIDE must be the first and only *PROCESS keyword",
                );
            } else {
                message(
                    &s,
                    diagnostics,
                    12,
                    "UNSUPPORTED_OPTION",
                    format!("unsupported *PROCESS option {arg}"),
                );
            }
        }
    }
    if macrocase.precedence < 6 {
        options.macrocase = Some(macrocase.value);
    }
    options.pcontrol = pcontrol.value;
    if optable.precedence < 6 {
        options.optable = Some(optable.value);
    }
    result
}

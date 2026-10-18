//! Declarative suite manifest (`manifest.toml`) for ZCVS and compatible suites.
//!
//! When a suite directory contains `manifest.toml`, `zeus-ccvs` reads the
//! module list and per-program execution attributes (`ccsid`, `parm`,
//! `expect_exit_code`, `jcl`, `expect = "compile-error"`,
//! `diagnostic_contains`) from it instead of the hardcoded CCVS85 table.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ExpectKind {
    #[default]
    Run,
    CompileOnly,
    CompileError,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProgramConfig {
    pub ccsid: Option<u32>,
    pub parm: Option<String>,
    pub expect_exit_code: Option<i32>,
    pub jcl: Option<String>,
    pub expect: ExpectKind,
    pub diagnostic_contains: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    pub required_modules: Vec<String>,
    pub optional_modules: Vec<String>,
    pub programs: BTreeMap<String, ProgramConfig>,
}

impl Manifest {
    /// Loads `manifest.toml` from `suite_dir` if present.
    pub fn load(suite_dir: &Path) -> Result<Option<Self>> {
        let path = suite_dir.join("manifest.toml");
        if !path.is_file() {
            return Ok(None);
        }
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Ok(Some(
            Self::parse(&text).with_context(|| format!("parsing {}", path.display()))?,
        ))
    }

    /// All modules in canonical order (`required_modules` then `optional_modules`).
    pub fn all_modules(&self) -> Vec<String> {
        self.required_modules
            .iter()
            .chain(self.optional_modules.iter())
            .cloned()
            .collect()
    }

    pub fn parse(text: &str) -> Result<Self> {
        let mut m = Manifest::default();
        let mut section = Section::Root;
        for (lineno, raw) in text.lines().enumerate() {
            let line = strip_comment(raw).trim();
            if line.is_empty() {
                continue;
            }
            if let Some(header) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                let header = header.trim();
                if header == "suite" {
                    section = Section::Suite;
                } else if let Some(prog) = header.strip_prefix("programs.") {
                    let prog = prog.trim().trim_matches('"').to_string();
                    if prog.is_empty() {
                        bail!("line {}: empty program name in [programs.*]", lineno + 1);
                    }
                    section = Section::Program(prog);
                } else {
                    bail!("line {}: unknown section [{header}]", lineno + 1);
                }
                continue;
            }
            let Some((key, val)) = line.split_once('=') else {
                bail!("line {}: expected key = value", lineno + 1);
            };
            let key = key.trim();
            let val = val.trim();
            match &section {
                Section::Root => bail!("line {}: key '{key}' outside of section", lineno + 1),
                Section::Suite => match key {
                    "name" => m.name = parse_str(val, lineno + 1)?,
                    "required_modules" => m.required_modules = parse_str_array(val, lineno + 1)?,
                    "optional_modules" => m.optional_modules = parse_str_array(val, lineno + 1)?,
                    _ => bail!("line {}: unknown [suite] key '{key}'", lineno + 1),
                },
                Section::Program(prog) => {
                    let cfg = m.programs.entry(prog.clone()).or_default();
                    match key {
                        "ccsid" => {
                            cfg.ccsid =
                                Some(val.parse().with_context(|| {
                                    format!("line {}: invalid ccsid", lineno + 1)
                                })?);
                        }
                        "parm" => cfg.parm = Some(parse_str(val, lineno + 1)?),
                        "expect_exit_code" => {
                            cfg.expect_exit_code = Some(val.parse().with_context(|| {
                                format!("line {}: invalid expect_exit_code", lineno + 1)
                            })?);
                        }
                        "jcl" => cfg.jcl = Some(parse_str(val, lineno + 1)?),
                        "expect" => {
                            let s = parse_str(val, lineno + 1)?;
                            cfg.expect = match s.as_str() {
                                "run" => ExpectKind::Run,
                                "compile-only" => ExpectKind::CompileOnly,
                                "compile-error" => ExpectKind::CompileError,
                                _ => bail!("line {}: unknown expect value '{s}'", lineno + 1),
                            };
                        }
                        "diagnostic_contains" => {
                            cfg.diagnostic_contains = Some(parse_str(val, lineno + 1)?);
                        }
                        _ => bail!("line {}: unknown [programs.{prog}] key '{key}'", lineno + 1),
                    }
                }
            }
        }
        Ok(m)
    }
}

enum Section {
    Root,
    Suite,
    Program(String),
}

fn strip_comment(line: &str) -> &str {
    let mut in_quotes = false;
    let mut escaped = false;
    for (i, c) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' && in_quotes {
            escaped = true;
            continue;
        }
        if c == '"' {
            in_quotes = !in_quotes;
        } else if c == '#' && !in_quotes {
            return &line[..i];
        }
    }
    line
}

fn parse_str(val: &str, line: usize) -> Result<String> {
    let Some(inner) = val.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
        bail!("line {line}: expected quoted string, got {val}");
    };
    Ok(inner.replace("\\\"", "\"").replace("\\\\", "\\"))
}

fn parse_str_array(val: &str, line: usize) -> Result<Vec<String>> {
    let Some(inner) = val.strip_prefix('[').and_then(|s| s.strip_suffix(']')) else {
        bail!("line {line}: expected array [...], got {val}");
    };
    let inner = inner.trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    inner
        .split(',')
        .map(|item| parse_str(item.trim(), line))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_zcvs_manifest() {
        let toml = r#"
[suite]
name = "ZCVS"
required_modules = ["ZD", "ZA", "ZN", "ZP", "ZF", "ZS", "ZE"]
optional_modules = ["ZX"]

[programs.ZN106M]
ccsid = 939

[programs.ZS103M]
parm = "NIGHTLY,20261016"
expect_exit_code = 4

[programs.ZF201M]
jcl = "ZF201M.JCL"

[programs.ZE101N]
expect = "compile-error"
diagnostic_contains = "END PROGRAM"
"#;
        let m = Manifest::parse(toml).unwrap();
        assert_eq!(m.name, "ZCVS");
        assert_eq!(m.required_modules.len(), 7);
        assert_eq!(m.optional_modules, vec!["ZX"]);
        assert_eq!(m.programs["ZN106M"].ccsid, Some(939));
        assert_eq!(
            m.programs["ZS103M"].parm.as_deref(),
            Some("NIGHTLY,20261016")
        );
        assert_eq!(m.programs["ZS103M"].expect_exit_code, Some(4));
        assert_eq!(m.programs["ZF201M"].jcl.as_deref(), Some("ZF201M.JCL"));
        assert_eq!(m.programs["ZE101N"].expect, ExpectKind::CompileError);
        assert_eq!(
            m.programs["ZE101N"].diagnostic_contains.as_deref(),
            Some("END PROGRAM")
        );
    }
}

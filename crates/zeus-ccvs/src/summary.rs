//! Summaries of suite results and comparison against a baseline.

use std::collections::BTreeMap;
use std::fmt::Write;

use crate::report::Counts;
use crate::runner::{MODULES, ModuleResult, Outcome, SuiteResult, order};

/// Modules that must pass (docs/proposal/0003 section 5.2).
pub const REQUIRED: &[&str] = &["NC", "SM", "IC", "SQ", "RL", "IX", "ST", "IF"];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ModuleSummary {
    pub programs: u32,
    pub ok: u32,
    pub compile_errors: u32,
    pub execute_errors: u32,
    pub timeouts: u32,
    pub counts: Counts,
}

impl ModuleSummary {
    pub fn of(m: &ModuleResult) -> Self {
        let mut s = ModuleSummary::default();
        for p in &m.programs {
            s.programs += 1;
            s.counts += p.outcome.counts();
            if p.outcome.ok() {
                s.ok += 1;
            }
            match p.outcome {
                Outcome::CompileError => s.compile_errors += 1,
                Outcome::ExecuteError { .. } => s.execute_errors += 1,
                Outcome::Timeout => s.timeouts += 1,
                _ => {}
            }
        }
        s
    }

    /// Passed per section 5.2: every program ran (or compiled) and no test
    /// failed. Visual-inspection items are tracked separately.
    pub fn passed(&self) -> bool {
        self.programs > 0 && self.ok == self.programs && self.counts.fail == 0
    }
}

pub fn summarize(r: &SuiteResult) -> BTreeMap<usize, (String, ModuleSummary)> {
    r.modules
        .iter()
        .map(|m| (order(&m.module), (m.module.clone(), ModuleSummary::of(m))))
        .collect()
}

fn status(s: Option<&ModuleSummary>, module: &str) -> &'static str {
    match s {
        None => "-",
        Some(s) if s.passed() => "PASS",
        Some(_) if REQUIRED.contains(&module) => "FAIL",
        Some(_) => "fail (optional)",
    }
}

/// GnuCOBOL-style per-program report of one result set.
pub fn program_report(r: &SuiteResult) -> String {
    let mut out = String::new();
    for m in &r.modules {
        let _ = writeln!(out, "Module {}", m.module);
        let _ = writeln!(out, "Filename    total pass fail deleted inspect");
        let _ = writeln!(out, "--------    ----- ---- ---- ------- -------");
        for p in &m.programs {
            let c = p.outcome.counts();
            let line = match &p.outcome {
                Outcome::Ran { .. } | Outcome::Compiled => format!(
                    "{:5} {:4} {:4} {:7} {:7} {}",
                    c.total,
                    c.pass,
                    c.fail,
                    c.deleted,
                    c.inspect,
                    if p.outcome.ok() { "OK" } else { "" }
                ),
                Outcome::CompileError => "***** compile error *****".to_string(),
                Outcome::ExecuteError { code } => format!(
                    "***** execute error {} *****",
                    code.map_or("signal".to_string(), |c| c.to_string())
                ),
                Outcome::Timeout => "***** timeout *****".to_string(),
            };
            let _ = writeln!(out, "{:<11} {}", p.program, line.trim_end());
        }
        let _ = writeln!(out);
    }
    out
}

/// Markdown table of module summaries; with a baseline, both are shown side
/// by side.
pub fn markdown(current: &SuiteResult, baseline: Option<&SuiteResult>) -> String {
    let cur = summarize(current);
    let base = baseline.map(summarize);
    let mut out = String::new();
    let label = |r: &SuiteResult| format!("{} ({})", r.toolchain, r.version);
    let _ = writeln!(out, "### CCVS85 results: {}\n", label(current));
    if let Some(b) = baseline {
        let _ = writeln!(out, "Baseline: {}\n", label(b));
        let _ = writeln!(
            out,
            "| Module | Required | Status | Programs OK | Tests passed | Failed | Compile err | Exec err | Baseline status | Baseline tests passed |"
        );
        let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|---|");
    } else {
        let _ = writeln!(
            out,
            "| Module | Required | Status | Programs OK | Tests passed | Failed | Compile err | Exec err |"
        );
        let _ = writeln!(out, "|---|---|---|---|---|---|---|---|");
    }
    let mut total = ModuleSummary::default();
    for (idx, module) in MODULES.iter().enumerate() {
        let c = cur.get(&idx).map(|(_, s)| s);
        let b = base.as_ref().and_then(|b| b.get(&idx)).map(|(_, s)| s);
        if c.is_none() && b.is_none() {
            continue;
        }
        let req = if REQUIRED.contains(module) { "yes" } else { "" };
        let cells = match c {
            Some(s) => {
                total.programs += s.programs;
                total.ok += s.ok;
                total.compile_errors += s.compile_errors;
                total.execute_errors += s.execute_errors;
                total.counts += s.counts;
                format!(
                    "{}/{} | {}/{} | {} | {} | {}",
                    s.ok,
                    s.programs,
                    s.counts.pass,
                    s.counts.total,
                    s.counts.fail,
                    s.compile_errors,
                    s.execute_errors
                )
            }
            None => "- | - | - | - | -".to_string(),
        };
        let _ = write!(
            out,
            "| {module} | {req} | {} | {cells} |",
            status(c, module)
        );
        if baseline.is_some() {
            let bt = b.map_or("-".to_string(), |s| {
                format!("{}/{}", s.counts.pass, s.counts.total)
            });
            let _ = write!(out, " {} | {bt} |", status(b, module));
        }
        out.push('\n');
    }
    let required_passed = REQUIRED
        .iter()
        .filter(|m| cur.values().any(|(name, s)| name == *m && s.passed()))
        .count();
    let _ = writeln!(
        out,
        "\nRequired modules passed: **{required_passed}/{}** - programs OK {}/{}, tests passed {}/{}",
        REQUIRED.len(),
        total.ok,
        total.programs,
        total.counts.pass,
        total.counts.total
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runner::ProgramResult;

    fn result(toolchain: &str, outcome: Outcome) -> SuiteResult {
        SuiteResult {
            toolchain: toolchain.to_string(),
            version: "1".to_string(),
            modules: vec![ModuleResult {
                module: "NC".to_string(),
                programs: vec![ProgramResult {
                    program: "NC101A".to_string(),
                    outcome,
                    seconds: 0.1,
                }],
            }],
        }
    }

    #[test]
    fn module_pass_and_fail() {
        let ok = result(
            "gnucobol",
            Outcome::Ran {
                counts: Counts {
                    total: 3,
                    pass: 3,
                    ..Counts::default()
                },
            },
        );
        let bad = result("zeus", Outcome::CompileError);
        assert!(ModuleSummary::of(&ok.modules[0]).passed());
        assert!(!ModuleSummary::of(&bad.modules[0]).passed());
        let md = markdown(&bad, Some(&ok));
        assert!(md.contains("| NC | yes | FAIL | 0/1 | 0/0 | 0 | 1 | 0 | PASS | 3/3 |"));
        assert!(md.contains("Required modules passed: **0/8**"));
        assert!(program_report(&bad).contains("NC101A      ***** compile error *****"));
    }
}

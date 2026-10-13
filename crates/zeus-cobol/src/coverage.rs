//! Code coverage of COBOL and PL/I programs run by the interpreters.
//!
//! Coverage is recorded when ZEUS_COVERAGE names a directory (or a
//! program sets one with [`set_dir`], as `zcobc run --coverage DIR`
//! does). Each run unit counts how often each statement ran and which
//! way each conditional branch went; when it ends (a program run, a job
//! step, a CICS task) the counts are added to the process's totals,
//! which are written to `DIR/zeus-<pid>.cov.json`. `zcov` merges the files of a directory
//! into a report: a text summary, LCOV (`genhtml`, IDE plug-ins) and an
//! HTML page per source file.
//!
//! What is counted:
//! * lines: the source lines with executable statements, with the number
//!   of times a statement on the line started (COBOL: the statement's
//!   first instruction; PL/I: the statement);
//! * branches: each conditional jump (IF, EVALUATE WHEN, PERFORM UNTIL,
//!   AT END, ... in COBOL; IF in PL/I) with the number of times each way
//!   was taken;
//! * functions: COBOL paragraphs and sections, PL/I procedures, with the
//!   number of times they were entered.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

/// The coverage of one program (a COBOL program or a PL/I module).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramCov {
    /// "COBOL" or "PL/I".
    pub language: String,
    pub name: String,
    /// Source files (absolute paths), indexed by the `file` fields.
    pub sources: Vec<String>,
    pub lines: Vec<LineCount>,
    pub branches: Vec<BranchCount>,
    pub functions: Vec<FnCount>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineCount {
    pub file: u32,
    pub line: u32,
    pub count: u64,
}

/// A conditional branch: `taken` is how often it jumped, `not_taken` how
/// often it went on to the next statement. `id` tells branches on one
/// line apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchCount {
    pub file: u32,
    pub line: u32,
    pub id: u32,
    pub taken: u64,
    pub not_taken: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FnCount {
    pub name: String,
    pub file: u32,
    pub line: u32,
    pub count: u64,
}

impl ProgramCov {
    fn key(&self) -> (String, String, Option<String>) {
        (
            self.language.clone(),
            self.name.clone(),
            self.sources.first().cloned(),
        )
    }

    /// Adds the counts of another run of the same program.
    pub fn merge(&mut self, o: &ProgramCov) {
        let mut lines: BTreeMap<(u32, u32), u64> = BTreeMap::new();
        for l in self.lines.iter().chain(&o.lines) {
            *lines.entry((l.file, l.line)).or_default() += l.count;
        }
        self.lines = lines
            .into_iter()
            .map(|((file, line), count)| LineCount { file, line, count })
            .collect();
        let mut br: BTreeMap<(u32, u32, u32), (u64, u64)> = BTreeMap::new();
        for b in self.branches.iter().chain(&o.branches) {
            let e = br.entry((b.file, b.line, b.id)).or_default();
            e.0 += b.taken;
            e.1 += b.not_taken;
        }
        self.branches = br
            .into_iter()
            .map(|((file, line, id), (taken, not_taken))| BranchCount {
                file,
                line,
                id,
                taken,
                not_taken,
            })
            .collect();
        for f in &o.functions {
            match self.functions.iter_mut().find(|x| x.name == f.name) {
                Some(x) => x.count += f.count,
                None => self.functions.push(f.clone()),
            }
        }
    }
}

/// The directory to write to (None: coverage off), and the process's
/// totals.
static STATE: Mutex<Option<(PathBuf, Vec<ProgramCov>)>> = Mutex::new(None);
static INIT: std::sync::Once = std::sync::Once::new();

fn init() {
    INIT.call_once(|| {
        if let Some(d) = std::env::var_os("ZEUS_COVERAGE").filter(|d| !d.is_empty()) {
            *STATE.lock().unwrap_or_else(|e| e.into_inner()) = Some((PathBuf::from(d), Vec::new()));
        }
    });
}

/// Whether coverage is being recorded.
pub fn enabled() -> bool {
    init();
    STATE.lock().unwrap_or_else(|e| e.into_inner()).is_some()
}

/// Records coverage into `dir` (in addition to ZEUS_COVERAGE: this
/// replaces its directory).
pub fn set_dir(dir: &Path) {
    init();
    let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let programs = st.take().map(|(_, p)| p).unwrap_or_default();
    *st = Some((dir.to_path_buf(), programs));
}

/// Adds a run unit's counts to the process's totals, and writes them
/// (so that servers that never end, such as zcics, leave their counts
/// too).
pub fn add(p: ProgramCov) {
    init();
    {
        let mut st = STATE.lock().unwrap_or_else(|e| e.into_inner());
        let Some((_, programs)) = st.as_mut() else {
            return;
        };
        match programs.iter_mut().find(|x| x.key() == p.key()) {
            Some(x) => x.merge(&p),
            None => programs.push(p),
        }
    }
    if let Err(e) = save() {
        eprintln!("zeus: coverage: {e}");
    }
}

/// Writes the process's totals to `DIR/zeus-<pid>.cov.json` (the file
/// is rewritten with all counts so far); its path, None when coverage is
/// off or nothing ran.
pub fn save() -> std::io::Result<Option<PathBuf>> {
    init();
    let st = STATE.lock().unwrap_or_else(|e| e.into_inner());
    let Some((dir, programs)) = st.as_ref() else {
        return Ok(None);
    };
    if programs.is_empty() {
        return Ok(None);
    }
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("zeus-{}.cov.json", std::process::id()));
    let data = serde_json::to_vec(&CovFile {
        programs: programs.clone(),
    })
    .map_err(std::io::Error::other)?;
    std::fs::write(&path, data)?;
    Ok(Some(path))
}

#[derive(Serialize, Deserialize)]
struct CovFile {
    programs: Vec<ProgramCov>,
}

/// The coverage files of a directory (`*.cov.json`), merged.
pub fn load_dir(dir: &Path) -> std::io::Result<Vec<ProgramCov>> {
    let mut out: Vec<ProgramCov> = Vec::new();
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.to_string_lossy().ends_with(".cov.json"))
        .collect();
    files.sort();
    for f in files {
        let data = std::fs::read(&f)?;
        let file: CovFile = serde_json::from_slice(&data)
            .map_err(|e| std::io::Error::other(format!("{}: {e}", f.display())))?;
        for p in file.programs {
            match out.iter_mut().find(|x| x.key() == p.key()) {
                Some(x) => x.merge(&p),
                None => out.push(p),
            }
        }
    }
    out.sort_by_key(|a| a.key());
    Ok(out)
}

/// Covered and total counts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Ratio {
    pub hit: usize,
    pub total: usize,
}

impl Ratio {
    pub fn percent(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            self.hit as f64 * 100.0 / self.total as f64
        }
    }

    fn add(&mut self, o: Ratio) {
        self.hit += o.hit;
        self.total += o.total;
    }
}

impl std::fmt::Display for Ratio {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:6.1}% {:>6}/{:<6}",
            self.percent(),
            self.hit,
            self.total
        )
    }
}

/// Lines, branches (each way counts) and functions covered.
pub fn summary(p: &ProgramCov) -> (Ratio, Ratio, Ratio) {
    let lines = Ratio {
        hit: p.lines.iter().filter(|l| l.count > 0).count(),
        total: p.lines.len(),
    };
    let branches = Ratio {
        hit: p
            .branches
            .iter()
            .map(|b| usize::from(b.taken > 0) + usize::from(b.not_taken > 0))
            .sum(),
        total: 2 * p.branches.len(),
    };
    let functions = Ratio {
        hit: p.functions.iter().filter(|f| f.count > 0).count(),
        total: p.functions.len(),
    };
    (lines, branches, functions)
}

/// The text report: a line per program and the totals; with `detail`,
/// the lines never run and the branches never taken one way.
pub fn text_report(programs: &[ProgramCov], detail: bool) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "{:<6} {:<30} {:<22} {:<22} {:<22}",
        "LANG", "PROGRAM", "LINES", "BRANCHES", "PARAGRAPHS/PROCS"
    );
    let mut total = (Ratio::default(), Ratio::default(), Ratio::default());
    for p in programs {
        let (l, b, f) = summary(p);
        let _ = writeln!(s, "{:<6} {:<30} {l} {b} {f}", p.language, p.name);
        total.0.add(l);
        total.1.add(b);
        total.2.add(f);
        if detail {
            let file = |i: u32| {
                p.sources
                    .get(i as usize)
                    .map(|f| {
                        Path::new(f)
                            .file_name()
                            .map(|n| n.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    })
                    .unwrap_or_default()
            };
            for l in p.lines.iter().filter(|l| l.count == 0) {
                let _ = writeln!(s, "    not run       {}:{}", file(l.file), l.line);
            }
            for b in &p.branches {
                let way = match (b.taken > 0, b.not_taken > 0) {
                    (true, true) => continue,
                    (false, false) => "never reached",
                    (false, true) => "never taken",
                    (true, false) => "always taken",
                };
                let _ = writeln!(s, "    branch {way:<14} {}:{}", file(b.file), b.line);
            }
            for f in p.functions.iter().filter(|f| f.count == 0) {
                let _ = writeln!(
                    s,
                    "    not entered   {} ({}:{})",
                    f.name,
                    file(f.file),
                    f.line
                );
            }
        }
    }
    let _ = writeln!(
        s,
        "{:<6} {:<30} {} {} {}",
        "", "TOTAL", total.0, total.1, total.2
    );
    s
}

/// LCOV tracefile (one record per source file).
pub fn lcov(programs: &[ProgramCov]) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    for p in programs {
        for (fi, path) in p.sources.iter().enumerate() {
            let fi = fi as u32;
            let lines: Vec<_> = p.lines.iter().filter(|l| l.file == fi).collect();
            let fns: Vec<_> = p.functions.iter().filter(|f| f.file == fi).collect();
            let brs: Vec<_> = p.branches.iter().filter(|b| b.file == fi).collect();
            if lines.is_empty() && fns.is_empty() && brs.is_empty() {
                continue;
            }
            let _ = writeln!(s, "TN:{}", p.name);
            let _ = writeln!(s, "SF:{path}");
            for f in &fns {
                let _ = writeln!(s, "FN:{},{}", f.line, f.name);
            }
            for f in &fns {
                let _ = writeln!(s, "FNDA:{},{}", f.count, f.name);
            }
            let _ = writeln!(s, "FNF:{}", fns.len());
            let _ = writeln!(s, "FNH:{}", fns.iter().filter(|f| f.count > 0).count());
            for b in &brs {
                let reached = b.taken + b.not_taken > 0;
                let c = |n: u64| if reached { n.to_string() } else { "-".into() };
                let _ = writeln!(s, "BRDA:{},{},0,{}", b.line, b.id, c(b.taken));
                let _ = writeln!(s, "BRDA:{},{},1,{}", b.line, b.id, c(b.not_taken));
            }
            let _ = writeln!(s, "BRF:{}", 2 * brs.len());
            let _ = writeln!(
                s,
                "BRH:{}",
                brs.iter()
                    .map(|b| usize::from(b.taken > 0) + usize::from(b.not_taken > 0))
                    .sum::<usize>()
            );
            for l in &lines {
                let _ = writeln!(s, "DA:{},{}", l.line, l.count);
            }
            let _ = writeln!(s, "LF:{}", lines.len());
            let _ = writeln!(s, "LH:{}", lines.iter().filter(|l| l.count > 0).count());
            let _ = writeln!(s, "end_of_record");
        }
    }
    s
}

fn html_escape(t: &str) -> String {
    t.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

const CSS: &str = "body{font-family:sans-serif;margin:1.5em;background:#fff;color:#222}\
table{border-collapse:collapse}td,th{padding:2px 10px;text-align:left}\
th{border-bottom:1px solid #999}pre{margin:0}.src td{font-family:monospace;white-space:pre;padding:0 6px}\
.hit{background:#dff5d8}.miss{background:#fbd9d6}.part{background:#fbefc4}.n{color:#777;text-align:right}\
@media (prefers-color-scheme:dark){body{background:#1b1b1b;color:#ddd}.hit{background:#1f3a1c}\
.miss{background:#4a1f1c}.part{background:#47401a}.n{color:#999}}";

/// An HTML report in `dir`: `index.html` and a page per source file with
/// the counts beside the lines (green: run, red: not run, yellow: a
/// branch on the line not taken both ways). The sources are read from
/// their recorded paths.
pub fn html(programs: &[ProgramCov], dir: &Path) -> std::io::Result<()> {
    use std::fmt::Write;
    std::fs::create_dir_all(dir)?;
    let mut index = format!(
        "<!doctype html><meta charset=utf-8><title>Zeus coverage</title><style>{CSS}</style>\
         <h1>Zeus coverage</h1><table><tr><th>Language<th>Program<th>Source<th>Lines<th>Branches<th>Paragraphs/procedures</tr>"
    );
    let mut page_no = 0;
    for p in programs {
        let (l, b, f) = summary(p);
        let main = p.sources.first().cloned().unwrap_or_default();
        let mut links = Vec::new();
        for (fi, path) in p.sources.iter().enumerate() {
            let fi = fi as u32;
            if !p.lines.iter().any(|x| x.file == fi) {
                continue;
            }
            page_no += 1;
            let page = format!("f{page_no}.html");
            let text = std::fs::read(path)
                .map(|d| String::from_utf8_lossy(&d).into_owned())
                .unwrap_or_default();
            let counts: BTreeMap<u32, u64> = p
                .lines
                .iter()
                .filter(|x| x.file == fi)
                .map(|x| (x.line, x.count))
                .collect();
            let mut partial: BTreeMap<u32, bool> = BTreeMap::new();
            for br in p.branches.iter().filter(|x| x.file == fi) {
                *partial.entry(br.line).or_default() |= br.taken == 0 || br.not_taken == 0;
            }
            let name = Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut h = format!(
                "<!doctype html><meta charset=utf-8><title>{n}</title><style>{CSS}</style>\
                 <p><a href=index.html>index</a><h1>{n}</h1><p>{prog} ({lang})<table class=src>",
                n = html_escape(&name),
                prog = html_escape(&p.name),
                lang = p.language
            );
            for (i, src) in text.lines().enumerate() {
                let ln = i as u32 + 1;
                let (class, c) = match counts.get(&ln) {
                    Some(0) => ("miss", "0".to_string()),
                    Some(&c) if partial.get(&ln).copied().unwrap_or(false) => {
                        ("part", c.to_string())
                    }
                    Some(&c) => ("hit", c.to_string()),
                    None => ("", String::new()),
                };
                let _ = write!(
                    h,
                    "<tr class=\"{class}\"><td class=n>{ln}<td class=n>{c}<td>{}</tr>",
                    html_escape(src)
                );
            }
            h.push_str("</table>");
            std::fs::write(dir.join(&page), h)?;
            links.push(format!("<a href={page}>{}</a>", html_escape(&name)));
        }
        let _ = write!(
            index,
            "<tr><td>{}<td>{}<td>{}<td>{l}<td>{b}<td>{f}</tr>",
            p.language,
            html_escape(&p.name),
            if links.is_empty() {
                html_escape(&main)
            } else {
                links.join(" ")
            }
        );
    }
    index.push_str("</table>");
    std::fs::write(dir.join("index.html"), index)
}

/// Coverage of a COBOL program from its instruction counts: `hits[pc]`
/// executions of each instruction, `jumped[pc]` the times a conditional
/// jump went to its target.
pub fn cobol_program(
    code: &crate::zir::ProgramCode,
    sources: &[String],
    hits: &[u64],
    jumped: &[u64],
) -> ProgramCov {
    use crate::zir::Instr;
    // the first instruction of each statement
    let mut first: Vec<Option<usize>> = vec![None; code.stmt_pos.len()];
    for (pc, &s) in code.instr_stmt.iter().enumerate() {
        if let Some(f) = first.get_mut(s as usize)
            && f.is_none()
        {
            *f = Some(pc);
        }
    }
    let mut lines: BTreeMap<(u32, u32), u64> = BTreeMap::new();
    for (s, f) in first.iter().enumerate() {
        let Some(pc) = f else { continue };
        let (file, line) = code.stmt_pos[s];
        let c = hits.get(*pc).copied().unwrap_or(0);
        let e = lines.entry((file, line)).or_default();
        *e = (*e).max(c);
    }
    let mut branches = Vec::new();
    for (pc, ins) in code.code.iter().enumerate() {
        if !matches!(ins, Instr::JumpIf(..) | Instr::JumpUnless(..)) {
            continue;
        }
        let Some(&(file, line)) = code
            .instr_stmt
            .get(pc)
            .and_then(|&s| code.stmt_pos.get(s as usize))
        else {
            continue;
        };
        let n = hits.get(pc).copied().unwrap_or(0);
        let j = jumped.get(pc).copied().unwrap_or(0);
        branches.push(BranchCount {
            file,
            line,
            id: pc as u32,
            taken: j,
            not_taken: n - j.min(n),
        });
    }
    let functions = code
        .paragraphs
        .iter()
        .filter(|p| (p.start as usize) < code.code.len() && p.pos.1 > 0)
        .filter_map(|p| {
            let name = match (&p.section, p.name.is_empty()) {
                (Some(s), true) => s.clone(),
                (None, true) => return None,
                (Some(s), false) => format!("{} OF {s}", p.name),
                (None, false) => p.name.clone(),
            };
            Some(FnCount {
                name,
                file: p.pos.0,
                line: p.pos.1,
                count: hits.get(p.start as usize).copied().unwrap_or(0),
            })
        })
        .collect();
    ProgramCov {
        language: "COBOL".into(),
        name: code.name.clone(),
        sources: sources.to_vec(),
        lines: lines
            .into_iter()
            .map(|((file, line), count)| LineCount { file, line, count })
            .collect(),
        branches,
        functions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_sums_counts() {
        let mut a = ProgramCov {
            language: "COBOL".into(),
            name: "P".into(),
            sources: vec!["/x/p.cbl".into()],
            lines: vec![LineCount {
                file: 0,
                line: 10,
                count: 1,
            }],
            branches: vec![BranchCount {
                file: 0,
                line: 10,
                id: 3,
                taken: 1,
                not_taken: 0,
            }],
            functions: vec![FnCount {
                name: "A".into(),
                file: 0,
                line: 9,
                count: 1,
            }],
        };
        let mut b = a.clone();
        b.lines[0].count = 0;
        b.lines.push(LineCount {
            file: 0,
            line: 12,
            count: 2,
        });
        b.branches[0] = BranchCount {
            not_taken: 4,
            taken: 0,
            ..b.branches[0]
        };
        a.merge(&b);
        assert_eq!(a.lines.len(), 2);
        assert_eq!(a.lines[0].count, 1);
        assert_eq!((a.branches[0].taken, a.branches[0].not_taken), (1, 4));
        assert_eq!(a.functions[0].count, 2);
        let (l, br, f) = summary(&a);
        assert_eq!((l.hit, l.total, br.hit, br.total, f.hit), (2, 2, 2, 2, 1));
        let t = lcov(&[a]);
        assert!(t.contains("SF:/x/p.cbl\n"));
        assert!(t.contains("DA:10,1\n") && t.contains("BRDA:10,3,1,4\n"));
    }
}

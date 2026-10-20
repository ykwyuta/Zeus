//! `report.html`: the branch directions a program's cases cover and those
//! they do not, on the program's source. Each conditional branch shows
//! both ways of its condition (covered: the cases that take it; not
//! covered: why); the statements and paragraphs the cases never run are
//! marked from the coverage of a replay of the cases.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write;

use zeus_cobol::coverage::ProgramCov;
use zeus_cobol::front::SourceEncoding;
use zeus_cobol::zir::{Instr, Module};

use crate::engine::{Dir, Gap, Report};

fn esc(t: &str) -> String {
    t.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// One way of a branch's condition.
enum Way {
    /// Covered by these cases (numbers from 1).
    Covered(Vec<usize>),
    Gap(Gap),
}

/// A conditional branch: its program and instruction, the way taken when
/// the condition holds and when it does not.
struct BranchView {
    program: String,
    pc: u32,
    holds: Way,
    fails: Way,
}

impl Way {
    fn open(&self) -> bool {
        matches!(self, Way::Gap(g) if !matches!(g, Gap::Impossible))
    }
}

const STYLE: &str = r#"
:root{--bg:#fff;--fg:#1d1d1f;--muted:#6b6b70;--line:#e3e3e6;--ok:#d7f2d2;--ok-fg:#1b5e20;
--gap:#fbd6d2;--gap-fg:#8e1b12;--part:#fbefc2;--never:#f6e0de;--imp:#e7e7ea;--imp-fg:#55555a;
--accent:#2457c5;--bar:#2e7d32;--bar-bg:#e5e5e8}
@media (prefers-color-scheme:dark){:root:not([data-theme="light"]){--bg:#17171a;--fg:#e6e6e9;
--muted:#9a9aa2;--line:#2c2c31;--ok:#1d3a1e;--ok-fg:#9fdc9a;--gap:#4a1f1b;--gap-fg:#ffb3aa;
--part:#45391a;--never:#3a2422;--imp:#2a2a2f;--imp-fg:#a8a8b0;--accent:#8fb0ff;--bar:#66bb6a;
--bar-bg:#2c2c31}}
:root[data-theme="dark"]{--bg:#17171a;--fg:#e6e6e9;--muted:#9a9aa2;--line:#2c2c31;--ok:#1d3a1e;
--ok-fg:#9fdc9a;--gap:#4a1f1b;--gap-fg:#ffb3aa;--part:#45391a;--never:#3a2422;--imp:#2a2a2f;
--imp-fg:#a8a8b0;--accent:#8fb0ff;--bar:#66bb6a;--bar-bg:#2c2c31}
*{box-sizing:border-box}
body{margin:0;padding:16px 24px;background:var(--bg);color:var(--fg);
font:14px/1.45 system-ui,-apple-system,"Segoe UI",sans-serif}
h1{font-size:20px;margin:0 0 4px}h2{font-size:16px;margin:24px 0 8px}
a{color:var(--accent)}.muted{color:var(--muted)}
.bar{height:10px;background:var(--bar-bg);border-radius:5px;overflow:hidden;max-width:420px;margin:8px 0}
.bar>span{display:block;height:100%;background:var(--bar)}
.stats{display:flex;flex-wrap:wrap;gap:8px 20px;margin:6px 0 12px}
.chip{display:inline-block;padding:1px 8px;border-radius:9px;font-size:12px;white-space:nowrap}
.ok{background:var(--ok);color:var(--ok-fg)}.gap{background:var(--gap);color:var(--gap-fg)}
.imp{background:var(--imp);color:var(--imp-fg)}
table{border-collapse:collapse}
.list td,.list th{padding:3px 10px;border-bottom:1px solid var(--line);text-align:left;vertical-align:top}
.list td.code{font-family:ui-monospace,Consolas,monospace;white-space:pre}
.src{width:100%;font-family:ui-monospace,Consolas,monospace;font-size:13px}
.src td{padding:0 6px;white-space:pre;vertical-align:top}
.src td.n,.src td.c{color:var(--muted);text-align:right;user-select:none}
.src td.m{white-space:nowrap}.src td:last-child{width:100%}
.wrap{overflow-x:auto}
.src tr.never{background:var(--never)}.src tr.part{background:var(--part)}
.src tr.done td.m .d{opacity:.85}
.d{display:inline-block;min-width:16px;padding:0 4px;margin-right:2px;border-radius:4px;
font-size:11px;text-align:center;cursor:help}
.controls{position:sticky;top:0;background:var(--bg);padding:6px 0;border-bottom:1px solid var(--line);z-index:1}
body.focus .src tr.far{display:none}
body.focus .src tr.cut td{border-top:1px dashed var(--line)}
:target{outline:2px solid var(--accent)}
@media (max-width:700px){body{padding:12px 16px}.src{font-size:12px}}
"#;

/// The source lines of a module's files: the source (in its encoding)
/// and the COPY members (as UTF-8).
fn source_lines(module: &Module, encoding: SourceEncoding) -> Vec<Vec<String>> {
    module
        .sources
        .iter()
        .enumerate()
        .map(|(k, path)| {
            let bytes = std::fs::read(path).unwrap_or_default();
            let text = if k == 0 {
                zeus_cobol::front::input::source_text(&bytes, encoding)
            } else {
                String::from_utf8_lossy(&bytes).into_owned()
            };
            text.lines().map(str::to_string).collect()
        })
        .collect()
}

/// The page. `coverage`: the statement coverage of the cases replayed.
pub fn page(
    module: &Module,
    program: &str,
    report: &Report,
    coverage: Option<&ProgramCov>,
    encoding: SourceEncoding,
) -> String {
    let gaps: HashMap<&Dir, &Gap> = report.gaps.iter().map(|(d, g)| (d, g)).collect();
    // the cases (from 1) taking each direction
    let mut taken: HashMap<Dir, Vec<usize>> = HashMap::new();
    for (k, c) in report.cases.iter().enumerate() {
        let mut seen = BTreeSet::new();
        for d in &c.out.branches {
            if seen.insert(d.clone()) {
                taken.entry(d.clone()).or_default().push(k + 1);
            }
        }
    }
    let way = |d: Dir| match gaps.get(&d) {
        Some(g) => Way::Gap((*g).clone()),
        None => Way::Covered(taken.get(&d).cloned().unwrap_or_default()),
    };
    // the branches by (file, line)
    let mut at: BTreeMap<(u32, u32), Vec<BranchView>> = BTreeMap::new();
    let pcs: BTreeSet<(String, u32)> = report.all.iter().map(|d| (d.0.clone(), d.1)).collect();
    for (prog, pc) in pcs {
        let Some(code) = module.programs.iter().find(|p| p.name == prog) else {
            continue;
        };
        // the way a jump goes when its condition holds
        let jumps_when_holds = match code.code.get(pc as usize) {
            Some(Instr::JumpIf(..)) => true,
            Some(Instr::JumpUnless(..)) => false,
            _ => continue,
        };
        let pos = code
            .instr_stmt
            .get(pc as usize)
            .and_then(|&s| code.stmt_pos.get(s as usize))
            .copied()
            .unwrap_or((0, 0));
        let holds = way((prog.clone(), pc, jumps_when_holds));
        let fails = way((prog.clone(), pc, !jumps_when_holds));
        at.entry(pos).or_default().push(BranchView {
            program: prog,
            pc,
            holds,
            fails,
        });
    }
    let lines = source_lines(module, encoding);
    // statement counts by (file, line), coverage files mapped to the
    // module's by path
    let mut counts: HashMap<(u32, u32), u64> = HashMap::new();
    let mut entered: Vec<(String, u32, u32, u64)> = Vec::new();
    if let Some(cov) = coverage {
        let file_of = |f: u32| -> Option<u32> {
            let p = cov.sources.get(f as usize)?;
            module.sources.iter().position(|s| s == p).map(|k| k as u32)
        };
        for l in &cov.lines {
            if let Some(f) = file_of(l.file) {
                *counts.entry((f, l.line)).or_default() += l.count;
            }
        }
        for p in &cov.functions {
            if let Some(f) = file_of(p.file) {
                entered.push((p.name.clone(), f, p.line, p.count));
            }
        }
    }

    let total = report.all.len();
    let open: Vec<&(Dir, Gap)> = report
        .gaps
        .iter()
        .filter(|(_, g)| !matches!(g, Gap::Impossible | Gap::Infeasible))
        .collect();
    let covered = total - report.gaps.len();
    let feasible = total
        - report
            .gaps
            .iter()
            .filter(|(_, g)| matches!(g, Gap::Impossible | Gap::Infeasible))
            .count();
    let pct = |a: usize, b: usize| {
        if b == 0 {
            100.0
        } else {
            100.0 * a as f64 / b as f64
        }
    };

    let mut h = String::new();
    let _ = write!(
        h,
        "<!doctype html><html lang=en><meta charset=utf-8>\
         <meta name=viewport content=\"width=device-width,initial-scale=1\">\
         <title>{p} coverage</title><style>{STYLE}</style><body>\
         <h1>{p}: C1 coverage of the generated cases</h1>\
         <div class=bar title=\"{pc:.1}% of the feasible directions\"><span style=\"width:{pc:.1}%\"></span></div>\
         <div class=stats><span><b>{covered}</b> of {total} branch directions covered ({all:.1}%)</span>\
         <span>{pc:.1}% of the feasible</span><span>{runs} runs</span><span>{n} cases</span></div>",
        p = esc(program),
        pc = pct(covered, feasible),
        all = pct(covered, total),
        runs = report.runs,
        n = report.cases.len(),
    );
    // the reasons, counted
    let mut by_reason: BTreeMap<&str, usize> = BTreeMap::new();
    for (_, g) in &report.gaps {
        *by_reason.entry(g.reason()).or_default() += 1;
    }
    if !by_reason.is_empty() {
        h.push_str("<p>");
        for (r, n) in &by_reason {
            let class = if r.starts_with("impossible") || r.starts_with("infeasible") {
                "imp"
            } else {
                "gap"
            };
            let _ = write!(h, "<span class=\"chip {class}\">{n} {}</span> ", esc(r));
        }
        h.push_str("</p>");
    }
    h.push_str(
        "<p class=muted>Each branch shows the two ways of its condition: \
         <span class=\"chip ok\">T</span> / <span class=\"chip ok\">F</span> covered (hover: the cases), \
         <span class=\"chip gap\">T</span> not covered (hover: why), \
         <span class=\"chip imp\">T</span> impossible or infeasible. \
         Lines shaded red ran in no case; yellow: a branch on the line is not covered both ways.</p>",
    );

    // the directions not covered: where the cases stop (reached, a way
    // not taken), then what lies beyond (not reached), by paragraph
    let several = module.programs.len() > 1;
    struct Row {
        f: u32,
        l: u32,
        way: String,
        gap: Gap,
        para: String,
    }
    let mut rows: Vec<Row> = Vec::new();
    for ((prog, pc, jumped), g) in &open {
        let Some(code) = module.programs.iter().find(|p| p.name == *prog) else {
            continue;
        };
        let (f, l) = code
            .instr_stmt
            .get(*pc as usize)
            .and_then(|&s| code.stmt_pos.get(s as usize))
            .copied()
            .unwrap_or((0, 0));
        let holds = match code.code.get(*pc as usize) {
            Some(Instr::JumpIf(..)) => *jumped,
            _ => !*jumped,
        };
        // the paragraph the line is in
        let para = code
            .paragraphs
            .iter()
            .filter(|p| p.pos.0 == f && p.pos.1 <= l && !p.name.is_empty())
            .max_by_key(|p| p.pos.1)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        rows.push(Row {
            f,
            l,
            way: if several {
                format!("{} ({prog})", if holds { "true" } else { "false" })
            } else {
                (if holds { "true" } else { "false" }).to_string()
            },
            gap: (*g).clone(),
            para,
        });
    }
    rows.sort_by(|a, b| (a.f, a.l, &a.way).cmp(&(b.f, b.l, &b.way)));
    let line_ref = |f: u32, l: u32| {
        format!(
            "<a href=\"#f{f}-{l}\">{}{l}</a>",
            if f == 0 {
                String::new()
            } else {
                format!("{}:", esc(&file_name(module, f)))
            }
        )
    };
    let text_at = |f: u32, l: u32| {
        lines
            .get(f as usize)
            .and_then(|t| t.get(l.saturating_sub(1) as usize))
            .map(|s| s.trim_end().to_string())
            .unwrap_or_default()
    };
    let stops: Vec<&Row> = rows.iter().filter(|r| r.gap != Gap::NotReached).collect();
    if !stops.is_empty() {
        let _ = write!(
            h,
            "<h2>Where the cases stop ({})</h2><p class=muted>Branches the cases reach with a way \
             no case takes: what lies beyond them is not reached.</p>\
             <table class=list><tr><th>Line<th>Paragraph<th>Way not taken<th>Why<th>Source</tr>",
            stops.len()
        );
        let mut k = 0;
        while k < stops.len() {
            // the ways of one line with the same reason, together
            let r = stops[k];
            let mut ways = vec![r.way.clone()];
            let mut j = k + 1;
            while j < stops.len() && stops[j].f == r.f && stops[j].l == r.l && stops[j].gap == r.gap
            {
                ways.push(stops[j].way.clone());
                j += 1;
            }
            let _ = write!(
                h,
                "<tr><td>{}<td>{}<td>{}<td><span class=\"chip gap\">{}</span><td class=code>{}</tr>",
                line_ref(r.f, r.l),
                esc(&r.para),
                esc(&ways.join(", ")),
                esc(r.gap.reason()),
                esc(&text_at(r.f, r.l))
            );
            k = j;
        }
        h.push_str("</table>");
    }
    let beyond: Vec<&Row> = rows.iter().filter(|r| r.gap == Gap::NotReached).collect();
    if !beyond.is_empty() {
        // by paragraph: the lines and the directions
        let mut paras: Vec<(String, u32, Vec<u32>, usize)> = Vec::new();
        for r in &beyond {
            match paras.iter_mut().find(|p| p.0 == r.para && p.1 == r.f) {
                Some(p) => {
                    if !p.2.contains(&r.l) {
                        p.2.push(r.l);
                    }
                    p.3 += 1;
                }
                None => paras.push((r.para.clone(), r.f, vec![r.l], 1)),
            }
        }
        let _ = write!(
            h,
            "<details><summary><b>Not reached ({} directions in {} paragraphs)</b></summary>\
             <table class=list><tr><th>Paragraph<th>Directions<th>Lines</tr>",
            beyond.len(),
            paras.len()
        );
        for (name, f, ls, n) in &paras {
            let refs: Vec<String> = ls.iter().map(|&l| line_ref(*f, l)).collect();
            let _ = write!(
                h,
                "<tr><td>{}<td>{n}<td>{}</tr>",
                if name.is_empty() {
                    "-".to_string()
                } else {
                    esc(name)
                },
                refs.join(" ")
            );
        }
        h.push_str("</table></details>");
    }

    // paragraphs no case entered
    let never: Vec<&(String, u32, u32, u64)> = entered.iter().filter(|p| p.3 == 0).collect();
    if !never.is_empty() {
        let _ = write!(h, "<h2>Paragraphs no case runs ({})</h2><p>", never.len());
        for (name, f, l, _) in never {
            let _ = write!(h, "<a href=\"#f{f}-{l}\">{}</a> ", esc(name));
        }
        h.push_str("</p>");
    }

    h.push_str(
        "<div class=controls><label><input type=checkbox id=focus> \
         Only the lines near what is not covered</label></div>",
    );
    // the files: the source, then the COPY members with branches or
    // statements
    for (f, text) in lines.iter().enumerate() {
        let f = f as u32;
        let has = at.keys().any(|k| k.0 == f) || counts.keys().any(|k| k.0 == f);
        if f > 0 && !has {
            continue;
        }
        let _ = write!(
            h,
            "<h2>{}</h2><div class=wrap><table class=src>",
            esc(&file_name(module, f))
        );
        // the lines worth showing in the focused view
        let mut near: BTreeSet<u32> = BTreeSet::new();
        let mut mark = |l: u32| {
            for k in l.saturating_sub(3)..=l + 3 {
                near.insert(k);
            }
        };
        for ((ff, l), bs) in &at {
            if *ff == f && bs.iter().any(|b| b.holds.open() || b.fails.open()) {
                mark(*l);
            }
        }
        for ((ff, l), c) in &counts {
            if *ff == f && *c == 0 {
                mark(*l);
            }
        }
        let mut shown_before = true;
        for (i, src) in text.iter().enumerate() {
            let l = i as u32 + 1;
            let bs = at.get(&(f, l));
            let count = counts.get(&(f, l));
            let partial = bs.is_some_and(|bs| bs.iter().any(|b| b.holds.open() || b.fails.open()));
            let mut class = match count {
                Some(0) => "never",
                _ if partial => "part",
                Some(_) => "done",
                None => "",
            }
            .to_string();
            let is_near = near.contains(&l);
            if !is_near {
                class.push_str(" far");
            } else if !shown_before {
                class.push_str(" cut");
            }
            shown_before = is_near;
            let mut marks = String::new();
            for b in bs.into_iter().flatten() {
                for (label, w) in [("T", &b.holds), ("F", &b.fails)] {
                    let (cls, tip) = match w {
                        Way::Covered(cs) => (
                            "ok",
                            format!(
                                "condition {}: cases {}",
                                if label == "T" { "true" } else { "false" },
                                cs.iter()
                                    .map(|c| format!("{c:03}"))
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            ),
                        ),
                        Way::Gap(g) => (
                            if matches!(g, Gap::Impossible | Gap::Infeasible) {
                                "imp"
                            } else {
                                "gap"
                            },
                            format!(
                                "condition {}: {}",
                                if label == "T" { "true" } else { "false" },
                                g.reason()
                            ),
                        ),
                    };
                    let _ = write!(
                        marks,
                        "<span class=\"d {cls}\" title=\"{} (instruction {} of {})\">{label}</span>",
                        esc(&tip),
                        b.pc,
                        esc(&b.program)
                    );
                }
                marks.push(' ');
            }
            let _ = write!(
                h,
                "<tr id=\"f{f}-{l}\" class=\"{class}\"><td class=n>{l}<td class=c>{}<td class=m>{marks}<td>{}</tr>",
                count.map_or(String::new(), |c| c.to_string()),
                esc(src)
            );
        }
        h.push_str("</table></div>");
    }
    h.push_str(
        "<script>const f=document.getElementById('focus');\
         try{f.checked=localStorage.getItem('ztestgen-focus')==='1'}catch(e){}\
         const set=()=>{document.body.classList.toggle('focus',f.checked);\
         try{localStorage.setItem('ztestgen-focus',f.checked?'1':'0')}catch(e){}};\
         f.addEventListener('change',set);set();</script></body></html>\n",
    );
    h
}

fn file_name(module: &Module, f: u32) -> String {
    module
        .sources
        .get(f as usize)
        .map(|p| {
            std::path::Path::new(p)
                .file_name()
                .map_or(p.clone(), |n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| format!("file {f}"))
}

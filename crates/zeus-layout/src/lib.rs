//! Fixed-length layouts of the data a COBOL program exchanges - files,
//! cards, MQ message buffers, its parameters and the arguments of the
//! programs it calls - written as copybooks, one per layout (every
//! REDEFINES splits a record into layouts). See
//! docs/proposal/0013-fixed-length-layouts.md.

pub mod check;
pub mod iface;
pub mod render;
pub mod select;
pub mod split;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use zeus_cobol::front::ast::Program;
use zeus_cobol::front::{self, FileTable, FrontOptions};
use zeus_cobol::sema::data::{self, DataModel, Settings};

use check::Expect;
use render::{Text, clauses};
use split::{Choices, Splitter};

#[derive(Debug, Clone)]
pub struct Options {
    pub copy_dirs: Vec<PathBuf>,
    /// Kinds to write (empty: all).
    pub kinds: Vec<String>,
    /// Keep VALUE clauses.
    pub values: bool,
    /// Start positions in columns 73-80.
    pub offsets: bool,
    /// Keep plain PIC X bases redefined by groups as layouts.
    pub keep_base: bool,
    pub max_layouts: usize,
    /// Compile each copybook and compare the positions.
    pub check: bool,
    /// Directories of BMS mapsets: their symbolic maps are COPY members.
    pub bms_dirs: Vec<PathBuf>,
    /// Level-01 records written as layouts of kind RECORD (resource: the
    /// record's name) when no interface has them (the data of test cases
    /// a program receives otherwise, such as a terminal's input).
    pub records: Vec<String>,
    /// The representation of the source file (not of its COPY members).
    pub source_encoding: zeus_cobol::front::SourceEncoding,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            copy_dirs: Vec::new(),
            kinds: Vec::new(),
            values: false,
            offsets: true,
            keep_base: false,
            max_layouts: 256,
            check: true,
            bms_dirs: Vec::new(),
            records: Vec::new(),
            source_encoding: Default::default(),
        }
    }
}

/// One layout written as a copybook.
#[derive(Debug, Clone)]
pub struct Layout {
    pub program: String,
    pub kind: &'static str,
    pub resource: String,
    /// The layout's name (several layouts), else empty.
    pub label: String,
    /// 1-based number and count among the layouts of the datum.
    pub index: usize,
    pub count: usize,
    /// `FIELD = 'X'` conditions selecting the layout.
    pub selector: String,
    /// Chosen items: (choice point, chosen item).
    pub choices: Vec<(String, String)>,
    /// Choice points left as REDEFINES (over the limit).
    pub kept: Vec<String>,
    pub length: String,
    /// The source of the record: file and line.
    pub source: String,
    pub file_name: String,
    pub text: String,
    /// The result of the compile check (None: not checked).
    pub check: Option<Result<(), String>>,
}

/// File-name characters for a name.
fn safe(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// File-name characters for each part of a dotted name.
fn safe_parts(s: &str) -> String {
    s.split('.').map(safe).collect::<Vec<_>>().join(".")
}

/// The name of an alternative: its words not in the base's name
/// (EXPORT-CUSTOMER-DATA for EXPORT-RECORD-DATA -> CUSTOMER).
fn alt_name(base: &str, alt: &str) -> String {
    let b: HashSet<&str> = base.split('-').collect();
    let w: Vec<&str> = alt.split('-').filter(|w| !b.contains(w)).collect();
    if w.is_empty() {
        alt.to_string()
    } else {
        w.join("-")
    }
}

/// The layouts of a record: the record, its splitter, the choices of
/// each layout, the choice points left as REDEFINES, the record types.
type RootLayouts<'a> = (
    usize,
    Splitter<'a>,
    Vec<Choices>,
    Vec<usize>,
    HashMap<usize, Vec<select::Evidence>>,
);

struct Ctx<'a> {
    dm: &'a DataModel,
    sp: &'a Splitter<'a>,
    opts: &'a Options,
}

impl Ctx<'_> {
    fn name(&self, id: usize) -> String {
        self.dm.items[id]
            .name
            .clone()
            .unwrap_or_else(|| "FILLER".into())
    }

    /// The end (offset after it) of item `id` and its subordinate items
    /// as chosen.
    fn end(&self, id: usize, choices: &Choices, top: bool) -> u32 {
        let it = &self.dm.items[id];
        let n = if top {
            1
        } else {
            it.occurs.as_ref().map_or(1, |o| o.max)
        };
        if it.children.is_empty() {
            return it.offset + it.len * n;
        }
        // a group: its items as chosen (in its first occurrence), then the
        // other occurrences
        let mut e = 0;
        for g in self.sp.groups(id) {
            if g.len() > 1
                && let Some(&c) = choices.get(&g[0])
            {
                e = e.max(self.end(c, choices, false));
                continue;
            }
            for &c in &g {
                e = e.max(self.end(c, choices, false));
            }
        }
        if e == 0 {
            e = it.offset + it.len;
        }
        e + it.len * (n - 1)
    }

    /// Writes item `id` and its subordinate items as chosen.
    #[allow(clippy::too_many_arguments)]
    fn item(
        &self,
        t: &mut Text,
        id: usize,
        base: u32,
        depth: usize,
        choices: &Choices,
        redefines: bool,
        top: Option<u8>,
        expect: &mut Vec<Expect>,
        odo: &mut Vec<String>,
    ) {
        let it = &self.dm.items[id];
        let col = (8 + 4 * depth).min(36);
        let mut e = it.entry.clone();
        let level = match top {
            // the record (or an item of a row): no REDEFINES, and no
            // OCCURS on a level-01 item
            Some(l) => {
                e.redefines = None;
                if l == 1 {
                    e.occurs = None;
                }
                l
            }
            None => it.level,
        };
        if let Some(o) = &e.occurs
            && let Some(d) = &o.depending
        {
            odo.push(d.name.clone());
        }
        let pos = it.offset - base + 1;
        t.entry(
            col,
            level,
            &self.name(id),
            &clauses(&e, redefines, self.opts.values),
            self.opts.offsets.then_some(pos),
        );
        expect.push(Expect {
            name: self.name(id),
            pos,
            len: it.len,
        });
        for &c in &it.conditions {
            let ce = &self.dm.items[c].entry;
            t.entry(
                (col + 4).min(40),
                88,
                &self.name(c),
                &clauses(ce, false, true),
                None,
            );
        }
        for g in self.sp.groups(id) {
            if g.len() > 1
                && let Some(&c) = choices.get(&g[0])
            {
                self.item(t, c, base, depth + 1, choices, false, None, expect, odo);
                continue;
            }
            for (k, &c) in g.iter().enumerate() {
                self.item(t, c, base, depth + 1, choices, k > 0, None, expect, odo);
            }
        }
    }
}

/// Writes the COBOL symbolic maps of the mapsets under `opts.bms_dirs`
/// to a temporary directory added to the COPY directories (kept while the
/// returned directory lives).
pub fn symbolic_maps(opts: &mut Options) -> std::io::Result<Option<tempfile::TempDir>> {
    fn scan(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                scan(&p, out);
            } else if p.extension().is_some_and(|x| x.eq_ignore_ascii_case("bms")) {
                out.push(p);
            }
        }
    }
    let mut maps = Vec::new();
    for d in &opts.bms_dirs {
        scan(d, &mut maps);
    }
    if maps.is_empty() {
        return Ok(None);
    }
    let tmp = tempfile::tempdir()?;
    for m in maps {
        let Ok(text) = std::fs::read_to_string(&m) else {
            continue;
        };
        let Ok(ms) = zeus_bms::parse(&text) else {
            continue;
        };
        let name = ms.name.to_ascii_uppercase();
        std::fs::write(
            tmp.path().join(format!("{name}.cpy")),
            zeus_bms::symbolic::cobol(&ms),
        )?;
    }
    opts.copy_dirs.push(tmp.path().to_path_buf());
    Ok(Some(tmp))
}

/// The layouts of the data exchanged by the programs of a source file.
pub fn analyze_file(path: &Path, opts: &Options) -> Result<Vec<Layout>, String> {
    let fo = FrontOptions {
        copy_dirs: opts.copy_dirs.clone(),
        source_encoding: opts.source_encoding,
        ..Default::default()
    };
    let mut files = FileTable::default();
    let progs =
        front::parse_file(path, &fo, &mut files).map_err(|d| format!("{}: {d}", path.display()))?;
    let mut out = Vec::new();
    for p in &progs {
        program(p, &files, opts, &mut out)?;
    }
    Ok(out)
}

fn program(
    p: &Program,
    files: &FileTable,
    opts: &Options,
    out: &mut Vec<Layout>,
) -> Result<(), String> {
    let settings = Settings {
        decimal_comma: p.env.decimal_point_comma,
        currency: p
            .env
            .currency_sign
            .as_ref()
            .and_then(|c| c.chars().next())
            .unwrap_or('$'),
    };
    let dm = data::build(p, &settings).map_err(|d| format!("{}: {d}", p.name))?;
    let file_name = |f: u32| {
        files
            .names
            .get(f as usize)
            .map(|n| {
                Path::new(n)
                    .file_name()
                    .map(|x| x.to_string_lossy().into_owned())
                    .unwrap_or_else(|| n.clone())
            })
            .unwrap_or_default()
    };
    let mut ifaces = iface::find(p, &dm);
    for name in &opts.records {
        let Some(id) = dm
            .items
            .iter()
            .position(|it| it.level == 1 && it.name.as_deref() == Some(name.as_str()))
        else {
            continue;
        };
        if ifaces.iter().any(|f| f.roots.contains(&id)) {
            continue;
        }
        ifaces.push(iface::Iface {
            kind: "RECORD",
            resource: name.clone(),
            detail: format!("RECORD  {name}"),
            roots: vec![id],
            record: None,
            split: true,
            row: None,
        });
    }
    for ifc in ifaces {
        if !opts.kinds.is_empty() && !opts.kinds.iter().any(|k| k.eq_ignore_ascii_case(ifc.kind)) {
            continue;
        }
        if let Some((name, items)) = &ifc.row {
            let sp = Splitter::new(&dm, opts.keep_base);
            let ctx = Ctx {
                dm: &dm,
                sp: &sp,
                opts,
            };
            let mut t = Text::default();
            let mut body = Text::default();
            let mut expect = Vec::new();
            let mut odo = Vec::new();
            let mut cum = 0u32;
            let mut parts = Vec::new();
            for &(v, ind) in items {
                for x in std::iter::once(v).chain(ind) {
                    let it = &dm.items[x];
                    let n = it.len * it.occurs.as_ref().map_or(1, |o| o.max);
                    ctx.item(
                        &mut body,
                        x,
                        it.offset - cum,
                        1,
                        &Choices::new(),
                        false,
                        Some(5),
                        &mut expect,
                        &mut odo,
                    );
                    parts.push(ctx.name(x));
                    cum += n;
                }
            }
            t.comment(&"=".repeat(64));
            t.comment(&format!(" PROGRAM   : {}", p.name));
            t.comment(&format!(" KIND      : {}", ifc.detail));
            t.comment(" LAYOUT    : 1 OF 1");
            t.comment(&format!(" ITEMS     : {}", parts.join(" ")));
            t.comment(&format!(" LENGTH    : {cum} BYTES"));
            t.comment(&"=".repeat(64));
            t.entry(8, 1, name, &[], opts.offsets.then_some(1));
            t.lines.extend(body.lines);
            expect.insert(
                0,
                Expect {
                    name: name.clone(),
                    pos: 1,
                    len: cum,
                },
            );
            let text = t.into_string();
            let extra: Vec<String> = odo
                .into_iter()
                .filter(|n| !expect.iter().any(|e| e.name == *n))
                .collect();
            let check = opts.check.then(|| check::check(&text, &extra, &expect));
            out.push(Layout {
                program: p.name.clone(),
                kind: ifc.kind,
                resource: ifc.resource.clone(),
                label: String::new(),
                index: 1,
                count: 1,
                selector: String::new(),
                choices: Vec::new(),
                kept: Vec::new(),
                length: format!("{cum} BYTES"),
                source: String::new(),
                file_name: format!(
                    "{}.{}.{}.cpy",
                    safe(&p.name),
                    ifc.kind,
                    safe_parts(&ifc.resource)
                ),
                text,
                check,
            });
            continue;
        }
        // the layouts of each record
        let mut per_root: Vec<RootLayouts> = Vec::new();
        for &root in &ifc.roots {
            let mut sp = Splitter::new(&dm, opts.keep_base);
            if !ifc.split {
                sp.kept = sp.choice_points(root).into_iter().collect();
            }
            let ev = select::infer(p, &dm, &sp, root);
            let protect: HashSet<usize> = sp
                .choice_points(root)
                .into_iter()
                .filter(|b| {
                    let group = sp
                        .groups(dm.items[*b].parent.unwrap_or(root))
                        .into_iter()
                        .find(|g| g[0] == *b)
                        .unwrap_or_default();
                    group.iter().any(|a| ev.contains_key(a))
                })
                .collect();
            let kept = sp.limit(root, opts.max_layouts, &protect);
            let vars = sp.variants(root);
            per_root.push((root, sp, vars, kept, ev));
        }
        let count: usize = per_root.iter().map(|r| r.2.len()).sum();
        let mut index = 0;
        for (root, sp, vars, kept, ev) in &per_root {
            let ctx = Ctx { dm: &dm, sp, opts };
            // the choice points where the layouts differ
            let varying: Vec<usize> = {
                let mut v: Vec<usize> = Vec::new();
                for c in vars {
                    for (b, a) in c {
                        if vars.iter().any(|o| o.get(b) != Some(a)) && !v.contains(b) {
                            v.push(*b);
                        }
                    }
                }
                v
            };
            for (n, choices) in vars.iter().enumerate() {
                index += 1;
                let mut label = Vec::new();
                if per_root.len() > 1 {
                    label.push(ctx.name(*root));
                }
                if vars.len() > 1 {
                    if varying.len() == 1 {
                        let b = varying[0];
                        label.push(alt_name(&ctx.name(b), &ctx.name(choices[&b])));
                    } else {
                        label.push(format!("L{:03}", n + 1));
                    }
                }
                let label = label.join(".");
                let mut sel: Vec<String> = Vec::new();
                let mut sel_at: Vec<String> = Vec::new();
                for a in choices.values() {
                    for e in ev.get(a).into_iter().flatten() {
                        let s = format!("{} = {}", e.field, e.value);
                        if !sel.contains(&s) {
                            sel.push(s);
                            if e.at.1 > 0 {
                                sel_at.push(format!("{}:{}", file_name(e.at.0), e.at.1));
                            }
                        }
                    }
                }
                let it = &dm.items[*root];
                let length = match ifc.record {
                    Some((min, max)) if min != max => format!("{min} TO {max} BYTES (VARIABLE)"),
                    _ => format!("{} BYTES", it.len),
                };
                let source = format!("{}:{}", file_name(it.pos.file), it.pos.line);
                let choice_list: Vec<(String, String)> = choices
                    .iter()
                    .map(|(b, a)| (ctx.name(*b), ctx.name(*a)))
                    .collect();
                let kept_names: Vec<String> = kept.iter().map(|b| ctx.name(*b)).collect();
                // header
                let mut t = Text::default();
                t.comment(&"=".repeat(64));
                t.comment(&format!(" PROGRAM   : {}", p.name));
                t.comment(&format!(" KIND      : {}", ifc.detail));
                t.comment(&format!(
                    " LAYOUT    : {index} OF {count}{}",
                    if label.is_empty() {
                        String::new()
                    } else {
                        format!("  {label}")
                    }
                ));
                if count > 1 {
                    if sel.is_empty() {
                        t.comment(" SELECTED  : RECORD TYPE NOT FOUND");
                    } else {
                        t.comment(&format!(" SELECTED  : {}", sel.join(" OR ")));
                        if !sel_at.is_empty() {
                            t.comment(&format!("             ({})", sel_at.join(", ")));
                        }
                    }
                }
                for (k, (b, a)) in choice_list.iter().enumerate() {
                    let head = if k == 0 {
                        " CHOICES   :"
                    } else {
                        "            "
                    };
                    t.comment(&format!("{head} {b} -> {a}"));
                }
                for (k, b) in kept_names.iter().enumerate() {
                    let head = if k == 0 {
                        " REDEFINES :"
                    } else {
                        "            "
                    };
                    t.comment(&format!(
                        "{head} {b} (NOT SPLIT: OVER {} LAYOUTS)",
                        opts.max_layouts
                    ));
                }
                t.comment(&format!(" LENGTH    : {length}"));
                t.comment(&format!(" SOURCE    : {source}"));
                t.comment(&"=".repeat(64));
                let mut expect = Vec::new();
                let mut odo = Vec::new();
                ctx.item(
                    &mut t,
                    *root,
                    it.offset,
                    0,
                    choices,
                    false,
                    Some(1),
                    &mut expect,
                    &mut odo,
                );
                // a shorter alternative chosen (REDEFINES by a larger
                // item): the rest of the record
                let used = ctx.end(*root, choices, true);
                if let Some(&first) = it.children.first()
                    && used < it.offset + it.len
                {
                    let rest = it.offset + it.len - used;
                    t.entry(
                        12,
                        dm.items[first].level,
                        "FILLER",
                        &[format!("PIC X({rest})")],
                        opts.offsets.then_some(used - it.offset + 1),
                    );
                    expect.push(Expect {
                        name: "FILLER".into(),
                        pos: used - it.offset + 1,
                        len: rest,
                    });
                }
                let text = t.into_string();
                let extra: Vec<String> = odo
                    .into_iter()
                    .filter(|n| !expect.iter().any(|e| e.name == *n))
                    .collect();
                let check = opts.check.then(|| check::check(&text, &extra, &expect));
                let mut file = format!(
                    "{}.{}.{}",
                    safe(&p.name),
                    ifc.kind,
                    safe_parts(&ifc.resource)
                );
                for part in label.split('.').filter(|x| !x.is_empty()) {
                    file.push('.');
                    file.push_str(&safe(part));
                }
                file.push_str(".cpy");
                out.push(Layout {
                    program: p.name.clone(),
                    kind: ifc.kind,
                    resource: ifc.resource.clone(),
                    label,
                    index,
                    count,
                    selector: sel.join(" OR "),
                    choices: choice_list,
                    kept: kept_names,
                    length,
                    source,
                    file_name: file,
                    text,
                    check,
                });
            }
        }
    }
    for n in &p.nested {
        program(n, files, opts, out)?;
    }
    Ok(())
}

fn csv(fields: &[&str]) -> String {
    fields
        .iter()
        .map(|f| {
            if f.contains([',', '"', '\n']) {
                format!("\"{}\"", f.replace('"', "\"\""))
            } else {
                f.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Writes the copybooks and their index `layouts.csv` to `dir`.
pub fn write(layouts: &[Layout], dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut index = String::from(
        "program,kind,resource,layout,index,count,selector,choices,length,file,source,check\r\n",
    );
    for l in layouts {
        std::fs::write(dir.join(&l.file_name), &l.text)?;
        let choices: Vec<String> = l.choices.iter().map(|(b, a)| format!("{b}->{a}")).collect();
        let check = match &l.check {
            None => "NOT_CHECKED".to_string(),
            Some(Ok(())) => "OK".to_string(),
            Some(Err(e)) => format!("ERROR: {e}"),
        };
        index.push_str(&csv(&[
            &l.program,
            l.kind,
            &l.resource,
            &l.label,
            &l.index.to_string(),
            &l.count.to_string(),
            &l.selector,
            &choices.join(" "),
            &l.length,
            &l.file_name,
            &l.source,
            &check,
        ]));
        index.push_str("\r\n");
    }
    std::fs::write(dir.join("layouts.csv"), index)
}

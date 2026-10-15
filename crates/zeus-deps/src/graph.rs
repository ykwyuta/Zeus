//! The dependency graph: files are nodes, references are edges.
//!
//! The analysis starts from the root files (or every source file under
//! the root directories) and follows each reference to the file it
//! names, then the references of that file, and so on. Names are looked
//! up the way the Zeus compilers and job engine look them up (COPY
//! libraries, %INCLUDE directories, procedure libraries), then among the
//! files under the roots and the library directories (programs by their
//! program id, mapsets, PSBs and DBDs by their names). What is not found
//! is a Zeus-provided asset (the system asset master) or a missing asset.

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};

use crate::master;
use crate::scan::{self, Ref, Scanned};

/// What to analyse.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Files, or directories whose source files are all analysed.
    pub roots: Vec<PathBuf>,
    /// Library directories: COPY / %INCLUDE members, procedures, and the
    /// other assets references are looked up in.
    pub libs: Vec<PathBuf>,
    /// Check COBOL and PL/I programs with the Zeus compilers (compile
    /// status, unsupported statements).
    pub compile: bool,
}

#[derive(Debug, Clone)]
pub struct Node {
    pub id: usize,
    pub node_type: &'static str,
    pub name: String,
    /// FILE, SYSTEM (a Zeus-provided asset), MISSING or DYNAMIC.
    pub status: &'static str,
    pub path: Option<PathBuf>,
    pub attrs: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Location {
    pub line: usize,
    pub col: usize,
    /// The source line (trimmed).
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct Edge {
    pub id: usize,
    pub edge_type: &'static str,
    pub from: usize,
    pub to: usize,
    pub target_name: String,
    /// FILE, SYSTEM, MISSING or DYNAMIC.
    pub resolution: &'static str,
    /// Attribute -> values (LIST attributes keep every value).
    pub attrs: BTreeMap<String, Vec<String>>,
    pub locations: Vec<Location>,
}

#[derive(Debug, Default)]
pub struct Graph {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub diagnostics: Vec<SyntaxDiagnostic>,
}

/// A compiler diagnostic or a confirmed statement that cannot be executed.
/// Missing positions stay absent: they are not guessed from message text.
#[derive(Debug, Clone)]
pub struct SyntaxDiagnostic {
    pub node_id: usize,
    pub program: String,
    pub language: &'static str,
    /// UNSUPPORTED, ERROR, WARNING, INFO or COMPILER_FAILURE.
    pub category: &'static str,
    /// COMPILE or RUNTIME (accepted by the compiler, rejected if executed).
    pub stage: &'static str,
    pub syntax: String,
    pub path: PathBuf,
    pub line: Option<u32>,
    pub statement: String,
    pub message: String,
}

impl SyntaxDiagnostic {
    fn source_line(&mut self, cache: &mut HashMap<PathBuf, Vec<String>>) {
        let lines = cache.entry(self.path.clone()).or_insert_with(|| {
            std::fs::read_to_string(&self.path)
                .unwrap_or_default()
                .lines()
                .map(|s| s.trim().to_string())
                .collect()
        });
        self.statement = self
            .line
            .and_then(|line| lines.get(line.checked_sub(1)? as usize))
            .cloned()
            .unwrap_or_default();
    }
}

fn explicitly_unsupported(message: &str) -> bool {
    let message = message.to_ascii_lowercase();
    ["not supported", "unsupported", "not implemented"]
        .iter()
        .any(|s| message.contains(s))
}

struct File {
    path: PathBuf,
    scanned: Scanned,
    lines: Vec<String>,
    size: u64,
}

/// Where a reference leads.
enum Target {
    File(PathBuf, &'static str),
    System(String, &'static str, &'static str),
    Missing(String, &'static str),
    Dynamic(String),
}

struct Analyzer {
    opts: Options,
    files: Vec<File>,
    by_path: HashMap<PathBuf, usize>,
    /// Name (program id, mapset, PSB, DBD, procedure, stem) -> files.
    index: HashMap<(&'static str, String), Vec<usize>>,
    transactions: HashMap<String, String>,
    graph: Graph,
    file_node: HashMap<PathBuf, usize>,
    virtual_node: HashMap<(&'static str, String), usize>,
    edge_of: HashMap<(usize, usize, &'static str), usize>,
    /// COPY / %INCLUDE relations between files: includer -> members, and
    /// member -> includers.
    members: HashMap<usize, HashSet<usize>>,
    includers: HashMap<usize, HashSet<usize>>,
}

fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn read(path: &Path) -> Option<(String, u64)> {
    let data = std::fs::read(path).ok()?;
    let n = data.len() as u64;
    Some((String::from_utf8_lossy(&data).into_owned(), n))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        if p.is_dir() {
            walk(&p, out);
        } else if p.metadata().is_ok_and(|m| m.len() <= 16 << 20) {
            out.push(p);
        }
    }
}

/// The program-like node types (a program reference resolves to them).
fn index_category(t: &str) -> &'static str {
    match t {
        "COBOL_PROGRAM" | "PLI_PROGRAM" | "HLASM_PROGRAM" => "PROGRAM",
        "HLASM_COPYBOOK" => "HLASM_COPY",
        "BMS_MAPSET" => "MAPSET",
        "IMS_PSB" => "PSB",
        "IMS_DBD" => "DBD",
        "JCL_PROC" => "PROC",
        "JCL_INCLUDE" | "JCL_JOB" => "JCLMEMBER",
        "COBOL_COPYBOOK" => "COPY",
        "PLI_INCLUDE" => "INCLUDE",
        _ => "OTHER",
    }
}

impl Analyzer {
    fn add_file(
        &mut self,
        path: PathBuf,
        node_type: &'static str,
        text: String,
        size: u64,
    ) -> usize {
        let scanned = scan::scan(node_type, &path, &text);
        let i = self.files.len();
        let cat = index_category(node_type);
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_ascii_uppercase())
            .unwrap_or_default();
        let mut names: Vec<String> = vec![stem];
        names.extend(scanned.name.iter().cloned());
        names.extend(scanned.aliases.iter().cloned());
        for n in names {
            let v = self.index.entry((cat, n)).or_default();
            if !v.contains(&i) {
                v.push(i);
            }
        }
        for (t, p) in &scanned.transactions {
            self.transactions
                .entry(t.clone())
                .or_insert_with(|| p.clone());
        }
        self.by_path.insert(canonical(&path), i);
        self.files.push(File {
            path,
            lines: text.lines().map(str::to_string).collect(),
            scanned,
            size,
        });
        i
    }

    /// A file found by a search: known, or read now (its type from the
    /// reference when the file does not tell).
    fn file_at(&mut self, path: &Path, expected: &'static str) -> Option<usize> {
        let c = canonical(path);
        if let Some(&i) = self.by_path.get(&c) {
            return Some(i);
        }
        let (text, size) = read(path)?;
        let t = match scan::classify(path, &text) {
            Some(t) if index_category(t) == index_category(expected) => t,
            _ => expected,
        };
        Some(self.add_file(c, t, text, size))
    }

    fn search(dirs: &[PathBuf], name: &str, exts: &[&str]) -> Option<PathBuf> {
        let lower = name.to_ascii_lowercase();
        for d in dirs {
            for n in [name, lower.as_str()] {
                for e in exts {
                    let p = d.join(format!("{n}{e}"));
                    if p.is_file() {
                        return Some(p);
                    }
                }
            }
        }
        None
    }

    fn indexed(&self, cat: &'static str, name: &str) -> Option<&File> {
        self.index
            .get(&(cat, name.to_string()))
            .and_then(|v| v.first())
            .map(|&i| &self.files[i])
    }

    fn program(&self, name: &str) -> Target {
        if let Some(f) = self.indexed("PROGRAM", name) {
            return Target::File(f.path.clone(), f.scanned.node_type);
        }
        if let Some(a) = master::system_program(name) {
            return Target::System(a.name.to_string(), a.node_type, a.provider);
        }
        Target::Missing(name.to_string(), "PROGRAM")
    }

    fn resolve(&mut self, r: &Ref, from: usize) -> (Target, Vec<&'static str>) {
        let from_dir = self.files[from]
            .path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let from_type = self.files[from].scanned.node_type;
        let pli = matches!(from_type, "PLI_PROGRAM" | "PLI_INCLUDE");
        let mut via = Vec::new();
        if r.identifier.is_some() {
            via.push("VALUE");
        }
        if r.dynamic {
            return (Target::Dynamic(r.target.clone()), via);
        }
        let name = r.target.as_str();
        let t = match r.edge {
            "HLASM_COPY" => {
                let mut dirs = vec![from_dir.clone()];
                dirs.extend(self.opts.libs.iter().cloned());
                if let Some(path) = Self::search(&dirs, name, &["", ".mac", ".MAC", ".asm", ".ASM"])
                {
                    Target::File(path, "HLASM_COPYBOOK")
                } else if let Some(f) = self.indexed("HLASM_COPY", name) {
                    Target::File(f.path.clone(), f.scanned.node_type)
                } else {
                    Target::Missing(name.into(), "HLASM_COPYBOOK")
                }
            }
            "COPY" | "SQL_INCLUDE" | "PLI_INCLUDE" => {
                let (expected, exts): (&'static str, &[&str]) = if pli {
                    (
                        "PLI_INCLUDE",
                        &["", ".pli", ".inc", ".cpy", ".PLI", ".INC", ".CPY"],
                    )
                } else {
                    (
                        "COBOL_COPYBOOK",
                        &[
                            "", ".cpy", ".CPY", ".cbl", ".CBL", ".cob", ".COB", ".dcl", ".DCL",
                        ],
                    )
                };
                // the compilers' search: COPY ... OF library, the
                // libraries, the directory of the source
                let mut dirs: Vec<PathBuf> = Vec::new();
                if let Some((_, lib)) = r.attrs.iter().find(|(k, _)| *k == "library") {
                    dirs.push(from_dir.join(lib));
                    dirs.extend(self.opts.libs.iter().map(|d| d.join(lib)));
                }
                dirs.extend(self.opts.libs.iter().cloned());
                dirs.push(from_dir.clone());
                let builtin = r.edge == "SQL_INCLUDE" && matches!(name, "SQLCA" | "SQLDA");
                let own = canonical(&self.files[from].path);
                let cat = if pli { "INCLUDE" } else { "COPY" };
                let found = (!builtin)
                    .then(|| Self::search(&dirs, name, exts))
                    .flatten()
                    .filter(|p| canonical(p) != own)
                    // a program next to the source, when a copy member
                    // of that name is elsewhere (the build's COPY library)
                    .filter(|p| {
                        let program = self.by_path.get(&canonical(p)).is_some_and(|&j| {
                            self.files[j].scanned.node_type.ends_with("_PROGRAM")
                        });
                        !(program && self.indexed(cat, name).is_some())
                    });
                match found {
                    Some(p) => Target::File(p, expected),
                    None => {
                        if let Some(f) = (!builtin).then(|| self.indexed(cat, name)).flatten() {
                            Target::File(f.path.clone(), f.scanned.node_type)
                        } else if let Some(f) =
                            (!builtin).then(|| self.indexed("MAPSET", name)).flatten()
                        {
                            // the symbolic map generated from a mapset
                            via.push("BMS");
                            Target::File(f.path.clone(), f.scanned.node_type)
                        } else if let Some(p) = master::system_copybook(name, pli) {
                            Target::System(name.to_string(), "SYSTEM_COPYBOOK", p)
                        } else {
                            Target::Missing(name.to_string(), expected)
                        }
                    }
                }
            }
            "CICS_TRANSID" => match self.transactions.get(name).cloned() {
                Some(p) => {
                    via.push("CSD");
                    self.program(&p.to_ascii_uppercase())
                }
                None => Target::Missing(name.to_string(), "CICS_TRANSACTION"),
            },
            "CICS_MAP" | "CSD_MAPSET" => match self.indexed("MAPSET", name) {
                Some(f) => Target::File(f.path.clone(), f.scanned.node_type),
                None => Target::Missing(name.to_string(), "BMS_MAPSET"),
            },
            "DLI_PSB" | "JCL_IMS_PSB" => {
                // utility regions name a DBD
                let dbd =
                    r.attrs.iter().any(|(k, v)| {
                        *k == "region" && matches!(v.as_str(), "ULU" | "ULR" | "DBB")
                    }) && self.indexed("PSB", name).is_none();
                let (cat, t) = if dbd {
                    ("DBD", "IMS_DBD")
                } else {
                    ("PSB", "IMS_PSB")
                };
                match self.indexed(cat, name) {
                    Some(f) => Target::File(f.path.clone(), f.scanned.node_type),
                    None => Target::Missing(name.to_string(), t),
                }
            }
            "PSB_DBD" => match self.indexed("DBD", name) {
                Some(f) => Target::File(f.path.clone(), f.scanned.node_type),
                None => Target::Missing(name.to_string(), "IMS_DBD"),
            },
            "JCL_EXEC_PROC" | "JCL_INCLUDE" => {
                let (cat, expected) = if r.edge == "JCL_EXEC_PROC" {
                    ("PROC", "JCL_PROC")
                } else {
                    ("JCLMEMBER", "JCL_INCLUDE")
                };
                let mut dirs = vec![from_dir.clone()];
                dirs.extend(self.opts.libs.iter().cloned());
                match Self::search(
                    &dirs,
                    name,
                    &["", ".proc", ".prc", ".jcl", ".PROC", ".PRC", ".JCL"],
                ) {
                    Some(p) => Target::File(p, expected),
                    None => match self.indexed(cat, name) {
                        Some(f) => Target::File(f.path.clone(), f.scanned.node_type),
                        None => Target::Missing(name.to_string(), expected),
                    },
                }
            }
            // a mapset defined as a program (older CSDs)
            "CSD_PROGRAM" => match self.program(name) {
                Target::Missing(..) if self.indexed("MAPSET", name).is_some() => {
                    let f = self.indexed("MAPSET", name).unwrap();
                    Target::File(f.path.clone(), f.scanned.node_type)
                }
                t => t,
            },
            // CALL, FETCH, CICS LINK / XCTL / LOAD, EXEC PGM, RUN
            // PROGRAM, IMS programs, CSD programs and transactions
            _ => self.program(name),
        };
        (t, via)
    }

    fn virtual_node(&mut self, t: &'static str, name: &str, status: &'static str) -> usize {
        if let Some(&n) = self.virtual_node.get(&(t, name.to_string())) {
            if status == "MISSING" && self.graph.nodes[n - 1].status == "OPTIONAL" {
                self.graph.nodes[n - 1].status = "MISSING";
            }
            return n;
        }
        let id = self.graph.nodes.len() + 1;
        self.graph.nodes.push(Node {
            id,
            node_type: t,
            name: name.to_string(),
            status,
            path: None,
            attrs: Vec::new(),
        });
        self.virtual_node.insert((t, name.to_string()), id);
        id
    }

    /// The node of file `i`; true when it is new.
    fn file_node(&mut self, i: usize) -> (usize, bool) {
        let path = canonical(&self.files[i].path);
        if let Some(&n) = self.file_node.get(&path) {
            return (n, false);
        }
        let f = &self.files[i];
        let id = self.graph.nodes.len() + 1;
        self.graph.nodes.push(Node {
            id,
            node_type: f.scanned.node_type,
            name: f.scanned.name.clone().unwrap_or_default(),
            status: "FILE",
            path: Some(f.path.clone()),
            attrs: Vec::new(),
        });
        self.file_node.insert(path, id);
        (id, true)
    }

    fn run(&mut self) {
        // the candidate files: under the roots and the libraries
        let mut roots: Vec<PathBuf> = Vec::new();
        let mut all: Vec<PathBuf> = Vec::new();
        for r in &self.opts.roots {
            if r.is_dir() {
                let mut v = Vec::new();
                walk(r, &mut v);
                roots.extend(v.iter().cloned());
                all.extend(v);
            } else {
                roots.push(r.clone());
                all.push(r.clone());
            }
        }
        for l in &self.opts.libs.clone() {
            walk(l, &mut all);
        }
        let mut root_files: Vec<usize> = Vec::new();
        let root_set: HashSet<PathBuf> = roots.iter().map(|p| canonical(p)).collect();
        for p in all {
            let c = canonical(&p);
            if self.by_path.contains_key(&c) {
                continue;
            }
            let Some((text, size)) = read(&p) else {
                continue;
            };
            let Some(t) = scan::classify(&p, &text) else {
                continue;
            };
            let i = self.add_file(c.clone(), t, text, size);
            if root_set.contains(&c) {
                root_files.push(i);
            }
        }
        // the references, recursively from the roots
        let mut queue: VecDeque<usize> = VecDeque::new();
        for i in root_files {
            let (_, new) = self.file_node(i);
            if new {
                queue.push_back(i);
            }
        }
        // targets named by variables wait until the COPY relations are
        // known (a member may use a constant of the program copying it)
        let mut deferred: Vec<(usize, Ref)> = Vec::new();
        loop {
            while let Some(i) = queue.pop_front() {
                let refs = self.files[i].scanned.refs.clone();
                for r in refs {
                    if r.dynamic && r.identifier.is_some() {
                        deferred.push((i, r));
                    } else {
                        self.link(i, &r, &mut queue);
                    }
                }
            }
            if deferred.is_empty() {
                break;
            }
            for (i, r) in std::mem::take(&mut deferred) {
                let max = if r.edge == "CICS_TRANSID" { 4 } else { 8 };
                let ident = r.identifier.clone().unwrap_or_default();
                let lits = self.variable_values(i, &ident, max);
                if lits.is_empty() {
                    self.link(i, &r, &mut queue);
                }
                for l in lits {
                    let mut x = r.clone();
                    x.target = l;
                    x.dynamic = false;
                    self.link(i, &x, &mut queue);
                }
            }
        }
        self.node_attributes();
    }

    /// The constants variable `ident` may hold, from the file, the files
    /// copying it and the members they copy (VALUE, MOVE, INITIAL and
    /// assignments, followed through moves between variables).
    fn variable_values(&self, i: usize, ident: &str, max: usize) -> Vec<String> {
        let mut unit: HashSet<usize> = HashSet::from([i]);
        let mut up = vec![i];
        while let Some(f) = up.pop() {
            for &p in self.includers.get(&f).into_iter().flatten() {
                if unit.insert(p) {
                    up.push(p);
                }
            }
        }
        let mut down: Vec<usize> = unit.iter().copied().collect();
        while let Some(f) = down.pop() {
            for &m in self.members.get(&f).into_iter().flatten() {
                if unit.insert(m) {
                    down.push(m);
                }
            }
        }
        let mut values: scan::Values = scan::Values::new();
        let mut moves: Vec<(String, String)> = Vec::new();
        for &f in &unit {
            for (k, v) in &self.files[f].scanned.values {
                for l in v {
                    scan::add_value(&mut values, k, l);
                }
            }
            moves.extend(self.files[f].scanned.moves.iter().cloned());
        }
        for _ in 0..8 {
            let mut changed = false;
            for (from, to) in &moves {
                let add: Vec<String> = values.get(from).cloned().unwrap_or_default();
                for l in add {
                    let v = values.entry(to.clone()).or_default();
                    if !v.contains(&l) {
                        v.push(l);
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        values
            .get(ident)
            .map(|v| {
                v.iter()
                    .filter(|l| scan::is_name(l, max))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resolves reference `r` of file `i` and adds its edge (and the
    /// node of a newly reached file to the queue).
    fn link(&mut self, i: usize, r: &Ref, queue: &mut VecDeque<usize>) {
        let from = self.file_node(i).0;
        let (target, via) = self.resolve(r, i);
        let (to, resolution) = match target {
            Target::File(p, expected) => match self.file_at(&p, expected) {
                Some(j) => {
                    if matches!(r.edge, "COPY" | "SQL_INCLUDE" | "PLI_INCLUDE") {
                        self.members.entry(i).or_default().insert(j);
                        self.includers.entry(j).or_default().insert(i);
                    }
                    let (n, new) = self.file_node(j);
                    if new {
                        queue.push_back(j);
                    }
                    (n, "FILE")
                }
                None => (self.virtual_node(expected, &r.target, "MISSING"), "MISSING"),
            },
            Target::System(n, t, provider) => {
                let id = self.virtual_node(t, &n, "SYSTEM");
                let node = &mut self.graph.nodes[id - 1];
                if node.attrs.is_empty() {
                    node.attrs.push(("provider".into(), provider.into()));
                }
                (id, "SYSTEM")
            }
            Target::Missing(n, t)
                if r.edge == "HLASM_EXTERNAL"
                    && r.attrs.iter().any(|(k, v)| *k == "weak" && v == "true") =>
            {
                (self.virtual_node(t, &n, "OPTIONAL"), "OPTIONAL")
            }
            Target::Missing(n, t) => (self.virtual_node(t, &n, "MISSING"), "MISSING"),
            Target::Dynamic(n) => {
                let id = self.virtual_node("DYNAMIC_TARGET", &n, "DYNAMIC");
                let node = &mut self.graph.nodes[id - 1];
                if node.attrs.is_empty() {
                    node.attrs.push(("identifier".into(), n.clone()));
                }
                (id, "DYNAMIC")
            }
        };
        let text = self.files[i]
            .lines
            .get(r.line.saturating_sub(1))
            .map(|l| l.trim().to_string())
            .unwrap_or_default();
        let key = (from, to, r.edge);
        let e = match self.edge_of.get(&key) {
            Some(&e) => e,
            None => {
                let id = self.graph.edges.len() + 1;
                self.graph.edges.push(Edge {
                    id,
                    edge_type: r.edge,
                    from,
                    to,
                    target_name: r.target.clone(),
                    resolution,
                    attrs: BTreeMap::new(),
                    locations: Vec::new(),
                });
                self.edge_of.insert(key, id);
                id
            }
        };
        let edge = &mut self.graph.edges[e - 1];
        let loc = Location {
            line: r.line,
            col: r.col,
            text,
        };
        if !edge.locations.contains(&loc) {
            edge.locations.push(loc);
        }
        let mut add = |k: &str, v: &str| {
            if v.is_empty() {
                return;
            }
            let vals = edge.attrs.entry(k.to_string()).or_default();
            for x in v.split(' ').filter(|x| !x.is_empty()) {
                if !vals.iter().any(|y| y == x) {
                    vals.push(x.to_string());
                }
            }
        };
        for (k, v) in &r.attrs {
            add(k, v);
        }
        for v in via {
            add("via", v);
        }
    }

    fn node_attributes(&mut self) {
        let mut diagnostic_sources = HashMap::new();
        // copy / include directories for the compile checks
        let mut copy_dirs: Vec<PathBuf> = self.opts.libs.clone();
        for f in &self.files {
            if matches!(
                f.scanned.node_type,
                "COBOL_COPYBOOK" | "PLI_INCLUDE" | "HLASM_COPYBOOK"
            ) && let Some(d) = f.path.parent()
                && !copy_dirs.iter().any(|x| x == d)
            {
                copy_dirs.push(d.to_path_buf());
            }
        }
        // the symbolic maps of the mapsets (COBOL and PL/I members)
        let symbolic = tempfile::tempdir().ok();
        if let Some(d) = &symbolic {
            for f in self
                .files
                .iter()
                .filter(|f| f.scanned.node_type == "BMS_MAPSET")
            {
                let Ok(text) = std::fs::read_to_string(&f.path) else {
                    continue;
                };
                let Ok(ms) = zeus_bms::parse(&text) else {
                    continue;
                };
                let name = ms.name.to_ascii_uppercase();
                let _ = std::fs::write(
                    d.path().join(format!("{name}.cpy")),
                    zeus_bms::symbolic::cobol(&ms),
                );
                let _ = std::fs::write(
                    d.path().join(format!("{name}.inc")),
                    zeus_bms::symbolic::pli(&ms),
                );
            }
            copy_dirs.push(d.path().to_path_buf());
        }
        let mut referenced: HashMap<usize, usize> = HashMap::new();
        for e in &self.graph.edges {
            *referenced.entry(e.to).or_default() += 1;
        }
        let file_of: HashMap<usize, usize> = self
            .files
            .iter()
            .enumerate()
            .filter_map(|(i, f)| self.file_node.get(&canonical(&f.path)).map(|&n| (n, i)))
            .collect();
        for node in &mut self.graph.nodes {
            let mut attrs: Vec<(String, String)> = Vec::new();
            if let Some(&i) = file_of.get(&node.id) {
                let f = &self.files[i];
                attrs.push((
                    "file_name".into(),
                    f.path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                ));
                attrs.push(("size_bytes".into(), f.size.to_string()));
                attrs.push(("line_count".into(), f.lines.len().to_string()));
                for (k, v) in &f.scanned.attrs {
                    // attributes of other node types (a COBOL scan of a
                    // copy member) are not reported
                    if master::NODE_ATTRS
                        .iter()
                        .any(|a| a.code == *k && (a.owner == node.node_type || a.owner == "*"))
                    {
                        attrs.push((k.to_string(), v.clone()));
                    }
                }
                if self.opts.compile {
                    let (check_attrs, diagnostics) =
                        compile_check(node.node_type, &f.path, &copy_dirs);
                    attrs.extend(check_attrs);
                    self.graph
                        .diagnostics
                        .extend(diagnostics.into_iter().map(|mut d| {
                            d.node_id = node.id;
                            if d.program.is_empty() {
                                d.program = node.name.clone();
                            }
                            d.source_line(&mut diagnostic_sources);
                            d
                        }));
                } else if matches!(
                    node.node_type,
                    "COBOL_PROGRAM" | "PLI_PROGRAM" | "HLASM_PROGRAM"
                ) {
                    attrs.push(("compile_status".into(), "NOT_CHECKED".into()));
                }
            }
            attrs.append(&mut node.attrs);
            attrs.push((
                "referenced_by".into(),
                referenced.get(&node.id).copied().unwrap_or(0).to_string(),
            ));
            node.attrs = attrs;
        }
    }
}

/// Compiles a program with the Zeus compiler: its status, the first error
/// and the statements Zeus cannot run.
fn compile_check(
    node_type: &str,
    path: &Path,
    dirs: &[PathBuf],
) -> (Vec<(String, String)>, Vec<SyntaxDiagnostic>) {
    let mut out = Vec::new();
    let mut diagnostics = Vec::new();
    match node_type {
        "HLASM_PROGRAM" => {
            let report = zeus_cobol::hlasm::compile_report(path, dirs);
            let rc = report.return_code();
            out.push((
                "compile_status".into(),
                if report.module.is_some() {
                    if rc > 0 { "WARNING" } else { "OK" }
                } else {
                    "ERROR"
                }
                .into(),
            ));
            out.push((
                "compile_diagnostics_complete".into(),
                report.complete.to_string(),
            ));
            if !report.diagnostics.is_empty() {
                out.push((
                    "compile_message".into(),
                    report
                        .diagnostics
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                ));
            }
            for d in report.diagnostics {
                let unsupported =
                    d.category == "UNRESOLVED_OPERATION" || explicitly_unsupported(&d.message);
                diagnostics.push(SyntaxDiagnostic {
                    node_id: 0,
                    program: String::new(),
                    language: "HLASM",
                    category: if unsupported {
                        "UNSUPPORTED"
                    } else if d.severity >= 8 {
                        "ERROR"
                    } else if d.severity > 0 {
                        "WARNING"
                    } else {
                        "INFO"
                    },
                    stage: "COMPILE",
                    syntax: if unsupported {
                        d.operation
                    } else {
                        String::new()
                    },
                    path: PathBuf::from(d.file),
                    line: u32::try_from(d.line).ok().filter(|n| *n > 0),
                    statement: String::new(),
                    message: format!("severity {}: {}", d.severity, d.message),
                });
            }
            out.push(("compile_return_code".into(), rc.to_string()));
        }
        "COBOL_PROGRAM" => {
            let opts = zeus_cobol::front::FrontOptions {
                copy_dirs: dirs.to_vec(),
                ..Default::default()
            };
            let r = std::panic::catch_unwind(|| zeus_cobol::compile_file(path, &opts));
            match r {
                Ok(Ok(module)) => {
                    out.push(("compile_status".into(), "OK".into()));
                    let mut unsupported: Vec<String> = Vec::new();
                    for p in &module.programs {
                        for (i, ins) in p.code.iter().enumerate() {
                            if let zeus_cobol::zir::Instr::Unsupported(n) = ins {
                                if !unsupported.contains(n) {
                                    unsupported.push(n.clone());
                                }
                                let position = p
                                    .instr_stmt
                                    .get(i)
                                    .and_then(|&s| p.stmt_pos.get(s as usize));
                                diagnostics.push(SyntaxDiagnostic {
                                    node_id: 0,
                                    program: p.name.clone(),
                                    language: "COBOL",
                                    category: "UNSUPPORTED",
                                    stage: "RUNTIME",
                                    syntax: n.clone(),
                                    path: position
                                        .and_then(|&(file, _)| module.sources.get(file as usize))
                                        .map(PathBuf::from)
                                        .unwrap_or_else(|| path.to_path_buf()),
                                    line: position.map(|&(_, line)| line),
                                    statement: String::new(),
                                    message: format!(
                                        "Statement {n} is accepted but cannot be executed"
                                    ),
                                });
                            }
                        }
                    }
                    if !unsupported.is_empty() {
                        out.push(("unsupported".into(), unsupported.join(" ")));
                    }
                }
                Ok(Err(d)) => {
                    out.push(("compile_status".into(), "ERROR".into()));
                    out.push(("compile_message".into(), d.to_string()));
                    diagnostics.push(SyntaxDiagnostic {
                        node_id: 0,
                        program: String::new(),
                        language: "COBOL",
                        category: if explicitly_unsupported(&d.message) {
                            "UNSUPPORTED"
                        } else {
                            "ERROR"
                        },
                        stage: "COMPILE",
                        syntax: if explicitly_unsupported(&d.message) {
                            d.message.clone()
                        } else {
                            String::new()
                        },
                        path: d
                            .file
                            .as_ref()
                            .map(PathBuf::from)
                            .unwrap_or_else(|| path.to_path_buf()),
                        line: (d.pos.line > 0).then_some(d.pos.line),
                        statement: String::new(),
                        message: d.message,
                    });
                }
                Err(_) => {
                    out.push(("compile_status".into(), "ERROR".into()));
                    out.push(("compile_message".into(), "the compiler failed".into()));
                    diagnostics.push(compiler_failure("COBOL", path));
                }
            }
        }
        "PLI_PROGRAM" => {
            let opts = zeus_pli::Options {
                include_dirs: dirs.to_vec(),
                margins: None,
            };
            match std::panic::catch_unwind(|| zeus_pli::compile_file(path, &opts)) {
                Ok((rc, c)) => {
                    let status = match (rc, c.module.is_some()) {
                        (0, true) => "OK",
                        (r, true) if r <= 4 => "WARNING",
                        _ => "ERROR",
                    };
                    out.push(("compile_status".into(), status.into()));
                    if status != "OK"
                        && let Some(m) = c.messages.first()
                    {
                        out.push(("compile_message".into(), m.clone()));
                    }
                    for message in &c.messages {
                        diagnostics.push(pli_diagnostic(message, path));
                    }
                }
                Err(_) => {
                    out.push(("compile_status".into(), "ERROR".into()));
                    out.push(("compile_message".into(), "the compiler failed".into()));
                    diagnostics.push(compiler_failure("PL/I", path));
                }
            }
        }
        _ => {}
    }
    (out, diagnostics)
}

fn compiler_failure(language: &'static str, path: &Path) -> SyntaxDiagnostic {
    SyntaxDiagnostic {
        node_id: 0,
        program: String::new(),
        language,
        category: "COMPILER_FAILURE",
        stage: "COMPILE",
        syntax: String::new(),
        path: path.to_path_buf(),
        line: None,
        statement: String::new(),
        message: "the compiler failed".into(),
    }
}

fn pli_diagnostic(message: &str, fallback: &Path) -> SyntaxDiagnostic {
    // Split at the severity delimiter first, then split the path from the
    // right to preserve Windows drive letters and colons in diagnostics.
    let parsed = ["S", "E", "W", "I"].into_iter().find_map(|severity| {
        let (prefix, body) = message.split_once(&format!(": {severity}: "))?;
        let (file, line) = prefix.rsplit_once(':')?;
        Some((file, line.parse::<u32>().ok()?, severity, body))
    });
    let (file, line, severity, body) = match parsed {
        Some((file, line, severity, body)) => (PathBuf::from(file), Some(line), severity, body),
        None => (fallback.to_path_buf(), None, "E", message),
    };
    let unsupported = explicitly_unsupported(body);
    SyntaxDiagnostic {
        node_id: 0,
        program: String::new(),
        language: "PL/I",
        category: if unsupported {
            "UNSUPPORTED"
        } else if severity == "W" {
            "WARNING"
        } else if severity == "I" {
            "INFO"
        } else {
            "ERROR"
        },
        stage: "COMPILE",
        syntax: if unsupported {
            body.to_string()
        } else {
            String::new()
        },
        path: file,
        line,
        statement: String::new(),
        message: body.to_string(),
    }
}

/// Analyses the dependencies of the roots.
pub fn analyze(opts: &Options) -> Graph {
    let mut a = Analyzer {
        opts: opts.clone(),
        files: Vec::new(),
        by_path: HashMap::new(),
        index: HashMap::new(),
        transactions: HashMap::new(),
        graph: Graph::default(),
        file_node: HashMap::new(),
        virtual_node: HashMap::new(),
        edge_of: HashMap::new(),
        members: HashMap::new(),
        includers: HashMap::new(),
    };
    a.run();
    a.graph
}

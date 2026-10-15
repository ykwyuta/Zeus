//! The CSV files: the masters and the results of an analysis.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::graph::Graph;
use crate::master;

/// A CSV file being written (RFC 4180 quoting; optionally with a UTF-8
/// byte order mark for spreadsheet programs).
pub struct Csv {
    out: std::io::BufWriter<std::fs::File>,
}

impl Csv {
    pub fn create(path: &Path, header: &[&str], bom: bool) -> std::io::Result<Csv> {
        let mut out = std::io::BufWriter::new(std::fs::File::create(path)?);
        if bom {
            out.write_all("\u{feff}".as_bytes())?;
        }
        let mut c = Csv { out };
        c.row(header)?;
        Ok(c)
    }

    pub fn row<S: AsRef<str>>(&mut self, fields: &[S]) -> std::io::Result<()> {
        let line: Vec<String> = fields.iter().map(|f| field(f.as_ref())).collect();
        self.out.write_all(line.join(",").as_bytes())?;
        self.out.write_all(b"\r\n")
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}

fn field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// The file names of the outputs.
pub const MASTER_FILES: &[&str] = &[
    "node_types.csv",
    "edge_types.csv",
    "node_attribute_types.csv",
    "edge_attribute_types.csv",
    "system_assets.csv",
];
pub const RESULT_FILES: &[&str] = &[
    "nodes.csv",
    "edges.csv",
    "node_attributes.csv",
    "edge_attributes.csv",
    "edge_locations.csv",
    "missing_assets.csv",
    "unsupported_syntax.csv",
    "unsupported_syntax_locations.csv",
    "compile_diagnostics.csv",
];

/// Writes the masters to `dir`.
pub fn write_masters(dir: &Path, bom: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut c = Csv::create(
        &dir.join("node_types.csv"),
        &["node_type", "name", "is_file", "extensions", "description"],
        bom,
    )?;
    for t in master::NODE_TYPES {
        c.row(&[
            t.code,
            t.name,
            if t.is_file { "Y" } else { "N" },
            &t.extensions.join(" "),
            t.description,
        ])?;
    }
    c.finish()?;
    let mut c = Csv::create(
        &dir.join("edge_types.csv"),
        &[
            "edge_type",
            "name",
            "from_node_types",
            "to_node_types",
            "description",
        ],
        bom,
    )?;
    for t in master::EDGE_TYPES {
        c.row(&[
            t.code,
            t.name,
            &t.from.join(" "),
            &t.to.join(" "),
            t.description,
        ])?;
    }
    c.finish()?;
    for (file, owner, attrs) in [
        ("node_attribute_types.csv", "node_type", master::NODE_ATTRS),
        ("edge_attribute_types.csv", "edge_type", master::EDGE_ATTRS),
    ] {
        let mut c = Csv::create(
            &dir.join(file),
            &[owner, "attribute", "name", "data_type", "description"],
            bom,
        )?;
        for a in attrs {
            c.row(&[a.owner, a.code, a.name, a.data_type, a.description])?;
        }
        c.finish()?;
    }
    let mut c = Csv::create(
        &dir.join("system_assets.csv"),
        &["asset_name", "node_type", "provider", "description"],
        bom,
    )?;
    for a in master::SYSTEM_ASSETS {
        c.row(&[a.name, a.node_type, a.provider, a.description])?;
    }
    c.finish()
}

/// A path as reported: relative to `base` when under it.
fn shown(p: &Option<PathBuf>, base: &Path) -> String {
    match p {
        Some(p) => p.strip_prefix(base).unwrap_or(p).display().to_string(),
        None => String::new(),
    }
}

/// Writes the results of an analysis to `dir` (paths relative to `base`
/// when under it).
pub fn write_results(g: &Graph, dir: &Path, base: &Path, bom: bool) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let node = |id: usize| &g.nodes[id - 1];
    let mut c = Csv::create(
        &dir.join("nodes.csv"),
        &["node_id", "node_type", "name", "status", "path"],
        bom,
    )?;
    for n in &g.nodes {
        c.row(&[
            n.id.to_string(),
            n.node_type.into(),
            n.name.clone(),
            n.status.into(),
            shown(&n.path, base),
        ])?;
    }
    c.finish()?;
    let mut c = Csv::create(
        &dir.join("edges.csv"),
        &[
            "edge_id",
            "edge_type",
            "from_node_id",
            "from_name",
            "to_node_id",
            "to_name",
            "resolution",
            "occurrences",
        ],
        bom,
    )?;
    for e in &g.edges {
        c.row(&[
            e.id.to_string(),
            e.edge_type.into(),
            e.from.to_string(),
            node(e.from).name.clone(),
            e.to.to_string(),
            node(e.to).name.clone(),
            e.resolution.into(),
            e.locations.len().to_string(),
        ])?;
    }
    c.finish()?;
    let mut c = Csv::create(
        &dir.join("node_attributes.csv"),
        &["node_id", "node_type", "attribute", "value"],
        bom,
    )?;
    for n in &g.nodes {
        for (k, v) in &n.attrs {
            c.row(&[n.id.to_string(), n.node_type.into(), k.clone(), v.clone()])?;
        }
    }
    c.finish()?;
    let mut c = Csv::create(
        &dir.join("edge_attributes.csv"),
        &["edge_id", "edge_type", "attribute", "value"],
        bom,
    )?;
    for e in &g.edges {
        let mut rows: Vec<(String, String)> = vec![
            ("target_name".into(), e.target_name.clone()),
            ("occurrences".into(), e.locations.len().to_string()),
            ("resolution".into(), e.resolution.into()),
        ];
        for (k, v) in &e.attrs {
            rows.push((k.clone(), v.join(" ")));
        }
        for (k, v) in rows {
            c.row(&[e.id.to_string(), e.edge_type.into(), k, v])?;
        }
    }
    c.finish()?;
    let mut c = Csv::create(
        &dir.join("edge_locations.csv"),
        &[
            "edge_id",
            "seq",
            "node_id",
            "path",
            "line",
            "column",
            "statement",
        ],
        bom,
    )?;
    for e in &g.edges {
        let from = node(e.from);
        for (k, l) in e.locations.iter().enumerate() {
            c.row(&[
                e.id.to_string(),
                (k + 1).to_string(),
                from.id.to_string(),
                shown(&from.path, base),
                l.line.to_string(),
                l.col.to_string(),
                l.text.clone(),
            ])?;
        }
    }
    c.finish()?;
    let mut c = Csv::create(
        &dir.join("missing_assets.csv"),
        &[
            "asset_name",
            "node_type",
            "reason",
            "node_id",
            "referenced_by_node_id",
            "referenced_by_name",
            "referenced_by_path",
            "edge_id",
            "edge_type",
            "line",
            "column",
            "occurrences",
        ],
        bom,
    )?;
    for e in &g.edges {
        let to = node(e.to);
        let reason = match to.status {
            "MISSING" => "NOT_FOUND",
            "DYNAMIC" => "DYNAMIC",
            _ => continue,
        };
        let from = node(e.from);
        let first = e.locations.first();
        c.row(&[
            to.name.clone(),
            to.node_type.into(),
            reason.into(),
            to.id.to_string(),
            from.id.to_string(),
            from.name.clone(),
            shown(&from.path, base),
            e.id.to_string(),
            e.edge_type.into(),
            first.map(|l| l.line.to_string()).unwrap_or_default(),
            first.map(|l| l.col.to_string()).unwrap_or_default(),
            e.locations.len().to_string(),
        ])?;
    }
    c.finish()?;
    write_syntax_results(g, dir, base, bom)
}

/// Compiler-confirmed unsupported constructs, with separate general errors.
fn write_syntax_results(g: &Graph, dir: &Path, base: &Path, bom: bool) -> std::io::Result<()> {
    let mut groups = BTreeMap::new();
    for d in g.diagnostics.iter().filter(|d| d.category == "UNSUPPORTED") {
        groups
            .entry((d.language, d.syntax.as_str(), d.stage))
            .or_insert_with(Vec::new)
            .push(d);
    }
    let mut summary = Csv::create(
        &dir.join("unsupported_syntax.csv"),
        &[
            "syntax_id",
            "language",
            "syntax",
            "stage",
            "occurrences",
            "programs",
            "files",
        ],
        bom,
    )?;
    let mut locations = Csv::create(
        &dir.join("unsupported_syntax_locations.csv"),
        &[
            "syntax_id",
            "node_id",
            "program",
            "program_path",
            "language",
            "syntax",
            "stage",
            "path",
            "line",
            "statement",
            "message",
        ],
        bom,
    )?;
    for (i, ((language, syntax, stage), occurrences)) in groups.iter().enumerate() {
        let id = (i + 1).to_string();
        let programs: BTreeSet<_> = occurrences
            .iter()
            .map(|d| (d.node_id, &d.program))
            .collect();
        let files: BTreeSet<_> = occurrences.iter().map(|d| &d.path).collect();
        summary.row(&[
            id.clone(),
            language.to_string(),
            syntax.to_string(),
            stage.to_string(),
            occurrences.len().to_string(),
            programs.len().to_string(),
            files.len().to_string(),
        ])?;
        for d in occurrences {
            let root = &g.nodes[d.node_id - 1];
            locations.row(&[
                id.clone(),
                d.node_id.to_string(),
                d.program.clone(),
                shown(&root.path, base),
                d.language.into(),
                d.syntax.clone(),
                d.stage.into(),
                shown(&Some(d.path.clone()), base),
                d.line.map(|n| n.to_string()).unwrap_or_default(),
                d.statement.clone(),
                d.message.clone(),
            ])?;
        }
    }
    summary.finish()?;
    locations.finish()?;
    let mut diagnostics = Csv::create(
        &dir.join("compile_diagnostics.csv"),
        &[
            "node_id",
            "program",
            "program_path",
            "language",
            "category",
            "stage",
            "path",
            "line",
            "statement",
            "message",
        ],
        bom,
    )?;
    for d in &g.diagnostics {
        diagnostics.row(&[
            d.node_id.to_string(),
            d.program.clone(),
            shown(&g.nodes[d.node_id - 1].path, base),
            d.language.into(),
            d.category.into(),
            d.stage.into(),
            shown(&Some(d.path.clone()), base),
            d.line.map(|n| n.to_string()).unwrap_or_default(),
            d.statement.clone(),
            d.message.clone(),
        ])?;
    }
    diagnostics.finish()
}

//! Semantic analysis: scopes and declarations, structure mapping, and the
//! translation of statements and expressions into the typed IR.

mod decl;
mod exec;
pub mod expr;
mod stmt;

use std::collections::HashMap;

use crate::ast::{self, Attr, DeclItem, StmtKind};
use crate::builtin::Bif;
use crate::ir::*;
use crate::lexer::{Diag, Pos};
use crate::types::Ty;

pub type Result<T> = std::result::Result<T, Diag>;

/// What a name means in a block.
#[derive(Debug, Clone)]
pub enum Sym {
    Var(VarId),
    Label(LabelId),
    Proc(ProcId),
    Entry(ProcId, usize),
    EntryDecl(EntryDecl),
    File(FileId),
    Builtin(Bif),
    Cond(u32),
    /// Named constant (VALUE).
    Const(Expr),
    /// FORMAT statement label.
    Format(u32),
    /// GENERIC entry: its alternatives.
    Generic(Vec<crate::ast::GenericAlt>),
}

#[derive(Debug, Clone)]
pub struct EntryDecl {
    /// External name.
    pub name: String,
    /// Parameter descriptors (None: no descriptor list).
    pub params: Option<Vec<Option<Ty>>>,
    pub returns: Option<Ty>,
}

#[derive(Debug, Default)]
pub struct Scope {
    pub names: HashMap<String, Vec<Sym>>,
}

/// A procedure waiting for its body to be compiled.
/// A declared procedure waiting for its body.
pub struct Prepared {
    id: ProcId,
    proc: ast::Procedure,
    enabled: u16,
    nested: Vec<Prepared>,
}

pub struct PendingProc {
    id: ProcId,
    proc: ast::Procedure,
    prefixes: Vec<(String, bool)>,
}

pub struct Compiler {
    pub m: Module,
    pub diags: Vec<Diag>,
    pub scopes: Vec<Scope>,
    pub cur: BlockId,
    pub enabled: u16,
    pub file: u16,
    pub list_seq: u32,
    pub loop_seq: u32,
    /// Enclosing DO groups: their labels and loop ids.
    pub loops: Vec<(Vec<String>, LoopId)>,
    pub externals: HashMap<String, VarId>,
    /// REFER entries of the declaration being mapped: index, object name.
    pub pending_refers: Vec<(u32, String, Pos)>,
    /// EXEC SQL DECLARE CURSOR: text and host variables by cursor name.
    pub sql_cursors: HashMap<String, (String, HostVars)>,
    /// EXEC SQL WHENEVER: SQLERROR, SQLWARNING, NOT FOUND targets.
    pub whenever: [Option<String>; 3],
    /// REFER initial extents, compiled after the block's declarations.
    pub late_refers: Vec<(u32, crate::ast::Expr)>,
    /// The parameter descriptors and RETURNS of ENTRY variables.
    pub entry_sigs: HashMap<VarId, EntryDecl>,
    /// ORDINAL types: name and members (name, value) in order.
    pub ordinals: Vec<(String, Vec<(String, i64)>)>,
    /// The ordinal type of variables and of member names.
    pub ordinal_vars: HashMap<VarId, usize>,
    pub ordinal_members: HashMap<String, usize>,
    /// The rules of the DEFAULT statements.
    pub defaults: Vec<crate::ast::DefaultRule>,
    /// DEFINE STRUCTURE types: their members (levels from 1); DEFINE
    /// ALIAS types: their attributes.
    pub structs: HashMap<String, Vec<DeclItem>>,
    pub aliases: HashMap<String, Vec<crate::ast::Attr>>,
    /// The structure type of HANDLE variables.
    pub handles: HashMap<VarId, String>,
    /// Declarations of level-1 variables (for LIKE).
    pub decl_ast: HashMap<VarId, Vec<DeclItem>>,
    /// The outermost procedure block (implicit declarations).
    pub outer: BlockId,
    /// Nesting depth of aggregate-index placeholders.
    pub index_rank: u8,
    pub in_select_subject: Option<Ty>,
}

impl Compiler {
    pub fn new(ccsid: u32) -> Compiler {
        Compiler {
            m: Module {
                name: String::new(),
                ccsid,
                blocks: Vec::new(),
                procs: Vec::new(),
                vars: Vec::new(),
                files: Vec::new(),
                labels: Vec::new(),
                formats: Vec::new(),
                static_size: 0,
                static_init: Vec::new(),
                static_inits: Vec::new(),
                main: None,
                conditions: Vec::new(),
                sources: Vec::new(),
                source_paths: Vec::new(),
                stmt_lines: Vec::new(),
                refers: Vec::new(),
            },
            diags: Vec::new(),
            scopes: Vec::new(),
            cur: 0,
            enabled: cond::DEFAULT,
            file: 0,
            list_seq: 0,
            loop_seq: 0,
            loops: Vec::new(),
            externals: HashMap::new(),
            pending_refers: Vec::new(),
            sql_cursors: HashMap::new(),
            whenever: [None, None, None],
            late_refers: Vec::new(),
            entry_sigs: HashMap::new(),
            ordinals: Vec::new(),
            ordinal_vars: HashMap::new(),
            ordinal_members: HashMap::new(),
            defaults: Vec::new(),
            structs: HashMap::new(),
            aliases: HashMap::new(),
            handles: HashMap::new(),
            decl_ast: HashMap::new(),
            outer: 0,
            index_rank: 0,
            in_select_subject: None,
        }
    }

    pub fn err(&self, pos: Pos, msg: impl Into<String>) -> Diag {
        Diag::error(pos, msg)
    }

    pub fn warn(&mut self, pos: Pos, msg: impl Into<String>) {
        self.diags.push(Diag::warning(pos, msg));
    }

    pub fn new_block(&mut self, parent: Option<BlockId>, name: &str) -> BlockId {
        self.m.blocks.push(Block {
            parent,
            proc: None,
            name: name.to_string(),
            auto_size: 0,
            extents: Vec::new(),
            inits: Vec::new(),
            body: List {
                id: 0,
                stmts: Vec::new(),
            },
            on_unit: false,
        });
        self.scopes.push(Scope::default());
        (self.m.blocks.len() - 1) as BlockId
    }

    pub fn define(&mut self, block: BlockId, name: &str, sym: Sym) {
        self.scopes[block as usize]
            .names
            .entry(name.to_string())
            .or_default()
            .push(sym);
    }

    /// Symbols of a name visible from the current block, innermost first.
    pub fn lookup(&self, name: &str) -> Vec<(BlockId, Sym)> {
        let mut out = Vec::new();
        let mut b = Some(self.cur);
        while let Some(id) = b {
            if let Some(v) = self.scopes[id as usize].names.get(name) {
                out.extend(v.iter().map(|s| (id, s.clone())));
            }
            b = self.m.blocks[id as usize].parent;
        }
        out
    }

    pub fn new_list_id(&mut self) -> u32 {
        self.list_seq += 1;
        self.list_seq
    }

    /// Allocates automatic storage in a block's frame.
    pub fn alloc_auto(&mut self, block: BlockId, size: u32, align: u32, residue: u32) -> u32 {
        let b = &mut self.m.blocks[block as usize];
        let off = align_to(b.auto_size, align.max(1), residue);
        b.auto_size = off + size;
        off
    }

    pub fn alloc_static(&mut self, size: u32, align: u32, residue: u32) -> u32 {
        let off = align_to(self.m.static_size, align.max(1), residue);
        self.m.static_size = off + size;
        self.m.static_init.resize(self.m.static_size as usize, 0);
        off
    }

    /// File constant for a name (declared contextually when unknown).
    pub fn file_for(&mut self, name: &str, pos: Pos) -> Result<FileId> {
        for (_, s) in self.lookup(name) {
            match s {
                Sym::File(f) => return Ok(f),
                Sym::Var(_) => return Err(self.err(pos, format!("{name} is not a file"))),
                _ => {}
            }
        }
        Ok(self.declare_file(name, &[], 0))
    }

    pub fn declare_file(&mut self, name: &str, attrs: &[Attr], block: BlockId) -> FileId {
        let mut f = FileDecl {
            name: name.to_string(),
            ddname: name.chars().take(8).collect(),
            stream: true,
            print: name == "SYSPRINT",
            input: false,
            output: false,
            update: false,
            keyed: false,
            direct: false,
            regional: false,
            record_format: None,
            record_keys: None,
        };
        for a in attrs {
            match a {
                Attr::Record => f.stream = false,
                Attr::Stream => f.stream = true,
                Attr::Print => f.print = true,
                Attr::Input => f.input = true,
                Attr::Output => f.output = true,
                Attr::Update => f.update = true,
                Attr::Keyed => f.keyed = true,
                Attr::Direct => f.direct = true,
                Attr::Environment(e) => {
                    let upper = e.to_ascii_uppercase();
                    let words: Vec<_> = upper
                        .split(|c: char| !c.is_ascii_alphanumeric())
                        .filter(|s| !s.is_empty())
                        .collect();
                    f.regional |= words.contains(&"REGIONAL");
                    let number = |key: &str| {
                        words
                            .iter()
                            .position(|w| *w == key)
                            .and_then(|i| words.get(i + 1))
                            .and_then(|n| n.parse::<u32>().ok())
                    };
                    if words.iter().any(|w| matches!(*w, "V" | "VB")) {
                        f.record_format = Some(zeus_ds::format::RecordFormat::Variable);
                    } else if words.iter().any(|w| matches!(*w, "F" | "FB")) {
                        f.record_format = Some(zeus_ds::format::RecordFormat::Fixed(
                            number("RECSIZE").unwrap_or(80).max(1),
                        ));
                    }
                    if let Some(len) = number("KEYLENGTH") {
                        f.record_keys =
                            Some((len, number("KEYLOC").unwrap_or(1).saturating_sub(1)));
                    }
                }
                Attr::External(Some(n)) => f.ddname = n.chars().take(8).collect(),
                _ => {}
            }
        }
        if f.keyed || f.direct || f.update {
            f.stream = false;
        }
        self.m.files.push(f);
        let id = (self.m.files.len() - 1) as FileId;
        // files are known in the whole compilation (declared at package
        // level when contextual)
        self.define(block, name, Sym::File(id));
        id
    }

    /// Implicitly declares a variable in the outermost block (default
    /// attributes by first letter).
    pub fn implicit_var(&mut self, name: &str, pos: Pos) -> Result<VarId> {
        self.warn(pos, format!("{name} is implicitly declared"));
        let item = DeclItem {
            level: 1,
            name: name.to_string(),
            dims: Vec::new(),
            attrs: Vec::new(),
            pos,
        };
        let save = self.cur;
        self.cur = self.outer;
        let r = self.declare_items(&[item], self.outer, &[]);
        self.cur = save;
        r?;
        match self.scopes[self.outer as usize]
            .names
            .get(name)
            .and_then(|v| v.last())
        {
            Some(Sym::Var(v)) => Ok(*v),
            _ => Err(self.err(pos, format!("cannot declare {name}"))),
        }
    }

    /// Declares a procedure: its names, parameters, RETURNS and entry
    /// points, and (recursively) those of its internal procedures, so
    /// that calls can be compiled before the bodies.
    pub fn declare_proc(&mut self, pp: PendingProc, parent: Option<BlockId>) -> Result<Prepared> {
        let PendingProc { id, proc, prefixes } = pp;
        let block = self.m.procs[id as usize].block;
        let save = (self.cur, self.enabled);
        self.cur = block;
        if parent.is_none() {
            self.outer = block;
        }
        self.enabled = apply_prefixes(self.enabled, &prefixes);
        let mut all_params: Vec<String> = proc.params.clone();
        for s in &proc.body {
            if let StmtKind::Entry(p, _) = &s.kind {
                for n in p {
                    if !all_params.contains(n) {
                        all_params.push(n.clone());
                    }
                }
            }
        }
        let nested = self.scan_block(block, &proc.body, &all_params)?;
        let var_of = |me: &Compiler, n: &str| match me.scopes[block as usize]
            .names
            .get(n)
            .and_then(|v| v.last())
        {
            Some(Sym::Var(v)) => Some(*v),
            _ => None,
        };
        let mut params = Vec::new();
        for p in &proc.params {
            params.push(
                var_of(self, p)
                    .ok_or_else(|| self.err(proc.pos, format!("parameter {p} is not declared")))?,
            );
        }
        self.m.procs[id as usize].params = params;
        if let Some(r) = &proc.returns {
            let t = self.scalar_type(r, "R", proc.pos)?;
            self.m.procs[id as usize].returns = Some(t);
        }
        let mut entries = Vec::new();
        for (idx, s) in proc.body.iter().enumerate() {
            if let StmtKind::Entry(p, _) = &s.kind {
                let vars: Vec<VarId> = p.iter().filter_map(|n| var_of(self, n)).collect();
                for name in &s.labels {
                    entries.push((name.clone(), vars.clone(), idx));
                }
            }
        }
        self.m.procs[id as usize].entries = entries;
        let mut prepared = Vec::new();
        for n in nested {
            prepared.push(self.declare_proc(n, Some(block))?);
        }
        let enabled = self.enabled;
        self.cur = save.0;
        self.enabled = save.1;
        Ok(Prepared {
            id,
            proc,
            enabled,
            nested: prepared,
        })
    }

    /// Compiles the body of a declared procedure and of its internal
    /// procedures.
    pub fn body_proc(&mut self, p: Prepared) -> Result<()> {
        let block = self.m.procs[p.id as usize].block;
        let save = (self.cur, self.enabled, std::mem::take(&mut self.loops));
        self.cur = block;
        self.enabled = p.enabled;
        let body = self.list(&p.proc.body)?;
        self.m.blocks[block as usize].body = body;
        for n in p.nested {
            self.body_proc(n)?;
        }
        self.cur = save.0;
        self.enabled = save.1;
        self.loops = save.2;
        Ok(())
    }

    /// Declares the names of a block: DECLAREs, labels, internal
    /// procedures (returned for later compilation), FORMAT statements and
    /// parameters.
    fn scan_block(
        &mut self,
        block: BlockId,
        body: &[ast::Stmt],
        params: &[String],
    ) -> Result<Vec<PendingProc>> {
        let mut decls: Vec<DeclItem> = Vec::new();
        let mut labels: Vec<String> = Vec::new();
        let mut procs = Vec::new();
        collect(body, &mut decls, &mut labels, &mut procs);
        // internal procedures: entry constants of this block
        let mut pending = Vec::new();
        for (p, prefixes) in procs {
            let pid = self.new_proc(&p, block, false);
            for n in &p.names {
                self.define(block, n, Sym::Proc(pid));
            }
            for s in &p.body {
                if let StmtKind::Entry(_, _) = s.kind {
                    for (k, n) in s.labels.iter().enumerate() {
                        self.define(block, n, Sym::Entry(pid, k));
                    }
                }
            }
            pending.push(PendingProc {
                id: pid,
                proc: p,
                prefixes,
            });
        }
        for l in labels {
            self.m.labels.push(LabelDef {
                name: l.clone(),
                block,
                list: 0,
                index: 0,
            });
            let id = (self.m.labels.len() - 1) as LabelId;
            self.define(block, &l, Sym::Label(id));
        }
        // FORMAT statements
        self.scan_formats(block, body)?;
        // parameters not in a DECLARE get default attributes
        for p in params {
            if !decls.iter().any(|d| d.level == 1 && d.name == *p) {
                decls.push(DeclItem {
                    level: 1,
                    name: p.clone(),
                    dims: Vec::new(),
                    attrs: Vec::new(),
                    pos: Pos::default(),
                });
            }
        }
        self.declare_items(&decls, block, params)?;
        Ok(pending)
    }

    fn scan_formats(&mut self, block: BlockId, body: &[ast::Stmt]) -> Result<()> {
        for s in body {
            match &s.kind {
                StmtKind::Format(f) => {
                    let save = self.cur;
                    self.cur = block;
                    let fmts = self.formats(f)?;
                    self.cur = save;
                    self.m.formats.push(fmts);
                    let id = (self.m.formats.len() - 1) as u32;
                    for l in &s.labels {
                        // FORMAT labels replace the label constant
                        if let Some(v) = self.scopes[block as usize].names.get_mut(l) {
                            v.retain(|x| !matches!(x, Sym::Label(_)));
                        }
                        self.define(block, l, Sym::Format(id));
                    }
                }
                StmtKind::Do { body, .. } => self.scan_formats(block, body)?,
                _ => {}
            }
        }
        Ok(())
    }

    fn new_proc(&mut self, p: &ast::Procedure, parent: BlockId, external: bool) -> ProcId {
        let name = p.names.first().cloned().unwrap_or_default();
        let block = self.new_block(if external { None } else { Some(parent) }, &name);
        let main = p.options.iter().any(|o| o == "MAIN");
        self.m.procs.push(Proc {
            name: name.clone(),
            block,
            params: Vec::new(),
            returns: None,
            main,
            noexecops: p.options.iter().any(|o| o == "NOEXECOPS"),
            external,
            recursive: p.recursive,
            entries: Vec::new(),
        });
        let id = (self.m.procs.len() - 1) as ProcId;
        self.m.blocks[block as usize].proc = Some(id);
        id
    }

    /// Compiles a program: its external procedures.
    pub fn program(&mut self, stmts: &[ast::Stmt]) -> Result<()> {
        // package level: a block for files and external names
        let package = self.new_block(None, "*PACKAGE");
        self.outer = package;
        let mut procs = Vec::new();
        let mut decls = Vec::new();
        let mut flat: Vec<&ast::Stmt> = Vec::new();
        for s in stmts {
            if let StmtKind::Begin(body) = &s.kind {
                // PACKAGE: its declarations and procedures
                flat.extend(body.iter());
            } else {
                flat.push(s);
            }
        }
        for s in flat {
            match &s.kind {
                StmtKind::Procedure(p) => procs.push((p.clone(), s.prefixes.clone())),
                StmtKind::Declare(d) => decls.extend(d.iter().cloned()),
                StmtKind::Null => {}
                _ => return Err(self.err(s.pos, "statement outside a procedure")),
            }
        }
        // package-level declarations are STATIC
        let decls: Vec<DeclItem> = decls
            .into_iter()
            .map(|mut d| {
                if d.level == 1
                    && !d
                        .attrs
                        .iter()
                        .any(|a| matches!(a, Attr::Static | Attr::Based(_) | Attr::Controlled))
                {
                    d.attrs.push(Attr::Static);
                }
                d
            })
            .collect();
        self.cur = package;
        self.declare_items(&decls, package, &[])?;
        let mut pending = Vec::new();
        for (p, prefixes) in procs {
            let pid = self.new_proc(&p, package, true);
            self.m.blocks[self.m.procs[pid as usize].block as usize].parent = Some(package);
            for n in &p.names {
                self.define(package, n, Sym::Proc(pid));
            }
            if self.m.procs[pid as usize].main && self.m.main.is_none() {
                self.m.main = Some(pid);
            }
            pending.push(PendingProc {
                id: pid,
                proc: p,
                prefixes,
            });
        }
        if pending.is_empty() {
            return Err(self.err(Pos::default(), "no procedure in the source"));
        }
        self.m.name = self.m.procs[self.m.main.unwrap_or(0) as usize].name.clone();
        let mut prepared = Vec::new();
        for p in pending {
            self.cur = package;
            let outer = self.m.procs[p.id as usize].block;
            prepared.push((self.declare_proc(p, None)?, outer));
        }
        for (p, outer) in prepared {
            self.cur = package;
            self.outer = outer;
            self.body_proc(p)?;
        }
        self.outer = package;
        Ok(())
    }
}

/// Collects the declarations, statement labels and internal procedures of
/// a block (not of nested blocks).
fn collect(
    body: &[ast::Stmt],
    decls: &mut Vec<DeclItem>,
    labels: &mut Vec<String>,
    procs: &mut Vec<(ast::Procedure, Vec<(String, bool)>)>,
) {
    for s in body {
        match &s.kind {
            StmtKind::Procedure(p) => {
                procs.push((p.clone(), s.prefixes.clone()));
                continue;
            }
            StmtKind::Entry(..) | StmtKind::Format(_) => continue,
            _ => {}
        }
        labels.extend(s.labels.iter().cloned());
        match &s.kind {
            StmtKind::Declare(d) => decls.extend(d.iter().cloned()),
            StmtKind::Do { body, .. } => collect(body, decls, labels, procs),
            StmtKind::If { then, els, .. } => {
                collect(std::slice::from_ref(then), decls, labels, procs);
                if let Some(e) = els {
                    collect(std::slice::from_ref(e), decls, labels, procs);
                }
            }
            StmtKind::Select { whens, other, .. } => {
                for (_, s) in whens {
                    collect(std::slice::from_ref(s), decls, labels, procs);
                }
                if let Some(o) = other {
                    collect(std::slice::from_ref(o), decls, labels, procs);
                }
            }
            _ => {}
        }
    }
}

pub fn apply_prefixes(mut enabled: u16, prefixes: &[(String, bool)]) -> u16 {
    for (name, on) in prefixes {
        if let Some(b) = cond::bit(name) {
            if *on {
                enabled |= b;
            } else {
                enabled &= !b;
            }
        }
    }
    enabled
}

/// The smallest offset >= `at` that is `residue` modulo `align`.
pub fn align_to(at: u32, align: u32, residue: u32) -> u32 {
    let r = residue % align;
    at + (r + align - at % align) % align
}

/// Compiles parsed statements into a module.
pub fn compile(
    stmts: &[ast::Stmt],
    ccsid: u32,
    sources: Vec<String>,
) -> (Option<Module>, Vec<Diag>) {
    let mut c = Compiler::new(ccsid);
    c.m.source_paths = sources
        .iter()
        .map(|n| {
            std::fs::canonicalize(n)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| n.clone())
        })
        .collect();
    c.m.sources = sources;
    match c.program(stmts) {
        Ok(()) => {
            let mut m = c.m;
            m.stmt_lines.sort_unstable();
            m.stmt_lines.dedup();
            (Some(m), c.diags)
        }
        Err(d) => {
            c.diags.push(d);
            (None, c.diags)
        }
    }
}

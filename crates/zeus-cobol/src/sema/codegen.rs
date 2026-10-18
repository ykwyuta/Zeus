//! Code generation: AST procedure division to ZIR.

use std::collections::HashMap;

use zeus_decimal::Dec;

use super::data::{Area, DataModel, Settings};
use super::operand::rel;
use crate::diag::{Diag, Result};
use crate::ebcdic;
use crate::front::ast::{self, *};
use crate::front::source::Pos;
use crate::zir::{self, Cx, DataType, Ex, FileDef, Instr, Loc, Op, Src};

/// A jump target, resolved to an instruction index after generation.
type Label = u32;

pub struct Gen<'a> {
    entries: Vec<zir::EntryPoint>,
    nested: bool,
    /// EXEC statements (labels unresolved until the end).
    execs: Vec<zir::ExecDef>,
    /// SQL cursor declarations (DECLARE c CURSOR FOR ...) by cursor name.
    sql_cursors: HashMap<String, SqlText>,
    sqlims_whenever: HashMap<String, ProcName>,
    pub prog: &'a ast::Program,
    pub dm: DataModel,
    pub code: Vec<Instr>,
    labels: Vec<Option<u32>>,
    /// Paragraphs in code order: (name, section, start label).
    paras: Vec<(String, Option<String>, Label)>,
    /// Source position of each paragraph (file, line).
    para_pos: Vec<(u32, u32)>,
    /// Statements (file, line), the statement of each instruction, and
    /// the statement being generated.
    stmt_pos: Vec<(u32, u32)>,
    instr_stmt: Vec<u32>,
    cur_stmt: u32,
    /// The statement before the last position marker (the one containing
    /// the marked statement).
    marker_outer: u32,
    /// Section name -> (first paragraph, last paragraph).
    sections: HashMap<String, (u32, u32)>,
    /// Entry label of each named section with paragraphs (by first
    /// paragraph): precedes the first paragraph's label so that debugging
    /// can tell a reference to the section from one to the paragraph.
    section_labels: HashMap<u32, Label>,
    /// Paragraph ends: label of each paragraph's ParaEnd instruction.
    para_end_labels: Vec<Label>,
    current_para: u32,
    current_section: Option<String>,
    sentence_end: Option<Label>,
    /// Lexically enclosing inline PERFORMs: (exit, cycle).
    inline_performs: Vec<(Label, Label)>,
    temps: u32,
    alter_slots: HashMap<u32, u32>,
    alter_init: Vec<Label>,
    pub files: Vec<FileDef>,
    file_index: HashMap<String, u32>,
    pub symbolics: HashMap<String, u8>,
    pub classes: HashMap<String, Vec<u8>>,
    pub switch_conds: HashMap<String, (u32, bool)>,
    switch_names: HashMap<String, u32>,
    switches: Vec<String>,
    /// Segment number of each paragraph, and whether any segment is
    /// independent (50-99).
    para_segments: Vec<u8>,
    segmented: bool,
    /// USE FOR DEBUGGING (WITH DEBUGGING MODE only).
    dbg: Debugging,
    /// Generating declaratives (no debugging triggers there).
    in_decl: bool,
    in_label_decl: bool,
    /// Report Writer reports.
    reports: Vec<zir::ReportDef>,
    pub pos: Pos,
}

#[derive(Default)]
struct Debugging {
    decls: Vec<(u32, u32)>,
    /// Paragraph index -> (section, name).
    paras: HashMap<u32, (u32, String)>,
    /// Section name -> (section, name).
    sections: HashMap<String, (u32, String)>,
    all_procs: Option<u32>,
    /// Item descriptor -> (section, ALL REFERENCES, name).
    items: HashMap<u32, (u32, bool, String)>,
    /// File index -> (section, name).
    files: HashMap<u32, (u32, String)>,
    /// Instruction ranges of the statements generated so far inside the
    /// current statement.
    stmt_ranges: Vec<(usize, usize)>,
    /// Statements being generated: (first instruction, index into
    /// `stmt_ranges`, triggers already placed).
    stmt_stack: Vec<(usize, usize, bool)>,
}

impl Debugging {
    fn procs(&self) -> bool {
        self.all_procs.is_some() || !self.paras.is_empty() || !self.sections.is_empty()
    }
}

fn native_ordinal_byte(ord: u32) -> u8 {
    (ord.saturating_sub(1)).min(255) as u8
}

impl<'a> Gen<'a> {
    /// `outer` is the enclosing program's data model and module index;
    /// `ancestors` lists the containing programs, nearest first.
    pub fn new(
        prog: &'a ast::Program,
        outer: Option<(u32, &DataModel)>,
        ancestors: &[(u32, &'a ast::Program)],
    ) -> Result<Gen<'a>> {
        if prog.env.files.len() > 65535 {
            return Err(Diag::error(
                prog.pos,
                "at most 65535 SELECT file names are allowed",
            ));
        }
        if crate::front::lp64()
            && prog
                .procedure
                .sections
                .iter()
                .flat_map(|s| &s.paragraphs)
                .flat_map(|p| &p.sentences)
                .flatten()
                .any(|s| matches!(s,Stmt::GoTo {targets,..} if targets.is_empty()))
        {
            return Err(Diag::error(
                prog.pos,
                "LP(64) cannot use GO TO without a procedure name",
            ));
        }
        if crate::front::thread() && !prog.recursive {
            return Err(Diag::error(
                prog.pos,
                "THREAD requires PROGRAM-ID IS RECURSIVE",
            ));
        }
        if prog.recursive && (outer.is_some() || !prog.nested.is_empty() || prog.initial) {
            return Err(Diag::error(
                prog.pos,
                "RECURSIVE cannot be INITIAL, nested, or contain nested programs",
            ));
        }
        if prog.recursive
            && (prog.env.segment_limit.is_some()
                || prog
                    .procedure
                    .sections
                    .iter()
                    .chain(&prog.procedure.declaratives)
                    .any(|s| {
                    s.segment.is_some() || matches!(s.use_stmt, Some(UseStmt::Debugging(_)))
                        || s.paragraphs.iter().flat_map(|p| &p.sentences).flatten()
                            .any(|st| matches!(st, Stmt::GoTo { targets, .. } if targets.is_empty()))
                    }))
        {
            return Err(Diag::error(
                prog.pos,
                "RECURSIVE cannot use segmentation, USE FOR DEBUGGING, or GO TO without a procedure name",
            ));
        }
        let settings = Settings {
            decimal_comma: prog.env.decimal_point_comma,
            currency: prog
                .env
                .currency_sign
                .as_ref()
                .and_then(|s| s.chars().next())
                .unwrap_or('$'),
        };
        let mut dm = super::data::build(prog, &settings)?;
        if let Some((idx, odm)) = outer {
            for item in &odm.items {
                if item.global
                    && item.area == Area::Special
                    && let Some(name) = &item.name
                {
                    dm.by_name.remove(name);
                }
            }
            dm.import_globals(odm, idx);
        }
        let mut g = Gen {
            entries: Vec::new(),
            nested: outer.is_some(),
            prog,
            dm,
            code: Vec::new(),
            labels: Vec::new(),
            paras: Vec::new(),
            para_pos: Vec::new(),
            stmt_pos: Vec::new(),
            instr_stmt: Vec::new(),
            cur_stmt: u32::MAX,
            marker_outer: u32::MAX,
            sections: HashMap::new(),
            para_end_labels: Vec::new(),
            section_labels: HashMap::new(),
            current_para: 0,
            current_section: None,
            sentence_end: None,
            inline_performs: Vec::new(),
            temps: 0,
            alter_slots: HashMap::new(),
            alter_init: Vec::new(),
            files: Vec::new(),
            file_index: HashMap::new(),
            symbolics: HashMap::new(),
            classes: HashMap::new(),
            switch_conds: HashMap::new(),
            switch_names: HashMap::new(),
            switches: Vec::new(),
            para_segments: Vec::new(),
            segmented: false,
            dbg: Debugging::default(),
            in_decl: false,
            in_label_decl: false,
            reports: Vec::new(),
            execs: Vec::new(),
            sql_cursors: sql_cursors(prog),
            sqlims_whenever: HashMap::new(),
            pos: prog.pos,
        };
        g.environment()?;
        // A FALSE value is a declaration-time literal, never a data item
        // evaluated when SET executes. SYMBOLIC CHARACTERS are literals too.
        let normalize_symbolic = |value: &mut Operand| {
            if let Operand::Ref(r) = value
                && r.qualifiers.is_empty()
                && r.subscripts.is_empty()
                && r.refmod.is_none()
                && let Some(&byte) = g.symbolics.get(&r.name)
            {
                *value = Operand::Lit(Literal::Hex(vec![byte]));
            }
        };
        for item in &mut g.dm.items {
            if let Some(value) = &mut item.entry.false_value {
                normalize_symbolic(value);
                if let Operand::All(inner) = value {
                    normalize_symbolic(inner);
                }
                let literal = match value {
                    Operand::Lit(_) | Operand::Fig(_) => true,
                    Operand::All(inner) => {
                        matches!(inner.as_ref(), Operand::Lit(_) | Operand::Fig(_))
                    }
                    _ => false,
                };
                if !literal {
                    return Err(Diag::error(
                        item.pos,
                        "literal required for WHEN SET TO FALSE",
                    ));
                }
            }
        }
        // GLOBAL files of containing programs
        for f in g.dm.global_files.clone() {
            if g.file_index.contains_key(&f) {
                continue;
            }
            for &(idx, anc) in ancestors {
                let Some(fd) = anc.data.files.iter().find(|d| d.name == f && d.global) else {
                    continue;
                };
                if let Some(pos) = anc.env.files.iter().position(|fc| fc.name == f) {
                    g.declare_file_from(
                        &anc.env.files[pos],
                        fd,
                        &anc.env,
                        Some((idx, pos as u32)),
                    )?;
                }
                break;
            }
        }
        Ok(g)
    }

    fn environment(&mut self) -> Result<()> {
        let env = &self.prog.env;
        if let Some(item) = self
            .dm
            .items
            .iter()
            .find(|item| matches!(item.name.as_deref(), Some("SHIFT-IN" | "SHIFT-OUT")))
        {
            return Err(Diag::error(
                item.pos,
                "SHIFT-IN and SHIFT-OUT are read-only special registers",
            ));
        }
        for (name, ords, _alpha) in &env.symbolic_chars {
            if matches!(name.as_str(), "SHIFT-IN" | "SHIFT-OUT") {
                return Err(Diag::error(
                    self.prog.pos,
                    "SHIFT-IN and SHIFT-OUT cannot be redefined",
                ));
            }
            self.symbolics
                .insert(name.clone(), native_ordinal_byte(ords[0]));
        }
        self.symbolics.extend([
            ("SHIFT-IN".into(), crate::codepage::SI),
            ("SHIFT-OUT".into(), crate::codepage::SO),
        ]);
        for class in &env.classes {
            let mut set = Vec::new();
            for item in &class.items {
                match item {
                    AlphabetItem::Char(c) => set.extend(spec_bytes(c)),
                    AlphabetItem::Range(a, b) => {
                        let (a, b) = (spec_bytes(a)[0], spec_bytes(b)[0]);
                        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
                        set.extend(lo..=hi);
                    }
                }
            }
            self.classes.insert(class.name.clone(), set);
        }
        for (implementor, mnemonic) in &env.mnemonics {
            if implementor.starts_with("SWITCH") || implementor.starts_with("UPSI") {
                let idx = self.switch_index(implementor);
                self.switch_names.insert(mnemonic.clone(), idx);
            }
        }
        for (implementor, cond, on) in &env.switch_conditions {
            let idx = self.switch_index(implementor);
            self.switch_conds.insert(cond.clone(), (idx, *on));
        }
        for fc in &env.files {
            self.declare_file(fc)?;
        }
        Ok(())
    }

    fn switch_index(&mut self, implementor: &str) -> u32 {
        if let Some(i) = self.switches.iter().position(|s| s == implementor) {
            return i as u32;
        }
        self.switches.push(implementor.to_string());
        (self.switches.len() - 1) as u32
    }

    fn declare_file(&mut self, fc: &FileControl) -> Result<()> {
        let prog = self.prog;
        let fd = prog
            .data
            .files
            .iter()
            .find(|f| f.name == fc.name)
            .ok_or_else(|| Diag::error(fc.pos, format!("no FD for file {}", fc.name)))?;
        self.declare_file_from(fc, fd, &prog.env, None)
    }

    /// A RECORD KEY described outside the file's records (in WORKING-
    /// STORAGE, as some programs do): the key is taken at the same
    /// position within the file's record area.
    fn key_in_record(&self, l: Loc, area: usize, rec_desc: u32) -> Loc {
        let Some(i) = self.dm.items.iter().position(|it| it.desc == l.desc) else {
            return l;
        };
        let mut root = i;
        while let Some(p) = self.dm.items[root].parent {
            root = p;
        }
        if root == area || self.dm.items[root].area == self.dm.items[area].area {
            return l;
        }
        let item = &self.dm.items[i];
        let off = item.offset - self.dm.items[root].offset;
        Loc {
            desc: rec_desc,
            subs: Vec::new(),
            refmod: Some((
                Box::new(Ex::Lit(Dec::from_i64(off as i64 + 1))),
                Some(Box::new(Ex::Lit(Dec::from_i64(item.len as i64)))),
            )),
        }
    }

    fn declare_file_from(
        &mut self,
        fc: &FileControl,
        fd: &FileDescription,
        file_env: &Environment,
        owner: Option<(u32, u32)>,
    ) -> Result<()> {
        let (area, records) = self
            .dm
            .files
            .get(&fc.name)
            .cloned()
            .ok_or_else(|| Diag::error(fc.pos, "file record area missing"))?;
        let rec_desc = self.dm.items[area].desc;
        if fc.reserve.is_some() && fc.organization == Organization::LineSequential {
            return Err(Diag::error(
                fc.pos,
                "RESERVE is not supported for LINE SEQUENTIAL",
            ));
        }
        if file_env.apply_write_only.contains(&fc.name)
            && fc.organization != Organization::Sequential
        {
            return Err(Diag::error(
                fc.pos,
                "APPLY WRITE-ONLY requires QSAM sequential organization",
            ));
        }
        if let Some(padding) = &fc.padding
            && owner.is_none()
        {
            let valid = match padding {
                Operand::Lit(Literal::Str(s)) => s.chars().count() == 1,
                Operand::Lit(Literal::Hex(b)) => b.len() == 1,
                Operand::Lit(Literal::National(b)) => b.len() == 2,
                Operand::Fig(
                    Figurative::Space
                    | Figurative::HighValue
                    | Figurative::LowValue
                    | Figurative::Quote,
                ) => true,
                Operand::Ref(r) => {
                    let id = self.dm.resolve(r)?;
                    let item = &self.dm.items[id];
                    let ty = &self.dm.descs[item.desc as usize].ty;
                    !matches!(item.area, Area::File(_))
                        && (item.len == 1 && matches!(ty, DataType::Alphanumeric { .. })
                            || item.len == 2 && matches!(ty, DataType::National))
                        && (!fd.external || matches!(item.area, Area::External(_)))
                }
                _ => false,
            };
            if !valid {
                return Err(Diag::error(
                    fc.pos,
                    "PADDING CHARACTER requires a one-character alphanumeric/national literal or non-file data item",
                ));
            }
        }
        let max_len = self.dm.items[area].len;
        let min_len = records
            .iter()
            .map(|&r| self.dm.items[r].len)
            .min()
            .unwrap_or(max_len);
        // Physical QSAM format is independent of the logical record lengths:
        // even equal-length records can explicitly request an RDW with mode V.
        let qsam = fc.organization == Organization::Sequential && !fd.sort;
        let recording_mode = if qsam {
            let block_small = fd
                .block_contains
                .is_some_and(|(_, b, records)| !records && b > 0 && b < max_len);
            let inferred = if block_small {
                'S'
            } else if fd.record_min.is_some()
                && fd.record_min == fd.record_max
                && !fd.record_varying
            {
                'F'
            } else if fd.record_varying
                || fd.record_depending.is_some()
                || records
                    .iter()
                    .any(|&r| self.dm.descs[self.dm.items[r].desc as usize].odo.is_some())
                || min_len != max_len
                || fd
                    .record_min
                    .zip(fd.record_max)
                    .is_some_and(|(a, b)| a != b)
            {
                'V'
            } else {
                'F'
            };
            let mode = fd.recording_mode.unwrap_or(inferred);
            if mode == 'U' && fd.block_contains.is_some() {
                return Err(Diag::error(
                    fd.pos,
                    "RECORDING MODE U cannot specify BLOCK CONTAINS",
                ));
            }
            if mode == 'S' && fd.block_contains.is_none_or(|(_, _, records)| records) {
                return Err(Diag::error(
                    fd.pos,
                    "RECORDING MODE S requires BLOCK CONTAINS CHARACTERS",
                ));
            }
            Some(mode)
        } else {
            None
        };
        let code_set_ascii = if let Some(name) = &fd.code_set {
            let alphabet = file_env
                .alphabets
                .iter()
                .find(|a| &a.name == name)
                .ok_or_else(|| {
                    Diag::error(fd.pos, format!("CODE-SET alphabet {name} not defined"))
                })?;
            let ascii = match alphabet.kind {
                AlphabetKind::Native => false,
                AlphabetKind::StandardOne | AlphabetKind::StandardTwo => true,
                _ => {
                    return Err(Diag::error(
                        fd.pos,
                        "CODE-SET requires NATIVE, EBCDIC, STANDARD-1 or STANDARD-2",
                    ));
                }
            };
            // ASCII file conversion has its own code set; IBM specifies
            // that the CODEPAGE compilation option does not select it.
            if ascii && recording_mode == Some('S') {
                return Err(Diag::error(
                    fd.pos,
                    "RECORDING MODE S cannot use ASCII CODE-SET",
                ));
            }
            if !fd.sort {
                fn check(dm: &DataModel, id: usize) -> Result<()> {
                    let item = &dm.items[id];
                    if item.level == 66 || item.level == 88 {
                        return Ok(());
                    }
                    if item.children.is_empty()
                        && (item.usage != Usage::Display
                            || matches!(
                                dm.descs[item.desc as usize].ty,
                                DataType::National | DataType::Dbcs
                            )
                            || matches!(dm.descs[item.desc as usize].ty,
                                DataType::Numeric(n) if n.signed && !matches!(n.sign, zir::SignPos::LeadingSeparate | zir::SignPos::TrailingSeparate)))
                    {
                        return Err(Diag::error(
                            item.pos,
                            "CODE-SET requires USAGE DISPLAY and SIGN SEPARATE for signed numeric data",
                        ));
                    }
                    for &child in &item.children {
                        check(dm, child)?;
                    }
                    Ok(())
                }
                for &record in &records {
                    check(&self.dm, record)?;
                }
            }
            ascii && !fd.sort
        } else {
            false
        };
        // Items of a containing program that are not GLOBAL cannot be
        // named here; such clauses are dropped from the imported file.
        let opt_loc = |g: &mut Gen, r: &Option<DataRef>| -> Result<Option<Loc>> {
            match r {
                Some(r) => match g.loc(r) {
                    Ok(l) => Ok(Some(l)),
                    Err(_) if owner.is_some() => Ok(None),
                    Err(e) => Err(e),
                },
                None => Ok(None),
            }
        };
        let status = opt_loc(self, &fc.file_status)?;
        let vsam_status = opt_loc(self, &fc.vsam_status)?;
        if owner.is_none()
            && let Some(r) = &fc.vsam_status
        {
            let id = self.dm.resolve(r)?;
            let item = &self.dm.items[id];
            let desc = &self.dm.descs[item.desc as usize];
            let local = self.dm.local_storage.is_some_and(|(start, len)| {
                item.area == Area::Working && item.offset >= start && item.offset < start + len
            });
            if !matches!(
                fc.organization,
                Organization::Indexed | Organization::Relative
            ) || !matches!(item.area, Area::Working | Area::Linkage(_))
                || local
                || desc.ty != DataType::Group
                || desc.len != 6
                || !desc.dims.is_empty()
                || desc.odo.is_some()
                || !r.subscripts.is_empty()
                || r.refmod.is_some()
            {
                return Err(Diag::error(
                    r.pos,
                    "second FILE STATUS item requires a fixed 6-byte WORKING-STORAGE/LINKAGE group for a VSAM file",
                ));
            }
        }
        let relative_key = opt_loc(self, &fc.relative_key)?;
        let record_key =
            opt_loc(self, &fc.record_key)?.map(|l| self.key_in_record(l, area, rec_desc));
        let record_depending = opt_loc(self, &fd.record_depending)?;
        let mut alternate_keys = Vec::new();
        for (k, dup) in &fc.alternate_keys {
            if let Some(l) = opt_loc(self, &Some(k.clone()))? {
                alternate_keys.push((l, *dup));
            }
        }
        let linage = match &fd.linage {
            Some(l) if owner.is_none() => {
                let opt = |g: &mut Gen, o: &Option<Operand>| -> Result<Option<Ex>> {
                    o.as_ref().map(|o| g.ex_operand(o)).transpose()
                };
                let id = self.dm.linage_counters[&fd.name];
                Some(zir::LinageDef {
                    lines: self.ex_operand(&l.lines)?,
                    footing: opt(self, &l.footing)?,
                    top: opt(self, &l.top)?,
                    bottom: opt(self, &l.bottom)?,
                    counter: self.dm.items[id].desc,
                })
            }
            _ => None,
        };
        let def = FileDef {
            label_records: fd
                .label_records
                .iter()
                .map(|name| {
                    self.loc(&DataRef {
                        name: name.clone(),
                        qualifiers: vec![fc.name.clone()],
                        subscripts: vec![],
                        refmod: None,
                        pos: fd.pos,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            name: fc.name.clone(),
            assign: fc.assign.clone(),
            org: match fc.organization {
                Organization::Sequential => zir::Org::Sequential,
                Organization::LineSequential => zir::Org::LineSequential,
                Organization::Relative => zir::Org::Relative,
                Organization::Indexed => zir::Org::Indexed,
            },
            access: match fc.access {
                Access::Sequential => zir::AccessMode::Sequential,
                Access::Random => zir::AccessMode::Random,
                Access::Dynamic => zir::AccessMode::Dynamic,
            },
            optional: fc.optional,
            record: rec_desc,
            min_len: fd.record_min.unwrap_or(min_len).min(max_len),
            max_len,
            recording_mode,
            block_size: if qsam {
                fd.block_contains
                    .map(|(_, max, records)| {
                        if records && max != 0 {
                            max.saturating_mul(
                                max_len + if recording_mode == Some('V') { 4 } else { 0 },
                            )
                            .saturating_add(if recording_mode == Some('V') { 4 } else { 0 })
                        } else {
                            max
                        }
                    })
                    .or_else(|| {
                        (crate::front::block0() && recording_mode != Some('U')).then_some(0)
                    })
            } else {
                None
            },
            reserve: fc.reserve,
            write_only: file_env.apply_write_only.contains(&fc.name)
                || (qsam && recording_mode == Some('V') && crate::front::awo()),
            same_sort_area: file_env
                .same_sort_area
                .iter()
                .filter(|g| g.contains(&fc.name))
                .flat_map(|g| g.iter().filter(|name| **name != fc.name).cloned())
                .collect(),
            label_standard: fd.label_standard,
            code_set_ascii,
            same_area: if matches!(
                fc.organization,
                Organization::Relative | Organization::Indexed
            ) {
                file_env
                    .same_area
                    .iter()
                    .filter(|g| g.contains(&fc.name))
                    .flat_map(|g| g.iter().cloned())
                    .filter(|n| n != &fc.name)
                    .collect()
            } else {
                Vec::new()
            },
            record_depending,
            status,
            vsam_status,
            relative_key,
            record_key,
            alternate_keys,
            print: false,
            linage,
            sort: fd.sort,
            error_proc: None,
            external: fd.external,
            owner,
        };
        self.file_index
            .insert(fc.name.clone(), self.files.len() as u32);
        self.files.push(def);
        Ok(())
    }

    // ---- labels --------------------------------------------------------

    fn label(&mut self) -> Label {
        self.labels.push(None);
        (self.labels.len() - 1) as u32
    }

    fn bind(&mut self, l: Label) {
        self.labels[l as usize] = Some(self.code.len() as u32);
    }

    fn emit(&mut self, i: Instr) {
        if matches!(i, Instr::Perform { .. }) && !self.in_decl && self.dbg.procs() {
            self.code
                .push(Instr::DebugCause(zir::debug_cause::PERFORM, self.pos.line));
            self.instr_stmt.push(self.cur_stmt);
        }
        self.code.push(i);
        self.instr_stmt.push(self.cur_stmt);
    }

    fn temp(&mut self) -> u32 {
        self.temps += 1;
        self.temps - 1
    }

    // ---- procedure layout ----------------------------------------------

    fn collect_paragraphs(&mut self, sections: &[Section]) {
        for sec in sections {
            let first = self.paras.len() as u32;
            if sec.name.is_some() && !sec.paragraphs.is_empty() {
                let l = self.label();
                self.section_labels.insert(first, l);
            }
            let seg = sec.segment.unwrap_or(0).min(99) as u8;
            self.segmented |= seg >= 50;
            let n = sec.paragraphs.len().max(1);
            self.para_segments.extend(std::iter::repeat_n(seg, n));
            for p in &sec.paragraphs {
                // A paragraph consisting of a single GO TO can be ALTERed.
                let alterable = p.sentences.len() == 1
                    && matches!(p.sentences[0].as_slice(), [Stmt::Line(_), Stmt::GoTo { targets, depending: None, .. }]
                        if targets.len() <= 1 && !targets.iter().any(|t| t.name == "MORE-LABELS"));
                if alterable {
                    let slot = self.alter_init.len() as u32;
                    self.alter_init.push(u32::MAX);
                    self.alter_slots.insert(self.paras.len() as u32, slot);
                }
                let l = self.label();
                let name = p.name.clone().unwrap_or_default();
                self.paras.push((name, sec.name.clone(), l));
                self.para_pos.push((p.pos.file, p.pos.line));
                let e = self.label();
                self.para_end_labels.push(e);
            }
            if sec.paragraphs.is_empty() {
                let l = self.label();
                self.paras.push((String::new(), sec.name.clone(), l));
                self.para_pos.push((sec.pos.file, sec.pos.line));
                let e = self.label();
                self.para_end_labels.push(e);
            }
            let last = self.paras.len() as u32 - 1;
            if let Some(n) = &sec.name {
                self.sections.insert(n.clone(), (first, last));
            }
        }
    }

    /// Resolves a procedure-name to (first paragraph, last paragraph).
    fn proc_range(&self, p: &ProcName) -> Result<(u32, u32)> {
        if p.section.is_none()
            && let Some(&(a, b)) = self.sections.get(&p.name)
        {
            // A paragraph of the same name in the current section wins.
            let local = self
                .paras
                .iter()
                .position(|(n, s, _)| n == &p.name && s.is_some() && s == &self.current_section);
            if local.is_none() {
                return Ok((a, b));
            }
        }
        let matches: Vec<u32> = self
            .paras
            .iter()
            .enumerate()
            .filter(|(_, (n, s, _))| {
                n == &p.name
                    && match &p.section {
                        Some(sec) => s.as_ref() == Some(sec),
                        None => true,
                    }
            })
            .map(|(i, _)| i as u32)
            .collect();
        let pick = match matches.len() {
            0 => None,
            1 => Some(matches[0]),
            _ => matches
                .iter()
                .copied()
                .find(|&i| self.paras[i as usize].1 == self.current_section),
        };
        match pick {
            Some(i) => Ok((i, i)),
            None => Err(Diag::error(
                p.pos,
                format!("procedure {} is not defined (or ambiguous)", p.name),
            )),
        }
    }

    /// ZIR form of an EXEC block: option arguments resolved to data
    /// areas, values, procedure labels or names.
    fn sqlims_whenever(&mut self, e: &ExecBlock) -> Result<()> {
        let words: Vec<_> = e
            .sql
            .as_ref()
            .map(|s| s.text.split_whitespace().collect())
            .unwrap_or_default();
        let mut i = 1;
        let condition = if words.get(i).is_some_and(|w| w.eq_ignore_ascii_case("NOT")) {
            i += 1;
            if !words
                .get(i)
                .is_some_and(|w| w.eq_ignore_ascii_case("FOUND"))
            {
                return Err(Diag::error(e.pos, "WHENEVER NOT FOUND expected"));
            }
            "NOTFOUND"
        } else if words
            .get(i)
            .is_some_and(|w| w.eq_ignore_ascii_case("SQLERROR"))
        {
            "SQLERROR"
        } else if words
            .get(i)
            .is_some_and(|w| w.eq_ignore_ascii_case("SQLWARNING"))
        {
            "SQLWARNING"
        } else {
            return Err(Diag::error(e.pos, "WHENEVER condition expected"));
        };
        i += 1;
        if words
            .get(i)
            .is_some_and(|w| w.eq_ignore_ascii_case("CONTINUE"))
            && words.len() == i + 1
        {
            self.sqlims_whenever.remove(condition);
            return Ok(());
        }
        if words.get(i).is_some_and(|w| w.eq_ignore_ascii_case("GO")) {
            i += 1;
            if !words.get(i).is_some_and(|w| w.eq_ignore_ascii_case("TO")) {
                return Err(Diag::error(e.pos, "GO TO expected"));
            }
        } else if !words.get(i).is_some_and(|w| w.eq_ignore_ascii_case("GOTO")) {
            return Err(Diag::error(e.pos, "WHENEVER requires CONTINUE or GO TO"));
        }
        i += 1;
        if words.len() != i + 1 {
            return Err(Diag::error(
                e.pos,
                "WHENEVER requires an unqualified procedure name",
            ));
        }
        let target = ProcName {
            name: words[i].to_ascii_uppercase(),
            section: None,
            pos: e.pos,
        };
        self.proc_start(&target)?;
        self.sqlims_whenever.insert(condition.into(), target);
        Ok(())
    }

    fn exec_def(&mut self, e: &ExecBlock) -> Result<zir::ExecDef> {
        let kind = match e.kind {
            ExecKind::Cics => zir::ExecKind::Cics,
            ExecKind::Sql => zir::ExecKind::Sql,
            ExecKind::SqlIms => zir::ExecKind::SqlIms,
            ExecKind::Dli => zir::ExecKind::Dli,
        };
        let mut options = Vec::new();
        for o in &e.options {
            if e.kind == ExecKind::Dli
                && o.name == "WHERE"
                && let Some(ExecArg::Raw(t)) = &o.arg
            {
                options.extend(self.dli_where(t, e.pos)?);
                continue;
            }
            let arg = match &o.arg {
                None => None,
                Some(ExecArg::Raw(t)) => Some(zir::ExecVal::Name(t.clone())),
                Some(ExecArg::Expr(x)) => Some(self.exec_val(x, e.kind)?),
            };
            options.push(zir::ExecOpt {
                name: o.name.clone(),
                arg,
            });
        }
        // EXEC DLI reports to the DL/I interface block
        if e.kind == ExecKind::Dli {
            let dib = DataRef {
                name: "DLZDIB".into(),
                qualifiers: Vec::new(),
                subscripts: Vec::new(),
                refmod: None,
                pos: e.pos,
            };
            if self.dm.resolve(&dib).is_ok() {
                options.push(zir::ExecOpt {
                    name: "DIB".into(),
                    arg: Some(zir::ExecVal::Area(self.loc(&dib)?)),
                });
            }
        }
        // BMS: SEND MAP without FROM and RECEIVE MAP without INTO / SET use
        // the symbolic map named after the map (mapO / mapI).
        if e.kind == ExecKind::Cics && (e.command == "SEND" || e.command == "RECEIVE") {
            let has = |n: &str| e.options.iter().any(|o| o.name == n);
            let map = e
                .options
                .iter()
                .find(|o| o.name == "MAP")
                .and_then(|o| match &o.arg {
                    Some(ExecArg::Expr(Expr::Operand(Operand::Lit(Literal::Str(s))))) => {
                        Some(s.trim().to_ascii_uppercase())
                    }
                    _ => None,
                });
            let (opt, suffix, wanted) = if e.command == "SEND" {
                ("FROM", 'O', !has("FROM") && !has("MAPONLY"))
            } else {
                ("INTO", 'I', !has("INTO") && !has("SET"))
            };
            if let (Some(map), true) = (map, wanted) {
                let r = DataRef {
                    name: format!("{map}{suffix}"),
                    qualifiers: Vec::new(),
                    subscripts: Vec::new(),
                    refmod: None,
                    pos: e.pos,
                };
                if self.dm.resolve(&r).is_ok() {
                    options.push(zir::ExecOpt {
                        name: opt.to_string(),
                        arg: Some(zir::ExecVal::Area(self.loc(&r)?)),
                    });
                }
            }
        }
        let mut host = Vec::new();
        let mut host_varchar = Vec::new();
        let mut sql = e.sql.clone();
        if matches!(e.kind, ExecKind::Sql | ExecKind::SqlIms) {
            // OPEN: the query and host variables of the cursor declaration
            // (the declaration itself is not executable)
            if e.command == "OPEN"
                && let Some(text) = &sql
                && let Some(name) = text.text.split_whitespace().nth(1)
            {
                let name = format!("{:?}:{}", e.kind, name.to_ascii_uppercase());
                let Some(decl) = self.sql_cursors.get(&name).cloned() else {
                    return Err(Diag::error(e.pos, format!("cursor {name} is not declared")));
                };
                let query = cursor_query(&decl.text).unwrap_or_default();
                options.push(zir::ExecOpt {
                    name: "QUERY".into(),
                    arg: Some(zir::ExecVal::Name(query)),
                });
                // the cursor's host variables, then OPEN's (USING)
                let mut host = decl.host.clone();
                host.extend(text.host.iter().cloned());
                sql = Some(SqlText {
                    text: text.text.clone(),
                    host,
                });
            }
            // the SQLCA the statement reports to
            let ca = DataRef {
                name: if e.kind == ExecKind::SqlIms {
                    "SQLIMSCA"
                } else {
                    "SQLCA"
                }
                .into(),
                qualifiers: Vec::new(),
                subscripts: Vec::new(),
                refmod: None,
                pos: e.pos,
            };
            if self.dm.resolve(&ca).is_ok() {
                if e.kind == ExecKind::SqlIms {
                    let id = self.dm.resolve(&ca)?;
                    if self.dm.items[id].len != 224 {
                        return Err(Diag::error(
                            e.pos,
                            "SQLIMSCA must contain exactly 224 bytes",
                        ));
                    }
                }
                options.push(zir::ExecOpt {
                    name: ca.name.clone(),
                    arg: Some(zir::ExecVal::Area(self.loc(&ca)?)),
                });
            } else if e.kind == ExecKind::SqlIms
                && !matches!(
                    e.command.as_str(),
                    "DECLARE" | "BEGIN" | "END" | "WHENEVER" | "INCLUDE"
                )
            {
                return Err(Diag::error(e.pos, "executable SQLIMS requires SQLIMSCA"));
            }
        }
        if let Some(sql) = &sql {
            for (index, h) in sql.host.iter().enumerate() {
                let descriptor = e.kind == ExecKind::SqlIms
                    && (e.command == "DESCRIBE"
                        || (e.command == "FETCH"
                            && sql
                                .text
                                .split_whitespace()
                                .any(|w| w.eq_ignore_ascii_case("DESCRIPTOR")))
                        || (e.command == "PREPARE"
                            && index == 0
                            && sql
                                .text
                                .split_whitespace()
                                .nth(2)
                                .is_some_and(|w| w.eq_ignore_ascii_case("INTO"))));
                let refs = if e.kind == ExecKind::SqlIms && !descriptor {
                    self.sqlims_host_fields(&h.var)?
                } else {
                    vec![h.var.clone()]
                };
                for (n, r) in refs.iter().enumerate() {
                    let v = self.loc(r)?;
                    let indicator = if let Some(indicator) = &h.indicator {
                        let mut indicator = indicator.clone();
                        let id = self.dm.resolve(&indicator)?;
                        if let Some(occurs) = &self.dm.items[id].occurs
                            && e.kind == ExecKind::SqlIms
                        {
                            if n >= occurs.max as usize {
                                None
                            } else {
                                indicator.subscripts.push(Subscript::Expr(Expr::Operand(
                                    Operand::Lit(Literal::Num((n + 1).to_string())),
                                )));
                                Some(indicator)
                            }
                        } else if n == 0 {
                            Some(indicator)
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    let i = indicator.as_ref().map(|r| self.loc(r)).transpose()?;
                    if e.kind == ExecKind::SqlIms
                        && let Some(i) = &i
                    {
                        let desc = &self.dm.descs[i.desc as usize];
                        if desc.len != 2
                            || !matches!(
                                desc.ty,
                                DataType::Numeric(zir::NumType {
                                    usage: zir::NumUsage::Binary | zir::NumUsage::Native,
                                    signed: true,
                                    scale: 0,
                                    ..
                                })
                            )
                        {
                            return Err(Diag::error(
                                e.pos,
                                "SQLIMS indicator must be a signed binary halfword",
                            ));
                        }
                    }
                    host_varchar.push(self.is_varchar(r));
                    host.push((v, i));
                }
            }
        }
        if e.kind == ExecKind::SqlIms {
            for (condition, target) in &self.sqlims_whenever {
                options.push(zir::ExecOpt {
                    name: condition.clone(),
                    arg: Some(zir::ExecVal::Label(self.proc_start(target)?)),
                });
            }
        }
        Ok(zir::ExecDef {
            kind,
            line: e.pos.line,
            command: e.command.clone(),
            options,
            sql: sql.as_ref().map(|s| s.text.clone()),
            host,
            host_varchar,
        })
    }

    /// `WHERE (field op operand)` of EXEC DLI: the option WHERE names the
    /// field and the operator, WHEREVAL is the operand (a data area, or
    /// the literal text in WHERE).
    fn dli_where(&mut self, text: &str, pos: Pos) -> Result<Vec<zir::ExecOpt>> {
        let words: Vec<&str> = text.split_whitespace().collect();
        let ops = [
            "=", "EQ", "^=", "¬=", "NE", ">", "GT", "<", "LT", ">=", "GE", "<=", "LE", "=>", "=<",
        ];
        let Some(at) = words
            .iter()
            .position(|w| ops.contains(&w.to_ascii_uppercase().as_str()))
        else {
            return Err(Diag::error(
                pos,
                format!("DL/I qualification expected: {text}"),
            ));
        };
        if words
            .iter()
            .any(|w| matches!(w.to_ascii_uppercase().as_str(), "AND" | "OR" | "&" | "|"))
        {
            return Err(Diag::error(
                pos,
                format!("only one DL/I qualification is supported: {text}"),
            ));
        }
        let field = words[..at].join("");
        let op = words[at].to_ascii_uppercase();
        let operand = &words[at + 1..];
        // a data name, possibly qualified (NAME OF GROUP)
        let mut parts = operand
            .split(|w| w.eq_ignore_ascii_case("OF") || w.eq_ignore_ascii_case("IN"))
            .map(|p| p.join(""));
        let name = parts.next().unwrap_or_default().to_ascii_uppercase();
        let r = DataRef {
            name,
            qualifiers: parts.map(|q| q.to_ascii_uppercase()).collect(),
            subscripts: Vec::new(),
            refmod: None,
            pos,
        };
        let value = if !r.name.is_empty() && self.dm.resolve(&r).is_ok() {
            Some(zir::ExecVal::Area(self.loc(&r)?))
        } else {
            None
        };
        let mut out = vec![zir::ExecOpt {
            name: "WHERE".into(),
            arg: Some(zir::ExecVal::Name(match &value {
                Some(_) => format!("{field} {op}"),
                None => format!("{field} {op} {}", operand.join(" ")),
            })),
        }];
        if let Some(v) = value {
            out.push(zir::ExecOpt {
                name: "WHEREVAL".into(),
                arg: Some(v),
            });
        }
        Ok(out)
    }

    /// A VARCHAR host variable: a group of a halfword length and the
    /// text, both level 49 (as DCLGEN writes them).
    fn sqlims_host_fields(&self, r: &DataRef) -> Result<Vec<DataRef>> {
        let id = self.dm.resolve(r)?;
        let item = &self.dm.items[id];
        if item.children.is_empty() || self.is_varchar(r) {
            return Ok(vec![r.clone()]);
        }
        let mut refs = Vec::new();
        for &child in &item.children {
            let child = &self.dm.items[child];
            let Some(name) = &child.name else { continue };
            if child.entry.redefines.is_some() || child.occurs.is_some() {
                return Err(Diag::error(
                    r.pos,
                    "SQLIMS host structure cannot contain REDEFINES or OCCURS",
                ));
            }
            let mut field = r.clone();
            field.name = name.clone();
            field.qualifiers.insert(0, r.name.clone());
            refs.extend(self.sqlims_host_fields(&field)?);
        }
        Ok(refs)
    }

    fn is_varchar(&self, r: &DataRef) -> bool {
        let Ok(id) = self.dm.resolve(r) else {
            return false;
        };
        let it = &self.dm.items[id];
        match it.children.as_slice() {
            [len, text] => {
                let (l, t) = (&self.dm.items[*len], &self.dm.items[*text]);
                l.level == 49 && t.level == 49 && l.len == 2 && l.pic.is_some()
            }
            _ => false,
        }
    }

    fn exec_val(&mut self, x: &Expr, kind: ExecKind) -> Result<zir::ExecVal> {
        Ok(match x {
            Expr::Operand(Operand::Ref(r)) => {
                if self.dm.resolve(r).is_ok() {
                    zir::ExecVal::Area(self.loc(r)?)
                } else {
                    let p = ProcName {
                        name: r.name.clone(),
                        section: r.qualifiers.first().cloned(),
                        pos: r.pos,
                    };
                    match self.proc_start(&p) {
                        Ok(l) => zir::ExecVal::Label(l),
                        // DL/I segment and PCB names are not data items
                        Err(_) if kind == ExecKind::Dli => zir::ExecVal::Name(r.name.clone()),
                        Err(_) => {
                            self.dm.resolve(r)?;
                            unreachable!()
                        }
                    }
                }
            }
            Expr::Operand(Operand::AddressOf(r)) => zir::ExecVal::Address(self.loc(r)?),
            _ => zir::ExecVal::Value(self.src_expr(x)?),
        })
    }

    fn proc_start(&self, p: &ProcName) -> Result<Label> {
        let (a, _) = self.proc_range(p)?;
        Ok(self.entry_label(p, a))
    }

    /// Label where control enters procedure `p` whose first paragraph is
    /// `first`: the section entry for a section name.
    fn entry_label(&self, p: &ProcName, first: u32) -> Label {
        let section = p.section.is_none()
            && self.sections.get(&p.name).is_some_and(|&(a, _)| a == first)
            && self.paras[first as usize].0 != p.name;
        match self.section_labels.get(&first) {
            Some(&l) if section => l,
            _ => self.paras[first as usize].2,
        }
    }

    // ---- program -------------------------------------------------------

    pub fn generate(mut self, parent: Option<u32>) -> Result<zir::ProgramCode> {
        let proc = &self.prog.procedure;
        self.collect_paragraphs(&proc.sections);
        let decl_start = self.paras.len();
        self.collect_paragraphs(&proc.declaratives);
        self.mark_print_files(&proc.sections);
        self.setup_debugging()?;
        self.build_reports()?;

        for e in &self.prog.data.execs {
            if e.kind == ExecKind::SqlIms && e.command == "WHENEVER" {
                self.sqlims_whenever(e)?;
            }
        }

        let mut para_idx = 0u32;
        for sec in &proc.sections {
            para_idx = self.gen_section(sec, para_idx)?;
        }
        // Falling off the end of the procedure division.
        self.emit(Instr::GoBack);
        self.in_decl = true;
        let mut para_idx = decl_start as u32;
        let mut uses = Vec::new();
        let mut label_uses: Vec<zir::LabelUse> = Vec::new();
        for sec in &proc.declaratives {
            let first = para_idx;
            para_idx = self.gen_section(sec, para_idx)?;
            if let Some(UseStmt::Label {
                target,
                global,
                beginning,
                reel,
            }) = &sec.use_stmt
            {
                let (files, mode) = match target {
                    UseTarget::Files(names) => (
                        names
                            .iter()
                            .map(|name| self.file(name))
                            .collect::<Result<Vec<_>>>()?,
                        None,
                    ),
                    UseTarget::Input => (vec![], Some(zir::OpenMode::Input)),
                    UseTarget::Output => (vec![], Some(zir::OpenMode::Output)),
                    UseTarget::InputOutput => (vec![], Some(zir::OpenMode::InputOutput)),
                    UseTarget::Extend => (vec![], Some(zir::OpenMode::Extend)),
                };
                for &file in &files {
                    if self.files[file as usize].sort || !self.files[file as usize].label_standard {
                        return Err(Diag::error(
                            self.pos,
                            "LABEL declarative requires a nonsort file with standard labels",
                        ));
                    }
                }
                for prior in &label_uses {
                    let same_target = if files.is_empty() && prior.files.is_empty() {
                        mode == prior.mode
                    } else {
                        files.iter().any(|file| prior.files.contains(file))
                    };
                    if same_target
                        && (beginning.is_none()
                            || prior.beginning.is_none()
                            || *beginning == prior.beginning)
                        && (reel.is_none() || prior.reel.is_none() || *reel == prior.reel)
                    {
                        return Err(Diag::error(
                            self.pos,
                            "overlapping LABEL declaratives request multiple procedures for the same event",
                        ));
                    }
                }
                label_uses.push(zir::LabelUse {
                    files,
                    mode,
                    start: first,
                    end: para_idx - 1,
                    global: *global,
                    beginning: *beginning,
                    reel: *reel,
                });
            }
            if let Some(UseStmt::Error(target, global)) = &sec.use_stmt {
                let (files, mode) = match target {
                    UseTarget::Files(names) => (
                        names
                            .iter()
                            .filter_map(|n| self.file_index.get(n).copied())
                            .collect(),
                        None,
                    ),
                    UseTarget::Input => (vec![], Some(zir::OpenMode::Input)),
                    UseTarget::Output => (vec![], Some(zir::OpenMode::Output)),
                    UseTarget::InputOutput => (vec![], Some(zir::OpenMode::InputOutput)),
                    UseTarget::Extend => (vec![], Some(zir::OpenMode::Extend)),
                };
                for &f in &files {
                    if self.files[f as usize].error_proc.is_none() {
                        self.files[f as usize].error_proc = Some(first);
                    }
                }
                uses.push(zir::UseDecl {
                    files,
                    mode,
                    start: first,
                    end: para_idx - 1,
                    global: *global,
                });
            }
        }
        self.emit(Instr::GoBack);

        // Resolve labels.
        let labels = self.labels.clone();
        let res = |l: u32| -> u32 { labels[l as usize].unwrap_or(0) };
        for ins in &mut self.code {
            match ins {
                Instr::Jump(t) | Instr::JumpIf(_, t) | Instr::JumpUnless(_, t) => *t = res(*t),
                Instr::Perform { start, ret, .. } => {
                    *start = res(*start);
                    *ret = res(*ret);
                }
                Instr::GoDepending(_, ts) => {
                    for t in ts {
                        *t = res(*t);
                    }
                }
                Instr::Alter(_, t) => *t = res(*t),
                Instr::SearchStep { at_end, .. } => *at_end = res(*at_end),
                Instr::Sort { input, output, .. } => {
                    if let Some((s, _)) = input {
                        *s = res(*s);
                    }
                    if let Some((s, _)) = output {
                        *s = res(*s);
                    }
                }
                _ => {}
            }
        }
        let alter_init: Vec<u32> = self.alter_init.iter().map(|&l| res(l)).collect();
        let mut execs = std::mem::take(&mut self.execs);
        for e in &mut execs {
            for o in &mut e.options {
                if let Some(zir::ExecVal::Label(l)) = &mut o.arg {
                    *l = res(*l);
                }
            }
        }
        let paragraphs = self
            .paras
            .iter()
            .zip(&self.para_pos)
            .map(|((n, s, l), &pos)| zir::Paragraph {
                name: n.clone(),
                section: s.clone(),
                start: res(*l),
                pos,
            })
            .collect();
        let segments = if self.segmented {
            self.paras
                .iter()
                .zip(&self.para_segments)
                .map(|((_, _, l), &seg)| (res(*l), seg))
                .collect()
        } else {
            Vec::new()
        };
        let mut alter_segments = vec![0u8; self.alter_init.len()];
        for (&para, &slot) in &self.alter_slots {
            alter_segments[slot as usize] = self.para_segments[para as usize];
        }
        let collating = self.collating()?;
        let (low_value, high_value) = match &collating {
            Some(w) => {
                let lo = (0..256).min_by_key(|&b| w[b]).unwrap_or(0) as u8;
                let hi = (0..256).max_by_key(|&b| w[b]).unwrap_or(255) as u8;
                (lo, hi)
            }
            None => (0x00, 0xFF),
        };
        let data_execs = self
            .prog
            .data
            .execs
            .iter()
            .map(|e| self.exec_def(e))
            .collect::<Result<Vec<_>>>()?;
        // without PROCEDURE DIVISION USING, the parameters of the first
        // ENTRY statement (ENTRY 'DLITCBL' USING pcb...: the entry IMS
        // calls)
        let entry_using: Vec<DataRef> = if self.prog.procedure.using.is_empty() {
            self.prog
                .procedure
                .sections
                .iter()
                .flat_map(|s| &s.paragraphs)
                .flat_map(|p| &p.sentences)
                .flatten()
                .find_map(|st| match st {
                    Stmt::Entry { using, .. } => Some(using.clone()),
                    _ => None,
                })
                .unwrap_or_default()
        } else {
            Vec::new()
        };
        let using = self
            .prog
            .procedure
            .using
            .iter()
            .chain(entry_using.iter())
            .map(|r| {
                let id = self.dm.resolve(r)?;
                match self.dm.items[id].area {
                    Area::Linkage(n) => Ok(n),
                    _ => Err(Diag::error(
                        r.pos,
                        "USING item must be in the LINKAGE SECTION",
                    )),
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let using_by_value = if !self.prog.procedure.using.is_empty() {
            self.prog.procedure.using_by_value.clone()
        } else {
            self.prog
                .procedure
                .sections
                .iter()
                .flat_map(|s| &s.paragraphs)
                .flat_map(|p| &p.sentences)
                .flatten()
                .find_map(|st| match st {
                    Stmt::Entry { using_by_value, .. } => Some(using_by_value.clone()),
                    _ => None,
                })
                .unwrap_or_default()
        };
        for (r, &value) in self
            .prog
            .procedure
            .using
            .iter()
            .chain(&entry_using)
            .zip(&using_by_value)
        {
            let id = self.dm.resolve(r)?;
            let item = &self.dm.items[id];
            let desc = &self.dm.descs[item.desc as usize];
            if !matches!(item.level, 1 | 77)
                || r.refmod.is_some()
                || !r.subscripts.is_empty()
                || (value && !desc.ty.by_value_allowed(desc.len))
            {
                return Err(Diag::error(
                    r.pos,
                    "invalid formal USING item or BY VALUE category",
                ));
            }
        }
        let result = if let Some(r) = &self.prog.procedure.giving {
            let id = self.dm.resolve(r)?;
            let item = &self.dm.items[id];
            if parent.is_some()
                || !matches!(item.level, 1 | 77)
                || !matches!(item.area, Area::Linkage(_))
                || !r.subscripts.is_empty()
                || r.refmod.is_some()
            {
                return Err(Diag::error(
                    r.pos,
                    "PROCEDURE RETURNING requires a level 01/77 LINKAGE item in a non-nested program",
                ));
            }
            if let Area::Linkage(slot) = item.area
                && using.contains(&slot)
            {
                return Err(Diag::error(
                    r.pos,
                    "RETURNING item cannot also appear in USING",
                ));
            }
            Some(self.loc(r)?)
        } else {
            None
        };
        Ok(zir::ProgramCode {
            ccsid: None,
            compiled_at: None,
            invocation_storage: {
                self.dm
                    .items
                    .iter()
                    .filter(|item| {
                        item.area == Area::Special
                            && item.owner.is_none()
                            && ((self.prog.recursive
                                && matches!(
                                    item.name.as_deref(),
                                    Some(
                                        "RETURN-CODE"
                                            | "TALLY"
                                            | "SORT-RETURN"
                                            | "SORT-CONTROL"
                                            | "SORT-CORE-SIZE"
                                            | "SORT-FILE-SIZE"
                                            | "SORT-MESSAGE"
                                            | "SORT-MODE-SIZE"
                                    )
                                ))
                                || item.index_of.is_some_and(|table| {
                                    let table = &self.dm.items[table];
                                    (self.prog.recursive && matches!(table.area, Area::Linkage(_)))
                                        || (table.area == Area::Working
                                            && self.dm.local_storage.is_some_and(|(start, len)| {
                                                table.offset >= start && table.offset < start + len
                                            }))
                                }))
                    })
                    .map(|item| item.desc)
                    .collect()
            },
            ssrange: crate::front::ssrange(),
            dynam: crate::front::dynam(),
            dll: crate::front::dll() || crate::front::lp64(),
            exportall: crate::front::exportall(),
            thread: crate::front::thread(),
            lp64: crate::front::lp64(),
            decimal_comma: self.prog.env.decimal_point_comma,
            currency_sign: self.prog.env.currency_sign.clone(),
            rerun: self
                .prog
                .env
                .rerun
                .iter()
                .map(|r| {
                    if self.prog.recursive {
                        return Err(Diag::error(
                            r.pos,
                            "RERUN is forbidden in RECURSIVE programs",
                        ));
                    }
                    if self
                        .prog
                        .env
                        .files
                        .iter()
                        .any(|f| zeus_ds::dd::same_assignment(&f.assign, &r.checkpoint))
                    {
                        return Err(Diag::error(
                            r.pos,
                            "RERUN checkpoint assignment must be distinct from file assignments",
                        ));
                    }
                    let file = r.file.as_ref().map(|name| self.file(name)).transpose()?;
                    if let Some(f) = file {
                        let def = &self.files[f as usize];
                        if def.owner.is_some()
                            || def.external
                            || def.sort
                            || def.org != zir::Org::Sequential
                        {
                            return Err(Diag::error(
                                r.pos,
                                "RERUN requires a locally declared non-EXTERNAL sequential file",
                            ));
                        }
                    } else if parent.is_some() {
                        return Err(Diag::error(
                            r.pos,
                            "sort RERUN cannot occur in a contained program",
                        ));
                    }
                    if self
                        .prog
                        .env
                        .rerun
                        .iter()
                        .filter(|other| {
                            other.file == r.file && other.records.is_some() == r.records.is_some()
                        })
                        .count()
                        > 1
                    {
                        return Err(Diag::error(
                            r.pos,
                            "duplicate RERUN interval for the same file",
                        ));
                    }
                    Ok(zir::Rerun {
                        checkpoint: r.checkpoint.clone(),
                        file,
                        records: r.records,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            dynamic_init: self.dm.dynamic_init.clone(),
            numeric: Some(crate::front::numeric()),
            name: self.prog.name.clone(),
            recursive: self.prog.recursive,
            initial: self.prog.initial,
            common: self.prog.common,
            parent,
            static_init: self.dm.static_init.clone(),
            local_storage: self.dm.local_storage,
            descs: self.dm.descs.clone(),
            code: self.code,
            paragraphs,
            files: self.files,
            linkage_count: self.dm.linkage_count,
            using,
            using_by_value,
            entries: self.entries,
            result,
            temps: self.temps,
            alter_slots: alter_init.len() as u32,
            alter_init,
            segments,
            alter_segments,
            collating,
            high_value,
            low_value,
            switches: self.switches,
            externals: self.dm.externals.clone(),
            debug_decls: self.dbg.decls.clone(),
            reports: std::mem::take(&mut self.reports),
            execs,
            data_execs,
            debug_item: self
                .dm
                .by_name
                .get("DEBUG-ITEM")
                .and_then(|v| v.first())
                .map(|&id| self.dm.items[id].desc),
            debugging: false,
            uses,
            label_uses,
            return_code: self
                .dm
                .by_name
                .get("RETURN-CODE")
                .into_iter()
                .flatten()
                .find(|&&id| self.dm.items[id].area == super::data::Area::Special)
                .map(|&id| self.dm.items[id].desc),
            stmt_pos: self.stmt_pos,
            instr_stmt: self.instr_stmt,
        })
    }

    fn collating(&self) -> Result<Option<Vec<u16>>> {
        let Some(name) = &self.prog.env.program_collating else {
            return Ok(None);
        };
        self.alphabet_weights(name).map(Some)
    }

    pub fn alphabet_weights(&self, name: &str) -> Result<Vec<u16>> {
        let alpha = self
            .prog
            .env
            .alphabets
            .iter()
            .find(|a| a.name == name)
            .ok_or_else(|| Diag::error(self.pos, format!("alphabet {name} not defined")))?;
        Ok(match &alpha.kind {
            AlphabetKind::Native => (0..256u16).collect(),
            AlphabetKind::StandardOne | AlphabetKind::StandardTwo => {
                // ASCII order of the EBCDIC characters.
                (0..256u16)
                    .map(|b| u16::from(ebcdic::to_latin1(b as u8)))
                    .collect()
            }
            AlphabetKind::Literals(groups) => {
                let mut w = vec![u16::MAX; 256];
                let mut next = 0u16;
                for group in groups {
                    let mut chars = Vec::new();
                    for item in group {
                        match item {
                            AlphabetItem::Char(c) => chars.extend(spec_bytes(c)),
                            AlphabetItem::Range(a, b) => {
                                let (a, b) = (spec_bytes(a)[0], spec_bytes(b)[0]);
                                if a <= b {
                                    chars.extend(a..=b);
                                } else {
                                    chars.extend((b..=a).rev());
                                }
                            }
                        }
                    }
                    // Characters of a group without ALSO each get a weight;
                    // ALSO groups share one.
                    let also = group.len() > 1 && group.iter().all(|i| {
                        matches!(i, AlphabetItem::Char(CharSpec::Text(t)) if t.chars().count() == 1)
                            || matches!(i, AlphabetItem::Char(CharSpec::Ordinal(_)))
                    });
                    if also {
                        for c in chars {
                            if w[c as usize] == u16::MAX {
                                w[c as usize] = next;
                            }
                        }
                        next += 1;
                    } else {
                        for c in chars {
                            if w[c as usize] == u16::MAX {
                                w[c as usize] = next;
                                next += 1;
                            }
                        }
                    }
                }
                // Unspecified characters follow in native order.
                for x in w.iter_mut() {
                    if *x == u16::MAX {
                        *x = next;
                        next += 1;
                    }
                }
                w
            }
        })
    }

    fn mark_print_files(&mut self, sections: &[Section]) {
        fn walk(stmts: &[Stmt], out: &mut Vec<DataRef>) {
            for s in stmts {
                match s {
                    Stmt::Write {
                        record,
                        advancing: Some(_),
                        ..
                    } => out.push(record.clone()),
                    Stmt::If { then, els, .. } => {
                        walk(then, out);
                        if let Some(e) = els {
                            walk(e, out);
                        }
                    }
                    Stmt::Evaluate { whens, other, .. } => {
                        for (_, b) in whens {
                            walk(b, out);
                        }
                        if let Some(o) = other {
                            walk(o, out);
                        }
                    }
                    Stmt::Perform {
                        body: PerformBody::Inline(b),
                        ..
                    } => walk(b, out),
                    _ => {}
                }
                for h in handlers_of(s) {
                    walk(h, out);
                }
            }
        }
        let mut recs = Vec::new();
        for sec in sections {
            for p in &sec.paragraphs {
                for s in &p.sentences {
                    walk(s, &mut recs);
                }
            }
        }
        for r in recs {
            if let Ok(f) = self.file_of_record(&r) {
                self.files[f as usize].print = true;
            }
        }
        // LINAGE files are print files too.
        for fd in &self.prog.data.files {
            if fd.linage.is_some()
                && let Some(&f) = self.file_index.get(&fd.name)
            {
                self.files[f as usize].print = true;
            }
        }
    }

    /// Resolves the USE FOR DEBUGGING statements (WITH DEBUGGING MODE).
    fn setup_debugging(&mut self) -> Result<()> {
        if !self.prog.env.debugging_mode {
            return Ok(());
        }
        let prog = self.prog;
        for sec in &prog.procedure.declaratives {
            let Some(UseStmt::Debugging(targets)) = &sec.use_stmt else {
                continue;
            };
            let Some(&(first, last)) = sec.name.as_ref().and_then(|n| self.sections.get(n)) else {
                continue;
            };
            let decl = self.dbg.decls.len() as u32;
            self.dbg.decls.push((first, last));
            for t in targets {
                let DebugTarget::Name { name: r, all_refs } = t else {
                    self.dbg.all_procs = Some(decl);
                    continue;
                };
                let mut shown = r.name.clone();
                for q in &r.qualifiers {
                    shown.push_str(" OF ");
                    shown.push_str(q);
                }
                if r.qualifiers.is_empty()
                    && let Some(&f) = self.file_index.get(&r.name)
                {
                    self.dbg.files.insert(f, (decl, shown));
                    continue;
                }
                if r.qualifiers.is_empty() && self.sections.contains_key(&r.name) {
                    self.dbg.sections.insert(r.name.clone(), (decl, shown));
                    continue;
                }
                let pn = ProcName {
                    name: r.name.clone(),
                    section: r.qualifiers.first().cloned(),
                    pos: r.pos,
                };
                if let Ok((p, _)) = self.proc_range(&pn) {
                    self.dbg.paras.insert(p, (decl, shown));
                    continue;
                }
                let id = self.dm.resolve(r)?;
                let desc = self.dm.items[id].desc;
                self.dbg.items.insert(desc, (decl, *all_refs, shown));
            }
        }
        Ok(())
    }

    /// Report Writer: builds the report descriptions.
    fn build_reports(&mut self) -> Result<()> {
        let prog = self.prog;
        for (r, rd) in prog.data.reports.iter().enumerate() {
            let fd = prog
                .data
                .files
                .iter()
                .find(|f| f.reports.contains(&rd.name))
                .ok_or_else(|| Diag::error(rd.pos, format!("no FD names report {}", rd.name)))?;
            let file = *self
                .file_index
                .get(&fd.name)
                .ok_or_else(|| Diag::error(rd.pos, format!("file {} has no SELECT", fd.name)))?;
            self.files[file as usize].print = true;
            let page = match &rd.page {
                Some(p) => {
                    let limit = p.limit.unwrap_or(66);
                    let heading = p.heading.unwrap_or(1);
                    let last_detail = p.last_detail.or(p.footing).unwrap_or(limit);
                    Some(zir::PageDef {
                        limit,
                        heading,
                        first_detail: p.first_detail.unwrap_or(heading),
                        last_detail,
                        footing: p.footing.unwrap_or(last_detail),
                    })
                }
                None => None,
            };
            let mut controls = Vec::new();
            for c in &rd.controls {
                controls.push(match c {
                    Some(d) => Some(self.loc(d)?),
                    None => None,
                });
            }
            let control_level = |name: &Option<String>| -> Option<u32> {
                rd.controls
                    .iter()
                    .position(|c| match (c, name) {
                        (None, None) => true,
                        (Some(d), Some(n)) => &d.name == n,
                        _ => false,
                    })
                    .map(|p| p as u32)
            };
            let desc_of = |g: &Gen, i: usize| -> Option<u32> {
                g.dm.by_name
                    .get(&super::data::report_item_name(r, i))
                    .and_then(|v| v.first())
                    .map(|&id| g.dm.items[id].desc)
            };
            let mut groups: Vec<zir::GroupDef> = Vec::new();
            // (group, entry index, SUM clauses) of every sum counter
            let mut sum_entries: Vec<(u32, usize)> = Vec::new();
            for (i, e) in rd.entries.iter().enumerate() {
                let rc = e.report.as_deref().cloned().unwrap_or_default();
                if e.level == 1 {
                    let (kind, control) = match &rc.ty {
                        Some(GroupType::ReportHeading) => (zir::GroupKind::ReportHeading, None),
                        Some(GroupType::PageHeading) => (zir::GroupKind::PageHeading, None),
                        Some(GroupType::ControlHeading(n)) => {
                            (zir::GroupKind::ControlHeading, control_level(n))
                        }
                        Some(GroupType::ControlFooting(n)) => {
                            (zir::GroupKind::ControlFooting, control_level(n))
                        }
                        Some(GroupType::PageFooting) => (zir::GroupKind::PageFooting, None),
                        Some(GroupType::ReportFooting) => (zir::GroupKind::ReportFooting, None),
                        Some(GroupType::Detail) | None => (zir::GroupKind::Detail, None),
                    };
                    groups.push(zir::GroupDef {
                        name: e.name.clone().unwrap_or_default(),
                        kind,
                        control,
                        lines: Vec::new(),
                        next_group: rc.next_group.map(line_pos),
                        use_proc: None,
                    });
                }
                let Some(group) = groups.last_mut() else {
                    return Err(Diag::error(e.pos, "report group must start at level 01"));
                };
                if let Some(l) = rc.line {
                    group.lines.push(zir::LineDef {
                        pos: line_pos(l),
                        fields: Vec::new(),
                    });
                }
                let elementary = e.picture.is_some();
                if !elementary {
                    continue;
                }
                let g = (groups.len() - 1) as u32;
                let desc = desc_of(self, i)
                    .ok_or_else(|| Diag::error(e.pos, "report item not laid out"))?;
                let value = if !rc.sum.is_empty() {
                    sum_entries.push((g, i));
                    zir::FieldValue::Sum((sum_entries.len() - 1) as u32)
                } else if let Some(s) = &rc.source {
                    zir::FieldValue::Source(self.src(s)?)
                } else {
                    zir::FieldValue::Constant
                };
                let group = groups.last_mut().unwrap();
                if group.lines.is_empty() {
                    group.lines.push(zir::LineDef {
                        pos: zir::LinePos::Plus(1),
                        fields: Vec::new(),
                    });
                }
                group.lines.last_mut().unwrap().fields.push(zir::FieldDef {
                    column: rc.column,
                    desc,
                    value,
                    group_indicate: rc.group_indicate,
                });
            }
            // sum counters
            let mut sums = Vec::new();
            for &(g, i) in &sum_entries {
                let rc = rd.entries[i].report.as_deref().unwrap();
                let mut operands = Vec::new();
                let mut upon = Vec::new();
                let mut reset = None;
                for s in &rc.sum {
                    for item in &s.items {
                        let counter = sum_entries.iter().position(|&(_, j)| {
                            rd.entries[j].name.as_deref() == Some(item.name.as_str())
                        });
                        operands.push(match counter {
                            Some(k) => zir::SumOperand::Sum(k as u32),
                            None => zir::SumOperand::Data(Ex::Load(self.loc(item)?)),
                        });
                    }
                    for u in &s.upon {
                        if let Some(p) = groups.iter().position(|gd| &gd.name == u) {
                            upon.push(p as u32);
                        }
                    }
                    if let Some(rs) = &s.reset {
                        reset = control_level(rs);
                    }
                }
                sums.push(zir::SumDef {
                    group: g,
                    operands,
                    upon,
                    reset,
                });
            }
            // USE BEFORE REPORTING
            for sec in &prog.procedure.declaratives {
                if let Some(UseStmt::BeforeReporting(gname)) = &sec.use_stmt
                    && let Some(gd) = groups.iter_mut().find(|gd| &gd.name == gname)
                    && let Some(&range) = sec.name.as_ref().and_then(|n| self.sections.get(n))
                {
                    gd.use_proc = Some(range);
                }
            }
            let (lc, pc) = self.dm.report_counters[&rd.name];
            let code = match &rd.code {
                Some(Operand::Lit(l)) => Some(super::lit::literal_bytes(l)),
                _ => None,
            };
            self.reports.push(zir::ReportDef {
                name: rd.name.clone(),
                file,
                page,
                controls,
                groups,
                sums,
                line_counter: self.dm.items[lc].desc,
                page_counter: self.dm.items[pc].desc,
                code,
            });
        }
        Ok(())
    }

    fn report_index(&self, name: &str, pos: Pos) -> Result<u32> {
        self.reports
            .iter()
            .position(|r| r.name == name)
            .map(|p| p as u32)
            .ok_or_else(|| Diag::error(pos, format!("{name} is not a report")))
    }

    /// Debugging trigger on entering a section by its name.
    fn debug_section_start(&mut self, para_idx: u32) {
        if self.in_decl || !self.dbg.procs() {
            return;
        }
        let Some(sname) = self.paras[para_idx as usize].1.clone() else {
            return;
        };
        let d = self
            .dbg
            .sections
            .get(&sname)
            .cloned()
            .or_else(|| self.dbg.all_procs.map(|d| (d, sname.clone())));
        if let Some((decl, name)) = d {
            self.emit(Instr::DebugProc {
                decl,
                name,
                contents: None,
                line: self.pos.line,
            });
        }
    }

    /// Debugging trigger at the start of a paragraph.
    fn debug_para_start(&mut self, para_idx: u32) {
        if self.in_decl || !self.dbg.procs() {
            return;
        }
        let line = self.pos.line;
        let pname = self.paras[para_idx as usize].0.clone();
        if !pname.is_empty() {
            let d = self
                .dbg
                .paras
                .get(&para_idx)
                .cloned()
                .or_else(|| self.dbg.all_procs.map(|d| (d, pname.clone())));
            if let Some((decl, name)) = d {
                self.emit(Instr::DebugProc {
                    decl,
                    name,
                    contents: None,
                    line,
                });
            }
        }
        self.emit(Instr::DebugCause(zir::debug_cause::FALL_THROUGH, line));
    }

    fn debug_goto(&mut self) {
        if !self.in_decl && self.dbg.procs() {
            self.emit(Instr::DebugCause(zir::debug_cause::GO_TO, self.pos.line));
        }
    }

    /// Debugging trigger after an input-output statement on file `f`.
    fn debug_file(&mut self, f: u32, read: bool) {
        if self.in_decl {
            return;
        }
        if let Some((decl, name)) = self.dbg.files.get(&f).cloned() {
            let record = read.then(|| self.files[f as usize].record);
            self.emit(Instr::DebugFile {
                decl,
                name,
                record,
                line: self.pos.line,
            });
        }
    }

    /// Debugging triggers for the identifiers referenced by `instrs`:
    /// receiving operands, or any reference for ALL REFERENCES (or when
    /// `any` is set, as for PERFORM VARYING).
    fn debug_data_triggers(&self, instrs: &[Instr], any: bool) -> Vec<Instr> {
        let mut out: Vec<Instr> = Vec::new();
        let mut seen: Vec<u32> = Vec::new();
        for ins in instrs {
            for (loc, receiving) in instr_locs(ins) {
                if let Some((decl, all, name)) = self.dbg.items.get(&loc.desc)
                    && (receiving || *all || any)
                    && !seen.contains(&loc.desc)
                {
                    seen.push(loc.desc);
                    out.push(Instr::DebugData {
                        decl: *decl,
                        name: name.clone(),
                        loc,
                        line: self.pos.line,
                    });
                }
            }
        }
        out
    }

    fn segment_mark(&mut self, para_idx: u32) {
        if self.segmented {
            let seg = self.para_segments[para_idx as usize];
            self.emit(Instr::Segment(seg));
        }
    }

    fn gen_section(&mut self, sec: &Section, mut para_idx: u32) -> Result<u32> {
        self.in_label_decl = matches!(sec.use_stmt, Some(UseStmt::Label { .. }));
        self.dm.label_scope = self.in_label_decl;
        self.current_section = sec.name.clone();
        // A USE AFTER ERROR section named in USE FOR DEBUGGING.
        let use_debug = if self.in_decl && matches!(sec.use_stmt, Some(UseStmt::Error(..))) {
            sec.name
                .as_ref()
                .and_then(|n| self.dbg.sections.get(n).cloned())
        } else {
            None
        };
        let first_para = para_idx;
        if sec.paragraphs.is_empty() {
            let l = self.paras[para_idx as usize].2;
            self.bind(l);
            self.segment_mark(para_idx);
            self.debug_section_start(para_idx);
            self.debug_para_start(para_idx);
            let e = self.para_end_labels[para_idx as usize];
            self.bind(e);
            self.emit(Instr::ParaEnd(para_idx));
            return Ok(para_idx + 1);
        }
        for p in &sec.paragraphs {
            self.current_para = para_idx;
            if let Some(&l) = self.section_labels.get(&para_idx) {
                self.bind(l);
                self.segment_mark(para_idx);
                self.debug_section_start(para_idx);
            }
            let l = self.paras[para_idx as usize].2;
            self.bind(l);
            self.segment_mark(para_idx);
            self.debug_para_start(para_idx);
            if para_idx == first_para
                && let Some((decl, name)) = use_debug.clone()
            {
                self.emit(Instr::DebugProc {
                    decl,
                    name,
                    contents: Some("USE PROCEDURE".to_string()),
                    line: self.pos.line,
                });
            }
            self.pos = p.pos;
            if let Some(&slot) = self.alter_slots.get(&para_idx) {
                let Stmt::GoTo { targets, .. } = &p.sentences[0][1] else {
                    unreachable!()
                };
                let target = match targets.first() {
                    Some(t) => self.proc_start(t)?,
                    None => self.para_end_labels[para_idx as usize],
                };
                self.alter_init[slot as usize] = target;
                self.debug_goto();
                self.emit(Instr::AlterableGo(slot));
            } else {
                for sentence in &p.sentences {
                    let end = self.label();
                    self.sentence_end = Some(end);
                    self.stmts(sentence)?;
                    self.bind(end);
                }
            }
            let e = self.para_end_labels[para_idx as usize];
            self.bind(e);
            self.emit(Instr::ParaEnd(para_idx));
            para_idx += 1;
        }
        Ok(para_idx)
    }

    // ---- statements ----------------------------------------------------

    fn stmts(&mut self, stmts: &[Stmt]) -> Result<()> {
        for s in stmts {
            self.stmt(s)?;
        }
        Ok(())
    }

    /// Generates `on` / `not on` handlers for a condition flag tested by
    /// `cond` (true = the exception occurred).
    fn use_check(&mut self, file: u32, at_end: &[&Handlers], invalid_key: &[&Handlers]) {
        let has = |hs: &[&Handlers]| hs.iter().any(|h| h.on.is_some());
        let handled = has(at_end) as u8 | (has(invalid_key) as u8) << 1;
        self.emit(Instr::UseCheck { file, handled });
    }

    fn handlers(&mut self, h: &Handlers, cond: Cx) -> Result<()> {
        if h.on.is_none() && h.not_on.is_none() {
            return Ok(());
        }
        self.debug_flush();
        let not_l = self.label();
        let end = self.label();
        self.emit(Instr::JumpUnless(cond, not_l));
        if let Some(on) = &h.on {
            self.stmts(on)?;
        }
        self.emit(Instr::Jump(end));
        self.bind(not_l);
        if let Some(n) = &h.not_on {
            self.stmts(n)?;
        }
        self.bind(end);
        Ok(())
    }

    fn sum(&mut self, exprs: &[Expr]) -> Result<Ex> {
        let mut it = exprs.iter();
        let first = it
            .next()
            .ok_or_else(|| Diag::error(self.pos, "operand expected"))?;
        let mut acc = self.ex(first)?;
        for e in it {
            acc = Ex::Bin(Op::Add, Box::new(acc), Box::new(self.ex(e)?));
        }
        Ok(acc)
    }

    /// A value used by several receivers is computed once, before any
    /// receiver changes.
    fn once(&mut self, value: Ex, receivers: usize) -> Ex {
        if receivers <= 1 || matches!(value, Ex::Lit(_) | Ex::Temp(_)) {
            return value;
        }
        let t = self.temp();
        self.emit(Instr::SetTemp(t, value));
        Ex::Temp(t)
    }

    fn store(&mut self, value: Ex, r: &Receiver, check: bool) -> Result<()> {
        let dst = self.loc(&r.target)?;
        self.emit(Instr::Store {
            value,
            dst,
            rounded: r.rounded,
            check,
        });
        Ok(())
    }

    /// Pairs of corresponding elementary items of two groups.
    fn corresponding(&mut self, a: &DataRef, b: &DataRef) -> Result<Vec<(DataRef, DataRef)>> {
        let ia = self.dm.resolve(a)?;
        let ib = self.dm.resolve(b)?;
        let mut pairs = Vec::new();
        for &ca in &self.dm.items[ia].children.clone() {
            let it = &self.dm.items[ca];
            let Some(name) = it.name.clone() else {
                continue;
            };
            if it.entry.redefines.is_some() || it.occurs.is_some() || it.usage == Usage::Index {
                continue;
            }
            if let Some(&cb) = self.dm.items[ib].children.iter().find(|&&x| {
                let y = &self.dm.items[x];
                y.name.as_deref() == Some(&name)
                    && y.entry.redefines.is_none()
                    && y.occurs.is_none()
            }) {
                let mut ra = a.clone();
                ra.name = name.clone();
                ra.qualifiers = std::iter::once(a.name.clone())
                    .chain(a.qualifiers.clone())
                    .collect();
                let mut rb = b.clone();
                rb.name = name.clone();
                rb.qualifiers = std::iter::once(b.name.clone())
                    .chain(b.qualifiers.clone())
                    .collect();
                let both_groups = !self.dm.items[ca].children.is_empty()
                    && !self.dm.items[cb].children.is_empty();
                if both_groups {
                    pairs.extend(self.corresponding(&ra, &rb)?);
                } else {
                    pairs.push((ra, rb));
                }
            }
        }
        Ok(pairs)
    }

    fn file_of_record(&self, r: &DataRef) -> Result<u32> {
        let id = self.dm.resolve(r)?;
        let rec = self.dm.record_of(id);
        match self.dm.items[rec].area {
            Area::File(_) | Area::External(_) => {
                let name = self
                    .dm
                    .files
                    .iter()
                    .find(|(_, (_, recs))| recs.contains(&rec))
                    .map(|(n, _)| n.clone())
                    .ok_or_else(|| Diag::error(r.pos, "record not in a file"))?;
                self.file_index
                    .get(&name)
                    .copied()
                    .ok_or_else(|| Diag::error(r.pos, format!("file {name} has no SELECT")))
            }
            _ => Err(Diag::error(
                r.pos,
                format!("{} is not a file record", r.name),
            )),
        }
    }

    fn file(&self, name: &str) -> Result<u32> {
        self.file_index
            .get(name)
            .copied()
            .ok_or_else(|| Diag::error(self.pos, format!("file {name} is not defined")))
    }

    fn stmt(&mut self, s: &Stmt) -> Result<()> {
        // a statement's position: its instructions belong to it
        if let Stmt::Line(pos) = s {
            self.pos = *pos;
            self.marker_outer = self.cur_stmt;
            self.cur_stmt = self.stmt_pos.len() as u32;
            self.stmt_pos.push((pos.file, pos.line));
            return Ok(());
        }
        // after the statement, instructions belong to the statement that
        // contains it again
        let outer = self.marker_outer;
        let r = self.stmt_debug(s);
        self.cur_stmt = outer;
        r
    }

    fn stmt_debug(&mut self, s: &Stmt) -> Result<()> {
        if self.dbg.items.is_empty() || self.in_decl {
            return self.stmt_code(s);
        }
        let start = self.code.len();
        let mark = self.dbg.stmt_ranges.len();
        // PERFORM VARYING and GO TO DEPENDING place their own triggers.
        let own = matches!(
            s,
            Stmt::Perform {
                kind: PerformKind::Varying { .. },
                ..
            }
        ) || matches!(
            s,
            Stmt::GoTo {
                depending: Some(_),
                ..
            }
        );
        self.dbg.stmt_stack.push((start, mark, own));
        let r = self.stmt_code(s);
        self.debug_flush();
        self.dbg.stmt_stack.pop();
        r?;
        self.dbg.stmt_ranges.truncate(mark);
        self.dbg.stmt_ranges.push((start, self.code.len()));
        Ok(())
    }

    /// Places the identifier debugging triggers of the statement being
    /// generated (at its end, or before its exception handlers).
    fn debug_flush(&mut self) {
        let Some(&(start, mark, done)) = self.dbg.stmt_stack.last() else {
            return;
        };
        if done || self.in_decl {
            return;
        }
        self.dbg.stmt_stack.last_mut().unwrap().2 = true;
        let inner = self.dbg.stmt_ranges[mark..].to_vec();
        let instrs: Vec<Instr> = (start..self.code.len())
            .filter(|i| !inner.iter().any(|&(a, b)| (a..b).contains(i)))
            .map(|i| self.code[i].clone())
            .filter(|i| {
                !matches!(
                    i,
                    Instr::Jump(_) | Instr::JumpIf(..) | Instr::JumpUnless(..)
                )
            })
            .collect();
        for t in self.debug_data_triggers(&instrs, false) {
            self.emit(t);
        }
    }

    fn stmt_code(&mut self, s: &Stmt) -> Result<()> {
        let receivers = match s {
            Stmt::Add { to, giving, .. } => Some(if giving.is_empty() { to } else { giving }),
            Stmt::Subtract {
                subtrahend_of,
                giving,
                ..
            } => Some(if giving.is_empty() {
                subtrahend_of
            } else {
                giving
            }),
            Stmt::Multiply { into, giving, .. } | Stmt::Divide { into, giving, .. } => {
                Some(if giving.is_empty() { into } else { giving })
            }
            Stmt::Compute { targets, .. } => Some(targets),
            _ => None,
        };
        if let Some(receivers) = receivers {
            let mut decimals = 0;
            let mut floating = false;
            for receiver in receivers {
                let mut items = vec![self.dm.resolve(&receiver.target)?];
                while let Some(id) = items.pop() {
                    let item = &self.dm.items[id];
                    items.extend(item.children.iter().copied());
                    match self.dm.descs[item.desc as usize].ty {
                        DataType::Numeric(t) => {
                            decimals = decimals.max(t.scale + i32::from(receiver.rounded))
                        }
                        DataType::NumericEdited { scale, .. } => {
                            decimals = decimals.max(scale + i32::from(receiver.rounded))
                        }
                        DataType::Float { .. } | DataType::ExternalFloat { .. } => floating = true,
                        _ => {}
                    }
                }
            }
            self.emit(Instr::NumericContext { decimals, floating });
        }
        if (self.prog.recursive || crate::front::lp64())
            && (matches!(s, Stmt::Alter(_))
                || matches!(s, Stmt::GoTo { targets, .. } if targets.is_empty()))
        {
            return Err(Diag::error(
                self.pos,
                if crate::front::lp64() {
                    "LP(64) cannot use ALTER or GO TO without a procedure name"
                } else {
                    "RECURSIVE cannot use ALTER or GO TO without a procedure name"
                },
            ));
        }
        match s {
            Stmt::Initiate(names) | Stmt::Terminate(names) => {
                for n in names {
                    let r = self.report_index(n, self.pos)?;
                    self.emit(if matches!(s, Stmt::Initiate(_)) {
                        Instr::Initiate(r)
                    } else {
                        Instr::Terminate(r)
                    });
                }
            }
            Stmt::Generate(d) => {
                let target = self.reports.iter().enumerate().find_map(|(r, rd)| {
                    if rd.name == d.name {
                        Some((r as u32, None))
                    } else {
                        rd.groups
                            .iter()
                            .position(|g| g.name == d.name)
                            .map(|g| (r as u32, Some(g as u32)))
                    }
                });
                let (report, group) = target.ok_or_else(|| {
                    Diag::error(d.pos, format!("{} is not a report or detail group", d.name))
                })?;
                self.emit(Instr::Generate { report, group });
            }
            Stmt::SuppressPrinting => self.emit(Instr::SuppressPrinting),
            Stmt::Move {
                from,
                to,
                corresponding,
                pos,
            } => {
                self.pos = *pos;
                if *corresponding {
                    let Operand::Ref(src) = from else {
                        return Err(Diag::error(*pos, "MOVE CORRESPONDING needs a group"));
                    };
                    for t in to {
                        for (a, b) in self.corresponding(src, t)? {
                            let s = Src::Loc(self.loc(&a)?);
                            let d = self.loc(&b)?;
                            self.emit(Instr::Move(s, d));
                        }
                    }
                } else {
                    let src = self.src(from)?;
                    for t in to {
                        let d = self.loc(t)?;
                        self.emit(Instr::Move(src.clone(), d));
                    }
                }
            }
            Stmt::Add {
                from,
                to,
                giving,
                corresponding,
                size_error,
                pos,
            } => {
                self.pos = *pos;
                let check = size_error.on.is_some() || size_error.not_on.is_some();
                self.emit(Instr::ClearSizeError);
                if *corresponding {
                    let Expr::Operand(Operand::Ref(src)) = &from[0] else {
                        unreachable!()
                    };
                    for t in to {
                        for (a, b) in self.corresponding(src, &t.target)? {
                            let v = Ex::Bin(
                                Op::Add,
                                Box::new(Ex::Load(self.loc(&b)?)),
                                Box::new(Ex::Load(self.loc(&a)?)),
                            );
                            let r = Receiver {
                                target: b,
                                rounded: t.rounded,
                            };
                            self.store(v, &r, check)?;
                        }
                    }
                } else if !giving.is_empty() {
                    let sum = self.sum(from)?;
                    let sum = self.once(sum, giving.len());
                    for r in giving {
                        self.store(sum.clone(), r, check)?;
                    }
                } else {
                    let sum = self.sum(from)?;
                    let sum = self.once(sum, to.len());
                    for r in to {
                        let v = Ex::Bin(
                            Op::Add,
                            Box::new(Ex::Load(self.loc(&r.target)?)),
                            Box::new(sum.clone()),
                        );
                        self.store(v, r, check)?;
                    }
                }
                self.handlers(size_error, Cx::SizeError)?;
            }
            Stmt::Subtract {
                from,
                subtrahend_of,
                giving,
                corresponding,
                size_error,
                pos,
            } => {
                self.pos = *pos;
                let check = size_error.on.is_some() || size_error.not_on.is_some();
                self.emit(Instr::ClearSizeError);
                if *corresponding {
                    let Expr::Operand(Operand::Ref(src)) = &from[0] else {
                        unreachable!()
                    };
                    for t in subtrahend_of {
                        for (a, b) in self.corresponding(src, &t.target)? {
                            let v = Ex::Bin(
                                Op::Sub,
                                Box::new(Ex::Load(self.loc(&b)?)),
                                Box::new(Ex::Load(self.loc(&a)?)),
                            );
                            let r = Receiver {
                                target: b,
                                rounded: t.rounded,
                            };
                            self.store(v, &r, check)?;
                        }
                    }
                } else if !giving.is_empty() {
                    let minuend = self.ex(&from[0])?;
                    let subs = self.sum(&from[1..])?;
                    let v = Ex::Bin(Op::Sub, Box::new(minuend), Box::new(subs));
                    let v = self.once(v, giving.len());
                    for r in giving {
                        self.store(v.clone(), r, check)?;
                    }
                } else {
                    let subs = self.sum(from)?;
                    let subs = self.once(subs, subtrahend_of.len());
                    for r in subtrahend_of {
                        let v = Ex::Bin(
                            Op::Sub,
                            Box::new(Ex::Load(self.loc(&r.target)?)),
                            Box::new(subs.clone()),
                        );
                        self.store(v, r, check)?;
                    }
                }
                self.handlers(size_error, Cx::SizeError)?;
            }
            Stmt::Multiply {
                by,
                operand,
                into,
                giving,
                size_error,
                pos,
            } => {
                self.pos = *pos;
                let check = size_error.on.is_some() || size_error.not_on.is_some();
                self.emit(Instr::ClearSizeError);
                let a = self.ex(by)?;
                let a = self.once(a, into.len());
                if giving.is_empty() {
                    for r in into {
                        let v = Ex::Bin(
                            Op::Mul,
                            Box::new(a.clone()),
                            Box::new(Ex::Load(self.loc(&r.target)?)),
                        );
                        self.store(v, r, check)?;
                    }
                } else {
                    let b = self.ex(operand)?;
                    let v = Ex::Bin(Op::Mul, Box::new(a), Box::new(b));
                    let v = self.once(v, giving.len());
                    for r in giving {
                        self.store(v.clone(), r, check)?;
                    }
                }
                self.handlers(size_error, Cx::SizeError)?;
            }
            Stmt::Divide {
                divisor,
                dividend,
                into,
                giving,
                remainder,
                size_error,
                pos,
            } => {
                self.pos = *pos;
                let check = size_error.on.is_some() || size_error.not_on.is_some();
                self.emit(Instr::ClearSizeError);
                let d = self.ex(divisor)?;
                let d = self.once(d, into.len().max(giving.len()));
                match dividend {
                    None => {
                        for r in into {
                            let v = Ex::Bin(
                                Op::Div,
                                Box::new(Ex::Load(self.loc(&r.target)?)),
                                Box::new(d.clone()),
                            );
                            self.store(v, r, check)?;
                        }
                    }
                    Some(dv) => {
                        let dv = self.ex(dv)?;
                        if let Some(rem) = remainder {
                            let r = &giving[0];
                            let quot = self.loc(&r.target)?;
                            let rem = self.loc(rem)?;
                            self.emit(Instr::DivRem {
                                dividend: dv,
                                divisor: d,
                                quot,
                                rounded: r.rounded,
                                rem,
                                check,
                            });
                        } else {
                            let v = Ex::Bin(Op::Div, Box::new(dv), Box::new(d));
                            let v = self.once(v, giving.len());
                            for r in giving {
                                self.store(v.clone(), r, check)?;
                            }
                        }
                    }
                }
                self.handlers(size_error, Cx::SizeError)?;
            }
            Stmt::Compute {
                targets,
                expr,
                size_error,
                pos,
            } => {
                self.pos = *pos;
                let check = size_error.on.is_some() || size_error.not_on.is_some();
                self.emit(Instr::ClearSizeError);
                let v = self.ex(expr)?;
                let v = self.once(v, targets.len());
                for r in targets {
                    self.store(v.clone(), r, check)?;
                }
                self.handlers(size_error, Cx::SizeError)?;
            }
            Stmt::If { cond, then, els } => {
                let c = self.cond(cond)?;
                let else_l = self.label();
                let end = self.label();
                self.emit(Instr::JumpUnless(c, else_l));
                self.stmts(then)?;
                self.emit(Instr::Jump(end));
                self.bind(else_l);
                if let Some(e) = els {
                    self.stmts(e)?;
                }
                self.bind(end);
            }
            Stmt::Line(pos) => {
                self.pos = *pos;
            }
            Stmt::NextSentence => {
                let l = self
                    .sentence_end
                    .ok_or_else(|| Diag::error(self.pos, "NEXT SENTENCE outside a sentence"))?;
                self.emit(Instr::Jump(l));
            }
            Stmt::Continue | Stmt::Exit => {}
            Stmt::ExitPerform { cycle } => {
                // IBM ignores EXIT PERFORM outside an inline PERFORM.
                if let Some(&(end, next)) = self.inline_performs.last() {
                    self.emit(Instr::Jump(if *cycle { next } else { end }));
                }
            }
            Stmt::ExitParagraph => {
                let l = self.para_end_labels[self.current_para as usize];
                self.emit(Instr::Jump(l));
            }
            Stmt::ExitSection => {
                let last = self
                    .current_section
                    .as_ref()
                    .and_then(|s| self.sections.get(s))
                    .map(|&(_, b)| b)
                    .unwrap_or(self.current_para);
                let l = self.para_end_labels[last as usize];
                self.emit(Instr::Jump(l));
            }
            Stmt::GoTo {
                targets,
                depending,
                pos,
            } => {
                self.pos = *pos;
                match depending {
                    None => match targets.first() {
                        Some(t) if self.in_label_decl && t.name == "MORE-LABELS" => {
                            self.emit(Instr::MoreLabels);
                        }
                        Some(t) => {
                            let l = self.proc_start(t)?;
                            self.debug_goto();
                            self.emit(Instr::Jump(l));
                        }
                        None => {
                            return Err(Diag::error(
                                *pos,
                                "GO TO without a target must be ALTERed",
                            ));
                        }
                    },
                    Some(d) => {
                        let v = Ex::Load(self.loc(d)?);
                        let ls = targets
                            .iter()
                            .map(|t| self.proc_start(t))
                            .collect::<Result<Vec<_>>>()?;
                        // ALL REFERENCES: before the transfer of control
                        let gd = Instr::GoDepending(v, ls);
                        if !self.in_decl {
                            let ts: Vec<Instr> = self
                                .debug_data_triggers(std::slice::from_ref(&gd), false)
                                .into_iter()
                                .filter(|t| {
                                    matches!(t, Instr::DebugData { loc, .. }
                                        if self.dbg.items.get(&loc.desc).is_some_and(|i| i.1))
                                })
                                .collect();
                            for t in ts {
                                self.emit(t);
                            }
                        }
                        self.debug_goto();
                        self.emit(gd);
                    }
                }
            }
            Stmt::Alter(pairs) => {
                for (from, to) in pairs {
                    let (p, _) = self.proc_range(from)?;
                    let slot = *self.alter_slots.get(&p).ok_or_else(|| {
                        Diag::error(from.pos, format!("{} cannot be altered", from.name))
                    })?;
                    let target = self.proc_start(to)?;
                    self.emit(Instr::Alter(slot, target));
                    if !self.in_decl {
                        let d = self
                            .dbg
                            .paras
                            .get(&p)
                            .cloned()
                            .or_else(|| self.dbg.all_procs.map(|d| (d, from.name.clone())));
                        if let Some((decl, name)) = d {
                            self.emit(Instr::DebugProc {
                                decl,
                                name,
                                contents: Some(to.name.clone()),
                                line: self.pos.line,
                            });
                        }
                    }
                }
            }
            Stmt::Perform { body, kind, pos } => {
                self.pos = *pos;
                self.perform(body, kind)?;
            }
            Stmt::Evaluate {
                subjects,
                whens,
                other,
            } => self.evaluate(subjects, whens, other.as_deref())?,
            Stmt::StopRun => self.emit(Instr::StopRun),
            Stmt::StopLiteral(op) => {
                let s = self.src(op)?;
                self.emit(Instr::StopLiteral(s));
            }
            Stmt::GoBack => self.emit(Instr::GoBack),
            Stmt::ExitProgram => self.emit(Instr::ExitProgram),
            Stmt::Display {
                items,
                upon,
                no_advancing,
            } => {
                let items = items
                    .iter()
                    .map(|i| self.src(i))
                    .collect::<Result<Vec<_>>>()?;
                let upon_console = upon.as_deref().is_some_and(|u| {
                    u == "CONSOLE"
                        || self
                            .prog
                            .env
                            .mnemonics
                            .iter()
                            .any(|(imp, m)| m == u && imp == "CONSOLE")
                });
                self.emit(Instr::Display {
                    items,
                    upon_console,
                    advance: !no_advancing,
                });
            }
            Stmt::Accept { target, from, pos } => {
                self.pos = *pos;
                let dst = self.loc(target)?;
                let from = match from.as_deref() {
                    None => zir::AcceptFrom::Stdin,
                    Some("DATE") => zir::AcceptFrom::Date,
                    Some("DATE YYYYMMDD") => zir::AcceptFrom::DateYyyymmdd,
                    Some("DAY") => zir::AcceptFrom::Day,
                    Some("DAY YYYYDDD") => zir::AcceptFrom::DayYyyyddd,
                    Some("DAY-OF-WEEK") => zir::AcceptFrom::DayOfWeek,
                    Some("TIME") => zir::AcceptFrom::Time,
                    Some(_) => zir::AcceptFrom::Stdin,
                };
                self.emit(Instr::Accept { dst, from });
            }
            Stmt::Open(groups) => {
                for (mode, files) in groups {
                    let m = match mode {
                        OpenMode::Input => zir::OpenMode::Input,
                        OpenMode::Output => zir::OpenMode::Output,
                        OpenMode::InputOutput => zir::OpenMode::InputOutput,
                        OpenMode::Extend => zir::OpenMode::Extend,
                    };
                    for f in files {
                        let fi = self.file(&f.name)?;
                        if f.reversed || f.no_rewind {
                            if self.files[fi as usize].org != zir::Org::Sequential {
                                return Err(Diag::error(
                                    self.pos,
                                    "tape OPEN options require sequential organization",
                                ));
                            }
                            self.emit(Instr::OpenTape {
                                file: fi,
                                mode: m,
                                reversed: f.reversed,
                                no_rewind: f.no_rewind,
                            });
                        } else {
                            self.emit(Instr::Open(fi, m));
                        }
                        self.debug_file(fi, false);
                        self.emit(Instr::UseCheck {
                            file: fi,
                            handled: 0,
                        });
                    }
                }
            }
            Stmt::Close(files) => {
                for (f, opt) in files {
                    let fi = self.file(f)?;
                    let lock = opt.as_deref() == Some("LOCK");
                    if opt.as_deref() == Some("NO REWIND") {
                        self.emit(Instr::CloseTape {
                            file: fi,
                            no_rewind: opt.as_deref() == Some("NO REWIND"),
                            removal: false,
                        });
                    } else if matches!(opt.as_deref(), Some(o) if o.starts_with("REEL") || o.starts_with("UNIT"))
                    {
                        self.emit(Instr::CloseReel(fi));
                    } else {
                        self.emit(Instr::Close(fi, lock));
                        self.debug_file(fi, false);
                    }
                    self.emit(Instr::UseCheck {
                        file: fi,
                        handled: 0,
                    });
                }
            }
            Stmt::Write {
                record,
                from,
                advancing,
                end_of_page,
                invalid_key,
            } => {
                let fi = self.file_of_record(record)?;
                let rec_desc = self.loc(record)?.desc;
                let from = match from {
                    Some(f) => Some(self.src(f)?),
                    None => None,
                };
                let advance = match advancing {
                    Some((before, adv)) => Some((
                        *before,
                        match adv {
                            Advancing::Page => zir::Advance::Page,
                            Advancing::Lines(n) => zir::Advance::Lines(self.ex_operand(n)?),
                            Advancing::Mnemonic(m) => {
                                // C01 / TOP-OF-PAGE style channel names
                                let is_page = self.prog.env.mnemonics.iter().any(|(imp, mn)| {
                                    mn == m
                                        && (imp == "C01"
                                            || imp == "TOP"
                                            || imp.starts_with("CSP")
                                            || imp == "FORMFEED")
                                });
                                if is_page {
                                    zir::Advance::Page
                                } else {
                                    zir::Advance::Lines(Ex::Lit(Dec::from_i64(1)))
                                }
                            }
                        },
                    )),
                    None => None,
                };
                self.emit(Instr::Write {
                    file: fi,
                    record: rec_desc,
                    from,
                    advance,
                });
                self.use_check(fi, &[], &[invalid_key]);
                // END-OF-PAGE and INVALID KEY share one parsed phrase set.
                self.handlers(end_of_page, Cx::IoCond)?;
            }
            Stmt::Rewrite {
                record,
                from,
                invalid_key,
            } => {
                let fi = self.file_of_record(record)?;
                let rec_desc = self.loc(record)?.desc;
                let from = match from {
                    Some(f) => Some(self.src(f)?),
                    None => None,
                };
                self.emit(Instr::Rewrite {
                    file: fi,
                    record: rec_desc,
                    from,
                });
                self.use_check(fi, &[], &[invalid_key]);
                self.handlers(invalid_key, Cx::IoCond)?;
            }
            Stmt::Read {
                file,
                next,
                into,
                key,
                at_end,
                invalid_key,
            } => {
                let fi = self.file(file)?;
                let into = match into {
                    Some(i) => Some(self.loc(i)?),
                    None => None,
                };
                let key = match key {
                    Some(k) => {
                        let kl = self.loc(k)?;
                        let def = &self.files[fi as usize];
                        let pos = def
                            .alternate_keys
                            .iter()
                            .position(|(l, _)| l.desc == kl.desc);
                        pos.map(|p| p as u32 + 1)
                    }
                    None => None,
                };
                self.emit(Instr::Read {
                    file: fi,
                    next: *next,
                    into,
                    key,
                });
                self.debug_file(fi, true);
                self.use_check(fi, &[at_end], &[invalid_key]);
                // AT END and INVALID KEY share one parsed phrase set.
                self.handlers(at_end, Cx::IoCond)?;
            }
            Stmt::Delete { file, invalid_key } => {
                let fi = self.file(file)?;
                self.emit(Instr::Delete(fi));
                self.debug_file(fi, false);
                self.use_check(fi, &[], &[invalid_key]);
                self.handlers(invalid_key, Cx::IoCond)?;
            }
            Stmt::Start {
                file,
                key,
                invalid_key,
            } => {
                let fi = self.file(file)?;
                let (r, k) = match key {
                    Some((op, k)) => (rel(*op), Some(self.loc(k)?)),
                    None => (zir::Rel::Eq, None),
                };
                self.emit(Instr::Start {
                    file: fi,
                    rel: r,
                    key: k,
                });
                self.debug_file(fi, false);
                self.use_check(fi, &[], &[invalid_key]);
                self.handlers(invalid_key, Cx::IoCond)?;
            }
            Stmt::Call {
                interface,
                program,
                using,
                returning,
                exception,
                pos,
            } => {
                self.pos = *pos;
                self.validate_call_target(program, true)?;
                let program = self.src(program)?;
                let mut args = Vec::new();
                for a in using {
                    args.push(match a {
                        CallArg::Reference(Operand::Ref(r)) => {
                            zir::CallArgMode::Reference(self.loc(r)?)
                        }
                        CallArg::Reference(op) | CallArg::Content(op) => {
                            let len = match op {
                                Operand::Ref(r) => {
                                    let l = self.loc(r)?;
                                    self.dm.descs[l.desc as usize].len
                                }
                                Operand::Lit(l) => super::lit::literal_bytes(l).len() as u32,
                                _ => 1,
                            };
                            zir::CallArgMode::Content(self.src(op)?, len)
                        }
                        CallArg::Value(op) => {
                            match op {
                                Operand::Ref(r) if self.symbolics.contains_key(&r.name) && r.qualifiers.is_empty() && r.subscripts.is_empty() && r.refmod.is_none() => {},
                                Operand::Ref(r) => {
                                    let loc = self.loc(r)?;
                                    let desc = &self.dm.descs[loc.desc as usize];
                                    let valid = if let Some((_, length)) = &r.refmod {
                                        let display = desc.ty.is_national() || matches!(desc.ty,
                                            DataType::Group | DataType::Alphabetic { .. } | DataType::Alphanumeric { .. }
                                            | DataType::NumericEdited { national: false, .. } | DataType::AlphanumericEdited { national: false, .. }
                                            | DataType::ExternalFloat { national: false, .. })
                                            || matches!(&desc.ty, DataType::Numeric(t) if t.usage == zir::NumUsage::Display);
                                        let length_valid = match length.as_deref() {
                                            Some(Expr::Operand(Operand::Lit(Literal::Num(n)))) => n.parse::<u32>() == Ok(1),
                                            _ => true, // Runtime verifies a computed slice's width.
                                        };
                                        display && length_valid
                                    } else { desc.ty.by_value_allowed(desc.len) };
                                    if !valid {
                                        return Err(Diag::error(r.pos, "invalid CALL BY VALUE data category or length"));
                                    }
                                }
                                Operand::Lit(Literal::Num(n)) if !n.contains('E') => {
                                    if n.chars().filter(char::is_ascii_digit).count() > 9 {
                                        return Err(Diag::error(*pos, "BY VALUE fixed-point literal must contain at most nine digits"));
                                    }
                                }
                                Operand::Lit(l @ (Literal::Str(_) | Literal::Hex(_))) if super::lit::literal_bytes(l).len() != 1 => return Err(Diag::error(*pos, "BY VALUE character literal must have length one")),
                                Operand::Lit(Literal::National(b)) if b.len() != 2 => return Err(Diag::error(*pos, "BY VALUE national literal must have length one")),
                                _ => {},
                            }
                            match op {
                                Operand::Ref(_) | Operand::Lit(Literal::Str(_) | Literal::Hex(_) | Literal::National(_))
                                    | Operand::Fig(ast::Figurative::Space | ast::Figurative::Quote | ast::Figurative::HighValue | ast::Figurative::LowValue) => zir::CallArgMode::ValueData(self.src(op)?),
                                _ => zir::CallArgMode::Value(self.ex_operand(op)?),
                            }
                        }
                        CallArg::Omitted => zir::CallArgMode::Omitted,
                    });
                }
                let returning = match returning {
                    Some(r) => Some(self.loc(r)?),
                    None => None,
                };
                self.emit(Instr::Call {
                    interface: *interface,
                    program,
                    args,
                    returning,
                });
                self.handlers(exception, Cx::CallException)?;
            }
            Stmt::Cancel(progs) => {
                for p in progs {
                    self.validate_call_target(p, false)?;
                    let s = self.src(p)?;
                    self.emit(Instr::Cancel(s));
                }
            }
            Stmt::Set(set) => self.set(set)?,
            Stmt::Initialize {
                targets,
                replacing,
                options,
            } => {
                for t in targets {
                    self.initialize(t, replacing, options)?;
                }
            }
            Stmt::Inspect { target, ops } => self.inspect(target, ops)?,
            Stmt::String {
                sources,
                into,
                pointer,
                overflow,
            } => {
                let mut srcs = Vec::new();
                for (s, d) in sources {
                    let d = match d {
                        Some(d) => Some(self.src(d)?),
                        None => None,
                    };
                    srcs.push((self.src(s)?, d));
                }
                let into = self.loc(into)?;
                let pointer = match pointer {
                    Some(p) => Some(self.loc(p)?),
                    None => None,
                };
                self.emit(Instr::StringStmt {
                    sources: srcs,
                    into,
                    pointer,
                });
                self.handlers(overflow, Cx::Overflow)?;
            }
            Stmt::Unstring {
                source,
                delimiters,
                into,
                pointer,
                tallying,
                overflow,
            } => {
                let source = self.loc(source)?;
                let mut delims = Vec::new();
                for (all, d) in delimiters {
                    delims.push((*all, self.src(d)?));
                }
                let mut targets = Vec::new();
                for (t, d, c) in into {
                    let t = self.loc(t)?;
                    let d = match d {
                        Some(d) => Some(self.loc(d)?),
                        None => None,
                    };
                    let c = match c {
                        Some(c) => Some(self.loc(c)?),
                        None => None,
                    };
                    targets.push((t, d, c));
                }
                let pointer = match pointer {
                    Some(p) => Some(self.loc(p)?),
                    None => None,
                };
                let tallying = match tallying {
                    Some(t) => Some(self.loc(t)?),
                    None => None,
                };
                self.emit(Instr::Unstring {
                    source,
                    delims,
                    into: targets,
                    pointer,
                    tallying,
                });
                self.handlers(overflow, Cx::Overflow)?;
            }
            Stmt::Search {
                table,
                all,
                varying,
                at_end,
                whens,
            } => self.search(table, *all, varying.as_ref(), at_end.as_deref(), whens)?,
            Stmt::Sort { .. } | Stmt::Merge { .. } | Stmt::Release { .. } | Stmt::Return { .. } => {
                self.sort_stmt(s)?
            }
            // Alternate entry points are recorded only; executed in line,
            // ENTRY does nothing.
            Stmt::Entry {
                name,
                using,
                using_by_value,
                pos,
            } => {
                if self.prog.procedure.giving.is_some() {
                    return Err(Diag::error(
                        self.pos,
                        "PROCEDURE RETURNING cannot be combined with ENTRY",
                    ));
                }
                let name = name.to_ascii_uppercase();
                if self.nested
                    || self.prog.name.eq_ignore_ascii_case(&name)
                    || self.entries.iter().any(|e| e.name == name)
                {
                    return Err(Diag::error(
                        *pos,
                        "ENTRY must be non-nested and its name must be unique",
                    ));
                }
                let mut slots = Vec::new();
                for (index, r) in using.iter().enumerate() {
                    let id = self.dm.resolve(r)?;
                    let item = &self.dm.items[id];
                    let Area::Linkage(slot) = item.area else {
                        return Err(Diag::error(r.pos, "ENTRY USING requires a LINKAGE item"));
                    };
                    if !matches!(item.level, 1 | 77)
                        || !r.subscripts.is_empty()
                        || r.refmod.is_some()
                        || slots.contains(&slot)
                    {
                        return Err(Diag::error(
                            r.pos,
                            "ENTRY USING requires unique level 01/77 LINKAGE items",
                        ));
                    }
                    slots.push(slot);
                    let desc = &self.dm.descs[item.desc as usize];
                    if using_by_value.get(index) == Some(&true)
                        && !desc.ty.by_value_allowed(desc.len)
                    {
                        return Err(Diag::error(r.pos, "invalid ENTRY USING BY VALUE category"));
                    }
                }
                self.entries.push(zir::EntryPoint {
                    name,
                    using: slots,
                    using_by_value: using_by_value.clone(),
                    pc: self.code.len() as u32,
                });
            }
            Stmt::Exec(e) => {
                self.pos = e.pos;
                if e.kind == ExecKind::SqlIms {
                    if self.in_decl {
                        return Err(Diag::error(
                            e.pos,
                            "EXEC SQLIMS is not permitted in COBOL declaratives",
                        ));
                    }
                    if e.command == "WHENEVER" {
                        self.sqlims_whenever(e)?;
                        return Ok(());
                    }
                    if matches!(e.command.as_str(), "DECLARE" | "BEGIN" | "END") {
                        return Ok(());
                    }
                }
                let def = self.exec_def(e)?;
                self.execs.push(def);
                self.emit(Instr::Exec(self.execs.len() as u32 - 1));
            }
            Stmt::Unsupported(name, pos) => {
                self.pos = *pos;
                self.emit(Instr::Unsupported(name.clone()));
            }
        }
        Ok(())
    }

    /// CALL/CANCEL names have DISPLAY categories; entry pointers are a
    /// separate CALL format, never a license to treat any numeric as a name.
    fn validate_call_target(&mut self, target: &Operand, pointers: bool) -> Result<()> {
        let valid = match target {
            Operand::Lit(Literal::Str(_) | Literal::Hex(_)) => true,
            Operand::Ref(r) => {
                let loc = self.loc(r)?;
                let ty = &self.dm.descs[loc.desc as usize].ty;
                let display = matches!(
                    ty,
                    DataType::Group | DataType::Alphabetic { .. } | DataType::Alphanumeric { .. }
                ) || matches!(ty, DataType::Numeric(t) if t.usage == zir::NumUsage::Display);
                display
                    || (r.refmod.is_some()
                        && matches!(
                            ty,
                            DataType::NumericEdited {
                                national: false,
                                ..
                            } | DataType::AlphanumericEdited {
                                national: false,
                                ..
                            } | DataType::ExternalFloat {
                                national: false,
                                ..
                            }
                        ))
                    || (pointers && ty.is_entry_pointer() && r.refmod.is_none())
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(Diag::error(
                self.pos,
                if pointers {
                    "CALL target requires an alphanumeric literal, a DISPLAY alphabetic/alphanumeric/numeric item, or an entry pointer"
                } else {
                    "CANCEL target requires an alphanumeric literal or a DISPLAY alphabetic/alphanumeric/numeric item"
                },
            ))
        }
    }

    fn set(&mut self, set: &SetStmt) -> Result<()> {
        match set {
            SetStmt::Entry(targets, name, interface) => {
                let valid_name = match name {
                    Operand::Lit(Literal::Str(_) | Literal::Hex(_)) => true,
                    Operand::Ref(r) => {
                        let loc = self.loc(r)?;
                        let ty = &self.dm.descs[loc.desc as usize].ty;
                        matches!(ty, DataType::Group | DataType::Alphanumeric { .. })
                            || (r.refmod.is_some()
                                && (matches!(
                                    ty,
                                    DataType::Alphabetic { .. }
                                        | DataType::NumericEdited {
                                            national: false,
                                            ..
                                        }
                                        | DataType::AlphanumericEdited {
                                            national: false,
                                            ..
                                        }
                                        | DataType::ExternalFloat {
                                            national: false,
                                            ..
                                        }
                                ) || matches!(ty, DataType::Numeric(t) if t.usage == zir::NumUsage::Display)))
                    }
                    _ => false,
                };
                if !valid_name {
                    return Err(Diag::error(
                        self.pos,
                        "SET TO ENTRY requires an alphanumeric program name",
                    ));
                }
                let mut locs = Vec::new();
                for target in targets {
                    let loc = self.loc(target)?;
                    if !self.dm.descs[loc.desc as usize].ty.is_entry_pointer()
                        || target.refmod.is_some()
                    {
                        return Err(Diag::error(
                            target.pos,
                            "SET TO ENTRY requires a procedure/function-pointer receiver",
                        ));
                    }
                    locs.push(loc);
                }
                let name = self.src(name)?;
                if let Some(dynamic) = interface {
                    self.emit(Instr::SetEntryWithInterface(locs, name, *dynamic));
                } else {
                    self.emit(Instr::SetEntry(locs, name));
                }
            }
            SetStmt::To(targets, value) => {
                let v = match value {
                    Operand::Ref(r) => {
                        let l = self.loc(r)?;
                        Ex::Load(l)
                    }
                    op => self.ex_operand(op)?,
                };
                for t in targets {
                    let l = self.loc(t)?;
                    self.emit(Instr::SetIndex(l, v.clone()));
                }
            }
            SetStmt::UpDown(targets, up, by) => {
                let b = self.ex(by)?;
                for t in targets {
                    let l = self.loc(t)?;
                    let v = Ex::Bin(
                        if *up { Op::Add } else { Op::Sub },
                        Box::new(Ex::Load(l.clone())),
                        Box::new(b.clone()),
                    );
                    self.emit(Instr::SetIndex(l, v));
                }
            }
            SetStmt::Switch(list) => {
                for (name, on) in list {
                    let idx = *self
                        .switch_names
                        .get(name)
                        .ok_or_else(|| Diag::error(self.pos, format!("{name} is not a switch")))?;
                    self.emit(Instr::SetSwitch(idx, *on));
                }
            }
            SetStmt::Address(targets, value) => {
                let v = self.ex_operand(value)?;
                for t in targets {
                    let id = self.dm.resolve(t)?;
                    match self.dm.items[id].area {
                        Area::Linkage(n) if self.dm.items[id].parent.is_none() => {
                            self.emit(Instr::SetAddress(n, v.clone()));
                        }
                        _ => {
                            return Err(Diag::error(
                                t.pos,
                                format!("{} is not a LINKAGE SECTION record", t.name),
                            ));
                        }
                    }
                }
            }
            SetStmt::CondTrue(targets) | SetStmt::CondFalse(targets) => {
                for t in targets {
                    let id = self.dm.resolve(t)?;
                    let Some(var) = self.dm.items[id].cond_of else {
                        return Err(Diag::error(
                            t.pos,
                            format!("{} is not a condition-name", t.name),
                        ));
                    };
                    let value = if matches!(set, SetStmt::CondFalse(_)) {
                        self.dm.items[id].entry.false_value.clone().ok_or_else(|| {
                            Diag::error(t.pos, "SET TO FALSE requires WHEN SET TO FALSE")
                        })?
                    } else {
                        match &self.dm.items[id].entry.value {
                            Some(ValueClause::Condition(v)) => v[0].0.clone(),
                            Some(ValueClause::Literal(v)) => v.clone(),
                            None => return Err(Diag::error(t.pos, "condition-name without VALUE")),
                        }
                    };
                    let mut vr = t.clone();
                    vr.name = self.dm.items[var].name.clone().unwrap_or_default();
                    vr.qualifiers.clear();
                    let dst = self.loc_of(var, &vr)?;
                    let src = self.src(&value)?;
                    self.emit(Instr::Move(src, dst));
                }
            }
        }
        Ok(())
    }

    /// INITIALIZE: moves SPACES/ZEROS (or the REPLACING values) to every
    /// elementary item, all occurrences included.
    fn initialize(
        &mut self,
        target: &DataRef,
        replacing: &[(String, Operand)],
        options: &InitializeOptions,
    ) -> Result<()> {
        let id = self.dm.resolve(target)?;
        if self.dm.items[id].entry.renames.is_some() {
            return Err(Diag::error(
                target.pos,
                "INITIALIZE receiver cannot have a RENAMES clause",
            ));
        }
        if self.dm.items[id].cond_of.is_some() {
            return Err(Diag::error(
                target.pos,
                "INITIALIZE receiver cannot be a condition-name",
            ));
        }
        let base = self.loc_of(id, target)?;
        let base_desc = self.dm.descs[base.desc as usize].clone();
        if matches!(
            base_desc.ty,
            DataType::Index
                | DataType::Pointer
                | DataType::ProcedurePointer
                | DataType::FunctionPointer
        ) {
            return Err(Diag::error(
                target.pos,
                "INITIALIZE receiver cannot be INDEX or POINTER",
            ));
        }
        let mut repl = Vec::new();
        for (cat, v) in replacing {
            repl.push((cat.as_str(), self.src(v)?));
        }
        // Collect elementary items (excluding FILLER, REDEFINES, indexes).
        let mut leaves = Vec::new();
        self.leaves(id, true, options.with_filler, &mut leaves);
        for leaf in leaves {
            let desc_idx = self.dm.items[leaf].desc;
            let d = self.dm.descs[desc_idx as usize].clone();
            let cat = match &d.ty {
                DataType::Alphabetic { .. } => "ALPHABETIC",
                DataType::Alphanumeric { .. } | DataType::Group => "ALPHANUMERIC",
                DataType::Dbcs => "DBCS",
                DataType::National => "NATIONAL",
                DataType::Numeric(_) | DataType::Float { .. } | DataType::ExternalFloat { .. } => {
                    "NUMERIC"
                }
                DataType::NumericEdited { .. } => "NUMERIC-EDITED",
                DataType::AlphanumericEdited { national: true, .. } => "NATIONAL-EDITED",
                DataType::AlphanumericEdited { .. } => "ALPHANUMERIC-EDITED",
                DataType::Index
                | DataType::Pointer
                | DataType::ProcedurePointer
                | DataType::FunctionPointer => continue,
            };
            let value = if options
                .value_categories
                .iter()
                .any(|c| c == "ALL" || c == cat)
            {
                match self.dm.items[leaf].entry.value.clone() {
                    Some(ValueClause::Literal(v)) => {
                        // VALUE initialization ignores JUSTIFIED. A full-width
                        // sending value preserves that initial layout on MOVE.
                        if matches!(
                            d.ty,
                            DataType::Alphanumeric { just: true }
                                | DataType::Alphabetic { just: true }
                        ) {
                            let mut initial = vec![0; d.len as usize];
                            self.dm.store_value(&d.ty, &mut initial, &v, target.pos)?;
                            Some(Src::Alnum(initial))
                        } else {
                            Some(self.src(&v)?)
                        }
                    }
                    _ => None,
                }
            } else {
                None
            };
            let replacement = repl.iter().find(|(c, _)| *c == cat).map(|(_, s)| s.clone());
            let src = if let Some(s) = value.or(replacement) {
                s
            } else if options.to_default
                || (replacing.is_empty() && options.value_categories.is_empty())
            {
                match cat {
                    "NUMERIC" | "NUMERIC-EDITED" => Src::Fig(zir::Fig::Zero),
                    _ => Src::Fig(zir::Fig::Space),
                }
            } else {
                continue;
            };
            // Every combination of the dimensions below the target.
            let extra = &d.dims[base_desc.dims.len().min(d.dims.len())..];
            let counts: Vec<u32> = extra.iter().map(|x| x.max).collect();
            let mut idx = vec![1u32; counts.len()];
            loop {
                let mut subs: Vec<Ex> = if base.subs.is_empty() && !base_desc.dims.is_empty() {
                    vec![Ex::Lit(Dec::from_i64(1)); base_desc.dims.len()]
                } else {
                    base.subs.clone()
                };
                subs.extend(idx.iter().map(|&i| Ex::Lit(Dec::from_i64(i as i64))));
                let loc = Loc {
                    desc: desc_idx,
                    subs: if d.dims.is_empty() { Vec::new() } else { subs },
                    refmod: None,
                };
                self.emit(Instr::Move(src.clone(), loc));
                // next combination
                let mut k = counts.len();
                loop {
                    if k == 0 {
                        break;
                    }
                    k -= 1;
                    idx[k] += 1;
                    if idx[k] <= counts[k] {
                        break;
                    }
                    idx[k] = 1;
                    if k == 0 {
                        k = usize::MAX;
                        break;
                    }
                }
                if counts.is_empty() || k == usize::MAX {
                    break;
                }
            }
        }
        Ok(())
    }

    fn leaves(&self, id: usize, top: bool, with_filler: bool, out: &mut Vec<usize>) {
        let it = &self.dm.items[id];
        if !top && (it.entry.redefines.is_some() || it.entry.renames.is_some()) {
            return;
        }
        if it.children.is_empty() {
            if top || with_filler || it.name.is_some() {
                out.push(id);
            }
        } else {
            for &c in &it.children {
                self.leaves(c, false, with_filler, out);
            }
        }
    }

    fn inspect(&mut self, target: &DataRef, ops: &[InspectOp]) -> Result<()> {
        let loc = self.loc(target)?;
        let mut steps = Vec::new();
        let delims = |g: &mut Gen, d: &[(bool, Operand)]| -> Result<(Option<Src>, Option<Src>)> {
            let mut before = None;
            let mut after = None;
            for (b, op) in d {
                let s = g.src(op)?;
                if *b {
                    before = Some(s);
                } else {
                    after = Some(s);
                }
            }
            Ok((before, after))
        };
        for op in ops {
            match op {
                InspectOp::Tallying(clauses) => {
                    for c in clauses {
                        let counter = self.loc(&c.counter)?;
                        for item in &c.items {
                            let (kind, d) = match item {
                                TallyItem::Characters(d) => (zir::InspectKind::Characters, d),
                                TallyItem::Pattern(leading, p, d) => {
                                    let s = self.src(p)?;
                                    (
                                        if *leading {
                                            zir::InspectKind::Leading(s)
                                        } else {
                                            zir::InspectKind::All(s)
                                        },
                                        d,
                                    )
                                }
                            };
                            let (before, after) = delims(self, d)?;
                            steps.push(zir::InspectStep::Tally {
                                counter: counter.clone(),
                                kind,
                                before,
                                after,
                            });
                        }
                    }
                }
                InspectOp::Replacing(clauses) => {
                    for c in clauses {
                        let (kind, by, d) = match c {
                            ReplaceClause::Characters(by, d) => {
                                (zir::InspectKind::Characters, self.src(by)?, d)
                            }
                            ReplaceClause::Pattern(mode, p, by, d) => {
                                let s = self.src(p)?;
                                (
                                    match mode {
                                        ReplaceMode::All => zir::InspectKind::All(s),
                                        ReplaceMode::Leading => zir::InspectKind::Leading(s),
                                        ReplaceMode::First => zir::InspectKind::First(s),
                                    },
                                    self.src(by)?,
                                    d,
                                )
                            }
                        };
                        let (before, after) = delims(self, d)?;
                        steps.push(zir::InspectStep::Replace {
                            kind,
                            by,
                            before,
                            after,
                        });
                    }
                }
                InspectOp::Converting {
                    from,
                    to,
                    delims: d,
                } => {
                    let (before, after) = delims(self, d)?;
                    steps.push(zir::InspectStep::Convert {
                        from: self.src(from)?,
                        to: self.src(to)?,
                        before,
                        after,
                    });
                }
            }
        }
        // TALLYING and REPLACING phrases are separate passes.
        let tallies: Vec<_> = steps
            .iter()
            .filter(|s| matches!(s, zir::InspectStep::Tally { .. }))
            .cloned()
            .collect();
        let others: Vec<_> = steps
            .into_iter()
            .filter(|s| !matches!(s, zir::InspectStep::Tally { .. }))
            .collect();
        if !tallies.is_empty() {
            self.emit(Instr::Inspect(loc.clone(), tallies));
        }
        if !others.is_empty() {
            self.emit(Instr::Inspect(loc, others));
        }
        Ok(())
    }

    fn search(
        &mut self,
        table: &DataRef,
        all: bool,
        varying: Option<&DataRef>,
        at_end: Option<&[Stmt]>,
        whens: &[(Cond, Vec<Stmt>)],
    ) -> Result<()> {
        let tid = self.dm.resolve(table)?;
        let occurs = self.dm.items[tid]
            .occurs
            .clone()
            .ok_or_else(|| Diag::error(table.pos, format!("{} has no OCCURS", table.name)))?;
        // VARYING one of the table's own index-names selects that index.
        let index_name = match varying {
            Some(v) if v.qualifiers.is_empty() && occurs.indexed_by.contains(&v.name) => {
                Some(v.name.clone())
            }
            _ => occurs.indexed_by.first().cloned(),
        };
        let index_loc = match (&index_name, varying) {
            (Some(n), _) => {
                let r = DataRef {
                    name: n.clone(),
                    qualifiers: Vec::new(),
                    subscripts: Vec::new(),
                    refmod: None,
                    pos: table.pos,
                };
                self.loc(&r)?
            }
            (None, Some(v)) => self.loc(v)?,
            (None, None) => {
                return Err(Diag::error(table.pos, "SEARCH needs an INDEXED BY table"));
            }
        };
        let varying_loc = match varying {
            Some(v) if index_name.is_some() => {
                let l = self.loc(v)?;
                if l.desc == index_loc.desc {
                    None
                } else {
                    Some(l)
                }
            }
            _ => None,
        };
        let max = match &occurs.depending {
            Some(d) => Ex::Load(self.loc(d)?),
            None => Ex::Lit(Dec::from_i64(occurs.max as i64)),
        };
        let top = self.label();
        let end = self.label();
        let end_l = self.label();
        if all {
            // SEARCH ALL examines the whole table.
            self.emit(Instr::SetIndex(
                index_loc.clone(),
                Ex::Lit(Dec::from_i64(1)),
            ));
        }
        self.bind(top);
        self.emit(Instr::JumpIf(
            Cx::Cmp(
                Src::Num(Ex::Load(index_loc.clone())),
                zir::Rel::Gt,
                Src::Num(max.clone()),
                zir::CmpMode::Numeric,
            ),
            end_l,
        ));
        self.emit(Instr::JumpIf(
            Cx::Cmp(
                Src::Num(Ex::Load(index_loc.clone())),
                zir::Rel::Lt,
                Src::Num(Ex::Lit(Dec::from_i64(1))),
                zir::CmpMode::Numeric,
            ),
            end_l,
        ));
        for (c, body) in whens {
            let next = self.label();
            let cx = self.cond(c)?;
            self.emit(Instr::JumpUnless(cx, next));
            self.stmts(body)?;
            self.emit(Instr::Jump(end));
            self.bind(next);
        }
        let inc = Ex::Bin(
            Op::Add,
            Box::new(Ex::Load(index_loc.clone())),
            Box::new(Ex::Lit(Dec::from_i64(1))),
        );
        self.emit(Instr::SetIndex(index_loc.clone(), inc));
        if let Some(v) = varying_loc {
            let inc = Ex::Bin(
                Op::Add,
                Box::new(Ex::Load(v.clone())),
                Box::new(Ex::Lit(Dec::from_i64(1))),
            );
            self.emit(Instr::SetIndex(v, inc));
        }
        self.emit(Instr::Jump(top));
        self.bind(end_l);
        if let Some(a) = at_end {
            self.stmts(a)?;
        }
        self.bind(end);
        Ok(())
    }

    fn evaluate(
        &mut self,
        subjects: &[EvalSubject],
        whens: &[(Vec<WhenClause>, Vec<Stmt>)],
        other: Option<&[Stmt]>,
    ) -> Result<()> {
        let end = self.label();
        for (clauses, body) in whens {
            let mut any: Option<Cx> = None;
            for clause in clauses {
                let mut all: Option<Cx> = None;
                for (subj, obj) in subjects.iter().zip(&clause.objects) {
                    let c = self.when_cond(subj, obj)?;
                    all = Some(match all {
                        None => c,
                        Some(p) => Cx::And(Box::new(p), Box::new(c)),
                    });
                }
                let c = all.unwrap_or(Cx::True);
                any = Some(match any {
                    None => c,
                    Some(p) => Cx::Or(Box::new(p), Box::new(c)),
                });
            }
            let next = self.label();
            self.emit(Instr::JumpUnless(any.unwrap_or(Cx::True), next));
            self.stmts(body)?;
            self.emit(Instr::Jump(end));
            self.bind(next);
        }
        if let Some(o) = other {
            self.stmts(o)?;
        }
        self.bind(end);
        Ok(())
    }

    fn when_cond(&mut self, subj: &EvalSubject, obj: &EvalObject) -> Result<Cx> {
        let truth = |g: &mut Gen, s: &EvalSubject| -> Result<Option<Cx>> {
            Ok(match s {
                EvalSubject::True => Some(Cx::True),
                EvalSubject::False => Some(Cx::Not(Box::new(Cx::True))),
                EvalSubject::Cond(c) => Some(g.cond(c)?),
                EvalSubject::Expr(e) => match g.condition_ref(e) {
                    Some(r) => Some(g.condition_name(&r)?),
                    None => None,
                },
            })
        };
        Ok(match obj {
            EvalObject::Any => Cx::True,
            EvalObject::True | EvalObject::False | EvalObject::Cond(_) => {
                let o = match obj {
                    EvalObject::True => Cx::True,
                    EvalObject::False => Cx::Not(Box::new(Cx::True)),
                    EvalObject::Cond(c) => self.cond(c)?,
                    _ => unreachable!(),
                };
                match truth(self, subj)? {
                    Some(Cx::True) if matches!(subj, EvalSubject::True) => o,
                    Some(s) => Cx::Or(
                        Box::new(Cx::And(Box::new(s.clone()), Box::new(o.clone()))),
                        Box::new(Cx::And(
                            Box::new(Cx::Not(Box::new(s))),
                            Box::new(Cx::Not(Box::new(o))),
                        )),
                    ),
                    None => {
                        return Err(Diag::error(
                            self.pos,
                            "WHEN condition does not match EVALUATE subject",
                        ));
                    }
                }
            }
            EvalObject::Value {
                not,
                from,
                thru: None,
            } if !matches!(subj, EvalSubject::Expr(e) if self.condition_ref(e).is_none())
                && self.condition_ref(from).is_some() =>
            {
                let r = self.condition_ref(from).unwrap();
                let o = self.condition_name(&r)?;
                let o = if *not { Cx::Not(Box::new(o)) } else { o };
                match truth(self, subj)? {
                    Some(Cx::True) => o,
                    Some(s) => Cx::Or(
                        Box::new(Cx::And(Box::new(s.clone()), Box::new(o.clone()))),
                        Box::new(Cx::And(
                            Box::new(Cx::Not(Box::new(s))),
                            Box::new(Cx::Not(Box::new(o))),
                        )),
                    ),
                    None => {
                        return Err(Diag::error(
                            self.pos,
                            "WHEN condition does not match EVALUATE subject",
                        ));
                    }
                }
            }
            EvalObject::Value { not, from, thru } => {
                let EvalSubject::Expr(e) = subj else {
                    return Err(Diag::error(
                        self.pos,
                        "WHEN value requires an EVALUATE expression",
                    ));
                };
                let c = match thru {
                    None => self.relation(e, RelOp::Eq, from)?,
                    Some(t) => Cx::And(
                        Box::new(self.relation(e, RelOp::Ge, from)?),
                        Box::new(self.relation(e, RelOp::Le, t)?),
                    ),
                };
                if *not { Cx::Not(Box::new(c)) } else { c }
            }
        })
    }

    fn perform_call(&mut self, body: &PerformBody) -> Result<()> {
        match body {
            PerformBody::Procedures(a, b) => {
                let (first, last_a) = self.proc_range(a)?;
                let last = match b {
                    Some(b) => self.proc_range(b)?.1,
                    None => last_a,
                };
                let start = self.entry_label(a, first);
                let ret = self.label();
                self.emit(Instr::Perform {
                    start,
                    end: last,
                    ret,
                });
                self.bind(ret);
            }
            PerformBody::Inline(stmts) => self.stmts(stmts)?,
        }
        Ok(())
    }

    fn perform(&mut self, body: &PerformBody, kind: &PerformKind) -> Result<()> {
        let end = self.label();
        let cycle = self.label();
        let inline = matches!(body, PerformBody::Inline(_));
        if inline {
            self.inline_performs.push((end, cycle));
        }
        match kind {
            PerformKind::Once => {
                self.perform_call(body)?;
                self.bind(cycle);
            }
            PerformKind::Times(n) => {
                let t = self.temp();
                let v = self.ex_operand(n)?;
                self.emit(Instr::SetTemp(t, v));
                let top = self.label();
                self.bind(top);
                self.emit(Instr::JumpIf(
                    Cx::Cmp(
                        Src::Num(Ex::Temp(t)),
                        zir::Rel::Le,
                        Src::Num(Ex::Lit(Dec::ZERO)),
                        zir::CmpMode::Numeric,
                    ),
                    end,
                ));
                self.emit(Instr::SetTemp(
                    t,
                    Ex::Bin(
                        Op::Sub,
                        Box::new(Ex::Temp(t)),
                        Box::new(Ex::Lit(Dec::from_i64(1))),
                    ),
                ));
                self.perform_call(body)?;
                self.bind(cycle);
                self.emit(Instr::Jump(top));
            }
            PerformKind::Until { test_after, cond } => {
                let c = self.cond(cond)?;
                let top = self.label();
                self.bind(top);
                if !test_after {
                    self.emit(Instr::JumpIf(c.clone(), end));
                }
                self.perform_call(body)?;
                self.bind(cycle);
                if *test_after {
                    self.emit(Instr::JumpUnless(c, top));
                } else {
                    self.emit(Instr::Jump(top));
                }
            }
            PerformKind::Varying { test_after, loops } => {
                let mut vars = Vec::new();
                for l in loops {
                    let var = self.loc(&l.var)?;
                    let from = self.ex(&l.from)?;
                    let by = self.ex(&l.by)?;
                    let until = self.cond(&l.until)?;
                    vars.push((var, from, by, until));
                }
                // Debugging: after each initialization or augmentation of a
                // variable and each evaluation of a condition.
                let set = |g: &mut Gen, var: &Loc, v: Ex| {
                    let ins = Instr::SetIndex(var.clone(), v);
                    let ts = if g.in_decl {
                        vec![]
                    } else {
                        g.debug_data_triggers(std::slice::from_ref(&ins), true)
                    };
                    g.emit(ins);
                    for t in ts {
                        g.emit(t);
                    }
                };
                let test = |g: &mut Gen, c: &Cx| {
                    if !g.in_decl {
                        let probe = Instr::JumpIf(c.clone(), 0);
                        for t in g.debug_data_triggers(std::slice::from_ref(&probe), true) {
                            g.emit(t);
                        }
                    }
                };
                let inc = |var: &Loc, by: &Ex| {
                    Ex::Bin(
                        Op::Add,
                        Box::new(Ex::Load(var.clone())),
                        Box::new(by.clone()),
                    )
                };
                if !test_after {
                    for (var, from, _, _) in &vars {
                        set(self, var, from.clone());
                    }
                    let tests: Vec<Label> = (0..vars.len()).map(|_| self.label()).collect();
                    // test of level 0
                    self.bind(tests[0]);
                    test(self, &vars[0].3);
                    self.emit(Instr::JumpIf(vars[0].3.clone(), end));
                    for k in 1..vars.len() {
                        self.bind(tests[k]);
                        let over = self.label();
                        let cont = self.label();
                        test(self, &vars[k].3);
                        self.emit(Instr::JumpUnless(vars[k].3.clone(), cont));
                        self.bind(over);
                        // level k exhausted: augment level k-1, reset level k
                        let (pv, _, pb, _) = vars[k - 1].clone();
                        set(self, &pv, inc(&pv, &pb));
                        set(self, &vars[k].0.clone(), vars[k].1.clone());
                        self.emit(Instr::Jump(tests[k - 1]));
                        self.bind(cont);
                    }
                    self.perform_call(body)?;
                    self.bind(cycle);
                    let (lv, _, lb, _) = vars.last().unwrap().clone();
                    set(self, &lv, inc(&lv, &lb));
                    self.emit(Instr::Jump(*tests.last().unwrap()));
                } else {
                    for (var, from, _, _) in &vars {
                        set(self, var, from.clone());
                    }
                    let body_l = self.label();
                    self.bind(body_l);
                    self.perform_call(body)?;
                    self.bind(cycle);
                    // innermost to outermost
                    for k in (0..vars.len()).rev() {
                        let next = self.label();
                        test(self, &vars[k].3);
                        self.emit(Instr::JumpIf(vars[k].3.clone(), next));
                        let (v, _, b, _) = vars[k].clone();
                        set(self, &v, inc(&v, &b));
                        for (iv, ifrom, _, _) in vars[k + 1..].iter().cloned() {
                            set(self, &iv, ifrom);
                        }
                        self.emit(Instr::Jump(body_l));
                        self.bind(next);
                    }
                }
            }
        }
        self.bind(end);
        if inline {
            self.inline_performs.pop();
        }
        Ok(())
    }

    fn sort_stmt(&mut self, s: &Stmt) -> Result<()> {
        match s {
            Stmt::Sort {
                file,
                keys,
                duplicates,
                collating,
                input,
                output,
            } => self.sort_merge(
                file,
                keys,
                *duplicates,
                collating.as_deref(),
                Some(input),
                output,
                &[],
            ),
            Stmt::Merge {
                file,
                keys,
                collating,
                using,
                output,
            } => self.sort_merge(file, keys, false, collating.as_deref(), None, output, using),
            Stmt::Release { record, from } => {
                let fi = self.file_of_record(record)?;
                if !self.files[fi as usize].sort
                    || self.dm.items[self.dm.resolve(record)?].parent.is_some()
                {
                    return Err(Diag::error(
                        self.pos,
                        "RELEASE requires an SD logical record",
                    ));
                }
                let from = match from {
                    Some(f) => Some(self.src(f)?),
                    None => None,
                };
                self.emit(Instr::Release(fi, from));
                Ok(())
            }
            Stmt::Return { file, into, at_end } => {
                let fi = self.file(file)?;
                if !self.files[fi as usize].sort {
                    return Err(Diag::error(self.pos, "RETURN requires an SD file"));
                }
                let into = match into {
                    Some(i) => Some(self.loc(i)?),
                    None => None,
                };
                self.emit(Instr::Return(fi, into));
                self.handlers(at_end, Cx::IoCond)
            }
            _ => unreachable!(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn sort_merge(
        &mut self,
        file: &str,
        keys: &[(bool, Vec<DataRef>)],
        duplicates: bool,
        collating: Option<&str>,
        input: Option<&SortIo>,
        output: &SortIo,
        merge_using: &[String],
    ) -> Result<()> {
        let fi = self.file(file)?;
        if crate::front::thread() {
            return Err(Diag::error(
                self.pos,
                "file SORT/MERGE is not supported with THREAD",
            ));
        }
        if input.is_none() && merge_using.len() < 2 {
            return Err(Diag::error(
                self.pos,
                "MERGE requires at least two USING files",
            ));
        }
        if !self.files[fi as usize].sort {
            return Err(Diag::error(self.pos, "SORT/MERGE requires an SD file"));
        }
        let mut ks = Vec::new();
        for (asc, refs) in keys {
            for r in refs {
                if self.file_of_record(r)? != fi {
                    return Err(Diag::error(
                        r.pos,
                        "SORT/MERGE key must belong to its SD file",
                    ));
                }
                let l = self.loc(r)?;
                let desc = &self.dm.descs[l.desc as usize];
                if l.refmod.is_some()
                    || !l.subs.is_empty()
                    || !desc.dims.is_empty()
                    || desc.dynamic_offset.is_some()
                    || desc.dynamic_len.is_some()
                    || desc.odo.is_some()
                    || matches!(
                        desc.ty,
                        DataType::Dbcs
                            | DataType::Index
                            | DataType::Pointer
                            | DataType::ProcedurePointer
                            | DataType::FunctionPointer
                            | DataType::ExternalFloat { national: true, .. }
                    )
                    || matches!(&desc.ty,DataType::Numeric(t) if t.usage == zir::NumUsage::National)
                {
                    return Err(Diag::error(
                        r.pos,
                        "invalid SORT/MERGE key category or variable/table layout",
                    ));
                }
                ks.push(zir::SortKey {
                    ascending: *asc,
                    desc: l.desc,
                });
            }
        }
        let mut using = Vec::new();
        let mut in_proc = None;
        match input {
            Some(SortIo::Using(files)) => {
                for f in files {
                    using.push(self.file(f)?);
                }
            }
            Some(SortIo::Procedure(a, b)) => {
                let (first, la) = self.proc_range(a)?;
                let last = match b {
                    Some(b) => self.proc_range(b)?.1,
                    None => la,
                };
                in_proc = Some((self.entry_label(a, first), last));
            }
            _ => {
                for f in merge_using {
                    using.push(self.file(f)?);
                }
            }
        }
        let mut giving = Vec::new();
        let mut out_proc = None;
        match output {
            SortIo::Giving(files) => {
                for f in files {
                    giving.push(self.file(f)?);
                }
            }
            SortIo::Procedure(a, b) => {
                let (first, la) = self.proc_range(a)?;
                let last = match b {
                    Some(b) => self.proc_range(b)?.1,
                    None => la,
                };
                out_proc = Some((self.entry_label(a, first), last));
            }
            SortIo::Using(_) => {}
        }
        let collating = match collating {
            Some(c) => Some(self.alphabet_weights(c)?),
            None => self.collating()?,
        };
        let participants: Vec<_> = std::iter::once(fi)
            .chain(using.iter().copied())
            .chain(giving.iter().copied())
            .collect();
        for (n, &f) in participants.iter().enumerate() {
            for &g in &participants[n + 1..] {
                if self.files[f as usize]
                    .same_sort_area
                    .contains(&self.files[g as usize].name)
                {
                    return Err(Diag::error(
                        self.pos,
                        "SORT/MERGE files cannot share SAME SORT AREA",
                    ));
                }
            }
        }
        let _ = duplicates;
        self.emit(Instr::SortBegin(fi));
        for file in using {
            self.emit(Instr::SortRead(file));
        }
        if let Some((start, end)) = in_proc {
            let ret = self.label();
            self.emit(Instr::DebugCause(zir::debug_cause::SORT_INPUT, 0));
            let in_decl = self.in_decl;
            self.in_decl = true;
            self.emit(Instr::Perform { start, end, ret });
            self.in_decl = in_decl;
            self.bind(ret);
        }
        self.emit(Instr::SortCheckpoint);
        self.emit(Instr::SortOrder {
            file: fi,
            keys: ks,
            collating,
            merge: input.is_none(),
        });
        self.emit(Instr::SortCheckpoint);
        if let Some((start, end)) = out_proc {
            let ret = self.label();
            self.emit(Instr::DebugCause(
                if input.is_none() {
                    zir::debug_cause::MERGE_OUTPUT
                } else {
                    zir::debug_cause::SORT_OUTPUT
                },
                0,
            ));
            let in_decl = self.in_decl;
            self.in_decl = true;
            self.emit(Instr::Perform { start, end, ret });
            self.in_decl = in_decl;
            self.bind(ret);
        } else {
            for file in giving {
                self.emit(Instr::SortWrite(file));
            }
        }
        self.emit(Instr::SortEnd);
        Ok(())
    }
}

fn spec_bytes(c: &CharSpec) -> Vec<u8> {
    match c {
        CharSpec::Text(t) => ebcdic::encode(t),
        CharSpec::Ordinal(n) => vec![native_ordinal_byte(*n)],
    }
}

fn handlers_of(s: &Stmt) -> Vec<&Vec<Stmt>> {
    let hs: Vec<&Handlers> = match s {
        Stmt::Add { size_error, .. }
        | Stmt::Subtract { size_error, .. }
        | Stmt::Multiply { size_error, .. }
        | Stmt::Divide { size_error, .. }
        | Stmt::Compute { size_error, .. } => vec![size_error],
        Stmt::Read { at_end, .. } => vec![at_end],
        Stmt::Write { end_of_page, .. } => vec![end_of_page],
        Stmt::Call { exception, .. } => vec![exception],
        _ => vec![],
    };
    hs.into_iter()
        .flat_map(|h| h.on.iter().chain(h.not_on.iter()))
        .collect()
}

/// Compiles all programs of a source file into a module.
pub fn compile(programs: &[ast::Program]) -> Result<zir::Module> {
    let compiled_at = crate::vm::timestamp();
    let mut out = Vec::new();
    for p in programs {
        compile_nested(p, None, &[], &mut out)?;
    }
    // IBM uses the first literal CALL convention for a name throughout
    // a compilation unit, diagnosing later conflicting conventions.
    let mut diagnostics = Vec::new();
    let mut conventions = HashMap::new();
    for program in &mut out {
        if program.parent.is_none() {
            conventions.clear();
        }
        for (pc, instruction) in program.code.iter_mut().enumerate() {
            for (loc, receiving) in instr_locs(instruction) {
                let name = &program.descs[loc.desc as usize].name;
                if receiving
                    && matches!(
                        name.as_str(),
                        "SORT-CONTROL"
                            | "SORT-CORE-SIZE"
                            | "SORT-FILE-SIZE"
                            | "SORT-MESSAGE"
                            | "SORT-MODE-SIZE"
                    )
                {
                    let (file, line) = program
                        .instr_stmt
                        .get(pc)
                        .and_then(|s| program.stmt_pos.get(*s as usize))
                        .copied()
                        .unwrap_or_default();
                    diagnostics.push(Diag::warning(
                        Pos { file, line },
                        format!("setting {name}; SORT control statements take precedence"),
                    ));
                }
            }
            if let Instr::Call {
                program: Src::Alnum(bytes),
                interface,
                ..
            } = instruction
            {
                let name = crate::codepage::active()
                    .decode(bytes)
                    .trim()
                    .to_ascii_uppercase();
                let dynamic = interface
                    .unwrap_or(zir::CallInterface::default_for(program.dynam, program.dll));
                let first = *conventions.entry(name.clone()).or_insert(dynamic);
                if first != dynamic {
                    let (file, line) = program
                        .instr_stmt
                        .get(pc)
                        .and_then(|s| program.stmt_pos.get(*s as usize))
                        .copied()
                        .unwrap_or_default();
                    diagnostics.push(Diag::warning(
                        Pos { file, line },
                        format!(
                            "conflicting CALLINTERFACE for {name}; using the first {} convention",
                            first.name()
                        ),
                    ));
                    *interface = Some(first);
                }
            }
        }
    }
    let mut exports = std::collections::HashSet::new();
    let mut externals = HashMap::new();
    for program in &out {
        for rule in &program.rerun {
            if out.iter().any(|other| {
                other
                    .files
                    .iter()
                    .any(|file| zeus_ds::dd::same_assignment(&file.assign, &rule.checkpoint))
            }) {
                return Err(Diag::error(
                    Pos::default(),
                    "RERUN checkpoint assignment must be distinct throughout the compilation unit",
                ));
            }
        }
        for area in &program.externals {
            if externals
                .insert(area.name.clone(), area.len)
                .is_some_and(|len| len != area.len)
            {
                return Err(Diag::error(
                    Pos::default(),
                    format!("EXTERNAL {} storage lengths must agree", area.name),
                ));
            }
        }
    }
    for program in out.iter().filter(|p| p.parent.is_none()) {
        for name in std::iter::once(&program.name).chain(program.entries.iter().map(|e| &e.name)) {
            if !exports.insert(name.clone()) {
                return Err(Diag::error(
                    Pos::default(),
                    format!("duplicate program/ENTRY export {name}"),
                ));
            }
        }
    }
    Ok(zir::Module {
        diagnostics,
        optimize: crate::front::optimize(),
        programs: out,
        ccsid: crate::codepage::active().ccsid,
        sources: Vec::new(),
        compiled_at: Some(compiled_at),
    })
}

fn compile_nested<'a>(
    p: &'a ast::Program,
    outer: Option<(u32, &DataModel)>,
    ancestors: &[(u32, &'a ast::Program)],
    out: &mut Vec<zir::ProgramCode>,
) -> Result<()> {
    let idx = out.len() as u32;
    let g = Gen::new(p, outer, ancestors)?;
    let dm = (!p.nested.is_empty()).then(|| g.dm.clone());
    let code = g.generate(outer.map(|(i, _)| i))?;
    out.push(code);
    if let Some(dm) = dm {
        let mut anc = vec![(idx, p)];
        anc.extend_from_slice(ancestors);
        for n in &p.nested {
            compile_nested(n, Some((idx, &dm)), &anc, out)?;
        }
    }
    Ok(())
}

/// Data references of an instruction: (location, receiving operand).
fn instr_locs(ins: &Instr) -> Vec<(Loc, bool)> {
    fn walk(v: &serde_json::Value, receiving: bool, out: &mut Vec<(Loc, bool)>) {
        match v {
            serde_json::Value::Object(m) => {
                if m.contains_key("desc") && m.contains_key("subs") && m.contains_key("refmod") {
                    if let Ok(l) = serde_json::from_value::<Loc>(v.clone()) {
                        out.push((l, receiving));
                    }
                    for k in ["subs", "refmod"] {
                        if let Some(x) = m.get(k) {
                            walk(x, false, out);
                        }
                    }
                    return;
                }
                for (k, x) in m {
                    let r = receiving
                        || matches!(
                            k.as_str(),
                            "dst" | "quot" | "rem" | "into" | "pointer" | "tallying" | "index"
                        );
                    walk(x, r, out);
                }
            }
            serde_json::Value::Array(a) => {
                for x in a {
                    walk(x, receiving, out);
                }
            }
            _ => {}
        }
    }
    let mut out = Vec::new();
    let Ok(v) = serde_json::to_value(ins) else {
        return out;
    };
    // positional receiving operands of tuple variants
    let receiving_at = match ins {
        Instr::Move(..) | Instr::Return(..) => Some(1),
        Instr::SetIndex(..)
        | Instr::Inspect(..)
        | Instr::SetEntry(..)
        | Instr::SetEntryWithInterface(..) => Some(0),
        _ => None,
    };
    match (&v, receiving_at) {
        (serde_json::Value::Object(m), Some(at)) => {
            if let Some(serde_json::Value::Array(a)) = m.values().next() {
                for (i, x) in a.iter().enumerate() {
                    walk(x, i == at, &mut out);
                }
            }
        }
        _ => walk(&v, false, &mut out),
    }
    let record = |d: u32| Loc {
        desc: d,
        subs: Vec::new(),
        refmod: None,
    };
    match ins {
        // the record written counts as a receiving item
        Instr::Write { record: d, .. } | Instr::Rewrite { record: d, .. } => {
            out.push((record(*d), true));
        }
        _ => {}
    }
    out
}

fn line_pos(l: LineClause) -> zir::LinePos {
    match l {
        LineClause::Absolute(n) => zir::LinePos::Absolute(n),
        LineClause::Plus(n) => zir::LinePos::Plus(n),
        LineClause::NextPage => zir::LinePos::NextPage,
        LineClause::AbsoluteNextPage(n) => zir::LinePos::AbsoluteNextPage(n),
    }
}

/// The SQL cursor declarations of a program: in the data division and in
/// the sentences of the procedure division.
fn sql_cursors(prog: &ast::Program) -> HashMap<String, SqlText> {
    let mut out = HashMap::new();
    let mut add = |e: &ExecBlock| {
        if !matches!(e.kind, ExecKind::Sql | ExecKind::SqlIms) || e.command != "DECLARE" {
            return;
        }
        let Some(sql) = &e.sql else { return };
        // DECLARE c [ASENSITIVE|INSENSITIVE|SENSITIVE ...] [NO] [SCROLL] CURSOR
        let words: Vec<&str> = sql.text.split_whitespace().collect();
        let attrs = [
            "ASENSITIVE",
            "INSENSITIVE",
            "SENSITIVE",
            "STATIC",
            "DYNAMIC",
            "NO",
            "SCROLL",
        ];
        let mut i = 2;
        while words
            .get(i)
            .is_some_and(|w| attrs.iter().any(|a| w.eq_ignore_ascii_case(a)))
        {
            i += 1;
        }
        if words
            .get(i)
            .is_some_and(|w| w.eq_ignore_ascii_case("CURSOR"))
        {
            out.insert(
                format!("{:?}:{}", e.kind, words[1].to_ascii_uppercase()),
                sql.clone(),
            );
        }
    };
    for e in &prog.data.execs {
        add(e);
    }
    for sec in &prog.procedure.sections {
        for p in &sec.paragraphs {
            for sentence in &p.sentences {
                for st in sentence {
                    if let Stmt::Exec(e) = st {
                        add(e);
                    }
                }
            }
        }
    }
    out
}

/// The query of `DECLARE c CURSOR [WITH HOLD] FOR query`.
fn cursor_query(decl: &str) -> Option<String> {
    let words: Vec<&str> = decl.split_whitespace().collect();
    let at = words.iter().position(|w| w.eq_ignore_ascii_case("FOR"))?;
    Some(words[at + 1..].join(" "))
}

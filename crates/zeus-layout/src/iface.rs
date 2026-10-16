//! The data a program exchanges: files (FD records, READ INTO, WRITE /
//! REWRITE FROM), cards (SYSIN files, ACCEPT), its parameters, the
//! arguments of the programs it calls and MQ message buffers.

use zeus_cobol::front::ast::{
    CallArg, ExecArg, ExecBlock, ExecKind, Expr, Handlers, Literal, OpenMode, Operand,
    Organization, PerformBody, Program, Stmt, ValueClause,
};
use zeus_cobol::sema::data::{Area, DataModel};

/// An item of a row and its indicator variable.
pub type RowItem = (usize, Option<usize>);

/// A paragraph: its section, its name and its sentences.
pub type Para<'a> = (Option<String>, Option<String>, &'a [Vec<Stmt>]);

/// One exchanged datum and the records describing it.
#[derive(Debug, Clone)]
pub struct Iface {
    /// FILE, CARD, MQ, PARM or CALL.
    pub kind: &'static str,
    /// DD name, MQ buffer, parameter number, program and argument number.
    pub resource: String,
    /// The header's KIND line.
    pub detail: String,
    /// Record items (several: one layout set each).
    pub roots: Vec<usize>,
    /// RECORD CONTAINS / VARYING of a file: (min, max).
    pub record: Option<(u32, u32)>,
    /// Split REDEFINES into layouts (not for BMS symbolic maps, whose
    /// fields all redefine their length and attribute bytes).
    pub split: bool,
    /// A record made of separate items (a query's host variables and
    /// their indicators): its name and items.
    pub row: Option<(String, Vec<RowItem>)>,
}

/// Visits statements, their nested statements included.
pub fn walk<'a>(stmts: &'a [Stmt], f: &mut dyn FnMut(&'a Stmt)) {
    fn handlers<'a>(h: &'a Handlers, f: &mut dyn FnMut(&'a Stmt)) {
        if let Some(s) = &h.on {
            walk(s, f);
        }
        if let Some(s) = &h.not_on {
            walk(s, f);
        }
    }
    for s in stmts {
        f(s);
        match s {
            Stmt::If { then, els, .. } => {
                walk(then, f);
                if let Some(e) = els {
                    walk(e, f);
                }
            }
            Stmt::Evaluate { whens, other, .. } => {
                for (_, b) in whens {
                    walk(b, f);
                }
                if let Some(o) = other {
                    walk(o, f);
                }
            }
            Stmt::Perform {
                body: PerformBody::Inline(b),
                ..
            } => walk(b, f),
            Stmt::Search { at_end, whens, .. } => {
                if let Some(a) = at_end {
                    walk(a, f);
                }
                for (_, b) in whens {
                    walk(b, f);
                }
            }
            Stmt::Read {
                at_end,
                invalid_key,
                ..
            } => {
                handlers(at_end, f);
                handlers(invalid_key, f);
            }
            Stmt::Write {
                end_of_page,
                invalid_key,
                ..
            } => {
                handlers(end_of_page, f);
                handlers(invalid_key, f);
            }
            Stmt::Rewrite { invalid_key, .. } => handlers(invalid_key, f),
            Stmt::Call { exception, .. } => handlers(exception, f),
            _ => {}
        }
    }
}

/// All statements of a program's procedure division, by paragraph.
pub fn paragraphs(p: &Program) -> Vec<Para<'_>> {
    let mut out = Vec::new();
    for s in p.procedure.declaratives.iter().chain(&p.procedure.sections) {
        for para in &s.paragraphs {
            out.push((s.name.clone(), para.name.clone(), para.sentences.as_slice()));
        }
    }
    out
}

/// Visits every statement of a program.
pub fn each_stmt<'a>(p: &'a Program, f: &mut dyn FnMut(&'a Stmt)) {
    for (_, _, sentences) in paragraphs(p) {
        for s in sentences {
            walk(s, f);
        }
    }
}

/// The DD name of an ASSIGN target (`UT-S-NAME` -> NAME).
pub fn dd_name(assign: &str) -> String {
    let a = assign.trim_matches(['\'', '"']).to_ascii_uppercase();
    let parts: Vec<&str> = a.split('-').collect();
    if parts.len() > 1 && matches!(parts[0], "UT" | "S" | "DA" | "AS" | "UR") {
        parts.last().unwrap().to_string()
    } else {
        a
    }
}

fn is_card_dd(dd: &str) -> bool {
    matches!(dd, "SYSIN" | "SYSIPT")
}

struct Ifaces(Vec<Iface>);

impl Ifaces {
    fn add(&mut self, kind: &'static str, resource: &str, detail: String, root: usize) {
        if let Some(i) = self
            .0
            .iter_mut()
            .find(|i| i.kind == kind && i.resource == resource)
        {
            if !i.roots.contains(&root) {
                i.roots.push(root);
            }
            return;
        }
        self.0.push(Iface {
            kind,
            resource: resource.to_string(),
            detail,
            roots: vec![root],
            record: None,
            split: kind != "BMS",
            row: None,
        });
    }

    fn row(
        &mut self,
        resource: &str,
        detail: String,
        name: String,
        items: Vec<(usize, Option<usize>)>,
    ) {
        if items.is_empty()
            || self
                .0
                .iter()
                .any(|i| i.kind == "SQLRESULT" && i.resource == resource)
        {
            return;
        }
        self.0.push(Iface {
            kind: "SQLRESULT",
            resource: resource.to_string(),
            detail,
            roots: Vec::new(),
            record: None,
            split: true,
            row: Some((name, items)),
        });
    }
}

/// The argument of an EXEC option.
fn opt<'a>(b: &'a ExecBlock, name: &str) -> Option<&'a ExecArg> {
    b.options
        .iter()
        .find(|o| o.name.eq_ignore_ascii_case(name))
        .and_then(|o| o.arg.as_ref())
}

/// The name an EXEC option gives: a literal, the VALUE of the data item
/// naming it, else the data name.
fn opt_name(dm: &DataModel, b: &ExecBlock, name: &str) -> Option<String> {
    match opt(b, name)? {
        ExecArg::Expr(Expr::Operand(Operand::Ref(r))) => {
            let value = dm
                .resolve(r)
                .ok()
                .and_then(|id| match &dm.items[id].entry.value {
                    Some(ValueClause::Literal(Operand::Lit(Literal::Str(v))))
                        if !v.trim().is_empty() =>
                    {
                        Some(v.trim().to_ascii_uppercase())
                    }
                    _ => None,
                });
            Some(value.unwrap_or_else(|| r.name.clone()))
        }
        ExecArg::Expr(Expr::Operand(o)) => literal_name(o),
        ExecArg::Raw(r) => Some(r.trim().trim_matches(['\'', '"']).to_ascii_uppercase()),
        _ => None,
    }
}

fn opt_item(dm: &DataModel, b: &ExecBlock, name: &str) -> Option<usize> {
    match opt(b, name)? {
        ExecArg::Expr(Expr::Operand(Operand::Ref(r))) => dm.resolve(r).ok(),
        _ => None,
    }
}

/// The host variables after INTO in a SELECT or FETCH statement.
fn into_vars(b: &ExecBlock) -> Vec<usize> {
    let Some(sql) = &b.sql else { return Vec::new() };
    let u = sql.text.to_ascii_uppercase();
    let words: Vec<&str> = u.split_whitespace().collect();
    // the position of each word's start in the text, to count markers
    let Some(into) = find_word(&u, "INTO") else {
        return Vec::new();
    };
    let end = if words.first() == Some(&"FETCH") {
        u.len()
    } else {
        find_word(&u[into..], "FROM").map_or(u.len(), |e| into + e)
    };
    let before = u[..into].matches('?').count();
    let n = u[into..end].matches('?').count();
    (before..before + n).collect()
}

fn find_word(u: &str, w: &str) -> Option<usize> {
    let b = u.as_bytes();
    let mut from = 0;
    while let Some(i) = u[from..].find(w) {
        let at = from + i;
        let ok_before = at == 0 || !(b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'-');
        let after = at + w.len();
        let ok_after = after >= b.len() || !(b[after].is_ascii_alphanumeric() || b[after] == b'-');
        if ok_before && ok_after {
            return Some(at);
        }
        from = after;
    }
    None
}

/// The records sharing the area of 01 record `id` (01 REDEFINES).
fn record_group(dm: &DataModel, id: usize) -> Vec<usize> {
    let it = &dm.items[id];
    if it.parent.is_some() || matches!(it.area, Area::File(_)) {
        return vec![id];
    }
    let same = |a: usize| dm.items[a].parent.is_none() && dm.items[a].area == it.area;
    let base = it
        .entry
        .redefines
        .as_ref()
        .and_then(|n| dm.by_name.get(n))
        .and_then(|v| v.iter().copied().find(|&b| same(b)))
        .unwrap_or(id);
    let name = dm.items[base].name.clone();
    let mut out = vec![base];
    for (i, x) in dm.items.iter().enumerate() {
        if i != base && same(i) && x.entry.redefines.is_some() && x.entry.redefines == name {
            out.push(i);
        }
    }
    out
}

fn literal_name(o: &Operand) -> Option<String> {
    match o {
        Operand::Lit(Literal::Str(s)) => Some(s.trim().to_ascii_uppercase()),
        Operand::Ref(r) => Some(r.name.clone()),
        _ => None,
    }
}

/// The data a program exchanges.
pub fn find(p: &Program, dm: &DataModel) -> Vec<Iface> {
    let mut out = Ifaces(Vec::new());
    let res = |o: &Operand| match o {
        Operand::Ref(r) => dm.resolve(r).ok(),
        _ => None,
    };
    // open modes of each file
    let mut modes: Vec<(String, &'static str)> = Vec::new();
    each_stmt(p, &mut |s| {
        if let Stmt::Open(list) = s {
            for (m, files) in list {
                let m = match m {
                    OpenMode::Input => "INPUT",
                    OpenMode::Output => "OUTPUT",
                    OpenMode::InputOutput => "I-O",
                    OpenMode::Extend => "EXTEND",
                };
                for f in files {
                    if !modes.iter().any(|(n, x)| n == &f.name && *x == m) {
                        modes.push((f.name.clone(), m));
                    }
                }
            }
        }
    });
    // files and their records
    let mut file_res: Vec<(String, &'static str, String)> = Vec::new();
    for fc in p.env.files.iter().filter(|f| !f.sort) {
        let dd = dd_name(&fc.assign);
        let kind = if is_card_dd(&dd) { "CARD" } else { "FILE" };
        let m: Vec<&str> = modes
            .iter()
            .filter(|(n, _)| *n == fc.name)
            .map(|(_, m)| *m)
            .collect();
        let org = match fc.organization {
            Organization::Sequential => "SEQUENTIAL",
            Organization::LineSequential => "LINE SEQUENTIAL",
            Organization::Relative => "RELATIVE",
            Organization::Indexed => "INDEXED",
        };
        let detail = format!(
            "{kind}{}  DD: {dd}  FILE: {}  ORGANIZATION: {org}",
            if m.is_empty() {
                String::new()
            } else {
                format!(" ({})", m.join(", "))
            },
            fc.name
        );
        if let Some((_, recs)) = dm.files.get(&fc.name) {
            for &r in recs {
                out.add(kind, &dd, detail.clone(), r);
            }
        }
        if let Some(fd) = p.data.files.iter().find(|f| f.name == fc.name)
            && let Some(i) = out
                .0
                .iter_mut()
                .find(|i| i.kind == kind && i.resource == dd)
            && (fd.record_min.is_some() || fd.record_max.is_some())
        {
            let max = fd.record_max.or(fd.record_min).unwrap_or(0);
            i.record = Some((fd.record_min.unwrap_or(max), max));
        }
        file_res.push((fc.name.clone(), kind, dd));
    }
    let file_of_record = |id: usize| -> Option<String> {
        let rec = dm.record_of(id);
        dm.files
            .iter()
            .find(|(_, (_, recs))| recs.contains(&rec))
            .map(|(n, _)| n.clone())
    };
    let add_file_area = |out: &mut Ifaces, file: &str, area: usize| {
        if let Some((_, kind, dd)) = file_res.iter().find(|(n, _, _)| n == file) {
            let detail = out
                .0
                .iter()
                .find(|i| i.kind == *kind && i.resource == *dd)
                .map(|i| i.detail.clone())
                .unwrap_or_default();
            for r in record_group(dm, area) {
                out.add(kind, dd, detail.clone(), r);
            }
        }
    };
    let mut commareas: Vec<(String, String, usize)> = Vec::new();
    let mut execs: Vec<&ExecBlock> = Vec::new();
    let mut accepts: Vec<(String, usize)> = Vec::new();
    let mut calls: Vec<(String, Vec<Option<usize>>)> = Vec::new();
    each_stmt(p, &mut |s| match s {
        Stmt::Read {
            file,
            into: Some(r),
            ..
        } => {
            if let Ok(id) = dm.resolve(r) {
                add_file_area(&mut out, file, id);
            }
        }
        Stmt::Write {
            record,
            from: Some(from),
            ..
        }
        | Stmt::Rewrite {
            record,
            from: Some(from),
            ..
        } => {
            if let (Ok(rec), Some(id)) = (dm.resolve(record), res(from))
                && let Some(f) = file_of_record(rec)
            {
                add_file_area(&mut out, &f, id);
            }
        }
        Stmt::Accept { target, from, .. } => {
            let from = from.clone().unwrap_or_else(|| "SYSIN".into());
            let device = p
                .env
                .mnemonics
                .iter()
                .find(|(_, m)| *m == from)
                .map(|(i, _)| i.clone())
                .unwrap_or(from);
            let u = device.to_ascii_uppercase();
            if !(u.starts_with("DATE") || u.starts_with("DAY") || u.starts_with("TIME"))
                && let Ok(id) = dm.resolve(target)
            {
                accepts.push((u, id));
            }
        }
        Stmt::Move {
            from: Operand::Ref(r),
            to,
            ..
        } if r.name == "DFHCOMMAREA" => {
            for t in to {
                if let Ok(id) = dm.resolve(t) {
                    commareas.push((
                        "IN".into(),
                        "COMMAREA  DFHCOMMAREA (THE COMMAREA PASSED TO THE PROGRAM)".into(),
                        id,
                    ));
                }
            }
        }
        Stmt::Move { from, to, .. } if to.iter().any(|t| t.name == "DFHCOMMAREA") => {
            if let Some(id) = res(from) {
                commareas.push((
                    "IN".into(),
                    "COMMAREA  DFHCOMMAREA (THE COMMAREA PASSED TO THE PROGRAM)".into(),
                    id,
                ));
            }
        }
        Stmt::Exec(b) => execs.push(b),
        Stmt::Call { program, using, .. } => {
            let value = match program {
                Operand::Ref(r) => {
                    dm.resolve(r)
                        .ok()
                        .and_then(|id| match &dm.items[id].entry.value {
                            Some(ValueClause::Literal(Operand::Lit(Literal::Str(v))))
                                if !v.trim().is_empty() =>
                            {
                                Some(v.trim().to_ascii_uppercase())
                            }
                            _ => None,
                        })
                }
                _ => None,
            };
            if let Some(name) = value.or_else(|| literal_name(program)) {
                let args = using
                    .iter()
                    .map(|a| match a {
                        CallArg::Reference(o) | CallArg::Content(o) => res(o),
                        _ => None,
                    })
                    .collect();
                calls.push((name, args));
            }
        }
        _ => {}
    });
    // the COMMAREA passed to the program
    if let Some(u) = p.procedure.using.iter().find(|u| u.name == "DFHCOMMAREA")
        && let Ok(id) = dm.resolve(u)
        && dm.items[id].len > 1
    {
        commareas.insert(
            0,
            (
                "IN".into(),
                "COMMAREA  DFHCOMMAREA (THE COMMAREA PASSED TO THE PROGRAM)".into(),
                id,
            ),
        );
    }
    // the paragraph of each SELECT INTO (its name names the result)
    let mut select_para: Vec<((u32, u32), String)> = Vec::new();
    for (sec, para, sentences) in paragraphs(p) {
        let name = para.or(sec).unwrap_or_else(|| "PROCEDURE".into());
        for st in sentences {
            walk(st, &mut |x| {
                if let Stmt::Exec(b) = x
                    && b.kind == ExecKind::Sql
                    && b.command.eq_ignore_ascii_case("SELECT")
                {
                    select_para.push(((b.pos.file, b.pos.line), name.clone()));
                }
            });
        }
    }
    let mut select_names: Vec<String> = Vec::new();
    for b in &execs {
        let cmd = b.command.to_ascii_uppercase();
        match b.kind {
            ExecKind::Cics => match cmd.as_str() {
                "READ" | "READNEXT" | "READPREV" | "WRITE" | "REWRITE" => {
                    let file = opt_name(dm, b, "FILE").or_else(|| opt_name(dm, b, "DATASET"));
                    let area = opt_item(dm, b, "INTO").or_else(|| opt_item(dm, b, "FROM"));
                    if let (Some(f), Some(a)) = (file, area) {
                        out.add("FILE", &f, format!("FILE (CICS)  FILE: {f}"), a);
                    }
                }
                "LINK" | "XCTL" | "RETURN" => {
                    if let Some(a) = opt_item(dm, b, "COMMAREA") {
                        let target = if cmd == "RETURN" {
                            "RETURN".to_string()
                        } else {
                            opt_name(dm, b, "PROGRAM").unwrap_or_else(|| "PROGRAM".into())
                        };
                        let detail = if cmd == "RETURN" {
                            "COMMAREA  RETURN COMMAREA (TO THE NEXT TRANSACTION)".to_string()
                        } else {
                            format!("COMMAREA  {cmd} PROGRAM({target})")
                        };
                        commareas.push((target, detail, a));
                    }
                }
                "SEND" | "RECEIVE" => {
                    let Some(map) = opt_name(dm, b, "MAP") else {
                        continue;
                    };
                    let mapset = opt_name(dm, b, "MAPSET").unwrap_or_else(|| map.clone());
                    let (area_opt, suffix) = if cmd == "SEND" {
                        ("FROM", "O")
                    } else {
                        ("INTO", "I")
                    };
                    let area = opt_item(dm, b, area_opt).or_else(|| {
                        dm.by_name
                            .get(&format!("{map}{suffix}"))
                            .and_then(|v| v.iter().copied().find(|&i| dm.items[i].parent.is_none()))
                    });
                    if let Some(a) = area {
                        let name = dm.items[a].name.clone().unwrap_or_default();
                        out.add(
                            "BMS",
                            &format!("{mapset}.{name}"),
                            format!("BMS  {cmd} MAP({map}) MAPSET({mapset})"),
                            a,
                        );
                    }
                }
                _ => {}
            },
            ExecKind::Sql if matches!(cmd.as_str(), "SELECT" | "FETCH") => {
                let Some(sql) = &b.sql else { continue };
                let idx = into_vars(b);
                let items: Vec<RowItem> = idx
                    .iter()
                    .filter_map(|&k| sql.host.get(k))
                    .filter_map(|h| {
                        let v = dm.resolve(&h.var).ok()?;
                        let ind = h.indicator.as_ref().and_then(|i| dm.resolve(i).ok());
                        Some((v, ind))
                    })
                    .collect();
                let words: Vec<String> = sql
                    .text
                    .split_whitespace()
                    .map(|w| w.to_ascii_uppercase())
                    .collect();
                let (resource, name) = if cmd == "FETCH" {
                    // FETCH [NEXT ...] [FROM] cursor INTO
                    let cur = words
                        .iter()
                        .take_while(|w| *w != "INTO")
                        .last()
                        .cloned()
                        .unwrap_or_else(|| "CURSOR".into());
                    (cur.clone(), format!("{cur}-ROW"))
                } else {
                    // SELECT.paragraph, numbered after the first in a paragraph
                    let para = select_para
                        .iter()
                        .find(|(at, _)| *at == (b.pos.file, b.pos.line))
                        .map(|(_, n)| n.clone())
                        .unwrap_or_else(|| format!("LINE-{}", b.pos.line));
                    let mut r = format!("SELECT.{para}");
                    let mut k = 1;
                    while select_names.contains(&r) {
                        k += 1;
                        r = format!("SELECT.{para}.{k}");
                    }
                    select_names.push(r.clone());
                    (r, "SELECT-ROW".to_string())
                };
                // a single structure: the structure itself
                if let [(v, None)] = items[..]
                    && !dm.items[v].children.is_empty()
                {
                    out.add(
                        "SQLRESULT",
                        &resource,
                        format!("SQLRESULT  {cmd} INTO (LINE {})", b.pos.line),
                        v,
                    );
                } else {
                    out.row(
                        &resource,
                        format!("SQLRESULT  {cmd} INTO (LINE {})", b.pos.line),
                        name,
                        items,
                    );
                }
            }
            _ => {}
        }
    }
    for (target, detail, id) in commareas {
        for r in record_group(dm, id) {
            out.add("COMMAREA", &target, detail.clone(), r);
        }
    }
    // tables: the host structure following each DECLARE TABLE
    for b in p.data.execs.iter().filter(|b| b.kind == ExecKind::Sql) {
        let Some(sql) = &b.sql else { continue };
        let words: Vec<String> = sql
            .text
            .split_whitespace()
            .map(|w| w.to_ascii_uppercase())
            .collect();
        if words.first().map(String::as_str) != Some("DECLARE")
            || words.get(2).map(String::as_str) != Some("TABLE")
        {
            continue;
        }
        let table = words[1].clone();
        let host = dm
            .items
            .iter()
            .enumerate()
            .filter(|(_, it)| {
                it.parent.is_none()
                    && it.level == 1
                    && it.pos.file == b.pos.file
                    && it.pos.line > b.pos.line
                    && matches!(it.area, Area::Working | Area::Linkage(_))
            })
            .min_by_key(|(_, it)| it.pos.line)
            .map(|(i, _)| i);
        if let Some(h) = host {
            out.add(
                "TABLE",
                &table,
                format!("TABLE {table} (DCLGEN HOST STRUCTURE)"),
                h,
            );
        }
    }
    for (dev, id) in accepts {
        out.add("CARD", &dev, format!("CARD  ACCEPT FROM {dev}"), id);
    }
    let using: Vec<_> = p
        .procedure
        .using
        .iter()
        .filter(|u| !matches!(u.name.as_str(), "DFHEIBLK" | "DFHCOMMAREA"))
        .collect();
    for (i, u) in using.into_iter().enumerate() {
        if let Ok(id) = dm.resolve(u) {
            out.add(
                "PARM",
                &(i + 1).to_string(),
                format!("PARAMETER {} (PROCEDURE DIVISION USING {})", i + 1, u.name),
                id,
            );
        }
    }
    for (name, args) in calls {
        // the message buffer of MQGET / MQPUT / MQPUT1 is the sixth argument
        if matches!(name.as_str(), "MQGET" | "MQPUT" | "MQPUT1") {
            if let Some(Some(id)) = args.get(5) {
                let buf = dm.items[*id].name.clone().unwrap_or_default();
                out.add("MQ", &buf, format!("MQ  MESSAGE BUFFER ({name})"), *id);
            }
            continue;
        }
        if name.starts_with("MQ") {
            continue;
        }
        for (i, a) in args.iter().enumerate() {
            if let Some(id) = a {
                out.add(
                    "CALL",
                    &format!("{name}.{}", i + 1),
                    format!("CALL {name} ARGUMENT {}", i + 1),
                    *id,
                );
            }
        }
    }
    out.0
}

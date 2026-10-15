//! The record type selecting each layout, inferred from the procedure
//! division: a paragraph moving a literal to a field of the common part
//! and using the items of one alternative only (writers), and the
//! branches of EVALUATE / IF on such a field (readers).

use std::collections::{HashMap, HashSet};

use zeus_cobol::front::ast::{
    Cond, EvalObject, EvalSubject, Expr, Operand, PerformBody, Program, RelOp, Stmt, ValueClause,
};
use zeus_cobol::sema::data::DataModel;

use crate::iface::{each_stmt, paragraphs, walk};
use crate::render::operand;
use crate::split::Splitter;

/// Evidence that `field = value` selects an alternative.
#[derive(Debug, Clone, PartialEq)]
pub struct Evidence {
    pub field: String,
    pub value: String,
    /// Source file index and line.
    pub at: (u32, u32),
}

/// Names (data and procedure names) mentioned in statements.
fn names(s: &str, out: &mut HashSet<String>) {
    let mut rest = s;
    while let Some(i) = rest.find("name: \"") {
        rest = &rest[i + 7..];
        if let Some(e) = rest.find('"') {
            out.insert(rest[..e].to_string());
            rest = &rest[e..];
        }
    }
}

/// Paragraphs performed by statements.
fn performed(stmts: &[Stmt], out: &mut Vec<String>) {
    walk(stmts, &mut |s| {
        if let Stmt::Perform {
            body: PerformBody::Procedures(a, b),
            ..
        } = s
        {
            out.push(a.name.clone());
            if let Some(b) = b {
                out.push(b.name.clone());
            }
        }
    });
}

struct Procs<'a> {
    /// Paragraph or section name -> its statements.
    by_name: HashMap<String, Vec<&'a [Stmt]>>,
}

impl<'a> Procs<'a> {
    fn new(p: &'a Program) -> Self {
        let mut by_name: HashMap<String, Vec<&'a [Stmt]>> = HashMap::new();
        for (sec, para, sentences) in paragraphs(p) {
            for s in sentences {
                if let Some(n) = &para {
                    by_name.entry(n.clone()).or_default().push(s);
                }
                if let Some(n) = &sec {
                    by_name.entry(n.clone()).or_default().push(s);
                }
            }
        }
        Procs { by_name }
    }

    /// The names used by statements and the paragraphs they perform
    /// (two levels deep).
    fn region(&self, stmts: &[&[Stmt]]) -> HashSet<String> {
        let mut out = HashSet::new();
        let mut todo: Vec<(&[Stmt], u32)> = stmts.iter().map(|s| (*s, 0)).collect();
        let mut seen: HashSet<String> = HashSet::new();
        while let Some((s, depth)) = todo.pop() {
            names(&format!("{s:?}"), &mut out);
            if depth >= 2 {
                continue;
            }
            let mut p = Vec::new();
            performed(s, &mut p);
            for n in p {
                if seen.insert(n.clone())
                    && let Some(b) = self.by_name.get(&n)
                {
                    todo.extend(b.iter().map(|x| (*x, depth + 1)));
                }
            }
        }
        out
    }
}

fn literal(e: &Expr) -> Option<String> {
    match e {
        Expr::Operand(o @ (Operand::Lit(_) | Operand::Fig(_))) => Some(operand(o)),
        _ => None,
    }
}

/// Record types per alternative item, for the choice points of `root`.
pub fn infer(
    p: &Program,
    dm: &DataModel,
    sp: &Splitter,
    root: usize,
) -> HashMap<usize, Vec<Evidence>> {
    let procs = Procs::new(p);
    // the named items of each alternative, and the items inside one
    // choice point -> its alternatives and the names in each
    type Alts = Vec<(usize, Vec<(usize, HashSet<String>)>)>;
    let mut alts: Alts = Vec::new();
    let mut inside: HashSet<usize> = HashSet::new();
    fn subtree(dm: &DataModel, id: usize, out: &mut Vec<usize>) {
        out.push(id);
        for &c in &dm.items[id].children {
            subtree(dm, c, out);
        }
    }
    for b in sp.choice_points(root) {
        let group = sp
            .groups(dm.items[b].parent.unwrap_or(root))
            .into_iter()
            .find(|g| g[0] == b)
            .unwrap_or_default();
        let mut list = Vec::new();
        for a in sp.alternatives(&group) {
            let mut ids = Vec::new();
            subtree(dm, a, &mut ids);
            let set = ids
                .iter()
                .filter_map(|&i| dm.items[i].name.clone())
                .filter(|n| n != "FILLER")
                .collect();
            inside.extend(ids);
            list.push((a, set));
        }
        for &g in &group {
            let mut ids = Vec::new();
            subtree(dm, g, &mut ids);
            inside.extend(ids);
        }
        alts.push((b, list));
    }
    let mut out: HashMap<usize, Vec<Evidence>> = HashMap::new();
    if alts.is_empty() {
        return out;
    }
    let record = dm.record_of(root);
    // a field of the common part of the record
    let common = |o: &Operand| -> Option<usize> {
        let Operand::Ref(r) = o else { return None };
        let id = dm.resolve(r).ok()?;
        (dm.record_of(id) == record && !inside.contains(&id)).then_some(id)
    };
    let mut attribute = |region: &HashSet<String>, field: usize, value: String, at: (u32, u32)| {
        for (_, list) in &alts {
            let hit: Vec<usize> = list
                .iter()
                .filter(|(_, set)| !set.is_disjoint(region))
                .map(|(a, _)| *a)
                .collect();
            if let [a] = hit[..] {
                let e = Evidence {
                    field: dm.items[field].name.clone().unwrap_or_default(),
                    value: value.clone(),
                    at,
                };
                let v = out.entry(a).or_default();
                if !v.iter().any(|x| x.field == e.field && x.value == e.value) {
                    v.push(e);
                }
            }
        }
    };
    let cond_value = |r: &zeus_cobol::front::ast::DataRef| -> Option<(usize, String)> {
        let id = dm.resolve(r).ok()?;
        let var = dm.items[id].cond_of?;
        if dm.record_of(var) != record || inside.contains(&var) {
            return None;
        }
        let Some(ValueClause::Condition(vals)) = &dm.items[id].entry.value else {
            return None;
        };
        let v: Vec<String> = vals.iter().map(|(a, _)| operand(a)).collect();
        Some((var, v.join(" OR ")))
    };
    // writers: a paragraph moving a literal to a common field
    for (_, _, sentences) in paragraphs(p) {
        let mut moves = Vec::new();
        for s in sentences {
            walk(s, &mut |st| {
                if let Stmt::Move { from, to, pos, .. } = st
                    && matches!(from, Operand::Lit(_))
                {
                    for t in to {
                        if let Some(f) = common(&Operand::Ref(t.clone())) {
                            moves.push((f, operand(from), (pos.file, pos.line)));
                        }
                    }
                }
            });
        }
        if moves.is_empty() {
            continue;
        }
        let parts: Vec<&[Stmt]> = sentences.iter().map(|s| s.as_slice()).collect();
        let region = procs.region(&parts);
        for (f, v, at) in moves {
            attribute(&region, f, v, at);
        }
    }
    // readers: EVALUATE field WHEN literal, EVALUATE TRUE WHEN condition,
    // IF field = literal, IF condition
    each_stmt(p, &mut |s| match s {
        Stmt::Evaluate {
            subjects, whens, ..
        } => {
            let subject = match subjects.first() {
                Some(EvalSubject::Expr(Expr::Operand(o @ Operand::Ref(r)))) => {
                    common(o).map(|f| (Some(f), (r.pos.file, r.pos.line)))
                }
                Some(EvalSubject::True) => Some((None, (0, 0))),
                _ => None,
            };
            let Some((field, at)) = subject else { return };
            for (clauses, body) in whens {
                let region = procs.region(&[body.as_slice()]);
                for c in clauses {
                    match (field, c.objects.first()) {
                        (
                            Some(f),
                            Some(EvalObject::Value {
                                not: false,
                                from,
                                thru: None,
                            }),
                        ) => {
                            if let Some(v) = literal(from) {
                                attribute(&region, f, v, at);
                            }
                        }
                        (None, Some(EvalObject::Cond(Cond::Name(r)))) => {
                            if let Some((f, v)) = cond_value(r) {
                                attribute(&region, f, v, (r.pos.file, r.pos.line));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Stmt::If { cond, then, .. } => {
            let hit = match cond {
                Cond::Rel(Expr::Operand(o @ Operand::Ref(r)), RelOp::Eq, e) => common(o)
                    .zip(literal(e))
                    .map(|(f, v)| (f, v, (r.pos.file, r.pos.line))),
                Cond::Name(r) => cond_value(r).map(|(f, v)| (f, v, (r.pos.file, r.pos.line))),
                _ => None,
            };
            if let Some((f, v, at)) = hit {
                let region = procs.region(&[then.as_slice()]);
                attribute(&region, f, v, at);
            }
        }
        _ => {}
    });
    // a field value found for several alternatives of a choice point
    // (a constant every record gets) selects none of them
    for (_, list) in &alts {
        let mut seen: HashMap<(String, String), usize> = HashMap::new();
        for (a, _) in list {
            for e in out.get(a).into_iter().flatten() {
                *seen.entry((e.field.clone(), e.value.clone())).or_default() += 1;
            }
        }
        for (a, _) in list {
            if let Some(v) = out.get_mut(a) {
                v.retain(|e| seen.get(&(e.field.clone(), e.value.clone())) == Some(&1));
            }
        }
    }
    out.retain(|_, v| !v.is_empty());
    out
}

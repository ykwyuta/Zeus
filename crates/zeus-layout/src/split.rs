//! Layouts of a record: every group of items sharing an area (an item
//! and the items that REDEFINE it) is a choice point; a layout chooses
//! one of them at each choice point, nested choice points expanding
//! under their alternative only.

use std::collections::{BTreeMap, HashSet};

use zeus_cobol::front::ast::Usage;
use zeus_cobol::sema::data::DataModel;

/// The choices of a layout: choice point (its base item) -> chosen item.
pub type Choices = BTreeMap<usize, usize>;

pub struct Splitter<'a> {
    pub dm: &'a DataModel,
    /// Keep a base that is a plain PIC X container redefined by groups.
    pub keep_base: bool,
    /// Choice points left as REDEFINES (over the layout limit).
    pub kept: HashSet<usize>,
}

impl<'a> Splitter<'a> {
    pub fn new(dm: &'a DataModel, keep_base: bool) -> Self {
        Splitter {
            dm,
            keep_base,
            kept: HashSet::new(),
        }
    }

    /// The children of `id` grouped by area: an item and its REDEFINES.
    pub fn groups(&self, id: usize) -> Vec<Vec<usize>> {
        let mut out: Vec<Vec<usize>> = Vec::new();
        for &c in &self.dm.items[id].children {
            let it = &self.dm.items[c];
            if let Some(r) = &it.entry.redefines
                && let Some(g) = out.last_mut()
                && self.dm.items[g[0]].name.as_deref() == Some(r.as_str())
            {
                g.push(c);
                continue;
            }
            out.push(vec![c]);
        }
        out
    }

    /// The items a layout may choose at a choice point.
    pub fn alternatives(&self, group: &[usize]) -> Vec<usize> {
        if group.len() < 2 || self.keep_base {
            return group.to_vec();
        }
        let base = &self.dm.items[group[0]];
        let plain = base.children.is_empty()
            && matches!(base.usage, Usage::Display)
            && base.entry.picture.as_deref().is_some_and(|p| {
                p.chars()
                    .all(|c| matches!(c.to_ascii_uppercase(), 'X' | '(' | ')' | '0'..='9'))
            });
        let by_group = group[1..]
            .iter()
            .any(|&r| !self.dm.items[r].children.is_empty());
        if plain && by_group {
            group[1..].to_vec()
        } else {
            group.to_vec()
        }
    }

    /// The number of layouts of `id` (saturating).
    pub fn count(&self, id: usize) -> f64 {
        let mut n = 1.0f64;
        for g in self.groups(id) {
            let k = if g.len() == 1 {
                self.count(g[0])
            } else if self.kept.contains(&g[0]) {
                1.0
            } else {
                self.alternatives(&g).iter().map(|&a| self.count(a)).sum()
            };
            n *= k;
        }
        n
    }

    /// The layouts of `id`.
    pub fn variants(&self, id: usize) -> Vec<Choices> {
        let mut acc: Vec<Choices> = vec![Choices::new()];
        for g in self.groups(id) {
            let opts: Vec<Choices> = if g.len() == 1 {
                self.variants(g[0])
            } else if self.kept.contains(&g[0]) {
                vec![Choices::new()]
            } else {
                let mut v = Vec::new();
                for a in self.alternatives(&g) {
                    for mut c in self.variants(a) {
                        c.insert(g[0], a);
                        v.push(c);
                    }
                }
                v
            };
            let mut next = Vec::with_capacity(acc.len() * opts.len());
            for a in &acc {
                for o in &opts {
                    let mut c = a.clone();
                    c.extend(o.iter().map(|(k, v)| (*k, *v)));
                    next.push(c);
                }
            }
            acc = next;
        }
        acc
    }

    /// The choice points under `id` (bases of groups of two or more).
    pub fn choice_points(&self, id: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for g in self.groups(id) {
            if g.len() > 1 {
                out.push(g[0]);
            }
            for &c in &g {
                out.extend(self.choice_points(c));
            }
        }
        out
    }

    /// Leaves choice points as REDEFINES, the smallest first, until `id`
    /// has at most `max` layouts; choice points in `protect` (those with
    /// a known record type) are split as long as possible. Returns the
    /// kept choice points in order.
    pub fn limit(&mut self, id: usize, max: usize, protect: &HashSet<usize>) -> Vec<usize> {
        let mut kept = Vec::new();
        while self.count(id) > max as f64 {
            let mut cands: Vec<usize> = self
                .choice_points(id)
                .into_iter()
                .filter(|b| !self.kept.contains(b))
                .collect();
            if cands.is_empty() {
                break;
            }
            cands.sort_by_key(|&b| (protect.contains(&b), self.dm.items[b].len, b));
            let b = cands[0];
            self.kept.insert(b);
            kept.push(b);
        }
        kept
    }
}

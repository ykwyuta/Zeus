//! Data division: storage layout, descriptors, initial values and name
//! resolution.

use std::collections::HashMap;

use zeus_decimal::{Dec, binary};

use super::lit;
use super::picture::{self, Category, Picture};
use crate::diag::{Diag, Result};
use crate::ebcdic;
use crate::front::ast::{self, DataEntry, DataRef, Usage, ValueClause};
use crate::front::source::Pos;
use crate::vm::data as vdata;
use crate::zir::{
    Arg, Base, DataType, Desc, Dim, Ex, ExternalArea, Func, Loc, NumType, NumUsage, Odo, Op,
    SignPos,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    Working,
    Linkage(u32),
    File(usize),
    /// Hidden or special items (indexes, special registers).
    Special,
    /// EXTERNAL record (index into `DataModel::externals`).
    External(u32),
}

#[derive(Debug, Clone)]
pub struct Item {
    pub name: Option<String>,
    pub level: u8,
    pub pos: Pos,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    pub area: Area,
    pub offset: u32,
    /// Length of one occurrence.
    pub len: u32,
    pub occurs: Option<ast::Occurs>,
    pub pic: Option<Picture>,
    pub usage: Usage,
    pub entry: DataEntry,
    /// Level-88 condition-names of this item.
    pub conditions: Vec<usize>,
    /// The conditional variable of a level-88 item.
    pub cond_of: Option<usize>,
    /// Descriptor index.
    pub desc: u32,
    /// Index-name: the table it indexes.
    pub index_of: Option<usize>,
    /// GLOBAL item of a containing program (its module program index).
    pub owner: Option<u32>,
    /// Visible to contained programs (GLOBAL).
    pub global: bool,
    /// RENAMES: (first, last) items.
    pub renames: Option<(usize, usize)>,
}

#[derive(Debug, Default, Clone)]
pub struct DataModel {
    pub label_scope: bool,
    pub label_roots: Vec<usize>,
    pub dynamic_init: Vec<(Loc, Vec<u8>)>,
    pub items: Vec<Item>,
    pub descs: Vec<Desc>,
    pub static_init: Vec<u8>,
    /// LOCAL-STORAGE bytes within the image; restored on each invocation.
    pub local_storage: Option<(u32, u32)>,
    pub linkage_count: u32,
    pub by_name: HashMap<String, Vec<usize>>,
    /// File name -> record area item, all record items.
    pub files: HashMap<String, (usize, Vec<usize>)>,
    pub decimal_comma: bool,
    pub currency: char,
    pub externals: Vec<ExternalArea>,
    /// Files visible to contained programs: name -> (FD is GLOBAL).
    pub global_files: Vec<String>,
    /// LINAGE-COUNTER item of each file with a LINAGE clause.
    pub linage_counters: HashMap<String, usize>,
    /// LINE-COUNTER and PAGE-COUNTER items of each report.
    pub report_counters: HashMap<String, (usize, usize)>,
}

pub struct Settings {
    pub decimal_comma: bool,
    pub currency: char,
}

fn align(n: u32, a: u32) -> u32 {
    n.div_ceil(a) * a
}

impl DataModel {
    /// Keep maximum storage allocation, but calculate complex ODO locations
    /// from the current object values on every reference.
    fn dynamic_layout(&mut self) -> Result<()> {
        for (id, item) in self.items.iter().enumerate() {
            if item.entry.justified
                && (!item.children.is_empty()
                    || item.level == 66
                    || item.level == 88
                    || !item.pic.as_ref().is_some_and(|p| {
                        matches!(
                            p.category,
                            Category::Alphabetic | Category::Alphanumeric | Category::Dbcs
                        )
                    })
                    || !matches!(
                        item.usage,
                        Usage::Display | Usage::Display1 | Usage::National
                    ))
            {
                return Err(Diag::error(
                    item.pos,
                    "JUSTIFIED requires an elementary alphabetic, alphanumeric, DBCS or national item",
                ));
            }
            if item.entry.blank_when_zero
                && (!item.children.is_empty()
                    || !matches!(item.usage, Usage::Display | Usage::National)
                    || !item.pic.as_ref().is_some_and(|p| {
                        matches!(p.category, Category::Numeric | Category::NumericEdited)
                            && !p.signed
                    })
                    || item.entry.picture.as_ref().is_some_and(|p| p.contains('*')))
            {
                return Err(Diag::error(
                    item.pos,
                    "BLANK WHEN ZERO requires an unsigned DISPLAY/NATIONAL numeric or numeric-edited PICTURE without '*'",
                ));
            }
            if item.entry.sign.is_some() {
                fn signed(dm: &DataModel, id: usize) -> bool {
                    let item = &dm.items[id];
                    if item.children.is_empty() {
                        item.pic
                            .as_ref()
                            .is_some_and(|p| p.category == Category::Numeric && p.signed)
                            && matches!(item.usage, Usage::Display | Usage::National)
                    } else {
                        item.children.iter().any(|&i| signed(dm, i))
                    }
                }
                if !signed(self, id)
                    && !item
                        .pic
                        .as_ref()
                        .is_some_and(|p| p.category == Category::ExternalFloat)
                {
                    return Err(Diag::error(
                        item.pos,
                        "SIGN requires a signed DISPLAY/NATIONAL numeric item or a group containing one",
                    ));
                }
            }
        }
        fn lit(n: u32) -> Ex {
            Ex::Lit(Dec::from_i64(n as i64))
        }
        fn bin(op: Op, a: Ex, b: Ex) -> Ex {
            if let (Ex::Lit(x), Ex::Lit(y)) = (&a, &b) {
                let x = x.coef as u32;
                let y = y.coef as u32;
                return lit(match op {
                    Op::Add => x + y,
                    Op::Sub => x - y,
                    Op::Mul => x * y,
                    _ => unreachable!(),
                });
            }
            match (&a, &b, op) {
                (_, Ex::Lit(v), Op::Add | Op::Sub) if *v == Dec::ZERO => a,
                (Ex::Lit(v), _, Op::Add) if *v == Dec::ZERO => b,
                (_, Ex::Lit(v), Op::Mul) if *v == Dec::from_i64(1) => a,
                _ => Ex::Bin(op, Box::new(a), Box::new(b)),
            }
        }
        fn variable(e: &Ex) -> Option<Box<Ex>> {
            if matches!(e, Ex::Lit(_)) {
                None
            } else {
                Some(Box::new(e.clone()))
            }
        }
        fn walk(dm: &mut DataModel, id: usize, start: Ex, lengths: &mut [Ex]) -> Result<Ex> {
            let item = dm.items[id].clone();
            let mut size = lit(item.len);
            if !item.children.is_empty() {
                let mut cursor = lit(0);
                let mut end = lit(0);
                let overlays = item
                    .children
                    .iter()
                    .any(|&child| dm.items[child].entry.redefines.is_some());
                let mut offsets = HashMap::new();
                for child in &item.children {
                    let ci = dm.items[*child].clone();
                    let mut relative = if let Some(target) = &ci.entry.redefines {
                        offsets
                            .get(target)
                            .cloned()
                            .unwrap_or_else(|| cursor.clone())
                    } else {
                        cursor.clone()
                    };
                    if ci.entry.sync {
                        let alignment = match ci.usage {
                            Usage::Binary
                            | Usage::NativeBinary
                            | Usage::Index
                            | Usage::Pointer
                            | Usage::Pointer32
                            | Usage::ProcedurePointer
                            | Usage::FunctionPointer
                            | Usage::Float
                            | Usage::Double => dm.elementary_len(*child)?.max(1),
                            _ => 1,
                        };
                        if alignment > 1 {
                            let absolute = bin(Op::Add, start.clone(), relative.clone());
                            if let Ex::Lit(n) = absolute {
                                relative = bin(
                                    Op::Sub,
                                    lit(align(n.coef as u32, alignment)),
                                    start.clone(),
                                );
                            } else {
                                let rounded = Ex::Func(
                                    Func::IntegerPart,
                                    vec![Arg::Num(Ex::Bin(
                                        Op::Div,
                                        Box::new(bin(Op::Add, absolute, lit(alignment - 1))),
                                        Box::new(lit(alignment)),
                                    ))],
                                );
                                relative = bin(
                                    Op::Sub,
                                    bin(Op::Mul, rounded, lit(alignment)),
                                    start.clone(),
                                );
                            }
                        }
                    }
                    if let Some(name) = &ci.name {
                        offsets.insert(name.clone(), relative.clone());
                    }
                    let total = walk(
                        dm,
                        *child,
                        bin(Op::Add, start.clone(), relative.clone()),
                        lengths,
                    )?;
                    let next = bin(Op::Add, relative, total);
                    if ci.entry.redefines.is_none() {
                        cursor = next.clone();
                    }
                    end = match (&end, &next) {
                        (Ex::Lit(a), Ex::Lit(b)) => Ex::Lit((*a).max(*b)),
                        _ if !overlays => next,
                        _ => Ex::Func(Func::Max, vec![Arg::Num(end), Arg::Num(next)]),
                    };
                }
                size = end;
            }
            lengths[id] = size.clone();
            let desc = &mut dm.descs[item.desc as usize];
            desc.dynamic_offset = variable(&start);
            desc.dynamic_len = variable(&size);
            let count = if let Some(o) = &item.occurs {
                if let Some(dep) = &o.depending {
                    let target = dm.resolve(dep)?;
                    let ty = &dm.descs[dm.items[target].desc as usize].ty;
                    if !matches!(ty, DataType::Numeric(n) if n.scale == 0) {
                        return Err(Diag::error(
                            dep.pos,
                            "ODO object must be an integer data item",
                        ));
                    }
                    Ex::Load(Loc {
                        desc: dm.items[target].desc,
                        subs: vec![],
                        refmod: None,
                    })
                } else {
                    lit(o.max.max(1))
                }
            } else {
                lit(1)
            };
            Ok(bin(Op::Mul, size, count))
        }
        let roots: Vec<_> = self
            .items
            .iter()
            .enumerate()
            .filter(|(_, i)| i.parent.is_none() && i.cond_of.is_none() && i.index_of.is_none())
            .map(|(i, _)| i)
            .collect();
        let mut lengths = vec![lit(0); self.items.len()];
        for id in roots {
            walk(self, id, lit(self.items[id].offset), &mut lengths)?;
        }
        for id in 0..self.items.len() {
            let item = self.items[id].clone();
            if item.cond_of.is_some() {
                continue;
            }
            let mut parents = vec![];
            let mut p = Some(id);
            while let Some(i) = p {
                if self.items[i].occurs.is_some() {
                    parents.push(i);
                }
                p = self.items[i].parent;
            }
            parents.reverse();
            if let Some(&first) = parents.first() {
                let root = &self.descs[self.items[first].desc as usize];
                self.descs[item.desc as usize].table_bounds = Some(crate::zir::TableBounds {
                    offset: root.offset,
                    dynamic_offset: root.dynamic_offset.clone(),
                    len: self.items[first].len * self.items[first].occurs.as_ref().unwrap().max,
                });
            }
            for (k, p) in parents.into_iter().enumerate() {
                self.descs[item.desc as usize].dims[k].dynamic_stride = variable(&lengths[p]);
                if let Some(dep) = self.items[p]
                    .occurs
                    .as_ref()
                    .and_then(|o| o.depending.as_ref())
                {
                    let target = self.resolve(dep)?;
                    self.descs[item.desc as usize].dims[k].count = Some(Box::new(Ex::Load(Loc {
                        desc: self.items[target].desc,
                        subs: vec![],
                        refmod: None,
                    })));
                }
            }
            if let Some(dep) = item.occurs.as_ref().and_then(|o| o.depending.as_ref()) {
                let target = self.resolve(dep)?;
                let desc = &self.descs[self.items[target].desc as usize];
                if desc.dynamic_offset.is_some() || !desc.dims.is_empty() {
                    return Err(Diag::error(
                        dep.pos,
                        "ODO object cannot be variably located or subordinate to OCCURS",
                    ));
                }
            }
            let desc = &self.descs[item.desc as usize];
            if desc.dynamic_offset.is_none() && desc.dims.iter().all(|d| d.dynamic_stride.is_none())
            {
                continue;
            }
            if !matches!(item.area, Area::Working) || item.level == 66 {
                continue;
            }
            let mut p = Some(id);
            let mut excluded = false;
            while let Some(i) = p {
                if self.items[i].entry.redefines.is_some()
                    || (i != id && self.items[i].entry.value.is_some())
                {
                    excluded = true;
                }
                p = self.items[i].parent;
            }
            if !excluded && (item.children.is_empty() || item.entry.value.is_some()) {
                self.dynamic_init.push((
                    Loc {
                        desc: item.desc,
                        subs: vec![],
                        refmod: None,
                    },
                    self.static_init[item.offset as usize..(item.offset + item.len) as usize]
                        .to_vec(),
                ));
            }
        }
        Ok(())
    }
    fn add(&mut self, item: Item) -> usize {
        let id = self.items.len();
        if let Some(n) = &item.name {
            self.by_name.entry(n.clone()).or_default().push(id);
        }
        self.items.push(item);
        id
    }

    fn new_item(e: &DataEntry, area: Area) -> Item {
        Item {
            name: e.name.clone(),
            level: e.level,
            pos: e.pos,
            parent: None,
            children: Vec::new(),
            area,
            offset: 0,
            len: 0,
            occurs: e.occurs.clone(),
            pic: None,
            usage: e.usage.unwrap_or_default(),
            entry: e.clone(),
            conditions: Vec::new(),
            cond_of: None,
            desc: 0,
            index_of: None,
            owner: None,
            global: e.global,
            renames: None,
        }
    }

    /// Builds the item tree of one section; returns the record (level 01
    /// and 77) items.
    fn build_tree(
        &mut self,
        entries: &[DataEntry],
        area_of: &mut dyn FnMut(usize) -> Area,
    ) -> Result<Vec<usize>> {
        let mut records = Vec::new();
        let mut stack: Vec<usize> = Vec::new();
        let mut last_non88: Option<usize> = None;
        for e in entries {
            match e.level {
                88 => {
                    let Some(owner) = last_non88 else {
                        return Err(Diag::error(
                            e.pos,
                            "level 88 without a conditional variable",
                        ));
                    };
                    let mut it = Self::new_item(e, self.items[owner].area);
                    it.cond_of = Some(owner);
                    it.parent = Some(owner);
                    let id = self.add(it);
                    self.items[owner].conditions.push(id);
                }
                66 => {
                    let Some(&record) = stack.first() else {
                        return Err(Diag::error(e.pos, "level 66 without a record"));
                    };
                    let mut it = Self::new_item(e, self.items[record].area);
                    it.parent = Some(record);
                    let id = self.add(it);
                    // resolved after layout
                    self.items[id].renames = None;
                }
                1 | 77 => {
                    let area = area_of(records.len());
                    let id = self.add(Self::new_item(e, area));
                    records.push(id);
                    stack.clear();
                    stack.push(id);
                    last_non88 = Some(id);
                }
                lvl => {
                    while let Some(&top) = stack.last() {
                        if self.items[top].level < lvl && self.items[top].level != 77 {
                            break;
                        }
                        stack.pop();
                    }
                    let Some(&parent) = stack.last() else {
                        return Err(Diag::error(
                            e.pos,
                            format!("level {lvl} item outside a record"),
                        ));
                    };
                    let mut it = Self::new_item(e, self.items[parent].area);
                    it.parent = Some(parent);
                    let id = self.add(it);
                    self.items[parent].children.push(id);
                    stack.push(id);
                    last_non88 = Some(id);
                }
            }
        }
        Ok(records)
    }

    fn analyze_pics(&mut self, settings: &Settings) -> Result<()> {
        for it in &mut self.items {
            if let Some(p) = &it.entry.picture {
                it.pic = Some(
                    picture::analyze(p, settings.decimal_comma, settings.currency)
                        .map_err(|m| Diag::error(it.pos, m))?,
                );
                let pic = it.pic.as_ref().unwrap();
                if pic.category == Category::ExternalFloat
                    && (!matches!(it.usage, Usage::Display | Usage::National)
                        || it.entry.blank_when_zero
                        || it.entry.justified
                        || it.entry.value.is_some())
                {
                    return Err(Diag::error(
                        it.pos,
                        "external floating item requires DISPLAY/NATIONAL and forbids VALUE, JUSTIFIED and BLANK WHEN ZERO",
                    ));
                }
                if matches!(pic.category, Category::Numeric | Category::NumericEdited)
                    && pic.digits > crate::front::numeric().operand_digits()
                {
                    return Err(Diag::error(
                        it.pos,
                        "PICTURE exceeds ARITH(COMPAT/EXTEND) digit limit",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Storage length of an elementary item.
    fn elementary_len(&self, id: usize) -> Result<u32> {
        let it = &self.items[id];
        if matches!(
            it.usage,
            Usage::Index | Usage::Pointer | Usage::Pointer32 | Usage::Float | Usage::Double
        ) && (it.pic.is_some()
            || it.entry.sign.is_some()
            || it.entry.justified
            || it.entry.blank_when_zero)
        {
            return Err(Diag::error(
                it.pos,
                "INDEX, POINTER and internal floating items forbid PICTURE, SIGN, JUSTIFIED and BLANK WHEN ZERO",
            ));
        }
        if matches!(
            it.usage,
            Usage::Binary | Usage::NativeBinary | Usage::Packed
        ) && !it
            .pic
            .as_ref()
            .is_some_and(|p| p.category == Category::Numeric)
        {
            return Err(Diag::error(
                it.pos,
                "binary and packed-decimal items require a numeric PICTURE",
            ));
        }
        if matches!(it.usage, Usage::ProcedurePointer | Usage::FunctionPointer)
            && (it.pic.is_some()
                || it.entry.sign.is_some()
                || it.entry.justified
                || it.entry.blank_when_zero)
        {
            return Err(Diag::error(
                it.pos,
                "procedure/function-pointer cannot have PICTURE, SIGN, JUSTIFIED or BLANK WHEN ZERO",
            ));
        }
        Ok(match it.usage {
            Usage::Index | Usage::Pointer | Usage::FunctionPointer => {
                if crate::front::lp64() {
                    8
                } else {
                    4
                }
            }
            Usage::Pointer32 | Usage::Float => 4,
            Usage::ProcedurePointer => 8,
            Usage::Double => 8,
            Usage::Binary | Usage::NativeBinary => {
                let p = it
                    .pic
                    .as_ref()
                    .ok_or_else(|| Diag::error(it.pos, "PICTURE required"))?;
                binary::len_for_digits(p.digits) as u32
            }
            Usage::Packed => {
                let p = it
                    .pic
                    .as_ref()
                    .ok_or_else(|| Diag::error(it.pos, "PICTURE required"))?;
                (p.digits / 2 + 1).max(1)
            }
            Usage::Display | Usage::Display1 | Usage::National => {
                let p = it.pic.as_ref().ok_or_else(|| {
                    Diag::error(
                        it.pos,
                        format!(
                            "PICTURE required for {}",
                            it.name.as_deref().unwrap_or("FILLER")
                        ),
                    )
                })?;
                if it.usage == Usage::National
                    && (matches!(p.category, Category::Alphabetic | Category::Alphanumeric)
                        || (p.category == Category::AlphanumericEdited
                            && !it.entry.picture.as_ref().is_some_and(|s| s.contains('N')))
                        || it.entry.picture.as_ref().is_some_and(|s| s.contains('G')))
                {
                    return Err(Diag::error(
                        it.pos,
                        "USAGE NATIONAL requires a national or numeric PICTURE",
                    ));
                }
                let sign = it.entry.sign.or_else(|| self.inherited_sign(id));
                let sep =
                    matches!(sign, Some((_, true))) && p.category == Category::Numeric && p.signed;
                if it.usage == Usage::National && p.category != Category::Dbcs {
                    if p.category == Category::Numeric && p.signed && !sep {
                        return Err(Diag::error(
                            it.pos,
                            "signed NATIONAL numeric items require SIGN SEPARATE",
                        ));
                    }
                    let national_edit = it.entry.picture.as_ref().is_some_and(|s| s.contains('N'));
                    if national_edit {
                        p.size
                    } else {
                        2 * (p.size + u32::from(sep))
                    }
                } else {
                    p.size + u32::from(sep)
                }
            }
        })
    }

    /// Computes lengths and offsets of `id` and its subordinates, placed at
    /// `offset`. Returns the item's total size (all occurrences).
    fn layout(&mut self, id: usize, offset: u32) -> Result<u32> {
        if !self.items[id].children.is_empty()
            && matches!(
                self.items[id].usage,
                Usage::ProcedurePointer | Usage::FunctionPointer
            )
        {
            return Err(Diag::error(
                self.items[id].pos,
                "procedure/function-pointer must be elementary",
            ));
        }
        let inherited_national = self.items[id]
            .parent
            .is_some_and(|p| self.items[p].entry.group_national);
        if inherited_national || self.items[id].entry.group_national {
            let item = &mut self.items[id];
            if item.entry.usage.is_some_and(|u| u != Usage::National)
                || (!item.children.is_empty()
                    && (item.entry.usage.is_some() || item.entry.justified))
                || (item.children.is_empty() && item.entry.group_national)
            {
                return Err(Diag::error(
                    item.pos,
                    "invalid USAGE/JUSTIFIED or elementary GROUP-USAGE NATIONAL",
                ));
            }
            if !item.children.is_empty() {
                item.entry.group_national = true;
            }
            item.usage = Usage::National;
        }
        if self.items[id]
            .pic
            .as_ref()
            .is_some_and(|p| p.category == Category::AlphanumericEdited)
            && self.items[id]
                .entry
                .picture
                .as_ref()
                .is_some_and(|s| s.contains('N'))
        {
            if self.items[id]
                .entry
                .usage
                .is_some_and(|u| u != Usage::National)
            {
                return Err(Diag::error(
                    self.items[id].pos,
                    "national-edited items require USAGE NATIONAL",
                ));
            }
            self.items[id].usage = Usage::National;
        }
        // Inherit USAGE from groups.
        if let Some(parent) = self.items[id].parent
            && self.items[id].entry.usage.is_none()
            && self.items[id].usage != Usage::National
            && self.items[id].cond_of.is_none()
        {
            let pu = self.items[parent].usage;
            if pu != Usage::Display {
                self.items[id].usage = pu;
            }
        }
        let mut offset = offset;
        if self.items[id].entry.sync {
            let a = match self.items[id].usage {
                Usage::Binary
                | Usage::NativeBinary
                | Usage::Index
                | Usage::Pointer
                | Usage::Pointer32
                | Usage::ProcedurePointer
                | Usage::FunctionPointer
                | Usage::Float => self.elementary_len(id)?,
                Usage::Double => 8,
                _ => 1,
            };
            offset = align(offset, a.max(1));
        }
        self.items[id].offset = offset;
        let children = self.items[id].children.clone();
        let len = if children.is_empty() {
            self.elementary_len(id)?
        } else {
            let mut cursor = offset;
            let mut end = offset;
            for &c in &children {
                if let Some(target) = self.items[c].entry.redefines.clone() {
                    let t = children
                        .iter()
                        .copied()
                        .find(|&x| self.items[x].name.as_deref() == Some(&target))
                        .ok_or_else(|| {
                            Diag::error(
                                self.items[c].pos,
                                format!("REDEFINES target {target} not found"),
                            )
                        })?;
                    let size = self.layout(c, self.items[t].offset)?;
                    end = end.max(self.items[t].offset + size);
                } else {
                    let size = self.layout(c, cursor)?;
                    // SYNC may have moved the child
                    cursor = self.items[c].offset + size;
                    end = end.max(cursor);
                }
            }
            end - offset
        };
        self.items[id].len = len;
        let count = self.items[id].occurs.as_ref().map_or(1, |o| o.max.max(1));
        Ok(len * count)
    }

    /// OCCURS dimensions of an item (outermost first).
    fn dims(&self, id: usize) -> Vec<Dim> {
        let mut dims = Vec::new();
        let mut cur = Some(id);
        while let Some(c) = cur {
            if let Some(o) = &self.items[c].occurs {
                dims.push(Dim {
                    count: None,
                    dynamic_stride: None,
                    stride: self.items[c].len,
                    max: o.max,
                });
            }
            cur = self.items[c].parent;
        }
        dims.reverse();
        dims
    }

    fn data_type(&self, id: usize) -> DataType {
        let it = &self.items[id];
        if !it.children.is_empty() || it.renames.is_some() {
            return if it.entry.group_national {
                DataType::National
            } else {
                DataType::Group
            };
        }
        match it.usage {
            Usage::Index => return DataType::Index,
            Usage::Pointer | Usage::Pointer32 => return DataType::Pointer,
            Usage::ProcedurePointer => return DataType::ProcedurePointer,
            Usage::FunctionPointer => return DataType::FunctionPointer,
            Usage::Float => return DataType::Float { long: false },
            Usage::Double => return DataType::Float { long: true },
            _ => {}
        }
        let Some(p) = &it.pic else {
            return DataType::Group;
        };
        let just = it.entry.justified;
        match p.category {
            Category::ExternalFloat => DataType::ExternalFloat {
                picture: picture::expand(it.entry.picture.as_ref().unwrap()).unwrap(),
                national: it.usage == Usage::National,
                comma: self.decimal_comma,
            },
            Category::Alphabetic => DataType::Alphabetic { just },
            Category::Alphanumeric => DataType::Alphanumeric { just },
            // PICTURE N: national with USAGE NATIONAL or NSYMBOL(NATIONAL)
            // (unless USAGE DISPLAY-1); PICTURE G: DBCS
            Category::Dbcs => {
                let pic_n = it
                    .entry
                    .picture
                    .as_deref()
                    .is_some_and(|p| p.to_ascii_uppercase().contains('N'));
                let national = pic_n
                    && (it.usage == Usage::National
                        || (it.usage != Usage::Display1 && !crate::front::nsymbol_dbcs()));
                if national {
                    DataType::National
                } else {
                    DataType::Dbcs
                }
            }
            Category::AlphanumericEdited => DataType::AlphanumericEdited {
                national: it.usage == Usage::National,
                edit: p.edit.clone(),
            },
            Category::NumericEdited => DataType::NumericEdited {
                national: it.usage == Usage::National,
                edit: p.edit.clone(),
                digits: p.digits,
                scale: p.scale,
                blank_zero: it.entry.blank_when_zero,
                currency: self.currency as u8,
                point: ebcdic::from_latin1(if self.decimal_comma { b',' } else { b'.' }),
            },
            Category::Numeric if it.entry.blank_when_zero => DataType::NumericEdited {
                national: it.usage == Usage::National,
                edit: (0..p.digits)
                    .map(|_| super::picture::EditSym::Digit)
                    .collect(),
                digits: p.digits,
                scale: p.scale,
                blank_zero: true,
                currency: self.currency as u8,
                point: ebcdic::from_latin1(if self.decimal_comma { b',' } else { b'.' }),
            },
            Category::Numeric => {
                let usage = match it.usage {
                    Usage::Packed => NumUsage::Packed,
                    Usage::National => NumUsage::National,
                    Usage::Binary if crate::front::trunc() == crate::front::Trunc::Bin => {
                        NumUsage::Native
                    }
                    Usage::Binary => NumUsage::Binary,
                    Usage::NativeBinary => NumUsage::Native,
                    _ => NumUsage::Display,
                };
                let sign = match it.entry.sign.or_else(|| self.inherited_sign(id)) {
                    None => SignPos::None,
                    Some((false, false)) => SignPos::Trailing,
                    Some((true, false)) => SignPos::Leading,
                    Some((false, true)) => SignPos::TrailingSeparate,
                    Some((true, true)) => SignPos::LeadingSeparate,
                };
                DataType::Numeric(NumType {
                    usage,
                    digits: p.digits,
                    scale: p.scale,
                    signed: p.signed,
                    sign,
                })
            }
        }
    }

    fn inherited_sign(&self, id: usize) -> Option<(bool, bool)> {
        let mut cur = self.items[id].parent;
        while let Some(c) = cur {
            if let Some(s) = self.items[c].entry.sign {
                return Some(s);
            }
            cur = self.items[c].parent;
        }
        None
    }

    fn base(&self, id: usize) -> Base {
        match (self.items[id].area, self.items[id].owner) {
            (Area::External(k), _) => Base::External(k),
            (Area::Linkage(n), Some(p)) => Base::OuterLinkage(p, n),
            (_, Some(p)) => Base::Outer(p),
            (Area::Linkage(n), None) => Base::Linkage(n),
            _ => Base::Static,
        }
    }

    fn external_index(&mut self, name: &str, len: u32) -> u32 {
        if let Some(i) = self.externals.iter().position(|e| e.name == name) {
            let e = &mut self.externals[i];
            e.len = e.len.max(len);
            return i as u32;
        }
        self.externals.push(ExternalArea {
            name: name.to_string(),
            len,
        });
        (self.externals.len() - 1) as u32
    }

    fn set_area(&mut self, id: usize, area: Area) {
        self.items[id].area = area;
        for c in self.items[id].children.clone() {
            self.set_area(c, area);
        }
        for c in self.items[id].conditions.clone() {
            self.items[c].area = area;
        }
    }

    /// Imports the GLOBAL items of a containing program (`index` in the
    /// module) so that a contained program can reference them.
    pub fn import_globals(&mut self, outer: &DataModel, index: u32) {
        let mut map: HashMap<usize, usize> = HashMap::new();
        // Roots: global records (own or already imported from further out).
        let roots: Vec<usize> = (0..outer.items.len())
            .filter(|&i| {
                let it = &outer.items[i];
                it.parent.is_none() && it.cond_of.is_none() && it.global
            })
            .collect();
        fn collect(dm: &DataModel, id: usize, out: &mut Vec<usize>) {
            out.push(id);
            for &c in &dm.items[id].children {
                collect(dm, c, out);
            }
            for &c in &dm.items[id].conditions {
                out.push(c);
            }
        }
        let mut ids = Vec::new();
        for r in roots {
            collect(outer, r, &mut ids);
        }
        // index-names of imported tables
        for i in 0..outer.items.len() {
            if let Some(t) = outer.items[i].index_of
                && ids.contains(&t)
            {
                ids.push(i);
            }
        }
        for &i in &ids {
            let mut it = outer.items[i].clone();
            if it.owner.is_none() {
                it.owner = Some(index);
            }
            it.global = true;
            let new = self.items.len();
            map.insert(i, new);
            if let Some(n) = &it.name {
                self.by_name.entry(n.clone()).or_default().push(new);
            }
            if let Area::External(k) = it.area {
                let e = &outer.externals[k as usize];
                let k2 = self.external_index(&e.name.clone(), e.len);
                it.area = Area::External(k2);
            }
            self.items.push(it);
        }
        for &i in &ids {
            let n = map[&i];
            let it = &mut self.items[n];
            it.parent = it.parent.and_then(|p| map.get(&p).copied());
            it.children = it
                .children
                .iter()
                .filter_map(|c| map.get(c).copied())
                .collect();
            it.conditions = it
                .conditions
                .iter()
                .filter_map(|c| map.get(c).copied())
                .collect();
            it.cond_of = it.cond_of.and_then(|c| map.get(&c).copied());
            it.index_of = it.index_of.and_then(|c| map.get(&c).copied());
            it.renames = it
                .renames
                .and_then(|(a, b)| Some((*map.get(&a)?, *map.get(&b)?)));
        }
        // global files
        for f in &outer.global_files {
            if self.files.contains_key(f) {
                continue;
            }
            if let Some((area, recs)) = outer.files.get(f)
                && let Some(&a) = map.get(area)
            {
                let recs = recs.iter().filter_map(|r| map.get(r).copied()).collect();
                self.files.insert(f.clone(), (a, recs));
                self.global_files.push(f.clone());
            }
        }
        for &i in &ids {
            let n = map[&i];
            if self.items[n].cond_of.is_none() {
                self.make_desc(n);
                let source = &outer.descs[outer.items[i].desc as usize];
                let target = &mut self.descs[self.items[n].desc as usize];
                target.dynamic_offset = source.dynamic_offset.clone();
                target.dynamic_len = source.dynamic_len.clone();
                target.dims = source.dims.clone();
                target.table_bounds = source.table_bounds.clone();
            }
        }
    }

    /// Creates the descriptor of an item.
    fn make_desc(&mut self, id: usize) {
        fn abi(
            dm: &DataModel,
            id: usize,
            base: u32,
            mut occurs: Vec<u32>,
            out: &mut Vec<crate::zir::AbiField>,
        ) {
            let item = &dm.items[id];
            if let Some(o) = &item.occurs {
                occurs.push(o.max);
            }
            if item.children.is_empty() {
                out.push(crate::zir::AbiField {
                    offset: item.offset - base,
                    len: item.len,
                    ty: dm.data_type(id).abi_type(),
                    synchronized: item.entry.sync,
                    occurs,
                });
            } else {
                for &child in &item.children {
                    if dm.items[child].level != 66 {
                        abi(dm, child, base, occurs.clone(), out);
                    }
                }
            }
        }
        let mut fields = Vec::new();
        abi(self, id, self.items[id].offset, vec![], &mut fields);
        let it = &self.items[id];
        let desc = Desc {
            table_bounds: None,
            right_justify: it.entry.justified,
            dynamic_offset: None,
            dynamic_len: None,
            abi: fields,
            name: it.name.clone().unwrap_or_else(|| "FILLER".into()),
            definition: (it.pos.file, it.pos.line),
            base: self.base(id),
            offset: it.offset,
            len: it.len,
            dims: self.dims(id),
            ty: self.data_type(id),
            odo: None,
        };
        self.items[id].desc = self.descs.len() as u32;
        self.descs.push(desc);
    }

    /// Writes the initial value of an item (and its subordinates) into
    /// static storage.
    fn init_values(&mut self, id: usize, base: u32, image: &mut Vec<u8>) -> Result<()> {
        let it = self.items[id].clone();
        if it.cond_of.is_some()
            || it.owner.is_some()
            || matches!(it.area, Area::Linkage(_) | Area::External(_))
        {
            return Ok(());
        }
        let count = it.occurs.as_ref().map_or(1, |o| o.max.max(1));
        // offset of the first occurrence relative to the record
        let start = base + it.offset;
        let total = (start + it.len * count) as usize;
        if image.len() < total {
            image.resize(total, 0);
        }
        let ty = self.descs[it.desc as usize].ty.clone();
        for k in 0..count {
            let s = (start + k * it.len) as usize;
            let field = &mut image[s..s + it.len as usize];
            if let Some(ValueClause::Literal(v)) = &it.entry.value {
                self.store_value(&ty, field, v, it.pos)?;
                continue;
            }
            if it.children.is_empty() {
                default_value(&ty, field);
            }
        }
        if it.entry.value.is_none() || !it.children.is_empty() {
            for &c in &it.children {
                if self.items[c].entry.redefines.is_some() {
                    continue;
                }
                if matches!(it.entry.value, Some(ValueClause::Literal(_))) {
                    continue;
                }
                for k in 0..count {
                    let sub_base = base + k * it.len;
                    self.init_values(c, sub_base, image)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn store_value(
        &self,
        ty: &DataType,
        field: &mut [u8],
        v: &ast::Operand,
        pos: Pos,
    ) -> Result<()> {
        if let ast::Operand::Lit(ast::Literal::Num(text)) = v {
            if text.contains('E') {
                let value =
                    crate::numeric::floating_literal(text).map_err(|m| Diag::error(pos, m))?;
                if value
                    .store(ty, field, false)
                    .map_err(|m| Diag::error(pos, m))?
                {
                    return Err(Diag::error(
                        pos,
                        "VALUE exceeds the receiving item's capacity",
                    ));
                }
                return Ok(());
            }
            if matches!(ty, DataType::Numeric(t) if t.usage == NumUsage::Native) {
                let value =
                    crate::numeric::number::Number::parse(text).map_err(|m| Diag::error(pos, m))?;
                if value
                    .store(ty, field, false)
                    .map_err(|m| Diag::error(pos, m))?
                {
                    return Err(Diag::error(
                        pos,
                        "VALUE exceeds the native binary storage range",
                    ));
                }
                return Ok(());
            }
        }
        let value = lit::constant(v, self.decimal_comma).map_err(|m| Diag::error(pos, m))?;
        if ty.is_entry_pointer() && !matches!(value, lit::Const::Null) {
            return Err(Diag::error(
                pos,
                "procedure/function-pointer VALUE must be NULL",
            ));
        }
        if ty.is_national() && *ty != DataType::National {
            match &value {
                lit::Const::Num(d, _) => {
                    crate::vm::eval::move_into(&crate::vm::eval::Val::Num(*d), ty, field)
                }
                lit::Const::National(b) => vdata::move_national(b, field),
                lit::Const::Bytes(b) => vdata::move_national(&vdata::national_of(b), field),
                lit::Const::Fig(f) => vdata::fill_pattern(field, &vdata::national_fig(f.byte())),
                lit::Const::All(b) => vdata::fill_pattern(field, &vdata::national_of(b)),
                lit::Const::Null => field.fill(0),
            }
            return Ok(());
        }
        match (&value, ty) {
            // national items: national literals as they are, other
            // literals and figurative constants converted (national spaces)
            (lit::Const::National(b), DataType::National) => vdata::move_national(b, field),
            (lit::Const::National(b), _) => vdata::move_alnum(b, field, false),
            (lit::Const::Bytes(b), DataType::National) => {
                vdata::move_national(&vdata::national_of(b), field);
            }
            (lit::Const::Fig(f), DataType::National) => {
                vdata::fill_pattern(field, &vdata::national_fig(f.byte()));
            }
            (lit::Const::All(p), DataType::National) => {
                vdata::fill_pattern(field, &vdata::national_of(p));
            }
            (lit::Const::Num(d, _), DataType::Numeric(t)) => {
                vdata::store_num(t, field, *d, false);
            }
            (
                lit::Const::Num(d, _),
                DataType::NumericEdited {
                    edit,
                    digits,
                    scale,
                    blank_zero,
                    currency,
                    point,
                    ..
                },
            ) => {
                vdata::edit_num(
                    edit,
                    *digits,
                    *scale,
                    *blank_zero,
                    *currency,
                    *point,
                    *d,
                    field,
                );
            }
            (lit::Const::Num(d, _), DataType::Float { .. }) => {
                crate::numeric::number::Number::decimal(
                    *d,
                    d.integer_digits() as i32,
                    d.scale as i32,
                )
                .store(ty, field, false)
                .map_err(|m| Diag::error(pos, m))?;
            }
            (lit::Const::Fig(_), DataType::Float { .. }) => field.fill(0),
            (lit::Const::Num(_, text), _) => vdata::move_alnum(text, field, false),
            (lit::Const::Fig(f), DataType::Numeric(t)) if *f == lit::FigByte::Zero => {
                vdata::store_num(t, field, Dec::ZERO, false);
            }
            (lit::Const::Fig(f), _) => field.fill(f.byte()),
            (lit::Const::All(p), _) => vdata::fill_pattern(field, p),
            (lit::Const::Bytes(b), DataType::Alphanumeric { .. } | DataType::Alphabetic { .. }) => {
                // VALUE is not affected by JUSTIFIED.
                vdata::move_alnum(b, field, false);
            }
            (lit::Const::Bytes(b), _) => vdata::move_alnum(b, field, false),
            (lit::Const::Null, _) => field.fill(0),
        }
        Ok(())
    }
}

fn default_value(ty: &DataType, field: &mut [u8]) {
    match ty {
        DataType::Numeric(t) => {
            vdata::store_num(t, field, Dec::ZERO, false);
        }
        ty if ty.is_national() => vdata::fill_pattern(field, &vdata::national_fig(ebcdic::SPACE)),
        DataType::Index
        | DataType::Pointer
        | DataType::ProcedurePointer
        | DataType::FunctionPointer
        | DataType::Float { .. } => field.fill(0),
        _ => field.fill(ebcdic::SPACE),
    }
}

/// Builds the data model of a program.
pub fn build(prog: &ast::Program, settings: &Settings) -> Result<DataModel> {
    for (name, pos) in &prog.env.multiple_file_tape {
        if !prog
            .env
            .files
            .iter()
            .any(|f| &f.name == name && matches!(f.organization, ast::Organization::Sequential))
            || !prog.data.files.iter().any(|f| &f.name == name && !f.sort)
        {
            return Err(Diag::error(
                *pos,
                "MULTIPLE FILE requires a locally declared QSAM file",
            ));
        }
    }
    for (groups, kind) in [
        (&prog.env.same_area, "AREA"),
        (&prog.env.same_record_area, "RECORD"),
        (&prog.env.same_sort_area, "SORT"),
    ] {
        let mut seen = std::collections::HashSet::new();
        for group in groups {
            let mut local = std::collections::HashSet::new();
            let mut has_sort = false;
            for name in group {
                let fd = prog
                    .data
                    .files
                    .iter()
                    .find(|f| &f.name == name)
                    .ok_or_else(|| {
                        Diag::error(prog.pos, format!("SAME {kind} file {name} is not declared"))
                    })?;
                if !prog.env.files.iter().any(|f| &f.name == name)
                    || !local.insert(name)
                    || fd.external
                    || (kind != "SORT" && (!seen.insert(name) || fd.sort))
                    || (kind == "SORT" && fd.sort && !seen.insert(name))
                {
                    return Err(Diag::error(
                        fd.pos,
                        "invalid, duplicate, external or sort file in SAME group",
                    ));
                }
                has_sort |= fd.sort;
                if kind == "RECORD"
                    && (fd.global
                        || fd.records.iter().any(|r| r.global)
                        || fd.record_min == Some(0))
                {
                    return Err(Diag::error(
                        fd.pos,
                        "SAME RECORD forbids GLOBAL and RECORD CONTAINS 0",
                    ));
                }
            }
            if kind == "SORT" && !has_sort {
                return Err(Diag::error(prog.pos, "SAME SORT requires a sort file"));
            }
        }
    }
    for area in &prog.env.same_area {
        for group in prog
            .env
            .same_record_area
            .iter()
            .chain(&prog.env.same_sort_area)
        {
            if area.iter().any(|f| group.contains(f)) && !area.iter().all(|f| group.contains(f)) {
                return Err(Diag::error(
                    prog.pos,
                    "overlapping SAME groups must include the complete SAME AREA group",
                ));
            }
        }
    }
    let mut external_names = std::collections::HashSet::new();
    for entry in &prog.data.working {
        if entry.external
            && (entry.level != 1
                || entry.name.is_none()
                || !external_names.insert(entry.name.clone()))
        {
            return Err(Diag::error(
                entry.pos,
                "EXTERNAL requires a unique named level 01 WORKING-STORAGE record",
            ));
        }
    }
    for entry in &prog.data.local {
        if entry.external {
            return Err(Diag::error(
                entry.pos,
                "LOCAL-STORAGE cannot specify EXTERNAL",
            ));
        }
    }
    for entry in prog
        .data
        .linkage
        .iter()
        .chain(prog.data.files.iter().flat_map(|f| &f.records))
    {
        if entry.external {
            return Err(Diag::error(
                entry.pos,
                "data EXTERNAL is permitted only in WORKING-STORAGE",
            ));
        }
    }
    let all_entries: Vec<_> = prog
        .data
        .working
        .iter()
        .chain(&prog.data.local)
        .chain(&prog.data.linkage)
        .chain(prog.data.files.iter().flat_map(|f| &f.records))
        .collect();
    for entry in &all_entries {
        if entry.global
            && (entry.level != 1
                || entry.name.is_none()
                || all_entries.iter().filter(|e| e.name == entry.name).count() > 1)
        {
            return Err(Diag::error(
                entry.pos,
                "GLOBAL requires a unique named level 01 item",
            ));
        }
    }
    let mut dm = DataModel {
        decimal_comma: settings.decimal_comma,
        currency: settings.currency,
        ..DataModel::default()
    };
    // File section: records of each FD share an area.
    let mut file_records = Vec::new();
    let mut label_records = Vec::new();
    for (fi, fd) in prog.data.files.iter().enumerate() {
        let recs = dm.build_tree(&fd.records, &mut |_| Area::File(fi))?;
        if fd.global {
            for &r in &recs {
                dm.items[r].global = true;
            }
            dm.global_files.push(fd.name.clone());
        }
        let (labels, data): (Vec<_>, Vec<_>) = recs.into_iter().partition(|&r| {
            dm.items[r]
                .name
                .as_ref()
                .is_some_and(|n| fd.label_records.contains(n))
        });
        if data.is_empty() && fd.reports.is_empty() {
            return Err(Diag::error(
                fd.pos,
                "FD requires a data record distinct from its labels",
            ));
        }
        for name in &fd.label_records {
            if !labels
                .iter()
                .any(|&r| dm.items[r].name.as_ref() == Some(name))
            {
                return Err(Diag::error(
                    fd.pos,
                    format!("label record {name} must be level 01 under its FD"),
                ));
            }
        }
        label_records.push(labels);
        file_records.push(data);
    }
    let mut working = prog.data.working.clone();
    working.extend(debug_item_entries());
    // Report groups are laid out as working-storage records; every entry
    // gets a unique name (see `report_item_name`).
    for (r, rd) in prog.data.reports.iter().enumerate() {
        for (i, e) in rd.entries.iter().enumerate() {
            let mut e = e.clone();
            e.name = Some(report_item_name(r, i));
            e.report = None;
            working.push(e);
        }
    }
    let ws = dm.build_tree(&working, &mut |_| Area::Working)?;
    for e in &prog.data.local {
        if e.external {
            return Err(Diag::error(e.pos, "LOCAL-STORAGE cannot specify EXTERNAL"));
        }
    }
    let ls = dm.build_tree(&prog.data.local, &mut |_| Area::Working)?;
    let mut n = 0u32;
    let lk = dm.build_tree(&prog.data.linkage, &mut |_| {
        n += 1;
        Area::Linkage(n - 1)
    })?;
    dm.linkage_count = n;
    // A linkage record redefining another shares its base address.
    for &r in &lk {
        if let Some(target) = dm.items[r].entry.redefines.clone()
            && let Some(&t) = lk
                .iter()
                .find(|&&x| dm.items[x].name.as_deref() == Some(&target))
        {
            let area = dm.items[t].area;
            dm.set_area(r, area);
        }
    }
    dm.analyze_pics(settings)?;

    // Layout: working-storage, then file record areas, each record aligned
    // to a doubleword.
    let mut cursor = 0u32;
    let place = |dm: &mut DataModel, records: &[usize], cursor: &mut u32| -> Result<()> {
        for &r in records {
            if let Some(target) = dm.items[r].entry.redefines.clone() {
                let t = records
                    .iter()
                    .copied()
                    .find(|&x| dm.items[x].name.as_deref() == Some(&target))
                    .ok_or_else(|| {
                        Diag::error(
                            dm.items[r].pos,
                            format!("REDEFINES target {target} not found"),
                        )
                    })?;
                let off = dm.items[t].offset;
                let size = dm.layout(r, off)?;
                *cursor = (*cursor).max(off + size);
            } else if dm.items[r].entry.external {
                let size = dm.layout(r, 0)?;
                let name = dm.items[r].name.clone().unwrap_or_default();
                let k = dm.external_index(&name, size);
                dm.set_area(r, Area::External(k));
            } else {
                *cursor = align(*cursor, 8);
                let size = dm.layout(r, *cursor)?;
                *cursor += size;
            }
        }
        Ok(())
    };
    place(&mut dm, &ws, &mut cursor)?;
    let local_start = cursor;
    place(&mut dm, &ls, &mut cursor)?;
    if cursor > local_start {
        dm.local_storage = Some((local_start, cursor - local_start));
    }
    // SAME RECORD AREA: the files of a group share one record area.
    cursor = align(cursor, 8);
    for labels in &label_records {
        for &r in labels {
            if dm.layout(r, cursor)? != 80 {
                return Err(Diag::error(
                    dm.items[r].pos,
                    "user label record must contain 80 bytes",
                ));
            }
        }
    }
    dm.label_roots = label_records.iter().flatten().copied().collect();
    if !dm.label_roots.is_empty() {
        cursor += 80;
    }
    let mut same_start: HashMap<usize, u32> = HashMap::new();
    let mut shared_groups = prog.env.same_record_area.clone();
    // Plain SAME AREA is documentation for QSAM, record sharing for VSAM.
    shared_groups.extend(prog.env.same_area.iter().map(|g| {
        g.iter()
            .filter(|n| {
                prog.env.files.iter().any(|f| {
                    &f.name == *n
                        && matches!(
                            f.organization,
                            ast::Organization::Indexed | ast::Organization::Relative
                        )
                })
            })
            .cloned()
            .collect::<Vec<_>>()
    }));
    for (fi, recs) in file_records.iter().enumerate() {
        let external = prog.data.files[fi].external;
        let fname = &prog.data.files[fi].name;
        let group = shared_groups.iter().position(|g| g.contains(fname));
        cursor = align(cursor, 8);
        let shared = group.and_then(|g| same_start.get(&g).copied());
        let start = if external {
            0
        } else if let Some(s) = shared {
            s
        } else {
            if let Some(g) = group {
                same_start.insert(g, cursor);
            }
            cursor
        };
        let mut max = 0;
        for &r in recs {
            let size = dm.layout(r, start)?;
            max = max.max(size);
        }
        let fd = &prog.data.files[fi];
        if let Some(m) = fd.record_max {
            max = max.max(m);
        }
        let area_kind = if external {
            let k = dm.external_index(&format!("FILE {}", fd.name), max.max(1));
            for &r in recs {
                dm.set_area(r, Area::External(k));
            }
            Area::External(k)
        } else {
            cursor = cursor.max(start + max.max(1));
            Area::File(fi)
        };
        dm.files.insert(fd.name.clone(), (usize::MAX, recs.clone()));
        // record area pseudo-item
        let area = Item {
            name: None,
            level: 1,
            pos: fd.pos,
            parent: None,
            children: Vec::new(),
            area: area_kind,
            offset: start,
            len: max.max(1),
            occurs: None,
            pic: None,
            usage: Usage::Display,
            entry: DataEntry::default(),
            conditions: Vec::new(),
            cond_of: None,
            desc: 0,
            index_of: None,
            owner: None,
            global: fd.global,
            renames: None,
        };
        let id = dm.items.len();
        dm.items.push(area);
        dm.files.get_mut(&fd.name).unwrap().0 = id;
    }
    for &r in &lk {
        dm.layout(r, 0)?;
    }
    // Index-names use the LP address width.
    let index_width = if crate::front::lp64() { 8 } else { 4 };
    let tables: Vec<usize> = (0..dm.items.len())
        .filter(|&i| {
            dm.items[i]
                .occurs
                .as_ref()
                .is_some_and(|o| !o.indexed_by.is_empty())
        })
        .collect();
    for t in tables {
        let names = dm.items[t].occurs.as_ref().unwrap().indexed_by.clone();
        for name in names {
            cursor = align(cursor, index_width);
            let mut e = DataEntry {
                level: 1,
                name: Some(name.clone()),
                usage: Some(Usage::Index),
                ..DataEntry::default()
            };
            e.pos = dm.items[t].pos;
            let mut it = DataModel::new_item(&e, Area::Special);
            it.offset = cursor;
            it.len = index_width;
            it.usage = Usage::Index;
            it.index_of = Some(t);
            dm.add(it);
            cursor += index_width;
        }
    }
    // LINE-COUNTER and PAGE-COUNTER of each report.
    for rd in &prog.data.reports {
        let mut ids = [0usize; 2];
        for (k, name) in ["LINE-COUNTER", "PAGE-COUNTER"].into_iter().enumerate() {
            cursor = align(cursor, 4);
            let e = DataEntry {
                level: 1,
                name: Some(name.to_string()),
                picture: Some("9(9)".to_string()),
                usage: Some(Usage::Binary),
                pos: rd.pos,
                ..DataEntry::default()
            };
            let mut it = DataModel::new_item(&e, Area::Special);
            it.pic = Some(picture::analyze("9(9)", false, '$').unwrap());
            it.offset = cursor;
            it.len = 4;
            cursor += 4;
            ids[k] = dm.items.len();
            dm.add(it);
        }
        dm.report_counters.insert(rd.name.clone(), (ids[0], ids[1]));
    }
    // LINAGE-COUNTER of each file with LINAGE.
    for fd in &prog.data.files {
        if fd.linage.is_none() {
            continue;
        }
        cursor = align(cursor, 4);
        let e = DataEntry {
            level: 1,
            name: Some("LINAGE-COUNTER".to_string()),
            picture: Some("9(9)".to_string()),
            usage: Some(Usage::Binary),
            pos: fd.pos,
            ..DataEntry::default()
        };
        let mut it = DataModel::new_item(&e, Area::Special);
        it.pic = Some(picture::analyze("9(9)", false, '$').unwrap());
        it.offset = cursor;
        it.len = 4;
        cursor += 4;
        let id = dm.items.len();
        dm.add(it);
        dm.linage_counters.insert(fd.name.clone(), id);
    }
    // Special registers.
    for (name, pic, usage) in [
        ("RETURN-CODE", "S9(4)", Usage::Binary),
        ("TALLY", "9(5)", Usage::Binary),
        ("SORT-RETURN", "S9(4)", Usage::Binary),
        ("SORT-CORE-SIZE", "S9(8)", Usage::Binary),
        ("SORT-FILE-SIZE", "S9(8)", Usage::Binary),
        ("SORT-MODE-SIZE", "S9(5)", Usage::Binary),
        ("SORT-MESSAGE", "X(8)", Usage::Display),
        ("SORT-CONTROL", "X(8)", Usage::Display),
    ] {
        cursor = align(cursor, 4);
        let e = DataEntry {
            level: 1,
            name: Some(name.to_string()),
            picture: Some(pic.to_string()),
            usage: Some(usage),
            global: name.starts_with("SORT-"),
            ..DataEntry::default()
        };
        let mut it = DataModel::new_item(&e, Area::Special);
        it.pic = Some(picture::analyze(pic, false, '$').unwrap());
        it.offset = cursor;
        it.len = 2;
        if name == "TALLY" || matches!(name, "SORT-CORE-SIZE" | "SORT-FILE-SIZE" | "SORT-MODE-SIZE")
        {
            it.len = 4;
        }
        if usage == Usage::Display {
            it.len = 8;
        }
        cursor += it.len;
        dm.add(it);
    }
    // RENAMES (level 66).
    for i in 0..dm.items.len() {
        if dm.items[i].level != 66 {
            continue;
        }
        let Some((a, b)) = dm.items[i].entry.renames.clone() else {
            continue;
        };
        let record = dm.items[i].parent.unwrap();
        let first = dm.resolve_in_record(&a, record)?;
        let last = match &b {
            Some(b) => dm.resolve_in_record(b, record)?,
            None => first,
        };
        let start = dm.items[first].offset;
        let end = dm.items[last].offset + dm.items[last].len;
        dm.items[i].offset = start;
        dm.items[i].len = end.saturating_sub(start);
        if b.is_none() {
            // renames a single item: same type
            let src = dm.items[first].clone();
            dm.items[i].pic = src.pic;
            dm.items[i].usage = src.usage;
            dm.items[i].entry.picture = src.entry.picture;
            dm.items[i].children = Vec::new();
            if !src.children.is_empty() {
                dm.items[i].renames = Some((first, last));
            }
        } else {
            dm.items[i].renames = Some((first, last));
        }
        dm.items[i].occurs = None;
    }
    for i in 0..dm.items.len() {
        if dm.items[i].cond_of.is_none() {
            dm.make_desc(i);
        }
    }
    // Groups containing an OCCURS DEPENDING ON table have a variable length.
    for i in 0..dm.items.len() {
        if dm.items[i].cond_of.is_some() || dm.items[i].children.is_empty() {
            continue;
        }
        if let Some(t) = dm.find_odo(i) {
            let dep = dm.items[t]
                .occurs
                .as_ref()
                .unwrap()
                .depending
                .clone()
                .unwrap();
            let dep_id = dm.resolve(&dep)?;
            let odo = Odo {
                fixed: dm.items[t].offset - dm.items[i].offset,
                stride: dm.items[t].len,
                depending: Box::new(Loc {
                    desc: dm.items[dep_id].desc,
                    subs: Vec::new(),
                    refmod: None,
                }),
            };
            let d = dm.items[i].desc as usize;
            dm.descs[d].odo = Some(odo);
        }
    }
    // Initial values.
    let mut image = vec![0u8; cursor as usize];
    for &r in ws.iter().chain(ls.iter()) {
        // A redefining record shares the storage of the redefined one.
        if dm.items[r].entry.redefines.is_none() {
            dm.init_values(r, 0, &mut image)?;
        }
    }
    for recs in file_records.iter().chain(&label_records) {
        for &r in recs {
            if dm.items[r].entry.redefines.is_none() {
                // record areas start as spaces
                let it = &dm.items[r];
                let (s, e) = (it.offset as usize, (it.offset + it.len) as usize);
                if image.len() < e {
                    image.resize(e, 0);
                }
                image[s..e].fill(ebcdic::SPACE);
            }
        }
    }
    for i in 0..dm.items.len() {
        if matches!(
            dm.items[i].name.as_deref(),
            Some("SORT-MESSAGE" | "SORT-CONTROL")
        ) {
            let it = &dm.items[i];
            let value = if it.name.as_deref() == Some("SORT-MESSAGE") {
                "SYSOUT  "
            } else {
                "IGZSRTCD"
            };
            image[it.offset as usize..(it.offset + it.len) as usize]
                .copy_from_slice(&crate::codepage::active().encode(value));
        }
        if dm.items[i].area == Area::Special && dm.items[i].usage == Usage::Index {
            let off = dm.items[i].offset as usize;
            let len = dm.items[i].len as usize;
            binary::write(&mut image[off..off + len], 1);
        }
    }
    image.resize(align(cursor.max(image.len() as u32), 8) as usize, 0);
    dm.static_init = image;
    dm.dynamic_layout()?;
    // Named report items (sum counters) can be referenced by their names.
    for (r, rd) in prog.data.reports.iter().enumerate() {
        for (i, e) in rd.entries.iter().enumerate() {
            if let Some(n) = &e.name
                && let Some(&id) = dm
                    .by_name
                    .get(&report_item_name(r, i))
                    .and_then(|v| v.first())
            {
                dm.by_name.entry(n.clone()).or_default().push(id);
            }
        }
    }
    Ok(dm)
}

impl DataModel {
    /// The OCCURS DEPENDING ON table ending a group, if any.
    /// The smallest length of a record: with its OCCURS DEPENDING ON
    /// table at the minimum number of occurrences.
    pub(crate) fn min_record_len(&self, id: usize) -> u32 {
        let item = &self.items[id];
        match self.find_odo(id) {
            Some(t) => {
                let table = &self.items[t];
                let min = table.occurs.as_ref().map_or(0, |o| o.min);
                table.offset - item.offset + min * table.len
            }
            None => item.len,
        }
    }

    fn find_odo(&self, id: usize) -> Option<usize> {
        let last = *self.items[id]
            .children
            .iter()
            .rev()
            .find(|&&c| self.items[c].entry.redefines.is_none())?;
        if self.items[last]
            .occurs
            .as_ref()
            .is_some_and(|o| o.depending.is_some())
        {
            return Some(last);
        }
        if self.items[last].occurs.is_none() {
            return self.find_odo(last);
        }
        None
    }

    fn resolve_in_record(&self, r: &DataRef, record: usize) -> Result<usize> {
        let cands = self.by_name.get(&r.name).cloned().unwrap_or_default();
        cands
            .into_iter()
            .find(|&c| self.record_of(c) == record && self.items[c].level != 66)
            .ok_or_else(|| Diag::error(r.pos, format!("{} not found for RENAMES", r.name)))
    }

    pub fn record_of(&self, mut id: usize) -> usize {
        while let Some(p) = self.items[id].parent {
            id = p;
        }
        id
    }

    /// Whether `ancestor` qualifies `id` (is an ancestor or its file).
    fn qualified_by(&self, id: usize, quals: &[String]) -> bool {
        let mut cur = self.items[id].parent;
        let mut q = 0;
        while q < quals.len() {
            match cur {
                Some(c) => {
                    if self.items[c].name.as_deref() == Some(quals[q].as_str()) {
                        q += 1;
                    }
                    cur = self.items[c].parent;
                }
                None => {
                    // the remaining qualifier may be the file name
                    let rec = self.record_of(id);
                    if let Area::File(_) = self.items[rec].area
                        && q + 1 == quals.len()
                        && self.files.get(&quals[q]).is_some_and(|(area, recs)| {
                            recs.contains(&rec) || self.items[*area].area == self.items[rec].area
                        })
                    {
                        return true;
                    }
                    return false;
                }
            }
        }
        true
    }

    /// Resolves a (possibly qualified) data name to an item.
    pub fn resolve(&self, r: &DataRef) -> Result<usize> {
        if r.name == "LINAGE-COUNTER"
            && let Some(q) = r.qualifiers.first()
            && let Some(&id) = self.linage_counters.get(q)
        {
            return Ok(id);
        }
        if (r.name == "LINE-COUNTER" || r.name == "PAGE-COUNTER")
            && let Some(q) = r.qualifiers.first()
            && let Some(&(lc, pc)) = self.report_counters.get(q)
        {
            return Ok(if r.name == "LINE-COUNTER" { lc } else { pc });
        }
        let cands: Vec<usize> = self
            .by_name
            .get(&r.name)
            .map(|v| {
                v.iter()
                    .copied()
                    .filter(|&c| self.qualified_by(c, &r.qualifiers))
                    .collect()
            })
            .unwrap_or_default();
        // Names of the program itself hide GLOBAL names of containing ones.
        let own: Vec<usize> = cands
            .iter()
            .copied()
            .filter(|&c| self.items[c].owner.is_none())
            .collect();
        let cands = if own.is_empty() {
            // the innermost containing program wins
            let max_owner = cands.iter().filter_map(|&c| self.items[c].owner).max();
            cands
                .into_iter()
                .filter(|&c| self.items[c].owner == max_owner)
                .collect()
        } else {
            own
        };
        if self.label_scope && r.qualifiers.is_empty() && cands.len() > 1 {
            let root = |mut id: usize| {
                while let Some(parent) = self.items[id].parent {
                    id = parent;
                }
                id
            };
            let first = &self.descs[self.items[cands[0]].desc as usize];
            if cands.len() == self.label_roots.len()
                && cands.iter().all(|&id| {
                    let item = &self.items[id];
                    let desc = &self.descs[item.desc as usize];
                    item.children.is_empty()
                        && item.cond_of.is_none()
                        && self.label_roots.contains(&root(id))
                        && desc.offset == first.offset
                        && desc.len == first.len
                        && desc.ty == first.ty
                        && desc.abi == first.abi
                })
            {
                return Ok(cands[0]);
            }
        }
        match cands.len() {
            0 => Err(Diag::error(
                r.pos,
                format!("{} is not defined", display_name(r)),
            )),
            1 => Ok(cands[0]),
            _ => {
                // An unqualified reference to a record-level name wins.
                let top: Vec<usize> = cands
                    .iter()
                    .copied()
                    .filter(|&c| self.items[c].parent.is_none())
                    .collect();
                if top.len() == 1 {
                    return Ok(top[0]);
                }
                Err(Diag::error(
                    r.pos,
                    format!("{} is ambiguous", display_name(r)),
                ))
            }
        }
    }
}

fn display_name(r: &DataRef) -> String {
    let mut s = r.name.clone();
    for q in &r.qualifiers {
        s.push_str(" OF ");
        s.push_str(q);
    }
    s
}

/// Length of DEBUG-CONTENTS.
pub const DEBUG_CONTENTS_LEN: u32 = 1024;

/// The DEBUG-ITEM special register:
/// DEBUG-LINE X(6), DEBUG-NAME X(30), DEBUG-SUB-1..3 S9999 SIGN LEADING
/// SEPARATE and DEBUG-CONTENTS, separated by one-character fillers.
fn debug_item_entries() -> Vec<DataEntry> {
    let e = |level: u8, name: Option<&str>, pic: Option<&str>| DataEntry {
        level,
        name: name.map(str::to_string),
        picture: pic.map(str::to_string),
        ..DataEntry::default()
    };
    let sub = |n: u32| DataEntry {
        sign: Some((true, true)),
        ..e(2, Some(&format!("DEBUG-SUB-{n}")), Some("S9999"))
    };
    let contents = format!("X({DEBUG_CONTENTS_LEN})");
    vec![
        e(1, Some("DEBUG-ITEM"), None),
        e(2, Some("DEBUG-LINE"), Some("X(6)")),
        e(2, None, Some("X")),
        e(2, Some("DEBUG-NAME"), Some("X(30)")),
        e(2, None, Some("X")),
        sub(1),
        e(2, None, Some("X")),
        sub(2),
        e(2, None, Some("X")),
        sub(3),
        e(2, None, Some("X")),
        e(2, Some("DEBUG-CONTENTS"), Some(&contents)),
    ]
}

/// Data name under which entry `i` of report `r` is laid out.
pub fn report_item_name(r: usize, i: usize) -> String {
    format!("RW#{r}#{i}")
}

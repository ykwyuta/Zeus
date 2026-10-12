//! Declarations: attributes and defaults, structure mapping, storage
//! classes and INITIAL.

use super::*;
use crate::ast::{Bound as AstBound, Extent, Init as AstInit};
use crate::types::{Ext, Var as Varying};

/// A declared item with its members (after LIKE expansion).
struct Node {
    item: DeclItem,
    children: Vec<Node>,
}

/// Mapping unit: length, alignment and offset from a doubleword boundary
/// (bits).
#[derive(Debug, Clone, Copy)]
struct Unit {
    len: u64,
    align: u64,
    off: u64,
}

fn has(attrs: &[Attr], f: impl Fn(&Attr) -> bool) -> bool {
    attrs.iter().any(f)
}

impl Compiler {
    /// Value of a restricted integer expression (constants, named
    /// constants, + - * /).
    pub fn const_int(&self, e: &ast::Expr) -> Option<i64> {
        match e {
            ast::Expr::Num {
                text,
                binary: false,
                ..
            } if text.chars().all(|c| c.is_ascii_digit()) => text.parse().ok(),
            ast::Expr::Num {
                text, binary: true, ..
            } if !text.contains('.') => i64::from_str_radix(text, 2).ok(),
            ast::Expr::Unary("-", x, _) => self.const_int(x).map(|v| -v),
            ast::Expr::Unary("+", x, _) => self.const_int(x),
            ast::Expr::Binary(op, a, b, _) => {
                let (a, b) = (self.const_int(a)?, self.const_int(b)?);
                match *op {
                    "+" => Some(a + b),
                    "-" => Some(a - b),
                    "*" => Some(a * b),
                    "/" if b != 0 => Some(a / b),
                    _ => None,
                }
            }
            ast::Expr::Ref(r)
                if r.parts.len() == 1 && r.parts[0].1.is_empty() && r.locator.is_none() =>
            {
                for (_, s) in self.lookup(&r.parts[0].0) {
                    match s {
                        Sym::Const(x) => {
                            if let Ek::Fixed(m, q) = &*x.k {
                                let d = 10i128.pow((*q).max(0) as u32);
                                return Some((m / d) as i64);
                            }
                            return None;
                        }
                        Sym::Var(_) | Sym::Label(_) => return None,
                        _ => {}
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn extent(&mut self, e: &Extent, block: BlockId, dyn_ok: bool, pos: Pos) -> Result<Ext> {
        match e {
            Extent::Star => Ok(Ext::Star),
            Extent::Refer(x, r) => {
                let _ = block;
                let n = self.m.refers.len() as u32;
                self.m.refers.push(Refer {
                    object: VarId::MAX,
                    init: super::expr::mk(Ek::Fixed(0, 0), Ty::fixed_bin(31, 0)),
                });
                self.late_refers.push((n, x.clone()));
                let name = r.parts.last().map(|p| p.0.clone()).unwrap_or_default();
                self.pending_refers.push((n, name, pos));
                Ok(Ext::Refer(n))
            }
            Extent::Expr(x) => match self.const_int(x) {
                Some(n) => Ok(Ext::Const(n.max(0) as u32)),
                None if dyn_ok => {
                    let save = self.cur;
                    self.cur = block;
                    let ex = self.expr(x)?;
                    let ex = self.conv(ex, &Ty::fixed_bin(31, 0));
                    self.cur = save;
                    let b = &mut self.m.blocks[block as usize];
                    b.extents.push(ex);
                    Ok(Ext::Slot((b.extents.len() - 1) as u32))
                }
                None => Err(self.err(pos, "extent must be a constant")),
            },
        }
    }

    fn bound(&mut self, e: &Extent, block: BlockId, dyn_ok: bool, pos: Pos) -> Result<Bound> {
        Ok(match self.extent_signed(e, block, dyn_ok, pos)? {
            Some(b) => b,
            None => Bound::Star,
        })
    }

    fn extent_signed(
        &mut self,
        e: &Extent,
        block: BlockId,
        dyn_ok: bool,
        pos: Pos,
    ) -> Result<Option<Bound>> {
        match e {
            Extent::Star => Ok(None),
            Extent::Refer(..) => match self.extent(e, block, dyn_ok, pos)? {
                Ext::Refer(n) => Ok(Some(Bound::Refer(n))),
                _ => Ok(None),
            },
            Extent::Expr(x) => match self.const_int(x) {
                Some(n) => Ok(Some(Bound::Const(n))),
                None => match self.extent(e, block, dyn_ok, pos)? {
                    Ext::Slot(s) => Ok(Some(Bound::Slot(s))),
                    _ => Ok(None),
                },
            },
        }
    }

    /// The scalar data type of a list of attributes (with the language
    /// defaults for missing ones).
    pub fn scalar_type(&mut self, attrs: &[Attr], name: &str, pos: Pos) -> Result<Ty> {
        let block = self.cur;
        self.scalar_type_in(attrs, name, pos, block, false)
    }

    fn scalar_type_in(
        &mut self,
        attrs: &[Attr],
        name: &str,
        pos: Pos,
        block: BlockId,
        dyn_ok: bool,
    ) -> Result<Ty> {
        let mut fixed = false;
        let mut float = false;
        let mut bin = false;
        let mut dec = false;
        let mut prec: Option<(i64, Option<i64>)> = None;
        let mut var = Varying::Fixed;
        let mut unsigned = false;
        let mut string: Option<(bool, Ext)> = None;
        for a in attrs {
            match a {
                Attr::Fixed => fixed = true,
                Attr::Float => float = true,
                Attr::Binary => bin = true,
                Attr::Decimal => dec = true,
                Attr::Precision(p, q) => {
                    let p = self
                        .const_int(p)
                        .ok_or_else(|| self.err(pos, "precision must be a constant"))?;
                    let q = match q {
                        Some(q) => Some(
                            self.const_int(q)
                                .ok_or_else(|| self.err(pos, "scale must be a constant"))?,
                        ),
                        None => None,
                    };
                    prec = Some((p, q));
                }
                Attr::Unsigned => unsigned = true,
                Attr::Varying => var = Varying::Varying,
                Attr::VaryingZ => var = Varying::VaryingZ,
                Attr::Complex => return Err(self.err(pos, "COMPLEX data is not supported")),
                Attr::Character(l) | Attr::Bit(l) => {
                    let len = match l {
                        Some(e) => self.extent(e, block, dyn_ok, pos)?,
                        None => Ext::Const(1),
                    };
                    string = Some((matches!(a, Attr::Bit(_)), len));
                }
                Attr::Graphic(_) | Attr::Widechar(_) => {
                    return Err(self.err(pos, "GRAPHIC and WIDECHAR data are not supported"));
                }
                Attr::Picture(s) => {
                    let p = crate::picture::Picture::parse(s).map_err(|e| self.err(pos, e))?;
                    return Ok(Ty::Pic(Box::new(p)));
                }
                Attr::Pointer | Attr::Handle => return Ok(Ty::Pointer),
                Attr::Offset => return Ok(Ty::Offset),
                Attr::Label => return Ok(Ty::Label),
                Attr::Format => return Ok(Ty::Format),
                Attr::Area(_) => return Ok(Ty::Area),
                Attr::Entry(_) => return Ok(Ty::Entry),
                Attr::File => return Ok(Ty::File),
                _ => {}
            }
        }
        if let Some((bit, len)) = string {
            return Ok(if bit {
                let aligned = has(attrs, |a| matches!(a, Attr::Aligned));
                Ty::Bit { len, var, aligned }
            } else {
                Ty::Char { len, var }
            });
        }
        let any = fixed || float || bin || dec || prec.is_some();
        if !any {
            // first letter I-N: FIXED BINARY(15), else FLOAT DECIMAL(6)
            let c = name.chars().next().unwrap_or('A');
            return Ok(if ('I'..='N').contains(&c) {
                Ty::fixed_bin(15, 0)
            } else {
                Ty::Float { dec: true, p: 6 }
            });
        }
        // a scale factor implies FIXED; missing scale FLOAT, base DECIMAL
        let fixed = fixed || (!float && prec.is_some_and(|(_, q)| q.is_some()));
        let dec = dec || !bin;
        if fixed {
            let (p, q) = match prec {
                Some((p, q)) => (p, q.unwrap_or(0)),
                None => (if dec { 5 } else { 15 }, 0),
            };
            let max = if dec { 31 } else { 63 };
            if p < 1 || p > max {
                return Err(self.err(pos, format!("precision {p} is out of range")));
            }
            Ok(Ty::Fixed {
                dec,
                p: p as u8,
                q: q.clamp(-128, 127) as i16,
                unsigned: unsigned && !dec,
            })
        } else {
            let p = match prec {
                Some((p, _)) => p,
                None => {
                    if dec {
                        6
                    } else {
                        21
                    }
                }
            };
            Ok(Ty::Float {
                dec,
                p: p.clamp(1, 109) as u8,
            })
        }
    }

    /// Expands LIKE attributes: the members of the referenced structure
    /// follow the item.
    fn expand_like(&self, items: &[DeclItem]) -> Result<Vec<DeclItem>> {
        let mut out = Vec::new();
        for it in items {
            let like = it.attrs.iter().find_map(|a| match a {
                Attr::Like(r) => Some(r.clone()),
                _ => None,
            });
            let mut it2 = it.clone();
            it2.attrs.retain(|a| !matches!(a, Attr::Like(_)));
            out.push(it2);
            let Some(r) = like else { continue };
            let target = self
                .lookup(r.name())
                .into_iter()
                .find_map(|(_, s)| match s {
                    Sym::Var(v) => Some(v),
                    _ => None,
                })
                .ok_or_else(|| self.err(r.pos, format!("LIKE: {} is not declared", r.name())))?;
            let root = self.m.vars[target as usize].root;
            let decl = self
                .decl_ast
                .get(&root)
                .ok_or_else(|| self.err(r.pos, format!("LIKE: {} is not a structure", r.name())))?;
            // the subtree of the target within its declaration
            let tname = &self.m.vars[target as usize].name;
            let start = decl
                .iter()
                .position(|d| &d.name == tname && d.level == self.m.vars[target as usize].level)
                .unwrap_or(0);
            let base = decl[start].level;
            for d in decl[start + 1..].iter().take_while(|d| d.level > base) {
                let mut c = d.clone();
                c.level = it.level + (d.level - base);
                out.push(c);
            }
        }
        Ok(out)
    }

    /// Declares the items of a DECLARE (or several) in a block.
    pub fn declare_items(
        &mut self,
        items: &[DeclItem],
        block: BlockId,
        params: &[String],
    ) -> Result<()> {
        let save = self.cur;
        self.cur = block;
        let r = self.declare_items_inner(items, block, params);
        self.cur = save;
        r
    }

    fn declare_items_inner(
        &mut self,
        items: &[DeclItem],
        block: BlockId,
        params: &[String],
    ) -> Result<()> {
        // trees by level
        let mut roots: Vec<Node> = Vec::new();
        // declarations are processed one level-1 tree at a time so that
        // LIKE can refer to earlier structures
        let mut groups: Vec<Vec<DeclItem>> = Vec::new();
        for it in items {
            if it.level <= 1 || groups.is_empty() {
                groups.push(vec![it.clone()]);
            } else {
                groups.last_mut().unwrap().push(it.clone());
            }
        }
        let mut late: Vec<(VarId, Vec<Attr>, Pos)> = Vec::new();
        for g in groups {
            let g = self.expand_like(&g)?;
            let tree = build_tree(&g);
            for t in tree {
                let vars = self.declare_tree(&t, block, params, &mut late, &g)?;
                let _ = vars;
                roots.push(t);
            }
        }
        // locators, DEFINED bases and INITIAL values (they may refer to
        // names declared later in the block)
        for (v, attrs, pos) in late {
            self.late_attrs(v, &attrs, pos)?;
        }
        for (n, x) in std::mem::take(&mut self.late_refers) {
            let ex = self.expr(&x)?;
            self.m.refers[n as usize].init = self.conv(ex, &Ty::fixed_bin(31, 0));
        }
        Ok(())
    }

    fn declare_tree(
        &mut self,
        t: &Node,
        block: BlockId,
        params: &[String],
        late: &mut Vec<(VarId, Vec<Attr>, Pos)>,
        group: &[DeclItem],
    ) -> Result<Option<VarId>> {
        let it = &t.item;
        let attrs = &it.attrs;
        let pos = it.pos;
        let is_param = params.contains(&it.name);
        if !is_param && t.children.is_empty() {
            // non-variable declarations
            if has(attrs, |a| matches!(a, Attr::Builtin)) {
                if let Some(b) = crate::builtin::lookup(&it.name) {
                    self.define(block, &it.name, Sym::Builtin(b));
                }
                return Ok(None);
            }
            if has(attrs, |a| matches!(a, Attr::Condition)) {
                self.m.conditions.push(it.name.clone());
                let id = (self.m.conditions.len() - 1) as u32;
                self.define(block, &it.name, Sym::Cond(id));
                return Ok(None);
            }
            let variable = has(attrs, |a| {
                matches!(
                    a,
                    Attr::Static
                        | Attr::Automatic
                        | Attr::Based(_)
                        | Attr::Controlled
                        | Attr::Initial(_)
                        | Attr::Aligned
                        | Attr::Unaligned
                )
            }) || !it.dims.is_empty();
            if has(attrs, |a| matches!(a, Attr::File)) && !variable {
                self.declare_file(&it.name, attrs, block);
                return Ok(None);
            }
            if has(attrs, |a| matches!(a, Attr::Entry(_) | Attr::Returns(_))) && !variable {
                let mut params_d = None;
                let mut returns = None;
                let mut ext = it.name.clone();
                for a in attrs {
                    match a {
                        Attr::Entry(ps) => {
                            if !ps.is_empty() || params_d.is_none() {
                                let mut v = Vec::new();
                                for p in ps {
                                    if p.is_empty() {
                                        v.push(None);
                                    } else {
                                        let has_data = p.iter().any(|a| {
                                            !matches!(
                                                a,
                                                Attr::Byaddr
                                                    | Attr::Byvalue
                                                    | Attr::Optional
                                                    | Attr::Aligned
                                                    | Attr::Unaligned
                                                    | Attr::Connected
                                                    | Attr::Assignable
                                                    | Attr::NonAssignable
                                                    | Attr::Dimension(_)
                                            )
                                        });
                                        let is_aggr =
                                            p.iter().any(|a| matches!(a, Attr::Dimension(_)));
                                        if has_data && !is_aggr {
                                            v.push(Some(
                                                self.scalar_type_in(p, "X", pos, block, false)
                                                    .unwrap_or(Ty::Entry),
                                            ));
                                        } else {
                                            v.push(None);
                                        }
                                    }
                                }
                                params_d = if ps.is_empty() { None } else { Some(v) };
                            }
                        }
                        Attr::Returns(r) => {
                            returns = Some(self.scalar_type_in(r, "R", pos, block, false)?)
                        }
                        Attr::External(Some(n)) => ext = n.clone(),
                        _ => {}
                    }
                }
                self.define(
                    block,
                    &it.name,
                    Sym::EntryDecl(EntryDecl {
                        name: ext,
                        params: params_d,
                        returns,
                    }),
                );
                return Ok(None);
            }
            if let Some(v) = attrs.iter().find_map(|a| match a {
                Attr::Value(e) => Some(e.clone()),
                _ => None,
            }) {
                let x = self.expr(&v)?;
                let ty = if has(attrs, |a| {
                    matches!(
                        a,
                        Attr::Fixed
                            | Attr::Float
                            | Attr::Binary
                            | Attr::Decimal
                            | Attr::Character(_)
                            | Attr::Bit(_)
                            | Attr::Precision(..)
                    )
                }) {
                    self.scalar_type_in(attrs, &it.name, pos, block, false)?
                } else {
                    x.ty.clone()
                };
                let x = self.conv(x, &ty);
                // keep constant values foldable
                let x = fold_const(x);
                self.define(block, &it.name, Sym::Const(x));
                return Ok(None);
            }
        }
        // a variable (or structure)
        let external = has(attrs, |a| matches!(a, Attr::External(_)));
        let class = if is_param {
            0
        } else if has(attrs, |a| matches!(a, Attr::Based(_))) {
            2
        } else if has(attrs, |a| matches!(a, Attr::Defined(_))) {
            3
        } else if has(attrs, |a| matches!(a, Attr::Controlled)) {
            4
        } else if has(attrs, |a| matches!(a, Attr::Static))
            || external
            || self.m.blocks[block as usize].proc.is_none()
                && self.m.blocks[block as usize].name == "*PACKAGE"
        {
            1
        } else {
            5
        };
        let n_ext0 = self.m.blocks[block as usize].extents.len();
        self.pending_refers.clear();
        let dyn_ok = (class == 5 || class == 4) && t.children.is_empty();
        let root = self.m.vars.len() as VarId;
        let mut members = Vec::new();
        let unit = self.map_node(
            t,
            block,
            root,
            None,
            Vec::new(),
            None,
            dyn_ok || class == 0,
            &mut members,
            late,
        )?;
        // REFER objects: members of the same structure
        for (n, name, p) in std::mem::take(&mut self.pending_refers) {
            if class != 2 {
                return Err(self.err(p, "REFER is allowed only in a BASED structure"));
            }
            let obj = (root..self.m.vars.len() as VarId).find(|&v| {
                let x = &self.m.vars[v as usize];
                x.name == name && x.ty.is_some() && x.dims.is_empty()
            });
            let Some(obj) = obj else {
                return Err(self.err(
                    p,
                    format!("REFER object {name} is not a scalar member of the structure"),
                ));
            };
            self.m.refers[n as usize].object = obj;
        }
        let controlled = if class == 4 {
            Some(self.controlled_extents(root, block, n_ext0, pos)?)
        } else {
            None
        };
        let size_bytes = unit.len.div_ceil(8) as u32;
        let align = (unit.align / 8).max(1) as u32;
        let residue = (unit.off / 8) as u32 % 8;
        let has_slots = self.var_has_slots(root);
        let storage = match class {
            0 => {
                let k = params.iter().position(|p| *p == it.name).unwrap_or(0) as u32;
                Storage::Param(k)
            }
            1 => {
                if external && let Some(&other) = self.externals.get(&it.name) {
                    match &self.m.vars[other as usize].storage {
                        Storage::Static(o) => Storage::Static(*o),
                        _ => Storage::Static(self.alloc_static(size_bytes, align, residue)),
                    }
                } else {
                    Storage::Static(self.alloc_static(size_bytes, align, residue))
                }
            }
            2 => Storage::Based(None),
            3 => Storage::Defined(
                Box::new(Ref {
                    var: 0,
                    subs: Vec::new(),
                    locator: None,
                }),
                None,
            ),
            4 => {
                let shared = if external {
                    self.externals.get(&it.name).and_then(|&o| {
                        match &self.m.vars[o as usize].storage {
                            Storage::Controlled(a, _) => Some(*a),
                            _ => None,
                        }
                    })
                } else {
                    None
                };
                let anchor = shared.unwrap_or_else(|| self.alloc_static(4, 4, 0));
                Storage::Controlled(anchor, controlled.unwrap_or_default())
            }
            _ if has_slots => Storage::AutoDyn(self.alloc_auto(block, 4, 4, 0)),
            _ => Storage::Auto(self.alloc_auto(block, size_bytes, align.min(8), residue)),
        };
        self.m.vars[root as usize].storage = storage;
        if external {
            self.externals.entry(it.name.clone()).or_insert(root);
        }
        self.decl_ast.insert(root, group.to_vec());
        Ok(Some(root))
    }

    fn var_has_slots(&self, v: VarId) -> bool {
        let var = &self.m.vars[v as usize];
        var.dims
            .iter()
            .any(|d| matches!(d.lb, Bound::Slot(_)) || matches!(d.ub, Bound::Slot(_)))
            || matches!(
                var.ty.as_ref().and_then(|t| t.str_len()),
                Some(Ext::Slot(_))
            )
    }

    /// Maps a node: declares its variable (and members) and returns its
    /// mapping unit.
    /// Moves the adjustable and `*` extents of a CONTROLLED variable from
    /// the block's entry slots to its generation slots.
    fn controlled_extents(
        &mut self,
        root: VarId,
        block: BlockId,
        n_ext0: usize,
        pos: Pos,
    ) -> Result<Vec<Option<Expr>>> {
        let moved = self.m.blocks[block as usize].extents.split_off(n_ext0);
        if !self.m.vars[root as usize].members.is_empty() {
            if !moved.is_empty() {
                return Err(self.err(pos, "extents of a CONTROLLED structure must be constants"));
            }
            return Ok(Vec::new());
        }
        let mut out: Vec<Option<Expr>> = Vec::new();
        let mut slot = |e: Option<Expr>| {
            out.push(e);
            (out.len() - 1) as u32
        };
        let var = self.m.vars[root as usize].clone();
        let mut dims = var.dims.clone();
        for d in &mut dims {
            for b in [&mut d.lb, &mut d.ub] {
                match *b {
                    Bound::Slot(s) => {
                        *b = Bound::Slot(slot(Some(moved[s as usize - n_ext0].clone())))
                    }
                    Bound::Star => *b = Bound::Slot(slot(None)),
                    _ => {}
                }
            }
        }
        let ty = var.ty.as_ref().map(|t| match t.str_len() {
            Some(Ext::Slot(s)) => {
                t.with_len(Ext::Slot(slot(Some(moved[s as usize - n_ext0].clone()))))
            }
            Some(Ext::Star) if !matches!(t, Ty::Pic(_)) => t.with_len(Ext::Slot(slot(None))),
            _ => t.clone(),
        });
        let v = &mut self.m.vars[root as usize];
        v.dims = dims;
        v.ty = ty;
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    fn map_node(
        &mut self,
        t: &Node,
        block: BlockId,
        root: VarId,
        parent: Option<VarId>,
        parent_dims: Vec<Dim>,
        inherited_align: Option<bool>,
        dyn_ok: bool,
        members: &mut Vec<VarId>,
        late: &mut Vec<(VarId, Vec<Attr>, Pos)>,
    ) -> Result<Unit> {
        let it = &t.item;
        let attrs = &it.attrs;
        let explicit_align = if has(attrs, |a| matches!(a, Attr::Aligned)) {
            Some(true)
        } else if has(attrs, |a| matches!(a, Attr::Unaligned)) {
            Some(false)
        } else {
            inherited_align
        };
        let id = self.m.vars.len() as VarId;
        self.m.vars.push(Var {
            name: it.name.clone(),
            block,
            storage: Storage::Member,
            root,
            parent,
            members: Vec::new(),
            bit_offset: 0,
            ty: None,
            dims: Vec::new(),
            size: 0,
            level: it.level,
            packed_bits: false,
        });
        members.push(id);
        self.define(block, &it.name, Sym::Var(id));
        // own dimensions
        let mut own: Vec<(Bound, Bound)> = Vec::new();
        for b in &it.dims {
            let AstBound { lower, upper } = b;
            let ub = self.bound(upper, block, dyn_ok, it.pos)?;
            let lb = match lower {
                Some(l) => self.bound(l, block, dyn_ok, it.pos)?,
                None => Bound::Const(1),
            };
            own.push((lb, ub));
        }
        // element unit
        let elem: Unit;
        if t.children.is_empty() {
            let ty = self.scalar_type_in(attrs, &it.name, it.pos, block, dyn_ok)?;
            let aligned = explicit_align
                .unwrap_or(!matches!(ty, Ty::Char { .. } | Ty::Bit { .. } | Ty::Pic(_)));
            let ty = match ty {
                Ty::Bit { len, var, .. } => Ty::Bit { len, var, aligned },
                t => t,
            };
            let packed = matches!(
                ty,
                Ty::Bit {
                    var: Varying::Fixed,
                    aligned: false,
                    ..
                }
            );
            let (size, align) = ty.storage(aligned);
            elem = if packed {
                let n = match ty.str_len() {
                    Some(Ext::Const(n)) => u64::from(n),
                    _ => 8,
                };
                Unit {
                    len: n,
                    align: 1,
                    off: 0,
                }
            } else {
                Unit {
                    len: u64::from(size) * 8,
                    align: u64::from(align) * 8,
                    off: 0,
                }
            };
            let v = &mut self.m.vars[id as usize];
            v.ty = Some(ty);
            v.packed_bits = packed;
            v.size = size;
        } else {
            // a structure: map the members, then pair them
            let mut units = Vec::new();
            let mut kids = Vec::new();
            for c in &t.children {
                let start = self.m.vars.len() as VarId;
                // member dimensions are fixed up after the element stride
                // is known: map with no parent dims first
                let u = self.map_node(
                    c,
                    block,
                    root,
                    Some(id),
                    Vec::new(),
                    explicit_align,
                    false,
                    members,
                    late,
                )?;
                units.push(u);
                kids.push(start);
            }
            let (u, offs) = pair(&units);
            for (k, off) in kids.iter().zip(offs) {
                self.shift(*k, off);
            }
            self.m.vars[id as usize].members = kids;
            self.m.vars[id as usize].size = u.len.div_ceil(8) as u32;
            elem = u;
        }
        // dimensions: strides from the element
        let stride = if elem.align <= 1 {
            elem.len
        } else {
            elem.len.div_ceil(elem.align) * elem.align
        };
        let mut count: u64 = 1;
        let mut dims: Vec<Dim> = Vec::new();
        let mut strides = Vec::new();
        let mut s = stride;
        for (lb, ub) in own.iter().rev() {
            strides.push(s);
            let extent = match (lb, ub) {
                (Bound::Const(l), Bound::Const(u)) => (u - l + 1).max(0) as u64,
                _ => 1,
            };
            s *= extent;
            count *= extent;
        }
        strides.reverse();
        for ((lb, ub), st) in own.iter().zip(strides) {
            dims.push(Dim {
                lb: *lb,
                ub: *ub,
                stride: st as u32,
            });
        }
        let mut all = parent_dims;
        all.extend(dims.clone());
        self.m.vars[id as usize].dims = all;
        // members of an array of structures inherit its dimensions
        if !dims.is_empty() && !t.children.is_empty() {
            let kids = self.m.vars[id as usize].members.clone();
            for k in kids {
                self.prepend_dims(k, &dims);
            }
        }
        if t.children.is_empty() || it.level == 1 {
            let extra: Vec<Attr> = attrs
                .iter()
                .filter(|a| {
                    matches!(
                        a,
                        Attr::Based(_) | Attr::Defined(_) | Attr::Position(_) | Attr::Initial(_)
                    )
                })
                .cloned()
                .collect();
            if !extra.is_empty() {
                late.push((id, extra, it.pos));
            }
        }
        Ok(Unit {
            len: if own.is_empty() {
                elem.len
            } else {
                stride * count
            },
            align: elem.align,
            off: elem.off,
        })
    }

    fn prepend_dims(&mut self, v: VarId, dims: &[Dim]) {
        let mut d = dims.to_vec();
        d.append(&mut self.m.vars[v as usize].dims);
        self.m.vars[v as usize].dims = d;
        for k in self.m.vars[v as usize].members.clone() {
            self.prepend_dims(k, dims);
        }
    }

    /// Adds `off` bits to the offset of a variable and its members.
    fn shift(&mut self, v: VarId, off: u64) {
        self.m.vars[v as usize].bit_offset += off;
        for k in self.m.vars[v as usize].members.clone() {
            self.shift(k, off);
        }
    }

    /// BASED locators, DEFINED bases and INITIAL values.
    fn late_attrs(&mut self, v: VarId, attrs: &[Attr], pos: Pos) -> Result<()> {
        let mut position = None;
        for a in attrs {
            if let Attr::Position(e) = a {
                let x = self.expr(e)?;
                position = Some(Box::new(self.conv(x, &Ty::fixed_bin(31, 0))));
            }
        }
        for a in attrs {
            match a {
                Attr::Based(Some(e)) => {
                    let x = self.expr(e)?;
                    let x = self.conv(x, &Ty::Pointer);
                    self.m.vars[v as usize].storage = Storage::Based(Some(Box::new(x)));
                }
                Attr::Defined(r) => {
                    let base = self.reference(r, true)?;
                    self.m.vars[v as usize].storage =
                        Storage::Defined(Box::new(base), position.clone());
                }
                Attr::Initial(items) => {
                    let ty = self.m.vars[v as usize]
                        .ty
                        .clone()
                        .ok_or_else(|| self.err(pos, "INITIAL on a structure"))?;
                    let mut values = Vec::new();
                    let mut fill = false;
                    self.init_values(items, &ty, &mut values, &mut fill, pos)?;
                    let init = Init {
                        var: v,
                        values,
                        fill,
                    };
                    let root = self.m.vars[v as usize].root;
                    match &self.m.vars[root as usize].storage {
                        Storage::Static(_) => self.m.static_inits.push(init),
                        Storage::Based(_) | Storage::Controlled(..) => {
                            self.m.static_inits.push(Init {
                                var: v,
                                values: init.values,
                                fill: init.fill,
                            })
                        }
                        _ => self.m.blocks[self.m.vars[v as usize].block as usize]
                            .inits
                            .push(init),
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn init_values(
        &mut self,
        items: &[AstInit],
        ty: &Ty,
        out: &mut Vec<Option<Expr>>,
        fill: &mut bool,
        pos: Pos,
    ) -> Result<()> {
        for it in items {
            match it {
                AstInit::Star => out.push(None),
                AstInit::Value(e) => {
                    let x = self.expr(e)?;
                    out.push(Some(self.conv(x, ty)));
                }
                AstInit::Repeat(Extent::Star | Extent::Refer(..), inner) => {
                    self.init_values(inner, ty, out, fill, pos)?;
                    *fill = true;
                }
                AstInit::Repeat(Extent::Expr(n), inner) => {
                    let n = self
                        .const_int(n)
                        .ok_or_else(|| self.err(pos, "iteration factor must be a constant"))?;
                    for _ in 0..n.max(0) {
                        self.init_values(inner, ty, out, fill, pos)?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Folds constant conversions of named constants.
fn fold_const(x: Expr) -> Expr {
    if let Ek::Conv(inner) = &*x.k {
        match (&*inner.k, &x.ty) {
            (Ek::Fixed(m, q), Ty::Fixed { q: tq, .. }) if q == tq => {
                return Expr {
                    k: Box::new(Ek::Fixed(*m, *q)),
                    ty: x.ty,
                };
            }
            (Ek::Fixed(m, q), Ty::Fixed { q: tq, .. }) if *q < *tq => {
                let m = m * 10i128.pow((tq - q) as u32);
                return Expr {
                    k: Box::new(Ek::Fixed(m, *tq)),
                    ty: x.ty,
                };
            }
            _ => {}
        }
    }
    x
}

fn build_tree(items: &[DeclItem]) -> Vec<Node> {
    fn take(items: &[DeclItem], i: &mut usize) -> Node {
        let item = items[*i].clone();
        *i += 1;
        let mut children = Vec::new();
        while *i < items.len() && items[*i].level > item.level {
            children.push(take(items, i));
        }
        Node { item, children }
    }
    let mut out = Vec::new();
    let mut i = 0;
    while i < items.len() {
        out.push(take(items, &mut i));
    }
    out
}

/// The structure mapping pairing algorithm: combines units left to right;
/// returns the structure unit and the offsets of the units (bits from the
/// structure start).
fn pair(units: &[Unit]) -> (Unit, Vec<u64>) {
    let mut cur = units[0];
    let mut offs = vec![0u64];
    for u in &units[1..] {
        let e = cur.off + cur.len;
        // first valid position p >= e: (p - u.off) mod u.align == 0
        let a = u.align.max(1);
        let rem = (e + 64 * a - u.off % a) % a;
        let p = if rem == 0 { e } else { e + (a - rem) };
        let g = p - e;
        let ca = cur.align.max(1);
        let s = (g / ca) * ca;
        let pad = g - s;
        offs.push(cur.len + pad);
        cur = Unit {
            len: cur.len + pad + u.len,
            align: cur.align.max(u.align),
            off: (cur.off + s) % 64,
        };
    }
    (cur, offs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing() {
        // FIXED BIN(31), CHAR(1), FIXED BIN(15): offsets 0, 4, 6
        let u = |len: u64, align: u64| Unit {
            len: len * 8,
            align: align * 8,
            off: 0,
        };
        let (s, offs) = pair(&[u(4, 4), u(1, 1), u(2, 2)]);
        assert_eq!(offs, [0, 32, 48]);
        assert_eq!(s.len, 64);
        // CHAR(1), FIXED BIN(31): the char moves next to the fullword
        let (s, offs) = pair(&[u(1, 1), u(4, 4)]);
        assert_eq!(offs, [0, 8]);
        assert_eq!(s.off, 24);
        assert_eq!(s.len, 40);
    }
}

#[cfg(test)]
mod mapping_tests {
    use crate::lexer::{Files, LexOptions, tokenize_text};
    use crate::parser::Parser;

    /// The structure mapping example of the Language Reference: member
    /// offsets and the structure's length and offset from a doubleword.
    #[test]
    fn language_reference_example() {
        let src = " M: PROC OPTIONS(MAIN);
 DCL 1 A ALIGNED STATIC, 2 B FIXED BIN(31), 2 C, 3 D FLOAT DEC(14),
   3 E, 4 F ENTRY VARIABLE, 4 G, 5 H CHAR(2), 5 I FLOAT DEC(13),
   4 J FIXED BIN(31), 3 K CHAR(2), 3 L FIXED BIN(20),
   2 M, 3 N, 4 P FIXED BIN(15), 4 Q CHAR(5), 4 R FLOAT DEC(2),
   3 S, 4 T FLOAT DEC(15), 4 U BIT(3), 4 V CHAR(1),
   3 W FIXED BIN(31), 2 X PIC '$9V99';
 END M;
";
        let mut files = Files::default();
        let toks = tokenize_text(src, "t", &LexOptions::default(), &mut files).unwrap();
        let stmts = Parser::new(toks).program().unwrap();
        let (m, diags) = crate::sema::compile(&stmts, 37, Vec::new());
        let m = m.unwrap_or_else(|| panic!("{diags:?}"));
        let off = |name: &str| {
            let v = m.vars.iter().find(|v| v.name == name).unwrap();
            v.bit_offset / 8
        };
        let expect = [
            ("B", 0),
            ("D", 4),
            ("F", 16),
            ("H", 26),
            ("I", 28),
            ("J", 36),
            ("K", 40),
            ("L", 44),
            ("P", 48),
            ("Q", 50),
            ("R", 56),
            ("T", 60),
            ("U", 68),
            ("V", 69),
            ("W", 72),
            ("X", 76),
        ];
        for (n, o) in expect {
            assert_eq!(off(n), o, "offset of {n}");
        }
        let a = m.vars.iter().find(|v| v.name == "A").unwrap();
        assert_eq!(a.size, 80);
        // allocated 4 bytes past a doubleword boundary
        let crate::ir::Storage::Static(s) = a.storage else {
            panic!()
        };
        assert_eq!((0x1000 + s) % 8, 4);
    }
}

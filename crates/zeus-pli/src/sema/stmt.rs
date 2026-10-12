//! Statements.

use super::expr::{bit_any, char_any, mk};
use super::*;
use crate::ast::{DataItem as AstItem, DoSpec as AstDo, Format as AstFmt, Transmit as AstTransmit};

impl Compiler {
    /// Compiles a statement list; labels are bound to their positions.
    pub fn list(&mut self, stmts: &[ast::Stmt]) -> Result<List> {
        let id = self.new_list_id();
        let mut out = Vec::new();
        for s in stmts {
            let index = out.len();
            let mut labels = Vec::new();
            for l in &s.labels {
                if let Some(Sym::Label(lid)) = self.scopes[self.cur as usize]
                    .names
                    .get(l)
                    .and_then(|v| v.iter().find(|x| matches!(x, Sym::Label(_))).cloned())
                {
                    let def = &mut self.m.labels[lid as usize];
                    def.list = id;
                    def.index = index;
                    labels.push(lid);
                }
            }
            let save = self.enabled;
            self.enabled = apply_prefixes(self.enabled, &s.prefixes);
            self.file = s.pos.file;
            let stmt = self.statement(s)?;
            out.push(Node {
                labels,
                line: s.pos.line,
                file: s.pos.file,
                enabled: self.enabled,
                stmt,
            });
            self.enabled = save;
        }
        Ok(List { id, stmts: out })
    }

    fn unit(&mut self, s: &ast::Stmt) -> Result<List> {
        self.list(std::slice::from_ref(s))
    }

    fn cond_of(&mut self, c: &ast::CondName, pos: Pos) -> Result<Cond> {
        let file = |me: &mut Compiler| -> Result<FileId> {
            let n = c
                .arg
                .clone()
                .ok_or_else(|| me.err(pos, format!("{} needs a file name", c.name)))?;
            me.file_for(&n, pos)
        };
        Ok(match c.name.as_str() {
            "ERROR" => Cond::Error,
            "FINISH" => Cond::Finish,
            "CONVERSION" => Cond::Conversion,
            "FIXEDOVERFLOW" => Cond::FixedOverflow,
            "OVERFLOW" => Cond::Overflow,
            "UNDERFLOW" => Cond::Underflow,
            "ZERODIVIDE" => Cond::ZeroDivide,
            "SIZE" => Cond::Size,
            "STRINGRANGE" => Cond::StringRange,
            "STRINGSIZE" => Cond::StringSize,
            "SUBSCRIPTRANGE" => Cond::SubscriptRange,
            "STORAGE" => Cond::Storage,
            "AREA" => Cond::Area,
            "ATTENTION" => Cond::Attention,
            "INVALIDOP" => Cond::InvalidOp,
            "ANYCONDITION" | "ANYCOND" => Cond::AnyCondition,
            "ENDFILE" => Cond::EndFile(file(self)?),
            "ENDPAGE" => Cond::EndPage(file(self)?),
            "KEY" => Cond::Key(file(self)?),
            "RECORD" => Cond::Record(file(self)?),
            "TRANSMIT" => Cond::Transmit(file(self)?),
            "UNDEFINEDFILE" => Cond::UndefinedFile(file(self)?),
            "NAME" => Cond::Name(file(self)?),
            "CONDITION" => {
                let n = c
                    .arg
                    .clone()
                    .ok_or_else(|| self.err(pos, "CONDITION needs a name"))?;
                match self.sym(&n) {
                    Some(Sym::Cond(id)) => Cond::Condition(id),
                    _ => {
                        self.m.conditions.push(n.clone());
                        let id = (self.m.conditions.len() - 1) as u32;
                        self.define(self.outer, &n, Sym::Cond(id));
                        Cond::Condition(id)
                    }
                }
            }
            n => return Err(self.err(pos, format!("condition {n} is not supported"))),
        })
    }

    /// Compiles a nested block (BEGIN, ON-unit).
    fn nested_block(&mut self, body: &[ast::Stmt], name: &str, on_unit: bool) -> Result<BlockId> {
        let b = self.new_block(Some(self.cur), name);
        self.m.blocks[b as usize].on_unit = on_unit;
        let save = (self.cur, std::mem::take(&mut self.loops));
        self.cur = b;
        let nested = self.scan_block(b, body, &[])?;
        let mut prepared = Vec::new();
        for n in nested {
            prepared.push(self.declare_proc(n, Some(b))?);
        }
        let list = self.list(body)?;
        self.m.blocks[b as usize].body = list;
        for p in prepared {
            self.body_proc(p)?;
        }
        self.cur = save.0;
        self.loops = save.1;
        Ok(b)
    }

    fn do_kind(&mut self, spec: &AstDo) -> Result<DoKind> {
        Ok(match spec {
            AstDo::Group => DoKind::Group,
            AstDo::Forever => DoKind::Loop {
                while_: None,
                until: None,
            },
            AstDo::Loop { while_, until } => DoKind::Loop {
                while_: match while_ {
                    Some(e) => Some(self.test(e)?),
                    None => None,
                },
                until: match until {
                    Some(e) => Some(self.test(e)?),
                    None => None,
                },
            },
            AstDo::Iterative { var, specs } => {
                let (t, ty) = self.target(&ast::Expr::Ref(var.clone()))?;
                let ty =
                    ty.ok_or_else(|| self.err(var.pos, "DO control variable must be a scalar"))?;
                let mut out = Vec::new();
                for s in specs {
                    let start = self.expr(&s.start)?;
                    let start = self.conv(start, &ty);
                    let arith = |c: &mut Compiler, e: &Option<ast::Expr>| -> Result<Option<Expr>> {
                        Ok(match e {
                            Some(e) => {
                                let x = c.expr(e)?;
                                Some(c.to_arith(x))
                            }
                            None => None,
                        })
                    };
                    let to = arith(self, &s.to)?;
                    let by = arith(self, &s.by)?;
                    let repeat = match &s.repeat {
                        Some(e) => {
                            let x = self.expr(e)?;
                            Some(self.conv(x, &ty))
                        }
                        None => None,
                    };
                    let while_ = match &s.while_ {
                        Some(e) => Some(self.test(e)?),
                        None => None,
                    };
                    let until = match &s.until {
                        Some(e) => Some(self.test(e)?),
                        None => None,
                    };
                    out.push(DoSpec {
                        start,
                        to,
                        by,
                        repeat,
                        while_,
                        until,
                    });
                }
                DoKind::Iter {
                    var: t,
                    var_ty: ty,
                    specs: out,
                }
            }
        })
    }

    fn loop_target(&self, label: &Option<String>, pos: Pos) -> Result<LoopId> {
        match label {
            None => self
                .loops
                .last()
                .map(|l| l.1)
                .ok_or_else(|| self.err(pos, "LEAVE / ITERATE outside a DO group")),
            Some(n) => self
                .loops
                .iter()
                .rev()
                .find(|l| l.0.contains(n))
                .map(|l| l.1)
                .ok_or_else(|| self.err(pos, format!("no enclosing DO group labelled {n}"))),
        }
    }

    pub fn formats(&mut self, f: &[AstFmt]) -> Result<Vec<Fmt>> {
        let mut out = Vec::new();
        for x in f {
            out.push(self.format(x)?);
        }
        Ok(out)
    }

    fn int_expr(&mut self, e: &ast::Expr) -> Result<Expr> {
        let x = self.expr(e)?;
        let x = self.to_arith(x);
        Ok(self.conv(x, &Ty::fixed_bin(31, 0)))
    }

    fn opt_int(&mut self, e: &Option<ast::Expr>) -> Result<Option<Expr>> {
        Ok(match e {
            Some(e) => Some(self.int_expr(e)?),
            None => None,
        })
    }

    fn format(&mut self, f: &AstFmt) -> Result<Fmt> {
        Ok(match f {
            AstFmt::A(w) => Fmt::A(self.opt_int(w)?),
            AstFmt::B(w) => Fmt::B(self.opt_int(w)?),
            AstFmt::F(w, d, s) => Fmt::F(self.int_expr(w)?, self.opt_int(d)?, self.opt_int(s)?),
            AstFmt::E(w, d, s) => Fmt::E(self.int_expr(w)?, self.opt_int(d)?, self.opt_int(s)?),
            AstFmt::P(s) => {
                let p =
                    crate::picture::Picture::parse(s).map_err(|e| self.err(Pos::default(), e))?;
                Fmt::P(Box::new(p))
            }
            AstFmt::X(w) => Fmt::X(self.int_expr(w)?),
            AstFmt::Skip(n) => Fmt::Skip(self.opt_int(n)?),
            AstFmt::Page => Fmt::Page,
            AstFmt::Line(n) => Fmt::Line(self.int_expr(n)?),
            AstFmt::Column(n) => Fmt::Column(self.int_expr(n)?),
            AstFmt::R(r) => match self.sym(r.name()) {
                Some(Sym::Format(id)) => Fmt::R(id),
                _ => return Err(self.err(r.pos, format!("{} is not a FORMAT label", r.name()))),
            },
            AstFmt::Repeat(n, items) => Fmt::Repeat(self.int_expr(n)?, self.formats(items)?),
        })
    }

    fn data_items(&mut self, items: &[AstItem], get: bool, named: bool) -> Result<Vec<DataItem>> {
        let mut out = Vec::new();
        for it in items {
            match it {
                AstItem::Expr(e) => {
                    let name = match e {
                        ast::Expr::Ref(r) => r
                            .parts
                            .iter()
                            .map(|p| p.0.clone())
                            .collect::<Vec<_>>()
                            .join("."),
                        _ => String::new(),
                    };
                    let item = if get {
                        let (t, ty) = self.target(e)?;
                        match (t, ty) {
                            (Target::Ref(r), _) if self.is_aggregate(&r) => DataItem::Aggregate(r),
                            (t, Some(ty)) => DataItem::Target(t, ty),
                            (t, None) => DataItem::Target(t, char_any()),
                        }
                    } else {
                        match e {
                            ast::Expr::Ref(r)
                                if {
                                    let names: Vec<String> =
                                        r.parts.iter().map(|p| p.0.clone()).collect();
                                    matches!(self.sym(&names[0]), Some(Sym::Var(_)) | None)
                                        && self.resolve_var(&names).is_some()
                                        && !self.entry_invocation(r)
                                } =>
                            {
                                let rr = self.reference(r, true)?;
                                if self.is_aggregate(&rr) {
                                    DataItem::Aggregate(rr)
                                } else {
                                    DataItem::Expr(self.load(rr, r.pos)?)
                                }
                            }
                            _ => DataItem::Expr(self.expr(e)?),
                        }
                    };
                    out.push(if named {
                        DataItem::Named(name, Box::new(item))
                    } else {
                        item
                    });
                }
                AstItem::Repeat(inner, var, specs) => {
                    let kind = self.do_kind(&AstDo::Iterative {
                        var: var.clone(),
                        specs: specs.clone(),
                    })?;
                    let items = self.data_items(inner, get, named)?;
                    out.push(DataItem::Repeat(items, Box::new(kind)));
                }
            }
        }
        Ok(out)
    }

    fn stream(&mut self, o: &ast::StreamOpts, get: bool, pos: Pos) -> Result<Stream> {
        let file = match &o.file {
            Some(ast::Expr::Ref(r)) if r.parts.len() == 1 && r.parts[0].1.is_empty() => {
                let f = self.file_for(&r.parts[0].0, pos)?;
                Some(mk(Ek::FileConst(f), Ty::File))
            }
            Some(e) => Some(self.expr(e)?),
            None => None,
        };
        let string = match &o.string {
            Some(e) if get => {
                let x = self.expr(e)?;
                Some(StringTarget::Source(self.to_char(x)))
            }
            Some(e) => {
                let (t, ty) = self.target(e)?;
                Some(StringTarget::Target(t, ty.unwrap_or(char_any())))
            }
            None => None,
        };
        let skip = match &o.skip {
            Some(Some(n)) => Some(self.int_expr(n)?),
            Some(None) => Some(mk(Ek::Fixed(1, 0), Ty::fixed_bin(31, 0))),
            None => None,
        };
        let line = self.opt_int(&o.line)?;
        let data = match &o.data {
            None => None,
            Some(AstTransmit::List(items)) => {
                Some(Transmit::List(self.data_items(items, get, false)?))
            }
            Some(AstTransmit::Data(items)) => {
                Some(Transmit::Data(self.data_items(items, get, true)?))
            }
            Some(AstTransmit::Edit(pairs)) => {
                let mut v = Vec::new();
                for (items, fmts) in pairs {
                    v.push((self.data_items(items, get, false)?, self.formats(fmts)?));
                }
                Some(Transmit::Edit(v))
            }
        };
        Ok(Stream {
            file,
            string,
            skip,
            page: o.page,
            line,
            data,
        })
    }

    fn file_expr(&mut self, e: &Option<ast::Expr>, pos: Pos) -> Result<Option<Expr>> {
        Ok(match e {
            Some(ast::Expr::Ref(r)) if r.parts.len() == 1 && r.parts[0].1.is_empty() => {
                let f = self.file_for(&r.parts[0].0, pos)?;
                Some(mk(Ek::FileConst(f), Ty::File))
            }
            Some(e) => Some(self.expr(e)?),
            None => None,
        })
    }

    fn record(&mut self, o: &ast::RecordOpts, pos: Pos) -> Result<RecordIo> {
        let mut r = RecordIo {
            file: self.file_expr(&o.file, pos)?,
            ..Default::default()
        };
        if let Some(e) = &o.into {
            let (t, _) = self.target(e)?;
            let size = match &t {
                Target::Ref(rr) => self.m.vars[rr.var as usize].size,
                _ => 0,
            };
            r.into = Some((t, size));
        }
        if let Some(e) = &o.from {
            r.from = Some(match e {
                ast::Expr::Ref(rf) => {
                    let names: Vec<String> = rf.parts.iter().map(|p| p.0.clone()).collect();
                    if self.resolve_var(&names).is_some() {
                        let rr = self.reference(rf, true)?;
                        if self.is_aggregate(&rr) {
                            mk(Ek::Aggregate(rr), char_any())
                        } else {
                            // a scalar: its storage bytes
                            mk(Ek::Aggregate(rr), char_any())
                        }
                    } else {
                        let x = self.expr(e)?;
                        self.to_char(x)
                    }
                }
                _ => {
                    let x = self.expr(e)?;
                    self.to_char(x)
                }
            });
        }
        if let Some(e) = &o.set {
            r.set = Some(self.target(e)?.0);
        }
        for (src, dst) in [(&o.key, &mut r.key), (&o.keyfrom, &mut r.keyfrom)] {
            if let Some(e) = src {
                let x = self.expr(e)?;
                *dst = Some(self.to_char(x));
            }
        }
        if let Some(e) = &o.keyto {
            let (t, ty) = self.target(e)?;
            r.keyto = Some((t, ty.unwrap_or(char_any())));
        }
        r.ignore = self.opt_int(&o.ignore)?;
        Ok(r)
    }

    /// Assignment of a structure or array.
    fn aggregate_assign(
        &mut self,
        targets: Vec<Target>,
        value: &ast::Expr,
        by_name: bool,
        pos: Pos,
    ) -> Result<Stmt> {
        let Target::Ref(first) = &targets[0] else {
            return Err(self.err(pos, "invalid aggregate assignment"));
        };
        let first = first.clone();
        let structure = self.m.vars[first.var as usize].ty.is_none();
        if structure {
            // expand member by member (or BY NAME)
            let src = match value {
                ast::Expr::Ref(r) => {
                    let names: Vec<String> = r.parts.iter().map(|p| p.0.clone()).collect();
                    match self.resolve_var(&names) {
                        Some(v) if self.m.vars[v as usize].ty.is_none() => {
                            Some(self.reference(r, true)?)
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            let mut stmts = Vec::new();
            for t in &targets {
                let Target::Ref(t) = t else { continue };
                self.expand_struct_assign(t, src.as_ref(), value, by_name, pos, &mut stmts)?;
            }
            let id = self.new_list_id();
            let line = pos.line;
            return Ok(Stmt::Group(List {
                id,
                stmts: stmts
                    .into_iter()
                    .map(|s| Node {
                        labels: Vec::new(),
                        line,
                        file: pos.file,
                        enabled: self.enabled,
                        stmt: s,
                    })
                    .collect(),
            }));
        }
        let rank = first.subs.iter().filter(|s| s.is_none()).count() as u8;
        let save = self.index_rank;
        self.index_rank = rank;
        let x = self.expr(value);
        self.index_rank = save;
        let x = x?;
        let ty = self.m.vars[first.var as usize].ty.clone().unwrap();
        let x = self.conv(x, &ty);
        let targets: Vec<Target> = targets
            .into_iter()
            .map(|t| match t {
                Target::Ref(r) => Target::Ref(self.fill_index(r)),
                t => t,
            })
            .collect();
        Ok(Stmt::ArrayAssign {
            targets,
            value: x,
            shape: first,
            rank,
        })
    }

    fn expand_struct_assign(
        &mut self,
        t: &Ref,
        src: Option<&Ref>,
        value: &ast::Expr,
        by_name: bool,
        pos: Pos,
        out: &mut Vec<Stmt>,
    ) -> Result<()> {
        let tv = &self.m.vars[t.var as usize];
        if tv.ty.is_some() {
            // a leaf
            let ty = tv.ty.clone().unwrap();
            let rank = t.subs.iter().filter(|s| s.is_none()).count() as u8;
            let save = self.index_rank;
            self.index_rank = rank;
            let x = match src {
                Some(s) => self.load(s.clone(), pos),
                None => self.expr(value),
            };
            self.index_rank = save;
            let x = self.conv(x?, &ty);
            if rank == 0 {
                out.push(Stmt::Assign(vec![Target::Ref(t.clone())], x));
            } else {
                out.push(Stmt::ArrayAssign {
                    targets: vec![Target::Ref(self.fill_index(t.clone()))],
                    value: x,
                    shape: t.clone(),
                    rank,
                });
            }
            return Ok(());
        }
        let tm = tv.members.clone();
        let sm: Vec<VarId> = src
            .map(|s| self.m.vars[s.var as usize].members.clone())
            .unwrap_or_default();
        for (i, m) in tm.iter().enumerate() {
            let sub_src = match src {
                None => None,
                Some(s) if by_name => {
                    let name = &self.m.vars[*m as usize].name;
                    match sm.iter().find(|x| &self.m.vars[**x as usize].name == name) {
                        Some(x) => Some(member_ref(s, *x, &self.m)),
                        None => continue,
                    }
                }
                Some(s) => match sm.get(i) {
                    Some(x) => Some(member_ref(s, *x, &self.m)),
                    None => {
                        return Err(self.err(pos, "structures do not have the same structuring"));
                    }
                },
            };
            let tr = member_ref(t, *m, &self.m);
            self.expand_struct_assign(&tr, sub_src.as_ref(), value, by_name, pos, out)?;
        }
        Ok(())
    }

    pub fn statement(&mut self, s: &ast::Stmt) -> Result<Stmt> {
        let pos = s.pos;
        Ok(match &s.kind {
            StmtKind::Procedure(_)
            | StmtKind::Declare(_)
            | StmtKind::Entry(..)
            | StmtKind::Format(_)
            | StmtKind::Null => Stmt::Null,
            StmtKind::Begin(body) => Stmt::Begin(self.nested_block(body, "BEGIN", false)?),
            StmtKind::Assign {
                targets,
                op,
                value,
                by_name,
            } => {
                let mut ts = Vec::new();
                let mut tys = Vec::new();
                for t in targets {
                    let (t, ty) = self.target(t)?;
                    ts.push(t);
                    tys.push(ty);
                }
                let aggregate = matches!(&ts[0], Target::Ref(r) if self.is_aggregate(r));
                let value = match op {
                    Some(o) => {
                        // compound assignment: target op value
                        ast::Expr::Binary(
                            o,
                            Box::new(targets[0].clone()),
                            Box::new(value.clone()),
                            pos,
                        )
                    }
                    None => value.clone(),
                };
                if aggregate {
                    return self.aggregate_assign(ts, &value, *by_name, pos);
                }
                let x = if matches!(tys[0], Some(Ty::Entry)) {
                    self.entry_expr(&value)?
                } else {
                    self.expr(&value)?
                };
                let ty = tys[0].clone().unwrap_or(char_any());
                if ts.len() == 1 {
                    Stmt::Assign(ts, self.conv(x, &ty))
                } else {
                    // multiple targets: each converts the value itself
                    Stmt::Assign(ts, x)
                }
            }
            StmtKind::If { cond, then, els } => {
                let c = self.test(cond)?;
                let t = self.unit(then)?;
                let e = match els {
                    Some(e) => self.unit(e)?,
                    None => List {
                        id: self.new_list_id(),
                        stmts: Vec::new(),
                    },
                };
                Stmt::If(c, t, e)
            }
            StmtKind::Do { spec, body } => {
                let kind = self.do_kind(spec)?;
                self.loop_seq += 1;
                let id = self.loop_seq;
                self.loops.push((s.labels.clone(), id));
                let body = self.list(body);
                self.loops.pop();
                let body = body?;
                if matches!(kind, DoKind::Group) {
                    // a plain group can still be left with LEAVE
                    Stmt::Do(id, kind, body)
                } else {
                    Stmt::Do(id, kind, body)
                }
            }
            StmtKind::Select {
                subject,
                whens,
                other,
            } => {
                let subj = match subject {
                    Some(e) => Some(self.expr(e)?),
                    None => None,
                };
                let mut ws = Vec::new();
                for (vals, unit) in whens {
                    let mut conds = Vec::new();
                    for v in vals {
                        let c = match &subj {
                            Some(sx) => {
                                let x = self.expr(v)?;
                                let sv = mk(Ek::Subject, sx.ty.clone());
                                self.compare(CmpOp::Eq, sv, x)
                            }
                            None => self.test(v)?,
                        };
                        conds.push(c);
                    }
                    ws.push((conds, self.unit(unit)?));
                }
                let o = match other {
                    Some(o) => self.unit(o)?,
                    None => List {
                        id: 0,
                        stmts: Vec::new(),
                    },
                };
                Stmt::Select(subj, ws, o)
            }
            StmtKind::Call(r) => self.call_stmt(r)?,
            StmtKind::Return(v) => {
                let proc = self.enclosing_proc();
                match v {
                    Some(e) => {
                        let x = self.expr(e)?;
                        let rt = proc
                            .and_then(|p| self.m.procs[p as usize].returns.clone())
                            .ok_or_else(|| {
                                self.err(pos, "RETURN with a value in a procedure without RETURNS")
                            })?;
                        Stmt::Return(Some(self.conv(x, &rt)))
                    }
                    None => Stmt::Return(None),
                }
            }
            StmtKind::GoTo(e) => {
                let x = match e {
                    ast::Expr::Ref(r) if r.parts.len() == 1 && r.parts[0].1.is_empty() => {
                        match self.sym(&r.parts[0].0) {
                            Some(Sym::Label(l)) => mk(Ek::Label(l), Ty::Label),
                            Some(Sym::Var(_)) => self.expr(e)?,
                            _ => {
                                return Err(
                                    self.err(pos, format!("{} is not a label", r.parts[0].0))
                                );
                            }
                        }
                    }
                    _ => self.expr(e)?,
                };
                Stmt::GoTo(x)
            }
            StmtKind::Leave(l) => Stmt::Leave(self.loop_target(l, pos)?),
            StmtKind::Iterate(l) => Stmt::Iterate(self.loop_target(l, pos)?),
            StmtKind::On { conds, snap, unit } => {
                let mut cs = Vec::new();
                for c in conds {
                    cs.push(self.cond_of(c, pos)?);
                }
                let block = match unit {
                    None => None,
                    Some(u) => {
                        let body: Vec<ast::Stmt> = match &u.kind {
                            StmtKind::Begin(b) if u.labels.is_empty() => b.clone(),
                            _ => vec![(**u).clone()],
                        };
                        Some(self.nested_block(&body, "ON-UNIT", true)?)
                    }
                };
                Stmt::On(cs, block, *snap)
            }
            StmtKind::Signal(c) => Stmt::Signal(self.cond_of(c, pos)?),
            StmtKind::Revert(cs) => {
                let mut v = Vec::new();
                for c in cs {
                    v.push(self.cond_of(c, pos)?);
                }
                Stmt::Revert(v)
            }
            StmtKind::Resignal => Stmt::Resignal,
            StmtKind::Stop => Stmt::Stop,
            StmtKind::Exit => Stmt::Exit,
            StmtKind::Put(o) => Stmt::Put(self.stream(o, false, pos)?),
            StmtKind::Get(o) => Stmt::Get(self.stream(o, true, pos)?),
            StmtKind::Open(files) => {
                let mut v = Vec::new();
                for f in files {
                    let file = self
                        .file_expr(&Some(f.file.clone()), pos)?
                        .ok_or_else(|| self.err(pos, "OPEN needs FILE"))?;
                    let has = |a: fn(&Attr) -> bool| f.attrs.iter().any(a);
                    let title = match &f.title {
                        Some(t) => {
                            let x = self.expr(t)?;
                            Some(self.to_char(x))
                        }
                        None => None,
                    };
                    v.push(OpenSpec {
                        file,
                        input: has(|a| matches!(a, Attr::Input)),
                        output: has(|a| matches!(a, Attr::Output)),
                        update: has(|a| matches!(a, Attr::Update)),
                        print: has(|a| matches!(a, Attr::Print)).then_some(true),
                        keyed: has(|a| matches!(a, Attr::Keyed)),
                        title,
                        linesize: self.opt_int(&f.linesize)?,
                        pagesize: self.opt_int(&f.pagesize)?,
                    });
                }
                Stmt::Open(v)
            }
            StmtKind::Close(files) => {
                let mut v = Vec::new();
                for f in files {
                    if let Some(x) = self.file_expr(&Some(f.clone()), pos)? {
                        v.push(x);
                    }
                }
                Stmt::Close(v)
            }
            StmtKind::Read(o) => Stmt::Read(self.record(o, pos)?),
            StmtKind::Write(o) => Stmt::Write(self.record(o, pos)?),
            StmtKind::Rewrite(o) => Stmt::Rewrite(self.record(o, pos)?),
            StmtKind::Delete(o) => Stmt::Delete(self.record(o, pos)?),
            StmtKind::Locate(..) => return Err(self.err(pos, "LOCATE is not supported")),
            StmtKind::Allocate(items) => {
                let mut v = Vec::new();
                for (r, set, len) in items {
                    let names: Vec<String> = r.parts.iter().map(|p| p.0.clone()).collect();
                    let var = self.resolve_var(&names).ok_or_else(|| {
                        self.err(pos, format!("{} is not declared", names.join(".")))
                    })?;
                    let mut given: Vec<Option<Expr>> = Vec::new();
                    match &self.m.vars[var as usize].storage {
                        Storage::Based(_) => {
                            if len.is_some() || r.parts.iter().any(|p| !p.1.is_empty()) {
                                return Err(self.err(
                                    pos,
                                    "extents on ALLOCATE are allowed only for CONTROLLED variables",
                                ));
                            }
                        }
                        Storage::Controlled(_, slots) => {
                            given = vec![None; slots.len()];
                            let v = self.m.vars[var as usize].clone();
                            let bounds: Vec<&crate::ast::Expr> =
                                r.parts.iter().flat_map(|p| p.1.iter().flatten()).collect();
                            if !bounds.is_empty() && bounds.len() != v.dims.len() {
                                return Err(self.err(
                                    pos,
                                    format!("{} needs {} bounds", v.name, v.dims.len()),
                                ));
                            }
                            let fb = Ty::fixed_bin(31, 0);
                            for (d, e) in v.dims.iter().zip(bounds) {
                                let Bound::Slot(s) = d.ub else {
                                    return Err(self.err(
                                        pos,
                                        format!("the bounds of {} are constants", v.name),
                                    ));
                                };
                                let x = self.expr(e)?;
                                given[s as usize] = Some(self.conv(x, &fb));
                            }
                            if let Some(l) = len {
                                let Some(crate::types::Ext::Slot(s)) =
                                    v.ty.as_ref().and_then(Ty::str_len)
                                else {
                                    return Err(self.err(
                                        pos,
                                        format!("the length of {} is a constant", v.name),
                                    ));
                                };
                                let x = self.expr(l)?;
                                given[s as usize] = Some(self.conv(x, &fb));
                            }
                        }
                        _ => {
                            return Err(self.err(
                                pos,
                                format!("{} is not BASED or CONTROLLED", names.join(".")),
                            ));
                        }
                    }
                    let set = match set {
                        Some(e) => Some(self.target(e)?.0),
                        None => None,
                    };
                    v.push((var, set, given));
                }
                Stmt::Allocate(v)
            }
            StmtKind::Fetch(items) => {
                let mut v = Vec::new();
                for (name, title) in items {
                    let t = match title {
                        Some(e) => {
                            let x = self.expr(e)?;
                            Some(self.to_char(x))
                        }
                        None => None,
                    };
                    // the external name of the entry
                    let ext = match self.sym(name) {
                        Some(Sym::EntryDecl(d)) => d.name.clone(),
                        _ => name.clone(),
                    };
                    v.push((ext, t));
                }
                Stmt::Fetch(v)
            }
            StmtKind::Release(names) => Stmt::Release(
                names
                    .iter()
                    .map(|n| match self.sym(n) {
                        Some(Sym::EntryDecl(d)) => d.name.clone(),
                        _ => n.clone(),
                    })
                    .collect(),
            ),
            StmtKind::Free(refs) => {
                let mut v = Vec::new();
                for r in refs {
                    let names: Vec<String> = r.parts.iter().map(|p| p.0.clone()).collect();
                    let var = self.resolve_var(&names).ok_or_else(|| {
                        self.err(pos, format!("{} is not declared", names.join(".")))
                    })?;
                    let loc = match &r.locator {
                        Some(l) => {
                            let x = self.ref_expr(l)?;
                            Some(self.conv(x, &Ty::Pointer))
                        }
                        None => None,
                    };
                    v.push((var, loc));
                }
                Stmt::Free(v)
            }
            StmtKind::Display { value, reply } => {
                let x = self.expr(value)?;
                let x = self.to_char(x);
                let reply = match reply {
                    Some(e) => {
                        let (t, ty) = self.target(e)?;
                        Some((t, ty.unwrap_or(char_any())))
                    }
                    None => None,
                };
                Stmt::Display(x, reply)
            }
            StmtKind::Delay(e) => Stmt::Delay(self.int_expr(e)?),
            StmtKind::Exec(kind, text) => self.exec_stmt(kind, text, pos)?,
        })
    }

    fn enclosing_proc(&self) -> Option<ProcId> {
        let mut b = Some(self.cur);
        while let Some(id) = b {
            if let Some(p) = self.m.blocks[id as usize].proc {
                return Some(p);
            }
            b = self.m.blocks[id as usize].parent;
        }
        None
    }
}

/// A reference to a member with the subscripts of the containing
/// reference (members have the same leading dimensions).
fn member_ref(parent: &Ref, member: VarId, m: &Module) -> Ref {
    let n = m.vars[member as usize].dims.len();
    let mut subs = parent.subs.clone();
    subs.resize(n, None);
    Ref {
        var: member,
        subs,
        locator: parent.locator.clone(),
    }
}

#[allow(dead_code)]
fn unused() -> (Ty, Ty) {
    (bit_any(), char_any())
}

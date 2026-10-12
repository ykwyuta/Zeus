//! Expressions: name resolution, typing and conversions.

use super::*;
use crate::lexer::StrKind;
use crate::types::{
    Ext, Var as Varying, arith_bit_len, arith_char_len, arith_result, bin_to_dec_digits,
    dec_as_bin, dec_to_bin_digits, operand_arith, power_result,
};

pub fn mk(k: Ek, ty: Ty) -> Expr {
    Expr { k: Box::new(k), ty }
}

/// Natural-length character type of an expression result.
pub fn char_any() -> Ty {
    Ty::Char {
        len: Ext::Star,
        var: Varying::Varying,
    }
}

pub fn bit_any() -> Ty {
    Ty::Bit {
        len: Ext::Star,
        var: Varying::Varying,
        aligned: false,
    }
}

fn const_len(t: &Ty) -> Option<u32> {
    match t.str_len() {
        Some(Ext::Const(n)) => Some(n),
        _ => None,
    }
}

impl Compiler {
    /// A conversion to `ty` (none when the type is already `ty`).
    pub fn conv(&self, e: Expr, ty: &Ty) -> Expr {
        if e.ty == *ty {
            return e;
        }
        mk(Ek::Conv(e), ty.clone())
    }

    /// Type conversion of an operand of an arithmetic operator.
    pub fn to_arith(&self, e: Expr) -> Expr {
        match &e.ty {
            Ty::Fixed { .. } | Ty::Float { .. } => e,
            t => {
                let a = operand_arith(t);
                self.conv(e, &a)
            }
        }
    }

    /// Converts an arithmetic operand to the base and scale of the result
    /// `rt`, keeping its precision.
    fn to_base(&self, e: Expr, rt: &Ty) -> Expr {
        let target = match (&e.ty, rt) {
            (
                Ty::Fixed {
                    dec: true, p, q, ..
                },
                Ty::Fixed { dec: false, .. },
            ) => {
                let (p, q) = dec_as_bin(*p, *q);
                Ty::Fixed {
                    dec: false,
                    p: p.clamp(1, 63) as u8,
                    q: q as i16,
                    unsigned: false,
                }
            }
            (Ty::Fixed { dec, p, .. }, Ty::Float { dec: rd, .. })
            | (Ty::Float { dec, p }, Ty::Float { dec: rd, .. }) => {
                let p = i32::from(*p);
                let p = match (*dec, *rd) {
                    (true, false) => dec_to_bin_digits(p),
                    (false, true) => bin_to_dec_digits(p),
                    _ => p,
                };
                Ty::Float {
                    dec: *rd,
                    p: p.clamp(1, 109) as u8,
                }
            }
            _ => return e,
        };
        self.conv(e, &target)
    }

    pub fn to_bit(&self, e: Expr) -> Expr {
        if e.ty.is_bit() {
            return e;
        }
        let ty = if e.ty.is_char() {
            Ty::Bit {
                len: e.ty.str_len().unwrap_or(Ext::Star),
                var: Varying::Varying,
                aligned: false,
            }
        } else if e.ty.arith().is_some() {
            Ty::bit(arith_bit_len(&operand_arith(&e.ty)))
        } else {
            bit_any()
        };
        self.conv(e, &ty)
    }

    pub fn to_char(&self, e: Expr) -> Expr {
        if matches!(e.ty, Ty::Char { .. }) {
            return e;
        }
        let ty = match &e.ty {
            Ty::Pic(p) => Ty::char(p.len as u32),
            Ty::Bit { len, .. } => Ty::Char {
                len: *len,
                var: Varying::Varying,
            },
            t if t.arith().is_some() => Ty::char(arith_char_len(t)),
            _ => char_any(),
        };
        self.conv(e, &ty)
    }

    /// Converts a condition (IF, WHILE, UNTIL, WHEN) to a bit string.
    pub fn test(&mut self, e: &ast::Expr) -> Result<Expr> {
        let x = self.expr(e)?;
        Ok(self.to_bit(x))
    }

    pub fn arith_op(&self, op: ArOp, a: Expr, b: Expr) -> Expr {
        let a = self.to_arith(a);
        let b = self.to_arith(b);
        let rt = if op == ArOp::Pow {
            let n = match (&*b.k, &b.ty) {
                (Ek::Fixed(m, 0), Ty::Fixed { .. }) if *m >= 0 => Some(*m as u32),
                _ => None,
            };
            power_result(&a.ty, &b.ty, n)
        } else {
            let o = match op {
                ArOp::Add => "+",
                ArOp::Sub => "-",
                ArOp::Mul => "*",
                _ => "/",
            };
            arith_result(o, &a.ty, &b.ty)
        };
        let (a, b) = if op == ArOp::Pow {
            let a = if matches!(rt, Ty::Float { .. }) {
                self.to_base(a, &rt)
            } else {
                a
            };
            (a, b)
        } else {
            (self.to_base(a, &rt), self.to_base(b, &rt))
        };
        mk(Ek::Arith(op, a, b), rt)
    }

    pub fn compare(&self, op: CmpOp, a: Expr, b: Expr) -> Expr {
        let arith = |t: &Ty| t.arith().is_some();
        let (a, b) = if arith(&a.ty) || arith(&b.ty) {
            let a = self.to_arith(a);
            let b = self.to_arith(b);
            let rt = arith_result("+", &a.ty, &b.ty);
            (self.to_base(a, &rt), self.to_base(b, &rt))
        } else if matches!(a.ty, Ty::Pointer | Ty::Offset)
            || matches!(b.ty, Ty::Pointer | Ty::Offset)
        {
            (self.conv(a, &Ty::Pointer), self.conv(b, &Ty::Pointer))
        } else if a.ty.is_char() || b.ty.is_char() {
            (self.to_char(a), self.to_char(b))
        } else if a.ty.is_bit() || b.ty.is_bit() {
            (self.to_bit(a), self.to_bit(b))
        } else {
            (a, b)
        };
        mk(Ek::Cmp(op, a, b), Ty::bit(1))
    }

    fn concat(&self, a: Expr, b: Expr) -> Expr {
        // RULES(IBM): BIT when either operand is a bit string or binary
        // arithmetic, otherwise CHARACTER
        let bitish = |t: &Ty| t.is_bit() || (matches!(t, Ty::Fixed { dec: false, .. }));
        let bit = bitish(&a.ty) || bitish(&b.ty);
        let (a, b) = if bit && !a.ty.is_char() && !b.ty.is_char() {
            (self.to_bit(a), self.to_bit(b))
        } else {
            (self.to_char(a), self.to_char(b))
        };
        let len = match (const_len(&a.ty), const_len(&b.ty)) {
            (Some(x), Some(y)) => Ext::Const(x + y),
            _ => Ext::Star,
        };
        let ty = if a.ty.is_bit() {
            Ty::Bit {
                len,
                var: Varying::Varying,
                aligned: false,
            }
        } else {
            Ty::Char {
                len,
                var: Varying::Varying,
            }
        };
        mk(Ek::Concat(a, b), ty)
    }

    fn bits(&self, op: BitOp, a: Expr, b: Expr) -> Expr {
        let a = self.to_bit(a);
        let b = self.to_bit(b);
        let len = match (const_len(&a.ty), const_len(&b.ty)) {
            (Some(x), Some(y)) => Ext::Const(x.max(y)),
            _ => Ext::Star,
        };
        mk(
            Ek::Bits(op, a, b),
            Ty::Bit {
                len,
                var: Varying::Varying,
                aligned: false,
            },
        )
    }

    /// A numeric constant.
    pub fn number(&self, text: &str, binary: bool, pos: Pos) -> Result<Expr> {
        let bad = || self.err(pos, format!("invalid number {text}"));
        if let Some(e) = text.find('E') {
            let mant = &text[..e];
            let digits = mant.chars().filter(|c| c.is_ascii_digit()).count();
            let v: f64 = if binary {
                let (i, f) = mant.split_once('.').unwrap_or((mant, ""));
                let iv = i128::from_str_radix(if i.is_empty() { "0" } else { i }, 2)
                    .map_err(|_| bad())?;
                let fv = if f.is_empty() {
                    0
                } else {
                    i128::from_str_radix(f, 2).map_err(|_| bad())?
                };
                let exp: i32 = text[e + 1..].parse().map_err(|_| bad())?;
                (iv as f64 + fv as f64 / 2f64.powi(f.len() as i32)) * 2f64.powi(exp)
            } else {
                text.parse().map_err(|_| bad())?
            };
            return Ok(mk(
                Ek::Float(v),
                Ty::Float {
                    dec: !binary,
                    p: digits.clamp(1, 109) as u8,
                },
            ));
        }
        let (i, f) = text.split_once('.').unwrap_or((text, ""));
        let digits = format!("{i}{f}");
        let p = digits.len().max(1);
        let q = f.len() as i16;
        let m = if binary {
            i128::from_str_radix(if digits.is_empty() { "0" } else { &digits }, 2)
                .map_err(|_| bad())?
        } else {
            if p > 31 {
                return Err(self.err(pos, format!("constant {text} has too many digits")));
            }
            digits.parse::<i128>().map_err(|_| bad())?
        };
        let max = if binary { 63 } else { 31 };
        Ok(mk(
            Ek::Fixed(m, q),
            Ty::Fixed {
                dec: !binary,
                p: p.min(max) as u8,
                q,
                unsigned: false,
            },
        ))
    }

    fn string(&self, text: &str, kind: StrKind, pos: Pos) -> Result<Expr> {
        let bad = || self.err(pos, format!("invalid string constant '{text}'"));
        Ok(match kind {
            StrKind::Char => {
                let b = zeus_cobol::ebcdic::encode(text);
                let n = b.len() as u32;
                mk(Ek::Char(b), Ty::char(n))
            }
            StrKind::Hex => {
                let t: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                if !t.len().is_multiple_of(2) {
                    return Err(bad());
                }
                let b: Vec<u8> = (0..t.len())
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&t[i..i + 2], 16))
                    .collect::<std::result::Result<_, _>>()
                    .map_err(|_| bad())?;
                let n = b.len() as u32;
                mk(Ek::Char(b), Ty::char(n))
            }
            StrKind::Bit(r) => {
                let mut bits = Vec::new();
                for c in text.chars() {
                    let v = c.to_digit(1 << r).ok_or_else(bad)?;
                    for k in (0..r).rev() {
                        bits.push(v >> k & 1 == 1);
                    }
                }
                let n = bits.len() as u32;
                mk(Ek::Bit(bits), Ty::bit(n))
            }
            StrKind::HexBin | StrKind::HexUnsigned => {
                let v = u64::from_str_radix(text, 16).map_err(|_| bad())?;
                let wide = text.len() > 8;
                let unsigned = kind == StrKind::HexUnsigned;
                let m = if unsigned {
                    i128::from(v)
                } else if wide {
                    i128::from(v as i64)
                } else {
                    i128::from(v as u32 as i32)
                };
                mk(
                    Ek::Fixed(m, 0),
                    Ty::Fixed {
                        dec: false,
                        p: match (wide, unsigned) {
                            (false, false) => 31,
                            (false, true) => 32,
                            (true, false) => 63,
                            (true, true) => 64,
                        },
                        q: 0,
                        unsigned,
                    },
                )
            }
        })
    }

    pub fn expr(&mut self, e: &ast::Expr) -> Result<Expr> {
        Ok(match e {
            ast::Expr::Num { text, binary, pos } => self.number(text, *binary, *pos)?,
            ast::Expr::Str { text, kind, pos } => self.string(text, *kind, *pos)?,
            ast::Expr::Ref(r) => self.ref_expr(r)?,
            ast::Expr::Star(p) => return Err(self.err(*p, "'*' is not valid here")),
            ast::Expr::Unary(op, x, _) => {
                let x = self.expr(x)?;
                match *op {
                    "-" => {
                        let x = self.to_arith(x);
                        let ty = x.ty.clone();
                        mk(Ek::Neg(x), ty)
                    }
                    "+" => self.to_arith(x),
                    _ => {
                        let x = self.to_bit(x);
                        let ty = x.ty.clone();
                        mk(Ek::Not(x), ty)
                    }
                }
            }
            ast::Expr::Binary(op, a, b, _) => {
                let a = self.expr(a)?;
                let b = self.expr(b)?;
                match *op {
                    "+" => self.arith_op(ArOp::Add, a, b),
                    "-" => self.arith_op(ArOp::Sub, a, b),
                    "*" => self.arith_op(ArOp::Mul, a, b),
                    "/" => self.arith_op(ArOp::Div, a, b),
                    "**" => self.arith_op(ArOp::Pow, a, b),
                    "||" => self.concat(a, b),
                    "&" => self.bits(BitOp::And, a, b),
                    "|" => self.bits(BitOp::Or, a, b),
                    "¬" => self.bits(BitOp::Xor, a, b),
                    o => {
                        let c = match o {
                            "=" => CmpOp::Eq,
                            "¬=" => CmpOp::Ne,
                            "<" => CmpOp::Lt,
                            ">" => CmpOp::Gt,
                            "<=" | "¬>" => CmpOp::Le,
                            _ => CmpOp::Ge,
                        };
                        self.compare(c, a, b)
                    }
                }
            }
        })
    }

    /// Candidate variables for a (possibly qualified) name, innermost
    /// block first.
    pub fn resolve_var(&self, names: &[String]) -> Option<VarId> {
        let last = names.last()?;
        let mut b = Some(self.cur);
        while let Some(id) = b {
            if let Some(syms) = self.scopes[id as usize].names.get(last) {
                let vars: Vec<VarId> = syms
                    .iter()
                    .filter_map(|s| match s {
                        Sym::Var(v) => Some(*v),
                        _ => None,
                    })
                    .collect();
                // qualification: the containing names in order
                let matching: Vec<VarId> = vars
                    .iter()
                    .copied()
                    .filter(|&v| {
                        let mut chain = Vec::new();
                        let mut p = self.m.vars[v as usize].parent;
                        while let Some(x) = p {
                            chain.push(self.m.vars[x as usize].name.clone());
                            p = self.m.vars[x as usize].parent;
                        }
                        chain.reverse();
                        let mut it = chain.iter();
                        names[..names.len() - 1].iter().all(|q| it.any(|c| c == q))
                    })
                    .collect();
                if matching.len() == 1 {
                    return Some(matching[0]);
                }
                if matching.len() > 1 {
                    // fully qualified or level-1 preferred
                    if let Some(v) = matching.iter().find(|&&v| {
                        let mut depth = 0;
                        let mut p = self.m.vars[v as usize].parent;
                        while let Some(x) = p {
                            depth += 1;
                            p = self.m.vars[x as usize].parent;
                        }
                        depth == names.len() - 1
                    }) {
                        return Some(*v);
                    }
                    return Some(matching[0]);
                }
            }
            b = self.m.blocks[id as usize].parent;
        }
        None
    }

    /// The symbol of a simple (unqualified) name, innermost first.
    pub fn sym(&self, name: &str) -> Option<Sym> {
        self.lookup(name).into_iter().map(|(_, s)| s).next()
    }

    /// A variable reference with its subscripts.
    pub fn reference(&mut self, r: &ast::Ref, allow_aggregate: bool) -> Result<Ref> {
        let names: Vec<String> = r.parts.iter().map(|p| p.0.clone()).collect();
        let v = match self.resolve_var(&names) {
            Some(v) => v,
            None if names.len() == 1 && r.parts[0].1.is_empty() => {
                self.implicit_var(&names[0], r.pos)?
            }
            None => return Err(self.err(r.pos, format!("{} is not declared", names.join(".")))),
        };
        let mut subs = Vec::new();
        for (_, lists) in &r.parts {
            for l in lists {
                for a in l {
                    if let ast::Expr::Star(_) = a {
                        subs.push(None);
                    } else {
                        let x = self.expr(a)?;
                        let x = self.to_arith(x);
                        subs.push(Some(self.conv(x, &Ty::fixed_bin(31, 0))));
                    }
                }
            }
        }
        let ndims = self.m.vars[v as usize].dims.len();
        if !subs.is_empty() && subs.len() != ndims {
            return Err(self.err(
                r.pos,
                format!(
                    "{} needs {ndims} subscripts, {} given",
                    names.join("."),
                    subs.len()
                ),
            ));
        }
        let aggregate = (subs.is_empty() && ndims > 0)
            || subs.iter().any(|s| s.is_none())
            || self.m.vars[v as usize].ty.is_none();
        if aggregate && !allow_aggregate {
            return Err(self.err(r.pos, format!("{} is an aggregate", names.join("."))));
        }
        if subs.is_empty() {
            subs = vec![None; ndims];
        }
        let locator = match &r.locator {
            Some(l) => {
                let x = self.ref_expr(l)?;
                Some(Box::new(self.conv(x, &Ty::Pointer)))
            }
            None => None,
        };
        Ok(Ref {
            var: v,
            subs,
            locator,
        })
    }

    /// Whether a reference has free (`*` or omitted) dimensions or is a
    /// structure.
    pub fn is_aggregate(&self, r: &Ref) -> bool {
        r.subs.iter().any(|s| s.is_none()) || self.m.vars[r.var as usize].ty.is_none()
    }

    /// Loads a scalar reference; aggregates are replaced by their element
    /// at the current aggregate index when inside an aggregate assignment.
    pub fn load(&self, r: Ref, pos: Pos) -> Result<Expr> {
        let v = &self.m.vars[r.var as usize];
        if self.is_aggregate(&r) {
            if self.index_rank > 0 && v.ty.is_some() {
                let r = self.fill_index(r);
                let ty = v.ty.clone().unwrap();
                return Ok(mk(Ek::Load(r), ty));
            }
            let ty = char_any();
            return Ok(mk(Ek::Aggregate(r), ty));
        }
        let ty =
            v.ty.clone()
                .ok_or_else(|| self.err(pos, "structure used as a scalar"))?;
        Ok(mk(Ek::Load(r), ty))
    }

    /// Replaces the free subscripts of a reference by aggregate indexes.
    pub fn fill_index(&self, mut r: Ref) -> Ref {
        let mut k = 0u8;
        for s in r.subs.iter_mut() {
            if s.is_none() {
                *s = Some(mk(Ek::Index(k), Ty::fixed_bin(31, 0)));
                k += 1;
            }
        }
        r
    }

    /// A reference in an expression: a variable, named constant, label,
    /// file, function invocation or built-in.
    pub fn ref_expr(&mut self, r: &ast::Ref) -> Result<Expr> {
        let names: Vec<String> = r.parts.iter().map(|p| p.0.clone()).collect();
        if names.len() == 1 && r.locator.is_none() {
            let name = &names[0];
            let args: &Vec<Vec<ast::Expr>> = &r.parts[0].1;
            let sym = self
                .lookup(name)
                .into_iter()
                .map(|(_, s)| s)
                .find(|s| !matches!(s, Sym::Var(_)) || true);
            match sym {
                Some(Sym::Var(_)) | None => {}
                Some(Sym::Const(x)) => return Ok(x),
                Some(Sym::Label(l)) => return Ok(mk(Ek::Label(l), Ty::Label)),
                Some(Sym::Format(_)) => {
                    return Err(self.err(r.pos, "format label used in an expression"));
                }
                Some(Sym::File(f)) => return Ok(mk(Ek::FileConst(f), Ty::File)),
                Some(Sym::Proc(p)) => {
                    if args.is_empty() && self.m.procs[p as usize].returns.is_none() {
                        return Ok(mk(Ek::EntryConst(p), Ty::Entry));
                    }
                    return self.call_expr(
                        Callee::Proc(p),
                        args.first().map(|v| v.as_slice()).unwrap_or(&[]),
                        r.pos,
                    );
                }
                Some(Sym::Entry(p, k)) => {
                    return self.call_expr(
                        Callee::Entry(p, k),
                        args.first().map(|v| v.as_slice()).unwrap_or(&[]),
                        r.pos,
                    );
                }
                Some(Sym::EntryDecl(d)) => {
                    let callee = self.external_callee(&d.name);
                    return self.call_entry_decl(
                        callee,
                        &d,
                        args.first().map(|v| v.as_slice()).unwrap_or(&[]),
                        r.pos,
                    );
                }
                Some(Sym::Builtin(b)) => {
                    return self.builtin(b, args.first().map(|v| v.as_slice()), r.pos);
                }
                Some(Sym::Cond(_)) => {
                    return Err(self.err(r.pos, format!("condition {name} used in an expression")));
                }
            }
            if self.resolve_var(&names).is_none() {
                // the CICS translator's constants
                if matches!(name.as_str(), "DFHRESP" | "DFHVALUE")
                    && let [arg] = args.first().map(Vec::as_slice).unwrap_or(&[])
                    && let ast::Expr::Ref(c) = arg
                {
                    let cond = c.parts[0].0.as_str();
                    let v = if name == "DFHRESP" {
                        zeus_cobol::front::cics::resp_value(cond)
                    } else {
                        zeus_cobol::front::cics::cvda_value(cond)
                    };
                    let v = v.ok_or_else(|| self.err(r.pos, format!("{name}({cond}) is not known")))?;
                    return Ok(mk(Ek::Fixed(i128::from(v), 0), Ty::fixed_bin(31, 0)));
                }
                if let Some(b) = crate::builtin::lookup(name) {
                    return self.builtin(b, args.first().map(|v| v.as_slice()), r.pos);
                }
                if !args.is_empty() {
                    // an undeclared function: external entry
                    let callee = self.external_callee(name);
                    let c = name.chars().next().unwrap_or('A');
                    let returns = if ('I'..='N').contains(&c) {
                        Ty::fixed_bin(15, 0)
                    } else {
                        Ty::Float { dec: true, p: 6 }
                    };
                    let d = EntryDecl {
                        name: name.clone(),
                        params: None,
                        returns: Some(returns),
                    };
                    return self.call_entry_decl(callee, &d, &args[0], r.pos);
                }
            }
        }
        let rr = self.reference(r, true)?;
        // an ENTRY variable invoked with arguments is not supported
        self.load(rr, r.pos)
    }

    /// The callee for an external name: a procedure of this compilation
    /// or one resolved at run time.
    pub fn external_callee(&self, name: &str) -> Callee {
        for (i, p) in self.m.procs.iter().enumerate() {
            if p.external && p.name == name {
                return Callee::Proc(i as ProcId);
            }
        }
        Callee::External(name.to_string())
    }

    fn param_types(&self, callee: &Callee) -> Option<Vec<Option<Ty>>> {
        let vars = match callee {
            Callee::Proc(p) => self.m.procs[*p as usize].params.clone(),
            Callee::Entry(p, k) => self.m.procs[*p as usize]
                .entries
                .get(*k)
                .map(|e| e.1.clone())?,
            _ => return None,
        };
        Some(
            vars.iter()
                .map(|v| {
                    let var = &self.m.vars[*v as usize];
                    if var.dims.is_empty() {
                        var.ty.clone()
                    } else {
                        None
                    }
                })
                .collect(),
        )
    }

    /// Arguments of a call: by reference when the argument is a variable
    /// matching the parameter, otherwise a dummy.
    pub fn args(&mut self, params: Option<&[Option<Ty>]>, args: &[ast::Expr]) -> Result<Vec<Arg>> {
        let mut out = Vec::new();
        for (i, a) in args.iter().enumerate() {
            let pty = params.and_then(|p| p.get(i).cloned()).flatten();
            if let ast::Expr::Star(_) = a {
                out.push(Arg::Omitted);
                continue;
            }
            // a variable reference (not parenthesised)
            if let ast::Expr::Ref(r) = a {
                let names: Vec<String> = r.parts.iter().map(|p| p.0.clone()).collect();
                let is_var = matches!(self.sym(&names[0]), Some(Sym::Var(_)) | None)
                    && self.resolve_var(&names).is_some();
                if is_var {
                    let rr = self.reference(r, true)?;
                    let vty = self.m.vars[rr.var as usize].ty.clone();
                    let matches = match (&pty, &vty) {
                        (None, _) => true,
                        (Some(p), Some(v)) => types_match(p, v),
                        _ => false,
                    };
                    if self.is_aggregate(&rr) || matches {
                        out.push(Arg::Ref(rr));
                        continue;
                    }
                    let x = self.load(rr, r.pos)?;
                    let target = resolve_star(pty.as_ref().unwrap(), &x.ty);
                    out.push(Arg::Dummy(self.conv(x, &target)));
                    continue;
                }
            }
            let x = self.expr(a)?;
            let x = match &pty {
                Some(p) => {
                    let target = resolve_star(p, &x.ty);
                    self.conv(x, &target)
                }
                None => x,
            };
            out.push(Arg::Dummy(x));
        }
        Ok(out)
    }

    fn call_expr(&mut self, callee: Callee, args: &[ast::Expr], pos: Pos) -> Result<Expr> {
        let params = self.param_types(&callee);
        let returns = match &callee {
            Callee::Proc(p) | Callee::Entry(p, _) => self.m.procs[*p as usize].returns.clone(),
            _ => None,
        };
        let returns =
            returns.ok_or_else(|| self.err(pos, "procedure without RETURNS used as a function"))?;
        let a = self.args(params.as_deref(), args)?;
        Ok(mk(Ek::Call(callee, a), returns))
    }

    fn call_entry_decl(
        &mut self,
        callee: Callee,
        d: &EntryDecl,
        args: &[ast::Expr],
        pos: Pos,
    ) -> Result<Expr> {
        let params = match &callee {
            Callee::Proc(_) => self.param_types(&callee),
            _ => d.params.clone(),
        };
        let returns = d
            .returns
            .clone()
            .or_else(|| match &callee {
                Callee::Proc(p) => self.m.procs[*p as usize].returns.clone(),
                _ => None,
            })
            .ok_or_else(|| self.err(pos, format!("{} has no RETURNS attribute", d.name)))?;
        let a = self.args(params.as_deref(), args)?;
        Ok(mk(Ek::Call(callee, a), returns))
    }

    /// A CALL statement's callee and arguments.
    pub fn call_stmt(&mut self, r: &ast::Ref) -> Result<Stmt> {
        let name = r.parts[0].0.clone();
        let args: &[ast::Expr] = r.parts[0].1.first().map(|v| v.as_slice()).unwrap_or(&[]);
        match self.sym(&name) {
            Some(Sym::Proc(p)) => {
                let params = self.param_types(&Callee::Proc(p));
                let a = self.args(params.as_deref(), args)?;
                Ok(Stmt::Call(Callee::Proc(p), a))
            }
            Some(Sym::Entry(p, k)) => {
                let params = self.param_types(&Callee::Entry(p, k));
                let a = self.args(params.as_deref(), args)?;
                Ok(Stmt::Call(Callee::Entry(p, k), a))
            }
            Some(Sym::EntryDecl(d)) => {
                let callee = self.external_callee(&d.name);
                let params = match &callee {
                    Callee::Proc(_) => self.param_types(&callee),
                    _ => d.params.clone(),
                };
                let a = self.args(params.as_deref(), args)?;
                Ok(Stmt::Call(callee, a))
            }
            Some(Sym::Builtin(b)) => self.call_builtin(b, args, r.pos),
            Some(Sym::Var(v)) if matches!(self.m.vars[v as usize].ty, Some(Ty::Entry)) => {
                Err(self.err(r.pos, "CALL of an ENTRY variable is not supported"))
            }
            Some(_) => Err(self.err(r.pos, format!("{name} cannot be called"))),
            None => {
                if let Some(b) = crate::builtin::lookup(&name) {
                    return self.call_builtin(b, args, r.pos);
                }
                let callee = self.external_callee(&name);
                let params = self.param_types(&callee);
                let a = self.args(params.as_deref(), args)?;
                Ok(Stmt::Call(callee, a))
            }
        }
    }

    fn call_builtin(&mut self, b: Bif, args: &[ast::Expr], pos: Pos) -> Result<Stmt> {
        use crate::builtin::Bif::*;
        match b {
            PliRetC => {
                let x = self.expr(
                    args.first()
                        .ok_or_else(|| self.err(pos, "PLIRETC needs an argument"))?,
                )?;
                let x = self.to_arith(x);
                Ok(Stmt::CallBuiltin(
                    b,
                    vec![self.conv(x, &Ty::fixed_bin(31, 0))],
                ))
            }
            PliDump => Ok(Stmt::CallBuiltin(b, Vec::new())),
            PliFill | PliMove => {
                let mut v = Vec::new();
                for a in args {
                    v.push(self.expr(a)?);
                }
                Ok(Stmt::CallBuiltin(b, v))
            }
            _ => Err(self.err(pos, format!("built-in subroutine {b:?} is not supported"))),
        }
    }

    /// A built-in function reference.
    pub fn builtin(&mut self, b: Bif, args: Option<&[ast::Expr]>, pos: Pos) -> Result<Expr> {
        use crate::builtin::Bif::*;
        let args = match args {
            Some(a) => a,
            None if b.no_args() => &[],
            None => return Err(self.err(pos, format!("built-in {b:?} needs arguments"))),
        };
        match b {
            Null => return Ok(mk(Ek::Null, Ty::Pointer)),
            SysNull => return Ok(mk(Ek::SysNull, Ty::Pointer)),
            OnCode | OnChar | OnSource | OnKey | OnFile | OnLoc | OnCount | DataField => {
                let ty = crate::builtin::result_type(b, &[], &[]).map_err(|e| self.err(pos, e))?;
                return Ok(mk(Ek::OnValue(b), ty));
            }
            _ => {}
        }
        if b.takes_ref() {
            let first = args
                .first()
                .ok_or_else(|| self.err(pos, format!("{b:?} needs an argument")))?;
            if let ast::Expr::Ref(r) = first {
                let names: Vec<std::string::String> = r.parts.iter().map(|p| p.0.clone()).collect();
                if self.resolve_var(&names).is_some()
                    || matches!(b, Addr | Size | CurrentSize) && self.resolve_var(&names).is_some()
                {
                    let rr = self.reference(r, true)?;
                    let mut rest = Vec::new();
                    for a in &args[1..] {
                        let x = self.expr(a)?;
                        let x = self.to_arith(x);
                        rest.push(self.conv(x, &Ty::fixed_bin(31, 0)));
                    }
                    let vty = self.m.vars[rr.var as usize]
                        .ty
                        .clone()
                        .unwrap_or(char_any());
                    let ty = match b {
                        String => {
                            if vty.is_bit() {
                                bit_any()
                            } else {
                                char_any()
                            }
                        }
                        Unspec => bit_any(),
                        Sum | Prod | All | Any => crate::builtin::result_type(b, &[vty], &[])
                            .map_err(|e| self.err(pos, e))?,
                        MaxLength => Ty::fixed_bin(31, 0),
                        _ => crate::builtin::result_type(b, &[vty], &[])
                            .map_err(|e| self.err(pos, e))?,
                    };
                    let aggregate = self.is_aggregate(&rr);
                    if matches!(
                        b,
                        Sum | Prod
                            | All
                            | Any
                            | Dim
                            | Lbound
                            | Hbound
                            | Addr
                            | Size
                            | CurrentSize
                            | String
                            | Allocation
                            | Present
                            | Omitted
                            | Unspec
                    ) || aggregate
                    {
                        return Ok(mk(Ek::RefBuiltin(b, rr, rest), ty));
                    }
                    // MAXLENGTH, LENGTH of a scalar: on its value
                    let x = self.load(rr, pos)?;
                    let mut v = vec![x];
                    v.extend(rest);
                    return Ok(mk(Ek::Builtin(b, v), ty));
                }
            }
        }
        // value arguments
        let mut xs = Vec::new();
        for a in args {
            xs.push(self.expr(a)?);
        }
        let consts: Vec<Option<i64>> = args.iter().map(|a| self.const_int(a)).collect();
        let tys: Vec<Ty> = xs.iter().map(|x| x.ty.clone()).collect();
        let rt = crate::builtin::result_type(b, &tys, &consts).map_err(|e| self.err(pos, e))?;
        let int = |c: &Compiler, x: Expr| {
            let x = c.to_arith(x);
            c.conv(x, &Ty::fixed_bin(31, 0))
        };
        let xs: Vec<Expr> = match b {
            // conversion built-ins
            Binary | Decimal | Fixed | Float | FixedBin | FixedDec | FloatBin | FloatDec
            | Precision | Signed | Unsigned => {
                let x = self.to_arith(xs.into_iter().next().unwrap());
                return Ok(self.conv(x, &rt));
            }
            Char => {
                let x = xs.into_iter().next().unwrap();
                return Ok(match consts.get(1).copied().flatten() {
                    Some(_) => self.conv(x, &rt),
                    None => self.to_char(x),
                });
            }
            Bit => {
                let x = xs.into_iter().next().unwrap();
                return Ok(match consts.get(1).copied().flatten() {
                    Some(_) => self.conv(x, &rt),
                    None => self.to_bit(x),
                });
            }
            Add | Subtract | Multiply | Divide => {
                let mut it = xs.into_iter();
                let (a, c) = (it.next().unwrap(), it.next().unwrap());
                let op = match b {
                    Add => ArOp::Add,
                    Subtract => ArOp::Sub,
                    Multiply => ArOp::Mul,
                    _ => ArOp::Div,
                };
                let a = self.to_arith(a);
                let c = self.to_arith(c);
                let a = self.to_base(a, &rt);
                let c = self.to_base(c, &rt);
                return Ok(mk(Ek::Arith(op, a, c), rt));
            }
            Max | Min | Mod | Rem => {
                let common = crate::builtin::common(&tys);
                xs.into_iter()
                    .map(|x| {
                        let x = self.to_arith(x);
                        self.to_base(x, &common)
                    })
                    .collect()
            }
            Abs | Sign | Round | Trunc | Ceil | Floor => {
                let mut it = xs.into_iter();
                let mut v = vec![self.to_arith(it.next().unwrap())];
                v.extend(it.map(|x| int(self, x)));
                v
            }
            Sqrt | Exp | Log | Log2 | Log10 | Sin | Cos | Tan | Sinh | Cosh | Tanh | Asin
            | Acos | Atan | Atanh | Sind | Cosd | Tand | Atand => xs
                .into_iter()
                .map(|x| {
                    let x = self.to_arith(x);
                    self.conv(x, &rt)
                })
                .collect(),
            Substr | Index | Search | Verify | Tally | Copy | Repeat | Left | Right | Center
            | CenterRight | Trim | Translate | Lowercase | Uppercase | Reverse | Length
            | MaxLength | Hex | Bool | High | Low | Unspec => {
                // string arguments stay strings; others to CHAR; counts to
                // FIXED BIN(31)
                let string_args: &[usize] = match b {
                    Substr | Copy | Repeat | Reverse | Lowercase | Uppercase | Length
                    | MaxLength | Hex | Unspec => &[0],
                    Index | Search | Verify | Tally | Bool => &[0, 1],
                    Left | Right | Center | CenterRight => &[0, 2],
                    Trim | Translate => &[0, 1, 2],
                    _ => &[],
                };
                let bitctx = matches!(b, Bool)
                    || (string_args.first() == Some(&0)
                        && xs.first().is_some_and(|x| x.ty.is_bit())
                        && !matches!(
                            b,
                            Hex | Left
                                | Right
                                | Center
                                | CenterRight
                                | Lowercase
                                | Uppercase
                                | Trim
                                | Translate
                        ));
                xs.into_iter()
                    .enumerate()
                    .map(|(i, x)| {
                        if string_args.contains(&i) {
                            if matches!(b, Hex | Unspec) {
                                x
                            } else if bitctx && (x.ty.is_bit() || matches!(b, Bool)) {
                                self.to_bit(x)
                            } else if x.ty.is_bit() && bitctx {
                                x
                            } else {
                                self.to_char(x)
                            }
                        } else {
                            int(self, x)
                        }
                    })
                    .collect()
            }
            _ => xs
                .into_iter()
                .map(|x| {
                    if x.ty.arith().is_some() && !x.ty.is_string() {
                        int(self, x)
                    } else {
                        x
                    }
                })
                .collect(),
        };
        // string results take the kind of their first argument
        let rt = match b {
            Substr | Copy | Repeat | Reverse => {
                if xs[0].ty.is_bit() {
                    bit_any()
                } else {
                    char_any()
                }
            }
            Trim | Translate | Lowercase | Uppercase => char_any(),
            _ => rt,
        };
        Ok(mk(Ek::Builtin(b, xs), rt))
    }

    /// An assignment target: a scalar or aggregate reference or a
    /// pseudovariable.
    pub fn target(&mut self, e: &ast::Expr) -> Result<(Target, Option<Ty>)> {
        let ast::Expr::Ref(r) = e else {
            return Err(self.err(e.pos(), "invalid assignment target"));
        };
        if r.parts.len() == 1 && r.locator.is_none() && !r.parts[0].1.is_empty() {
            let name = &r.parts[0].0;
            let pseudo = match self.sym(name) {
                Some(Sym::Builtin(b)) => Some(b),
                None if self.resolve_var(std::slice::from_ref(name)).is_none() => {
                    crate::builtin::lookup(name)
                }
                _ => None,
            };
            if let Some(b) = pseudo {
                let args = &r.parts[0].1[0];
                let ast::Expr::Ref(inner) = args
                    .first()
                    .ok_or_else(|| self.err(r.pos, "pseudovariable needs an argument"))?
                else {
                    return Err(self.err(r.pos, "pseudovariable needs a variable"));
                };
                let rr = self.reference(inner, true)?;
                return match b {
                    Bif::Substr => {
                        let mut it = args[1..].iter();
                        let s = it
                            .next()
                            .ok_or_else(|| self.err(r.pos, "SUBSTR needs a start"))?;
                        let s = self.expr(s)?;
                        let s = self.to_arith(s);
                        let s = self.conv(s, &Ty::fixed_bin(31, 0));
                        let n = match it.next() {
                            Some(n) => {
                                let n = self.expr(n)?;
                                let n = self.to_arith(n);
                                Some(self.conv(n, &Ty::fixed_bin(31, 0)))
                            }
                            None => None,
                        };
                        let ty = if self.m.vars[rr.var as usize]
                            .ty
                            .as_ref()
                            .is_some_and(|t| t.is_bit())
                        {
                            bit_any()
                        } else {
                            char_any()
                        };
                        Ok((Target::Substr(rr, s, n), Some(ty)))
                    }
                    Bif::Unspec => Ok((Target::Unspec(rr), Some(bit_any()))),
                    Bif::String => Ok((Target::String(rr), Some(char_any()))),
                    _ => Err(self.err(r.pos, format!("{b:?} is not a pseudovariable"))),
                };
            }
        }
        let rr = self.reference(r, true)?;
        let ty = self.m.vars[rr.var as usize].ty.clone();
        Ok((Target::Ref(rr), ty))
    }
}

/// Whether an argument's attributes match a parameter (no dummy needed).
fn types_match(p: &Ty, v: &Ty) -> bool {
    match (p, v) {
        (Ty::Char { len: pl, var: pv }, Ty::Char { len: vl, var: vv }) => {
            pv == vv && (*pl == Ext::Star || pl == vl)
        }
        (
            Ty::Bit {
                len: pl, var: pv, ..
            },
            Ty::Bit {
                len: vl, var: vv, ..
            },
        ) => pv == vv && (*pl == Ext::Star || pl == vl),
        _ => p == v,
    }
}

/// The type of a dummy argument: `*` extents from the argument.
fn resolve_star(p: &Ty, a: &Ty) -> Ty {
    match p {
        Ty::Char {
            len: Ext::Star,
            var,
        } => Ty::Char {
            len: a.str_len().unwrap_or(Ext::Star),
            var: *var,
        },
        Ty::Bit {
            len: Ext::Star,
            var,
            aligned,
        } => Ty::Bit {
            len: a.str_len().unwrap_or(Ext::Star),
            var: *var,
            aligned: *aligned,
        },
        t => t.clone(),
    }
}

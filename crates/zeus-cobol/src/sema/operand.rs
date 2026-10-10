//! Lowering of data references, operands, expressions and conditions.

use zeus_decimal::Dec;

use super::codegen::Gen;
use super::lit;
use crate::diag::{Diag, Result};
use crate::front::ast::{
    self, BinOp, ClassTest, Cond, DataRef, Expr, Figurative, Literal, Operand, RelOp, SignTest,
    Subscript,
};
use crate::zir::{
    Arg, ClassKind, CmpMode, Cx, DataType, Ex, Fig, Func, Loc, Op, Rel, SignKind, Src,
};

pub fn rel(op: RelOp) -> Rel {
    match op {
        RelOp::Eq => Rel::Eq,
        RelOp::Ne => Rel::Ne,
        RelOp::Lt => Rel::Lt,
        RelOp::Le => Rel::Le,
        RelOp::Gt => Rel::Gt,
        RelOp::Ge => Rel::Ge,
    }
}

fn fig(f: &Figurative) -> Option<Fig> {
    Some(match f {
        Figurative::Zero => Fig::Zero,
        Figurative::Space => Fig::Space,
        Figurative::HighValue => Fig::High,
        Figurative::LowValue => Fig::Low,
        Figurative::Quote => Fig::Quote,
        Figurative::Symbolic(_) => return None,
    })
}

/// Alphanumeric-valued intrinsic functions.
fn alnum_function(f: Func) -> bool {
    matches!(
        f,
        Func::Char
            | Func::CurrentDate
            | Func::LowerCase
            | Func::UpperCase
            | Func::Reverse
            | Func::Trim
            | Func::WhenCompiled
    )
}

pub fn function(name: &str) -> Option<Func> {
    Some(match name {
        "ABS" => Func::Abs,
        "ACOS" => Func::Acos,
        "ANNUITY" => Func::Annuity,
        "ASIN" => Func::Asin,
        "ATAN" => Func::Atan,
        "CHAR" => Func::Char,
        "COS" => Func::Cos,
        "CURRENT-DATE" => Func::CurrentDate,
        "DATE-OF-INTEGER" => Func::DateOfInteger,
        "DAY-OF-INTEGER" => Func::DayOfInteger,
        "FACTORIAL" => Func::Factorial,
        "INTEGER-OF-DATE" => Func::IntegerOfDate,
        "INTEGER-OF-DAY" => Func::IntegerOfDay,
        "INTEGER" => Func::Integer,
        "INTEGER-PART" => Func::IntegerPart,
        "LENGTH" => Func::Length,
        "LOG" => Func::Log,
        "LOG10" => Func::Log10,
        "LOWER-CASE" => Func::LowerCase,
        "MAX" => Func::Max,
        "MEAN" => Func::Mean,
        "MEDIAN" => Func::Median,
        "MIDRANGE" => Func::Midrange,
        "MIN" => Func::Min,
        "MOD" => Func::Mod,
        "NUMVAL" => Func::NumVal,
        "NUMVAL-C" => Func::NumValC,
        "ORD" => Func::Ord,
        "ORD-MAX" => Func::OrdMax,
        "ORD-MIN" => Func::OrdMin,
        "PRESENT-VALUE" => Func::PresentValue,
        "RANDOM" => Func::Random,
        "RANGE" => Func::Range,
        "REM" => Func::Rem,
        "REVERSE" => Func::Reverse,
        "SIN" => Func::Sin,
        "SQRT" => Func::Sqrt,
        "STANDARD-DEVIATION" => Func::StandardDeviation,
        "SUM" => Func::Sum,
        "TAN" => Func::Tan,
        "UPPER-CASE" => Func::UpperCase,
        "VARIANCE" => Func::Variance,
        "WHEN-COMPILED" => Func::WhenCompiled,
        "DATE-TO-YYYYMMDD" => Func::DateToYyyymmdd,
        "DAY-TO-YYYYDDD" => Func::DayToYyyyddd,
        "YEAR-TO-YYYY" => Func::YearToYyyy,
        "TRIM" => Func::Trim,
        "TEST-NUMVAL" => Func::TestNumVal,
        "TEST-NUMVAL-C" => Func::TestNumValC,
        _ => return None,
    })
}

impl Gen<'_> {
    /// A data reference as a location.
    pub fn loc(&mut self, r: &DataRef) -> Result<Loc> {
        let id = self.dm.resolve(r)?;
        if self.dm.items[id].cond_of.is_some() {
            return Err(Diag::error(
                r.pos,
                format!("{} is a condition-name", r.name),
            ));
        }
        self.loc_of(id, r)
    }

    pub fn loc_of(&mut self, id: usize, r: &DataRef) -> Result<Loc> {
        let mut subs = Vec::new();
        for s in &r.subscripts {
            match s {
                Subscript::Expr(e) => subs.push(self.ex(e)?),
                Subscript::All => {
                    return Err(Diag::error(
                        r.pos,
                        "ALL subscript is only allowed in function arguments",
                    ));
                }
            }
        }
        let refmod = match &r.refmod {
            Some((s, l)) => Some((
                Box::new(self.ex(s)?),
                match l {
                    Some(l) => Some(Box::new(self.ex(l)?)),
                    None => None,
                },
            )),
            None => None,
        };
        let desc = self.dm.items[id].desc;
        let dims = self.dm.descs[desc as usize].dims.len();
        if !subs.is_empty() && subs.len() != dims {
            return Err(Diag::error(
                r.pos,
                format!(
                    "{} needs {} subscript(s), {} given",
                    r.name,
                    dims,
                    subs.len()
                ),
            ));
        }
        Ok(Loc { desc, subs, refmod })
    }

    pub fn desc_type(&self, loc: &Loc) -> &DataType {
        &self.dm.descs[loc.desc as usize].ty
    }

    /// Whether an operand has a numeric value (for comparisons).
    pub fn is_numeric_operand(&self, op: &Operand) -> bool {
        match op {
            Operand::Lit(Literal::Num(_)) => true,
            Operand::Ref(r) => self.dm.resolve(r).ok().is_some_and(|id| {
                let it = &self.dm.items[id];
                it.cond_of.is_none()
                    && r.refmod.is_none()
                    && self.dm.descs[it.desc as usize].ty.is_numeric()
            }),
            Operand::Func(f) => {
                function(&f.name).is_some_and(|f| !alnum_function(f))
                    && !matches!(function(&f.name), Some(Func::Max | Func::Min))
                    || matches!(function(&f.name), Some(Func::Max | Func::Min))
                        && f.args.first().is_some_and(|a| self.is_numeric_expr(a))
            }
            Operand::LengthOf(_) => true,
            _ => false,
        }
    }

    pub fn is_numeric_expr(&self, e: &Expr) -> bool {
        match e {
            Expr::Operand(op) => self.is_numeric_operand(op),
            _ => true,
        }
    }

    /// Numeric expression.
    pub fn ex(&mut self, e: &Expr) -> Result<Ex> {
        Ok(match e {
            Expr::Operand(op) => self.ex_operand(op)?,
            Expr::Neg(x) => Ex::Neg(Box::new(self.ex(x)?)),
            Expr::Bin(op, a, b) => {
                let op = match op {
                    BinOp::Add => Op::Add,
                    BinOp::Sub => Op::Sub,
                    BinOp::Mul => Op::Mul,
                    BinOp::Div => Op::Div,
                    BinOp::Pow => Op::Pow,
                };
                Ex::Bin(op, Box::new(self.ex(a)?), Box::new(self.ex(b)?))
            }
        })
    }

    pub fn ex_operand(&mut self, op: &Operand) -> Result<Ex> {
        Ok(match op {
            Operand::Lit(Literal::Num(n)) => {
                let (d, _) = lit::numeric(n).map_err(|m| Diag::error(self.pos, m))?;
                Ex::Lit(d)
            }
            Operand::Fig(Figurative::Zero) => Ex::Lit(Dec::ZERO),
            Operand::Ref(r) => Ex::Load(self.loc(r)?),
            Operand::LengthOf(r) => Ex::LengthOf(self.loc(r)?),
            Operand::AddressOf(r) => Ex::AddressOf(self.loc(r)?),
            Operand::Null => Ex::Lit(Dec::ZERO),
            Operand::Func(f) => {
                let func = function(&f.name)
                    .ok_or_else(|| Diag::error(f.pos, format!("unknown function {}", f.name)))?;
                let args = self.func_args(&f.args)?;
                Ex::Func(func, args)
            }
            Operand::Lit(l) => {
                // An alphanumeric literal in an arithmetic context: digits.
                let bytes = lit::literal_bytes(l);
                let v = crate::vm::data::alnum_as_number(&bytes);
                Ex::Lit(v)
            }
            other => {
                return Err(Diag::error(
                    self.pos,
                    format!("operand not allowed in arithmetic: {other:?}"),
                ));
            }
        })
    }

    fn func_args(&mut self, args: &[Expr]) -> Result<Vec<Arg>> {
        let mut out = Vec::new();
        for a in args {
            // TRIM (x LEADING / TRAILING)
            if let Expr::Operand(Operand::Ref(r)) = a
                && (r.name == "LEADING" || r.name == "TRAILING")
                && r.qualifiers.is_empty()
                && self.dm.resolve(r).is_err()
            {
                let flag = if r.name == "LEADING" { "L" } else { "T" };
                out.push(Arg::Src(Src::Alnum(crate::ebcdic::encode(flag))));
                continue;
            }
            if let Expr::Operand(Operand::Ref(r)) = a
                && r.subscripts.iter().any(|s| matches!(s, Subscript::All))
            {
                let id = self.dm.resolve(r)?;
                let mut rr = r.clone();
                rr.subscripts.clear();
                let loc = self.loc_of(id, &rr)?;
                out.push(Arg::All(loc));
                continue;
            }
            if self.is_numeric_expr(a) {
                out.push(Arg::Num(self.ex(a)?));
            } else {
                out.push(Arg::Src(self.src_expr(a)?));
            }
        }
        Ok(out)
    }

    /// General operand (MOVE source, comparison operand, DISPLAY item).
    pub fn src(&mut self, op: &Operand) -> Result<Src> {
        Ok(match op {
            Operand::Ref(r) => {
                if let Some(sym) = self.symbolic_char(r) {
                    return Ok(Src::Alnum(vec![sym]));
                }
                Src::Loc(self.loc(r)?)
            }
            Operand::Lit(Literal::Num(n)) => {
                let (d, digits) = lit::numeric(n).map_err(|m| Diag::error(self.pos, m))?;
                Src::NumLit(d, digits)
            }
            Operand::Lit(l) => Src::Alnum(lit::literal_bytes(l)),
            Operand::Fig(Figurative::Symbolic(name)) => {
                let b = self.symbolic_byte(name)?;
                Src::Alnum(vec![b])
            }
            Operand::Fig(f) => Src::Fig(fig(f).unwrap()),
            Operand::All(inner) => match inner.as_ref() {
                Operand::Lit(l) => Src::All(lit::literal_bytes(l)),
                Operand::Fig(f) => match fig(f) {
                    Some(f) => Src::Fig(f),
                    None => return Err(Diag::error(self.pos, "invalid ALL operand")),
                },
                Operand::Ref(r) => match self.symbolic_char(r) {
                    Some(b) => Src::All(vec![b]),
                    None => return Err(Diag::error(r.pos, "ALL requires a literal")),
                },
                _ => return Err(Diag::error(self.pos, "invalid ALL operand")),
            },
            Operand::Func(f) => {
                let func = function(&f.name)
                    .ok_or_else(|| Diag::error(f.pos, format!("unknown function {}", f.name)))?;
                let numeric = self.is_numeric_operand(op);
                let args = self.func_args(&f.args)?;
                if numeric {
                    Src::Num(Ex::Func(func, args))
                } else {
                    let refmod = match &f.refmod {
                        Some((s, l)) => Some((
                            Box::new(self.ex(s)?),
                            match l {
                                Some(l) => Some(Box::new(self.ex(l)?)),
                                None => None,
                            },
                        )),
                        None => None,
                    };
                    Src::Func(func, args, refmod)
                }
            }
            Operand::LengthOf(r) => Src::Num(Ex::LengthOf(self.loc(r)?)),
            Operand::AddressOf(r) => Src::Num(Ex::AddressOf(self.loc(r)?)),
            Operand::Null => Src::Num(Ex::Lit(Dec::ZERO)),
        })
    }

    pub fn src_expr(&mut self, e: &Expr) -> Result<Src> {
        match e {
            Expr::Operand(op) => self.src(op),
            other => Ok(Src::Num(self.ex(other)?)),
        }
    }

    fn symbolic_char(&self, r: &DataRef) -> Option<u8> {
        if !r.qualifiers.is_empty() || !r.subscripts.is_empty() {
            return None;
        }
        self.symbolics.get(&r.name).copied()
    }

    fn symbolic_byte(&self, name: &str) -> Result<u8> {
        self.symbolics
            .get(name)
            .copied()
            .ok_or_else(|| Diag::error(self.pos, format!("unknown symbolic character {name}")))
    }

    // ---- conditions ----------------------------------------------------

    pub fn cond(&mut self, c: &Cond) -> Result<Cx> {
        Ok(match c {
            Cond::Not(x) => Cx::Not(Box::new(self.cond(x)?)),
            Cond::And(a, b) => Cx::And(Box::new(self.cond(a)?), Box::new(self.cond(b)?)),
            Cond::Or(a, b) => Cx::Or(Box::new(self.cond(a)?), Box::new(self.cond(b)?)),
            Cond::Rel(a, op, b) => self.relation(a, *op, b)?,
            Cond::Class(e, t) => {
                let src = self.src_expr(e)?;
                let kind = match t {
                    ClassTest::Numeric => ClassKind::Numeric,
                    ClassTest::Alphabetic => ClassKind::Alphabetic,
                    ClassTest::AlphabeticLower => ClassKind::Lower,
                    ClassTest::AlphabeticUpper => ClassKind::Upper,
                    ClassTest::Class(name) => {
                        ClassKind::Set(self.classes.get(name).cloned().ok_or_else(|| {
                            Diag::error(self.pos, format!("unknown class {name}"))
                        })?)
                    }
                };
                Cx::Class(src, kind)
            }
            Cond::Sign(e, s) => Cx::Sign(
                self.ex(e)?,
                match s {
                    SignTest::Positive => SignKind::Positive,
                    SignTest::Negative => SignKind::Negative,
                    SignTest::Zero => SignKind::Zero,
                },
            ),
            Cond::Name(r) => self.condition_name(r)?,
        })
    }

    pub fn condition_name(&mut self, r: &DataRef) -> Result<Cx> {
        if r.qualifiers.is_empty()
            && r.subscripts.is_empty()
            && let Some(&(sw, on)) = self.switch_conds.get(&r.name)
        {
            return Ok(Cx::Switch(sw, on));
        }
        let id = self.dm.resolve(r)?;
        let Some(var) = self.dm.items[id].cond_of else {
            // A data item used as a condition: treat as "not zero/space".
            return Err(Diag::error(
                r.pos,
                format!("{} is not a condition-name", r.name),
            ));
        };
        let mut vr = r.clone();
        vr.name = self.dm.items[var].name.clone().unwrap_or_default();
        vr.qualifiers.clear();
        let loc = self.loc_of(var, &vr)?;
        let numeric = self.desc_type(&loc).is_numeric();
        let mode = if numeric {
            CmpMode::Numeric
        } else {
            CmpMode::Alnum
        };
        let values = match &self.dm.items[id].entry.value {
            Some(ast::ValueClause::Condition(v)) => v.clone(),
            Some(ast::ValueClause::Literal(v)) => vec![(v.clone(), None)],
            None => return Err(Diag::error(r.pos, "condition-name without VALUE")),
        };
        let mut out: Option<Cx> = None;
        for (from, thru) in values {
            let a = self.src(&from)?;
            let c = match thru {
                None => Cx::Cmp(Src::Loc(loc.clone()), Rel::Eq, a, mode),
                Some(t) => {
                    let b = self.src(&t)?;
                    Cx::And(
                        Box::new(Cx::Cmp(Src::Loc(loc.clone()), Rel::Ge, a, mode)),
                        Box::new(Cx::Cmp(Src::Loc(loc.clone()), Rel::Le, b, mode)),
                    )
                }
            };
            out = Some(match out {
                None => c,
                Some(prev) => Cx::Or(Box::new(prev), Box::new(c)),
            });
        }
        Ok(out.unwrap_or(Cx::True))
    }

    /// A bare reference to a condition-name or switch-status condition.
    pub fn condition_ref(&self, e: &Expr) -> Option<DataRef> {
        let Expr::Operand(Operand::Ref(r)) = e else {
            return None;
        };
        if r.qualifiers.is_empty()
            && r.subscripts.is_empty()
            && self.switch_conds.contains_key(&r.name)
        {
            return Some(r.clone());
        }
        let id = self.dm.resolve(r).ok()?;
        self.dm.items[id].cond_of.map(|_| r.clone())
    }

    pub fn relation(&mut self, a: &Expr, op: RelOp, b: &Expr) -> Result<Cx> {
        // An abbreviated relation whose object is a condition-name is the
        // condition itself ("A = 1 AND COND-NAME").
        if let Some(r) = self.condition_ref(b) {
            return self.condition_name(&r);
        }
        let fig_zero = |e: &Expr| matches!(e, Expr::Operand(Operand::Fig(Figurative::Zero)));
        let na = self.is_numeric_expr(a) || (fig_zero(a) && self.is_numeric_expr(b));
        let nb = self.is_numeric_expr(b) || (fig_zero(b) && self.is_numeric_expr(a));
        let mode = if na && nb {
            CmpMode::Numeric
        } else {
            CmpMode::Alnum
        };
        let (sa, sb) = if mode == CmpMode::Numeric {
            (Src::Num(self.ex(a)?), Src::Num(self.ex(b)?))
        } else {
            (self.src_expr(a)?, self.src_expr(b)?)
        };
        Ok(Cx::Cmp(sa, rel(op), sb, mode))
    }
}

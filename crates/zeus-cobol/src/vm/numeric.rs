//! COBOL expression evaluation without host floating-point or i128 intermediates.
use super::{Machine, RResult, RunError, SIZE_ERROR};
use crate::{
    numeric::number::Number,
    zir::{self, DataType, Ex, Func, Src},
};

impl Machine {
    pub(super) fn wide_numeric_function(&self, cur: usize, f: Func, args: &[zir::Arg]) -> bool {
        Self::aggregate_function(f)
            || matches!(
                f,
                Func::Integer | Func::IntegerPart | Func::Mod | Func::Rem | Func::Factorial
            )
            || (matches!(f, Func::OrdMax | Func::OrdMin)
                && args.iter().all(|a| match a {
                    zir::Arg::Num(_) | zir::Arg::Src(Src::Float(_) | Src::Num(_)) => true,
                    zir::Arg::Src(Src::Loc(loc))
                    | zir::Arg::All(loc)
                    | zir::Arg::Table { loc, .. } => self.loc_type(cur, loc).is_numeric(),
                    _ => false,
                }))
    }
    pub(super) fn aggregate_function(f: Func) -> bool {
        matches!(
            f,
            Func::Abs
                | Func::Sum
                | Func::Mean
                | Func::Min
                | Func::Max
                | Func::Median
                | Func::Midrange
                | Func::Range
                | Func::Variance
                | Func::StandardDeviation
                | Func::Annuity
                | Func::PresentValue
        )
    }
    fn floating_argument(&self, cur: usize, arg: &zir::Arg) -> bool {
        match arg {
            zir::Arg::Num(ex) => self.float_expression(cur, ex),
            zir::Arg::Src(Src::Loc(loc)) | zir::Arg::All(loc) | zir::Arg::Table { loc, .. } => {
                self.loc_type(cur, loc).is_floating()
            }
            zir::Arg::Src(Src::Float(_)) => true,
            zir::Arg::Src(Src::Num(ex)) => self.float_expression(cur, ex),
            _ => false,
        }
    }
    pub(super) fn all_locations(&mut self, cur: usize, loc: &zir::Loc) -> RResult<Vec<zir::Loc>> {
        self.table_locations(cur, loc, &[])
    }

    pub(super) fn table_locations(
        &mut self,
        cur: usize,
        loc: &zir::Loc,
        subs: &[Option<Ex>],
    ) -> RResult<Vec<zir::Loc>> {
        let desc = self.instances[cur].code.descs[loc.desc as usize].clone();
        let owner = match desc.base {
            zir::Base::Outer(p) | zir::Base::OuterLinkage(p, _) => {
                let module = self.instances[cur].module.clone();
                self.instantiate(&module, p)
            }
            _ => cur,
        };
        let mut locations = vec![zir::Loc {
            desc: loc.desc,
            subs: vec![],
            refmod: loc.refmod.clone(),
        }];
        for (index, dim) in desc.dims.iter().enumerate() {
            let count = match &dim.count {
                Some(ex) => {
                    let count = self.eval_ex(owner, ex)?.coef;
                    if self.range_checks(cur) && (count < 1 || count > dim.max as i128) {
                        return super::err("SSRANGE: ALL table has an invalid occurrence count");
                    }
                    count.clamp(0, dim.max as i128) as u32
                }
                None => dim.max,
            };
            let fixed = match subs.get(index).and_then(Option::as_ref) {
                Some(ex) => {
                    let value = self.eval_ex(cur, ex)?;
                    let integer = value
                        .rescale(0, zeus_decimal::Rounding::Truncate)
                        .ok_or_else(|| RunError("table subscript outside integer range".into()))?;
                    if value != integer || integer.coef < 1 || integer.coef > count as i128 {
                        return super::err("table subscript out of bounds");
                    }
                    Some(integer.coef as u32)
                }
                None => None,
            };
            let mut next = vec![];
            for location in locations {
                for n in fixed.unwrap_or(1)..=fixed.unwrap_or(count) {
                    let mut location = location.clone();
                    location
                        .subs
                        .push(Ex::Lit(zeus_decimal::Dec::from_i64(n as i64)));
                    next.push(location);
                }
            }
            locations = next;
        }
        Ok(locations)
    }
    pub(super) fn float_expression(&self, cur: usize, ex: &Ex) -> bool {
        match ex {
            Ex::Float(_) => true,
            Ex::Load(loc) => self.loc_type(cur, loc).is_floating(),
            Ex::Bin(op, a, b) => {
                self.float_expression(cur, a)
                    || self.float_expression(cur, b)
                    || (matches!(op, zir::Op::Pow) && self.expression_decimals(cur, b) > 0)
            }
            Ex::Neg(x) => self.float_expression(cur, x),
            Ex::Temp(t) => self.instances[cur].wide_temps[*t as usize]
                .as_ref()
                .is_some_and(|x| x.floating),
            Ex::Func(f, args) => {
                matches!(
                    f,
                    Func::Sqrt
                        | Func::Log
                        | Func::Log10
                        | Func::Sin
                        | Func::Cos
                        | Func::Tan
                        | Func::Asin
                        | Func::Acos
                        | Func::Atan
                        | Func::Mean
                        | Func::Median
                        | Func::Midrange
                        | Func::Variance
                        | Func::StandardDeviation
                        | Func::Annuity
                        | Func::PresentValue
                        | Func::NumVal
                        | Func::NumValC
                        | Func::Random
                ) || ((Self::aggregate_function(*f) || *f == Func::Rem)
                    && args.iter().any(|a| self.floating_argument(cur, a)))
            }
            _ => false,
        }
    }
    pub(super) fn expression_decimals(&self, cur: usize, ex: &Ex) -> i32 {
        match ex {
            Ex::Lit(d) => d.scale as i32,
            Ex::Load(loc) => match self.loc_type(cur, loc) {
                DataType::Numeric(t) => t.scale.max(0),
                DataType::NumericEdited { scale, .. } => scale.max(0),
                _ => 0,
            },
            Ex::Bin(_, a, b) => self
                .expression_decimals(cur, a)
                .max(self.expression_decimals(cur, b)),
            Ex::Neg(x) => self.expression_decimals(cur, x),
            Ex::Temp(t) => self.instances[cur].wide_temps[*t as usize]
                .as_ref()
                .map_or(0, |x| x.decimal.max(0)),
            _ => 0,
        }
    }
    pub(super) fn wide_expression(
        &mut self,
        cur: usize,
        ex: &Ex,
        decimals: i32,
        floating: bool,
    ) -> RResult<Number> {
        let settings = self.instances[cur].code.numeric.unwrap_or_default();
        let floating = floating || self.float_expression(cur, ex);
        Ok(match ex {
            Ex::Lit(d) => Number::decimal(
                *d,
                d.integer_digits() as i32 + i32::from(d.is_zero() && d.scale == 0),
                d.scale as i32,
            ),
            Ex::Float(text) => Number::parse(text)
                .and_then(|n| n.as_float(settings.float_width(), true))
                .map_err(RunError)?,
            Ex::Load(loc) => {
                let (a, len) = self.addr(cur, loc)?;
                Number::from_item(&self.loc_type(cur, loc), &self.mem[a..a + len])
            }
            Ex::Temp(t) => self.instances[cur].wide_temps[*t as usize]
                .clone()
                .unwrap_or_else(|| {
                    let value = self.instances[cur].temps[*t as usize];
                    Number::decimal(
                        value,
                        value.integer_digits().max(1) as i32,
                        value.scale as i32,
                    )
                }),
            Ex::Neg(x) => self.wide_expression(cur, x, decimals, floating)?.negate(),
            Ex::Bin(op, a, b) => {
                let a = self.wide_expression(cur, a, decimals, floating)?;
                let b = self.wide_expression(cur, b, decimals, floating)?;
                a.binary(*op, b, decimals, floating, settings)
                    .map_err(|_| RunError(SIZE_ERROR.into()))?
            }
            Ex::Func(f, args) => {
                if *f == Func::Random {
                    use num_traits::{ToPrimitive, Zero};
                    if args.len() > 1 {
                        return super::err("RANDOM accepts at most one seed");
                    }
                    if let Some(arg) = args.first() {
                        let seed = match arg {
                            zir::Arg::Num(ex) => self.wide_expression(cur, ex, 0, false)?,
                            zir::Arg::Src(src) => self.numeric_source(cur, src, 0)?,
                            zir::Arg::All(_) | zir::Arg::Table { .. } => {
                                return super::err("RANDOM requires a scalar seed");
                            }
                        };
                        if (seed.value.negative && !seed.value.n.is_zero())
                            || &seed.value.n % &seed.value.d != num_bigint::BigUint::zero()
                        {
                            return super::err("RANDOM seed must be a nonnegative integer");
                        }
                        // IBM guarantees distinct sequences only for a smaller
                        // seed range; it does not require reducing seeds to it.
                        // Keep Zeus's existing 64-bit seed mapping so that
                        // applications retain their deterministic sequences.
                        let seed = ((&seed.value.n / &seed.value.d)
                            % (num_bigint::BigUint::from(1u8) << 64usize))
                            .to_u64()
                            .unwrap();
                        self.random = seed ^ 0x0005_DEEC_E66D;
                    }
                    self.random = self
                        .random
                        .wrapping_mul(6364136223846793005)
                        .wrapping_add(1442695040888963407);
                    return Number::random(self.random, settings).map_err(RunError);
                }
                if matches!(f, Func::NumVal | Func::NumValC) {
                    return self.numval_number(cur, *f, args);
                }
                if self.wide_numeric_function(cur, *f, args) {
                    let mut values = vec![];
                    for arg in args {
                        match arg {
                            zir::Arg::Num(ex) => {
                                values.push(self.wide_expression(cur, ex, decimals, floating)?)
                            }
                            zir::Arg::Src(src) => {
                                values.push(self.numeric_source(cur, src, decimals)?)
                            }
                            zir::Arg::All(loc) => {
                                for loc in self.all_locations(cur, loc)? {
                                    values.push(self.numeric_source(
                                        cur,
                                        &Src::Loc(loc),
                                        decimals,
                                    )?);
                                }
                            }
                            zir::Arg::Table { loc, subs } => {
                                for loc in self.table_locations(cur, loc, subs)? {
                                    values.push(self.numeric_source(
                                        cur,
                                        &Src::Loc(loc),
                                        decimals,
                                    )?);
                                }
                            }
                        }
                    }
                    return if Self::aggregate_function(*f) {
                        Number::aggregate(*f, &values, settings, floating)
                    } else {
                        Number::integer_function(*f, &values, settings, floating)
                    }
                    .map_err(RunError);
                }
                if matches!(
                    f,
                    Func::Sqrt
                        | Func::Log
                        | Func::Log10
                        | Func::Sin
                        | Func::Cos
                        | Func::Tan
                        | Func::Asin
                        | Func::Acos
                        | Func::Atan
                ) {
                    let arg = args
                        .first()
                        .ok_or_else(|| RunError("function requires an argument".into()))?;
                    let value = match arg {
                        zir::Arg::Num(ex) => self.wide_expression(cur, ex, decimals, false)?,
                        zir::Arg::Src(src) => self.numeric_source(cur, src, decimals)?,
                        zir::Arg::All(_) | zir::Arg::Table { .. } => {
                            return Err(RunError("function requires a scalar argument".into()));
                        }
                    };
                    return Number::function(*f, value, settings).map_err(RunError);
                }
                let value = self.func_num(cur, *f, args)?;
                Number::decimal(
                    value,
                    value.integer_digits().max(1) as i32,
                    value.scale as i32,
                )
            }
            Ex::LengthOf(_) | Ex::AddressOf(_) => {
                let value = self.eval_ex(cur, ex)?;
                Number::decimal(value, value.integer_digits().max(1) as i32, 0)
            }
        })
    }
    pub(super) fn numeric_store(
        &mut self,
        cur: usize,
        ex: &Ex,
        dst: &zir::Loc,
        rounded: bool,
        check: bool,
    ) -> RResult<()> {
        if self.instances[cur].code.numeric.is_none() {
            let value = self.eval_ex(cur, ex)?;
            return self.store(cur, value, dst, rounded, check);
        }
        let (decimals, context_float) = self.instances[cur].numeric_context;
        let ty = self.loc_type(cur, dst);
        let floating = context_float || ty.is_floating();
        let value = self.wide_expression(cur, ex, decimals, floating)?;
        let (a, len) = self.addr(cur, dst)?;
        let mut bytes = self.mem[a..a + len].to_vec();
        let overflow = match value.store(&ty, &mut bytes, rounded) {
            Ok(overflow) => overflow,
            Err(_) => {
                self.size_error = true;
                return Ok(());
            }
        };
        self.size_error |= overflow;
        if !check || !overflow {
            self.mem[a..a + len].copy_from_slice(&bytes);
        }
        Ok(())
    }
    pub(super) fn numeric_temp(&mut self, cur: usize, target: u32, ex: &Ex) -> RResult<()> {
        if self.instances[cur].code.numeric.is_none() {
            self.instances[cur].temps[target as usize] = self.eval_ex(cur, ex)?;
            return Ok(());
        }
        let (decimals, floating) = self.instances[cur].numeric_context;
        let value = self.wide_expression(cur, ex, decimals, floating)?;
        self.instances[cur].temps[target as usize] =
            value.to_dec().unwrap_or(zeus_decimal::Dec::ZERO);
        self.instances[cur].wide_temps[target as usize] = Some(value);
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn numeric_div_rem(
        &mut self,
        cur: usize,
        dividend: &Ex,
        divisor: &Ex,
        quot: &zir::Loc,
        rounded: bool,
        rem: &zir::Loc,
        check: bool,
    ) -> RResult<()> {
        if self.instances[cur].code.numeric.is_none() {
            let a = self.eval_ex(cur, dividend)?;
            let b = self.eval_ex(cur, divisor)?;
            return self.div_rem(cur, a, b, quot, rounded, rem, check);
        }
        let (decimals, floating) = self.instances[cur].numeric_context;
        let a = self.wide_expression(cur, dividend, decimals, floating)?;
        let b = self.wide_expression(cur, divisor, decimals, floating)?;
        if num_traits::Zero::is_zero(&b.value.n) {
            self.size_error = true;
            return Ok(());
        }
        let ty = self.loc_type(cur, quot);
        let scale = match &ty {
            DataType::Numeric(t) => t.scale,
            DataType::NumericEdited { scale, .. } => *scale,
            _ => 0,
        };
        let quotient = Number {
            value: a.value.divide(&b.value),
            integer: a.integer + b.decimal,
            decimal: decimals,
            floating: floating || a.floating || b.floating,
        };
        let truncated = Number::from_raw(quotient.raw(scale, false), scale);
        let mut product = truncated.value.multiply(&b.value);
        product.negative = !product.negative;
        let remainder = Number {
            value: a.value.add(&product, false),
            integer: a.integer,
            decimal: decimals,
            floating: quotient.floating,
        };
        let (qa, qlen) = self.addr(cur, quot)?;
        let mut qbytes = self.mem[qa..qa + qlen].to_vec();
        let overflow = match quotient.store(&ty, &mut qbytes, rounded) {
            Ok(x) => x,
            Err(_) => {
                self.size_error = true;
                return Ok(());
            }
        };
        self.size_error |= overflow;
        if overflow && check {
            return Ok(());
        }
        self.mem[qa..qa + qlen].copy_from_slice(&qbytes);
        let (ra, rlen) = self.addr(cur, rem)?;
        let mut rbytes = self.mem[ra..ra + rlen].to_vec();
        let overflow = match remainder.store(&self.loc_type(cur, rem), &mut rbytes, false) {
            Ok(x) => x,
            Err(_) => {
                self.size_error = true;
                return Ok(());
            }
        };
        self.size_error |= overflow;
        if !overflow || !check {
            self.mem[ra..ra + rlen].copy_from_slice(&rbytes);
        }
        Ok(())
    }
    pub(super) fn source_decimals(&self, cur: usize, src: &Src) -> i32 {
        match src {
            Src::Num(ex) => self.expression_decimals(cur, ex),
            Src::Loc(loc) => self.expression_decimals(cur, &Ex::Load(loc.clone())),
            Src::NumLit(d, _) => d.scale as i32,
            _ => 0,
        }
    }
    pub(super) fn numeric_source(
        &mut self,
        cur: usize,
        src: &Src,
        decimals: i32,
    ) -> RResult<Number> {
        match src {
            Src::Num(ex) => self.wide_expression(cur, ex, decimals, self.float_expression(cur, ex)),
            _ => {
                let value = self.eval_src(cur, src)?;
                Ok(match value {
                    super::eval::Val::Item(ty, bytes) => Number::from_item(&ty, &bytes),
                    v => {
                        let d = self.val_num(&v);
                        Number::decimal(d, d.integer_digits().max(1) as i32, d.scale as i32)
                    }
                })
            }
        }
    }
}

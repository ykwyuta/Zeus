//! Exact rational intermediates, bounded only at COBOL/HFP result boundaries.
use super::Settings;
use crate::hlasm::float::{Ratio, Rounding};
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, Signed, ToPrimitive, Zero};
use zeus_decimal::Dec;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(crate) struct Number {
    pub value: Ratio,
    pub integer: i32,
    pub decimal: i32,
    pub floating: bool,
}

impl Number {
    pub fn decimal(value: Dec, integer: i32, decimal: i32) -> Self {
        let factor = BigUint::from(10u8).pow(value.scale);
        Self {
            value: Ratio {
                negative: value.coef < 0,
                n: value.coef.unsigned_abs().into(),
                d: factor,
            },
            integer,
            decimal,
            floating: false,
        }
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        let (value, digits, exponent) = Ratio::decimal(text, 0)?;
        Ok(Self {
            value,
            integer: digits.len() as i32 + exponent,
            decimal: -exponent,
            floating: text.contains(['E', 'e']),
        })
    }
    pub fn zero() -> Self {
        Self::decimal(Dec::ZERO, 1, 0)
    }
    pub fn random(state: u64, settings: Settings) -> Result<Self, String> {
        Self {
            value: Ratio {
                negative: false,
                n: (state >> 8).max(1).into(),
                d: (BigUint::one() << 56),
            },
            integer: 0,
            decimal: 0,
            floating: true,
        }
        .as_float(settings.float_width(), true)
    }
    pub fn integer_function(
        func: crate::zir::Func,
        values: &[Self],
        settings: Settings,
        floating: bool,
    ) -> Result<Self, String> {
        use crate::zir::Func;
        if values.is_empty() {
            return Err("numeric function requires an argument".into());
        }
        let integer = |value: &Ratio, floor: bool| {
            let mut n = &value.n / &value.d;
            if floor && value.negative && &value.n % &value.d != BigUint::zero() {
                n += 1u8;
            }
            if value.negative {
                -BigInt::from(n)
            } else {
                BigInt::from(n)
            }
        };
        let from_integer = |value: BigInt| Self {
            integer: value.magnitude().to_str_radix(10).len() as i32,
            decimal: 0,
            floating: false,
            value: Ratio {
                negative: value.is_negative(),
                n: value.magnitude().clone(),
                d: BigUint::one(),
            },
        };
        if matches!(func, Func::OrdMax | Func::OrdMin) {
            let mut best = 0;
            for i in 1..values.len() {
                let order = values[i].compare(&values[best]);
                if (func == Func::OrdMax && order.is_gt())
                    || (func == Func::OrdMin && order.is_lt())
                {
                    best = i;
                }
            }
            return Ok(from_integer(BigInt::from(best + 1)));
        }
        let count = if matches!(func, Func::Mod | Func::Rem) {
            2
        } else {
            1
        };
        if values.len() != count {
            return Err(format!("{func:?} requires {count} arguments"));
        }
        let first = &values[0].value;
        Ok(match func {
            Func::Integer | Func::IntegerPart => {
                from_integer(integer(first, func == Func::Integer))
            }
            Func::Factorial => {
                let n = integer(first, false);
                let limit = match settings.arithmetic {
                    super::Arithmetic::Compat => 28,
                    super::Arithmetic::Extend => 29,
                };
                let n = n
                    .to_u32()
                    .filter(|&n| n <= limit && &first.n % &first.d == BigUint::zero())
                    .ok_or("FACTORIAL argument outside the ARITH range")?;
                from_integer((1..=n).fold(BigInt::one(), |a, b| a * b))
            }
            Func::Mod | Func::Rem => {
                let second = &values[1].value;
                if second.n.is_zero() {
                    return Err("MOD/REM by zero".into());
                }
                if func == Func::Mod
                    && values
                        .iter()
                        .any(|v| &v.value.n % &v.value.d != BigUint::zero())
                {
                    return Err("MOD arguments must be integers".into());
                }
                let q = from_integer(integer(&first.divide(second), func == Func::Mod));
                let mut product = second.multiply(&q.value);
                product.negative = !product.negative;
                let mut result = Self {
                    value: first.add(&product, false),
                    integer: values.iter().map(|v| v.integer).min().unwrap().max(1),
                    decimal: values.iter().map(|v| v.decimal).max().unwrap(),
                    floating: false,
                };
                if func == Func::Rem && (floating || values.iter().any(|v| v.floating)) {
                    result = result.as_float(settings.float_width(), true)?;
                }
                result
            }
            _ => return Err("unsupported integer function".into()),
        })
    }
    pub fn aggregate(
        func: crate::zir::Func,
        values: &[Self],
        settings: Settings,
        floating: bool,
    ) -> Result<Self, String> {
        use crate::zir::Func;
        if values.is_empty() {
            return Err("numeric function requires an argument".into());
        }
        // Reduce fractions between terms, keeping large tables linear in size.
        fn reduce(mut value: Ratio) -> Ratio {
            let (mut a, mut b) = (value.n.clone(), value.d.clone());
            while !b.is_zero() {
                let r = &a % &b;
                a = b;
                b = r;
            }
            if !a.is_zero() {
                value.n /= &a;
                value.d /= a;
            }
            value
        }
        let add = |a: &Ratio, b: &Ratio| reduce(a.add(b, false));
        let sub = |a: &Ratio, b: &Ratio| {
            let mut b = b.clone();
            b.negative = !b.negative;
            add(a, &b)
        };
        let one = Self::parse("1")?.value;
        let n = Self::decimal(Dec::from_i64(values.len() as i64), 10, 0).value;
        let mut ordered = values.to_vec();
        ordered.sort_by(Self::compare);
        let sum = || {
            values
                .iter()
                .fold(Self::zero().value, |a, b| add(&a, &b.value))
        };
        let value = match func {
            Func::Abs => {
                let mut value = values[0].value.clone();
                value.negative = false;
                value
            }
            Func::Sum => sum(),
            Func::Mean => reduce(sum().divide(&n)),
            Func::Min => ordered[0].value.clone(),
            Func::Max => ordered[ordered.len() - 1].value.clone(),
            Func::Range => sub(&ordered[ordered.len() - 1].value, &ordered[0].value),
            Func::Midrange => add(&ordered[0].value, &ordered[ordered.len() - 1].value)
                .divide(&Self::parse("2")?.value),
            Func::Median => {
                let k = ordered.len() / 2;
                if ordered.len() % 2 == 1 {
                    ordered[k].value.clone()
                } else {
                    add(&ordered[k - 1].value, &ordered[k].value).divide(&Self::parse("2")?.value)
                }
            }
            Func::Variance | Func::StandardDeviation => {
                let mean = reduce(sum().divide(&n));
                let variance = values
                    .iter()
                    .fold(Self::zero().value, |total, value| {
                        let delta = sub(&value.value, &mean);
                        add(&total, &reduce(delta.multiply(&delta)))
                    })
                    .divide(&n);
                if func == Func::StandardDeviation {
                    super::math::function(Func::Sqrt, &variance)?
                } else {
                    variance
                }
            }
            Func::Annuity => {
                if values.len() != 2
                    || values[0].value.negative
                    || values[1].value.negative
                    || values[1].value.n.is_zero()
                    || !(&values[1].value.n % &values[1].value.d).is_zero()
                {
                    return Err(
                        "ANNUITY requires a nonnegative rate and a positive integer period count"
                            .into(),
                    );
                }
                if values[0].value.n.is_zero() {
                    one.divide(&values[1].value)
                } else {
                    let base = Self {
                        value: add(&one, &values[0].value),
                        integer: 0,
                        decimal: 0,
                        floating: true,
                    };
                    let factor = base.binary(
                        crate::zir::Op::Pow,
                        values[1].clone().negate(),
                        0,
                        true,
                        settings,
                    )?;
                    values[0].value.divide(&sub(&one, &factor.value))
                }
            }
            Func::PresentValue => {
                if values.len() < 2
                    || values[0].compare(&Self::parse("-1")?) != std::cmp::Ordering::Greater
                {
                    return Err(
                        "PRESENT-VALUE requires a rate greater than -1 and cash flows".into(),
                    );
                }
                let base = add(&one, &values[0].value);
                let mut divisor = one.clone();
                let mut total = Self::zero().value;
                for cash in &values[1..] {
                    divisor = reduce(divisor.multiply(&base));
                    total = add(&total, &cash.value.divide(&divisor));
                }
                total
            }
            _ => return Err("unsupported aggregate".into()),
        };
        let floating = floating
            || values.iter().any(|v| v.floating)
            || matches!(
                func,
                Func::Mean
                    | Func::Median
                    | Func::Midrange
                    | Func::Variance
                    | Func::StandardDeviation
                    | Func::Annuity
                    | Func::PresentValue
            );
        let result = Self {
            value: reduce(value),
            integer: values.iter().map(|v| v.integer).max().unwrap_or(1)
                + values.len().ilog10() as i32
                + 1,
            decimal: values.iter().map(|v| v.decimal).max().unwrap_or(0),
            floating,
        };
        if floating {
            result.as_float(settings.float_width(), true)
        } else {
            Ok(result)
        }
    }
    pub fn function(
        func: crate::zir::Func,
        value: Self,
        settings: Settings,
    ) -> Result<Self, String> {
        Self {
            value: super::math::function(func, &value.value)?,
            integer: 0,
            decimal: 0,
            floating: true,
        }
        .as_float(settings.float_width(), true)
    }
    pub fn negate(mut self) -> Self {
        if !self.value.n.is_zero() {
            self.value.negative = !self.value.negative;
        }
        self
    }
    pub fn compare(&self, other: &Self) -> std::cmp::Ordering {
        if self.value.n.is_zero() && other.value.n.is_zero() {
            return std::cmp::Ordering::Equal;
        }
        if self.value.negative != other.value.negative {
            return if self.value.negative {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            };
        }
        let order = (&self.value.n * &other.value.d).cmp(&(&other.value.n * &self.value.d));
        if self.value.negative {
            order.reverse()
        } else {
            order
        }
    }
    pub fn raw(&self, scale: i32, rounded: bool) -> BigInt {
        let factor = BigUint::from(10u8).pow(scale.unsigned_abs());
        let (n, d) = if scale >= 0 {
            (&self.value.n * factor, self.value.d.clone())
        } else {
            (self.value.n.clone(), &self.value.d * factor)
        };
        let mut q = &n / &d;
        if rounded && (&n % &d) * 2u8 >= d {
            q += 1u8;
        }
        BigInt::from_biguint(
            if self.value.negative {
                Sign::Minus
            } else {
                Sign::Plus
            },
            q,
        )
    }
    pub fn from_raw(raw: BigInt, scale: i32) -> Self {
        let negative = raw.is_negative();
        let mut n = raw.magnitude().clone();
        let mut d = BigUint::one();
        if scale >= 0 {
            d = BigUint::from(10u8).pow(scale as u32);
        } else {
            n *= BigUint::from(10u8).pow(scale.unsigned_abs());
        }
        Self {
            value: Ratio { negative, n, d },
            integer: 0,
            decimal: scale,
            floating: false,
        }
    }
    pub fn to_dec(&self) -> Option<Dec> {
        // Keep every significant decimal digit that the legacy Dec can hold.
        if self.value.n.is_zero() {
            return Some(Dec::ZERO);
        }
        let mut order =
            self.value.n.to_str_radix(10).len() as i32 - self.value.d.to_str_radix(10).len() as i32;
        if self.raw(-order, false).is_zero() {
            order -= 1;
        }
        let scale = (37 - order).clamp(0, 400) as u32;
        Some(Dec::new(self.raw(scale as i32, false).to_i128()?, scale).normalize())
    }
    pub fn from_hfp(bytes: &[u8]) -> Self {
        let width = bytes.len();
        let mut full = [0; 16];
        full[16 - width..].copy_from_slice(bytes);
        let raw = u128::from_be_bytes(full);
        let precision = if width == 16 { 112 } else { width * 8 - 8 };
        let exponent = ((raw >> (width * 8 - 8)) & 127) as i32 - 64;
        let fraction = if width == 16 {
            ((raw >> 64) & 0x00ff_ffff_ffff_ffff) << 56 | (raw & 0x00ff_ffff_ffff_ffff)
        } else {
            raw & ((1u128 << precision) - 1)
        };
        Self {
            value: Ratio::binary(
                raw >> (width * 8 - 1) != 0,
                fraction,
                exponent * 4 - precision as i32,
            ),
            integer: 0,
            decimal: 0,
            floating: true,
        }
    }
    pub fn from_item(ty: &crate::zir::DataType, bytes: &[u8]) -> Self {
        use crate::zir::{DataType, NumUsage};
        match ty {
            DataType::Float { .. } => Self::from_hfp(bytes),
            DataType::ExternalFloat {
                picture,
                national,
                comma,
            } => super::external::decode(picture, *national, *comma, bytes)
                .unwrap_or_else(|_| Self::zero()),
            DataType::Numeric(t) => {
                let digits = if t.usage == NumUsage::Native {
                    match bytes.len() {
                        2 => 5,
                        4 => 10,
                        _ => 20,
                    }
                } else {
                    t.digits
                };
                Self::decimal(
                    crate::vm::data::value_of(ty, bytes),
                    digits as i32 - t.scale,
                    t.scale,
                )
            }
            DataType::NumericEdited { digits, scale, .. } => Self::decimal(
                crate::vm::data::value_of(ty, bytes),
                *digits as i32 - *scale,
                *scale,
            ),
            _ => {
                let value = crate::vm::data::value_of(ty, bytes);
                Self::decimal(value, value.integer_digits() as i32, value.scale as i32)
            }
        }
    }
    /// Store after computing overflow, so ON SIZE ERROR can retain the receiver.
    pub fn store(
        &self,
        ty: &crate::zir::DataType,
        bytes: &mut [u8],
        rounded: bool,
    ) -> Result<bool, String> {
        use crate::vm::data;
        use crate::zir::{DataType, NumUsage};
        match ty {
            DataType::Float { .. } => {
                bytes.copy_from_slice(&self.hfp(bytes.len(), true)?);
                Ok(false)
            }
            DataType::ExternalFloat {
                picture,
                national,
                comma,
            } => super::external::encode(self, picture, *national, *comma, bytes),
            DataType::Numeric(t) => {
                let raw = self.raw(t.scale, rounded);
                let raw = if t.signed { raw } else { raw.abs() };
                let (overflow, raw) = if t.usage == NumUsage::Native {
                    let (lo, hi) = zeus_decimal::binary::range(bytes.len(), t.signed);
                    (
                        raw < lo.into() || raw > hi.into(),
                        raw % (BigInt::one() << (bytes.len() * 8)),
                    )
                } else {
                    let cap = BigInt::from(10u8).pow(t.digits);
                    (raw.abs() >= cap, raw % cap)
                };
                data::store_raw(t, bytes, raw.to_i128().ok_or("numeric receiver overflow")?);
                Ok(overflow)
            }
            DataType::NumericEdited {
                digits,
                scale,
                national,
                edit,
                blank_zero,
                currency,
                point,
            } => {
                let raw = self.raw(*scale, rounded);
                let cap = BigInt::from(10u8).pow(*digits);
                let overflow = raw.abs() >= cap;
                let value = Self::from_raw(raw % cap, *scale)
                    .to_dec()
                    .ok_or("edited receiver overflow")?;
                let mut output = vec![
                    0;
                    if *national {
                        bytes.len() / 2
                    } else {
                        bytes.len()
                    }
                ];
                data::edit_num(
                    edit,
                    *digits,
                    *scale,
                    *blank_zero,
                    *currency,
                    *point,
                    value,
                    &mut output,
                );
                if *national {
                    bytes.copy_from_slice(&data::national_of(&output));
                } else {
                    bytes.copy_from_slice(&output);
                }
                Ok(overflow)
            }
            _ => {
                let value = self
                    .to_dec()
                    .ok_or("numeric conversion outside fixed-point range")?;
                crate::vm::eval::move_into(&crate::vm::eval::Val::Num(value), ty, bytes);
                Ok(false)
            }
        }
    }
    pub fn hfp(&self, width: usize, round: bool) -> Result<Vec<u8>, String> {
        if self.value.n.is_zero() {
            return Ok(vec![0; width]);
        }
        let precision = if width == 16 { 112 } else { width * 8 - 8 };
        let mut e = self.value.floor_log2().div_euclid(4) + 1;
        if e < -64 {
            return Ok(vec![0; width]);
        }
        let (mut q, _) = self.value.quantize(
            4 * e - precision as i32,
            if round {
                Rounding::NearestEven
            } else {
                Rounding::Zero
            },
        );
        if q.bits() > precision as u64 {
            q >>= 4usize;
            e += 1;
        }
        if e > 63 {
            return Err("floating-point exponent overflow".into());
        }
        let q = q.to_u128().ok_or("floating-point fraction overflow")?;
        let sign = u128::from(self.value.negative);
        let characteristic = (e + 64) as u128;
        let raw = if width == 16 {
            ((sign << 63 | characteristic << 56 | q >> 56) << 64)
                | (sign << 63 | ((characteristic + 114) & 127) << 56 | q & 0x00ff_ffff_ffff_ffff)
        } else {
            sign << (width * 8 - 1) | characteristic << (width * 8 - 8) | q
        };
        Ok(raw.to_be_bytes()[16 - width..].to_vec())
    }
    pub fn as_float(&self, width: usize, round: bool) -> Result<Self, String> {
        Ok(Self::from_hfp(&self.hfp(width, round)?))
    }
    pub fn fit(mut self, dmax: i32, settings: Settings) -> Self {
        let max = settings.intermediate_digits() as i32;
        if self.integer + self.decimal > max {
            if self.decimal <= dmax {
                self.integer = max - self.decimal;
            } else if self.integer + dmax <= max {
                self.decimal = max - self.integer;
            } else {
                self.integer = max - dmax;
                self.decimal = dmax;
            }
        }
        let digits = (self.integer + self.decimal).max(0) as u32;
        let raw = self.raw(self.decimal, false) % BigInt::from(10u8).pow(digits);
        self.value = Self::from_raw(raw, self.decimal).value;
        self
    }
    pub fn binary(
        self,
        op: crate::zir::Op,
        other: Self,
        dmax: i32,
        floating: bool,
        settings: Settings,
    ) -> Result<Self, String> {
        use crate::zir::Op;
        let floating = floating || self.floating || other.floating;
        let (a, b) = if floating {
            (
                self.as_float(settings.float_width(), true)?,
                other.as_float(settings.float_width(), true)?,
            )
        } else {
            (self, other)
        };
        let (value, integer, decimal) = match op {
            Op::Add | Op::Sub => {
                let mut y = b.value.clone();
                if matches!(op, Op::Sub) {
                    y.negative = !y.negative;
                }
                let value = if floating && !a.value.n.is_zero() && !y.n.is_zero() {
                    let exponent = a
                        .value
                        .floor_log2()
                        .div_euclid(4)
                        .max(y.floor_log2().div_euclid(4))
                        + 1;
                    let precision = if settings.float_width() == 16 {
                        112
                    } else {
                        56
                    };
                    a.value.add_hex_guard(&y, exponent, precision)
                } else {
                    a.value.add(&y, false)
                };
                (
                    value,
                    a.integer.max(b.integer) + 1,
                    a.decimal.max(b.decimal),
                )
            }
            Op::Mul => (
                a.value.multiply(&b.value),
                a.integer + b.integer,
                a.decimal + b.decimal,
            ),
            Op::Div => {
                if b.value.n.is_zero() {
                    return Err("division by zero".into());
                }
                (
                    a.value.divide(&b.value),
                    a.integer + b.decimal,
                    (a.decimal - b.decimal).max(dmax),
                )
            }
            Op::Pow => {
                let exponent = b.to_dec().ok_or("exponent out of range")?.normalize();
                if exponent.scale != 0 {
                    return Self {
                        value: super::math::power(&a.value, &b.value)?,
                        integer: 0,
                        decimal: 0,
                        floating: true,
                    }
                    .as_float(settings.float_width(), false);
                }
                let count = exponent.coef.unsigned_abs();
                if count > 999999999 {
                    return Err("exponent exceeds nine digits".into());
                }
                if count == 0 && a.value.n.is_zero() {
                    return Err("zero raised to zero".into());
                }
                if count > 10000 {
                    if a.value.n == a.value.d {
                        let mut one = Self::parse("1")?;
                        one.value.negative = a.value.negative && count % 2 != 0;
                        return if floating {
                            one.as_float(settings.float_width(), false)
                        } else {
                            Ok(one)
                        };
                    }
                    if a.value.n.is_zero() {
                        return if exponent.coef < 0 {
                            Err("zero raised to a negative exponent".into())
                        } else {
                            Ok(Self::zero())
                        };
                    }
                    if !floating && exponent.coef > 0 && (&a.value.n % &a.value.d).is_zero() {
                        let modulus = BigUint::from(10u8).pow(settings.intermediate_digits());
                        let coefficient =
                            (&a.value.n / &a.value.d).modpow(&BigUint::from(count), &modulus);
                        let value = BigInt::from_biguint(
                            if a.value.negative && count % 2 != 0 {
                                Sign::Minus
                            } else {
                                Sign::Plus
                            },
                            coefficient,
                        );
                        return Ok(Self::from_raw(value, 0));
                    }
                    let mut positive = a.value.clone();
                    positive.negative = false;
                    let value = super::math::power(&positive, &b.value)?;
                    let result = Self {
                        value: Ratio {
                            negative: a.value.negative && count % 2 != 0,
                            ..value
                        },
                        integer: a.integer.saturating_mul(count as i32),
                        decimal: dmax,
                        floating,
                    };
                    return if floating {
                        result.as_float(settings.float_width(), false)
                    } else {
                        Ok(result.fit(dmax, settings))
                    };
                }
                let mut value = Ratio {
                    negative: a.value.negative && count % 2 != 0,
                    n: a.value.n.pow(count as u32),
                    d: a.value.d.pow(count as u32),
                };
                if exponent.coef < 0 {
                    if value.n.is_zero() {
                        return Err("zero raised to a negative exponent".into());
                    }
                    std::mem::swap(&mut value.n, &mut value.d);
                }
                if exponent.coef < 0 {
                    (value, 1 + a.decimal.saturating_mul(count as i32), dmax)
                } else {
                    (
                        value,
                        a.integer.saturating_mul(count as i32),
                        a.decimal.saturating_mul(count as i32),
                    )
                }
            }
        };
        let result = Self {
            value,
            integer,
            decimal,
            floating,
        };
        if floating {
            result.as_float(settings.float_width(), false)
        } else {
            Ok(result.fit(dmax, settings))
        }
    }
}

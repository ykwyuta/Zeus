//! Transcendental functions with binary guard precision above extended HFP.
//! Range reduction retains enough bits for every finite HFP argument.
use crate::hlasm::float::Ratio;
use crate::zir::Func;
use num_bigint::{BigInt, BigUint, Sign};
use num_traits::{One, Signed, ToPrimitive, Zero};

const BITS: usize = 512;
fn unit() -> BigInt {
    BigInt::one() << BITS
}
fn mul(a: &BigInt, b: &BigInt) -> BigInt {
    a * b / unit()
}
fn div(a: &BigInt, b: &BigInt) -> BigInt {
    a * unit() / b
}
fn input(x: &Ratio) -> BigInt {
    BigInt::from_biguint(
        if x.negative { Sign::Minus } else { Sign::Plus },
        (&x.n << BITS) / &x.d,
    )
}
fn output(x: BigInt) -> Ratio {
    Ratio {
        negative: x.is_negative(),
        n: x.magnitude().clone(),
        d: BigUint::one() << BITS,
    }
}
fn sqrt(x: &BigInt) -> Result<BigInt, String> {
    if x.is_negative() {
        return Err("SQRT requires a nonnegative argument".into());
    }
    Ok(BigInt::from((x.magnitude() << BITS).sqrt()))
}
fn atan_series(x: &BigInt) -> BigInt {
    let square = -mul(x, x);
    let mut power = x.clone();
    let mut result = power.clone();
    for k in 1..4096u32 {
        power = mul(&power, &square);
        let term = &power / (2 * k + 1);
        if term.is_zero() {
            break;
        }
        result += term;
    }
    result
}
fn pi() -> BigInt {
    16 * atan_series(&(unit() / 5)) - 4 * atan_series(&(unit() / 239))
}
fn atan(x: &BigInt) -> BigInt {
    if x.is_negative() {
        return -atan(&-x);
    }
    if x > &unit() {
        return pi() / 2 - atan(&div(&unit(), x));
    }
    if x > &(unit() / 2) {
        return pi() / 4 + atan_series(&div(&(x - unit()), &(x + unit())));
    }
    atan_series(x)
}
fn log_series(x: &BigInt) -> BigInt {
    let z = div(&(x - unit()), &(x + unit()));
    let square = mul(&z, &z);
    let mut power = z.clone();
    let mut result = power.clone();
    for k in 1..4096u32 {
        power = mul(&power, &square);
        let term = &power / (2 * k + 1);
        if term.is_zero() {
            break;
        }
        result += term;
    }
    result * 2
}
fn log(x: &Ratio) -> Result<BigInt, String> {
    if x.negative || x.n.is_zero() {
        return Err("LOG requires a positive argument".into());
    }
    let exponent = x.floor_log2();
    let mantissa = if exponent >= 0 {
        BigInt::from((&x.n << BITS) / (&x.d << exponent as usize))
    } else {
        BigInt::from((&x.n << (BITS + exponent.unsigned_abs() as usize)) / &x.d)
    };
    Ok(log_series(&mantissa) + exponent * log_series(&(unit() * 2)))
}
fn exp(x: &BigInt) -> Result<BigInt, String> {
    let ln2 = log_series(&(unit() * 2));
    let exponent = (x / &ln2)
        .to_i32()
        .ok_or("floating-point exponent overflow")?;
    if exponent > 260 {
        return Err("floating-point exponent overflow".into());
    }
    if exponent < -520 {
        return Ok(BigInt::zero());
    }
    let remainder = x - &ln2 * exponent;
    let mut term = unit();
    let mut result = term.clone();
    for k in 1..4096u32 {
        term = mul(&term, &remainder) / k;
        if term.is_zero() {
            break;
        }
        result += &term;
    }
    Ok(if exponent >= 0 {
        result << exponent as usize
    } else {
        result >> exponent.unsigned_abs() as usize
    })
}
fn sin_cos(x: &BigInt) -> (BigInt, BigInt) {
    let pi = pi();
    let period = &pi * 2;
    let mut x = x % &period;
    if x > pi {
        x -= &period;
    }
    if x < -&pi {
        x += &period;
    }
    let square = -mul(&x, &x);
    let mut sin = x.clone();
    let mut cos = unit();
    let mut s = sin.clone();
    let mut c = cos.clone();
    for k in 1..4096u32 {
        s = mul(&s, &square) / (2 * k * (2 * k + 1));
        c = mul(&c, &square) / ((2 * k - 1) * 2 * k);
        sin += &s;
        cos += &c;
        if s.is_zero() && c.is_zero() {
            break;
        }
    }
    (sin, cos)
}
pub(super) fn function(func: Func, value: &Ratio) -> Result<Ratio, String> {
    let x = input(value);
    Ok(output(match func {
        Func::Sqrt => sqrt(&x)?,
        Func::Log => log(value)?,
        Func::Log10 => div(&log(value)?, &log_series(&(unit() * 10))),
        Func::Atan => atan(&x),
        Func::Asin | Func::Acos => {
            if x.abs() > unit() {
                return Err("ASIN/ACOS argument must be between -1 and 1".into());
            }
            let asin = if x.abs() == unit() {
                if x.is_negative() { -pi() / 2 } else { pi() / 2 }
            } else {
                atan(&div(&x, &sqrt(&(unit() - mul(&x, &x)))?))
            };
            if func == Func::Acos {
                pi() / 2 - asin
            } else {
                asin
            }
        }
        Func::Sin => sin_cos(&x).0,
        Func::Cos => sin_cos(&x).1,
        Func::Tan => {
            let (sin, cos) = sin_cos(&x);
            if cos.is_zero() {
                return Err("TAN argument at a pole".into());
            }
            div(&sin, &cos)
        }
        _ => return Err("not a transcendental function".into()),
    }))
}
pub(super) fn power(base: &Ratio, exponent: &Ratio) -> Result<Ratio, String> {
    if base.negative || base.n.is_zero() {
        return Err("fractional power requires a positive base".into());
    }
    Ok(output(exp(&mul(&log(base)?, &input(exponent)))?))
}

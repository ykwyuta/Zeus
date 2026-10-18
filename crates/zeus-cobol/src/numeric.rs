//! Compiler-selected decimal precision and numeric sign conventions.
use serde::{Deserialize, Serialize};
pub(crate) mod external;
mod math;
pub(crate) mod number;
pub(crate) mod text;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Arithmetic {
    #[default]
    Compat,
    Extend,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Numproc {
    #[default]
    Nopfd,
    Pfd,
    Mig,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Numcls {
    #[default]
    Primary,
    Alternate,
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub arithmetic: Arithmetic,
    pub numproc: Numproc,
    pub numcls: Numcls,
}
impl Settings {
    pub fn operand_digits(self) -> u32 {
        if self.arithmetic == Arithmetic::Extend {
            31
        } else {
            18
        }
    }
    pub fn intermediate_digits(self) -> u32 {
        if self.arithmetic == Arithmetic::Extend {
            31
        } else {
            30
        }
    }
    pub fn float_width(self) -> usize {
        if self.arithmetic == Arithmetic::Extend {
            16
        } else {
            8
        }
    }
}

pub(crate) fn floating_literal(text: &str) -> Result<number::Number, String> {
    let (mantissa, exponent) = text
        .split_once(['E', 'e'])
        .ok_or("floating literal requires E")?;
    let unsigned = mantissa.strip_prefix(['+', '-']).unwrap_or(mantissa);
    let exponent_digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
    let digits = unsigned.chars().filter(char::is_ascii_digit).count();
    if !unsigned.contains('.')
        || unsigned.chars().filter(|c| *c == '.').count() != 1
        || !(1..=16).contains(&digits)
        || !unsigned.chars().all(|c| c.is_ascii_digit() || c == '.')
        || !(1..=2).contains(&exponent_digits.len())
        || !exponent_digits.chars().all(|c| c.is_ascii_digit())
    {
        return Err("floating literal requires a decimal point, 1..16 mantissa digits and 1..2 exponent digits".into());
    }
    let value = number::Number::parse(text)?;
    use num_traits::Zero;
    if !value.value.n.is_zero() {
        let order = value.value.floor_log2().div_euclid(4) + 1;
        if !(-64..=63).contains(&order) {
            return Err("floating literal outside IBM HFP exponent range".into());
        }
        value.hfp(8, true)?;
    }
    Ok(value)
}

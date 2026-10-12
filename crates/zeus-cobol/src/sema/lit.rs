//! Literal and figurative-constant values.

use zeus_decimal::Dec;

use crate::ebcdic;
use crate::front::ast::{Figurative, Literal, Operand};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FigByte {
    Zero,
    Space,
    High,
    Low,
    Quote,
}

impl FigByte {
    pub fn byte(self) -> u8 {
        match self {
            FigByte::Zero => ebcdic::ZERO,
            FigByte::Space => ebcdic::SPACE,
            FigByte::High => 0xFF,
            FigByte::Low => 0x00,
            FigByte::Quote => ebcdic::QUOTE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Const {
    /// Numeric literal: value and digits (EBCDIC, without sign and point).
    Num(Dec, Vec<u8>),
    Bytes(Vec<u8>),
    /// National literal (UTF-16 big-endian).
    National(Vec<u8>),
    Fig(FigByte),
    All(Vec<u8>),
    Null,
}

pub fn numeric(text: &str) -> Result<(Dec, Vec<u8>), String> {
    let t = text.replace(',', ".");
    let d = Dec::parse(&t).ok_or_else(|| format!("invalid numeric literal {text}"))?;
    let digits: Vec<u8> = t
        .chars()
        .filter(|c| c.is_ascii_digit())
        .map(|c| 0xF0 + c.to_digit(10).unwrap() as u8)
        .collect();
    Ok((d, digits))
}

pub fn figurative(f: &Figurative) -> Option<FigByte> {
    Some(match f {
        Figurative::Zero => FigByte::Zero,
        Figurative::Space => FigByte::Space,
        Figurative::HighValue => FigByte::High,
        Figurative::LowValue => FigByte::Low,
        Figurative::Quote => FigByte::Quote,
        Figurative::Symbolic(_) => return None,
    })
}

pub fn literal_bytes(l: &Literal) -> Vec<u8> {
    match l {
        Literal::Str(s) => ebcdic::encode(s),
        Literal::Hex(b) | Literal::National(b) => b.clone(),
        Literal::Num(n) => ebcdic::encode(n),
    }
}

/// The value of a constant operand (VALUE clauses).
pub fn constant(op: &Operand, _decimal_comma: bool) -> Result<Const, String> {
    Ok(match op {
        Operand::Lit(Literal::Num(n)) => {
            let (d, digits) = numeric(n)?;
            Const::Num(d, digits)
        }
        Operand::Lit(Literal::National(b)) => Const::National(b.clone()),
        Operand::Lit(l) => Const::Bytes(literal_bytes(l)),
        Operand::Fig(f) => match figurative(f) {
            Some(b) => Const::Fig(b),
            None => return Err("symbolic characters are not allowed here".into()),
        },
        Operand::All(inner) => match inner.as_ref() {
            Operand::Lit(l) => Const::All(literal_bytes(l)),
            Operand::Fig(f) => match figurative(f) {
                Some(b) => Const::Fig(b),
                None => return Err("invalid ALL operand".into()),
            },
            _ => return Err("invalid ALL operand".into()),
        },
        Operand::Null => Const::Null,
        _ => return Err("literal expected in VALUE clause".into()),
    })
}

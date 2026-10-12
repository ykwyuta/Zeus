//! Data types: attributes of values and their storage, and the precision
//! rules of the language (RULES(IBM), LIMITS(FIXEDDEC(15) FIXEDBIN(31))).

use serde::{Deserialize, Serialize};

/// Maximum FIXED DECIMAL precision (N).
pub const MAX_DEC: u8 = 15;
/// Maximum FIXED BINARY precision (M).
pub const MAX_BIN: u8 = 31;
pub const MAX_FLOAT_DEC: u8 = 33;
pub const MAX_FLOAT_BIN: u8 = 109;

/// A string extent: a constant, `*` (taken from the argument), or decided
/// at block entry (adjustable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ext {
    Const(u32),
    Star,
    /// The value computed into the activation's extent slot n (for a
    /// CONTROLLED variable: the slot of its current generation).
    Slot(u32),
    /// `REFER`: the value of the module's refer entry n.
    Refer(u32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Var {
    Fixed,
    Varying,
    VaryingZ,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ty {
    /// FIXED DECIMAL / BINARY (p,q)
    Fixed {
        dec: bool,
        p: u8,
        q: i16,
        unsigned: bool,
    },
    Float {
        dec: bool,
        p: u8,
    },
    Char {
        len: Ext,
        var: Var,
    },
    Bit {
        len: Ext,
        var: Var,
        aligned: bool,
    },
    /// GRAPHIC (DBCS, `graphic`) or WIDECHAR (UTF-16) string: two bytes
    /// a character. Its values are the stored bytes; in expressions it is
    /// converted to CHARACTER.
    Wchar {
        len: Ext,
        var: Var,
        graphic: bool,
    },
    /// COMPLEX: real and imaginary parts of the arithmetic type.
    Complex(Box<Ty>),
    /// Numeric or character PICTURE.
    Pic(Box<crate::picture::Picture>),
    Pointer,
    /// An offset in the area of the variable (VarId), if it names one.
    Offset(Option<u32>),
    Entry,
    File,
    Label,
    Format,
    /// An area of the given size: a 16-byte header (size and bytes used)
    /// then the allocations.
    Area(u32),
}

/// The bytes before the allocations of an area.
pub const AREA_HEADER: u32 = 16;

impl Ty {
    pub fn fixed_dec(p: u8, q: i16) -> Ty {
        Ty::Fixed {
            dec: true,
            p,
            q,
            unsigned: false,
        }
    }

    pub fn fixed_bin(p: u8, q: i16) -> Ty {
        Ty::Fixed {
            dec: false,
            p,
            q,
            unsigned: false,
        }
    }

    pub fn char(n: u32) -> Ty {
        Ty::Char {
            len: Ext::Const(n),
            var: Var::Fixed,
        }
    }

    pub fn char_var(n: u32) -> Ty {
        Ty::Char {
            len: Ext::Const(n),
            var: Var::Varying,
        }
    }

    pub fn bit(n: u32) -> Ty {
        Ty::Bit {
            len: Ext::Const(n),
            var: Var::Fixed,
            aligned: false,
        }
    }

    /// Computational types: arithmetic, strings and pictures.
    pub fn is_arith(&self) -> bool {
        matches!(self, Ty::Fixed { .. } | Ty::Float { .. }) || self.numeric_pic().is_some()
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Ty::Char { .. } | Ty::Bit { .. }) || matches!(self, Ty::Pic(p) if !p.numeric)
    }

    pub fn is_char(&self) -> bool {
        matches!(self, Ty::Char { .. }) || matches!(self, Ty::Pic(p) if !p.numeric)
    }

    pub fn is_bit(&self) -> bool {
        matches!(self, Ty::Bit { .. })
    }

    /// The type of the parts of a COMPLEX type.
    pub fn complex_part(&self) -> Option<&Ty> {
        match self {
            Ty::Complex(t) => Some(t),
            _ => None,
        }
    }

    /// The arithmetic attributes of a numeric picture.
    pub fn numeric_pic(&self) -> Option<Ty> {
        match self {
            Ty::Pic(p) if p.numeric => Some(p.arith()),
            _ => None,
        }
    }

    /// Arithmetic attributes of an arithmetic type (pictures: implied).
    pub fn arith(&self) -> Option<Ty> {
        match self {
            Ty::Fixed { .. } | Ty::Float { .. } => Some(self.clone()),
            _ => self.numeric_pic(),
        }
    }

    pub fn is_dec(&self) -> bool {
        matches!(
            self.arith(),
            Some(Ty::Fixed { dec: true, .. } | Ty::Float { dec: true, .. })
        )
    }

    pub fn is_float(&self) -> bool {
        matches!(self.arith(), Some(Ty::Float { .. }))
    }

    pub fn precision(&self) -> (u8, i16) {
        match self.arith() {
            Some(Ty::Fixed { p, q, .. }) => (p, q),
            Some(Ty::Float { p, .. }) => (p, 0),
            _ => (0, 0),
        }
    }

    pub fn str_len(&self) -> Option<Ext> {
        match self {
            Ty::Char { len, .. } | Ty::Bit { len, .. } | Ty::Wchar { len, .. } => Some(*len),
            Ty::Pic(p) => Some(Ext::Const(p.len as u32)),
            _ => None,
        }
    }

    /// The type with another string length.
    pub fn with_len(&self, l: Ext) -> Ty {
        match self {
            Ty::Char { var, .. } => Ty::Char { len: l, var: *var },
            Ty::Wchar { var, graphic, .. } => Ty::Wchar {
                len: l,
                var: *var,
                graphic: *graphic,
            },
            Ty::Bit { var, aligned, .. } => Ty::Bit {
                len: l,
                var: *var,
                aligned: *aligned,
            },
            t => t.clone(),
        }
    }

    pub fn varying(&self) -> Var {
        match self {
            Ty::Char { var, .. } | Ty::Bit { var, .. } | Ty::Wchar { var, .. } => *var,
            _ => Var::Fixed,
        }
    }

    /// Storage size in bytes and alignment of a scalar (string lengths
    /// constant), and whether it is a bit-packed unaligned bit string.
    pub fn storage(&self, aligned: bool) -> (u32, u32) {
        let len = |e: &Ext| match e {
            Ext::Const(n) => *n,
            _ => 0,
        };
        match self {
            Ty::Fixed { dec: true, p, .. } => (u32::from(*p) / 2 + 1, 1),
            Ty::Fixed {
                dec: false,
                p,
                unsigned,
                ..
            } => {
                let p = u32::from(*p) + u32::from(*unsigned);
                let n = match p {
                    0..=8 => 1,
                    9..=16 => 2,
                    17..=32 => 4,
                    _ => 8,
                };
                (n, if aligned { n } else { 1 })
            }
            Ty::Float { .. } => {
                let n = float_bytes(self);
                (n, if aligned { n.min(8) } else { 1 })
            }
            Ty::Char { len: l, var } => match var {
                Var::Fixed => (len(l), 1),
                Var::Varying => (len(l) + 2, if aligned { 2 } else { 1 }),
                Var::VaryingZ => (len(l) + 1, 1),
            },
            Ty::Bit { len: l, var, .. } => {
                let bytes = len(l).div_ceil(8);
                match var {
                    Var::Fixed => (bytes, 1),
                    _ => (bytes + 2, if aligned { 2 } else { 1 }),
                }
            }
            Ty::Wchar { len: l, var, .. } => match var {
                Var::Fixed => (2 * len(l), if aligned { 2 } else { 1 }),
                _ => (2 * len(l) + 2, if aligned { 2 } else { 1 }),
            },
            Ty::Complex(t) => {
                let (n, a) = t.storage(aligned);
                (2 * n, a)
            }
            Ty::Pic(p) => (p.len as u32, 1),
            Ty::Pointer | Ty::Offset(_) | Ty::File => (4, if aligned { 4 } else { 1 }),
            Ty::Area(n) => (crate::types::AREA_HEADER + n, if aligned { 8 } else { 1 }),
            Ty::Entry | Ty::Label | Ty::Format => (8, if aligned { 4 } else { 1 }),
        }
    }
}

/// Bytes of a FLOAT value: short (4), long (8) or extended (16).
pub fn float_bytes(t: &Ty) -> u32 {
    match t {
        Ty::Float { dec: true, p } if *p <= 6 => 4,
        Ty::Float { dec: true, p } if *p <= 16 => 8,
        Ty::Float { dec: false, p } if *p <= 21 => 4,
        Ty::Float { dec: false, p } if *p <= 53 => 8,
        _ => 16,
    }
}

/// CEIL(n*3.32) with n=16 giving 53.
pub fn dec_to_bin_digits(n: i32) -> i32 {
    if n == 16 {
        return 53;
    }
    let a = n.abs() * 332;
    let c = (a + 99) / 100;
    c * n.signum()
}

/// CEIL(n/3.32), away from zero for negative scales.
pub fn bin_to_dec_digits(n: i32) -> i32 {
    let a = n.abs() * 100;
    let c = (a + 331) / 332;
    c * n.signum()
}

fn cap(p: i32, max: u8) -> u8 {
    p.clamp(1, i32::from(max)) as u8
}

/// The FIXED BINARY attributes a FIXED DECIMAL converts to.
pub fn dec_as_bin(p: u8, q: i16) -> (i32, i32) {
    (
        1 + dec_to_bin_digits(i32::from(p)),
        dec_to_bin_digits(i32::from(q)),
    )
}

/// The FIXED DECIMAL attributes a FIXED BINARY converts to.
pub fn bin_as_dec(p: u8, q: i16) -> (i32, i32) {
    (
        1 + bin_to_dec_digits(i32::from(p)),
        bin_to_dec_digits(i32::from(q)),
    )
}

/// Arithmetic attributes after the type conversion of an operand of an
/// arithmetic operator: CHAR → FIXED DEC(N,0), BIT → FIXED BIN(M,0)
/// unsigned, pictures to their implied attributes.
pub fn operand_arith(t: &Ty) -> Ty {
    match t.arith() {
        Some(a) => a,
        None if t.is_bit() => Ty::Fixed {
            dec: false,
            p: MAX_BIN,
            q: 0,
            unsigned: true,
        },
        None => Ty::fixed_dec(MAX_DEC, 0),
    }
}

/// Result attributes of an infix arithmetic operator (+ - * /) with
/// arithmetic operands (after `operand_arith`).
pub fn arith_result(op: &str, a: &Ty, b: &Ty) -> Ty {
    match (a, b) {
        (
            Ty::Fixed {
                dec: d1,
                p: p1,
                q: q1,
                ..
            },
            Ty::Fixed {
                dec: d2,
                p: p2,
                q: q2,
                ..
            },
        ) => {
            let (dec, (p1, q1), (p2, q2)) = if *d1 && *d2 {
                (
                    true,
                    (i32::from(*p1), i32::from(*q1)),
                    (i32::from(*p2), i32::from(*q2)),
                )
            } else {
                let conv = |d: bool, p: u8, q: i16| {
                    if d {
                        dec_as_bin(p, q)
                    } else {
                        (i32::from(p), i32::from(q))
                    }
                };
                (false, conv(*d1, *p1, *q1), conv(*d2, *p2, *q2))
            };
            let max = if dec { MAX_DEC } else { MAX_BIN };
            let n = i32::from(max);
            let (p, q) = match op {
                "+" | "-" => {
                    let q = q1.max(q2);
                    (1 + (p1 - q1).max(p2 - q2) + q, q)
                }
                "*" => (1 + p1 + p2, q1 + q2),
                _ => (n, n - p1 + q1 - q2),
            };
            Ty::Fixed {
                dec,
                p: cap(p, max),
                q: q.clamp(-128, 127) as i16,
                unsigned: false,
            }
        }
        _ => {
            // at least one FLOAT
            let (d1, p1) = float_prec(a);
            let (d2, p2) = float_prec(b);
            if d1 && d2 {
                Ty::Float {
                    dec: true,
                    p: cap(p1.max(p2), MAX_FLOAT_DEC),
                }
            } else {
                let b1 = if d1 { dec_to_bin_digits(p1) } else { p1 };
                let b2 = if d2 { dec_to_bin_digits(p2) } else { p2 };
                Ty::Float {
                    dec: false,
                    p: cap(b1.max(b2), MAX_FLOAT_BIN),
                }
            }
        }
    }
}

/// Base and precision of an operand converted to FLOAT.
fn float_prec(t: &Ty) -> (bool, i32) {
    match t {
        Ty::Fixed { dec, p, .. } | Ty::Float { dec, p } => (*dec, i32::from(*p)),
        _ => (true, 6),
    }
}

/// Result of `x ** y`; `y_int` is the value of y when it is an unsigned
/// integer constant.
pub fn power_result(a: &Ty, b: &Ty, y_int: Option<u32>) -> Ty {
    if let (Ty::Fixed { dec, p, q, .. }, Some(n)) = (a, y_int)
        && n > 0
    {
        let pp = (i32::from(*p) + 1) * n as i32 - 1;
        let max = if *dec { MAX_DEC } else { MAX_BIN };
        if pp <= i32::from(max) {
            return Ty::Fixed {
                dec: *dec,
                p: pp as u8,
                q: (i32::from(*q) * n as i32).clamp(-128, 127) as i16,
                unsigned: false,
            };
        }
    }
    if let (Ty::Float { .. }, Ty::Fixed { q: 0, .. }) = (a, b) {
        return a.clone();
    }
    let fa = to_float_ty(a);
    let fb = to_float_ty(b);
    arith_result("*", &fa, &fb)
}

/// FLOAT attributes for an arithmetic type (same base and precision).
pub fn to_float_ty(t: &Ty) -> Ty {
    match t {
        Ty::Fixed { dec, p, .. } => Ty::Float { dec: *dec, p: *p },
        Ty::Float { .. } => t.clone(),
        _ => Ty::Float { dec: true, p: 6 },
    }
}

/// Length of the character string an arithmetic value converts to.
pub fn arith_char_len(t: &Ty) -> u32 {
    match t.arith() {
        Some(Ty::Fixed {
            dec: true, p, q, ..
        }) => {
            if q >= 0 && i32::from(q) <= i32::from(p) {
                u32::from(p) + 3
            } else {
                let k = (i32::from(q).unsigned_abs()).to_string().len() as u32;
                u32::from(p) + k + 3
            }
        }
        Some(Ty::Fixed {
            dec: false, p, q, ..
        }) => {
            let (p, q) = bin_as_dec(p, q);
            arith_char_len(&Ty::fixed_dec(p.clamp(1, 255) as u8, q as i16))
        }
        Some(Ty::Float { dec: true, p }) => u32::from(p) + 8,
        Some(Ty::Float { dec: false, p }) => {
            u32::from(bin_to_dec_digits(i32::from(p)).max(1) as u8) + 8
        }
        _ => 0,
    }
}

/// Length of the bit string an arithmetic value converts to.
pub fn arith_bit_len(t: &Ty) -> u32 {
    let m = i32::from(MAX_BIN);
    let n = match t.arith() {
        Some(Ty::Fixed {
            dec: false, p, q, ..
        }) => i32::from(p) - i32::from(q),
        Some(Ty::Fixed {
            dec: true, p, q, ..
        }) => dec_to_bin_digits(i32::from(p) - i32::from(q)),
        Some(Ty::Float { dec: false, p }) => i32::from(p),
        Some(Ty::Float { dec: true, p }) => dec_to_bin_digits(i32::from(p)),
        _ => 0,
    };
    n.clamp(0, m) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precision_rules() {
        assert_eq!(dec_to_bin_digits(15), 50);
        assert_eq!(dec_to_bin_digits(16), 53);
        assert_eq!(bin_to_dec_digits(31), 10);
        assert_eq!(bin_to_dec_digits(15), 5);
        // 1/3: (15,14); 25 + 1/3: capped (15,14)
        let one = Ty::fixed_dec(1, 0);
        let third = arith_result("/", &one, &one);
        assert_eq!(third, Ty::fixed_dec(15, 14));
        assert_eq!(
            arith_result("+", &Ty::fixed_dec(2, 0), &third),
            Ty::fixed_dec(15, 14)
        );
        assert_eq!(
            arith_result("*", &Ty::fixed_dec(3, 2), &Ty::fixed_dec(4, 1)),
            Ty::fixed_dec(8, 3)
        );
        // DEC(3,2) + BIN(10) -> BIN(18,7)
        assert_eq!(
            arith_result("+", &Ty::fixed_dec(3, 2), &Ty::fixed_bin(10, 0)),
            Ty::fixed_bin(18, 7)
        );
        assert_eq!(arith_char_len(&Ty::fixed_dec(5, 0)), 8);
        assert_eq!(arith_char_len(&Ty::fixed_bin(31, 0)), 14);
        assert_eq!(arith_char_len(&Ty::fixed_bin(15, 0)), 9);
        assert_eq!(arith_char_len(&Ty::Float { dec: true, p: 5 }), 13);
        assert_eq!(arith_bit_len(&Ty::fixed_dec(1, 0)), 4);
        assert_eq!(
            power_result(&Ty::fixed_dec(1, 0), &one, Some(3)),
            Ty::fixed_dec(5, 0)
        );
    }
}

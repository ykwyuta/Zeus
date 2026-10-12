//! Built-in functions, pseudovariables and subroutines: names and result
//! attributes.

use serde::{Deserialize, Serialize};

use crate::types::{MAX_BIN, MAX_DEC, Ty, bin_to_dec_digits, dec_to_bin_digits, operand_arith};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Bif {
    // arithmetic
    Abs,
    Add,
    Subtract,
    Multiply,
    Divide,
    Mod,
    Rem,
    Round,
    Trunc,
    Ceil,
    Floor,
    Sign,
    Max,
    Min,
    Precision,
    Binary,
    Decimal,
    Fixed,
    Float,
    FixedBin,
    FixedDec,
    FloatBin,
    FloatDec,
    Signed,
    Unsigned,
    // mathematical
    Sqrt,
    Exp,
    Log,
    Log2,
    Log10,
    Sin,
    Cos,
    Tan,
    Sinh,
    Cosh,
    Tanh,
    Asin,
    Acos,
    Atan,
    Atanh,
    Sind,
    Cosd,
    Tand,
    Atand,
    // arrays
    All,
    Any,
    Sum,
    Prod,
    Dim,
    Lbound,
    Hbound,
    // strings
    Bit,
    Char,
    Bool,
    Copy,
    Repeat,
    Collate,
    High,
    Low,
    Index,
    Search,
    Verify,
    Tally,
    Length,
    MaxLength,
    Left,
    Right,
    Center,
    CenterRight,
    Lowercase,
    Uppercase,
    Reverse,
    String,
    Substr,
    Translate,
    Trim,
    Hex,
    Unspec,
    // storage
    Addr,
    Null,
    SysNull,
    Size,
    CurrentSize,
    Allocation,
    PointerAdd,
    PointerValue,
    BinaryValue,
    Empty,
    /// POINTER(offset, area)
    PointerOf,
    /// OFFSET(pointer, area)
    OffsetOf,
    // date and time
    Date,
    Time,
    Datetime,
    Days,
    DaysToDate,
    Weekday,
    // conditions
    OnCode,
    OnChar,
    OnSource,
    OnKey,
    OnFile,
    OnLoc,
    OnCount,
    DataField,
    // program
    PliRetV,
    PliRetC,
    PliDump,
    PliSrtA,
    PliFill,
    PliMove,
    Omitted,
    Present,
    LineNo,
    PageNo,
    Count,
    Random,
}

/// Looks up a built-in name.
pub fn lookup(name: &str) -> Option<Bif> {
    use Bif::*;
    Some(match name {
        "ABS" => Abs,
        "ADD" => Add,
        "SUBTRACT" => Subtract,
        "MULTIPLY" => Multiply,
        "DIVIDE" => Divide,
        "MOD" => Mod,
        "REM" => Rem,
        "ROUND" => Round,
        "TRUNC" => Trunc,
        "CEIL" => Ceil,
        "FLOOR" => Floor,
        "SIGN" => Sign,
        "MAX" => Max,
        "MIN" => Min,
        "PRECISION" | "PREC" => Precision,
        "BINARY" | "BIN" => Binary,
        "DECIMAL" | "DEC" => Decimal,
        "FIXED" => Fixed,
        "FLOAT" => Float,
        "FIXEDBIN" => FixedBin,
        "FIXEDDEC" => FixedDec,
        "FLOATBIN" => FloatBin,
        "FLOATDEC" => FloatDec,
        "SIGNED" => Signed,
        "UNSIGNED" => Unsigned,
        "SQRT" => Sqrt,
        "EXP" => Exp,
        "LOG" => Log,
        "LOG2" => Log2,
        "LOG10" => Log10,
        "SIN" => Sin,
        "COS" => Cos,
        "TAN" => Tan,
        "SINH" => Sinh,
        "COSH" => Cosh,
        "TANH" => Tanh,
        "ASIN" => Asin,
        "ACOS" => Acos,
        "ATAN" => Atan,
        "ATANH" => Atanh,
        "SIND" => Sind,
        "COSD" => Cosd,
        "TAND" => Tand,
        "ATAND" => Atand,
        "ALL" => All,
        "ANY" => Any,
        "SUM" => Sum,
        "PROD" => Prod,
        "DIMENSION" | "DIM" => Dim,
        "LBOUND" => Lbound,
        "HBOUND" => Hbound,
        "BIT" => Bit,
        "CHARACTER" | "CHAR" => Char,
        "BOOL" => Bool,
        "COPY" => Copy,
        "REPEAT" => Repeat,
        "COLLATE" => Collate,
        "HIGH" => High,
        "LOW" => Low,
        "INDEX" => Index,
        "SEARCH" => Search,
        "VERIFY" => Verify,
        "TALLY" => Tally,
        "LENGTH" => Length,
        "MAXLENGTH" => MaxLength,
        "LEFT" => Left,
        "RIGHT" => Right,
        "CENTER" | "CENTRE" | "CENTERLEFT" | "CENTRELEFT" => Center,
        "CENTERRIGHT" | "CENTRERIGHT" => CenterRight,
        "LOWERCASE" | "LOWER" => Lowercase,
        "UPPERCASE" | "UPPER" => Uppercase,
        "REVERSE" => Reverse,
        "STRING" => String,
        "SUBSTR" => Substr,
        "TRANSLATE" => Translate,
        "TRIM" => Trim,
        "HEX" => Hex,
        "UNSPEC" => Unspec,
        "ADDR" => Addr,
        "POINTER" | "PTR" => PointerOf,
        "OFFSET" => OffsetOf,
        "NULL" => Null,
        "SYSNULL" => SysNull,
        "SIZE" | "STORAGE" | "STG" => Size,
        "CURRENTSIZE" | "CURRENTSTORAGE" | "CSTG" => CurrentSize,
        "ALLOCATION" | "ALLOCN" => Allocation,
        "POINTERADD" | "PTRADD" => PointerAdd,
        "POINTERVALUE" | "PTRVALUE" => PointerValue,
        "BINARYVALUE" | "BINVALUE" => BinaryValue,
        "EMPTY" => Empty,
        "DATE" => Date,
        "TIME" => Time,
        "DATETIME" => Datetime,
        "DAYS" => Days,
        "DAYSTODATE" => DaysToDate,
        "WEEKDAY" => Weekday,
        "ONCODE" => OnCode,
        "ONCHAR" => OnChar,
        "ONSOURCE" => OnSource,
        "ONKEY" => OnKey,
        "ONFILE" => OnFile,
        "ONLOC" => OnLoc,
        "ONCOUNT" => OnCount,
        "DATAFIELD" => DataField,
        "PLIRETV" => PliRetV,
        "PLIRETC" => PliRetC,
        "PLIDUMP" => PliDump,
        "PLISRTA" => PliSrtA,
        "PLIFILL" => PliFill,
        "PLIMOVE" => PliMove,
        "OMITTED" => Omitted,
        "PRESENT" => Present,
        "LINENO" => LineNo,
        "PAGENO" => PageNo,
        "COUNT" => Count,
        "RANDOM" => Random,
        _ => return None,
    })
}

impl Bif {
    /// Built-ins usable without an argument list.
    pub fn no_args(self) -> bool {
        use Bif::*;
        matches!(
            self,
            Null | SysNull
                | Date
                | Time
                | Datetime
                | Collate
                | OnCode
                | OnChar
                | OnSource
                | OnKey
                | OnFile
                | OnLoc
                | OnCount
                | DataField
                | PliRetV
                | Empty
                | Days
                | Weekday
                | Random
        )
    }

    /// Built-ins whose first argument is a reference (not converted).
    pub fn takes_ref(self) -> bool {
        use Bif::*;
        matches!(
            self,
            Addr | Dim
                | Lbound
                | Hbound
                | Size
                | CurrentSize
                | Allocation
                | String
                | Unspec
                | Sum
                | Prod
                | All
                | Any
                | Omitted
                | Present
                | LineNo
                | PageNo
                | Count
                | MaxLength
        )
    }
}

pub const BIFPREC: u8 = 31;

fn fixed_bin31() -> Ty {
    Ty::fixed_bin(BIFPREC, 0)
}

/// Common arithmetic attributes of several operands (MAX, MIN, MOD):
/// the base and scale as for infix operators.
pub fn common(tys: &[Ty]) -> Ty {
    let tys: Vec<Ty> = tys.iter().map(operand_arith).collect();
    let float = tys.iter().any(|t| matches!(t, Ty::Float { .. }));
    let dec = tys.iter().all(|t| t.is_dec());
    if float {
        let p = tys
            .iter()
            .map(|t| {
                let (p, _) = t.precision();
                let p = i32::from(p);
                if dec || t.is_dec() == dec {
                    p
                } else {
                    dec_to_bin_digits(p)
                }
            })
            .max()
            .unwrap_or(6);
        return Ty::Float {
            dec,
            p: p.clamp(1, 109) as u8,
        };
    }
    // fixed: (min(n, max(p-q) + max(q)), max(q)) in the common base
    let conv: Vec<(i32, i32)> = tys
        .iter()
        .map(|t| {
            let (p, q) = t.precision();
            if dec || !t.is_dec() {
                (i32::from(p), i32::from(q))
            } else {
                crate::types::dec_as_bin(p, q)
            }
        })
        .collect();
    let maxint = conv.iter().map(|(p, q)| p - q).max().unwrap_or(1);
    let maxq = conv.iter().map(|(_, q)| *q).max().unwrap_or(0);
    let n = i32::from(if dec { MAX_DEC } else { MAX_BIN });
    Ty::Fixed {
        dec,
        p: (maxint + maxq).clamp(1, n) as u8,
        q: maxq as i16,
        unsigned: false,
    }
}

/// Result attributes of a built-in function. `args` are the argument
/// types; `consts` the values of integer constant arguments.
pub fn result_type(b: Bif, args: &[Ty], consts: &[Option<i64>]) -> Result<Ty, String> {
    use Bif::*;
    let arg = |i: usize| {
        args.get(i)
            .cloned()
            .ok_or_else(|| format!("{b:?}: argument {} missing", i + 1))
    };
    let cint = |i: usize| consts.get(i).copied().flatten();
    let str_len = |t: &Ty| -> u32 {
        match t.str_len() {
            Some(crate::types::Ext::Const(n)) => n,
            _ => match t.arith() {
                Some(a) if !t.is_string() => {
                    if matches!(a, Ty::Fixed { dec: false, .. }) {
                        crate::types::arith_bit_len(&a)
                    } else {
                        crate::types::arith_char_len(&a)
                    }
                }
                _ => 0,
            },
        }
    };
    Ok(match b {
        Abs => operand_arith(&arg(0)?),
        Sign | Index | Search | Verify | Tally | Length | MaxLength | Dim | Lbound | Hbound
        | Size | CurrentSize | Allocation | OnCode | OnCount | PliRetV | Days | Weekday
        | LineNo | PageNo | Count => fixed_bin31(),
        Add | Subtract | Multiply | Divide => {
            let a = operand_arith(&arg(0)?);
            let c = operand_arith(&arg(1)?);
            let base = crate::types::arith_result("+", &a, &c);
            let p = cint(2).ok_or("precision must be a constant")? as u8;
            let q = cint(3).unwrap_or(0) as i16;
            match base {
                Ty::Fixed { dec, .. } => Ty::Fixed {
                    dec,
                    p,
                    q,
                    unsigned: false,
                },
                Ty::Float { dec, .. } => Ty::Float { dec, p },
                t => t,
            }
        }
        Mod | Rem => {
            let c = common(&[arg(0)?, arg(1)?]);
            if let Ty::Fixed { dec, q, .. } = c {
                let (p2, q2) = operand_arith(&arg(1)?).precision();
                let n = i32::from(if dec { MAX_DEC } else { MAX_BIN });
                Ty::Fixed {
                    dec,
                    p: (i32::from(p2) - i32::from(q2) + i32::from(q)).clamp(1, n) as u8,
                    q,
                    unsigned: false,
                }
            } else {
                c
            }
        }
        Max | Min => common(args),
        Round => {
            let a = operand_arith(&arg(0)?);
            let n = cint(1).unwrap_or(0);
            match a {
                Ty::Fixed { dec, p, q, .. } => {
                    let max = i64::from(if dec { MAX_DEC } else { MAX_BIN });
                    Ty::Fixed {
                        dec,
                        p: (i64::from(p) - i64::from(q) + 1 + n).clamp(1, max) as u8,
                        q: n as i16,
                        unsigned: false,
                    }
                }
                t => t,
            }
        }
        Trunc | Ceil | Floor => match operand_arith(&arg(0)?) {
            Ty::Fixed { dec, p, q, .. } => {
                let max = i32::from(if dec { MAX_DEC } else { MAX_BIN });
                Ty::Fixed {
                    dec,
                    p: (i32::from(p) - i32::from(q) + 1).clamp(1, max) as u8,
                    q: 0,
                    unsigned: false,
                }
            }
            t => t,
        },
        Precision => {
            let a = operand_arith(&arg(0)?);
            let p = cint(1).ok_or("precision must be a constant")? as u8;
            let q = cint(2).unwrap_or(0) as i16;
            match a {
                Ty::Fixed { dec, .. } => Ty::Fixed {
                    dec,
                    p,
                    q,
                    unsigned: false,
                },
                Ty::Float { dec, .. } => Ty::Float { dec, p },
                t => t,
            }
        }
        Binary | Decimal | FixedBin | FixedDec | Fixed => {
            let a = operand_arith(&arg(0)?);
            let to_dec = match b {
                Decimal | FixedDec => true,
                Binary | FixedBin => false,
                _ => a.is_dec() || args[0].is_char(),
            };
            let fixed = matches!(b, FixedBin | FixedDec | Fixed) || matches!(a, Ty::Fixed { .. });
            let (p, q) = if let Some(p) = cint(1) {
                (p as i32, cint(2).unwrap_or(0) as i32)
            } else if b == Fixed {
                (if to_dec { 5 } else { 15 }, 0)
            } else {
                match a {
                    Ty::Fixed { dec, p, q, .. } if dec != to_dec => {
                        if to_dec {
                            crate::types::bin_as_dec(p, q)
                        } else {
                            crate::types::dec_as_bin(p, q)
                        }
                    }
                    Ty::Float { dec, p } if !fixed || matches!(b, FixedBin | FixedDec) => {
                        let p = i32::from(p);
                        match (dec, to_dec) {
                            (true, false) => (dec_to_bin_digits(p), 0),
                            (false, true) => (bin_to_dec_digits(p), 0),
                            _ => (p, 0),
                        }
                    }
                    t => {
                        let (p, q) = t.precision();
                        (i32::from(p), i32::from(q))
                    }
                }
            };
            let max = i32::from(if to_dec { MAX_DEC } else { MAX_BIN });
            if fixed {
                Ty::Fixed {
                    dec: to_dec,
                    p: p.clamp(1, max) as u8,
                    q: q as i16,
                    unsigned: false,
                }
            } else {
                Ty::Float {
                    dec: to_dec,
                    p: p.clamp(1, 109) as u8,
                }
            }
        }
        Float | FloatBin | FloatDec => {
            let a = operand_arith(&arg(0)?);
            let dec = match b {
                FloatDec => true,
                FloatBin => false,
                _ => a.is_dec(),
            };
            let p = match cint(1) {
                Some(p) => p as i32,
                None => {
                    let (p, _) = a.precision();
                    let p = i32::from(p);
                    match (a.is_dec(), dec) {
                        (true, false) => dec_to_bin_digits(p),
                        (false, true) => bin_to_dec_digits(p),
                        _ => p,
                    }
                }
            };
            Ty::Float {
                dec,
                p: p.clamp(1, 109) as u8,
            }
        }
        Signed | Unsigned => {
            let (p, q) = operand_arith(&arg(0)?).precision();
            Ty::Fixed {
                dec: false,
                p,
                q,
                unsigned: b == Unsigned,
            }
        }
        Sqrt | Exp | Log | Log2 | Log10 | Sin | Cos | Tan | Sinh | Cosh | Tanh | Asin | Acos
        | Atan | Atanh | Sind | Cosd | Tand | Atand => {
            crate::types::to_float_ty(&operand_arith(&arg(0)?))
        }
        Random => Ty::Float { dec: false, p: 53 },
        All | Any => Ty::Bit {
            len: crate::types::Ext::Const(str_len(&arg(0)?)),
            var: crate::types::Var::Fixed,
            aligned: false,
        },
        Sum => match operand_arith(&arg(0)?) {
            Ty::Fixed { dec, q, .. } => Ty::Fixed {
                dec,
                p: if dec { MAX_DEC } else { MAX_BIN },
                q,
                unsigned: false,
            },
            t => t,
        },
        Prod => match operand_arith(&arg(0)?) {
            Ty::Fixed { dec, q: 0, .. } => Ty::Fixed {
                dec,
                p: if dec { MAX_DEC } else { MAX_BIN },
                q: 0,
                unsigned: false,
            },
            t => crate::types::to_float_ty(&t),
        },
        Bit => match cint(1) {
            Some(n) => Ty::bit(n.max(0) as u32),
            None => {
                let a = arg(0)?;
                if a.is_bit() {
                    a
                } else if a.is_char() {
                    Ty::bit(str_len(&a))
                } else {
                    Ty::bit(crate::types::arith_bit_len(&a))
                }
            }
        },
        Char => match cint(1) {
            Some(n) => Ty::char(n.max(0) as u32),
            None => {
                let a = arg(0)?;
                if a.is_char() {
                    Ty::char_var(str_len(&a))
                } else if a.is_bit() {
                    Ty::char(str_len(&a))
                } else {
                    Ty::char(crate::types::arith_char_len(&a))
                }
            }
        },
        Bool => Ty::bit(str_len(&arg(0)?).max(str_len(&arg(1)?))),
        Copy | Repeat | Reverse | Substr | Trim | Lowercase | Uppercase | Translate => {
            let a = arg(0)?;
            let n = str_len(&a);
            let bit = a.is_bit()
                || (matches!(b, Repeat | Reverse)
                    && matches!(a.arith(), Some(Ty::Fixed { dec: false, .. }))
                    && !a.is_string());
            if bit {
                Ty::Bit {
                    len: crate::types::Ext::Const(n.max(1) * 64),
                    var: crate::types::Var::Varying,
                    aligned: false,
                }
            } else {
                Ty::Char {
                    len: crate::types::Ext::Const(match b {
                        Translate | Lowercase | Uppercase | Reverse | Trim | Substr => n.max(1),
                        _ => 32767,
                    }),
                    var: crate::types::Var::Varying,
                }
            }
        }
        Left | Right | Center | CenterRight => match cint(1) {
            Some(n) => Ty::char(n.max(0) as u32),
            None => Ty::char_var(32767),
        },
        Collate => Ty::char(256),
        High | Low => match cint(0) {
            Some(n) => Ty::char(n.max(0) as u32),
            None => Ty::char_var(32767),
        },
        Hex => Ty::char_var(65535),
        String => Ty::char_var(32767),
        Unspec => Ty::Bit {
            len: crate::types::Ext::Const(32767),
            var: crate::types::Var::Varying,
            aligned: false,
        },
        Addr | Null | SysNull | PointerAdd | PointerValue => Ty::Pointer,
        BinaryValue => fixed_bin31(),
        Empty => Ty::Area(0),
        PointerOf => Ty::Pointer,
        OffsetOf => Ty::Offset(None),
        Date => Ty::char(6),
        Time => Ty::char(9),
        Datetime => Ty::char_var(26),
        DaysToDate => Ty::char_var(26),
        OnChar => Ty::char(1),
        OnSource | DataField | OnKey => Ty::char_var(32767),
        OnFile | OnLoc => Ty::char_var(31),
        Omitted | Present => Ty::bit(1),
        PliRetC | PliDump | PliSrtA | PliFill | PliMove => Ty::Entry,
    })
}

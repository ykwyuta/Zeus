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
    OrdinalName,
    OrdinalSucc,
    OrdinalPred,
    /// FIRST / LAST of an ORDINAL type
    First,
    Last,
    /// OFFSET(pointer, area)
    OffsetOf,
    // date and time
    Date,
    Time,
    Datetime,
    Days,
    DaysToDate,
    Weekday,
    DaysToSecs,
    SecsToDays,
    ValidDate,
    // Enterprise PL/I integer, string and floating-point inquiry built-ins
    Iand,
    Ior,
    Ieor,
    Inot,
    Isll,
    Isrl,
    Raise2,
    SearchR,
    VerifyR,
    CharVal,
    Huge,
    Tiny,
    Epsilon,
    MaxExp,
    MinExp,
    Radix,
    PointerDiff,
    PointerSubtract,
    OffsetValue,
    OffsetAdd,
    OffsetSubtract,
    OffsetDiff,
    AddrData,
    BitLocation,
    Rank,
    Exponent,
    Scale,
    SecsToDate,
    Cs,
    Cds,
    Between,
    BetweenExclusive,
    BetweenLeftExclusive,
    BetweenRightExclusive,
    InList,
    IfThenElse,
    Iclz,
    Irll,
    Irrl,
    Lower2,
    Popcnt,
    ISigned,
    IUnsigned,
    RoundAwayFromZero,
    RoundToEven,
    MaxVal,
    MinVal,
    Pred,
    Succ,
    Places,
    PrecVal,
    ScaleVal,
    Y4Date,
    OnText,
    OnActual,
    OnExpected,
    OnOperator,
    OnPackage,
    OnSubscript,
    OnLbound,
    OnHbound,
    OnLine,
    OnArea,
    /// SHAnDIGEST/INIT/UPDATE/FINAL: family 1, 2 or 3, digest bits, and
    /// the operation (0 digest, 1 init, 2 update, 3 final).
    Sha(u8, u16, u8),
    WHigh,
    WLow,
    InArray,
    Allocate,
    Alloc31,
    AllocNext,
    AllocSize,
    AvailableArea,
    CheckStg,
    Unallocated,
    QuickSort,
    BinSearch,
    QuickSortX,
    BinSearchX,
    WcharVal,
    Ulength,
    Ulength8,
    Ulength16,
    Upos,
    Usubstr,
    Usupplementary,
    Uvalid,
    Uwidth,
    Utf8,
    Utf8ToChar,
    Utf8ToWchar,
    MemUvalid8,
    MemUvalid16,
    MemCu12,
    MemCu14,
    MemCu21,
    MemCu24,
    MemCu41,
    MemCu42,
    MemConvert,
    NullEntry,
    EntryAddr,
    FileOpen,
    FileId,
    FileNew,
    FileDdTest,
    FileDdInt,
    FileDdWord,
    Repattern,
    Indicators,
    PicSpec,
    CodePage,
    SubTo,
    Poly,
    WsCollapse16,
    XmlScrub,
    XmlScrub16,
    XmlChar,
    WsReplace16,
    Base64Encode16,
    Base64Decode16,
    MainName,
    ProcedureName,
    PackageName,
    SourceFile,
    SourceLine,
    IsMain,
    GetEnv,
    PutEnv,
    Uuid4,
    Valid,
    Microsecs,
    Secs,
    MicrosecsToDate,
    DaysToMicrosecs,
    MicrosecsToDays,
    Timestamp,
    UtcDatetime,
    UtcSecs,
    UtcMicrosecs,
    MaxDate,
    MinDate,
    StckToDate,
    StckeToDate,
    JulianToSmf,
    SmfToJulian,
    PliStck,
    PliStckE,
    PliStckF,
    PliStckLocal,
    PliStckUtc,
    PliStckELocal,
    PliStckEUtc,
    PliStckP,
    PliStckPLocal,
    PliStckPUtc,
    Y4Julian,
    Y4Year,
    ReplaceBy2,
    HexImage,
    Compare,
    Location,
    Erf,
    Erfc,
    Gamma,
    LogGamma,
    OnCondCond,
    OnCondId,
    Collapse,
    Squeeze,
    IndexR,
    Replace,
    Scrubout,
    Unhex,
    MemIndex,
    MemSearch,
    MemSearchR,
    MemVerify,
    MemVerifyR,
    MemCollapse,
    MemSqueeze,
    MemReplace,
    WsCollapse,
    WsReplace,
    HexEncode,
    HexDecode,
    HexEncode8,
    HexDecode8,
    Base64Encode,
    Base64Decode,
    Base64Encode8,
    Base64Decode8,
    WhereDiff,
    Hex8,
    HexImage8,
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
    PliFree,
    /// NEW(:t:): storage for a structure type.
    New,
    Graphic,
    Widechar,
    Real,
    Imag,
    Conjg,
    Complex,
    Omitted,
    Present,
    LineNo,
    PageNo,
    Count,
    SameKey,
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
        "ORDINALNAME" => OrdinalName,
        "ORDINALSUCC" => OrdinalSucc,
        "ORDINALPRED" => OrdinalPred,
        "FIRST" => First,
        "LAST" => Last,
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
        "ONLOC" | "ONPROC" | "ONPROCEDURE" => OnLoc,
        "ONCOUNT" => OnCount,
        "DATAFIELD" => DataField,
        "PLIRETV" => PliRetV,
        "PLIRETC" => PliRetC,
        "PLIDUMP" => PliDump,
        "PLIFREE" => PliFree,
        "NEW" => New,
        "GRAPHIC" => Graphic,
        "REAL" => Real,
        "IMAG" => Imag,
        "CONJG" => Conjg,
        "COMPLEX" | "CPLX" => Complex,
        "WIDECHAR" | "WCHAR" => Widechar,
        "HANDLE" => Addr,
        "PLISRTA" => PliSrtA,
        "PLIFILL" => PliFill,
        "PLIMOVE" | "PLIOVER" => PliMove,
        "OMITTED" => Omitted,
        "PRESENT" => Present,
        "LINENO" => LineNo,
        "PAGENO" => PageNo,
        "COUNT" => Count,
        "SAMEKEY" => SameKey,
        "RANDOM" => Random,
        "DAYSTOSECS" => DaysToSecs,
        "SECSTODAYS" => SecsToDays,
        "VALIDDATE" => ValidDate,
        "IAND" => Iand,
        "IOR" => Ior,
        "IEOR" => Ieor,
        "INOT" => Inot,
        "ISLL" => Isll,
        "ISRL" => Isrl,
        "RAISE2" => Raise2,
        "SEARCHR" => SearchR,
        "VERIFYR" => VerifyR,
        "CHARVAL" | "BYTE" => CharVal,
        "HUGE" => Huge,
        "TINY" => Tiny,
        "EPSILON" => Epsilon,
        "MAXEXP" => MaxExp,
        "MINEXP" => MinExp,
        "RADIX" => Radix,
        "POINTERDIFF" => PointerDiff,
        "POINTERSUBTRACT" | "PTRSUBTRACT" => PointerSubtract,
        "OFFSETVALUE" => OffsetValue,
        "OFFSETADD" => OffsetAdd,
        "OFFSETSUBTRACT" => OffsetSubtract,
        "OFFSETDIFF" => OffsetDiff,
        "ADDRDATA" => AddrData,
        "BITLOCATION" | "BITLOC" => BitLocation,
        "RANK" => Rank,
        "EXPONENT" => Exponent,
        "SCALE" => Scale,
        "SECSTODATE" => SecsToDate,
        "CS" => Cs,
        "CDS" => Cds,
        "BETWEEN" => Between,
        "BETWEENEXCLUSIVE" => BetweenExclusive,
        "BETWEENLEFTEXCLUSIVE" => BetweenLeftExclusive,
        "BETWEENRIGHTEXCLUSIVE" => BetweenRightExclusive,
        "INLIST" => InList,
        "IFTHENELSE" => IfThenElse,
        "ICLZ" => Iclz,
        "IRLL" => Irll,
        "IRRL" => Irrl,
        "LOWER2" => Lower2,
        "POPCNT" => Popcnt,
        "ISIGNED" => ISigned,
        "IUNSIGNED" => IUnsigned,
        "ROUNDAWAYFROMZERO" => RoundAwayFromZero,
        "ROUNDTOEVEN" => RoundToEven,
        "MAXVAL" => MaxVal,
        "MINVAL" => MinVal,
        "PRED" => Pred,
        "SUCC" => Succ,
        "PLACES" => Places,
        "PRECVAL" => PrecVal,
        "SCALEVAL" => ScaleVal,
        "Y4DATE" => Y4Date,
        "ONTEXT" => OnText,
        "ONACTUAL" => OnActual,
        "ONEXPECTED" => OnExpected,
        "ONOPERATOR" => OnOperator,
        "ONPACKAGE" => OnPackage,
        "ONSUBSCRIPT" => OnSubscript,
        "ONLBOUND" => OnLbound,
        "ONHBOUND" => OnHbound,
        "ONLINE" => OnLine,
        "ONAREA" => OnArea,
        "WHIGH" => WHigh,
        "INARRAY" => InArray,
        "ALLOCATE" | "ALLOC" => Allocate,
        "ALLOC31" => Alloc31,
        "ALLOCNEXT" => AllocNext,
        "ALLOCSIZE" => AllocSize,
        "AVAILABLEAREA" => AvailableArea,
        "CHECKSTG" => CheckStg,
        "UNALLOCATED" => Unallocated,
        "QUICKSORT" => QuickSort,
        "BINSEARCH" => BinSearch,
        "QUICKSORTX" => QuickSortX,
        "BINSEARCHX" => BinSearchX,
        "WCHARVAL" => WcharVal,
        "ULENGTH" => Ulength,
        "ULENGTH8" => Ulength8,
        "ULENGTH16" => Ulength16,
        "UPOS" => Upos,
        "USUBSTR" => Usubstr,
        "USUPPLEMENTARY" => Usupplementary,
        "UVALID" => Uvalid,
        "UWIDTH" => Uwidth,
        "UTF8" => Utf8,
        "UTF8TOCHAR" => Utf8ToChar,
        "UTF8TOWCHAR" => Utf8ToWchar,
        "MEMUVALID8" => MemUvalid8,
        "MEMUVALID16" => MemUvalid16,
        "MEMCU12" => MemCu12,
        "MEMCU14" => MemCu14,
        "MEMCU21" => MemCu21,
        "MEMCU24" => MemCu24,
        "MEMCU41" => MemCu41,
        "MEMCU42" => MemCu42,
        "MEMCONVERT" => MemConvert,
        "NULLENTRY" => NullEntry,
        "ENTRYADDR" => EntryAddr,
        "FILEOPEN" => FileOpen,
        "FILEID" => FileId,
        "FILENEW" => FileNew,
        "FILEDDTEST" => FileDdTest,
        "FILEDDINT" => FileDdInt,
        "FILEDDWORD" => FileDdWord,
        "REPATTERN" => Repattern,
        "INDICATORS" => Indicators,
        "PICSPEC" => PicSpec,
        "CODEPAGE" => CodePage,
        "WLOW" => WLow,
        "SUBTO" => SubTo,
        "POLY" => Poly,
        "WSCOLLAPSE16" => WsCollapse16,
        "XMLSCRUB" => XmlScrub,
        "XMLSCRUB16" | "XMLCLEAN" => XmlScrub16,
        "XMLCHAR" => XmlChar,
        "WHITESPACECOLLAPSE" => WsCollapse16,
        "WSREPLACE16" => WsReplace16,
        "BASE64ENCODE16" => Base64Encode16,
        "BASE64DECODE16" => Base64Decode16,
        "MAINNAME" => MainName,
        "PROCEDURENAME" => ProcedureName,
        "PROCNAME" => ProcedureName,
        "PACKAGENAME" => PackageName,
        "SOURCEFILE" => SourceFile,
        "SOURCELINE" => SourceLine,
        "ISMAIN" => IsMain,
        "GETENV" => GetEnv,
        "PUTENV" => PutEnv,
        "UUID4" => Uuid4,
        "VALID" => Valid,
        "MICROSECS" => Microsecs,
        "SECS" => Secs,
        "MICROSECSTODATE" => MicrosecsToDate,
        "DAYSTOMICROSECS" => DaysToMicrosecs,
        "MICROSECSTODAYS" => MicrosecsToDays,
        "TIMESTAMP" => Timestamp,
        "UTCDATETIME" => UtcDatetime,
        "UTCSECS" => UtcSecs,
        "UTCMICROSECS" => UtcMicrosecs,
        "MAXDATE" => MaxDate,
        "MINDATE" => MinDate,
        "STCKTODATE" => StckToDate,
        "STCKETODATE" => StckeToDate,
        "JULIANTOSMF" => JulianToSmf,
        "SMFTOJULIAN" => SmfToJulian,
        "PLISTCK" => PliStck,
        "PLISTCKE" => PliStckE,
        "PLISTCKF" => PliStckF,
        "PLISTCKLOCAL" => PliStckLocal,
        "PLISTCKUTC" => PliStckUtc,
        "PLISTCKELOCAL" => PliStckELocal,
        "PLISTCKEUTC" => PliStckEUtc,
        "PLISTCKP" => PliStckP,
        "PLISTCKPLOCAL" => PliStckPLocal,
        "PLISTCKPUTC" => PliStckPUtc,
        "Y4JULIAN" => Y4Julian,
        "Y4YEAR" => Y4Year,
        "REPLACEBY2" => ReplaceBy2,
        "HEXIMAGE" => HexImage,
        "COMPARE" => Compare,
        "LOCATION" => Location,
        "LOC" => Location,
        "ERF" => Erf,
        "ERFC" => Erfc,
        "GAMMA" => Gamma,
        "LOGGAMMA" => LogGamma,
        "ONCONDCOND" => OnCondCond,
        "COLLAPSE" => Collapse,
        "SQUEEZE" => Squeeze,
        "INDEXR" => IndexR,
        "REPLACE" => Replace,
        "SCRUBOUT" => Scrubout,
        "UNHEX" => Unhex,
        "MEMINDEX" => MemIndex,
        "MEMSEARCH" => MemSearch,
        "MEMSEARCHR" => MemSearchR,
        "MEMVERIFY" => MemVerify,
        "MEMVERIFYR" => MemVerifyR,
        "MEMCOLLAPSE" => MemCollapse,
        "MEMSQUEEZE" => MemSqueeze,
        "MEMREPLACE" => MemReplace,
        "WSCOLLAPSE" => WsCollapse,
        "WSREPLACE" => WsReplace,
        "HEXENCODE" => HexEncode,
        "HEXDECODE" => HexDecode,
        "HEXENCODE8" => HexEncode8,
        "HEXDECODE8" => HexDecode8,
        "BASE64ENCODE" => Base64Encode,
        "BASE64DECODE" => Base64Decode,
        "BASE64ENCODE8" => Base64Encode8,
        "BASE64DECODE8" => Base64Decode8,
        "WHEREDIFF" => WhereDiff,
        "HEX8" => Hex8,
        "HEXIMAGE8" => HexImage8,
        "ONCONDID" => OnCondId,
        _ => return sha_name(name),
    })
}

/// SHA1DIGEST, SHA2INIT256, SHA3FINAL512 ...
fn sha_name(name: &str) -> Option<Bif> {
    let rest = name.strip_prefix("SHA")?;
    let family = rest
        .get(0..1)?
        .parse::<u8>()
        .ok()
        .filter(|f| (1..=3).contains(f))?;
    let rest = &rest[1..];
    let (op, bits) = [("DIGEST", 0u8), ("INIT", 1), ("UPDATE", 2), ("FINAL", 3)]
        .iter()
        .find_map(|(w, op)| rest.strip_prefix(w).map(|b| (*op, b)))?;
    let bits: u16 = if family == 1 {
        if !bits.is_empty() {
            return None;
        }
        160
    } else {
        bits.parse()
            .ok()
            .filter(|b| matches!(b, 224 | 256 | 384 | 512))?
    };
    Some(Bif::Sha(family, bits, op))
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
                | Microsecs
                | OnText
                | OnActual
                | OnExpected
                | OnOperator
                | OnPackage
                | OnSubscript
                | OnLbound
                | OnHbound
                | OnLine
                | OnArea
                | MainName
                | ProcedureName
                | PackageName
                | SourceFile
                | SourceLine
                | IsMain
                | CodePage
                | CheckStg
                | NullEntry
                | FileNew
                | Uuid4
                | Secs
                | Timestamp
                | UtcDatetime
                | UtcSecs
                | UtcMicrosecs
                | Random
        )
    }

    /// Built-ins whose first argument is a reference (not converted).
    pub fn takes_ref(self) -> bool {
        use Bif::*;
        matches!(
            self,
            Addr | AddrData
                | BitLocation
                | Location
                | Dim
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
                | SameKey
                | FileOpen
                | FileId
                | FileNew
                | FileDdTest
                | FileDdInt
                | FileDdWord
                | MaxLength
                | Poly
                | Valid
                | PliStck
                | PliStckE
                | PliStckF
                | PliStckLocal
                | PliStckUtc
                | PliStckELocal
                | PliStckEUtc
                | PliStckP
                | PliStckPLocal
                | PliStckPUtc
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
        // the integer built-ins keep FIXED BIN(31) unless an argument needs 63 bits
        Iand | Ior | Ieor | Inot | Isll | Isrl | Raise2 => {
            let wide = args
                .iter()
                .any(|t| t.arith().is_some_and(|a| a.precision().0 > 31));
            Ty::fixed_bin(if wide { 63 } else { BIFPREC }, 0)
        }
        SearchR | VerifyR | MaxExp | MinExp | Radix | PointerDiff | SecsToDays | OffsetDiff
        | BitLocation | Rank | Exponent | Cs | Cds => fixed_bin31(),
        PointerSubtract | AddrData => Ty::Pointer,
        Between | BetweenExclusive | BetweenLeftExclusive | BetweenRightExclusive | InList => {
            Ty::bit(1)
        }
        Iclz | Places | PrecVal | ScaleVal | Compare | Location | OnCondId => fixed_bin31(),
        Irll | Irrl | Lower2 | Popcnt => match operand_arith(&arg(0)?) {
            t @ Ty::Fixed {
                dec: false, q: 0, ..
            } => t,
            _ => fixed_bin31(),
        },
        ISigned | IUnsigned => {
            let (p, unsigned) = match operand_arith(&arg(0)?) {
                Ty::Fixed {
                    dec: false,
                    p,
                    q: 0,
                    unsigned,
                } => (p, unsigned),
                _ => (BIFPREC, false),
            };
            let p = match (b, unsigned) {
                (ISigned, true) if matches!(p, 8 | 16 | 32 | 64) => p - 1,
                (IUnsigned, false) if matches!(p, 7 | 15 | 31 | 63) => p + 1,
                _ => p,
            };
            Ty::Fixed {
                dec: false,
                p,
                q: 0,
                unsigned: b == IUnsigned,
            }
        }
        MaxVal | MinVal => operand_arith(&arg(0)?),
        Pred | Succ => match operand_arith(&arg(0)?) {
            t @ Ty::Float { .. } => t,
            _ => return Err(format!("{b:?} requires a REAL FLOAT argument")),
        },
        Y4Date => Ty::char(8),
        OnText | OnActual | OnExpected | OnPackage | OnArea => Ty::char_var(32767),
        OnOperator => Ty::char(2),
        OnSubscript | OnLbound | OnHbound => Ty::fixed_bin(63, 0),
        OnLine => fixed_bin31(),
        Sha(_, bits, 0 | 3) => Ty::char(u32::from(bits) / 8),
        Sha(..) => Ty::Pointer,
        WHigh | WLow => match cint(0) {
            Some(n) => Ty::Wchar {
                len: crate::types::Ext::Const(n.max(0) as u32),
                var: crate::types::Var::Fixed,
                graphic: false,
            },
            None => Ty::Wchar {
                len: crate::types::Ext::Const(16383),
                var: crate::types::Var::Varying,
                graphic: false,
            },
        },
        SubTo => {
            let a = arg(0)?;
            if a.is_bit() {
                Ty::Bit {
                    len: crate::types::Ext::Const(str_len(&a).max(1)),
                    var: crate::types::Var::Varying,
                    aligned: false,
                }
            } else {
                Ty::char_var(str_len(&a).max(1))
            }
        }
        Poly => match arg(0)?.arith() {
            Some(t @ Ty::Float { .. }) => t,
            _ => return Err("POLY requires a REAL FLOAT array".into()),
        },
        WsCollapse16 | WsReplace16 | Base64Encode16 | Base64Decode16 => Ty::fixed_bin(63, 0),
        // size_t under LP(32)
        XmlScrub | XmlScrub16 | XmlChar => fixed_bin31(),
        MainName | ProcedureName | PackageName | SourceFile => Ty::char_var(255),
        SourceLine | CodePage => fixed_bin31(),
        InArray | CheckStg | Unallocated => Ty::bit(1),
        Allocate | Alloc31 | AllocNext => Ty::Pointer,
        // size_t under LP(32)
        AllocSize | AvailableArea => fixed_bin31(),
        // size_t under LP(32)
        QuickSort | BinSearch | QuickSortX | BinSearchX => fixed_bin31(),
        PicSpec => Ty::char(1),
        Ulength | Ulength8 | Ulength16 | Upos | Usupplementary | Uvalid | Uwidth => fixed_bin31(),
        // a CHAR character is at most 3 UTF-8 bytes (a WIDECHAR, too)
        Utf8 => Ty::char_var((3 * str_len(&arg(0)?).max(1)).min(32767)),
        Utf8ToChar => Ty::char_var(str_len(&arg(0)?).max(1)),
        Utf8ToWchar => Ty::Wchar {
            len: crate::types::Ext::Const(str_len(&arg(0)?).max(1)),
            var: crate::types::Var::Varying,
            graphic: false,
        },
        MemUvalid8 | MemUvalid16 | MemCu12 | MemCu14 | MemCu21 | MemCu24 | MemCu41 | MemCu42
        | MemConvert => Ty::fixed_bin(63, 0),
        NullEntry => Ty::Entry,
        FileOpen => Ty::bit(1),
        FileId | FileDdInt => Ty::fixed_bin(63, 0),
        FileDdTest => fixed_bin31(),
        FileDdWord => Ty::char_var(64),
        FileNew => Ty::File,
        Repattern => Ty::char_var(26),
        Indicators => fixed_bin31(),
        EntryAddr => Ty::Pointer,
        // the same string type as x, at most its length
        Usubstr => match arg(0)? {
            Ty::Wchar { len, .. } => Ty::Wchar {
                len,
                var: crate::types::Var::Varying,
                graphic: false,
            },
            a => Ty::Char {
                len: crate::types::Ext::Const(str_len(&a).max(1)),
                var: crate::types::Var::Varying,
            },
        },
        WcharVal => Ty::Wchar {
            len: crate::types::Ext::Const(1),
            var: crate::types::Var::Fixed,
            graphic: false,
        },
        IsMain | PutEnv | Valid => Ty::bit(1),
        GetEnv => Ty::char_var(32767),
        Uuid4 => Ty::char(36),
        Microsecs | DaysToMicrosecs | UtcMicrosecs => Ty::fixed_bin(63, 0),
        MicrosecsToDays => fixed_bin31(),
        Secs | UtcSecs => Ty::Float { dec: false, p: 53 },
        Timestamp => Ty::char(26),
        UtcDatetime => Ty::char(17),
        MicrosecsToDate | MaxDate | MinDate | StckToDate | StckeToDate => Ty::char_var(32),
        JulianToSmf => Ty::char(4),
        SmfToJulian => Ty::char(7),
        PliStck | PliStckE | PliStckF | PliStckLocal | PliStckUtc | PliStckELocal | PliStckEUtc
        | PliStckP | PliStckPLocal | PliStckPUtc => fixed_bin31(),
        Y4Julian => Ty::char(7),
        Y4Year => Ty::char(4),
        ReplaceBy2 | HexImage => Ty::char_var(65_535),
        OnCondCond => Ty::char_var(255),
        Collapse | Squeeze | Replace | Scrubout | Unhex | Hex8 | HexImage8 => Ty::char_var(65_535),
        IndexR | MemIndex | MemSearch | MemSearchR | MemVerify | MemVerifyR | MemCollapse
        | MemSqueeze | MemReplace | WsCollapse | WsReplace | HexEncode | HexDecode | HexEncode8
        | HexDecode8 | Base64Encode | Base64Decode | Base64Encode8 | Base64Decode8 | WhereDiff => {
            Ty::fixed_bin(63, 0)
        }
        IfThenElse => {
            let (y, z) = (arg(1)?, arg(2)?);
            if y.arith().is_some() && z.arith().is_some() && !y.is_string() && !z.is_string() {
                common(&[y, z])
            } else {
                y
            }
        }
        OffsetValue | OffsetAdd | OffsetSubtract => Ty::Offset(None),
        SecsToDate => Ty::char_var(26),
        Scale => match operand_arith(&arg(0)?) {
            t @ Ty::Float { .. } => t,
            _ => return Err("SCALE requires a floating-point argument".into()),
        },
        CharVal => Ty::char(1),
        ValidDate => Ty::bit(1),
        DaysToSecs => Ty::Float { dec: true, p: 16 },
        Huge | Tiny | Epsilon => match operand_arith(&arg(0)?) {
            t @ Ty::Float { .. } => t,
            _ => return Err(format!("{b:?} requires a floating-point argument")),
        },
        Sign | Index | Search | Verify | Tally | Length | MaxLength | Dim | Lbound | Hbound
        | Size | CurrentSize | Allocation | OnCode | OnCount | PliRetV | Days | Weekday
        | LineNo | PageNo | Count => fixed_bin31(),
        SameKey => Ty::bit(1),
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
        Round | RoundAwayFromZero | RoundToEven => {
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
        Erf | Erfc | Gamma | LogGamma | Sqrt | Exp | Log | Log2 | Log10 | Sin | Cos | Tan
        | Sinh | Cosh | Tanh | Asin | Acos | Atan | Atanh | Sind | Cosd | Tand | Atand => {
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
        Addr | Null | SysNull | PointerAdd | PointerValue | New => Ty::Pointer,
        Real | Imag | Conjg | Complex => match args.first() {
            Some(Ty::Complex(p)) if b != Conjg => (**p).clone(),
            Some(t) => t.clone(),
            None => Ty::Float { dec: true, p: 6 },
        },
        Graphic | Widechar => Ty::Wchar {
            len: crate::types::Ext::Star,
            var: crate::types::Var::Varying,
            graphic: matches!(b, Graphic),
        },
        BinaryValue => fixed_bin31(),
        Empty => Ty::Area(0),
        PointerOf => Ty::Pointer,
        OrdinalName => Ty::char_var(31),
        OrdinalSucc | OrdinalPred | First | Last => fixed_bin31(),
        OffsetOf => Ty::Offset(None),
        Date => Ty::char(6),
        Time => Ty::char(9),
        Datetime => Ty::char_var(26),
        DaysToDate => Ty::char_var(26),
        OnChar => Ty::char(1),
        OnSource | DataField | OnKey => Ty::char_var(32767),
        OnFile | OnLoc => Ty::char_var(31),
        Omitted | Present => Ty::bit(1),
        PliRetC | PliDump | PliSrtA | PliFill | PliMove | PliFree => Ty::Entry,
    })
}

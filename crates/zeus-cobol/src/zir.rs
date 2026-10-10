//! ZIR: the compiled form of a COBOL program, executed by [`crate::vm`].
//!
//! Control flow is a flat instruction list with jumps; data is addressed
//! through descriptors that give the location and type of each data item in
//! the program's storage, so all data keeps its z/OS byte layout (EBCDIC,
//! zoned, packed and big-endian binary).

use serde::{Deserialize, Serialize};
use zeus_decimal::Dec;

pub use crate::sema::picture::{Category, EditSym};

/// A whole compilation unit: the outermost program and nested programs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    pub programs: Vec<ProgramCode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramCode {
    pub name: String,
    pub initial: bool,
    pub common: bool,
    /// Index of the containing program (nested programs).
    pub parent: Option<u32>,
    /// Initial image of the static storage (working-storage with VALUEs,
    /// file record areas, index items).
    pub static_init: Vec<u8>,
    pub descs: Vec<Desc>,
    pub code: Vec<Instr>,
    pub paragraphs: Vec<Paragraph>,
    pub files: Vec<FileDef>,
    /// Number of LINKAGE SECTION base addresses.
    pub linkage_count: u32,
    /// EXTERNAL areas used by the program.
    pub externals: Vec<ExternalArea>,
    /// Linkage base numbers bound by PROCEDURE DIVISION USING.
    pub using: Vec<u32>,
    /// Number of temporary numeric registers.
    pub temps: u32,
    /// Number of ALTER-able GO TO slots.
    pub alter_slots: u32,
    /// Initial targets of the ALTER-able GO TO slots.
    pub alter_init: Vec<u32>,
    /// Collating sequence for alphanumeric comparisons (byte -> weight).
    pub collating: Option<Vec<u16>>,
    /// HIGH-VALUE and LOW-VALUE characters (the highest and lowest of the
    /// program collating sequence).
    pub high_value: u8,
    pub low_value: u8,
    /// Switch names and their initial on/off status.
    pub switches: Vec<String>,
    /// Debugging declaratives are active.
    pub debugging: bool,
    /// USE AFTER ERROR declaratives.
    pub uses: Vec<UseDecl>,
}

/// A USE AFTER STANDARD ERROR declarative section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UseDecl {
    /// Files named in the USE statement (indexes in `files`).
    pub files: Vec<u32>,
    /// USE ... ON INPUT / OUTPUT / I-O / EXTEND.
    pub mode: Option<OpenMode>,
    /// First and last paragraph of the section.
    pub start: u32,
    pub end: u32,
    pub global: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paragraph {
    pub name: String,
    pub section: Option<String>,
    pub start: u32,
}

/// Where a data item lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Base {
    /// The program's static storage.
    Static,
    /// A LINKAGE SECTION record (base address supplied by the caller).
    Linkage(u32),
    /// An EXTERNAL record (index into `ProgramCode::externals`).
    External(u32),
    /// GLOBAL data of a containing program (module program index): its
    /// static storage ...
    Outer(u32),
    /// ... or one of its LINKAGE SECTION records.
    OuterLinkage(u32, u32),
}

/// An EXTERNAL data record or file record area, shared by name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalArea {
    pub name: String,
    pub len: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dim {
    pub stride: u32,
    pub max: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignPos {
    None,
    Trailing,
    Leading,
    TrailingSeparate,
    LeadingSeparate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NumUsage {
    Display,
    Packed,
    /// Binary truncated to the picture (COMP, COMP-4, BINARY).
    Binary,
    /// Native binary (COMP-5).
    Native,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct NumType {
    pub usage: NumUsage,
    pub digits: u32,
    pub scale: i32,
    pub signed: bool,
    pub sign: SignPos,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataType {
    Group,
    Alphabetic {
        just: bool,
    },
    Alphanumeric {
        just: bool,
    },
    Numeric(NumType),
    NumericEdited {
        edit: Vec<EditSym>,
        digits: u32,
        scale: i32,
        blank_zero: bool,
        currency: u8,
        point: u8,
    },
    AlphanumericEdited {
        edit: Vec<EditSym>,
    },
    /// USAGE INDEX / index-name: 4-byte binary occurrence number.
    Index,
    Pointer,
}

impl DataType {
    pub fn is_numeric(&self) -> bool {
        matches!(self, DataType::Numeric(_) | DataType::Index)
    }
}

/// Descriptor of a data item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Desc {
    pub name: String,
    pub base: Base,
    pub offset: u32,
    pub len: u32,
    /// OCCURS dimensions from the outermost to the innermost.
    pub dims: Vec<Dim>,
    pub ty: DataType,
    /// Group containing an OCCURS DEPENDING ON table: the variable part.
    pub odo: Option<Odo>,
}

/// Variable-length group: its length is `fixed + count * stride`, where
/// count is the value of the DEPENDING ON item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Odo {
    pub fixed: u32,
    pub stride: u32,
    pub depending: Box<Loc>,
}

/// A data reference: descriptor, subscripts and reference modification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loc {
    pub desc: u32,
    pub subs: Vec<Ex>,
    pub refmod: Option<(Box<Ex>, Option<Box<Ex>>)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

/// Numeric expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Ex {
    Lit(Dec),
    Load(Loc),
    Bin(Op, Box<Ex>, Box<Ex>),
    Neg(Box<Ex>),
    Temp(u32),
    /// Length of the referenced item in bytes.
    LengthOf(Loc),
    Func(Func, Vec<Arg>),
}

/// Intrinsic function argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Arg {
    Num(Ex),
    Src(Src),
    /// All occurrences of a table (subscript ALL).
    All(Loc),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Func {
    Abs,
    Acos,
    Annuity,
    Asin,
    Atan,
    Char,
    Cos,
    CurrentDate,
    DateOfInteger,
    DayOfInteger,
    Factorial,
    IntegerOfDate,
    IntegerOfDay,
    Integer,
    IntegerPart,
    Length,
    Log,
    Log10,
    LowerCase,
    Max,
    Mean,
    Median,
    Midrange,
    Min,
    Mod,
    NumVal,
    NumValC,
    Ord,
    OrdMax,
    OrdMin,
    PresentValue,
    Random,
    Range,
    Rem,
    Reverse,
    Sin,
    Sqrt,
    StandardDeviation,
    Sum,
    Tan,
    UpperCase,
    Variance,
    WhenCompiled,
    DateToYyyymmdd,
    DayToYyyyddd,
    YearToYyyy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Fig {
    Zero,
    Space,
    High,
    Low,
    Quote,
}

/// A general operand (MOVE source, comparison operand, DISPLAY item).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Src {
    Loc(Loc),
    /// Alphanumeric literal (EBCDIC bytes).
    Alnum(Vec<u8>),
    /// Numeric literal: value and its digits as EBCDIC characters.
    NumLit(Dec, Vec<u8>),
    Fig(Fig),
    /// ALL literal: repeated to the receiving length.
    All(Vec<u8>),
    /// Numeric expression value (arithmetic, numeric functions).
    Num(Ex),
    /// Alphanumeric function result.
    Func(Func, Vec<Arg>, Option<(Box<Ex>, Option<Box<Ex>>)>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rel {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpMode {
    Numeric,
    Alnum,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClassKind {
    Numeric,
    Alphabetic,
    Lower,
    Upper,
    /// User class: the set of allowed EBCDIC bytes.
    Set(Vec<u8>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignKind {
    Positive,
    Negative,
    Zero,
}

/// Condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Cx {
    Cmp(Src, Rel, Src, CmpMode),
    Class(Src, ClassKind),
    Sign(Ex, SignKind),
    Switch(u32, bool),
    Not(Box<Cx>),
    And(Box<Cx>, Box<Cx>),
    Or(Box<Cx>, Box<Cx>),
    /// The size-error flag of the last arithmetic statement.
    SizeError,
    /// I/O condition of the last I/O statement (AT END, INVALID KEY, ...).
    IoCond,
    /// The exception condition of the last CALL.
    CallException,
    /// STRING/UNSTRING overflow condition.
    Overflow,
    True,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpenMode {
    Input,
    Output,
    InputOutput,
    Extend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Org {
    Sequential,
    LineSequential,
    Relative,
    Indexed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessMode {
    Sequential,
    Random,
    Dynamic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDef {
    pub name: String,
    pub assign: String,
    pub org: Org,
    pub access: AccessMode,
    pub optional: bool,
    /// Record area descriptor (the largest record).
    pub record: u32,
    pub min_len: u32,
    pub max_len: u32,
    pub record_depending: Option<Loc>,
    pub status: Option<Loc>,
    pub relative_key: Option<Loc>,
    pub record_key: Option<Loc>,
    pub alternate_keys: Vec<(Loc, bool)>,
    /// The file is written with ADVANCING (a print file).
    pub print: bool,
    pub linage: Option<LinageDef>,
    pub sort: bool,
    /// Declarative procedure (paragraph index) for errors.
    pub error_proc: Option<u32>,
    /// EXTERNAL file: its state is shared by name across programs.
    pub external: bool,
    /// GLOBAL file of a containing program: (module program index, file
    /// index there) holding the file's state.
    pub owner: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinageDef {
    pub lines: Ex,
    pub footing: Option<Ex>,
    pub top: Option<Ex>,
    pub bottom: Option<Ex>,
    /// LINAGE-COUNTER item.
    pub counter: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Advance {
    Lines(Ex),
    Page,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CallArgMode {
    Reference(Loc),
    Content(Src, u32),
    Value(Ex),
    Omitted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InspectStep {
    Tally {
        counter: Loc,
        kind: InspectKind,
        before: Option<Src>,
        after: Option<Src>,
    },
    Replace {
        kind: InspectKind,
        by: Src,
        before: Option<Src>,
        after: Option<Src>,
    },
    Convert {
        from: Src,
        to: Src,
        before: Option<Src>,
        after: Option<Src>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InspectKind {
    Characters,
    All(Src),
    Leading(Src),
    First(Src),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortKey {
    pub ascending: bool,
    pub desc: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Instr {
    /// No operation (placeholder).
    Nop,
    Move(Src, Loc),
    /// Store an arithmetic result; sets the size-error flag on overflow.
    /// When `check` is set, a receiver with a size error is not changed.
    Store {
        value: Ex,
        dst: Loc,
        rounded: bool,
        check: bool,
    },
    /// DIVIDE ... REMAINDER: quotient into `quot`, remainder into `rem`.
    DivRem {
        dividend: Ex,
        divisor: Ex,
        quot: Loc,
        rounded: bool,
        rem: Loc,
        check: bool,
    },
    ClearSizeError,
    SetTemp(u32, Ex),
    Jump(u32),
    JumpIf(Cx, u32),
    JumpUnless(Cx, u32),
    /// Executes paragraphs `start`..=`end` and returns to `ret`.
    Perform {
        start: u32,
        end: u32,
        ret: u32,
    },
    /// End of paragraph `para`: return if it ends the active PERFORM.
    ParaEnd(u32),
    /// After an I/O statement on `file`: runs the applicable USE AFTER
    /// ERROR procedure if the statement failed. `handled` bit 0: the
    /// statement has AT END, bit 1: INVALID KEY.
    UseCheck {
        file: u32,
        handled: u8,
    },
    GoDepending(Ex, Vec<u32>),
    /// GO TO whose target can be changed by ALTER.
    AlterableGo(u32),
    Alter(u32, u32),
    StopRun,
    StopLiteral(Src),
    ExitProgram,
    GoBack,
    Display {
        items: Vec<Src>,
        upon_console: bool,
        advance: bool,
    },
    Accept {
        dst: Loc,
        from: AcceptFrom,
    },
    Open(u32, OpenMode),
    Close(u32, bool),
    /// CLOSE REEL / UNIT.
    CloseReel(u32),
    Write {
        file: u32,
        /// Descriptor of the record named in the statement.
        record: u32,
        from: Option<Src>,
        advance: Option<(bool, Advance)>,
    },
    Rewrite {
        file: u32,
        record: u32,
        from: Option<Src>,
    },
    Read {
        file: u32,
        next: bool,
        into: Option<Loc>,
        key: Option<u32>,
    },
    Delete(u32),
    Start {
        file: u32,
        rel: Rel,
        key: Option<Loc>,
    },
    Call {
        program: Src,
        args: Vec<CallArgMode>,
        returning: Option<Loc>,
    },
    Cancel(Src),
    Initialize {
        dst: Loc,
        replacing: Vec<(Category, Src)>,
    },
    Inspect(Loc, Vec<InspectStep>),
    StringStmt {
        sources: Vec<(Src, Option<Src>)>,
        into: Loc,
        pointer: Option<Loc>,
    },
    Unstring {
        source: Loc,
        delims: Vec<(bool, Src)>,
        into: Vec<(Loc, Option<Loc>, Option<Loc>)>,
        pointer: Option<Loc>,
        tallying: Option<Loc>,
    },
    /// SET index/item TO value (index semantics when the target is an
    /// index; occurrence numbers otherwise).
    SetIndex(Loc, Ex),
    SetSwitch(u32, bool),
    /// SEARCH: varies `index` over the table until `whens` (one jump per
    /// WHEN) - compiled as a loop; this increments the index and jumps to
    /// `at_end` when it exceeds `max`.
    SearchStep {
        index: Loc,
        max: Ex,
        at_end: u32,
    },
    Sort {
        file: u32,
        keys: Vec<SortKey>,
        duplicates: bool,
        using: Vec<u32>,
        giving: Vec<u32>,
        input: Option<(u32, u32)>,
        output: Option<(u32, u32)>,
        collating: Option<Vec<u16>>,
        merge: bool,
    },
    Release(u32, Option<Src>),
    Return(u32, Option<Loc>),
    /// Execute the next statement only once per program activation (used
    /// for one-time initialisation of nested INITIAL programs).
    Unsupported(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcceptFrom {
    Stdin,
    Date,
    DateYyyymmdd,
    Day,
    DayYyyyddd,
    DayOfWeek,
    Time,
}

//! ZIR: the compiled form of a COBOL program, executed by [`crate::vm`].
//!
//! Control flow is a flat instruction list with jumps; data is addressed
//! through descriptors that give the location and type of each data item in
//! the program's storage, so all data keeps its z/OS byte layout (EBCDIC,
//! zoned, packed and big-endian binary).

use serde::{Deserialize, Serialize};
use zeus_decimal::Dec;

pub use crate::sema::picture::{Category, EditSym};

/// Linkage selected for a CALL or SET TO ENTRY. Boolean serialization
/// retains compatibility with modules containing STATIC/DYNAMIC directives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "InterfaceValue", into = "InterfaceValue")]
pub enum CallInterface {
    Static,
    Dynamic,
    Dll,
}
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum InterfaceValue {
    Legacy(bool),
    Name(String),
}
impl TryFrom<InterfaceValue> for CallInterface {
    type Error = String;
    fn try_from(value: InterfaceValue) -> Result<Self, String> {
        match value {
            InterfaceValue::Legacy(false) => Ok(Self::Static),
            InterfaceValue::Legacy(true) => Ok(Self::Dynamic),
            InterfaceValue::Name(name) if name == "DLL" => Ok(Self::Dll),
            _ => Err("invalid CALL interface".into()),
        }
    }
}
impl From<CallInterface> for InterfaceValue {
    fn from(value: CallInterface) -> Self {
        match value {
            CallInterface::Static => Self::Legacy(false),
            CallInterface::Dynamic => Self::Legacy(true),
            CallInterface::Dll => Self::Name("DLL".into()),
        }
    }
}
impl CallInterface {
    pub fn default_for(dynam: bool, dll: bool) -> Self {
        if dynam {
            Self::Dynamic
        } else if dll {
            Self::Dll
        } else {
            Self::Static
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Static => "STATIC",
            Self::Dynamic => "DYNAMIC",
            Self::Dll => "DLL",
        }
    }
}

/// A whole compilation unit: the outermost program and nested programs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    #[serde(default)]
    pub diagnostics: Vec<crate::diag::Diag>,
    #[serde(default)]
    pub optimize: u8,
    pub programs: Vec<ProgramCode>,
    /// The CCSID of the literals (the code page the module was compiled
    /// for).
    #[serde(default = "default_ccsid")]
    pub ccsid: u32,
    /// The source files (the program and its copy members), indexed by
    /// the file numbers of `ProgramCode::stmt_pos` (for coverage).
    #[serde(default)]
    pub sources: Vec<String>,
    /// UTC compilation timestamp; absent in modules produced by older Zeus.
    #[serde(default)]
    pub compiled_at: Option<String>,
}

impl Module {
    pub fn compilation_return_code(&self) -> i32 {
        self.diagnostics
            .iter()
            .map(|d| d.severity.return_code())
            .max()
            .unwrap_or(0)
    }
}

fn default_ccsid() -> u32 {
    37
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgramCode {
    /// Per-compile-unit metadata retained when the binder combines objects.
    #[serde(default)]
    pub ccsid: Option<u32>,
    #[serde(default)]
    pub compiled_at: Option<String>,
    #[serde(default)]
    pub lp64: bool,
    /// Special registers and LINKAGE indexes allocated for each invocation.
    #[serde(default)]
    pub invocation_storage: Vec<u32>,
    #[serde(default)]
    pub thread: bool,
    #[serde(default)]
    pub dll: bool,
    #[serde(default)]
    pub exportall: bool,
    #[serde(default)]
    pub ssrange: bool,
    #[serde(default)]
    pub dynam: bool,
    #[serde(default)]
    pub decimal_comma: bool,
    #[serde(default)]
    pub currency_sign: Option<String>,
    #[serde(default)]
    pub rerun: Vec<Rerun>,
    #[serde(default)]
    pub dynamic_init: Vec<(Loc, Vec<u8>)>,
    #[serde(default)]
    pub numeric: Option<crate::numeric::Settings>,
    pub name: String,
    #[serde(default)]
    pub recursive: bool,
    pub initial: bool,
    pub common: bool,
    /// Index of the containing program (nested programs).
    pub parent: Option<u32>,
    /// Initial image of the static storage (working-storage with VALUEs,
    /// file record areas, index items).
    pub static_init: Vec<u8>,
    /// LOCAL-STORAGE (offset, length) in the image, initialized per call.
    /// Older modules did not distinguish LOCAL-STORAGE from WORKING-STORAGE.
    #[serde(default)]
    pub local_storage: Option<(u32, u32)>,
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
    #[serde(default)]
    pub using_by_value: Vec<bool>,
    #[serde(default)]
    pub entries: Vec<EntryPoint>,
    /// PROCEDURE DIVISION RETURNING result in LINKAGE storage.
    #[serde(default)]
    pub result: Option<Loc>,
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
    #[serde(default)]
    pub label_uses: Vec<LabelUse>,
    /// Segmentation: (first instruction, segment number) of every
    /// paragraph in code order; empty when the program has no independent
    /// segments.
    pub segments: Vec<(u32, u8)>,
    /// Segment number of each ALTER-able GO TO slot.
    pub alter_segments: Vec<u8>,
    /// USE FOR DEBUGGING sections: first and last paragraph.
    pub debug_decls: Vec<(u32, u32)>,
    /// Descriptor of DEBUG-ITEM.
    pub debug_item: Option<u32>,
    /// Report Writer reports.
    pub reports: Vec<ReportDef>,
    /// EXEC CICS / SQL / DLI statements (operand of `Instr::Exec`).
    #[serde(default)]
    pub execs: Vec<ExecDef>,
    /// EXEC blocks of the data division (SQL DECLARE CURSOR, ...).
    #[serde(default)]
    pub data_execs: Vec<ExecDef>,
    /// Descriptor of the RETURN-CODE special register.
    #[serde(default)]
    pub return_code: Option<u32>,
    /// The statements of the procedure division: source file and line.
    #[serde(default)]
    pub stmt_pos: Vec<(u32, u32)>,
    /// The statement of each instruction (an index into `stmt_pos`, or
    /// `u32::MAX`).
    #[serde(default)]
    pub instr_stmt: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rerun {
    pub checkpoint: String,
    pub file: Option<u32>,
    pub records: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryPoint {
    #[serde(default)]
    pub using_by_value: Vec<bool>,
    pub name: String,
    pub pc: u32,
    pub using: Vec<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecKind {
    Cics,
    Sql,
    Dli,
    SqlIms,
}

/// An embedded EXEC statement, run by the host interface (CICS, Db2, IMS).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecDef {
    pub kind: ExecKind,
    /// Source line (for diagnostics and EIB fields).
    pub line: u32,
    /// CICS / DLI: command word; SQL: first keyword.
    pub command: String,
    pub options: Vec<ExecOpt>,
    /// SQL text with `?` for each host variable.
    pub sql: Option<String>,
    /// SQL host variables and indicator variables, in order.
    pub host: Vec<(Loc, Option<Loc>)>,
    /// Which host variables are VARCHAR structures (length and text).
    #[serde(default)]
    pub host_varchar: Vec<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecOpt {
    pub name: String,
    pub arg: Option<ExecVal>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecVal {
    /// A data area: its address and length are passed.
    Area(Loc),
    /// A value: literal or expression.
    Value(Src),
    /// `ADDRESS OF item`: a pointer value or receiver (SET option).
    Address(Loc),
    /// A procedure name (HANDLE CONDITION / AID / ABEND LABEL).
    Label(u32),
    /// A name that is not a data item (DL/I segment names, ...) or text
    /// that is not a COBOL expression.
    Name(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportDef {
    pub name: String,
    /// The print file the report is written to.
    pub file: u32,
    pub page: Option<PageDef>,
    /// Control data items, major to minor; None = FINAL.
    pub controls: Vec<Option<Loc>>,
    pub groups: Vec<GroupDef>,
    pub sums: Vec<SumDef>,
    /// LINE-COUNTER and PAGE-COUNTER descriptors.
    pub line_counter: u32,
    pub page_counter: u32,
    /// CODE clause characters prefixed to every line.
    pub code: Option<Vec<u8>>,
}

/// PAGE clause, with the defaults of the omitted parts filled in.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PageDef {
    pub limit: u32,
    pub heading: u32,
    pub first_detail: u32,
    pub last_detail: u32,
    pub footing: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroupKind {
    ReportHeading,
    PageHeading,
    ControlHeading,
    Detail,
    ControlFooting,
    PageFooting,
    ReportFooting,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupDef {
    pub name: String,
    pub kind: GroupKind,
    /// Control level (index into `controls`) of CH / CF groups.
    pub control: Option<u32>,
    pub lines: Vec<LineDef>,
    pub next_group: Option<LinePos>,
    /// USE BEFORE REPORTING section: (first, last paragraph).
    pub use_proc: Option<(u32, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinePos {
    Absolute(u32),
    Plus(u32),
    NextPage,
    AbsoluteNextPage(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineDef {
    pub pos: LinePos,
    pub fields: Vec<FieldDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDef {
    /// Print column (1-based); None for a field that is not printed.
    pub column: Option<u32>,
    /// The field's storage (its PICTURE does the editing).
    pub desc: u32,
    pub value: FieldValue,
    pub group_indicate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FieldValue {
    /// The field holds a constant (VALUE clause).
    Constant,
    Source(Src),
    /// Sum counter.
    Sum(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SumDef {
    /// Report group holding the sum counter.
    pub group: u32,
    pub operands: Vec<SumOperand>,
    /// Detail groups whose GENERATE adds the operands (empty: any).
    pub upon: Vec<u32>,
    /// Control level at whose footing the counter is reset; None: the
    /// level of its own group.
    pub reset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SumOperand {
    /// A data item added when a detail is generated (subtotalling).
    Data(Ex),
    /// Another sum counter, added when its control footing is presented
    /// (rolling forward).
    Sum(u32),
}

/// How control reached a procedure (DEBUG-CONTENTS of a procedure
/// debugging section).
pub mod debug_cause {
    pub const FALL_THROUGH: u8 = 0;
    pub const START_PROGRAM: u8 = 1;
    pub const GO_TO: u8 = 2;
    pub const PERFORM: u8 = 3;
    pub const SORT_INPUT: u8 = 4;
    pub const SORT_OUTPUT: u8 = 5;
    pub const MERGE_OUTPUT: u8 = 6;
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
    /// Where it is in the source: file and line.
    #[serde(default)]
    pub pos: (u32, u32),
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
    #[serde(default)]
    pub count: Option<Box<Ex>>,
    #[serde(default)]
    pub dynamic_stride: Option<Box<Ex>>,
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
    National,
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
    /// DBCS data: double-byte characters without SO / SI.
    Dbcs,
    /// National data: UTF-16 big-endian.
    National,
    Numeric(NumType),
    NumericEdited {
        #[serde(default)]
        national: bool,
        edit: Vec<EditSym>,
        digits: u32,
        scale: i32,
        blank_zero: bool,
        currency: u8,
        point: u8,
    },
    AlphanumericEdited {
        #[serde(default)]
        national: bool,
        edit: Vec<EditSym>,
    },
    /// USAGE INDEX / index-name: 4-byte binary occurrence number.
    Index,
    Pointer,
    ProcedurePointer,
    FunctionPointer,
    /// COMP-1 (short) / COMP-2 (long): IBM hexadecimal floating point.
    Float {
        long: bool,
    },
    ExternalFloat {
        picture: String,
        national: bool,
        comma: bool,
    },
}

impl DataType {
    pub fn abi_type(&self) -> Self {
        let mut ty = self.clone();
        match &mut ty {
            Self::NumericEdited {
                currency,
                point,
                edit,
                ..
            } => {
                for symbol in edit {
                    if let EditSym::Insert(byte) = symbol
                        && matches!(crate::ebcdic::to_latin1(*byte), b',' | b'.')
                    {
                        *byte = 0;
                    }
                }
                *currency = 0;
                *point = 0;
            }
            Self::ExternalFloat { picture, comma, .. } => {
                *picture = picture.replace(',', ".");
                *comma = false;
            }
            _ => {}
        }
        ty
    }
    pub fn by_value_allowed(&self, len: u32) -> bool {
        match self {
            Self::Numeric(t) => matches!(t.usage, NumUsage::Binary | NumUsage::Native),
            Self::Float { .. } | Self::Pointer | Self::ProcedurePointer | Self::FunctionPointer => {
                true
            }
            Self::Alphabetic { .. } | Self::Alphanumeric { .. } => len == 1,
            Self::National => len == 2,
            _ => false,
        }
    }
    pub fn is_floating(&self) -> bool {
        matches!(self, Self::Float { .. } | Self::ExternalFloat { .. })
    }
    pub fn is_entry_pointer(&self) -> bool {
        matches!(self, Self::ProcedurePointer | Self::FunctionPointer)
    }
    pub fn is_national(&self) -> bool {
        matches!(
            self,
            Self::National
                | Self::Numeric(NumType {
                    usage: NumUsage::National,
                    ..
                })
                | Self::NumericEdited { national: true, .. }
                | Self::AlphanumericEdited { national: true, .. }
                | Self::ExternalFloat { national: true, .. }
        )
    }

    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            DataType::Numeric(_)
                | DataType::Index
                | DataType::Float { .. }
                | DataType::ExternalFloat { .. }
        )
    }
}

/// Descriptor of a data item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Desc {
    #[serde(default)]
    pub definition: (u32, u32),
    #[serde(default)]
    pub table_bounds: Option<TableBounds>,
    #[serde(default)]
    pub right_justify: bool,
    #[serde(default)]
    pub dynamic_offset: Option<Box<Ex>>,
    #[serde(default)]
    pub dynamic_len: Option<Box<Ex>>,
    #[serde(default)]
    pub abi: Vec<AbiField>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableBounds {
    pub offset: u32,
    pub dynamic_offset: Option<Box<Ex>>,
    pub len: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AbiField {
    pub offset: u32,
    pub len: u32,
    pub ty: DataType,
    pub synchronized: bool,
    pub occurs: Vec<u32>,
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
    Float(String),
    Load(Loc),
    Bin(Op, Box<Ex>, Box<Ex>),
    Neg(Box<Ex>),
    Temp(u32),
    /// Length of the referenced item in bytes.
    LengthOf(Loc),
    /// Address of the referenced item (0 for an unaddressable linkage
    /// item).
    AddressOf(Loc),
    Func(Func, Vec<Arg>),
}

/// Intrinsic function argument.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Arg {
    Num(Ex),
    Src(Src),
    /// All occurrences of a table (subscript ALL).
    All(Loc),
    /// A table argument retaining fixed subscripts alongside ALL positions.
    Table {
        loc: Loc,
        subs: Vec<Option<Ex>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Func {
    /// NATIONAL-OF(alphanumeric [, CCSID]): national (UTF-16) data.
    NationalOf,
    /// DISPLAY-OF(national [, CCSID]): alphanumeric data in a code page.
    DisplayOf,
    /// TRIM (Enterprise COBOL); a second argument "L" / "T" trims
    /// only leading / trailing spaces.
    Trim,
    TestNumVal,
    TestNumValC,
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
    ByteLength,
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
    WhenCompiledRegister,
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
    Apostrophe,
}

/// A general operand (MOVE source, comparison operand, DISPLAY item).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Src {
    Float(String),
    Loc(Loc),
    /// Alphanumeric literal (EBCDIC bytes).
    Alnum(Vec<u8>),
    /// National literal (UTF-16 big-endian).
    National(Vec<u8>),
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
    #[serde(default)]
    pub same_sort_area: Vec<String>,
    #[serde(default)]
    pub label_records: Vec<Loc>,
    pub name: String,
    pub assign: String,
    pub org: Org,
    pub access: AccessMode,
    pub optional: bool,
    /// Record area descriptor (the largest record).
    pub record: u32,
    pub min_len: u32,
    pub max_len: u32,
    /// Explicit or inferred QSAM F/V mode; absent in legacy object files.
    #[serde(default)]
    pub recording_mode: Option<char>,
    #[serde(default)]
    pub block_size: Option<u32>,
    #[serde(default)]
    pub reserve: Option<u32>,
    #[serde(default)]
    pub write_only: bool,
    #[serde(default)]
    pub label_standard: bool,
    /// CODE-SET STANDARD-1/2: external ASCII, internal EBCDIC.
    #[serde(default)]
    pub code_set_ascii: bool,
    /// SAME AREA peers (VSAM only): at most one can be open.
    #[serde(default)]
    pub same_area: Vec<String>,
    pub record_depending: Option<Loc>,
    pub status: Option<Loc>,
    #[serde(default)]
    pub vsam_status: Option<Loc>,
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
pub struct LabelUse {
    pub files: Vec<u32>,
    pub mode: Option<OpenMode>,
    pub start: u32,
    pub end: u32,
    pub global: bool,
    pub beginning: Option<bool>,
    pub reel: Option<bool>,
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
    ValueData(Src),
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
    MoreLabels,
    NumericContext {
        decimals: i32,
        floating: bool,
    },
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
    /// Start of a paragraph of segment n (programs with independent
    /// segments): entering an independent segment from another segment
    /// puts it in its initial state.
    Segment(u8),
    /// SET ADDRESS OF a linkage record (linkage base number) TO value.
    SetAddress(u32, Ex),
    SetEntry(Vec<Loc>, Src),
    /// Explicit CALLINTERFACE selection; old SetEntry retains compiler defaults.
    SetEntryWithInterface(Vec<Loc>, Src, CallInterface),
    /// Report Writer statements (report index; group index for GENERATE
    /// of a detail, None for summary reporting).
    Initiate(u32),
    Generate {
        report: u32,
        group: Option<u32>,
    },
    Terminate(u32),
    SuppressPrinting,
    /// Debugging: records how control is about to reach a procedure
    /// (a `debug_cause` value) and the source line causing it.
    DebugCause(u8, u32),
    /// Debugging: runs debugging section `decl` for procedure `name`.
    /// `contents` overrides the cause text (ALTER).
    DebugProc {
        decl: u32,
        name: String,
        contents: Option<String>,
        line: u32,
    },
    /// Debugging: runs debugging section `decl` for identifier `name`.
    DebugData {
        decl: u32,
        name: String,
        loc: Loc,
        line: u32,
    },
    /// Debugging: runs debugging section `decl` for file `name` after a
    /// successful input-output statement (`record`: READ).
    DebugFile {
        decl: u32,
        name: String,
        record: Option<u32>,
        line: u32,
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
    OpenTape {
        file: u32,
        mode: OpenMode,
        reversed: bool,
        no_rewind: bool,
    },
    CloseTape {
        file: u32,
        no_rewind: bool,
        removal: bool,
    },
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
        #[serde(default)]
        interface: Option<CallInterface>,
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
    SortBegin(u32),
    SortRead(u32),
    SortOrder {
        file: u32,
        keys: Vec<SortKey>,
        collating: Option<Vec<u16>>,
        merge: bool,
    },
    SortWrite(u32),
    SortCheckpoint,
    SortEnd,
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
    /// EXEC statement (index into `ProgramCode::execs`).
    Exec(u32),
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

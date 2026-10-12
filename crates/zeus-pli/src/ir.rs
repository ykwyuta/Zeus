//! The compiled program: blocks, variables with their storage, and typed
//! statements and expressions in which every conversion is explicit.

use serde::{Deserialize, Serialize};

use crate::types::Ty;

pub type VarId = u32;
pub type BlockId = u32;
pub type ProcId = u32;
pub type LabelId = u32;
pub type FileId = u32;
pub type LoopId = u32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    /// Name of the main (or first) external procedure.
    pub name: String,
    pub ccsid: u32,
    pub blocks: Vec<Block>,
    pub procs: Vec<Proc>,
    pub vars: Vec<Var>,
    pub files: Vec<FileDecl>,
    pub labels: Vec<LabelDef>,
    pub formats: Vec<Vec<Fmt>>,
    /// Static storage: size and initial image.
    pub static_size: u32,
    pub static_init: Vec<u8>,
    /// Static variables initialised at load time (values not known at
    /// compile time).
    pub static_inits: Vec<Init>,
    pub main: Option<ProcId>,
    /// Programmer-defined CONDITION names.
    pub conditions: Vec<String>,
    /// Source file names (for messages).
    pub sources: Vec<String>,
    /// `REFER` extents of BASED structures.
    #[serde(default)]
    pub refers: Vec<Refer>,
}

/// An extent `init REFER(object)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Refer {
    /// The member holding the extent.
    pub object: VarId,
    /// The extent at allocation (evaluated in the declaring block).
    pub init: Expr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    pub parent: Option<BlockId>,
    pub proc: Option<ProcId>,
    pub name: String,
    pub auto_size: u32,
    /// Extent expressions evaluated at block entry into slots.
    pub extents: Vec<Expr>,
    /// INITIAL of automatic variables (after the extents).
    pub inits: Vec<Init>,
    pub body: List,
    /// An ON-unit block.
    pub on_unit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proc {
    pub name: String,
    pub block: BlockId,
    pub params: Vec<VarId>,
    pub returns: Option<Ty>,
    pub main: bool,
    pub external: bool,
    pub recursive: bool,
    /// Secondary entry points: name, parameters, index in the body.
    pub entries: Vec<(String, Vec<VarId>, usize)>,
}

/// Bound of a dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Bound {
    Const(i64),
    /// From the argument (parameter) or the INITIAL list count.
    Star,
    Slot(u32),
    /// `REFER`: the module's refer entry n.
    Refer(u32),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dim {
    pub lb: Bound,
    pub ub: Bound,
    /// Distance between elements in bytes (bits for packed bit arrays).
    pub stride: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Storage {
    Static(u32),
    Auto(u32),
    /// AUTOMATIC with extents decided at block entry: the frame holds the
    /// address of storage allocated at entry.
    AutoDyn(u32),
    Param(u32),
    /// BASED with its declared locator.
    Based(Option<Box<Expr>>),
    /// DEFINED on a base reference, POSITION.
    Defined(Box<Ref>, Option<Box<Expr>>),
    /// CONTROLLED: the static word at the offset addresses the current
    /// generation. The extents (`*`: given on ALLOCATE) are evaluated
    /// into the generation's slots.
    Controlled(u32, Vec<Option<Expr>>),
    /// A structure member: storage of its level-1 root.
    Member,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Var {
    pub name: String,
    pub block: BlockId,
    pub storage: Storage,
    /// The level-1 variable (itself for level 1).
    pub root: VarId,
    pub parent: Option<VarId>,
    pub members: Vec<VarId>,
    /// Offset from the root element's start, in bits.
    pub bit_offset: u64,
    /// Scalar type; None for a structure.
    pub ty: Option<Ty>,
    /// All dimensions (inherited ones first).
    pub dims: Vec<Dim>,
    /// Size of one element in bytes (structures: mapped length).
    pub size: u32,
    pub level: u32,
    /// Bit-packed (unaligned) bit string.
    pub packed_bits: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Init {
    pub var: VarId,
    /// Values for the elements in row-major order (None: leave).
    pub values: Vec<Option<Expr>>,
    /// Repeat the last value for all remaining elements.
    pub fill: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileDecl {
    pub name: String,
    /// DD name (TITLE or the first 8 characters of the name).
    pub ddname: String,
    pub stream: bool,
    pub print: bool,
    pub input: bool,
    pub output: bool,
    pub update: bool,
    pub keyed: bool,
    pub direct: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelDef {
    pub name: String,
    pub block: BlockId,
    pub list: u32,
    pub index: usize,
}

/// A statement list (a block body, a DO group, a branch).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct List {
    pub id: u32,
    pub stmts: Vec<Node>,
}

/// Conditions that can be enabled or disabled by prefixes.
pub mod cond {
    pub const CONVERSION: u16 = 1;
    pub const FIXEDOVERFLOW: u16 = 2;
    pub const OVERFLOW: u16 = 4;
    pub const UNDERFLOW: u16 = 8;
    pub const ZERODIVIDE: u16 = 16;
    pub const SIZE: u16 = 32;
    pub const STRINGRANGE: u16 = 64;
    pub const STRINGSIZE: u16 = 128;
    pub const SUBSCRIPTRANGE: u16 = 256;
    pub const INVALIDOP: u16 = 512;
    /// enabled by default
    pub const DEFAULT: u16 =
        CONVERSION | FIXEDOVERFLOW | OVERFLOW | UNDERFLOW | ZERODIVIDE | INVALIDOP;

    pub fn bit(name: &str) -> Option<u16> {
        Some(match name {
            "CONVERSION" | "CONV" => CONVERSION,
            "FIXEDOVERFLOW" | "FOFL" => FIXEDOVERFLOW,
            "OVERFLOW" | "OFL" => OVERFLOW,
            "UNDERFLOW" | "UFL" => UNDERFLOW,
            "ZERODIVIDE" | "ZDIV" => ZERODIVIDE,
            "SIZE" => SIZE,
            "STRINGRANGE" | "STRG" => STRINGRANGE,
            "STRINGSIZE" | "STRZ" => STRINGSIZE,
            "SUBSCRIPTRANGE" | "SUBRG" => SUBSCRIPTRANGE,
            "INVALIDOP" => INVALIDOP,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub labels: Vec<LabelId>,
    pub line: u32,
    pub file: u16,
    /// Enabled conditions (cond bits).
    pub enabled: u16,
    pub stmt: Stmt,
}

/// A condition as named in ON / SIGNAL / REVERT.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Cond {
    Error,
    Finish,
    Conversion,
    FixedOverflow,
    Overflow,
    Underflow,
    ZeroDivide,
    Size,
    StringRange,
    StringSize,
    SubscriptRange,
    Storage,
    Area,
    Attention,
    InvalidOp,
    AnyCondition,
    EndFile(FileId),
    EndPage(FileId),
    Key(FileId),
    Record(FileId),
    Transmit(FileId),
    UndefinedFile(FileId),
    Name(FileId),
    Condition(u32),
}

impl Cond {
    pub fn name(&self) -> &'static str {
        match self {
            Cond::Error => "ERROR",
            Cond::Finish => "FINISH",
            Cond::Conversion => "CONVERSION",
            Cond::FixedOverflow => "FIXEDOVERFLOW",
            Cond::Overflow => "OVERFLOW",
            Cond::Underflow => "UNDERFLOW",
            Cond::ZeroDivide => "ZERODIVIDE",
            Cond::Size => "SIZE",
            Cond::StringRange => "STRINGRANGE",
            Cond::StringSize => "STRINGSIZE",
            Cond::SubscriptRange => "SUBSCRIPTRANGE",
            Cond::Storage => "STORAGE",
            Cond::Area => "AREA",
            Cond::Attention => "ATTENTION",
            Cond::InvalidOp => "INVALIDOP",
            Cond::AnyCondition => "ANYCONDITION",
            Cond::EndFile(_) => "ENDFILE",
            Cond::EndPage(_) => "ENDPAGE",
            Cond::Key(_) => "KEY",
            Cond::Record(_) => "RECORD",
            Cond::Transmit(_) => "TRANSMIT",
            Cond::UndefinedFile(_) => "UNDEFINEDFILE",
            Cond::Name(_) => "NAME",
            Cond::Condition(_) => "CONDITION",
        }
    }
}

/// A reference to a variable element or aggregate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ref {
    pub var: VarId,
    /// One per dimension of the variable; None: `*` (cross section).
    pub subs: Vec<Option<Expr>>,
    pub locator: Option<Box<Expr>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Expr {
    pub k: Box<Ek>,
    pub ty: Ty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitOp {
    And,
    Or,
    Xor,
}

/// How a callee is identified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Callee {
    Proc(ProcId),
    /// Secondary entry point of a procedure.
    Entry(ProcId, usize),
    /// External entry resolved at run time (another module).
    External(String),
    /// Entry variable.
    Var(Expr),
}

/// An argument: passed by reference (a variable of matching attributes)
/// or as a dummy (an expression converted to the parameter's type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Arg {
    Ref(Ref),
    Dummy(Expr),
    /// An aggregate expression or array argument.
    Omitted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Ek {
    /// A constant value (host form for strings; arithmetic text).
    Fixed(i128, i16),
    Float(f64),
    Char(Vec<u8>),
    Bit(Vec<bool>),
    Null,
    SysNull,
    /// Load a scalar element.
    Load(Ref),
    /// Assignment-like conversion to `ty`.
    Conv(Expr),
    Neg(Expr),
    Arith(ArOp, Expr, Expr),
    Cmp(CmpOp, Expr, Expr),
    Concat(Expr, Expr),
    Bits(BitOp, Expr, Expr),
    Not(Expr),
    Call(Callee, Vec<Arg>),
    Builtin(crate::builtin::Bif, Vec<Expr>),
    /// Built-ins on references: ADDR, LENGTH (varying), DIM, LBOUND,
    /// HBOUND, SIZE, STRING, ALLOCATION, and aggregate arguments of SUM,
    /// PROD, ALL, ANY.
    RefBuiltin(crate::builtin::Bif, Ref, Vec<Expr>),
    Label(LabelId),
    EntryConst(ProcId),
    FileConst(FileId),
    /// The current value of an ON built-in (ONCODE ...).
    OnValue(crate::builtin::Bif),
    /// An aggregate (array / structure) used as a whole: its bytes as a
    /// character string (record I/O, STRING).
    Aggregate(Ref),
    /// The k-th index of the element being processed by an aggregate
    /// assignment (or a repetitive specification).
    Index(u8),
    /// The value of the SELECT subject.
    Subject,
}

/// Assignment target.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Target {
    Ref(Ref),
    /// SUBSTR(ref, start[, length]) pseudovariable
    Substr(Ref, Expr, Option<Expr>),
    /// UNSPEC(ref)
    Unspec(Ref),
    /// STRING(ref)
    String(Ref),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DoKind {
    Group,
    Loop {
        while_: Option<Expr>,
        until: Option<Expr>,
    },
    Iter {
        var: Target,
        var_ty: Ty,
        specs: Vec<DoSpec>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoSpec {
    pub start: Expr,
    pub to: Option<Expr>,
    pub by: Option<Expr>,
    pub repeat: Option<Expr>,
    pub while_: Option<Expr>,
    pub until: Option<Expr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataItem {
    Expr(Expr),
    /// A target for GET (reference or pseudovariable).
    Target(Target, Ty),
    /// Aggregate reference (elements in order).
    Aggregate(Ref),
    Repeat(Vec<DataItem>, Box<DoKind>),
    /// PUT / GET DATA: name and element.
    Named(String, Box<DataItem>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Fmt {
    A(Option<Expr>),
    B(Option<Expr>),
    F(Expr, Option<Expr>, Option<Expr>),
    E(Expr, Option<Expr>, Option<Expr>),
    P(Box<crate::picture::Picture>),
    X(Expr),
    Skip(Option<Expr>),
    Page,
    Line(Expr),
    Column(Expr),
    R(u32),
    Repeat(Expr, Vec<Fmt>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Transmit {
    List(Vec<DataItem>),
    Edit(Vec<(Vec<DataItem>, Vec<Fmt>)>),
    Data(Vec<DataItem>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stream {
    pub file: Option<Expr>,
    pub string: Option<StringTarget>,
    /// SKIP(n); SKIP alone is SKIP(1).
    pub skip: Option<Expr>,
    pub page: bool,
    pub line: Option<Expr>,
    pub data: Option<Transmit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StringTarget {
    /// GET STRING(expr)
    Source(Expr),
    /// PUT STRING(target)
    Target(Target, Ty),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecordIo {
    pub file: Option<Expr>,
    pub into: Option<(Target, u32)>,
    pub from: Option<Expr>,
    pub set: Option<Target>,
    pub key: Option<Expr>,
    pub keyto: Option<(Target, Ty)>,
    pub keyfrom: Option<Expr>,
    pub ignore: Option<Expr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSpec {
    pub file: Expr,
    pub input: bool,
    pub output: bool,
    pub update: bool,
    pub print: Option<bool>,
    pub keyed: bool,
    pub title: Option<Expr>,
    pub linesize: Option<Expr>,
    pub pagesize: Option<Expr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Stmt {
    Assign(Vec<Target>, Expr),
    /// Array assignment: targets and value use `Index(k)` placeholders for
    /// the `rank` free dimensions of `shape`, iterated in row-major order.
    ArrayAssign {
        targets: Vec<Target>,
        value: Expr,
        shape: Ref,
        rank: u8,
    },
    If(Expr, List, List),
    Do(LoopId, DoKind, List),
    Select(Option<Expr>, Vec<(Vec<Expr>, List)>, List),
    Call(Callee, Vec<Arg>),
    /// CALL of a built-in subroutine (PLIRETC, PLIDUMP ...).
    CallBuiltin(crate::builtin::Bif, Vec<Expr>),
    Return(Option<Expr>),
    GoTo(Expr),
    Leave(LoopId),
    Iterate(LoopId),
    On(Vec<Cond>, Option<BlockId>, bool),
    Signal(Cond),
    Revert(Vec<Cond>),
    Resignal,
    Stop,
    Exit,
    Put(Stream),
    Get(Stream),
    Open(Vec<OpenSpec>),
    Close(Vec<Expr>),
    Read(RecordIo),
    Write(RecordIo),
    Rewrite(RecordIo),
    Delete(RecordIo),
    /// Variable, SET target, and extents given on the statement (for a
    /// CONTROLLED variable: by generation slot).
    Allocate(Vec<(VarId, Option<Target>, Vec<Option<Expr>>)>),
    Free(Vec<(VarId, Option<Expr>)>),
    /// FETCH: the entry names and the load module titles (character).
    Fetch(Vec<(String, Option<Expr>)>),
    /// RELEASE (empty: every fetched module).
    Release(Vec<String>),
    Display(Expr, Option<(Target, Ty)>),
    Delay(Expr),
    /// EXEC SQL / CICS / DLI.
    Exec(Box<ExecStmt>),
    Begin(BlockId),
    /// Executes a statement list (DO group without loop).
    Group(List),
    Null,
}

/// Host variables of an SQL statement and their indicator variables.
pub type HostVars = Vec<(Ref, Option<Ref>)>;

/// An EXEC statement: its text with host variables replaced by `?`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecStmt {
    /// SQL, CICS or DLI.
    pub kind: String,
    pub text: String,
    /// Host variables and their indicator variables.
    pub host: HostVars,
    /// OPEN: the query of the cursor.
    pub query: Option<String>,
    pub sqlca: Option<Ref>,
    /// WHENEVER actions: 0 SQLERROR, 1 SQLWARNING, 2 NOT FOUND, and the
    /// GO TO target.
    pub whenever: Vec<(u8, Expr)>,
    /// EXEC CICS: the command (its first word) and the options.
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub options: Vec<(String, Option<ExecOperand>)>,
}

/// The argument of an EXEC CICS option.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecOperand {
    /// A data area (a variable): passed as its storage.
    Ref(Ref),
    /// A value: an expression.
    Value(Expr),
    /// A label (HANDLE CONDITION, HANDLE ABEND, HANDLE AID).
    Label(LabelId),
    /// A name that is not a variable.
    Name(String),
}

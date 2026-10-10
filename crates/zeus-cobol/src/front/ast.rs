//! Abstract syntax tree of a COBOL source program.

#![allow(clippy::large_enum_variant)]

use super::source::Pos;

#[derive(Debug, Clone, Default)]
pub struct Program {
    pub name: String,
    pub pos: Pos,
    pub initial: bool,
    pub common: bool,
    /// RECURSIVE attribute (recursive CALLs are not supported yet).
    pub recursive: bool,
    pub env: Environment,
    pub data: DataDivision,
    pub procedure: Procedure,
    pub nested: Vec<Program>,
}

#[derive(Debug, Clone, Default)]
pub struct Environment {
    pub decimal_point_comma: bool,
    pub currency_sign: Option<String>,
    /// SPECIAL-NAMES mnemonic names: (implementor-name, mnemonic-name).
    pub mnemonics: Vec<(String, String)>,
    /// Switch status condition-names: (implementor-name, condition-name, on).
    pub switch_conditions: Vec<(String, String, bool)>,
    pub alphabets: Vec<Alphabet>,
    pub symbolic_chars: Vec<(String, Vec<u32>, Option<String>)>,
    pub classes: Vec<ClassDef>,
    pub program_collating: Option<String>,
    pub files: Vec<FileControl>,
    /// I-O-CONTROL SAME RECORD AREA groups.
    pub same_record_area: Vec<Vec<String>>,
    pub multiple_file_tape: bool,
    /// WITH DEBUGGING MODE: debugging sections are active.
    pub debugging_mode: bool,
}

#[derive(Debug, Clone)]
pub struct Alphabet {
    pub name: String,
    pub kind: AlphabetKind,
}

#[derive(Debug, Clone)]
pub enum AlphabetKind {
    Native,
    StandardOne,
    StandardTwo,
    /// Literal sequence: ranges/ALSO groups of characters (code points in
    /// the native set, 1-based ordinal for numeric literals).
    Literals(Vec<Vec<AlphabetItem>>),
}

#[derive(Debug, Clone)]
pub enum AlphabetItem {
    Char(CharSpec),
    Range(CharSpec, CharSpec),
}

#[derive(Debug, Clone)]
pub enum CharSpec {
    /// Alphanumeric literal (each character its own item when not a range).
    Text(String),
    /// Numeric literal: ordinal position in the native set (1-based).
    Ordinal(u32),
}

#[derive(Debug, Clone)]
pub struct ClassDef {
    pub name: String,
    pub items: Vec<AlphabetItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Organization {
    #[default]
    Sequential,
    LineSequential,
    Relative,
    Indexed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Access {
    #[default]
    Sequential,
    Random,
    Dynamic,
}

#[derive(Debug, Clone, Default)]
pub struct FileControl {
    pub name: String,
    pub pos: Pos,
    pub optional: bool,
    /// ASSIGN TO target (literal text or implementor-name).
    pub assign: String,
    pub organization: Organization,
    pub access: Access,
    pub relative_key: Option<DataRef>,
    pub record_key: Option<DataRef>,
    pub alternate_keys: Vec<(DataRef, bool)>,
    pub file_status: Option<DataRef>,
    pub reserve: Option<u32>,
    pub padding: Option<Operand>,
    pub record_delimiter: bool,
    /// SORT/MERGE file (SD).
    pub sort: bool,
}

#[derive(Debug, Clone, Default)]
pub struct DataDivision {
    pub files: Vec<FileDescription>,
    /// REPORT SECTION.
    pub reports: Vec<ReportDescription>,
    pub working: Vec<DataEntry>,
    pub local: Vec<DataEntry>,
    pub linkage: Vec<DataEntry>,
    /// EXEC blocks of the data division (EXEC SQL DECLARE ... and the
    /// like), in source order.
    pub execs: Vec<ExecBlock>,
}

/// Kind of an embedded EXEC block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecKind {
    Cics,
    Sql,
    Dli,
}

/// An `EXEC CICS|SQL|DLI ... END-EXEC` block.
#[derive(Debug, Clone, PartialEq)]
pub struct ExecBlock {
    pub kind: ExecKind,
    pub pos: Pos,
    /// CICS / DLI: the command (first word); SQL: the first keyword.
    pub command: String,
    /// CICS / DLI options in source order (the command excluded).
    pub options: Vec<ExecOption>,
    /// SQL: the statement text with each host variable replaced by `?`,
    /// and the host variables in order.
    pub sql: Option<SqlText>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExecOption {
    pub name: String,
    pub arg: Option<ExecArg>,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExecArg {
    /// A data reference, literal, LENGTH OF / ADDRESS OF or arithmetic
    /// expression.
    Expr(Expr),
    /// Text that is not a COBOL expression (DL/I WHERE conditions, names).
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqlText {
    pub text: String,
    pub host: Vec<SqlHostVar>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SqlHostVar {
    pub var: DataRef,
    pub indicator: Option<DataRef>,
}

#[derive(Debug, Clone, Default)]
pub struct FileDescription {
    pub name: String,
    pub pos: Pos,
    pub sort: bool,
    pub external: bool,
    pub global: bool,
    pub block_contains: Option<(u32, u32, bool)>,
    /// RECORD CONTAINS min TO max / RECORD IS VARYING ...
    pub record_min: Option<u32>,
    pub record_max: Option<u32>,
    pub record_depending: Option<DataRef>,
    pub label_standard: bool,
    /// REPORT IS / REPORTS ARE
    pub reports: Vec<String>,
    pub value_of: Vec<(String, Operand)>,
    pub data_records: Vec<String>,
    pub linage: Option<Linage>,
    pub code_set: Option<String>,
    pub records: Vec<DataEntry>,
}

#[derive(Debug, Clone)]
pub struct Linage {
    pub lines: Operand,
    pub footing: Option<Operand>,
    pub top: Option<Operand>,
    pub bottom: Option<Operand>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Usage {
    #[default]
    Display,
    Binary,
    Packed,
    /// COMP-5: native binary, not truncated to the picture.
    NativeBinary,
    Index,
    Pointer,
    /// COMP-1 / COMP-2 (hexadecimal floating point)
    Float,
    Double,
}

#[derive(Debug, Clone)]
pub struct Occurs {
    pub min: u32,
    pub max: u32,
    pub depending: Option<DataRef>,
    /// ASCENDING/DESCENDING KEY: (ascending, key names)
    pub keys: Vec<(bool, Vec<DataRef>)>,
    pub indexed_by: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum ValueClause {
    Literal(Operand),
    /// Level-88 values: single values or THRU ranges.
    Condition(Vec<(Operand, Option<Operand>)>),
}

#[derive(Debug, Clone, Default)]
pub struct DataEntry {
    pub level: u8,
    pub name: Option<String>,
    pub pos: Pos,
    pub redefines: Option<String>,
    pub external: bool,
    pub global: bool,
    pub picture: Option<String>,
    pub usage: Option<Usage>,
    /// SIGN clause: (leading, separate)
    pub sign: Option<(bool, bool)>,
    pub occurs: Option<Occurs>,
    pub sync: bool,
    pub justified: bool,
    pub blank_when_zero: bool,
    pub value: Option<ValueClause>,
    /// Level 66: RENAMES a [THRU b]
    pub renames: Option<(DataRef, Option<DataRef>)>,
    /// Report group description clauses (REPORT SECTION).
    pub report: Option<Box<ReportClauses>>,
}

/// RD entry and its report group descriptions.
#[derive(Debug, Clone, Default)]
pub struct ReportDescription {
    pub name: String,
    pub pos: Pos,
    pub global: bool,
    pub code: Option<Operand>,
    /// CONTROL clause, major to minor; FINAL first when given.
    pub controls: Vec<Option<DataRef>>,
    pub page: Option<PageClause>,
    /// Report group description entries (01 entries start a group).
    pub entries: Vec<DataEntry>,
}

#[derive(Debug, Clone, Default)]
pub struct PageClause {
    pub limit: Option<u32>,
    pub heading: Option<u32>,
    pub first_detail: Option<u32>,
    pub last_detail: Option<u32>,
    pub footing: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct ReportClauses {
    pub line: Option<LineClause>,
    pub next_group: Option<LineClause>,
    pub ty: Option<GroupType>,
    pub column: Option<u32>,
    pub source: Option<Operand>,
    pub sum: Vec<SumClause>,
    pub group_indicate: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineClause {
    Absolute(u32),
    Plus(u32),
    NextPage,
    /// LINE n ON NEXT PAGE
    AbsoluteNextPage(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupType {
    ReportHeading,
    PageHeading,
    /// CONTROL HEADING data-name, or FINAL (None)
    ControlHeading(Option<String>),
    Detail,
    ControlFooting(Option<String>),
    PageFooting,
    ReportFooting,
}

#[derive(Debug, Clone)]
pub struct SumClause {
    pub items: Vec<DataRef>,
    pub upon: Vec<String>,
    /// RESET ON data-name, or FINAL (Some(None)).
    pub reset: Option<Option<String>>,
}

#[derive(Debug, Clone, Default)]
pub struct Procedure {
    pub pos: Pos,
    /// USING parameters (BY REFERENCE).
    pub using: Vec<DataRef>,
    pub giving: Option<DataRef>,
    pub declaratives: Vec<Section>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Default)]
pub struct Section {
    /// None for the implicit section of a procedure division without
    /// sections.
    pub name: Option<String>,
    pub pos: Pos,
    pub segment: Option<u32>,
    /// USE statement of a declarative section.
    pub use_stmt: Option<UseStmt>,
    pub paragraphs: Vec<Paragraph>,
}

#[derive(Debug, Clone, Default)]
pub struct Paragraph {
    /// None for statements between a section header and its first
    /// paragraph.
    pub name: Option<String>,
    pub pos: Pos,
    pub sentences: Vec<Vec<Stmt>>,
}

#[derive(Debug, Clone)]
pub enum UseStmt {
    /// USE AFTER STANDARD EXCEPTION/ERROR PROCEDURE ON files | INPUT ...
    /// (target, GLOBAL)
    Error(UseTarget, bool),
    /// USE FOR DEBUGGING ON ...
    Debugging(Vec<DebugTarget>),
    BeforeReporting(String),
}

#[derive(Debug, Clone)]
pub enum DebugTarget {
    AllProcedures,
    /// A procedure-name, file-name or identifier; `all_refs` for
    /// ALL REFERENCES OF identifier.
    Name {
        name: DataRef,
        all_refs: bool,
    },
}

#[derive(Debug, Clone)]
pub enum UseTarget {
    Files(Vec<String>),
    Input,
    Output,
    InputOutput,
    Extend,
}

/// Reference to a data item, condition-name, index, file or procedure.
#[derive(Debug, Clone, PartialEq)]
pub struct DataRef {
    pub name: String,
    pub qualifiers: Vec<String>,
    pub subscripts: Vec<Subscript>,
    pub refmod: Option<(Box<Expr>, Option<Box<Expr>>)>,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Subscript {
    Expr(Expr),
    /// ALL (intrinsic-function arguments)
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Figurative {
    Zero,
    Space,
    HighValue,
    LowValue,
    Quote,
    /// Symbolic character.
    Symbolic(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    /// Numeric literal as written.
    Num(String),
    /// Alphanumeric literal (host characters; converted to EBCDIC later).
    Str(String),
    Hex(Vec<u8>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Ref(DataRef),
    Lit(Literal),
    Fig(Figurative),
    /// ALL literal / ALL figurative
    All(Box<Operand>),
    Func(Box<FuncCall>),
    /// LENGTH OF identifier
    LengthOf(DataRef),
    /// ADDRESS OF identifier
    AddressOf(DataRef),
    /// NULL / NULLS
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FuncCall {
    pub name: String,
    pub args: Vec<Expr>,
    pub refmod: Option<(Box<Expr>, Option<Box<Expr>>)>,
    pub pos: Pos,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Operand(Operand),
    Neg(Box<Expr>),
    Bin(BinOp, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl RelOp {
    pub fn negate(self) -> RelOp {
        match self {
            RelOp::Eq => RelOp::Ne,
            RelOp::Ne => RelOp::Eq,
            RelOp::Lt => RelOp::Ge,
            RelOp::Le => RelOp::Gt,
            RelOp::Gt => RelOp::Le,
            RelOp::Ge => RelOp::Lt,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ClassTest {
    Numeric,
    Alphabetic,
    AlphabeticLower,
    AlphabeticUpper,
    /// User-defined class name
    Class(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignTest {
    Positive,
    Negative,
    Zero,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cond {
    Rel(Expr, RelOp, Expr),
    Class(Expr, ClassTest),
    Sign(Expr, SignTest),
    /// Condition-name (level 88) or switch-status condition.
    Name(DataRef),
    Not(Box<Cond>),
    And(Box<Cond>, Box<Cond>),
    Or(Box<Cond>, Box<Cond>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProcRef {
    Name(String, Option<String>),
}

/// A procedure-name, optionally qualified by its section (`p OF s`).
#[derive(Debug, Clone, PartialEq)]
pub struct ProcName {
    pub name: String,
    pub section: Option<String>,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Receiver {
    pub target: DataRef,
    pub rounded: bool,
}

/// ON SIZE ERROR / AT END / INVALID KEY / ON OVERFLOW / ON EXCEPTION
/// handlers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Handlers {
    pub on: Option<Vec<Stmt>>,
    pub not_on: Option<Vec<Stmt>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PerformKind {
    Once,
    Times(Operand),
    Until {
        test_after: bool,
        cond: Cond,
    },
    Varying {
        test_after: bool,
        /// VARYING id FROM x BY y UNTIL cond, then AFTER ...
        loops: Vec<VaryingClause>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct VaryingClause {
    pub var: DataRef,
    pub from: Expr,
    pub by: Expr,
    pub until: Cond,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PerformBody {
    Procedures(ProcName, Option<ProcName>),
    Inline(Vec<Stmt>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    Input,
    Output,
    InputOutput,
    Extend,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Advancing {
    Lines(Operand),
    Mnemonic(String),
    Page,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WhenClause {
    /// One entry per subject; each a list of alternatives (ALSO).
    pub objects: Vec<EvalObject>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvalObject {
    Any,
    True,
    False,
    Cond(Cond),
    Value {
        not: bool,
        from: Expr,
        thru: Option<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum EvalSubject {
    True,
    False,
    Expr(Expr),
    Cond(Cond),
}

#[derive(Debug, Clone, PartialEq)]
pub enum InspectOp {
    Tallying(Vec<TallyClause>),
    Replacing(Vec<ReplaceClause>),
    Converting {
        from: Operand,
        to: Operand,
        delims: Vec<(bool, Operand)>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct TallyClause {
    pub counter: DataRef,
    pub items: Vec<TallyItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TallyItem {
    Characters(Vec<(bool, Operand)>),
    /// ALL / LEADING patterns: (leading, pattern, BEFORE/AFTER delimiters)
    Pattern(bool, Operand, Vec<(bool, Operand)>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ReplaceClause {
    Characters(Operand, Vec<(bool, Operand)>),
    /// ALL / LEADING / FIRST pattern BY replacement
    Pattern(ReplaceMode, Operand, Operand, Vec<(bool, Operand)>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceMode {
    All,
    Leading,
    First,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetStmt {
    /// SET targets TO value
    To(Vec<DataRef>, Operand),
    /// SET targets UP/DOWN BY value
    UpDown(Vec<DataRef>, bool, Expr),
    /// SET mnemonic TO ON/OFF
    Switch(Vec<(String, bool)>),
    /// SET condition-name TO TRUE
    CondTrue(Vec<DataRef>),
    /// SET ADDRESS OF linkage-items TO pointer value
    Address(Vec<DataRef>, Operand),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallArg {
    Reference(Operand),
    Content(Operand),
    Value(Operand),
    Omitted,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortIo {
    Using(Vec<String>),
    Giving(Vec<String>),
    Procedure(ProcName, Option<ProcName>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Accept {
        target: DataRef,
        from: Option<String>,
        pos: Pos,
    },
    Add {
        from: Vec<Expr>,
        to: Vec<Receiver>,
        giving: Vec<Receiver>,
        corresponding: bool,
        size_error: Handlers,
        pos: Pos,
    },
    Alter(Vec<(ProcName, ProcName)>),
    Call {
        program: Operand,
        using: Vec<CallArg>,
        returning: Option<DataRef>,
        exception: Handlers,
        pos: Pos,
    },
    Cancel(Vec<Operand>),
    Close(Vec<(String, Option<String>)>),
    Compute {
        targets: Vec<Receiver>,
        expr: Expr,
        size_error: Handlers,
        pos: Pos,
    },
    Continue,
    Delete {
        file: String,
        invalid_key: Handlers,
    },
    Display {
        items: Vec<Operand>,
        upon: Option<String>,
        no_advancing: bool,
    },
    Divide {
        /// DIVIDE a INTO b / DIVIDE a BY b
        divisor: Expr,
        dividend: Option<Expr>,
        into: Vec<Receiver>,
        giving: Vec<Receiver>,
        remainder: Option<DataRef>,
        size_error: Handlers,
        pos: Pos,
    },
    Evaluate {
        subjects: Vec<EvalSubject>,
        whens: Vec<(Vec<WhenClause>, Vec<Stmt>)>,
        other: Option<Vec<Stmt>>,
    },
    ExitProgram,
    ExitParagraph,
    ExitSection,
    Exit,
    GoBack,
    GoTo {
        targets: Vec<ProcName>,
        depending: Option<DataRef>,
        pos: Pos,
    },
    If {
        cond: Cond,
        then: Vec<Stmt>,
        els: Option<Vec<Stmt>>,
    },
    Initialize {
        targets: Vec<DataRef>,
        replacing: Vec<(String, Operand)>,
    },
    Inspect {
        target: DataRef,
        ops: Vec<InspectOp>,
    },
    Merge {
        file: String,
        keys: Vec<(bool, Vec<DataRef>)>,
        collating: Option<String>,
        using: Vec<String>,
        output: SortIo,
    },
    NextSentence,
    Move {
        from: Operand,
        to: Vec<DataRef>,
        corresponding: bool,
        pos: Pos,
    },
    Multiply {
        by: Expr,
        operand: Expr,
        into: Vec<Receiver>,
        giving: Vec<Receiver>,
        size_error: Handlers,
        pos: Pos,
    },
    Open(Vec<(OpenMode, Vec<String>)>),
    Perform {
        body: PerformBody,
        kind: PerformKind,
        pos: Pos,
    },
    Read {
        file: String,
        next: bool,
        into: Option<DataRef>,
        key: Option<DataRef>,
        at_end: Handlers,
        invalid_key: Handlers,
    },
    Release {
        record: DataRef,
        from: Option<Operand>,
    },
    Return {
        file: String,
        into: Option<DataRef>,
        at_end: Handlers,
    },
    Rewrite {
        record: DataRef,
        from: Option<Operand>,
        invalid_key: Handlers,
    },
    Search {
        table: DataRef,
        all: bool,
        varying: Option<DataRef>,
        at_end: Option<Vec<Stmt>>,
        whens: Vec<(Cond, Vec<Stmt>)>,
    },
    Set(SetStmt),
    Sort {
        file: String,
        keys: Vec<(bool, Vec<DataRef>)>,
        duplicates: bool,
        collating: Option<String>,
        input: SortIo,
        output: SortIo,
    },
    Start {
        file: String,
        key: Option<(RelOp, DataRef)>,
        invalid_key: Handlers,
    },
    StopRun,
    StopLiteral(Operand),
    String {
        sources: Vec<(Operand, Option<Operand>)>,
        into: DataRef,
        pointer: Option<DataRef>,
        overflow: Handlers,
    },
    Subtract {
        from: Vec<Expr>,
        subtrahend_of: Vec<Receiver>,
        giving: Vec<Receiver>,
        corresponding: bool,
        size_error: Handlers,
        pos: Pos,
    },
    Unstring {
        source: DataRef,
        delimiters: Vec<(bool, Operand)>,
        into: Vec<(DataRef, Option<DataRef>, Option<DataRef>)>,
        pointer: Option<DataRef>,
        tallying: Option<DataRef>,
        overflow: Handlers,
    },
    Write {
        record: DataRef,
        from: Option<Operand>,
        /// (before, advancing)
        advancing: Option<(bool, Advancing)>,
        end_of_page: Handlers,
        invalid_key: Handlers,
    },
    /// Report Writer
    Initiate(Vec<String>),
    Generate(DataRef),
    Terminate(Vec<String>),
    SuppressPrinting,
    /// Statement the code generator cannot handle yet.
    Unsupported(String, Pos),
    Exec(ExecBlock),
    /// ENTRY literal [USING ...]: an alternate entry point.
    Entry {
        name: String,
        using: Vec<DataRef>,
        pos: Pos,
    },
}

//! Syntax tree of PL/I source.

use crate::lexer::{Pos, StrKind};

/// A reference: `[locator ->] name[(args)][.name[(args)]]...`. Function
/// references, array elements and structure qualification all have this
/// form; semantic analysis tells them apart.
#[derive(Debug, Clone, PartialEq)]
pub struct Ref {
    pub locator: Option<Box<Ref>>,
    /// Qualified name parts, each with its argument / subscript lists
    /// (`F(1)(2)` has two lists).
    pub parts: Vec<(String, Vec<Vec<Expr>>)>,
    pub pos: Pos,
}

impl Ref {
    pub fn simple(name: &str, pos: Pos) -> Ref {
        Ref {
            locator: None,
            parts: vec![(name.to_string(), Vec::new())],
            pos,
        }
    }

    /// The last name of the reference.
    pub fn name(&self) -> &str {
        self.parts.last().map(|p| p.0.as_str()).unwrap_or("")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Decimal or binary numeric constant as written.
    Num {
        text: String,
        binary: bool,
        pos: Pos,
    },
    Str {
        text: String,
        kind: StrKind,
        pos: Pos,
    },
    Ref(Ref),
    /// Prefix + - ¬
    Unary(&'static str, Box<Expr>, Pos),
    /// Infix operators: ** * / + - || < > = ¬= <= >= ¬< ¬> & |
    Binary(&'static str, Box<Expr>, Box<Expr>, Pos),
    /// `*` as an argument (cross sections, extents).
    Star(Pos),
}

impl Expr {
    pub fn pos(&self) -> Pos {
        match self {
            Expr::Num { pos, .. } | Expr::Str { pos, .. } | Expr::Star(pos) => *pos,
            Expr::Ref(r) => r.pos,
            Expr::Unary(_, _, p) | Expr::Binary(_, _, _, p) => *p,
        }
    }
}

/// An extent or bound: an expression or `*`.
#[derive(Debug, Clone, PartialEq)]
pub enum Extent {
    Expr(Expr),
    Star,
}

/// Dimension bound pair `[lower:]upper`.
#[derive(Debug, Clone, PartialEq)]
pub struct Bound {
    pub lower: Option<Extent>,
    pub upper: Extent,
}

/// Data and other attributes of a declaration.
#[derive(Debug, Clone, PartialEq)]
pub enum Attr {
    Fixed,
    Float,
    Binary,
    Decimal,
    Real,
    Complex,
    /// (p[,q]) after FIXED / FLOAT / BINARY / DECIMAL
    Precision(Expr, Option<Expr>),
    Signed,
    Unsigned,
    Character(Option<Extent>),
    Bit(Option<Extent>),
    Graphic(Option<Extent>),
    Widechar(Option<Extent>),
    Varying,
    VaryingZ,
    Nonvarying,
    Picture(String),
    Pointer,
    Offset,
    Handle,
    Area(Option<Extent>),
    Label,
    Format,
    Entry(Vec<Vec<Attr>>),
    Returns(Vec<Attr>),
    File,
    Stream,
    Record,
    Input,
    Output,
    Update,
    Sequential,
    Direct,
    Keyed,
    Print,
    Buffered,
    Unbuffered,
    Environment(String),
    Static,
    Automatic,
    Controlled,
    Based(Option<Expr>),
    Defined(Ref),
    Position(Expr),
    Initial(Vec<Init>),
    InitialCall(Ref),
    External(Option<String>),
    Internal,
    Aligned,
    Unaligned,
    Dimension(Vec<Bound>),
    Like(Ref),
    Builtin,
    Condition,
    Value(Expr),
    Options(Vec<String>),
    Parameter,
    Generic,
    Abnormal,
    Normal,
    Assignable,
    NonAssignable,
    Connected,
    Byaddr,
    Byvalue,
    Optional,
    Irreducible,
    Reducible,
    Type(String),
}

/// INITIAL item: a value, `*` (no value), or an iteration factor.
#[derive(Debug, Clone, PartialEq)]
pub enum Init {
    Value(Expr),
    Star,
    Repeat(Extent, Vec<Init>),
}

/// One declared name (after factoring).
#[derive(Debug, Clone, PartialEq)]
pub struct DeclItem {
    pub level: u32,
    pub name: String,
    pub dims: Vec<Bound>,
    pub attrs: Vec<Attr>,
    pub pos: Pos,
}

/// Condition names of ON / SIGNAL / REVERT and condition prefixes.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CondName {
    pub name: String,
    /// The file or condition name argument: ENDFILE(F), CONDITION(C).
    pub arg: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DoSpec {
    /// `DO;`
    Group,
    /// `DO WHILE(e) UNTIL(e);` without a control variable
    Loop {
        while_: Option<Expr>,
        until: Option<Expr>,
    },
    /// `DO FOREVER;` / `DO LOOP;`
    Forever,
    /// `DO var = spec, spec ...;`
    Iterative { var: Ref, specs: Vec<DoItem> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DoItem {
    pub start: Expr,
    pub to: Option<Expr>,
    pub by: Option<Expr>,
    pub repeat: Option<Expr>,
    pub while_: Option<Expr>,
    pub until: Option<Expr>,
}

/// Data list items of GET / PUT: an expression or a repetitive
/// specification `(item, ... DO var = ...)`.
#[derive(Debug, Clone, PartialEq)]
pub enum DataItem {
    Expr(Expr),
    Repeat(Vec<DataItem>, Ref, Vec<DoItem>),
}

/// Edit-directed format items.
#[derive(Debug, Clone, PartialEq)]
pub enum Format {
    /// A, A(w)
    A(Option<Expr>),
    /// B, B(w)
    B(Option<Expr>),
    /// F(w[,d[,s]])
    F(Expr, Option<Expr>, Option<Expr>),
    /// E(w[,d[,s]])
    E(Expr, Option<Expr>, Option<Expr>),
    P(String),
    X(Expr),
    Skip(Option<Expr>),
    Page,
    Line(Expr),
    Column(Expr),
    /// R(label): the formats of a FORMAT statement
    R(Ref),
    /// n (formats): iteration
    Repeat(Expr, Vec<Format>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Transmit {
    List(Vec<DataItem>),
    /// pairs of data lists and format lists
    Edit(Vec<(Vec<DataItem>, Vec<Format>)>),
    Data(Vec<DataItem>),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct StreamOpts {
    pub file: Option<Expr>,
    pub string: Option<Expr>,
    pub skip: Option<Option<Expr>>,
    pub page: bool,
    pub line: Option<Expr>,
    pub copy: Option<Option<Expr>>,
    pub data: Option<Transmit>,
}

/// Options of the record I/O statements.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RecordOpts {
    pub file: Option<Expr>,
    pub into: Option<Expr>,
    pub from: Option<Expr>,
    pub set: Option<Expr>,
    pub key: Option<Expr>,
    pub keyto: Option<Expr>,
    pub keyfrom: Option<Expr>,
    pub ignore: Option<Expr>,
}

/// OPEN / CLOSE file specification.
#[derive(Debug, Clone, PartialEq)]
pub struct FileOpen {
    pub file: Expr,
    pub attrs: Vec<Attr>,
    pub title: Option<Expr>,
    pub linesize: Option<Expr>,
    pub pagesize: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Procedure {
    pub names: Vec<String>,
    pub params: Vec<String>,
    pub options: Vec<String>,
    pub returns: Option<Vec<Attr>>,
    pub recursive: bool,
    pub body: Vec<Stmt>,
    pub pos: Pos,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    Procedure(Procedure),
    Begin(Vec<Stmt>),
    /// ENTRY statement (secondary entry point) with its parameters.
    Entry(Vec<String>, Option<Vec<Attr>>),
    Declare(Vec<DeclItem>),
    Assign {
        targets: Vec<Expr>,
        op: Option<&'static str>,
        value: Expr,
        by_name: bool,
    },
    If {
        cond: Expr,
        then: Box<Stmt>,
        els: Option<Box<Stmt>>,
    },
    Do {
        spec: DoSpec,
        body: Vec<Stmt>,
    },
    Select {
        subject: Option<Expr>,
        whens: Vec<(Vec<Expr>, Stmt)>,
        other: Option<Box<Stmt>>,
    },
    Call(Ref),
    Return(Option<Expr>),
    GoTo(Expr),
    Leave(Option<String>),
    Iterate(Option<String>),
    On {
        conds: Vec<CondName>,
        snap: bool,
        /// None: SYSTEM
        unit: Option<Box<Stmt>>,
    },
    Signal(CondName),
    Revert(Vec<CondName>),
    Resignal,
    Stop,
    Exit,
    Put(StreamOpts),
    Get(StreamOpts),
    Open(Vec<FileOpen>),
    Close(Vec<Expr>),
    Read(RecordOpts),
    Write(RecordOpts),
    Rewrite(RecordOpts),
    Delete(RecordOpts),
    Locate(Ref, RecordOpts),
    Allocate(Vec<(Ref, Option<Expr>)>),
    Free(Vec<Ref>),
    Display {
        value: Expr,
        reply: Option<Expr>,
    },
    Delay(Expr),
    Format(Vec<Format>),
    Null,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub labels: Vec<String>,
    /// Condition prefixes: (SIZE, NOFOFL):
    pub prefixes: Vec<(String, bool)>,
    pub kind: StmtKind,
    pub pos: Pos,
}

//! Parser: tokens to statements.
//!
//! PL/I keywords are not reserved: a statement is an assignment when it
//! starts with a reference followed by `=` (or a compound assignment
//! operator, or `,` for multiple targets) and the expression ends the
//! statement; otherwise its first word is the statement keyword.

use crate::ast::*;
use crate::lexer::{Diag, Pos, Result, StrKind, Tok, Token};

/// The SQLCA declaration (EXEC SQL INCLUDE SQLCA): 136 bytes.
const SQLCA: &str = "DCL 1 SQLCA, 2 SQLCAID CHAR(8), 2 SQLCABC FIXED BIN(31), \
2 SQLCODE FIXED BIN(31), 2 SQLERRM CHAR(70) VARYING, 2 SQLERRP CHAR(8), \
2 SQLERRD(6) FIXED BIN(31), 2 SQLWARN, 3 SQLWARN0 CHAR(1), 3 SQLWARN1 CHAR(1), \
3 SQLWARN2 CHAR(1), 3 SQLWARN3 CHAR(1), 3 SQLWARN4 CHAR(1), 3 SQLWARN5 CHAR(1), \
3 SQLWARN6 CHAR(1), 3 SQLWARN7 CHAR(1), 2 SQLEXT, 3 SQLWARN8 CHAR(1), \
3 SQLWARN9 CHAR(1), 3 SQLWARNA CHAR(1), 3 SQLSTATE CHAR(5);";

/// The SQLDA declaration (EXEC SQL INCLUDE SQLDA): based on SQLDAPTR,
/// SQLN entries of 44 bytes (allocate it with SQLSIZE set).
const SQLDA: &str = "DCL 1 SQLDA BASED(SQLDAPTR), 2 SQLDAID CHAR(8), \
2 SQLDABC FIXED BIN(31), 2 SQLN FIXED BIN(15), 2 SQLD FIXED BIN(15), \
2 SQLVAR(SQLSIZE REFER(SQLN)), 3 SQLTYPE FIXED BIN(15), 3 SQLLEN FIXED BIN(15), \
3 SQLDATA POINTER, 3 SQLIND POINTER, 3 SQLNAME CHAR(30) VARYING, \
SQLSIZE FIXED BIN(15), SQLDAPTR POINTER;";

pub struct Parser {
    toks: Vec<Token>,
    i: usize,
    /// Unnamed structure members (`*`) so far.
    anon: u32,
    /// Attribute lists end at VALUE (in a DEFAULT statement).
    stop_at_value: bool,
}

const COMPOUND: &[&str] = &["+=", "-=", "*=", "/=", "|=", "&=", "||=", "**="];

impl Parser {
    pub fn new(toks: Vec<Token>) -> Parser {
        Parser {
            toks,
            i: 0,
            anon: 0,
            stop_at_value: false,
        }
    }

    /// Whether all tokens have been consumed.
    pub fn at_end(&self) -> bool {
        matches!(self.tok(), Tok::Eof)
    }

    fn tok(&self) -> &Tok {
        &self.toks[self.i.min(self.toks.len() - 1)].tok
    }

    fn tok_at(&self, k: usize) -> &Tok {
        &self.toks[(self.i + k).min(self.toks.len() - 1)].tok
    }

    fn pos(&self) -> Pos {
        self.toks[self.i.min(self.toks.len() - 1)].pos
    }

    fn bump(&mut self) -> Tok {
        let t = self.tok().clone();
        if self.i < self.toks.len() - 1 {
            self.i += 1;
        }
        t
    }

    fn is_op(&self, op: &str) -> bool {
        matches!(self.tok(), Tok::Op(o) if *o == op)
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.tok(), Tok::Ident(s) if s == w)
    }

    fn is_word_at(&self, k: usize, w: &str) -> bool {
        matches!(self.tok_at(k), Tok::Ident(s) if s == w)
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if self.is_op(op) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_word(&mut self, w: &str) -> bool {
        if self.is_word(w) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_op(&mut self, op: &str) -> Result<()> {
        if self.eat_op(op) {
            Ok(())
        } else {
            Err(self.unexpected(&format!("'{op}'")))
        }
    }

    fn unexpected(&self, what: &str) -> Diag {
        Diag::error(self.pos(), format!("{what} expected, found {}", self.tok()))
    }

    fn ident(&mut self) -> Result<String> {
        match self.tok() {
            Tok::Ident(s) => {
                let s = s.clone();
                self.bump();
                Ok(s)
            }
            _ => Err(self.unexpected("name")),
        }
    }

    // ---- expressions -------------------------------------------------

    pub fn expr(&mut self) -> Result<Expr> {
        self.or_expr()
    }

    fn or_expr(&mut self) -> Result<Expr> {
        let mut e = self.and_expr()?;
        loop {
            let pos = self.pos();
            if self.eat_op("|") {
                let r = self.and_expr()?;
                e = Expr::Binary("|", Box::new(e), Box::new(r), pos);
            } else if self.is_op("¬") && !matches!(self.tok_at(1), Tok::Op("=" | "<" | ">")) {
                // infix ¬: exclusive or
                self.bump();
                let r = self.and_expr()?;
                e = Expr::Binary("¬", Box::new(e), Box::new(r), pos);
            } else {
                return Ok(e);
            }
        }
    }

    fn and_expr(&mut self) -> Result<Expr> {
        let mut e = self.cmp_expr()?;
        loop {
            let pos = self.pos();
            if self.eat_op("&") {
                let r = self.cmp_expr()?;
                e = Expr::Binary("&", Box::new(e), Box::new(r), pos);
            } else {
                return Ok(e);
            }
        }
    }

    fn cmp_expr(&mut self) -> Result<Expr> {
        let mut e = self.cat_expr()?;
        loop {
            let pos = self.pos();
            let op = match self.tok() {
                Tok::Op(o @ ("=" | "¬=" | "<" | ">" | "<=" | ">=" | "¬<" | "¬>")) => *o,
                _ => return Ok(e),
            };
            self.bump();
            let r = self.cat_expr()?;
            e = Expr::Binary(op, Box::new(e), Box::new(r), pos);
        }
    }

    fn cat_expr(&mut self) -> Result<Expr> {
        let mut e = self.add_expr()?;
        loop {
            let pos = self.pos();
            if self.eat_op("||") {
                let r = self.add_expr()?;
                e = Expr::Binary("||", Box::new(e), Box::new(r), pos);
            } else {
                return Ok(e);
            }
        }
    }

    fn add_expr(&mut self) -> Result<Expr> {
        let mut e = self.mul_expr()?;
        loop {
            let pos = self.pos();
            let op = match self.tok() {
                Tok::Op(o @ ("+" | "-")) => *o,
                _ => return Ok(e),
            };
            self.bump();
            let r = self.mul_expr()?;
            e = Expr::Binary(op, Box::new(e), Box::new(r), pos);
        }
    }

    fn mul_expr(&mut self) -> Result<Expr> {
        let mut e = self.unary()?;
        loop {
            let pos = self.pos();
            let op = match self.tok() {
                Tok::Op(o @ ("*" | "/")) => *o,
                _ => return Ok(e),
            };
            self.bump();
            let r = self.unary()?;
            e = Expr::Binary(op, Box::new(e), Box::new(r), pos);
        }
    }

    /// Priority 1: prefix operators and **, right to left.
    fn unary(&mut self) -> Result<Expr> {
        let pos = self.pos();
        for op in ["+", "-", "¬"] {
            if self.eat_op(op) {
                let e = self.unary()?;
                return Ok(Expr::Unary(op, Box::new(e), pos));
            }
        }
        let base = self.primary()?;
        if self.is_op("**") {
            let pos = self.pos();
            self.bump();
            let exp = self.unary()?;
            return Ok(Expr::Binary("**", Box::new(base), Box::new(exp), pos));
        }
        Ok(base)
    }

    fn primary(&mut self) -> Result<Expr> {
        let pos = self.pos();
        match self.tok().clone() {
            Tok::Num { text, binary } => {
                self.bump();
                // iSUB
                if !binary
                    && matches!(self.tok(), Tok::Ident(s) if s == "SUB")
                    && let Ok(i) = text.parse::<u8>()
                {
                    self.bump();
                    return Ok(Expr::ISub(i, pos));
                }
                Ok(Expr::Num { text, binary, pos })
            }
            Tok::Str { text, kind } => {
                self.bump();
                Ok(Expr::Str { text, kind, pos })
            }
            // :type: in NEW(:t:) and other type functions
            Tok::Op(":")
                if matches!(self.tok_at(1), Tok::Ident(_))
                    && matches!(self.tok_at(2), Tok::Op(":")) =>
            {
                self.bump();
                let t = self.ident()?;
                self.bump();
                Ok(Expr::Ref(Ref::simple(&format!(":{t}:"), pos)))
            }
            Tok::Op("(") => {
                self.bump();
                let e = self.expr()?;
                self.expect_op(")")?;
                // (n)'string': a repeated string constant
                if let (
                    Expr::Num {
                        text: n,
                        binary: false,
                        ..
                    },
                    Tok::Str { text, kind },
                ) = (&e, self.tok().clone())
                    && let Ok(n) = n.parse::<usize>()
                {
                    self.bump();
                    let text = match kind {
                        StrKind::Char | StrKind::Bit(_) | StrKind::Hex => text.repeat(n),
                        _ => text,
                    };
                    return Ok(Expr::Str { text, kind, pos });
                }
                Ok(e)
            }
            Tok::Ident(_) => Ok(Expr::Ref(self.reference()?)),
            Tok::Op("*") => {
                self.bump();
                Ok(Expr::Star(pos))
            }
            _ => Err(self.unexpected("expression")),
        }
    }

    /// Argument / subscript list after '(' (consumed by the caller).
    fn args(&mut self) -> Result<Vec<Expr>> {
        let mut out = Vec::new();
        if self.eat_op(")") {
            return Ok(out);
        }
        loop {
            if self.is_op("*") && matches!(self.tok_at(1), Tok::Op("," | ")")) {
                let pos = self.pos();
                self.bump();
                out.push(Expr::Star(pos));
            } else {
                out.push(self.expr()?);
            }
            if self.eat_op(",") {
                continue;
            }
            self.expect_op(")")?;
            return Ok(out);
        }
    }

    fn basic_ref(&mut self) -> Result<Ref> {
        let pos = self.pos();
        let mut parts = Vec::new();
        loop {
            let name = self.ident()?;
            let mut lists = Vec::new();
            while self.is_op("(") {
                self.bump();
                lists.push(self.args()?);
            }
            parts.push((name, lists));
            if self.is_op(".") && matches!(self.tok_at(1), Tok::Ident(_)) {
                self.bump();
                continue;
            }
            break;
        }
        Ok(Ref {
            locator: None,
            parts,
            pos,
        })
    }

    /// A reference with locator qualifiers: `p->q->x.y(1)`.
    pub fn reference(&mut self) -> Result<Ref> {
        let mut r = self.basic_ref()?;
        while self.is_op("->") || self.is_op("=>") {
            self.bump();
            let mut next = self.basic_ref()?;
            next.locator = Some(Box::new(r));
            r = next;
        }
        Ok(r)
    }

    // ---- declarations --------------------------------------------------

    fn extent(&mut self) -> Result<Extent> {
        if self.is_op("*") && matches!(self.tok_at(1), Tok::Op(")" | "," | ":")) {
            self.bump();
            return Ok(Extent::Star);
        }
        let e = self.expr()?;
        if self.eat_word("REFER") {
            self.expect_op("(")?;
            let r = self.basic_ref()?;
            self.expect_op(")")?;
            return Ok(Extent::Refer(e, r));
        }
        Ok(Extent::Expr(e))
    }

    /// Bound pairs after '(' (consumed).
    fn bounds(&mut self) -> Result<Vec<Bound>> {
        let mut out = Vec::new();
        loop {
            let a = self.extent()?;
            let b = if self.eat_op(":") {
                Some(self.extent()?)
            } else {
                None
            };
            out.push(match b {
                Some(u) => Bound {
                    lower: Some(a),
                    upper: u,
                },
                None => Bound {
                    lower: None,
                    upper: a,
                },
            });
            if self.eat_op(",") {
                continue;
            }
            self.expect_op(")")?;
            return Ok(out);
        }
    }

    fn opt_extent(&mut self) -> Result<Option<Extent>> {
        if self.eat_op("(") {
            let e = self.extent()?;
            self.expect_op(")")?;
            Ok(Some(e))
        } else {
            Ok(None)
        }
    }

    fn paren_text(&mut self) -> Result<String> {
        // the tokens of a parenthesised list as text (OPTIONS, ENV)
        self.expect_op("(")?;
        let mut depth = 1;
        let mut out = String::new();
        loop {
            match self.bump() {
                Tok::Op("(") => {
                    depth += 1;
                    out.push('(');
                }
                Tok::Op(")") => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(out);
                    }
                    out.push(')');
                }
                Tok::Eof => return Err(self.unexpected("')'")),
                t => {
                    if !out.is_empty() && !out.ends_with('(') {
                        out.push(' ');
                    }
                    out.push_str(&t.to_string());
                }
            }
        }
    }

    fn word_list(&mut self) -> Result<Vec<String>> {
        Ok(self
            .paren_text()?
            .split([' ', ','])
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect())
    }

    fn init_items(&mut self) -> Result<Vec<Init>> {
        // after '(' (consumed)
        let mut out = Vec::new();
        if self.eat_op(")") {
            return Ok(out);
        }
        loop {
            out.push(self.init_item()?);
            if self.eat_op(",") {
                continue;
            }
            self.expect_op(")")?;
            return Ok(out);
        }
    }

    fn init_item(&mut self) -> Result<Init> {
        if self.is_op("*") && matches!(self.tok_at(1), Tok::Op("," | ")")) {
            self.bump();
            return Ok(Init::Star);
        }
        if self.is_op("(") {
            // iteration factor (n) followed by an item or a list
            let save = self.i;
            self.bump();
            let factor = self.extent()?;
            if self.eat_op(")") {
                let starts_item = match self.tok() {
                    Tok::Num { .. } | Tok::Str { .. } | Tok::Ident(_) => true,
                    Tok::Op(o) => matches!(*o, "(" | "*" | "+" | "-"),
                    Tok::Eof => false,
                };
                if starts_item {
                    let items = if self.is_op("(") {
                        // (n)(list) or (n)(expr)
                        self.bump();
                        self.init_items()?
                    } else {
                        vec![self.init_item()?]
                    };
                    return Ok(Init::Repeat(factor, items));
                }
            }
            self.i = save;
        }
        Ok(Init::Value(self.expr()?))
    }

    fn precision(&mut self, attrs: &mut Vec<Attr>) -> Result<()> {
        if self.is_op("(") {
            self.bump();
            let p = self.expr()?;
            let q = if self.eat_op(",") {
                Some(self.expr()?)
            } else {
                None
            };
            self.expect_op(")")?;
            attrs.push(Attr::Precision(p, q));
        }
        Ok(())
    }

    /// Attributes up to ',' / ';' / ')' at this level.
    pub fn attributes(&mut self) -> Result<Vec<Attr>> {
        let mut attrs = Vec::new();
        loop {
            if self.stop_at_value && self.is_word("VALUE") {
                return Ok(attrs);
            }
            let Tok::Ident(w) = self.tok().clone() else {
                // (p,q) alone after a base
                if self.is_op("(") && !attrs.is_empty() {
                    self.precision(&mut attrs)?;
                    continue;
                }
                return Ok(attrs);
            };
            self.bump();
            let a = match w.as_str() {
                "FIXED" | "FLOAT" | "BIN" | "BINARY" | "DEC" | "DECIMAL" => {
                    attrs.push(match w.as_str() {
                        "FIXED" => Attr::Fixed,
                        "FLOAT" => Attr::Float,
                        "BIN" | "BINARY" => Attr::Binary,
                        _ => Attr::Decimal,
                    });
                    self.precision(&mut attrs)?;
                    continue;
                }
                "PRECISION" | "PREC" => {
                    self.precision(&mut attrs)?;
                    continue;
                }
                "REAL" => Attr::Real,
                "COMPLEX" | "CPLX" => Attr::Complex,
                "SIGNED" | "SIGN" => Attr::Signed,
                "UNSIGNED" | "UNSIGN" => Attr::Unsigned,
                "CHARACTER" | "CHAR" => Attr::Character(self.opt_extent()?),
                "BIT" => Attr::Bit(self.opt_extent()?),
                "GRAPHIC" | "G" => Attr::Graphic(self.opt_extent()?),
                "WIDECHAR" | "WCHAR" => Attr::Widechar(self.opt_extent()?),
                "VARYING" | "VAR" => Attr::Varying,
                "VARYINGZ" | "VARZ" => Attr::VaryingZ,
                "NONVARYING" | "NONVAR" => Attr::Nonvarying,
                "PICTURE" | "PIC" => match self.bump() {
                    Tok::Str { text, .. } => Attr::Picture(text),
                    _ => return Err(self.unexpected("picture string")),
                },
                "POINTER" | "PTR" => Attr::Pointer,
                "OFFSET" => {
                    let area = if self.is_op("(") {
                        Some(self.paren_text()?.trim().to_ascii_uppercase())
                    } else {
                        None
                    };
                    Attr::Offset(area.filter(|a| !a.is_empty()))
                }
                "HANDLE" => {
                    // HANDLE name or HANDLE(name)
                    let t = if self.is_op("(") {
                        Some(self.paren_text()?)
                    } else if matches!(self.tok(), Tok::Ident(_)) {
                        Some(self.ident()?)
                    } else {
                        None
                    };
                    Attr::Handle(t.map(|t| t.trim().to_ascii_uppercase()))
                }
                "AREA" => Attr::Area(self.opt_extent()?),
                "LABEL" => {
                    if self.is_op("(") {
                        self.paren_text()?;
                    }
                    Attr::Label
                }
                "FORMAT" => Attr::Format,
                "ENTRY" => {
                    let mut params = Vec::new();
                    if self.eat_op("(") && !self.eat_op(")") {
                        loop {
                            // a descriptor: [level] attributes, or *
                            if matches!(self.tok(), Tok::Num { .. }) {
                                self.bump();
                            }
                            if self.is_op("*") {
                                self.bump();
                                params.push(Vec::new());
                            } else if self.eat_op("(") {
                                // an array: passed as it is (no
                                // conversion to a dummy)
                                self.bounds()?;
                                self.attributes()?;
                                params.push(Vec::new());
                            } else {
                                params.push(self.attributes()?);
                            }
                            if self.eat_op(",") {
                                continue;
                            }
                            self.expect_op(")")?;
                            break;
                        }
                    }
                    Attr::Entry(params)
                }
                "RETURNS" => {
                    self.expect_op("(")?;
                    let a = self.attributes()?;
                    self.expect_op(")")?;
                    Attr::Returns(a)
                }
                "FILE" => Attr::File,
                "STREAM" => Attr::Stream,
                "RECORD" => Attr::Record,
                "INPUT" => Attr::Input,
                "OUTPUT" => Attr::Output,
                "UPDATE" => Attr::Update,
                "SEQUENTIAL" | "SEQL" => Attr::Sequential,
                "DIRECT" => Attr::Direct,
                "KEYED" => Attr::Keyed,
                "PRINT" => Attr::Print,
                "BUFFERED" | "BUF" => Attr::Buffered,
                "UNBUFFERED" | "UNBUF" => Attr::Unbuffered,
                "ENVIRONMENT" | "ENV" => Attr::Environment(self.paren_text()?),
                "STATIC" => Attr::Static,
                "AUTOMATIC" | "AUTO" => Attr::Automatic,
                "CONTROLLED" | "CTL" => Attr::Controlled,
                "BASED" => {
                    if self.eat_op("(") {
                        let e = self.expr()?;
                        self.expect_op(")")?;
                        Attr::Based(Some(e))
                    } else {
                        Attr::Based(None)
                    }
                }
                "DEFINED" | "DEF" => {
                    if self.eat_op("(") {
                        let r = self.reference()?;
                        self.expect_op(")")?;
                        Attr::Defined(r)
                    } else {
                        Attr::Defined(self.reference()?)
                    }
                }
                "POSITION" | "POS" => {
                    self.expect_op("(")?;
                    let e = self.expr()?;
                    self.expect_op(")")?;
                    Attr::Position(e)
                }
                "INITIAL" | "INIT" => {
                    if self.eat_word("CALL") {
                        Attr::InitialCall(self.reference()?)
                    } else if self.eat_word("TO") {
                        // INITIAL TO(area) (...): not supported beyond values
                        self.paren_text()?;
                        self.expect_op("(")?;
                        Attr::Initial(self.init_items()?)
                    } else {
                        self.expect_op("(")?;
                        Attr::Initial(self.init_items()?)
                    }
                }
                "EXTERNAL" | "EXT" => {
                    if self.is_op("(") {
                        self.bump();
                        let n = match self.bump() {
                            Tok::Str { text, .. } => text,
                            Tok::Ident(s) => s,
                            _ => return Err(self.unexpected("external name")),
                        };
                        self.expect_op(")")?;
                        Attr::External(Some(n))
                    } else {
                        Attr::External(None)
                    }
                }
                "INTERNAL" | "INT" => Attr::Internal,
                "ALIGNED" => Attr::Aligned,
                "UNALIGNED" | "UNAL" => Attr::Unaligned,
                "DIMENSION" | "DIM" => {
                    self.expect_op("(")?;
                    Attr::Dimension(self.bounds()?)
                }
                "LIKE" => Attr::Like(self.reference()?),
                "BUILTIN" => Attr::Builtin,
                "VARIABLE" => Attr::Variable,
                "CONDITION" | "COND" => Attr::Condition,
                "VALUE" => {
                    self.expect_op("(")?;
                    let e = self.expr()?;
                    self.expect_op(")")?;
                    Attr::Value(e)
                }
                "OPTIONS" => Attr::Options(self.word_list()?),
                "PARAMETER" | "PARM" => Attr::Parameter,
                "GENERIC" => Attr::Generic(self.generic_list()?),
                "ORDINAL" if matches!(self.tok(), Tok::Ident(_)) => Attr::Type(self.ident()?),
                "ABNORMAL" => Attr::Abnormal,
                "NORMAL" => Attr::Normal,
                "ASSIGNABLE" | "ASGN" => Attr::Assignable,
                "NONASSIGNABLE" | "NONASGN" => Attr::NonAssignable,
                "CONNECTED" | "CONN" | "NONCONNECTED" | "NONCONN" => Attr::Connected,
                "BYADDR" => Attr::Byaddr,
                "BYVALUE" => Attr::Byvalue,
                "OPTIONAL" => Attr::Optional,
                "IRREDUCIBLE" | "IRRED" => Attr::Irreducible,
                "REDUCIBLE" | "RED" => Attr::Reducible,
                "TYPE" => {
                    // TYPE name or TYPE(name)
                    let t = if self.is_op("(") {
                        self.paren_text()?
                    } else {
                        self.ident()?
                    };
                    Attr::Type(t.trim().to_ascii_uppercase())
                }
                "NOINIT" | "NOINITIAL" | "NATIVE" | "NONNATIVE" | "HEXADEC" | "IEEE"
                | "BIGENDIAN" | "LITTLEENDIAN" | "ASSEMBLER" | "LOCAL" | "RESERVED"
                | "CONSTANT" | "LIMITED" | "INONLY" | "INOUT" | "OUTONLY" => continue,
                _ => {
                    self.i -= 1;
                    return Ok(attrs);
                }
            };
            attrs.push(a);
        }
    }

    /// One declaration item (possibly a factored list) with the inherited
    /// attributes.
    fn decl_item(&mut self, out: &mut Vec<DeclItem>) -> Result<()> {
        let pos = self.pos();
        let level = match self.tok().clone() {
            Tok::Num {
                text,
                binary: false,
            } if text.chars().all(|c| c.is_ascii_digit()) => {
                self.bump();
                text.parse::<u32>().unwrap_or(1)
            }
            _ => 1,
        };
        if self.eat_op("(") {
            // factored: (a, b, ...) attributes
            let mut inner = Vec::new();
            loop {
                self.decl_item(&mut inner)?;
                if self.eat_op(",") {
                    continue;
                }
                self.expect_op(")")?;
                break;
            }
            let dims = if self.is_op("(") {
                self.bump();
                self.bounds()?
            } else {
                Vec::new()
            };
            let attrs = self.attributes()?;
            for mut it in inner {
                if it.level == 1 && level != 1 {
                    it.level = level;
                }
                if it.dims.is_empty() {
                    it.dims = dims.clone();
                }
                it.attrs.extend(attrs.iter().cloned());
                out.push(it);
            }
            return Ok(());
        }
        // `*`: an unnamed member (no reference reaches it)
        let name = if level > 1 && self.eat_op("*") {
            self.anon += 1;
            format!("*{}", self.anon)
        } else {
            self.ident()?
        };
        let mut dims = if self.is_op("(") {
            self.bump();
            self.bounds()?
        } else {
            Vec::new()
        };
        let mut attrs = self.attributes()?;
        attrs.retain(|a| match a {
            Attr::Dimension(b) => {
                dims = b.clone();
                false
            }
            _ => true,
        });
        out.push(DeclItem {
            level,
            name,
            dims,
            attrs,
            pos,
        });
        Ok(())
    }

    /// The alternatives of GENERIC: `(entry WHEN(desc, ...), ...,
    /// entry OTHERWISE)`.
    fn generic_list(&mut self) -> Result<Vec<crate::ast::GenericAlt>> {
        self.expect_op("(")?;
        let mut out = Vec::new();
        loop {
            let entry = self.ident()?;
            let when = if self.eat_word("OTHERWISE") {
                None
            } else {
                if !self.eat_word("WHEN") {
                    return Err(self.unexpected("WHEN or OTHERWISE"));
                }
                self.expect_op("(")?;
                let mut descs = Vec::new();
                let mut cur: Vec<String> = Vec::new();
                let mut depth = 0;
                loop {
                    match self.bump() {
                        Tok::Op("(") => depth += 1,
                        Tok::Op(")") if depth > 0 => depth -= 1,
                        Tok::Op(")") => {
                            if !cur.is_empty() || !descs.is_empty() {
                                descs.push(std::mem::take(&mut cur));
                            }
                            break;
                        }
                        Tok::Op(",") if depth == 0 => descs.push(std::mem::take(&mut cur)),
                        Tok::Op("*") if depth == 0 => cur.push("*".into()),
                        Tok::Ident(w) if depth == 0 => cur.push(w.to_ascii_uppercase()),
                        Tok::Eof => return Err(self.unexpected("')'")),
                        _ => {}
                    }
                }
                Some(descs)
            };
            out.push(crate::ast::GenericAlt { entry, when });
            if self.eat_op(",") {
                continue;
            }
            self.expect_op(")")?;
            return Ok(out);
        }
    }

    fn declare(&mut self) -> Result<Vec<DeclItem>> {
        let mut out = Vec::new();
        loop {
            self.decl_item(&mut out)?;
            if self.eat_op(",") {
                continue;
            }
            self.expect_op(";")?;
            return Ok(out);
        }
    }

    // ---- statements ----------------------------------------------------

    fn semi(&mut self) -> Result<()> {
        self.expect_op(";")
    }

    fn paren_expr(&mut self) -> Result<Expr> {
        self.expect_op("(")?;
        let e = self.expr()?;
        self.expect_op(")")?;
        Ok(e)
    }

    fn cond_name(&mut self) -> Result<CondName> {
        let name = self.ident()?;
        let name = match name.as_str() {
            "FOFL" => "FIXEDOVERFLOW".to_string(),
            "OFL" => "OVERFLOW".to_string(),
            "UFL" => "UNDERFLOW".to_string(),
            "ZDIV" => "ZERODIVIDE".to_string(),
            "CONV" => "CONVERSION".to_string(),
            "STRG" => "STRINGRANGE".to_string(),
            "STRZ" => "STRINGSIZE".to_string(),
            "SUBRG" => "SUBSCRIPTRANGE".to_string(),
            "UNDF" => "UNDEFINEDFILE".to_string(),
            "COND" => "CONDITION".to_string(),
            _ => name,
        };
        let arg = if self.eat_op("(") {
            let a = match self.bump() {
                Tok::Ident(s) => s,
                _ => return Err(self.unexpected("name")),
            };
            // ENDFILE(F(1)) etc.: subscripted file references are not
            // distinguished
            while !self.is_op(")") && !matches!(self.tok(), Tok::Eof) {
                self.bump();
            }
            self.expect_op(")")?;
            Some(a)
        } else {
            None
        };
        Ok(CondName { name, arg })
    }

    fn do_item(&mut self) -> Result<DoItem> {
        let start = self.expr()?;
        let mut it = DoItem {
            thru: None,
            start,
            to: None,
            by: None,
            repeat: None,
            while_: None,
            until: None,
        };
        loop {
            if self.eat_word("TO") {
                it.to = Some(self.expr()?);
            } else if self.is_word("UPTHRU") || self.is_word("DOWNTHRU") {
                let up = self.ident()? == "UPTHRU";
                it.to = Some(self.expr()?);
                it.thru = Some(up);
            } else if self.eat_word("BY") {
                it.by = Some(self.expr()?);
            } else if self.eat_word("REPEAT") {
                it.repeat = Some(self.expr()?);
            } else if self.is_word("WHILE") && self.is_op_at(1, "(") {
                self.bump();
                it.while_ = Some(self.paren_expr()?);
            } else if self.is_word("UNTIL") && self.is_op_at(1, "(") {
                self.bump();
                it.until = Some(self.paren_expr()?);
            } else {
                return Ok(it);
            }
        }
    }

    fn is_op_at(&self, k: usize, op: &str) -> bool {
        matches!(self.tok_at(k), Tok::Op(o) if *o == op)
    }

    pub fn do_spec(&mut self) -> Result<DoSpec> {
        if self.is_op(";") {
            return Ok(DoSpec::Group);
        }
        if (self.is_word("FOREVER") || self.is_word("LOOP")) && self.is_op_at(1, ";") {
            self.bump();
            return Ok(DoSpec::Forever);
        }
        if (self.is_word("WHILE") || self.is_word("UNTIL")) && self.is_op_at(1, "(") {
            let mut while_ = None;
            let mut until = None;
            loop {
                if self.eat_word("WHILE") {
                    while_ = Some(self.paren_expr()?);
                } else if self.eat_word("UNTIL") {
                    until = Some(self.paren_expr()?);
                } else {
                    break;
                }
            }
            return Ok(DoSpec::Loop { while_, until });
        }
        let var = self.reference()?;
        self.expect_op("=")?;
        let mut specs = vec![self.do_item()?];
        while self.eat_op(",") {
            specs.push(self.do_item()?);
        }
        Ok(DoSpec::Iterative { var, specs })
    }

    /// Statements up to the END of a group / block.
    fn body(&mut self) -> Result<Vec<Stmt>> {
        let mut out = Vec::new();
        loop {
            if matches!(self.tok(), Tok::Eof) {
                return Err(self.unexpected("END"));
            }
            // END [label];
            if self.is_word("END")
                && (self.is_op_at(1, ";")
                    || matches!(self.tok_at(1), Tok::Ident(_)) && self.is_op_at(2, ";"))
            {
                self.bump();
                if matches!(self.tok(), Tok::Ident(_)) {
                    self.bump();
                }
                self.semi()?;
                return Ok(out);
            }
            out.push(self.statement()?);
        }
    }

    fn data_items(&mut self) -> Result<Vec<DataItem>> {
        // after '(' (consumed)
        let mut out = Vec::new();
        if self.eat_op(")") {
            return Ok(out);
        }
        loop {
            out.push(self.data_item()?);
            if self.eat_op(",") {
                continue;
            }
            self.expect_op(")")?;
            return Ok(out);
        }
    }

    fn data_item(&mut self) -> Result<DataItem> {
        if self.is_op("(") {
            // repetitive specification: (items DO var = ...)
            let save = self.i;
            self.bump();
            let mut items = Vec::new();
            let ok = loop {
                match self.data_item() {
                    Ok(it) => items.push(it),
                    Err(_) => break false,
                }
                if self.eat_op(",") {
                    continue;
                }
                break self.is_word("DO");
            };
            if ok {
                self.bump();
                let var = self.reference()?;
                self.expect_op("=")?;
                let mut specs = vec![self.do_item()?];
                while self.eat_op(",") {
                    specs.push(self.do_item()?);
                }
                self.expect_op(")")?;
                return Ok(DataItem::Repeat(items, var, specs));
            }
            self.i = save;
        }
        Ok(DataItem::Expr(self.expr()?))
    }

    fn formats(&mut self) -> Result<Vec<Format>> {
        // after '(' (consumed)
        let mut out = Vec::new();
        loop {
            out.push(self.format()?);
            if self.eat_op(",") {
                continue;
            }
            self.expect_op(")")?;
            return Ok(out);
        }
    }

    fn format(&mut self) -> Result<Format> {
        // iteration factor: integer constant or (expr)
        let factor = match self.tok().clone() {
            Tok::Num {
                text,
                binary: false,
            } => {
                let pos = self.pos();
                self.bump();
                Some(Expr::Num {
                    text,
                    binary: false,
                    pos,
                })
            }
            Tok::Op("(") => {
                self.bump();
                let e = self.expr()?;
                self.expect_op(")")?;
                Some(e)
            }
            _ => None,
        };
        if let Some(n) = factor {
            let items = if self.eat_op("(") {
                self.formats()?
            } else {
                vec![self.format()?]
            };
            return Ok(Format::Repeat(n, items));
        }
        let w = self.ident()?;
        let opt = |p: &mut Parser| -> Result<Option<Expr>> {
            if p.is_op("(") {
                Ok(Some(p.paren_expr()?))
            } else {
                Ok(None)
            }
        };
        let three = |p: &mut Parser| -> Result<(Expr, Option<Expr>, Option<Expr>)> {
            p.expect_op("(")?;
            let w = p.expr()?;
            let d = if p.eat_op(",") { Some(p.expr()?) } else { None };
            let s = if p.eat_op(",") { Some(p.expr()?) } else { None };
            p.expect_op(")")?;
            Ok((w, d, s))
        };
        Ok(match w.as_str() {
            "A" => Format::A(opt(self)?),
            "B" | "B1" | "B2" | "B3" | "B4" => Format::B(opt(self)?),
            "F" => {
                let (w, d, s) = three(self)?;
                Format::F(w, d, s)
            }
            "E" => {
                let (w, d, s) = three(self)?;
                Format::E(w, d, s)
            }
            "P" => match self.bump() {
                Tok::Str { text, .. } => Format::P(text),
                _ => return Err(self.unexpected("picture string")),
            },
            "X" => Format::X(self.paren_expr()?),
            "SKIP" => Format::Skip(opt(self)?),
            "PAGE" => Format::Page,
            "LINE" => Format::Line(self.paren_expr()?),
            "COLUMN" | "COL" => Format::Column(self.paren_expr()?),
            "R" => {
                self.expect_op("(")?;
                let r = self.reference()?;
                self.expect_op(")")?;
                Format::R(r)
            }
            _ => {
                return Err(Diag::error(
                    self.pos(),
                    format!("format item {w} is not supported"),
                ));
            }
        })
    }

    fn stream_opts(&mut self, get: bool) -> Result<StreamOpts> {
        let mut o = StreamOpts::default();
        while let Tok::Ident(w) = self.tok().clone() {
            self.bump();
            match w.as_str() {
                "FILE" => o.file = Some(self.paren_expr()?),
                "STRING" => o.string = Some(self.paren_expr()?),
                "SKIP" => {
                    o.skip = Some(if self.is_op("(") {
                        Some(self.paren_expr()?)
                    } else {
                        None
                    })
                }
                "PAGE" => o.page = true,
                "LINE" => o.line = Some(self.paren_expr()?),
                "COPY" => {
                    o.copy = Some(if self.is_op("(") {
                        Some(self.paren_expr()?)
                    } else {
                        None
                    })
                }
                "LIST" => {
                    self.expect_op("(")?;
                    o.data = Some(Transmit::List(self.data_items()?));
                }
                "DATA" => {
                    let items = if self.eat_op("(") {
                        self.data_items()?
                    } else {
                        Vec::new()
                    };
                    o.data = Some(Transmit::Data(items));
                }
                "EDIT" => {
                    let mut pairs = Vec::new();
                    while self.is_op("(") {
                        self.bump();
                        let items = self.data_items()?;
                        self.expect_op("(")?;
                        let fmts = self.formats()?;
                        pairs.push((items, fmts));
                    }
                    o.data = Some(Transmit::Edit(pairs));
                }
                _ => {
                    return Err(Diag::error(
                        self.pos(),
                        format!(
                            "{} option {w} is not supported",
                            if get { "GET" } else { "PUT" }
                        ),
                    ));
                }
            }
        }
        self.semi()?;
        Ok(o)
    }

    fn record_opts(&mut self) -> Result<RecordOpts> {
        let mut o = RecordOpts::default();
        while let Tok::Ident(w) = self.tok().clone() {
            self.bump();
            let e = self.paren_expr()?;
            match w.as_str() {
                "FILE" => o.file = Some(e),
                "INTO" => o.into = Some(e),
                "FROM" => o.from = Some(e),
                "SET" => o.set = Some(e),
                "KEY" => o.key = Some(e),
                "KEYTO" => o.keyto = Some(e),
                "KEYFROM" => o.keyfrom = Some(e),
                "IGNORE" => o.ignore = Some(e),
                "EVENT" | "NOLOCK" => {}
                _ => {
                    return Err(Diag::error(
                        self.pos(),
                        format!("record option {w} is not supported"),
                    ));
                }
            }
        }
        self.semi()?;
        Ok(o)
    }

    fn open(&mut self) -> Result<Vec<FileOpen>> {
        let mut out = Vec::new();
        loop {
            let mut f = FileOpen {
                file: Expr::Star(self.pos()),
                attrs: Vec::new(),
                title: None,
                linesize: None,
                pagesize: None,
            };
            loop {
                if self.eat_word("FILE") {
                    f.file = self.paren_expr()?;
                } else if self.eat_word("TITLE") {
                    f.title = Some(self.paren_expr()?);
                } else if self.eat_word("LINESIZE") {
                    f.linesize = Some(self.paren_expr()?);
                } else if self.eat_word("PAGESIZE") {
                    f.pagesize = Some(self.paren_expr()?);
                } else if self.is_word("TAB") || self.is_word("ENVIRONMENT") || self.is_word("ENV")
                {
                    self.bump();
                    self.paren_text()?;
                } else if matches!(self.tok(), Tok::Ident(_)) {
                    let a = self.attributes()?;
                    if a.is_empty() {
                        return Err(self.unexpected("OPEN option"));
                    }
                    f.attrs.extend(a);
                } else {
                    break;
                }
            }
            out.push(f);
            if self.eat_op(",") {
                continue;
            }
            self.semi()?;
            return Ok(out);
        }
    }

    /// Tries an assignment statement; None (position restored) when the
    /// statement is not one.
    fn try_assignment(&mut self) -> Result<Option<StmtKind>> {
        let save = self.i;
        let mut targets = Vec::new();
        loop {
            let Ok(r) = self.reference() else {
                self.i = save;
                return Ok(None);
            };
            targets.push(Expr::Ref(r));
            if self.eat_op(",") {
                continue;
            }
            break;
        }
        let op = match self.tok() {
            Tok::Op("=") => None,
            Tok::Op(o) if COMPOUND.contains(o) => Some(&o[..o.len() - 1]),
            _ => {
                self.i = save;
                return Ok(None);
            }
        };
        let op: Option<&'static str> = op.map(|o| match o {
            "+" => "+",
            "-" => "-",
            "*" => "*",
            "/" => "/",
            "|" => "|",
            "&" => "&",
            "||" => "||",
            _ => "**",
        });
        self.bump();
        let Ok(value) = self.expr() else {
            self.i = save;
            return Ok(None);
        };
        let mut by_name = false;
        if self.is_op(",") && self.is_word_at(1, "BY") && self.is_word_at(2, "NAME") {
            self.i += 3;
            by_name = true;
        }
        if !self.is_op(";") {
            self.i = save;
            return Ok(None);
        }
        self.bump();
        Ok(Some(StmtKind::Assign {
            targets,
            op,
            value,
            by_name,
        }))
    }

    pub fn statement(&mut self) -> Result<Stmt> {
        let pos = self.pos();
        let mut prefixes = Vec::new();
        let mut labels = Vec::new();
        loop {
            // condition prefix: (name, ...):
            if self.is_op("(") {
                let save = self.i;
                self.bump();
                let mut list = Vec::new();
                let ok = loop {
                    match self.tok().clone() {
                        Tok::Ident(w) => {
                            self.bump();
                            if self.is_op("(") {
                                self.paren_text()?;
                            }
                            let (on, name) = match w.strip_prefix("NO") {
                                Some(n) if !n.is_empty() && w != "NOTE" => (false, n.to_string()),
                                _ => (true, w),
                            };
                            list.push((name, on));
                        }
                        _ => break false,
                    }
                    if self.eat_op(",") {
                        continue;
                    }
                    break self.eat_op(")") && self.eat_op(":");
                };
                if ok {
                    prefixes.extend(list);
                    continue;
                }
                self.i = save;
            }
            if let Tok::Ident(n) = self.tok().clone()
                && self.is_op_at(1, ":")
            {
                self.bump();
                self.bump();
                labels.push(n);
                continue;
            }
            break;
        }
        let kind = self.statement_kind(&labels)?;
        Ok(Stmt {
            labels,
            prefixes,
            kind,
            pos,
        })
    }

    fn statement_kind(&mut self, labels: &[String]) -> Result<StmtKind> {
        if self.eat_op(";") {
            return Ok(StmtKind::Null);
        }
        if let Some(a) = self.try_assignment()? {
            return Ok(a);
        }
        let pos = self.pos();
        let w = self.ident()?;
        Ok(match w.as_str() {
            "PROCEDURE" | "PROC" => {
                let mut params = Vec::new();
                if self.eat_op("(") && !self.eat_op(")") {
                    loop {
                        params.push(self.ident()?);
                        if self.eat_op(",") {
                            continue;
                        }
                        self.expect_op(")")?;
                        break;
                    }
                }
                let mut options = Vec::new();
                let mut returns = None;
                let mut recursive = false;
                #[allow(clippy::while_let_loop)]
                loop {
                    match self.tok().clone() {
                        Tok::Ident(o) => {
                            self.bump();
                            match o.as_str() {
                                "OPTIONS" => options.extend(self.word_list()?),
                                "RETURNS" => {
                                    self.expect_op("(")?;
                                    returns = Some(self.attributes()?);
                                    self.expect_op(")")?;
                                }
                                "RECURSIVE" => recursive = true,
                                "EXTERNAL" | "EXT" => {
                                    if self.is_op("(") {
                                        self.paren_text()?;
                                    }
                                }
                                "ORDER" | "REORDER" | "NONRECURSIVE" | "CHARGRAPHIC"
                                | "NOCHARGRAPHIC" => {}
                                _ => {
                                    return Err(Diag::error(
                                        self.pos(),
                                        format!("PROCEDURE option {o} is not supported"),
                                    ));
                                }
                            }
                        }
                        _ => break,
                    }
                }
                self.semi()?;
                let body = self.body()?;
                StmtKind::Procedure(Procedure {
                    names: labels.to_vec(),
                    params,
                    options,
                    returns,
                    recursive,
                    body,
                    pos,
                })
            }
            "PACKAGE" => {
                // the procedures of a package
                while !self.is_op(";") {
                    self.bump();
                }
                self.semi()?;
                StmtKind::Begin(self.body()?)
            }
            "BEGIN" => {
                while matches!(self.tok(), Tok::Ident(_)) {
                    self.bump();
                    if self.is_op("(") {
                        self.paren_text()?;
                    }
                }
                self.semi()?;
                StmtKind::Begin(self.body()?)
            }
            "ENTRY" => {
                let mut params = Vec::new();
                if self.eat_op("(") && !self.eat_op(")") {
                    loop {
                        params.push(self.ident()?);
                        if self.eat_op(",") {
                            continue;
                        }
                        self.expect_op(")")?;
                        break;
                    }
                }
                let mut returns = None;
                while let Tok::Ident(o) = self.tok().clone() {
                    self.bump();
                    match o.as_str() {
                        "RETURNS" => {
                            self.expect_op("(")?;
                            returns = Some(self.attributes()?);
                            self.expect_op(")")?;
                        }
                        _ => {
                            if self.is_op("(") {
                                self.paren_text()?;
                            }
                        }
                    }
                }
                self.semi()?;
                StmtKind::Entry(params, returns)
            }
            "DECLARE" | "DCL" => StmtKind::Declare(self.declare()?),
            // DEFINE ORDINAL name (member [VALUE(n)], ...) [PRECISION(p)]
            // [SIGNED | UNSIGNED]: the declaration of the type's name
            "DEFINE" if self.eat_word("STRUCTURE") => {
                let mut items = self.declare()?;
                match items.first_mut() {
                    Some(root) if root.level == 1 => root.attrs.push(Attr::StructDef),
                    _ => return Err(self.unexpected("a level-1 structure name")),
                }
                StmtKind::Declare(items)
            }
            "DEFINE" if self.eat_word("ALIAS") => {
                let name = self.ident()?;
                let attrs = self.attributes()?;
                self.semi()?;
                StmtKind::Declare(vec![DeclItem {
                    level: 1,
                    name,
                    dims: Vec::new(),
                    attrs: vec![Attr::AliasDef(attrs)],
                    pos,
                }])
            }
            "DEFINE" if self.eat_word("ORDINAL") => {
                let name = self.ident()?;
                self.expect_op("(")?;
                let mut members = Vec::new();
                loop {
                    let m = self.ident()?;
                    let v = if self.eat_word("VALUE") {
                        Some(self.paren_expr()?)
                    } else {
                        None
                    };
                    members.push((m, v));
                    if self.eat_op(",") {
                        continue;
                    }
                    self.expect_op(")")?;
                    break;
                }
                while !self.is_op(";") && !matches!(self.tok(), Tok::Eof) {
                    self.bump();
                }
                self.semi()?;
                StmtKind::Declare(vec![DeclItem {
                    level: 1,
                    name,
                    dims: Vec::new(),
                    attrs: vec![Attr::OrdinalDef(members)],
                    pos,
                }])
            }
            "DEFAULT" | "DFT" => {
                use crate::ast::{DefaultRange, DefaultRule};
                let mut rules = Vec::new();
                loop {
                    let mut ranges = Vec::new();
                    let mut descriptors = false;
                    if self.eat_word("RANGE") {
                        self.expect_op("(")?;
                        loop {
                            if self.eat_op("*") {
                                ranges.push(DefaultRange::All);
                            } else {
                                let a = self.ident()?;
                                if self.eat_op(":") {
                                    let b = self.ident()?;
                                    ranges.push(DefaultRange::Letters(
                                        a.chars().next().unwrap_or('A'),
                                        b.chars().next().unwrap_or('Z'),
                                    ));
                                } else {
                                    ranges.push(DefaultRange::Prefix(a));
                                }
                            }
                            if !self.eat_op(",") {
                                break;
                            }
                        }
                        self.expect_op(")")?;
                    } else if self.eat_word("DESCRIPTORS") {
                        descriptors = true;
                    } else if self.is_op("(") {
                        // a factored rule: not supported, skipped
                        self.paren_text()?;
                    }
                    let mut attrs = Vec::new();
                    let mut values = Vec::new();
                    self.stop_at_value = true;
                    loop {
                        if self.is_word("VALUE") && self.is_op_at(1, "(") {
                            self.bump();
                            self.bump();
                            loop {
                                values.push(self.attributes()?);
                                if !self.eat_op(",") {
                                    break;
                                }
                            }
                            self.expect_op(")")?;
                            continue;
                        }
                        let a = self.attributes()?;
                        if a.is_empty() {
                            break;
                        }
                        attrs.extend(a);
                    }
                    self.stop_at_value = false;
                    if !descriptors {
                        rules.push(DefaultRule {
                            ranges,
                            attrs,
                            values,
                        });
                    }
                    if !self.eat_op(",") {
                        break;
                    }
                }
                self.semi()?;
                StmtKind::Declare(vec![DeclItem {
                    level: 1,
                    name: "*DEFAULT".into(),
                    dims: Vec::new(),
                    attrs: vec![Attr::DefaultRules(rules)],
                    pos,
                }])
            }
            "IF" => {
                let cond = self.expr()?;
                if !self.eat_word("THEN") {
                    return Err(self.unexpected("THEN"));
                }
                let then = Box::new(self.statement()?);
                let els = if self.eat_word("ELSE") {
                    Some(Box::new(self.statement()?))
                } else {
                    None
                };
                StmtKind::If { cond, then, els }
            }
            "DO" => {
                let spec = self.do_spec()?;
                self.semi()?;
                let body = self.body()?;
                StmtKind::Do { spec, body }
            }
            "SELECT" => {
                let subject = if self.is_op("(") {
                    Some(self.paren_expr()?)
                } else {
                    None
                };
                self.semi()?;
                let mut whens = Vec::new();
                let mut other = None;
                loop {
                    if self.eat_word("WHEN") {
                        self.expect_op("(")?;
                        let mut vals = Vec::new();
                        loop {
                            vals.push(self.expr()?);
                            if self.eat_op(",") {
                                continue;
                            }
                            self.expect_op(")")?;
                            break;
                        }
                        let unit = self.statement()?;
                        whens.push((vals, unit));
                    } else if self.eat_word("OTHERWISE") || self.eat_word("OTHER") {
                        other = Some(Box::new(self.statement()?));
                    } else if self.eat_word("END") {
                        if matches!(self.tok(), Tok::Ident(_)) {
                            self.bump();
                        }
                        self.semi()?;
                        break;
                    } else {
                        return Err(self.unexpected("WHEN, OTHERWISE or END"));
                    }
                }
                StmtKind::Select {
                    subject,
                    whens,
                    other,
                }
            }
            "CALL" => {
                let r = self.reference()?;
                self.semi()?;
                StmtKind::Call(r)
            }
            "RETURN" => {
                let v = if self.is_op("(") {
                    Some(self.paren_expr()?)
                } else {
                    None
                };
                self.semi()?;
                StmtKind::Return(v)
            }
            "GO" | "GOTO" => {
                if w == "GO" && !self.eat_word("TO") {
                    return Err(self.unexpected("TO"));
                }
                let e = self.expr()?;
                self.semi()?;
                StmtKind::GoTo(e)
            }
            "LEAVE" | "ITERATE" => {
                let l = if let Tok::Ident(n) = self.tok().clone() {
                    self.bump();
                    Some(n)
                } else {
                    None
                };
                self.semi()?;
                if w == "LEAVE" {
                    StmtKind::Leave(l)
                } else {
                    StmtKind::Iterate(l)
                }
            }
            "ON" => {
                let mut conds = vec![self.cond_name()?];
                while self.eat_op(",") {
                    conds.push(self.cond_name()?);
                }
                let snap = self.eat_word("SNAP");
                if self.is_word("SYSTEM") && self.is_op_at(1, ";") {
                    self.bump();
                    self.bump();
                    StmtKind::On {
                        conds,
                        snap,
                        unit: None,
                    }
                } else {
                    StmtKind::On {
                        conds,
                        snap,
                        unit: Some(Box::new(self.statement()?)),
                    }
                }
            }
            "SIGNAL" => {
                let c = self.cond_name()?;
                self.semi()?;
                StmtKind::Signal(c)
            }
            "REVERT" => {
                let mut conds = vec![self.cond_name()?];
                while self.eat_op(",") {
                    conds.push(self.cond_name()?);
                }
                self.semi()?;
                StmtKind::Revert(conds)
            }
            "RESIGNAL" => {
                self.semi()?;
                StmtKind::Resignal
            }
            "STOP" => {
                self.semi()?;
                StmtKind::Stop
            }
            "EXIT" => {
                self.semi()?;
                StmtKind::Exit
            }
            "PUT" => StmtKind::Put(self.stream_opts(false)?),
            "GET" => StmtKind::Get(self.stream_opts(true)?),
            "OPEN" => StmtKind::Open(self.open()?),
            "CLOSE" => {
                let mut files = Vec::new();
                loop {
                    if self.eat_word("FILE") {
                        files.push(self.paren_expr()?);
                    }
                    while self.is_word("ENVIRONMENT") || self.is_word("ENV") {
                        self.bump();
                        self.paren_text()?;
                    }
                    if self.eat_op(",") {
                        continue;
                    }
                    self.semi()?;
                    break;
                }
                StmtKind::Close(files)
            }
            "READ" => StmtKind::Read(self.record_opts()?),
            "WRITE" => StmtKind::Write(self.record_opts()?),
            "REWRITE" => StmtKind::Rewrite(self.record_opts()?),
            "DELETE" => StmtKind::Delete(self.record_opts()?),
            "LOCATE" => {
                let r = self.reference()?;
                StmtKind::Locate(r, self.record_opts()?)
            }
            "ALLOCATE" | "ALLOC" => {
                let mut out = Vec::new();
                loop {
                    if matches!(self.tok(), Tok::Num { .. }) {
                        self.bump();
                    }
                    let r = self.basic_ref()?;
                    let mut set = None;
                    let mut len = None;
                    let mut area = None;
                    while let Tok::Ident(o) = self.tok().clone() {
                        self.bump();
                        let e = self.paren_expr()?;
                        match o.as_str() {
                            "SET" => set = Some(e),
                            "IN" => area = Some(e),
                            "CHARACTER" | "CHAR" | "BIT" => len = Some(e),
                            _ => {}
                        }
                    }
                    out.push((r, set, len, area));
                    if self.eat_op(",") {
                        continue;
                    }
                    self.semi()?;
                    break;
                }
                StmtKind::Allocate(out)
            }
            "FETCH" => {
                let mut out = Vec::new();
                loop {
                    let name = self.ident()?.to_ascii_uppercase();
                    let mut title = None;
                    loop {
                        if self.eat_word("TITLE") {
                            title = Some(self.paren_expr()?);
                        } else if self.eat_word("SET") {
                            self.paren_expr()?;
                        } else {
                            break;
                        }
                    }
                    out.push((name, title));
                    if self.eat_op(",") {
                        continue;
                    }
                    self.semi()?;
                    break;
                }
                StmtKind::Fetch(out)
            }
            "RELEASE" => {
                let mut out = Vec::new();
                if !self.eat_op("*") {
                    loop {
                        out.push(self.ident()?.to_ascii_uppercase());
                        if !self.eat_op(",") {
                            break;
                        }
                    }
                }
                self.semi()?;
                StmtKind::Release(out)
            }
            "FREE" => {
                let mut out = Vec::new();
                loop {
                    out.push(self.reference()?);
                    if self.eat_word("IN") {
                        self.paren_expr()?;
                    }
                    if self.eat_op(",") {
                        continue;
                    }
                    self.semi()?;
                    break;
                }
                StmtKind::Free(out)
            }
            "DISPLAY" => {
                let value = self.paren_expr()?;
                let mut reply = None;
                while let Tok::Ident(o) = self.tok().clone() {
                    self.bump();
                    let e = self.paren_expr()?;
                    if o == "REPLY" {
                        reply = Some(e);
                    }
                }
                self.semi()?;
                StmtKind::Display { value, reply }
            }
            "EXEC"
                if matches!(self.tok(), Tok::Ident(_))
                    && matches!(self.tok_at(1), Tok::Str { .. }) =>
            {
                let kind = self.ident()?;
                let Tok::Str { text, .. } = self.bump() else {
                    unreachable!()
                };
                self.semi()?;
                let words: Vec<String> = text
                    .split_whitespace()
                    .map(|w| w.to_ascii_uppercase())
                    .collect();
                if kind == "SQL" && (words == ["INCLUDE", "SQLCA"] || words == ["INCLUDE", "SQLDA"])
                {
                    // the SQL communication area, the descriptor area
                    let decl = if words[1] == "SQLCA" { SQLCA } else { SQLDA };
                    let mut toks = crate::lexer::tokenize_raw(decl, pos)?;
                    toks.push(Token { tok: Tok::Eof, pos });
                    return Parser::new(toks).statement().map(|s| s.kind);
                }
                StmtKind::Exec(kind, text)
            }
            "DELAY" => {
                let e = self.paren_expr()?;
                self.semi()?;
                StmtKind::Delay(e)
            }
            "FORMAT" => {
                self.expect_op("(")?;
                let f = self.formats()?;
                self.semi()?;
                StmtKind::Format(f)
            }
            _ => {
                return Err(Diag::error(pos, format!("statement {w} is not recognized")));
            }
        })
    }

    /// The external procedures of a compilation.
    pub fn program(&mut self) -> Result<Vec<Stmt>> {
        let mut out = Vec::new();
        while !matches!(self.tok(), Tok::Eof) {
            out.push(self.statement()?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::{Files, LexOptions, tokenize_text};

    fn parse(src: &str) -> Vec<Stmt> {
        let mut files = Files::default();
        let toks = tokenize_text(src, "t", &LexOptions::default(), &mut files).unwrap();
        Parser::new(toks).program().unwrap()
    }

    #[test]
    fn statements() {
        let p = parse(
            " T: PROC OPTIONS(MAIN);
               DCL (A, B) FIXED DEC(5,2) INIT(0),
                 1 S, 2 X(10) CHAR(3) VAR, 2 Y BIT(1);
               DCL IF FIXED BIN(31);
               IF = 1;
               IF A = B THEN A = -B ** 2; ELSE DO; B += 1; END;
               DO I = 1 TO 10 BY 2, 20 REPEAT I*2 WHILE(I < 100);
                 PUT SKIP EDIT(I, 'X')(F(5), A);
               END;
               SELECT (A);
                 WHEN (1, 2) PUT LIST('ONE');
                 OTHERWISE;
               END;
               ON ENDFILE(SYSIN) EOF = '1'B;
               (SIZE): L1: X = P->Q.R(1,2);
               PUT LIST((S.X(I) DO I = 1 TO 3));
             END T;",
        );
        let StmtKind::Procedure(proc) = &p[0].kind else {
            panic!()
        };
        assert_eq!(proc.names, ["T"]);
        assert_eq!(proc.options, ["MAIN"]);
        let StmtKind::Declare(d) = &proc.body[0].kind else {
            panic!()
        };
        assert_eq!(d.len(), 5);
        assert_eq!(d[3].dims.len(), 1);
        assert!(matches!(proc.body[2].kind, StmtKind::Assign { .. }));
        assert!(matches!(
            proc.body[3].kind,
            StmtKind::If { els: Some(_), .. }
        ));
        assert!(matches!(proc.body[4].kind, StmtKind::Do { .. }));
        assert!(matches!(proc.body[5].kind, StmtKind::Select { .. }));
        assert!(matches!(proc.body[6].kind, StmtKind::On { .. }));
        assert_eq!(proc.body[7].labels, ["L1"]);
        assert_eq!(proc.body[7].prefixes, [("SIZE".to_string(), true)]);
        let StmtKind::Put(o) = &proc.body[8].kind else {
            panic!()
        };
        assert!(matches!(
            &o.data,
            Some(Transmit::List(l)) if matches!(l[0], DataItem::Repeat(..))
        ));
    }
}

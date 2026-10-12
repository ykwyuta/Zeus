//! Identifiers, operands, arithmetic expressions and conditions.

use super::{Parser, VERBS};
use crate::diag::Result;
use crate::front::ast::*;
use crate::front::lexer::Tok;

/// Reserved words that can never be data names in an operand position.
const NOT_OPERANDS: &[&str] = &[
    "TO",
    "FROM",
    "BY",
    "INTO",
    "GIVING",
    "ROUNDED",
    "ON",
    "NOT",
    "SIZE",
    "ERROR",
    "END-ADD",
    "END-SUBTRACT",
    "END-MULTIPLY",
    "END-DIVIDE",
    "END-COMPUTE",
    "END-IF",
    "END-PERFORM",
    "END-READ",
    "END-WRITE",
    "END-EVALUATE",
    "END-SEARCH",
    "END-STRING",
    "END-UNSTRING",
    "END-CALL",
    "END-START",
    "END-DELETE",
    "END-REWRITE",
    "END-RETURN",
    "END-ACCEPT",
    "END-DISPLAY",
    "ELSE",
    "WHEN",
    "THEN",
    "AND",
    "OR",
    "IS",
    "THAN",
    "EQUAL",
    "GREATER",
    "LESS",
    "UNTIL",
    "TIMES",
    "VARYING",
    "AFTER",
    "BEFORE",
    "THRU",
    "THROUGH",
    "REMAINDER",
    "UPON",
    "WITH",
    "DELIMITED",
    "POINTER",
    "TALLYING",
    "REPLACING",
    "CONVERTING",
    "USING",
    "AT",
    "INVALID",
    "OVERFLOW",
    "EXCEPTION",
    "CORRESPONDING",
    "CORR",
    "ADVANCING",
    "LINE",
    "LINES",
    "PAGE",
    "END-OF-PAGE",
    "EOP",
    "ALSO",
    "OTHER",
    "COUNT",
    "IN",
    "OF",
    "RETURNING",
    "TEST",
    "DEPENDING",
    "KEY",
    "NEXT",
    "RECORD",
    "INTO",
    "FOR",
    "LEADING",
    "FIRST",
    "CHARACTERS",
    "DELIMITER",
    "UP",
    "DOWN",
];

fn figurative(w: &str) -> Option<Figurative> {
    Some(match w {
        "ZERO" | "ZEROS" | "ZEROES" => Figurative::Zero,
        "SPACE" | "SPACES" => Figurative::Space,
        "HIGH-VALUE" | "HIGH-VALUES" => Figurative::HighValue,
        "LOW-VALUE" | "LOW-VALUES" => Figurative::LowValue,
        "QUOTE" | "QUOTES" => Figurative::Quote,
        _ => return None,
    })
}

impl Parser {
    /// Whether the current token can start an operand.
    pub(super) fn is_operand_start(&self) -> bool {
        match self.peek().map(|t| &t.tok) {
            Some(Tok::Num(_)) | Some(Tok::Str(_)) | Some(Tok::Hex(_)) | Some(Tok::National(_)) => {
                true
            }
            Some(Tok::Word(w)) => {
                !VERBS.contains(&w.as_str()) && !NOT_OPERANDS.contains(&w.as_str())
            }
            _ => false,
        }
    }

    /// identifier: name { IN|OF name } [ ( subscripts ) ] [ ( start : [len] ) ]
    pub(super) fn data_ref(&mut self) -> Result<DataRef> {
        let pos = self.pos();
        let name = self.name()?;
        let mut r = DataRef {
            name,
            qualifiers: Vec::new(),
            subscripts: Vec::new(),
            refmod: None,
            pos,
        };
        while self.is("OF") || self.is("IN") {
            // Do not swallow "IN" of other constructs (e.g. INSPECT ...
            // REPLACING ... ) - qualification is always followed by a word.
            if !matches!(self.peek_at(1).map(|t| &t.tok), Some(Tok::Word(_))) {
                break;
            }
            self.i += 1;
            r.qualifiers.push(self.name()?);
        }
        if self.in_procedure && !self.tables.contains(&r.name) && !self.paren_is_refmod() {
            // not a table: a parenthesis here starts something else
            return Ok(r);
        }
        self.subscripts_and_refmod(&mut r.subscripts, &mut r.refmod)?;
        Ok(r)
    }

    fn subscripts_and_refmod(
        &mut self,
        subs: &mut Vec<Subscript>,
        refmod: &mut Option<(Box<Expr>, Option<Box<Expr>>)>,
    ) -> Result<()> {
        for _ in 0..2 {
            if !self.is_tok(&Tok::LParen) {
                break;
            }
            // Decide between subscripts and reference modification by
            // looking for a colon at the top level of the parentheses.
            let mut depth = 0;
            let mut colon = false;
            let mut k = 0;
            while let Some(t) = self.peek_at(k) {
                match t.tok {
                    Tok::LParen => depth += 1,
                    Tok::RParen => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    Tok::Colon if depth == 1 => colon = true,
                    _ => {}
                }
                k += 1;
            }
            self.i += 1;
            if colon {
                let start = self.expr()?;
                self.expect_tok(&Tok::Colon, "colon")?;
                let len = if self.is_tok(&Tok::RParen) {
                    None
                } else {
                    Some(Box::new(self.expr()?))
                };
                self.expect_tok(&Tok::RParen, "right parenthesis")?;
                *refmod = Some((Box::new(start), len));
                break;
            }
            if !subs.is_empty() {
                // a second parenthesised list that is not a refmod
                self.i -= 1;
                break;
            }
            while !self.is_tok(&Tok::RParen) {
                if self.eof() {
                    return self.err("right parenthesis expected");
                }
                if self.accept("ALL") {
                    subs.push(Subscript::All);
                } else {
                    subs.push(Subscript::Expr(self.expr()?));
                }
            }
            self.i += 1;
        }
        Ok(())
    }

    pub(super) fn literal_operand(&mut self) -> Option<Operand> {
        let lit = match self.peek().map(|t| &t.tok) {
            Some(Tok::Num(n)) => Literal::Num(n.clone()),
            Some(Tok::Str(s)) => Literal::Str(s.clone()),
            Some(Tok::Hex(b)) => Literal::Hex(b.clone()),
            Some(Tok::National(b)) => Literal::National(b.clone()),
            _ => return None,
        };
        self.i += 1;
        Some(Operand::Lit(lit))
    }

    /// An operand: literal, figurative constant, ALL literal, identifier,
    /// function reference, LENGTH OF / ADDRESS OF.
    pub(super) fn operand(&mut self) -> Result<Operand> {
        if let Some(lit) = self.literal_operand() {
            return Ok(lit);
        }
        let Some(word) = self.peek().and_then(|t| t.word()).map(str::to_string) else {
            return self.err("operand expected");
        };
        if let Some(f) = figurative(&word) {
            self.i += 1;
            return Ok(Operand::Fig(f));
        }
        match word.as_str() {
            "ALL" => {
                self.i += 1;
                let inner = self.operand()?;
                Ok(Operand::All(Box::new(inner)))
            }
            "NULL" | "NULLS" => {
                self.i += 1;
                Ok(Operand::Null)
            }
            "FUNCTION" => {
                let pos = self.pos();
                self.i += 1;
                let name = self.name()?;
                let mut args = Vec::new();
                if self.is_tok(&Tok::LParen) && !self.paren_is_refmod() {
                    self.i += 1;
                    while !self.is_tok(&Tok::RParen) {
                        if self.eof() {
                            return self.err("right parenthesis expected");
                        }
                        args.push(self.expr()?);
                    }
                    self.i += 1;
                }
                let mut refmod = None;
                let mut subs = Vec::new();
                self.subscripts_and_refmod(&mut subs, &mut refmod)?;
                Ok(Operand::Func(Box::new(FuncCall {
                    name,
                    args,
                    refmod,
                    pos,
                })))
            }
            "LENGTH" if self.is_at(1, "OF") => {
                self.i += 2;
                Ok(Operand::LengthOf(self.data_ref()?))
            }
            "ADDRESS" if self.is_at(1, "OF") => {
                self.i += 2;
                Ok(Operand::AddressOf(self.data_ref()?))
            }
            _ => Ok(Operand::Ref(self.data_ref()?)),
        }
    }

    /// Whether the parenthesis at the cursor contains a top-level colon.
    fn paren_is_refmod(&self) -> bool {
        let mut depth = 0;
        let mut k = 0;
        while let Some(t) = self.peek_at(k) {
            match t.tok {
                Tok::LParen => depth += 1,
                Tok::RParen => {
                    depth -= 1;
                    if depth == 0 {
                        return false;
                    }
                }
                Tok::Colon if depth == 1 => return true,
                _ => {}
            }
            k += 1;
        }
        false
    }

    // ---- arithmetic expressions ----------------------------------------

    pub(super) fn expr(&mut self) -> Result<Expr> {
        let mut e = self.term()?;
        loop {
            let op = if self.accept_tok(&Tok::Plus) {
                BinOp::Add
            } else if self.accept_tok(&Tok::Minus) {
                BinOp::Sub
            } else {
                break;
            };
            let rhs = self.term()?;
            e = Expr::Bin(op, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }

    fn term(&mut self) -> Result<Expr> {
        let mut e = self.power()?;
        loop {
            let op = if self.accept_tok(&Tok::Star) {
                BinOp::Mul
            } else if self.accept_tok(&Tok::Slash) {
                BinOp::Div
            } else {
                break;
            };
            let rhs = self.power()?;
            e = Expr::Bin(op, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }

    fn power(&mut self) -> Result<Expr> {
        let mut e = self.unary()?;
        while self.accept_tok(&Tok::Power) {
            let rhs = self.unary()?;
            e = Expr::Bin(BinOp::Pow, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }

    fn unary(&mut self) -> Result<Expr> {
        if self.accept_tok(&Tok::Minus) {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        if self.accept_tok(&Tok::Plus) {
            return self.unary();
        }
        if self.accept_tok(&Tok::LParen) {
            let e = self.expr()?;
            self.expect_tok(&Tok::RParen, "right parenthesis")?;
            return Ok(e);
        }
        Ok(Expr::Operand(self.operand()?))
    }

    // ---- conditions ----------------------------------------------------

    /// Parses a relational operator (with an optional leading IS and NOT).
    /// Returns None without consuming if there is none.
    pub(super) fn relop(&mut self) -> Option<RelOp> {
        let save = self.i;
        self.accept("IS");
        let not = self.accept("NOT");
        let op = if self.accept_tok(&Tok::Eq) || self.accept("EQUAL") || self.accept("EQUALS") {
            self.accept("TO");
            Some(RelOp::Eq)
        } else if self.accept_tok(&Tok::Gt) {
            Some(RelOp::Gt)
        } else if self.accept_tok(&Tok::Lt) {
            Some(RelOp::Lt)
        } else if self.accept_tok(&Tok::Ge) {
            Some(RelOp::Ge)
        } else if self.accept_tok(&Tok::Le) {
            Some(RelOp::Le)
        } else if self.accept("GREATER") {
            self.accept("THAN");
            if self.is("OR") && self.is_at(1, "EQUAL") {
                self.i += 2;
                self.accept("TO");
                Some(RelOp::Ge)
            } else {
                Some(RelOp::Gt)
            }
        } else if self.accept("LESS") {
            self.accept("THAN");
            if self.is("OR") && self.is_at(1, "EQUAL") {
                self.i += 2;
                self.accept("TO");
                Some(RelOp::Le)
            } else {
                Some(RelOp::Lt)
            }
        } else {
            None
        };
        match op {
            Some(op) => Some(if not { op.negate() } else { op }),
            None => {
                self.i = save;
                None
            }
        }
    }

    pub(super) fn cond(&mut self) -> Result<Cond> {
        let mut last = None;
        self.or_cond(&mut last)
    }

    fn or_cond(&mut self, last: &mut Option<(Expr, RelOp)>) -> Result<Cond> {
        let mut c = self.and_cond(last)?;
        while self.accept("OR") {
            let rhs = self.and_cond(last)?;
            c = Cond::Or(Box::new(c), Box::new(rhs));
        }
        Ok(c)
    }

    fn and_cond(&mut self, last: &mut Option<(Expr, RelOp)>) -> Result<Cond> {
        let mut c = self.not_cond(last)?;
        while self.accept("AND") {
            let rhs = self.not_cond(last)?;
            c = Cond::And(Box::new(c), Box::new(rhs));
        }
        Ok(c)
    }

    fn not_cond(&mut self, last: &mut Option<(Expr, RelOp)>) -> Result<Cond> {
        // NOT followed by a relational operator belongs to an abbreviated
        // relation ("A = B OR NOT > C").
        if self.is("NOT") && last.is_some() {
            let save = self.i;
            if self.relop().is_some() {
                self.i = save;
                return self.simple_cond(last);
            }
            self.i = save;
        }
        if self.accept("NOT") {
            let c = self.not_cond(last)?;
            return Ok(Cond::Not(Box::new(c)));
        }
        self.simple_cond(last)
    }

    fn simple_cond(&mut self, last: &mut Option<(Expr, RelOp)>) -> Result<Cond> {
        // Abbreviated relation with an explicit operator: "... OR > C".
        if let Some((subject, _)) = last.clone() {
            let save = self.i;
            if let Some(op) = self.relop() {
                let obj = self.expr()?;
                *last = Some((subject.clone(), op));
                return Ok(Cond::Rel(subject, op, obj));
            }
            self.i = save;
        }
        if self.is_tok(&Tok::LParen) {
            // Either a parenthesised condition or an arithmetic expression
            // starting a relation; try the condition first.
            let save = self.i;
            self.i += 1;
            let mut inner_last = last.clone();
            if let Ok(c) = self.or_cond(&mut inner_last)
                && self.accept_tok(&Tok::RParen)
                && !self.next_is_arith_or_relop()
            {
                *last = inner_last;
                return Ok(c);
            }
            self.i = save;
        }
        let subject = self.expr()?;
        // class / sign / relation
        let save = self.i;
        self.accept("IS");
        let not = self.accept("NOT");
        let class = if self.accept("NUMERIC") {
            Some(ClassTest::Numeric)
        } else if self.accept("ALPHABETIC") {
            Some(ClassTest::Alphabetic)
        } else if self.accept("ALPHABETIC-LOWER") {
            Some(ClassTest::AlphabeticLower)
        } else if self.accept("ALPHABETIC-UPPER") {
            Some(ClassTest::AlphabeticUpper)
        } else if let Some(w) = self.peek().and_then(|t| t.word())
            && self.class_names.contains(w)
        {
            let w = w.to_string();
            self.i += 1;
            Some(ClassTest::Class(w))
        } else {
            None
        };
        if let Some(class) = class {
            let c = Cond::Class(subject, class);
            return Ok(if not { Cond::Not(Box::new(c)) } else { c });
        }
        let sign = if self.accept("POSITIVE") {
            Some(SignTest::Positive)
        } else if self.accept("NEGATIVE") {
            Some(SignTest::Negative)
        } else if self.is_any(&["ZERO", "ZEROS", "ZEROES"]) && !self.next_after_zero_is_operand() {
            self.i += 1;
            Some(SignTest::Zero)
        } else {
            None
        };
        if let Some(sign) = sign {
            let c = Cond::Sign(subject, sign);
            return Ok(if not { Cond::Not(Box::new(c)) } else { c });
        }
        self.i = save;
        if let Some(op) = self.relop() {
            let obj = self.expr()?;
            *last = Some((subject.clone(), op));
            return Ok(Cond::Rel(subject, op, obj));
        }
        // Abbreviated relation with implied subject and operator.
        if let Some((s, op)) = last.clone() {
            return Ok(Cond::Rel(s, op, subject));
        }
        match subject {
            Expr::Operand(Operand::Ref(r)) => Ok(Cond::Name(r)),
            _ => self.err("condition expected"),
        }
    }

    /// After a parenthesised group: does an arithmetic or relational
    /// operator follow (meaning the group was an arithmetic expression)?
    fn next_is_arith_or_relop(&mut self) -> bool {
        if matches!(
            self.peek().map(|t| &t.tok),
            Some(Tok::Plus | Tok::Minus | Tok::Star | Tok::Slash | Tok::Power)
        ) {
            return true;
        }
        let save = self.i;
        let r = self.relop().is_some();
        self.i = save;
        if r {
            return true;
        }
        let save = self.i;
        self.accept("IS");
        self.accept("NOT");
        let class_or_sign = self.is_any(&[
            "NUMERIC",
            "ALPHABETIC",
            "ALPHABETIC-LOWER",
            "ALPHABETIC-UPPER",
            "POSITIVE",
            "NEGATIVE",
            "ZERO",
        ]);
        self.i = save;
        class_or_sign
    }

    /// "A IS ZERO" is a sign test; "A = ZERO" never reaches here. ZERO
    /// followed by an arithmetic operator would be odd; treat ZERO as a
    /// sign test unless followed by an operator.
    fn next_after_zero_is_operand(&self) -> bool {
        matches!(
            self.peek_at(1).map(|t| &t.tok),
            Some(Tok::Plus | Tok::Minus | Tok::Star | Tok::Slash | Tok::Power)
        )
    }
}

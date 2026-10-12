//! Tokenizer for COBOL source text.

use super::source::Pos;
use crate::diag::{Diag, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    /// A COBOL word (upper-cased): reserved word or user-defined name.
    Word(String),
    /// Numeric literal as written, e.g. `-12.5` or `007`.
    Num(String),
    /// Alphanumeric literal (characters as written, quotes removed).
    Str(String),
    /// Hexadecimal literal `X"..."` (bytes as written).
    Hex(Vec<u8>),
    /// National literal `N"..."` (NSYMBOL(NATIONAL)) or `NX"..."`:
    /// UTF-16 big-endian.
    National(Vec<u8>),
    /// PICTURE character-string.
    Pic(String),
    /// An embedded `EXEC <kind> ... END-EXEC` block: the kind (`CICS`,
    /// `SQL`, `DLI`) and the source characters between it and END-EXEC.
    Exec(String, Vec<(char, Pos)>),
    /// `== ... ==` pseudo-text.
    Pseudo(Vec<Token>),
    Period,
    LParen,
    RParen,
    Colon,
    Plus,
    Minus,
    Star,
    Slash,
    Power,
    Eq,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub tok: Tok,
    pub pos: Pos,
}

impl Token {
    pub fn is_word(&self, w: &str) -> bool {
        matches!(&self.tok, Tok::Word(x) if x == w)
    }

    pub fn word(&self) -> Option<&str> {
        match &self.tok {
            Tok::Word(w) => Some(w),
            _ => None,
        }
    }

    /// Text used for pseudo-text comparison and diagnostics.
    pub fn text(&self) -> String {
        match &self.tok {
            Tok::Word(w) | Tok::Num(w) | Tok::Pic(w) => w.clone(),
            Tok::Str(s) => format!("\"{s}\""),
            Tok::Hex(b) => format!(
                "X\"{}\"",
                b.iter().map(|x| format!("{x:02X}")).collect::<String>()
            ),
            Tok::National(b) => format!(
                "NX\"{}\"",
                b.iter().map(|x| format!("{x:02X}")).collect::<String>()
            ),
            Tok::Pseudo(_) => "==...==".to_string(),
            Tok::Exec(k, _) => format!("EXEC {k}"),
            Tok::Period => ".".into(),
            Tok::LParen => "(".into(),
            Tok::RParen => ")".into(),
            Tok::Colon => ":".into(),
            Tok::Plus => "+".into(),
            Tok::Minus => "-".into(),
            Tok::Star => "*".into(),
            Tok::Slash => "/".into(),
            Tok::Power => "**".into(),
            Tok::Eq => "=".into(),
            Tok::Lt => "<".into(),
            Tok::Gt => ">".into(),
            Tok::Le => "<=".into(),
            Tok::Ge => ">=".into(),
        }
    }
}

struct Lexer<'a> {
    s: &'a [(char, Pos)],
    i: usize,
    out: Vec<Token>,
    decimal_comma: bool,
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

fn is_space(c: char) -> bool {
    c == ' ' || c == '\n'
}

impl Lexer<'_> {
    fn peek(&self, k: usize) -> Option<char> {
        self.s.get(self.i + k).map(|(c, _)| *c)
    }

    fn pos(&self) -> Pos {
        self.s
            .get(self.i)
            .or(self.s.last())
            .map(|(_, p)| *p)
            .unwrap_or_default()
    }

    fn at_separator_end(&self, k: usize) -> bool {
        self.peek(k).is_none_or(is_space)
    }

    /// Whether the previous token allows a picture string to follow.
    fn expects_picture(&self) -> bool {
        let n = self.out.len();
        let is = |k: usize, w: &str| n >= k && self.out[n - k].is_word(w);
        is(1, "PIC") || is(1, "PICTURE") || (is(1, "IS") && (is(2, "PIC") || is(2, "PICTURE")))
    }

    /// Whether a sign here starts a numeric literal (unary sign attached to
    /// the number): previous character blank or `(`, next a digit.
    fn signed_number_start(&self) -> bool {
        let prev = if self.i == 0 {
            ' '
        } else {
            self.s[self.i - 1].0
        };
        let next = self.peek(1);
        let next2 = self.peek(2);
        (is_space(prev) || prev == '(' || prev == '=')
            && (next.is_some_and(|c| c.is_ascii_digit())
                || (next == Some(self.point()) && next2.is_some_and(|c| c.is_ascii_digit())))
    }

    fn point(&self) -> char {
        if self.decimal_comma { ',' } else { '.' }
    }

    fn push(&mut self, tok: Tok, pos: Pos) {
        self.out.push(Token { tok, pos });
    }

    fn literal(&mut self, quote: char) -> Result<String> {
        let pos = self.pos();
        self.i += 1;
        let mut s = String::new();
        loop {
            match self.peek(0) {
                None | Some('\n') => {
                    return Err(Diag::error(pos, "unterminated literal"));
                }
                Some(c) if c == quote => {
                    if self.peek(1) == Some(quote) {
                        s.push(quote);
                        self.i += 2;
                    } else {
                        self.i += 1;
                        return Ok(s);
                    }
                }
                Some(c) => {
                    s.push(c);
                    self.i += 1;
                }
            }
        }
    }

    fn number(&mut self) -> String {
        let mut s = String::new();
        if let Some(c @ ('+' | '-')) = self.peek(0) {
            s.push(c);
            self.i += 1;
        }
        let point = self.point();
        while let Some(c) = self.peek(0) {
            if c.is_ascii_digit() {
                s.push(c);
                self.i += 1;
            } else if c == point && self.peek(1).is_some_and(|d| d.is_ascii_digit()) {
                s.push('.');
                self.i += 1;
            } else {
                break;
            }
        }
        s
    }

    fn picture(&mut self) {
        let pos = self.pos();
        let mut s = String::new();
        while let Some(c) = self.peek(0) {
            if is_space(c) {
                break;
            }
            // A period, comma or semicolon followed by a blank ends the
            // picture string.
            if matches!(c, '.' | ',' | ';') && self.at_separator_end(1) {
                break;
            }
            // end of pseudo-text
            if c == '=' && self.peek(1) == Some('=') {
                break;
            }
            s.push(c.to_ascii_uppercase());
            self.i += 1;
        }
        self.push(Tok::Pic(s), pos);
    }

    fn run(&mut self) -> Result<()> {
        while let Some(c) = self.peek(0) {
            let pos = self.pos();
            if is_space(c) {
                self.i += 1;
                continue;
            }
            if (c == ',' || c == ';') && self.at_separator_end(1) {
                self.i += 1;
                continue;
            }
            let is_word_is = matches!(self.peek(0), Some('I' | 'i'))
                && matches!(self.peek(1), Some('S' | 's'))
                && self.at_separator_end(2);
            if self.expects_picture() && !is_word_is {
                self.picture();
                continue;
            }
            match c {
                '"' | '\'' => {
                    let s = self.literal(c)?;
                    self.push(Tok::Str(s), pos);
                }
                // national hexadecimal literal NX"..." (UTF-16 code units)
                'N' | 'n'
                    if matches!(self.peek(1), Some('X' | 'x'))
                        && matches!(self.peek(2), Some('"' | '\'')) =>
                {
                    self.i += 2;
                    let q = self.peek(0).unwrap();
                    let s = self.literal(q)?;
                    let bytes = parse_hex(&s).filter(|b| b.len() % 2 == 0).ok_or_else(|| {
                        Diag::error(
                            pos,
                            format!("invalid national hexadecimal literal NX\"{s}\""),
                        )
                    })?;
                    self.push(Tok::National(bytes), pos);
                }
                // N"...": national (UTF-16) or, with NSYMBOL(DBCS), DBCS;
                // G"...": DBCS (double-byte characters of the code page)
                'G' | 'g' | 'N' | 'n' if matches!(self.peek(1), Some('"' | '\'')) => {
                    let national = matches!(c, 'N' | 'n') && !super::nsymbol_dbcs();
                    self.i += 1;
                    let q = self.peek(0).unwrap();
                    let s = self.literal(q)?;
                    if national {
                        let bytes = s.encode_utf16().flat_map(u16::to_be_bytes).collect();
                        self.push(Tok::National(bytes), pos);
                    } else {
                        let bytes = crate::codepage::active().encode_dbcs(&s);
                        self.push(Tok::Hex(bytes), pos);
                    }
                }
                'X' | 'x' if matches!(self.peek(1), Some('"' | '\'')) => {
                    self.i += 1;
                    let q = self.peek(0).unwrap();
                    let s = self.literal(q)?;
                    let bytes = parse_hex(&s).ok_or_else(|| {
                        Diag::error(pos, format!("invalid hexadecimal literal X\"{s}\""))
                    })?;
                    self.push(Tok::Hex(bytes), pos);
                }
                '.' if self.at_separator_end(1) => {
                    self.i += 1;
                    self.push(Tok::Period, pos);
                }
                '=' if self.peek(1) == Some('=') => {
                    self.i += 2;
                    let start = self.out.len();
                    loop {
                        // Skip blanks to find the closing delimiter.
                        while self.peek(0).is_some_and(is_space) {
                            self.i += 1;
                        }
                        match self.peek(0) {
                            None => return Err(Diag::error(pos, "unterminated pseudo-text")),
                            Some('=') if self.peek(1) == Some('=') => {
                                self.i += 2;
                                break;
                            }
                            _ => {
                                let is_word_is = matches!(self.peek(0), Some('I' | 'i'))
                                    && matches!(self.peek(1), Some('S' | 's'))
                                    && self.at_separator_end(2);
                                if self.expects_picture() && !is_word_is {
                                    self.picture();
                                } else {
                                    self.one()?;
                                }
                            }
                        }
                    }
                    let inner = self.out.split_off(start);
                    self.push(Tok::Pseudo(inner), pos);
                }
                _ => self.one()?,
            }
        }
        Ok(())
    }

    /// Word at `i` (upper-cased) and the index after it.
    fn word_at(&self, mut i: usize) -> (String, usize) {
        let mut w = String::new();
        while let Some((c, _)) = self.s.get(i) {
            if !is_word_char(*c) {
                break;
            }
            w.push(c.to_ascii_uppercase());
            i += 1;
        }
        (w, i)
    }

    /// `EXEC CICS|SQL|DLI ... END-EXEC` at the current position: one
    /// [`Tok::Exec`] holding the raw text (literals kept intact).
    fn exec_block(&mut self) -> Result<Option<()>> {
        let pos = self.pos();
        let (w, mut j) = self.word_at(self.i);
        if w != "EXEC" && w != "EXECUTE" {
            return Ok(None);
        }
        while self.s.get(j).is_some_and(|(c, _)| is_space(*c)) {
            j += 1;
        }
        let (kind, k) = self.word_at(j);
        if !matches!(kind.as_str(), "CICS" | "SQL" | "DLI" | "SQLIMS") {
            return Ok(None);
        }
        let start = k;
        let mut i = k;
        let mut quote: Option<char> = None;
        loop {
            let Some((c, _)) = self.s.get(i) else {
                return Err(Diag::error(pos, "END-EXEC expected"));
            };
            match quote {
                Some(q) if *c == q => quote = None,
                Some(_) => {}
                None if *c == '\'' || *c == '"' => quote = Some(*c),
                None if c.is_ascii_alphabetic() && (i == 0 || !is_word_char(self.s[i - 1].0)) => {
                    let (w, e) = self.word_at(i);
                    if w == "END-EXEC" {
                        let text = self.s[start..i].to_vec();
                        self.i = e;
                        self.push(Tok::Exec(kind, text), pos);
                        return Ok(Some(()));
                    }
                    i = e;
                    continue;
                }
                None => {}
            }
            i += 1;
        }
    }

    /// Lexes one ordinary token at the current position.
    fn one(&mut self) -> Result<()> {
        let pos = self.pos();
        let c = self.peek(0).unwrap();
        let point = self.point();
        if c == '"' || c == '\'' {
            let s = self.literal(c)?;
            self.push(Tok::Str(s), pos);
            return Ok(());
        }
        if (c == '+' || c == '-') && self.signed_number_start() {
            let n = self.number();
            self.push(Tok::Num(n), pos);
            return Ok(());
        }
        if c == point && self.peek(1).is_some_and(|d| d.is_ascii_digit()) {
            let n = self.number();
            self.push(Tok::Num(n), pos);
            return Ok(());
        }
        if c.is_ascii_alphanumeric() {
            if let Some(()) = self.exec_block()? {
                return Ok(());
            }
            let start = self.i;
            while self.peek(0).is_some_and(is_word_char) {
                self.i += 1;
            }
            let word: String = self.s[start..self.i]
                .iter()
                .map(|(c, _)| c.to_ascii_uppercase())
                .collect();
            if word.chars().all(|c| c.is_ascii_digit()) {
                // Possibly a decimal number like 12.5
                self.i = start;
                let n = self.number();
                self.push(Tok::Num(n), pos);
            } else {
                self.push(Tok::Word(word), pos);
            }
            return Ok(());
        }
        self.i += 1;
        let tok = match c {
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            ':' => Tok::Colon,
            '+' => Tok::Plus,
            '-' => Tok::Minus,
            '/' => Tok::Slash,
            '*' if self.peek(0) == Some('*') => {
                self.i += 1;
                Tok::Power
            }
            '*' => Tok::Star,
            '=' => Tok::Eq,
            '<' if self.peek(0) == Some('=') => {
                self.i += 1;
                Tok::Le
            }
            '>' if self.peek(0) == Some('=') => {
                self.i += 1;
                Tok::Ge
            }
            '<' => Tok::Lt,
            '>' => Tok::Gt,
            '.' => Tok::Period,
            ',' | ';' => return Ok(()),
            other => {
                return Err(Diag::error(pos, format!("unexpected character {other:?}")));
            }
        };
        self.push(tok, pos);
        Ok(())
    }
}

fn parse_hex(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

pub fn lex(stream: &[(char, Pos)], decimal_comma: bool) -> Result<Vec<Token>> {
    let mut lx = Lexer {
        s: stream,
        i: 0,
        out: Vec::new(),
        decimal_comma,
    };
    lx.run()?;
    Ok(lx.out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        let stream: Vec<(char, Pos)> = s.chars().map(|c| (c, Pos::default())).collect();
        lex(&stream, false)
            .unwrap()
            .into_iter()
            .map(|t| t.tok)
            .collect()
    }

    fn w(s: &str) -> Tok {
        Tok::Word(s.into())
    }

    #[test]
    fn words_numbers_period() {
        assert_eq!(
            toks("move -1.5 to a-b. 100-x"),
            [
                w("MOVE"),
                Tok::Num("-1.5".into()),
                w("TO"),
                w("A-B"),
                Tok::Period,
                w("100-X")
            ]
        );
        assert_eq!(
            toks("VALUE .5."),
            [w("VALUE"), Tok::Num(".5".into()), Tok::Period]
        );
        assert_eq!(toks("A - 1"), [w("A"), Tok::Minus, Tok::Num("1".into())]);
        assert_eq!(
            toks("(-1)"),
            [Tok::LParen, Tok::Num("-1".into()), Tok::RParen]
        );
    }

    #[test]
    fn pictures() {
        assert_eq!(
            toks("PIC S9(3)V99. PICTURE IS $**,**9.99CR."),
            [
                w("PIC"),
                Tok::Pic("S9(3)V99".into()),
                Tok::Period,
                w("PICTURE"),
                w("IS"),
                Tok::Pic("$**,**9.99CR".into()),
                Tok::Period
            ]
        );
    }

    #[test]
    fn literals() {
        assert_eq!(
            toks("\"A\"\"B\" 'C' X\"C1F0\""),
            [
                Tok::Str("A\"B".into()),
                Tok::Str("C".into()),
                Tok::Hex(vec![0xC1, 0xF0])
            ]
        );
    }

    #[test]
    fn operators_and_pseudo_text() {
        assert_eq!(
            toks("A ** 2 >= B <= C"),
            [
                w("A"),
                Tok::Power,
                Tok::Num("2".into()),
                Tok::Ge,
                w("B"),
                Tok::Le,
                w("C")
            ]
        );
        let t = toks("REPLACING ==X Y== BY ====");
        assert_eq!(t.len(), 4);
        match &t[1] {
            Tok::Pseudo(inner) => assert_eq!(inner.len(), 2),
            other => panic!("{other:?}"),
        }
        assert_eq!(t[3], Tok::Pseudo(vec![]));
    }

    #[test]
    fn comma_separators() {
        assert_eq!(toks("A, B; C"), [w("A"), w("B"), w("C")]);
    }
}

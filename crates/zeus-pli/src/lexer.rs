//! Source text and tokens.
//!
//! The source is read between the margins (MARGINS(2,72) by default, as
//! the compiler's default; `*PROCESS` / `%PROCESS` lines at the top give
//! compiler options, of which MARGINS is used). Each line contributes its
//! columns between the margins, padded with blanks, so a string constant
//! continued on the next line includes the blanks up to the right margin.
//! Comments are `/* ... */`. `%INCLUDE name;`, `%INCLUDE dd(name);` and
//! `%INCLUDE 'file';` insert the tokens of a member found in the include
//! directories; listing control statements (%PAGE, %SKIP, %PRINT,
//! %NOPRINT) are ignored.

use std::fmt;
use std::path::{Path, PathBuf};

/// Position of a token: file (index in the file table) and line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pos {
    pub file: u16,
    pub line: u32,
}

/// A diagnostic message.
#[derive(Debug, Clone)]
pub struct Diag {
    pub pos: Pos,
    /// I (informational), W (warning), E (error), S (severe).
    pub severity: char,
    pub message: String,
}

impl Diag {
    pub fn error(pos: Pos, message: impl Into<String>) -> Diag {
        Diag {
            pos,
            severity: 'S',
            message: message.into(),
        }
    }

    pub fn warning(pos: Pos, message: impl Into<String>) -> Diag {
        Diag {
            pos,
            severity: 'W',
            message: message.into(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Diag>;

/// Kind of a string constant: its suffix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrKind {
    /// 'abc'
    Char,
    /// 'C1C2'X: characters in hexadecimal
    Hex,
    /// '1010'B (B1), 'F0'B4 (hexadecimal digits for 4 bits each), B2, B3
    Bit(u8),
    /// '00FF'XN: a FIXED BINARY constant in hexadecimal
    HexBin,
    /// '00FF'XU: unsigned FIXED BINARY in hexadecimal
    HexUnsigned,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    /// A numeric constant as written (without the B suffix) and whether it
    /// is binary (suffix B).
    Num {
        text: String,
        binary: bool,
    },
    Str {
        text: String,
        kind: StrKind,
    },
    /// Operators and punctuation: + - * / ** || ¬ & | = ¬= < > <= >= ¬< ¬>
    /// -> ( ) , ; : . and compound assignment operators.
    Op(&'static str),
    Eof,
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tok::Ident(s) => write!(f, "{s}"),
            Tok::Num { text, binary } => write!(f, "{text}{}", if *binary { "B" } else { "" }),
            Tok::Str { text, .. } => write!(f, "'{text}'"),
            Tok::Op(o) => write!(f, "{o}"),
            Tok::Eof => write!(f, "end of source"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub pos: Pos,
}

/// Source files read for a compilation.
#[derive(Debug, Default)]
pub struct Files {
    pub names: Vec<String>,
}

/// Lexer options.
#[derive(Debug, Clone)]
pub struct LexOptions {
    pub margins: (usize, usize),
    pub include_dirs: Vec<PathBuf>,
}

impl Default for LexOptions {
    fn default() -> Self {
        LexOptions {
            margins: (2, 72),
            include_dirs: Vec::new(),
        }
    }
}

/// `MARGINS(m,n)` among compiler options.
fn margins_option(opts: &str) -> Option<(usize, usize)> {
    let u = opts.to_ascii_uppercase();
    let i = u.find("MARGINS(").or_else(|| u.find("MAR("))?;
    let rest = &u[i..];
    let open = rest.find('(')?;
    let close = rest.find(')')?;
    let mut it = rest[open + 1..close].split(',');
    let a = it.next()?.trim().parse().ok()?;
    let b = it.next()?.trim().parse().ok()?;
    Some((a, b))
}

/// The text between the margins of each line, padded; `*PROCESS` lines
/// at the top are compiler options.
pub fn prepare(text: &str, opts: &LexOptions) -> (String, (usize, usize)) {
    let mut margins = opts.margins;
    let mut out = String::new();
    let mut header = true;
    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if header {
            let u = line.trim_start().to_ascii_uppercase();
            if u.starts_with("*PROCESS") || u.starts_with("%PROCESS") {
                if let Some(m) = margins_option(&u) {
                    margins = m;
                }
                out.push('\n');
                continue;
            }
            header = false;
        }
        let chars: Vec<char> = line.chars().collect();
        let (m1, m2) = margins;
        let width = m2 + 1 - m1;
        let mut s: String = chars
            .iter()
            .skip(m1.saturating_sub(1))
            .take(width)
            .collect();
        let n = s.chars().count();
        s.extend(std::iter::repeat_n(' ', width.saturating_sub(n)));
        out.push_str(&s);
        out.push('\n');
    }
    (out, margins)
}

struct Lexer {
    c: Vec<char>,
    i: usize,
    line: u32,
    file: u16,
    out: Vec<Token>,
}

fn ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '@' | '#' | '$')
}

impl Lexer {
    fn pos(&self) -> Pos {
        Pos {
            file: self.file,
            line: self.line,
        }
    }

    fn peek(&self, k: usize) -> Option<char> {
        self.c.get(self.i + k).copied()
    }

    fn push(&mut self, tok: Tok, pos: Pos) {
        self.out.push(Token { tok, pos });
    }

    fn skip_blanks(&mut self) -> Result<()> {
        loop {
            match self.peek(0) {
                Some('\n') => {
                    self.line += 1;
                    self.i += 1;
                }
                Some(c) if c.is_whitespace() => self.i += 1,
                Some('/') if self.peek(1) == Some('*') => {
                    let pos = self.pos();
                    self.i += 2;
                    loop {
                        match self.peek(0) {
                            None => return Err(Diag::error(pos, "comment not closed")),
                            Some('*') if self.peek(1) == Some('/') => {
                                self.i += 2;
                                break;
                            }
                            Some('\n') => {
                                self.line += 1;
                                self.i += 1;
                            }
                            Some(_) => self.i += 1,
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn run(&mut self) -> Result<()> {
        loop {
            self.skip_blanks()?;
            let pos = self.pos();
            let Some(c) = self.peek(0) else {
                return Ok(());
            };
            if ident_char(c) && !c.is_ascii_digit() {
                let start = self.i;
                while self.peek(0).is_some_and(ident_char) {
                    self.i += 1;
                }
                let w: String = self.c[start..self.i].iter().collect();
                let w = w.to_ascii_uppercase();
                let exec = w == "EXEC";
                self.push(Tok::Ident(w), pos);
                if exec {
                    self.exec_text()?;
                }
                continue;
            }
            if c.is_ascii_digit() || (c == '.' && self.peek(1).is_some_and(|d| d.is_ascii_digit()))
            {
                self.number(pos);
                continue;
            }
            if c == '\'' || c == '"' {
                self.string(c, pos)?;
                continue;
            }
            if c == '%' {
                self.i += 1;
                self.percent(pos)?;
                continue;
            }
            let two: String = self.c[self.i..(self.i + 3).min(self.c.len())]
                .iter()
                .collect();
            let ops: &[&'static str] = &[
                "||=", "**=", "¬=", "^=", "~=", "¬<", "^<", "~<", "¬>", "^>", "~>", "<=", ">=",
                "<>", "->", "=>", "**", "||", "!!", "+=", "-=", "*=", "/=", "|=", "&=", "+", "-",
                "*", "/", "¬", "^", "~", "&", "|", "!", "=", "<", ">", "(", ")", ",", ";", ":",
                ".",
            ];
            let Some(op) = ops.iter().find(|o| two.starts_with(**o)) else {
                return Err(Diag::error(pos, format!("invalid character '{c}'")));
            };
            self.i += op.chars().count();
            // the NOT and OR symbols and their alternatives
            let op: &'static str = match *op {
                "^" | "~" => "¬",
                "^=" | "~=" | "<>" => "¬=",
                "^<" | "~<" => "¬<",
                "^>" | "~>" => "¬>",
                "!" => "|",
                "!!" => "||",
                o => o,
            };
            self.push(Tok::Op(op), pos);
        }
    }

    /// `EXEC SQL|CICS|DLI text;`: the kind as a name and the text up to
    /// the semicolon (outside quotes) as a character string.
    fn exec_text(&mut self) -> Result<()> {
        let save = (self.i, self.line);
        self.skip_blanks()?;
        let pos = self.pos();
        let start = self.i;
        while self.peek(0).is_some_and(ident_char) {
            self.i += 1;
        }
        let kind: String = self.c[start..self.i]
            .iter()
            .collect::<String>()
            .to_ascii_uppercase();
        if !matches!(kind.as_str(), "SQL" | "CICS" | "DLI") {
            (self.i, self.line) = save;
            return Ok(());
        }
        self.push(Tok::Ident(kind), pos);
        let tpos = self.pos();
        let mut text = String::new();
        let mut quote = None;
        loop {
            let Some(c) = self.peek(0) else {
                return Err(Diag::error(
                    tpos,
                    "';' expected at the end of the EXEC statement",
                ));
            };
            self.i += 1;
            match (quote, c) {
                (None, ';') => break,
                (None, '\'' | '"') => quote = Some(c),
                (Some(q), c) if c == q => quote = None,
                _ => {}
            }
            if c == '\n' {
                self.line += 1;
                text.push(' ');
            } else {
                text.push(c);
            }
        }
        self.push(
            Tok::Str {
                text: text.trim().to_string(),
                kind: StrKind::Char,
            },
            tpos,
        );
        let end = self.pos();
        self.push(Tok::Op(";"), end);
        Ok(())
    }

    fn number(&mut self, pos: Pos) {
        let start = self.i;
        while self.peek(0).is_some_and(|c| c.is_ascii_digit() || c == '_') {
            self.i += 1;
        }
        if self.peek(0) == Some('.') {
            self.i += 1;
            while self.peek(0).is_some_and(|c| c.is_ascii_digit() || c == '_') {
                self.i += 1;
            }
        }
        // exponent: E or S/D/Q (single, double, extended) followed by an
        // optionally signed integer
        if let Some(e) = self.peek(0)
            && matches!(e.to_ascii_uppercase(), 'E' | 'S' | 'D' | 'Q')
        {
            let (a, b) = (self.peek(1), self.peek(2));
            let digit = |c: Option<char>| c.is_some_and(|c| c.is_ascii_digit());
            if digit(a) || (matches!(a, Some('+' | '-')) && digit(b)) {
                self.i += 2;
                while self.peek(0).is_some_and(|c| c.is_ascii_digit()) {
                    self.i += 1;
                }
            }
        }
        let mut text: String = self.c[start..self.i]
            .iter()
            .filter(|&&c| c != '_')
            .collect::<String>()
            .to_ascii_uppercase();
        for x in ["S", "D", "Q"] {
            if let Some(p) = text.find(x) {
                text.replace_range(p..p + 1, "E");
            }
        }
        let binary =
            matches!(self.peek(0), Some('B' | 'b')) && !self.peek(1).is_some_and(ident_char);
        if binary {
            self.i += 1;
        }
        self.push(Tok::Num { text, binary }, pos);
    }

    fn string(&mut self, q: char, pos: Pos) -> Result<()> {
        self.i += 1;
        let mut text = String::new();
        loop {
            match self.peek(0) {
                None => return Err(Diag::error(pos, "string constant not closed")),
                Some(c) if c == q && self.peek(1) == Some(q) => {
                    text.push(q);
                    self.i += 2;
                }
                Some(c) if c == q => {
                    self.i += 1;
                    break;
                }
                Some('\n') => {
                    self.line += 1;
                    self.i += 1;
                }
                Some(c) => {
                    text.push(c);
                    self.i += 1;
                }
            }
        }
        // suffix
        let mut suffix = String::new();
        while let Some(c) = self.peek(0)
            && ident_char(c)
            && suffix.len() < 2
        {
            suffix.push(c.to_ascii_uppercase());
            self.i += 1;
        }
        let kind = match suffix.as_str() {
            "" => StrKind::Char,
            "X" => StrKind::Hex,
            "B" | "B1" => StrKind::Bit(1),
            "B2" => StrKind::Bit(2),
            "B3" => StrKind::Bit(3),
            "B4" | "BX" => StrKind::Bit(4),
            "XN" => StrKind::HexBin,
            "XU" => StrKind::HexUnsigned,
            "E" | "A" => StrKind::Char,
            s => return Err(Diag::error(pos, format!("invalid string suffix {s}"))),
        };
        self.push(Tok::Str { text, kind }, pos);
        Ok(())
    }

    /// A % statement: %INCLUDE inserts a member; listing statements are
    /// ignored.
    fn percent(&mut self, pos: Pos) -> Result<()> {
        self.skip_blanks()?;
        let start = self.i;
        while self.peek(0).is_some_and(ident_char) {
            self.i += 1;
        }
        let w: String = self.c[start..self.i]
            .iter()
            .collect::<String>()
            .to_ascii_uppercase();
        match w.as_str() {
            "PAGE" | "SKIP" | "PRINT" | "NOPRINT" | "PUSH" | "POP" | "PROCESS" => {
                while let Some(c) = self.peek(0)
                    && c != ';'
                {
                    if c == '\n' {
                        self.line += 1;
                    }
                    self.i += 1;
                }
                self.i += 1;
                Ok(())
            }
            _ => {
                // a preprocessor statement: the preprocessor executes it
                self.i = start;
                self.push(Tok::Op("%"), pos);
                Ok(())
            }
        }
    }
}

/// Finds an include member: the name as given, upper and lower case, with
/// or without the extensions .pli, .inc, .cpy.
pub fn find_include(dirs: &[PathBuf], member: &str) -> Option<PathBuf> {
    let direct = Path::new(member);
    if direct.is_absolute() && direct.is_file() {
        return Some(direct.to_path_buf());
    }
    for d in dirs {
        for name in [
            member.to_string(),
            member.to_ascii_uppercase(),
            member.to_ascii_lowercase(),
        ] {
            for ext in ["", ".pli", ".inc", ".cpy", ".PLI", ".INC", ".CPY"] {
                let p = d.join(format!("{name}{ext}"));
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Tokens of a source file (with included members).
pub fn tokenize(path: &Path, opts: &LexOptions, files: &mut Files) -> Result<Vec<Token>> {
    let text = std::fs::read(path)
        .map_err(|e| Diag::error(Pos::default(), format!("{}: {e}", path.display())))?;
    let text = String::from_utf8_lossy(&text).into_owned();
    tokenize_text(&text, &path.display().to_string(), opts, files)
}

pub fn tokenize_text(
    text: &str,
    name: &str,
    opts: &LexOptions,
    files: &mut Files,
) -> Result<Vec<Token>> {
    let (prepared, _) = prepare(text, opts);
    files.names.push(name.to_string());
    let file = (files.names.len() - 1) as u16;
    let mut lx = Lexer {
        c: prepared.chars().collect(),
        i: 0,
        line: 1,
        file,
        out: Vec::new(),
    };
    lx.run()?;
    let pos = Pos {
        file,
        line: lx.line,
    };
    let mut out = lx.out;
    out.push(Token { tok: Tok::Eof, pos });
    Ok(out)
}

/// Tokens of text without margins (preprocessor replacement values),
/// positioned at `pos`; no end-of-file token.
pub fn tokenize_raw(text: &str, pos: Pos) -> Result<Vec<Token>> {
    let mut lx = Lexer {
        c: text.chars().collect(),
        i: 0,
        line: pos.line,
        file: pos.file,
        out: Vec::new(),
    };
    lx.run()?;
    Ok(lx.out)
}

/// The source text of tokens (for preprocessor arguments and values).
pub fn tokens_text(toks: &[Token]) -> String {
    let mut out = String::new();
    for t in toks {
        if !out.is_empty() {
            out.push(' ');
        }
        match &t.tok {
            Tok::Str { text, kind } => {
                out.push('\'');
                out.push_str(&text.replace('\'', "''"));
                out.push('\'');
                out.push_str(match kind {
                    StrKind::Char => "",
                    StrKind::Hex => "X",
                    StrKind::Bit(1) => "B",
                    StrKind::Bit(2) => "B2",
                    StrKind::Bit(3) => "B3",
                    StrKind::Bit(_) => "B4",
                    StrKind::HexBin => "XN",
                    StrKind::HexUnsigned => "XU",
                });
            }
            other => out.push_str(&other.to_string()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        let mut files = Files::default();
        tokenize_text(src, "t", &LexOptions::default(), &mut files)
            .unwrap()
            .into_iter()
            .map(|t| t.tok)
            .collect()
    }

    #[test]
    fn tokens() {
        let t = toks(" X = 'IT''S' || 'C1'X; /* c */ Y(1)->Z ^= 1.5E3;\n");
        assert_eq!(t[0], Tok::Ident("X".into()));
        assert_eq!(
            t[2],
            Tok::Str {
                text: "IT'S".into(),
                kind: StrKind::Char
            }
        );
        assert_eq!(t[3], Tok::Op("||"));
        assert!(t.contains(&Tok::Op("->")));
        assert!(t.contains(&Tok::Op("¬=")));
        assert!(t.contains(&Tok::Num {
            text: "1.5E3".into(),
            binary: false
        }));
        // margins: column 1 is ignored
        assert_eq!(toks("XA = 1;")[0], Tok::Ident("A".into()));
        assert!(toks(" B = 101B;").contains(&Tok::Num {
            text: "101".into(),
            binary: true
        }));
    }
}

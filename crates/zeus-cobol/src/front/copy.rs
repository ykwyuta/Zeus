//! Text manipulation: the COPY and REPLACE statements, applied to the token
//! stream before parsing.

use super::lexer::{Tok, Token};
use crate::diag::{Diag, Result};

/// Loads the tokens of a library text: `(text-name, library-name)`.
/// Loads and lexes a COPY member: name, library, the COPY token and the
/// partial-word replacements to apply to its text first.
pub type Loader<'a> =
    dyn FnMut(&str, Option<&str>, &Token, &[(String, String)]) -> Result<Vec<Token>> + 'a;

/// A replacement pair: tokens to match and their replacement.
struct Pair {
    from: Vec<Token>,
    to: Vec<Token>,
    /// None: whole text; Some(true): LEADING; Some(false): TRAILING.
    edge: Option<bool>,
}

fn partial_word(tokens: &[Token]) -> Option<String> {
    match tokens {
        [
            Token {
                tok: Tok::Word(w) | Tok::Num(w) | Tok::Pic(w),
                ..
            },
        ] if w.is_ascii() => Some(w.clone()),
        [
            Token {
                tok: Tok::Minus, ..
            },
            Token {
                tok: Tok::Word(w), ..
            },
        ] if w.is_ascii() => Some(format!("-{w}")),
        [
            Token {
                tok: Tok::Minus, ..
            },
        ] => Some("-".into()),
        [] => Some(String::new()),
        _ => None,
    }
}

fn same(a: &Token, b: &Token) -> bool {
    match (&a.tok, &b.tok) {
        (Tok::Pseudo(x), Tok::Pseudo(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| same(p, q))
        }
        (x, y) => x == y,
    }
}

fn matches_at(text: &[Token], at: usize, pattern: &[Token]) -> bool {
    !pattern.is_empty()
        && text.len() >= at + pattern.len()
        && text[at..at + pattern.len()]
            .iter()
            .zip(pattern)
            .all(|(a, b)| same(a, b))
}

/// Applies replacement pairs to a token sequence (each source token is
/// replaced at most once; pairs are tried in order).
fn replace(text: Vec<Token>, pairs: &[Pair]) -> Result<Vec<Token>> {
    if pairs.is_empty() {
        return Ok(text);
    }
    let mut out = Vec::with_capacity(text.len());
    let mut i = 0;
    while i < text.len() {
        let mut matched = false;
        for Pair { from, to, edge } in pairs {
            if let Some(leading) = edge {
                let Some(word) = partial_word(&text[i..i + 1]) else {
                    continue;
                };
                let pattern = partial_word(from).unwrap();
                let rest = if *leading {
                    word.strip_prefix(&pattern)
                } else {
                    word.strip_suffix(&pattern)
                };
                let Some(rest) = rest else { continue };
                let by = partial_word(to).unwrap();
                let value = if *leading {
                    format!("{by}{rest}")
                } else {
                    format!("{rest}{by}")
                };
                if value.is_empty() {
                    // Deleting an entire text word must not leave an empty
                    // PICTURE token that could masquerade as a zero-size field.
                } else if matches!(text[i].tok, Tok::Pic(_)) {
                    out.push(Token {
                        tok: Tok::Pic(value),
                        pos: text[i].pos,
                    });
                } else {
                    let stream: Vec<_> = value.chars().map(|c| (c, text[i].pos)).collect();
                    out.extend(super::lexer::lex(&stream, false)?);
                }
                i += 1;
                matched = true;
                break;
            }
            if !matches_at(&text, i, from) {
                continue;
            }
            let pos = text[i].pos;
            out.extend(to.iter().cloned().map(|mut t| {
                t.pos = pos;
                t
            }));
            i += from.len();
            matched = true;
            break;
        }
        if !matched {
            out.push(text[i].clone());
            i += 1;
        }
    }
    Ok(out)
}

/// An operand of REPLACING / REPLACE: pseudo-text, or a single literal or
/// word (an identifier with qualification is taken token by token up to BY
/// or the next operand).
fn operand(tokens: &[Token], i: &mut usize, stop: &[&str]) -> Result<Vec<Token>> {
    let t = tokens
        .get(*i)
        .ok_or_else(|| Diag::error(tokens[*i - 1].pos, "REPLACING operand expected"))?;
    if let Tok::Pseudo(inner) = &t.tok {
        *i += 1;
        return Ok(inner.clone());
    }
    let mut out = vec![t.clone()];
    *i += 1;
    // identifier: name { OF|IN name } [ ( subscripts ) ]
    while let Some(t) = tokens.get(*i) {
        if t.is_word("OF") || t.is_word("IN") {
            out.push(t.clone());
            if let Some(n) = tokens.get(*i + 1) {
                out.push(n.clone());
            }
            *i += 2;
        } else if t.tok == Tok::LParen && out.last().is_some_and(|l| matches!(l.tok, Tok::Word(_)))
        {
            let mut depth = 0;
            while let Some(t) = tokens.get(*i) {
                out.push(t.clone());
                *i += 1;
                match t.tok {
                    Tok::LParen => depth += 1,
                    Tok::RParen => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        } else {
            break;
        }
        if tokens
            .get(*i)
            .is_some_and(|t| stop.iter().any(|s| t.is_word(s)))
        {
            break;
        }
    }
    Ok(out)
}

fn replacing_pairs(tokens: &[Token], i: &mut usize, end_period: bool) -> Result<Vec<Pair>> {
    let mut pairs = Vec::new();
    loop {
        match tokens.get(*i) {
            Some(t) if t.tok == Tok::Period => {
                if end_period {
                    *i += 1;
                }
                break;
            }
            None => {
                return Err(Diag::error(
                    tokens[*i - 1].pos,
                    "REPLACE statement must end with a period",
                ));
            }
            Some(_) => {}
        }
        let edge = match tokens.get(*i).and_then(Token::word) {
            Some("LEADING") => {
                *i += 1;
                Some(true)
            }
            Some("TRAILING") => {
                *i += 1;
                Some(false)
            }
            _ => None,
        };
        let pos = tokens.get(*i).map(|t| t.pos).unwrap_or_default();
        if edge.is_some() && !matches!(tokens.get(*i).map(|t| &t.tok), Some(Tok::Pseudo(_))) {
            return Err(Diag::error(
                pos,
                "partial-word operand requires pseudo-text delimiters",
            ));
        }
        let from = operand(tokens, i, &["BY"])?;
        if from.is_empty() {
            return Err(Diag::error(pos, "empty replacement search operand"));
        }
        match tokens.get(*i) {
            Some(t) if t.is_word("BY") => *i += 1,
            Some(t) => return Err(Diag::error(t.pos, "BY expected in REPLACING")),
            None => return Err(Diag::error(tokens[*i - 1].pos, "BY expected")),
        }
        if edge.is_some() && !matches!(tokens.get(*i).map(|t| &t.tok), Some(Tok::Pseudo(_))) {
            return Err(Diag::error(
                pos,
                "partial-word operand requires pseudo-text delimiters",
            ));
        }
        let to = operand(tokens, i, &[])?;
        if edge.is_some() && (partial_word(&from).is_none() || partial_word(&to).is_none()) {
            return Err(Diag::error(
                pos,
                "partial-word replacement requires one SBCS text word",
            ));
        }
        pairs.push(Pair { from, to, edge });
    }
    Ok(pairs)
}

/// Expands COPY statements (recursively) in `tokens`.
pub fn expand_copy(tokens: Vec<Token>, load: &mut Loader, depth: usize) -> Result<Vec<Token>> {
    if depth > 20 {
        let pos = tokens.first().map(|t| t.pos).unwrap_or_default();
        return Err(Diag::error(pos, "COPY nesting too deep"));
    }
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        let t = &tokens[i];
        if let Some(name) = sql_include(t) {
            // EXEC SQL INCLUDE member END-EXEC: the member's text, as COPY;
            // the period ending the entry belongs to the member.
            i += 1;
            if tokens.get(i).is_some_and(|t| t.tok == Tok::Period) {
                i += 1;
            }
            let text = load(&name, None, t, &[])?;
            out.extend(expand_copy(text, load, depth + 1)?);
            continue;
        }
        if !t.is_word("COPY") {
            out.push(t.clone());
            i += 1;
            continue;
        }
        let copy_tok = t.clone();
        i += 1;
        let name = match tokens.get(i).map(|t| &t.tok) {
            Some(Tok::Word(w)) => w.clone(),
            Some(Tok::Str(s)) => s.clone(),
            _ => return Err(Diag::error(copy_tok.pos, "text-name expected after COPY")),
        };
        i += 1;
        let mut library = None;
        if tokens
            .get(i)
            .is_some_and(|t| t.is_word("OF") || t.is_word("IN"))
        {
            library = match tokens.get(i + 1).map(|t| &t.tok) {
                Some(Tok::Word(w)) => Some(w.clone()),
                Some(Tok::Str(s)) => Some(s.clone()),
                _ => return Err(Diag::error(copy_tok.pos, "library-name expected")),
            };
            i += 2;
        }
        if tokens.get(i).is_some_and(|t| t.is_word("SUPPRESS")) {
            i += 1;
        }
        let mut pairs = Vec::new();
        if tokens.get(i).is_some_and(|t| t.is_word("REPLACING")) {
            i += 1;
            pairs = replacing_pairs(&tokens, &mut i, false)?;
        }
        match tokens.get(i) {
            Some(t) if t.tok == Tok::Period => i += 1,
            _ => {
                return Err(Diag::error(
                    copy_tok.pos,
                    "COPY statement must end with a period",
                ));
            }
        }
        // `(tag)` and `:tag:` pseudo-text replace part of a word: done on
        // the member's text.
        let mut partial = Vec::new();
        pairs.retain(|Pair { from, to, edge }| {
            if edge.is_some() {
                return true;
            }
            let tag = match from.as_slice() {
                [
                    a,
                    Token {
                        tok: Tok::Word(w), ..
                    },
                    b,
                ] if (a.tok == Tok::LParen && b.tok == Tok::RParen)
                    || (a.tok == Tok::Colon && b.tok == Tok::Colon) =>
                {
                    if a.tok == Tok::LParen {
                        format!("({w})")
                    } else {
                        format!(":{w}:")
                    }
                }
                _ => return true,
            };
            let by = to.iter().map(Token::text).collect::<Vec<_>>().join(" ");
            partial.push((tag, by));
            false
        });
        let text = load(&name, library.as_deref(), &copy_tok, &partial)?;
        let text = expand_copy(text, load, depth + 1)?;
        out.extend(replace(text, &pairs)?);
    }
    Ok(out)
}

/// The member named by `EXEC SQL INCLUDE member END-EXEC`.
fn sql_include(t: &Token) -> Option<String> {
    let Tok::Exec(kind, text) = &t.tok else {
        return None;
    };
    if kind != "SQL" {
        return None;
    }
    let text: String = text.iter().map(|(c, _)| *c).collect();
    let mut words = text.split_whitespace();
    if !words.next()?.eq_ignore_ascii_case("INCLUDE") {
        return None;
    }
    let name = words.next()?.trim_matches(|c| c == '\'' || c == '"');
    words.next().is_none().then(|| name.to_ascii_uppercase())
}

/// Applies REPLACE statements: each REPLACE ... . replaces the pairs in the
/// following text until the next REPLACE (or REPLACE OFF).
pub fn apply_replace(tokens: Vec<Token>) -> Result<Vec<Token>> {
    if !tokens.iter().any(|t| t.is_word("REPLACE")) {
        return Ok(tokens);
    }
    let mut out = Vec::with_capacity(tokens.len());
    let mut pairs: Vec<Pair> = Vec::new();
    let mut segment: Vec<Token> = Vec::new();
    let mut i = 0;
    let mut program_depth = 0usize;
    while i < tokens.len() {
        let t = &tokens[i];
        if (t.is_word("IDENTIFICATION") || t.is_word("ID"))
            && tokens.get(i + 1).is_some_and(|t| t.is_word("DIVISION"))
        {
            program_depth += 1;
        }
        if t.is_word("END")
            && tokens.get(i + 1).is_some_and(|t| t.is_word("PROGRAM"))
            && tokens.get(i + 3).is_some_and(|t| t.tok == Tok::Period)
        {
            segment.extend_from_slice(&tokens[i..i + 4]);
            i += 4;
            program_depth = program_depth.saturating_sub(1);
            if program_depth == 0 {
                out.extend(replace(std::mem::take(&mut segment), &pairs)?);
                pairs.clear();
            }
            continue;
        }
        // REPLACE starts a statement only when followed by pseudo-text or OFF.
        let starts = t.is_word("REPLACE")
            && tokens.get(i + 1).is_some_and(|n| {
                matches!(n.tok, Tok::Pseudo(_))
                    || n.is_word("OFF")
                    || n.is_word("LEADING")
                    || n.is_word("TRAILING")
            });
        if !starts {
            segment.push(t.clone());
            i += 1;
            continue;
        }
        out.extend(replace(std::mem::take(&mut segment), &pairs)?);
        i += 1;
        if tokens[i].is_word("OFF") {
            pairs.clear();
            i += 1;
            if !tokens.get(i).is_some_and(|t| t.tok == Tok::Period) {
                return Err(Diag::error(t.pos, "REPLACE OFF must end with a period"));
            }
            i += 1;
        } else {
            pairs = replacing_pairs(&tokens, &mut i, true)?;
        }
    }
    out.extend(replace(segment, &pairs)?);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::front::lexer::lex;
    use crate::front::source::Pos;

    fn toks(s: &str) -> Vec<Token> {
        let stream: Vec<(char, Pos)> = s.chars().map(|c| (c, Pos::default())).collect();
        lex(&stream, false).unwrap()
    }

    fn text(t: &[Token]) -> String {
        t.iter().map(Token::text).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn copy_with_replacing() {
        let src = toks("A. COPY LIB1 REPLACING ==X== BY ==Y Z== \"Q\" BY 5. B.");
        let mut load = |name: &str, lib: Option<&str>, _: &Token, _: &[(String, String)]| {
            assert_eq!(name, "LIB1");
            assert_eq!(lib, None);
            Ok(toks("MOVE X TO \"Q\" X."))
        };
        let out = expand_copy(src, &mut load, 0).unwrap();
        assert_eq!(text(&out), "A . MOVE Y Z TO 5 Y Z . B .");
    }

    #[test]
    fn nested_copy_and_library() {
        let src = toks("COPY A OF \"../lib\".");
        let mut load = |name: &str, lib: Option<&str>, _: &Token, _: &[(String, String)]| match name
        {
            "A" => {
                assert_eq!(lib, Some("../lib"));
                Ok(toks("1 COPY B. 2"))
            }
            _ => Ok(toks("X")),
        };
        let out = expand_copy(src, &mut load, 0).unwrap();
        assert_eq!(text(&out), "1 X 2");
    }

    #[test]
    fn replace_statement() {
        let out = apply_replace(toks("A B. REPLACE ==B== BY ==C==. A B. REPLACE OFF. B.")).unwrap();
        assert_eq!(text(&out), "A B . A C . B .");
    }
}

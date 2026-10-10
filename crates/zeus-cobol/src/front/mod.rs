//! Front end: source reading, text manipulation, lexing and parsing.

pub mod ast;
pub mod copy;
pub mod lexer;
pub mod parser;
pub mod source;

use std::path::{Path, PathBuf};

use crate::diag::{Diag, Result};
use lexer::Token;

/// Files read during a compilation (index = `Pos::file`).
#[derive(Debug, Default)]
pub struct FileTable {
    pub names: Vec<String>,
}

/// Options affecting the front end.
#[derive(Debug, Clone, Default)]
pub struct FrontOptions {
    /// Directories searched for COPY members.
    pub copy_dirs: Vec<PathBuf>,
}

/// Source-wide settings that must be known before lexing: WITH DEBUGGING
/// MODE (debugging lines) and DECIMAL-POINT IS COMMA.
fn prescan(text: &str) -> (bool, bool) {
    let mut debugging = false;
    let mut comma = false;
    for line in text.lines() {
        let ind = line.chars().nth(6).unwrap_or(' ');
        if ind == '*' || ind == '/' {
            continue;
        }
        let area: String = line
            .chars()
            .skip(7)
            .take(65)
            .collect::<String>()
            .to_ascii_uppercase();
        if area.contains("DEBUGGING") && area.contains("MODE") {
            debugging = true;
        }
        if area.contains("DECIMAL-POINT") {
            comma = true;
        }
        if area.contains("PROCEDURE") && area.contains("DIVISION") {
            break;
        }
    }
    (debugging, comma)
}

fn find_copy_member(
    dirs: &[PathBuf],
    base: &Path,
    name: &str,
    lib: Option<&str>,
) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let mut search: Vec<PathBuf> = Vec::new();
    if let Some(lib) = lib {
        let l = Path::new(lib);
        search.push(if l.is_absolute() {
            l.to_path_buf()
        } else {
            base.join(l)
        });
        for d in dirs {
            search.push(d.join(lib));
        }
    }
    search.push(base.to_path_buf());
    search.extend(dirs.iter().cloned());
    for dir in search {
        for ext in ["", ".cpy", ".CPY", ".cbl", ".CBL", ".cob", ".COB"] {
            candidates.push(dir.join(format!("{name}{ext}")));
        }
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// Reads, preprocesses and parses a source file.
pub fn parse_file(
    path: &Path,
    opts: &FrontOptions,
    files: &mut FileTable,
) -> Result<Vec<ast::Program>> {
    let text = std::fs::read(path).map_err(|e| {
        Diag::error(
            Default::default(),
            format!("cannot read {}: {e}", path.display()),
        )
    })?;
    let text = String::from_utf8_lossy(&text).into_owned();
    let (debugging, comma) = prescan(&text);
    let tokens = preprocess(path, &text, opts, files, debugging, comma)?;
    // Settings that only appear in copied text require a second pass.
    let has = |w: &str| tokens.iter().any(|t| t.is_word(w));
    let comma2 = comma || has("DECIMAL-POINT");
    let debugging2 = debugging
        || tokens
            .windows(2)
            .any(|p| p[0].is_word("DEBUGGING") && p[1].is_word("MODE"));
    let tokens = if comma2 != comma || debugging2 != debugging {
        files.names.clear();
        preprocess(path, &text, opts, files, debugging2, comma2)?
    } else {
        tokens
    };
    parser::parse(tokens)
}

fn preprocess(
    path: &Path,
    text: &str,
    opts: &FrontOptions,
    files: &mut FileTable,
    debugging: bool,
    comma: bool,
) -> Result<Vec<Token>> {
    files.names.push(path.display().to_string());
    let stream = source::read(text, 0, debugging);
    let tokens = lexer::lex(&stream, comma)?;
    let base = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let mut load = |name: &str, lib: Option<&str>, at: &Token| -> Result<Vec<Token>> {
        let member = find_copy_member(&opts.copy_dirs, &base, name, lib)
            .ok_or_else(|| Diag::error(at.pos, format!("COPY member {name} not found")))?;
        let text = std::fs::read(&member)
            .map_err(|e| Diag::error(at.pos, format!("cannot read {}: {e}", member.display())))?;
        let text = String::from_utf8_lossy(&text).into_owned();
        let file = files.names.len() as u32;
        files.names.push(member.display().to_string());
        let stream = source::read(&text, file, debugging);
        lexer::lex(&stream, comma)
    };
    let tokens = copy::expand_copy(tokens, &mut load, 0)?;
    copy::apply_replace(tokens)
}

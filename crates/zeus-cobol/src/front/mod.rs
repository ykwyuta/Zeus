//! Front end: source reading, text manipulation, lexing and parsing.

pub mod ast;
pub mod cics;
pub mod copy;
pub mod lexer;
pub mod parser;
pub mod source;
pub mod syslib;
mod syslib_mq;

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
    /// Compile debugging lines (D in column 7) without WITH DEBUGGING MODE.
    pub debugging_lines: bool,
    /// Translate as a CICS program (the CICS compiler option); a source
    /// with EXEC CICS commands is always translated.
    pub cics: bool,
    /// NSYMBOL(DBCS): N literals and PICTURE N are DBCS (default
    /// NSYMBOL(NATIONAL): national, UTF-16).
    pub nsymbol_dbcs: bool,
}

static NSYMBOL_DBCS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// NSYMBOL(DBCS) in effect for the compilation (`FrontOptions`).
pub fn nsymbol_dbcs() -> bool {
    NSYMBOL_DBCS.load(std::sync::atomic::Ordering::Relaxed)
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
    source: &Path,
    name: &str,
    lib: Option<&str>,
) -> Option<PathBuf> {
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
    // the COPY libraries, then the directory of the source
    search.extend(dirs.iter().cloned());
    search.push(base.to_path_buf());
    let lower = name.to_ascii_lowercase();
    let mut candidates: Vec<PathBuf> = Vec::new();
    for dir in search {
        for n in [name, lower.as_str()] {
            for ext in [
                "", ".cpy", ".CPY", ".cbl", ".CBL", ".cob", ".COB", ".dcl", ".DCL",
            ] {
                candidates.push(dir.join(format!("{n}{ext}")));
            }
        }
    }
    // a program is not its own COPY member
    let source = source.canonicalize().ok();
    candidates
        .into_iter()
        .find(|p| p.is_file() && (source.is_none() || p.canonicalize().ok() != source))
}

/// Replaces `(tag)` / `:tag:` strings (case-insensitive) in the source
/// text of a COPY member.
fn replace_partial(text: &str, partial: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (tag, by) in partial {
        let upper = out.to_ascii_uppercase();
        let mut r = String::with_capacity(out.len());
        let mut last = 0;
        for (i, _) in upper.match_indices(tag.as_str()) {
            r.push_str(&out[last..i]);
            r.push_str(by);
            last = i + tag.len();
        }
        r.push_str(&out[last..]);
        out = r;
    }
    out
}

/// Takes the CBL / PROCESS statements (compiler options) off the start of
/// a source; returns the remaining text and the option text.
fn process_statements(text: &str) -> (String, String) {
    let mut options = String::new();
    let mut out = String::with_capacity(text.len());
    let mut leading = true;
    for line in text.split_inclusive('\n') {
        if leading {
            let ind = line.chars().nth(6).unwrap_or(' ');
            let body: String = line.chars().take(72).collect();
            let t = body.trim_start().to_ascii_uppercase();
            let is_process = (t.starts_with("CBL ") || t.starts_with("PROCESS ") || t == "CBL")
                && ind != '*'
                && ind != '/';
            // sequence-numbered lines: the statement starts after column 7
            let t7: String = body
                .chars()
                .skip(7)
                .collect::<String>()
                .trim_start()
                .to_ascii_uppercase();
            let is_process = is_process || t7.starts_with("CBL ") || t7.starts_with("PROCESS ");
            if is_process {
                options.push_str(&t);
                options.push(' ');
                out.push('\n');
                continue;
            }
            if !(body.trim().is_empty() || ind == '*' || ind == '/') {
                leading = false;
            }
        }
        out.push_str(line);
    }
    (out, options)
}

/// Reads, preprocesses and parses a source file.
pub fn parse_file(
    path: &Path,
    opts: &FrontOptions,
    files: &mut FileTable,
) -> Result<Vec<ast::Program>> {
    NSYMBOL_DBCS.store(opts.nsymbol_dbcs, std::sync::atomic::Ordering::Relaxed);
    let (tokens, cics_option) = source_tokens(path, opts, files)?;
    let cics = opts.cics || cics_option || cics::uses(&tokens, "CICS");
    let dli = cics::uses(&tokens, "DLI");
    let mut programs = parser::parse(tokens)?;
    for p in &mut programs {
        if cics {
            cics::insert_eib(p)?;
        }
        if dli {
            cics::insert_dib(p)?;
        }
    }
    Ok(programs)
}

/// The tokens of a source file after COPY and REPLACE, and whether its
/// CBL/PROCESS statements name the CICS option.
pub fn source_tokens(
    path: &Path,
    opts: &FrontOptions,
    files: &mut FileTable,
) -> Result<(Vec<Token>, bool)> {
    let text = std::fs::read(path).map_err(|e| {
        Diag::error(
            Default::default(),
            format!("cannot read {}: {e}", path.display()),
        )
    })?;
    let text = String::from_utf8_lossy(&text).into_owned();
    let (text, options) = process_statements(&text);
    let cics_option = options
        .split(|c: char| !c.is_ascii_alphanumeric())
        .any(|w| w == "CICS");
    let (debugging, comma) = prescan(&text);
    let debugging = debugging || opts.debugging_lines;
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
    Ok((tokens, cics_option))
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
    let mut load = |name: &str,
                    lib: Option<&str>,
                    at: &Token,
                    partial: &[(String, String)]|
     -> Result<Vec<Token>> {
        let (text, path) = match find_copy_member(&opts.copy_dirs, &base, path, name, lib) {
            Some(member) => {
                let text = std::fs::read(&member).map_err(|e| {
                    Diag::error(at.pos, format!("cannot read {}: {e}", member.display()))
                })?;
                (
                    String::from_utf8_lossy(&text).into_owned(),
                    member.display().to_string(),
                )
            }
            None => match syslib::member(&name.to_ascii_uppercase()) {
                Some(text) => (text, format!("<system>/{name}")),
                None => {
                    return Err(Diag::error(at.pos, format!("COPY member {name} not found")));
                }
            },
        };
        let text = replace_partial(&text, partial);
        let file = files.names.len() as u32;
        files.names.push(path);
        let stream = source::read(&text, file, debugging);
        lexer::lex(&stream, comma)
    };
    let tokens = copy::expand_copy(tokens, &mut load, 0)?;
    let tokens = copy::apply_replace(tokens)?;
    cics::translate(tokens)
}

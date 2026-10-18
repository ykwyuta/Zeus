//! Front end: source reading, text manipulation, lexing and parsing.

pub mod ast;
pub mod cics;
mod conditional;
pub mod copy;
pub mod input;
pub mod lexer;
mod options;
pub mod parser;
pub mod source;
pub mod syslib;
mod syslib_mq;

use std::path::{Path, PathBuf};

use crate::diag::{Diag, Result};
pub use input::SourceEncoding;
use lexer::Token;

/// Files read during a compilation (index = `Pos::file`).
#[derive(Debug, Default)]
pub struct FileTable {
    pub names: Vec<String>,
    pub text: std::collections::HashMap<String, String>,
}

/// Options affecting the front end.
#[derive(Debug, Clone, Default)]
pub struct FrontOptions {
    pub source_encoding: SourceEncoding,
    /// External COPY members with a different physical representation.
    pub copy_encodings: std::collections::HashMap<PathBuf, SourceEncoding>,
    /// Invocation options, overridden by CBL/PROCESS statements.
    pub compiler_options: String,
    pub listing: crate::listing::ListingOptions,
    pub optimize: u8,
    /// None keeps the integrated translator's automatic SQL extension.
    pub sql: Option<bool>,
    pub sqlims: bool,
    pub apost: bool,
    pub awo: bool,
    pub block0: bool,
    pub ssrange: bool,
    pub codepage: Option<u32>,
    pub dynam: bool,
    pub dll: bool,
    pub exportall: bool,
    pub thread: bool,
    pub lp64: bool,
    /// Values imported by >>DEFINE name AS PARAMETER.
    pub defines: std::collections::HashMap<String, String>,
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
    pub trunc: Trunc,
    pub numeric: crate::numeric::Settings,
}

/// Binary truncation selected by CBL/PROCESS TRUNC.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Trunc {
    #[default]
    Std,
    Opt,
    Bin,
}

thread_local! {
    static LP64: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static THREAD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static DLL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static EXPORTALL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static OPTIMIZE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static LISTING: std::cell::RefCell<crate::listing::ListingOptions> = std::cell::RefCell::new(crate::listing::ListingOptions::default());
    static DYNAM: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static COMPILE_CCSID: std::cell::Cell<u32> = const { std::cell::Cell::new(37) };
    static APOST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static AWO: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static BLOCK0: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static SSRANGE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static NSYMBOL_DBCS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static TRUNC: std::cell::Cell<Trunc> = const { std::cell::Cell::new(Trunc::Std) };
    static NUMERIC: std::cell::Cell<crate::numeric::Settings> = const { std::cell::Cell::new(crate::numeric::Settings {
        arithmetic: crate::numeric::Arithmetic::Compat, numproc: crate::numeric::Numproc::Nopfd, numcls: crate::numeric::Numcls::Primary,
    }) };
}

pub(crate) fn trunc() -> Trunc {
    TRUNC.get()
}
pub(crate) fn numeric() -> crate::numeric::Settings {
    NUMERIC.get()
}
pub(crate) fn apost() -> bool {
    APOST.get()
}
pub(crate) fn awo() -> bool {
    AWO.get()
}
pub(crate) fn block0() -> bool {
    BLOCK0.get()
}
pub(crate) fn ssrange() -> bool {
    SSRANGE.get()
}
pub(crate) fn dynam() -> bool {
    DYNAM.get()
}
pub(crate) fn dll() -> bool {
    DLL.get()
}
pub(crate) fn exportall() -> bool {
    EXPORTALL.get()
}
pub(crate) fn thread() -> bool {
    THREAD.get()
}
pub(crate) fn lp64() -> bool {
    LP64.get()
}
pub(crate) fn optimize() -> u8 {
    OPTIMIZE.get()
}
pub(crate) fn listing_options() -> crate::listing::ListingOptions {
    LISTING.with_borrow(Clone::clone)
}
pub(crate) fn compilation_ccsid() -> u32 {
    COMPILE_CCSID.get()
}

/// NSYMBOL(DBCS) in effect for the compilation (`FrontOptions`).
pub fn nsymbol_dbcs() -> bool {
    NSYMBOL_DBCS.get()
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
    // Compiler directives are processed before text replacement and must
    // retain their literal values even inside COPY REPLACING members.
    if text.lines().any(|line| {
        line.chars()
            .skip(7)
            .collect::<String>()
            .trim_start()
            .starts_with(">>")
    }) {
        return text
            .split_inclusive('\n')
            .map(|line| {
                if line
                    .chars()
                    .skip(7)
                    .collect::<String>()
                    .trim_start()
                    .starts_with(">>")
                {
                    line.to_string()
                } else {
                    replace_partial(line, partial)
                }
            })
            .collect();
    }
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
            let is_process = (is_process || t7.starts_with("CBL ") || t7.starts_with("PROCESS "))
                && ind != '*'
                && ind != '/';
            if is_process {
                let statement = if t.starts_with("CBL ") || t.starts_with("PROCESS ") || t == "CBL"
                {
                    body.trim_start().to_string()
                } else {
                    body.chars()
                        .skip(7)
                        .collect::<String>()
                        .trim_start()
                        .to_string()
                };
                options.push_str(statement.split_once(' ').map_or("", |(_, rest)| rest));
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
    let (tokens, cics_option) = source_tokens(path, opts, files)?;
    parse_tokens(tokens, cics_option)
}

pub(crate) fn parse_tokens(tokens: Vec<Token>, cics_option: bool) -> Result<Vec<ast::Program>> {
    if lp64() && cics::uses(&tokens, "CICS") {
        return Err(Diag::error(
            Default::default(),
            "EXEC CICS is not supported with LP(64)",
        ));
    }
    let cics = cics_option || cics::uses(&tokens, "CICS");
    if cics && dynam() {
        return Err(Diag::error(
            Default::default(),
            "DYNAM cannot be combined with CICS",
        ));
    }
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
    LISTING.set(opts.listing.clone());
    OPTIMIZE.set(opts.optimize);
    let invocation = options::parse(&opts.compiler_options, opts)
        .map_err(|d| d.during(crate::diag::Phase::Options))?
        .apply(opts);
    LISTING.set(invocation.listing.clone());
    OPTIMIZE.set(invocation.optimize);
    let bytes = std::fs::read(path).map_err(|e| {
        Diag::error(
            Default::default(),
            format!("cannot read {}: {e}", path.display()),
        )
        .during(crate::diag::Phase::Options)
    })?;
    let clean_options = |text: String| {
        if opts.source_encoding.padded() {
            input::clean(&text)
        } else {
            text
        }
    };
    // Discover a source CODEPAGE using the invariant compiler-option
    // spelling, then decode the entire source with the selected CCSID.
    let initial_ccsid = invocation
        .codepage
        .unwrap_or_else(|| crate::codepage::active().ccsid);
    let bootstrap = input::decode(&bytes, opts.source_encoding, initial_ccsid, 0, false)?;
    let (_, option_text) = process_statements(&bootstrap);
    let settings = options::parse(&clean_options(option_text), &invocation)
        .map_err(|d| d.during(crate::diag::Phase::Options))?;
    let ccsid = settings
        .codepage
        .unwrap_or_else(|| crate::codepage::active().ccsid);
    let decoded = input::decode(&bytes, opts.source_encoding, ccsid, 0, true)?;
    files.text.insert(
        path.display().to_string(),
        if opts.source_encoding.padded() {
            input::clean(&decoded)
        } else {
            decoded.clone()
        },
    );
    let (text, option_text) = process_statements(&decoded);
    let settings = options::parse(&clean_options(option_text), &invocation)
        .map_err(|d| d.during(crate::diag::Phase::Options))?;
    if settings
        .codepage
        .is_some_and(|c| crate::codepage::get(c).is_none())
    {
        return Err(
            Diag::error(Default::default(), "unsupported CODEPAGE CCSID")
                .during(crate::diag::Phase::Options),
        );
    }
    let _codepage = crate::codepage::scope(settings.codepage);
    COMPILE_CCSID.set(crate::codepage::active().ccsid);
    let cics_option = settings.cics;
    let effective_opts = settings.apply(opts);
    NSYMBOL_DBCS.set(settings.dbcs);
    TRUNC.set(settings.trunc);
    NUMERIC.set(settings.numeric);
    APOST.set(settings.apost);
    AWO.set(settings.awo);
    BLOCK0.set(settings.block0);
    SSRANGE.set(settings.ssrange);
    DYNAM.set(settings.dynam);
    DLL.set(settings.dll);
    EXPORTALL.set(settings.exportall);
    THREAD.set(settings.thread);
    LP64.set(settings.lp64);
    OPTIMIZE.set(settings.optimize);
    LISTING.set(settings.listing);
    // Discover settings from active, expanded tokens. An inactive branch
    // must not enable debugging lines or change the decimal separator.
    let comma = false;
    let debugging = opts.debugging_lines;
    let tokens = preprocess(path, &text, &effective_opts, files, debugging, comma)?;
    // Settings that only appear in copied text require a second pass.
    let has = |w: &str| tokens.iter().any(|t| t.is_word(w));
    let comma2 = comma || has("DECIMAL-POINT");
    let debugging2 = debugging
        || tokens
            .windows(2)
            .any(|p| p[0].is_word("DEBUGGING") && p[1].is_word("MODE"));
    let tokens = if comma2 != comma || debugging2 != debugging {
        files.names.clear();
        preprocess(path, &text, &effective_opts, files, debugging2, comma2)?
    } else {
        tokens
    };
    if effective_opts.sql == Some(false) && cics::uses(&tokens, "SQL") {
        return Err(Diag::error(
            Default::default(),
            "embedded SQL is disabled by NOSQL",
        ));
    }
    if !effective_opts.sqlims && cics::uses(&tokens, "SQLIMS") {
        return Err(Diag::error(
            Default::default(),
            "EXEC SQLIMS requires SQLIMS and LP(32) compiler options",
        ));
    }
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
    let mut variables = conditional::Variables::parameters(opts)
        .map_err(|d| d.during(crate::diag::Phase::Options))?;
    let tokens = expand_conditional_source(
        path,
        text,
        0,
        opts,
        files,
        debugging,
        comma,
        &mut variables,
        0,
        false,
    )?;
    let tokens =
        copy::apply_replace(tokens).map_err(|d| d.during(crate::diag::Phase::Preprocess))?;
    cics::translate(tokens)
}

#[allow(clippy::too_many_arguments)]
fn expand_conditional_source(
    path: &Path,
    text: &str,
    file: u32,
    opts: &FrontOptions,
    files: &mut FileTable,
    debugging: bool,
    comma: bool,
    variables: &mut conditional::Variables,
    depth: usize,
    replacing: bool,
) -> Result<Vec<Token>> {
    if depth > 20 {
        return Err(Diag::error(Default::default(), "COPY nesting too deep"));
    }
    conditional::expand(
        text,
        file,
        opts.source_encoding.padded(),
        variables,
        &mut |chunk, first_line, variables| {
            let mut stream =
                source::read_input(chunk, file, debugging, opts.source_encoding.padded());
            for (_, pos) in &mut stream {
                pos.line += first_line - 1;
            }
            let tokens = lexer::lex(&stream, comma)?;
            let mut load = |name: &str,
                            lib: Option<&str>,
                            at: &Token,
                            partial: &[(String, String)],
                            replacing: bool|
             -> Result<Vec<Token>> {
                let source = files
                    .names
                    .get(at.pos.file as usize)
                    .map(PathBuf::from)
                    .unwrap_or_else(|| path.to_path_buf());
                let base = source.parent().unwrap_or(Path::new("."));
                let next_file = files.names.len() as u32;
                let mut copy_opts = opts.clone();
                let (text, path) = match find_copy_member(&opts.copy_dirs, base, &source, name, lib)
                {
                    Some(member) => {
                        copy_opts.source_encoding = opts
                            .copy_encodings
                            .get(&member)
                            .copied()
                            .or_else(|| {
                                member
                                    .canonicalize()
                                    .ok()
                                    .and_then(|p| opts.copy_encodings.get(&p).copied())
                            })
                            .unwrap_or(opts.source_encoding);
                        let bytes = std::fs::read(&member).map_err(|e| {
                            Diag::error(at.pos, format!("cannot read {}: {e}", member.display()))
                        })?;
                        let path = member.display().to_string();
                        // Register before decoding so malformed COPY data
                        // reports its own file and record number.
                        files.names.push(path.clone());
                        let text = input::decode(
                            &bytes,
                            copy_opts.source_encoding,
                            crate::codepage::active().ccsid,
                            next_file,
                            true,
                        )?;
                        (text, path)
                    }
                    None => match syslib::member(&name.to_ascii_uppercase()) {
                        Some(text) => {
                            copy_opts.source_encoding = SourceEncoding::Utf8;
                            (text, format!("<system>/{name}"))
                        }
                        None => {
                            return Err(Diag::error(
                                at.pos,
                                format!("COPY member {name} not found"),
                            ));
                        }
                    },
                };
                if files.names.len() == next_file as usize {
                    files.names.push(path.clone());
                }
                files.text.insert(
                    path.clone(),
                    if copy_opts.source_encoding.padded() {
                        input::clean(&text)
                    } else {
                        text.clone()
                    },
                );
                let text = replace_partial(&text, partial);
                let file = next_file;
                if text.lines().any(|line| {
                    line.chars()
                        .skip(7)
                        .collect::<String>()
                        .trim_start()
                        .starts_with(">>")
                }) {
                    return expand_conditional_source(
                        Path::new(&path),
                        &text,
                        file,
                        &copy_opts,
                        files,
                        debugging,
                        comma,
                        variables,
                        depth + 1,
                        replacing,
                    );
                }
                let stream =
                    source::read_input(&text, file, debugging, copy_opts.source_encoding.padded());
                lexer::lex(&stream, comma)
            };
            copy::expand_copy_context(tokens, &mut load, depth, replacing)
                .map_err(|d| d.during(crate::diag::Phase::Preprocess))
        },
    )
}

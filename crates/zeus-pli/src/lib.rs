//! Zeus PL/I: a compiler for the Enterprise PL/I for z/OS dialect and an
//! interpreter for the compiled modules.
//!
//! The source (margins, `%INCLUDE`) is tokenised ([`lexer`]), parsed
//! ([`parser`]) and translated ([`sema`]) into a typed program ([`ir`])
//! in which every conversion follows the language's rules (RULES(IBM),
//! LIMITS(FIXEDDEC(15) FIXEDBIN(31))). The interpreter ([`vm`]) keeps
//! data in z/OS representations: EBCDIC characters, packed decimal,
//! big-endian binary and hexadecimal floating point.

pub mod ast;
pub mod builtin;
pub mod ir;
pub mod lexer;
pub mod link;
pub mod parser;
pub mod picture;
pub mod preproc;
pub mod sema;
pub mod types;
pub mod vm;

use std::path::{Path, PathBuf};

/// Compiler options.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Directories searched for %INCLUDE members.
    pub include_dirs: Vec<PathBuf>,
    /// MARGINS(m,n); default (2,72).
    pub margins: Option<(usize, usize)>,
}

/// The result of a compilation.
pub struct Compiled {
    pub module: Option<ir::Module>,
    /// Messages in listing form.
    pub messages: Vec<String>,
}

/// Magic prefix of a serialised PL/I module.
const MAGIC: &str = "{\"zeus_pli\":";

fn messages(diags: &[lexer::Diag], files: &[String]) -> Vec<String> {
    diags
        .iter()
        .map(|d| {
            let file = files
                .get(d.pos.file as usize)
                .map(String::as_str)
                .unwrap_or("?");
            let code = match d.severity {
                'S' => "IBM1000I S",
                'E' => "IBM1000I E",
                'W' => "IBM1000I W",
                _ => "IBM1000I I",
            };
            let _ = code;
            format!("{file}:{}: {}: {}", d.pos.line, d.severity, d.message)
        })
        .collect()
}

/// Compiles a source file. Returns the return code (0, 4 with warnings,
/// 12 or 16 on errors) and the result.
pub fn compile_file(path: &Path, opts: &Options) -> (i32, Compiled) {
    let lo = lexer::LexOptions {
        margins: opts.margins.unwrap_or((2, 72)),
        include_dirs: {
            let mut d = opts.include_dirs.clone();
            if let Some(p) = path.parent() {
                d.push(p.to_path_buf());
            }
            d
        },
    };
    let mut files = lexer::Files::default();
    let toks = match lexer::tokenize(path, &lo, &mut files) {
        Ok(t) => t,
        Err(d) => {
            return (
                16,
                Compiled {
                    module: None,
                    messages: messages(&[d], &files.names),
                },
            );
        }
    };
    let (toks, pp_diags) = match preproc::preprocess(toks, &lo, &mut files) {
        Ok(t) => t,
        Err(d) => {
            return (
                16,
                Compiled {
                    module: None,
                    messages: messages(&[d], &files.names),
                },
            );
        }
    };
    let stmts = match parser::Parser::new(toks).program() {
        Ok(s) => s,
        Err(d) => {
            let mut all = pp_diags;
            all.push(d);
            return (
                12,
                Compiled {
                    module: None,
                    messages: messages(&all, &files.names),
                },
            );
        }
    };
    let ccsid = zeus_cobol::codepage::active().ccsid;
    let (module, sema_diags) = sema::compile(&stmts, ccsid, files.names.clone());
    let mut diags = pp_diags;
    diags.extend(sema_diags);
    let msgs = messages(&diags, &files.names);
    let rc = if module.is_none() || diags.iter().any(|d| d.severity == 'S' || d.severity == 'E') {
        12
    } else if diags.iter().any(|d| d.severity == 'W') {
        4
    } else {
        0
    };
    (
        rc,
        Compiled {
            module: if rc >= 12 { None } else { module },
            messages: msgs,
        },
    )
}

/// The serialised form of a module.
pub fn module_bytes(m: &ir::Module) -> std::io::Result<Vec<u8>> {
    let body = serde_json::to_vec(m).map_err(std::io::Error::other)?;
    let mut out = MAGIC.as_bytes().to_vec();
    out.extend(body);
    out.push(b'}');
    Ok(out)
}

pub fn save_module(m: &ir::Module, path: &Path) -> std::io::Result<()> {
    std::fs::write(path, module_bytes(m)?)
}

/// Whether the bytes are a serialised PL/I module.
pub fn is_module(data: &[u8]) -> bool {
    data.starts_with(MAGIC.as_bytes())
}

pub fn load_module_bytes(data: &[u8]) -> std::io::Result<ir::Module> {
    use serde::Deserialize;
    if !is_module(data) {
        return Err(std::io::Error::other("not a PL/I module"));
    }
    let body = &data[MAGIC.len()..data.len() - 1];
    let mut de = serde_json::Deserializer::from_slice(body);
    de.disable_recursion_limit();
    ir::Module::deserialize(&mut de).map_err(std::io::Error::other)
}

pub fn load_module(path: &Path) -> std::io::Result<ir::Module> {
    load_module_bytes(&std::fs::read(path)?)
}

/// Runs a module.
pub fn run(module: ir::Module, env: vm::RunEnv) -> vm::RunEnd {
    zeus_cobol::codepage::set_active(module.ccsid);
    let mut m = vm::Machine::new(module, env);
    m.run()
}

/// Runs a module as the main program of the run unit `sys` (a COBOL
/// machine: its storage, COBOL programs and EXEC interface); gives the
/// run unit back.
pub fn run_in(
    module: ir::Module,
    env: vm::RunEnv,
    sys: zeus_cobol::vm::Machine,
) -> (vm::RunEnd, zeus_cobol::vm::Machine) {
    zeus_cobol::codepage::set_active(module.ccsid);
    let mut m = vm::Machine::with_storage(module, env, sys);
    let end = m.run();
    (end, m.detach())
}

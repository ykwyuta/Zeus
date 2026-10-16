//! The Zeus COBOL compiler (Enterprise COBOL 4.2 dialect) and its runtime.
//!
//! Pipeline: source reading and COPY/REPLACE ([`front`]), semantic analysis
//! and code generation to ZIR ([`sema`], [`zir`]), and execution by the ZIR
//! interpreter ([`vm`]).

pub mod codepage;
pub mod coverage;
pub mod diag;
pub mod ebcdic;
pub mod front;
pub mod hlasm;
pub mod image;
pub mod interlang;
pub mod inventory;
pub mod native;
pub mod sema;
pub mod vm;
pub mod zir;

use std::path::Path;

use diag::Diag;
use front::{FileTable, FrontOptions};

/// Compiles a source file into a ZIR module. Diagnostics carry file names.
pub fn compile_file(path: &Path, opts: &FrontOptions) -> Result<zir::Module, Diag> {
    let compiled_at = vm::timestamp();
    let mut files = FileTable::default();
    let name = |d: Diag, files: &FileTable| {
        let mut d = d;
        d.file = files.names.get(d.pos.file as usize).cloned();
        d
    };
    let programs = front::parse_file(path, opts, &mut files).map_err(|d| name(d, &files))?;
    let mut module = sema::codegen::compile(&programs).map_err(|d| name(d, &files))?;
    module.compiled_at = Some(compiled_at);
    // the source files, as absolute paths (coverage reports)
    module.sources = files
        .names
        .iter()
        .map(|n| {
            std::fs::canonicalize(n)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| n.clone())
        })
        .collect();
    Ok(module)
}

/// Serialised form of a module (`.zo` file).
pub fn save_module(module: &zir::Module, path: &Path) -> std::io::Result<()> {
    std::fs::write(path, module_bytes(module)?)
}

/// The contents of a module's `.zo` file.
pub fn module_bytes(module: &zir::Module) -> std::io::Result<Vec<u8>> {
    serde_json::to_vec(module).map_err(std::io::Error::other)
}

/// A module from the contents of its `.zo` file.
pub fn load_module_bytes(data: &[u8]) -> std::io::Result<zir::Module> {
    use serde::Deserialize;
    // long condition chains nest deeply
    let mut de = serde_json::Deserializer::from_slice(data);
    de.disable_recursion_limit();
    zir::Module::deserialize(&mut de).map_err(std::io::Error::other)
}

pub fn load_module(path: &Path) -> std::io::Result<zir::Module> {
    load_module_bytes(&std::fs::read(path)?)
}

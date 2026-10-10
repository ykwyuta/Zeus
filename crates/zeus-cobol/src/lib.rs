//! The Zeus COBOL compiler (Enterprise COBOL 4.2 dialect) and its runtime.
//!
//! Pipeline: source reading and COPY/REPLACE ([`front`]), semantic analysis
//! and code generation to ZIR ([`sema`], [`zir`]), and execution by the ZIR
//! interpreter ([`vm`]).

pub mod diag;
pub mod ebcdic;
pub mod front;
pub mod sema;
pub mod vm;
pub mod zir;

use std::path::Path;

use diag::Diag;
use front::{FileTable, FrontOptions};

/// Compiles a source file into a ZIR module. Diagnostics carry file names.
pub fn compile_file(path: &Path, opts: &FrontOptions) -> Result<zir::Module, Diag> {
    let mut files = FileTable::default();
    let name = |d: Diag, files: &FileTable| {
        let mut d = d;
        d.file = files.names.get(d.pos.file as usize).cloned();
        d
    };
    let programs = front::parse_file(path, opts, &mut files).map_err(|d| name(d, &files))?;
    sema::codegen::compile(&programs).map_err(|d| name(d, &files))
}

/// Serialised form of a module (`.zo` file).
pub fn save_module(module: &zir::Module, path: &Path) -> std::io::Result<()> {
    let data = serde_json::to_vec(module).map_err(std::io::Error::other)?;
    std::fs::write(path, data)
}

pub fn load_module(path: &Path) -> std::io::Result<zir::Module> {
    let data = std::fs::read(path)?;
    serde_json::from_slice(&data).map_err(std::io::Error::other)
}

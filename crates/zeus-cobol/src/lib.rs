//! The Zeus COBOL compiler (Enterprise COBOL 4.2 dialect) and its runtime.
//!
//! Pipeline: source reading and COPY/REPLACE ([`front`]), semantic analysis
//! and code generation to ZIR ([`sema`], [`zir`]), and execution by the ZIR
//! interpreter ([`vm`]).

pub mod clock;
pub mod codepage;
pub mod concolic;
pub mod coverage;
pub mod diag;
pub mod dll;
pub mod ebcdic;
pub mod front;
pub mod hlasm;
pub mod image;
pub mod interlang;
pub mod inventory;
pub mod link;
pub mod listing;
pub mod native;
pub mod numeric;
pub mod sema;
pub mod vm;
pub mod zir;

use std::path::Path;

use diag::Diag;
use front::{FileTable, FrontOptions};

/// Compiles a source file into a ZIR module. Diagnostics carry file names.
pub fn compile_file(path: &Path, opts: &FrontOptions) -> Result<zir::Module, Diag> {
    compile_impl(path, opts, None)
}

/// Compile once and return the configured listing, including error diagnostics.
pub fn compile_file_with_listing(
    path: &Path,
    opts: &FrontOptions,
) -> (Result<zir::Module, Diag>, String) {
    let mut listing = String::new();
    let module = compile_impl(path, opts, Some(&mut listing));
    (module, listing)
}

/// Compiles each program of a batch compilation separately (with its own
/// CBL / PROCESS options) and binds them into one module.
fn compile_impl(
    path: &Path,
    opts: &FrontOptions,
    mut listing: Option<&mut String>,
) -> Result<zir::Module, Diag> {
    let mut modules = Vec::new();
    let mut unit = 0;
    loop {
        let mut unit_listing = String::new();
        let (module, units) = compile_unit(
            path,
            opts,
            unit,
            listing.is_some().then_some(&mut unit_listing),
        );
        if let Some(listing) = listing.as_deref_mut() {
            listing.push_str(&unit_listing);
        }
        modules.push(module?);
        unit += 1;
        if unit >= units {
            break;
        }
    }
    if modules.len() == 1 {
        return Ok(modules.pop().unwrap());
    }
    link::modules(modules).map_err(|e| Diag::error(Default::default(), e))
}

/// One program of the source; also returns the number of programs (1 when
/// the source cannot be read).
fn compile_unit(
    path: &Path,
    opts: &FrontOptions,
    unit: usize,
    listing: Option<&mut String>,
) -> (Result<zir::Module, Diag>, usize) {
    let compiled_at = vm::timestamp();
    let mut files = FileTable::default();
    let mut listing_tokens = Vec::new();
    let mut units = 1;
    let name = |d: Diag, files: &FileTable| {
        let mut d = d;
        d.file = files
            .names
            .get(d.pos.file as usize)
            .cloned()
            .or_else(|| Some(path.display().to_string()));
        d
    };
    let result = (|| {
        let (tokens, cics, count) =
            front::source_unit_tokens(path, opts, &mut files, unit).map_err(|d| name(d, &files))?;
        units = count;
        if listing.is_some() {
            listing_tokens.clone_from(&tokens);
        }
        let programs = front::parse_tokens(tokens, cics).map_err(|d| name(d, &files))?;
        let _codepage = codepage::scope(Some(front::compilation_ccsid()));
        let mut module = sema::codegen::compile(&programs).map_err(|d| name(d, &files))?;
        module.compiled_at = Some(compiled_at);
        for diagnostic in &mut module.diagnostics {
            diagnostic.file = files.names.get(diagnostic.pos.file as usize).cloned();
        }
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
    })();
    if let Some(listing) = listing {
        *listing = crate::listing::render(
            path,
            &files,
            &listing_tokens,
            &result,
            &front::listing_options(),
        );
    }
    (result, units)
}

/// Serialised form of a module (`.zo` file).
pub fn save_module(module: &zir::Module, path: &Path) -> std::io::Result<()> {
    std::fs::write(path, module_bytes(module)?)?;
    dll::save_sidefile(module, path)
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

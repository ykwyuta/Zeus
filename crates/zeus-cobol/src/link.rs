//! Binding separately compiled COBOL units into one Zeus program object.
use crate::zir::{Base, Module};
use std::collections::HashSet;

pub fn modules(modules: Vec<Module>) -> Result<Module, String> {
    let first = modules.first().ok_or("no COBOL objects to bind")?;
    let mut output = Module {
        diagnostics: Vec::new(),
        optimize: first.optimize,
        programs: Vec::new(),
        ccsid: first.ccsid,
        sources: Vec::new(),
        compiled_at: first.compiled_at.clone(),
    };
    let mut symbols = HashSet::new();
    for mut module in modules {
        let program_offset =
            u32::try_from(output.programs.len()).map_err(|_| "too many bound programs")?;
        let source_offset =
            u32::try_from(output.sources.len()).map_err(|_| "too many bound sources")?;
        for program in &mut module.programs {
            if program.parent.is_none() {
                for name in std::iter::once(&program.name)
                    .chain(program.entries.iter().map(|entry| &entry.name))
                {
                    if !symbols.insert(name.clone()) {
                        return Err(format!("duplicate bound COBOL entry {name}"));
                    }
                }
            }
            program.ccsid = Some(program.ccsid.unwrap_or(module.ccsid));
            if program.compiled_at.is_none() {
                program.compiled_at.clone_from(&module.compiled_at);
            }
            if let Some(parent) = &mut program.parent {
                *parent += program_offset;
            }
            for desc in &mut program.descs {
                match &mut desc.base {
                    Base::Outer(owner) | Base::OuterLinkage(owner, _) => *owner += program_offset,
                    _ => {}
                }
                desc.definition.0 += source_offset;
            }
            for file in &mut program.files {
                if let Some((owner, _)) = &mut file.owner {
                    *owner += program_offset;
                }
            }
            for position in &mut program.stmt_pos {
                position.0 += source_offset;
            }
            for paragraph in &mut program.paragraphs {
                paragraph.pos.0 += source_offset;
            }
        }
        for diagnostic in &mut module.diagnostics {
            diagnostic.pos.file += source_offset;
        }
        output.programs.extend(module.programs);
        output.sources.extend(module.sources);
        output.diagnostics.extend(module.diagnostics);
    }
    Ok(output)
}

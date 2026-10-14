//! Link Zeus assembler modules without depending on platform or IBM object
//! formats. The linked image shares one address space and instruction map.
use super::{Module, load_module};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

const MAX_IMAGE: usize = 16 << 20;

fn exports(module: &Module) -> HashMap<String, u32> {
    let mut entries = module.entries.clone();
    entries
        .entry(module.name.to_ascii_uppercase())
        .or_insert(module.entry);
    entries
}

pub fn link_modules(modules: &[Module]) -> Result<Module, String> {
    let first = modules.first().ok_or("no HLASM modules to link")?;
    let mut linked = Module {
        zeus_hlasm: 1,
        name: first.name.clone(),
        alignment: 8,
        instruction_templates: true,
        read_only: Vec::new(),
        rmode: 31,
        storage: Vec::new(),
        entry: first.entry,
        code: Vec::new(),
        relocations: Vec::new(),
        entries: HashMap::new(),
        externals: HashMap::new(),
    };
    for module in modules {
        let alignment = module.required_alignment()?;
        module.validate_read_only()?;
        linked.rmode = linked.rmode.min(module.rmode);
        linked.alignment = linked.alignment.max(module.alignment);
        let origin = linked.storage.len().next_multiple_of(alignment);
        let end = origin
            .checked_add(module.storage.len())
            .filter(|n| *n <= MAX_IMAGE)
            .ok_or("linked HLASM image exceeds 16 MiB")?;
        linked.storage.resize(origin, 0);
        linked.storage.extend(&module.storage);
        linked.read_only.extend(
            module
                .read_only
                .iter()
                .map(|r| r.start + origin as u32..r.end + origin as u32),
        );
        if !module.instruction_templates {
            super::template::initialize(&module.code, &mut linked.storage[origin..end])?;
        }
        debug_assert_eq!(linked.storage.len(), end);
        for (name, entry) in exports(module) {
            if entry as usize >= module.storage.len() {
                return Err(format!("invalid exported address {name}"));
            }
            if linked
                .entries
                .insert(name.clone(), entry + origin as u32)
                .is_some()
            {
                return Err(format!("duplicate external definition {name}"));
            }
        }
        for instruction in &module.code {
            if instruction.offset as usize >= module.storage.len() {
                return Err("instruction outside module storage".into());
            }
            let mut instruction = instruction.clone();
            if let Some(expression) = &mut instruction.relative_expression {
                expression.addend = i64::try_from(origin)
                    .ok()
                    .and_then(|origin| origin.checked_mul(expression.local_coefficient))
                    .and_then(|n| n.checked_add(expression.addend))
                    .ok_or("relative relocation overflow")?;
            }
            instruction.offset += origin as u32;
            linked.code.push(instruction);
        }
        for relocation in &module.relocations {
            if (relocation.offset as usize)
                .checked_add(4)
                .is_none_or(|n| n > module.storage.len())
            {
                return Err("relocation outside module storage".into());
            }
            let mut relocation = relocation.clone();
            relocation.offset += origin as u32;
            if let Some(expression) = &mut relocation.expression {
                expression.rebase(origin)?;
            } else if relocation.symbol.is_none() {
                relocation.target = relocation
                    .target
                    .checked_add(origin as u32)
                    .ok_or("relocation overflow")?;
            }
            linked.relocations.push(relocation);
        }
        for (name, weak) in &module.externals {
            linked
                .externals
                .entry(name.clone())
                .and_modify(|old| *old &= weak)
                .or_insert(*weak);
        }
    }
    for (symbol, weak) in &linked.externals {
        if !weak && !linked.entries.contains_key(symbol) {
            return Err(format!("unresolved external symbol {symbol}"));
        }
    }
    for ins in &mut linked.code {
        if let Some(expression) = &mut ins.relative_expression {
            let mut fix = || -> Result<(), String> {
                if ins.relative_external.is_some() {
                    return Err("conflicting relative relocations".into());
                }
                super::operation::relative(&ins.op)
                    .ok_or("relative relocation on a non-relative instruction")?;
                let address = ins
                    .address
                    .as_ref()
                    .ok_or("relative relocation has no operand")?;
                if address.base != 0 || address.index != 0 {
                    return Err("relative relocation cannot use base/index registers".into());
                }
                let mut pending = Vec::new();
                for term in &expression.terms {
                    if let Some(&target) = linked.entries.get(&term.symbol) {
                        expression.addend = i64::from(target)
                            .checked_mul(term.coefficient)
                            .and_then(|n| n.checked_add(expression.addend))
                            .ok_or("relative relocation overflow")?;
                        expression.local_coefficient = expression
                            .local_coefficient
                            .checked_add(term.coefficient)
                            .ok_or("relative relocation overflow")?;
                    } else if term.weak {
                        pending.push(term.clone());
                    } else {
                        return Err(format!("unresolved external symbol {}", term.symbol));
                    }
                }
                expression.terms = pending;
                // Also validates the net coefficient after resolving terms.
                expression.target_at(0)?;
                Ok(())
            };
            fix().map_err(|e| {
                format!(
                    "{}:{}: {}: relative expression: {e}",
                    ins.file, ins.line, ins.op
                )
            })?;
            if expression.terms.is_empty() {
                let displacement = expression
                    .addend
                    .checked_sub(i64::from(ins.offset))
                    .ok_or("relative relocation overflow")?;
                super::operation::check_relative(
                    displacement,
                    super::operation::relative(&ins.op).unwrap().bits,
                )
                .map_err(|e| {
                    format!(
                        "{}:{}: {}: relative expression: {e}",
                        ins.file, ins.line, ins.op
                    )
                })?;
                ins.address.as_mut().unwrap().displacement = displacement;
                ins.relative_expression = None;
            }
        }
        let Some(reference) = &ins.relative_external else {
            continue;
        };
        let fix = || -> Result<Option<i64>, String> {
            let spec = super::operation::relative(&ins.op)
                .ok_or("relative relocation on a non-relative instruction")?;
            let address = ins
                .address
                .as_ref()
                .ok_or("relative relocation has no operand")?;
            if address.base != 0 || address.index != 0 {
                return Err("relative relocation cannot use base/index registers".into());
            }
            if let Some(&target) = linked.entries.get(&reference.symbol) {
                let displacement = i64::from(target)
                    .checked_add(reference.addend)
                    .and_then(|target| target.checked_sub(i64::from(ins.offset)))
                    .ok_or("relative relocation overflow")?;
                super::operation::check_relative(displacement, spec.bits)?;
                Ok(Some(displacement))
            } else if reference.weak {
                Ok(None)
            } else {
                Err(format!("unresolved external symbol {}", reference.symbol))
            }
        };
        let displacement = fix().map_err(|e| {
            format!(
                "{}:{}: {}: external {}: {e}",
                ins.file, ins.line, ins.op, reference.symbol
            )
        })?;
        if let Some(displacement) = displacement {
            ins.address.as_mut().unwrap().displacement = displacement;
            ins.relative_external = None;
        }
    }
    for r in &mut linked.relocations {
        if let Some(expression) = &mut r.expression {
            if r.symbol.is_some() || r.target != 0 || r.addend != 0 || r.weak {
                return Err("conflicting address relocations".into());
            }
            expression
                .resolve(&linked.entries)
                .map_err(|e| format!("address constant at {}: {e}", r.offset))?;
        }
        if let Some(symbol) = &r.symbol {
            if let Some(target) = linked.entries.get(symbol) {
                r.target = *target;
                r.symbol = None;
            } else if !r.weak {
                return Err(format!("unresolved external symbol {symbol}"));
            }
        }
    }
    linked
        .externals
        .retain(|name, _| !linked.entries.contains_key(name));
    linked.zeus_hlasm = if !linked.read_only.is_empty() || linked.rmode == 24 {
        6
    } else if linked.alignment > 8 {
        5
    } else {
        super::format_version(&linked.code, &linked.relocations)
    };
    // Check relocated values without making them permanent in the object.
    linked.storage_at(0)?;
    Ok(linked)
}

/// Pull strong references transitively from libraries. Weak references never
/// cause a file read; an explicitly included definition can still resolve one.
pub fn link_with_libraries(mut modules: Vec<Module>, dirs: &[PathBuf]) -> Result<Module, String> {
    if modules.is_empty() {
        return Err("no HLASM modules to link".into());
    }
    let mut attempted = HashSet::new();
    loop {
        let defined: HashSet<String> = modules
            .iter()
            .flat_map(|m| exports(m).into_keys())
            .collect();
        let mut required = Vec::new();
        for module in &modules {
            required.extend(
                module
                    .relocations
                    .iter()
                    .filter_map(|r| r.expression.as_ref())
                    .flat_map(|expression| &expression.terms)
                    .filter(|term| !term.weak)
                    .map(|term| term.symbol.clone()),
            );
            required.extend(
                module
                    .code
                    .iter()
                    .filter_map(|ins| ins.relative_expression.as_ref())
                    .flat_map(|expression| &expression.terms)
                    .filter(|term| !term.weak)
                    .map(|term| term.symbol.clone()),
            );
            required.extend(
                module
                    .code
                    .iter()
                    .filter_map(|ins| ins.relative_external.as_ref())
                    .filter(|reference| !reference.weak)
                    .map(|reference| reference.symbol.clone()),
            );
            required.extend(
                module
                    .externals
                    .iter()
                    .filter(|(_, weak)| !**weak)
                    .map(|(name, _)| name.clone()),
            );
            required.extend(
                module
                    .relocations
                    .iter()
                    .filter(|r| !r.weak)
                    .filter_map(|r| r.symbol.clone()),
            );
        }
        required.sort();
        required.dedup();
        let mut added = false;
        for symbol in required {
            if modules.iter().any(|m| exports(m).contains_key(&symbol)) {
                continue;
            }
            if defined.contains(&symbol) || !attempted.insert(symbol.clone()) {
                continue;
            }
            let mut found = None;
            'libraries: for dir in dirs {
                for name in [symbol.clone(), symbol.to_ascii_lowercase()] {
                    for extension in [".zo", ""] {
                        let path = dir.join(format!("{name}{extension}"));
                        if !path.is_file() {
                            continue;
                        }
                        let bytes =
                            std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
                        let module =
                            load_module(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
                        if !exports(&module).contains_key(&symbol) {
                            return Err(format!("{} does not define {symbol}", path.display()));
                        }
                        found = Some(module);
                        break 'libraries;
                    }
                }
            }
            if let Some(module) = found {
                // Another symbol might already have pulled this same member.
                let already = modules.iter().any(|m| m.name == module.name);
                if !already {
                    modules.push(module);
                    added = true;
                }
            }
        }
        if modules.len() > 4096 {
            return Err("HLASM library inclusion exceeds 4096 modules".into());
        }
        if !added {
            return link_modules(&modules);
        }
    }
}

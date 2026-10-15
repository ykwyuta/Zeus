//! Conventional 80-byte OBJ output, from the public Program Management
//! Advanced Facilities record diagrams (Appendix A). Never emit Zeus templates.
pub(super) mod instructions;
use super::super::{Module, RelativeTerm, RelocationExpression};
use crate::ebcdic;
use std::collections::BTreeMap;

fn card(kind: &str) -> [u8; 80] {
    let mut c = [ebcdic::SPACE; 80];
    c[0] = 2;
    c[1..4].copy_from_slice(&ebcdic::encode(kind));
    c
}
fn number(bytes: &mut [u8], at: usize, width: usize, value: u32) {
    bytes[at..at + width].copy_from_slice(&value.to_be_bytes()[4 - width..]);
}
fn symbol(name: &str, kind: u8, address: u32, flags: u8, length: u32) -> Result<[u8; 16], String> {
    let bytes = ebcdic::encode(name);
    if name.is_empty()
        || bytes.len() > 8
        || !name.bytes().all(|b| b.is_ascii_graphic())
        || ebcdic::decode(&bytes) != name
    {
        return Err(format!(
            "OBJ output requires a representable 1..8-byte external name: {name}"
        ));
    }
    let mut s = [ebcdic::SPACE; 16];
    s[..bytes.len()].copy_from_slice(&bytes);
    s[8] = kind;
    // ER/WX address and length fields are blank, not binary zero.
    if matches!(kind, 0 | 1) {
        number(&mut s, 9, 3, address);
        number(&mut s, 13, 3, length);
    }
    s[12] = flags;
    Ok(s)
}
fn esd(output: &mut Vec<u8>, id: Option<u16>, items: &[[u8; 16]]) {
    let mut c = card("ESD");
    number(&mut c, 10, 2, (16 * items.len()) as u32);
    if let Some(id) = id {
        number(&mut c, 14, 2, u32::from(id));
    }
    for (n, item) in items.iter().enumerate() {
        c[16 + n * 16..32 + n * 16].copy_from_slice(item);
    }
    output.extend(c);
}
fn claim(ranges: &mut BTreeMap<usize, usize>, start: usize, end: usize) -> Result<(), String> {
    if ranges
        .range(..end)
        .next_back()
        .is_some_and(|(_, previous_end)| *previous_end > start)
    {
        return Err("OBJ output has overlapping instruction or relocation fields".into());
    }
    ranges.insert(start, end);
    Ok(())
}

/// Flatten an image into one doubleword-aligned control section. Reject any
/// metadata that cannot be represented without changing its runtime meaning.
pub(in crate::hlasm) fn bytes(module: &Module) -> Result<Vec<u8>, String> {
    module.required_alignment()?;
    module.validate_read_only()?;
    if module.alignment > 8 || module.storage.is_empty() || module.storage.len() > 0xFF_FFFF {
        return Err(
            "OBJ output requires a nonempty image below 16 MiB, aligned at most to 8 bytes".into(),
        );
    }
    if module.entry as usize >= module.storage.len() {
        return Err("OBJ output entry is outside its control section".into());
    }
    if module
        .entry_modes
        .values()
        .any(|mode| *mode != module.amode)
    {
        return Err(
            "OBJ output of mixed entry addressing modes requires multiple control sections".into(),
        );
    }
    let read_only = !module.read_only.is_empty();
    if read_only
        && (module.read_only.len() != 1 || module.read_only[0] != (0..module.storage.len() as u32))
    {
        return Err(
            "OBJ output of partially read-only storage requires multiple control sections".into(),
        );
    }
    if !module.dcbs.is_empty() || module.bindings.requires_extended() {
        return Err(
            "OBJ output cannot encode Zeus data-control blocks or extended section bindings".into(),
        );
    }
    if !module.object_relocations.is_empty() {
        return Err(
            "OBJ output cannot encode mode-sensitive or relative-immediate object relocations"
                .into(),
        );
    }
    if module
        .entries
        .get(&module.name)
        .copied()
        .unwrap_or(module.entry)
        != 0
    {
        return Err("OBJ output control-section name must designate offset zero".into());
    }
    let mut storage = module.storage.clone();
    let mut occupied = BTreeMap::new();
    for instruction in &module.code {
        if !instruction.offset.is_multiple_of(2) {
            return Err("OBJ output instruction must be halfword aligned".into());
        }
        let bytes = instructions::encode(instruction).map_err(|e| {
            format!(
                "{}:{}: {}: OBJ output: {e}",
                instruction.file, instruction.line, instruction.op
            )
        })?;
        let start = instruction.offset as usize;
        let end = start
            .checked_add(bytes.len())
            .filter(|end| *end <= storage.len())
            .ok_or("OBJ output instruction exceeds its image")?;
        claim(&mut occupied, start, end)?;
        storage[start..end].copy_from_slice(&bytes);
    }
    let mut externals = module
        .externals
        .iter()
        .map(|(name, weak)| (name.clone(), *weak))
        .collect::<BTreeMap<_, _>>();
    let mut relocations = Vec::new();
    for r in &module.relocations {
        let expression = if let Some(expression) = &r.expression {
            if r.target != 0 || r.symbol.is_some() || r.addend != 0 || r.weak {
                return Err("OBJ output has conflicting address relocations".into());
            }
            expression.clone()
        } else if let Some(symbol) = &r.symbol {
            RelocationExpression {
                addend: r.addend,
                local_coefficient: 0,
                terms: vec![RelativeTerm {
                    symbol: symbol.clone(),
                    coefficient: 1,
                    weak: r.weak,
                }],
            }
        } else {
            RelocationExpression {
                addend: i64::from(r.target)
                    .checked_add(r.addend)
                    .ok_or("OBJ output relocation overflow")?,
                local_coefficient: 1,
                terms: vec![],
            }
        };
        let start = r.offset as usize;
        let end = start
            .checked_add(r.width as usize)
            .filter(|end| *end <= storage.len())
            .ok_or("OBJ output relocation exceeds its image")?;
        if !(1..=8).contains(&r.width) {
            return Err("OBJ output relocation width must be 1..8 bytes".into());
        }
        claim(&mut occupied, start, end)?;
        storage[start..end]
            .copy_from_slice(&expression.addend.to_be_bytes()[8 - r.width as usize..]);
        for term in &expression.terms {
            if module.entries.contains_key(&term.symbol) || term.symbol == module.name {
                return Err(
                    "OBJ output requires local external references to be resolved before emission"
                        .into(),
                );
            }
            externals
                .entry(term.symbol.clone())
                .and_modify(|weak| *weak &= term.weak)
                .or_insert(term.weak);
        }
        relocations.push((r.offset, r.width, expression));
    }
    if externals.contains_key(&module.name)
        || externals
            .keys()
            .any(|name| module.entries.contains_key(name))
    {
        return Err("OBJ output name is both defined and external".into());
    }
    if externals.len() > 65533 {
        return Err("OBJ output exceeds the ESD identifier limit".into());
    }
    let flags = (if module.amode == 64 {
        0x10
    } else if module.amode == 31 {
        2
    } else {
        1
    }) | if module.rmode == 31 { 4 } else { 0 }
        | if read_only { 8 } else { 0 };
    let mut output = Vec::new();
    esd(
        &mut output,
        Some(1),
        &[symbol(&module.name, 0, 0, flags, storage.len() as u32)?],
    );
    let mut identifiers = BTreeMap::new();
    let mut items = Vec::new();
    for (n, (name, weak)) in externals.iter().enumerate() {
        let id = n as u16 + 2;
        identifiers.insert(name.clone(), id);
        items.push(symbol(
            name,
            if *weak { 10 } else { 2 },
            0,
            ebcdic::SPACE,
            0,
        )?);
    }
    let mut at = 0;
    while at < items.len() {
        let id = at + 2;
        let mut count = (items.len() - at).min(3);
        // X'4040' in the first-ID field denotes an all-LD record. Keep that
        // binary ID inside a group instead of using it as a record header.
        if at + count < items.len() && id + count == 0x4040 {
            count -= 1;
        }
        esd(&mut output, Some(id as u16), &items[at..at + count]);
        at += count;
    }
    items.clear();
    let entries = module.entries.iter().collect::<BTreeMap<_, _>>();
    for (name, at) in entries {
        if *at as usize >= storage.len() {
            return Err(format!(
                "OBJ output exported address {name} is outside its image"
            ));
        }
        if name != &module.name {
            items.push(symbol(name, 1, *at, ebcdic::SPACE, 1)?);
        }
    }
    for chunk in items.chunks(3) {
        esd(&mut output, None, chunk);
    }
    for (n, chunk) in storage.chunks(56).enumerate() {
        let mut c = card("TXT");
        number(&mut c, 5, 3, (n * 56) as u32);
        number(&mut c, 10, 2, chunk.len() as u32);
        number(&mut c, 14, 2, 1);
        c[16..16 + chunk.len()].copy_from_slice(chunk);
        output.extend(c);
    }
    let mut rld = Vec::new();
    for (at, width, expression) in relocations {
        let terms = std::iter::once((1, expression.local_coefficient, false)).chain(
            expression
                .terms
                .iter()
                .map(|t| (identifiers[&t.symbol], t.coefficient, true)),
        );
        for (reference, coefficient, external) in terms {
            // Guard malicious/oversized coefficients before expanding RLD items.
            if coefficient.unsigned_abs() > 1_000_000
                || rld.len() as u64 + coefficient.unsigned_abs() * 8 > 32 << 20
            {
                return Err("OBJ output relocation dictionary exceeds its size limit".into());
            }
            for _ in 0..coefficient.unsigned_abs() {
                let mut item = [0u8; 8];
                number(&mut item, 0, 2, u32::from(reference));
                number(&mut item, 2, 2, 1);
                item[4] = ((width - 1) & 3) << 2
                    | if width > 4 { 0x40 } else { 0 }
                    | if coefficient < 0 { 2 } else { 0 }
                    | if external { 0x10 } else { 0 };
                number(&mut item, 5, 3, at);
                rld.extend(item);
            }
        }
    }
    for chunk in rld.chunks(56) {
        let mut c = card("RLD");
        number(&mut c, 10, 2, chunk.len() as u32);
        c[16..16 + chunk.len()].copy_from_slice(chunk);
        output.extend(c);
    }
    let mut c = card("END");
    number(&mut c, 5, 3, module.entry);
    number(&mut c, 14, 2, 1);
    output.extend(c);
    if output.len() > 64 << 20 {
        return Err("OBJ output exceeds 64 MiB".into());
    }
    Ok(output)
}

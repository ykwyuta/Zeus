//! GOFF architecture-level-1 output from the public Program Management
//! Advanced Facilities Appendix C. Emit fixed 80-byte physical records.
mod fixups;
mod layout;
use crate::hlasm::{
    Module, ObjectAddressMode, ObjectRelocation, RelativeTerm, RelocationExpression,
};
use std::collections::BTreeMap;

fn word(b: &mut [u8], at: usize, n: u32) {
    b[at..at + 4].copy_from_slice(&n.to_be_bytes());
}
fn half(b: &mut [u8], at: usize, n: usize) {
    b[at..at + 2].copy_from_slice(&(n as u16).to_be_bytes());
}
fn mode(n: u8) -> u8 {
    match n {
        24 => 1,
        31 => 2,
        64 => 4,
        _ => unreachable!(),
    }
}
fn name(text: &str) -> Result<Vec<u8>, String> {
    let bytes = crate::ebcdic::encode(text);
    if bytes.is_empty()
        || bytes.len() > 32767
        || bytes.iter().any(|n| !(0x41..=0xfe).contains(n))
        || crate::ebcdic::decode(&bytes) != text
    {
        return Err(format!(
            "GOFF output requires a representable 1..32767-byte external name: {text}"
        ));
    }
    Ok(bytes)
}
fn esd(
    id: u32,
    kind: u8,
    parent: u32,
    text: &str,
    offset: u32,
    length: u32,
    attributes: [u8; 10],
) -> Result<Vec<u8>, String> {
    let name = name(text)?;
    let mut b = vec![0; 72 + name.len()];
    b[0] = 3;
    b[3] = kind;
    word(&mut b, 4, id);
    word(&mut b, 8, parent);
    word(&mut b, 16, offset);
    word(&mut b, 24, length);
    if kind != 0 {
        b[40] = 1;
    }
    b[60..70].copy_from_slice(&attributes);
    half(&mut b, 70, name.len());
    b[72..].copy_from_slice(&name);
    Ok(b)
}

#[derive(Default)]
struct Output {
    bytes: Vec<u8>,
    logical: u32,
}
impl Output {
    fn record(&mut self, b: &[u8]) -> Result<(), String> {
        self.logical = self
            .logical
            .checked_add(1)
            .ok_or("GOFF output record count overflow")?;
        let mut rest = b;
        let mut first = true;
        while !rest.is_empty() {
            if self.bytes.len() + 80 > 64 << 20 {
                return Err("GOFF output exceeds 64 MiB".into());
            }
            let mut card = [0; 80];
            let start = if first { 0 } else { 3 };
            let count = rest.len().min(80 - start);
            card[start..start + count].copy_from_slice(&rest[..count]);
            rest = &rest[count..];
            card[0] = 3;
            card[1] = (b[1] & 0xf0) | if first { 0 } else { 2 } | u8::from(!rest.is_empty());
            self.bytes.extend(card);
            first = false;
        }
        Ok(())
    }
}

fn claim(
    used: &mut BTreeMap<usize, usize>,
    at: usize,
    size: usize,
    image: usize,
) -> Result<(), String> {
    let end = at
        .checked_add(size)
        .filter(|end| *end <= image)
        .ok_or("GOFF output field exceeds its image")?;
    if used
        .range(..end)
        .next_back()
        .is_some_and(|(_, end)| *end > at)
    {
        return Err("GOFF output has overlapping instruction or relocation fields".into());
    }
    used.insert(at, end);
    Ok(())
}

fn expression(r: &crate::hlasm::Relocation) -> Result<RelocationExpression, String> {
    if let Some(e) = &r.expression {
        if r.target != 0 || r.symbol.is_some() || r.addend != 0 || r.weak {
            return Err("GOFF output has conflicting address relocations".into());
        }
        return Ok(e.clone());
    }
    Ok(if let Some(symbol) = &r.symbol {
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
                .ok_or("GOFF output relocation overflow")?,
            local_coefficient: 1,
            terms: vec![],
        }
    })
}

/// Loadable byte-stream elements. Unrepresentable layout/binder
/// attributes are rejected, rather than silently flattened away.
pub(in crate::hlasm) fn bytes(module: &Module) -> Result<Vec<u8>, String> {
    module.required_alignment()?;
    if !matches!(module.alignment, 8 | 16 | 4096) {
        return Err("GOFF output alignment must be 8, 16 or 4096 bytes".into());
    }
    module.validate_read_only()?;
    module.bindings.validate(module)?;
    if module.storage.is_empty() || module.storage.len() > 16 << 20 {
        return Err("GOFF output requires a nonempty image up to 16 MiB".into());
    }
    if module.entry as usize >= module.storage.len()
        || module
            .entries
            .values()
            .any(|at| *at as usize >= module.storage.len())
    {
        return Err("GOFF output entry is outside its image".into());
    }
    if module
        .entries
        .get(&module.name)
        .copied()
        .unwrap_or(module.entry)
        != 0
    {
        return Err("GOFF output section name must designate offset zero".into());
    }
    if module.entry_mode(module.entry) != module.amode || module.entry_mode(0) != module.amode {
        return Err("GOFF output main and section entry must use the default AMODE".into());
    }
    if module.entry_modes.iter().any(|(at, mode)| {
        *mode != module.amode && !module.entries.values().any(|entry| entry == at)
    }) {
        return Err("GOFF output nondefault entry mode requires an exported label".into());
    }
    if !module.dcbs.is_empty()
        || !module.bindings.quantities.is_empty()
        || !module.bindings.psects.is_empty()
    {
        return Err(
            "GOFF output cannot encode Zeus DCBs or unresolved section quantities/PSECTs".into(),
        );
    }
    let mut storage = module.storage.clone();
    let mut used = BTreeMap::new();
    let mut code = Vec::new();
    let mut fixups = Vec::new();
    for i in &module.code {
        if !i.offset.is_multiple_of(2) {
            return Err("GOFF output instruction must be halfword aligned".into());
        }
        let mut instruction = i.clone();
        if i.relative_external.is_some() && i.relative_expression.is_some() {
            return Err("GOFF output has conflicting relative instructions".into());
        }
        if i.relative_external.is_some() || i.relative_expression.is_some() {
            let spec = crate::hlasm::operation::relative(&i.op)
                .ok_or("GOFF output relative fixup on a non-relative instruction")?;
            let target = if let Some(r) = &i.relative_external {
                RelocationExpression {
                    addend: r.addend,
                    local_coefficient: 0,
                    terms: vec![RelativeTerm {
                        symbol: r.symbol.clone(),
                        coefficient: 1,
                        weak: r.weak,
                    }],
                }
            } else {
                i.relative_expression.clone().unwrap()
            };
            fixups.push(ObjectRelocation {
                offset: i
                    .offset
                    .checked_add(2)
                    .ok_or("GOFF output offset overflow")?,
                width: (spec.bits / 8) as u8,
                addressing_mode: ObjectAddressMode::Known(24),
                relative_immediate: true,
                expression: RelocationExpression {
                    addend: target
                        .addend
                        .checked_sub(i64::from(i.offset))
                        .ok_or("GOFF output relative overflow")?,
                    local_coefficient: target
                        .local_coefficient
                        .checked_sub(1)
                        .ok_or("GOFF output relative overflow")?,
                    terms: target.terms,
                },
            });
            instruction.relative_external = None;
            instruction.relative_expression = None;
            instruction
                .address
                .as_mut()
                .ok_or("GOFF output missing relative operand")?
                .displacement = 0;
        }
        let encoded = super::write::instructions::encode(&instruction).map_err(|e| {
            format!(
                "{}:{}: {}: GOFF output: {}",
                i.file,
                i.line,
                i.op,
                e.replace("OBJ output", "GOFF output")
            )
        })?;
        claim(&mut used, i.offset as usize, encoded.len(), storage.len())?;
        code.push(i.offset..i.offset + encoded.len() as u32);
        storage[i.offset as usize..i.offset as usize + encoded.len()].copy_from_slice(&encoded);
    }
    let mut ordinary = Vec::new();
    for r in &module.relocations {
        if !(1..=8).contains(&r.width) {
            return Err("GOFF output relocation width must be 1..8 bytes".into());
        }
        claim(
            &mut used,
            r.offset as usize,
            r.width as usize,
            storage.len(),
        )?;
        ordinary.push((r.offset, r.width, expression(r)?));
    }
    for r in &module.object_relocations {
        claim(
            &mut used,
            r.offset as usize,
            r.width as usize,
            storage.len(),
        )?;
        fixups.push(r.clone());
    }
    code.sort_by_key(|r| r.start);
    let elements = layout::plan(module, &code)?;
    let mut entries = module
        .entries
        .iter()
        .map(|(s, n)| (s.clone(), *n))
        .collect::<BTreeMap<_, _>>();
    entries.entry(module.name.clone()).or_insert(0);
    let mut externals = module
        .externals
        .iter()
        .map(|(s, w)| (s.clone(), *w))
        .collect::<BTreeMap<_, _>>();
    for e in ordinary
        .iter()
        .map(|(_, _, e)| e)
        .chain(fixups.iter().map(|r| &r.expression))
    {
        for term in &e.terms {
            if entries.contains_key(&term.symbol) {
                return Err(
                    "GOFF output requires local external references to be resolved first".into(),
                );
            }
            externals
                .entry(term.symbol.clone())
                .and_modify(|w| *w &= term.weak)
                .or_insert(term.weak);
        }
    }
    if externals.keys().any(|s| entries.contains_key(s)) {
        return Err("GOFF output name is both defined and external".into());
    }
    let mut out = Output::default();
    let mut header = vec![0; 60];
    header[..3].copy_from_slice(&[3, 0xf0, 0]);
    word(&mut header, 48, 1);
    out.record(&header)?;
    let mut attributes = [0; 10];
    // SD identifies the section; the matching LD carries its callable AMODE.
    out.record(&esd(1, 0, 0, &module.name, 0, 0, attributes)?)?;
    for element in &elements {
        attributes = [0; 10];
        attributes[1] = if module.rmode == 31 { 3 } else { 1 };
        attributes[3] =
            if element.executable { 2 } else { 1 } | if element.readonly { 8 } else { 0 };
        attributes[6] = element.alignment.trailing_zeros() as u8;
        out.record(&esd(
            element.id,
            1,
            1,
            &element.class,
            0,
            element.range.end - element.range.start,
            attributes,
        )?)?;
    }
    let mut labels = BTreeMap::new();
    let mut id = elements.len() as u32 + 2;
    for (s, at) in &entries {
        labels.insert(s.clone(), (id, *at, module.entry_mode(*at)));
        attributes = [0; 10];
        attributes[0] = mode(module.entry_mode(*at));
        let (parent, offset) = layout::position(&elements, *at, 1)?;
        out.record(&esd(id, 2, parent, s, offset, 0, attributes)?)?;
        id = id
            .checked_add(1)
            .ok_or("GOFF output ESD identifier overflow")?;
    }
    let mut references = BTreeMap::new();
    for (s, weak) in externals {
        references.insert(s.clone(), id);
        attributes = [0; 10];
        attributes[4] = u8::from(weak);
        out.record(&esd(id, 4, 1, &s, 0, 0, attributes)?)?;
        id = id
            .checked_add(1)
            .ok_or("GOFF output ESD identifier overflow")?;
    }
    let mut relocations = fixups::Writer::new(
        &mut storage,
        &labels,
        &references,
        labels[&module.name].0,
        &elements,
    );
    for (at, width, e) in ordinary {
        relocations.address(at, width, e, None)?;
    }
    for r in fixups {
        if r.relative_immediate {
            relocations.relative(r.offset, r.width, r.expression)?;
        } else {
            relocations.address(r.offset, r.width, r.expression, Some(r.addressing_mode))?;
        }
    }
    let rld = relocations.finish();
    for element in &elements {
        for (n, chunk) in storage[element.range.start as usize..element.range.end as usize]
            .chunks(32760)
            .enumerate()
        {
            let mut b = vec![0; 24 + chunk.len()];
            b[..3].copy_from_slice(&[3, 0x10, 0]);
            word(&mut b, 4, element.id);
            word(&mut b, 12, (n * 32760) as u32);
            half(&mut b, 22, chunk.len());
            b[24..].copy_from_slice(chunk);
            out.record(&b)?;
        }
    }
    for chunk in rld.chunks(32760) {
        let mut b = vec![0; 6 + chunk.len()];
        b[..3].copy_from_slice(&[3, 0x20, 0]);
        half(&mut b, 4, chunk.len());
        b[6..].copy_from_slice(chunk);
        out.record(&b)?;
    }
    let mut end = vec![0; 26];
    end[..3].copy_from_slice(&[3, 0x40, 0]);
    end[3] = 1;
    end[4] = mode(module.amode);
    let count = out
        .logical
        .checked_add(1)
        .ok_or("GOFF output record count overflow")?;
    word(&mut end, 8, count);
    let (parent, offset) = layout::position(&elements, module.entry, 1)?;
    word(&mut end, 12, parent);
    word(&mut end, 20, offset);
    out.record(&end)?;
    Ok(out.bytes)
}

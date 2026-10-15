//! GOFF logical records and loadable byte-stream elements. Field definitions
//! follow Program Management Advanced Facilities, GOFF Appendix C.
mod records;
mod relocations;
use crate::hlasm::{self, Module};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone)]
struct Definition {
    kind: u8,
    parent: u32,
    name: String,
    origin: u32,
    length: Option<u32>,
    namespace: u8,
    attributes: [u8; 10],
    fill: u8,
}
struct Text {
    element: u32,
    offset: u32,
    bytes: Vec<u8>,
}
#[derive(Default)]
struct Builder {
    definitions: BTreeMap<u32, Definition>,
    text: Vec<Text>,
    relocations: Vec<relocations::Fixup>,
    previous_fixup: [Option<u32>; 3],
    entry: Option<(u32, u32)>,
    entry_name: Option<String>,
    entry_mode: Option<u8>,
    count: u32,
}
fn uint(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn half(bytes: &[u8], at: usize) -> usize {
    usize::from(u16::from_be_bytes(bytes[at..at + 2].try_into().unwrap()))
}
fn mode(value: u8) -> Result<u8, String> {
    match value {
        0 | 1 => Ok(24),
        2 | 3 => Ok(31),
        4 => Ok(64),
        _ => Err("unsupported GOFF addressing mode".into()),
    }
}
fn text_name(bytes: &[u8]) -> String {
    crate::ebcdic::decode(bytes).trim_end().to_string()
}
impl Builder {
    fn process(&mut self, record: records::Record) -> Result<Option<Module>, String> {
        self.count += 1;
        let b = &record.bytes;
        match record.kind {
            15 => {
                if self.count != 1 {
                    return Err("GOFF HDR must begin an object file".into());
                }
                if uint(b, 48) > 1 || half(b, 52) != 0 {
                    return Err("unsupported GOFF header architecture or module properties".into());
                }
            }
            0 => {
                let id = uint(b, 4);
                let parent = uint(b, 8);
                let kind = b[3];
                if id != self.definitions.len() as u32 + 1 {
                    return Err("GOFF ESDIDs must be consecutive starting at one".into());
                }
                if kind > 4 {
                    return Err("invalid GOFF symbol type".into());
                }
                if kind == 0 && parent != 0 {
                    return Err("GOFF SD cannot have a parent".into());
                }
                if kind != 0 && (parent == 0 || !self.definitions.contains_key(&parent)) {
                    return Err("GOFF parent must be defined first".into());
                }
                if kind != 0
                    && self.definitions[&parent].kind != if matches!(kind, 1 | 4) { 0 } else { 1 }
                {
                    return Err("invalid GOFF parent symbol type".into());
                }
                let mut attributes = [0; 10];
                attributes.copy_from_slice(&b[60..70]);
                if attributes[4] & 15 > 1 {
                    return Err("invalid GOFF binding strength".into());
                }
                if !matches!(attributes[3] & 7, 0..=2) || attributes[6] & 31 > 12 {
                    return Err("invalid GOFF execution/alignment attributes".into());
                }
                // GOFF class names are case-sensitive (Appendix C).
                let name = text_name(&b[72..]);
                if b[70..72] == [0, 0] {
                    return Err("GOFF symbol name cannot have zero length".into());
                }
                let length = match uint(b, 24) {
                    u32::MAX => None,
                    n => Some(n),
                };
                self.definitions.insert(
                    id,
                    Definition {
                        kind,
                        parent,
                        name,
                        origin: uint(b, 16),
                        length,
                        namespace: b[40],
                        attributes,
                        fill: if b[41] & 0x80 != 0 { b[42] } else { 0 },
                    },
                );
            }
            1 => {
                let element = uint(b, 4);
                let def = self
                    .definitions
                    .get(&element)
                    .ok_or("GOFF text refers to undefined element")?;
                if !matches!(def.kind, 1 | 3) || b[3] > 2 || b[3] != def.attributes[2] >> 4 {
                    return Err("GOFF text style disagrees with its element".into());
                }
                let encoding = half(b, 20);
                let true_length = uint(b, 16) as usize;
                let bytes = match encoding {
                    0 => {
                        if true_length != 0 {
                            return Err("unencoded GOFF text has nonzero true length".into());
                        }
                        b[24..].to_vec()
                    }
                    1 => {
                        if true_length > 16 << 20 {
                            return Err("expanded GOFF text exceeds 16 MiB".into());
                        }
                        let mut out = Vec::new();
                        let mut at = 24;
                        while at < b.len() {
                            if at + 4 > b.len() {
                                return Err("truncated GOFF text repetition".into());
                            }
                            let repeat = half(b, at);
                            let length = half(b, at + 2);
                            at += 4;
                            if repeat == 0
                                || length == 0
                                || at + length > b.len()
                                || out
                                    .len()
                                    .checked_add(repeat * length)
                                    .is_none_or(|n| n > true_length)
                            {
                                return Err("invalid GOFF text repetition length".into());
                            }
                            for _ in 0..repeat {
                                out.extend_from_slice(&b[at..at + length]);
                            }
                            at += length;
                        }
                        if out.len() != true_length {
                            return Err("GOFF expanded text length disagrees with header".into());
                        }
                        out
                    }
                    _ => return Err("unsupported GOFF text encoding".into()),
                };
                self.text.push(Text {
                    element,
                    offset: uint(b, 12),
                    bytes,
                });
            }
            2 => self.relocations.extend(relocations::decode(
                &b[6..],
                &self.definitions,
                &mut self.previous_fixup,
            )?),
            3 => {
                if !(b.len() - 8).is_multiple_of(12) {
                    return Err("invalid GOFF LEN item length".into());
                }
                for item in b[8..].as_chunks::<12>().0.iter() {
                    let def = self
                        .definitions
                        .get_mut(&uint(item, 0))
                        .filter(|d| matches!(d.kind, 1 | 3))
                        .ok_or("GOFF LEN refers to undefined element")?;
                    if def.length.is_some() {
                        return Err("GOFF LEN requires a deferred element".into());
                    }
                    def.length = Some(uint(item, 8));
                }
            }
            4 => {
                let expected = uint(b, 8);
                if expected != 0 && expected != self.count {
                    return Err("GOFF END logical record count mismatch".into());
                }
                match b[3] & 3 {
                    0 => {}
                    1 => {
                        let id = uint(b, 12);
                        if !self.definitions.contains_key(&id) {
                            return Err("GOFF entry refers to undefined element".into());
                        }
                        self.entry = Some((id, uint(b, 20)));
                        self.entry_mode = Some(mode(b[4])?);
                    }
                    2 => {
                        self.entry_name = Some(text_name(&b[26..]));
                        self.entry_mode = Some(mode(b[4])?);
                    }
                    _ => return Err("invalid GOFF END entry flags".into()),
                }
                return Ok(Some(std::mem::take(self).build()?));
            }
            _ => return Err("unknown GOFF record type".into()),
        }
        Ok(None)
    }
    fn build(self) -> Result<Module, String> {
        let mut image = Vec::new();
        let mut placement = HashMap::new();
        let mut machine_code = Vec::new();
        let mut read_only = Vec::new();
        let mut bindings = hlasm::Bindings::default();
        let mut alignment = 8u32;
        let mut rmode = 31;
        // Keep contributions to each class together, preserving ESD order within
        // a class. Class identity is independent of the section's external name.
        let mut elements = self
            .definitions
            .iter()
            .filter(|(_, d)| d.kind == 1)
            .collect::<Vec<_>>();
        elements.sort_by_key(|(id, d)| (&d.name, **id));
        for (&id, d) in elements {
            if d.attributes[2] & 15 != 0 || d.attributes[5] & 0x20 != 0 {
                return Err("GOFF MERGE/common elements require further binder support".into());
            }
            if d.attributes[6] & 0x20 != 0 {
                return Err("GOFF XPLINK linkage is not yet supported".into());
            }
            let length = d
                .length
                .ok_or("GOFF element length was not supplied by LEN")?;
            if d.attributes[5] >> 6 == 2 {
                continue;
            }
            if d.attributes[5] >> 6 != 0 || d.attributes[2] >> 4 != 0 {
                return Err("GOFF deferred or structured loadable class is not supported".into());
            }
            let boundary = 1u32 << u32::from(d.attributes[6] & 31);
            alignment = alignment.max(boundary);
            let start = image.len().next_multiple_of(boundary as usize);
            let end = start
                .checked_add(length as usize)
                .filter(|n| *n <= 16 << 20)
                .ok_or("GOFF image exceeds 16 MiB")?;
            image.resize(start, 0);
            image.resize(end, d.fill);
            placement.insert(id, start as u32);
            if length != 0 && d.attributes[3] & 7 != 1 {
                machine_code.push(start as u32..end as u32);
            }
            if length != 0 && d.attributes[3] & 8 != 0 {
                read_only.push(start as u32..end as u32);
            }
            if !matches!(d.attributes[1], 0 | 1 | 3) {
                return Err("unsupported GOFF residence mode".into());
            }
            if d.attributes[1] != 3 {
                rmode = 24;
            }
            bindings.sections.push(hlasm::BindingSection {
                name: d.name.clone(),
                length,
                alignment: boundary,
                dummy: false,
                class: Some(d.name.clone()),
                concatenate: true,
                priority: 0,
                image: Some(start as u32),
            });
        }
        if self.definitions.values().any(|d| d.kind == 3) {
            return Err("GOFF PART allocation is not yet supported".into());
        }
        let mapped = |id: u32, offset: u32| -> Result<u32, String> {
            let d = self.definitions.get(&id).ok_or("undefined GOFF symbol")?;
            if d.kind != 1 {
                return Err("GOFF address requires an element".into());
            }
            let delta = offset
                .checked_sub(d.origin)
                .ok_or("GOFF offset precedes its element origin")?;
            if delta > d.length.unwrap_or(0) {
                return Err("GOFF offset exceeds element length".into());
            }
            Ok(*placement
                .get(&id)
                .ok_or("GOFF reference to a nonloaded element")?
                + delta)
        };
        for text in &self.text {
            if !placement.contains_key(&text.element) {
                continue;
            }
            let at = mapped(text.element, text.offset)? as usize;
            let end = at
                .checked_add(text.bytes.len())
                .filter(|n| {
                    *n <= placement[&text.element] as usize
                        + self.definitions[&text.element].length.unwrap() as usize
                })
                .ok_or("GOFF text exceeds its element")?;
            image[at..end].copy_from_slice(&text.bytes);
        }
        let mut entries = HashMap::new();
        let mut externals = HashMap::new();
        let mut modes = BTreeMap::new();
        let mut module_name = None;
        for (&id, d) in &self.definitions {
            let at = match d.kind {
                0 => {
                    let first = self.definitions.iter().find(|(child, e)| {
                        e.kind == 1 && e.parent == id && placement.contains_key(child)
                    });
                    let Some((&child, _)) = first else { continue };
                    if module_name.is_none() {
                        module_name = Some(if d.name.is_empty() {
                            "OBJECT".into()
                        } else {
                            d.name.clone()
                        });
                    }
                    placement[&child]
                }
                2 => {
                    if d.namespace != 1 {
                        continue;
                    }
                    mapped(d.parent, d.origin)?
                }
                4 => {
                    externals.insert(d.name.clone(), d.attributes[4] & 15 == 1);
                    continue;
                }
                _ => continue,
            };
            if !d.name.is_empty() {
                if entries
                    .insert(d.name.clone(), at)
                    .is_some_and(|old| old != at)
                {
                    return Err("duplicate GOFF external definition".into());
                }
                modes.insert(at, mode(d.attributes[0])?);
            }
        }
        let (entry, amode) = if let Some((id, offset)) = self.entry {
            let d = &self.definitions[&id];
            let at = match d.kind {
                1 => mapped(id, offset)?,
                4 => *entries
                    .get(&d.name)
                    .ok_or("GOFF external END entry requires linking support")?,
                _ => return Err("GOFF END entry requires ED or ER".into()),
            };
            (at, self.entry_mode.unwrap_or(mode(d.attributes[0])?))
        } else if let Some(name) = &self.entry_name {
            let at = *entries.get(name).ok_or("GOFF named entry is undefined")?;
            (at, self.entry_mode.unwrap_or(modes[&at]))
        } else {
            let at = *placement
                .values()
                .min()
                .ok_or("GOFF has no loadable elements")?;
            (at, modes.get(&at).copied().unwrap_or(24))
        };
        modes.insert(entry, amode);
        let mut relocated = BTreeMap::<(u32, u8), hlasm::RelocationExpression>::new();
        let mut mode_fixups = BTreeMap::new();
        let mut relative_fixups = std::collections::BTreeSet::new();
        for fix in &self.relocations {
            let at = mapped(fix.position, fix.offset)?;
            let width = fix.flags[4];
            let end = placement[&fix.position] + self.definitions[&fix.position].length.unwrap();
            if at.checked_add(u32::from(width)).is_none_or(|n| n > end) {
                return Err("GOFF relocation exceeds its element".into());
            }
            if relocated.keys().any(|&(offset, w)| {
                (offset, w) != (at, width)
                    && offset < at + u32::from(width)
                    && at < offset + u32::from(w)
            }) {
                return Err("GOFF relocation fields partially overlap".into());
            }
            let relative = fix.flags[1] >> 4 == 6;
            let store = fix.flags[2] & 1 != 0;
            if relative && self.definitions[&fix.reference].kind != 4 {
                return Err("GOFF relative-immediate currently requires an external label".into());
            }
            if !store
                && relocated.contains_key(&(at, width))
                && (relative || relative_fixups.contains(&(at, width)))
            {
                return Err(
                    "GOFF relative-immediate supports one external referent per target field"
                        .into(),
                );
            }
            let expr = relocated.entry((at, width)).or_insert_with(|| {
                let mut raw = [0; 8];
                raw[8 - width as usize..]
                    .copy_from_slice(&image[at as usize..at as usize + width as usize]);
                hlasm::RelocationExpression {
                    addend: if relative {
                        if width == 2 {
                            i64::from(i16::from_be_bytes(raw[6..].try_into().unwrap())) * 2
                        } else {
                            i64::from(i32::from_be_bytes(raw[4..].try_into().unwrap())) * 2
                        }
                    } else {
                        u64::from_be_bytes(raw) as i64
                    },
                    local_coefficient: 0,
                    terms: Vec::new(),
                }
            });
            if fix.flags[2] & 1 != 0 {
                expr.addend = 0;
                expr.local_coefficient = 0;
                expr.terms.clear();
                mode_fixups.remove(&(at, width));
                relative_fixups.remove(&(at, width));
            }
            if relative {
                let element_start = placement[&fix.position];
                let pc = at
                    .checked_sub(2)
                    .filter(|pc| *pc >= element_start)
                    .ok_or("GOFF relative-immediate field precedes its instruction")?;
                expr.addend = expr
                    .addend
                    .checked_sub(i64::from(pc))
                    .ok_or("GOFF relative-immediate arithmetic overflow")?;
                expr.local_coefficient = -1;
                relative_fixups.insert((at, width));
            }
            let sign = if fix.flags[2] & 2 != 0 { -1 } else { 1 };
            let referent = &self.definitions[&fix.reference];
            if fix.flags[1] >> 4 == 2 {
                let length = match (fix.flags[1] & 15, referent.kind) {
                    (0, 2 | 4) => 0, // Labels have no length, including external labels.
                    (1, 1) => referent
                        .length
                        .ok_or("GOFF length reference to an undefined length")?,
                    (2, _) => {
                        return Err(
                            "GOFF class-length relocation requires class binding support".into(),
                        );
                    }
                    _ => {
                        return Err(
                            "GOFF length relocation referent type does not match ESD".into()
                        );
                    }
                };
                expr.addend = expr
                    .addend
                    .checked_add(sign * i64::from(length))
                    .ok_or("GOFF length relocation arithmetic overflow")?;
                continue;
            }
            if fix.flags[0] & 1 != 0 {
                let mode = if referent.kind == 4 {
                    hlasm::ObjectAddressMode::External {
                        symbol: referent.name.clone(),
                        weak: referent.attributes[4] & 15 == 1,
                    }
                } else {
                    hlasm::ObjectAddressMode::Known(mode(referent.attributes[0])?)
                };
                if mode_fixups
                    .insert((at, width), mode.clone())
                    .is_some_and(|old| old != mode)
                {
                    return Err("conflicting GOFF relocation addressing modes".into());
                }
            }
            let local = match referent.kind {
                1 => Some(
                    i64::from(
                        *placement
                            .get(&fix.reference)
                            .ok_or("GOFF reference to nonloaded element")?,
                    ) - i64::from(referent.origin),
                ),
                2 => Some(i64::from(mapped(referent.parent, referent.origin)?)),
                4 => {
                    expr.terms.push(hlasm::RelativeTerm {
                        symbol: referent.name.clone(),
                        coefficient: sign,
                        weak: referent.attributes[4] & 15 == 1,
                    });
                    None
                }
                _ => return Err("unsupported GOFF relocation referent".into()),
            };
            if let Some(local) = local {
                expr.local_coefficient += sign;
                expr.addend = expr
                    .addend
                    .checked_add(sign * local)
                    .ok_or("GOFF relocation arithmetic overflow")?;
            }
        }
        let mut relocations = Vec::new();
        let mut object_relocations = Vec::new();
        for ((offset, width), expression) in relocated {
            let relative_immediate = relative_fixups.contains(&(offset, width));
            if relative_immediate || mode_fixups.contains_key(&(offset, width)) {
                let addressing_mode = mode_fixups
                    .remove(&(offset, width))
                    .unwrap_or(hlasm::ObjectAddressMode::Known(24));
                object_relocations.push(hlasm::ObjectRelocation {
                    offset,
                    width,
                    expression,
                    addressing_mode,
                    relative_immediate,
                });
            } else {
                relocations.push(hlasm::Relocation {
                    offset,
                    width,
                    target: 0,
                    symbol: None,
                    addend: 0,
                    weak: false,
                    expression: Some(expression),
                });
            }
        }
        machine_code.sort_by_key(|r| r.start);
        read_only.sort_by_key(|r| r.start);
        if image.is_empty() {
            image.resize(8, 0);
        }
        let module = Module {
            zeus_hlasm: if relative_fixups.is_empty() { 11 } else { 12 },
            name: module_name.ok_or("GOFF has no section")?,
            alignment,
            instruction_templates: true,
            machine_code,
            read_only,
            rmode,
            amode,
            entry_modes: modes,
            storage: image,
            entry,
            code: Vec::new(),
            relocations,
            object_relocations,
            entries,
            externals,
            dcbs: Vec::new(),
            bindings,
        };
        module.required_alignment()?;
        module.bindings.validate(&module)?;
        Ok(module)
    }
}
pub(super) fn is_goff(bytes: &[u8]) -> bool {
    bytes.starts_with(&[3, 0xf0, 0]) || bytes.get(4..7) == Some(&[3, 0xf0, 0])
}
pub(super) fn load(bytes: &[u8]) -> Result<Module, String> {
    let mut builder = Builder::default();
    let mut modules = Vec::new();
    for record in records::read(bytes)? {
        if builder.count == 0 && record.kind != 15 {
            return Err("GOFF file must begin with HDR".into());
        }
        if let Some(module) = builder.process(record)? {
            modules.push(module);
        }
    }
    if builder.count != 0 || modules.is_empty() {
        return Err("GOFF file is missing END".into());
    }
    if modules.len() == 1 {
        Ok(modules.remove(0))
    } else {
        hlasm::link_modules(&modules)
    }
}

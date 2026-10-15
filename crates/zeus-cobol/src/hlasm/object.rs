//! IBM OBJ card-image input/output. Record definitions come from the public Program
//! Management Advanced Facilities reference, Appendix A (not IBM source code).
pub(super) mod fixups;
mod goff;
pub(super) mod goff_write;
pub(super) mod write;
use super::{Module, RelativeTerm, Relocation, RelocationExpression};
pub use fixups::{AddressMode as ObjectAddressMode, ObjectRelocation};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone)]
struct Symbol {
    kind: u8,
    name: String,
    origin: u32,
    length: u32,
    flags: u8,
}
struct Label {
    name: String,
    section: u16,
    address: u32,
}
struct Text {
    section: u16,
    address: u32,
    bytes: Vec<u8>,
}
struct Fixup {
    reference: u16,
    position: u16,
    address: u32,
    width: u8,
    sign: i64,
}
#[derive(Default)]
struct Deck {
    symbols: BTreeMap<u16, Symbol>,
    labels: Vec<Label>,
    text: Vec<Text>,
    fixups: Vec<Fixup>,
    entry: Option<(u16, u32)>,
    entry_name: Option<String>,
    deferred: Option<u32>,
}
fn word(b: &[u8]) -> u16 {
    u16::from_be_bytes(b.try_into().unwrap())
}
fn address(b: &[u8]) -> u32 {
    (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2])
}
fn name(b: &[u8]) -> String {
    crate::ebcdic::decode(b).trim_end().to_string()
}
fn length(card: &[u8], maximum: usize) -> Result<usize, String> {
    let n = usize::from(word(&card[10..12]));
    if n > maximum {
        return Err("OBJ record payload exceeds its card image".into());
    }
    Ok(n)
}
impl Deck {
    fn esd(&mut self, c: &[u8]) -> Result<(), String> {
        let n = length(c, 48)?;
        if n == 0 || n % 16 != 0 {
            return Err("invalid OBJ ESD item length".into());
        }
        let mut id = word(&c[14..16]);
        for item in c[16..16 + n].chunks_exact(16) {
            let kind = item[8];
            let text = name(&item[..8]);
            if kind == 1 {
                let section = u16::try_from(address(&item[13..16]))
                    .map_err(|_| "OBJ label has invalid section ID")?;
                self.labels.push(Label {
                    name: text,
                    section,
                    address: address(&item[9..12]),
                });
            } else {
                if id == 0 || c[14..16] == [0x40, 0x40] {
                    return Err("OBJ ESD requires a nonzero identifier".into());
                }
                if !matches!(kind, 0 | 2 | 4 | 5 | 6 | 10) {
                    return Err(format!("unsupported OBJ ESD type {kind:02X}"));
                }
                if self
                    .symbols
                    .insert(
                        id,
                        Symbol {
                            kind,
                            name: text,
                            origin: address(&item[9..12]),
                            length: address(&item[13..16]),
                            flags: item[12],
                        },
                    )
                    .is_some()
                {
                    return Err("duplicate OBJ ESD identifier".into());
                }
                id = id.checked_add(1).ok_or("OBJ ESD identifier overflow")?;
            }
        }
        Ok(())
    }
    fn section(&self, id: u16) -> Result<&Symbol, String> {
        self.symbols
            .get(&id)
            .filter(|s| matches!(s.kind, 0 | 4))
            .ok_or_else(|| format!("OBJ reference to undefined or non-text section {id}"))
    }
    fn txt(&mut self, c: &[u8]) -> Result<(), String> {
        let n = length(c, 56)?;
        let section = word(&c[14..16]);
        self.section(section)?;
        self.text.push(Text {
            section,
            address: address(&c[5..8]),
            bytes: c[16..16 + n].to_vec(),
        });
        Ok(())
    }
    fn rld(&mut self, c: &[u8]) -> Result<(), String> {
        let n = length(c, 56)?;
        let mut at = 16;
        let end = at + n;
        let (mut reference, mut position) = (0, 0);
        let mut compressed = false;
        while at < end {
            if !compressed {
                if at + 4 > end {
                    return Err("truncated OBJ RLD pointers".into());
                }
                reference = word(&c[at..at + 2]);
                position = word(&c[at + 2..at + 4]);
                at += 4;
                self.section(position)?;
                if !self.symbols.contains_key(&reference) {
                    return Err("OBJ RLD reference is undefined".into());
                }
            }
            if at + 4 > end {
                return Err("truncated OBJ RLD item".into());
            }
            let flags = c[at];
            if flags & 0xa0 != 0 {
                return Err(
                    "OBJ RLD Q/CXD or reserved relocation form is not yet supported".into(),
                );
            }
            let width = ((flags >> 2) & 3) + 1 + if flags & 0x40 != 0 { 4 } else { 0 };
            self.fixups.push(Fixup {
                reference,
                position,
                address: address(&c[at + 1..at + 4]),
                width,
                sign: if flags & 2 != 0 { -1 } else { 1 },
            });
            compressed = flags & 1 != 0;
            at += 4;
        }
        if compressed {
            return Err("OBJ RLD compression flag has no following item".into());
        }
        Ok(())
    }
    fn end(&mut self, c: &[u8]) -> Result<(), String> {
        if c[28] == 0 {
            self.deferred = Some(u32::from_be_bytes(c[28..32].try_into().unwrap()));
        }
        if c[5..8] != [0x40; 3] || c[14..16] != [0x40; 2] {
            let id = word(&c[14..16]);
            self.section(id)?;
            self.entry = Some((id, address(&c[5..8])));
        } else {
            let value = name(&c[16..24]);
            if !value.is_empty() {
                self.entry_name = Some(value);
            }
        }
        Ok(())
    }
    fn module(mut self) -> Result<Module, String> {
        if self.deferred.is_none()
            && self
                .symbols
                .values()
                .any(|s| matches!(s.kind, 0 | 4) && s.length == 0)
        {
            return Err("OBJ zero-length section requires its length on END".into());
        }
        if let Some(length) = self.deferred {
            let sections = self
                .symbols
                .values_mut()
                .filter(|s| matches!(s.kind, 0 | 4) && s.length == 0)
                .collect::<Vec<_>>();
            if sections.len() != 1 {
                return Err("OBJ END length requires exactly one deferred section".into());
            }
            sections.into_iter().next().unwrap().length = length;
        }
        let mut storage = Vec::new();
        let mut placement = HashMap::new();
        let mut ranges = Vec::new();
        let mut entries = HashMap::new();
        let mut externals = HashMap::new();
        let mut read_only = Vec::new();
        let mut modes = BTreeMap::new();
        let mut rmode = 31;
        let mut first = None;
        for (&id, s) in &self.symbols {
            if matches!(s.kind, 2 | 10) {
                externals.insert(s.name.clone(), s.kind == 10);
                continue;
            }
            if !matches!(s.kind, 0 | 4) {
                return Err("OBJ common/pseudo-register allocation is not yet supported".into());
            }
            let start = storage.len().next_multiple_of(8);
            let end = start
                .checked_add(s.length as usize)
                .filter(|n| *n <= 16 << 20)
                .ok_or("OBJ image exceeds 16 MiB")?;
            storage.resize(end, 0);
            placement.insert(id, start as u32);
            if start < end {
                ranges.push(start as u32..end as u32);
            }
            if s.flags & 8 != 0 && start < end {
                read_only.push(start as u32..end as u32);
            }
            if s.flags & 0x20 == 0 && s.flags & 4 == 0 {
                rmode = 24;
            }
            let mode = if s.flags & 0x10 != 0 {
                64
            } else if s.flags & 3 >= 2 {
                31
            } else {
                24
            };
            modes.insert(start as u32, mode);
            if !s.name.is_empty() && entries.insert(s.name.clone(), start as u32).is_some() {
                return Err("duplicate OBJ section name".into());
            }
            if first.is_none() {
                first = Some((
                    id,
                    if s.name.is_empty() {
                        "OBJECT".into()
                    } else {
                        s.name.clone()
                    },
                    mode,
                ));
            }
        }
        let (first_id, module_name, amode) = first.ok_or("OBJ has no text sections")?;
        let map = |id: u16, address: u32| -> Result<u32, String> {
            let s = self.section(id)?;
            let delta = address
                .checked_sub(s.origin)
                .ok_or("OBJ address is below its section origin")?;
            if delta > s.length {
                return Err("OBJ address exceeds section length".into());
            }
            Ok(placement[&id] + delta)
        };
        for text in &self.text {
            let at = map(text.section, text.address)? as usize;
            let s = self.section(text.section)?;
            let end = at
                .checked_add(text.bytes.len())
                .filter(|n| *n <= placement[&text.section] as usize + s.length as usize)
                .ok_or("OBJ text exceeds section length")?;
            storage[at..end].copy_from_slice(&text.bytes);
        }
        for label in &self.labels {
            let at = map(label.section, label.address)?;
            if entries.insert(label.name.clone(), at).is_some() {
                return Err("duplicate OBJ external definition".into());
            }
            modes.insert(at, modes[&placement[&label.section]]);
        }
        let entry = if let Some((id, address)) = self.entry {
            map(id, address)?
        } else if let Some(name) = &self.entry_name {
            *entries.get(name).ok_or("OBJ symbolic entry is undefined")?
        } else {
            placement[&first_id]
        };
        let mut relocations = BTreeMap::<(u32, u8), RelocationExpression>::new();
        for fix in &self.fixups {
            let at = map(fix.position, fix.address)?;
            let section_end = placement[&fix.position] + self.section(fix.position)?.length;
            if at
                .checked_add(u32::from(fix.width))
                .is_none_or(|end| end > section_end)
            {
                return Err("OBJ relocation field exceeds section length".into());
            }
            // Partially overlapping fields have ambiguous application order.
            if relocations.keys().any(|&(offset, width)| {
                (offset, width) != (at, fix.width)
                    && offset < at + u32::from(fix.width)
                    && at < offset + u32::from(width)
            }) {
                return Err("OBJ relocation fields partially overlap".into());
            }
            let expr = relocations.entry((at, fix.width)).or_insert_with(|| {
                let mut raw = [0; 8];
                raw[8 - fix.width as usize..]
                    .copy_from_slice(&storage[at as usize..at as usize + fix.width as usize]);
                RelocationExpression {
                    addend: u64::from_be_bytes(raw) as i64,
                    local_coefficient: 0,
                    terms: Vec::new(),
                }
            });
            let target = &self.symbols[&fix.reference];
            match target.kind {
                0 | 4 => {
                    expr.local_coefficient += fix.sign;
                    expr.addend = expr
                        .addend
                        .checked_add(
                            fix.sign
                                * (i64::from(placement[&fix.reference]) - i64::from(target.origin)),
                        )
                        .ok_or("OBJ relocation arithmetic overflow")?;
                }
                2 | 10 => expr.terms.push(RelativeTerm {
                    symbol: target.name.clone(),
                    coefficient: fix.sign,
                    weak: target.kind == 10,
                }),
                _ => return Err("unsupported OBJ relocation target type".into()),
            }
        }
        let relocations = relocations
            .into_iter()
            .map(|((offset, width), expression)| Relocation {
                offset,
                width,
                target: 0,
                symbol: None,
                addend: 0,
                weak: false,
                expression: Some(expression),
            })
            .collect();
        if storage.is_empty() {
            storage.resize(8, 0);
        }
        let module = Module {
            zeus_hlasm: 11,
            name: module_name,
            alignment: 8,
            instruction_templates: true,
            machine_code: ranges,
            read_only,
            rmode,
            amode,
            entry_modes: modes,
            storage,
            entry,
            code: Vec::new(),
            relocations,
            object_relocations: Vec::new(),
            entries,
            externals,
            dcbs: Vec::new(),
            bindings: super::Bindings::default(),
        };
        super::machine::validate(&module)?;
        Ok(module)
    }
}

pub(super) fn is_deck(bytes: &[u8]) -> bool {
    if goff::is_goff(bytes) {
        return true;
    }
    bytes.first() == Some(&2)
        && bytes.get(1..4).is_some_and(|tag| {
            ["ESD", "TXT", "RLD", "END", "SYM", "XSD"]
                .iter()
                .any(|kind| crate::ebcdic::encode(kind) == tag)
        })
}
pub(super) fn load(bytes: &[u8]) -> Result<Module, String> {
    if goff::is_goff(bytes) {
        return goff::load(bytes);
    }
    if bytes.is_empty() || !bytes.len().is_multiple_of(80) {
        return Err("OBJ input requires complete 80-byte card images".into());
    }
    if bytes.len() > 64 << 20 {
        return Err("OBJ input exceeds 64 MiB".into());
    }
    let mut deck = Deck::default();
    let mut modules = Vec::new();
    let mut pending = false;
    for (index, c) in bytes.chunks_exact(80).enumerate() {
        let result = (|| {
            if c[0] != 2 {
                return Err("invalid OBJ record prefix".into());
            }
            match name(&c[1..4]).as_str() {
                "ESD" => {
                    pending = true;
                    deck.esd(c)
                }
                "TXT" => {
                    pending = true;
                    deck.txt(c)
                }
                "RLD" => {
                    pending = true;
                    deck.rld(c)
                }
                "SYM" => {
                    pending = true;
                    Ok(())
                }
                "END" => {
                    deck.end(c)?;
                    modules.push(std::mem::take(&mut deck).module()?);
                    pending = false;
                    Ok(())
                }
                "XSD" => Err("OBJ XSD extended names are not yet supported".into()),
                _ => Err("unknown OBJ record type".into()),
            }
        })();
        result.map_err(|e: String| format!("OBJ record {}: {e}", index + 1))?;
    }
    if pending || modules.is_empty() {
        return Err("OBJ input is missing its END record".into());
    }
    if modules.len() == 1 {
        Ok(modules.remove(0))
    } else {
        super::link_modules(&modules)
    }
}

/// Shared architectural encoder for ADATA instruction records.
pub(super) fn instruction_bytes(i: &super::Instruction) -> Result<Vec<u8>, String> {
    write::instructions::encode(i)
}

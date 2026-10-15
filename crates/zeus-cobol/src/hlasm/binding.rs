//! Zeus bind-time section quantities. No IBM object records are generated.
use super::Module;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Bindings {
    pub sections: Vec<Section>,
    pub quantities: Vec<Quantity>,
    #[serde(default)]
    pub psects: Vec<Psect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub labels: Vec<Label>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    pub name: String,
    pub length: u32,
    pub alignment: u32,
    pub dummy: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default)]
    pub concatenate: bool,
    #[serde(default)]
    pub priority: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<u32>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantityKind {
    Offset,
    LongOffset,
    Length,
    Total,
    Psect,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Psect {
    pub symbol: String,
    pub offset: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub class: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub symbol: String,
    pub offset: u32,
    pub section: u32,
    pub class: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quantity {
    pub offset: u32,
    pub width: u8,
    pub kind: QuantityKind,
    pub symbol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub element: Option<usize>,
    #[serde(default)]
    pub addend: u32,
}
impl Bindings {
    pub fn is_empty(&self) -> bool {
        !self.requires_extended() && self.labels.is_empty()
    }
    pub(super) fn requires_extended(&self) -> bool {
        !self.sections.is_empty() || !self.quantities.is_empty() || !self.psects.is_empty()
    }
    pub(super) fn merge(&mut self, other: &Self, origin: u32) {
        let section_count = self.sections.len();
        self.sections
            .extend(other.sections.iter().cloned().map(|mut s| {
                s.image = s.image.map(|p| p + origin);
                s
            }));
        self.labels
            .extend(other.labels.iter().cloned().map(|mut l| {
                l.offset += origin;
                l.section += origin;
                l
            }));
        self.psects
            .extend(other.psects.iter().cloned().map(|mut p| {
                if p.target.is_none() {
                    p.offset += origin;
                }
                p
            }));
        self.quantities
            .extend(other.quantities.iter().cloned().map(|mut q| {
                q.offset += origin;
                q.element = q.element.map(|i| i + section_count);
                q
            }));
    }
    pub(super) fn validate(&self, module: &Module) -> Result<(), String> {
        if self.requires_extended() && module.zeus_hlasm < 9 {
            return Err("section quantities require HLASM module version 9".into());
        }
        for s in &self.sections {
            if !super::assemble::symbol_name(&s.name)
                || !s.alignment.is_power_of_two()
                || s.alignment > 4096
                || s.length > 16 << 20
                || s.class
                    .as_ref()
                    .is_some_and(|name| !super::assemble::symbol_name(name))
                || s.priority > 0x7fffffff
                || s.image.is_some_and(|at| {
                    (at as usize)
                        .checked_add(s.length as usize)
                        .is_none_or(|end| end > module.storage.len())
                })
            {
                return Err("invalid section quantity definition".into());
            }
        }
        for label in &self.labels {
            if !super::assemble::symbol_name(&label.symbol)
                || label.offset as usize >= module.storage.len()
                || label.section as usize >= module.storage.len()
                || label
                    .class
                    .as_ref()
                    .is_some_and(|s| !super::assemble::symbol_name(s))
            {
                return Err("invalid binding label metadata".into());
            }
        }
        let mut names = std::collections::HashSet::new();
        for p in &self.psects {
            if !super::assemble::symbol_name(&p.symbol)
                || !names.insert(&p.symbol)
                || p.class
                    .as_ref()
                    .is_some_and(|s| !super::assemble::symbol_name(s))
                || p.target
                    .as_ref()
                    .is_some_and(|s| !super::assemble::symbol_name(s))
                || (p.target.is_none()
                    && (p.offset as usize >= module.storage.len()
                        || module.read_only.iter().any(|r| r.contains(&p.offset))))
            {
                return Err("invalid or duplicate PSECT association".into());
            }
        }
        for q in &self.quantities {
            if q.element.is_some_and(|i| {
                i >= self.sections.len()
                    || !matches!(q.kind, QuantityKind::Offset | QuantityKind::LongOffset)
                    || (!self.sections[i].concatenate && self.sections[i].class.is_none())
            }) || !(1..=8).contains(&q.width)
                || (matches!(q.kind, QuantityKind::LongOffset) && q.width != 3)
                || (matches!(q.kind, QuantityKind::Total) && (q.width != 4 || !q.symbol.is_empty()))
                || (!matches!(q.kind, QuantityKind::Total)
                    && !super::assemble::symbol_name(&q.symbol))
                || (q.offset as usize)
                    .checked_add(q.width as usize)
                    .is_none_or(|end| end > module.storage.len())
            {
                return Err("invalid section quantity reference".into());
            }
        }
        Ok(())
    }
    pub(super) fn initialize(
        &self,
        module: &Module,
        storage: &mut [u8],
        base: usize,
    ) -> Result<(), String> {
        self.validate(module)?;
        // Mapped Zeus images keep their instruction/data layout. A mapped
        // class spans its actual elements, retaining inter-element image gaps.
        // Q, J and R must all describe that same physical span.
        let mut mapped = BTreeMap::<String, (u32, u32)>::new();
        for section in &self.sections {
            if section.concatenate
                && let Some(start) = section.image
            {
                let end = start
                    .checked_add(section.length)
                    .ok_or("class extent overflow")?;
                mapped
                    .entry(section.name.clone())
                    .and_modify(|range| {
                        range.0 = range.0.min(start);
                        range.1 = range.1.max(end);
                    })
                    .or_insert((start, end));
            }
        }
        let mut merged = BTreeMap::<String, Section>::new();
        let mut order = Vec::new();
        let mut element_positions = Vec::new();
        for s in &self.sections {
            if let Some(old) = merged.get_mut(&s.name) {
                if old.dummy != s.dummy || old.class != s.class || old.concatenate != s.concatenate
                {
                    return Err(format!(
                        "conflicting section quantity definition {}",
                        s.name
                    ));
                }
                element_positions.push(if s.concatenate {
                    old.length
                        .checked_add(s.alignment - 1)
                        .ok_or("class length overflow")?
                        & !(s.alignment - 1)
                } else {
                    0
                });
                old.length = if old.concatenate && s.concatenate {
                    old.length
                        .checked_add(s.alignment - 1)
                        .map(|length| length & !(s.alignment - 1))
                        .and_then(|length| length.checked_add(s.length))
                        .filter(|n| *n <= 16 << 20)
                        .ok_or("class length exceeds 16 MiB")?
                } else {
                    old.length.max(s.length)
                };
                old.alignment = old.alignment.max(s.alignment);
                old.priority = old.priority.max(s.priority);
            } else {
                element_positions.push(0);
                order.push(s.name.clone());
                merged.insert(s.name.clone(), s.clone());
            }
        }
        for (name, (start, end)) in &mapped {
            merged.get_mut(name).ok_or("missing mapped class")?.length = end - start;
        }
        let mut total = 0u32;
        let mut class_lengths = BTreeMap::<String, u32>::new();
        let mut offsets = BTreeMap::new();
        order.sort_by_key(|name| std::cmp::Reverse(merged[name].priority));
        for name in order {
            let s = &merged[&name];
            if s.dummy {
                let position =
                    if let Some(class) = s.class.as_ref().filter(|name| name.as_str() != "B_PRV") {
                        class_lengths.entry(class.clone()).or_default()
                    } else {
                        &mut total
                    };
                *position = position
                    .checked_add(s.alignment - 1)
                    .ok_or("dummy section offset overflow")?
                    & !(s.alignment - 1);
                offsets.insert(name, *position);
                *position = position
                    .checked_add(s.length)
                    .filter(|n| *n <= 16 << 20)
                    .ok_or("external dummy storage exceeds 16 MiB")?;
            } else if s.concatenate {
                offsets.insert(name, 0);
            }
        }
        for (name, length) in class_lengths {
            offsets.insert(name.clone(), 0);
            if let Some(section) = merged.get_mut(&name) {
                section.length = length;
            } else {
                merged.insert(
                    name.clone(),
                    Section {
                        name,
                        length,
                        alignment: 1,
                        dummy: false,
                        class: None,
                        concatenate: true,
                        priority: 0,
                        image: None,
                    },
                );
            }
        }
        for q in &self.quantities {
            let value = if let Some(index) = q.element {
                let section = &self.sections[index];
                let origin = if section.concatenate {
                    if let Some(image) = section.image {
                        image - mapped[&section.name].0
                    } else {
                        element_positions[index]
                    }
                } else {
                    *offsets
                        .get(&section.name)
                        .ok_or("unresolved class part offset")?
                };
                origin
                    .checked_add(q.addend)
                    .ok_or("class label offset overflow")?
            } else {
                match q.kind {
                    QuantityKind::Psect => {
                        let association = self
                            .psects
                            .iter()
                            .find(|p| p.symbol == q.symbol)
                            .ok_or_else(|| format!("unresolved PSECT association {}", q.symbol))?;
                        let (offset, class) = if let Some(name) = &association.target {
                            let label = self
                                .labels
                                .iter()
                                .find(|label| label.symbol == *name)
                                .ok_or_else(|| format!("unresolved PSECT target {name}"))?;
                            (label.section, label.class.as_ref())
                        } else {
                            (association.offset, association.class.as_ref())
                        };
                        let offset = if let Some(class) = class {
                            mapped.get(class).ok_or("PSECT class is not mapped")?.0
                        } else {
                            offset
                        };
                        if module.read_only.iter().any(|r| r.contains(&offset)) {
                            return Err("PSECT must designate writable storage".into());
                        }
                        u32::try_from(base)
                            .ok()
                            .and_then(|base| base.checked_add(offset))
                            .ok_or("PSECT address overflow")?
                    }
                    QuantityKind::Total => total,
                    QuantityKind::Length => merged
                        .get(&q.symbol)
                        .map(|s| s.length)
                        .ok_or_else(|| format!("unresolved length constant {}", q.symbol))?,
                    QuantityKind::Offset | QuantityKind::LongOffset => {
                        if let Some(offset) = offsets.get(&q.symbol) {
                            *offset
                        } else if let Some(label) = self
                            .labels
                            .iter()
                            .find(|l| l.symbol == q.symbol && l.class.is_some())
                        {
                            let start = mapped
                                .get(label.class.as_ref().unwrap())
                                .ok_or("unmapped class label")?
                                .0;
                            label
                                .offset
                                .checked_sub(start)
                                .ok_or("invalid class label offset")?
                        } else {
                            return Err(format!("unresolved external dummy offset {}", q.symbol));
                        }
                    }
                }
            };
            let bytes = &mut storage[q.offset as usize..q.offset as usize + q.width as usize];
            if matches!(q.kind, QuantityKind::LongOffset) {
                if value > 524287 {
                    return Err(format!(
                        "QY offset out of signed 20-bit range: {}",
                        q.symbol
                    ));
                }
                bytes.copy_from_slice(&[
                    ((value >> 8) & 15) as u8,
                    value as u8,
                    (value >> 12) as u8,
                ]);
            } else {
                if matches!(q.kind, QuantityKind::Psect) && q.width == 3 && value > 0xffffff {
                    return Err("3-byte PSECT address exceeds 24 bits".into());
                }
                bytes.copy_from_slice(&u64::from(value).to_be_bytes()[8 - q.width as usize..]);
            }
        }
        Ok(())
    }
}

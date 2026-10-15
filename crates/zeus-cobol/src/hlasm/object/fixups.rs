//! Imported-object entry-mode tags and signed halfword-scaled displacements.
use crate::hlasm::{Module, RelocationExpression};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum AddressMode {
    Known(u8),
    External { symbol: String, weak: bool },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectRelocation {
    pub offset: u32,
    pub width: u8,
    pub expression: RelocationExpression,
    pub addressing_mode: AddressMode,
    /// Signed halfword-scaled instruction displacement; expression is in bytes.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub relative_immediate: bool,
}
pub(in crate::hlasm) fn validate(module: &Module) -> Result<(), String> {
    if !module.object_relocations.is_empty() && module.zeus_hlasm < 11 {
        return Err("object relocation attributes require module version 11".into());
    }
    let mut ranges = Vec::new();
    for r in &module.object_relocations {
        if r.relative_immediate && module.zeus_hlasm < 12 {
            return Err("relative-immediate object relocation requires module version 12".into());
        }
        if !(if r.relative_immediate {
            matches!(r.width, 2 | 4)
        } else {
            matches!(r.width, 4 | 8)
        }) || r
            .offset
            .checked_add(u32::from(r.width))
            .is_none_or(|end| end as usize > module.storage.len())
        {
            return Err("invalid mode-sensitive object relocation".into());
        }
        if r.relative_immediate && r.addressing_mode != AddressMode::Known(24) {
            return Err("relative-immediate relocation cannot carry entry-mode tags".into());
        }
        if let AddressMode::Known(mode) = r.addressing_mode {
            crate::hlasm::abi::mask(mode)?;
        }
        let end = r.offset + u32::from(r.width);
        if ranges.iter().any(|&(at, n)| at < end && r.offset < n)
            || module.relocations.iter().any(|other| {
                other.offset < end
                    && u64::from(r.offset) < u64::from(other.offset) + u64::from(other.width)
            })
        {
            return Err("overlapping object relocation fields".into());
        }
        ranges.push((r.offset, end));
    }
    Ok(())
}
pub(in crate::hlasm) fn initialize(
    module: &Module,
    image: &mut [u8],
    base: usize,
) -> Result<(), String> {
    validate(module)?;
    for r in &module.object_relocations {
        let value = r.expression.value_at(base)?;
        if r.relative_immediate {
            if value & 1 != 0 {
                return Err("relative-immediate target is not halfword aligned".into());
            }
            let displacement = value / 2;
            let bits = u32::from(r.width) * 8;
            if displacement < -(1i64 << (bits - 1)) || displacement >= (1i64 << (bits - 1)) {
                return Err("relative-immediate displacement exceeds signed field width".into());
            }
            let at = r.offset as usize;
            image[at..at + r.width as usize]
                .copy_from_slice(&displacement.to_be_bytes()[8 - r.width as usize..]);
            continue;
        }
        let mode = match &r.addressing_mode {
            AddressMode::Known(mode) => *mode,
            AddressMode::External { weak: true, .. } => 24,
            AddressMode::External { symbol, .. } => {
                return Err(format!("unresolved entry mode for {symbol}"));
            }
        };
        let value = u64::try_from(value).map_err(|_| "negative mode-sensitive entry address")?;
        if value > crate::hlasm::abi::mask(mode)? {
            return Err("mode-sensitive entry address exceeds its addressing mode".into());
        }
        if r.width < 8 && value > u64::from(u32::MAX) {
            return Err("mode-sensitive entry address exceeds field width".into());
        }
        let tagged = match mode {
            31 => value | 0x80000000,
            64 => value | 1,
            _ => value,
        };
        let at = r.offset as usize;
        image[at..at + r.width as usize]
            .copy_from_slice(&tagged.to_be_bytes()[8 - r.width as usize..]);
    }
    Ok(())
}
pub(in crate::hlasm) fn rebase(r: &mut ObjectRelocation, origin: u32) -> Result<(), String> {
    r.offset = r
        .offset
        .checked_add(origin)
        .ok_or("object relocation offset overflow")?;
    r.expression.rebase(origin as usize)
}
pub(in crate::hlasm) fn resolve(
    r: &mut ObjectRelocation,
    entries: &HashMap<String, u32>,
    modes: &BTreeMap<u32, u8>,
    default_mode: u8,
) -> Result<(), String> {
    r.expression.resolve(entries)?;
    if let AddressMode::External { symbol, weak } = &r.addressing_mode {
        r.addressing_mode = if let Some(at) = entries.get(symbol) {
            AddressMode::Known(modes.get(at).copied().unwrap_or(default_mode))
        } else if *weak {
            AddressMode::Known(24)
        } else {
            return Err(format!("unresolved entry mode for {symbol}"));
        };
    }
    Ok(())
}

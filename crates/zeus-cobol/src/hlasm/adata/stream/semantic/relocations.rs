//! Normalized image ESD identities and ordinary 1..8-byte signed RLD terms.
//! The published ADATA format intentionally omits some GOFF relocation types.
use super::super::{AdataRecord, text, word};
use crate::hlasm::{AssemblyReport, RelocationExpression};
use std::collections::{BTreeMap, BTreeSet};
pub(super) fn emit(
    out: &mut Vec<AdataRecord>,
    report: &AssemblyReport,
    ccsid: u16,
) -> Result<(), String> {
    let Some(module) = &report.module else {
        return Ok(());
    };
    let mut external: BTreeSet<String> = module.externals.keys().cloned().collect();
    for r in &module.relocations {
        if let Some(name) = &r.symbol {
            external.insert(name.clone());
        }
        if let Some(e) = &r.expression {
            external.extend(e.terms.iter().map(|t| t.symbol.clone()));
        }
    }
    for r in &module.object_relocations {
        external.extend(r.expression.terms.iter().map(|t| t.symbol.clone()));
    }
    let mut ids = BTreeMap::new();
    for (index, name) in external.into_iter().enumerate() {
        let id = index as u32 + 2;
        ids.insert(name.clone(), id);
        let mut data = vec![0; 52];
        data[0] = if module.externals.get(&name) == Some(&true) {
            0x0a
        } else {
            2
        };
        data[1] = 0x40;
        word(&mut data, 4, id);
        text(&mut data, 36, &name, ccsid)?;
        out.push(AdataRecord::new(0x20, 1, data));
    }
    let mut entries: Vec<_> = module
        .entries
        .iter()
        .filter(|(n, _)| **n != module.name)
        .collect();
    entries.sort_by_key(|(name, _)| *name);
    for (name, address) in entries {
        let mut data = vec![0; 52];
        data[0] = 1;
        data[1] = 0x40;
        word(&mut data, 12, *address);
        word(&mut data, 24, 1);
        text(&mut data, 36, name, ccsid)?;
        out.push(AdataRecord::new(0x20, 1, data));
    }
    let mut emit =
        |offset: u32, width: u8, local: i64, terms: &[(u32, i64, u8)]| -> Result<(), String> {
            if !(1..=8).contains(&width) {
                return Err("ADATA RLD width must be 1..8 bytes".into());
            }
            for (referent, coefficient, adcon_type) in
                std::iter::once((1, local, 0)).chain(terms.iter().copied())
            {
                if coefficient.unsigned_abs() > 1_000_000 {
                    return Err("ADATA RLD coefficient exceeds one million".into());
                }
                for _ in 0..coefficient.unsigned_abs() {
                    let mut data = vec![0; 17];
                    word(&mut data, 0, 1);
                    word(&mut data, 4, referent);
                    word(&mut data, 12, offset);
                    data[16] = ((width - 1) & 3) << 2
                        | if width > 4 { 0x40 } else { 0 }
                        | if coefficient < 0 { 2 } else { 0 }
                        | adcon_type;
                    out.push(AdataRecord::new(0x40, 1, data));
                }
            }
            Ok(())
        };
    let adcon_type = |offset: u32| -> u8 {
        report
            .listing
            .as_ref()
            .and_then(|listing| {
                listing
                    .statements
                    .iter()
                    .filter(|row| row.section.is_empty())
                    .flat_map(|row| &row.data_operands)
                    .find(|operand| {
                        let bits: u64 = operand
                            .values
                            .iter()
                            .map(|v| {
                                if v.bit_length == 0 {
                                    u64::from(v.byte_length) * 8
                                } else {
                                    u64::from(v.bit_length)
                                }
                            })
                            .sum();
                        u64::from(offset) >= u64::from(operand.offset)
                            && u64::from(offset)
                                < u64::from(operand.offset)
                                    + (bits * u64::from(operand.duplication)).div_ceil(8)
                    })
            })
            .map_or(0, |operand| {
                match operand.assembler_type.as_bytes().first() {
                    Some(b'V' | b'R') => 0x10,
                    Some(b'Q') => 0x20,
                    _ => 0,
                }
            })
    };
    let expression = |e: &RelocationExpression, kind: u8| -> Result<Vec<(u32, i64, u8)>, String> {
        e.terms
            .iter()
            .map(|t| {
                Ok((
                    *ids.get(&t.symbol)
                        .ok_or("ADATA missing external ESD identity")?,
                    t.coefficient,
                    kind,
                ))
            })
            .collect()
    };
    for r in &module.relocations {
        if let Some(e) = &r.expression {
            emit(
                r.offset,
                r.width,
                e.local_coefficient,
                &expression(e, adcon_type(r.offset))?,
            )?;
        } else if let Some(name) = &r.symbol {
            emit(
                r.offset,
                r.width,
                0,
                &[(
                    *ids.get(name).ok_or("ADATA missing external identity")?,
                    1,
                    adcon_type(r.offset),
                )],
            )?;
        } else {
            emit(r.offset, r.width, 1, &[])?;
        }
    }
    for r in &module.object_relocations {
        if !r.relative_immediate {
            emit(
                r.offset,
                r.width,
                r.expression.local_coefficient,
                &expression(&r.expression, adcon_type(r.offset))?,
            )?;
        }
    }
    Ok(())
}

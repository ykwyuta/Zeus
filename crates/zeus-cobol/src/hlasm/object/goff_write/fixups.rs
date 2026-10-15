//! Convert normalized linear expressions to GOFF R-address / R-RI items.
use super::layout::{self, Element};
use crate::hlasm::{ObjectAddressMode, RelocationExpression, operation};
use std::collections::BTreeMap;

pub(super) struct Writer<'a> {
    storage: &'a mut [u8],
    labels: &'a BTreeMap<String, (u32, u32, u8)>,
    externals: &'a BTreeMap<String, u32>,
    origin: u32,
    items: Vec<u8>,
    elements: &'a [Element],
}
impl<'a> Writer<'a> {
    pub fn new(
        storage: &'a mut [u8],
        labels: &'a BTreeMap<String, (u32, u32, u8)>,
        externals: &'a BTreeMap<String, u32>,
        origin: u32,
        elements: &'a [Element],
    ) -> Self {
        Self {
            storage,
            labels,
            externals,
            origin,
            items: vec![],
            elements,
        }
    }
    fn store(&mut self, at: u32, width: u8, value: i64) -> Result<(), String> {
        if !(1..=8).contains(&width) {
            return Err("GOFF output relocation width must be 1..8 bytes".into());
        }
        let end = (at as usize)
            .checked_add(width as usize)
            .ok_or("GOFF output relocation overflow")?;
        let field = self
            .storage
            .get_mut(at as usize..end)
            .ok_or("GOFF output relocation exceeds its image")?;
        field.copy_from_slice(&value.to_be_bytes()[8 - width as usize..]);
        Ok(())
    }
    fn item(
        &mut self,
        at: u32,
        width: u8,
        reference: u32,
        coefficient: i64,
        mode: bool,
        relative: bool,
    ) -> Result<(), String> {
        if coefficient.unsigned_abs() > 1_000_000
            || self.items.len() as u64 + coefficient.unsigned_abs() * 20 > 32 << 20
        {
            return Err("GOFF output relocation dictionary exceeds its size limit".into());
        }
        let (position, offset) = layout::position(self.elements, at, u32::from(width))?;
        for _ in 0..coefficient.unsigned_abs() {
            let mut b = [0; 20];
            b[0] = u8::from(mode);
            b[1] = if relative { 0x60 } else { 0 };
            b[2] = if coefficient < 0 { 2 } else { 0 };
            b[4] = width;
            b[8..12].copy_from_slice(&reference.to_be_bytes());
            b[12..16].copy_from_slice(&position.to_be_bytes());
            b[16..20].copy_from_slice(&offset.to_be_bytes());
            self.items.extend(b);
        }
        Ok(())
    }
    pub fn address(
        &mut self,
        at: u32,
        width: u8,
        mut e: RelocationExpression,
        mode: Option<ObjectAddressMode>,
    ) -> Result<(), String> {
        let tagged = if let Some(mode) = mode {
            match mode {
                ObjectAddressMode::Known(mode) => {
                    let (_, &(id, offset, _)) = self.labels.iter()
                        .filter(|(_, (_, _, m))| *m == mode)
                        .min_by_key(|(_, (_, offset, _))| *offset)
                        .ok_or("GOFF output mode-sensitive reference needs an exported label with the target AMODE")?;
                    e.addend = e
                        .addend
                        .checked_sub(i64::from(offset))
                        .ok_or("GOFF output tagged addend overflow")?;
                    e.local_coefficient = e
                        .local_coefficient
                        .checked_sub(1)
                        .ok_or("GOFF output tagged coefficient overflow")?;
                    Some(id)
                }
                ObjectAddressMode::External { symbol, .. } => {
                    let term = e
                        .terms
                        .iter_mut()
                        .find(|t| t.symbol == symbol && t.coefficient > 0)
                        .ok_or(
                            "GOFF output mode-sensitive external must have an additive reference",
                        )?;
                    term.coefficient -= 1;
                    Some(
                        *self
                            .externals
                            .get(&symbol)
                            .ok_or("GOFF output undefined mode-sensitive reference")?,
                    )
                }
            }
        } else {
            None
        };
        // Ordinary adcons wrap to their field width. Tagged pointers in the
        // current normalized loader need an initializer that survives its fetch.
        if tagged.is_some()
            && width < 8
            && (e.addend < 0 || e.addend as u64 >= 1u64 << (width as u32 * 8))
        {
            return Err("GOFF output mode-sensitive initializer wraps its field width".into());
        }
        self.store(at, width, e.addend)?;
        self.item(at, width, self.origin, e.local_coefficient, false, false)?;
        for term in e.terms {
            let id = *self
                .externals
                .get(&term.symbol)
                .ok_or("GOFF output undefined external reference")?;
            self.item(at, width, id, term.coefficient, false, false)?;
        }
        if let Some(id) = tagged {
            self.item(at, width, id, 1, true, false)?;
        }
        Ok(())
    }
    pub fn relative(&mut self, at: u32, width: u8, e: RelocationExpression) -> Result<(), String> {
        if !matches!(width, 2 | 4) {
            return Err("GOFF output relative-immediate width must be 2 or 4".into());
        }
        let pc = at
            .checked_sub(2)
            .ok_or("GOFF output relative-immediate precedes its instruction")?;
        layout::position(self.elements, pc, u32::from(width) + 2)?;
        if e.terms.is_empty() && e.local_coefficient == 0 {
            operation::check_relative(e.addend, u32::from(width) * 8)?;
            return self.store(at, width, e.addend / 2);
        }
        if e.local_coefficient != -1 || e.terms.len() != 1 || e.terms[0].coefficient != 1 {
            return Err(
                "GOFF output relative-immediate requires one additive external label".into(),
            );
        }
        let addend = e
            .addend
            .checked_add(i64::from(pc))
            .ok_or("GOFF output relative-immediate addend overflow")?;
        operation::check_relative(addend, u32::from(width) * 8)?;
        self.store(at, width, addend / 2)?;
        let id = *self
            .externals
            .get(&e.terms[0].symbol)
            .ok_or("GOFF output undefined relative reference")?;
        self.item(at, width, id, 1, false, true)
    }
    pub fn finish(self) -> Vec<u8> {
        self.items
    }
}

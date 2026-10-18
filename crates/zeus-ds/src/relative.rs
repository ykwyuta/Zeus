//! Relative record data sets: records in numbered slots (1-based), as
//! VSAM RRDS. Stored in variable format with an empty record for each
//! unused slot.

use std::path::Path;

use crate::format::{self, RecordFormat};
use crate::{DsError, Rel, Result};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct RelativeDataset {
    slots: Vec<Option<Vec<u8>>>,
}

impl RelativeDataset {
    pub fn load(path: &Path) -> Result<Option<RelativeDataset>> {
        let Some(data) = format::read_file(path)? else {
            return Ok(None);
        };
        let slots = format::decode(RecordFormat::Variable, &data)?
            .into_iter()
            .map(|r| (!r.is_empty()).then_some(r))
            .collect();
        Ok(Some(RelativeDataset { slots }))
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for r in &self.slots {
            format::encode_into(
                RecordFormat::Variable,
                r.as_deref().unwrap_or(&[]),
                0,
                &mut out,
            );
        }
        out
    }

    pub fn store(&self, path: &Path) -> Result<()> {
        format::write_file(path, &self.to_bytes())
    }

    /// Highest slot number in use or allocated.
    pub fn len(&self) -> u64 {
        self.slots.len() as u64
    }

    pub fn is_empty(&self) -> bool {
        self.slots.iter().all(Option::is_none)
    }

    pub fn get(&self, n: u64) -> Option<&[u8]> {
        if n == 0 {
            return None;
        }
        self.slots.get(n as usize - 1).and_then(|r| r.as_deref())
    }

    /// Writes a new record in slot `n`.
    pub fn insert(&mut self, n: u64, rec: Vec<u8>) -> Result<()> {
        if n == 0 {
            return Err(DsError::InvalidRecordNumber);
        }
        let i = n as usize - 1;
        if self.slots.len() <= i {
            self.slots.resize(i + 1, None);
        }
        if self.slots[i].is_some() {
            return Err(DsError::DuplicateKey);
        }
        self.slots[i] = Some(rec);
        Ok(())
    }

    /// Writes a record after the last slot; returns its number.
    pub fn append(&mut self, rec: Vec<u8>) -> u64 {
        self.slots.push(Some(rec));
        self.slots.len() as u64
    }

    pub fn replace(&mut self, n: u64, rec: Vec<u8>) -> Result<()> {
        match n
            .checked_sub(1)
            .and_then(|i| self.slots.get_mut(i as usize))
        {
            Some(slot @ Some(_)) => {
                *slot = Some(rec);
                Ok(())
            }
            _ => Err(DsError::NotFound),
        }
    }

    pub fn delete(&mut self, n: u64) -> Result<Vec<u8>> {
        n.checked_sub(1)
            .and_then(|i| self.slots.get_mut(i as usize))
            .and_then(Option::take)
            .ok_or(DsError::NotFound)
    }

    /// The first record in a slot numbered `from` or higher.
    pub fn next_from(&self, from: u64) -> Option<(u64, &[u8])> {
        let start = from.max(1) as usize - 1;
        self.slots
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(i, r)| r.as_deref().map(|r| (i as u64 + 1, r)))
    }

    /// The first record whose number satisfies `rel` with `key`.
    pub fn position(&self, rel: Rel, key: u64) -> Option<u64> {
        let from = match rel {
            Rel::Eq | Rel::Ge => key,
            Rel::Gt => key + 1,
        };
        let (n, _) = self.next_from(from)?;
        (rel != Rel::Eq || n == key).then_some(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots() {
        let mut d = RelativeDataset::default();
        d.insert(3, b"C".to_vec()).unwrap();
        assert_eq!(d.insert(3, b"X".to_vec()), Err(DsError::DuplicateKey));
        assert_eq!(
            d.insert(0, b"X".to_vec()),
            Err(DsError::InvalidRecordNumber)
        );
        assert_eq!(d.append(b"D".to_vec()), 4);
        assert_eq!(d.next_from(1), Some((3, &b"C"[..])));
        assert_eq!(d.position(Rel::Gt, 3), Some(4));
        assert_eq!(d.position(Rel::Eq, 2), None);
        assert_eq!(d.delete(3).unwrap(), b"C");
        assert_eq!(d.replace(3, vec![]), Err(DsError::NotFound));
        let bytes = d.to_bytes();
        assert_eq!(bytes.len(), 4 * 3 + 5);
    }
}

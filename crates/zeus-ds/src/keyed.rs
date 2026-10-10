//! Key-sequenced data sets with alternate indexes (as VSAM KSDS + AIX).
//!
//! Keys are fields of the record given by offset and length; key 0 is the
//! primary key, the others are alternate keys, unique or allowing
//! duplicates. Records with equal alternate keys are kept in the order
//! they were written (a record whose alternate key changes on REPLACE
//! moves behind the records already having its new key value).
//!
//! The data set is stored as variable-format records in write order.

use std::collections::BTreeMap;
use std::ops::Bound;
use std::path::Path;

use crate::format::{self, RecordFormat};
use crate::{DsError, Rel, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeySpec {
    pub offset: usize,
    pub len: usize,
    /// Alternate key allowing duplicates.
    pub duplicates: bool,
}

/// Position in the order of a key of reference: key value and, for
/// alternate keys, the write order among duplicates.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub key: Vec<u8>,
    pub seq: u64,
}

/// A record found by a positioning or browse request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub position: Position,
    pub primary: Vec<u8>,
    pub record: Vec<u8>,
}

/// Alternate index: (key, write sequence) -> primary key.
type AltIndex = BTreeMap<(Vec<u8>, u64), Vec<u8>>;

#[derive(Debug, Clone)]
pub struct KeyedDataset {
    keys: Vec<KeySpec>,
    /// Pad byte of a key extending past the end of a short record.
    pad: u8,
    /// Primary key -> (record, write sequence).
    primary: BTreeMap<Vec<u8>, (Vec<u8>, u64)>,
    /// One index per alternate key: (key, write sequence) -> primary key.
    alternates: Vec<AltIndex>,
    next_seq: u64,
}

impl KeyedDataset {
    /// An empty data set; `keys[0]` is the primary key.
    pub fn new(keys: Vec<KeySpec>, pad: u8) -> KeyedDataset {
        assert!(!keys.is_empty(), "a keyed data set needs a primary key");
        KeyedDataset {
            alternates: vec![BTreeMap::new(); keys.len() - 1],
            keys,
            pad,
            primary: BTreeMap::new(),
            next_seq: 0,
        }
    }

    /// Loads a data set; None when the file does not exist.
    pub fn load(path: &Path, keys: Vec<KeySpec>, pad: u8) -> Result<Option<KeyedDataset>> {
        let Some(data) = format::read_file(path)? else {
            return Ok(None);
        };
        let mut d = KeyedDataset::new(keys, pad);
        for r in format::decode(RecordFormat::Variable, &data)? {
            d.insert(r)
                .map_err(|e| DsError::Damaged(format!("{}: {e}", path.display())))?;
        }
        Ok(Some(d))
    }

    /// Records in write order, variable format.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut all: Vec<&(Vec<u8>, u64)> = self.primary.values().collect();
        all.sort_by_key(|(_, s)| *s);
        let mut out = Vec::new();
        for (r, _) in all {
            format::encode_into(RecordFormat::Variable, r, 0, &mut out);
        }
        out
    }

    pub fn store(&self, path: &Path) -> Result<()> {
        format::write_file(path, &self.to_bytes())
    }

    pub fn keys(&self) -> &[KeySpec] {
        &self.keys
    }

    pub fn len(&self) -> usize {
        self.primary.len()
    }

    pub fn is_empty(&self) -> bool {
        self.primary.is_empty()
    }

    /// Value of key `which` in `rec`.
    pub fn key_of(&self, rec: &[u8], which: usize) -> Vec<u8> {
        let k = self.keys[which];
        let mut v = rec
            .get(k.offset..(k.offset + k.len).min(rec.len()))
            .map(<[u8]>::to_vec)
            .unwrap_or_default();
        v.resize(k.len, self.pad);
        v
    }

    pub fn get(&self, primary: &[u8]) -> Option<&[u8]> {
        self.primary.get(primary).map(|(r, _)| r.as_slice())
    }

    /// Highest primary key.
    pub fn last_primary(&self) -> Option<&[u8]> {
        self.primary.keys().next_back().map(Vec::as_slice)
    }

    /// Records in primary key order.
    pub fn records(&self) -> impl Iterator<Item = &[u8]> {
        self.primary.values().map(|(r, _)| r.as_slice())
    }

    fn check_unique(&self, rec: &[u8], except: Option<&[u8]>) -> Result<()> {
        for (i, idx) in self.alternates.iter().enumerate() {
            if self.keys[i + 1].duplicates {
                continue;
            }
            let k = self.key_of(rec, i + 1);
            let clash = idx
                .range((k.clone(), 0)..=(k, u64::MAX))
                .any(|(_, pk)| Some(pk.as_slice()) != except);
            if clash {
                return Err(DsError::DuplicateAlternateKey);
            }
        }
        Ok(())
    }

    fn index(&mut self, rec: &[u8], primary: &[u8], seq: u64) {
        for i in 0..self.alternates.len() {
            let k = self.key_of(rec, i + 1);
            self.alternates[i].insert((k, seq), primary.to_vec());
        }
    }

    fn unindex(&mut self, rec: &[u8], seq: u64) {
        for i in 0..self.alternates.len() {
            let k = self.key_of(rec, i + 1);
            self.alternates[i].remove(&(k, seq));
        }
    }

    /// Adds a record.
    pub fn insert(&mut self, rec: Vec<u8>) -> Result<()> {
        let pk = self.key_of(&rec, 0);
        if self.primary.contains_key(&pk) {
            return Err(DsError::DuplicateKey);
        }
        self.check_unique(&rec, None)?;
        let seq = self.next_seq;
        self.next_seq += 1;
        self.index(&rec, &pk, seq);
        self.primary.insert(pk, (rec, seq));
        Ok(())
    }

    /// Replaces the record with the same primary key.
    pub fn replace(&mut self, rec: Vec<u8>) -> Result<()> {
        let pk = self.key_of(&rec, 0);
        let Some((old, seq)) = self.primary.get(&pk).cloned() else {
            return Err(DsError::NotFound);
        };
        self.check_unique(&rec, Some(&pk))?;
        let moved = (1..self.keys.len()).any(|i| self.key_of(&old, i) != self.key_of(&rec, i));
        self.unindex(&old, seq);
        let seq = if moved {
            self.next_seq += 1;
            self.next_seq - 1
        } else {
            seq
        };
        self.index(&rec, &pk, seq);
        self.primary.insert(pk, (rec, seq));
        Ok(())
    }

    /// Removes the record with primary key `primary`.
    pub fn delete(&mut self, primary: &[u8]) -> Result<Vec<u8>> {
        let (rec, seq) = self.primary.remove(primary).ok_or(DsError::NotFound)?;
        self.unindex(&rec, seq);
        Ok(rec)
    }

    fn found(&self, which: usize, key: Vec<u8>, seq: u64, pk: &[u8]) -> Found {
        Found {
            position: Position {
                key,
                seq: if which == 0 { 0 } else { seq },
            },
            primary: pk.to_vec(),
            record: self.primary[pk].0.clone(),
        }
    }

    /// The first record at or after (`inclusive`) or after `from` in the
    /// order of key `which`; from the start when `from` is None.
    pub fn next(&self, which: usize, from: Option<&Position>, inclusive: bool) -> Option<Found> {
        if which == 0 {
            let lower = match from {
                None => Bound::Unbounded,
                Some(p) if inclusive => Bound::Included(p.key.clone()),
                Some(p) => Bound::Excluded(p.key.clone()),
            };
            let (pk, (_, seq)) = self
                .primary
                .range::<Vec<u8>, _>((lower, Bound::Unbounded))
                .next()?;
            return Some(self.found(0, pk.clone(), *seq, pk));
        }
        let idx = &self.alternates[which - 1];
        let lower = match from {
            None => Bound::Unbounded,
            Some(p) if inclusive => Bound::Included((p.key.clone(), p.seq)),
            Some(p) => Bound::Excluded((p.key.clone(), p.seq)),
        };
        let ((k, seq), pk) = idx.range((lower, Bound::Unbounded)).next()?;
        Some(self.found(which, k.clone(), *seq, pk))
    }

    /// Positions on the first record whose key `which`, compared on the
    /// length of `search` (a leading part of the key), satisfies `rel`.
    pub fn position(&self, which: usize, search: &[u8], rel: Rel) -> Option<Found> {
        let n = search.len();
        let start = Position {
            key: search.to_vec(),
            seq: 0,
        };
        let mut cur = self.next(which, Some(&start), true);
        while let Some(f) = cur {
            let prefix = &f.position.key[..n.min(f.position.key.len())];
            let ok = match rel {
                Rel::Eq => return (prefix == search).then_some(f),
                Rel::Ge => prefix >= search,
                Rel::Gt => prefix > search,
            };
            if ok {
                return Some(f);
            }
            cur = self.next(which, Some(&f.position), false);
        }
        None
    }

    /// The first record (in write order among duplicates) whose key
    /// `which` equals `key`.
    pub fn get_by(&self, which: usize, key: &[u8]) -> Option<Found> {
        let f = self.position(which, key, Rel::Eq)?;
        (f.position.key == key).then_some(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Records: 2-byte primary key, 1-byte alternate key with duplicates,
    /// 1-byte unique alternate key.
    fn ds() -> KeyedDataset {
        KeyedDataset::new(
            vec![
                KeySpec {
                    offset: 0,
                    len: 2,
                    duplicates: false,
                },
                KeySpec {
                    offset: 2,
                    len: 1,
                    duplicates: true,
                },
                KeySpec {
                    offset: 3,
                    len: 1,
                    duplicates: false,
                },
            ],
            b' ',
        )
    }

    #[test]
    fn insert_and_unique_keys() {
        let mut d = ds();
        d.insert(b"02Ax".to_vec()).unwrap();
        d.insert(b"01Ay".to_vec()).unwrap();
        assert_eq!(d.insert(b"01Bz".to_vec()), Err(DsError::DuplicateKey));
        assert_eq!(
            d.insert(b"03Bx".to_vec()),
            Err(DsError::DuplicateAlternateKey)
        );
        assert_eq!(d.get(b"01"), Some(&b"01Ay"[..]));
        assert_eq!(d.last_primary(), Some(&b"02"[..]));
    }

    #[test]
    fn duplicates_in_write_order_and_moves() {
        let mut d = ds();
        d.insert(b"02Ax".to_vec()).unwrap();
        d.insert(b"01Ay".to_vec()).unwrap();
        d.insert(b"03Bz".to_vec()).unwrap();
        let order = |d: &KeyedDataset| {
            let mut v = Vec::new();
            let mut f = d.next(1, None, false);
            while let Some(x) = f {
                v.push(x.primary.clone());
                f = d.next(1, Some(&x.position), false);
            }
            v
        };
        assert_eq!(order(&d), [b"02".to_vec(), b"01".to_vec(), b"03".to_vec()]);
        // 02 gets alternate key B: it follows 03
        d.replace(b"02Bx".to_vec()).unwrap();
        assert_eq!(order(&d), [b"01".to_vec(), b"03".to_vec(), b"02".to_vec()]);
        assert_eq!(d.get_by(1, b"B").unwrap().primary, b"03");
        d.delete(b"03").unwrap();
        assert_eq!(d.get_by(1, b"B").unwrap().primary, b"02");
        assert_eq!(d.replace(b"09Cq".to_vec()), Err(DsError::NotFound));
    }

    #[test]
    fn position_on_leading_part() {
        let mut d = ds();
        for r in [b"10Aa", b"12Ab", b"20Ac"] {
            d.insert(r.to_vec()).unwrap();
        }
        assert_eq!(d.position(0, b"1", Rel::Eq).unwrap().primary, b"10");
        assert_eq!(d.position(0, b"1", Rel::Gt).unwrap().primary, b"20");
        assert_eq!(d.position(0, b"11", Rel::Ge).unwrap().primary, b"12");
        assert!(d.position(0, b"3", Rel::Ge).is_none());
        assert!(d.position(0, b"11", Rel::Eq).is_none());
    }

    #[test]
    fn store_and_load_keep_duplicate_order() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("ks");
        let mut d = ds();
        d.insert(b"02Ax".to_vec()).unwrap();
        d.insert(b"01Ay".to_vec()).unwrap();
        d.store(&p).unwrap();
        let e = KeyedDataset::load(&p, d.keys().to_vec(), b' ')
            .unwrap()
            .unwrap();
        assert_eq!(e.next(1, None, false).unwrap().primary, b"02");
        assert!(
            KeyedDataset::load(&dir.path().join("none"), d.keys().to_vec(), b' ')
                .unwrap()
                .is_none()
        );
    }
}

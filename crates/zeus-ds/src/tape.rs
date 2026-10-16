//! Uncompressed AWSTAPE images, with physical blocks and tape marks.
use crate::{DsError, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    Block(Vec<u8>),
    Mark,
}

#[derive(Debug, Clone)]
pub struct TapeFile {
    pub start: usize,
    pub next: usize,
    pub blocks: Vec<Vec<u8>>,
}

fn label(block: &[u8], id: &[u8]) -> bool {
    // Labels use the invariant EBCDIC uppercase letters/digits.
    let e: Vec<u8> = id
        .iter()
        .map(|b| match b {
            b'A'..=b'I' => 0xc1 + (b - b'A'),
            b'J'..=b'R' => 0xd1 + (b - b'J'),
            b'S'..=b'Z' => 0xe2 + (b - b'S'),
            b'0'..=b'9' => 0xf0 + (b - b'0'),
            _ => 0x40,
        })
        .collect();
    block.len() == 80 && block.starts_with(&e)
}

pub fn file(entries: &[Entry], position: usize) -> Result<TapeFile> {
    let labeled = matches!(entries.first(), Some(Entry::Block(b)) if label(b, b"VOL1"));
    file_with_labels(entries, position, labeled)
}

pub fn file_with_labels(entries: &[Entry], position: usize, labeled: bool) -> Result<TapeFile> {
    let start = position;
    let mut pos = position;
    if labeled
        && position == 0
        && !matches!(entries.get(pos), Some(Entry::Block(b)) if label(b, b"VOL1"))
    {
        return Err(DsError::Damaged("missing VOL1 label".into()));
    }
    if labeled
        && let Some(Entry::Block(b)) = entries.get(pos)
        && label(b, b"VOL1")
    {
        pos += 1;
    }
    if labeled
        && (!matches!(entries.get(pos), Some(Entry::Block(b)) if label(b, b"HDR1"))
            || !matches!(entries.get(pos + 1), Some(Entry::Block(b)) if label(b, b"HDR2")))
    {
        return Err(DsError::Damaged("missing HDR1/HDR2 labels".into()));
    }
    if labeled
        && let Some(Entry::Block(b)) = entries.get(pos)
        && label(b, b"HDR1")
    {
        while matches!(entries.get(pos), Some(Entry::Block(_))) {
            pos += 1;
        }
        if entries.get(pos) != Some(&Entry::Mark) {
            return Err(DsError::Damaged("missing tape header mark".into()));
        }
        pos += 1;
    }
    let mut blocks = Vec::new();
    while let Some(Entry::Block(b)) = entries.get(pos) {
        blocks.push(b.clone());
        pos += 1;
    }
    if entries.get(pos) != Some(&Entry::Mark) {
        return Err(DsError::Damaged("missing data tape mark".into()));
    }
    pos += 1;
    if labeled
        && (!matches!(entries.get(pos), Some(Entry::Block(b)) if label(b, b"EOF1"))
            || !matches!(entries.get(pos + 1), Some(Entry::Block(b)) if label(b, b"EOF2")))
    {
        return Err(DsError::Damaged("missing EOF1/EOF2 labels".into()));
    }
    if labeled
        && let Some(Entry::Block(b)) = entries.get(pos)
        && label(b, b"EOF1")
    {
        while matches!(entries.get(pos), Some(Entry::Block(_))) {
            pos += 1;
        }
        if entries.get(pos) != Some(&Entry::Mark) {
            return Err(DsError::Damaged("missing trailer tape mark".into()));
        }
        pos += 1;
    }
    Ok(TapeFile {
        start,
        next: pos,
        blocks,
    })
}

pub fn sequence(entries: &[Entry], number: usize) -> Result<usize> {
    let labeled = matches!(entries.first(), Some(Entry::Block(b)) if label(b, b"VOL1"));
    sequence_with_labels(entries, number, labeled)
}

pub fn sequence_with_labels(entries: &[Entry], number: usize, labeled: bool) -> Result<usize> {
    if number == 0 {
        return Err(DsError::Damaged(
            "tape file sequence must be positive".into(),
        ));
    }
    let mut position = 0;
    for _ in 1..number {
        position = file_with_labels(entries, position, labeled)?.next;
    }
    Ok(position)
}

pub fn decode(bytes: &[u8]) -> Result<Vec<Entry>> {
    let bad = || DsError::Damaged("invalid AWSTAPE header/segment".into());
    let mut entries = Vec::new();
    let mut offset = 0;
    let mut previous = 0;
    let mut pending: Option<Vec<u8>> = None;
    while offset < bytes.len() {
        let h = bytes.get(offset..offset + 6).ok_or_else(bad)?;
        let n = u16::from_le_bytes([h[0], h[1]]) as usize;
        let prev = u16::from_le_bytes([h[2], h[3]]) as usize;
        if prev != previous || h[5] != 0 || h[4] & !0xe0 != 0 {
            return Err(bad());
        }
        offset += 6;
        let data = bytes.get(offset..offset + n).ok_or_else(bad)?;
        if h[4] == 0x40 {
            if n != 0 || pending.is_some() {
                return Err(bad());
            }
            entries.push(Entry::Mark);
        } else {
            if n == 0 || h[4] & 0x40 != 0 {
                return Err(bad());
            }
            if h[4] & 0x80 != 0 {
                if pending.is_some() {
                    return Err(bad());
                }
                pending = Some(Vec::new());
            }
            pending.as_mut().ok_or_else(bad)?.extend_from_slice(data);
            if h[4] & 0x20 != 0 {
                entries.push(Entry::Block(pending.take().ok_or_else(bad)?));
            }
        }
        previous = n;
        offset += n;
    }
    if pending.is_some() {
        return Err(bad());
    }
    Ok(entries)
}

pub fn encode(entries: &[Entry]) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut previous = 0u16;
    for entry in entries {
        let (data, flag) = match entry {
            Entry::Block(b) => (b.as_slice(), 0xa0),
            Entry::Mark => (&[][..], 0x40),
        };
        let n = u16::try_from(data.len())
            .map_err(|_| DsError::Damaged("AWSTAPE block exceeds 65535 bytes".into()))?;
        if flag == 0xa0 && n == 0 {
            return Err(DsError::Damaged("empty AWSTAPE data block".into()));
        }
        bytes.extend_from_slice(&n.to_le_bytes());
        bytes.extend_from_slice(&previous.to_le_bytes());
        bytes.extend_from_slice(&[flag, 0]);
        bytes.extend_from_slice(data);
        previous = n;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aws_header_flags_lengths_marks_and_file_positions() {
        let bytes = b"\x03\0\0\0\xa0\0ABC\0\0\x03\0\x40\0\x02\0\0\0\xa0\0DE\0\0\x02\0\x40\0";
        let entries = decode(bytes).unwrap();
        assert_eq!(
            entries,
            vec![
                Entry::Block(b"ABC".to_vec()),
                Entry::Mark,
                Entry::Block(b"DE".to_vec()),
                Entry::Mark
            ]
        );
        assert_eq!(encode(&entries).unwrap(), bytes);
        assert_eq!(file(&entries, 0).unwrap().blocks, vec![b"ABC".to_vec()]);
        assert_eq!(sequence(&entries, 2).unwrap(), 2);
        assert_eq!(file(&entries, 2).unwrap().blocks, vec![b"DE".to_vec()]);
        let mut bad = bytes.to_vec();
        bad[11] = 4;
        assert!(decode(&bad).is_err());
        assert!(decode(&bytes[..bytes.len() - 1]).is_err());
    }
}

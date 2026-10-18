//! QSAM physical blocks. Lengths include the BDW/RDW/SDW, as on z/OS.
use crate::{DsError, Result};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Layout {
    pub mode: char,
    pub record_len: usize,
    pub block_size: usize,
    /// APPLY WRITE-ONLY: admit the next actual record instead of reserving
    /// room for a maximum-length record in every variable block.
    pub write_only: bool,
}

fn damaged(message: &str) -> DsError {
    DsError::Damaged(message.into())
}

fn bdw_length(bytes: &[u8]) -> Result<usize> {
    let header: [u8; 4] = bytes
        .get(..4)
        .ok_or_else(|| damaged("truncated BDW"))?
        .try_into()
        .unwrap();
    let length = if header[0] & 0x80 != 0 {
        (u32::from_be_bytes(header) & 0x7fff_ffff) as usize
    } else {
        let length = u16::from_be_bytes([header[0], header[1]]) as usize;
        if header[2..] != [0, 0] || length > 32760 {
            return Err(damaged("invalid nonextended BDW"));
        }
        length
    };
    if length < 8 {
        return Err(damaged("invalid BDW length"));
    }
    Ok(length)
}

impl Layout {
    pub fn validate(self) -> Result<()> {
        if self.block_size == 0 || self.block_size > 0x7fff_ffff || self.record_len == 0 {
            return Err(damaged("invalid QSAM block/record size"));
        }
        match self.mode {
            'F' if self.block_size >= self.record_len
                && self.block_size.is_multiple_of(self.record_len) =>
            {
                Ok(())
            }
            'V' if self.record_len <= 32756 && self.block_size >= self.record_len + 8 => Ok(()),
            'S' if (9..=32760).contains(&self.block_size) => Ok(()),
            'U' if self.block_size >= self.record_len => Ok(()),
            _ => Err(damaged(
                "record does not fit the selected physical block format",
            )),
        }
    }

    pub fn encode(self, records: &[Vec<u8>], pad: u8) -> Result<Vec<Vec<u8>>> {
        self.validate()?;
        let mut blocks = Vec::new();
        let mut block = if matches!(self.mode, 'V' | 'S') {
            vec![0; 4]
        } else {
            Vec::new()
        };
        let flush = |block: &mut Vec<u8>, blocks: &mut Vec<Vec<u8>>| {
            if matches!(self.mode, 'V' | 'S') {
                if block.len() == 4 {
                    return;
                }
                let length = block.len();
                if length > 32760 {
                    block[..4].copy_from_slice(&(length as u32 | 0x8000_0000).to_be_bytes());
                } else {
                    block[..2].copy_from_slice(&(length as u16).to_be_bytes());
                }
                blocks.push(std::mem::replace(block, vec![0; 4]));
            } else if !block.is_empty() {
                blocks.push(std::mem::take(block));
            }
        };
        for record in records {
            if record.len() > self.record_len || record.is_empty() && self.mode == 'S' {
                return Err(damaged("invalid logical record length"));
            }
            match self.mode {
                'F' => {
                    if block.len() + self.record_len > self.block_size {
                        flush(&mut block, &mut blocks);
                    }
                    block.extend_from_slice(record);
                    block.resize(block.len() + self.record_len - record.len(), pad);
                }
                'U' => blocks.push(record.clone()),
                'V' => {
                    let required = if self.write_only {
                        record.len()
                    } else {
                        self.record_len
                    } + 4;
                    if block.len() > 4 && block.len() + required > self.block_size {
                        flush(&mut block, &mut blocks);
                    }
                    block.extend_from_slice(&((record.len() + 4) as u16).to_be_bytes());
                    block.extend_from_slice(&[0, 0]);
                    block.extend_from_slice(record);
                }
                'S' => {
                    let mut offset = 0;
                    while offset < record.len() {
                        if self.block_size - block.len() <= 4 {
                            flush(&mut block, &mut blocks);
                        }
                        let n = (self.block_size - block.len() - 4).min(record.len() - offset);
                        let last = offset + n == record.len();
                        let flag = match (offset == 0, last) {
                            (true, true) => 0,
                            (true, false) => 1,
                            (false, true) => 2,
                            (false, false) => 3,
                        };
                        block.extend_from_slice(&((n + 4) as u16).to_be_bytes());
                        block.extend_from_slice(&[flag, 0]);
                        block.extend_from_slice(&record[offset..offset + n]);
                        offset += n;
                    }
                }
                _ => unreachable!(),
            }
        }
        flush(&mut block, &mut blocks);
        Ok(blocks)
    }

    pub fn decode(self, blocks: &[Vec<u8>]) -> Result<Vec<Vec<u8>>> {
        self.decode_with_blocks(blocks).map(|(records, _)| records)
    }

    /// Physical start/end block (zero based, inclusive) of each logical record.
    /// A volume can end in the middle of a spanned record.
    pub fn decode_with_blocks(
        self,
        blocks: &[Vec<u8>],
    ) -> Result<(Vec<Vec<u8>>, Vec<(usize, usize)>)> {
        self.validate()?;
        let mut records = Vec::new();
        let mut locations = Vec::new();
        let mut pending_start = 0;
        let mut pending: Option<Vec<u8>> = None;
        for (block_index, block) in blocks.iter().enumerate() {
            if block.len() > self.block_size {
                return Err(damaged("block exceeds BLKSIZE"));
            }
            if self.mode == 'U' {
                records.push(block.clone());
                locations.push((block_index, block_index));
                continue;
            }
            if self.mode == 'F' {
                if !block.len().is_multiple_of(self.record_len) {
                    return Err(damaged("short fixed record"));
                }
                records.extend(block.chunks(self.record_len).map(<[u8]>::to_vec));
                locations.extend(std::iter::repeat_n(
                    (block_index, block_index),
                    block.len() / self.record_len,
                ));
                continue;
            }
            if bdw_length(block)? != block.len() || self.mode == 'S' && block[0] & 0x80 != 0 {
                return Err(damaged("invalid BDW"));
            }
            let mut offset = 4;
            while offset < block.len() {
                if offset + 4 > block.len() {
                    return Err(damaged("truncated record descriptor"));
                }
                let n = u16::from_be_bytes([block[offset], block[offset + 1]]) as usize;
                let flag = block[offset + 2];
                if n < if self.mode == 'S' { 5 } else { 4 }
                    || offset + n > block.len()
                    || block[offset + 3] != 0
                    || flag > 3
                    || self.mode == 'V' && flag != 0
                {
                    return Err(damaged("invalid RDW/SDW"));
                }
                let data = &block[offset + 4..offset + n];
                match (flag, &mut pending) {
                    (0, None) => {
                        records.push(data.to_vec());
                        locations.push((block_index, block_index));
                    }
                    (1, None) => {
                        pending = Some(data.to_vec());
                        pending_start = block_index;
                    }
                    (2 | 3, Some(record)) => {
                        record.extend_from_slice(data);
                        if record.len() > self.record_len {
                            return Err(damaged("spanned record exceeds LRECL"));
                        }
                        if flag == 2 {
                            records.push(pending.take().unwrap());
                            locations.push((pending_start, block_index));
                        }
                    }
                    _ => return Err(damaged("out-of-order spanned segments")),
                }
                if records.last().is_some_and(|r| r.len() > self.record_len) {
                    return Err(damaged("record exceeds LRECL"));
                }
                offset += n;
            }
        }
        if pending.is_some() {
            return Err(damaged("unterminated spanned record"));
        }
        Ok((records, locations))
    }

    /// Raw F/V/S host files have inferable block boundaries. U must use a
    /// tape image because its variable block lengths have no descriptors.
    pub fn split(self, bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
        self.validate()?;
        if self.mode == 'F' {
            return Ok(bytes.chunks(self.block_size).map(<[u8]>::to_vec).collect());
        }
        if self.mode == 'U' {
            return Err(damaged(
                "mode U needs a tape image to preserve block boundaries",
            ));
        }
        let mut blocks = Vec::new();
        let mut offset = 0;
        while offset < bytes.len() {
            if offset + 4 > bytes.len() {
                return Err(damaged("truncated BDW"));
            }
            let n = bdw_length(&bytes[offset..])?;
            if n > self.block_size || offset + n > bytes.len() {
                return Err(damaged("invalid block length"));
            }
            blocks.push(bytes[offset..offset + n].to_vec());
            offset += n;
        }
        Ok(blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_variable_blocks_use_extended_bdw_and_small_tail_uses_classic_bdw() {
        let layout = Layout {
            mode: 'V',
            record_len: 32,
            block_size: 65536,
            write_only: false,
        };
        let records = vec![vec![0xc1; 32]; 2100];
        let blocks = layout.encode(&records, 0x40).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(
            u32::from_be_bytes(blocks[0][..4].try_into().unwrap()),
            0x8000_fff4
        );
        assert_eq!(blocks[1][0] & 0x80, 0);
        assert_eq!(layout.split(&blocks.concat()).unwrap(), blocks);
        assert_eq!(layout.decode(&blocks).unwrap(), records);
        let mut bad = blocks.clone();
        bad[0][0] &= 0x7f;
        assert!(layout.decode(&bad).is_err());
        assert!(
            Layout {
                mode: 'S',
                ..layout
            }
            .validate()
            .is_err()
        );
        assert!(
            Layout {
                record_len: 32757,
                ..layout
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn variable_blocks_match_known_descriptor_bytes_and_awo_packing() {
        let records = vec![b"AB".to_vec(), b"CD".to_vec()];
        let layout = Layout {
            mode: 'V',
            record_len: 8,
            block_size: 16,
            write_only: false,
        };
        let blocks = layout.encode(&records, 0x40).unwrap();
        assert_eq!(
            blocks,
            vec![
                b"\0\x0a\0\0\0\x06\0\0AB".to_vec(),
                b"\0\x0a\0\0\0\x06\0\0CD".to_vec()
            ]
        );
        let packed = Layout {
            write_only: true,
            ..layout
        }
        .encode(&records, 0x40)
        .unwrap();
        assert_eq!(packed, vec![b"\0\x10\0\0\0\x06\0\0AB\0\x06\0\0CD".to_vec()]);
        assert_eq!(layout.decode(&packed).unwrap(), records);
    }

    #[test]
    fn spanned_first_middle_last_and_damaged_sequence() {
        let layout = Layout {
            mode: 'S',
            record_len: 20,
            block_size: 12,
            write_only: false,
        };
        let blocks = layout.encode(&[b"ABCDEFGHIJ".to_vec()], 0x40).unwrap();
        assert_eq!(blocks[0], b"\0\x0c\0\0\0\x08\x01\0ABCD");
        assert_eq!(blocks[1], b"\0\x0c\0\0\0\x08\x03\0EFGH");
        assert_eq!(blocks[2], b"\0\x0a\0\0\0\x06\x02\0IJ");
        assert_eq!(
            layout.decode(&blocks).unwrap(),
            vec![b"ABCDEFGHIJ".to_vec()]
        );
        assert!(layout.decode(&blocks[..2]).is_err());
        assert!(layout.decode(&blocks[1..]).is_err());
        let mut bad = blocks.clone();
        bad[0][2] = 1;
        assert!(layout.decode(&bad).is_err());
    }

    #[test]
    fn fixed_and_undefined_preserve_their_physical_boundaries() {
        let records = vec![b"ABC".to_vec(), b"DEF".to_vec(), b"GHI".to_vec()];
        let layout = Layout {
            mode: 'F',
            record_len: 3,
            block_size: 6,
            write_only: false,
        };
        assert_eq!(
            layout.encode(&records, 0x40).unwrap(),
            vec![b"ABCDEF".to_vec(), b"GHI".to_vec()]
        );
        let u = Layout {
            mode: 'U',
            ..layout
        };
        assert_eq!(u.encode(&records, 0x40).unwrap(), records);
        assert_eq!(u.decode(&records).unwrap(), records);
        assert!(u.split(b"ABCDEF").is_err());
    }
}

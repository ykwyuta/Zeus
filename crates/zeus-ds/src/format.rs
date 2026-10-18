//! On-disk record formats.
//!
//! * Fixed: records of `lrecl` bytes back to back (RECFM=F).
//! * Variable: each record preceded by a 4-byte record descriptor word,
//!   a big-endian halfword length including the RDW and two zero bytes
//!   (RECFM=V). A 4-byte RDW alone is an empty record (used for unused
//!   relative record slots).
//! * Text: host text, one record per line (the caller converts between
//!   EBCDIC and the host character set).

use crate::{DsError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RecordFormat {
    Fixed(u32),
    Variable,
    Text,
}

/// Splits data set content into records.
pub fn decode(fmt: RecordFormat, data: &[u8]) -> Result<Vec<Vec<u8>>> {
    Ok(match fmt {
        RecordFormat::Fixed(n) => data.chunks(n.max(1) as usize).map(<[u8]>::to_vec).collect(),
        RecordFormat::Variable => {
            let mut out = Vec::new();
            let mut i = 0;
            while i + 4 <= data.len() {
                let len = u16::from_be_bytes([data[i], data[i + 1]]) as usize;
                if len < 4 || i + len > data.len() {
                    return Err(DsError::Damaged(format!(
                        "bad record descriptor at offset {i}"
                    )));
                }
                out.push(data[i + 4..i + len].to_vec());
                i += len;
            }
            if i != data.len() {
                return Err(DsError::Damaged("truncated record descriptor".into()));
            }
            out
        }
        RecordFormat::Text => {
            let mut out: Vec<Vec<u8>> = data
                .split(|&b| b == b'\n')
                .map(|l| l.strip_suffix(b"\r").unwrap_or(l).to_vec())
                .collect();
            if out.last().is_some_and(Vec::is_empty) {
                out.pop();
            }
            out
        }
    })
}

/// Appends one record in format `fmt`; a fixed-length record is padded
/// with `pad` or truncated.
pub fn encode_into(fmt: RecordFormat, rec: &[u8], pad: u8, out: &mut Vec<u8>) {
    match fmt {
        RecordFormat::Fixed(n) => {
            let n = n as usize;
            let k = rec.len().min(n);
            out.extend_from_slice(&rec[..k]);
            out.extend(std::iter::repeat_n(pad, n - k));
        }
        RecordFormat::Variable => {
            let len = (rec.len() + 4) as u16;
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(&[0, 0]);
            out.extend_from_slice(rec);
        }
        RecordFormat::Text => {
            out.extend_from_slice(rec);
            out.push(b'\n');
        }
    }
}

/// Reads a whole data set; None when the file does not exist.
pub fn read_file(path: &std::path::Path) -> Result<Option<Vec<u8>>> {
    match std::fs::read(path) {
        Ok(d) => Ok(Some(d)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(DsError::Io(format!("{}: {e}", path.display()))),
    }
}

/// Replaces a data set's content.
pub fn write_file(path: &std::path::Path, data: &[u8]) -> Result<()> {
    std::fs::write(path, data).map_err(|e| DsError::Io(format!("{}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_round_trip() {
        let mut out = Vec::new();
        for r in [&b"AB"[..], b"", b"CDE"] {
            encode_into(RecordFormat::Variable, r, 0x40, &mut out);
        }
        assert_eq!(&out[..6], &[0, 6, 0, 0, b'A', b'B']);
        let recs = decode(RecordFormat::Variable, &out).unwrap();
        assert_eq!(recs, vec![b"AB".to_vec(), vec![], b"CDE".to_vec()]);
        assert!(decode(RecordFormat::Variable, &out[..5]).is_err());
    }

    #[test]
    fn fixed_pads_and_text_lines() {
        let mut out = Vec::new();
        encode_into(RecordFormat::Fixed(4), b"AB", 0x40, &mut out);
        assert_eq!(out, [b'A', b'B', 0x40, 0x40]);
        assert_eq!(
            decode(RecordFormat::Text, b"a\r\nb\n").unwrap(),
            vec![b"a".to_vec(), b"b".to_vec()]
        );
    }
}

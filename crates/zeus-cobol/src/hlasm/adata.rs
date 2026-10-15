//! ADATA architecture-level-3 physical records and assembly-analysis output.
//! Field layouts follow the public HLASM Programmer's Guide, Appendix C.
use serde::{Deserialize, Serialize};
mod stream;
pub use stream::{AdataRecord, adata_bytes, decode_adata_records, encode_adata_records};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserData {
    pub values: [i32; 4],
    pub text: String,
}
impl UserData {
    /// One X'0070' record with its 12-byte common header, without an RDW.
    /// The enclosing file must declare the same CCSID in its identification.
    pub fn record(&self, ccsid: u32) -> Result<Vec<u8>, String> {
        let cp = crate::codepage::get(ccsid).ok_or("unsupported ADATA CCSID")?;
        let text = cp.encode(&self.text);
        if cp.decode(&text) != self.text {
            return Err("ADATA string cannot be represented in the selected CCSID".into());
        }
        if text.len() > 255 {
            return Err("ADATA character string exceeds 255 encoded bytes".into());
        }
        let mut record = vec![16, 0, 0x70, 3, 0, 1, 0, 0, 0, 0];
        record.extend_from_slice(&(24u16 + text.len() as u16).to_be_bytes());
        for value in self.values {
            record.extend_from_slice(&value.to_be_bytes());
        }
        record.extend_from_slice(&(if text.is_empty() { 0u32 } else { 36u32 }).to_be_bytes());
        record.extend_from_slice(&(text.len() as u32).to_be_bytes());
        record.extend_from_slice(&text);
        Ok(record)
    }
}

pub(super) fn parse(args: &str) -> Result<UserData, String> {
    let args = super::assemble::operands(args);
    if args.len() > 5 {
        return Err("ADATA accepts four decimal values and one quoted string".into());
    }
    let mut values = [0; 4];
    for (index, field) in args.iter().take(4).enumerate() {
        if field.is_empty() {
            continue;
        }
        let digits = field.strip_prefix(['+', '-']).unwrap_or(field);
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return Err("ADATA values must be decimal self-defining terms".into());
        }
        values[index] = field
            .parse::<i32>()
            .map_err(|_| "ADATA decimal value must fit signed 32 bits")?;
    }
    let text = match args.get(4).map(String::as_str) {
        None | Some("") => String::new(),
        Some(value) => {
            let inner = value
                .strip_prefix('\'')
                .and_then(|s| s.strip_suffix('\''))
                .ok_or("ADATA character string must be enclosed in apostrophes")?;
            let mut chars = inner.chars();
            let mut text = String::new();
            while let Some(c) = chars.next() {
                if c == '\'' && chars.next() != Some('\'') {
                    return Err("ADATA character string has an unpaired apostrophe".into());
                }
                text.push(c);
            }
            text
        }
    };
    Ok(UserData { values, text })
}

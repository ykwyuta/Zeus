//! Architectural GOFF RLD fields. Compression refers to the preceding item.
use super::Definition;
use std::collections::BTreeMap;
pub(super) struct Fixup {
    pub flags: [u8; 6],
    pub reference: u32,
    pub position: u32,
    pub offset: u32,
}
pub(super) fn decode(
    bytes: &[u8],
    defs: &BTreeMap<u32, Definition>,
    previous: &mut [Option<u32>; 3],
) -> Result<Vec<Fixup>, String> {
    let mut at = 0;
    let mut result = Vec::new();
    while at < bytes.len() {
        if at + 8 > bytes.len() {
            return Err("truncated GOFF relocation flags".into());
        }
        let flags: [u8; 6] = bytes[at..at + 6].try_into().unwrap();
        if flags[0] & 0x1e != 0 || bytes[at + 6..at + 8] != [0, 0] || flags[3] != 0 {
            return Err("invalid reserved GOFF relocation flags".into());
        }
        if !matches!(flags[1] >> 4, 0 | 2 | 6)
            || flags[1] & 15 > 3
            || flags[2] >> 1 > 1
            || flags[5] != 0
        {
            return Err(
                "GOFF non-address, bit-field or conditional relocation requires further support"
                    .into(),
            );
        }
        if flags[1] >> 4 != 0 && flags[0] & 1 != 0 {
            return Err(
                "GOFF addressing-mode sensitivity is valid only for address references".into(),
            );
        }
        if flags[1] >> 4 == 6
            && (flags[1] & 15 != 0 || !matches!(flags[4], 2 | 4) || flags[2] & 2 != 0)
        {
            return Err(
                "GOFF relative-immediate requires an additive label reference and width 2 or 4"
                    .into(),
            );
        }
        if !(1..=8).contains(&flags[4]) {
            return Err("invalid GOFF relocation width".into());
        }
        at += 8;
        for (field, mask) in [0x80, 0x40, 0x20].into_iter().enumerate() {
            if flags[0] & mask == 0 {
                if at + 4 > bytes.len() {
                    return Err("truncated GOFF relocation pointer or offset".into());
                }
                previous[field] = Some(u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()));
                at += 4;
            }
            if previous[field].is_none() {
                return Err("GOFF compressed relocation has no preceding value".into());
            }
        }
        let (reference, position, offset) = (
            previous[0].unwrap(),
            previous[1].unwrap(),
            previous[2].unwrap(),
        );
        if !defs.contains_key(&reference) || defs.get(&position).is_none_or(|d| d.kind != 1) {
            return Err("GOFF relocation requires preceding ESD definitions".into());
        }
        result.push(Fixup {
            flags,
            reference,
            position,
            offset,
        });
    }
    Ok(result)
}

//! Fixed-card and RDW physical framing, followed by GOFF logical continuation.
pub(super) struct Record {
    pub kind: u8,
    pub bytes: Vec<u8>,
}
fn half(b: &[u8], at: usize) -> usize {
    usize::from(u16::from_be_bytes([b[at], b[at + 1]]))
}
fn extent(b: &[u8], kind: u8) -> Result<(usize, usize), String> {
    let minimum = match kind {
        15 => 60,
        0 => 72,
        1 => 24,
        2 => 6,
        3 => 8,
        4 => 26,
        _ => return Err("invalid GOFF record type".into()),
    };
    if b.len() < minimum {
        return Err("truncated GOFF record header".into());
    }
    let length = match kind {
        15 => half(b, 52),
        0 => half(b, 70),
        1 => half(b, 22),
        2 => half(b, 4),
        3 => half(b, 6),
        4 => half(b, 24),
        _ => 0,
    };
    if matches!(kind, 0..=3) && length == 0 {
        return Err("GOFF record has empty variable data".into());
    }
    Ok((minimum, length))
}
pub(super) fn read(bytes: &[u8]) -> Result<Vec<Record>, String> {
    if bytes.len() > 64 << 20 {
        return Err("GOFF input exceeds 64 MiB".into());
    }
    let mut physical = Vec::new();
    let rdw = bytes.get(4..7) == Some(&[3, 0xf0, 0]) && bytes.get(2..4) == Some(&[0, 0]);
    if !rdw && bytes.first() == Some(&3) {
        if !bytes.len().is_multiple_of(80) {
            return Err("fixed GOFF input requires complete 80-byte records".into());
        }
        physical.extend(bytes.as_chunks::<80>().0.iter().map(|card| card.as_slice()));
    } else {
        let mut at = 0;
        while at < bytes.len() {
            if at + 4 > bytes.len() {
                return Err("truncated GOFF RDW".into());
            }
            let length = half(bytes, at);
            if length < 7 || at + length > bytes.len() || bytes[at + 2..at + 4] != [0, 0] {
                return Err("invalid GOFF RDW length or segment flags".into());
            }
            physical.push(&bytes[at + 4..at + length]);
            at += length;
        }
    }
    let mut result = Vec::new();
    let mut pending: Option<(Record, usize)> = None;
    for (index, b) in physical.into_iter().enumerate() {
        let parse = (|| {
            if b.len() < 3 || b[0] != 3 || b[1] & 12 != 0 || b[2] != 0 {
                return Err("invalid GOFF record prefix/version".into());
            }
            let kind = b[1] >> 4;
            let continuation = b[1] & 2 != 0;
            let continued = b[1] & 1 != 0;
            let (mut record, remaining) = if continuation {
                let (record, remaining) = pending.take().ok_or("orphan GOFF continuation")?;
                if record.kind != kind {
                    return Err("GOFF continuation changed record type".into());
                }
                (record, remaining)
            } else {
                if pending.is_some() {
                    return Err("GOFF continuation was interrupted".into());
                }
                let (start, length) = extent(b, kind)?;
                if continued && matches!(kind, 3 | 15) {
                    return Err("GOFF HDR/LEN cannot be continued".into());
                }
                let n = length.min(b.len() - start);
                let mut raw = b[..start + n].to_vec();
                raw[1] &= 0xf0;
                let record = Record { kind, bytes: raw };
                if !continued {
                    if n != length {
                        return Err("truncated GOFF variable field".into());
                    }
                    result.push(record);
                    return Ok(());
                }
                if n == length {
                    return Err("GOFF continued flag exceeds variable field length".into());
                }
                pending = Some((record, length - n));
                return Ok(());
            };
            let n = remaining.min(b.len() - 3);
            record.bytes.extend_from_slice(&b[3..3 + n]);
            if continued {
                if n == remaining {
                    return Err("GOFF continuation extends beyond declared field".into());
                }
                pending = Some((record, remaining - n));
            } else {
                if n != remaining {
                    return Err("GOFF final continuation is incomplete".into());
                }
                result.push(record);
            }
            Ok(())
        })();
        parse.map_err(|e: String| format!("GOFF physical record {}: {e}", index + 1))?;
    }
    if pending.is_some() {
        return Err("GOFF input ends during continuation".into());
    }
    Ok(result)
}

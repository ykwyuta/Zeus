//! Bit-length fields pack across adjacent operands, padding an incomplete
//! final byte only after the entire declaration has been assembled.
use super::{
    assemble::{Symbols, storage_item},
    constants,
};

pub(super) struct Constant {
    pub bytes: Vec<u8>,
    pub bits: usize,
    pub length: u32,
}

pub(super) fn append(output: &mut Vec<u8>, position: &mut usize, bytes: &[u8], count: usize) {
    output.resize((*position + count).div_ceil(8), 0);
    for bit in 0..count {
        if bytes[bit / 8] & (0x80 >> (bit % 8)) != 0 {
            output[(*position + bit) / 8] |= 0x80 >> ((*position + bit) % 8);
        }
    }
    *position += count;
}

/// Return bit-length and the index immediately following the length modifier.
pub(super) fn length(spec: &str, symbols: &Symbols) -> Result<Option<(u32, usize)>, String> {
    let at = constants::type_start(spec)?;
    let start = at
        + 1
        + usize::from(
            (spec[at..].starts_with("FD") || spec[at..].starts_with("AD"))
                || ["CA", "CE", "CU"].iter().any(|t| spec[at..].starts_with(t)),
        );
    let Some(tail) = spec[start..].strip_prefix("L.") else {
        return Ok(None);
    };
    let used = if tail.starts_with('(') {
        constants::parenthesized_end(tail)?
    } else {
        tail.bytes().take_while(u8::is_ascii_digit).count()
    };
    if used == 0 {
        return Err("bit length requires a decimal value or absolute expression".into());
    }
    let text = &tail[..used];
    let value = if text.starts_with('(') {
        &text[1..text.len() - 1]
    } else {
        text
    };
    let bits = constants::byte_length(&format!("L({value})"), symbols)?.unwrap();
    if bits == 0 {
        return Err("bit length must be positive".into());
    }
    Ok(Some((bits, start + 2 + used)))
}

pub(super) fn assemble(
    spec: &str,
    initialized: bool,
    symbols: &Symbols,
) -> Result<Option<Constant>, String> {
    let (clean, _, _) = constants::annotated(spec)?;
    let spec = clean.as_ref();
    let Some((bits, after_length)) = length(spec, symbols)? else {
        return Ok(None);
    };
    let (at, count) = constants::duplication(spec, symbols)?;
    let kind = spec.as_bytes()[at];
    let extension = usize::from(
        (spec[at..].starts_with("FD") || spec[at..].starts_with("AD"))
            || ["CA", "CE", "CU"].iter().any(|t| spec[at..].starts_with(t)),
    );
    let character = kind == b'C';
    let maximum = match kind {
        b'B' | b'X' => 2048,
        b'C' if !spec[at..].starts_with("CU") => 2048,
        b'F' | b'H' => 64,
        b'P' | b'Z' => 128,
        b'A' => 128,
        b'Y' => 16,
        _ => return Err("bit-length modifier is not supported for this constant type".into()),
    };
    if bits > maximum {
        return Err("bit length exceeds constant type limit".into());
    }
    let tail = &spec[after_length..];
    let quote = constants::nominal_quote(tail)?;
    let prefix = format!(
        "{}{}{}",
        &spec[at..at + 1 + extension],
        if matches!(kind, b'F' | b'H') {
            "L8"
        } else {
            ""
        },
        &tail[..quote.unwrap_or(tail.len())]
    );
    let nominal = if let Some(q) = quote {
        tail[q + 1..]
            .strip_suffix('\'')
            .ok_or("unterminated bit constant")?
    } else {
        ""
    };
    let address = matches!(kind, b'A' | b'Y');
    let values = if address {
        if tail.is_empty() && (!initialized || count == 0) {
            vec!["0".into()]
        } else {
            let nominal = tail
                .strip_prefix('(')
                .and_then(|s| s.strip_suffix(')'))
                .ok_or("bit-length address nominal requires parentheses")?;
            if nominal.is_empty() {
                return Err("empty bit-length address nominal".into());
            }
            super::assemble::operands(nominal)
        }
    } else if character {
        vec![nominal.to_string()]
    } else {
        nominal.split(',').map(str::to_string).collect::<Vec<_>>()
    };
    let total = (bits as usize)
        .checked_mul(values.len())
        .and_then(|n| n.checked_mul(count as usize))
        .filter(|n| *n <= 8 << 20)
        .ok_or("storage declaration exceeds 1 MiB")?;
    let mut bytes = Vec::with_capacity(total.div_ceil(8));
    let mut position = 0;
    for _ in 0..count {
        for value in &values {
            let normalized = format!("{prefix}'{value}'");
            let (data, negative) = if address {
                let value = super::expression::evaluate_early(value, symbols)?.value;
                let value = if spec[at..].starts_with("AD") {
                    value
                } else {
                    i64::from(value as i32)
                };
                (value.to_be_bytes().to_vec(), value < 0)
            } else {
                (
                    storage_item(&normalized, initialized, symbols)?.0,
                    matches!(kind, b'F' | b'H') && value.starts_with('-'),
                )
            };
            let mut field = vec![0u8; (bits as usize).div_ceil(8)];
            for bit in 0..bits as usize {
                let one = if !initialized {
                    false
                } else if character {
                    if bit < data.len() * 8 {
                        data[bit / 8] & (0x80 >> (bit % 8)) != 0
                    } else {
                        let pad = if spec[at..].starts_with("CA") {
                            0x20
                        } else {
                            0x40
                        };
                        pad & (0x80 >> ((bit - data.len() * 8) % 8)) != 0
                    }
                } else {
                    let from_right = bits as usize - 1 - bit;
                    if from_right < data.len() * 8 {
                        data[data.len() - 1 - from_right / 8] & (1 << (from_right % 8)) != 0
                    } else {
                        negative
                    }
                };
                if one {
                    field[bit / 8] |= 0x80 >> (bit % 8);
                }
            }
            append(&mut bytes, &mut position, &field, bits as usize);
        }
    }
    Ok(Some(Constant {
        bytes,
        bits: total,
        length: bits.div_ceil(8),
    }))
}

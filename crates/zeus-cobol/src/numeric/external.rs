//! DISPLAY/NATIONAL external decimal floating-point representations.
use super::number::Number;
use crate::{ebcdic, vm::data};
use num_traits::{Signed, Zero};

pub(crate) fn picture(text: &str, comma: bool) -> Result<(u32, i32, u32), String> {
    let (mantissa, exponent) = text
        .split_once('E')
        .ok_or("external floating PICTURE requires E")?;
    if !matches!(exponent, "+99" | "-99") || !mantissa.starts_with(['+', '-']) {
        return Err("external floating PICTURE requires signs before mantissa and exponent, and exponent 99".into());
    }
    let point = if comma { ',' } else { '.' };
    let chars: Vec<char> = mantissa[1..].chars().collect();
    let positions: Vec<usize> = chars
        .iter()
        .enumerate()
        .filter_map(|(i, c)| (*c == point || *c == 'V').then_some(i))
        .collect();
    let digits = chars.iter().filter(|c| **c == '9').count();
    if positions.len() != 1
        || !(1..=16).contains(&digits)
        || chars
            .iter()
            .any(|c| !matches!(*c, '9' | 'V') && *c != point)
    {
        return Err("external floating mantissa requires 1..16 digits and one actual or implied decimal point".into());
    }
    let scale = chars[positions[0] + 1..]
        .iter()
        .filter(|c| **c == '9')
        .count() as i32;
    Ok((
        digits as u32,
        scale,
        text.chars().filter(|c| *c != 'V').count() as u32,
    ))
}

pub(crate) fn decode(
    picture: &str,
    national: bool,
    comma: bool,
    bytes: &[u8],
) -> Result<Number, String> {
    let bytes = if national {
        data::display_of(bytes)
    } else {
        bytes.to_vec()
    };
    let mut input = bytes.into_iter().map(ebcdic::to_latin1);
    let mut text = String::new();
    for symbol in picture.chars() {
        if symbol == 'V' {
            text.push('.');
            continue;
        }
        let byte = input.next().ok_or("short external floating item")?;
        match symbol {
            '9' if byte.is_ascii_digit() => text.push(byte as char),
            '+' if matches!(byte, b'+' | b'-') => text.push(byte as char),
            '-' if matches!(byte, b' ' | b'-') => text.push(if byte == b' ' { '+' } else { '-' }),
            'E' if byte == b'E' => text.push('E'),
            c if c == if comma { ',' } else { '.' } && byte == c as u8 => text.push('.'),
            _ => return Err("invalid external floating item".into()),
        }
    }
    if input.next().is_some() {
        return Err("long external floating item".into());
    }
    Number::parse(&text)
}

pub(crate) fn encode(
    number: &Number,
    picture: &str,
    national: bool,
    comma: bool,
    out: &mut [u8],
) -> Result<bool, String> {
    let (mantissa, _) = picture
        .split_once('E')
        .ok_or("external floating PICTURE requires E")?;
    let digits = mantissa.chars().filter(|c| *c == '9').count();
    let before = mantissa
        .chars()
        .skip(1)
        .take_while(|c| !matches!(*c, 'V' | '.' | ','))
        .filter(|c| *c == '9')
        .count();
    let mut exponent = 0;
    let mut raw = num_bigint::BigInt::zero();
    if !number.value.n.is_zero() {
        let mut order = number.value.n.to_str_radix(10).len() as i32
            - number.value.d.to_str_radix(10).len() as i32;
        if number.raw(-order, false).is_zero() {
            order -= 1;
        }
        exponent = order + 1 - before as i32;
        raw = number
            .raw(digits as i32 - before as i32 - exponent, true)
            .abs();
        if raw.to_str_radix(10).len() > digits {
            raw /= 10;
            exponent += 1;
        }
    }
    if exponent.abs() > 99 {
        return Ok(true);
    }
    let coefficient = format!("{:0>width$}", raw.to_str_radix(10), width = digits);
    let mut coefficient = coefficient.bytes();
    let exponent_digits = format!("{:02}", exponent.abs());
    let mut exponent_digits = exponent_digits.bytes();
    let mut in_exponent = false;
    let mut bytes = Vec::new();
    for symbol in picture.chars() {
        let byte = match symbol {
            'V' => continue,
            'E' => {
                in_exponent = true;
                b'E'
            }
            '9' => if in_exponent {
                exponent_digits.next()
            } else {
                coefficient.next()
            }
            .unwrap(),
            '+' | '-' => {
                if if in_exponent {
                    exponent < 0
                } else {
                    number.value.negative && !number.value.n.is_zero()
                } {
                    b'-'
                } else if symbol == '+' {
                    b'+'
                } else {
                    b' '
                }
            }
            c => {
                if comma && c == ',' {
                    b','
                } else {
                    c as u8
                }
            }
        };
        bytes.push(ebcdic::from_latin1(byte));
    }
    if national {
        out.copy_from_slice(&data::national_of(&bytes));
    } else {
        out.copy_from_slice(&bytes);
    }
    Ok(false)
}

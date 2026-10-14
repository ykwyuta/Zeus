//! Exact decimal conversion for fixed-point source constants.

/// Parse byte length, binary scale and decimal exponent, in source order.
pub(super) fn fixed_modifiers(
    text: &str,
    symbols: &super::assemble::Symbols,
) -> Result<(Option<u32>, i32, i32), String> {
    let (length, tail) = if text.starts_with('L') {
        let (value, tail) = modifier(text, 'L', symbols)?;
        (
            Some(u32::try_from(value).map_err(|_| "length must be nonnegative")?),
            tail,
        )
    } else {
        (None, text)
    };
    let (scale, tail) = modifier(tail, 'S', symbols)?;
    let (exponent, tail) = modifier(tail, 'E', symbols)?;
    if !tail.is_empty() {
        return Err("unsupported or out-of-order fixed-point constant modifier".into());
    }
    if !(-187..=346).contains(&scale) {
        return Err("binary scale must be in -187..346".into());
    }
    check_exponent(exponent)?;
    Ok((length, scale, exponent))
}

fn modifier<'a>(
    text: &'a str,
    kind: char,
    symbols: &super::assemble::Symbols,
) -> Result<(i32, &'a str), String> {
    let Some(rest) = text.strip_prefix(kind) else {
        return Ok((0, text));
    };
    let signed = rest.strip_prefix(['+', '-']).unwrap_or(rest);
    if signed.starts_with('(') {
        let end = parenthesized_end(signed)?;
        let value = super::expression::evaluate_early(&signed[1..end - 1], symbols)?.value;
        let value = if rest.starts_with('-') {
            value.checked_neg().ok_or("modifier negation overflow")?
        } else {
            value
        };
        let value = i32::try_from(value)
            .map_err(|_| format!("{kind} modifier exceeds signed 32-bit range"))?;
        return Ok((value, &signed[end..]));
    }
    let digits = signed.bytes().take_while(u8::is_ascii_digit).count();
    let end = rest.len() - signed.len() + digits;
    if digits == 0 {
        return Err(format!("{kind} modifier requires a signed decimal integer"));
    }
    let value = rest[..end]
        .parse::<i32>()
        .map_err(|_| format!("{kind} modifier exceeds signed 32-bit range"))?;
    Ok((value, &rest[end..]))
}

/// End of a balanced modifier expression, respecting quoted self-defining terms.
pub(super) fn parenthesized_end(text: &str) -> Result<usize, String> {
    let (mut depth, mut quoted) = (0, false);
    for (at, c) in text.char_indices() {
        if c == '\'' && (quoted || !super::assemble::attribute_quote(text, at)) {
            quoted = !quoted;
        }
        if !quoted {
            if c == '(' {
                depth += 1;
            }
            if c == ')' {
                depth -= 1;
                if depth == 0 {
                    return Ok(at + 1);
                }
            }
        }
    }
    Err("unclosed constant modifier expression".into())
}

/// Find the nominal's opening quote rather than an attribute/self-defining quote.
pub(super) fn nominal_quote(text: &str) -> Result<Option<usize>, String> {
    let mut at = 0;
    while at < text.len() {
        match text.as_bytes()[at] {
            b'(' => at += parenthesized_end(&text[at..])?,
            b'\'' => return Ok(Some(at)),
            _ => at += 1,
        }
    }
    Ok(None)
}

fn check_exponent(exponent: i32) -> Result<(), String> {
    if !(-85..=75).contains(&exponent) {
        return Err(
            "decimal exponent must be in -85..75 after combining modifier and nominal".into(),
        );
    }
    Ok(())
}

/// Multiply decimal digits (least significant first) without host float rounding.
fn multiply(digits: &mut Vec<u8>, factor: u8) {
    let mut carry = 0;
    for digit in digits.iter_mut() {
        let product = *digit * factor + carry;
        *digit = product % 10;
        carry = product / 10;
    }
    if carry != 0 {
        digits.push(carry);
    }
}

pub(super) fn fixed_value(nominal: &str, scale: i32, exponent: i32) -> Result<i64, String> {
    let negative = nominal.starts_with('-');
    let magnitude = nominal.strip_prefix(['+', '-']).unwrap_or(nominal);
    let (magnitude, nominal_exponent) =
        if let Some((value, power)) = magnitude.split_once(['E', 'e']) {
            let exponent = power
                .parse::<i32>()
                .map_err(|_| "nominal exponent requires a signed decimal integer")?;
            (value, exponent)
        } else {
            (magnitude, 0)
        };
    let exponent = exponent
        .checked_add(nominal_exponent)
        .ok_or("combined decimal exponent overflow")?;
    check_exponent(exponent)?;
    let mut digits = Vec::new();
    let mut fractional = None;
    for c in magnitude.bytes() {
        if c == b'.' && fractional.is_none() {
            fractional = Some(0usize);
        } else if c.is_ascii_digit() {
            digits.push(c - b'0');
            if let Some(count) = &mut fractional {
                *count += 1;
            }
        } else {
            return Err(
                "fixed-point nominal requires decimal digits and at most one decimal point".into(),
            );
        }
    }
    if digits.is_empty() {
        return Err("fixed-point nominal requires decimal digits".into());
    }
    digits.reverse();
    let mut fractional = fractional.unwrap_or(0);
    if exponent < 0 {
        fractional += exponent.unsigned_abs() as usize;
    } else {
        let power = exponent as usize;
        if power > fractional {
            digits.splice(0..0, std::iter::repeat_n(0, power - fractional));
            fractional = 0;
        } else {
            fractional -= power;
        }
    }
    // Dividing by two is exactly multiplying by five and adding a decimal place.
    for _ in 0..scale.unsigned_abs() {
        multiply(&mut digits, if scale < 0 { 5 } else { 2 });
        fractional += usize::from(scale < 0);
    }
    let integer = digits.get(fractional..).unwrap_or_default();
    let mut value = 0u64;
    for &digit in integer.iter().rev() {
        value = value
            .checked_mul(10)
            .and_then(|n| n.checked_add(u64::from(digit)))
            .ok_or("fixed-point constant exceeds signed 64-bit range")?;
    }
    if fractional > 0 && digits.get(fractional - 1).is_some_and(|d| *d >= 5) {
        value = value
            .checked_add(1)
            .ok_or("fixed-point rounding exceeds signed 64-bit range")?;
    }
    let signed = if negative {
        -i128::from(value)
    } else {
        i128::from(value)
    };
    i64::try_from(signed).map_err(|_| "fixed-point constant exceeds signed 64-bit range".into())
}

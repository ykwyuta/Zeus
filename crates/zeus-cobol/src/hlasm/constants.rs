//! Exact decimal conversion for fixed-point source constants.

/// Locate the type after a decimal or parenthesized-expression duplication
/// factor. Alphabetic symbols inside the factor are not storage types.
pub(super) fn type_start(spec: &str) -> Result<usize, String> {
    let at = if spec.starts_with('(') {
        parenthesized_end(spec)?
    } else {
        spec.bytes().take_while(u8::is_ascii_digit).count()
    };
    if !spec.as_bytes().get(at).is_some_and(u8::is_ascii_alphabetic) {
        return Err("storage type expected after duplication factor".into());
    }
    Ok(at)
}

pub(super) fn duplication(
    spec: &str,
    symbols: &super::assemble::Symbols,
) -> Result<(usize, u32), String> {
    let at = type_start(spec)?;
    let count = if at == 0 {
        1
    } else if spec.starts_with('(') {
        u32::try_from(super::expression::evaluate_early(&spec[1..at - 1], symbols)?.value)
            .map_err(|_| "duplication factor must be nonnegative and fit 32 bits")?
    } else {
        spec[..at]
            .parse()
            .map_err(|_| "invalid duplication factor")?
    };
    if count > 1 << 20 {
        return Err("storage declaration exceeds 1 MiB".into());
    }
    Ok((at, count))
}

pub(super) fn byte_length(
    text: &str,
    symbols: &super::assemble::Symbols,
) -> Result<Option<u32>, String> {
    if text.is_empty() {
        return Ok(None);
    }
    let (length, rest) = modifier(text, 'L', symbols)?;
    if !text.starts_with('L') || !rest.is_empty() {
        return Err("unsupported storage constant modifier".into());
    }
    u32::try_from(length)
        .map(Some)
        .map_err(|_| "constant length must be nonnegative".into())
}

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

pub(super) fn modifier<'a>(
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

/// The optional P() annotation follows the type extension and contributes no
/// storage. Keep its parsing shared by layout, bit fields and macro attributes.
pub(super) fn annotated(
    spec: &str,
) -> Result<(std::borrow::Cow<'_, str>, Option<&str>, String), String> {
    let at = type_start(spec)?;
    let upper = spec[at..].to_ascii_uppercase();
    let extension = [
        "AD", "VD", "QD", "QY", "RD", "SY", "JD", "FD", "CA", "CE", "CU", "DB", "DH", "DD", "EB",
        "EH", "ED", "LB", "LH", "LD", "LQ",
    ]
    .iter()
    .any(|t| upper.starts_with(t));
    let end = at + 1 + usize::from(extension);
    let assembler = spec[at..end].to_ascii_uppercase();
    if !spec[end..].starts_with("P(") {
        return Ok((std::borrow::Cow::Borrowed(spec), None, assembler));
    }
    let close = end + 1 + parenthesized_end(&spec[end + 1..])?;
    let value = &spec[end + 2..close - 1];
    if value.is_empty() {
        return Err("program type requires a 32-bit absolute value".into());
    }
    Ok((
        std::borrow::Cow::Owned(format!("{}{}", &spec[..end], &spec[close..])),
        Some(value),
        assembler,
    ))
}
pub(super) fn annotations(
    spec: &str,
    symbols: &super::assemble::Symbols,
) -> Result<(Option<u32>, String), String> {
    let mut program = None;
    let mut assembler = String::new();
    for item in super::assemble::operands(spec) {
        let (_, term, kind) = annotated(&item)?;
        if assembler.is_empty() {
            assembler = kind;
        }
        if let Some(term) = term {
            let value = super::expression::evaluate_early(term, symbols)?.value;
            if !(i64::from(i32::MIN)..=i64::from(u32::MAX)).contains(&value) {
                return Err("program type exceeds 32-bit range".into());
            }
            if program.is_none() {
                program = Some(value as u32);
            }
        }
    }
    Ok((program, assembler))
}

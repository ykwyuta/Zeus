//! Numeric text conversion, independent of the host's locale and f64.

/// Normalize a NUMVAL/NUMVAL-C argument. Errors are one-based character
/// positions, also used by TEST-NUMVAL and TEST-NUMVAL-C.
pub(crate) fn parse(
    text: &str,
    currency: Option<&str>,
    comma: bool,
    limit: usize,
) -> Result<String, usize> {
    let chars: Vec<_> = text.chars().collect();
    let mut i = 0;
    let skip = |i: &mut usize| {
        while chars.get(*i) == Some(&' ') {
            *i += 1;
        }
    };
    let sign = |i: &mut usize| match chars.get(*i) {
        Some('+') => {
            *i += 1;
            Some(false)
        }
        Some('-') => {
            *i += 1;
            Some(true)
        }
        _ => None,
    };
    skip(&mut i);
    let mut negative = sign(&mut i);
    skip(&mut i);
    if let Some(currency) = currency.filter(|c| !c.is_empty()) {
        let token: Vec<_> = currency.chars().collect();
        if chars[i..].starts_with(&token) {
            i += token.len();
            skip(&mut i);
        }
    }
    let point = if comma { ',' } else { '.' };
    let group = if comma { '.' } else { ',' };
    let mut digits = 0;
    let mut decimal = false;
    let mut normalized = String::new();
    while let Some(&c) = chars.get(i) {
        if c.is_ascii_digit() {
            digits += 1;
            if digits > limit {
                return Err(i + 1);
            }
            normalized.push(c);
        } else if c == point && !decimal {
            decimal = true;
            normalized.push('.');
        } else if c == group
            && currency.is_some()
            && !decimal
            && i > 0
            && chars[i - 1].is_ascii_digit()
            && chars.get(i + 1).is_some_and(char::is_ascii_digit)
        {
            // A grouping separator contributes no digit to the value.
        } else {
            break;
        }
        i += 1;
    }
    if digits == 0 {
        return Err(i + 1);
    }
    skip(&mut i);
    if negative.is_none() {
        negative = sign(&mut i);
        if negative.is_none()
            && (chars[i..].starts_with(&['C', 'R']) || chars[i..].starts_with(&['D', 'B']))
        {
            negative = Some(true);
            i += 2;
        }
    }
    skip(&mut i);
    if i != chars.len() {
        return Err(i + 1);
    }
    if negative == Some(true) {
        normalized.insert(0, '-');
    }
    Ok(normalized)
}

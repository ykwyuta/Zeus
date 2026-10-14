//! Integer expressions retain relocation coefficients until the whole
//! expression is reduced. Same-section differences therefore become absolute.
use super::assemble::{Symbol, Symbols};
use std::collections::BTreeMap;

pub(super) struct Value {
    pub number: i64,
    pub relocations: BTreeMap<String, i64>,
    length: u32,
}

impl Value {
    fn absolute(number: i64) -> Self {
        Self {
            number,
            relocations: BTreeMap::new(),
            length: 1,
        }
    }

    fn symbol(s: Symbol) -> Self {
        Self {
            number: s.value,
            relocations: if s.complex.is_empty() {
                s.section.into_iter().map(|s| (s, 1)).collect()
            } else {
                s.complex
            },
            length: s.length,
        }
    }
}

struct Parser<'a> {
    text: &'a str,
    at: usize,
    symbols: &'a Symbols,
    section: &'a str,
    offset: u32,
    location_known: bool,
}

impl Parser<'_> {
    fn peek(&mut self) -> Option<u8> {
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.at += 1;
        }
        self.text.as_bytes().get(self.at).copied()
    }

    fn sum(&mut self) -> Result<Value, String> {
        let mut a = self.product()?;
        while let Some(op @ (b'+' | b'-')) = self.peek() {
            self.at += 1;
            let b = self.product()?;
            let sign = if op == b'+' { 1 } else { -1 };
            a.number = a
                .number
                .checked_add(b.number.checked_mul(sign).ok_or("expression overflow")?)
                .ok_or("expression overflow")?;
            for (s, n) in b.relocations {
                let coefficient = a.relocations.entry(s).or_default();
                *coefficient = coefficient
                    .checked_add(n.checked_mul(sign).ok_or("expression overflow")?)
                    .ok_or("expression overflow")?;
            }
            a.relocations.retain(|_, n| *n != 0);
        }
        Ok(a)
    }

    fn product(&mut self) -> Result<Value, String> {
        let mut a = self.term()?;
        while let Some(op @ (b'*' | b'/')) = self.peek() {
            self.at += 1;
            let b = self.term()?;
            if !a.relocations.is_empty() || !b.relocations.is_empty() {
                return Err("relocatable term in multiplication/division".into());
            }
            a.number = if op == b'*' {
                a.number.checked_mul(b.number)
            } else {
                a.number.checked_div(b.number)
            }
            .ok_or("expression overflow or division by zero")?;
        }
        Ok(a)
    }

    fn term(&mut self) -> Result<Value, String> {
        match self.peek().ok_or("expression term expected")? {
            op @ (b'+' | b'-') => {
                self.at += 1;
                let mut v = self.term()?;
                if op == b'-' {
                    v.number = v.number.checked_neg().ok_or("expression overflow")?;
                    for n in v.relocations.values_mut() {
                        *n = n.checked_neg().ok_or("expression overflow")?;
                    }
                }
                Ok(v)
            }
            b'(' => {
                self.at += 1;
                let v = self.sum()?;
                if self.peek() != Some(b')') {
                    return Err("missing expression closing parenthesis".into());
                }
                self.at += 1;
                Ok(v)
            }
            b'*' => {
                if !self.location_known {
                    return Err(
                        "location counter is not available during macro attribute evaluation"
                            .into(),
                    );
                }
                self.at += 1;
                Ok(Value::symbol(Symbol {
                    section: Some(self.section.into()),
                    value: self.offset.into(),
                    length: 1,
                    type_attribute: crate::ebcdic::encode("U")[0],
                    numeric_attributes: None,
                    program_type: None,
                    assembler_type: None,
                    complex: BTreeMap::new(),
                }))
            }
            c if c.is_ascii_alphabetic()
                && self.text.as_bytes().get(self.at + 1) == Some(&b'\'') =>
            {
                self.at += 2;
                if b"LTSI".contains(&c.to_ascii_uppercase()) {
                    if self.text.as_bytes().get(self.at) == Some(&b'=') {
                        let end = super::assemble::literal_end(self.text, self.at)?;
                        let (length, kind) = super::assemble::literal_attributes(
                            &self.text[self.at..end],
                            self.symbols,
                        )?;
                        let numeric = if b"SI".contains(&c.to_ascii_uppercase()) {
                            Some(super::assemble::storage_numeric_attributes(
                                &self.text[self.at + 1..end],
                                length,
                                self.symbols,
                            )?)
                        } else {
                            None
                        };
                        self.at = end;
                        return Ok(Value::absolute(match c.to_ascii_uppercase() {
                            b'L' => length.into(),
                            b'T' => kind.into(),
                            b'S' => numeric.unwrap().0,
                            _ => numeric.unwrap().1,
                        }));
                    }
                    let mut name = self.name()?;
                    if self.peek() == Some(b'.') {
                        self.at += 1;
                        name = self.name()?;
                    }
                    let s = self
                        .symbols
                        .get(&name)
                        .ok_or_else(|| format!("undefined symbol {name}"))?;
                    return Ok(Value::absolute(match c.to_ascii_uppercase() {
                        b'L' => s.length.into(),
                        b'T' => s.type_attribute.into(),
                        kind => {
                            let numeric = s
                                .numeric_attributes
                                .ok_or("scale/integer attribute is unavailable for this symbol")?;
                            if kind == b'S' { numeric.0 } else { numeric.1 }
                        }
                    }));
                }
                let mut literal = String::new();
                loop {
                    let rest = &self.text[self.at..];
                    let ch = rest
                        .chars()
                        .next()
                        .ok_or("unterminated self-defining term")?;
                    self.at += ch.len_utf8();
                    if ch == '\'' {
                        if self.text.as_bytes().get(self.at) == Some(&b'\'') {
                            self.at += 1;
                        } else {
                            break;
                        }
                    }
                    literal.push(ch);
                }
                let n = match c.to_ascii_uppercase() {
                    b'X' => {
                        i64::from_str_radix(&literal, 16).map_err(|_| "invalid hexadecimal term")?
                    }
                    b'B' => i64::from_str_radix(&literal, 2).map_err(|_| "invalid binary term")?,
                    b'C' => {
                        let bytes = crate::ebcdic::encode(&literal);
                        if bytes.len() > 4 {
                            return Err("character self-defining term exceeds four bytes".into());
                        }
                        bytes.into_iter().fold(0, |n, b| n * 256 + i64::from(b))
                    }
                    _ => {
                        return Err("unsupported expression attribute or self-defining term".into());
                    }
                };
                Ok(Value::absolute(n))
            }
            c if c.is_ascii_digit() => {
                let start = self.at;
                while self
                    .text
                    .as_bytes()
                    .get(self.at)
                    .is_some_and(u8::is_ascii_digit)
                {
                    self.at += 1;
                }
                Ok(Value::absolute(
                    self.text[start..self.at]
                        .parse()
                        .map_err(|_| "integer overflow")?,
                ))
            }
            _ => {
                let name = self.name()?;
                self.symbols
                    .get(&name)
                    .cloned()
                    .map(Value::symbol)
                    .ok_or_else(|| format!("undefined symbol {name}"))
            }
        }
    }

    fn name(&mut self) -> Result<String, String> {
        let start = self.at;
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(|c| c.is_ascii_alphanumeric() || b"_@$#".contains(c))
        {
            self.at += 1;
        }
        if self.at == start {
            return Err("invalid expression term".into());
        }
        Ok(self.text[start..self.at].to_ascii_uppercase())
    }
}

pub(super) fn evaluate(
    text: &str,
    symbols: &Symbols,
    section: &str,
    offset: u32,
) -> Result<Symbol, String> {
    let value = evaluate_terms(text, symbols, section, offset)?;
    let section = match value.relocations.len() {
        0 => None,
        1 if value.relocations.values().next() == Some(&1) => {
            value.relocations.keys().next().cloned()
        }
        _ => return Err("illegal relocatable expression".into()),
    };
    Ok(Symbol {
        section,
        value: value.number,
        length: value.length,
        type_attribute: crate::ebcdic::encode("U")[0],
        numeric_attributes: None,
        program_type: None,
        assembler_type: None,
        complex: BTreeMap::new(),
    })
}

/// EQU retains a normalized expression even if it is not yet absolute or
/// simply relocatable. Subsequent expressions substitute these coefficients.
pub(super) fn evaluate_equ(
    text: &str,
    symbols: &Symbols,
    section: &str,
    offset: u32,
) -> Result<Symbol, String> {
    let args = super::assemble::operands(text);
    let text = args
        .first()
        .filter(|s| !s.is_empty())
        .ok_or("EQU value required")?;
    if text.starts_with('=') {
        return Err("EQU cannot contain a literal".into());
    }
    let value = evaluate_terms(text, symbols, section, offset)?;
    let (section, complex) = match value.relocations.len() {
        0 => (None, BTreeMap::new()),
        1 if value.relocations.values().next() == Some(&1) => {
            (value.relocations.keys().next().cloned(), BTreeMap::new())
        }
        _ => (None, value.relocations),
    };
    Ok(Symbol {
        section,
        value: value.number,
        length: value.length,
        type_attribute: crate::ebcdic::encode("U")[0],
        numeric_attributes: Some((0, 0)),
        program_type: None,
        assembler_type: None,
        complex,
    })
}

#[derive(Clone)]
pub(super) struct EquAttributes {
    length: Option<u32>,
    type_attribute: u8,
    program_type: Option<u32>,
    assembler_type: Option<String>,
}

impl EquAttributes {
    pub(super) fn explicit_length(&self) -> Option<u32> {
        self.length
    }
    pub(super) fn type_byte(&self) -> u8 {
        self.type_attribute
    }
    pub(super) fn query(&self, program: bool) -> String {
        if program {
            self.program_type
                .map_or_else(String::new, |n| crate::ebcdic::decode(&n.to_be_bytes()))
        } else {
            self.assembler_type.clone().unwrap_or_default()
        }
    }
    pub fn apply(&self, symbol: &mut Symbol) {
        symbol.numeric_attributes = Some((0, 0));
        if let Some(length) = self.length {
            symbol.length = length;
        }
        symbol.type_attribute = self.type_attribute;
        symbol.program_type = self.program_type;
        symbol.assembler_type = self.assembler_type.clone();
    }
}

/// Attribute operands must be evaluated using only symbols available at the
/// definition, even if the EQU value itself needs a later assembly pass.
pub(super) fn equ_attributes(
    text: &str,
    symbols: &Symbols,
    section: &str,
    offset: u32,
) -> Result<EquAttributes, String> {
    equ_attributes_using(text, |text| evaluate(text, symbols, section, offset))
}

pub(super) fn early_equ_attributes(text: &str, symbols: &Symbols) -> Result<EquAttributes, String> {
    equ_attributes_using(text, |text| evaluate_early(text, symbols))
}

fn equ_attributes_using(
    text: &str,
    evaluate: impl Fn(&str) -> Result<Symbol, String>,
) -> Result<EquAttributes, String> {
    let args = super::assemble::operands(text);
    if args.len() > 5 {
        return Err("EQU accepts at most five operands".into());
    }
    let attribute =
        |index: usize, minimum: i64, limit: i64, name: &str| -> Result<Option<i64>, String> {
            let Some(text) = args.get(index).filter(|s| !s.is_empty()) else {
                return Ok(None);
            };
            let value = evaluate(text).map_err(|e| {
                format!("EQU {name} attribute must be known at its definition: {e}")
            })?;
            if value.section.is_some() || !(minimum..=limit).contains(&value.value) {
                return Err(format!(
                    "EQU {name} attribute must be absolute in {minimum}..={limit}"
                ));
            }
            Ok(Some(value.value))
        };
    Ok(EquAttributes {
        length: attribute(1, 0, 65535, "length")?.map(|n| n as u32),
        type_attribute: attribute(2, 0, 255, "type")?
            .map_or(crate::ebcdic::encode("U")[0], |n| n as u8),
        program_type: attribute(3, i64::from(i32::MIN), i64::from(u32::MAX), "program type")?
            .map(|n| n as u32),
        assembler_type: args.get(4).filter(|s| !s.is_empty()).map(|s| {
            let name = s.to_ascii_uppercase();
            if matches!(name.as_str(), "AR" | "CR" | "CR32" | "CR64" | "FPR" | "GR" | "GR32" | "GR64" | "VR") {
                Ok(name)
            } else {
                Err(format!("invalid EQU assembler type {s}; expected AR, CR, CR32, CR64, FPR, GR, GR32, GR64 or VR"))
            }
        }).transpose()?,
    })
}

/// Macro expansion can use already known absolute EQU values. Never invent a
/// zero location for expressions that require layout.
pub(super) fn evaluate_early(text: &str, symbols: &Symbols) -> Result<Symbol, String> {
    let mut parser = Parser {
        text,
        at: 0,
        symbols,
        section: "",
        offset: 0,
        location_known: false,
    };
    let value = parser.sum()?;
    if parser.peek().is_some() {
        return Err("unexpected expression suffix".into());
    }
    if !value.relocations.is_empty() {
        return Err("macro attribute expression requires a known absolute value".into());
    }
    Ok(Symbol {
        section: None,
        value: value.number,
        length: value.length,
        type_attribute: crate::ebcdic::encode("U")[0],
        numeric_attributes: None,
        program_type: None,
        assembler_type: None,
        complex: BTreeMap::new(),
    })
}

pub(super) fn evaluate_terms(
    text: &str,
    symbols: &Symbols,
    section: &str,
    offset: u32,
) -> Result<Value, String> {
    if text.starts_with('=') {
        return symbols
            .get(text)
            .cloned()
            .map(Value::symbol)
            .ok_or_else(|| format!("undefined literal {text}"));
    }
    let mut parser = Parser {
        text,
        at: 0,
        symbols,
        section,
        offset,
        location_known: true,
    };
    let value = parser.sum()?;
    if parser.peek().is_some() {
        return Err("unexpected expression suffix".into());
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equ_optional_program_and_assembler_metadata_distinguishes_null_and_zero_and_does_not_inherit()
     {
        let mut symbols = Symbols::new();
        for (text, tag, kind) in [
            ("7,,,0,GR32", Some(0), Some("GR32")),
            ("7,,,-1,GR64", Some(u32::MAX), Some("GR64")),
            ("7,,,,CR32", None, Some("CR32")),
            ("7,,,X'80000000'", Some(0x80000000), None),
        ] {
            let mut value = evaluate_equ(text, &symbols, "", 0).unwrap();
            equ_attributes(text, &symbols, "", 0)
                .unwrap()
                .apply(&mut value);
            assert_eq!(value.program_type, tag);
            assert_eq!(value.assembler_type.as_deref(), kind);
            symbols.insert("BASE".into(), value);
            let mut alias = evaluate_equ("BASE", &symbols, "", 0).unwrap();
            equ_attributes("BASE", &symbols, "", 0)
                .unwrap()
                .apply(&mut alias);
            assert_eq!(alias.value, 7);
            assert_eq!(alias.program_type, None);
            assert_eq!(alias.assembler_type, None);
        }
    }

    #[test]
    fn equ_program_attribute_is_captured_before_a_forward_value_resolves() {
        let mut symbols = Symbols::new();
        symbols.insert("HEAD".into(), evaluate_equ("*", &symbols, "", 8).unwrap());
        let text = "LATER,,,(*-HEAD),GR32";
        let attributes = equ_attributes(text, &symbols, "", 40).unwrap();
        assert!(evaluate_equ(text, &symbols, "", 40).is_err());
        symbols.insert("LATER".into(), evaluate_equ("*", &symbols, "", 80).unwrap());
        let mut value = evaluate_equ(text, &symbols, "", 80).unwrap();
        attributes.apply(&mut value);
        assert_eq!(value.value, 80);
        assert_eq!(value.program_type, Some(32));
        assert_eq!(value.assembler_type.as_deref(), Some("GR32"));
    }
}

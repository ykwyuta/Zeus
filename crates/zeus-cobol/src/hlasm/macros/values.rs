use super::super::assemble::{Symbols, expression};
use super::{Expansion, Scope, SetKind, SetSymbol, condition, operands};

const MAX_INDEX: usize = 1_000_000;
const MAX_STRING: usize = 4064;
const MAX_SUBSTITUTION: usize = 1 << 20;

struct Reference {
    name: String,
    indices: Vec<usize>,
    created: bool,
}

fn name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || b"_@$#".contains(&c)
}

/// Return the inside of a balanced parenthesis group. Quotes are respected,
/// including doubled apostrophes; array subscripts can themselves be arrays.
pub(super) fn group(text: &str, at: &mut usize) -> Result<String, String> {
    if text.as_bytes().get(*at) != Some(&b'(') {
        return Err("opening parenthesis required".into());
    }
    *at += 1;
    let start = *at;
    let (mut depth, mut quoted) = (1, false);
    while let Some(&c) = text.as_bytes().get(*at) {
        if c == b'\'' && (quoted || !super::super::assemble::attribute_quote(text, *at)) {
            quoted = !quoted;
        }
        if !quoted {
            if c == b'(' {
                depth += 1;
            }
            if c == b')' {
                depth -= 1;
                if depth == 0 {
                    let value = text[start..*at].to_string();
                    *at += 1;
                    return Ok(value);
                }
            }
        }
        *at += 1;
    }
    Err("unclosed macro parenthesis".into())
}

pub(super) fn number(text: &str) -> Result<i64, String> {
    number_at(text, 0)
}

fn number_at(text: &str, level: usize) -> Result<i64, String> {
    if level > 64 {
        return Err("macro arithmetic nesting exceeds 64".into());
    }
    let text = text.trim();
    if text.is_empty() {
        return Ok(0);
    }
    let mut quoted = false;
    let upper: String = text
        .chars()
        .map(|c| {
            if c == '\'' {
                quoted = !quoted;
            }
            if quoted { c } else { c.to_ascii_uppercase() }
        })
        .collect();
    let (mut depth, mut quote) = (0i32, false);
    for (at, c) in upper.char_indices() {
        if c == '\'' {
            quote = !quote;
        }
        if quote {
            continue;
        }
        if c == '(' {
            depth += 1;
        }
        if c == ')' {
            depth -= 1;
        }
        if depth == 0 {
            for op in [" INDEX ", " FIND "] {
                if upper[at..].starts_with(op) {
                    return numeric_function(
                        op.trim(),
                        &[text[..at].into(), text[at + op.len()..].into()],
                    );
                }
            }
        }
    }
    for operators in [
        &[" XOR "][..],
        &[" OR "][..],
        &[" AND "][..],
        &[" SLL ", " SRL ", " SLA ", " SRA "][..],
    ] {
        let (mut depth, mut quote, mut found) = (0, false, None);
        for (at, c) in upper.char_indices() {
            if c == '\'' {
                quote = !quote;
            }
            if quote {
                continue;
            }
            if c == '(' {
                depth += 1;
            }
            if c == ')' {
                depth -= 1;
            }
            if depth == 0 {
                for &op in operators {
                    if upper[at..].starts_with(op) {
                        found = Some((at, op));
                    }
                }
            }
        }
        if let Some((at, op)) = found {
            let a = i32::try_from(number_at(&upper[..at], level + 1)?)
                .map_err(|_| "bit operation exceeds 32 bits")?;
            let b = i32::try_from(number_at(&upper[at + op.len()..], level + 1)?)
                .map_err(|_| "bit operation exceeds 32 bits")?;
            let shift = b as u32 & 63;
            return Ok(i64::from(match op {
                " OR " => a | b,
                " XOR " => a ^ b,
                " AND " => a & b,
                " SLL " => (a as u32).checked_shl(shift).unwrap_or(0) as i32,
                " SRL " => (a as u32).checked_shr(shift).unwrap_or(0) as i32,
                " SLA " => {
                    ((a as u32 & 0x8000_0000)
                        | ((a as u32).checked_shl(shift).unwrap_or(0) & 0x7fff_ffff))
                        as i32
                }
                _ => a >> shift.min(31),
            }));
        }
    }
    if let Some(inner) = upper.strip_prefix("NOT ") {
        return Ok(i64::from(
            !i32::try_from(number_at(inner, level + 1)?)
                .map_err(|_| "bit operation exceeds 32 bits")?,
        ));
    }
    let mut reduced = String::new();
    let mut at = 0;
    validate_depth(text)?;
    while at < text.len() {
        let start = at;
        let c = text.as_bytes()[at];
        if c == b'(' {
            let inner = group(text, &mut at)?;
            reduced.push_str(&format!("({})", number_at(&inner, level + 1)?));
        } else if c.is_ascii_alphabetic() {
            while text.as_bytes().get(at).is_some_and(|c| name_char(*c)) {
                at += 1;
            }
            if text.as_bytes().get(at) == Some(&b'(') {
                let name = text[start..at].to_ascii_uppercase();
                let args = group(text, &mut at)?;
                reduced.push_str(&format!("({})", numeric_function(&name, &operands(&args))?));
            } else if matches!(
                text[start..at].to_ascii_uppercase().as_str(),
                "BYTE" | "SIGNED"
            ) && text.as_bytes().get(at).is_some_and(u8::is_ascii_whitespace)
            {
                return Err("character function used in arithmetic expression".into());
            } else if text.as_bytes().get(at) == Some(&b'\'')
                && at - start == 1
                && b"XBCxbc".contains(&c)
            {
                at += 1;
                loop {
                    let next = *text
                        .as_bytes()
                        .get(at)
                        .ok_or("unterminated self-defining term")?;
                    at += 1;
                    if next == b'\'' {
                        if text.as_bytes().get(at) == Some(&b'\'') {
                            at += 1;
                        } else {
                            break;
                        }
                    }
                }
                let n = expression(&text[start..at], &Symbols::new(), "", 0)?.value;
                let n = u32::try_from(n).map_err(|_| "self-defining term exceeds 32 bits")? as i32;
                reduced.push_str(&format!("({n})"));
            } else {
                reduced.push_str(&text[start..at]);
            }
        } else {
            let c = text[at..].chars().next().unwrap();
            reduced.push(c);
            at += c.len_utf8();
        }
    }
    let n = expression(&reduced, &Symbols::new(), "", 0)?;
    if n.section.is_some() {
        return Err("absolute macro arithmetic value required".into());
    }
    Ok(n.value)
}

fn validate_depth(text: &str) -> Result<(), String> {
    let (mut depth, mut quoted) = (0usize, false);
    let mut unary = 0;
    for (i, c) in text.char_indices() {
        if c == '\'' && (quoted || !super::super::assemble::attribute_quote(text, i)) {
            quoted = !quoted;
        }
        if !quoted {
            if matches!(c, '+' | '-') {
                unary += 1;
                if unary > 64 {
                    return Err("macro unary nesting exceeds 64".into());
                }
            } else if !c.is_whitespace() {
                unary = 0;
            }
            if c == '(' {
                depth += 1;
                if depth > 64 {
                    return Err("macro expression nesting exceeds 64".into());
                }
            }
            if c == ')' {
                depth = depth.saturating_sub(1);
            }
        }
    }
    Ok(())
}

fn one_arg(args: &[String]) -> Result<&str, String> {
    if args.len() != 1 {
        return Err("function requires one argument".into());
    }
    Ok(&args[0])
}

fn numeric_function(name: &str, args: &[String]) -> Result<i64, String> {
    if name == "DCLEN" {
        return Ok(paired(&character(one_arg(args)?)?).chars().count() as i64);
    }
    if matches!(name, "INDEX" | "FIND") {
        if args.len() != 2 {
            return Err("search function requires two character arguments".into());
        }
        let a = character(&args[0])?;
        let b = character(&args[1])?;
        if a.is_empty() || b.is_empty() {
            return Ok(0);
        }
        return Ok(if name == "INDEX" {
            a.find(&b).map_or(0, |i| a[..i].chars().count() + 1)
        } else {
            a.chars().position(|c| b.contains(c)).map_or(0, |i| i + 1)
        } as i64);
    }
    let a = character(one_arg(args)?)?;
    let n = match name {
        "B2A" | "X2A" => {
            let (base, limit) = if name == "B2A" { (2, 32) } else { (16, 8) };
            if a.len() > limit {
                return Err("representation conversion exceeds 32 bits".into());
            }
            if a.is_empty() {
                0
            } else {
                i64::from(
                    u32::from_str_radix(&a, base).map_err(|_| "invalid representation digits")?
                        as i32,
                )
            }
        }
        "C2A" => {
            let bytes = crate::ebcdic::encode(&a);
            if bytes.len() > 4 {
                return Err("C2A requires at most four bytes".into());
            }
            i64::from(bytes.into_iter().fold(0u32, |n, b| n * 256 + u32::from(b)) as i32)
        }
        "D2A" => i64::from(if a.is_empty() {
            0
        } else {
            a.parse::<i32>()
                .map_err(|_| "invalid signed decimal representation")?
        }),
        "ISBIN" | "ISHEX" | "ISDEC" | "ISSYM" => {
            if a.is_empty() {
                return Err("validity function requires a nonempty string".into());
            }
            let valid = match name {
                "ISBIN" => a.len() <= 32 && a.bytes().all(|b| b == b'0' || b == b'1'),
                "ISHEX" => a.len() <= 8 && a.bytes().all(|b| b.is_ascii_hexdigit()),
                "ISDEC" => {
                    a.len() <= 10
                        && a.bytes().all(|b| b.is_ascii_digit())
                        && a.parse::<i32>().is_ok()
                }
                _ => a.len() <= 63 && a.bytes().all(name_char) && !a.as_bytes()[0].is_ascii_digit(),
            };
            i64::from(valid)
        }
        _ => return Err(format!("unsupported arithmetic macro function {name}")),
    };
    Ok(n)
}

fn character_function(name: &str, args: &[String]) -> Result<String, String> {
    let input = one_arg(args)?;
    match name {
        "B2D" | "C2D" | "X2D" => Ok(format!(
            "{:+}",
            numeric_function(&format!("{}2A", &name[..1]), args)?
        )),
        "D2B" | "D2C" | "D2X" => {
            let value = character(input)?;
            if value.is_empty() {
                return if name == "D2B" {
                    Ok(String::new())
                } else {
                    Err("decimal character conversion requires a nonempty string".into())
                };
            }
            let value = value
                .parse::<i32>()
                .map_err(|_| "invalid signed decimal representation")?;
            Ok(match name {
                "D2B" => format!("{:032b}", value as u32),
                "D2X" => format!("{:08X}", value as u32),
                _ => crate::ebcdic::decode(&value.to_be_bytes()),
            })
        }
        "C2B" | "C2X" => {
            let bytes = crate::ebcdic::encode(&character(input)?);
            let width = if name == "C2B" { 8 } else { 2 };
            if bytes.len() > MAX_STRING / width {
                return Err("character value exceeds 4064 characters".into());
            }
            Ok(bytes
                .iter()
                .map(|b| {
                    if name == "C2B" {
                        format!("{b:08b}")
                    } else {
                        format!("{b:02X}")
                    }
                })
                .collect())
        }
        "B2C" | "B2X" | "X2B" | "X2C" => {
            let value = character(input)?;
            let binary = name.starts_with('B');
            if !value.bytes().all(|b| {
                if binary {
                    matches!(b, b'0' | b'1')
                } else {
                    b.is_ascii_hexdigit()
                }
            }) {
                return Err("invalid representation digits".into());
            }
            if name == "X2B" {
                if value.len() > MAX_STRING / 4 {
                    return Err("character value exceeds 4064 characters".into());
                }
                return Ok(value
                    .chars()
                    .map(|c| format!("{:04b}", c.to_digit(16).unwrap()))
                    .collect());
            }
            let width = if name == "B2C" {
                8
            } else if binary {
                4
            } else {
                2
            };
            let padded = format!(
                "{}{}",
                "0".repeat((width - value.len() % width) % width),
                value
            );
            let bytes = padded
                .as_bytes()
                .chunks(width)
                .map(|chunk| {
                    u8::from_str_radix(
                        std::str::from_utf8(chunk).unwrap(),
                        if binary { 2 } else { 16 },
                    )
                    .unwrap()
                })
                .collect::<Vec<_>>();
            Ok(if name == "B2X" {
                bytes.iter().map(|b| format!("{b:X}")).collect()
            } else {
                crate::ebcdic::decode(&bytes)
            })
        }
        "DCVAL" => Ok(paired(&character(input)?)),
        "DEQUOTE" => {
            let value = character(input)?;
            let value = value.strip_prefix('\'').unwrap_or(&value);
            Ok(value.strip_suffix('\'').unwrap_or(value).to_string())
        }
        "UPPER" => Ok(character(input)?.to_ascii_uppercase()),
        "LOWER" => Ok(character(input)?.to_ascii_lowercase()),
        "DOUBLE" => Ok(character(input)?.replace('\'', "''").replace('&', "&&")),
        "A2B" | "A2X" | "A2D" | "A2C" | "SIGNED" | "BYTE" => {
            let n = i32::try_from(number(input)?)
                .map_err(|_| "character conversion requires a signed 32-bit value")?;
            Ok(match name {
                "A2B" => format!("{:032b}", n as u32),
                "A2X" => format!("{:08X}", n as u32),
                "A2D" => format!("{n:+}"),
                "SIGNED" => n.to_string(),
                "BYTE" => crate::ebcdic::decode(&[n as u8]),
                _ => crate::ebcdic::decode(&n.to_be_bytes()),
            })
        }
        _ => Err(format!("unsupported character macro function {name}")),
    }
}

fn paired(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        out.push(c);
        if matches!(c, '\'' | '&') && chars.peek() == Some(&c) {
            chars.next();
        }
    }
    out
}

fn reference(
    text: &str,
    at: &mut usize,
    scope: &Scope,
    state: &Expansion,
    depth: usize,
) -> Result<Reference, String> {
    if depth > 64 {
        return Err("macro variable nesting exceeds 64".into());
    }
    if text.as_bytes().get(*at) != Some(&b'&') {
        return Err("SET variable required".into());
    }
    *at += 1;
    let created = text.as_bytes().get(*at) == Some(&b'(');
    let name = if created {
        let text = group(text, at)?;
        format!(
            "&{}",
            substitute_at(&text, scope, state, depth + 1)?.to_ascii_uppercase()
        )
    } else {
        let start = *at;
        while text.as_bytes().get(*at).is_some_and(|c| name_char(*c)) {
            *at += 1;
        }
        format!("&{}", text[start..*at].to_ascii_uppercase())
    };
    if name.len() < 2
        || name.len() > 64
        || !name.as_bytes()[1..].iter().all(|c| name_char(*c))
        || name.as_bytes()[1].is_ascii_digit()
    {
        return Err("invalid macro variable name".into());
    }
    let mut indices = Vec::new();
    if text.as_bytes().get(*at) == Some(&b'(') {
        let sub = group(text, at)?;
        for item in operands(&sub) {
            let item = substitute_at(&item, scope, state, depth + 1)?;
            let n = usize::try_from(number(&item)?).map_err(|_| "negative macro subscript")?;
            if n > MAX_INDEX {
                return Err("macro subscript exceeds 1,000,000".into());
            }
            indices.push(n);
        }
    }
    Ok(Reference {
        name,
        indices,
        created,
    })
}

fn parameter_item(mut value: String, indices: &[usize]) -> Result<String, String> {
    for &index in indices {
        if index == 0 {
            return Err("sublist indices start at 1".into());
        }
        let list = if let Some(inner) = value.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            operands(inner)
        } else if value.is_empty() {
            vec![]
        } else {
            vec![value]
        };
        value = list.get(index - 1).cloned().unwrap_or_default();
    }
    Ok(value)
}

impl Scope {
    fn set<'a>(&'a self, name: &str, state: &'a Expansion) -> Option<&'a SetSymbol> {
        if self.globals.contains(name) {
            state.globals.get(name)
        } else {
            self.sets.get(name)
        }
    }

    fn read(&self, r: &Reference, state: &Expansion) -> Result<String, String> {
        if !r.created {
            if r.name.starts_with("&SYS")
                && !matches!(r.name.as_str(), "&SYSLIST" | "&SYSMAC")
                && !r.indices.is_empty()
            {
                return Err("scalar system symbol cannot be subscripted".into());
            }
            if r.name == "&SYSM_HSEV" {
                return Ok(state
                    .diagnostics
                    .iter()
                    .filter(|d| d.category == "MNOTE")
                    .map(|d| d.severity)
                    .max()
                    .unwrap_or(0)
                    .to_string());
            }
            if r.name == "&SYSM_SEV" {
                return Ok(self.last_macro_severity.to_string());
            }
            if r.name == "&SYSLIST" {
                let Some((&index, tail)) = r.indices.split_first() else {
                    return Err("SYSLIST requires a positional index".into());
                };
                let value = if index == 0 {
                    self.label.clone()
                } else {
                    self.positional.get(index - 1).cloned().unwrap_or_default()
                };
                return parameter_item(value, tail);
            }
            if r.name == "&SYSMAC" {
                if r.indices.len() > 1 {
                    return Err("SYSMAC accepts one index".into());
                }
                let index = r.indices.first().copied().unwrap_or(0);
                return Ok(if index == self.macro_names.len() {
                    "OPEN CODE".into()
                } else {
                    self.macro_names.get(index).cloned().unwrap_or_default()
                });
            }
            if r.name == "&SYSNEST" {
                return Ok(self.macro_names.len().to_string());
            }
            if r.name == "&SYSSTMT" {
                return Ok(format!("{:08}", state.generated + 1));
            }
            if r.name == "&SYSOPT_CURR_OPTABLE" {
                return Ok(state.controls.optable.clone());
            }
            if let Some(value) = state.system_values.get(&r.name) {
                return Ok(value.clone());
            }
            if let Some(value) = self.values.get(&r.name) {
                return parameter_item(value.clone(), &r.indices);
            }
        }
        let set = self
            .set(&r.name, state)
            .ok_or_else(|| format!("undefined macro variable {}", r.name))?;
        if set.array {
            if r.indices.len() != 1 || r.indices[0] == 0 {
                return Err("SET array requires one positive subscript".into());
            }
            if r.indices[0] > set.extent {
                return Err("SET array subscript exceeds current extent".into());
            }
            Ok(set
                .elements
                .get(&r.indices[0])
                .cloned()
                .unwrap_or_else(|| set.kind.initial()))
        } else {
            if !r.indices.is_empty() {
                return Err("scalar SET symbol cannot be subscripted".into());
            }
            Ok(set.scalar.clone())
        }
    }

    fn attribute(&self, kind: u8, r: &Reference, state: &Expansion) -> Result<String, String> {
        if kind == b'K' {
            return Ok(self.read(r, state)?.chars().count().to_string());
        }
        if !r.created && r.name == "&SYSLIST" && r.indices.is_empty() {
            return Ok(self.positional.len().to_string());
        }
        if !r.created && r.name.starts_with("&SYS") && r.name != "&SYSLIST" {
            self.read(r, state)?;
            return Ok("0".into());
        }
        if let Some(set) = self.set(&r.name, state)
            && (r.created || !self.values.contains_key(&r.name))
        {
            if r.indices.is_empty() {
                return Ok(if set.array {
                    set.extent.to_string()
                } else {
                    "0".into()
                });
            }
            // N' of a SET element is zero; SETC text is not a parameter list.
            self.read(r, state)?;
            return Ok("0".into());
        }
        let value = self.read(r, state)?;
        Ok(if value.is_empty() {
            0
        } else if let Some(inner) = value.strip_prefix('(').and_then(|s| s.strip_suffix(')')) {
            operands(inner).len()
        } else {
            1
        }
        .to_string())
    }

    pub(super) fn declare(
        &mut self,
        text: &str,
        op: &str,
        state: &mut Expansion,
    ) -> Result<(), String> {
        let kind = SetKind::from_op(op);
        for item in operands(text) {
            let mut at = 0;
            let r = reference(&item, &mut at, self, state, 0)?;
            if at != item.len() || r.indices.len() > 1 || r.indices.contains(&0) {
                return Err("invalid SET symbol declaration".into());
            }
            self.writable(&r)?;
            let dimension = r.indices.first().copied();
            let table = if op.starts_with('G') {
                self.globals.insert(r.name.clone());
                &mut state.globals
            } else {
                &mut self.sets
            };
            if let Some(old) = table.get(&r.name) {
                if old.kind != kind || old.array != dimension.is_some() {
                    return Err("inconsistent SET symbol declaration".into());
                }
            } else {
                table.insert(r.name, SetSymbol::new(kind, dimension));
            }
        }
        Ok(())
    }

    fn writable(&self, r: &Reference) -> Result<(), String> {
        if !r.created && (r.name.starts_with("&SYS") || self.values.contains_key(&r.name)) {
            return Err(format!(
                "read-only macro parameter or system symbol {}",
                r.name
            ));
        }
        Ok(())
    }

    pub(super) fn assign(
        &mut self,
        label: &str,
        op: &str,
        text: &str,
        state: &mut Expansion,
    ) -> Result<(), String> {
        let mut at = 0;
        let r = reference(label, &mut at, self, state, 0)?;
        if at != label.len() || r.indices.len() > 1 || r.indices.contains(&0) {
            return Err("invalid SET assignment target".into());
        }
        self.writable(&r)?;
        let kind = SetKind::from_op(op);
        if self.set(&r.name, state).is_none() {
            self.sets.insert(
                r.name.clone(),
                SetSymbol::new(kind, r.indices.first().copied()),
            );
        }
        let old = self.set(&r.name, state).unwrap();
        if old.kind != kind || old.array != !r.indices.is_empty() {
            return Err("SET assignment disagrees with symbol type or dimension".into());
        }
        let items = operands(text);
        if r.indices.is_empty() && items.len() != 1 {
            return Err("multiple SET values require a subscripted target".into());
        }
        for (index, item) in items.iter().enumerate() {
            if item.is_empty() && old_array(self, &r.name, state) {
                continue;
            }
            let expanded = macro_expression(item, self, state)?;
            let value = match kind {
                SetKind::Arithmetic => i32::try_from(number(&expanded)?)
                    .map_err(|_| "SETA value outside signed 32-bit range")?
                    .to_string(),
                SetKind::Boolean => u8::from(condition(&expanded)?).to_string(),
                SetKind::Character => character(&expanded)?,
            };
            let set = if self.globals.contains(&r.name) {
                state.globals.get_mut(&r.name).unwrap()
            } else {
                self.sets.get_mut(&r.name).unwrap()
            };
            if set.array {
                let position = r.indices[0]
                    .checked_add(index)
                    .filter(|n| *n <= MAX_INDEX)
                    .ok_or("macro subscript exceeds 1,000,000")?;
                set.extent = set.extent.max(position);
                set.elements.insert(position, value);
            } else {
                set.scalar = value;
            }
        }
        Ok(())
    }
}

fn old_array(scope: &Scope, name: &str, state: &Expansion) -> bool {
    scope.set(name, state).is_some_and(|s| s.array)
}

pub(super) fn substitute(text: &str, scope: &Scope, state: &Expansion) -> Result<String, String> {
    attribute_queries(&substitute_at(text, scope, state, 0)?, state, 0)
}

pub(super) fn macro_expression(
    text: &str,
    scope: &Scope,
    state: &mut Expansion,
) -> Result<String, String> {
    let (mut at, mut quoted) = (0, false);
    let mut out = String::new();
    while at < text.len() {
        let c = text.as_bytes()[at];
        if !quoted
            && b"LTDSIOltdsio".contains(&c)
            && text.as_bytes().get(at + 1) == Some(&b'\'')
            && (at == 0 || !name_char(text.as_bytes()[at - 1]))
        {
            let kind = c.to_ascii_uppercase();
            at += 2;
            let mut value;
            if text.as_bytes().get(at) == Some(&b'&') {
                let r = reference(text, &mut at, scope, state, 0)?;
                if kind == b'T' && r.name.starts_with("&SYS") && r.name != "&SYSLIST" {
                    let value = scope.read(&r, state)?;
                    out.push_str(if super::super::context::numeric_symbol(&r.name) {
                        "'N'"
                    } else if r.name == "&SYSPARM" && value.is_empty() {
                        "'O'"
                    } else {
                        "'U'"
                    });
                    continue;
                }
                if let Some(set) = scope.set(&r.name, state)
                    && (r.created || !scope.values.contains_key(&r.name))
                    && set.kind != SetKind::Character
                {
                    if kind == b'T' {
                        out.push_str("'N'");
                        continue;
                    }
                    return Err(
                        "length/defined attribute cannot reference a SETA or SETB symbol".into(),
                    );
                }
                if kind == b'D' && r.name.starts_with("&SYS") && r.name != "&SYSLIST" {
                    return Err("defined attribute cannot reference a system variable".into());
                }
                value = scope.read(&r, state)?;
                if text.as_bytes().get(at) == Some(&b'.') {
                    at += 1;
                }
                if kind != b'D' {
                    for _ in 0..64 {
                        let Some(inner) = value.strip_prefix('(').and_then(|s| s.strip_suffix(')'))
                        else {
                            break;
                        };
                        value = operands(inner).into_iter().next().unwrap_or_default();
                    }
                    if !value.starts_with('=') {
                        if kind != b'O'
                            && let Some((qualifier, symbol)) = value.split_once('.')
                            && super::super::assemble::symbol_name(qualifier)
                        {
                            value = symbol.into();
                        }
                        let end = value.bytes().take_while(|b| name_char(*b)).count();
                        let omitted = value.is_empty();
                        value.truncate(end);
                        if !omitted && value.is_empty() {
                            value.push('?');
                        }
                    }
                }
            } else {
                let start = at;
                let literal = text.as_bytes().get(at) == Some(&b'=');
                if literal {
                    at = super::super::assemble::literal_end(text, at)?;
                } else {
                    while text.as_bytes().get(at).is_some_and(|c| name_char(*c)) {
                        at += 1;
                    }
                    if kind != b'O'
                        && text.as_bytes().get(at) == Some(&b'.')
                        && text.as_bytes().get(at + 1).is_some_and(|c| name_char(*c))
                    {
                        at += 1;
                        while text.as_bytes().get(at).is_some_and(|c| name_char(*c)) {
                            at += 1;
                        }
                    }
                }
                if at == start {
                    return Err("attribute requires an ordinary symbol or literal".into());
                }
                value = text[start..at].into();
                if literal {
                    value = substitute(&value, scope, state)?;
                }
            }
            if kind == b'O' {
                out.push_str(&format!("'{}'", state.operation_attribute(&value)));
                continue;
            }
            if value.starts_with('=') {
                out.push_str(&state.literal_attribute(&value, kind)?);
                continue;
            }
            if let Some((qualifier, symbol)) = value.split_once('.')
                && super::super::assemble::symbol_name(qualifier)
            {
                value = symbol.into();
            }
            if !super::super::assemble::symbol_name(&value) {
                if kind == b'T' {
                    out.push_str(if value.is_empty() { "'O'" } else { "'U'" });
                    continue;
                }
                return Err("attribute operand must represent an ordinary symbol".into());
            }
            out.push_str(&state.ordinary_attribute(&value, kind)?);
        } else {
            let ch = text[at..].chars().next().unwrap();
            if ch == '\'' && (quoted || !super::super::assemble::attribute_quote(text, at)) {
                quoted = !quoted;
            }
            out.push(ch);
            at += ch.len_utf8();
        }
        if out.len() > MAX_SUBSTITUTION {
            return Err("expanded macro operand exceeds 1 MiB".into());
        }
    }
    substitute(&out, scope, state)
}

fn attribute_queries(text: &str, state: &Expansion, depth: usize) -> Result<String, String> {
    if depth > 64 {
        return Err("attribute query nesting exceeds 64".into());
    }
    let (mut at, mut quoted) = (0, false);
    let mut out = String::new();
    while at < text.len() {
        let ch = text[at..].chars().next().unwrap();
        if ch == '\'' && (quoted || !super::super::assemble::attribute_quote(text, at)) {
            quoted = !quoted;
        }
        if !quoted && ch.is_ascii_alphabetic() && (at == 0 || !name_char(text.as_bytes()[at - 1])) {
            let start = at;
            while text.as_bytes().get(at).is_some_and(|c| name_char(*c)) {
                at += 1;
            }
            let name = text[start..at].to_ascii_uppercase();
            if matches!(name.as_str(), "SYSATTRP" | "SYSATTRA")
                && text.as_bytes().get(at) == Some(&b'(')
            {
                let args = group(text, &mut at)?;
                let args = attribute_queries(&args, state, depth + 1)?;
                let fields = operands(&args);
                if fields.len() != 1 {
                    return Err(format!("{name} requires one character argument"));
                }
                let symbol = if fields[0].is_empty() {
                    String::new()
                } else {
                    character(&fields[0])
                        .map_err(|e| format!("{name} requires a character expression: {e}"))?
                };
                let value = state.query_attribute(&symbol, name == "SYSATTRP")?;
                out.push('\'');
                out.push_str(&value.replace('\'', "''"));
                out.push('\'');
            } else {
                out.push_str(&text[start..at]);
            }
        } else {
            out.push(ch);
            at += ch.len_utf8();
        }
        if out.len() > MAX_SUBSTITUTION {
            return Err("expanded macro operand exceeds 1 MiB".into());
        }
    }
    Ok(out)
}

fn substitute_at(
    text: &str,
    scope: &Scope,
    state: &Expansion,
    depth: usize,
) -> Result<String, String> {
    if depth > 64 {
        return Err("macro substitution nesting exceeds 64".into());
    }
    let (mut at, mut quoted) = (0, false);
    let mut out = String::new();
    while at < text.len() {
        let c = text[at..].chars().next().unwrap();
        if !quoted
            && c.is_ascii()
            && b"KNkn".contains(&(c as u8))
            && text.as_bytes().get(at + 1) == Some(&b'\'')
            && text.as_bytes().get(at + 2) == Some(&b'&')
        {
            at += 2;
            let r = reference(text, &mut at, scope, state, depth + 1)?;
            out.push_str(&scope.attribute((c as u8).to_ascii_uppercase(), &r, state)?);
        } else if c == '&' {
            if text.as_bytes().get(at + 1) == Some(&b'&') {
                out.push('&');
                at += 2;
                continue;
            }
            let r = reference(text, &mut at, scope, state, depth + 1)?;
            let mut value = scope.read(&r, state)?;
            if quoted {
                if scope
                    .set(&r.name, state)
                    .is_some_and(|s| s.kind == SetKind::Arithmetic)
                    && (r.created || !scope.values.contains_key(&r.name))
                {
                    value = value.trim_start_matches('-').to_string();
                }
                value = value.replace('\'', "''");
            }
            out.push_str(&value);
            if text.as_bytes().get(at) == Some(&b'.') {
                at += 1;
            }
        } else {
            if c == '\'' {
                quoted = !quoted;
            }
            out.push(c);
            at += c.len_utf8();
        }
        if out.len() > MAX_SUBSTITUTION {
            return Err("expanded macro operand exceeds 1 MiB".into());
        }
    }
    Ok(out)
}

struct Character<'a> {
    text: &'a str,
    at: usize,
}

impl Character<'_> {
    fn spaces(&mut self) {
        while self
            .text
            .as_bytes()
            .get(self.at)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.at += 1;
        }
    }
    fn term(&mut self) -> Result<String, String> {
        self.spaces();
        let mut grouped = None;
        let copies = if self.text.as_bytes().get(self.at) == Some(&b'(') {
            let expr = group(self.text, &mut self.at)?;
            if let Some((op, operand)) = expr.trim().split_once(char::is_whitespace)
                && matches!(
                    op.to_ascii_uppercase().as_str(),
                    "UPPER" | "LOWER" | "DOUBLE" | "BYTE" | "SIGNED"
                )
            {
                grouped = Some(character_function(
                    &op.to_ascii_uppercase(),
                    &[operand.trim().into()],
                )?);
                1
            } else {
                usize::try_from(number(&expr)?)
                    .ok()
                    .filter(|n| *n <= MAX_STRING)
                    .ok_or("invalid character duplication factor")?
            }
        } else {
            1
        };
        self.spaces();
        let mut value;
        if let Some(result) = grouped {
            value = result;
        } else if self.text.as_bytes().get(self.at) == Some(&b'\'') {
            self.at += 1;
            value = String::new();
            loop {
                let c = self.text[self.at..]
                    .chars()
                    .next()
                    .ok_or("unterminated character term")?;
                self.at += c.len_utf8();
                if c == '\'' {
                    if self.text.as_bytes().get(self.at) == Some(&b'\'') {
                        self.at += 1;
                    } else {
                        break;
                    }
                }
                value.push(c);
                if value.chars().count() > MAX_STRING {
                    return Err("character value exceeds 4064 characters".into());
                }
            }
        } else {
            let start = self.at;
            while self
                .text
                .as_bytes()
                .get(self.at)
                .is_some_and(|c| name_char(*c))
            {
                self.at += 1;
            }
            if self.at == start {
                return Err("quoted character term or function required".into());
            }
            let name = self.text[start..self.at].to_ascii_uppercase();
            let args = group(self.text, &mut self.at)?;
            value = character_function(&name, &operands(&args))?;
        }
        self.spaces();
        if self.text.as_bytes().get(self.at) == Some(&b'(') {
            let sub = group(self.text, &mut self.at)?;
            let fields = operands(&sub);
            if fields.len() != 2 {
                return Err("substring requires start,length".into());
            }
            let start = usize::try_from(number(&fields[0])?)
                .ok()
                .filter(|n| *n > 0)
                .ok_or("substring start must be positive")?;
            let length = if fields[1].trim() == "*" {
                MAX_STRING
            } else {
                usize::try_from(number(&fields[1])?)
                    .ok()
                    .filter(|n| *n <= MAX_STRING)
                    .ok_or("invalid substring length")?
            };
            value = value.chars().skip(start - 1).take(length).collect();
        }
        if value
            .chars()
            .count()
            .checked_mul(copies)
            .is_none_or(|n| n > MAX_STRING)
        {
            return Err("character value exceeds 4064 characters".into());
        }
        Ok(value.repeat(copies))
    }
}

pub(super) fn character(text: &str) -> Result<String, String> {
    validate_depth(text)?;
    let mut parser = Character { text, at: 0 };
    let mut value = parser.term()?;
    loop {
        parser.spaces();
        if parser.at == text.len() {
            return Ok(value);
        }
        if text.as_bytes()[parser.at] == b'.' {
            parser.at += 1;
        } else if !matches!(text.as_bytes()[parser.at], b'\'' | b'(' | b'A'..=b'Z' | b'a'..=b'z') {
            return Err("invalid character concatenation".into());
        }
        value.push_str(&parser.term()?);
        if value.chars().count() > MAX_STRING {
            return Err("character value exceeds 4064 characters".into());
        }
    }
}

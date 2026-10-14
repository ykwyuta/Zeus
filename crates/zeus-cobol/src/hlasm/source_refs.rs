//! Lightweight source references shared with dependency analysis. This scanner
//! does not evaluate layout expressions or require macro expansion to succeed.
use super::{assemble, constants};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstantReference {
    pub name: String,
    /// Zero-based byte offset within the supplied operand text.
    pub offset: usize,
    pub kind: char,
}
fn nominal(spec: &str, origin: usize, refs: &mut Vec<ConstantReference>) {
    let spec = spec.to_ascii_uppercase();
    let Ok(at) = constants::type_start(&spec) else {
        return;
    };
    let kind = spec.as_bytes()[at] as char;
    if !matches!(kind, 'V' | 'Q' | 'J' | 'R') {
        return;
    }
    let mut tail = at + 1;
    if spec[tail..].starts_with('D') || (kind == 'Q' && spec[tail..].starts_with('Y')) {
        tail += 1;
    }
    if spec[tail..].starts_with("P(") {
        let Ok(end) = constants::parenthesized_end(&spec[tail + 1..]) else {
            return;
        };
        tail += 1 + end;
    }
    if spec[tail..].starts_with('L') {
        tail += 1;
        if spec[tail..].starts_with('.') {
            return;
        }
        if spec[tail..].starts_with('(') {
            let Ok(end) = constants::parenthesized_end(&spec[tail..]) else {
                return;
            };
            tail += end;
        } else {
            tail += spec[tail..].bytes().take_while(u8::is_ascii_digit).count();
        }
    }
    if !spec[tail..].starts_with('(') {
        return;
    }
    let Ok(end) = constants::parenthesized_end(&spec[tail..]) else {
        return;
    };
    let inner = &spec[tail + 1..tail + end - 1];
    let mut from = 0;
    for name in assemble::operands(inner) {
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let Some(at) = inner[from..].find(name).map(|at| at + from) else {
            continue;
        };
        from = at + name.len();
        if assemble::symbol_name(name) || name.contains('&') {
            refs.push(ConstantReference {
                name: name.into(),
                offset: origin + tail + 1 + at,
                kind,
            });
        }
    }
}
/// Find V/Q/J/R nominal references in DC operands or machine-operand literals.
/// Unknown/malformed declarations remain the compiler's diagnostic responsibility.
pub fn constant_references(text: &str, literals_only: bool) -> Vec<ConstantReference> {
    let mut refs = Vec::new();
    if !literals_only {
        let mut from = 0;
        for spec in assemble::operands(text) {
            let leading = spec.len() - spec.trim_start().len();
            nominal(spec.trim(), from + leading, &mut refs);
            from += spec.len() + 1;
        }
    } else {
        let (mut at, mut quoted) = (0, false);
        while at < text.len() {
            let c = text.as_bytes()[at];
            if c == b'\'' && (quoted || !assemble::attribute_quote(text, at)) {
                quoted = !quoted;
            }
            if !quoted
                && c == b'='
                && let Ok(end) = assemble::literal_end(text, at)
            {
                nominal(&text[at + 1..end], at + 1, &mut refs);
                at = end;
                continue;
            }
            at += 1;
        }
    }
    refs
}

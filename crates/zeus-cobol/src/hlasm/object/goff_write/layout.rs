//! Element placement must reproduce every normalized image offset exactly.
use crate::hlasm::Module;
use std::{collections::BTreeSet, ops::Range};

pub(super) struct Element {
    pub id: u32,
    pub class: String,
    pub range: Range<u32>,
    pub alignment: u32,
    pub readonly: bool,
    pub executable: bool,
}

fn base36(mut n: u64, width: usize) -> String {
    let mut bytes = vec![b'0'; width];
    for b in bytes.iter_mut().rev() {
        *b = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"[(n % 36) as usize];
        n /= 36;
    }
    String::from_utf8(bytes).unwrap()
}

fn generated_class(module: &str, ordinal: usize) -> Result<String, String> {
    if ordinal >= 26usize.pow(4) {
        return Err("GOFF output exceeds its generated element-name limit".into());
    }
    // Class names are global to a bound program, not scoped by SD. Include a
    // stable module-name fingerprint so unrelated modules do not all contribute
    // their first/second fragments to the same classes. Z_ is a translator prefix.
    let fingerprint = module.bytes().fold(0xcbf29ce484222325u64, |hash, b| {
        (hash ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    let leading = (b'A' + ((fingerprint / 36u64.pow(9)) % 26) as u8) as char;
    // Letter-only ordinals sort identically in ASCII and EBCDIC. Mixing
    // digits and letters would reverse their order across these encodings.
    let mut ordinal = ordinal;
    let mut suffix = [b'A'; 4];
    for b in suffix.iter_mut().rev() {
        *b += (ordinal % 26) as u8;
        ordinal /= 26;
    }
    Ok(format!(
        "Z_{leading}{}{}",
        base36(fingerprint, 9),
        std::str::from_utf8(&suffix).unwrap()
    ))
}

fn coverage(ranges: &[Range<u32>], area: &Range<u32>) -> (bool, bool) {
    let mut at = area.start;
    let mut any = false;
    let first = ranges.partition_point(|r| r.end <= area.start);
    for r in &ranges[first..] {
        if r.start >= area.end {
            break;
        }
        if r.start < area.end && area.start < r.end {
            any = true;
            if r.start > at {
                return (true, false);
            }
            at = at.max(r.end.min(area.end));
        }
    }
    (any, at == area.end)
}

fn properties(
    module: &Module,
    code: &[Range<u32>],
    area: &Range<u32>,
) -> Result<(bool, bool), String> {
    let (some, all) = coverage(&module.read_only, area);
    if some && !all {
        return Err("GOFF output named element has partial read-only protection".into());
    }
    let (has_code, executable) = coverage(&module.machine_code, area);
    // Source instruction metadata is compiled into architectural text. As in
    // the original writer, its enclosing source element can also contain data.
    let source = coverage(code, area).0;
    if has_code && !executable {
        return Err("GOFF output named element has partial executability".into());
    }
    Ok((all, executable || source))
}

pub(super) fn plan(module: &Module, code: &[Range<u32>]) -> Result<Vec<Element>, String> {
    let length = module.storage.len() as u32;
    let mut result = Vec::new();
    if !module.bindings.sections.is_empty() {
        let mut end = 0u32;
        let mut previous_class = "";
        for section in &module.bindings.sections {
            let class = section.class.as_deref().unwrap_or(&section.name);
            if section.dummy
                || !section.concatenate
                || section.priority != 0
                || section.name != class
                || section.alignment > module.alignment
                || !matches!(section.alignment, 1 | 2 | 4 | 8 | 16 | 4096)
            {
                return Err("GOFF output requires loadable concatenating class elements".into());
            }
            let start = end.next_multiple_of(section.alignment);
            // Do not move source bytes or invent a class for an unowned gap.
            if section.image != Some(start)
                || start > length
                || (start != end
                    && (module.storage[end as usize..start as usize]
                        .iter()
                        .any(|b| *b != 0)
                        || coverage(&module.read_only, &(end..start)).0
                        || coverage(&module.machine_code, &(end..start)).0
                        || coverage(code, &(end..start)).0))
            {
                return Err(
                    "GOFF output image gaps require multiple elements with explicit class metadata"
                        .into(),
                );
            }
            if class < previous_class {
                return Err("GOFF output class order would move image bytes".into());
            }
            previous_class = class;
            end = start
                .checked_add(section.length)
                .filter(|n| *n <= length)
                .ok_or("GOFF output element exceeds its image")?;
            if start == end {
                return Err("GOFF output zero-length elements require further support".into());
            }
            let area = start..end;
            let (readonly, executable) = properties(module, code, &area)?;
            result.push(Element {
                id: result.len() as u32 + 2,
                class: class.to_owned(),
                range: area,
                alignment: if result.is_empty() {
                    module.alignment
                } else {
                    section.alignment
                },
                readonly,
                executable,
            });
        }
        if end != length {
            append_fragments(module, code, end..length, &mut result)?;
        }
    } else {
        let (some_ro, all_ro) = coverage(&module.read_only, &(0..length));
        let (some_code, all_code) = coverage(&module.machine_code, &(0..length));
        if (!some_ro || all_ro) && (!some_code || all_code) {
            result.push(Element {
                id: 2,
                class: "B_TEXT".into(),
                range: 0..length,
                alignment: module.alignment,
                readonly: all_ro,
                executable: all_code || !code.is_empty(),
            });
        } else {
            append_fragments(module, code, 0..length, &mut result)?;
        }
    }
    for element in &result {
        if element.class.len() > 16 {
            return Err("GOFF output class name exceeds 16 bytes".into());
        }
    }
    for r in code {
        position(&result, r.start, r.end - r.start)?;
    }
    for label in &module.bindings.labels {
        if let Some(class) = &label.class {
            let (id, _) = position(&result, label.offset, 1)?;
            if result
                .iter()
                .find(|e| e.id == id)
                .is_none_or(|e| &e.class != class)
            {
                return Err("GOFF output label classes do not match their element".into());
            }
        }
    }
    Ok(result)
}

fn append_fragments(
    module: &Module,
    code: &[Range<u32>],
    area: Range<u32>,
    result: &mut Vec<Element>,
) -> Result<(), String> {
    let mut boundaries = BTreeSet::from([area.start, area.end]);
    for r in module
        .read_only
        .iter()
        .chain(module.machine_code.iter())
        .chain(code.iter())
    {
        for at in [r.start, r.end] {
            if area.start < at && at < area.end {
                boundaries.insert(at);
            }
        }
    }
    let points: Vec<_> = boundaries.into_iter().collect();
    let first = result.len();
    for pair in points.windows(2) {
        let range = pair[0]..pair[1];
        let readonly = coverage(&module.read_only, &range).1;
        let executable = coverage(&module.machine_code, &range).1 || coverage(code, &range).1;
        if result.len() > first
            && result
                .last()
                .is_some_and(|p| p.readonly == readonly && p.executable == executable)
        {
            result.last_mut().unwrap().range.end = range.end;
        } else {
            let class = generated_class(&module.name, result.len())?;
            if result.last().is_some_and(|p| p.class > class) {
                return Err("GOFF output generated class order would move image bytes".into());
            }
            result.push(Element {
                id: result.len() as u32 + 2,
                class,
                range,
                alignment: if result.is_empty() {
                    module.alignment
                } else {
                    1
                },
                readonly,
                executable,
            });
        }
    }
    Ok(())
}

pub(super) fn position(elements: &[Element], at: u32, width: u32) -> Result<(u32, u32), String> {
    let index = elements.partition_point(|e| e.range.start <= at);
    let element = index
        .checked_sub(1)
        .and_then(|n| elements.get(n))
        .filter(|e| at.checked_add(width).is_some_and(|end| end <= e.range.end))
        .ok_or("GOFF output field or instruction crosses an element boundary")?;
    Ok((element.id, at - element.range.start))
}

//! Data layouts of a program interface (the `.dai` files of a z/OS
//! asset): the fields of a COMMAREA or of a message segment, with their
//! lengths and data types, and the conversions between JSON and the bytes
//! the program sees (EBCDIC characters, zoned and packed decimal, binary).
//!
//! Fields are laid out one after the other. An array has `maxOccurs`
//! elements, or as many as the value of its `dependsOn` field (OCCURS
//! DEPENDING ON). Fields with `included="N"` are not in the JSON.

use serde_json::{Map, Number, Value};
use zeus_cobol::ebcdic;
use zeus_decimal::Dec;
use zeus_decimal::zoned::SignPosition;

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Struct,
    Array {
        max: usize,
        depends_on: Option<String>,
    },
    Char,
    /// Zoned or packed decimal.
    Decimal {
        packed: bool,
        signed: bool,
        scale: u32,
        sign_leading: bool,
        sign_separate: bool,
    },
    /// Binary integer.
    Binary {
        signed: bool,
    },
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub kind: Kind,
    /// Bytes of one occurrence (a structure: of its fields).
    pub bytes: usize,
    pub included: bool,
    /// Offset in the data when the layout gives it (startPos).
    pub start: Option<usize>,
    /// The field this one redefines (REDEFINES): it starts where that one
    /// does.
    pub redefines: Option<String>,
    pub children: Vec<Field>,
}

/// The fields of the first segment of a `.dai` file.
pub fn parse(xml: &str) -> Result<Vec<Field>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;
    let seg = doc
        .descendants()
        .find(|n| n.has_tag_name("segment"))
        .ok_or("no segment in the data layout")?;
    seg.children()
        .filter(|n| n.has_tag_name("field"))
        .map(field)
        .collect()
}

fn child_text<'a>(n: roxmltree::Node<'a, 'a>, tag: &str) -> Option<&'a str> {
    n.children()
        .find(|c| c.has_tag_name(tag))
        .and_then(|c| c.text())
}

fn field(n: roxmltree::Node) -> Result<Field, String> {
    let name = n.attribute("name").unwrap_or_default().to_string();
    let bytes: usize = child_text(n, "bytes")
        .and_then(|t| t.trim().parse().ok())
        .unwrap_or(0);
    let app = n.children().find(|c| c.has_tag_name("applicationDatatype"));
    let datatype = app.and_then(|a| a.attribute("datatype")).unwrap_or("CHAR");
    let marshaller = n.children().find(|c| c.has_tag_name("marshaller"));
    let flag = |k: &str| marshaller.and_then(|m| m.attribute(k)) == Some("Y");
    let children: Vec<Field> = n
        .children()
        .filter(|c| c.has_tag_name("field"))
        .map(field)
        .collect::<Result<_, _>>()?;
    let kind = match datatype {
        "STRUCT" => Kind::Struct,
        "ARRAY" => Kind::Array {
            max: n
                .attribute("maxOccurs")
                .and_then(|m| m.parse().ok())
                .unwrap_or(1),
            depends_on: n.attribute("dependsOn").map(str::to_string),
        },
        "DECIMAL" => {
            let conv = marshaller
                .and_then(|m| child_text(m, "typeConverter"))
                .unwrap_or("ZONEDDECIMAL");
            Kind::Decimal {
                packed: conv.trim() == "PACKEDDECIMAL",
                signed: flag("isSigned"),
                scale: app
                    .and_then(|a| a.attribute("scale"))
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                sign_leading: flag("isSignLeading"),
                sign_separate: flag("isSignSeparate"),
            }
        }
        "SHORT" | "INT" | "LONG" | "BYTE" => Kind::Binary { signed: true },
        "USHORT" | "UINT" | "ULONG" | "UBYTE" => Kind::Binary { signed: false },
        _ => Kind::Char,
    };
    // the size of one occurrence: the layout's (an array's divided by its
    // occurrences), else that of its fields (not counting redefinitions)
    let own: usize = children
        .iter()
        .filter(|c| c.redefines.is_none())
        .map(Field::size)
        .sum();
    let bytes = match &kind {
        Kind::Array { max, .. } if bytes > 0 && *max > 0 => bytes / max,
        Kind::Array { .. } => own,
        Kind::Struct if bytes == 0 => own,
        _ => bytes,
    };
    let redefines = n.attribute("srcLine").and_then(|l| {
        let w: Vec<&str> = l.split_whitespace().collect();
        w.iter()
            .position(|x| x.eq_ignore_ascii_case("REDEFINES"))
            .and_then(|i| w.get(i + 1))
            .map(|x| x.trim_end_matches('.').to_string())
    });
    Ok(Field {
        name,
        kind,
        bytes,
        included: n.attribute("included") != Some("N"),
        redefines,
        start: child_text(n, "startPos")
            .and_then(|t| t.trim().parse::<usize>().ok())
            .map(|p| p.saturating_sub(1)),
        children,
    })
}

impl Field {
    /// Bytes of the field (all occurrences of an array, at most).
    pub fn size(&self) -> usize {
        match &self.kind {
            Kind::Array { max, .. } => self.bytes * max,
            _ => self.bytes,
        }
    }
}

/// The total size of the fields (arrays at their maximum).
pub fn size(fields: &[Field]) -> usize {
    fields.iter().map(Field::size).sum()
}

fn sign_pos(signed: bool, leading: bool, separate: bool) -> SignPosition {
    match (signed, leading, separate) {
        (false, ..) => SignPosition::Unsigned,
        (true, false, false) => SignPosition::TrailingOverpunch,
        (true, true, false) => SignPosition::LeadingOverpunch,
        (true, false, true) => SignPosition::TrailingSeparate,
        (true, true, true) => SignPosition::LeadingSeparate,
    }
}

fn number(d: Dec) -> Value {
    let d = d.normalize();
    if d.scale == 0
        && let Ok(i) = i64::try_from(d.coef)
    {
        return Value::Number(Number::from(i));
    }
    let t = d.to_string();
    // keep the decimal digits exactly as text, then as a JSON number
    serde_json::from_str(&t)
        .unwrap_or_else(|_| Number::from_f64(d.to_f64()).map_or(Value::Null, Value::Number))
}

fn scalar(f: &Field, b: &[u8]) -> Value {
    match &f.kind {
        Kind::Char => {
            let t = ebcdic::decode(b);
            Value::String(t.trim_end_matches([' ', '\0']).to_string())
        }
        Kind::Decimal {
            packed,
            signed,
            scale,
            sign_leading,
            sign_separate,
        } => {
            let (mag, neg) = if *packed {
                zeus_decimal::packed::decode_unchecked(b)
            } else {
                zeus_decimal::zoned::decode_unchecked(
                    b,
                    sign_pos(*signed, *sign_leading, *sign_separate),
                )
            };
            let c = mag as i128;
            number(Dec::new(if neg { -c } else { c }, *scale))
        }
        Kind::Binary { signed } => {
            Value::Number(Number::from(zeus_decimal::binary::read(b, *signed) as i64))
        }
        _ => Value::Null,
    }
}

/// The JSON of the fields in `data` (shorter data: the missing bytes are
/// blanks).
pub fn decode(fields: &[Field], data: &[u8]) -> Value {
    let mut pos = 0;
    let mut m = Map::new();
    decode_into(fields, data, &mut pos, &mut m);
    Value::Object(m)
}

fn bytes_at(data: &[u8], pos: usize, n: usize) -> Vec<u8> {
    let mut b: Vec<u8> = data
        .get(pos..(pos + n).min(data.len()))
        .unwrap_or(&[])
        .to_vec();
    b.resize(n, ebcdic::SPACE);
    b
}

/// The occurrences of an array: its counter's value, or its maximum.
fn occurrences(
    f: &Field,
    max: usize,
    depends_on: &Option<String>,
    out: &Map<String, Value>,
) -> usize {
    let _ = f;
    depends_on
        .as_ref()
        .and_then(|d| out.get(d).or_else(|| find(out, d)))
        .and_then(Value::as_u64)
        .map_or(max, |n| (n as usize).min(max))
}

/// Where a field starts: its position in the layout, where the field it
/// redefines starts, or after the previous field.
fn start_of(f: &Field, pos: usize, starts: &[(String, usize)]) -> usize {
    if let Some(s) = f.start {
        return s;
    }
    if let Some(r) = &f.redefines
        && let Some((_, s)) = starts.iter().find(|(n, _)| n == r)
    {
        return *s;
    }
    pos
}

fn decode_into(fields: &[Field], data: &[u8], pos: &mut usize, out: &mut Map<String, Value>) {
    let mut starts: Vec<(String, usize)> = Vec::new();
    let mut end = *pos;
    for f in fields {
        // a field with a position (REDEFINES overlaps) starts there
        let mut at = start_of(f, *pos, &starts);
        starts.push((f.name.clone(), at));
        let v = match &f.kind {
            Kind::Struct => {
                let mut m = Map::new();
                decode_into(&f.children, data, &mut at, &mut m);
                Value::Object(m)
            }
            Kind::Array { max, depends_on } => {
                let n = occurrences(f, *max, depends_on, out);
                let mut items = Vec::new();
                for k in 0..n {
                    let mut p = at + k * f.bytes;
                    let mut m = Map::new();
                    decode_into(&f.children, data, &mut p, &mut m);
                    items.push(Value::Object(m));
                }
                // an array with OCCURS DEPENDING ON takes the room of its
                // occurrences
                at += f.bytes * if depends_on.is_some() { n } else { *max };
                Value::Array(items)
            }
            _ => {
                let v = scalar(f, &bytes_at(data, at, f.bytes));
                at += f.bytes;
                v
            }
        };
        // a redefinition does not move past the field it redefines
        end = end.max(at);
        *pos = end;
        // the counter of OCCURS DEPENDING ON is kept hidden when it is not
        // included
        let key = if f.included {
            f.name.clone()
        } else {
            format!("\u{0}{}", f.name)
        };
        out.insert(key, v);
    }
}

fn find<'a>(m: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    m.get(&format!("\u{0}{name}"))
}

/// Removes the hidden (not included) fields.
pub fn clean(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.retain(|k, _| !k.starts_with('\u{0}'));
            m.values_mut().for_each(clean);
        }
        Value::Array(a) => a.iter_mut().for_each(clean),
        _ => {}
    }
}

/// The bytes of the fields from JSON: every field first gets its initial
/// value (blanks, zeros), then the fields in the JSON their values (so
/// that a REDEFINES not in the JSON leaves the field it redefines). An
/// array with OCCURS DEPENDING ON takes as many elements as the JSON has,
/// and its counter is set.
pub fn encode(fields: &[Field], v: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    let empty = Map::new();
    let m = v.as_object().unwrap_or(&empty);
    let mut pos = 0;
    encode_into(fields, Some(m), &mut out, &mut pos, false);
    let mut pos = 0;
    encode_into(fields, Some(m), &mut out, &mut pos, true);
    out
}

fn put(out: &mut Vec<u8>, at: usize, b: &[u8]) {
    if out.len() < at + b.len() {
        out.resize(at + b.len(), ebcdic::SPACE);
    }
    out[at..at + b.len()].copy_from_slice(b);
}

/// Writes the fields at `pos`: their initial values (`values` false) or
/// the values the JSON has.
fn encode_into(
    fields: &[Field],
    m: Option<&Map<String, Value>>,
    out: &mut Vec<u8>,
    pos: &mut usize,
    values: bool,
) {
    // counters of OCCURS DEPENDING ON from their arrays' lengths
    let mut counts: Vec<(String, usize)> = Vec::new();
    for f in fields {
        if let Kind::Array {
            depends_on: Some(d),
            max,
        } = &f.kind
        {
            let n = m
                .and_then(|m| m.get(&f.name))
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            counts.push((d.clone(), n.min(*max)));
        }
    }
    let mut starts: Vec<(String, usize)> = Vec::new();
    let mut end = *pos;
    for f in fields {
        let mut at = start_of(f, *pos, &starts);
        starts.push((f.name.clone(), at));
        let v = m.and_then(|m| m.get(&f.name));
        match &f.kind {
            Kind::Struct => {
                let child = v.and_then(Value::as_object);
                if values && child.is_none() {
                    at += f.size();
                } else {
                    encode_into(&f.children, child, out, &mut at, values);
                }
            }
            Kind::Array { max, depends_on } => {
                let items: Vec<Value> = v.and_then(Value::as_array).cloned().unwrap_or_default();
                let n = if depends_on.is_some() {
                    items.len().min(*max)
                } else {
                    *max
                };
                for k in 0..n {
                    let mut p = at + k * f.bytes;
                    let item = items.get(k).and_then(Value::as_object);
                    if values && item.is_none() {
                        continue;
                    }
                    encode_into(&f.children, item, out, &mut p, values);
                }
                at += f.bytes * n;
            }
            _ => {
                let counted = counts
                    .iter()
                    .find(|c| c.0 == f.name)
                    .map(|c| Value::from(c.1));
                let v = counted.as_ref().or(v);
                if !values || v.is_some() {
                    put(
                        out,
                        at,
                        &encode_scalar(f, if values { v } else { counted.as_ref() }),
                    );
                }
                at += f.bytes;
            }
        }
        end = end.max(at);
        *pos = end;
    }
}

fn to_dec(v: Option<&Value>, scale: u32) -> Dec {
    let d = match v {
        Some(Value::Number(n)) => Dec::parse(&n.to_string()),
        Some(Value::String(s)) => Dec::parse(s.trim()),
        Some(Value::Bool(b)) => Some(Dec::from_i64(i64::from(*b))),
        _ => None,
    }
    .unwrap_or(Dec::ZERO);
    d.rescale(scale, zeus_decimal::Rounding::Truncate)
        .unwrap_or(Dec::ZERO)
}

fn encode_scalar(f: &Field, v: Option<&Value>) -> Vec<u8> {
    let mut b = vec![0u8; f.bytes];
    match &f.kind {
        Kind::Char => {
            let t = match v {
                Some(Value::String(s)) => s.clone(),
                Some(Value::Null) | None => String::new(),
                Some(o) => crate::expr::string_of(o),
            };
            let mut e = ebcdic::encode(&t);
            e.resize(f.bytes, ebcdic::SPACE);
            b.copy_from_slice(&e[..f.bytes]);
        }
        Kind::Decimal {
            packed,
            signed,
            scale,
            sign_leading,
            sign_separate,
        } => {
            let d = to_dec(v, *scale);
            let d = if *signed {
                d
            } else {
                Dec::new(d.coef.abs(), d.scale)
            };
            if *packed {
                zeus_decimal::packed::store(&mut b, d, *signed);
            } else {
                zeus_decimal::zoned::store(
                    &mut b,
                    d,
                    sign_pos(*signed, *sign_leading, *sign_separate),
                );
            }
        }
        Kind::Binary { .. } => {
            zeus_decimal::binary::write(&mut b, to_dec(v, 0).coef);
        }
        _ => {}
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const DAI: &str = r#"<?xml version="1.0"?>
<message><segment id="1">
 <field name="OUT" included="Y"><bytes>0</bytes><applicationDatatype datatype="STRUCT"/>
  <field name="LL" included="Y"><bytes>2</bytes><marshaller isNativeInteger="N"/><applicationDatatype datatype="USHORT"/></field>
  <field name="MSG" included="Y"><bytes>5</bytes><applicationDatatype datatype="CHAR"/></field>
  <field name="AMT" included="Y"><bytes>4</bytes><marshaller isSigned="Y"><typeConverter>PACKEDDECIMAL</typeConverter></marshaller><applicationDatatype datatype="DECIMAL" precision="7" scale="2"/></field>
  <field name="NUM" included="Y"><bytes>3</bytes><marshaller isSigned="N"><typeConverter>ZONEDDECIMAL</typeConverter></marshaller><applicationDatatype datatype="DECIMAL" precision="3" scale="0"/></field>
  <field name="CNT" included="N" dependedOn="true"><bytes>1</bytes><marshaller isSigned="N"><typeConverter>ZONEDDECIMAL</typeConverter></marshaller><applicationDatatype datatype="DECIMAL" precision="1" scale="0"/></field>
  <field name="ITEMS" included="Y" dependsOn="CNT" minOccurs="1" maxOccurs="3"><bytes>6</bytes><applicationDatatype datatype="ARRAY"/>
   <field name="ID" included="Y"><bytes>2</bytes><applicationDatatype datatype="CHAR"/></field>
  </field>
 </field>
</segment></message>"#;

    #[test]
    fn round_trip() {
        let f = parse(DAI).unwrap();
        let v = json!({"OUT": {"LL": 20, "MSG": "Hi", "AMT": -12.5, "NUM": 42,
            "ITEMS": [{"ID": "A"}, {"ID": "BC"}]}});
        let b = encode(&f, &v);
        assert_eq!(b.len(), 2 + 5 + 4 + 3 + 1 + 4);
        assert_eq!(&b[..2], &[0, 20]);
        assert_eq!(&b[7..11], &[0x00, 0x01, 0x25, 0x0D]);
        assert_eq!(&b[11..14], &[0xF0, 0xF4, 0xF2]);
        assert_eq!(b[14], 0xF2);
        let mut back = decode(&f, &b);
        clean(&mut back);
        assert_eq!(
            back,
            json!({"OUT": {"LL": 20, "MSG": "Hi", "AMT": -12.5, "NUM": 42,
                "ITEMS": [{"ID": "A"}, {"ID": "BC"}]}})
        );
    }
}

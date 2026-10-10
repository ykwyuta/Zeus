//! Reading BMS macro source.
//!
//! Assembler statement format: name field from column 1, operation,
//! operands up to the first blank outside quotes (the rest of the line is
//! a remark); a non-blank character in column 72 continues the statement
//! on the next line from column 16. Lines with `*` in column 1 are
//! comments.

use crate::{Field, Map, Mapset};

#[derive(Debug, Clone, PartialEq)]
pub struct Statement {
    pub label: Option<String>,
    pub op: String,
    pub operands: Vec<(String, Value)>,
    pub line: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A keyword with no value (positional operand).
    None,
    Word(String),
    /// A quoted string (quotes removed, '' and && undoubled).
    Str(String),
    List(Vec<String>),
}

impl Value {
    fn list(&self) -> Vec<String> {
        match self {
            Value::None => vec![],
            Value::Word(w) | Value::Str(w) => vec![w.clone()],
            Value::List(l) => l.clone(),
        }
    }

    fn text(&self) -> String {
        match self {
            Value::None => String::new(),
            Value::Word(w) | Value::Str(w) => w.clone(),
            Value::List(l) => l.join(","),
        }
    }
}

/// Splits source text into statements.
pub fn statements(text: &str) -> Result<Vec<Statement>, String> {
    let lines: Vec<Vec<char>> = text
        .lines()
        .map(|l| l.trim_end_matches('\r').chars().collect())
        .collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let l = &lines[i];
        let first = i + 1;
        if l.is_empty() || l[0] == '*' || l.iter().take(71).all(|c| *c == ' ') {
            i += 1;
            continue;
        }
        let body: String = l.iter().take(71).collect();
        let label = if l[0] != ' ' {
            Some(
                body.split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_string(),
            )
        } else {
            None
        };
        let mut rest = body.as_str();
        if label.is_some() {
            rest = rest.trim_start_matches(|c: char| c != ' ');
        }
        let rest = rest.trim_start();
        let op = rest
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();
        let start = rest.len() - rest[op.len()..].trim_start().len();
        let mut operands = String::new();
        let mut in_quote = false;
        let mut seg: Vec<char> = rest[start..].chars().collect();
        loop {
            let mut ended = false;
            for &c in &seg {
                if in_quote {
                    operands.push(c);
                    if c == '\'' {
                        in_quote = false;
                    }
                } else if c == ' ' {
                    ended = true;
                    break;
                } else {
                    if c == '\'' {
                        in_quote = true;
                    }
                    operands.push(c);
                }
            }
            let cont = lines[i].get(71).is_some_and(|c| *c != ' ');
            i += 1;
            if !cont || i >= lines.len() {
                break;
            }
            // operands ended by a blank after a comma go on on the next
            // line; otherwise the continuation lines hold remarks
            if ended && !in_quote && !operands.ends_with(',') {
                while lines
                    .get(i)
                    .is_some_and(|l| l.get(71).is_some_and(|c| *c != ' '))
                {
                    i += 1;
                }
                i += 1;
                break;
            }
            seg = lines[i].iter().skip(15).take(56).copied().collect();
        }
        out.push(Statement {
            label,
            op: op.to_ascii_uppercase(),
            operands: split_operands(&operands).map_err(|e| format!("line {first}: {e}"))?,
            line: first,
        });
    }
    Ok(out)
}

fn unquote(s: &str) -> String {
    s[1..s.len() - 1].replace("''", "'").replace("&&", "&")
}

/// Splits `A=B,C=(D,E),F='x,y',G` at top-level commas.
fn split_operands(s: &str) -> Result<Vec<(String, Value)>, String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut q = false;
    let mut cur = String::new();
    for c in s.chars() {
        match c {
            '\'' => q = !q,
            '(' if !q => depth += 1,
            ')' if !q => depth -= 1,
            ',' if !q && depth == 0 => {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    let mut out = Vec::new();
    for p in parts {
        let (k, v) = match p.split_once('=') {
            Some((k, v)) => (k.to_ascii_uppercase(), v.to_string()),
            None => {
                out.push((p.to_ascii_uppercase(), Value::None));
                continue;
            }
        };
        let value = if v.starts_with('\'') && v.ends_with('\'') && v.len() >= 2 {
            Value::Str(unquote(&v))
        } else if let Some(inner) = v.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
            Value::List(
                inner
                    .split(',')
                    .map(|x| {
                        let x = x.trim();
                        if x.starts_with('\'') && x.ends_with('\'') && x.len() >= 2 {
                            unquote(x)
                        } else {
                            x.to_ascii_uppercase()
                        }
                    })
                    .collect(),
            )
        } else {
            Value::Word(v.to_ascii_uppercase())
        };
        out.push((k, value));
    }
    Ok(out)
}

fn pair(v: &Value) -> Result<(u16, u16), String> {
    let l = v.list();
    let n = |s: &str| {
        s.parse::<u16>()
            .map_err(|_| format!("number expected: {s}"))
    };
    match l.as_slice() {
        [a, b] => Ok((n(a)?, n(b)?)),
        [a] => Ok((0, n(a)?)),
        _ => Err(format!("(line,column) expected: {}", v.text())),
    }
}

/// Extended attributes for EXTATT / DSATTS operands.
fn ext_attrs(ops: &[(String, Value)], inherited: &[String]) -> Vec<String> {
    let mut attrs = inherited.to_vec();
    for (k, v) in ops {
        match k.as_str() {
            "DSATTS" => attrs = v.list(),
            "EXTATT" if v.text() == "YES" && !ops.iter().any(|(k, _)| k == "DSATTS") => {
                attrs = ["COLOR", "PS", "HILIGHT", "VALIDN"]
                    .map(String::from)
                    .to_vec()
            }
            "EXTATT"
                if (v.text() == "NO" || v.text() == "MAPONLY")
                    && !ops.iter().any(|(k, _)| k == "DSATTS") =>
            {
                attrs.clear()
            }
            _ => {}
        }
    }
    attrs
}

/// Reads a mapset from BMS macro source.
pub fn parse(text: &str) -> Result<Mapset, String> {
    let mut ms: Option<Mapset> = None;
    for st in statements(text)? {
        let get = |k: &str| st.operands.iter().find(|(n, _)| n == k).map(|(_, v)| v);
        let err = |m: String| format!("line {}: {m}", st.line);
        match st.op.as_str() {
            "DFHMSD" => {
                if get("TYPE").is_some_and(|t| t.text() == "FINAL") {
                    break;
                }
                let mut m = Mapset {
                    name: st
                        .label
                        .clone()
                        .ok_or_else(|| err("mapset name missing".into()))?,
                    mode: get("MODE").map_or("OUT".into(), Value::text),
                    lang: get("LANG").map_or("ASM".into(), Value::text),
                    storage_auto: get("STORAGE").is_some_and(|v| v.text() == "AUTO"),
                    tioapfx: get("TIOAPFX").is_some_and(|v| v.text() == "YES"),
                    ctrl: get("CTRL").map(Value::list).unwrap_or_default(),
                    ..Mapset::default()
                };
                m.dsatts = ext_attrs(&st.operands, &[]);
                ms = Some(m);
            }
            "DFHMDI" => {
                let m = ms
                    .as_mut()
                    .ok_or_else(|| err("DFHMDI before DFHMSD".into()))?;
                let size = get("SIZE").map(pair).transpose().map_err(&err)?;
                let map = Map {
                    name: st
                        .label
                        .clone()
                        .ok_or_else(|| err("map name missing".into()))?,
                    size: size.unwrap_or((24, 80)),
                    line: get("LINE")
                        .map_or(Ok(1), |v| v.text().parse().map_err(|_| err("LINE".into())))?,
                    column: get("COLUMN").map_or(Ok(1), |v| {
                        v.text().parse().map_err(|_| err("COLUMN".into()))
                    })?,
                    ctrl: get("CTRL")
                        .map(Value::list)
                        .unwrap_or_else(|| m.ctrl.clone()),
                    tioapfx: get("TIOAPFX").map_or(m.tioapfx, |v| v.text() == "YES"),
                    dsatts: ext_attrs(&st.operands, &m.dsatts),
                    cursloc: get("CURSLOC").is_some_and(|v| v.text() == "YES"),
                    fields: Vec::new(),
                };
                m.maps.push(map);
            }
            "DFHMDF" => {
                let m = ms
                    .as_mut()
                    .ok_or_else(|| err("DFHMDF before DFHMSD".into()))?;
                let map = m
                    .maps
                    .last_mut()
                    .ok_or_else(|| err("DFHMDF before DFHMDI".into()))?;
                let pos = get("POS")
                    .map(pair)
                    .transpose()
                    .map_err(&err)?
                    .unwrap_or((0, 0));
                // POS=n: an offset from the start of the map
                let pos = if pos.0 == 0 {
                    let cols = map.size.1.max(1);
                    (pos.1 / cols + 1, pos.1 % cols + 1)
                } else {
                    pos
                };
                let s = |k: &str| get(k).map(Value::text);
                let f = Field {
                    name: st.label.clone(),
                    pos,
                    length: s("LENGTH")
                        .map(|l| l.parse().map_err(|_| err(format!("LENGTH {l}"))))
                        .transpose()?
                        .unwrap_or(0),
                    attrb: get("ATTRB").map(Value::list).unwrap_or_default(),
                    color: s("COLOR"),
                    hilight: s("HILIGHT"),
                    ps: s("PS"),
                    validn: get("VALIDN").map(Value::list).unwrap_or_default(),
                    outline: get("OUTLINE").map(Value::list).unwrap_or_default(),
                    justify: get("JUSTIFY").map(Value::list).unwrap_or_default(),
                    initial: s("INITIAL"),
                    xinit: s("XINIT"),
                    picin: s("PICIN"),
                    picout: s("PICOUT"),
                    occurs: s("OCCURS")
                        .map(|l| l.parse().map_err(|_| err(format!("OCCURS {l}"))))
                        .transpose()?
                        .unwrap_or(1),
                    grpname: s("GRPNAME"),
                };
                map.fields.push(f);
            }
            _ => {}
        }
    }
    ms.ok_or_else(|| "no DFHMSD found".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(label: &str, op: &str, operands: &str, cont: bool) -> String {
        let mut s = format!("{label:<8} {op:<6} {operands}");
        if cont {
            s = format!("{s:<71}-");
        }
        s
    }

    #[test]
    fn statements_and_continuation() {
        let text = [
            "* comment".to_string(),
            card("MS1", "DFHMSD", "TYPE=&&SYSPARM,MODE=INOUT,", true),
            format!("{:15}{}", "", "LANG=COBOL,TIOAPFX=YES,EXTATT=YES"),
            card("MAP1", "DFHMDI", "SIZE=(24,80)", false),
            card("", "DFHMDF", "POS=(1,1),LENGTH=10,ATTRB=(ASKIP,BRT),", true),
            format!("{:15}{}", "", "INITIAL='It''s a, test'  remark"),
            card(
                "FLD1",
                "DFHMDF",
                "POS=(2,5),LENGTH=4,ATTRB=(UNPROT,IC)",
                false,
            ),
            card("", "DFHMSD", "TYPE=FINAL", false),
        ]
        .join("\n");
        let ms = parse(&text).unwrap();
        assert_eq!(ms.name, "MS1");
        assert!(ms.tioapfx);
        assert_eq!(ms.dsatts, vec!["COLOR", "PS", "HILIGHT", "VALIDN"]);
        let m = &ms.maps[0];
        assert_eq!(m.fields.len(), 2);
        assert_eq!(m.fields[0].initial.as_deref(), Some("It's a, test"));
        assert_eq!(m.fields[0].attrb, vec!["ASKIP", "BRT"]);
        assert_eq!(m.fields[1].name.as_deref(), Some("FLD1"));
        assert_eq!(m.fields[1].pos, (2, 5));
    }

    #[test]
    fn literal_continued_across_lines() {
        let first = format!(
            "{:<8} {:<6} {}",
            "", "DFHMDF", "POS=(1,1),LENGTH=60,INITIAL='ABC"
        );
        let text = [
            card("MS1", "DFHMSD", "TYPE=MAP", false),
            card("M", "DFHMDI", "SIZE=(24,80)", false),
            format!("{:<71}X", first),
            format!("{:15}{}", "", "DEF'"),
        ]
        .join("\n");
        let ms = parse(&text).unwrap();
        let init = ms.maps[0].fields[0].initial.clone().unwrap();
        assert!(init.starts_with("ABC") && init.ends_with("DEF"));
        assert_eq!(init.len(), 3 + (71 - first.len()) + 3);
    }
}

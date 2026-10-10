//! COBOL symbolic maps.
//!
//! For each map, an input structure `<map>I` and an output structure
//! `<map>O` redefining it. After the optional 12-byte TIOA prefix, every
//! named field has, in the input structure, its length (`L`, halfword),
//! flag byte (`F`, redefined as the attribute byte `A`), one byte per
//! extended attribute (as FILLER) and the data (`I`); in the output
//! structure three bytes (FILLER), the extended attribute bytes (`C`,
//! `P`, `H`, `V`, `U`, `M`, `T`) and the data (`O`). A field with OCCURS
//! is a table of these entries.

use crate::{EXT_ATTRS, Map, Mapset};

fn line(out: &mut String, level: usize, text: &str) {
    let indent = 7 + 4 + (level.saturating_sub(1)) * 2;
    let l = format!("{:indent$}{text}", "");
    // fixed format: area B ends at column 72
    if l.len() <= 72 {
        out.push_str(&l);
        out.push('\n');
    } else {
        let (a, b) = l.split_at(l[..72].rfind(' ').unwrap_or(72));
        out.push_str(a);
        out.push('\n');
        out.push_str(&format!("{:15}{}\n", "", b.trim_start()));
    }
}

fn pic(p: &Option<String>, len: u16) -> String {
    match p {
        Some(p) => format!("PIC {p}"),
        None => format!("PIC X({len})"),
    }
}

fn suffixes(dsatts: &[String]) -> Vec<char> {
    EXT_ATTRS
        .iter()
        .filter(|(k, _)| dsatts.iter().any(|d| d == k))
        .map(|(_, s)| *s)
        .collect()
}

fn map_structures(out: &mut String, map: &Map, redefines: Option<&str>) {
    let ext = suffixes(&map.dsatts);
    let fields: Vec<_> = map.fields.iter().filter(|f| f.name.is_some()).collect();
    // input
    match redefines {
        Some(r) => line(out, 1, &format!("01  {}I REDEFINES {r}.", map.name)),
        None => line(out, 1, &format!("01  {}I.", map.name)),
    }
    if map.tioapfx {
        line(out, 2, "02  FILLER PIC X(12).");
    }
    for f in &fields {
        let n = f.name.as_deref().unwrap();
        let lvl = if f.occurs > 1 {
            line(out, 2, &format!("02  {n}D OCCURS {} TIMES.", f.occurs));
            3
        } else {
            2
        };
        let l = lvl;
        line(out, l, &format!("{l:02}  {n}L    COMP  PIC  S9(4)."));
        line(out, l, &format!("{l:02}  {n}F    PICTURE X."));
        line(out, l, &format!("{l:02}  FILLER REDEFINES {n}F."));
        line(out, l + 1, &format!("{:02} {n}A    PICTURE X.", l + 1));
        if !ext.is_empty() {
            line(
                out,
                l,
                &format!("{l:02}  FILLER   PICTURE X({}).", ext.len()),
            );
        }
        line(
            out,
            l,
            &format!("{l:02}  {n}I  {}.", pic(&f.picin, f.length)),
        );
    }
    // output
    line(
        out,
        1,
        &format!("01  {}O REDEFINES {}I.", map.name, map.name),
    );
    if map.tioapfx {
        line(out, 2, "02  FILLER PIC X(12).");
    }
    for f in &fields {
        let n = f.name.as_deref().unwrap();
        let lvl = if f.occurs > 1 {
            line(out, 2, &format!("02  FILLER OCCURS {} TIMES.", f.occurs));
            3
        } else {
            2
        };
        let l = lvl;
        line(out, l, &format!("{l:02}  FILLER PICTURE X(3)."));
        for s in &ext {
            line(out, l, &format!("{l:02}  {n}{s}    PICTURE X."));
        }
        line(
            out,
            l,
            &format!("{l:02}  {n}O  {}.", pic(&f.picout, f.length)),
        );
    }
}

/// The COBOL symbolic map copybook of a mapset.
pub fn cobol(ms: &Mapset) -> String {
    let mut out = String::new();
    let first = ms.maps.first().map(|m| format!("{}I", m.name));
    for (i, m) in ms.maps.iter().enumerate() {
        let redefines = if i > 0 && !ms.storage_auto {
            first.as_deref()
        } else {
            None
        };
        map_structures(&mut out, m, redefines);
    }
    out
}

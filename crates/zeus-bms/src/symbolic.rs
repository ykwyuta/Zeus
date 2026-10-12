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

/// The PL/I type of a field's data: its picture when written in PL/I
/// form, else CHAR.
fn pli_type(p: &Option<String>, len: u16) -> String {
    match p {
        // a COBOL repetition factor (9(5)) is not a PL/I picture
        Some(p) if !p.char_indices().any(|(i, c)| c == '(' && i > 0) => format!("PIC'{p}'"),
        _ => format!("CHAR({len})"),
    }
}

fn pli_line(out: &mut String, level: usize, text: &str, last: bool) {
    let indent = 2 + level * 2;
    out.push_str(&format!(
        "{:indent$}{level} {text}{}\n",
        "",
        if last { ";" } else { "," }
    ));
}

/// The input and output structures of a map: `<map>I` and `<map>O`
/// based on its storage.
fn pli_structures(out: &mut String, map: &Map) {
    let ext = suffixes(&map.dsatts);
    let fields: Vec<_> = map.fields.iter().filter(|f| f.name.is_some()).collect();
    let mut filler = 0;
    let mut fill = || {
        filler += 1;
        format!("{}_FILL{filler}", map.name)
    };
    for output in [false, true] {
        let n = &map.name;
        if output {
            out.push_str(&format!(" DCL 1 {n}O BASED(ADDR({n}I)) UNALIGNED,\n"));
        } else {
            out.push_str(&format!(" DCL 1 {n}I UNALIGNED,\n"));
        }
        // a structure needs a member
        if map.tioapfx || fields.is_empty() {
            pli_line(out, 2, &format!("{} CHAR(12)", fill()), fields.is_empty());
        }
        for (k, f) in fields.iter().enumerate() {
            let name = f.name.as_deref().unwrap();
            let last = k + 1 == fields.len();
            let l = if f.occurs > 1 {
                let d = if output { fill() } else { format!("{name}D") };
                pli_line(out, 2, &format!("{d}({})", f.occurs), false);
                3
            } else {
                2
            };
            let mut items = Vec::new();
            if output {
                items.push(format!("{} CHAR(2)", fill()));
                items.push(format!("{name}A CHAR(1)"));
                for s in &ext {
                    items.push(format!("{name}{s} CHAR(1)"));
                }
                items.push(format!("{name}O {}", pli_type(&f.picout, f.length)));
            } else {
                items.push(format!("{name}L FIXED BIN(15)"));
                items.push(format!("{name}F CHAR(1)"));
                if !ext.is_empty() {
                    items.push(format!("{} CHAR({})", fill(), ext.len()));
                }
                items.push(format!("{name}I {}", pli_type(&f.picin, f.length)));
            }
            let n = items.len();
            for (i, it) in items.iter().enumerate() {
                pli_line(out, l, it, last && i + 1 == n);
            }
        }
    }
}

/// The PL/I symbolic map include member of a mapset. Each map has storage
/// of its own; the output structure overlays the input structure, whose
/// attribute byte `A` is the flag byte `F`.
pub fn pli(ms: &Mapset) -> String {
    let mut out = String::new();
    for m in &ms.maps {
        pli_structures(&mut out, m);
    }
    out
}

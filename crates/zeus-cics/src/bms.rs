//! BMS mapping at run time: SEND MAP writes a map and its symbolic output
//! data to the screen, RECEIVE MAP builds the symbolic input structure
//! from the modified fields. The symbolic layout is the one
//! `zeus_bms::symbolic` generates.

use zeus_bms::{EXT_ATTRS, Field, Map};
use zeus_cobol::ebcdic;

use crate::screen::{Attr, Intensity, Screen};

/// Which parts of the map SEND MAP writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendMode {
    Both,
    MapOnly,
    DataOnly,
}

/// A named field's entry in the symbolic map.
struct Entry<'a> {
    field: &'a Field,
    /// Offset of the entry (length halfword) for occurrence 0.
    offset: usize,
    /// Size of one occurrence.
    size: usize,
}

fn ext_suffixes(map: &Map) -> Vec<char> {
    EXT_ATTRS
        .iter()
        .filter(|(k, _)| map.dsatts.iter().any(|d| d == k))
        .map(|(_, s)| *s)
        .collect()
}

fn entries(map: &Map) -> Vec<Entry<'_>> {
    let ext = ext_suffixes(map).len();
    let mut off = if map.tioapfx { 12 } else { 0 };
    let mut out = Vec::new();
    for f in &map.fields {
        if f.name.is_none() {
            continue;
        }
        let size = 3 + ext + f.length as usize;
        out.push(Entry {
            field: f,
            offset: off,
            size,
        });
        off += size * f.occurs.max(1) as usize;
    }
    out
}

/// Size of the symbolic map of `map`.
pub fn symbolic_size(map: &Map) -> usize {
    let e = entries(map);
    match e.last() {
        Some(l) => l.offset + l.size * l.field.occurs.max(1) as usize,
        None => usize::from(map.tioapfx) * 12,
    }
}

fn color(name: &str) -> u8 {
    match name {
        "BLUE" => 0xF1,
        "RED" => 0xF2,
        "PINK" => 0xF3,
        "GREEN" => 0xF4,
        "TURQUOISE" => 0xF5,
        "YELLOW" => 0xF6,
        "NEUTRAL" => 0xF7,
        _ => 0,
    }
}

fn hilight(name: &str) -> u8 {
    match name {
        "BLINK" => 0xF1,
        "REVERSE" => 0xF2,
        "UNDERLINE" => 0xF4,
        _ => 0,
    }
}

/// Attribute a field is defined with.
pub fn field_attr(f: &Field) -> Attr {
    let has = |a: &str| f.attrb.iter().any(|x| x == a);
    let (protected, numeric) = if f.attrb.is_empty() || has("ASKIP") {
        (true, true)
    } else {
        (has("PROT"), has("NUM"))
    };
    Attr {
        protected,
        numeric,
        intensity: if has("BRT") {
            Intensity::Bright
        } else if has("DRK") {
            Intensity::Dark
        } else {
            Intensity::Normal
        },
        mdt: has("FSET"),
        color: f.color.as_deref().map_or(0, color),
        hilight: f.hilight.as_deref().map_or(0, hilight),
    }
}

/// Screen address of the attribute of field `f` (occurrence `k`).
fn field_pos(screen: &Screen, map: &Map, f: &Field, k: usize) -> usize {
    let base = screen.addr(
        map.line as usize + f.pos.0 as usize - 1,
        map.column as usize + f.pos.1 as usize - 1,
    );
    // occurrences follow each other on the screen
    (base + k * (f.length as usize + 1)) % screen.size()
}

fn initial(f: &Field) -> Vec<u8> {
    if let Some(x) = &f.xinit {
        return (0..x.len() / 2)
            .filter_map(|i| u8::from_str_radix(&x[2 * i..2 * i + 2], 16).ok())
            .collect();
    }
    f.initial.as_deref().map(ebcdic::encode).unwrap_or_default()
}

/// SEND MAP. `cursor`: Some(None) for the CURSOR option without a value
/// (symbolic cursor positioning), Some(Some(addr)) for CURSOR(addr).
pub fn send_map(
    screen: &mut Screen,
    map: &Map,
    data: Option<&[u8]>,
    mode: SendMode,
    erase: bool,
    cursor: Option<Option<usize>>,
) {
    if erase {
        screen.erase();
    }
    let ext = ext_suffixes(map);
    let entries = entries(map);
    let mut cursor_at: Option<usize> = cursor.flatten();
    let mut ic: Option<usize> = None;
    for f in &map.fields {
        for k in 0..f.occurs.max(1) as usize {
            let pos = field_pos(screen, map, f, k);
            let start = (pos + 1) % screen.size();
            let mut attr = if mode == SendMode::DataOnly {
                match screen.attrs[pos] {
                    Some(a) => a,
                    None => continue,
                }
            } else {
                field_attr(f)
            };
            if f.attrb.iter().any(|a| a == "IC") && ic.is_none() {
                ic = Some(start);
            }
            if mode != SendMode::DataOnly {
                // the attribute replaces what is at its position; only the
                // initial value (no padding) is written after it
                screen.set_attr(pos, attr);
                let mut init = initial(f);
                init.truncate(f.length as usize);
                screen.write(start, &init);
            }
            // symbolic data
            let entry = entries.iter().find(|e| std::ptr::eq(e.field, f));
            if let (Some(e), Some(d), true) = (entry, data, mode != SendMode::MapOnly) {
                let off = e.offset + k * e.size;
                if off + e.size > d.len() {
                    continue;
                }
                let len = i16::from_be_bytes([d[off], d[off + 1]]);
                if len == -1 && cursor == Some(None) && cursor_at.is_none() {
                    cursor_at = Some(start);
                }
                let a = d[off + 2];
                if a != 0 {
                    attr = attr.with_byte(a);
                }
                for (j, s) in ext.iter().enumerate() {
                    let b = d[off + 3 + j];
                    if b == 0 {
                        continue;
                    }
                    match s {
                        'C' => attr.color = if b == 0xFF { 0 } else { b },
                        'H' => attr.hilight = if b == 0xFF { 0 } else { b },
                        _ => {}
                    }
                }
                screen.set_attr(pos, attr);
                let value = &d[off + 3 + ext.len()..off + e.size];
                if value.first().is_some_and(|&b| b != 0) {
                    let mut v = value.to_vec();
                    v.resize(f.length as usize, 0);
                    screen.write(start, &v);
                }
            } else if mode == SendMode::DataOnly {
                screen.set_attr(pos, attr);
            }
        }
    }
    if let Some(c) = cursor_at.or(ic) {
        screen.cursor = c;
    }
}

/// RECEIVE MAP: the symbolic input structure, or None (MAPFAIL) when no
/// field was received.
pub fn receive_map(screen: &Screen, map: &Map) -> Option<Vec<u8>> {
    let ext = ext_suffixes(map).len();
    let mut area = vec![0u8; symbolic_size(map)];
    let mut any = false;
    for e in entries(map) {
        let f = e.field;
        for k in 0..f.occurs.max(1) as usize {
            let pos = field_pos(screen, map, f, k);
            let Some(attr) = screen.attrs[pos] else {
                continue;
            };
            if !attr.mdt {
                continue;
            }
            any = true;
            let fr = crate::screen::FieldRef {
                attr_pos: pos,
                start: (pos + 1) % screen.size(),
                len: f.length as usize,
            };
            let mut data: Vec<u8> = screen
                .field_data(&fr)
                .into_iter()
                .filter(|&c| c != 0)
                .collect();
            data.truncate(f.length as usize);
            let off = e.offset + k * e.size;
            area[off..off + 2].copy_from_slice(&(data.len() as i16).to_be_bytes());
            if data.is_empty() {
                // erased by the operator
                area[off + 2] = 0x80;
                continue;
            }
            let right = f.justify.iter().any(|j| j == "RIGHT");
            let pad = if f.justify.iter().any(|j| j == "ZERO") {
                ebcdic::ZERO
            } else {
                ebcdic::SPACE
            };
            let mut v = vec![pad; f.length as usize];
            if right {
                let s = v.len() - data.len();
                v[s..].copy_from_slice(&data);
            } else {
                v[..data.len()].copy_from_slice(&data);
            }
            let d = off + 3 + ext;
            area[d..d + v.len()].copy_from_slice(&v);
        }
    }
    any.then_some(area)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapset() -> zeus_bms::Mapset {
        let text = [
            "MS1      DFHMSD TYPE=MAP,MODE=INOUT,LANG=COBOL,TIOAPFX=YES,EXTATT=YES",
            "M1       DFHMDI SIZE=(24,80),LINE=1,COLUMN=1",
            "         DFHMDF POS=(1,1),LENGTH=5,ATTRB=(ASKIP,NORM),INITIAL='USER:'",
            "USERID   DFHMDF POS=(1,8),LENGTH=8,ATTRB=(UNPROT,IC),COLOR=GREEN",
            "         DFHMDF POS=(1,17),LENGTH=0",
            "ERRMSG   DFHMDF POS=(3,1),LENGTH=20,ATTRB=(ASKIP,BRT),COLOR=RED",
        ]
        .join("\n");
        zeus_bms::parse(&text).unwrap()
    }

    #[test]
    fn send_and_receive() {
        let ms = mapset();
        let map = &ms.maps[0];
        // TIOAPFX 12 + USERID (3 + 4 + 8) + ERRMSG (3 + 4 + 20)
        assert_eq!(symbolic_size(map), 12 + 15 + 27);
        let mut out = vec![0u8; symbolic_size(map)];
        let msg = ebcdic::encode("WRONG PASSWORD");
        out[12 + 15 + 7..12 + 15 + 7 + msg.len()].copy_from_slice(&msg);
        let mut s = Screen::new(24, 80);
        send_map(&mut s, map, Some(&out), SendMode::Both, true, None);
        let text = s.render();
        assert!(text.starts_with(" USER:\n\n WRONG PASSWORD\n"), "{text}");
        assert_eq!(s.cursor, s.addr(1, 9));
        assert!(receive_map(&s, map).is_none());
        s.type_at(1, 9, "ADMIN01").unwrap();
        let inp = receive_map(&s, map).unwrap();
        assert_eq!(&inp[12..14], &7i16.to_be_bytes());
        assert_eq!(&inp[19..27], &ebcdic::encode("ADMIN01 ")[..]);
        // fields not received are nulls
        assert!(inp[12 + 15 + 7..].iter().all(|&b| b == 0));
    }
}

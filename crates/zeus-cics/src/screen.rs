//! A 3270 display: a buffer of character positions, some of which hold
//! field attributes. A field runs from the position after its attribute
//! to the next attribute (wrapping at the end of the buffer).

use zeus_cobol::ebcdic;

/// Display intensity of a field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Intensity {
    #[default]
    Normal,
    Bright,
    Dark,
}

/// Field attribute and extended attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Attr {
    pub protected: bool,
    pub numeric: bool,
    pub intensity: Intensity,
    /// Modified data tag.
    pub mdt: bool,
    /// Extended colour (0 = default, X'F1'-X'F7').
    pub color: u8,
    /// Extended highlighting (0 = default, X'F1' blink, X'F2' reverse,
    /// X'F4' underscore).
    pub hilight: u8,
}

/// 3270 attribute byte (the graphic form programs use, e.g. DFHBMPRF) to
/// attribute bits.
pub fn attr_bits(b: u8) -> u8 {
    const TABLE: [u8; 64] = [
        0x40, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E,
        0x4F, 0x50, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0x5A, 0x5B, 0x5C, 0x5D,
        0x5E, 0x5F, 0x60, 0x61, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0x6A, 0x6B, 0x6C,
        0x6D, 0x6E, 0x6F, 0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0x7A, 0x7B,
        0x7C, 0x7D, 0x7E, 0x7F,
    ];
    TABLE
        .iter()
        .position(|&x| x == b)
        .unwrap_or((b & 0x3F) as usize) as u8
}

/// Whether `b` is a 3270 attribute byte in graphic form.
pub fn is_attr_byte(b: u8) -> bool {
    (0..64).any(|bits| attr_byte(bits) == b)
}

/// The graphic attribute byte of attribute bits.
pub fn attr_byte(bits: u8) -> u8 {
    const TABLE: [u8; 64] = [
        0x40, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E,
        0x4F, 0x50, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0x5A, 0x5B, 0x5C, 0x5D,
        0x5E, 0x5F, 0x60, 0x61, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0x6A, 0x6B, 0x6C,
        0x6D, 0x6E, 0x6F, 0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0x7A, 0x7B,
        0x7C, 0x7D, 0x7E, 0x7F,
    ];
    TABLE[(bits & 0x3F) as usize]
}

impl Attr {
    /// From a 3270 attribute byte (graphic form), keeping the extended
    /// attributes.
    pub fn with_byte(self, b: u8) -> Attr {
        let bits = attr_bits(b);
        Attr {
            protected: bits & 0x20 != 0,
            numeric: bits & 0x10 != 0,
            intensity: match bits & 0x0C {
                0x08 => Intensity::Bright,
                0x0C => Intensity::Dark,
                _ => Intensity::Normal,
            },
            mdt: bits & 0x01 != 0,
            ..self
        }
    }

    pub fn byte(&self) -> u8 {
        let mut bits = 0;
        if self.protected {
            bits |= 0x20;
        }
        if self.numeric {
            bits |= 0x10;
        }
        bits |= match self.intensity {
            Intensity::Normal => 0,
            Intensity::Bright => 0x08,
            Intensity::Dark => 0x0C,
        };
        if self.mdt {
            bits |= 0x01;
        }
        attr_byte(bits)
    }

    /// Autoskip: protected and numeric.
    pub fn autoskip(&self) -> bool {
        self.protected && self.numeric
    }
}

#[derive(Debug, Clone)]
pub struct Screen {
    pub rows: usize,
    pub cols: usize,
    /// Characters (EBCDIC); 0 is a null.
    pub chars: Vec<u8>,
    /// Field attributes at their positions.
    pub attrs: Vec<Option<Attr>>,
    pub cursor: usize,
    /// The keyboard is locked (until the program sends FREEKB).
    pub locked: bool,
    pub alarm: bool,
}

/// A field: attribute position, data start and length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldRef {
    pub attr_pos: usize,
    pub start: usize,
    pub len: usize,
}

impl Screen {
    pub fn new(rows: usize, cols: usize) -> Screen {
        Screen {
            rows,
            cols,
            chars: vec![0; rows * cols],
            attrs: vec![None; rows * cols],
            cursor: 0,
            locked: false,
            alarm: false,
        }
    }

    /// Erases the screen and gives it `rows` x `cols` (Erase/Write or
    /// Erase/Write Alternate).
    pub fn erase_to(&mut self, rows: usize, cols: usize) {
        if (rows, cols) != (self.rows, self.cols) {
            let (locked, alarm) = (self.locked, self.alarm);
            *self = Screen::new(rows, cols);
            self.locked = locked;
            self.alarm = alarm;
        } else {
            self.erase();
        }
    }

    /// Erase All Unprotected: nulls in the unprotected fields, their
    /// modified data tags off, the cursor to the first of them.
    pub fn erase_unprotected(&mut self) {
        let n = self.size();
        let mut first = None;
        for f in self.fields() {
            let Some(a) = self.attrs[f.attr_pos].as_mut() else {
                continue;
            };
            if a.protected {
                continue;
            }
            a.mdt = false;
            first.get_or_insert(f.start);
            for k in 0..f.len {
                self.chars[(f.start + k) % n] = 0;
            }
        }
        if let Some(c) = first {
            self.cursor = c;
        }
    }

    /// The buffer as a Read Buffer sends it: characters, SF and the
    /// attribute byte at field positions.
    pub fn read_buffer(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.size() * 2);
        for (c, a) in self.chars.iter().zip(&self.attrs) {
            match a {
                Some(a) => {
                    out.push(0x1D);
                    out.push(a.byte());
                }
                None => out.push(*c),
            }
        }
        out
    }

    pub fn size(&self) -> usize {
        self.rows * self.cols
    }

    /// Buffer address of a 1-based line and column.
    pub fn addr(&self, line: usize, col: usize) -> usize {
        ((line.max(1) - 1) * self.cols + (col.max(1) - 1)) % self.size()
    }

    pub fn erase(&mut self) {
        self.chars.iter_mut().for_each(|c| *c = 0);
        self.attrs.iter_mut().for_each(|a| *a = None);
        self.cursor = 0;
    }

    /// Fields in buffer order.
    pub fn fields(&self) -> Vec<FieldRef> {
        let n = self.size();
        let starts: Vec<usize> = (0..n).filter(|&i| self.attrs[i].is_some()).collect();
        let mut out = Vec::new();
        for (k, &a) in starts.iter().enumerate() {
            let next = starts.get(k + 1).copied().unwrap_or(starts[0] + n);
            out.push(FieldRef {
                attr_pos: a,
                start: (a + 1) % n,
                len: next - a - 1,
            });
        }
        out
    }

    /// The field containing buffer position `pos` (None: unformatted).
    pub fn field_at(&self, pos: usize) -> Option<FieldRef> {
        let n = self.size();
        self.fields().into_iter().find(|f| {
            let off = (pos + n - f.attr_pos) % n;
            off <= f.len
        })
    }

    /// Defines a field attribute at `pos`.
    pub fn set_attr(&mut self, pos: usize, a: Attr) {
        self.attrs[pos] = Some(a);
        self.chars[pos] = 0;
    }

    /// Writes characters from `pos` (wrapping), not over attributes.
    pub fn write(&mut self, pos: usize, data: &[u8]) {
        let n = self.size();
        for (k, &b) in data.iter().enumerate() {
            let p = (pos + k) % n;
            if self.attrs[p].is_some() {
                break;
            }
            self.chars[p] = b;
        }
    }

    /// Data of a field.
    pub fn field_data(&self, f: &FieldRef) -> Vec<u8> {
        let n = self.size();
        (0..f.len).map(|k| self.chars[(f.start + k) % n]).collect()
    }

    /// The operator types `text` at a 1-based position: it goes into the
    /// unprotected field there and sets its modified data tag.
    pub fn type_at(&mut self, line: usize, col: usize, text: &str) -> Result<(), String> {
        let pos = self.addr(line, col);
        let bytes = ebcdic::encode(text);
        match self.field_at(pos) {
            None => {
                // unformatted screen
                self.write(pos, &bytes);
            }
            Some(f) => {
                let a = self.attrs[f.attr_pos].unwrap();
                if a.protected || pos == f.attr_pos {
                    return Err(format!("position ({line},{col}) is protected"));
                }
                let n = self.size();
                let off = (pos + n - f.start) % n;
                if off + bytes.len() > f.len {
                    return Err(format!("text does not fit the field at ({line},{col})"));
                }
                self.write(pos, &bytes);
                self.attrs[f.attr_pos] = Some(Attr { mdt: true, ..a });
            }
        }
        self.cursor = (pos + bytes.len()) % self.size();
        Ok(())
    }

    /// Erases the field at a position from there to its end (ERASE EOF).
    pub fn erase_eof(&mut self, line: usize, col: usize) -> Result<(), String> {
        let pos = self.addr(line, col);
        let f = self.field_at(pos).ok_or("unformatted screen")?;
        let a = self.attrs[f.attr_pos].unwrap();
        if a.protected {
            return Err(format!("position ({line},{col}) is protected"));
        }
        let n = self.size();
        let off = (pos + n - f.start) % n;
        for k in off..f.len {
            self.chars[(f.start + k) % n] = 0;
        }
        self.attrs[f.attr_pos] = Some(Attr { mdt: true, ..a });
        Ok(())
    }

    /// Text of the unformatted input (or of the whole screen): characters
    /// from the start, nulls dropped.
    pub fn unformatted_input(&self) -> Vec<u8> {
        let mut out: Vec<u8> = self.chars.iter().copied().filter(|&c| c != 0).collect();
        while out.last() == Some(&ebcdic::SPACE) {
            out.pop();
        }
        out
    }

    /// Fields whose modified data tag is on, with their data (nulls
    /// dropped, as the terminal sends them).
    pub fn modified_fields(&self) -> Vec<(FieldRef, Vec<u8>)> {
        self.fields()
            .into_iter()
            .filter(|f| self.attrs[f.attr_pos].is_some_and(|a| a.mdt))
            .map(|f| {
                let d = self
                    .field_data(&f)
                    .into_iter()
                    .filter(|&c| c != 0)
                    .collect();
                (f, d)
            })
            .collect()
    }

    /// The screen as text lines: attribute positions and nulls as blanks,
    /// dark fields hidden.
    pub fn render(&self) -> String {
        let mut dark = vec![false; self.size()];
        for f in self.fields() {
            if self.attrs[f.attr_pos].is_some_and(|a| a.intensity == Intensity::Dark) {
                for k in 0..f.len {
                    dark[(f.start + k) % self.size()] = true;
                }
            }
        }
        let mut out = String::new();
        for r in 0..self.rows {
            let mut line = String::new();
            for c in 0..self.cols {
                let p = r * self.cols + c;
                let ch = if self.attrs[p].is_some() || self.chars[p] == 0 || dark[p] {
                    ' '
                } else {
                    let b = ebcdic::decode_latin1(&[self.chars[p]])[0];
                    if b < 0x20 { ' ' } else { b as char }
                };
                line.push(ch);
            }
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_and_input() {
        let mut s = Screen::new(24, 80);
        let prot = Attr::default().with_byte(0xF0); // autoskip
        let unprot = Attr::default().with_byte(0x40);
        s.set_attr(s.addr(1, 1), prot);
        s.write(s.addr(1, 2), &ebcdic::encode("USER:"));
        s.set_attr(s.addr(1, 8), unprot);
        s.set_attr(s.addr(1, 17), prot);
        assert!(s.type_at(1, 2, "X").is_err());
        s.type_at(1, 9, "ADMIN").unwrap();
        let m = s.modified_fields();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].1, ebcdic::encode("ADMIN"));
        assert_eq!(m[0].0.len, 8);
        assert!(s.render().starts_with(" USER:  ADMIN\n"));
        assert_eq!(Attr::default().with_byte(0x61).byte(), 0x61);
    }
}

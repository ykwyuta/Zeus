//! TN3270: 3270 terminals over Telnet (RFC 1576).
//!
//! Telnet negotiation asks for the terminal type, binary transmission and
//! end-of-record marking; after it, each 3270 data stream record ends with
//! IAC EOR. Outbound, the screen is sent as an Erase/Write of all its
//! fields (SF or SFE with colour and highlighting, SBA, IC); inbound, a
//! Read Modified record (AID, cursor address, SBA and data per modified
//! field) is applied to the screen model.

use std::io::{self, Read, Write};

use crate::screen::{Attr, Screen, attr_bits, attr_byte};

pub const IAC: u8 = 255;
pub const DONT: u8 = 254;
pub const DO: u8 = 253;
pub const WONT: u8 = 252;
pub const WILL: u8 = 251;
pub const SB: u8 = 250;
pub const SE: u8 = 240;
pub const EOR: u8 = 239;
pub const OPT_BINARY: u8 = 0;
pub const OPT_TTYPE: u8 = 24;
pub const OPT_EOR: u8 = 25;

// 3270 commands and orders
const CMD_ERASE_WRITE: u8 = 0xF5;
const ORDER_SF: u8 = 0x1D;
const ORDER_SFE: u8 = 0x29;
const ORDER_SBA: u8 = 0x11;
const ORDER_IC: u8 = 0x13;

/// Buffer address in 12-bit form (two graphic 6-bit bytes).
pub fn encode_addr(a: usize) -> [u8; 2] {
    [
        attr_byte(((a >> 6) & 0x3F) as u8),
        attr_byte((a & 0x3F) as u8),
    ]
}

/// Buffer address from 12-bit or 14-bit form.
pub fn decode_addr(b: [u8; 2]) -> usize {
    if b[0] & 0xC0 == 0 {
        (((b[0] & 0x3F) as usize) << 8) | b[1] as usize
    } else {
        ((attr_bits(b[0]) as usize) << 6) | attr_bits(b[1]) as usize
    }
}

/// An Erase/Write of the whole screen.
pub fn write_screen(s: &Screen) -> Vec<u8> {
    let mut out = vec![CMD_ERASE_WRITE];
    // WCC: keyboard restore (and alarm)
    let wcc = 0x02 | if s.alarm { 0x04 } else { 0 };
    out.push(attr_byte(wcc));
    let mut pos = usize::MAX;
    let n = s.size();
    for p in 0..n {
        let a = s.attrs[p];
        let c = s.chars[p];
        if a.is_none() && c == 0 {
            continue;
        }
        if pos != p {
            out.push(ORDER_SBA);
            out.extend(encode_addr(p));
        }
        match a {
            Some(a) => field_order(&mut out, &a),
            None => out.push(c),
        }
        pos = p + 1;
    }
    out.push(ORDER_SBA);
    out.extend(encode_addr(s.cursor % n));
    out.push(ORDER_IC);
    out
}

fn field_order(out: &mut Vec<u8>, a: &Attr) {
    if a.color == 0 && a.hilight == 0 {
        out.push(ORDER_SF);
        out.push(a.byte());
        return;
    }
    let mut pairs = vec![(0xC0, a.byte())];
    if a.hilight != 0 {
        pairs.push((0x41, a.hilight));
    }
    if a.color != 0 {
        pairs.push((0x42, a.color));
    }
    out.push(ORDER_SFE);
    out.push(pairs.len() as u8);
    for (t, v) in pairs {
        out.push(t);
        out.push(v);
    }
}

/// Applies an outbound Write / Erase/Write record to a screen (what a
/// terminal does with it).
pub fn apply_write(s: &mut Screen, rec: &[u8]) {
    let Some(&cmd) = rec.first() else { return };
    if matches!(cmd, 0xF5 | 0x7E | 0x05 | 0x0D) {
        s.erase();
    }
    let n = s.size();
    let mut pos = 0usize;
    let mut i = 2;
    while i < rec.len() {
        match rec[i] {
            ORDER_SBA if i + 2 < rec.len() => {
                pos = decode_addr([rec[i + 1], rec[i + 2]]) % n;
                i += 3;
            }
            ORDER_SF if i + 1 < rec.len() => {
                s.set_attr(pos, Attr::default().with_byte(rec[i + 1]));
                pos = (pos + 1) % n;
                i += 2;
            }
            ORDER_SFE if i + 1 < rec.len() => {
                let k = rec[i + 1] as usize;
                let mut a = Attr::default();
                for j in 0..k {
                    let (t, v) = (rec[i + 2 + 2 * j], rec[i + 3 + 2 * j]);
                    match t {
                        0xC0 => {
                            a = Attr {
                                color: a.color,
                                hilight: a.hilight,
                                ..a.with_byte(v)
                            }
                        }
                        0x41 => a.hilight = v,
                        0x42 => a.color = v,
                        _ => {}
                    }
                }
                s.set_attr(pos, a);
                pos = (pos + 1) % n;
                i += 2 + 2 * k;
            }
            ORDER_IC => {
                s.cursor = pos;
                i += 1;
            }
            b => {
                s.chars[pos] = b;
                s.attrs[pos] = None;
                pos = (pos + 1) % n;
                i += 1;
            }
        }
    }
}

/// Applies an inbound record to the screen; returns the AID.
pub fn apply_input(s: &mut Screen, rec: &[u8]) -> Option<u8> {
    let aid = *rec.first()?;
    if rec.len() >= 3 {
        s.cursor = decode_addr([rec[1], rec[2]]) % s.size();
    }
    let mut i = 3;
    let mut cur: Option<usize> = None;
    let mut data: Vec<u8> = Vec::new();
    let flush = |s: &mut Screen, at: Option<usize>, data: &mut Vec<u8>| {
        let Some(at) = at else {
            data.clear();
            return;
        };
        match s.field_at(at) {
            Some(f) => {
                // the field's new content: the data, then nulls
                let n = s.size();
                for k in 0..f.len {
                    let b = data.get(k).copied().unwrap_or(0);
                    s.chars[(f.start + k) % n] = b;
                }
                if let Some(a) = s.attrs[f.attr_pos].as_mut() {
                    a.mdt = true;
                }
            }
            None => s.write(at, data),
        }
        data.clear();
    };
    if s.fields().is_empty() && i < rec.len() && rec[i] != ORDER_SBA {
        // unformatted screen: the data goes from the start
        cur = Some(0);
        s.erase();
    }
    while i < rec.len() {
        if rec[i] == ORDER_SBA && i + 2 < rec.len() {
            flush(s, cur, &mut data);
            cur = Some(decode_addr([rec[i + 1], rec[i + 2]]) % s.size());
            i += 3;
            continue;
        }
        data.push(rec[i]);
        i += 1;
    }
    flush(s, cur, &mut data);
    Some(aid)
}

/// Writes a record, doubling IAC bytes, followed by IAC EOR.
pub fn send_record(w: &mut impl Write, rec: &[u8]) -> io::Result<()> {
    let mut out = Vec::with_capacity(rec.len() + 2);
    for &b in rec {
        out.push(b);
        if b == IAC {
            out.push(IAC);
        }
    }
    out.extend([IAC, EOR]);
    w.write_all(&out)?;
    w.flush()
}

/// Reads Telnet input: 3270 records (ended by IAC EOR), option
/// negotiation and the terminal type; it answers nothing itself.
pub struct Reader<R: Read> {
    inner: R,
    buf: Vec<u8>,
}

impl<R: Read> Reader<R> {
    pub fn new(inner: R) -> Reader<R> {
        Reader {
            inner,
            buf: Vec::new(),
        }
    }

    fn byte(&mut self) -> io::Result<u8> {
        let mut b = [0u8; 1];
        self.inner.read_exact(&mut b)?;
        Ok(b[0])
    }

    /// The next 3270 record, option negotiation or terminal type.
    pub fn next_event(&mut self) -> io::Result<Event> {
        loop {
            let b = self.byte()?;
            if b != IAC {
                self.buf.push(b);
                continue;
            }
            let c = self.byte()?;
            match c {
                IAC => self.buf.push(IAC),
                EOR => return Ok(Event::Record(std::mem::take(&mut self.buf))),
                DO | DONT | WILL | WONT => {
                    let opt = self.byte()?;
                    return Ok(Event::Option(c, opt));
                }
                SB => {
                    let mut sub = Vec::new();
                    loop {
                        let x = self.byte()?;
                        if x == IAC {
                            let y = self.byte()?;
                            if y == SE {
                                break;
                            }
                            sub.push(y);
                        } else {
                            sub.push(x);
                        }
                    }
                    if sub.first() == Some(&OPT_TTYPE) && sub.get(1) == Some(&0) {
                        return Ok(Event::TerminalType(
                            String::from_utf8_lossy(&sub[2..]).to_string(),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Record(Vec<u8>),
    Option(u8, u8),
    TerminalType(String),
}

/// Server side of the negotiation: terminal type, then binary and EOR.
pub fn negotiate<S: Read + Write>(stream: &mut S) -> io::Result<String> {
    stream.write_all(&[IAC, DO, OPT_TTYPE])?;
    stream.flush()?;
    let mut r = Reader::new(&mut *stream);
    // WILL TERMINAL-TYPE, then the type
    loop {
        match r.next_event()? {
            Event::Option(WILL, OPT_TTYPE) => break,
            Event::Option(WONT, OPT_TTYPE) => {
                return Err(io::Error::other("the client is not a 3270 terminal"));
            }
            _ => {}
        }
    }
    drop(r);
    stream.write_all(&[IAC, SB, OPT_TTYPE, 1, IAC, SE])?;
    stream.flush()?;
    let mut r = Reader::new(&mut *stream);
    let ttype = loop {
        if let Event::TerminalType(t) = r.next_event()? {
            break t;
        }
    };
    drop(r);
    stream.write_all(&[
        IAC, DO, OPT_EOR, IAC, WILL, OPT_EOR, IAC, DO, OPT_BINARY, IAC, WILL, OPT_BINARY,
    ])?;
    stream.flush()?;
    Ok(ttype)
}

#[cfg(test)]
mod tests {
    use super::*;
    use zeus_cobol::ebcdic;

    #[test]
    fn addresses() {
        for a in [0, 1, 79, 80, 1919] {
            assert_eq!(decode_addr(encode_addr(a)), a);
        }
        assert_eq!(encode_addr(80), [0xC1, 0x50]);
    }

    #[test]
    fn write_and_read_modified() {
        let mut s = Screen::new(24, 80);
        s.set_attr(0, Attr::default().with_byte(0xF0));
        s.write(1, &ebcdic::encode("USER:"));
        s.set_attr(
            7,
            Attr {
                color: 0xF4,
                ..Attr::default()
            },
        );
        s.set_attr(16, Attr::default().with_byte(0xF0));
        s.cursor = 8;
        let w = write_screen(&s);
        assert_eq!(&w[..2], &[0xF5, 0xC2]);
        assert!(w.windows(2).any(|p| p == [ORDER_SF, 0xF0]));
        assert!(w.windows(4).any(|p| p == [ORDER_SFE, 2, 0xC0, 0x40]));
        // the operator enters ADMIN in the field at 8 and presses ENTER
        let mut rec = vec![0x7D];
        rec.extend(encode_addr(13));
        rec.push(ORDER_SBA);
        rec.extend(encode_addr(8));
        rec.extend(ebcdic::encode("ADMIN"));
        assert_eq!(apply_input(&mut s, &rec), Some(0x7D));
        let m = s.modified_fields();
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].1, ebcdic::encode("ADMIN"));
        // a terminal shows what was written
        let mut t = Screen::new(24, 80);
        apply_write(&mut t, &write_screen(&s));
        assert_eq!(t.render(), s.render());
        assert_eq!(t.cursor, s.cursor);
        assert_eq!(t.attrs[7].unwrap().color, 0xF4);
    }
}

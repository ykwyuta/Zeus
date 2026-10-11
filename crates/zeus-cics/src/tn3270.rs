//! TN3270 (RFC 1576) and TN3270E (RFC 2355): 3270 terminals over Telnet.
//!
//! The server offers TN3270E first: the client names its device type and
//! may ask for an LU name, then each record carries a 5-byte header. A
//! client that refuses gets plain TN3270: terminal type, binary
//! transmission and end-of-record marking. Either way records end with
//! IAC EOR.
//!
//! Outbound, the screen is sent as an Erase/Write of all its fields (SF,
//! or SFE with colour and highlighting for terminals with extended
//! attributes), SBA, RA for runs of a character and IC. Terminals with
//! extended attributes are asked for their capabilities with a Read
//! Partition Query; the Query Reply tells colour and highlighting support
//! and the alternate screen size. Inbound, a Read Modified record (AID,
//! cursor address, SBA and data per modified field) is applied to the
//! screen model.

use std::io::{self, Read, Write};

use crate::screen::{Attr, CharAttr, Screen, attr_bits, attr_byte};

pub const IAC: u8 = 255;
pub const DONT: u8 = 254;
pub const DO: u8 = 253;
pub const WONT: u8 = 252;
pub const WILL: u8 = 251;
pub const SB: u8 = 250;
pub const BRK: u8 = 243;
pub const IP: u8 = 244;
pub const AO: u8 = 245;
pub const SE: u8 = 240;
pub const EOR: u8 = 239;
pub const OPT_BINARY: u8 = 0;
pub const OPT_TIMING_MARK: u8 = 6;
pub const OPT_TTYPE: u8 = 24;
pub const OPT_EOR: u8 = 25;
pub const OPT_TN3270E: u8 = 40;

// TN3270E subnegotiation (RFC 2355)
const E_ASSOCIATE: u8 = 0;
const E_CONNECT: u8 = 1;
const E_DEVICE_TYPE: u8 = 2;
const E_FUNCTIONS: u8 = 3;
const E_IS: u8 = 4;
const E_REASON: u8 = 5;
const E_REJECT: u8 = 6;
const E_REQUEST: u8 = 7;
const E_SEND: u8 = 8;
/// FUNCTIONS.
pub const FN_BIND_IMAGE: u8 = 0;
pub const FN_DATA_STREAM_CTL: u8 = 1;
pub const FN_RESPONSES: u8 = 2;
pub const FN_SCS_CTL_CODES: u8 = 3;
pub const FN_SYSREQ: u8 = 4;
/// REASON codes of a DEVICE-TYPE REJECT.
const E_DEVICE_IN_USE: u8 = 1;
const E_INV_ASSOCIATE: u8 = 2;
const E_INV_NAME: u8 = 3;
const E_INV_DEVICE_TYPE: u8 = 4;
/// Header data types.
const E_3270_DATA: u8 = 0;
const E_SCS_DATA: u8 = 1;
const E_BIND_IMAGE: u8 = 3;
const E_UNBIND: u8 = 4;
const E_SSCP_LU_DATA: u8 = 7;
const E_PRINT_EOJ: u8 = 8;

// 3270 commands, orders and AIDs
const CMD_ERASE_WRITE: u8 = 0xF5;
const CMD_ERASE_WRITE_ALTERNATE: u8 = 0x7E;
const CMD_WSF: u8 = 0xF3;
const ORDER_PT: u8 = 0x05;
const ORDER_GE: u8 = 0x08;
const ORDER_SBA: u8 = 0x11;
const ORDER_EUA: u8 = 0x12;
const ORDER_IC: u8 = 0x13;
const ORDER_SF: u8 = 0x1D;
const ORDER_SA: u8 = 0x28;
const ORDER_SFE: u8 = 0x29;
const ORDER_MF: u8 = 0x2C;
const ORDER_RA: u8 = 0x3C;
const DUP: u8 = 0x1C;
/// Shift out / in (double-byte characters between them).
const SO: u8 = 0x0E;
const SI: u8 = 0x0F;
/// SCS new line, form feed; end of message (3270 printer buffers).
const SCS_NL: u8 = 0x15;
const SCS_FF: u8 = 0x0C;
const EM: u8 = 0x19;
const FM: u8 = 0x1E;
pub const AID_STRUCTURED_FIELD: u8 = 0x88;
const AID_CLEAR: u8 = 0x6D;

/// Buffer address in 12-bit form (two graphic 6-bit bytes), or 14-bit
/// form beyond 4095.
pub fn encode_addr(a: usize) -> [u8; 2] {
    if a > 0xFFF {
        return [((a >> 8) & 0x3F) as u8, (a & 0xFF) as u8];
    }
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

/// What a terminal can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// SFE / extended field attributes (terminal types ending in -E, 3279).
    pub extended: bool,
    pub color: bool,
    pub hilight: bool,
    /// Alternate screen size (rows, columns) from the model or the query.
    pub alternate: (usize, usize),
    /// Double-byte characters (DBCS-Asia in the Query Reply).
    pub dbcs: bool,
}

impl Capabilities {
    /// The terminal model of the region for these capabilities.
    pub fn model(&self) -> crate::region::Model {
        crate::region::Model {
            alternate: self.alternate,
            extended: self.extended,
            color: self.color,
            hilight: self.hilight,
            dbcs: self.dbcs,
        }
    }

    /// From a terminal type such as IBM-3278-2-E or IBM-3279-5-E.
    pub fn of_type(ttype: &str) -> Capabilities {
        let t = ttype.to_ascii_uppercase();
        let extended = t.ends_with("-E") || t.contains("3279") || t.contains("DYNAMIC");
        let alternate = match t.split('-').nth(2) {
            Some("3") => (32, 80),
            Some("4") => (43, 80),
            Some("5") => (27, 132),
            _ => (24, 80),
        };
        Capabilities {
            extended,
            color: extended && t.contains("3279"),
            hilight: extended,
            alternate,
            dbcs: false,
        }
    }

    /// Updates the capabilities from a Query Reply (an inbound structured
    /// field record).
    pub fn apply_query_reply(&mut self, rec: &[u8]) {
        let mut i = 1;
        while i + 4 <= rec.len() {
            let len = u16::from_be_bytes([rec[i], rec[i + 1]]) as usize;
            if len < 4 || i + len > rec.len() {
                break;
            }
            let sf = &rec[i..i + len];
            if sf[2] == 0x81 {
                match sf[3] {
                    // Color: pairs after the flags and count
                    0x86 => self.color = sf.get(5).is_some_and(|&n| n > 2),
                    0x87 => self.hilight = true,
                    // DBCS-Asia
                    0x91 => self.dbcs = true,
                    // Implicit Partition: default and alternate sizes
                    0xA6 if sf.len() >= 17 => {
                        let w = u16::from_be_bytes([sf[13], sf[14]]) as usize;
                        let h = u16::from_be_bytes([sf[15], sf[16]]) as usize;
                        if w > 0 && h > 0 {
                            self.alternate = (h, w);
                        }
                    }
                    _ => {}
                }
                self.extended = true;
            }
            i += len;
        }
    }
}

/// The Read Partition Query structured field.
pub fn query() -> Vec<u8> {
    vec![CMD_WSF, 0x00, 0x05, 0x01, 0xFF, 0x02]
}

/// A byte of character data that the terminal shows as a character:
/// codes below X'40' are orders or controls, sent as blanks.
fn graphic(c: u8) -> u8 {
    if c < 0x40 && !matches!(c, DUP | FM | SO | SI) {
        0x40
    } else {
        c
    }
}

/// An Erase/Write of the whole screen (Erase/Write Alternate for a screen
/// in the alternate size).
pub fn write_screen(s: &Screen, caps: &Capabilities) -> Vec<u8> {
    let cmd = if (s.rows, s.cols) == crate::region::DEFAULT_SIZE {
        CMD_ERASE_WRITE
    } else {
        CMD_ERASE_WRITE_ALTERNATE
    };
    let mut out = vec![cmd];
    // WCC: keyboard restore (and alarm)
    let wcc = 0x02 | if s.alarm { 0x04 } else { 0 };
    out.push(attr_byte(wcc));
    let mut pos = usize::MAX;
    let n = s.size();
    let mut p = 0;
    let mut dbcs = false;
    // the character attributes in force (Set Attribute)
    let mut cur = CharAttr::default();
    while p < n {
        let a = s.attrs[p];
        let c = s.chars[p];
        if a.is_none() && c == 0 {
            p += 1;
            continue;
        }
        if pos != p {
            out.push(ORDER_SBA);
            out.extend(encode_addr(p));
        }
        if a.is_none() && caps.extended {
            let want = CharAttr {
                color: if caps.color { s.cattrs[p].color } else { 0 },
                hilight: if caps.hilight { s.cattrs[p].hilight } else { 0 },
                charset: s.cattrs[p].charset,
            };
            for (t, have, v) in [
                (0x41, cur.hilight, want.hilight),
                (0x42, cur.color, want.color),
                (0x43, cur.charset, want.charset),
            ] {
                if have != v {
                    out.extend([ORDER_SA, t, v]);
                }
            }
            cur = want;
        }
        if let Some(a) = a {
            field_order(&mut out, &a, caps);
            dbcs = false;
            p += 1;
        } else if c == SO || c == SI || dbcs {
            // double-byte characters go as they are
            dbcs = (dbcs || c == SO) && c != SI;
            out.push(c);
            p += 1;
        } else {
            // a run of the same character: Repeat to Address
            let mut e = p + 1;
            while e < n
                && s.attrs[e].is_none()
                && s.chars[e] == c
                && s.cattrs[e] == s.cattrs[p]
                && c != SO
            {
                e += 1;
            }
            if e - p > 4 {
                out.push(ORDER_RA);
                out.extend(encode_addr(e % n));
                out.push(graphic(c));
                p = e;
            } else {
                out.push(graphic(c));
                p += 1;
            }
        }
        pos = p;
    }
    out.push(ORDER_SBA);
    out.extend(encode_addr(s.cursor % n));
    out.push(ORDER_IC);
    out
}

fn field_order(out: &mut Vec<u8>, a: &Attr, caps: &Capabilities) {
    let color = if caps.color { a.color } else { 0 };
    let hilight = if caps.hilight { a.hilight } else { 0 };
    let ext = color != 0
        || hilight != 0
        || a.validation != 0
        || a.outline != 0
        || a.sosi
        || a.charset != 0;
    if !caps.extended || !ext {
        out.push(ORDER_SF);
        out.push(a.byte());
        return;
    }
    let mut pairs = vec![(0xC0, a.byte())];
    if hilight != 0 {
        pairs.push((0x41, hilight));
    }
    if color != 0 {
        pairs.push((0x42, color));
    }
    if a.charset != 0 {
        pairs.push((0x43, a.charset));
    }
    if a.validation != 0 {
        pairs.push((0xC1, a.validation));
    }
    if a.outline != 0 {
        pairs.push((0xC2, a.outline));
    }
    // input control: SO / SI creation
    if a.sosi {
        pairs.push((0xFE, 0x01));
    }
    out.push(ORDER_SFE);
    out.push(pairs.len() as u8);
    for (t, v) in pairs {
        out.push(t);
        out.push(v);
    }
}

/// Applies an outbound Write / Erase/Write record to a screen (what a
/// terminal does with it): the WCC, then the orders and characters (a
/// Write starts at the cursor address, an Erase/Write at 0).
pub fn apply_write(s: &mut Screen, rec: &[u8]) {
    let Some(&cmd) = rec.first() else { return };
    let erase = matches!(cmd, 0xF5 | 0x7E | 0x05 | 0x0D);
    if erase {
        s.erase();
    }
    let n = s.size();
    if let Some(&wcc) = rec.get(1) {
        let bits = attr_bits(wcc);
        if bits & 0x01 != 0 {
            // reset the modified data tags
            for a in s.attrs.iter_mut().flatten() {
                a.mdt = false;
            }
        }
        if bits & 0x02 != 0 {
            s.locked = false;
        }
        if bits & 0x04 != 0 {
            s.alarm = true;
        }
    }
    let mut pos = if erase { 0 } else { s.cursor % n };
    let mut cur = CharAttr::default();
    let mut i = 2;
    let addr = |i: usize| decode_addr([rec[i], rec[i + 1]]) % n;
    let put = |s: &mut Screen, pos: &mut usize, c: u8, cur: CharAttr| {
        s.chars[*pos] = c;
        s.attrs[*pos] = None;
        s.cattrs[*pos] = cur;
        *pos = (*pos + 1) % n;
    };
    while i < rec.len() {
        match rec[i] {
            ORDER_SBA if i + 2 < rec.len() => {
                pos = addr(i + 1);
                i += 3;
            }
            ORDER_SF if i + 1 < rec.len() => {
                s.set_attr(pos, Attr::default().with_byte(rec[i + 1]));
                pos = (pos + 1) % n;
                i += 2;
            }
            ORDER_SFE | ORDER_MF if i + 1 < rec.len() => {
                let k = rec[i + 1] as usize;
                let mut a = if rec[i] == ORDER_MF {
                    s.attrs[pos].unwrap_or_default()
                } else {
                    Attr::default()
                };
                for j in 0..k {
                    let (Some(&t), Some(&v)) = (rec.get(i + 2 + 2 * j), rec.get(i + 3 + 2 * j))
                    else {
                        break;
                    };
                    match t {
                        0xC0 => a = a.with_byte(v),
                        0x41 => a.hilight = v,
                        0x42 => a.color = v,
                        0x43 => a.charset = v,
                        0xC1 => a.validation = v,
                        0xC2 => a.outline = v,
                        0xFE => a.sosi = v == 0x01,
                        _ => {}
                    }
                }
                s.set_attr(pos, a);
                pos = (pos + 1) % n;
                i += 2 + 2 * k;
            }
            ORDER_SA if i + 2 < rec.len() => {
                let v = rec[i + 2];
                match rec[i + 1] {
                    0x00 => cur = CharAttr::default(),
                    0x41 => cur.hilight = v,
                    0x42 => cur.color = v,
                    0x43 => cur.charset = v,
                    _ => {}
                }
                i += 3;
            }
            ORDER_IC => {
                s.cursor = pos;
                i += 1;
            }
            ORDER_RA if i + 3 < rec.len() => {
                let to = addr(i + 1);
                let mut c = rec[i + 3];
                let mut skip = 4;
                if c == ORDER_GE && i + 4 < rec.len() {
                    c = rec[i + 4];
                    skip = 5;
                }
                loop {
                    put(s, &mut pos, c, cur);
                    if pos == to {
                        break;
                    }
                }
                i += skip;
            }
            ORDER_EUA if i + 2 < rec.len() => {
                let to = addr(i + 1);
                loop {
                    let unprotected = s
                        .field_at(pos)
                        .is_some_and(|f| s.attrs[f.attr_pos].is_some_and(|a| !a.protected));
                    if s.attrs[pos].is_none() && unprotected {
                        s.chars[pos] = 0;
                    }
                    pos = (pos + 1) % n;
                    if pos == to {
                        break;
                    }
                }
                i += 3;
            }
            ORDER_PT => {
                // to the start of the next unprotected field
                if let Some(f) = s
                    .fields()
                    .into_iter()
                    .filter(|f| s.attrs[f.attr_pos].is_some_and(|a| !a.protected))
                    .find(|f| f.start > pos)
                {
                    pos = f.start;
                }
                i += 1;
            }
            ORDER_GE if i + 1 < rec.len() => {
                put(s, &mut pos, rec[i + 1], cur);
                i += 2;
            }
            c => {
                put(s, &mut pos, c, cur);
                i += 1;
            }
        }
    }
}

/// Applies an inbound record to the screen; returns the AID.
pub fn apply_input(s: &mut Screen, rec: &[u8]) -> Option<u8> {
    let aid = *rec.first()?;
    if aid == AID_CLEAR {
        // the terminal clears its screen and goes back to the default
        // size (a short read: the AID alone)
        let (r, c) = crate::region::DEFAULT_SIZE;
        s.erase_to(r, c);
        return Some(aid);
    }
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

/// Whether records are traced on standard error (ZEUS_TN3270_TRACE).
pub fn tracing() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("ZEUS_TN3270_TRACE").is_some())
}

pub fn trace(dir: &str, rec: &[u8]) {
    if tracing() {
        let hex: String = rec.iter().take(64).map(|b| format!("{b:02X}")).collect();
        eprintln!("tn3270 {dir} {} bytes: {hex}", rec.len());
    }
}

/// Writes a record, doubling IAC bytes, followed by IAC EOR.
pub fn send_record(w: &mut impl Write, rec: &[u8]) -> io::Result<()> {
    trace(">", rec);
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

/// A negotiated connection.
#[derive(Debug, Clone)]
pub struct Session {
    pub ttype: String,
    /// TN3270E: records carry a header.
    pub tn3270e: bool,
    /// The LU name of a TN3270E connection.
    pub device: Option<String>,
    /// The TN3270E functions agreed.
    pub functions: Vec<u8>,
    pub caps: Capabilities,
    /// The server's own Read Partition Query awaits its reply.
    pub query_pending: bool,
    /// The alternate screen size of the last BIND image sent.
    pub bound: Option<(usize, usize)>,
    /// SYSREQ was pressed: the terminal talks to the SSCP (SSCP-LU data).
    pub sscp: bool,
    seq: u16,
}

impl Session {
    pub fn new(ttype: &str, tn3270e: bool, device: Option<String>) -> Session {
        Session {
            ttype: ttype.to_string(),
            tn3270e,
            device,
            functions: Vec::new(),
            caps: Capabilities::of_type(ttype),
            query_pending: false,
            bound: None,
            sscp: false,
            seq: 0,
        }
    }

    /// A printer session (IBM-3287).
    pub fn is_printer(&self) -> bool {
        self.ttype.to_ascii_uppercase().starts_with("IBM-3287")
    }

    pub fn has(&self, f: u8) -> bool {
        self.functions.contains(&f)
    }

    fn send_typed(&mut self, w: &mut impl Write, data_type: u8, rec: &[u8]) -> io::Result<()> {
        let mut r = Vec::with_capacity(rec.len() + 5);
        r.extend([data_type, 0, 0]);
        r.extend(self.seq.to_be_bytes());
        self.seq = self.seq.wrapping_add(1);
        r.extend_from_slice(rec);
        send_record(w, &r)
    }

    /// The BIND image of the session (when BIND-IMAGE was agreed): sent
    /// before the first data.
    pub fn bind(&mut self, w: &mut impl Write) -> io::Result<()> {
        if !self.has(FN_BIND_IMAGE) {
            return Ok(());
        }
        let lu = if !self.is_printer() {
            2
        } else if self.has(FN_SCS_CTL_CODES) {
            1
        } else {
            3
        };
        let img = bind_image(lu, crate::region::DEFAULT_SIZE, self.caps.alternate);
        self.bound = Some(self.caps.alternate);
        self.send_typed(w, E_BIND_IMAGE, &img)
    }

    /// After a Query Reply: a new BIND image when the terminal's alternate
    /// size differs from the one bound (oversize screens).
    pub fn rebind(&mut self, w: &mut impl Write) -> io::Result<bool> {
        if !self.has(FN_BIND_IMAGE) || self.bound == Some(self.caps.alternate) {
            return Ok(false);
        }
        self.unbind(w)?;
        self.bind(w)?;
        Ok(true)
    }

    /// UNBIND (when BIND-IMAGE was agreed), before the session ends.
    pub fn unbind(&mut self, w: &mut impl Write) -> io::Result<()> {
        if !self.has(FN_BIND_IMAGE) {
            return Ok(());
        }
        // UNBIND, type normal
        self.send_typed(w, E_UNBIND, &[0x32, 0x01])
    }

    /// Data for the SSCP (after SYSREQ): unformatted text.
    pub fn send_sscp(&mut self, w: &mut impl Write, text: &[u8]) -> io::Result<()> {
        self.send_typed(w, E_SSCP_LU_DATA, text)
    }

    /// Prints pages on a printer session: SCS data (lines ended by NL,
    /// pages by FF) when SCS-CTL-CODES was agreed, else a 3270 write
    /// that starts the printer; then the end of the print job.
    pub fn print(&mut self, w: &mut impl Write, pages: &[Vec<Vec<u8>>]) -> io::Result<()> {
        if pages.is_empty() {
            return Ok(());
        }
        if !self.tn3270e || !self.has(FN_SCS_CTL_CODES) {
            for page in pages {
                let mut rec = vec![CMD_ERASE_WRITE, attr_byte(0x08 | 0x02)];
                for (i, l) in page.iter().enumerate() {
                    if i > 0 {
                        rec.push(SCS_NL);
                    }
                    rec.extend(l.iter().map(|&c| graphic(c)));
                }
                rec.push(SCS_NL);
                rec.push(EM);
                self.send(w, &rec)?;
            }
            return Ok(());
        }
        let mut scs = Vec::new();
        for (k, page) in pages.iter().enumerate() {
            if k > 0 {
                scs.push(SCS_FF);
            }
            for l in page {
                scs.extend(l.iter().map(|&c| graphic(c)));
                scs.push(SCS_NL);
            }
        }
        scs.push(SCS_FF);
        self.send_typed(w, E_SCS_DATA, &scs)?;
        self.send_typed(w, E_PRINT_EOJ, &[])
    }

    /// Sends a 3270 data record (with its TN3270E header).
    pub fn send(&mut self, w: &mut impl Write, rec: &[u8]) -> io::Result<()> {
        if !self.tn3270e {
            return send_record(w, rec);
        }
        let mut r = Vec::with_capacity(rec.len() + 5);
        r.extend([E_3270_DATA, 0, 0]);
        r.extend(self.seq.to_be_bytes());
        self.seq = self.seq.wrapping_add(1);
        r.extend_from_slice(rec);
        send_record(w, &r)
    }

    /// The data of an inbound record: 3270 data, or what the terminal
    /// sends the SSCP; None for other TN3270E data types (responses, ...).
    pub fn inbound(&self, rec: Vec<u8>) -> Option<Inbound> {
        if !self.tn3270e {
            return Some(Inbound::Data(rec));
        }
        if rec.len() < 5 {
            return None;
        }
        match rec[0] {
            E_3270_DATA => Some(Inbound::Data(rec[5..].to_vec())),
            E_SSCP_LU_DATA => Some(Inbound::Sscp(rec[5..].to_vec())),
            _ => None,
        }
    }
}

/// Reads Telnet input: 3270 records (ended by IAC EOR), option
/// negotiation, subnegotiations and commands; it answers nothing itself.
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

    /// The next 3270 record, option negotiation, subnegotiation or
    /// command.
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
                EOR => {
                    trace("<", &self.buf);
                    return Ok(Event::Record(std::mem::take(&mut self.buf)));
                }
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
                    return Ok(Event::Sub(sub));
                }
                IP | BRK => return Ok(Event::Attention),
                AO => return Ok(Event::SysReq),
                _ => {}
            }
        }
    }
}

/// Inbound data of a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Inbound {
    /// A 3270 data stream record (AID first).
    Data(Vec<u8>),
    /// Data for the SSCP (the terminal is in SYSREQ mode).
    Sscp(Vec<u8>),
}

/// A BIND request unit for an LU of type `lu` (2: display, 1: SCS
/// printer, 3: 3270 printer) with the default and alternate screen sizes
/// in its presentation space (SNA BIND format).
pub fn bind_image(lu: u8, default: (usize, usize), alternate: (usize, usize)) -> Vec<u8> {
    let mut b = vec![
        0x31, // BIND
        0x01, // format 0, cold
        0x03, // FM profile 3
        0x03, // TS profile 3
        0xB1, // primary LU protocols
        0xB0, // secondary LU protocols
        0x30, 0x80, // common protocols
        0x00, 0x00, // secondary send and receive pacing
        0x87, 0x87, // maximum RU sizes (1024)
        0x00, 0x00, // primary send and receive pacing
        lu,   // PS profile: LU type
        0x00, 0x00, 0x00, 0x00, 0x00, // PS characteristics
    ];
    if lu == 2 || lu == 3 {
        b.extend([
            default.0 as u8,
            default.1 as u8,
            alternate.0 as u8,
            alternate.1 as u8,
            0x7F, // default and alternate sizes as given
        ]);
    } else {
        b.extend([0, 0, 0, 0, 0]);
    }
    b.extend([0x00, 0x00, 0x00]); // cryptography, primary LU name length
    b
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    Record(Vec<u8>),
    Option(u8, u8),
    TerminalType(String),
    /// Any other subnegotiation (option code first).
    Sub(Vec<u8>),
    /// IP or BREAK: the ATTN key.
    Attention,
    /// AO: the SYSREQ key (TN3270E SYSREQ function).
    SysReq,
}

/// The answer to an option request after negotiation: TIMING-MARK is
/// acknowledged, other new options are refused (None: no answer, for the
/// options in use and refusals).
pub fn answer_option(cmd: u8, opt: u8) -> Option<[u8; 3]> {
    match (cmd, opt) {
        (DO, OPT_TIMING_MARK) => Some([IAC, WILL, OPT_TIMING_MARK]),
        (_, OPT_BINARY | OPT_EOR | OPT_TTYPE | OPT_TN3270E) => None,
        (DO, o) => Some([IAC, WONT, o]),
        (WILL, o) => Some([IAC, DONT, o]),
        _ => None,
    }
}

/// What a TN3270E client asks for its device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRequest<'a> {
    /// Any name (the server's).
    Any,
    /// CONNECT: this LU name.
    Connect(&'a str),
    /// ASSOCIATE: the printer of this display LU.
    Associate(&'a str),
}

/// Why a device request is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    InUse,
    InvalidName,
    InvalidAssociate,
}

/// Server side of the negotiation. TN3270E when the client agrees (the
/// device name comes from `name`, given the device type and the request),
/// plain TN3270 otherwise.
pub fn negotiate<S: Read + Write>(
    stream: &mut S,
    name: impl Fn(&str, DeviceRequest) -> Result<String, Refusal>,
) -> io::Result<Session> {
    stream.write_all(&[IAC, DO, OPT_TN3270E])?;
    stream.flush()?;
    let mut r = Reader::new(&mut *stream);
    let e = loop {
        match r.next_event()? {
            Event::Option(WILL, OPT_TN3270E) => break true,
            Event::Option(WONT, OPT_TN3270E) => break false,
            _ => {}
        }
    };
    drop(r);
    if e {
        return negotiate_e(stream, name);
    }
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
    // a terminal type of the form IBM-xxxx@LUNAME names the LU
    let (ttype, lu) = match ttype.split_once('@') {
        Some((t, l)) => (t.to_string(), Some(l.to_string())),
        None => (ttype, None),
    };
    let device = name(
        &ttype,
        lu.as_deref()
            .map_or(DeviceRequest::Any, DeviceRequest::Connect),
    )
    .ok();
    Ok(Session::new(&ttype, false, device))
}

fn sub(stream: &mut impl Write, body: &[u8]) -> io::Result<()> {
    let mut out = vec![IAC, SB, OPT_TN3270E];
    for &b in body {
        out.push(b);
        if b == IAC {
            out.push(IAC);
        }
    }
    out.extend([IAC, SE]);
    stream.write_all(&out)?;
    stream.flush()
}

fn negotiate_e<S: Read + Write>(
    stream: &mut S,
    name: impl Fn(&str, DeviceRequest) -> Result<String, Refusal>,
) -> io::Result<Session> {
    sub(stream, &[E_SEND, E_DEVICE_TYPE])?;
    // DEVICE-TYPE REQUEST <type> [CONNECT <name> | ASSOCIATE <name>]
    let (ttype, device) = loop {
        let mut r = Reader::new(&mut *stream);
        let ev = r.next_event()?;
        drop(r);
        let Event::Sub(s) = ev else { continue };
        if s.len() < 3 || s[0] != OPT_TN3270E || s[1] != E_DEVICE_TYPE || s[2] != E_REQUEST {
            continue;
        }
        let rest = &s[3..];
        let cut = rest
            .iter()
            .position(|&b| b == E_CONNECT || b == E_ASSOCIATE)
            .unwrap_or(rest.len());
        let ttype = String::from_utf8_lossy(&rest[..cut]).to_string();
        let arg = rest
            .get(cut + 1..)
            .map(|n| String::from_utf8_lossy(n).to_string())
            .unwrap_or_default();
        let printer = ttype.to_ascii_uppercase().starts_with("IBM-3287");
        let up = ttype.to_ascii_uppercase();
        if !(up.starts_with("IBM-327") || printer || up == "IBM-DYNAMIC") {
            sub(
                stream,
                &[E_DEVICE_TYPE, E_REJECT, E_REASON, E_INV_DEVICE_TYPE],
            )?;
            continue;
        }
        let req = match rest.get(cut) {
            Some(&E_CONNECT) => DeviceRequest::Connect(&arg),
            // only printers are associated with a display
            Some(_) if !printer => {
                sub(
                    stream,
                    &[E_DEVICE_TYPE, E_REJECT, E_REASON, E_INV_ASSOCIATE],
                )?;
                continue;
            }
            Some(_) => DeviceRequest::Associate(&arg),
            None => DeviceRequest::Any,
        };
        match name(&ttype, req) {
            Ok(n) => break (ttype, n),
            Err(why) => {
                let reason = match why {
                    Refusal::InUse => E_DEVICE_IN_USE,
                    Refusal::InvalidName => E_INV_NAME,
                    Refusal::InvalidAssociate => E_INV_ASSOCIATE,
                };
                sub(stream, &[E_DEVICE_TYPE, E_REJECT, E_REASON, reason])?;
            }
        }
    };
    let mut is = vec![E_DEVICE_TYPE, E_IS];
    is.extend(ttype.as_bytes());
    is.push(E_CONNECT);
    is.extend(device.as_bytes());
    sub(stream, &is)?;
    // FUNCTIONS REQUEST <list>: agree on those supported for the device
    let printer = ttype.to_ascii_uppercase().starts_with("IBM-3287");
    let supported: &[u8] = if printer {
        &[
            FN_BIND_IMAGE,
            FN_DATA_STREAM_CTL,
            FN_RESPONSES,
            FN_SCS_CTL_CODES,
        ]
    } else {
        &[FN_BIND_IMAGE, FN_RESPONSES, FN_SYSREQ]
    };
    let functions = loop {
        let mut r = Reader::new(&mut *stream);
        let ev = r.next_event()?;
        drop(r);
        let Event::Sub(s) = ev else { continue };
        if s.len() < 3 || s[0] != OPT_TN3270E || s[1] != E_FUNCTIONS {
            continue;
        }
        let asked = &s[3..];
        let ours: Vec<u8> = asked
            .iter()
            .copied()
            .filter(|f| supported.contains(f))
            .collect();
        match s[2] {
            E_REQUEST if ours.len() == asked.len() => {
                let mut b = vec![E_FUNCTIONS, E_IS];
                b.extend(&ours);
                sub(stream, &b)?;
                break ours;
            }
            E_REQUEST => {
                let mut b = vec![E_FUNCTIONS, E_REQUEST];
                b.extend(&ours);
                sub(stream, &b)?;
            }
            E_IS => break asked.to_vec(),
            _ => {}
        }
    };
    let mut session = Session::new(&ttype, true, Some(device));
    session.functions = functions;
    session.bind(stream)?;
    Ok(session)
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
        let caps = Capabilities::of_type("IBM-3279-2-E");
        let w = write_screen(&s, &caps);
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
        apply_write(&mut t, &write_screen(&s, &caps));
        assert_eq!(t.render(), s.render());
        assert_eq!(t.cursor, s.cursor);
        assert_eq!(t.attrs[7].unwrap().color, 0xF4);
    }

    /// A client: its bytes are read by the server, the server's replies
    /// collected.
    struct Duplex {
        input: io::Cursor<Vec<u8>>,
        output: Vec<u8>,
    }

    impl Read for Duplex {
        fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
            self.input.read(b)
        }
    }

    impl Write for Duplex {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            self.output.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn sb(body: &[u8]) -> Vec<u8> {
        let mut v = vec![IAC, SB, OPT_TN3270E];
        v.extend_from_slice(body);
        v.extend([IAC, SE]);
        v
    }

    #[test]
    fn tn3270e_negotiation() {
        let mut client = vec![IAC, WILL, OPT_TN3270E];
        let mut req = vec![E_DEVICE_TYPE, E_REQUEST];
        req.extend(b"IBM-3278-2-E");
        req.push(E_CONNECT);
        req.extend(b"TAKEN");
        client.extend(sb(&req));
        let mut req = vec![E_DEVICE_TYPE, E_REQUEST];
        req.extend(b"IBM-3279-2-E");
        client.extend(sb(&req));
        client.extend(sb(&[
            E_FUNCTIONS,
            E_REQUEST,
            FN_BIND_IMAGE,
            FN_RESPONSES,
            9,
        ]));
        client.extend(sb(&[E_FUNCTIONS, E_IS, FN_BIND_IMAGE, FN_RESPONSES]));
        let mut d = Duplex {
            input: io::Cursor::new(client),
            output: Vec::new(),
        };
        let mut session = negotiate(&mut d, |_, req| match req {
            DeviceRequest::Connect("TAKEN") => Err(Refusal::InUse),
            _ => Ok("ZEUS0001".into()),
        })
        .unwrap();
        assert!(session.tn3270e);
        assert_eq!(session.ttype, "IBM-3279-2-E");
        assert_eq!(session.device.as_deref(), Some("ZEUS0001"));
        assert_eq!(session.functions, [FN_BIND_IMAGE, FN_RESPONSES]);
        let out = &d.output;
        let has = |v: &[u8]| out.windows(v.len()).any(|w| w == v);
        // the name in use is refused, the second request accepted
        assert!(has(&sb(&[
            E_DEVICE_TYPE,
            E_REJECT,
            E_REASON,
            E_DEVICE_IN_USE
        ])));
        let mut is = vec![E_DEVICE_TYPE, E_IS];
        is.extend(b"IBM-3279-2-E");
        is.push(E_CONNECT);
        is.extend(b"ZEUS0001");
        assert!(has(&sb(&is)));
        // the unsupported function is left out
        assert!(has(&sb(&[
            E_FUNCTIONS,
            E_REQUEST,
            FN_BIND_IMAGE,
            FN_RESPONSES
        ])));
        // the BIND image follows, with the screen sizes of an LU 2
        let mut bind = vec![E_BIND_IMAGE, 0, 0, 0, 0];
        bind.extend(bind_image(2, (24, 80), (24, 80)));
        assert!(has(&bind));
        assert_eq!(
            &bind_image(2, (24, 80), (27, 132))[20..25],
            &[24, 80, 27, 132, 0x7F]
        );
        // records carry the header
        let mut w = Vec::new();
        session.send(&mut w, &[0xF5, 0xC2]).unwrap();
        session.send(&mut w, &[0xF5, 0xC2]).unwrap();
        assert_eq!(&w[..9], &[0, 0, 0, 0, 1, 0xF5, 0xC2, IAC, EOR]);
        assert_eq!(&w[9..14], &[0, 0, 0, 0, 2]);
        assert_eq!(
            session.inbound(vec![0, 0, 0, 0, 7, 0x7D]),
            Some(Inbound::Data(vec![0x7D]))
        );
        assert_eq!(
            session.inbound(vec![E_SSCP_LU_DATA, 0, 0, 0, 8, 0xD3]),
            Some(Inbound::Sscp(vec![0xD3]))
        );
        assert_eq!(session.inbound(vec![2, 0, 0, 0, 7, 0]), None);
        // SYSREQ mode: SSCP-LU data; LOGOFF: UNBIND
        let mut w = Vec::new();
        session.send_sscp(&mut w, &[0xC1]).unwrap();
        session.unbind(&mut w).unwrap();
        assert_eq!(w[0], E_SSCP_LU_DATA);
        assert!(w.windows(3).any(|x| x == [E_UNBIND, 0, 0]));
    }

    #[test]
    fn printer_session() {
        // a printer associated with display LU ZEUS0001, SCS data
        let mut client = vec![IAC, WILL, OPT_TN3270E];
        let mut req = vec![E_DEVICE_TYPE, E_REQUEST];
        req.extend(b"IBM-3287-1");
        req.push(E_ASSOCIATE);
        req.extend(b"ZEUS0001");
        client.extend(sb(&req));
        client.extend(sb(&[
            E_FUNCTIONS,
            E_REQUEST,
            FN_SCS_CTL_CODES,
            FN_DATA_STREAM_CTL,
        ]));
        let mut d = Duplex {
            input: io::Cursor::new(client),
            output: Vec::new(),
        };
        let mut session = negotiate(&mut d, |t, req| {
            assert_eq!(t, "IBM-3287-1");
            match req {
                DeviceRequest::Associate("ZEUS0001") => Ok("ZEUSPRT1".into()),
                _ => Err(Refusal::InvalidAssociate),
            }
        })
        .unwrap();
        assert!(session.is_printer());
        assert_eq!(session.device.as_deref(), Some("ZEUSPRT1"));
        let mut w = Vec::new();
        let pages = vec![vec![b"\xC1\xC2".to_vec(), vec![]], vec![vec![0xC3]]];
        session.print(&mut w, &pages).unwrap();
        // SCS: lines with NL, pages with FF, then the end of the job
        assert_eq!(
            &w[..14],
            &[
                E_SCS_DATA, 0, 0, 0, 0, 0xC1, 0xC2, SCS_NL, SCS_NL, SCS_FF, 0xC3, SCS_NL, SCS_FF,
                IAC
            ]
        );
        assert!(w.windows(2).any(|x| x == [IAC, EOR]));
        assert!(w.windows(3).any(|x| x == [E_PRINT_EOJ, 0, 0]));
    }

    #[test]
    fn plain_tn3270_negotiation() {
        let mut client = vec![IAC, WONT, OPT_TN3270E, IAC, WILL, OPT_TTYPE];
        client.extend([IAC, SB, OPT_TTYPE, 0]);
        client.extend(b"IBM-3278-2");
        client.extend([IAC, SE]);
        let mut d = Duplex {
            input: io::Cursor::new(client),
            output: Vec::new(),
        };
        let session = negotiate(&mut d, |_, _| Ok("ZEUS0001".into())).unwrap();
        assert!(!session.tn3270e);
        assert!(!session.caps.extended);
        assert!(d.output.ends_with(&[IAC, WILL, OPT_BINARY]));
    }

    #[test]
    fn data_stream_out() {
        let mut s = Screen::new(24, 80);
        s.set_attr(
            0,
            Attr {
                color: 0xF2,
                ..Attr::default()
            },
        );
        s.write(1, &[0xC1; 30]);
        // a control code in the data is not sent as an order
        s.write(40, &[0x11, 0xC2]);
        // basic terminal: SF only
        let plain = write_screen(&s, &Capabilities::of_type("IBM-3278-2"));
        assert!(!plain.contains(&ORDER_SFE));
        assert!(!plain[2..].windows(2).any(|w| w == [0x11, 0xC2]));
        // a run of characters is a Repeat to Address
        let mut ra = vec![ORDER_RA];
        ra.extend(encode_addr(31));
        ra.push(0xC1);
        assert!(plain.windows(4).any(|w| w == ra.as_slice()));
        let mut t = Screen::new(24, 80);
        apply_write(&mut t, &plain);
        assert_eq!(&t.chars[1..31], &[0xC1; 30]);
        assert_eq!(t.chars[40], 0x40);
        // colour terminal: SFE
        let ext = write_screen(&s, &Capabilities::of_type("IBM-3279-2-E"));
        assert!(ext.windows(2).any(|w| w == [ORDER_SFE, 2]));
    }

    #[test]
    fn character_attributes() {
        // a Write with SA: the characters keep their attributes, and the
        // screen goes out with SA again (no RA across a change)
        let mut s = Screen::new(24, 80);
        let mut rec = vec![0xF5, 0xC3, ORDER_SA, 0x42, 0xF2];
        rec.extend([0xC1; 6]);
        rec.extend([ORDER_SA, 0x42, 0x00, 0xC1, 0xC1]);
        apply_write(&mut s, &rec);
        assert_eq!(s.cattrs[0].color, 0xF2);
        assert_eq!(s.cattrs[5].color, 0xF2);
        assert_eq!(s.cattrs[6].color, 0);
        let caps = Capabilities::of_type("IBM-3279-2-E");
        let out = write_screen(&s, &caps);
        let mut t = Screen::new(24, 80);
        apply_write(&mut t, &out);
        assert_eq!(t.chars[..8], s.chars[..8]);
        assert_eq!(t.cattrs[..8], s.cattrs[..8]);
        assert!(out.windows(3).any(|w| w == [ORDER_SA, 0x42, 0xF2]));
        // a terminal without extended attributes gets no SA
        let plain = write_screen(&s, &Capabilities::of_type("IBM-3278-2"));
        assert!(!plain.contains(&ORDER_SA));
    }

    #[test]
    fn query_reply_and_clear() {
        let mut caps = Capabilities::of_type("IBM-3278-2-E");
        assert!(!caps.color);
        // Color (8 pairs) and Implicit Partition (alternate 27 x 132)
        let mut rec = vec![AID_STRUCTURED_FIELD];
        rec.extend([0, 8, 0x81, 0x86, 0, 8, 0, 0xF4]);
        rec.extend([
            0, 17, 0x81, 0xA6, 0, 0, 0x0B, 1, 0, 0, 80, 0, 24, 0, 132, 0, 27,
        ]);
        caps.apply_query_reply(&rec);
        assert!(caps.color);
        assert_eq!(caps.alternate, (27, 132));
        let mut s = Screen::new(24, 80);
        s.set_attr(0, Attr::default());
        assert_eq!(apply_input(&mut s, &[AID_CLEAR]), Some(AID_CLEAR));
        assert!(s.fields().is_empty());
        assert_eq!(answer_option(DO, OPT_TIMING_MARK), Some([IAC, WILL, 6]));
        assert_eq!(answer_option(DO, 31), Some([IAC, WONT, 31]));
        assert_eq!(answer_option(DO, OPT_EOR), None);
    }
}

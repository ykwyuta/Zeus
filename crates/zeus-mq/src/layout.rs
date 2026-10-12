//! MQI structures in the two forms: as programs on z/OS see them
//! (big-endian integers, EBCDIC characters, 4-byte pointers) and as the C
//! client library on this host takes them (native integers, ASCII or
//! UTF-8 characters, 8-byte pointers aligned on 8 bytes). The native
//! offsets follow from the z/OS ones: pointers, MQCHARV and 64-bit
//! integers are aligned and pointers take 8 bytes.

use zeus_cobol::ebcdic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// MQLONG.
    Int,
    /// Character data (names, formats).
    Char,
    /// Byte strings (message and correlation identifiers, tokens).
    Bytes,
    /// MQINT64 (message handles).
    Int64,
    /// A pointer (4 bytes on z/OS): null in the native form.
    Ptr,
    /// MQCHARV (20 bytes on z/OS): a variable-length string the native
    /// form points to in a buffer of its own (UTF-8).
    CharV,
}

/// A structure: its identifier, its length for each version and its
/// fields (z/OS offset, z/OS length, kind).
pub struct Layout {
    pub id: &'static [u8; 4],
    pub lengths: &'static [usize],
    pub fields: &'static [(usize, usize, Kind)],
}

use Kind::*;

/// The CCSID of strings in native structures (UTF-8).
pub const NATIVE_CCSID: i32 = 1208;

/// MQOD versions 1 to 4.
pub const OD: Layout = Layout {
    id: b"OD  ",
    lengths: &[168, 200, 336, 400],
    fields: &[
        (0, 4, Char),
        (4, 4, Int),
        (8, 4, Int),
        (12, 48, Char),
        (60, 48, Char),
        (108, 48, Char),
        (156, 12, Char),
        // version 2
        (168, 4, Int),
        (172, 4, Int),
        (176, 4, Int),
        (180, 4, Int),
        (184, 4, Int),
        (188, 4, Int),
        (192, 4, Ptr),
        (196, 4, Ptr),
        // version 3
        (200, 40, Bytes),
        (240, 48, Char),
        (288, 48, Char),
        // version 4
        (336, 20, CharV),
        (356, 20, CharV),
        (376, 20, CharV),
        (396, 4, Int),
    ],
};

/// MQMD versions 1 and 2.
pub const MD: Layout = Layout {
    id: b"MD  ",
    lengths: &[324, 364],
    fields: &[
        (0, 4, Char),
        (4, 4, Int),
        (8, 4, Int),
        (12, 4, Int),
        (16, 4, Int),
        (20, 4, Int),
        (24, 4, Int),
        (28, 4, Int),
        (32, 8, Char),
        (40, 4, Int),
        (44, 4, Int),
        (48, 24, Bytes),
        (72, 24, Bytes),
        (96, 4, Int),
        (100, 48, Char),
        (148, 48, Char),
        (196, 12, Char),
        (208, 32, Bytes),
        (240, 32, Char),
        (272, 4, Int),
        (276, 28, Char),
        (304, 8, Char),
        (312, 8, Char),
        (320, 4, Char),
        // version 2
        (324, 24, Bytes),
        (348, 4, Int),
        (352, 4, Int),
        (356, 4, Int),
        (360, 4, Int),
    ],
};

/// MQPMO versions 1 to 3.
pub const PMO: Layout = Layout {
    id: b"PMO ",
    lengths: &[128, 152, 176],
    fields: &[
        (0, 4, Char),
        (4, 4, Int),
        (8, 4, Int),
        (12, 4, Int),
        (16, 4, Int),
        (20, 4, Int),
        (24, 4, Int),
        (28, 4, Int),
        (32, 48, Char),
        (80, 48, Char),
        // version 2
        (128, 4, Int),
        (132, 4, Int),
        (136, 4, Int),
        (140, 4, Int),
        (144, 4, Ptr),
        (148, 4, Ptr),
        // version 3
        (152, 8, Int64),
        (160, 8, Int64),
        (168, 4, Int),
        (172, 4, Int),
    ],
};

/// MQGMO versions 1 to 4.
pub const GMO: Layout = Layout {
    id: b"GMO ",
    lengths: &[72, 80, 100, 112],
    fields: &[
        (0, 4, Char),
        (4, 4, Int),
        (8, 4, Int),
        (12, 4, Int),
        (16, 4, Int),
        (20, 4, Int),
        (24, 48, Char),
        // version 2
        (72, 4, Int),
        (76, 1, Char),
        (77, 1, Char),
        (78, 1, Char),
        (79, 1, Char),
        // version 3
        (80, 16, Bytes),
        (96, 4, Int),
        // version 4
        (100, 4, Int),
        (104, 8, Int64),
    ],
};

/// MQSD (subscription descriptor) version 1.
pub const SD: Layout = Layout {
    id: b"SD  ",
    lengths: &[312],
    fields: &[
        (0, 4, Char),
        (4, 4, Int),
        (8, 4, Int),
        (12, 48, Char),
        (60, 12, Char),
        (72, 40, Bytes),
        (112, 4, Int),
        (116, 20, CharV),
        (136, 20, CharV),
        (156, 20, CharV),
        (176, 24, Bytes),
        (200, 4, Int),
        (204, 32, Bytes),
        (236, 32, Char),
        (268, 20, CharV),
        (288, 4, Int),
        (292, 20, CharV),
    ],
};

/// The options structures of three MQLONGs: MQCMHO, MQDMHO, MQDMPO.
const OPTS3: &[(usize, usize, Kind)] = &[(0, 4, Char), (4, 4, Int), (8, 4, Int)];

pub const CMHO: Layout = Layout {
    id: b"CMHO",
    lengths: &[12],
    fields: OPTS3,
};

pub const DMHO: Layout = Layout {
    id: b"DMHO",
    lengths: &[12],
    fields: OPTS3,
};

pub const DMPO: Layout = Layout {
    id: b"DMPO",
    lengths: &[12],
    fields: OPTS3,
};

/// MQSRO (subscription request options).
pub const SRO: Layout = Layout {
    id: b"SRO ",
    lengths: &[16],
    fields: &[(0, 4, Char), (4, 4, Int), (8, 4, Int), (12, 4, Int)],
};

/// MQSMPO (set message property options).
pub const SMPO: Layout = Layout {
    id: b"SMPO",
    lengths: &[20],
    fields: &[
        (0, 4, Char),
        (4, 4, Int),
        (8, 4, Int),
        (12, 4, Int),
        (16, 4, Int),
    ],
};

/// MQPD (property descriptor).
pub const PD: Layout = Layout {
    id: b"PD  ",
    lengths: &[24],
    fields: &[
        (0, 4, Char),
        (4, 4, Int),
        (8, 4, Int),
        (12, 4, Int),
        (16, 4, Int),
        (20, 4, Int),
    ],
};

/// MQIMPO (inquire message property options).
pub const IMPO: Layout = Layout {
    id: b"IMPO",
    lengths: &[60],
    fields: &[
        (0, 4, Char),
        (4, 4, Int),
        (8, 4, Int),
        (12, 4, Int),
        (16, 4, Int),
        (20, 4, Int),
        (24, 4, Int),
        (28, 4, Int),
        (32, 20, CharV),
        (52, 8, Char),
    ],
};

/// MQCHARV alone (a property name argument).
pub const CHARV: Layout = Layout {
    id: b"    ",
    lengths: &[20],
    fields: &[(0, 20, CharV)],
};

/// Offset of the Version field (all structures).
pub const VERSION: usize = 4;

/// A structure in native form, with the buffers its MQCHARVs point to.
pub struct Native {
    pub bytes: Vec<u8>,
    bufs: Vec<Vec<u8>>,
}

fn aligned8(k: Kind) -> bool {
    matches!(k, Ptr | CharV | Int64)
}

fn native_len_of(k: Kind, len: usize) -> usize {
    match k {
        Ptr => 8,
        CharV => 24,
        _ => len,
    }
}

impl Layout {
    /// The version of a z/OS structure, limited to the versions known.
    pub fn version(&self, zos: &[u8]) -> usize {
        let v = be(zos, VERSION).clamp(1, self.lengths.len() as i32);
        v as usize
    }

    /// The z/OS length of `version`.
    pub fn len(&self, version: usize) -> usize {
        self.lengths[version - 1]
    }

    /// The native offsets of the fields.
    fn native_offsets(&self) -> Vec<usize> {
        let mut out = Vec::with_capacity(self.fields.len());
        let (mut zend, mut nend) = (0usize, 0usize);
        for &(off, len, kind) in self.fields {
            let mut n = nend + off.saturating_sub(zend);
            if aligned8(kind) {
                n = n.div_ceil(8) * 8;
            }
            out.push(n);
            zend = off + len;
            nend = n + native_len_of(kind, len);
        }
        out
    }

    /// The native length of `version`.
    pub fn native_len(&self, version: usize) -> usize {
        let zlen = self.len(version);
        let offs = self.native_offsets();
        let mut end = 0;
        let mut wide = false;
        for (&(off, len, kind), &n) in self.fields.iter().zip(&offs) {
            if off + len <= zlen {
                end = end.max(n + native_len_of(kind, len));
                wide |= aligned8(kind);
            }
        }
        if wide { end.div_ceil(8) * 8 } else { end }
    }

    /// Whether a z/OS structure has this structure's identifier.
    pub fn is(&self, zos: &[u8]) -> bool {
        zos.len() >= 4
            && zos[..4]
                .iter()
                .map(|&b| ebcdic::to_latin1(b))
                .eq(self.id.iter().copied())
    }

    /// The native form of the structure of `version` at `base` in `mem`;
    /// the version is set to `version`.
    pub fn to_native(&self, mem: &[u8], base: usize, version: usize) -> Native {
        let zlen = self.len(version);
        let zos = &mem[base..base + zlen];
        let mut out = vec![0u8; self.native_len(version)];
        let mut bufs = Vec::new();
        for (&(off, len, kind), n) in self.fields.iter().zip(self.native_offsets()) {
            if off + len > zlen {
                continue;
            }
            let src = &zos[off..off + len];
            match kind {
                Int => out[n..n + 4].copy_from_slice(&be(src, 0).to_ne_bytes()),
                Int64 => {
                    let v = i64::from_be_bytes(src[..8].try_into().unwrap());
                    out[n..n + 8].copy_from_slice(&v.to_ne_bytes());
                }
                Char => {
                    for (d, s) in out[n..n + len].iter_mut().zip(src) {
                        *d = ebcdic::to_latin1(*s);
                    }
                }
                Bytes => out[n..n + len].copy_from_slice(src),
                Ptr => {}
                // an MQCHARV without a string or a buffer: none
                CharV if charv_addr(mem, base, src).is_none() && be(src, 8) <= 0 => {
                    out[n + 20..n + 24].copy_from_slice(&NATIVE_CCSID.to_ne_bytes());
                    bufs.push(Vec::new());
                }
                CharV => {
                    let text = charv_text(mem, base, src);
                    let utf8 = ebcdic::decode(&text).into_bytes();
                    let cap = (be(src, 8).max(0) as usize * 3).max(utf8.len()).max(1);
                    let mut buf = utf8.clone();
                    buf.resize(cap, 0);
                    let v = &mut out[n..n + 24];
                    v[..8].copy_from_slice(&(buf.as_ptr() as u64).to_ne_bytes());
                    v[12..16].copy_from_slice(&(cap as i32).to_ne_bytes());
                    v[16..20].copy_from_slice(&(utf8.len() as i32).to_ne_bytes());
                    v[20..24].copy_from_slice(&NATIVE_CCSID.to_ne_bytes());
                    bufs.push(buf);
                }
            }
        }
        // (an MQCHARV alone has no version)
        if self.id != b"    " {
            out[VERSION..VERSION + 4].copy_from_slice(&(version as i32).to_ne_bytes());
        }
        Native { bytes: out, bufs }
    }

    /// Copies a native structure back into the z/OS one at `base` (its
    /// version stays; MQCHARV strings go to the program's buffers).
    pub fn to_zos(&self, native: &Native, mem: &mut [u8], base: usize, version: usize) {
        let zlen = self.len(version);
        let mut k = 0;
        for (&(off, len, kind), n) in self.fields.iter().zip(self.native_offsets()) {
            if off + len > zlen {
                continue;
            }
            if off == VERSION {
                continue;
            }
            let src = &native.bytes[n..];
            let at = base + off;
            match kind {
                Int => mem[at..at + 4].copy_from_slice(&ne(src, 0).to_be_bytes()),
                Int64 => {
                    let v = i64::from_ne_bytes(src[..8].try_into().unwrap());
                    mem[at..at + 8].copy_from_slice(&v.to_be_bytes());
                }
                Char => {
                    for (d, s) in mem[at..at + len].iter_mut().zip(&src[..len]) {
                        *d = ebcdic::from_latin1(*s);
                    }
                }
                Bytes => mem[at..at + len].copy_from_slice(&src[..len]),
                Ptr => {}
                CharV => {
                    let buf = &native.bufs[k];
                    k += 1;
                    let zbufsize = be(mem, at + 8);
                    if zbufsize <= 0 {
                        continue;
                    }
                    let vlen = ne(src, 16).max(0) as usize;
                    let text = String::from_utf8_lossy(&buf[..vlen.min(buf.len())]).into_owned();
                    let e = ebcdic::encode(&text);
                    if let Some(dest) = charv_addr(mem, base, &mem[at..at + 20]) {
                        let n = e
                            .len()
                            .min(zbufsize as usize)
                            .min(mem.len().saturating_sub(dest));
                        mem[dest..dest + n].copy_from_slice(&e[..n]);
                    }
                    mem[at + 12..at + 16].copy_from_slice(&(e.len() as i32).to_be_bytes());
                }
            }
        }
    }
}

/// The address of the string of a z/OS MQCHARV (`v`) in a structure at
/// `base`: VSPtr, else the structure's address plus VSOffset.
fn charv_addr(mem: &[u8], base: usize, v: &[u8]) -> Option<usize> {
    let ptr = u32::from_be_bytes([v[0], v[1], v[2], v[3]]);
    let off = be(v, 4);
    let a = if ptr != 0 && ptr != 0xFF00_0000 {
        ptr as usize
    } else if off > 0 {
        base + off as usize
    } else {
        return None;
    };
    (a < mem.len()).then_some(a)
}

/// The characters of a z/OS MQCHARV (VSLength -1: up to a null).
fn charv_text(mem: &[u8], base: usize, v: &[u8]) -> Vec<u8> {
    let Some(a) = charv_addr(mem, base, v) else {
        return Vec::new();
    };
    let len = be(v, 12);
    let end = if len < 0 {
        mem[a..]
            .iter()
            .position(|&b| b == 0)
            .map_or(mem.len(), |p| a + p)
    } else {
        (a + len as usize).min(mem.len())
    };
    mem[a..end].to_vec()
}

/// A big-endian MQLONG.
pub fn be(b: &[u8], at: usize) -> i32 {
    i32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// A native MQLONG.
pub fn ne(b: &[u8], at: usize) -> i32 {
    i32::from_ne_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

pub fn set_ne(b: &mut [u8], at: usize, v: i32) {
    b[at..at + 4].copy_from_slice(&v.to_ne_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_of_a_message_descriptor() {
        let mut zos = vec![0u8; 364];
        zos[..4].copy_from_slice(&ebcdic::encode("MD  "));
        zos[4..8].copy_from_slice(&2i32.to_be_bytes());
        zos[24..28].copy_from_slice(&785i32.to_be_bytes());
        zos[32..40].copy_from_slice(&ebcdic::encode("MQSTR   "));
        zos[48..72].copy_from_slice(&[0xAB; 24]);
        zos[360..364].copy_from_slice(&(-1i32).to_be_bytes());
        assert!(MD.is(&zos));
        assert_eq!(MD.version(&zos), 2);
        let n = MD.to_native(&zos, 0, 2);
        assert_eq!(&n.bytes[..4], b"MD  ");
        assert_eq!(ne(&n.bytes, 24), 785);
        assert_eq!(&n.bytes[32..40], b"MQSTR   ");
        assert_eq!(&n.bytes[48..72], &[0xAB; 24]);
        assert_eq!(ne(&n.bytes, 360), -1);
        let mut back = vec![0u8; 364];
        back[..4].copy_from_slice(&ebcdic::encode("MD  "));
        back[4..8].copy_from_slice(&2i32.to_be_bytes());
        MD.to_zos(&n, &mut back, 0, 2);
        assert_eq!(back, zos);
        // a version 1 descriptor converts its first 324 bytes
        assert_eq!(MD.to_native(&zos, 0, 1).bytes.len(), 324);
    }

    #[test]
    fn native_lengths() {
        assert_eq!(OD.native_len(1), 168);
        assert_eq!(OD.native_len(2), 208);
        assert_eq!(OD.native_len(3), 344);
        assert_eq!(OD.native_len(4), 424);
        assert_eq!(PMO.native_len(2), 160);
        assert_eq!(PMO.native_len(3), 184);
        assert_eq!(GMO.native_len(3), 100);
        assert_eq!(GMO.native_len(4), 112);
        assert_eq!(SD.native_len(1), 344);
        assert_eq!(IMPO.native_len(1), 64);
        assert_eq!(CHARV.native_len(1), 24);
    }

    #[test]
    fn variable_length_strings() {
        // an MQOD version 4 whose ObjectString is at VSOffset 400
        let mut mem = vec![0u8; 420];
        mem[..4].copy_from_slice(&ebcdic::encode("OD  "));
        mem[4..8].copy_from_slice(&4i32.to_be_bytes());
        mem[336 + 4..336 + 8].copy_from_slice(&400i32.to_be_bytes());
        mem[336 + 12..336 + 16].copy_from_slice(&7i32.to_be_bytes());
        mem[400..407].copy_from_slice(&ebcdic::encode("dev/abc"));
        // ResObjectString: a buffer of 10 at 410
        mem[376..380].copy_from_slice(&410u32.to_be_bytes());
        mem[376 + 8..376 + 12].copy_from_slice(&10i32.to_be_bytes());
        let mut n = OD.to_native(&mem, 0, 4);
        let v = &n.bytes[344..368];
        let p = u64::from_ne_bytes(v[..8].try_into().unwrap());
        assert_eq!(p, n.bufs[0].as_ptr() as u64);
        assert_eq!(&n.bufs[0][..7], b"dev/abc");
        assert_eq!(ne(v, 16), 7);
        assert_eq!(ne(v, 20), NATIVE_CCSID);
        // the queue manager returns a resolved string
        n.bufs[2][..5].copy_from_slice(b"dev/x");
        set_ne(&mut n.bytes, 392 + 16, 5);
        OD.to_zos(&n, &mut mem, 0, 4);
        assert_eq!(&mem[410..415], &ebcdic::encode("dev/x")[..]);
        assert_eq!(be(&mem, 376 + 12), 5);
    }
}

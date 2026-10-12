//! MQI structures in the two forms: as COBOL programs on z/OS see them
//! (big-endian integers, EBCDIC characters) and as the C client library
//! on this host takes them (native integers, ASCII characters). The
//! fields of the versions supported have the same offsets in both.

use zeus_cobol::ebcdic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// MQLONG.
    Int,
    /// Character data (names, formats).
    Char,
    /// Byte strings (message and correlation identifiers, tokens).
    Bytes,
}

/// A structure: its identifier, its length for each version and its
/// fields (offset, length, kind).
pub struct Layout {
    pub id: &'static [u8; 4],
    pub lengths: &'static [usize],
    pub fields: &'static [(usize, usize, Kind)],
}

use Kind::*;

/// MQOD version 1.
pub const OD: Layout = Layout {
    id: b"OD  ",
    lengths: &[168],
    fields: &[
        (0, 4, Char),
        (4, 4, Int),
        (8, 4, Int),
        (12, 48, Char),
        (60, 48, Char),
        (108, 48, Char),
        (156, 12, Char),
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

/// MQPMO version 1.
pub const PMO: Layout = Layout {
    id: b"PMO ",
    lengths: &[128],
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
    ],
};

/// MQGMO versions 1 to 3.
pub const GMO: Layout = Layout {
    id: b"GMO ",
    lengths: &[72, 80, 100],
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
    ],
};

/// Offset of the Version field (all structures).
pub const VERSION: usize = 4;

impl Layout {
    /// The version of a z/OS structure, limited to the versions known.
    pub fn version(&self, zos: &[u8]) -> usize {
        let v = be(zos, VERSION).clamp(1, self.lengths.len() as i32);
        v as usize
    }

    /// The length of `version`.
    pub fn len(&self, version: usize) -> usize {
        self.lengths[version - 1]
    }

    /// Whether a z/OS structure has this structure's identifier.
    pub fn is(&self, zos: &[u8]) -> bool {
        zos.len() >= 4 && zos[..4].iter().map(|&b| ebcdic::to_latin1(b)).eq(self.id.iter().copied())
    }

    /// The native form of the first `len(version)` bytes of `zos`; the
    /// version is set to `version`.
    pub fn to_native(&self, zos: &[u8], version: usize) -> Vec<u8> {
        let n = self.len(version);
        let mut out = vec![0u8; n];
        for &(off, len, kind) in self.fields.iter().filter(|f| f.0 + f.1 <= n) {
            let src = &zos[off..off + len];
            let dst = &mut out[off..off + len];
            match kind {
                Int => dst.copy_from_slice(&be(src, 0).to_ne_bytes()),
                Char => {
                    for (d, s) in dst.iter_mut().zip(src) {
                        *d = ebcdic::to_latin1(*s);
                    }
                }
                Bytes => dst.copy_from_slice(src),
            }
        }
        out[VERSION..VERSION + 4].copy_from_slice(&(version as i32).to_ne_bytes());
        out
    }

    /// Copies a native structure back into the z/OS one (its version
    /// stays).
    pub fn to_zos(&self, native: &[u8], zos: &mut [u8]) {
        let n = native.len();
        for &(off, len, kind) in self.fields.iter().filter(|f| f.0 + f.1 <= n) {
            if off == VERSION {
                continue;
            }
            let src = &native[off..off + len];
            let dst = &mut zos[off..off + len];
            match kind {
                Int => dst.copy_from_slice(&ne(src, 0).to_be_bytes()),
                Char => {
                    for (d, s) in dst.iter_mut().zip(src) {
                        *d = ebcdic::from_latin1(*s);
                    }
                }
                Bytes => dst.copy_from_slice(src),
            }
        }
    }
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
        let n = MD.to_native(&zos, 2);
        assert_eq!(&n[..4], b"MD  ");
        assert_eq!(ne(&n, 24), 785);
        assert_eq!(&n[32..40], b"MQSTR   ");
        assert_eq!(&n[48..72], &[0xAB; 24]);
        assert_eq!(ne(&n, 360), -1);
        let mut back = vec![0u8; 364];
        back[4..8].copy_from_slice(&2i32.to_be_bytes());
        MD.to_zos(&n, &mut back);
        assert_eq!(back, zos);
        // a version 1 descriptor converts its first 324 bytes
        assert_eq!(MD.to_native(&zos, 1).len(), 324);
    }
}

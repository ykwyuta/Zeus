//! BMS (Basic Mapping Support) map definitions.
//!
//! A mapset is written as assembler macro instructions: DFHMSD (the
//! mapset), DFHMDI (a map) and DFHMDF (a field). [`parse`] reads them into
//! a [`Mapset`]; [`symbolic::cobol`] generates the COBOL symbolic map
//! (the copybook the programs include) and the mapset itself, serialized,
//! is the physical map description the CICS runtime uses to build and
//! read 3270 screens.

pub mod parse;
pub mod symbolic;

use serde::{Deserialize, Serialize};

pub use parse::parse;

/// Extended attributes a symbolic map has a byte for, in the order the
/// bytes appear: (DSATTS keyword, field-name suffix).
pub const EXT_ATTRS: [(&str, char); 7] = [
    ("COLOR", 'C'),
    ("PS", 'P'),
    ("HILIGHT", 'H'),
    ("VALIDN", 'V'),
    ("OUTLINE", 'U'),
    ("SOSI", 'M'),
    ("TRANSP", 'T'),
];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Mapset {
    pub name: String,
    pub mode: String,
    pub lang: String,
    /// STORAGE=AUTO: every map has its own storage (otherwise the maps of
    /// the mapset redefine the first).
    pub storage_auto: bool,
    /// TIOAPFX=YES: 12-byte prefix at the start of each symbolic map.
    pub tioapfx: bool,
    pub ctrl: Vec<String>,
    /// Extended attributes in the symbolic map (DSATTS, or EXTATT=YES).
    pub dsatts: Vec<String>,
    pub maps: Vec<Map>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Map {
    pub name: String,
    /// Lines and columns.
    pub size: (u16, u16),
    pub line: u16,
    pub column: u16,
    pub ctrl: Vec<String>,
    pub tioapfx: bool,
    pub dsatts: Vec<String>,
    pub cursloc: bool,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: Option<String>,
    /// Line and column (1-based) of the attribute byte.
    pub pos: (u16, u16),
    pub length: u16,
    pub attrb: Vec<String>,
    pub color: Option<String>,
    pub hilight: Option<String>,
    pub ps: Option<String>,
    pub validn: Vec<String>,
    pub outline: Vec<String>,
    pub justify: Vec<String>,
    /// INITIAL (EBCDIC is produced at run time) or XINIT (hexadecimal).
    pub initial: Option<String>,
    pub xinit: Option<String>,
    pub picin: Option<String>,
    pub picout: Option<String>,
    pub occurs: u16,
    pub grpname: Option<String>,
}

//! Source compiler options: implement supported settings, diagnose the rest.
use super::{FrontOptions, Trunc};
use crate::diag::{Diag, Result};

pub(super) struct Effective {
    pub optimize: u8,
    pub sql: Option<bool>,
    pub sqlims: bool,
    pub listing: crate::listing::ListingOptions,
    pub cics: bool,
    pub dbcs: bool,
    pub trunc: Trunc,
    pub numeric: crate::numeric::Settings,
    pub defines: std::collections::HashMap<String, String>,
    pub apost: bool,
    pub awo: bool,
    pub block0: bool,
    pub ssrange: bool,
    pub codepage: Option<u32>,
    pub dynam: bool,
    pub dll: bool,
    pub exportall: bool,
    pub thread: bool,
    pub lp64: bool,
}
impl Effective {
    pub fn apply(&self, base: &FrontOptions) -> FrontOptions {
        FrontOptions {
            compiler_options: String::new(),
            optimize: self.optimize,
            sql: self.sql,
            sqlims: self.sqlims,
            listing: self.listing.clone(),
            cics: self.cics,
            nsymbol_dbcs: self.dbcs,
            trunc: self.trunc,
            numeric: self.numeric,
            defines: self.defines.clone(),
            apost: self.apost,
            awo: self.awo,
            block0: self.block0,
            ssrange: self.ssrange,
            codepage: self.codepage,
            dynam: self.dynam,
            dll: self.dll,
            exportall: self.exportall,
            thread: self.thread,
            lp64: self.lp64,
            ..base.clone()
        }
    }
}

pub(super) fn parse(text: &str, defaults: &FrontOptions) -> Result<Effective> {
    let mut optimize = defaults.optimize;
    let mut sql = defaults.sql;
    let mut sqlims = defaults.sqlims;
    let mut listing = defaults.listing.clone();
    let (mut list_here, mut offset_here) = (None, None);
    let mut cics = defaults.cics;
    let mut dbcs = defaults.nsymbol_dbcs;
    let mut trunc = defaults.trunc;
    let mut numeric = defaults.numeric;
    let mut defines = defaults.defines.clone();
    let mut apost = defaults.apost;
    let mut awo = defaults.awo;
    let mut block0 = defaults.block0;
    let mut ssrange = defaults.ssrange;
    let mut codepage = defaults.codepage;
    let mut dynam = defaults.dynam;
    let mut dll = defaults.dll;
    let mut exportall = defaults.exportall;
    let mut thread = defaults.thread;
    let mut lp64 = defaults.lp64;
    let mut rest = text.trim();
    while !rest.is_empty() {
        rest = rest.trim_start_matches(|c: char| c.is_ascii_whitespace() || c == ',');
        if rest.is_empty() {
            break;
        }
        let end = rest
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
            .unwrap_or(rest.len());
        let name = &rest[..end];
        if name.is_empty() {
            return Err(Diag::error(
                Default::default(),
                "invalid CBL/PROCESS option",
            ));
        }
        rest = rest[end..].trim_start();
        let arg = if rest.starts_with('(') {
            let end = option_end(rest)
                .ok_or_else(|| Diag::error(Default::default(), "unclosed compiler option"))?;
            let arg = rest[1..end].trim();
            rest = &rest[end + 1..];
            Some(arg)
        } else {
            None
        };
        let upper_name = name.to_ascii_uppercase();
        let upper_arg = arg.map(str::to_ascii_uppercase);
        match (upper_name.as_str(), upper_arg.as_deref()) {
            ("OPTIMIZE" | "OPT", Some("0")) | ("NOOPTIMIZE" | "NOOPT", None) => optimize = 0,
            ("OPTIMIZE" | "OPT", Some("1" | "STD")) | ("OPTIMIZE" | "OPT", None) => optimize = 1,
            ("OPTIMIZE" | "OPT", Some("2" | "FULL")) => optimize = 2,
            ("LP", Some("32")) => lp64 = false,
            ("LP", Some("64")) => lp64 = true,
            ("SQLIMS", None) => sqlims = true,
            ("NOSQLIMS", None) => sqlims = false,
            ("THREAD", None) => thread = true,
            ("NOTHREAD", None) => thread = false,
            ("DLL", None) => dll = true,
            ("NODLL", None) => dll = false,
            ("EXPORTALL" | "EXP", None) => exportall = true,
            ("NOEXPORTALL" | "NOEXP", None) => exportall = false,
            ("SQL", None) => sql = Some(true),
            ("NOSQL", None) => sql = Some(false),
            ("LIST", None) => {
                listing.list = true;
                list_here = Some(true);
            }
            ("NOLIST", None) => {
                listing.list = false;
                list_here = Some(false);
            }
            ("OFFSET" | "OFF", None) => {
                listing.offset = true;
                offset_here = Some(true);
            }
            ("NOOFFSET" | "NOOFF", None) => {
                listing.offset = false;
                offset_here = Some(false);
            }
            ("SOURCE" | "S", None | Some("DEC")) => {
                listing.source = true;
                listing.source_hex = false;
            }
            ("SOURCE" | "S", Some("HEX")) => {
                listing.source = true;
                listing.source_hex = true;
            }
            ("NOSOURCE" | "NOS", None) => listing.source = false,
            ("MAP", None) => listing.map = true,
            ("NOMAP", None) => listing.map = false,
            ("XREF" | "X", None | Some("FULL")) => listing.xref = Some(true),
            ("XREF" | "X", Some("SHORT")) => listing.xref = Some(false),
            ("NOXREF" | "NOX", None) => listing.xref = None,
            ("VBREF", None) => listing.vbref = true,
            ("NOVBREF", None) => listing.vbref = false,
            ("DYNAM" | "DYN", None) => dynam = true,
            ("NODYNAM" | "NODYN", None) => dynam = false,
            ("CODEPAGE" | "CP", Some(ccsid)) => {
                let ccsid = ccsid
                    .parse::<u32>()
                    .ok()
                    .filter(|c| crate::codepage::get(*c).is_some())
                    .ok_or_else(|| {
                        Diag::error(
                            Default::default(),
                            "CODEPAGE requires a supported EBCDIC CCSID (see codepages/README.md)",
                        )
                    })?;
                codepage = Some(ccsid);
            }
            ("SSRANGE" | "SSR", None) => ssrange = true,
            ("NOSSRANGE" | "NOSSR", None) => ssrange = false,
            ("APOST", None) => apost = true,
            ("QUOTE", None) => apost = false,
            ("AWO", None) => awo = true,
            ("NOAWO", None) => awo = false,
            ("BLOCK0", None) => block0 = true,
            ("NOBLOCK0", None) => block0 = false,
            ("DEFINE" | "DEF", Some(_)) => {
                let (name, value) = arg.unwrap().split_once('=').ok_or_else(|| {
                    Diag::error(Default::default(), "DEFINE requires name=literal")
                })?;
                defines.insert(name.trim().to_ascii_uppercase(), value.trim().to_string());
            }
            ("NODEFINE" | "NODEF", None) => defines.clear(),
            ("CICS", None) => cics = true,
            ("CICS", Some(arg)) if cics_options(arg) => cics = true,
            ("NOCICS", None) => cics = false,
            // SQL is translated by the integrated EXEC SQL front end.
            // Code is immutable and each Machine owns its data (RENT).
            ("RENT", None) => {}
            ("FLAG" | "F", Some(levels)) => {
                let levels: Vec<_> = levels.split(',').map(str::trim).collect();
                let parse_level = |value| match value {
                    "I" => Some(crate::diag::Severity::Info),
                    "W" => Some(crate::diag::Severity::Warning),
                    "E" => Some(crate::diag::Severity::Error),
                    "S" => Some(crate::diag::Severity::Severe),
                    "U" => Some(crate::diag::Severity::Unrecoverable),
                    _ => None,
                };
                let summary = levels.first().and_then(|s| parse_level(s));
                let embedded = levels.get(1).and_then(|s| parse_level(s));
                if summary.is_none()
                    || levels.len() > 2
                    || (levels.len() == 2 && (embedded.is_none() || embedded < summary))
                {
                    return Err(Diag::error(
                        Default::default(),
                        "FLAG requires I/W/E/S/U and an optional equal or higher embedding level",
                    ));
                }
                listing.flag = summary;
                listing.embedded_flag = embedded;
                listing.explicit_embedded_flag = levels.len() == 2;
            }
            ("NOFLAG" | "NOF", None) => {
                listing.flag = None;
                listing.embedded_flag = None;
                listing.explicit_embedded_flag = false;
            }
            ("NSYMBOL" | "NS", Some("DBCS")) => dbcs = true,
            ("NSYMBOL" | "NS", Some("NATIONAL" | "NAT")) => dbcs = false,
            ("TRUNC", Some("STD")) => trunc = Trunc::Std,
            ("TRUNC", Some("OPT")) => trunc = Trunc::Opt,
            ("TRUNC", Some("BIN")) => trunc = Trunc::Bin,
            ("ARITH", Some("COMPAT")) => numeric.arithmetic = crate::numeric::Arithmetic::Compat,
            ("ARITH", Some("EXTEND")) => numeric.arithmetic = crate::numeric::Arithmetic::Extend,
            ("NUMPROC", Some("NOPFD")) => numeric.numproc = crate::numeric::Numproc::Nopfd,
            ("NUMPROC", Some("PFD")) => numeric.numproc = crate::numeric::Numproc::Pfd,
            ("NUMPROC", Some("MIG")) => numeric.numproc = crate::numeric::Numproc::Mig,
            ("NUMCLS", Some("PRIM")) => numeric.numcls = crate::numeric::Numcls::Primary,
            ("NUMCLS", Some("ALT")) => numeric.numcls = crate::numeric::Numcls::Alternate,
            _ => {
                return Err(Diag::error(
                    Default::default(),
                    format!(
                        "unsupported or invalid CBL/PROCESS option: {name}{}",
                        arg.map_or(String::new(), |a| format!("({a})"))
                    ),
                ));
            }
        }
    }
    if lp64 {
        cics = false;
    }
    if dynam && cics {
        return Err(Diag::error(
            Default::default(),
            "DYNAM cannot be combined with CICS",
        ));
    }
    if lp64 {
        dll = false;
        thread = false;
        sqlims = false;
    }
    if dynam && dll {
        return Err(Diag::error(Default::default(), "DLL requires NODYNAM"));
    }
    if list_here == Some(true) && offset_here == Some(true) {
        return Err(Diag::error(
            Default::default(),
            "LIST and OFFSET are mutually exclusive at the same option level",
        ));
    }
    if list_here == Some(true) {
        listing.offset = false;
    }
    if offset_here == Some(true) {
        listing.list = false;
    }
    if optimize > 2 {
        return Err(Diag::error(
            Default::default(),
            "OPTIMIZE must be 0, 1 or 2",
        ));
    }
    if listing.explicit_embedded_flag && !listing.source {
        return Err(Diag::error(
            Default::default(),
            "FLAG with an embedding level requires SOURCE",
        ));
    }
    Ok(Effective {
        lp64,
        thread,
        dll,
        exportall,
        optimize,
        sql,
        sqlims,
        listing,
        dynam,
        codepage,
        ssrange,
        cics,
        dbcs,
        trunc,
        numeric,
        defines,
        apost,
        awo,
        block0,
    })
}

/// Parentheses and doubled delimiters inside DEFINE/CICS literals do not
/// terminate the containing option.
fn option_end(text: &str) -> Option<usize> {
    let mut quote = None;
    let mut depth = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((offset, c)) = chars.next() {
        if let Some(delimiter) = quote {
            if c == delimiter {
                if chars.peek().is_some_and(|(_, next)| *next == delimiter) {
                    chars.next();
                } else {
                    quote = None;
                }
            }
        } else {
            match c {
                '\'' | '"' => quote = Some(c),
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(offset);
                    }
                }
                _ => (),
            }
        }
    }
    None
}

fn cics_options(arg: &str) -> bool {
    let Some(options) = arg.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) else {
        return false;
    };
    !options.is_empty()
        && options
            .split(',')
            .all(|option| matches!(option.trim(), "SP" | "EDF" | "DLI"))
}

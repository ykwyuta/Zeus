//! Integer/floating conversion forms. M3 and M4 occupy byte 2, independently
//! of the two registers in byte 3.
use super::{Format, Rounding};
#[derive(Clone, Copy)]
pub(crate) struct Conversion {
    pub opcode: u16,
    pub bits: u32,
    pub width: usize,
    pub to_integer: bool,
    pub logical: bool,
    pub format: Format,
    pub masks: bool,
}
pub(crate) fn instruction(op: &str) -> Option<Conversion> {
    let advanced = op.ends_with('A');
    let root = op.strip_suffix('A').unwrap_or(op);
    let (opcode, bits, width, to_integer, logical, format) = match root {
        "CDGTR" => (0xb3f1, 64, 8, false, false, Format::Decimal),
        "CXGTR" => (0xb3f9, 64, 16, false, false, Format::Decimal),
        "CDFTR" => (0xb951, 32, 8, false, false, Format::Decimal),
        "CXFTR" => (0xb959, 32, 16, false, false, Format::Decimal),
        "CDLGTR" => (0xb952, 64, 8, false, true, Format::Decimal),
        "CXLGTR" => (0xb95a, 64, 16, false, true, Format::Decimal),
        "CDLFTR" => (0xb953, 32, 8, false, true, Format::Decimal),
        "CXLFTR" => (0xb95b, 32, 16, false, true, Format::Decimal),
        "CGDTR" => (0xb3e1, 64, 8, true, false, Format::Decimal),
        "CGXTR" => (0xb3e9, 64, 16, true, false, Format::Decimal),
        "CFDTR" => (0xb941, 32, 8, true, false, Format::Decimal),
        "CFXTR" => (0xb949, 32, 16, true, false, Format::Decimal),
        "CLGDTR" => (0xb942, 64, 8, true, true, Format::Decimal),
        "CLGXTR" => (0xb94a, 64, 16, true, true, Format::Decimal),
        "CLFDTR" => (0xb943, 32, 8, true, true, Format::Decimal),
        "CLFXTR" => (0xb94b, 32, 16, true, true, Format::Decimal),
        "CEFBR" => (0x94, 32, 4, false, false, Format::Binary),
        "CDFBR" => (0x95, 32, 8, false, false, Format::Binary),
        "CXFBR" => (0x96, 32, 16, false, false, Format::Binary),
        "CEGBR" => (0xa4, 64, 4, false, false, Format::Binary),
        "CDGBR" => (0xa5, 64, 8, false, false, Format::Binary),
        "CXGBR" => (0xa6, 64, 16, false, false, Format::Binary),
        "CELFBR" => (0x90, 32, 4, false, true, Format::Binary),
        "CDLFBR" => (0x91, 32, 8, false, true, Format::Binary),
        "CXLFBR" => (0x92, 32, 16, false, true, Format::Binary),
        "CELGBR" => (0xa0, 64, 4, false, true, Format::Binary),
        "CDLGBR" => (0xa1, 64, 8, false, true, Format::Binary),
        "CXLGBR" => (0xa2, 64, 16, false, true, Format::Binary),
        "CFEBR" => (0x98, 32, 4, true, false, Format::Binary),
        "CFDBR" => (0x99, 32, 8, true, false, Format::Binary),
        "CFXBR" => (0x9a, 32, 16, true, false, Format::Binary),
        "CGEBR" => (0xa8, 64, 4, true, false, Format::Binary),
        "CGDBR" => (0xa9, 64, 8, true, false, Format::Binary),
        "CGXBR" => (0xaa, 64, 16, true, false, Format::Binary),
        "CLFEBR" => (0x9c, 32, 4, true, true, Format::Binary),
        "CLFDBR" => (0x9d, 32, 8, true, true, Format::Binary),
        "CLFXBR" => (0x9e, 32, 16, true, true, Format::Binary),
        "CLGEBR" => (0xac, 64, 4, true, true, Format::Binary),
        "CLGDBR" => (0xad, 64, 8, true, true, Format::Binary),
        "CLGXBR" => (0xae, 64, 16, true, true, Format::Binary),
        "CEFR" => (0xb4, 32, 4, false, false, Format::Hex),
        "CDFR" => (0xb5, 32, 8, false, false, Format::Hex),
        "CXFR" => (0xb6, 32, 16, false, false, Format::Hex),
        "CEGR" => (0xc4, 64, 4, false, false, Format::Hex),
        "CDGR" => (0xc5, 64, 8, false, false, Format::Hex),
        "CXGR" => (0xc6, 64, 16, false, false, Format::Hex),
        "CFER" => (0xb8, 32, 4, true, false, Format::Hex),
        "CFDR" => (0xb9, 32, 8, true, false, Format::Hex),
        "CFXR" => (0xba, 32, 16, true, false, Format::Hex),
        "CGER" => (0xc8, 64, 4, true, false, Format::Hex),
        "CGDR" => (0xc9, 64, 8, true, false, Format::Hex),
        "CGXR" => (0xca, 64, 16, true, false, Format::Hex),
        _ => return None,
    };
    if advanced && (logical || format == Format::Hex || format == Format::Decimal && bits == 32) {
        return None;
    }
    Some(Conversion {
        opcode: if opcode > 255 {
            opcode
        } else {
            0xb300 | opcode
        },
        bits,
        width,
        to_integer,
        logical,
        format,
        masks: advanced || logical || format == Format::Decimal && bits == 32,
    })
}
/// RRF-e BFP modifiers: M3 is rounding; only bit 1 of M4 is defined.
pub(crate) fn validate_binary_masks(mask: u32) -> Result<(), String> {
    if mask & !0x4f != 0 {
        return Err("invalid BFP M4 modifier: only inexact suppression (4) is defined".into());
    }
    rounding(mask & 15, 0, Format::Binary)?;
    Ok(())
}
pub(crate) fn rounding(mask: u32, control: u32, format: Format) -> Result<Rounding, String> {
    if format == Format::Decimal {
        return Ok(decimal_rounding(mask, control));
    }
    Ok(match mask {
        0 if format == Format::Hex => Rounding::Zero,
        0 => match control & 7 {
            0 => Rounding::NearestEven,
            1 => Rounding::Zero,
            2 => Rounding::Positive,
            3 => Rounding::Negative,
            7 => Rounding::Prepare,
            _ => return Err("invalid BFP rounding mode".into()),
        },
        1 => Rounding::NearestAway,
        3 if format == Format::Binary => Rounding::Prepare,
        4 => Rounding::NearestEven,
        5 => Rounding::Zero,
        6 => Rounding::Positive,
        7 => Rounding::Negative,
        _ => return Err("invalid floating-point conversion rounding mask".into()),
    })
}

pub(crate) fn name(opcode: u16) -> Option<&'static str> {
    [
        "CDGTR", "CXGTR", "CDFTR", "CXFTR", "CDLGTR", "CXLGTR", "CDLFTR", "CXLFTR", "CGDTR",
        "CGXTR", "CFDTR", "CFXTR", "CLGDTR", "CLGXTR", "CLFDTR", "CLFXTR", "CEFBR", "CDFBR",
        "CXFBR", "CEGBR", "CDGBR", "CXGBR", "CELFBR", "CDLFBR", "CXLFBR", "CELGBR", "CDLGBR",
        "CXLGBR", "CFEBR", "CFDBR", "CFXBR", "CGEBR", "CGDBR", "CGXBR", "CLFEBR", "CLFDBR",
        "CLFXBR", "CLGEBR", "CLGDBR", "CLGXBR", "CEFR", "CDFR", "CXFR", "CEGR", "CDGR", "CXGR",
        "CFER", "CFDR", "CFXR", "CGER", "CGDR", "CGXR",
    ]
    .into_iter()
    .find(|op| instruction(op).is_some_and(|s| s.opcode == opcode))
}

pub(crate) fn decimal_rounding(mask: u32, control: u32) -> Rounding {
    let mask = if mask == 0 || mask == 2 {
        8 + ((control >> 4) & 7)
    } else {
        mask
    };
    match mask {
        1 | 12 => Rounding::NearestAway,
        3 | 15 => Rounding::Prepare,
        4 | 8 => Rounding::NearestEven,
        5 | 9 => Rounding::Zero,
        6 | 10 => Rounding::Positive,
        7 | 11 => Rounding::Negative,
        13 => Rounding::NearestZero,
        _ => Rounding::Away,
    }
}

pub(crate) fn radix_rounding(mask: u32) -> Result<Rounding, String> {
    if mask == 0 {
        Ok(Rounding::Zero)
    } else {
        rounding(mask, 0, Format::Binary)
    }
}

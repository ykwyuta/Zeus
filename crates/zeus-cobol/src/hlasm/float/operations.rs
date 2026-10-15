//! Floating-point source instruction forms and architectural opcode bytes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Add,
    Subtract,
    Multiply,
    Divide,
    Compare,
    SignalCompare,
    CompareExponent,
    Positive,
    Negative,
    Complement,
    Test,
    Convert,
    RoundInteger,
    Lengthen,
    Sqrt,
    Half,
    Zero,
    FusedAdd,
    FusedSubtract,
}

// op, secondary RRE opcode, destination bytes, source bytes, operation
pub(crate) const RRE: &[(&str, u8, usize, usize, Kind)] = &[
    ("MAEBR", 0x0e, 4, 4, Kind::FusedAdd),
    ("MADBR", 0x1e, 8, 8, Kind::FusedAdd),
    ("MSEBR", 0x0f, 4, 4, Kind::FusedSubtract),
    ("MSDBR", 0x1f, 8, 8, Kind::FusedSubtract),
    ("MDEBR", 0x0c, 8, 4, Kind::Multiply),
    ("MXDBR", 0x07, 16, 8, Kind::Multiply),
    ("AEBR", 0x0a, 4, 4, Kind::Add),
    ("ADBR", 0x1a, 8, 8, Kind::Add),
    ("AXBR", 0x4a, 16, 16, Kind::Add),
    ("SEBR", 0x0b, 4, 4, Kind::Subtract),
    ("SDBR", 0x1b, 8, 8, Kind::Subtract),
    ("SXBR", 0x4b, 16, 16, Kind::Subtract),
    ("MEEBR", 0x17, 4, 4, Kind::Multiply),
    ("MDBR", 0x1c, 8, 8, Kind::Multiply),
    ("MXBR", 0x4c, 16, 16, Kind::Multiply),
    ("DEBR", 0x0d, 4, 4, Kind::Divide),
    ("DDBR", 0x1d, 8, 8, Kind::Divide),
    ("DXBR", 0x4d, 16, 16, Kind::Divide),
    ("CEBR", 0x09, 4, 4, Kind::Compare),
    ("CDBR", 0x19, 8, 8, Kind::Compare),
    ("CXBR", 0x49, 16, 16, Kind::Compare),
    ("KEBR", 0x08, 4, 4, Kind::SignalCompare),
    ("KDBR", 0x18, 8, 8, Kind::SignalCompare),
    ("KXBR", 0x48, 16, 16, Kind::SignalCompare),
    ("LPEBR", 0x00, 4, 4, Kind::Positive),
    ("LPDBR", 0x10, 8, 8, Kind::Positive),
    ("LPXBR", 0x40, 16, 16, Kind::Positive),
    ("LNEBR", 0x01, 4, 4, Kind::Negative),
    ("LNDBR", 0x11, 8, 8, Kind::Negative),
    ("LNXBR", 0x41, 16, 16, Kind::Negative),
    ("LCEBR", 0x03, 4, 4, Kind::Complement),
    ("LCDBR", 0x13, 8, 8, Kind::Complement),
    ("LCXBR", 0x43, 16, 16, Kind::Complement),
    ("LTEBR", 0x02, 4, 4, Kind::Test),
    ("LTDBR", 0x12, 8, 8, Kind::Test),
    ("LTXBR", 0x42, 16, 16, Kind::Test),
    ("LDEBR", 0x04, 8, 4, Kind::Convert),
    ("LXDBR", 0x05, 16, 8, Kind::Convert),
    ("LXEBR", 0x06, 16, 4, Kind::Convert),
    ("LEDBR", 0x44, 4, 8, Kind::Convert),
    ("LDXBR", 0x45, 8, 16, Kind::Convert),
    ("LEXBR", 0x46, 4, 16, Kind::Convert),
    ("FIEBR", 0x57, 4, 4, Kind::RoundInteger),
    ("FIDBR", 0x5f, 8, 8, Kind::RoundInteger),
    ("FIXBR", 0x47, 16, 16, Kind::RoundInteger),
    ("SQEBR", 0x14, 4, 4, Kind::Sqrt),
    ("SQDBR", 0x15, 8, 8, Kind::Sqrt),
    ("SQXBR", 0x16, 16, 16, Kind::Sqrt),
    ("LZER", 0x74, 4, 4, Kind::Zero),
    ("LZDR", 0x75, 8, 8, Kind::Zero),
    ("LZXR", 0x76, 16, 16, Kind::Zero),
];

pub(crate) fn register(op: &str) -> Option<(u8, usize, usize, Kind)> {
    let root = op
        .strip_suffix('A')
        .filter(|r| {
            matches!(
                *r,
                "FIEBR" | "FIDBR" | "FIXBR" | "LEDBR" | "LDXBR" | "LEXBR"
            )
        })
        .unwrap_or(op);
    RRE.iter()
        .find(|s| s.0 == root)
        .map(|s| (s.1, s.2, s.3, s.4))
}
pub(crate) fn memory(op: &str) -> Option<(u8, usize, Kind)> {
    Some(match op {
        "LDEB" => (0x04, 4, Kind::Convert),
        "LXDB" => (0x05, 8, Kind::Convert),
        "LXEB" => (0x06, 4, Kind::Convert),
        "MXDB" => (0x07, 8, Kind::Multiply),
        "AEB" => (0x0a, 4, Kind::Add),
        "ADB" => (0x1a, 8, Kind::Add),
        "SEB" => (0x0b, 4, Kind::Subtract),
        "SDB" => (0x1b, 8, Kind::Subtract),
        "MEEB" => (0x17, 4, Kind::Multiply),
        "MDEB" => (0x0c, 4, Kind::Multiply),
        "MDB" => (0x1c, 8, Kind::Multiply),
        "DEB" => (0x0d, 4, Kind::Divide),
        "DDB" => (0x1d, 8, Kind::Divide),
        "CEB" => (0x09, 4, Kind::Compare),
        "CDB" => (0x19, 8, Kind::Compare),
        "KEB" => (0x08, 4, Kind::SignalCompare),
        "KDB" => (0x18, 8, Kind::SignalCompare),
        "SQEB" => (0x14, 4, Kind::Sqrt),
        "SQDB" => (0x15, 8, Kind::Sqrt),
        "MAEB" => (0x0e, 4, Kind::FusedAdd),
        "MADB" => (0x1e, 8, Kind::FusedAdd),
        "MSEB" => (0x0f, 4, Kind::FusedSubtract),
        "MSDB" => (0x1f, 8, Kind::FusedSubtract),
        _ => return None,
    })
}
pub(crate) fn memory_destination(op: &str) -> Option<usize> {
    memory(op).map(|(_, width, _)| match op {
        "LDEB" | "MDEB" => 8,
        "LXDB" | "LXEB" | "MXDB" => 16,
        _ => width,
    })
}
pub(crate) fn divide_integer(op: &str) -> Option<(u8, usize)> {
    match op {
        "DIEBR" => Some((0x53, 4)),
        "DIDBR" => Some((0x5b, 8)),
        _ => None,
    }
}
pub(crate) fn divide_integer_name(op: u8) -> Option<&'static str> {
    match op {
        0x53 => Some("DIEBR"),
        0x5b => Some("DIDBR"),
        _ => None,
    }
}
pub(crate) fn size(op: &str) -> Option<u32> {
    if op == "PFPO" {
        return Some(2);
    }
    if classify(op).is_some() || decimal_shift(op).is_some() {
        return Some(6);
    }
    if decimal_memory(op).is_some() {
        return Some(6);
    }
    if let Some(spec) = hex_unnormalized(op) {
        return Some(if spec.memory { 6 } else { 4 });
    }
    if divide_integer(op).is_some()
        || common(op).is_some()
        || decimal(op).is_some()
        || decimal_bits(op).is_some()
        || decimal_packed(op).is_some()
        || support(op).is_some()
        || super::convert::instruction(op).is_some()
    {
        Some(4)
    } else if let Some(spec) = hex(op) {
        Some(spec.size)
    } else if register(op).is_some() {
        Some(4)
    } else if memory(op).is_some() {
        Some(6)
    } else {
        Some(match op {
            "LER" | "LDR" => 2,
            "LXR" | "LE" | "LD" | "STE" | "STD" | "LFPC" | "STFPC" | "SFPC" | "EFPC" | "SRNM"
            | "SRNMB" | "SRNMT" => 4,
            _ => return None,
        })
    }
}

pub(crate) const COMMON: &[(&str, u8)] = &[
    ("LPDFR", 0x70),
    ("LNDFR", 0x71),
    ("LCDFR", 0x73),
    ("LDGR", 0xc1),
    ("LGDR", 0xcd),
];
pub(crate) fn common(op: &str) -> Option<u8> {
    COMMON.iter().find(|s| s.0 == op).map(|s| s.1)
}

pub(crate) struct Decimal {
    pub opcode: u8,
    pub width: usize,
    pub destination: usize,
    pub kind: Kind,
    pub arithmetic: bool,
    pub mask: bool,
}
pub(crate) fn decimal(op: &str) -> Option<Decimal> {
    if let Some((opcode, width, destination, mask)) = match op {
        "LDETR" => Some((0xd4, 4, 8, false)),
        "LXDTR" => Some((0xdc, 8, 16, false)),
        "LEDTR" => Some((0xd5, 8, 4, true)),
        "LDXTR" => Some((0xdd, 16, 8, true)),
        "FIDTR" => Some((0xd7, 8, 8, true)),
        "FIXTR" => Some((0xdf, 16, 16, true)),
        _ => None,
    } {
        return Some(Decimal {
            opcode,
            width,
            destination,
            kind: if op == "FIDTR" || op == "FIXTR" {
                Kind::RoundInteger
            } else {
                Kind::Convert
            },
            arithmetic: false,
            mask,
        });
    }
    let mask = op.ends_with('A');
    let root = op.strip_suffix('A').unwrap_or(op);
    let (opcode, width, kind) = match root {
        "ADTR" => (0xd2, 8, Kind::Add),
        "AXTR" => (0xda, 16, Kind::Add),
        "SDTR" => (0xd3, 8, Kind::Subtract),
        "SXTR" => (0xdb, 16, Kind::Subtract),
        "MDTR" => (0xd0, 8, Kind::Multiply),
        "MXTR" => (0xd8, 16, Kind::Multiply),
        "DDTR" => (0xd1, 8, Kind::Divide),
        "DXTR" => (0xd9, 16, Kind::Divide),
        "CEDTR" => (0xf4, 8, Kind::CompareExponent),
        "CEXTR" => (0xfc, 16, Kind::CompareExponent),
        "CDTR" => (0xe4, 8, Kind::Compare),
        "CXTR" => (0xec, 16, Kind::Compare),
        "KDTR" => (0xe0, 8, Kind::SignalCompare),
        "KXTR" => (0xe8, 16, Kind::SignalCompare),
        "LTDTR" => (0xd6, 8, Kind::Test),
        "LTXTR" => (0xde, 16, Kind::Test),
        _ => return None,
    };
    let arithmetic = matches!(
        kind,
        Kind::Add | Kind::Subtract | Kind::Multiply | Kind::Divide
    );
    if mask && !arithmetic {
        return None;
    }
    Some(Decimal {
        opcode,
        width,
        destination: width,
        kind,
        arithmetic,
        mask,
    })
}
pub(crate) fn decimal_name(opcode: u8) -> Option<&'static str> {
    [
        "ADTR", "AXTR", "SDTR", "SXTR", "MDTR", "MXTR", "DDTR", "DXTR", "CDTR", "CXTR", "KDTR",
        "KXTR", "LTDTR", "LTXTR", "CEDTR", "CEXTR", "LDETR", "LXDTR", "LEDTR", "LDXTR", "FIDTR",
        "FIXTR",
    ]
    .into_iter()
    .find(|op| decimal(op).is_some_and(|s| s.opcode == opcode))
}

pub(crate) struct Hex {
    pub opcode: u16,
    pub size: u32,
    pub width: usize,
    pub destination: usize,
    pub kind: Kind,
    pub memory: bool,
    pub unnormalized: bool,
}
pub(crate) fn hex(op: &str) -> Option<Hex> {
    use Kind::*;
    let (opcode, width, destination, kind, memory, unnormalized) = match op {
        "MAER" => (0xb32e, 4, 4, FusedAdd, false, false),
        "MADR" => (0xb33e, 8, 8, FusedAdd, false, false),
        "MSER" => (0xb32f, 4, 4, FusedSubtract, false, false),
        "MSDR" => (0xb33f, 8, 8, FusedSubtract, false, false),
        "MAE" => (0xed2e, 4, 4, FusedAdd, true, false),
        "MAD" => (0xed3e, 8, 8, FusedAdd, true, false),
        "MSE" => (0xed2f, 4, 4, FusedSubtract, true, false),
        "MSD" => (0xed3f, 8, 8, FusedSubtract, true, false),
        "FIER" => (0xb377, 4, 4, RoundInteger, false, false),
        "FIDR" => (0xb37f, 8, 8, RoundInteger, false, false),
        "FIXR" => (0xb367, 16, 16, RoundInteger, false, false),
        "LDER" => (0xb324, 4, 8, Lengthen, false, false),
        "LXDR" => (0xb325, 8, 16, Lengthen, false, false),
        "LXER" => (0xb326, 4, 16, Lengthen, false, false),
        "LDE" => (0xed24, 4, 8, Lengthen, true, false),
        "LXD" => (0xed25, 8, 16, Lengthen, true, false),
        "LXE" => (0xed26, 4, 16, Lengthen, true, false),
        "HER" => (0x34, 4, 4, Half, false, false),
        "HDR" => (0x24, 8, 8, Half, false, false),
        "SQER" => (0xb245, 4, 4, Sqrt, false, false),
        "SQDR" => (0xb244, 8, 8, Sqrt, false, false),
        "SQXR" => (0xb336, 16, 16, Sqrt, false, false),
        "SQE" => (0xed34, 4, 4, Sqrt, true, false),
        "SQD" => (0xed35, 8, 8, Sqrt, true, false),
        "AER" => (0x3a, 4, 4, Add, false, false),
        "ADR" => (0x2a, 8, 8, Add, false, false),
        "AXR" => (0x36, 16, 16, Add, false, false),
        "SER" => (0x3b, 4, 4, Subtract, false, false),
        "SDR" => (0x2b, 8, 8, Subtract, false, false),
        "SXR" => (0x37, 16, 16, Subtract, false, false),
        "AUR" => (0x3e, 4, 4, Add, false, true),
        "AWR" => (0x2e, 8, 8, Add, false, true),
        "SUR" => (0x3f, 4, 4, Subtract, false, true),
        "SWR" => (0x2f, 8, 8, Subtract, false, true),
        "CER" => (0x39, 4, 4, Compare, false, false),
        "CDR" => (0x29, 8, 8, Compare, false, false),
        "CXR" => (0xb369, 16, 16, Compare, false, false),
        "MER" | "MDER" => (0x3c, 4, 8, Multiply, false, false),
        "MDR" => (0x2c, 8, 8, Multiply, false, false),
        "MXR" => (0x26, 16, 16, Multiply, false, false),
        "MXDR" => (0x27, 8, 16, Multiply, false, false),
        "MEER" => (0xb337, 4, 4, Multiply, false, false),
        "DER" => (0x3d, 4, 4, Divide, false, false),
        "DDR" => (0x2d, 8, 8, Divide, false, false),
        "DXR" => (0xb22d, 16, 16, Divide, false, false),
        "LPER" => (0x30, 4, 4, Positive, false, false),
        "LPDR" => (0x20, 8, 8, Positive, false, false),
        "LPXR" => (0xb360, 16, 16, Positive, false, false),
        "LNER" => (0x31, 4, 4, Negative, false, false),
        "LNDR" => (0x21, 8, 8, Negative, false, false),
        "LNXR" => (0xb361, 16, 16, Negative, false, false),
        "LCER" => (0x33, 4, 4, Complement, false, false),
        "LCDR" => (0x23, 8, 8, Complement, false, false),
        "LCXR" => (0xb363, 16, 16, Complement, false, false),
        "LTER" => (0x32, 4, 4, Test, false, false),
        "LTDR" => (0x22, 8, 8, Test, false, false),
        "LTXR" => (0xb362, 16, 16, Test, false, false),
        "LRER" | "LEDR" => (0x35, 8, 4, Convert, false, true),
        "LRDR" | "LDXR" => (0x25, 16, 8, Convert, false, true),
        "LEXR" => (0xb366, 16, 4, Convert, false, true),
        "AE" => (0x7a, 4, 4, Add, true, false),
        "AD" => (0x6a, 8, 8, Add, true, false),
        "SE" => (0x7b, 4, 4, Subtract, true, false),
        "SD" => (0x6b, 8, 8, Subtract, true, false),
        "AU" => (0x7e, 4, 4, Add, true, true),
        "AW" => (0x6e, 8, 8, Add, true, true),
        "SU" => (0x7f, 4, 4, Subtract, true, true),
        "SW" => (0x6f, 8, 8, Subtract, true, true),
        "CE" => (0x79, 4, 4, Compare, true, false),
        "CD" => (0x69, 8, 8, Compare, true, false),
        "ME" | "MDE" => (0x7c, 4, 8, Multiply, true, false),
        "MD" => (0x6c, 8, 8, Multiply, true, false),
        "MXD" => (0x67, 8, 16, Multiply, true, false),
        "MEE" => (0xed37, 4, 4, Multiply, true, false),
        "DE" => (0x7d, 4, 4, Divide, true, false),
        "DD" => (0x6d, 8, 8, Divide, true, false),
        _ => return None,
    };
    Some(Hex {
        opcode,
        size: if memory {
            if opcode > 255 { 6 } else { 4 }
        } else if opcode > 255 {
            4
        } else {
            2
        },
        width,
        destination,
        kind,
        memory,
        unnormalized,
    })
}
pub(crate) fn check_register(r: u8, width: usize) -> Result<(), String> {
    if r > 15 || width == 16 && r & 2 != 0 {
        Err("extended floating-point register must be 0,1,4,5,8,9,12 or 13".into())
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DecimalBitKind {
    Exponent,
    Significance,
    Insert,
    Quantize,
    Reround,
}
pub(crate) struct DecimalBits {
    pub opcode: u8,
    pub width: usize,
    pub kind: DecimalBitKind,
}
pub(crate) fn decimal_bits(op: &str) -> Option<DecimalBits> {
    use DecimalBitKind::*;
    let (opcode, width, kind) = match op {
        "EEDTR" => (0xe5, 8, Exponent),
        "EEXTR" => (0xed, 16, Exponent),
        "ESDTR" => (0xe7, 8, Significance),
        "ESXTR" => (0xef, 16, Significance),
        "QADTR" => (0xf5, 8, Quantize),
        "QAXTR" => (0xfd, 16, Quantize),
        "RRDTR" => (0xf7, 8, Reround),
        "RRXTR" => (0xff, 16, Reround),
        "IEDTR" => (0xf6, 8, Insert),
        "IEXTR" => (0xfe, 16, Insert),
        _ => return None,
    };
    Some(DecimalBits {
        opcode,
        width,
        kind,
    })
}
pub(crate) fn decimal_bits_name(opcode: u8) -> Option<&'static str> {
    [
        "EEDTR", "EEXTR", "ESDTR", "ESXTR", "IEDTR", "IEXTR", "QADTR", "QAXTR", "RRDTR", "RRXTR",
    ]
    .into_iter()
    .find(|name| decimal_bits(name).is_some_and(|s| s.opcode == opcode))
}

pub(crate) fn hex_name(opcode: u16) -> Option<&'static str> {
    [
        "CXR", "MEER", "DXR", "LPXR", "LNXR", "LCXR", "LTXR", "LEXR", "SQER", "SQDR", "SQXR",
        "MAER", "MADR", "MSER", "MSDR", "FIER", "FIDR", "FIXR", "LDER", "LXDR", "LXER", "MAE",
        "MAD", "MSE", "MSD", "LDE", "LXD", "LXE", "SQE", "SQD", "MEE",
    ]
    .into_iter()
    .find(|op| hex(op).is_some_and(|s| s.opcode == opcode))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum HexPart {
    Full,
    High,
    Low,
}
pub(crate) struct HexUnnormalized {
    pub opcode: u16,
    pub memory: bool,
    pub add: bool,
    pub part: HexPart,
}
pub(crate) fn hex_unnormalized(op: &str) -> Option<HexUnnormalized> {
    use HexPart::*;
    let (opcode, memory, add, part) = match op {
        "MAYR" => (0xb33a, false, true, Full),
        "MAYHR" => (0xb33c, false, true, High),
        "MAYLR" => (0xb338, false, true, Low),
        "MAY" => (0xed3a, true, true, Full),
        "MAYH" => (0xed3c, true, true, High),
        "MAYL" => (0xed38, true, true, Low),
        "MYR" => (0xb33b, false, false, Full),
        "MYHR" => (0xb33d, false, false, High),
        "MYLR" => (0xb339, false, false, Low),
        "MY" => (0xed3b, true, false, Full),
        "MYH" => (0xed3d, true, false, High),
        "MYL" => (0xed39, true, false, Low),
        _ => return None,
    };
    Some(HexUnnormalized {
        opcode,
        memory,
        add,
        part,
    })
}
pub(crate) fn hex_unnormalized_name(opcode: u16) -> Option<&'static str> {
    [
        "MAYR", "MAYHR", "MAYLR", "MYR", "MYHR", "MYLR", "MAY", "MAYH", "MAYL", "MY", "MYH", "MYL",
    ]
    .into_iter()
    .find(|op| hex_unnormalized(op).is_some_and(|s| s.opcode == opcode))
}

pub(crate) struct DecimalPacked {
    pub opcode: u8,
    pub width: usize,
    pub to_decimal: bool,
    pub signed: bool,
}
impl DecimalPacked {
    pub fn mask(&self) -> bool {
        !self.to_decimal && self.signed
    }
}
pub(crate) fn decimal_packed(op: &str) -> Option<DecimalPacked> {
    let (opcode, width, to_decimal, signed) = match op {
        "CDSTR" => (0xf3, 8, true, true),
        "CXSTR" => (0xfb, 16, true, true),
        "CDUTR" => (0xf2, 8, true, false),
        "CXUTR" => (0xfa, 16, true, false),
        "CSDTR" => (0xe3, 8, false, true),
        "CSXTR" => (0xeb, 16, false, true),
        "CUDTR" => (0xe2, 8, false, false),
        "CUXTR" => (0xea, 16, false, false),
        _ => return None,
    };
    Some(DecimalPacked {
        opcode,
        width,
        to_decimal,
        signed,
    })
}
pub(crate) fn decimal_packed_name(opcode: u8) -> Option<&'static str> {
    [
        "CDSTR", "CXSTR", "CDUTR", "CXUTR", "CSDTR", "CSXTR", "CUDTR", "CUXTR",
    ]
    .into_iter()
    .find(|op| decimal_packed(op).is_some_and(|s| s.opcode == opcode))
}

pub(crate) struct DecimalMemory {
    pub width: usize,
    pub to_decimal: bool,
    pub zoned: bool,
}
impl DecimalMemory {
    pub fn max_length(&self) -> u32 {
        if self.zoned {
            if self.width == 8 { 16 } else { 34 }
        } else if self.width == 8 {
            9
        } else {
            18
        }
    }
}
pub(crate) fn decimal_memory(op: &str) -> Option<DecimalMemory> {
    let (width, to_decimal, zoned) = match op {
        "CDPT" => (8, true, false),
        "CXPT" => (16, true, false),
        "CPDT" => (8, false, false),
        "CPXT" => (16, false, false),
        "CDZT" => (8, true, true),
        "CXZT" => (16, true, true),
        "CZDT" => (8, false, true),
        "CZXT" => (16, false, true),
        _ => return None,
    };
    Some(DecimalMemory {
        width,
        to_decimal,
        zoned,
    })
}

pub(crate) fn binary_controlled(op: &str) -> bool {
    register(op).is_some_and(|(_, d, s, k)| k == Kind::RoundInteger || k == Kind::Convert && d < s)
}
pub(crate) fn binary_masks(op: &str) -> Option<bool> {
    if binary_controlled(op)
        && (op.ends_with('A') || register(op).is_some_and(|(_, _, _, k)| k == Kind::RoundInteger))
    {
        Some(op.ends_with('A'))
    } else {
        None
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SupportKind {
    BinaryToHex,
    HexToBinary,
    CopySign,
}
pub(crate) struct Support {
    pub opcode: u8,
    pub source: usize,
    pub destination: usize,
    pub kind: SupportKind,
}
pub(crate) fn support(op: &str) -> Option<Support> {
    use SupportKind::*;
    let (opcode, source, destination, kind) = match op {
        "THDER" => (0x58, 4, 8, BinaryToHex),
        "THDR" => (0x59, 8, 8, BinaryToHex),
        "TBEDR" => (0x50, 8, 4, HexToBinary),
        "TBDR" => (0x51, 8, 8, HexToBinary),
        "CPSDR" => (0x72, 8, 8, CopySign),
        _ => return None,
    };
    Some(Support {
        opcode,
        source,
        destination,
        kind,
    })
}
pub(crate) fn support_name(opcode: u8) -> Option<&'static str> {
    ["THDER", "THDR", "TBEDR", "TBDR", "CPSDR"]
        .into_iter()
        .find(|op| support(op).is_some_and(|s| s.opcode == opcode))
}

pub(crate) struct Classify {
    pub width: usize,
    pub decimal: bool,
    pub group: bool,
}
pub(crate) fn classify(op: &str) -> Option<Classify> {
    let (width, decimal, group) = match op {
        "TCEB" => (4, false, false),
        "TCDB" => (8, false, false),
        "TCXB" => (16, false, false),
        "TDCET" => (4, true, false),
        "TDCDT" => (8, true, false),
        "TDCXT" => (16, true, false),
        "TDGET" => (4, true, true),
        "TDGDT" => (8, true, true),
        "TDGXT" => (16, true, true),
        _ => return None,
    };
    Some(Classify {
        width,
        decimal,
        group,
    })
}
pub(crate) struct DecimalShift {
    pub width: usize,
    pub left: bool,
}
pub(crate) fn decimal_shift(op: &str) -> Option<DecimalShift> {
    let (width, left) = match op {
        "SLDT" => (8, true),
        "SLXT" => (16, true),
        "SRDT" => (8, false),
        "SRXT" => (16, false),
        _ => return None,
    };
    Some(DecimalShift { width, left })
}

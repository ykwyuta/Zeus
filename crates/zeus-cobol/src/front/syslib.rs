//! System COPY members supplied with the compiler (CICS, Db2 and Language
//! Environment interface definitions). Written for Zeus from the
//! documented interface values: 3270 AID codes, 3270 field attribute
//! bytes and extended attribute values, and the SQLCA / SQLDA layouts. Members in
//! the COPY search directories take precedence.

/// 3270 attribute byte bits (in the 6-bit form) to the EBCDIC graphic
/// character the attribute is written as.
fn attr_char(bits: u8) -> u8 {
    const TABLE: [u8; 64] = [
        0x40, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0x4A, 0x4B, 0x4C, 0x4D, 0x4E,
        0x4F, 0x50, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0x5A, 0x5B, 0x5C, 0x5D,
        0x5E, 0x5F, 0x60, 0x61, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0x6A, 0x6B, 0x6C,
        0x6D, 0x6E, 0x6F, 0xF0, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0x7A, 0x7B,
        0x7C, 0x7D, 0x7E, 0x7F,
    ];
    TABLE[(bits & 0x3F) as usize]
}

// attribute bits
const PROT: u8 = 0x20;
const NUM: u8 = 0x10;
const BRT: u8 = 0x08;
const DRK: u8 = 0x0C;
const MDT: u8 = 0x01;

/// A group `name` of one-byte constants.
fn entries(name: &str, items: &[(&str, u8)]) -> String {
    let mut s = format!("       01 {name}.\n");
    for (n, v) in items {
        s.push_str(&format!("          02 {n} PIC X VALUE X'{v:02X}'.\n"));
    }
    s
}

fn dfhaid() -> String {
    let mut v: Vec<(String, u8)> = vec![
        ("DFHNULL".into(), 0x00),
        ("DFHENTER".into(), 0x7D),
        ("DFHCLEAR".into(), 0x6D),
        ("DFHCLRP".into(), 0x6A),
        ("DFHPEN".into(), 0x7E),
        ("DFHOPID".into(), 0xE6),
        ("DFHMSRE".into(), 0xE7),
        ("DFHSTRF".into(), 0x88),
        ("DFHTRIG".into(), 0x7F),
        ("DFHPA1".into(), 0x6C),
        ("DFHPA2".into(), 0x6E),
        ("DFHPA3".into(), 0x6B),
    ];
    let pf = [
        0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0x7A, 0x7B, 0x7C, 0xC1, 0xC2, 0xC3,
        0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0x4A, 0x4B, 0x4C,
    ];
    for (i, b) in pf.iter().enumerate() {
        v.push((format!("DFHPF{}", i + 1), *b));
    }
    let v: Vec<(&str, u8)> = v.iter().map(|(n, b)| (n.as_str(), *b)).collect();
    entries("DFHAID", &v)
}

fn dfhbmsca() -> String {
    let a = attr_char;
    entries(
        "DFHBMSCA",
        &[
            // field attributes
            ("DFHBMUNP", a(0)),
            ("DFHBMUNN", a(NUM)),
            ("DFHBMPRO", a(PROT)),
            ("DFHBMASK", a(PROT | NUM)),
            ("DFHBMBRY", a(BRT)),
            ("DFHBMDAR", a(DRK)),
            ("DFHBMFSE", a(MDT)),
            ("DFHBMPRF", a(PROT | MDT)),
            ("DFHBMASF", a(PROT | NUM | MDT)),
            ("DFHBMASB", a(PROT | NUM | BRT)),
            ("DFHUNNOD", a(DRK | MDT)),
            ("DFHUNIMD", a(BRT | MDT)),
            ("DFHUNNUM", a(NUM | MDT)),
            ("DFHUNNUB", a(NUM | BRT)),
            ("DFHUNINT", a(NUM | BRT | MDT)),
            ("DFHUNNON", a(NUM | DRK | MDT)),
            ("DFHPROTI", a(PROT | BRT)),
            ("DFHPROTN", a(PROT | DRK)),
            // printer orders
            ("DFHBMPEM", 0x19),
            ("DFHBMPNL", 0x15),
            ("DFHBMPFF", 0x0C),
            ("DFHBMPCR", 0x0D),
            ("DFHBMPSO", 0x0E),
            ("DFHBMPSI", 0x0F),
            // flag byte
            ("DFHBMEOF", 0x80),
            ("DFHBMCUR", 0x02),
            ("DFHBMEC", 0x82),
            // extended attributes: default, colour, highlighting
            ("DFHDFCOL", 0x00),
            ("DFHBLUE", 0xF1),
            ("DFHRED", 0xF2),
            ("DFHPINK", 0xF3),
            ("DFHGREEN", 0xF4),
            ("DFHTURQ", 0xF5),
            ("DFHYELLO", 0xF6),
            ("DFHNEUTR", 0xF7),
            ("DFHDFHI", 0x00),
            ("DFHBLINK", 0xF1),
            ("DFHREVRS", 0xF2),
            ("DFHUNDLN", 0xF4),
        ],
    )
}

const SQLCA: &str = "
       01 SQLCA.
          05 SQLCAID PIC X(8).
          05 SQLCABC PIC S9(9) COMP-5.
          05 SQLCODE PIC S9(9) COMP-5.
          05 SQLERRM.
             49 SQLERRML PIC S9(4) COMP-5.
             49 SQLERRMC PIC X(70).
          05 SQLERRP PIC X(8).
          05 SQLERRD OCCURS 6 TIMES PIC S9(9) COMP-5.
          05 SQLWARN.
             10 SQLWARN0 PIC X.
             10 SQLWARN1 PIC X.
             10 SQLWARN2 PIC X.
             10 SQLWARN3 PIC X.
             10 SQLWARN4 PIC X.
             10 SQLWARN5 PIC X.
             10 SQLWARN6 PIC X.
             10 SQLWARN7 PIC X.
          05 SQLEXT.
             10 SQLWARN8 PIC X.
             10 SQLWARN9 PIC X.
             10 SQLWARNA PIC X.
             10 SQLSTATE PIC X(5).
";

const SQLDA: &str = "
       01 SQLDA.
          05 SQLDAID PIC X(8).
          05 SQLDABC PIC S9(9) COMP-5.
          05 SQLN PIC S9(4) COMP-5.
          05 SQLD PIC S9(4) COMP-5.
          05 SQLVAR OCCURS 0 TO 750 TIMES DEPENDING ON SQLN.
             10 SQLTYPE PIC S9(4) COMP-5.
             10 SQLLEN PIC S9(4) COMP-5.
             10 SQLDATA POINTER.
             10 SQLIND POINTER.
             10 SQLNAME.
                49 SQLNAMEL PIC S9(4) COMP-5.
                49 SQLNAMEC PIC X(30).
";

/// LE condition token: `CEE000` is the successful (all zero) token.
const CEEIGZCT: &str = "
              88 CEE000 VALUE LOW-VALUE.
";

/// Source text of a PL/I include member of the system library (the MQ
/// interface: CMQP, CMQEPP, CMQMDP, ...).
pub fn pli_member(name: &str) -> Option<String> {
    super::syslib_mq::pli_member(name)
}

/// Source text of a system member, in fixed format.
pub fn member(name: &str) -> Option<String> {
    Some(match name {
        "DFHAID" => dfhaid(),
        "DFHBMSCA" => dfhbmsca(),
        "SQLCA" => SQLCA.to_string(),
        "SQLDA" => SQLDA.to_string(),
        "CEEIGZCT" => CEEIGZCT.to_string(),
        _ => return super::syslib_mq::member(name),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_bytes() {
        assert_eq!(attr_char(0), 0x40);
        assert_eq!(attr_char(PROT | NUM), 0xF0);
        assert_eq!(attr_char(MDT), 0xC1);
        assert_eq!(attr_char(PROT | MDT), 0x61);
        assert_eq!(attr_char(DRK), 0x4C);
        assert_eq!(attr_char(PROT | NUM | BRT), 0xF8);
    }
}

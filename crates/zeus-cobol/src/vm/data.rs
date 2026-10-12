//! Conversions between data items in storage and values: numeric decoding
//! and storing (with size-error detection), numeric editing, de-editing and
//! alphanumeric moves. All character data is EBCDIC.

use zeus_decimal::{Dec, Rounding, binary, packed, zoned};

use crate::ebcdic;
use crate::zir::{DataType, EditSym, NumType, NumUsage, SignPos};

pub const SPACE: u8 = ebcdic::SPACE;

fn pow10(n: u32) -> i128 {
    10i128.pow(n)
}

fn sign_position(t: &NumType) -> zoned::SignPosition {
    if !t.signed {
        return zoned::SignPosition::Unsigned;
    }
    match t.sign {
        SignPos::None | SignPos::Trailing => zoned::SignPosition::TrailingOverpunch,
        SignPos::Leading => zoned::SignPosition::LeadingOverpunch,
        SignPos::TrailingSeparate => zoned::SignPosition::TrailingSeparate,
        SignPos::LeadingSeparate => zoned::SignPosition::LeadingSeparate,
    }
}

/// Raw integer held by a numeric item (before applying the scale).
fn raw_value(t: &NumType, bytes: &[u8]) -> i128 {
    match t.usage {
        NumUsage::Display => {
            let (v, neg) = zoned::decode_unchecked(bytes, sign_position(t));
            let v = v as i128;
            if neg && t.signed { -v } else { v }
        }
        NumUsage::Packed => {
            let (v, neg) = packed::decode_unchecked(bytes);
            let v = v as i128;
            if neg && t.signed { -v } else { v }
        }
        NumUsage::Binary | NumUsage::Native => binary::read(bytes, t.signed),
    }
}

/// Applies a picture scale to a raw integer.
pub fn scaled(raw: i128, scale: i32) -> Dec {
    if scale >= 0 {
        Dec::new(raw, scale as u32)
    } else {
        Dec::new(raw * pow10((-scale) as u32), 0)
    }
}

pub fn num_value(t: &NumType, bytes: &[u8]) -> Dec {
    scaled(raw_value(t, bytes), t.scale)
}

/// Numeric value of an item of any numeric-like type.
pub fn value_of(ty: &DataType, bytes: &[u8]) -> Dec {
    match ty {
        DataType::Numeric(t) => num_value(t, bytes),
        DataType::Index | DataType::Pointer => Dec::new(binary::read(bytes, true), 0),
        DataType::Float { .. } => float_dec(zeus_decimal::hfp::decode(bytes)),
        DataType::NumericEdited {
            edit, scale, point, ..
        } => deedit(edit, *scale, *point, bytes),
        // Alphanumeric data used as a number: an unsigned integer.
        _ => alnum_as_number(bytes),
    }
}

/// The decimal closest to a floating-point value (17 significant digits).
pub fn float_dec(f: f64) -> Dec {
    if f == 0.0 || !f.is_finite() {
        return Dec::ZERO;
    }
    let int_digits = f.abs().log10().floor() as i32 + 1;
    let scale = (17 - int_digits).clamp(0, 30) as u32;
    Dec::from_f64(f, scale).unwrap_or(Dec::ZERO).normalize()
}

/// Stores a value in a COMP-1 / COMP-2 field.
pub fn store_float(field: &mut [u8], v: Dec) {
    zeus_decimal::hfp::encode(v.to_f64(), field);
}

/// An alphanumeric field treated as an unsigned numeric integer.
pub fn alnum_as_number(bytes: &[u8]) -> Dec {
    let mut v: i128 = 0;
    for &b in bytes.iter().rev().take(37).collect::<Vec<_>>().iter().rev() {
        v = v * 10 + i128::from((*b & 0xF) % 10);
    }
    Dec::new(v, 0)
}

/// The integer to be stored for value `v` in a field of `scale`, with
/// truncation or rounding of excess fraction digits.
pub fn to_raw(v: Dec, scale: i32, rounded: bool) -> i128 {
    let mode = if rounded {
        Rounding::HalfAwayFromZero
    } else {
        Rounding::Truncate
    };
    if scale >= 0 {
        match v.rescale(scale as u32, mode) {
            Some(d) => d.coef,
            // Too many digits for 128 bits: keep the low-order digits.
            None => {
                let (t, _) = v.truncate_integer(30);
                t.rescale(scale as u32, mode).map(|d| d.coef).unwrap_or(0)
            }
        }
    } else {
        let int = v
            .rescale(0, Rounding::Truncate)
            .map(|d| d.coef)
            .unwrap_or(0);
        let f = pow10((-scale) as u32);
        let mut q = int / f;
        if rounded {
            // round on the full value
            let whole = v
                .rescale(1, Rounding::Truncate)
                .map(|d| d.coef)
                .unwrap_or(0);
            let r = (whole % (f * 10)).abs();
            if r * 2 >= f * 10 {
                q += whole.signum();
            }
        }
        q
    }
}

/// Maximum magnitude (exclusive) of the raw integer of a numeric item.
fn capacity(t: &NumType, len: usize) -> i128 {
    match t.usage {
        NumUsage::Native => {
            let (lo, hi) = binary::range(len, t.signed);
            hi.max(-lo) + 1
        }
        _ => pow10(t.digits.min(38)),
    }
}

/// Stores `v` into a numeric item. Returns true on size error (the value
/// did not fit; high-order digits are truncated).
pub fn store_num(t: &NumType, bytes: &mut [u8], v: Dec, rounded: bool) -> bool {
    let raw = to_raw(v, t.scale, rounded);
    store_raw(t, bytes, raw)
}

/// Whether `v` would cause a size error in the item.
pub fn would_overflow(t: &NumType, len: usize, v: Dec, rounded: bool) -> bool {
    let raw = to_raw(v, t.scale, rounded);
    raw.abs() >= capacity(t, len)
}

pub fn store_raw(t: &NumType, bytes: &mut [u8], raw: i128) -> bool {
    let cap = capacity(t, bytes.len());
    let overflow = raw.abs() >= cap;
    let mut r = raw % cap;
    if !t.signed {
        r = r.abs();
    }
    match t.usage {
        NumUsage::Display => {
            zoned::store(bytes, Dec::new(r, 0), sign_position(t));
        }
        NumUsage::Packed => {
            packed::store(bytes, Dec::new(r, 0), t.signed);
        }
        NumUsage::Binary | NumUsage::Native => binary::write(bytes, r),
    }
    overflow
}

/// Moves alphanumeric data: left-justified with space padding, or right
/// justified (JUSTIFIED RIGHT) with truncation on the left.
pub fn move_alnum(src: &[u8], dst: &mut [u8], just: bool) {
    let n = dst.len();
    if just && src.len() < n {
        let pad = n - src.len();
        dst[..pad].fill(SPACE);
        dst[pad..].copy_from_slice(src);
    } else if just {
        dst.copy_from_slice(&src[src.len() - n..]);
    } else if src.len() >= n {
        dst.copy_from_slice(&src[..n]);
    } else {
        dst[..src.len()].copy_from_slice(src);
        dst[src.len()..].fill(SPACE);
    }
}

/// Fills `dst` with `pattern` repeated.
pub fn fill_pattern(dst: &mut [u8], pattern: &[u8]) {
    if pattern.is_empty() {
        return;
    }
    for (i, b) in dst.iter_mut().enumerate() {
        *b = pattern[i % pattern.len()];
    }
}

/// Unsigned display digits of an integer value (zone F), `n` positions,
/// keeping the low-order digits.
pub fn display_digits(v: i128, n: usize) -> Vec<u8> {
    let mut out = vec![ebcdic::ZERO; n];
    let mut x = v.unsigned_abs();
    for b in out.iter_mut().rev() {
        *b = 0xF0 | (x % 10) as u8;
        x /= 10;
    }
    out
}

fn e(c: u8) -> u8 {
    ebcdic::from_latin1(c)
}

/// Edits `v` into a numeric-edited field. `currency` is the currency
/// symbol as a host character; `point` the decimal-point byte (EBCDIC).
#[allow(clippy::too_many_arguments)]
pub fn edit_num(
    syms: &[EditSym],
    digits: u32,
    scale: i32,
    blank_zero: bool,
    currency: u8,
    point: u8,
    v: Dec,
    out: &mut [u8],
) {
    let raw = to_raw(v, scale, false);
    let n = raw.unsigned_abs() % (pow10(digits.min(38)) as u128);
    let is_zero = n == 0;
    let neg = raw < 0 && !is_zero;
    if blank_zero && is_zero {
        out.fill(SPACE);
        return;
    }
    // Digits for the digit positions, left to right.
    let mut ds = Vec::with_capacity(digits as usize);
    let mut x = n;
    for _ in 0..digits {
        ds.push((x % 10) as u8);
        x /= 10;
    }
    ds.reverse();

    let asterisk = syms.contains(&EditSym::Asterisk);
    let fill = if asterisk { b'*' } else { b' ' };
    let has_nine = syms.contains(&EditSym::Digit);
    let float_ch = syms.iter().find_map(|s| match s {
        EditSym::Floating { ch, .. } => Some(*ch),
        _ => None,
    });
    let float_out = |c: u8| -> u8 {
        match c {
            b'+' => {
                if neg {
                    b'-'
                } else {
                    b'+'
                }
            }
            b'-' => {
                if neg {
                    b'-'
                } else {
                    b' '
                }
            }
            _ => currency,
        }
    };

    let mut res: Vec<u8> = Vec::with_capacity(out.len());
    let mut sig = false;
    // Inside a zero-suppression or floating-insertion region.
    let mut suppressing = false;
    let mut k = 0usize;
    let mut float_pos: Option<usize> = None;
    let mut float_pending = false;
    // Starts significance, placing a pending floating symbol.
    let start_sig =
        |res: &mut Vec<u8>, sig: &mut bool, float_pending: &mut bool, float_pos: Option<usize>| {
            if !*sig {
                *sig = true;
                if *float_pending {
                    if let (Some(p), Some(c)) = (float_pos, float_ch) {
                        res[p] = float_out(c);
                    }
                    *float_pending = false;
                }
            }
        };
    for sym in syms {
        match *sym {
            EditSym::Digit => {
                start_sig(&mut res, &mut sig, &mut float_pending, float_pos);
                res.push(b'0' + ds.get(k).copied().unwrap_or(0));
                k += 1;
            }
            EditSym::ZeroSuppress | EditSym::Asterisk => {
                suppressing = true;
                let d = ds.get(k).copied().unwrap_or(0);
                k += 1;
                if sig || d != 0 {
                    start_sig(&mut res, &mut sig, &mut float_pending, float_pos);
                    res.push(b'0' + d);
                } else {
                    res.push(if *sym == EditSym::Asterisk {
                        b'*'
                    } else {
                        b' '
                    });
                }
            }
            EditSym::Floating { first: true, .. } => {
                suppressing = true;
                float_pos = Some(res.len());
                float_pending = true;
                res.push(b' ');
            }
            EditSym::Floating { first: false, .. } => {
                let d = ds.get(k).copied().unwrap_or(0);
                k += 1;
                if sig || d != 0 {
                    start_sig(&mut res, &mut sig, &mut float_pending, float_pos);
                    res.push(b'0' + d);
                } else {
                    float_pos = Some(res.len());
                    res.push(b' ');
                }
            }
            EditSym::Point => {
                start_sig(&mut res, &mut sig, &mut float_pending, float_pos);
                res.push(0); // placeholder for the point character
            }
            EditSym::Insert(c) => {
                if sig || !suppressing {
                    res.push(c);
                } else {
                    if float_pending {
                        float_pos = Some(res.len());
                    }
                    res.push(fill);
                }
            }
            EditSym::Sign(c) => res.push(float_out(c)),
            EditSym::Credit(cr) => {
                let t: &[u8] = match (neg, cr) {
                    (true, true) => b"CR",
                    (true, false) => b"DB",
                    _ => b"  ",
                };
                res.extend_from_slice(t);
            }
            EditSym::Currency => res.push(currency),
            EditSym::Char => res.push(b' '),
            EditSym::ImpliedPoint | EditSym::Scale => {}
        }
    }
    if is_zero && !has_nine {
        // All digit positions suppressed and the value is zero.
        res.fill(if asterisk { b'*' } else { b' ' });
        if asterisk {
            // the decimal point stays
            let mut pos = 0;
            for sym in syms {
                match sym {
                    EditSym::Point => {
                        res[pos] = 0;
                        pos += 1;
                    }
                    EditSym::Credit(_) => pos += 2,
                    EditSym::ImpliedPoint | EditSym::Scale => {}
                    _ => pos += 1,
                }
            }
        }
    }
    for (o, r) in out
        .iter_mut()
        .zip(res.iter().chain(std::iter::repeat(&b' ')))
    {
        *o = if *r == 0 { point } else { e(*r) };
    }
}

/// De-edits a numeric-edited field into its value.
pub fn deedit(syms: &[EditSym], scale: i32, _point: u8, bytes: &[u8]) -> Dec {
    let mut v: i128 = 0;
    let mut neg = false;
    let mut pos = 0;
    for sym in syms {
        let b = bytes.get(pos).copied().unwrap_or(SPACE);
        match sym {
            EditSym::Digit
            | EditSym::ZeroSuppress
            | EditSym::Asterisk
            | EditSym::Floating { first: false, .. } => {
                let d = if (0xF0..=0xF9).contains(&b) {
                    b - 0xF0
                } else {
                    0
                };
                v = v * 10 + i128::from(d);
                if matches!(sym, EditSym::Floating { .. }) && b == e(b'-') {
                    neg = true;
                }
            }
            EditSym::Floating { first: true, .. } | EditSym::Sign(_) => {
                if b == e(b'-') {
                    neg = true;
                }
            }
            EditSym::Credit(_) if b == e(b'C') || b == e(b'D') => neg = true,
            _ => {}
        }
        pos += match sym {
            EditSym::Credit(_) => 2,
            EditSym::ImpliedPoint | EditSym::Scale => 0,
            _ => 1,
        };
    }
    scaled(if neg { -v } else { v }, scale)
}

/// Alphanumeric-edited move.
pub fn edit_alnum(syms: &[EditSym], src: &[u8], dst: &mut [u8]) {
    let mut it = src.iter();
    for (o, sym) in dst.iter_mut().zip(syms) {
        *o = match sym {
            EditSym::Insert(c) => e(*c),
            _ => it.next().copied().unwrap_or(SPACE),
        };
    }
}

/// The national space (U+0020).
pub const NATIONAL_SPACE: [u8; 2] = [0x00, 0x20];

/// Text as national data (UTF-16 big-endian).
pub fn utf16(text: &str) -> Vec<u8> {
    text.encode_utf16().flat_map(u16::to_be_bytes).collect()
}

/// National data as text.
pub fn from_utf16(b: &[u8]) -> String {
    let units: Vec<u16> = b
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&units)
}

/// Alphanumeric (mixed) data in the code page as national data.
pub fn national_of(b: &[u8]) -> Vec<u8> {
    utf16(&ebcdic::decode(b))
}

/// National data as alphanumeric data in the code page.
pub fn display_of(b: &[u8]) -> Vec<u8> {
    ebcdic::encode(&from_utf16(b))
}

/// A figurative constant's byte as a national character.
pub fn national_fig(byte: u8) -> Vec<u8> {
    match byte {
        0x00 => vec![0x00, 0x00],
        0xFF => vec![0xFF, 0xFF],
        b => utf16(&ebcdic::decode(&[b])),
    }
}

/// Moves national data into a national field: truncated, or padded with
/// national spaces.
pub fn move_national(src: &[u8], out: &mut [u8]) {
    let n = (src.len().min(out.len())) & !1;
    out[..n].copy_from_slice(&src[..n]);
    for pair in out[n..].chunks_mut(2) {
        pair.copy_from_slice(&NATIONAL_SPACE[..pair.len()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sema::picture::analyze;

    fn edited(pic: &str, v: &str) -> String {
        let p = analyze(pic, false, '$').unwrap();
        let mut out = vec![0u8; p.size as usize];
        edit_num(
            &p.edit,
            p.digits,
            p.scale,
            false,
            b'$',
            ebcdic::from_latin1(b'.'),
            Dec::parse(v).unwrap(),
            &mut out,
        );
        String::from_utf8(ebcdic::decode_latin1(&out)).unwrap()
    }

    #[test]
    fn editing() {
        assert_eq!(edited("ZZ,ZZ9.99", "1234.5"), " 1,234.50");
        assert_eq!(edited("ZZ,ZZ9.99", "-0.5"), "     0.50");
        assert_eq!(edited("ZZZZ.ZZ", "0"), "       ");
        assert_eq!(edited("ZZ.ZZ", "0.05"), "  .05");
        assert_eq!(edited("$**.99", "1.5"), "$*1.50");
        assert_eq!(edited("$**.99", "0"), "$**.00");
        assert_eq!(edited("**,**9", "0"), "*****0");
        assert_eq!(edited("**.**", "0"), "**.**");
        assert_eq!(edited("$$99.99CR", "-5.25"), " $05.25CR");
        assert_eq!(edited("$$$9.99", "12.3"), " $12.30");
        assert_eq!(edited("-9(4).99", "-12.5"), "-0012.50");
        assert_eq!(edited("+++9", "-5"), "  -5");
        assert_eq!(edited("---9", "5"), "   5");
        assert_eq!(edited("99/99/99", "123456"), "12/34/56");
        assert_eq!(edited("ZZBZZ0", "1234"), "12 340");
        assert_eq!(edited("$0(10)999", "492"), "$0000000000492");
        assert_eq!(edited("ZZ/ZZ", "5"), "    5");
        assert_eq!(edited("$99.99CR", "12.34"), "$12.34  ");
    }

    #[test]
    fn deediting() {
        let p = analyze("-ZZ9.99", false, '$').unwrap();
        let mut out = vec![0u8; p.size as usize];
        let pt = ebcdic::from_latin1(b'.');
        let v = Dec::parse("-12.50").unwrap();
        edit_num(&p.edit, p.digits, p.scale, false, b'$', pt, v, &mut out);
        assert_eq!(ebcdic::decode_latin1(&out), b"- 12.50");
        assert_eq!(deedit(&p.edit, p.scale, pt, &out), v);
    }

    #[test]
    fn numeric_store() {
        let t = NumType {
            usage: NumUsage::Display,
            digits: 3,
            scale: 1,
            signed: true,
            sign: SignPos::None,
        };
        let mut f = [0u8; 3];
        assert!(!store_num(&t, &mut f, Dec::parse("-12.36").unwrap(), true));
        assert_eq!(num_value(&t, &f), Dec::parse("-12.4").unwrap());
        assert!(store_num(&t, &mut f, Dec::parse("123.4").unwrap(), false));
        assert_eq!(num_value(&t, &f), Dec::parse("23.4").unwrap());
        let p = NumType {
            usage: NumUsage::Display,
            digits: 2,
            scale: -2,
            signed: false,
            sign: SignPos::None,
        };
        let mut g = [0u8; 2];
        store_num(&p, &mut g, Dec::parse("9900").unwrap(), false);
        assert_eq!(num_value(&p, &g), Dec::parse("9900").unwrap());
    }

    #[test]
    fn alnum_moves() {
        let mut d = [0u8; 4];
        move_alnum(&[1, 2], &mut d, false);
        assert_eq!(d, [1, 2, SPACE, SPACE]);
        move_alnum(&[1, 2], &mut d, true);
        assert_eq!(d, [SPACE, SPACE, 1, 2]);
        move_alnum(&[1, 2, 3, 4, 5], &mut d, true);
        assert_eq!(d, [2, 3, 4, 5]);
    }
}

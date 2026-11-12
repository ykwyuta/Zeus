//! Intrinsic functions added in Enterprise COBOL 5 and 6: constants and
//! exponentials, bit and hexadecimal conversion, NUMVAL-F, and the date and
//! time functions on ISO 8601 formats (Language Reference, "Intrinsic
//! functions"). Not compared with z/OS: the ZCVS checks ZA125A hold the
//! values the Language Reference gives.

use zeus_decimal::{Dec, Rounding};

use super::eval::{civil_from_days, days_from_civil};
use super::{RResult, err};

/// Integer date (day 1 = 1601-01-01) to days since 1970-01-01.
fn unix_days(n: i64) -> i64 {
    n - 1 + days_from_civil(1601, 1, 1)
}

fn integer_date(y: i64, m: i64, d: i64) -> i64 {
    days_from_civil(y, m, d) - days_from_civil(1601, 1, 1) + 1
}

fn leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn month_days(y: i64, m: i64) -> i64 {
    match m {
        2 if leap(y) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// E and PI to 34 digits, the precision the Language Reference gives.
pub(super) fn constant(pi: bool) -> Dec {
    let text = if pi {
        "3.141592653589793238462643383279503"
    } else {
        "2.718281828459045235360287471352662"
    };
    Dec::parse(text).expect("constant")
}

/// TEST-DATE-YYYYMMDD: 0 valid, 1 year outside 1601-9999, 2 month, 3 day.
pub(super) fn test_date(n: i64) -> i64 {
    let (y, m, d) = (n / 10000, n / 100 % 100, n % 100);
    if !(1601..=9999).contains(&y) || n < 0 {
        1
    } else if !(1..=12).contains(&m) {
        2
    } else if !(1..=month_days(y, m)).contains(&d) {
        3
    } else {
        0
    }
}

/// TEST-DAY-YYYYDDD: 0 valid, 1 year outside 1601-9999, 2 day of year.
pub(super) fn test_day(n: i64) -> i64 {
    let (y, d) = (n / 1000, n % 1000);
    if !(1601..=9999).contains(&y) || n < 0 {
        1
    } else if !(1..=if leap(y) { 366 } else { 365 }).contains(&d) {
        2
    } else {
        0
    }
}

/// TEST-NUMVAL-F: 0 when `s` is a valid NUMVAL-F argument, otherwise the
/// position of the first character in error (the length plus one when the
/// string ends too early). The form is [sp][+|-][sp]digits[.[digits]] or
/// .digits, then optionally [sp]E[sp][+|-][sp]digits, then [sp].
pub(super) fn test_numval_f(s: &str) -> usize {
    let c: Vec<char> = s.chars().collect();
    let n = c.len();
    let mut i = 0;
    let skip = |i: &mut usize| {
        while *i < n && c[*i] == ' ' {
            *i += 1;
        }
    };
    let at = |i: usize| if i < n { i + 1 } else { n + 1 };
    skip(&mut i);
    if i < n && (c[i] == '+' || c[i] == '-') {
        i += 1;
        skip(&mut i);
    }
    let mut digits = 0;
    let mut point = false;
    while i < n {
        match c[i] {
            '0'..='9' => digits += 1,
            '.' if !point => point = true,
            _ => break,
        }
        i += 1;
    }
    if digits == 0 {
        return at(i);
    }
    skip(&mut i);
    if i < n && c[i] == 'E' {
        i += 1;
        skip(&mut i);
        if i < n && (c[i] == '+' || c[i] == '-') {
            i += 1;
            skip(&mut i);
        }
        let start = i;
        while i < n && c[i].is_ascii_digit() {
            i += 1;
        }
        if i == start {
            return at(i);
        }
        skip(&mut i);
    }
    if i < n { i + 1 } else { 0 }
}

/// NUMVAL-F of a valid argument (TEST-NUMVAL-F is 0).
pub(super) fn numval_f(s: &str) -> RResult<Dec> {
    if test_numval_f(s) != 0 {
        return err(format!(
            "invalid NUMVAL-F argument at character {}",
            test_numval_f(s)
        ));
    }
    let t: String = s.chars().filter(|c| *c != ' ').collect();
    let (mantissa, exponent) = match t.split_once('E') {
        Some((m, e)) => (
            m,
            e.parse::<i64>()
                .map_err(|_| super::RunError("NUMVAL-F exponent".into()))?,
        ),
        None => (t.as_str(), 0),
    };
    let negative = mantissa.starts_with('-');
    let mantissa = mantissa.trim_start_matches(['+', '-']);
    let (int, frac) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{int}{frac}");
    // value = digits * 10**(exponent - frac.len())
    let shift = exponent - frac.len() as i64;
    let text = if shift >= 0 {
        format!("{digits}{}", "0".repeat(shift as usize))
    } else {
        let scale = (-shift) as usize;
        let padded = format!(
            "{}{digits}",
            "0".repeat(scale.saturating_sub(digits.len()) + 1)
        );
        let cut = padded.len() - scale;
        format!("{}.{}", &padded[..cut], &padded[cut..])
    };
    let value = Dec::parse(&text)
        .ok_or_else(|| super::RunError("NUMVAL-F result outside fixed-point range".into()))?;
    Ok(if negative { -value } else { value })
}

/// COMBINED-DATETIME(date, seconds) = date + seconds / 100000.
pub(super) fn combined(date: Dec, seconds: Dec) -> RResult<Dec> {
    seconds
        .checked_div(Dec::from_i64(100000), 20, Rounding::Truncate)
        .and_then(|s| date.checked_add(s))
        .map(|d| d.normalize())
        .ok_or_else(|| super::RunError("COMBINED-DATETIME out of range".into()))
}

/// A field of an ISO 8601 format (Language Reference, "Date and time
/// formats").
#[derive(Debug, Clone, Copy, PartialEq)]
enum Tok {
    Lit(char),
    Year,
    Month,
    Day,
    DayOfYear,
    Week,
    Weekday,
    Hour,
    Minute,
    Second,
    Fraction(usize),
}

impl Tok {
    fn width(self) -> usize {
        match self {
            Tok::Lit(_) | Tok::Weekday => 1,
            Tok::Year => 4,
            Tok::DayOfYear => 3,
            Tok::Fraction(n) => n,
            _ => 2,
        }
    }
}

fn tokens(format: &str) -> RResult<Vec<Tok>> {
    let mut out = Vec::new();
    let mut rest = format;
    while !rest.is_empty() {
        let (tok, used) = if rest.starts_with("YYYY") {
            (Tok::Year, 4)
        } else if rest.starts_with("DDD") {
            (Tok::DayOfYear, 3)
        } else if rest.starts_with("DD") {
            (Tok::Day, 2)
        } else if rest.starts_with("MM") {
            (Tok::Month, 2)
        } else if rest.starts_with("ww") {
            (Tok::Week, 2)
        } else if rest.starts_with('D') {
            (Tok::Weekday, 1)
        } else if rest.starts_with("hh") {
            (Tok::Hour, 2)
        } else if rest.starts_with("mm") {
            (Tok::Minute, 2)
        } else if rest.starts_with("ss") && !matches!(out.last(), Some(Tok::Lit('.' | ','))) {
            (Tok::Second, 2)
        } else if rest.starts_with('s') {
            let n = rest.chars().take_while(|c| *c == 's').count();
            (Tok::Fraction(n), n)
        } else {
            let c = rest.chars().next().unwrap();
            if !matches!(c, '-' | ':' | 'T' | 'W' | '.' | ',' | 'Z') {
                return err(format!("unsupported date/time format {format}"));
            }
            (Tok::Lit(c), c.len_utf8())
        };
        out.push(tok);
        rest = &rest[used..];
    }
    Ok(out)
}

/// The ISO 8601 week date of a civil date: (week-numbering year, week, weekday 1-7).
fn iso_week(y: i64, m: i64, d: i64) -> (i64, i64, i64) {
    let days = days_from_civil(y, m, d);
    // 1970-01-01 was a Thursday (weekday 4)
    let weekday = (days + 3).rem_euclid(7) + 1;
    let thursday = days - weekday + 4;
    let (wy, _, _) = civil_from_days(thursday);
    let week = (thursday - days_from_civil(wy, 1, 1)) / 7 + 1;
    (wy, week, weekday)
}

/// FORMATTED-DATE / the date part of FORMATTED-DATETIME.
fn format_date(toks: &[Tok], date: i64) -> RResult<String> {
    if !(1..=3_067_671).contains(&date) {
        return err("integer date outside 1 through 3067671");
    }
    let (y, m, d) = civil_from_days(unix_days(date));
    let doy = days_from_civil(y, m, d) - days_from_civil(y, 1, 1) + 1;
    let (wy, week, weekday) = iso_week(y, m, d);
    let weekly = toks.contains(&Tok::Week);
    let mut out = String::new();
    for t in toks {
        match *t {
            Tok::Lit(c) => out.push(c),
            Tok::Year => out += &format!("{:04}", if weekly { wy } else { y }),
            Tok::Month => out += &format!("{m:02}"),
            Tok::Day => out += &format!("{d:02}"),
            Tok::DayOfYear => out += &format!("{doy:03}"),
            Tok::Week => out += &format!("{week:02}"),
            Tok::Weekday => out += &format!("{weekday}"),
            _ => return err("a time field in a date format"),
        }
    }
    Ok(out)
}

/// FORMATTED-TIME / the time part of FORMATTED-DATETIME. Fractional
/// seconds are truncated to the digits of the format.
fn format_time(toks: &[Tok], seconds: Dec) -> RResult<String> {
    if seconds.coef < 0 || seconds.to_f64() >= 86400.0 {
        return err("seconds outside 0 through 86399");
    }
    let whole = seconds
        .rescale(0, Rounding::Truncate)
        .map_or(0, |d| d.coef as i64);
    let mut out = String::new();
    for t in toks {
        match *t {
            Tok::Lit(c) => out.push(c),
            Tok::Hour => out += &format!("{:02}", whole / 3600),
            Tok::Minute => out += &format!("{:02}", whole / 60 % 60),
            Tok::Second => out += &format!("{:02}", whole % 60),
            Tok::Fraction(n) => {
                let f = seconds
                    .rescale(n as u32, Rounding::Truncate)
                    .map_or(0, |d| d.coef)
                    .rem_euclid(10i128.pow(n as u32));
                out += &format!("{f:0n$}");
            }
            _ => return err("a date field in a time format"),
        }
    }
    Ok(out)
}

fn split_datetime(toks: &[Tok]) -> (&[Tok], &[Tok]) {
    match toks.iter().position(|t| *t == Tok::Lit('T')) {
        Some(i) => (&toks[..i], &toks[i + 1..]),
        None => (toks, &[]),
    }
}

pub(super) fn formatted_date(format: &str, date: i64) -> RResult<String> {
    format_date(&tokens(format)?, date)
}

pub(super) fn formatted_time(format: &str, seconds: Dec) -> RResult<String> {
    format_time(&tokens(format)?, seconds)
}

pub(super) fn formatted_datetime(format: &str, date: i64, seconds: Dec) -> RResult<String> {
    let toks = tokens(format)?;
    let (d, t) = split_datetime(&toks);
    if t.is_empty() {
        return err(format!("{format} is not a combined date and time format"));
    }
    Ok(format!(
        "{}T{}",
        format_date(d, date)?,
        format_time(t, seconds)?
    ))
}

/// The fields of `text` read by `toks`; Err(position) is the 1-based
/// position of the first character in error (TEST-FORMATTED-DATETIME).
fn parse(toks: &[Tok], text: &[char]) -> Result<Vec<(Tok, i64, usize)>, usize> {
    let mut at = 0;
    let mut fields = Vec::new();
    for t in toks {
        let w = t.width();
        if at + w > text.len() {
            return Err(text.len() + 1);
        }
        let part = &text[at..at + w];
        match *t {
            Tok::Lit(c) => {
                if part[0] != c {
                    return Err(at + 1);
                }
            }
            _ => {
                if let Some(bad) = part.iter().position(|c| !c.is_ascii_digit()) {
                    return Err(at + bad + 1);
                }
                let v: i64 = part.iter().collect::<String>().parse().unwrap();
                fields.push((*t, v, at + 1));
            }
        }
        at += w;
    }
    if at < text.len() {
        return Err(at + 1);
    }
    // values in range; an invalid value is reported at its first digit
    let get = |k: Tok| fields.iter().find(|f| f.0 == k).map(|f| (f.1, f.2));
    let year = get(Tok::Year);
    if let Some((y, p)) = year
        && !(1601..=9999).contains(&y)
    {
        return Err(p);
    }
    let y = year.map_or(2000, |v| v.0);
    if let Some((m, p)) = get(Tok::Month)
        && !(1..=12).contains(&m)
    {
        return Err(p);
    }
    if let Some((d, p)) = get(Tok::Day) {
        let m = get(Tok::Month).map_or(1, |v| v.0);
        if !(1..=month_days(y, m)).contains(&d) {
            return Err(p);
        }
    }
    if let Some((d, p)) = get(Tok::DayOfYear)
        && !(1..=if leap(y) { 366 } else { 365 }).contains(&d)
    {
        return Err(p);
    }
    if let Some((w, p)) = get(Tok::Week) {
        let last = iso_week(y, 12, 28).1;
        if !(1..=last).contains(&w) {
            return Err(p);
        }
    }
    if let Some((d, p)) = get(Tok::Weekday)
        && !(1..=7).contains(&d)
    {
        return Err(p);
    }
    for (k, max) in [(Tok::Hour, 23), (Tok::Minute, 59), (Tok::Second, 59)] {
        if let Some((v, p)) = get(k)
            && v > max
        {
            return Err(p);
        }
    }
    Ok(fields)
}

pub(super) fn test_formatted_datetime(format: &str, text: &str) -> RResult<i64> {
    let toks = tokens(format)?;
    let text: Vec<char> = text.chars().collect();
    Ok(match parse(&toks, &text) {
        Ok(_) => 0,
        Err(p) => p as i64,
    })
}

pub(super) fn integer_of_formatted_date(format: &str, text: &str) -> RResult<i64> {
    let toks = tokens(format)?;
    let chars: Vec<char> = text.chars().collect();
    let fields = parse(&toks, &chars)
        .map_err(|p| super::RunError(format!("invalid formatted date at character {p}")))?;
    let get = |k: Tok| fields.iter().find(|f| f.0 == k).map(|f| f.1);
    let y = get(Tok::Year).ok_or_else(|| super::RunError("date format without YYYY".into()))?;
    Ok(if let Some(w) = get(Tok::Week) {
        // Monday of week 1 is the Monday on or before January 4
        let jan4 = days_from_civil(y, 1, 4);
        let monday = jan4 - ((jan4 + 3).rem_euclid(7));
        let days = monday + (w - 1) * 7 + get(Tok::Weekday).unwrap_or(1) - 1;
        days - days_from_civil(1601, 1, 1) + 1
    } else if let Some(d) = get(Tok::DayOfYear) {
        integer_date(y, 1, 1) + d - 1
    } else {
        integer_date(y, get(Tok::Month).unwrap_or(1), get(Tok::Day).unwrap_or(1))
    })
}

pub(super) fn seconds_from_formatted_time(format: &str, text: &str) -> RResult<Dec> {
    let toks = tokens(format)?;
    let chars: Vec<char> = text.chars().collect();
    let fields = parse(&toks, &chars)
        .map_err(|p| super::RunError(format!("invalid formatted time at character {p}")))?;
    let get = |k: Tok| fields.iter().find(|f| f.0 == k).map(|f| f.1).unwrap_or(0);
    let whole = Dec::from_i64(get(Tok::Hour) * 3600 + get(Tok::Minute) * 60 + get(Tok::Second));
    Ok(
        match fields.iter().find(|f| matches!(f.0, Tok::Fraction(_))) {
            Some(&(Tok::Fraction(n), v, _)) => whole
                .checked_add(Dec::new(v as i128, n as u32))
                .unwrap_or(whole),
            _ => whole,
        },
    )
}

pub(super) fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

pub(super) fn bit_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:08b}")).collect()
}

pub(super) fn hex_to_char(text: &str) -> RResult<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return err("HEX-TO-CHAR needs an even number of hexadecimal digits");
    }
    (0..text.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&text[i..i + 2], 16)
                .map_err(|_| super::RunError("HEX-TO-CHAR argument is not hexadecimal".into()))
        })
        .collect()
}

pub(super) fn bit_to_char(text: &str) -> RResult<Vec<u8>> {
    if !text.len().is_multiple_of(8) {
        return err("BIT-TO-CHAR needs a multiple of eight bits");
    }
    (0..text.len())
        .step_by(8)
        .map(|i| {
            u8::from_str_radix(&text[i..i + 8], 2)
                .map_err(|_| super::RunError("BIT-TO-CHAR argument is not 0s and 1s".into()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_reference_values() {
        assert_eq!(formatted_date("YYYY-MM-DD", 1).unwrap(), "1601-01-01");
        assert_eq!(formatted_date("YYYYDDD", 365).unwrap(), "1601365");
        // 1601-01-01 is a Monday in ISO week 1
        assert_eq!(formatted_date("YYYYWwwD", 1).unwrap(), "1601W011");
        assert_eq!(
            formatted_time("hhmmss", Dec::from_i64(3661)).unwrap(),
            "010101"
        );
        assert_eq!(
            formatted_time("hh:mm:ss.ss", Dec::parse("86399.987").unwrap()).unwrap(),
            "23:59:59.98"
        );
        assert_eq!(
            formatted_datetime("YYYYMMDDThhmmss", 1, Dec::from_i64(61)).unwrap(),
            "16010101T000101"
        );
        assert_eq!(
            integer_of_formatted_date("YYYY-MM-DD", "1601-01-02").unwrap(),
            2
        );
        assert_eq!(
            integer_of_formatted_date("YYYYWwwD", "1601W011").unwrap(),
            1
        );
        assert_eq!(
            integer_of_formatted_date("YYYYDDD", "1601365").unwrap(),
            365
        );
        assert_eq!(
            seconds_from_formatted_time("hhmmss", "010101").unwrap(),
            Dec::from_i64(3661)
        );
        assert_eq!(
            test_formatted_datetime("YYYY-MM-DD", "2024-02-29").unwrap(),
            0
        );
        assert_eq!(
            test_formatted_datetime("YYYY-MM-DD", "2024/02/29").unwrap(),
            5
        );
        assert_eq!(
            test_formatted_datetime("YYYY-MM-DD", "2023-02-29").unwrap(),
            9
        );
        assert_eq!(test_formatted_datetime("YYYY-MM-DD", "2023-02").unwrap(), 8);
        assert_eq!(test_date(20240229), 0);
        assert_eq!(test_date(15001231), 1);
        assert_eq!(test_date(20241301), 2);
        assert_eq!(test_date(20230229), 3);
        assert_eq!(test_day(2024366), 0);
        assert_eq!(test_day(2023366), 2);
        assert_eq!(test_numval_f("1.5E+02"), 0);
        assert_eq!(test_numval_f(" -2.5E-1 "), 0);
        assert_eq!(test_numval_f("1.5X"), 4);
        assert_eq!(test_numval_f("1.5E"), 5);
        assert_eq!(numval_f(" 1.5E+02 ").unwrap(), Dec::from_i64(150));
        assert_eq!(numval_f("-2.5E-1").unwrap(), Dec::parse("-0.25").unwrap());
        assert_eq!(hex_of(&[0xc1, 0xc2]), "C1C2");
        assert_eq!(bit_of(&[0xc1]), "11000001");
        assert_eq!(hex_to_char("C1C2").unwrap(), vec![0xc1, 0xc2]);
        assert_eq!(bit_to_char("1100000111000010").unwrap(), vec![0xc1, 0xc2]);
        assert_eq!(
            combined(Dec::from_i64(1), Dec::from_i64(3661)).unwrap(),
            Dec::parse("1.03661").unwrap()
        );
    }
}

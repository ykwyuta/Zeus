//! Date/time patterns of the PL/I date/time built-in functions (Enterprise
//! PL/I Language Reference, Table 65 and Table 66): YYYY, YY, ZY, MM, ZM,
//! MMM, Mmm, DD, ZD, DDD, HH, MI, SS, 999 and 999999; other characters are
//! taken literally. Values are counted in Lilian form: day 1 is 15 October
//! 1582, so day 0 (14 October 1582) is second and microsecond 0.

/// A date and time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub y: i64,
    pub m: u32,
    pub d: u32,
    pub h: u32,
    pub mi: u32,
    pub s: u32,
    pub micro: u32,
}

const MONTHS: [&str; 12] = [
    "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tok {
    Lit(char),
    Yyyy,
    Yy,
    Zy,
    Mm,
    Zm,
    MonUpper,
    MonMixed,
    Dd,
    Zd,
    Ddd,
    Hh,
    Mi,
    Ss,
    Milli,
    Micro,
}

fn tokens(p: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut r = p;
    while !r.is_empty() {
        let (t, n) = if r.starts_with("YYYY") {
            (Tok::Yyyy, 4)
        } else if r.starts_with("YY") {
            (Tok::Yy, 2)
        } else if r.starts_with("ZY") {
            (Tok::Zy, 2)
        } else if r.starts_with("MMM") {
            (Tok::MonUpper, 3)
        } else if r.starts_with("Mmm") {
            (Tok::MonMixed, 3)
        } else if r.starts_with("MM") {
            (Tok::Mm, 2)
        } else if r.starts_with("ZM") {
            (Tok::Zm, 2)
        } else if r.starts_with("DDD") {
            (Tok::Ddd, 3)
        } else if r.starts_with("DD") {
            (Tok::Dd, 2)
        } else if r.starts_with("ZD") {
            (Tok::Zd, 2)
        } else if r.starts_with("HH") {
            (Tok::Hh, 2)
        } else if r.starts_with("MI") {
            (Tok::Mi, 2)
        } else if r.starts_with("SS") {
            (Tok::Ss, 2)
        } else if r.starts_with("999999") {
            (Tok::Micro, 6)
        } else if r.starts_with("999") {
            (Tok::Milli, 3)
        } else {
            let c = r.chars().next().unwrap();
            (Tok::Lit(c), c.len_utf8())
        };
        out.push(t);
        r = &r[n..];
    }
    out
}

/// Days since 1970-01-01 of a civil date.
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let (y, m) = if m <= 2 {
        (y - 1, i64::from(m) + 9)
    } else {
        (y, i64::from(m) - 3)
    };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * m + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Civil date of days since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// Lilian day (1 = 15 October 1582) of 1970-01-01 is 141428.
pub const LILIAN_1970: i64 = 141_428;

pub fn lilian_day(y: i64, m: u32, d: u32) -> i64 {
    days_from_civil(y, m, d) + LILIAN_1970
}

impl Stamp {
    /// Lilian microseconds.
    pub fn micros(&self) -> i64 {
        let day = lilian_day(self.y, self.m, self.d);
        ((day * 86_400 + i64::from(self.h) * 3600 + i64::from(self.mi) * 60 + i64::from(self.s))
            * 1_000_000)
            + i64::from(self.micro)
    }

    pub fn from_micros(us: i64) -> Stamp {
        let day = us.div_euclid(86_400_000_000);
        let rem = us.rem_euclid(86_400_000_000);
        let (y, m, d) = civil_from_days(day - LILIAN_1970);
        let secs = rem / 1_000_000;
        Stamp {
            y,
            m,
            d,
            h: (secs / 3600) as u32,
            mi: (secs / 60 % 60) as u32,
            s: (secs % 60) as u32,
            micro: (rem % 1_000_000) as u32,
        }
    }
}

/// The text of a stamp in a pattern. A pattern "without zeros" is padded on
/// the right to 8 characters.
pub fn format(t: &Stamp, pattern: &str) -> String {
    let doy = days_from_civil(t.y, t.m, t.d) - days_from_civil(t.y, 1, 1) + 1;
    let mut out = String::new();
    let mut zero_less = false;
    for tok in tokens(pattern) {
        match tok {
            Tok::Lit(c) => out.push(c),
            Tok::Yyyy => out += &format!("{:04}", t.y),
            Tok::Yy => out += &format!("{:02}", t.y.rem_euclid(100)),
            Tok::Zy => {
                zero_less = true;
                out += &format!("{}", t.y.rem_euclid(100))
            }
            Tok::Mm => out += &format!("{:02}", t.m),
            Tok::Zm => {
                zero_less = true;
                out += &format!("{}", t.m)
            }
            Tok::MonUpper => out += MONTHS[(t.m - 1) as usize],
            Tok::MonMixed => {
                let m = MONTHS[(t.m - 1) as usize];
                out += &m[..1];
                out += &m[1..].to_ascii_lowercase();
            }
            Tok::Dd => out += &format!("{:02}", t.d),
            Tok::Zd => {
                zero_less = true;
                out += &format!("{}", t.d)
            }
            Tok::Ddd => out += &format!("{doy:03}"),
            Tok::Hh => out += &format!("{:02}", t.h),
            Tok::Mi => out += &format!("{:02}", t.mi),
            Tok::Ss => out += &format!("{:02}", t.s),
            Tok::Milli => out += &format!("{:03}", t.micro / 1000),
            Tok::Micro => out += &format!("{:06}", t.micro),
        }
    }
    if zero_less {
        while out.len() < 8 {
            out.push(' ');
        }
    }
    out
}

/// Widens a two-digit year with the window w (a year such as 1950).
/// Whether the pattern holds the year in two digits (YY or ZY) only.
pub fn two_digit_year(pattern: &str) -> bool {
    let t = tokens(pattern);
    !t.contains(&Tok::Yyyy) && t.iter().any(|x| matches!(x, Tok::Yy | Tok::Zy))
}

pub fn widen(yy: i64, w: i64) -> i64 {
    let century = if yy < w % 100 { w / 100 + 1 } else { w / 100 };
    century * 100 + yy
}

/// Reads a text in a pattern. Missing day and month are 1; a two-digit
/// year is widened with w. None when the text does not fit the pattern or
/// is not a valid date.
pub fn parse(text: &str, pattern: &str, w: i64) -> Option<Stamp> {
    let text = text.trim_end();
    let b = text.as_bytes();
    let mut i = 0;
    let mut t = Stamp {
        y: 1,
        m: 1,
        d: 1,
        h: 0,
        mi: 0,
        s: 0,
        micro: 0,
    };
    let mut doy: Option<i64> = None;
    let mut year_set = false;
    let num = |i: &mut usize, n: usize, zero_less: bool| -> Option<i64> {
        let start = *i;
        let max = (*i + n).min(b.len());
        while *i < max && b[*i].is_ascii_digit() {
            *i += 1;
        }
        if *i == start || (!zero_less && *i - start != n) {
            return None;
        }
        text[start..*i].parse().ok()
    };
    for tok in tokens(pattern) {
        match tok {
            Tok::Lit(c) => {
                if b.get(i).copied()? as char != c {
                    return None;
                }
                i += 1;
            }
            Tok::Yyyy => {
                t.y = num(&mut i, 4, false)?;
                year_set = true;
            }
            Tok::Yy | Tok::Zy => {
                t.y = widen(num(&mut i, 2, tok == Tok::Zy)?, w);
                year_set = true;
            }
            Tok::Mm | Tok::Zm => t.m = num(&mut i, 2, tok == Tok::Zm)? as u32,
            Tok::MonUpper | Tok::MonMixed => {
                let m = text.get(i..i + 3)?;
                let k = MONTHS.iter().position(|x| {
                    if tok == Tok::MonUpper {
                        *x == m
                    } else {
                        x[..1] == m[..1] && x[1..].to_ascii_lowercase() == m[1..]
                    }
                })?;
                t.m = k as u32 + 1;
                i += 3;
            }
            Tok::Dd | Tok::Zd => t.d = num(&mut i, 2, tok == Tok::Zd)? as u32,
            Tok::Ddd => doy = Some(num(&mut i, 3, false)?),
            Tok::Hh => t.h = num(&mut i, 2, false)? as u32,
            Tok::Mi => t.mi = num(&mut i, 2, false)? as u32,
            Tok::Ss => t.s = num(&mut i, 2, false)? as u32,
            Tok::Milli => t.micro = num(&mut i, 3, false)? as u32 * 1000,
            Tok::Micro => t.micro = num(&mut i, 6, false)? as u32,
        }
    }
    if i != b.len() {
        return None;
    }
    let _ = year_set;
    let leap = (t.y % 4 == 0 && t.y % 100 != 0) || t.y % 400 == 0;
    if let Some(n) = doy {
        if !(1..=if leap { 366 } else { 365 }).contains(&n) {
            return None;
        }
        let (y, m, d) = civil_from_days(days_from_civil(t.y, 1, 1) + n - 1);
        t.y = y;
        t.m = m;
        t.d = d;
    }
    let days = match t.m {
        2 if leap => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1..=12 => 31,
        _ => return None,
    };
    if !(1..=days).contains(&t.d) || t.h > 23 || t.mi > 59 || t.s > 59 || !(1..=9999).contains(&t.y)
    {
        return None;
    }
    Some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns() {
        let t = parse("20240229", "YYYYMMDD", 1950).unwrap();
        assert_eq!(format(&t, "DDMmmYYYY"), "29Feb2024");
        assert_eq!(format(&t, "YYYYDDD"), "2024060");
        assert_eq!(format(&t, "ZY-ZM-ZD"), "24-2-29 ");
        assert_eq!(parse("8-1-20", "ZY-ZM-ZD", 1950).unwrap().y, 2008);
        assert!(parse("20230229", "YYYYMMDD", 1950).is_none());
        // Lilian day 1 is 15 October 1582, microsecond 86400 * 10**6
        let t = parse("15821015", "YYYYMMDD", 1950).unwrap();
        assert_eq!(t.micros(), 86_400_000_000);
        assert_eq!(
            format(&Stamp::from_micros(0), "YYYY-MM-DD-HH.MI.SS.999999"),
            "1582-10-14-00.00.00.000000"
        );
    }
}

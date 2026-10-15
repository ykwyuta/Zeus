//! Language Environment callable services used by COBOL programs.
//!
//! Implemented from the published interface descriptions (parameter
//! lists and feedback codes):
//!
//! * `CEENCOD (c1, c2, case, severity, control, facility, isi, token, fc)`:
//!   constructs a condition token with checked fields and optional feedback;
//! * `CEE3ABD (abcode, timing)`: ends the run unit with user abend
//!   `abcode`;
//! * `CEEDAYS (input_char_date, picture_string, output_Lilian_date, fc)`:
//!   converts a date to its Lilian day number (day 1 = 15 October 1582);
//! * `CEEGMT (Lilian, seconds, fc)` and `CEELOCT (Lilian, seconds,
//!   Gregorian, fc)`: the current date and time (Zeus runs in UTC), as a
//!   Lilian day number, Lilian seconds (COMP-2, from 00:00 on 14 October
//!   1582) and `YYYYMMDDHHMISS999`;
//! * `CEEDATM (seconds, picture, output, fc)` and `CEEDATE (Lilian,
//!   picture, output, fc)`: Lilian seconds / days as text (80 bytes) in a
//!   picture (YYYY YY MM DD DDD HH MI SS 9.. MMM Mmm WWW Www);
//! * `CEESECS (timestamp, picture, seconds, fc)`: text to Lilian seconds.
//!
//! Feedback codes are 12 bytes: severity and message number (halfwords),
//! case/severity/control byte, facility id `CEE` and the instance specific
//! information word.

use super::{Machine, RResult, RunError};
use crate::ebcdic;
mod condition;
pub(super) mod handlers;

/// Prefix of the error that ends the run unit with an abend.
pub const ABEND: &str = "\u{0}abend ";

impl RunError {
    /// The abend code (`U0999`, ...) when the run unit ended abnormally.
    pub fn abend(&self) -> Option<&str> {
        self.0.strip_prefix(ABEND)
    }
}

/// Day number of a proleptic Gregorian date counted from 1 March year 0.
fn day_number(y: i64, m: i64, d: i64) -> i64 {
    let (y, m) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    365 * y + y / 4 - y / 100 + y / 400 + (153 * m + 2) / 5 + d - 1
}

/// Lilian day number (15 October 1582 = 1).
pub fn lilian(y: i64, m: i64, d: i64) -> i64 {
    day_number(y, m, d) - day_number(1582, 10, 15) + 1
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// The current time: Lilian day and seconds into the day (UTC).
fn now_lilian() -> (i64, f64) {
    let unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let days = (unix / 86400.0).floor();
    (lilian(1970, 1, 1) + days as i64, unix - days * 86400.0)
}

/// Lilian day -> (year, month, day).
fn civil_of_lilian(l: i64) -> (i64, i64, i64) {
    super::eval::civil_from_days(l - lilian(1970, 1, 1))
}

const MONTHS: [&str; 12] = [
    "JANUARY",
    "FEBRUARY",
    "MARCH",
    "APRIL",
    "MAY",
    "JUNE",
    "JULY",
    "AUGUST",
    "SEPTEMBER",
    "OCTOBER",
    "NOVEMBER",
    "DECEMBER",
];
const DAYS: [&str; 7] = [
    "SUNDAY",
    "MONDAY",
    "TUESDAY",
    "WEDNESDAY",
    "THURSDAY",
    "FRIDAY",
    "SATURDAY",
];

/// Lilian seconds as text in a date/time picture.
fn format_lilian_seconds(secs: f64, picture: &str) -> String {
    let day = (secs / 86400.0).floor() as i64;
    let sod = secs - day as f64 * 86400.0;
    let (y, m, d) = civil_of_lilian(day);
    let doy = day - lilian(y, 1, 1) + 1;
    // 15 October 1582 (Lilian day 1) was a Friday
    let wd = (day + 4).rem_euclid(7) as usize;
    let h = (sod / 3600.0) as i64;
    let mi = ((sod % 3600.0) / 60.0) as i64;
    let s = (sod % 60.0) as i64;
    let frac = sod.fract();
    let p: Vec<char> = picture.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let starts = |i: usize, t: &str| p[i..].iter().take(t.len()).copied().eq(t.chars());
    let case = |t: &str, name: &str| -> String {
        let n: String = name.chars().take(t.len()).collect();
        if t.chars().nth(1).is_some_and(|c| c.is_ascii_lowercase()) {
            let mut c = n.chars();
            c.next()
                .map(|f| f.to_string() + &c.as_str().to_ascii_lowercase())
                .unwrap_or_default()
        } else {
            n
        }
    };
    while i < p.len() {
        if starts(i, "YYYY") {
            out += &format!("{y:04}");
            i += 4;
        } else if starts(i, "YY") {
            out += &format!("{:02}", y % 100);
            i += 2;
        } else if starts(i, "MMM") || starts(i, "Mmm") {
            out += &case(
                &p[i..i + 3].iter().collect::<String>(),
                MONTHS[(m - 1) as usize],
            );
            i += 3;
        } else if starts(i, "MM") {
            out += &format!("{m:02}");
            i += 2;
        } else if starts(i, "MI") {
            out += &format!("{mi:02}");
            i += 2;
        } else if starts(i, "DDD") {
            out += &format!("{doy:03}");
            i += 3;
        } else if starts(i, "DD") {
            out += &format!("{d:02}");
            i += 2;
        } else if starts(i, "HH") {
            out += &format!("{h:02}");
            i += 2;
        } else if starts(i, "SS") {
            out += &format!("{s:02}");
            i += 2;
        } else if starts(i, "WWW") || starts(i, "Www") {
            out += &case(&p[i..i + 3].iter().collect::<String>(), DAYS[wd]);
            i += 3;
        } else if p[i] == '9' {
            let n = p[i..].iter().take_while(|&&c| c == '9').count();
            let f = format!("{:.*}", n, frac);
            out += f.get(2..2 + n).unwrap_or("");
            i += n;
        } else {
            out.push(p[i]);
            i += 1;
        }
    }
    out
}

/// CEEDAYS condition (message number), None on success.
fn ceedays(date: &str, picture: &str) -> Result<i64, u16> {
    let pic: Vec<char> = picture.trim_end().chars().collect();
    let date: Vec<char> = date.chars().collect();
    if pic.is_empty() {
        return Err(2518);
    }
    let (mut y, mut m, mut d) = (None, None, None);
    let mut doy = None;
    let (mut i, mut j) = (0, 0);
    while i < pic.len() {
        let token = ["YYYY", "YY", "MM", "DDD", "DD"].into_iter().find(|t| {
            pic[i..]
                .iter()
                .collect::<String>()
                .to_ascii_uppercase()
                .starts_with(t)
        });
        match token {
            Some(t) => {
                let n = t.len();
                if j + n > date.len() {
                    return Err(2507);
                }
                let s: String = date[j..j + n].iter().collect();
                if !s.chars().all(|c| c.is_ascii_digit()) {
                    return Err(2520);
                }
                let v: i64 = s.parse().unwrap_or(0);
                match t {
                    "YYYY" => y = Some(v),
                    // two-digit years: century window 1900-1999 (default)
                    "YY" => y = Some(1900 + v),
                    "MM" => m = Some(v),
                    "DDD" => doy = Some(v),
                    _ => d = Some(v),
                }
                i += n;
                j += n;
            }
            None => {
                // a separator in the picture must be present in the date
                if j >= date.len() {
                    return Err(2507);
                }
                i += 1;
                j += 1;
            }
        }
    }
    let Some(y) = y else { return Err(2518) };
    if let Some(n) = doy {
        let len = if days_in_month(y, 2) == 29 { 366 } else { 365 };
        if n < 1 || n > len {
            return Err(2508);
        }
        let r = lilian(y, 1, 1) + n - 1;
        return if lilian(y, 1, 1) < 1 {
            Err(2513)
        } else {
            Ok(r)
        };
    }
    let (Some(m), Some(d)) = (m, d) else {
        return Err(2518);
    };
    if !(1..=12).contains(&m) {
        return Err(2517);
    }
    if d < 1 || d > days_in_month(y, m) {
        return Err(2508);
    }
    let r = lilian(y, m, d);
    if r < 1 || y > 9999 {
        return Err(2513);
    }
    Ok(r)
}

/// Number of arguments in the supported AMODE 31 callable-service interface.
pub(super) fn parameter_count(name: &str) -> Option<usize> {
    match name {
        "CEENCOD" | "CEEDCOD" => Some(9),
        "CEEHDLR" | "CEESGL" | "CEEGQDT" => Some(3),
        "CEEHDLU" => Some(2),
        "CEE3ABD" => Some(2),
        "CEEGMT" => Some(3),
        "CEEDAYS" | "CEELOCT" | "CEEDATM" | "CEEDATE" | "CEESECS" => Some(4),
        _ => None,
    }
}

impl Machine {
    fn read_vstring(&self, a: usize) -> String {
        let n = u16::from_be_bytes([self.mem[a], self.mem[a + 1]]) as usize;
        let bytes = &self.mem[a + 2..a + 2 + n];
        ebcdic::decode_latin1(bytes)
            .iter()
            .map(|&b| b as char)
            .collect()
    }

    fn feedback(&mut self, a: Option<usize>, msg: Option<u16>) {
        self.condition_feedback(a, msg.map(|message| (3, message)));
    }

    fn condition_feedback(&mut self, a: Option<usize>, condition: Option<(u16, u16)>) {
        let Some(a) = a else { return };
        let mut fc = [0u8; 12];
        if let Some((severity, message)) = condition {
            fc[0..2].copy_from_slice(&severity.to_be_bytes());
            fc[2..4].copy_from_slice(&message.to_be_bytes());
            fc[4] = 0x41 | ((severity as u8) << 3);
            fc[5..8].copy_from_slice(&ebcdic::encode("CEE"));
        }
        self.mem[a..a + 12].copy_from_slice(&fc);
    }

    /// Runs `name` when it is a callable service; returns false otherwise.
    pub(super) fn le_service(&mut self, name: &str, args: &[Option<usize>]) -> RResult<bool> {
        let layouts: &[usize] = match name {
            "CEENCOD" => &[2, 2, 2, 2, 2, 3, 4, 12, 12],
            "CEEDCOD" => &[12, 2, 2, 2, 2, 2, 3, 4, 12],
            "CEEHDLR" => &[4, 4, 12],
            "CEEHDLU" => &[4, 12],
            "CEESGL" | "CEEGQDT" => &[12, 4, 12],
            "CEE3ABD" => &[4, 4],
            "CEEDAYS" => &[0, 0, 4, 12],
            "CEEGMT" => &[4, 8, 12],
            "CEELOCT" => &[4, 8, 17, 12],
            "CEEDATM" => &[8, 0, 80, 12],
            "CEEDATE" => &[4, 0, 80, 12],
            "CEESECS" => &[0, 0, 8, 12],
            _ => return Ok(false),
        };
        for (index, &size) in layouts.iter().enumerate() {
            if let Some(Some(at)) = args.get(index) {
                let length = if size == 0 {
                    let bytes = self
                        .mem
                        .get(
                            *at..at.checked_add(2).ok_or_else(|| {
                                RunError(format!("{name}: parameter address overflow"))
                            })?,
                        )
                        .ok_or_else(|| {
                            RunError(format!("{name}: string length outside run-unit storage"))
                        })?;
                    2 + usize::from(u16::from_be_bytes(bytes.try_into().unwrap()))
                } else {
                    size
                };
                if at
                    .checked_add(length)
                    .is_none_or(|end| end > self.mem.len())
                {
                    return super::err(format!(
                        "{name}: parameter {} outside run-unit storage",
                        index + 1
                    ));
                }
                let output = match name {
                    "CEENCOD" => index >= 7,
                    "CEEDCOD" | "CEEHDLU" | "CEEGQDT" => index >= 1,
                    "CEEHDLR" | "CEESGL" => index >= 2,
                    "CEE3ABD" => false,
                    "CEEGMT" | "CEELOCT" => true,
                    _ => index >= 2,
                };
                if output {
                    self.hlasm_storage
                        .check_write(*at, length)
                        .map_err(RunError)?;
                }
            }
        }
        let fullword = |m: &Machine, a: Option<usize>| -> i64 {
            a.map_or(0, |a| {
                i32::from_be_bytes([m.mem[a], m.mem[a + 1], m.mem[a + 2], m.mem[a + 3]]) as i64
            })
        };
        match name {
            "CEEDCOD" => {
                self.decompose_condition(args)?;
                Ok(true)
            }
            "CEEHDLR" | "CEEHDLU" | "CEESGL" | "CEEGQDT" => {
                self.condition_service(name, args)?;
                Ok(true)
            }
            "CEENCOD" => {
                self.construct_condition(args)?;
                Ok(true)
            }
            "CEE3ABD" => {
                let code = fullword(self, args.first().copied().flatten());
                Err(RunError(format!("{ABEND}U{code:04}")))
            }
            "CEEDAYS" => {
                let (Some(Some(date)), Some(Some(pic))) = (args.first(), args.get(1)) else {
                    return super::err("CEEDAYS: missing arguments");
                };
                let date = self.read_vstring(*date);
                let pic = self.read_vstring(*pic);
                let r = ceedays(&date, &pic);
                if let Some(Some(out)) = args.get(2) {
                    let v = r.unwrap_or(0) as i32;
                    self.mem[*out..*out + 4].copy_from_slice(&v.to_be_bytes());
                }
                let fc = args.get(3).copied().flatten();
                self.feedback(fc, r.err());
                if r.is_err() && fc.is_none() {
                    return Err(RunError(format!("{ABEND}U4038")));
                }
                Ok(true)
            }
            "CEEGMT" | "CEELOCT" => {
                let (day, sod) = now_lilian();
                if let Some(Some(a)) = args.first() {
                    self.mem[*a..*a + 4].copy_from_slice(&(day as i32).to_be_bytes());
                }
                let secs = day as f64 * 86400.0 + sod;
                if let Some(Some(a)) = args.get(1) {
                    zeus_decimal::hfp::encode(secs, &mut self.mem[*a..*a + 8]);
                }
                let fc = if name == "CEELOCT" {
                    if let Some(Some(a)) = args.get(2) {
                        let t = format_lilian_seconds(secs, "YYYYMMDDHHMISS999");
                        self.mem[*a..*a + 17].copy_from_slice(&ebcdic::encode(&t));
                    }
                    args.get(3).copied().flatten()
                } else {
                    args.get(2).copied().flatten()
                };
                self.feedback(fc, None);
                Ok(true)
            }
            "CEEDATM" | "CEEDATE" => {
                let (Some(Some(v)), Some(Some(pic)), Some(Some(out))) =
                    (args.first(), args.get(1), args.get(2))
                else {
                    return super::err(format!("{name}: missing arguments"));
                };
                let secs = if name == "CEEDATM" {
                    zeus_decimal::hfp::decode(&self.mem[*v..*v + 8])
                } else {
                    fullword(self, Some(*v)) as f64 * 86400.0
                };
                let pic = self.read_vstring(*pic);
                let fc = args.get(3).copied().flatten();
                if secs < 86400.0 {
                    self.feedback(fc, Some(if name == "CEEDATM" { 2517 } else { 2512 }));
                    self.mem[*out..*out + 80].fill(ebcdic::SPACE);
                    return Ok(true);
                }
                let mut t = ebcdic::encode(&format_lilian_seconds(secs, pic.trim_end()));
                t.resize(80, ebcdic::SPACE);
                self.mem[*out..*out + 80].copy_from_slice(&t);
                self.feedback(fc, None);
                Ok(true)
            }
            "CEESECS" => {
                let (Some(Some(ts)), Some(Some(pic)), Some(Some(out))) =
                    (args.first(), args.get(1), args.get(2))
                else {
                    return super::err("CEESECS: missing arguments");
                };
                let ts = self.read_vstring(*ts);
                let pic = self.read_vstring(*pic);
                // the date part as for CEEDAYS, then HH MI SS
                let date_pic: String = pic
                    .chars()
                    .map(|c| if "YMD".contains(c) { c } else { ' ' })
                    .collect();
                let fc = args.get(3).copied().flatten();
                let field = |tok: &str| -> i64 {
                    pic.find(tok)
                        .and_then(|i| ts.get(i..i + tok.len()))
                        .and_then(|v| v.trim().parse().ok())
                        .unwrap_or(0)
                };
                let date: String = ts
                    .chars()
                    .zip(date_pic.chars())
                    .map(|(c, p)| if p == ' ' { ' ' } else { c })
                    .collect();
                match ceedays(&date.replace(' ', ""), &date_pic.replace(' ', "")) {
                    Ok(day) => {
                        let secs = day as f64 * 86400.0
                            + (field("HH") * 3600 + field("MI") * 60 + field("SS")) as f64;
                        zeus_decimal::hfp::encode(secs, &mut self.mem[*out..*out + 8]);
                        self.feedback(fc, None);
                    }
                    Err(e) => {
                        self.mem[*out..*out + 8].fill(0);
                        self.feedback(fc, Some(e));
                        if fc.is_none() {
                            return Err(RunError(format!("{ABEND}U4038")));
                        }
                    }
                }
                Ok(true)
            }
            _ => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lilian_days() {
        assert_eq!(lilian(1582, 10, 15), 1);
        assert_eq!(lilian(1582, 10, 16), 2);
        // days since 15 October 1582, counted independently
        assert_eq!(lilian(1993, 5, 14), 149_962);
        assert_eq!(lilian(2000, 3, 1) - lilian(2000, 2, 28), 2);
    }

    #[test]
    fn lilian_seconds_text() {
        let secs = lilian(2026, 10, 12) as f64 * 86400.0 + 13.0 * 3600.0 + 5.0 * 60.0 + 7.25;
        assert_eq!(
            format_lilian_seconds(secs, "YYYY-MM-DD HH:MI:SS.99"),
            "2026-10-12 13:05:07.25"
        );
        assert_eq!(
            format_lilian_seconds(secs, "Www DD Mmm YY"),
            "Mon 12 Oct 26"
        );
        assert_eq!(
            format_lilian_seconds(86400.0, "YYYYMMDD DDD WWW"),
            "15821015 288 FRI"
        );
    }

    #[test]
    fn ceedays_pictures() {
        assert_eq!(ceedays("2022-07-18", "YYYY-MM-DD"), Ok(lilian(2022, 7, 18)));
        assert_eq!(ceedays("20220230", "YYYYMMDD"), Err(2508));
        assert_eq!(ceedays("20221301", "YYYYMMDD"), Err(2517));
        assert_eq!(ceedays("2022AB01", "YYYYMMDD"), Err(2520));
        assert_eq!(ceedays("2022", "YYYYMMDD"), Err(2507));
        assert_eq!(ceedays("15000101", "YYYYMMDD"), Err(2513));
        assert_eq!(ceedays("2022", "XXXX"), Err(2518));
    }
}

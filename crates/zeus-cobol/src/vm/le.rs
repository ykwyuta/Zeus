//! Language Environment callable services used by COBOL programs.
//!
//! Implemented from the published interface descriptions (parameter
//! lists and feedback codes):
//!
//! * `CEE3ABD (abcode, timing)`: ends the run unit with user abend
//!   `abcode`;
//! * `CEEDAYS (input_char_date, picture_string, output_Lilian_date, fc)`:
//!   converts a date to its Lilian day number (day 1 = 15 October 1582).
//!
//! Feedback codes are 12 bytes: severity and message number (halfwords),
//! case/severity/control byte, facility id `CEE` and the instance specific
//! information word.

use super::{Machine, RResult, RunError};
use crate::ebcdic;

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
        let Some(a) = a else { return };
        let mut fc = [0u8; 12];
        if let Some(n) = msg {
            fc[0..2].copy_from_slice(&3u16.to_be_bytes());
            fc[2..4].copy_from_slice(&n.to_be_bytes());
            fc[4] = 0x59;
            fc[5..8].copy_from_slice(&ebcdic::encode("CEE"));
        }
        self.mem[a..a + 12].copy_from_slice(&fc);
    }

    /// Runs `name` when it is a callable service; returns false otherwise.
    pub(super) fn le_service(&mut self, name: &str, args: &[Option<usize>]) -> RResult<bool> {
        let fullword = |m: &Machine, a: Option<usize>| -> i64 {
            a.map_or(0, |a| {
                i32::from_be_bytes([m.mem[a], m.mem[a + 1], m.mem[a + 2], m.mem[a + 3]]) as i64
            })
        };
        match name {
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

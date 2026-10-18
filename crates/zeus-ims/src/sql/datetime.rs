//! Calendar conversion for IMS DATE/TIME/TIMESTAMP catalog marshaling.
//! LONG holds milliseconds since 1970-01-01; CHAR uses the DBD PATTERN.
use super::SqlError;

type Result<T> = std::result::Result<T, SqlError>;
fn invalid() -> SqlError {
    SqlError::new("22007", "invalid IMS date/time value or pattern")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DateTime {
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    micros: u32,
}

impl Default for DateTime {
    fn default() -> Self {
        Self {
            year: 1970,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            micros: 0,
        }
    }
}

fn leap(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
fn month_days(year: i32, month: u32) -> u32 {
    match month {
        2 => {
            if leap(year) {
                29
            } else {
                28
            }
        }
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => 0,
    }
}
fn year_start(year: i32) -> i64 {
    let y = i64::from(year) - 1;
    y * 365 + y / 4 - y / 100 + y / 400
}

enum Part {
    Literal(String),
    Field(char, usize),
}

fn pattern(source: &str) -> Result<Vec<Part>> {
    let chars: Vec<_> = source.chars().collect();
    let mut parts = Vec::new();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        if c == '\'' {
            i += 1;
            let mut text = String::new();
            loop {
                let c = *chars.get(i).ok_or_else(invalid)?;
                i += 1;
                if c == '\'' {
                    if chars.get(i) == Some(&'\'') {
                        i += 1;
                    } else {
                        break;
                    }
                }
                text.push(c);
            }
            parts.push(Part::Literal(text));
        } else if c.is_ascii_alphabetic() {
            let start = i;
            while chars.get(i) == Some(&c) {
                i += 1;
            }
            let n = i - start;
            if !matches!(
                (c, n),
                ('y', 4) | ('M' | 'd' | 'H' | 'm' | 's', 2) | ('S', 1..=9)
            ) {
                return Err(invalid());
            }
            parts.push(Part::Field(c, n));
        } else {
            parts.push(Part::Literal(c.to_string()));
            i += 1;
        }
    }
    Ok(parts)
}

impl DateTime {
    fn validate(&self) -> Result<()> {
        if !(1..=9999).contains(&self.year)
            || self.day == 0
            || self.day > month_days(self.year, self.month)
            || self.hour > 23
            || self.minute > 59
            || self.second > 59
            || self.micros > 999_999
        {
            return Err(invalid());
        }
        Ok(())
    }

    pub(super) fn from_millis(millis: i64) -> Result<Self> {
        let ordinal = millis
            .div_euclid(86_400_000)
            .checked_add(year_start(1970))
            .ok_or_else(invalid)?;
        if ordinal < 0 || ordinal >= year_start(10000) {
            return Err(invalid());
        }
        let mut lo = 1;
        let mut hi = 10000;
        while lo + 1 < hi {
            let mid = (lo + hi) / 2;
            if year_start(mid) <= ordinal {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        let mut remaining = (ordinal - year_start(lo)) as u32;
        let mut month = 1;
        while remaining >= month_days(lo, month) {
            remaining -= month_days(lo, month);
            month += 1;
        }
        let daytime = millis.rem_euclid(86_400_000) as u32;
        Ok(Self {
            year: lo,
            month,
            day: remaining + 1,
            hour: daytime / 3_600_000,
            minute: daytime / 60_000 % 60,
            second: daytime / 1000 % 60,
            micros: daytime % 1000 * 1000,
        })
    }

    pub(super) fn millis(&self) -> Result<i64> {
        self.validate()?;
        let days = year_start(self.year)
            + (1..self.month)
                .map(|m| i64::from(month_days(self.year, m)))
                .sum::<i64>()
            + i64::from(self.day)
            - 1
            - year_start(1970);
        Ok(days * 86_400_000
            + i64::from(self.hour) * 3_600_000
            + i64::from(self.minute) * 60_000
            + i64::from(self.second) * 1000
            + i64::from(self.micros / 1000))
    }

    pub(super) fn parse(text: &str, source: &str) -> Result<Self> {
        let mut value = Self::default();
        let mut pos = 0;
        let mut seen = Vec::new();
        for part in pattern(source)? {
            match part {
                Part::Literal(literal) => {
                    if !text.get(pos..).is_some_and(|t| t.starts_with(&literal)) {
                        return Err(invalid());
                    }
                    pos += literal.len();
                }
                Part::Field(field, length) => {
                    if seen.contains(&field) {
                        return Err(invalid());
                    }
                    seen.push(field);
                    let digits = text.get(pos..pos + length).ok_or_else(invalid)?;
                    if !digits.bytes().all(|b| b.is_ascii_digit()) {
                        return Err(invalid());
                    }
                    let n: u32 = digits.parse().map_err(|_| invalid())?;
                    pos += length;
                    match field {
                        'y' => value.year = n as i32,
                        'M' => value.month = n,
                        'd' => value.day = n,
                        'H' => value.hour = n,
                        'm' => value.minute = n,
                        's' => value.second = n,
                        'S' => {
                            value.micros = if length <= 6 {
                                n * 10u32.pow(6 - length as u32)
                            } else {
                                n / 10u32.pow(length as u32 - 6)
                            }
                        }
                        _ => return Err(invalid()),
                    }
                }
            }
        }
        if pos != text.len() {
            return Err(invalid());
        }
        value.validate()?;
        Ok(value)
    }

    pub(super) fn format(&self, source: &str) -> Result<String> {
        self.validate()?;
        let mut output = String::new();
        for part in pattern(source)? {
            match part {
                Part::Literal(s) => output.push_str(&s),
                Part::Field(field, length) => {
                    let value = match field {
                        'y' => self.year as u32,
                        'M' => self.month,
                        'd' => self.day,
                        'H' => self.hour,
                        'm' => self.minute,
                        's' => self.second,
                        'S' => {
                            if length <= 6 {
                                self.micros / 10u32.pow(6 - length as u32)
                            } else {
                                self.micros * 10u32.pow(length as u32 - 6)
                            }
                        }
                        _ => return Err(invalid()),
                    };
                    output.push_str(&format!("{value:0length$}"));
                }
            }
        }
        Ok(output)
    }

    pub(super) fn canonical(&self, kind: &str) -> Result<String> {
        self.format(match kind {
            "DATE" => "yyyy-MM-dd",
            "TIME" => "HH:mm:ss",
            "TIMESTAMP" => "yyyy-MM-dd HH:mm:ss.SSSSSS",
            _ => return Err(invalid()),
        })
    }

    pub(super) fn parse_canonical(text: &str, kind: &str) -> Result<Self> {
        let source = match kind {
            "DATE" => "yyyy-MM-dd".into(),
            "TIME" => "HH:mm:ss".into(),
            "TIMESTAMP" => {
                if text.len() == 19 {
                    "yyyy-MM-dd HH:mm:ss".into()
                } else {
                    let fraction = text
                        .len()
                        .checked_sub(20)
                        .filter(|n| (1..=6).contains(n))
                        .ok_or_else(invalid)?;
                    format!("yyyy-MM-dd HH:mm:ss.{}", "S".repeat(fraction))
                }
            }
            _ => return Err(invalid()),
        };
        Self::parse(text, &source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn epoch_milliseconds_handle_negative_values_and_leap_centuries() {
        for text in [
            "0001-01-01 00:00:00.000000",
            "1900-03-01 12:34:56.123000",
            "1969-12-31 23:59:59.999000",
            "1970-01-01 00:00:00.000000",
            "2000-02-29 12:00:00.000000",
            "9999-12-31 23:59:59.999000",
        ] {
            let date = DateTime::parse_canonical(text, "TIMESTAMP").unwrap();
            assert_eq!(
                DateTime::from_millis(date.millis().unwrap())
                    .unwrap()
                    .canonical("TIMESTAMP")
                    .unwrap(),
                text
            );
        }
        assert_eq!(
            DateTime::from_millis(-1)
                .unwrap()
                .canonical("TIMESTAMP")
                .unwrap(),
            "1969-12-31 23:59:59.999000"
        );
        assert!(DateTime::parse_canonical("1900-02-29", "DATE").is_err());
        assert!(DateTime::parse_canonical("2024-01-01 24:00:00", "TIMESTAMP").is_err());
    }
    #[test]
    fn character_patterns_preserve_fractional_precision_and_literals() {
        let date = DateTime::parse("02292024T235959123456", "MMddyyyy'T'HHmmssSSSSSS").unwrap();
        assert_eq!(
            date.canonical("TIMESTAMP").unwrap(),
            "2024-02-29 23:59:59.123456"
        );
        assert_eq!(
            date.format("yyyyMMddHHmmssSSS").unwrap(),
            "20240229235959123"
        );
        assert!(DateTime::parse("20240230", "yyyyMMdd").is_err());
        assert!(DateTime::parse("20240101suffix", "yyyyMMdd").is_err());
    }
}

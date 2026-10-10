//! Clock values: CICS ABSTIME is milliseconds since 1 January 1900.

/// Days from 1970-01-01 to a civil date.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Civil date of days since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The current ABSTIME (milliseconds since 1900-01-01, UTC).
pub fn abstime_now() -> i64 {
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    ms - days_from_civil(1900, 1, 1) * 86_400_000
}

/// Broken-down ABSTIME: (year, month, day, day of year, h, m, s, ms).
pub fn split(abstime: i64) -> (i64, i64, i64, i64, i64, i64, i64, i64) {
    let ms_total = abstime + days_from_civil(1900, 1, 1) * 86_400_000;
    let days = ms_total.div_euclid(86_400_000);
    let rest = ms_total.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    let doy = days - days_from_civil(y, 1, 1) + 1;
    (
        y,
        m,
        d,
        doy,
        rest / 3_600_000,
        rest / 60_000 % 60,
        rest / 1000 % 60,
        rest % 1000,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
        let t =
            (days_from_civil(2022, 7, 18) - days_from_civil(1900, 1, 1)) * 86_400_000 + 3_723_004;
        assert_eq!(split(t), (2022, 7, 18, 199, 1, 2, 3, 4));
    }
}

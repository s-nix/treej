//! RFC 3339 formatting of file times in UTC, written by hand.

use std::time::{SystemTime, UNIX_EPOCH};

/// Formats a time as `YYYY-MM-DDTHH:MM:SSZ`, truncated to whole seconds.
pub fn rfc3339(time: SystemTime) -> String {
    let secs: i64 = match time.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs().min(i64::MAX as u64) as i64,
        Err(e) => {
            // Before 1970: round down to the earlier whole second.
            let d = e.duration();
            let whole = d.as_secs().min(i64::MAX as u64) as i64;
            if d.subsec_nanos() > 0 {
                -whole - 1
            } else {
                -whole
            }
        }
    };
    format_unix(secs)
}

/// Formats seconds since the Unix epoch.
pub fn format_unix(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Converts days since 1970-01-01 to a proleptic Gregorian (year, month, day).
/// Algorithm from Howard Hinnant, "chrono-Compatible Low-Level Date Algorithms".
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097); // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn known_dates() {
        assert_eq!(format_unix(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_unix(1_700_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(format_unix(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(format_unix(1_794_320_520), "2026-11-10T14:22:00Z");
        assert_eq!(format_unix(253_402_300_799), "9999-12-31T23:59:59Z");
        assert_eq!(format_unix(-1), "1969-12-31T23:59:59Z");
        // Windows FILETIME zero.
        assert_eq!(format_unix(-11_644_473_600), "1601-01-01T00:00:00Z");
    }

    #[test]
    fn system_time() {
        let t = UNIX_EPOCH + Duration::from_millis(1_700_000_000_999);
        assert_eq!(rfc3339(t), "2023-11-14T22:13:20Z");
        let t = UNIX_EPOCH - Duration::from_millis(500);
        assert_eq!(rfc3339(t), "1969-12-31T23:59:59Z");
    }
}

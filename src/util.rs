//! Formatting and date helpers shared by the builders.

use std::time::{SystemTime, UNIX_EPOCH};

const KIB: f64 = 1024.0;

#[allow(clippy::cast_precision_loss, reason = "display rounding only")]
const fn to_f64(n: u64) -> f64 {
    n as f64
}

/// Formats a byte count in binary units with conventional labels, as RAM is
/// usually described: `16 GB`, `512 MB`.
#[must_use]
pub fn bytes_binary(n: u64) -> String {
    let v = to_f64(n);
    let (value, unit) = if v >= KIB.powi(4) {
        (v / KIB.powi(4), "TB")
    } else if v >= KIB.powi(3) {
        (v / KIB.powi(3), "GB")
    } else if v >= KIB.powi(2) {
        (v / KIB.powi(2), "MB")
    } else if v >= KIB {
        (v / KIB, "KB")
    } else {
        (v, "bytes")
    };
    trim_number(value, unit)
}

/// Formats a byte count in decimal units, as drive makers label capacity:
/// `4 TB`, `512 GB`.
#[must_use]
pub fn bytes_decimal(n: u64) -> String {
    let v = to_f64(n);
    let (value, unit) = if v >= 1e12 {
        (v / 1e12, "TB")
    } else if v >= 1e9 {
        (v / 1e9, "GB")
    } else if v >= 1e6 {
        (v / 1e6, "MB")
    } else if v >= 1e3 {
        (v / 1e3, "KB")
    } else {
        (v, "bytes")
    };
    trim_number(value, unit)
}

fn trim_number(value: f64, unit: &str) -> String {
    let s = if value >= 100.0 || (value - value.round()).abs() < 0.01 {
        format!("{value:.0}")
    } else if value >= 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    };
    format!("{s} {unit}")
}

/// A percentage of `part` in `whole`, rounded.
#[must_use]
pub fn percent(part: u64, whole: u64) -> Option<u64> {
    if whole == 0 {
        return None;
    }
    let p = (to_f64(part) / to_f64(whole) * 100.0).round();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "0..=100 after rounding"
    )]
    Some(p.clamp(0.0, 100.0) as u64)
}

/// Formats a duration in seconds as `2 d 01 h 37 m`.
#[must_use]
pub fn duration(seconds: u64) -> String {
    let d = seconds / 86_400;
    let h = (seconds % 86_400) / 3600;
    let m = (seconds % 3600) / 60;
    if d > 0 {
        format!("{d} d {h:02} h {m:02} m")
    } else {
        format!("{h} h {m:02} m")
    }
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
#[must_use]
pub const fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = m as i64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date for a count of days since 1970-01-01.
#[must_use]
pub const fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "1..=31 by construction"
    )]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "1..=12 by construction"
    )]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The current UTC time as ISO 8601, to the second.
#[must_use]
pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let secs = i64::try_from(secs).unwrap_or(0);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// A calendar date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Date {
    /// Year.
    pub year: i64,
    /// Month, 1 to 12.
    pub month: u32,
    /// Day, 1 to 31.
    pub day: u32,
}

impl Date {
    /// Parses `YYYY-MM-DD` (optionally followed by a time) or `MM/DD/YYYY`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let valid = |year: i64, month: u32, day: u32| {
            ((1980..=2200).contains(&year) && (1..=12).contains(&month) && (1..=31).contains(&day))
                .then_some(Self { year, month, day })
        };
        if let Some(date) = s.get(..10)
            && date.as_bytes().get(4) == Some(&b'-')
        {
            let mut parts = date.split('-');
            let y = parts.next()?.parse().ok()?;
            let m = parts.next()?.parse().ok()?;
            let d = parts.next()?.parse().ok()?;
            return valid(y, m, d);
        }
        let mut parts = s.split('/');
        let m = parts.next()?.trim().parse().ok()?;
        let d = parts.next()?.trim().parse().ok()?;
        let y: i64 = parts.next()?.trim().get(..4)?.parse().ok()?;
        valid(y, m, d)
    }

    /// Days since 1970-01-01.
    #[must_use]
    pub const fn days(self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    /// Whole years from `self` to `later`.
    #[must_use]
    pub const fn years_until(self, later: Self) -> i64 {
        (later.days() - self.days()).div_euclid(365)
    }

    /// ISO format, `YYYY-MM-DD`.
    #[must_use]
    pub fn iso(self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

/// Describes how old `then` is relative to `now`, such as `3 years old`.
#[must_use]
pub fn age_label(then: Date, now: Date) -> String {
    let days = now.days() - then.days();
    if days < 0 {
        return "dated in the future".to_owned();
    }
    let years = days / 365;
    match years {
        0 if days < 60 => format!("{days} days old"),
        0 => format!("{} months old", days / 30),
        1 => "1 year old".to_owned(),
        y => format!("{y} years old"),
    }
}

/// Collapses runs of whitespace and trims.
#[must_use]
pub fn tidy(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes() {
        assert_eq!(bytes_binary(48 * 1024 * 1024 * 1024), "48 GB");
        assert_eq!(bytes_binary(4095 * 1024 * 1024), "4 GB");
        assert_eq!(bytes_binary(1536 * 1024 * 1024), "1.50 GB");
        assert_eq!(bytes_binary(512), "512 bytes");
        assert_eq!(bytes_decimal(4_000_787_030_016), "4 TB");
        assert_eq!(bytes_decimal(512_110_190_592), "512 GB");
        assert_eq!(bytes_decimal(2_048_408_248_320), "2.05 TB");
    }

    #[test]
    fn formats_durations_and_percent() {
        assert_eq!(duration(5861), "1 h 37 m");
        assert_eq!(duration(2 * 86_400 + 3720), "2 d 01 h 02 m");
        assert_eq!(percent(1, 3), Some(33));
        assert_eq!(percent(1, 0), None);
    }

    #[test]
    fn civil_round_trip() {
        for days in [-1000, 0, 1, 19_000, 20_772, 60_000] {
            let (y, m, d) = civil_from_days(days);
            assert_eq!(days_from_civil(y, m, d), days);
        }
        assert_eq!(days_from_civil(2026, 11, 15), 20_772);
    }

    #[test]
    fn parses_dates() {
        let iso = Date::parse("2024-11-18T00:00:00.0000000Z").unwrap();
        let us = Date::parse("11/18/2024").unwrap();
        assert_eq!(iso, us);
        assert_eq!(iso.iso(), "2024-11-18");
        assert!(Date::parse("garbage").is_none());
        assert!(Date::parse("2024-13-01").is_none());
        assert!(Date::parse("06/21/2006").is_some());
    }

    #[test]
    fn ages() {
        let now = Date::parse("2026-11-15").unwrap();
        assert_eq!(
            age_label(Date::parse("2024-11-18").unwrap(), now),
            "1 year old"
        );
        assert_eq!(
            age_label(Date::parse("2018-01-01").unwrap(), now),
            "8 years old"
        );
        assert_eq!(
            age_label(Date::parse("2026-11-01").unwrap(), now),
            "14 days old"
        );
        assert_eq!(Date::parse("2023-11-14").unwrap().years_until(now), 3);
    }

    #[test]
    fn now_is_iso() {
        let n = now_iso();
        assert_eq!(n.len(), 20);
        assert!(Date::parse(&n).is_some());
    }
}

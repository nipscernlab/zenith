//! Times and durations written for a person, and for the files ZENITH writes.
//!
//! Every time ZENITH shows is in UTC, which is what SOLAR writes in `meta.started_at` and
//! in its log, so that a time in the History and a time in the Log can be compared at a
//! glance. The standard library has no calendar, so the conversion from the epoch is here,
//! with the algorithm from Howard Hinnant's `days_from_civil`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// A moment in UTC, split into its fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utc {
    /// The year.
    pub year: i64,
    /// The month, from 1.
    pub month: u32,
    /// The day of the month, from 1.
    pub day: u32,
    /// The hour.
    pub hour: u32,
    /// The minute.
    pub minute: u32,
    /// The second.
    pub second: u32,
    /// The microseconds within the second.
    pub micros: u32,
}

impl Utc {
    /// The fields of a system time. A time before 1970 is shown as 1970, which no clock
    /// ZENITH reads can produce.
    #[must_use]
    pub fn of(time: SystemTime) -> Self {
        let since = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
        let seconds = i64::try_from(since.as_secs()).unwrap_or(i64::MAX);
        let days = seconds.div_euclid(86_400);
        let of_day = seconds.rem_euclid(86_400);
        let (year, month, day) = civil_from_days(days);
        Self {
            year,
            month,
            day,
            hour: u32::try_from(of_day / 3600).unwrap_or(0),
            minute: u32::try_from(of_day % 3600 / 60).unwrap_or(0),
            second: u32::try_from(of_day % 60).unwrap_or(0),
            micros: since.subsec_micros(),
        }
    }

    /// `2026-09-27T15:47:00.906833Z`, RFC 3339 with microseconds, as SOLAR writes it.
    #[must_use]
    pub fn rfc3339(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:06}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second, self.micros
        )
    }

    /// `15:47:00.906`, the time of day to the millisecond.
    #[must_use]
    pub fn time_of_day(&self) -> String {
        format!(
            "{:02}:{:02}:{:02}.{:03}",
            self.hour,
            self.minute,
            self.second,
            self.micros / 1000
        )
    }

    /// `2026-09-27T15-47-00Z`, for a file name, which cannot hold a colon on Windows.
    #[must_use]
    pub fn file_stamp(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}-{:02}-{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// The civil date of a day counted from 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (
        year,
        u32::try_from(month).unwrap_or(1),
        u32::try_from(day).unwrap_or(1),
    )
}

/// A short duration, as a latency: `0.52 ms`, `12.3 ms`, `1.24 s`.
#[must_use]
pub fn latency(duration: Duration) -> String {
    let micros = duration.as_micros();
    if micros < 10_000 {
        format!("{}.{:02} ms", micros / 1000, micros % 1000 / 10)
    } else if micros < 1_000_000 {
        format!("{}.{} ms", micros / 1000, micros % 1000 / 100)
    } else {
        format!(
            "{}.{:02} s",
            micros / 1_000_000,
            micros % 1_000_000 / 10_000
        )
    }
}

/// SOLAR's own time for a call, from `meta.duration_us`, in the same form as a latency.
#[must_use]
pub fn micros(duration_us: u64) -> String {
    latency(Duration::from_micros(duration_us))
}

/// A long duration, as the time something has lasted: `42s` in the first minute, then
/// `12m`, then `3h 04m`. The coarser steps are what let an idle ZENITH wake once a minute.
#[must_use]
pub fn lasted(duration: Duration) -> String {
    let seconds = duration.as_secs();
    if seconds < 60 {
        format!("{seconds}s")
    } else if seconds < 3600 {
        format!("{}m", seconds / 60)
    } else {
        format!("{}h {:02}m", seconds / 3600, seconds % 3600 / 60)
    }
}

/// How long until [`lasted`] would write something different, which is when the status
/// bar next needs drawing.
#[must_use]
pub fn until_lasted_changes(duration: Duration) -> Duration {
    let step: u128 = if duration.as_secs() < 60 { 1 } else { 60 };
    let elapsed = duration.as_micros() % (step * 1_000_000);
    let left = step * 1_000_000 - elapsed;
    Duration::from_micros(u64::try_from(left).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64, micros: u32) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds) + Duration::from_micros(u64::from(micros))
    }

    #[test]
    fn the_epoch_and_a_known_moment_are_converted_exactly() {
        assert_eq!(Utc::of(UNIX_EPOCH).rfc3339(), "1970-01-01T00:00:00.000000Z");
        // 27 September 2026, 15:47:00.906833 UTC.
        let moment = Utc::of(at(1_790_524_020, 906_833));
        assert_eq!(moment.rfc3339(), "2026-09-27T15:47:00.906833Z");
        assert_eq!(moment.time_of_day(), "15:47:00.906");
        assert_eq!(moment.file_stamp(), "2026-09-27T15-47-00Z");
    }

    #[test]
    fn leap_days_and_the_turn_of_a_century_are_right() {
        // 29 February 2000 and 1 March 2100.
        assert_eq!(
            Utc::of(at(951_782_400, 0)).rfc3339(),
            "2000-02-29T00:00:00.000000Z"
        );
        assert_eq!(
            Utc::of(at(4_107_542_400, 0)).rfc3339(),
            "2100-03-01T00:00:00.000000Z"
        );
    }

    #[test]
    fn latencies_keep_two_significant_decimals_where_they_matter() {
        assert_eq!(latency(Duration::from_micros(520)), "0.52 ms");
        assert_eq!(latency(Duration::from_micros(7)), "0.00 ms");
        assert_eq!(latency(Duration::from_micros(12_345)), "12.3 ms");
        assert_eq!(latency(Duration::from_millis(1240)), "1.24 s");
        assert_eq!(micros(170), "0.17 ms");
    }

    #[test]
    fn what_has_lasted_is_counted_in_seconds_then_minutes_then_hours() {
        assert_eq!(lasted(Duration::from_secs(42)), "42s");
        assert_eq!(lasted(Duration::from_secs(12 * 60 + 5)), "12m");
        assert_eq!(lasted(Duration::from_mins(184)), "3h 04m");
    }

    #[test]
    fn the_next_change_is_a_second_away_at_first_and_a_minute_away_after() {
        assert_eq!(
            until_lasted_changes(Duration::from_millis(1_250)),
            Duration::from_millis(750)
        );
        assert_eq!(
            until_lasted_changes(Duration::from_secs(90)),
            Duration::from_secs(30)
        );
    }
}

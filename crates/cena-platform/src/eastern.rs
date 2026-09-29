//! The game's calendar: Eastern time, from Unix seconds, with no time crate.
//!
//! The game's clock is Eastern. The loot cap resets on the first of the month
//! at midnight Eastern and `;loot`'s *today* starts at midnight Eastern
//! (loottracker's `month_start_eastern`, `eastern_offset`); the player log's
//! archives are Eastern months and Sunday-to-Saturday Eastern weeks (`plan/25`
//! D6, the author: *"backup system should be in eastern time like the
//! server"*). Moved down from the binary's `loot/period.rs` when the log became
//! its second reader, rather than copied.
//!
//! Civil-date arithmetic is Howard Hinnant's `days_from_civil` and its
//! inverse; the Eastern offset is the US rule since 2007: daylight time from
//! the second Sunday of March at 2:00 to the first Sunday of November at
//! 2:00, local. That is a fixed rule with no table to go stale, and a test
//! pins both transitions.

/// Seconds in an hour.
pub const HOUR: i64 = 3600;
/// Seconds in a day.
pub const DAY: i64 = 86_400;

/// A calendar date: `(year, month, day)`, month and day from 1.
pub type Date = (i64, i64, i64);

/// Days since 1970-01-01 for a proleptic Gregorian date.
#[must_use]
pub fn days_from_civil((y, m, d): Date) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date `days` since 1970-01-01 falls on.
#[must_use]
pub fn civil_from_days(days: i64) -> Date {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

/// Day of the week, 0 = Sunday, for days since 1970-01-01 (a Thursday).
#[must_use]
pub fn weekday(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

/// The `n`-th (1-based) Sunday of a month, as days since the epoch.
fn nth_sunday(y: i64, m: i64, n: i64) -> i64 {
    let first = days_from_civil((y, m, 1));
    let to_sunday = (7 - weekday(first)) % 7;
    first + to_sunday + 7 * (n - 1)
}

/// Eastern's offset from UTC at Unix time `at`, in seconds: -4h in daylight
/// time, -5h otherwise.
#[must_use]
pub fn offset(at: i64) -> i64 {
    // The transitions are at 2:00 local, i.e. 7:00 UTC entering (standard
    // time) and 6:00 UTC leaving (daylight time).
    let (y, _, _) = civil_from_days((at - 5 * HOUR).div_euclid(DAY));
    let starts = nth_sunday(y, 3, 2) * DAY + 7 * HOUR;
    let ends = nth_sunday(y, 11, 1) * DAY + 6 * HOUR;
    if at >= starts && at < ends {
        -4 * HOUR
    } else {
        -5 * HOUR
    }
}

/// The Eastern date at Unix time `at`.
#[must_use]
pub fn date(at: i64) -> Date {
    civil_from_days((at + offset(at)).div_euclid(DAY))
}

/// Unix time of midnight Eastern on `date`.
#[must_use]
pub fn midnight(date: Date) -> i64 {
    let utc_midnight = days_from_civil(date) * DAY;
    // Guess with the offset at UTC midnight, then correct with the offset at
    // the guessed instant; the two differ only on a transition day.
    let guess = utc_midnight - offset(utc_midnight);
    utc_midnight - offset(guess)
}

/// Unix time now, in whole seconds.
#[must_use]
pub fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// `YYYY-MM-DD`.
#[must_use]
pub fn format((y, m, d): Date) -> String {
    format!("{y:04}-{m:02}-{d:02}")
}

/// A `YYYY-MM-DD` read back; `None` for anything else.
#[must_use]
pub fn parse(text: &str) -> Option<Date> {
    let mut parts = text.split('-');
    let (y, m, d) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() || y.len() != 4 || m.len() != 2 || d.len() != 2 {
        return None;
    }
    let date = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    (civil_from_days(days_from_civil(date)) == date).then_some(date)
}

/// Whether an archive period starts on `date`: a Sunday (a week) or the
/// first of a month (a month).
///
/// The player log cuts its file at midnight Eastern on every such date,
/// whichever archive the player chose, so a file never straddles a week or a
/// month and a change of choice later finds nothing to split.
#[must_use]
pub fn starts_period(date: Date) -> bool {
    date.2 == 1 || weekday(days_from_civil(date)) == 0
}

/// The last date on or before `date` that [`starts_period`]: where the
/// stretch holding `date` began. Never more than six days back.
#[must_use]
pub fn stretch_start(date: Date) -> Date {
    let mut days = days_from_civil(date);
    while !starts_period(civil_from_days(days)) {
        days -= 1;
    }
    civil_from_days(days)
}

/// The Sunday on or before `date`: the week holding it.
#[must_use]
pub fn week_start(date: Date) -> Date {
    let days = days_from_civil(date);
    civil_from_days(days - weekday(days))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_arithmetic_round_trips() {
        assert_eq!(days_from_civil((1970, 1, 1)), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2026-09-24 is 20,720 days after the epoch.
        assert_eq!(days_from_civil((2026, 9, 24)), 20_720);
        assert_eq!(civil_from_days(20_720), (2026, 9, 24));
        assert_eq!(weekday(20_720), 4, "a Thursday");
    }

    #[test]
    fn the_us_transitions_of_2026() {
        // Daylight time begins 2026-03-08 at 2:00 EST (07:00Z) and ends
        // 2026-11-01 at 2:00 EDT (06:00Z).
        let begins = days_from_civil((2026, 3, 8)) * DAY + 7 * HOUR;
        let ends = days_from_civil((2026, 11, 1)) * DAY + 6 * HOUR;
        assert_eq!(offset(begins - 1), -5 * HOUR);
        assert_eq!(offset(begins), -4 * HOUR);
        assert_eq!(offset(ends - 1), -4 * HOUR);
        assert_eq!(offset(ends), -5 * HOUR);
    }

    #[test]
    fn midnight_is_eastern_midnight() {
        // 2026-09-24 00:00 EDT is 04:00Z.
        assert_eq!(
            midnight((2026, 9, 24)),
            days_from_civil((2026, 9, 24)) * DAY + 4 * HOUR
        );
        // 2026-01-15 00:00 EST is 05:00Z.
        assert_eq!(
            midnight((2026, 1, 15)),
            days_from_civil((2026, 1, 15)) * DAY + 5 * HOUR
        );
    }

    #[test]
    fn a_date_reads_back_and_a_false_one_does_not() {
        assert_eq!(parse("2026-09-24"), Some((2026, 9, 24)));
        assert_eq!(format((2026, 9, 4)), "2026-09-04");
        for bad in ["2026-02-30", "2026-9-24", "2026-09-24-1", "today", ""] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn periods_start_on_sundays_and_the_first() {
        // September 2026: the 1st is a Tuesday; the Sundays are 6, 13, 20, 27.
        assert!(starts_period((2026, 9, 1)));
        assert!(starts_period((2026, 9, 27)));
        assert!(!starts_period((2026, 9, 30)));
        assert_eq!(stretch_start((2026, 9, 5)), (2026, 9, 1));
        assert_eq!(stretch_start((2026, 9, 6)), (2026, 9, 6));
        assert_eq!(
            stretch_start((2026, 10, 3)),
            (2026, 10, 1),
            "the month turns mid-week"
        );
        assert_eq!(stretch_start((2026, 9, 30)), (2026, 9, 27));
        assert_eq!(week_start((2026, 10, 3)), (2026, 9, 27));
        assert_eq!(week_start((2026, 9, 27)), (2026, 9, 27));
    }
}

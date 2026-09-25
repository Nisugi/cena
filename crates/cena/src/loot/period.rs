//! Report periods in Eastern time, from server seconds, with no time crate.
//!
//! The loot cap resets on the first of the month at midnight Eastern, and
//! `today` means since midnight Eastern, because the game's clock is
//! Eastern (loottracker's `month_start_eastern`, `eastern_offset`). The rows
//! are stamped in server seconds -- Unix seconds, from `<prompt time=>` --
//! so a period is two of those.
//!
//! Civil-date arithmetic is Howard Hinnant's `days_from_civil` and its
//! inverse; the Eastern offset is the US rule since 2007: daylight time from
//! the second Sunday of March at 2:00 to the first Sunday of November at
//! 2:00, local. That is a fixed rule with no table to go stale, and a test
//! pins both transitions.

use cena_session::ledger::report::Period;

const HOUR: i64 = 3600;
const DAY: i64 = 86_400;

/// Server seconds as the rows store them. A prompt's time is a `u32`, far
/// inside `f64`'s exact range.
#[expect(
    clippy::cast_precision_loss,
    reason = "seconds since 1970 fit in 52 bits"
)]
const fn secs(at: i64) -> f64 {
    at as f64
}

/// A row's stamp back to whole seconds.
#[expect(clippy::cast_possible_truncation, reason = "stamps are whole seconds")]
const fn whole(at: f64) -> i64 {
    at as i64
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The date `days` since 1970-01-01 falls on: `(year, month, day)`.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
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
fn weekday(days: i64) -> i64 {
    (days + 4).rem_euclid(7)
}

/// The `n`-th (1-based) Sunday of a month, as days since the epoch.
fn nth_sunday(y: i64, m: i64, n: i64) -> i64 {
    let first = days_from_civil(y, m, 1);
    let to_sunday = (7 - weekday(first)) % 7;
    first + to_sunday + 7 * (n - 1)
}

/// Eastern's offset from UTC at Unix time `at`, in seconds: -4h in daylight
/// time, -5h otherwise.
fn offset(at: i64) -> i64 {
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

/// The Eastern civil date at `at`.
fn local_date(at: i64) -> (i64, i64, i64) {
    civil_from_days((at + offset(at)).div_euclid(DAY))
}

/// Unix time of midnight Eastern on the given date.
fn midnight(y: i64, m: i64, d: i64) -> i64 {
    let utc_midnight = days_from_civil(y, m, d) * DAY;
    // Guess with the offset at UTC midnight, then correct with the offset at
    // the guessed instant; the two differ only on a transition day.
    let guess = utc_midnight - offset(utc_midnight);
    utc_midnight - offset(guess)
}

/// Since midnight Eastern today.
#[must_use]
pub(crate) fn today(now: i64) -> Period {
    let (y, m, d) = local_date(now);
    Period {
        since: secs(midnight(y, m, d)),
        until: secs(now),
    }
}

/// The last `hours` hours.
#[must_use]
pub(crate) fn last_hours(now: i64, hours: i64) -> Period {
    Period {
        since: secs(now - hours * HOUR),
        until: secs(now),
    }
}

/// The calendar month containing `now`, Eastern, to now.
#[must_use]
pub(crate) fn this_month(now: i64) -> Period {
    let (y, m, _) = local_date(now);
    Period {
        since: secs(midnight(y, m, 1)),
        until: secs(now),
    }
}

/// A whole calendar month, Eastern.
#[must_use]
pub(crate) fn month(y: i64, m: i64) -> Period {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    Period {
        since: secs(midnight(y, m, 1)),
        until: secs(midnight(ny, nm, 1)),
    }
}

/// The calendar month before the one containing `now`.
#[must_use]
pub(crate) fn last_month(now: i64) -> Period {
    let (y, m, _) = local_date(now);
    if m == 1 {
        month(y - 1, 12)
    } else {
        month(y, m - 1)
    }
}

/// The Eastern civil date of `at`, for a report's label: `YYYY-MM-DD`.
#[must_use]
pub(crate) fn date_label(at: f64) -> String {
    let (y, m, d) = local_date(whole(at));
    format!("{y:04}-{m:02}-{d:02}")
}

/// The Eastern civil date and time of `at`: `YYYY-MM-DD HH:MM`.
#[must_use]
pub(crate) fn time_label(at: f64) -> String {
    let at = whole(at);
    let local = at + offset(at);
    let (y, m, d) = civil_from_days(local.div_euclid(DAY));
    let secs = local.rem_euclid(DAY);
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}",
        secs / HOUR,
        (secs % HOUR) / 60
    )
}

#[cfg(test)]
#[expect(
    clippy::cast_possible_truncation,
    reason = "test stamps are whole seconds"
)]
mod tests {
    use super::*;

    #[test]
    fn civil_arithmetic_round_trips() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 2026-09-24 is 20,720 days after the epoch.
        assert_eq!(days_from_civil(2026, 9, 24), 20_720);
        assert_eq!(civil_from_days(20_720), (2026, 9, 24));
        assert_eq!(weekday(20_720), 4, "a Thursday");
    }

    #[test]
    fn the_us_transitions_of_2026() {
        // Daylight time begins 2026-03-08 at 2:00 EST (07:00Z) and ends
        // 2026-11-01 at 2:00 EDT (06:00Z).
        let begins = days_from_civil(2026, 3, 8) * DAY + 7 * HOUR;
        let ends = days_from_civil(2026, 11, 1) * DAY + 6 * HOUR;
        assert_eq!(offset(begins - 1), -5 * HOUR);
        assert_eq!(offset(begins), -4 * HOUR);
        assert_eq!(offset(ends - 1), -4 * HOUR);
        assert_eq!(offset(ends), -5 * HOUR);
    }

    #[test]
    fn midnight_is_eastern_midnight() {
        // 2026-09-24 00:00 EDT is 04:00Z.
        assert_eq!(
            midnight(2026, 9, 24),
            days_from_civil(2026, 9, 24) * DAY + 4 * HOUR
        );
        // 2026-01-15 00:00 EST is 05:00Z.
        assert_eq!(
            midnight(2026, 1, 15),
            days_from_civil(2026, 1, 15) * DAY + 5 * HOUR
        );
    }

    #[test]
    fn periods_from_a_september_evening() {
        // 2026-09-24 21:30 EDT = 2026-09-25 01:30Z.
        let now = days_from_civil(2026, 9, 25) * DAY + HOUR + 30 * 60;
        assert_eq!(
            today(now).since as i64,
            midnight(2026, 9, 24),
            "still the 24th in Eastern"
        );
        assert_eq!(this_month(now).since as i64, midnight(2026, 9, 1));
        let last = last_month(now);
        assert_eq!(last.since as i64, midnight(2026, 8, 1));
        assert_eq!(last.until as i64, midnight(2026, 9, 1));
        assert_eq!(month(2025, 12).until as i64, midnight(2026, 1, 1));
        assert_eq!(date_label(secs(now)), "2026-09-24");
        assert_eq!(time_label(secs(now)), "2026-09-24 21:30");
    }
}

//! Report periods in Eastern time, from server seconds.
//!
//! The loot cap resets on the first of the month at midnight Eastern, and
//! `today` means since midnight Eastern, because the game's clock is
//! Eastern. The rows are stamped in server seconds -- Unix seconds, from
//! `<prompt time=>` -- so a period is two of those. The calendar itself is
//! [`cena_platform::eastern`], moved down from here when the player log's
//! archives needed it too.

use cena_platform::eastern::{DAY, HOUR, civil_from_days, date as local_date, midnight, offset};
use cena_session::ledger::report::Period;

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

/// Since midnight Eastern today.
#[must_use]
pub(crate) fn today(now: i64) -> Period {
    let (y, m, d) = local_date(now);
    Period {
        since: secs(midnight((y, m, d))),
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
        since: secs(midnight((y, m, 1))),
        until: secs(now),
    }
}

/// A whole calendar month, Eastern.
#[must_use]
pub(crate) fn month(y: i64, m: i64) -> Period {
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    Period {
        since: secs(midnight((y, m, 1))),
        until: secs(midnight((ny, nm, 1))),
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
    fn periods_from_a_september_evening() {
        // 2026-09-24 21:30 EDT = 2026-09-25 01:30Z.
        let now = cena_platform::eastern::days_from_civil((2026, 9, 25)) * DAY + HOUR + 30 * 60;
        assert_eq!(
            today(now).since as i64,
            midnight((2026, 9, 24)),
            "still the 24th in Eastern"
        );
        assert_eq!(this_month(now).since as i64, midnight((2026, 9, 1)));
        let last = last_month(now);
        assert_eq!(last.since as i64, midnight((2026, 8, 1)));
        assert_eq!(last.until as i64, midnight((2026, 9, 1)));
        assert_eq!(month(2025, 12).until as i64, midnight((2026, 1, 1)));
        assert_eq!(date_label(secs(now)), "2026-09-24");
        assert_eq!(time_label(secs(now)), "2026-09-24 21:30");
    }
}

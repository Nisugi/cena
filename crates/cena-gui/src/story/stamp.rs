//! When a line arrived, on the player's clock, and how a widget of lines
//! says it: the author, 2026-09-28, *"timestamp should also offer the
//! granularity, XX:XX, XX:XX:XX, XX:XX:XX AM/PM, 12/24 hour"*. `VellumFE`
//! stamps a line as it arrives and says `7:08 AM`
//! (`frontend/tui/text_window.rs`, `format_timestamp_start`); this does the
//! same, and says it with seconds or not, on either clock.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

/// When a line arrived: the hour, minute and second on the player's clock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Stamp {
    hour: u8,
    minute: u8,
    second: u8,
}

/// Which clock a time is said on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Hours {
    /// 1 to 12, with AM or PM: `VellumFE`'s.
    #[default]
    Twelve,
    /// 00 to 23.
    TwentyFour,
}

impl Stamp {
    /// Now, on the player's clock.
    pub(crate) fn now() -> Self {
        let now = jiff::Zoned::now();
        let part = |value: i8| u8::try_from(value).unwrap_or(0);
        Self {
            hour: part(now.hour()),
            minute: part(now.minute()),
            second: part(now.second()),
        }
    }

    /// `hour`:`minute`:`second`, as a test sets one.
    #[cfg(test)]
    pub(crate) const fn at(hour: u8, minute: u8, second: u8) -> Self {
        Self {
            hour,
            minute,
            second,
        }
    }

    /// As a widget says it, with `seconds` or not, on `hours`' clock:
    /// `7:08 PM`, `7:08:05 PM`, `19:08`, `19:08:05`.
    pub(crate) fn said(self, seconds: bool, hours: Hours) -> String {
        let mut said = match hours {
            Hours::Twelve => {
                let hour = match self.hour % 12 {
                    0 => 12,
                    hour => hour,
                };
                format!("{hour}:{:02}", self.minute)
            }
            Hours::TwentyFour => format!("{:02}:{:02}", self.hour, self.minute),
        };
        if seconds {
            let _ = write!(said, ":{:02}", self.second);
        }
        if hours == Hours::Twelve {
            said.push_str(if self.hour < 12 { " AM" } else { " PM" });
        }
        said
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each of the four ways, and the two hours a 12-hour clock calls 12.
    #[test]
    fn a_time_is_said_on_either_clock_with_seconds_or_not() {
        let evening = Stamp::at(19, 8, 5);
        assert_eq!(evening.said(false, Hours::Twelve), "7:08 PM");
        assert_eq!(evening.said(true, Hours::Twelve), "7:08:05 PM");
        assert_eq!(evening.said(false, Hours::TwentyFour), "19:08");
        assert_eq!(evening.said(true, Hours::TwentyFour), "19:08:05");
        assert_eq!(Stamp::at(0, 30, 0).said(false, Hours::Twelve), "12:30 AM");
        assert_eq!(Stamp::at(12, 0, 0).said(false, Hours::Twelve), "12:00 PM");
        assert_eq!(Stamp::at(7, 5, 9).said(true, Hours::TwentyFour), "07:05:09");
    }
}

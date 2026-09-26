//! The interaction monitor (`monitor_interaction`, `bigshot.lic:6797-6826`):
//! a line from any window that matches one of `monitor.strings` and none of
//! `monitor.safe` is put in front of the player. bigshot pops a window and
//! echoes `AUTOBOT ALERT:`; the hunt keeps going either way.
//!
//! The patterns are bigshot's, joined by `||` in its profile and read as one
//! case-insensitive alternation (its `/io`). With the monitor on and no
//! strings of its own, the profile gets bigshot's default list (`:556`).
//! A pattern that is not a valid expression switches the monitor off and
//! says so once, rather than alerting on nothing.

use regex::{Regex, RegexBuilder};

use super::engine::Hunt;

/// bigshot's default `monitor_strings` (`bigshot.lic:556`), for a profile
/// that turns the monitor on without its own.
pub(super) const DEFAULT_STRINGS: &[&str] = &[
    "SEND",
    "POLICY",
    r"[Rr](\s)*[Ee](\s)*[Pp](\s)*[Oo](\s)*[Rr](\s)*[Tt]",
    "speaking to you",
    "unresponsive",
    "taps you",
    "nods to you",
    "lease respond",
    "not in control",
    "violation",
    "lease speak",
    "peak out loud",
    "Y U SHOU D",
    "whispers,",
    "speaking to you",
    "smiles at you",
    "waves to you",
    "grins at you",
    "hugs you",
    "takes hold your hand",
    "grabs your hand",
    "clasps your hand",
    "trying to drag you",
];

/// The compiled watch and safe lists.
#[derive(Debug, Default)]
pub(super) enum Watch {
    /// Not compiled yet.
    #[default]
    Unread,
    /// The monitor is off, or its patterns did not compile.
    Off,
    /// Watching: the alert list, and the lines exempt from it.
    On(Regex, Option<Regex>),
}

impl Hunt {
    /// One whole line of the story, from any window.
    pub fn watched(&mut self, line: &str) {
        if matches!(self.watch, Watch::Unread) {
            self.watch = self.compile_watch();
        }
        if let Watch::On(alert, safe) = &self.watch
            && alert.is_match(line)
            && !safe.as_ref().is_some_and(|safe| safe.is_match(line))
        {
            self.alerts.push(line.trim().to_owned());
        }
    }

    /// What the monitor wants put in front of the player since it was
    /// last asked.
    pub fn take_alerts(&mut self) -> Vec<String> {
        std::mem::take(&mut self.alerts)
    }

    fn compile_watch(&mut self) -> Watch {
        let monitor = &self.profile.monitor;
        if !monitor.interaction {
            return Watch::Off;
        }
        let strings: Vec<&str> = if monitor.strings.is_empty() {
            DEFAULT_STRINGS.to_vec()
        } else {
            monitor.strings.iter().map(String::as_str).collect()
        };
        let safe: Vec<&str> = monitor.safe.iter().map(String::as_str).collect();
        let alert = either(&strings);
        let safe = (!safe.is_empty()).then(|| either(&safe));
        match (alert, safe.transpose()) {
            (Ok(alert), Ok(safe)) => Watch::On(alert, safe),
            (Err(why), _) | (_, Err(why)) => {
                self.notes.push(format!(
                    "the interaction monitor is off: a pattern does not read ({why})"
                ));
                Watch::Off
            }
        }
    }
}

/// The patterns as one case-insensitive alternation.
fn either(patterns: &[&str]) -> Result<Regex, regex::Error> {
    RegexBuilder::new(&patterns.join("|"))
        .case_insensitive(true)
        .build()
}

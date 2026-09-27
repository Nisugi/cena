//! The flags a character's triggers set (`plan/45` §3b, Stage 2): named,
//! set until cleared or for so many of the game's seconds, and read by the
//! guard word `flag "<name>"` -- so a line's trigger can feed a condition,
//! or hold back a hunt's step. `VellumFE`'s text rules could set a status
//! its conditions never read (`plan/45` §2a); here they read the same one.
//!
//! **Not what the game said, but what the player's own triggers concluded
//! from it.** Kept in the game state all the same, because every reader of
//! the state must agree on it: the session sets a flag and publishes the
//! change, and a behavior folding the session's events makes the same change
//! to its own copy. A reconnect keeps them; they are the player's, not the
//! game's.
//!
//! Time is the game's clock, as every other expiry in the model is: a flag
//! set for 30 seconds holds until the game second 30 after the one it was
//! set in. Set for a time before the clock is known, when it ends cannot be
//! said, and it reads as unknown until it is set again or cleared.

use std::collections::BTreeMap;

/// Every flag set, by its name lowercased.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Flags {
    set: BTreeMap<String, Until>,
}

/// Until when a flag holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Until {
    /// Until it is cleared.
    Cleared,
    /// Until this game second.
    Second(u32),
    /// For a time begun before the game's clock was known.
    Unknown,
}

/// One change to the flags: what a trigger's flag makes, and what the session
/// publishes so every copy of the state makes it too.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlagChange {
    /// The flag's name, as the trigger wrote it.
    pub name: String,
    /// Set until then; `None` clears it.
    pub until: Option<Until>,
}

impl Flags {
    /// Make `change`; whether it changed anything.
    pub fn apply(&mut self, change: &FlagChange) -> bool {
        let name = change.name.to_lowercase();
        match change.until {
            Some(until) => self.set.insert(name, until) != Some(until),
            None => self.set.remove(&name).is_some(),
        }
    }

    /// Whether `name` holds at game second `now`, ignoring case: `None` when
    /// it was set for a time and when that time ends cannot be said.
    #[must_use]
    pub fn holds(&self, name: &str, now: Option<u32>) -> Option<bool> {
        match self.set.get(&name.to_lowercase()) {
            None => Some(false),
            Some(Until::Cleared) => Some(true),
            Some(Until::Second(end)) => now.map(|now| now < *end),
            Some(Until::Unknown) => None,
        }
    }

    /// Every flag set, by name, lowercased, and until when; one whose time
    /// has run out is listed until it is set again or cleared.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Until)> {
        self.set.iter().map(|(name, until)| (name.as_str(), *until))
    }
}

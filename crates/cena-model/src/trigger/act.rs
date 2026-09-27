//! Act: a trigger's line, sent as if the player typed it (`plan/45` Stage
//! 5). The author's answer (§1 row 1): a trigger may act, *"b is fine"* --
//! from the player's own rules, never from one that came from elsewhere
//! until the player approves it, which the file sees to
//! (`cena_behavior::triggers`).
//!
//! Resolved here into an [`Act`], as attention is; the session paces it and
//! publishes it, and the binary sends it through the `;` command table first
//! and otherwise to the game, marked as a trigger's so a log can tell it from
//! the player's. It never counts as the player being there.
//!
//! **Paced twice.** A trigger acts at most once in its `cooldown`, the one
//! its attention keeps ([`super::Cooldowns`]); and a character's triggers
//! send at most [`MAX_SENDS`] lines in [`SEND_WINDOW`] game seconds between
//! them ([`Pace`]), so a trigger that answers itself -- its line's reply
//! matching its own words -- or two that answer each other are held back
//! and said, not looped. `VellumFE`'s triggers never send, so this has no
//! precedent there; the numbers are CLAUDE'S, to confirm (`plan/45` §6e).

use std::collections::VecDeque;

use super::{Hit, Matcher};

/// Lines a character's triggers may send in [`SEND_WINDOW`] game seconds.
pub const MAX_SENDS: usize = 5;

/// The game seconds [`MAX_SENDS`] is counted over.
pub const SEND_WINDOW: u32 = 10;

/// A trigger's line to send, resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act {
    /// The trigger, by name.
    pub trigger: String,
    /// The line, as if typed: a Hydra command, or the game's.
    pub line: String,
    /// Seconds before this trigger acts again.
    pub cooldown: u32,
}

/// When a character's triggers last sent, by the game's clock.
#[derive(Debug, Clone, Default)]
pub struct Pace {
    sent: VecDeque<u32>,
    /// When sends held back were last said, so a runaway is said once a
    /// window, not once a line.
    said: Option<u32>,
}

impl Pace {
    /// `acts` split into those sent at game second `now` and those held back
    /// because [`MAX_SENDS`] have gone in the last [`SEND_WINDOW`]. With the
    /// clock unknown, none is sent: a pace nobody can measure is no pace.
    pub fn admit(&mut self, acts: Vec<Act>, now: Option<u32>) -> (Vec<Act>, Vec<Act>) {
        let Some(now) = now else {
            return (Vec::new(), acts);
        };
        while self
            .sent
            .front()
            .is_some_and(|&at| now.saturating_sub(at) >= SEND_WINDOW)
        {
            self.sent.pop_front();
        }
        let room = MAX_SENDS.saturating_sub(self.sent.len());
        let mut acts = acts;
        let held = acts.split_off(room.min(acts.len()));
        self.sent.extend(std::iter::repeat_n(now, acts.len()));
        (acts, held)
    }

    /// Whether sends held back at game second `now` are to be said: once in
    /// each [`SEND_WINDOW`], and always when the clock is unknown.
    pub fn say_held(&mut self, now: Option<u32>) -> bool {
        let Some(now) = now else {
            return true;
        };
        let due = self
            .said
            .is_none_or(|said| now.saturating_sub(said) >= SEND_WINDOW);
        if due {
            self.said = Some(now);
        }
        due
    }
}

impl Matcher {
    /// The line trigger `rank` sends: at `hit` in the line `text`, its
    /// groups filled in, or, with no hit, for a condition. `None` when it
    /// sends none.
    pub(super) fn act(&self, rank: usize, hit: Option<&Hit>, text: &str) -> Option<Act> {
        let trigger = self.triggers.get(rank)?;
        let rule = &trigger.rule;
        let template = rule.send.as_ref()?;
        let line = match hit {
            Some(hit) => self.expand(hit, template, text),
            None => template.clone(),
        };
        Some(Act {
            trigger: trigger.name.clone(),
            line,
            cooldown: rule.cooldown,
        })
    }

    /// The line condition `rank` sends when it fires.
    #[must_use]
    pub fn condition_act(&self, rank: usize) -> Option<Act> {
        self.act(rank, None, "")
    }
}

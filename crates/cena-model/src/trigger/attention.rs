//! Attention: a trigger's sound, notification and banner (`plan/45` §3b,
//! Stage 3). Resolved here, where the hit and its groups are, into an
//! [`Attention`] the session publishes; played or shown by whoever reads
//! it -- the binary's desk for a sound and a notification, a viewer for a
//! banner -- so a session nobody watches still sounds.
//!
//! **It fires on the line as the game sent it**, squelched or not: *"hide
//! this but tell me"* (`plan/45` §4). `true` says that line, and a
//! condition, which has no line, says its trigger's name. Words say
//! themselves, a regex's `$1` filled in as a substitute's is.
//!
//! **A trigger's attention comes at most once in its `cooldown`**, kept per
//! character by the session ([`Cooldowns`]) on the game's clock, as every
//! expiry in the model is: `VellumFE`'s per-rule cooldown
//! (`reference/VellumFE/src/core/alerts.rs:197-204`). A thing several
//! characters see at once is the binary's to sound once.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::{Hit, Matcher};

/// What a notification or a banner says (`notify`, `alert`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "RawSay")]
pub enum Say {
    /// `true`: the line, or a condition's name.
    Line,
    /// These words; with a regex, `$1` is a group.
    Words(String),
}

/// A say as written: `true`, or words.
#[derive(Deserialize)]
#[serde(untagged)]
enum RawSay {
    On(bool),
    Words(String),
}

impl TryFrom<RawSay> for Say {
    type Error = String;

    fn try_from(raw: RawSay) -> Result<Self, String> {
        match raw {
            RawSay::On(true) => Ok(Self::Line),
            RawSay::On(false) => Err("`false` says nothing: leave it out".into()),
            RawSay::Words(words) if words.trim().is_empty() => {
                Err("says nothing: give it words, or `true` for the line".into())
            }
            RawSay::Words(words) => Ok(Self::Words(words)),
        }
    }
}

/// One trigger's call for attention, as the session publishes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attention {
    /// The trigger, by name: what its cooldown is kept by.
    pub trigger: String,
    /// The sound to play, as the trigger names it: a file in the sounds
    /// folder, or a path.
    pub sound: Option<String>,
    /// What the OS notification says.
    pub notify: Option<String>,
    /// What the banner says.
    pub alert: Option<String>,
    /// Seconds before this trigger's attention comes again.
    pub cooldown: u32,
}

/// When each trigger last did what it does beyond the line -- its attention
/// and its send, together -- by the game's clock: one per character and per
/// set of triggers, so new triggers start cool.
#[derive(Debug, Clone, Default)]
pub struct Cooldowns {
    last: BTreeMap<String, u32>,
}

impl Cooldowns {
    /// Whether `trigger` may call for attention or send at game second
    /// `now`, `cooldown` seconds after it last did; when it may, its
    /// cooldown starts now. With the clock unknown it may, and none starts.
    pub fn admit(&mut self, trigger: &str, cooldown: u32, now: Option<u32>) -> bool {
        let Some(now) = now else {
            return true;
        };
        let cooling = self
            .last
            .get(trigger)
            .is_some_and(|&last| now.saturating_sub(last) < cooldown);
        if !cooling {
            self.last.insert(trigger.to_owned(), now);
        }
        !cooling
    }
}

impl Matcher {
    /// The attention trigger `rank` calls for: at `hit` in the line `text`,
    /// or, with no hit, for a condition. `None` when it calls for none.
    pub(super) fn attention(
        &self,
        rank: usize,
        hit: Option<&Hit>,
        text: &str,
    ) -> Option<Attention> {
        let trigger = self.triggers.get(rank)?;
        let rule = &trigger.rule;
        if rule.sound.is_none() && rule.notify.is_none() && rule.alert.is_none() {
            return None;
        }
        let said = |say: &Say| match (say, hit) {
            (Say::Line, Some(_)) => text.to_owned(),
            (Say::Line, None) => trigger.name.clone(),
            (Say::Words(words), Some(hit)) => self.expand(hit, words, text),
            (Say::Words(words), None) => words.clone(),
        };
        Some(Attention {
            trigger: trigger.name.clone(),
            sound: rule.sound.clone(),
            notify: rule.notify.as_ref().map(said),
            alert: rule.alert.as_ref().map(said),
            cooldown: rule.cooldown,
        })
    }

    /// The attention condition `rank` calls for when it fires.
    #[must_use]
    pub fn condition_attention(&self, rank: usize) -> Option<Attention> {
        self.attention(rank, None, "")
    }
}

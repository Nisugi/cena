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
use std::ops::Range;

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
    /// The byte spans of [`Self::line`] the regex's groups filled in: text
    /// the game sent, which may be another player's words. In order; empty
    /// for a line sent as written. What may be sent knowing them is the
    /// binary's (`crates/cena/src/triggers/act.rs`): a group may fill in what
    /// a command is given, never choose the command (the crate review of
    /// 2026-10-01, MO-F-3).
    pub captured: Vec<Range<usize>>,
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
        let (line, captured) = match hit.and_then(|hit| self.captures(hit, text)) {
            Some(captures) => expand_marked(&captures, template),
            None => (template.clone(), Vec::new()),
        };
        Some(Act {
            trigger: trigger.name.clone(),
            line,
            captured,
            cooldown: rule.cooldown,
        })
    }

    /// The line condition `rank` sends when it fires.
    #[must_use]
    pub fn condition_act(&self, rank: usize) -> Option<Act> {
        self.act(rank, None, "")
    }
}

/// `template` with `captures`' groups filled in, exactly as
/// [`regex::Captures::expand`] fills it, and the spans of the line each
/// group filled.
///
/// The template is read as the regex crate reads it (`$$` a `$`; `${name}`;
/// `$` and the longest run of letters, digits and `_`; any other `$` as
/// written) and each reference is filled by `expand` itself, so the line is
/// the crate's to the byte (`a_send_marks_what_its_groups_filled_in`).
fn expand_marked(captures: &regex::Captures<'_>, template: &str) -> (String, Vec<Range<usize>>) {
    let (mut line, mut filled) = (String::new(), Vec::new());
    let mut rest = template;
    while let Some(at) = rest.find('$') {
        line.push_str(&rest[..at]);
        rest = &rest[at..];
        if rest[1..].starts_with('$') {
            line.push('$');
            rest = &rest[2..];
            continue;
        }
        let Some(end) = reference_end(rest) else {
            line.push('$');
            rest = &rest[1..];
            continue;
        };
        let start = line.len();
        captures.expand(&rest[..end], &mut line);
        if line.len() > start {
            filled.push(start..line.len());
        }
        rest = &rest[end..];
    }
    line.push_str(rest);
    (line, filled)
}

/// Where the group reference at the start of `rest` (its `$` first) ends,
/// as the regex crate reads one; `None` when the `$` starts none.
fn reference_end(rest: &str) -> Option<usize> {
    let after = rest.get(1..)?;
    if let Some(braced) = after.strip_prefix('{') {
        // `$`, `{`, the name, `}`.
        return braced.find('}').map(|close| close + 3);
    }
    let run = after
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count();
    (run > 0).then_some(run + 1)
}

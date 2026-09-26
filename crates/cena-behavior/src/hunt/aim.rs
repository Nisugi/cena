//! Where an attack is aimed: bigshot's `ambush` and `archery_aim` lists
//! (`bigshot.lic:6541-6570`, `:6359-6385`).
//!
//! A routine step `ambush`, or `ambush <part>`, becomes `ambush #<id>
//! <part>` when hidden and `attack #<id> <part>` when not, the part the
//! next on the `aim.ambush` list (bigshot's default, head, right leg, left
//! leg, chest, when the list is empty). The game refusing the part -- `You
//! cannot aim that high!`, `does not have a head!`, `is already missing
//! that!` -- moves to the next; past the end, `chest`.
//!
//! A step that begins with `fire`, with `aim.archery` set, is preceded by
//! `aim <part>` whenever the bow is not already aimed there (the game's
//! `You're now aiming at the <part> of`, `state/incident.rs`), skipping a
//! part an arrow is already stuck in.
//!
//! bigshot's third list, `aim`, is for unarmed combat (`cmd_ucs`,
//! `:5479-5529`), which waits on `plan/33`'s UCS guards; it is not imported.

use super::engine::Hunt;

/// bigshot's ambush parts when the list is empty (`bigshot.lic:6552`).
const AMBUSH_DEFAULT: &[&str] = &["head", "right leg", "left leg", "chest"];

/// Where the aim lists stand for the current target.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct Aiming {
    /// The next part on the list to try.
    at: usize,
    /// Where the bow is aimed, as the game last said.
    pub(super) aimed: Option<String>,
    /// Parts an arrow is stuck in, on this target.
    pub(super) stuck: Vec<String>,
}

impl Aiming {
    /// A new target: every list from the top.
    pub(super) fn reset(&mut self) {
        self.at = 0;
        self.stuck.clear();
    }

    /// The game refused the part: the next one.
    pub(super) fn refused(&mut self) {
        self.at += 1;
    }

    /// The place on the list reached.
    pub(super) const fn at(&self) -> usize {
        self.at
    }
}

/// The line a step becomes once aimed, and a step to send before it.
pub(super) enum Aimed {
    /// Send this instead of the step.
    Instead(String),
    /// Send this first; the step waits for the next tick.
    First(String),
}

impl Hunt {
    /// What aiming makes of `send`, against `target`, `hidden` or not.
    /// `None`: the step goes as written.
    pub(super) fn aim(&self, send: &str, target: i64, hidden: bool) -> Option<Aimed> {
        let aim = &self.profile.aim;
        let (verb, named) = send.trim().split_once(' ').unwrap_or((send.trim(), ""));
        if verb.eq_ignore_ascii_case("ambush") {
            // `ambush <part>` names the one part (`cmd_ambush`'s string
            // argument, `bigshot.lic:6554`).
            let parts: Vec<&str> = if !named.trim().is_empty() {
                vec![named.trim()]
            } else if aim.ambush.is_empty() {
                AMBUSH_DEFAULT.to_vec()
            } else {
                aim.ambush.iter().map(String::as_str).collect()
            };
            let part = parts.get(self.aiming.at).copied().unwrap_or("chest");
            let verb = if hidden { "ambush" } else { "attack" };
            return Some(Aimed::Instead(format!("{verb} #{target} {part}")));
        }
        let fire = send
            .split_whitespace()
            .next()
            .is_some_and(|verb| verb.eq_ignore_ascii_case("fire"));
        if !fire || aim.archery.is_empty() {
            return None;
        }
        let part = aim
            .archery
            .iter()
            .skip(self.aiming.at)
            .find(|part| {
                let part = part.to_ascii_lowercase();
                !self
                    .aiming
                    .stuck
                    .iter()
                    .any(|s| part.contains(&s.to_ascii_lowercase()))
            })
            .or_else(|| aim.archery.first())?;
        let already = self
            .aiming
            .aimed
            .as_deref()
            .is_some_and(|aimed| aimed.eq_ignore_ascii_case(part));
        (!already).then(|| Aimed::First(format!("aim {part}")))
    }
}

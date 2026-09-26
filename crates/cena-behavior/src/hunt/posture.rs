//! How the hunter stands and in which stance each step goes: bigshot's
//! `stand` (`bigshot.lic:7064-7077`) and its hunting stance before a
//! command (`:4051-4053`).

use cena_session::GameState;

use super::engine::Hunt;
use super::said::{Ending, Said};

/// The steps that do not take the hunting stance first, by their first
/// word's start (`bigshot.lic:4051`).
const STANCE_EXEMPT: &[&str] = &[
    "wait",
    "sleep",
    "wand",
    "berserk",
    "script",
    "hide",
    "nudgeweapon",
];

impl Hunt {
    /// Dead: the hunt is over. Down, and able to move: stand, in the stand
    /// stance first when the profile names one (bigshot's `stand`,
    /// `bigshot.lic:7064-7077`), unless the kneel was a recovery's or an
    /// archer's with a crossbow ([`crossbow_kneel`]).
    pub(super) fn survival(&self, state: &GameState) -> Option<Said> {
        let recovering = self.recovering();
        let status = state.status.known();
        if status.dead() == Some(true) {
            return Some(Said::Done(Ending::Dead));
        }
        let down = status.standing() == Some(false)
            || status.prone() == Some(true)
            || status.sitting() == Some(true)
            || status.kneeling() == Some(true);
        let held = status.stunned() == Some(true) || status.webbed() == Some(true);
        if !down || held || recovering || crossbow_kneel(state) {
            return None;
        }
        let line = Self::stance_for(self.profile.stance.stand.as_deref(), state)
            .unwrap_or_else(|| "stand".to_owned());
        Some(Said::Send { line, target: None })
    }

    /// A line the step waits behind: the hunting stance, or the censer
    /// before a spell (`hunt/censer.rs`).
    pub(super) fn waits_behind(
        &mut self,
        send: &str,
        state: &GameState,
        target: i64,
        now: Option<u32>,
    ) -> Option<Said> {
        // Kneeling with a crossbow is kept only for what wants it.
        if crossbow_kneel(state) && !keeps_kneeling(send) {
            return Some(Said::Send {
                line: "stand".to_owned(),
                target: None,
            });
        }
        if let Some(line) = self.stance_before(send, state) {
            return Some(Said::Send {
                line,
                target: Some(target),
            });
        }
        self.censer_first(state, send, now)
            .map(|line| Said::Send { line, target: None })
    }

    /// The hunting stance, taken before a step that wants it: every step but
    /// a bare spell number, `wait`, `sleep`, `wand`, `berserk`, `script`,
    /// `hide` and `nudgeweapon`, bigshot's prefixes (`bigshot.lic:4051-4053`).
    fn stance_before(&self, send: &str, state: &GameState) -> Option<String> {
        let first = send.split_whitespace().next()?.to_ascii_lowercase();
        let exempt = first.starts_with(|c: char| c.is_ascii_digit())
            || STANCE_EXEMPT.iter().any(|word| first.starts_with(word));
        if exempt {
            return None;
        }
        Self::stance_for(self.profile.stance.hunting.as_deref(), state)
    }
}

/// Kneeling with a crossbow in the left hand, as an archer kneels to fire:
/// bigshot does not stand for `fire`, `kneel`, `hide` or 608 then
/// (`stand`, `bigshot.lic:7067`).
fn crossbow_kneel(state: &GameState) -> bool {
    state.status.known().kneeling() == Some(true)
        && state.left_hand.noun().is_some_and(|noun| {
            ["arbalest", "kut'ziko", "crossbow", "kut'zikokra"]
                .iter()
                .any(|bow| noun.eq_ignore_ascii_case(bow))
        })
}

/// A step bigshot sends kneeling with a crossbow: `fire`, `kneel`, `hide`,
/// or 608.
fn keeps_kneeling(send: &str) -> bool {
    let lower = send.trim().to_ascii_lowercase();
    ["fire", "kneel", "hide"]
        .iter()
        .any(|word| lower.starts_with(word))
        || lower == "608"
        || lower.starts_with("608 ")
        || lower.starts_with("incant 608")
}

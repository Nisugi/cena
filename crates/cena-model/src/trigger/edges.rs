//! A condition's memory: when its guard words last read true or false, so it
//! fires on the change and not while it lasts (`plan/45` §3a).
//!
//! `VellumFE`'s rules (`reference/VellumFE/src/core/alerts.rs:241-291`),
//! kept whole because each answers a way a warning loses trust:
//!
//! - **The first reading is taken silently.** A condition that already holds
//!   when the triggers load is not news: a character logging in hurt is not
//!   told so by every trigger at once.
//! - **It fires only on false to true**, however long it then stays true.
//! - **Once it has fired, it must stay false for `rearm` seconds** before it
//!   fires again, so health hovering on its line does not fire at every
//!   prompt. The first rise is not held back: a trigger saved mid-fight must
//!   work in that fight.
//!
//! **A reading the game has not given is no reading**: the memory is left as
//! it was, so a reconnect's unknown health neither fires nor re-arms. The
//! clock is the game's; where it is not known, a rise after a fire waits for
//! one where it is, unless `rearm` is 0.
//!
//! The memory is the session's, one per character and per set of triggers:
//! new triggers start new memory, so their first reading is silent too.

use super::Matcher;
use crate::GameState;
use crate::guard::{Condition, Facts};

/// What each condition of one set of triggers last read.
#[derive(Clone, Debug, Default)]
pub struct Edges {
    /// By rank in [`Matcher::triggers`].
    edges: Vec<Edge>,
}

#[derive(Clone, Copy, Debug, Default)]
struct Edge {
    /// The last reading, once there has been one.
    held: Option<bool>,
    /// The game second it last turned false; `None` while true, or when the
    /// clock was not known.
    false_since: Option<u32>,
    /// It has fired before, so `rearm` applies.
    fired: bool,
}

impl Edges {
    /// The conditions in `matcher` that fire now, for the character `state`
    /// is, by rank: each on its change to true, with its `only_if` holding.
    pub fn fire(&mut self, matcher: &Matcher, state: &GameState) -> Vec<usize> {
        let now = state.game_time_now();
        let facts = Facts::new(state, state.targeting.current());
        let mut fired = Vec::new();
        for &rank in matcher.conditions() {
            let Some(trigger) = matcher.triggers().get(rank) else {
                continue;
            };
            if self.edges.len() <= rank {
                self.edges.resize(rank + 1, Edge::default());
            }
            let Some(edge) = self.edges.get_mut(rank) else {
                continue;
            };
            let rule = &trigger.rule;
            let rose = edge.observe(reading(&rule.condition, &facts), rule.rearm, now);
            if rose && rule.only_if_holds(state) {
                edge.fired = true;
                fired.push(rank);
            }
        }
        fired
    }
}

impl Edge {
    /// Take one reading; whether it is a rise that may fire.
    fn observe(&mut self, reading: Option<bool>, rearm: u32, now: Option<u32>) -> bool {
        let Some(now_true) = reading else {
            return false;
        };
        let Some(was_true) = self.held.replace(now_true) else {
            self.false_since = if now_true { None } else { now };
            return false;
        };
        let armed = !self.fired
            || rearm == 0
            || self
                .false_since
                .zip(now)
                .is_some_and(|(since, now)| now.saturating_sub(since) >= rearm);
        if now_true {
            self.false_since = None;
        } else if was_true {
            self.false_since = now;
        }
        now_true && !was_true && armed
    }
}

/// Guard words read together: true when all hold, false when one does not,
/// unknown otherwise.
fn reading(condition: &[Condition], facts: &Facts<'_>) -> Option<bool> {
    let mut all = Some(true);
    for word in condition {
        match word.holds(facts) {
            Some(true) => {}
            Some(false) => return Some(false),
            None => all = None,
        }
    }
    all
}

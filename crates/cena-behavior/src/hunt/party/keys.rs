//! Three `[group]` keys read where a member fights (`plan/39` Stage 5):
//! `disable_commands` and `troubadours_rally`, each member's own, and
//! `final_loot`, the leader's.

use cena_session::GameState;

use super::super::engine::{Held, Hunt};
use super::super::profile::Step;
use super::super::said::Said;
use crate::cast::{self, NotReady};
use crate::group::{self, Role};

/// Troubadour's Rally.
const RALLY: u16 = 1040;

/// The room's words for a member Troubadour's Rally frees
/// (`group_status_ailments`, `bigshot.lic:6706`).
const RALLIED: &[&str] = &[
    "webbed",
    "sleeping",
    "stunned",
    "frozen",
    "immobilized",
    "held in place",
    "horrified",
    "staggered",
];

impl Hunt {
    /// The steps to run against the target: `disable_commands` when fried
    /// in a group (`find_routine`, `bigshot.lic:7166-7168`: *"DISABLE
    /// override always wins"*), else the target's routine.
    pub(in crate::hunt) fn routine_steps(&self, state: &GameState) -> Option<Vec<Step>> {
        let disabled = &self.profile.group.disable_commands;
        let grouped = matches!(self.role(), Some(Role::Lead | Role::Follow));
        let fried = self
            .profile
            .rest
            .fried
            .filter(|at| *at <= 100)
            .zip(state.character.experience.mind_percent)
            .is_some_and(|(at, mind)| mind >= at);
        if grouped && fried && !disabled.is_empty() {
            return Some(disabled.clone());
        }
        self.profile.routines.get(&self.routine).cloned()
    }

    /// Troubadour's Rally before a routine step, when it is known: stuck
    /// itself, or a member of its group here is (`group_status_ailments`,
    /// `bigshot.lic:6698-6712`; `cmd_1040`, `:6256-6273`, which pulses for
    /// mana when it cannot afford it).
    pub(in crate::hunt) fn rally(&self, state: &GameState) -> Option<Said> {
        if !self.profile.group.troubadours_rally
            || state.known_spells.knows(u32::from(RALLY)) != Some(true)
        {
            return None;
        }
        let known = state.status.known();
        let on = |value: Option<bool>| value == Some(true);
        let stuck = on(known.webbed()) || on(known.sleeping()) || on(known.stunned());
        let member = state.room.players.iter().any(|player| {
            group::in_group(state, &player.noun)
                && player
                    .status
                    .as_ref()
                    .is_some_and(|status| RALLIED.iter().any(|word| status.as_str().contains(word)))
        });
        if !stuck && !member {
            return None;
        }
        let line = match cast::ready(state, RALLY, 1, 0) {
            Ok(()) => format!("incant {RALLY}"),
            Err(NotReady::Mana(..)) => "mana pulse".to_owned(),
            Err(_) => return None,
        };
        Some(Said::Send { line, target: None })
    }

    /// `final_loot`: the leader, or a hunt alone, loots the room once more
    /// before leaving a room it held (`bs_wander`, `bigshot.lic:9435-9438`:
    /// *"any missed items by follower, dropped by poofed creatures, or left
    /// behind by other players"*). The loot profile's planner does it.
    pub(in crate::hunt) fn final_loot(&mut self) -> Option<Said> {
        if !self.profile.group.final_loot
            || self.loot.is_none()
            || self.role() == Some(Role::Follow)
            || self.held != Some(Held::Mine)
        {
            return None;
        }
        if self.grouping.final_looted == self.room {
            return None;
        }
        self.grouping.final_looted.clone_from(&self.room);
        Some(Said::Loot(Vec::new()))
    }
}

//! The rest arm of the hunt machine: the reasons to go, the walk there,
//! the wait, the walk back and the prepare commands (`plan/30` §3, the Rest
//! row of [`Hunt`]'s table). Split from `engine.rs` when the loot arm grew
//! to call the planner; the fields it reads are `pub(super)` for it.
//!
//! Two of the reasons are the loot planner's (`plan/31`): every bag full,
//! and a box left in hand. The driver hands them in through
//! [`Hunt::loot_ended`], and the next tick rests on them.

use cena_map::RoomId;
use cena_session::{Able, GameState, Injuries};

use super::engine::{Hunt, REST_BEAT};
use super::said::{Ending, Here, Phase, Said, Why};

impl Hunt {
    // --- rest -----------------------------------------------------------------

    /// The rest cycle: reasons to go, the walk there, the wait, the walk
    /// back, and the prepare commands.
    pub(super) fn rest(&mut self, state: &GameState, here: Here<'_>) -> Option<Said> {
        match self.phase {
            Phase::Hunting => {
                let why = self.rest_reason(state)?;
                self.must_rest = None;
                let Some(resting) = self.profile.rooms.resting else {
                    return Some(Said::Done(Ending::NoRestingRoom));
                };
                self.phase = Phase::ToRest(why);
                self.notes
                    .push(format!("{why}: walking to the resting room."));
                Some(self.step_toward(
                    RoomId(resting),
                    here,
                    Phase::Resting(why),
                    &self.profile.rest.commands.clone(),
                ))
            }
            Phase::ToRest(why) => {
                let resting = RoomId(self.profile.rooms.resting?);
                // Arrived with loot to sell: the round first, then the rest
                // (`plan/31` Stage 4; the author: *"sells typically happen
                // during the rest"*).
                if here.room == Some(resting) && self.wants_to_sell(state) {
                    self.phase = Phase::Selling(why);
                    self.notes.push("selling before resting.".to_owned());
                    return Some(Said::Sell);
                }
                if here.room == Some(resting) && self.wants_to_heal(state) {
                    self.phase = Phase::Healing(why);
                    self.notes.push("healing before resting.".to_owned());
                    return Some(Said::Heal);
                }
                Some(self.step_toward(
                    resting,
                    here,
                    Phase::Resting(why),
                    &self.profile.rest.commands.clone(),
                ))
            }
            Phase::Selling(why) => {
                // The round ended where it began; from anywhere else, the
                // walk back is the rest's first step. Hurt, the herbs come
                // before the rest commands (`plan/36`).
                let resting = RoomId(self.profile.rooms.resting?);
                if here.room == Some(resting) && self.wants_to_heal(state) {
                    self.phase = Phase::Healing(why);
                    self.notes.push("healing before resting.".to_owned());
                    return Some(Said::Heal);
                }
                Some(self.step_toward(
                    resting,
                    here,
                    Phase::Resting(why),
                    &self.profile.rest.commands.clone(),
                ))
            }
            Phase::Healing(why) => {
                let resting = RoomId(self.profile.rooms.resting?);
                Some(self.step_toward(
                    resting,
                    here,
                    Phase::Resting(why),
                    &self.profile.rest.commands.clone(),
                ))
            }
            Phase::Resting(why) => Some(self.resting(state, here, why)),
            Phase::Returning => {
                let hunting = RoomId(self.profile.rooms.hunting?);
                Some(self.step_toward(
                    hunting,
                    here,
                    Phase::Preparing,
                    &self.profile.prepare.clone(),
                ))
            }
            Phase::Preparing => {
                if let Some(line) = self.pending.pop_front() {
                    return Some(Said::Send { line, target: None });
                }
                self.phase = Phase::Hunting;
                self.notes.push("hunting.".to_owned());
                None
            }
        }
    }

    /// Whether the selling bags hold something a shop buys, by the loot
    /// profile's town settings; nothing to sell, or no loot profile, means
    /// straight to the rest.
    fn wants_to_sell(&self, state: &GameState) -> bool {
        let Some(profile) = &self.loot else {
            return false;
        };
        let Some(resting) = self.profile.rooms.resting else {
            return false;
        };
        let town = crate::town::Town::for_profile(profile);
        crate::town::Seller::new(town, state, RoomId(resting)).is_some()
    }

    /// Whether the heal profile has something to treat now.
    fn wants_to_heal(&self, state: &GameState) -> bool {
        self.heal
            .as_ref()
            .is_some_and(|profile| crate::heal::Healer::wanted(profile, state))
    }

    /// Walk toward `goal`; on arrival, move to `then` with `commands` to send.
    fn step_toward(
        &mut self,
        goal: RoomId,
        here: Here<'_>,
        then: Phase,
        commands: &[String],
    ) -> Said {
        if here.room == Some(goal) {
            self.phase = then;
            self.pending = commands.iter().cloned().collect();
            return match self.pending.pop_front() {
                Some(line) => Said::Send { line, target: None },
                None => Said::Wait(1),
            };
        }
        Said::Walk(goal)
    }

    /// At the rest room: the commands, the wait, then the walk back.
    fn resting(&mut self, state: &GameState, here: Here<'_>, why: Why) -> Said {
        if let Some(line) = self.pending.pop_front() {
            return Said::Send { line, target: None };
        }
        if let Some(still) = self.still_resting(state) {
            let _ = still;
            return Said::Wait(REST_BEAT);
        }
        self.fried_kills = 0;
        self.heard.rested_for_injury = why == Why::Injured;
        if let Some(ending) = self.count_rest() {
            return Said::Done(ending);
        }
        let Some(hunting) = self.profile.rooms.hunting else {
            return Said::Done(Ending::NoHuntingRoom);
        };
        self.phase = Phase::Returning;
        self.notes.push("rested: walking back.".to_owned());
        self.step_toward(
            RoomId(hunting),
            here,
            Phase::Preparing,
            &self.profile.prepare.clone(),
        )
    }

    /// One more rest done; the hunt's end when `rest.stop_after` is reached.
    fn count_rest(&mut self) -> Option<Ending> {
        self.rests = self.rests.saturating_add(1);
        let limit = self.profile.rest.stop_after?;
        (self.rests >= limit).then_some(Ending::Rested(self.rests))
    }

    /// Why to rest now, if a reason holds.
    fn rest_reason(&self, state: &GameState) -> Option<Why> {
        if let Some(why) = self.must_rest {
            return Some(why);
        }
        let rest = &self.profile.rest;
        if self.wounded(state) {
            return Some(Why::Wounded);
        }
        let mind = state.character.experience.mind_percent;
        let fried = rest
            .fried
            .filter(|at| *at <= 100)
            .zip(mind)
            .is_some_and(|(at, mind)| mind >= at);
        if fried && self.fried_kills >= rest.overkill {
            return Some(Why::Fried);
        }
        let heavy = rest
            .encumbered
            .zip(state.character.encumbrance_percent)
            .is_some_and(|(at, now)| now >= at);
        if heavy {
            return Some(Why::Encumbered);
        }
        let dry = rest
            .mana_below
            .zip(state.mana())
            .is_some_and(|(below, mana)| mana.percent < below);
        dry.then_some(Why::Mana)
    }

    /// `rest.when`: any one holding is enough.
    fn wounded(&self, state: &GameState) -> bool {
        let when = &self.profile.rest.when;
        if when.bleeding && state.status.known().bleeding() == Some(true) {
            return true;
        }
        if when
            .health_at_most
            .zip(state.health())
            .is_some_and(|(at_most, health)| health.percent <= at_most)
        {
            return true;
        }
        let injuries = Injuries::new(&state.character.injuries);
        (when.cannot_cast && injuries.able_to_cast() != Able::Yes)
            || (when.cannot_use_ranged && injuries.able_to_use_ranged() != Able::Yes)
            || when
                .creeping_dread
                .is_some_and(|at| debuff_stacks(state, "Creeping Dread").is_some_and(|n| n >= at))
            || when
                .crushing_dread
                .is_some_and(|at| debuff_stacks(state, "Crushing Dread").is_some_and(|n| n >= at))
            || (when.wot_poison && debuff_stacks(state, "Wall of Thorns Poison").is_some())
            || (when.confused && debuff_stacks(state, "Confused").is_some())
    }

    /// Why the rest is not over, or `None` when it is. A threshold whose
    /// vital the game has not stated keeps resting.
    fn still_resting(&self, state: &GameState) -> Option<&'static str> {
        if self.wounded(state) {
            return Some("wounded");
        }
        let until = &self.profile.rest.until;
        let mind = state.character.experience.mind_percent;
        if until
            .experience
            .is_some_and(|at| mind.is_none_or(|mind| mind > at))
        {
            return Some("mind still above threshold");
        }
        if until
            .mana
            .is_some_and(|at| state.mana().is_none_or(|v| v.percent < at))
        {
            return Some("mana still below threshold");
        }
        if until.spirit.is_some_and(|at| {
            state
                .spirit()
                .and_then(|v| v.current)
                .is_none_or(|spirit| spirit < i32::try_from(at).unwrap_or(i32::MAX))
        }) {
            return Some("spirit still below threshold");
        }
        if until
            .stamina
            .is_some_and(|at| state.stamina().is_none_or(|v| v.percent < at))
        {
            return Some("stamina still below threshold");
        }
        None
    }
}

/// A debuff on the character whose name holds `name`, with its stack
/// count: bigshot's `key.to_s[/\((\d+)\)/, 1].to_i`, so a debuff with no
/// count is 0 stacks. `None`: not on.
fn debuff_stacks(state: &GameState, name: &str) -> Option<u32> {
    state
        .effects
        .iter()
        .find(|(_, effect)| effect.category == "Debuffs" && effect.text.contains(name))
        .map(|(_, effect)| {
            effect
                .text
                .split_once('(')
                .and_then(|(_, rest)| rest.split_once(')'))
                .and_then(|(n, _)| n.trim().parse().ok())
                .unwrap_or(0)
        })
}

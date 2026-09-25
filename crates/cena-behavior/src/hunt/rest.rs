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
                Some(self.step_toward(
                    resting,
                    here,
                    Phase::Resting(why),
                    &self.profile.rest.commands.clone(),
                ))
            }
            Phase::Selling(why) => {
                // The round ended where it began; from anywhere else, the
                // walk back is the rest's first step.
                let resting = RoomId(self.profile.rooms.resting?);
                Some(self.step_toward(
                    resting,
                    here,
                    Phase::Resting(why),
                    &self.profile.rest.commands.clone(),
                ))
            }
            Phase::Resting(_) => {
                if let Some(line) = self.pending.pop_front() {
                    return Some(Said::Send { line, target: None });
                }
                if let Some(still) = self.still_resting(state) {
                    let _ = still;
                    return Some(Said::Wait(REST_BEAT));
                }
                self.fried_kills = 0;
                let Some(hunting) = self.profile.rooms.hunting else {
                    return Some(Said::Done(Ending::NoHuntingRoom));
                };
                self.phase = Phase::Returning;
                self.notes.push("rested: walking back.".to_owned());
                Some(self.step_toward(
                    RoomId(hunting),
                    here,
                    Phase::Preparing,
                    &self.profile.prepare.clone(),
                ))
            }
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
        let town = crate::town::Town::from_table(&profile.town);
        crate::town::Seller::new(town, state, RoomId(resting)).is_some()
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

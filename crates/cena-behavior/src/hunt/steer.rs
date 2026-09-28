//! A hunt steered from outside: hold, resume and retreat (`plan/35` §4, M7
//! step 3b).
//!
//! > **AUTHOR, 2026-09-27**, choosing among the proposals: hold is **"defend,
//! > start nothing"** -- survival, flee and rest still act; no new target, no
//! > looting, no buffs, no wandering, until resume or stop. Retreat is **"the
//! > rest room, then end"** -- walk to the profile's resting room now and end
//! > the hunt there, so the agent decides what is next.
//!
//! # What a hold keeps doing
//!
//! Every arm above the fighting ones, as they always act: an incident's
//! answer, survival (stand up), the group's, a rest the character needs,
//! and flee. **The creature already being fought is fought on**: the
//! author's words were *no new target*, and a creature attacking the
//! character is defended against. When it is gone, nothing takes its place.
//! Nothing is looted, no buff is cast, no room is wandered to, and a hunt
//! outside its rooms is not walked back. A rest that finishes while held
//! stays in the resting room, rather than walking back to hunt, until the
//! hunt is resumed; the rest is counted once, when it is resumed.
//!
//! # What a retreat does
//!
//! It wins over a hold and over a rest in progress: the hunt drops its
//! target, walks to the resting room, fogging and by waypoints as a rest
//! would, and ends there, [`Ending::Retreated`]. Arrived, it neither sells
//! nor heals nor rests: the agent asked for it to end.
//!
//! # Read once a tick, by the driver
//!
//! The controls are shared with whoever steers ([`Steering`]), and read by
//! [`Hunt::heed`], which the driver calls before each tick. [`Hunt::tick`]
//! still reads only what it is given.

use cena_map::RoomId;
use cena_session::GameState;

use super::engine::Hunt;
use super::said::{Ending, Here, Phase, Said, Why};
use crate::operation::Steering;

/// What the hunt's controls said at the last [`Hunt::heed`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Steered {
    pub(super) hold: bool,
    pub(super) retreat: bool,
}

impl Hunt {
    /// Steered by `steering`, read once a tick ([`Self::heed`]).
    #[must_use]
    pub fn steered_by(mut self, steering: Steering) -> Self {
        self.steering = Some(steering);
        self
    }

    /// Take in what the controls say now. The driver calls it before each
    /// tick; a hunt with no controls keeps what [`Self::hold`] and
    /// [`Self::retreat`] set.
    pub fn heed(&mut self) {
        if let Some(steering) = &self.steering {
            self.steered = Steered {
                hold: steering.held(),
                retreat: steering.retreating(),
            };
        }
    }

    /// Hold, or go on: for a hunt driven by hand, as a test drives it.
    pub fn hold(&mut self, on: bool) {
        self.steered.hold = on;
    }

    /// Retreat: for a hunt driven by hand.
    pub fn retreat(&mut self) {
        self.steered = Steered {
            hold: false,
            retreat: true,
        };
    }

    /// A retreat under way: to the resting room, and the end there. Asked
    /// before the rest arm, so a rest in progress gives way to it; `None`
    /// leaves the walk to the rest arm, as for any rest.
    pub(super) fn retreating(&mut self, here: Here<'_>) -> Option<Said> {
        if !self.steered.retreat {
            return None;
        }
        let Some(resting) = self.profile.rooms.resting.map(RoomId) else {
            return Some(Said::Done(Ending::NoRestingRoom));
        };
        if here.room == Some(resting) {
            return Some(Said::Done(Ending::Retreated));
        }
        if self.phase == Phase::ToRest(Why::Retreat) {
            return None;
        }
        self.phase = Phase::ToRest(Why::Retreat);
        self.field_rest = false;
        self.target_gone();
        self.pending.clear();
        self.notes
            .push("retreating to the resting room, as asked; the hunt ends there.".to_owned());
        self.set_out(Why::Retreat)
    }

    /// Held on the way back from a rest: it does not walk back or prepare to
    /// hunt again. Asked before the rest arm.
    pub(super) fn held_away(&self) -> Option<Said> {
        (self.steered.hold && matches!(self.phase, Phase::Returning | Phase::Preparing))
            .then_some(Said::Wait(1))
    }

    /// Whether a rest that is over should stay where it is: held.
    pub(super) const fn rest_held(&self) -> bool {
        self.steered.hold
    }

    /// Held in the hunting ground: the creature already being fought is
    /// fought on, and nothing new is begun. Asked after flee; `None` when
    /// not held.
    pub(super) fn held(
        &mut self,
        state: &GameState,
        here: Here<'_>,
        now: Option<u32>,
    ) -> Option<Said> {
        if !self.steered.hold {
            return None;
        }
        let fighting = self
            .target
            .filter(|id| self.fightable(state).any(|creature| creature.id == *id));
        let Some(target) = fighting else {
            self.target_gone();
            return Some(Said::Wait(1));
        };
        if self.phase != Phase::Hunting || !self.may_fight() || self.paused(now) {
            return Some(Said::Wait(1));
        }
        Some(
            self.fight(state, here, target, now)
                .unwrap_or(Said::Wait(1)),
        )
    }
}

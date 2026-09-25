//! The hunt's walking arms: leaving a room that is too crowded or holds
//! what the profile always flees, and wandering the ground between fights
//! (`plan/30` §3, the Flee and Wander rows of [`Hunt`]'s table). Split from
//! `engine.rs` as it neared its cap, as `rest.rs` was.

use cena_map::RoomId;
use cena_session::GameState;

use super::engine::{Hunt, listed};
use super::said::{Here, Phase, Said};

impl Hunt {
    // --- flee ---------------------------------------------------------------

    /// Too many fightable creatures, or one the profile always flees from.
    pub(super) fn flee(
        &mut self,
        state: &GameState,
        here: Here<'_>,
        now: Option<u32>,
    ) -> Option<Said> {
        if self.phase != Phase::Hunting {
            return None;
        }
        let flee = &self.profile.flee;
        let counted = self
            .could_fight(state)
            .filter(|creature| !listed(&flee.uncounted, creature))
            .count();
        let crowd = flee.count.is_some_and(|limit| counted > limit as usize);
        let always = state
            .creatures()
            .in_room()
            .any(|creature| listed(&flee.from, creature));
        if !(crowd || always) {
            return None;
        }
        let to = self.next_room(here, now)?;
        Some(Said::Walk(to))
    }

    // --- wander -------------------------------------------------------------------

    /// Nothing to fight: wait the profile's moment, take the wander stance,
    /// then walk to a fresh room.
    pub(super) fn wander(
        &mut self,
        state: &GameState,
        here: Here<'_>,
        now: Option<u32>,
    ) -> Option<Said> {
        // Unknown who is here: stay until the game says.
        let stay = self.fightable(state).next().is_some()
            && (self.held.is_none() && !self.in_sanctuary() || self.may_fight());
        if self.phase != Phase::Hunting || stay {
            return None;
        }
        let waited = self.arrived.zip(now).is_none_or(|(arrived, now)| {
            f64::from(now.saturating_sub(arrived)) >= self.profile.wander.wait
        });
        if !waited {
            return Some(Said::Wait(1));
        }
        if let Some(line) = Self::stance_for(self.profile.stance.wander.as_deref(), state) {
            return Some(Said::Send { line, target: None });
        }
        let to = self.next_room(here, now)?;
        Some(Said::Walk(to))
    }

    /// The next room to walk to: a crossable exit not on the boundary, a
    /// fresh one if any, else the one least recently visited (`flee.rb`'s
    /// `Walker`).
    pub(super) fn next_room(&mut self, here: Here<'_>, now: Option<u32>) -> Option<RoomId> {
        let room = here.room?;
        if !self.visited.contains(&room) {
            self.visited.push(room);
        }
        let boundaries = &self.profile.rooms.boundaries;
        let options: Vec<RoomId> = here
            .exits
            .iter()
            .copied()
            .filter(|exit| !boundaries.contains(&exit.0) && *exit != room)
            .collect();
        if options.is_empty() {
            return None;
        }
        let fresh: Vec<RoomId> = options
            .iter()
            .copied()
            .filter(|exit| !self.visited.contains(exit))
            .collect();
        let chosen = if fresh.is_empty() {
            self.visited
                .iter()
                .copied()
                .find(|seen| options.contains(seen))?
        } else {
            let roll = self.roll(now);
            let at = usize::try_from(roll % fresh.len() as u64).unwrap_or(0);
            fresh[at]
        };
        self.visited.retain(|seen| *seen != chosen);
        self.visited.push(chosen);
        Some(chosen)
    }

    /// The next number from the seed, mixed with the clock so two hunts on
    /// one seed do not walk in step.
    fn roll(&mut self, now: Option<u32>) -> u64 {
        let mut x = self.seed ^ u64::from(now.unwrap_or(0));
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        x ^= x >> 33;
        self.seed = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
        x
    }
}

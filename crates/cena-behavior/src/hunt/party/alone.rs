//! Travelling apart (`independent_travel`, `independent_return`; `plan/39`
//! Stage 5): the leader disbands before the walk, each member walks the
//! leader's rooms on its own, and they join again at the other end.
//!
//! | Key | The leader | A follower | bigshot |
//! |---|---|---|---|
//! | `independent_return` | `disband group` as the rest begins | its own fog home, the leader's waypoints, the leader's resting room | `PREP_REST`, `LEAVE_GROUP`, `FOG_RETURN`, `GO2_WAYPOINTS`, `GO2_RESTING_ROOM` (`bigshot.lic:7478-7496`) |
//! | `independent_travel` | `disband group` as the walk back begins | the leader's rally rooms, the leader's hunting room | `GO2_RALLY_ROOM`, `GO2_HUNTING_ROOM` (`:7246-7263`, `:7291`) |
//!
//! At the far end they join again: the leader's gather at the rest
//! (`hold_for_group`) and before it hunts (`gather`).

use cena_session::GameState;

use super::super::engine::Hunt;
use super::super::said::{Here, Phase, Said, Why};
use crate::group::Leading;

impl Hunt {
    /// Lead: `disband group` before a walk its members take apart, once per
    /// leg: the walk home of rest `rest` (`back` false), or the walk back
    /// out after it.
    pub(in crate::hunt) fn disband(
        &mut self,
        state: &GameState,
        rest: u32,
        back: bool,
    ) -> Option<Said> {
        let table = &self.profile.group;
        let apart = if back {
            table.independent_travel
        } else {
            table.independent_return
        };
        if !apart || state.group.is_empty() {
            return None;
        }
        let leg = (rest, back);
        if self.grouping.disbanded == Some(leg) {
            return None;
        }
        self.grouping.disbanded = Some(leg);
        Some(Said::Send {
            line: "disband group".to_owned(),
            target: None,
        })
    }

    /// Follow: whether the leader's walk now is one its members take apart.
    pub(super) fn apart(leading: &Leading) -> bool {
        match leading.phase {
            Some(Phase::ToRest(_) | Phase::Selling(_) | Phase::Healing(_)) => {
                leading.independent_return
            }
            Some(Phase::Returning) => leading.independent_travel,
            _ => false,
        }
    }

    /// Follow: the leader's walk, taken on its own: its own fog home, then
    /// the leader's waypoints or rally rooms, then the leader's room at the
    /// far end. `None` once there.
    pub(super) fn walk_apart(&mut self, here: Here<'_>, leading: &Leading) -> Option<Said> {
        let back = leading.phase == Some(Phase::Returning);
        let (goal, via, why) = if back {
            (leading.rooms.hunting?, &leading.rooms.rally, None)
        } else {
            let why = match leading.phase? {
                Phase::ToRest(why) | Phase::Selling(why) | Phase::Healing(why) => why,
                _ => Why::Wounded,
            };
            (leading.rooms.resting?, &leading.rooms.waypoints, Some(why))
        };
        let leg = (leading.rest, back);
        if self.grouping.walking != Some(leg) {
            self.grouping.walking = Some(leg);
            self.waypoints = via.iter().copied().collect();
            // Its own fog, as bigshot's follower runs its own `fog_return`.
            if let Some(why) = why
                && self.fogs(why)
            {
                self.pending = self.profile.rest.fog.iter().cloned().collect();
                self.fogged = false;
            }
        }
        if let Some(line) = self.pending.pop_front() {
            return Some(Said::Send { line, target: None });
        }
        if let Some(said) = self.next_waypoint(here) {
            return Some(said);
        }
        (here.room != Some(goal)).then_some(Said::Walk(goal))
    }
}

//! `Routine::MinotaurMaze`: the minotaur maze beneath the Landing.
//!
//! Upstream (`upstream_scripts/minotaur_maze.rb`; recogniser `minotaur_maze`
//! sets `target_room_id`, here always the goal, and `maze_rooms`): remember,
//! for each room, where each direction led. From a room pick, in order, a
//! direction known to lead to the target; an exit not yet tried; a direction
//! leading to a room from which the target is one move; any exit at random.
//! `move` it, record where it landed, and stop at the target. Landing outside
//! `maze_rooms` means the walker fell out: go back to the room it left and
//! carry on. When escorting a child, wait up to five seconds (`50.times {
//! sleep 0.1 }`) after each move for the child to catch up.
//!
//! **Deviations.** Upstream loops for ever; this stops at `MAX_TURNS` moves.
//! Upstream remembers in a global for the whole session; what this learns
//! is kept by the driver for the trip and lent to the next crossing
//! (`super::Kept`), as the Confluence's is. Upstream goes back by the one
//! `wayto` of the room it fell into; a solver is not shown the map, so this
//! asks for `Next::WalkTo` the room it left, and if that fails it ends and
//! the trip plans again. Upstream tells an escort by the bounty's text; this
//! tells it by a `child` standing with the walker before the move. A move
//! that fails is recorded as leading back to the room it was made from, as
//! upstream records it, so it is not tried twice.

use cena_map::RoomId;

use super::{MAX_TURNS, Next, Seen, Solver};

/// Upstream's `50.times { ...; sleep 0.1 }`.
const CHILD_WAITS: u32 = 50;
const CHILD_WAIT_MS: u64 = 100;

/// Per room, where each direction led, in the order learned. Upstream's
/// `$minotaur_maze_dirs`, kept between crossings (`super::Kept`).
pub(in crate::travel) type Learned = Vec<(RoomId, Vec<(String, RoomId)>)>;

pub(super) struct MinotaurMaze {
    rooms: Vec<RoomId>,
    learned: Learned,
    turns: u32,
    at: At,
}

enum At {
    Choosing,
    /// Moved `dir` from `start`; `child` came along, and has been waited for
    /// `waits` times.
    Moved {
        start: RoomId,
        dir: String,
        child: Option<String>,
        waits: u32,
    },
    /// Walking back to the maze after falling out of it.
    Back {
        child: Option<String>,
        waits: u32,
    },
}

impl MinotaurMaze {
    pub fn new(rooms: Vec<RoomId>, learned: Learned) -> Self {
        MinotaurMaze {
            rooms,
            learned,
            turns: 0,
            at: At::Choosing,
        }
    }

    fn from(&self, room: RoomId) -> &[(String, RoomId)] {
        self.learned
            .iter()
            .find(|(known, _)| *known == room)
            .map_or(&[], |(_, dirs)| dirs.as_slice())
    }

    fn record(&mut self, start: RoomId, dir: String, end: RoomId) {
        let at = self
            .learned
            .iter()
            .position(|(known, _)| *known == start)
            .unwrap_or_else(|| {
                self.learned.push((start, Vec::new()));
                self.learned.len() - 1
            });
        let Some((_, dirs)) = self.learned.get_mut(at) else {
            return;
        };
        match dirs.iter_mut().find(|(known, _)| *known == dir) {
            Some(known) => known.1 = end,
            None => dirs.push((dir, end)),
        }
    }

    /// Upstream's four choices, in its order.
    fn choose(&self, start: RoomId, seen: &Seen<'_>) -> Option<String> {
        let known = self.from(start);
        let exits = seen.walker.exits.as_deref().unwrap_or(&[]);
        let leads = |to: RoomId| self.from(to).iter().any(|(_, end)| *end == seen.goal);
        known
            .iter()
            .find(|(_, end)| *end == seen.goal)
            .map(|(dir, _)| dir)
            .or_else(|| {
                exits
                    .iter()
                    .find(|exit| known.iter().all(|(dir, _)| dir != *exit))
            })
            .or_else(|| {
                known
                    .iter()
                    .find(|(_, end)| leads(*end))
                    .map(|(dir, _)| dir)
            })
            .or_else(|| {
                let pick = usize::try_from(seen.random).unwrap_or(usize::MAX);
                exits.get(pick.checked_rem(exits.len())?)
            })
            .cloned()
    }

    fn choosing(&mut self, seen: &Seen<'_>) -> Next {
        let Some(start) = seen.here else {
            return Next::Done;
        };
        if start == seen.goal {
            return Next::Done;
        }
        if self.turns >= MAX_TURNS {
            return Next::Failed;
        }
        let Some(dir) = self.choose(start, seen) else {
            return Next::Failed;
        };
        self.turns += 1;
        self.at = At::Moved {
            start,
            dir: dir.clone(),
            child: child_here(seen),
            waits: 0,
        };
        Next::Go(dir)
    }
}

/// The id of the child standing with the walker, if one is.
fn child_here(seen: &Seen<'_>) -> Option<String> {
    let creatures = &seen.state.room.creatures;
    let child = creatures.iter().find(|npc| npc.noun == "child")?;
    Some(child.id.clone())
}

/// Whether to wait once more for the child to catch up.
fn waits_for(child: Option<&String>, waits: u32, seen: &Seen<'_>) -> bool {
    let creatures = &seen.state.room.creatures;
    child.is_some_and(|id| waits < CHILD_WAITS && creatures.iter().all(|npc| npc.id != *id))
}

impl Solver for MinotaurMaze {
    fn keep(self: Box<Self>, kept: &mut super::Kept) {
        kept.maze = self.learned;
    }

    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match std::mem::replace(&mut self.at, At::Choosing) {
            At::Choosing => self.choosing(seen),
            At::Moved {
                start,
                dir,
                child,
                waits,
            } => {
                if waits_for(child.as_ref(), waits, seen) {
                    self.at = At::Moved {
                        start,
                        dir,
                        child,
                        waits: waits + 1,
                    };
                    return Next::Pause(CHILD_WAIT_MS);
                }
                let Some(end) = seen.here else {
                    return Next::Done;
                };
                self.record(start, dir, end);
                if end == seen.goal {
                    return Next::Done;
                }
                if self.rooms.contains(&end) {
                    return self.choosing(seen);
                }
                self.at = At::Back { child, waits: 0 };
                Next::WalkTo(start)
            }
            At::Back { child, waits } => {
                if !seen.ok {
                    return Next::Done;
                }
                if waits_for(child.as_ref(), waits, seen) {
                    self.at = At::Back {
                        child,
                        waits: waits + 1,
                    };
                    return Next::Pause(CHILD_WAIT_MS);
                }
                self.choosing(seen)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use cena_session::RoomItem;

    use super::super::testing::Scene;
    use super::*;

    const GOAL: u32 = 9;

    fn maze() -> MinotaurMaze {
        MinotaurMaze::new(
            vec![RoomId(1), RoomId(2), RoomId(3), RoomId(GOAL)],
            Learned::new(),
        )
    }

    fn room(here: u32, exits: &[&str]) -> Scene {
        let mut scene = Scene::at(here, GOAL);
        scene.walker.exits = Some(exits.iter().map(|exit| (*exit).to_owned()).collect());
        scene
    }

    fn with_child(mut scene: Scene) -> Scene {
        scene.state.room.creatures.push(RoomItem {
            id: "77".into(),
            noun: "child".into(),
            text: "a small child".into(),
            before: None,
            after: None,
            status: None,
        });
        scene
    }

    fn go(dir: &str) -> Next {
        Next::Go(dir.into())
    }

    #[test]
    fn untried_exits_first_and_then_the_way_it_learned() {
        let mut solver = maze();
        assert_eq!(room(1, &["n", "e"]).ask(&mut solver), go("n"));
        // n led to 2; from 2, s leads back to 1.
        assert_eq!(room(2, &["s"]).ask(&mut solver), go("s"));
        // Back in 1: n is known, e is not.
        assert_eq!(room(1, &["n", "e"]).ask(&mut solver), go("e"));
        assert_eq!(room(GOAL, &["w"]).ask(&mut solver), Next::Done);
    }

    #[test]
    fn a_direction_known_to_reach_the_target_beats_an_untried_one() {
        let mut solver = maze();
        solver.record(RoomId(1), "e".into(), RoomId(GOAL));
        assert_eq!(room(1, &["n", "e"]).ask(&mut solver), go("e"));
    }

    #[test]
    fn with_nothing_untried_it_heads_for_a_room_one_move_from_the_target() {
        let mut solver = maze();
        solver.record(RoomId(1), "n".into(), RoomId(2));
        solver.record(RoomId(1), "e".into(), RoomId(3));
        solver.record(RoomId(3), "up".into(), RoomId(GOAL));
        assert_eq!(room(1, &["n", "e"]).ask(&mut solver), go("e"));
    }

    #[test]
    fn with_nothing_to_go_on_it_picks_an_exit_by_the_seed() {
        let mut solver = maze();
        solver.record(RoomId(1), "n".into(), RoomId(2));
        solver.record(RoomId(1), "e".into(), RoomId(3));
        let mut scene = room(1, &["n", "e"]);
        scene.random = 5;
        assert_eq!(scene.ask(&mut solver), go("e"));
    }

    #[test]
    fn falling_out_of_the_maze_walks_back_and_carries_on() {
        let mut solver = maze();
        assert_eq!(room(1, &["n", "e"]).ask(&mut solver), go("n"));
        assert_eq!(room(50, &["out"]).ask(&mut solver), Next::WalkTo(RoomId(1)));
        // n is remembered as the way out, so e is next.
        assert_eq!(room(1, &["n", "e"]).ask(&mut solver), go("e"));
    }

    #[test]
    fn no_way_back_ends_it_for_the_trip_to_plan_again() {
        let mut solver = maze();
        assert_eq!(room(1, &["n"]).ask(&mut solver), go("n"));
        assert_eq!(room(50, &[]).ask(&mut solver), Next::WalkTo(RoomId(1)));
        let mut lost = room(50, &[]);
        lost.failed = true;
        assert_eq!(lost.ask(&mut solver), Next::Done);
    }

    #[test]
    fn a_child_is_waited_for_until_it_arrives() {
        let mut solver = maze();
        assert_eq!(with_child(room(1, &["n"])).ask(&mut solver), go("n"));
        assert_eq!(room(2, &["s"]).ask(&mut solver), Next::Pause(100));
        assert_eq!(room(2, &["s"]).ask(&mut solver), Next::Pause(100));
        assert_eq!(with_child(room(2, &["s"])).ask(&mut solver), go("s"));
    }

    #[test]
    fn a_child_that_never_comes_is_waited_for_fifty_times() {
        let mut solver = maze();
        assert_eq!(with_child(room(1, &["n"])).ask(&mut solver), go("n"));
        for _ in 0..CHILD_WAITS {
            assert_eq!(room(2, &["s"]).ask(&mut solver), Next::Pause(100));
        }
        assert_eq!(room(2, &["s"]).ask(&mut solver), go("s"));
    }

    #[test]
    fn nobody_is_waited_for_when_no_child_came_along() {
        let mut solver = maze();
        assert_eq!(room(1, &["n"]).ask(&mut solver), go("n"));
        assert_eq!(room(2, &["s"]).ask(&mut solver), go("s"));
    }

    #[test]
    fn it_stops_after_the_walkers_bound() {
        let mut solver = maze();
        let mut asked = 0;
        // Two rooms that only lead to each other.
        let last = loop {
            let here = 1 + asked % 2;
            let next = room(here, &["n"]).ask(&mut solver);
            if next != go("n") || asked > MAX_TURNS + 5 {
                break next;
            }
            asked += 1;
        };
        assert_eq!(last, Next::Failed);
        assert_eq!(asked, MAX_TURNS);
    }

    #[test]
    fn a_room_with_no_exits_and_nothing_learned_cannot_be_left() {
        let mut solver = maze();
        assert_eq!(room(1, &[]).ask(&mut solver), Next::Failed);
    }

    #[test]
    fn an_unlocated_walker_ends_it() {
        let mut solver = maze();
        let mut scene = room(1, &["n"]);
        scene.here = None;
        assert_eq!(scene.ask(&mut solver), Next::Done);
    }
}

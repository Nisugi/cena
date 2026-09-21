//! `Routine::Signposts`: the underwater route off River's Rest (75 exits).
//!
//! Upstream (`recognise/routines.rs`, `signposts`): empty the hands if the
//! walk starts in one of a few rooms; then, until at the goal, `put "swim
//! <the direction this room says>"`, wait a second and the roundtime out. A
//! current may carry the walker elsewhere on the route, which is why the
//! direction is looked up afresh each stroke. A room with no signpost means
//! the walker is lost: upstream swims at random, **which is not copied**
//! (`cena_map::Routine::Signposts`) -- the routine ends and the trip replans.

use cena_map::{Action, RoomId, Step};

use super::{MAX_TURNS, Next, Seen, Solver};

pub(super) struct Signposts {
    verb: String,
    dirs: Vec<(RoomId, String)>,
    hands_free_in: Vec<RoomId>,
    begun: bool,
    hands_emptied: bool,
    turns: u32,
}

impl Signposts {
    pub fn new(verb: String, dirs: Vec<(RoomId, String)>, hands_free_in: Vec<RoomId>) -> Self {
        Signposts {
            verb,
            dirs,
            hands_free_in,
            begun: false,
            hands_emptied: false,
            turns: 0,
        }
    }

    /// Give the hands back, if they were emptied, and then end.
    fn end(&mut self) -> Next {
        if std::mem::take(&mut self.hands_emptied) {
            return Next::Steps(vec![step(Action::FillHands)]);
        }
        Next::Done
    }
}

fn step(action: Action) -> Step {
    Step { action, when: None }
}

impl Solver for Signposts {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        let Some(here) = seen.here else {
            return self.end();
        };
        if !std::mem::replace(&mut self.begun, true) && self.hands_free_in.contains(&here) {
            self.hands_emptied = true;
            return Next::Steps(vec![step(Action::EmptyHands)]);
        }
        if here == seen.goal || self.turns >= MAX_TURNS {
            return self.end();
        }
        let Some((_, dir)) = self.dirs.iter().find(|(room, _)| *room == here) else {
            return self.end();
        };
        self.turns += 1;
        Next::Put(format!("{} {dir}", self.verb))
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Scene;
    use super::*;

    fn route() -> Signposts {
        Signposts::new(
            "swim".into(),
            vec![(RoomId(1), "east".into()), (RoomId(2), "down".into())],
            vec![RoomId(1)],
        )
    }

    #[test]
    fn each_room_says_which_way_and_the_hands_come_back_at_the_end() {
        let mut swim = route();
        assert_eq!(
            Scene::at(1, 3).ask(&mut swim),
            Next::Steps(vec![step(Action::EmptyHands)])
        );
        assert_eq!(
            Scene::at(1, 3).ask(&mut swim),
            Next::Put("swim east".into())
        );
        // The current carried it nowhere: the same room says the same again.
        assert_eq!(
            Scene::at(1, 3).ask(&mut swim),
            Next::Put("swim east".into())
        );
        assert_eq!(
            Scene::at(2, 3).ask(&mut swim),
            Next::Put("swim down".into())
        );
        assert_eq!(
            Scene::at(3, 3).ask(&mut swim),
            Next::Steps(vec![step(Action::FillHands)])
        );
        assert_eq!(Scene::at(3, 3).ask(&mut swim), Next::Done);
    }

    #[test]
    fn hands_are_left_alone_when_the_walk_starts_elsewhere() {
        let mut swim = route();
        assert_eq!(
            Scene::at(2, 3).ask(&mut swim),
            Next::Put("swim down".into())
        );
        assert_eq!(Scene::at(3, 3).ask(&mut swim), Next::Done);
    }

    #[test]
    fn a_room_with_no_signpost_ends_it_and_nothing_is_guessed() {
        let mut swim = route();
        assert_eq!(Scene::at(9, 3).ask(&mut swim), Next::Done);
    }
}

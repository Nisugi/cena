//! `Routine::Patrol`: the Rift's ways out (570 exits).
//!
//! Upstream (`recognise/routines.rs`, `patrol`): find the walker's place on
//! the circuit -- its room's position in `starts` is the position in `dirs` to
//! begin from -- and `move dirs[index]`, round and round, until `checkloot`
//! shows a landmark. Some must be worked open first: up to five times, stand
//! if knocked down and `push fissure` until it "cannot be opened any
//! farther". Then go through, send what comes `after`, and plan again: where
//! the way out lands is not known in advance.

use cena_map::{Landmark, RoomId};

use super::{MAX_TURNS, Next, Seen, Solver};

pub(super) struct Patrol {
    starts: Vec<Option<RoomId>>,
    dirs: Vec<String>,
    landmarks: Vec<Landmark>,
    after: Vec<String>,
    /// The position in `dirs` of the next move. `None` until placed.
    index: Option<usize>,
    turns: u32,
    at: At,
}

enum At {
    Walking,
    /// Working landmark `.0` open: `.1` pushes made, and whether the last
    /// thing sent was the push (so its answer is worth reading).
    Opening(usize, u32, bool),
    Entered,
    After(usize),
}

impl Patrol {
    pub fn new(
        starts: Vec<Option<RoomId>>,
        dirs: Vec<String>,
        landmarks: Vec<Landmark>,
        after: Vec<String>,
    ) -> Self {
        Patrol {
            starts,
            dirs,
            landmarks,
            after,
            index: None,
            turns: 0,
            at: At::Walking,
        }
    }

    fn walk(&mut self, seen: &Seen<'_>) -> Next {
        // The first listed that is present wins.
        if let Some(found) = self
            .landmarks
            .iter()
            .position(|landmark| seen.sees_noun(&landmark.noun))
        {
            self.at = At::Opening(found, 0, false);
            return self.open(seen);
        }
        if self.index.is_none() {
            // Not on the circuit: lost, and the trip plans again.
            let placed = seen
                .here
                .and_then(|here| self.starts.iter().position(|room| *room == Some(here)));
            let Some(placed) = placed else {
                return Next::Done;
            };
            self.index = Some(placed % self.dirs.len().max(1));
        }
        let (Some(index), false) = (self.index, self.turns >= MAX_TURNS) else {
            return Next::Failed;
        };
        let Some(dir) = self.dirs.get(index) else {
            return Next::Failed;
        };
        self.turns += 1;
        self.index = Some((index + 1) % self.dirs.len());
        Next::Go(dir.clone())
    }

    fn open(&mut self, seen: &Seen<'_>) -> Next {
        let At::Opening(found, made, just_pushed) = self.at else {
            return Next::Failed;
        };
        let Some(landmark) = self.landmarks.get(found) else {
            return Next::Failed;
        };
        let wide_open = landmark
            .open
            .as_ref()
            .is_none_or(|open| (just_pushed && seen.answered(&open.until)) || made >= open.tries);
        if let (Some(open), false) = (&landmark.open, wide_open) {
            if seen
                .walker
                .posture
                .as_deref()
                .is_some_and(|is| is != "standing")
            {
                self.at = At::Opening(found, made, false);
                return Next::Put("stand".to_owned());
            }
            self.at = At::Opening(found, made + 1, true);
            return Next::Put(open.command.clone());
        }
        self.at = At::Entered;
        Next::Go(landmark.enter.clone())
    }
}

impl Solver for Patrol {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Walking => self.walk(seen),
            At::Opening(..) => self.open(seen),
            At::Entered => {
                self.at = At::After(0);
                self.next(seen)
            }
            At::After(sent) => match self.after.get(sent) {
                Some(command) => {
                    self.at = At::After(sent + 1);
                    Next::Put(command.clone())
                }
                None => Next::Done,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use cena_map::Opening;

    use super::super::testing::Scene;
    use super::*;

    fn landmark(noun: &str, open: Option<Opening>) -> Landmark {
        Landmark {
            noun: noun.into(),
            enter: format!("go {noun}"),
            open,
        }
    }

    fn circuit(landmarks: Vec<Landmark>, after: Vec<String>) -> Patrol {
        Patrol::new(
            vec![Some(RoomId(1)), None, Some(RoomId(3))],
            vec!["north".into(), "east".into(), "south".into(), "west".into()],
            landmarks,
            after,
        )
    }

    #[test]
    fn it_joins_the_circuit_where_it_stands_and_goes_round() {
        let mut rift = circuit(vec![landmark("thread", None)], vec!["stand".into()]);
        // Room 3 is third in `starts`, so the walk begins at the third move.
        assert_eq!(Scene::at(3, 9).ask(&mut rift), Next::Go("south".into()));
        assert_eq!(Scene::at(4, 9).ask(&mut rift), Next::Go("west".into()));
        assert_eq!(Scene::at(5, 9).ask(&mut rift), Next::Go("north".into()));
        assert_eq!(
            Scene::at(6, 9).showing("thread").ask(&mut rift),
            Next::Go("go thread".into())
        );
        assert_eq!(Scene::at(7, 9).ask(&mut rift), Next::Put("stand".into()));
        assert_eq!(Scene::at(7, 9).ask(&mut rift), Next::Done);
    }

    #[test]
    fn a_walker_off_the_circuit_is_lost_and_says_so_by_ending() {
        let mut rift = circuit(vec![landmark("thread", None)], Vec::new());
        assert_eq!(Scene::at(8, 9).ask(&mut rift), Next::Done);
    }

    #[test]
    fn a_fissure_is_pushed_until_it_is_wide_standing_up_between() {
        let fissure = Opening {
            command: "push fissure".into(),
            until: "cannot be opened any farther".into(),
            tries: 5,
        };
        let mut rift = circuit(vec![landmark("fissure", Some(fissure))], Vec::new());
        let seen = || Scene::at(1, 9).showing("fissure");
        assert_eq!(seen().ask(&mut rift), Next::Put("push fissure".into()));
        let mut knocked = seen().answered(&["Grasping the distorted edges..."]);
        knocked.walker.posture = Some("prone".into());
        assert_eq!(knocked.ask(&mut rift), Next::Put("stand".into()));
        // The answer to `stand` is not the fissure's.
        assert_eq!(
            seen()
                .answered(&["cannot be opened any farther"])
                .ask(&mut rift),
            Next::Put("push fissure".into())
        );
        assert_eq!(
            seen()
                .answered(&["A wide fissure cannot be opened any farther."])
                .ask(&mut rift),
            Next::Go("go fissure".into())
        );
    }

    #[test]
    fn five_pushes_is_enough_whatever_the_fissure_says() {
        let fissure = Opening {
            command: "push fissure".into(),
            until: "never".into(),
            tries: 2,
        };
        let mut rift = circuit(vec![landmark("fissure", Some(fissure))], Vec::new());
        let seen = || Scene::at(1, 9).showing("fissure");
        assert_eq!(seen().ask(&mut rift), Next::Put("push fissure".into()));
        assert_eq!(seen().ask(&mut rift), Next::Put("push fissure".into()));
        assert_eq!(seen().ask(&mut rift), Next::Go("go fissure".into()));
    }
}

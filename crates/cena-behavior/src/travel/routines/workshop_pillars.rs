//! `Puzzle::WorkshopPillars`: the wizards' workshop, room 15571.
//!
//! Upstream (`upstream_scripts/workshop_pillars.rb`): of air, fire and earth
//! -- in that order -- take the first whose three spells the walker knows
//! all of; move twice toward it (north, west, south); and cast each spell at
//! its pillar, waiting for the mana. Knowing no full set, upstream echoes
//! and exits: [`Next::Stop`].
//!
//! # Where this is not upstream
//!
//! - A move that fails fails the exit; upstream's `move` would carry on and
//!   cast at pillars that are not there.
//! - A cast hindered too often (`casting`) fails it too.

use super::casting::{self, Casting, Pay, Turn};
use super::{Next, Seen, Solver};

/// The way to each element's side, and what is cast at what there.
const ELEMENTS: [(&str, [(&str, u16); 3]); 3] = [
    (
        "north",
        [("grey orb", 505), ("blue orb", 914), ("white orb", 912)],
    ),
    (
        "west",
        [
            ("blue flame", 908),
            ("scarlet flame", 519),
            ("white flame", 906),
        ],
    ),
    (
        "south",
        [
            ("blue crystal", 520),
            ("black crystal", 510),
            ("violet crystal", 909),
        ],
    ),
];
const TOO_FEW: &str = "You do not know enough spells to get into the workshop.";

#[derive(Default)]
pub(super) struct WorkshopPillars {
    /// The side chosen: an index into [`ELEMENTS`].
    side: Option<usize>,
    moves: u32,
    pillar: usize,
    cast: Option<Casting>,
}

impl Solver for WorkshopPillars {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        if self.side.is_none() {
            let knows_all = |(_, pillars): &(&str, [(&str, u16); 3])| {
                pillars.iter().all(|(_, n)| casting::knows(seen, *n))
            };
            self.side = ELEMENTS.iter().position(knows_all);
        }
        let Some(side) = self.side else {
            return Next::Stop(TOO_FEW.to_owned());
        };
        let Some((way, pillars)) = ELEMENTS.get(side) else {
            return Next::Failed;
        };
        if !seen.ok {
            return Next::Failed;
        }
        if self.moves < 2 {
            self.moves += 1;
            return Next::Go((*way).to_owned());
        }
        loop {
            let Some((pillar, number)) = pillars.get(self.pillar) else {
                return Next::Done;
            };
            let cast = self
                .cast
                .get_or_insert_with(|| Casting::new(*number, pillar, Pay::Affordable));
            match cast.turn(seen) {
                Turn::Ask(next) => return next,
                Turn::Landed => {
                    self.pillar += 1;
                    self.cast = None;
                }
                Turn::NoMana => return cast.no_mana(),
                Turn::Hindered => return Next::Failed,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::casting::MANA_WAIT_MS;
    use super::super::casting::tests::{affording, knowing};
    use super::super::testing::Scene;
    use super::*;

    #[test]
    fn the_side_known_in_full_is_walked_to_and_each_pillar_cast_at() {
        // Two of air, all of fire: fire it is.
        let knows = || knowing(Scene::at(1, 2), &[505, 914, 908, 519, 906]);
        let mut workshop = WorkshopPillars::default();
        let asked: Vec<Next> = (0..9).map(|_| knows().ask(&mut workshop)).collect();
        let put = |command: &str| Next::Put(command.to_owned());
        assert_eq!(
            asked,
            [
                Next::Go("west".into()),
                Next::Go("west".into()),
                put("prepare 908"),
                put("cast blue flame"),
                put("prepare 519"),
                put("cast scarlet flame"),
                put("prepare 906"),
                put("cast white flame"),
                Next::Done,
            ]
        );
    }

    #[test]
    fn air_is_taken_before_earth_when_both_are_known() {
        let mut workshop = WorkshopPillars::default();
        let scene = knowing(Scene::at(1, 2), &[505, 914, 912, 520, 510, 909]);
        assert_eq!(scene.ask(&mut workshop), Next::Go("north".into()));
    }

    #[test]
    fn earth_is_south() {
        let mut workshop = WorkshopPillars::default();
        let scene = knowing(Scene::at(1, 2), &[520, 510, 909]);
        assert_eq!(scene.ask(&mut workshop), Next::Go("south".into()));
    }

    #[test]
    fn no_full_set_stops_the_trip_and_says_why() {
        let mut workshop = WorkshopPillars::default();
        let scene = knowing(Scene::at(1, 2), &[505, 914, 908, 519, 520]);
        assert_eq!(scene.ask(&mut workshop), Next::Stop(TOO_FEW.into()));
    }

    #[test]
    fn a_move_that_fails_fails_the_exit() {
        let mut workshop = WorkshopPillars::default();
        let knows = || knowing(Scene::at(1, 2), &[505, 914, 912]);
        knows().ask(&mut workshop);
        let mut stuck = knows();
        stuck.failed = true;
        assert_eq!(stuck.ask(&mut workshop), Next::Failed);
    }

    #[test]
    fn each_spell_waits_for_its_own_mana() {
        let mut workshop = WorkshopPillars::default();
        let knows = || knowing(Scene::at(1, 2), &[505, 914, 912]);
        knows().ask(&mut workshop);
        knows().ask(&mut workshop);
        assert_eq!(
            affording(knows(), &[914]).ask(&mut workshop),
            Next::Pause(MANA_WAIT_MS)
        );
        assert_eq!(
            affording(knows(), &[505]).ask(&mut workshop),
            Next::Put("prepare 505".into())
        );
    }
}

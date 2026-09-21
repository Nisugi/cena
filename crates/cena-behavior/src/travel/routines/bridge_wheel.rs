//! `Puzzle::BridgeWheel`: the drawbridge and its wheel (room 14726).
//!
//! Upstream (`upstream_scripts/bridge_wheel.rb`): `go bridge`. If it is "which
//! is presently pulled open", `go opening` and `climb platform` until "you
//! were able to pull yourself up" (or there is no platform to find), emptying
//! the hands when "You figure freeing up both hands might help." and filling
//! them once up. Then `turn wheel`: if "The wheel begins to move" or it has
//! "already been turned and a locking bar is holding it in place.", `down`,
//! `out`, `go bridge`. If it will not budge, cast what is known and affordable
//! of 606, 509 and 9605 and turn once more; if it still will not, upstream
//! pauses until somebody helps.
//!
//! # Where this differs, and why
//!
//! - **The pause is [`Next::Stop`]**: the trip ends and says to find help.
//! - Upstream climbs without end; [`MAX_TURNS`] bounds it.
//! - The spells are cast by name through [`Action::Cast`], so the walker
//!   chooses how; upstream's `Spell#cast` is the same choice made by Lich.

use cena_map::{Action, Step};
use cena_session::spells;

use super::{MAX_TURNS, Next, Seen, Solver};

const PULLED_OPEN: &str = "which is presently pulled open";
const UP: [&str; 2] = [
    "you were able to pull yourself up",
    "I could not find what you were referring to",
];
const HANDS: &str = "You figure freeing up both hands might help.";
const TURNED: [&str; 2] = [
    "The wheel begins to move",
    "already been turned and a locking bar is holding it in place.",
];
/// The strength spells upstream tries, by number: a ranger's, a wizard's and
/// a Guardian sigil.
const BUFFS: [u16; 3] = [606, 509, 9605];
const NEEDS_HELP: &str = "You do not have the strength to turn the bridge's wheel on your own. \
    Find somebody to turn it, or to make you stronger, and start the trip again.";

#[derive(Default)]
pub(super) struct BridgeWheel {
    at: At,
    climbs: u32,
    refill: bool,
    /// The buffs still to try. `None` until the wheel has refused once.
    buffs: Option<Vec<String>>,
}

#[derive(Default)]
enum At {
    #[default]
    Start,
    Tried,
    Climbing,
    Turn,
    Turned,
    Buffing,
    Leaving(usize),
}

fn one(action: Action) -> Next {
    Next::Steps(vec![Step { action, when: None }])
}

impl BridgeWheel {
    fn climb(&mut self, seen: &Seen<'_>) -> Next {
        if UP.iter().any(|line| seen.answered(line)) {
            self.at = At::Turn;
            if self.refill {
                return one(Action::FillHands);
            }
            return self.next(seen);
        }
        if seen.answered(HANDS) && !self.refill {
            self.refill = true;
            return one(Action::EmptyHands);
        }
        if self.climbs >= MAX_TURNS {
            return Next::Failed;
        }
        self.climbs += 1;
        Next::Put("climb platform".to_owned())
    }

    fn turned(&mut self, seen: &Seen<'_>) -> Next {
        if TURNED.iter().any(|line| seen.answered(line)) {
            self.at = At::Leaving(0);
            return self.next(seen);
        }
        if self.buffs.is_some() {
            return Next::Stop(NEEDS_HELP.to_owned());
        }
        let can = |name: &String| {
            let has = |set: &Option<std::collections::HashSet<String>>| {
                set.as_ref().is_some_and(|set| set.contains(name))
            };
            has(&seen.walker.known_spells) && has(&seen.walker.affordable_spells)
        };
        self.buffs = Some(
            BUFFS
                .iter()
                .filter_map(|number| spells::spell(*number))
                .map(|spell| spell.name.clone())
                .filter(can)
                .collect(),
        );
        self.at = At::Buffing;
        self.next(seen)
    }
}

impl Solver for BridgeWheel {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Start => {
                self.at = At::Tried;
                Next::Put("go bridge".to_owned())
            }
            At::Tried if seen.answered(PULLED_OPEN) => {
                self.at = At::Climbing;
                Next::Go("go opening".to_owned())
            }
            At::Tried => Next::Done,
            At::Climbing => self.climb(seen),
            At::Turn => {
                self.at = At::Turned;
                Next::Put("turn wheel".to_owned())
            }
            At::Turned => self.turned(seen),
            At::Buffing => {
                if let Some(left) = self.buffs.as_mut().filter(|left| !left.is_empty()) {
                    return one(Action::Cast(left.remove(0)));
                }
                self.at = At::Turn;
                self.next(seen)
            }
            At::Leaving(gone) => {
                self.at = At::Leaving(gone + 1);
                match ["down", "out", "go bridge"].get(gone) {
                    Some(way) => Next::Go((*way).to_owned()),
                    None => Next::Done,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Scene;
    use super::*;

    fn said(line: &str) -> Scene {
        Scene::at(1, 2).answered(&[line])
    }

    /// A wheel room reached: the bridge open, the platform climbed.
    fn at_the_wheel() -> BridgeWheel {
        let mut wheel = BridgeWheel::default();
        Scene::at(1, 2).ask(&mut wheel);
        said(PULLED_OPEN).ask(&mut wheel);
        Scene::at(1, 2).ask(&mut wheel);
        assert_eq!(said(UP[0]).ask(&mut wheel), Next::Put("turn wheel".into()));
        wheel
    }

    fn strong(scene: &mut Scene, affordable: bool) -> String {
        let name = spells::spell(509).unwrap().name.clone();
        scene.walker.known_spells = Some([name.clone()].into());
        scene.walker.affordable_spells = Some(if affordable {
            [name.clone()].into()
        } else {
            [].into()
        });
        name
    }

    #[test]
    fn a_bridge_that_is_down_is_simply_crossed() {
        let mut wheel = BridgeWheel::default();
        assert_eq!(
            Scene::at(1, 2).ask(&mut wheel),
            Next::Put("go bridge".into())
        );
        assert_eq!(Scene::at(2, 2).ask(&mut wheel), Next::Done);
    }

    #[test]
    fn an_open_bridge_is_gone_under_climbed_turned_and_crossed() {
        let mut wheel = BridgeWheel::default();
        Scene::at(1, 2).ask(&mut wheel);
        assert_eq!(
            said(PULLED_OPEN).ask(&mut wheel),
            Next::Go("go opening".into())
        );
        let climb = Next::Put("climb platform".into());
        assert_eq!(Scene::at(1, 2).ask(&mut wheel), climb);
        assert_eq!(said("You decide to climb back down").ask(&mut wheel), climb);
        assert_eq!(said(UP[0]).ask(&mut wheel), Next::Put("turn wheel".into()));
        let sent: Vec<Next> = (0..4).map(|_| said(TURNED[0]).ask(&mut wheel)).collect();
        assert_eq!(
            sent,
            [
                Next::Go("down".into()),
                Next::Go("out".into()),
                Next::Go("go bridge".into()),
                Next::Done,
            ]
        );
    }

    #[test]
    fn full_hands_are_emptied_for_the_climb_and_filled_at_the_top() {
        let mut wheel = BridgeWheel::default();
        Scene::at(1, 2).ask(&mut wheel);
        said(PULLED_OPEN).ask(&mut wheel);
        Scene::at(1, 2).ask(&mut wheel);
        assert_eq!(said(HANDS).ask(&mut wheel), one(Action::EmptyHands));
        assert_eq!(
            Scene::at(1, 2).ask(&mut wheel),
            Next::Put("climb platform".into())
        );
        assert_eq!(said(UP[1]).ask(&mut wheel), one(Action::FillHands));
        assert_eq!(
            Scene::at(1, 2).ask(&mut wheel),
            Next::Put("turn wheel".into())
        );
    }

    #[test]
    fn a_locked_wheel_was_turned_already() {
        let mut wheel = at_the_wheel();
        assert_eq!(said(TURNED[1]).ask(&mut wheel), Next::Go("down".into()));
    }

    #[test]
    fn a_wheel_that_will_not_budge_is_turned_again_with_strength() {
        let mut wheel = at_the_wheel();
        let mut weak = said("you just don't have enough strength to budge it");
        let name = strong(&mut weak, true);
        assert_eq!(weak.ask(&mut wheel), one(Action::Cast(name)));
        assert_eq!(weak.ask(&mut wheel), Next::Put("turn wheel".into()));
        assert_eq!(said(TURNED[0]).ask(&mut wheel), Next::Go("down".into()));
    }

    #[test]
    fn a_spell_that_cannot_be_paid_for_is_not_cast() {
        let mut wheel = at_the_wheel();
        let mut weak = said("you just don't have enough strength to budge it");
        strong(&mut weak, false);
        assert_eq!(weak.ask(&mut wheel), Next::Put("turn wheel".into()));
    }

    #[test]
    fn still_too_weak_stops_the_trip_and_asks_for_help() {
        let mut wheel = at_the_wheel();
        let weak = said("you just don't have enough strength to budge it");
        assert_eq!(weak.ask(&mut wheel), Next::Put("turn wheel".into()));
        assert_eq!(weak.ask(&mut wheel), Next::Stop(NEEDS_HELP.into()));
    }

    #[test]
    fn the_climb_is_bounded() {
        let mut wheel = BridgeWheel::default();
        Scene::at(1, 2).ask(&mut wheel);
        said(PULLED_OPEN).ask(&mut wheel);
        let climbs = (0..MAX_TURNS + 5)
            .filter(|_| Scene::at(1, 2).ask(&mut wheel) == Next::Put("climb platform".into()))
            .count();
        assert_eq!(climbs, MAX_TURNS as usize);
        assert_eq!(Scene::at(1, 2).ask(&mut wheel), Next::Failed);
    }
}

//! `Routine::BronzeGate`: the Graveyard's bronze gate, rooms 4140 and 4141.
//!
//! Upstream (`upstream_scripts/bronze_gate_in.rb`, `bronze_gate_out.rb`):
//! `go gate` until the far room's `Obvious paths` line answers. Between
//! tries, the first of these:
//!
//! 1. The first known of Unlock (407), Consecrate (1604), Bless Item (304)
//!    and Force Projection (1207), cast at `gate`, again when hindered.
//! 2. On the way **out** only (`batter`): a Warrior of 15 or more empties the
//!    hands, `batter gate`, waits half a second and the roundtime, and takes
//!    them back.
//! 3. Empty hands, `push bronze gate`, up to 16 seconds for the gate to give,
//!    and take them back.
//!
//! # Where this is not upstream
//!
//! - Being through is *the walker standing at the goal*, as well as the
//!   `Obvious` line: the room's paths are not always a line of the answer.
//! - `until` is unbounded upstream; here it is [`MAX_TURNS`] tries.
//! - The lines that say the gate gave are upstream's regex cut to the
//!   substrings an [`Next::Await`] can hold; ` open` stands for its first
//!   alternative, `gate .* open`.

use cena_map::{Action, Step};

use super::casting::{self, Casting, Pay, Turn};
use super::{MAX_TURNS, Next, Seen, Solver};

const SPELLS: [u16; 4] = [407, 1604, 304, 1207];
const PUSH_MS: u64 = 16_000;
const GAVE: [&str; 4] = [
    " open",
    "creak loudly as they give way",
    "It's opened wide enough to slip through now",
    "through a massive bronze gate.",
];

enum At {
    /// `go gate` is next.
    Trying,
    /// `go gate` was sent.
    Tried,
    Casting(Casting),
    /// By hand: what is still to ask for, the next first.
    ByHand(Vec<Next>),
}

pub(super) struct BronzeGate {
    batter: bool,
    at: At,
    tries: u32,
}

fn step(action: Action) -> Step {
    Step { action, when: None }
}

impl BronzeGate {
    pub fn new(batter: bool) -> Self {
        BronzeGate {
            batter,
            at: At::Trying,
            tries: 0,
        }
    }

    fn by_hand(&self, seen: &Seen<'_>) -> Vec<Next> {
        let warrior = seen.walker.profession.as_deref() == Some("Warrior")
            && seen.walker.level.is_some_and(|level| level >= 15);
        let work = if self.batter && warrior {
            vec![Next::Put("batter gate".into()), Next::Pause(500)]
        } else {
            vec![
                Next::Put("push bronze gate".into()),
                Next::Await(GAVE.map(str::to_owned).into(), PUSH_MS),
            ]
        };
        let mut all = vec![Next::Steps(vec![step(Action::EmptyHands)])];
        all.extend(work);
        all.push(Next::Steps(vec![step(Action::FillHands)]));
        all
    }
}

impl Solver for BronzeGate {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        loop {
            match &mut self.at {
                At::Trying => {
                    if self.tries >= MAX_TURNS {
                        return Next::Failed;
                    }
                    self.tries += 1;
                    self.at = At::Tried;
                    return Next::Put("go gate".into());
                }
                At::Tried => {
                    if seen.here == Some(seen.goal) || seen.answered("Obvious") {
                        return Next::Done;
                    }
                    self.at = match SPELLS.into_iter().find(|n| casting::knows(seen, *n)) {
                        Some(number) => At::Casting(Casting::new(number, "gate", Pay::Affordable)),
                        None => At::ByHand(self.by_hand(seen)),
                    };
                }
                At::Casting(cast) => match cast.turn(seen) {
                    Turn::Ask(next) => return next,
                    Turn::Landed => self.at = At::Trying,
                    Turn::NoMana => return cast.no_mana(),
                    Turn::Hindered => return Next::Failed,
                },
                At::ByHand(left) => {
                    // The gate gave to the push: no need to wait for it.
                    if matches!(left.first(), Some(Next::Await(..)))
                        && GAVE.iter().any(|line| seen.answered(line))
                    {
                        left.remove(0);
                    }
                    if left.is_empty() {
                        self.at = At::Trying;
                    } else {
                        return left.remove(0);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::casting::tests::knowing;
    use super::super::testing::Scene;
    use super::*;

    const CLOSED: &[&str] = &["The bronze gate appears to be closed."];

    fn hands(action: Action) -> Next {
        Next::Steps(vec![step(action)])
    }

    #[test]
    fn an_open_gate_is_one_try() {
        let mut gate = BronzeGate::new(false);
        assert_eq!(Scene::at(1, 2).ask(&mut gate), Next::Put("go gate".into()));
        assert_eq!(Scene::at(2, 2).ask(&mut gate), Next::Done);
    }

    #[test]
    fn the_far_rooms_paths_are_being_through_even_unlocated() {
        let mut gate = BronzeGate::new(false);
        Scene::at(1, 2).ask(&mut gate);
        let mut scene = Scene::at(1, 2).answered(&["Obvious paths: northeast, northwest, up"]);
        scene.here = None;
        assert_eq!(scene.ask(&mut gate), Next::Done);
    }

    #[test]
    fn the_first_known_spell_is_cast_at_it_and_the_gate_tried_again() {
        let mut gate = BronzeGate::new(true);
        // 304 and 1207 known: 304 is earlier in upstream's list.
        let knows = || knowing(Scene::at(1, 2), &[1207, 304]);
        assert_eq!(knows().ask(&mut gate), Next::Put("go gate".into()));
        assert_eq!(
            knows().answered(CLOSED).ask(&mut gate),
            Next::Put("prepare 304".into())
        );
        assert_eq!(knows().ask(&mut gate), Next::Put("cast gate".into()));
        assert_eq!(knows().ask(&mut gate), Next::Put("go gate".into()));
        assert_eq!(Scene::at(2, 2).ask(&mut gate), Next::Done);
    }

    fn warrior(level: u32) -> Scene {
        let mut scene = Scene::at(1, 2);
        scene.walker.profession = Some("Warrior".into());
        scene.walker.level = Some(level);
        scene
    }

    #[test]
    fn a_warrior_batters_it_on_the_way_out() {
        let mut gate = BronzeGate::new(true);
        warrior(15).ask(&mut gate);
        let asked: Vec<Next> = (0..5)
            .map(|_| warrior(15).answered(CLOSED).ask(&mut gate))
            .collect();
        assert_eq!(
            asked,
            [
                hands(Action::EmptyHands),
                Next::Put("batter gate".into()),
                Next::Pause(500),
                hands(Action::FillHands),
                Next::Put("go gate".into()),
            ]
        );
    }

    #[test]
    fn everyone_else_pushes_and_waits_for_it_to_give() {
        // A young warrior going out, and a grown one going in.
        for (batter, level) in [(true, 14), (false, 50)] {
            let mut gate = BronzeGate::new(batter);
            warrior(level).ask(&mut gate);
            let asked: Vec<Next> = (0..5)
                .map(|_| warrior(level).answered(CLOSED).ask(&mut gate))
                .collect();
            assert_eq!(
                asked,
                [
                    hands(Action::EmptyHands),
                    Next::Put("push bronze gate".into()),
                    Next::Await(GAVE.map(str::to_owned).into(), PUSH_MS),
                    hands(Action::FillHands),
                    Next::Put("go gate".into()),
                ]
            );
        }
    }

    #[test]
    fn a_push_that_is_answered_at_once_is_not_waited_on() {
        let mut gate = BronzeGate::new(false);
        Scene::at(1, 2).ask(&mut gate);
        Scene::at(1, 2).answered(CLOSED).ask(&mut gate);
        assert_eq!(
            Scene::at(1, 2).ask(&mut gate),
            Next::Put("push bronze gate".into())
        );
        assert_eq!(
            Scene::at(1, 2)
                .answered(&["The bronze gate pops open!"])
                .ask(&mut gate),
            hands(Action::FillHands)
        );
    }

    #[test]
    fn a_gate_that_never_opens_fails_the_exit() {
        let mut gate = BronzeGate::new(false);
        let mut tries = 0;
        loop {
            let next = Scene::at(1, 2).answered(CLOSED).ask(&mut gate);
            assert!(tries <= MAX_TURNS * 5, "unbounded");
            match next {
                Next::Failed => break,
                Next::Put(command) if command == "go gate" => tries += 1,
                _ => {}
            }
        }
        assert_eq!(tries, MAX_TURNS);
    }
}

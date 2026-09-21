//! `Puzzle::RolarenGate`: the rolaren gate, rooms 3239 and 3264.
//!
//! Upstream (`upstream_scripts/rolaren_gate.rb`): `open rolaren gate`. Open,
//! or already so: `go rolaren gate`. Locked, the first of these the walker
//! can do:
//!
//! 1. **Climb it**, when `(1 - encumbrance/100) * climbing bonus >= 50`;
//!    Sigil of Resolve first, if known and neither it nor `POPed muscles` is
//!    running.
//! 2. **Phase (704)** at the gate, which carries the walker through -- the one
//!    cast here that is `Action::CastAt`, which waits to be carried.
//! 3. **Unlock (407) or Force Projection (1207)** at it, again while the gate
//!    "vibrates slightly" or "remains unaffected", then `go rolaren gate`.
//! 4. None: upstream echoes why and exits, which is [`Next::Stop`].
//!
//! Any other answer to `open` is upstream's `$go2_restart`: [`Next::Done`].
//!
//! # Where this is not upstream
//!
//! - The recast in 3 is unbounded upstream; here it is [`MAX_TURNS`] casts,
//!   and then the exit fails.
//! - Waiting for the sigil's stamina is left to the driver's cast.
//! - The climbing bonus is recomputed from ranks (`Skills.to_bonus`), as
//!   upstream does. The curve is `cena_model::to_bonus`, **copied** because
//!   this crate cannot reach it; it moves when `cena-session` exports it.

use cena_map::{Action, Step};

use super::casting::{self, Casting, Pay, Turn};
use super::{MAX_TURNS, Next, Seen, Solver};

const GO: &str = "go rolaren gate";
const STUBBORN: [&str; 2] = [
    "The gate vibrates slightly but nothing else happens",
    "A translucent force slams into the gate, but it remains unaffected",
];
const NO_WAY: &str = "You can't get through the rolaren gate: you can't cast 704, 407 or \
                      1207, and you climb too poorly or carry too much to go over it.";

#[derive(Default)]
enum At {
    #[default]
    Start,
    Opening,
    /// The sigil was asked for; the climb is next.
    Climb,
    /// Casting at the lock, this many times so far.
    Forcing(Casting, u32),
    /// The last move is made, or Phase is cast.
    Leaving,
}

#[derive(Default)]
pub(super) struct RolarenGate {
    at: At,
}

fn step(action: Action) -> Step {
    Step { action, when: None }
}

/// `Skills.to_bonus`. See the module docs for why it is here.
fn to_bonus(ranks: u32) -> u32 {
    match ranks {
        0..=10 => ranks * 5,
        11..=20 => 50 + (ranks - 10) * 4,
        21..=30 => 90 + (ranks - 20) * 3,
        31..=40 => 120 + (ranks - 30) * 2,
        _ => 140 + (ranks - 40),
    }
}

/// `(1 - encumbrance/100) * bonus >= 50`, in whole numbers.
fn can_climb(seen: &Seen<'_>) -> bool {
    let ranks = seen
        .walker
        .skills
        .as_ref()
        .and_then(|skills| skills.get("climbing"))
        .copied()
        .unwrap_or(0);
    let free = 100u32.saturating_sub(seen.walker.encumbrance.unwrap_or(0));
    free * to_bonus(ranks) >= 5000
}

impl RolarenGate {
    fn locked(&mut self, seen: &Seen<'_>) -> Next {
        if can_climb(seen) {
            let running = |name: &str| {
                seen.walker
                    .active_spells
                    .as_ref()
                    .is_some_and(|active| active.contains(name))
            };
            let known = seen
                .walker
                .known_spells
                .as_ref()
                .is_some_and(|known| known.contains("Sigil of Resolve"));
            if known && !running("Sigil of Resolve") && !running("POPed muscles") {
                self.at = At::Climb;
                return Next::Steps(vec![step(Action::Cast("Sigil of Resolve".into()))]);
            }
            return self.leave("climb rolaren gate");
        }
        if let Some(phase) = casting::name_of(704).filter(|_| casting::knows(seen, 704)) {
            self.at = At::Leaving;
            return Next::Steps(vec![step(Action::CastAt(
                phase.to_owned(),
                "rolaren gate".into(),
            ))]);
        }
        if let Some(number) = [407, 1207]
            .into_iter()
            .find(|number| casting::knows(seen, *number))
        {
            self.at = At::Forcing(Casting::new(number, "rolaren gate", Pay::Affordable), 0);
            return self.next(seen);
        }
        Next::Stop(NO_WAY.to_owned())
    }

    fn leave(&mut self, command: &str) -> Next {
        self.at = At::Leaving;
        Next::Go(command.to_owned())
    }
}

impl Solver for RolarenGate {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match &mut self.at {
            At::Start => {
                self.at = At::Opening;
                Next::Put("open rolaren gate".into())
            }
            At::Opening => {
                if seen.answered("You open") || seen.answered("That is already open") {
                    self.leave(GO)
                } else if seen.answered("It appears to be locked") {
                    self.locked(seen)
                } else {
                    Next::Done
                }
            }
            At::Climb => self.leave("climb rolaren gate"),
            At::Forcing(cast, casts) => match cast.turn(seen) {
                Turn::Ask(next) => next,
                Turn::Landed if STUBBORN.iter().any(|line| seen.answered(line)) => {
                    *casts += 1;
                    if *casts >= MAX_TURNS {
                        return Next::Failed;
                    }
                    self.next(seen)
                }
                Turn::Landed => self.leave(GO),
                Turn::NoMana => cast.no_mana(),
                Turn::Hindered => Next::Failed,
            },
            At::Leaving => Next::Done,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::casting::tests::knowing;
    use super::super::testing::Scene;
    use super::*;

    const LOCKED: &[&str] = &["It appears to be locked."];

    fn opened(gate: &mut RolarenGate) {
        assert_eq!(
            Scene::at(1, 2).ask(gate),
            Next::Put("open rolaren gate".into())
        );
    }

    fn climber(mut scene: Scene, ranks: u32, encumbrance: u32) -> Scene {
        scene.walker.skills = Some([("climbing".to_owned(), ranks)].into());
        scene.walker.encumbrance = Some(encumbrance);
        scene
    }

    #[test]
    fn an_open_gate_is_walked_through() {
        for line in ["You open the rolaren gate.", "That is already open."] {
            let mut gate = RolarenGate::default();
            opened(&mut gate);
            assert_eq!(
                Scene::at(1, 2).answered(&[line]).ask(&mut gate),
                Next::Go(GO.into())
            );
            assert_eq!(Scene::at(2, 2).ask(&mut gate), Next::Done);
        }
    }

    #[test]
    fn any_other_answer_ends_it_for_the_trip_to_look() {
        let mut gate = RolarenGate::default();
        opened(&mut gate);
        assert_eq!(
            Scene::at(1, 2).answered(&["What?"]).ask(&mut gate),
            Next::Done
        );
    }

    #[test]
    fn a_good_climber_climbs_and_a_loaded_one_does_not() {
        // 10 ranks is a bonus of 50: exactly enough with nothing carried.
        let mut gate = RolarenGate::default();
        opened(&mut gate);
        assert_eq!(
            climber(Scene::at(1, 2), 10, 0)
                .answered(LOCKED)
                .ask(&mut gate),
            Next::Go("climb rolaren gate".into())
        );
        assert_eq!(Scene::at(2, 2).ask(&mut gate), Next::Done);

        let mut gate = RolarenGate::default();
        opened(&mut gate);
        assert_eq!(
            climber(Scene::at(1, 2), 10, 1)
                .answered(LOCKED)
                .ask(&mut gate),
            Next::Stop(NO_WAY.into())
        );
    }

    #[test]
    fn the_sigil_goes_up_before_the_climb_unless_it_is_running() {
        let sigil = |scene: Scene| {
            let mut scene = climber(scene, 40, 0);
            scene.walker.known_spells = Some(["Sigil of Resolve".to_owned()].into());
            scene
        };
        let mut gate = RolarenGate::default();
        opened(&mut gate);
        assert_eq!(
            sigil(Scene::at(1, 2)).answered(LOCKED).ask(&mut gate),
            Next::Steps(vec![step(Action::Cast("Sigil of Resolve".into()))])
        );
        assert_eq!(
            Scene::at(1, 2).ask(&mut gate),
            Next::Go("climb rolaren gate".into())
        );

        for running in ["Sigil of Resolve", "POPed muscles"] {
            let mut gate = RolarenGate::default();
            opened(&mut gate);
            let mut scene = sigil(Scene::at(1, 2)).answered(LOCKED);
            scene.walker.active_spells = Some([running.to_owned()].into());
            assert_eq!(
                scene.ask(&mut gate),
                Next::Go("climb rolaren gate".into()),
                "{running}"
            );
        }
    }

    #[test]
    fn phase_carries_the_walker_and_is_tried_before_unlock() {
        let mut gate = RolarenGate::default();
        opened(&mut gate);
        let phase = casting::name_of(704).unwrap().to_owned();
        assert_eq!(
            knowing(Scene::at(1, 2), &[704, 407])
                .answered(LOCKED)
                .ask(&mut gate),
            Next::Steps(vec![step(Action::CastAt(phase, "rolaren gate".into()))])
        );
        assert_eq!(Scene::at(2, 2).ask(&mut gate), Next::Done);
    }

    #[test]
    fn unlock_is_cast_again_while_the_gate_shrugs_it_off() {
        let mut gate = RolarenGate::default();
        opened(&mut gate);
        let knows = || knowing(Scene::at(1, 2), &[1207]);
        assert_eq!(
            knows().answered(LOCKED).ask(&mut gate),
            Next::Put("prepare 1207".into())
        );
        assert_eq!(
            knows().ask(&mut gate),
            Next::Put("cast rolaren gate".into())
        );
        assert_eq!(
            knows().answered(&[STUBBORN[1]]).ask(&mut gate),
            Next::Put("prepare 1207".into())
        );
        assert_eq!(
            knows().ask(&mut gate),
            Next::Put("cast rolaren gate".into())
        );
        assert_eq!(
            knows().answered(&["The gate clicks."]).ask(&mut gate),
            Next::Go(GO.into())
        );
        assert_eq!(Scene::at(2, 2).ask(&mut gate), Next::Done);
    }

    #[test]
    fn a_gate_that_never_gives_fails_the_exit() {
        let mut gate = RolarenGate::default();
        opened(&mut gate);
        let knows = || knowing(Scene::at(1, 2), &[407]);
        let mut next = knows().answered(LOCKED).ask(&mut gate);
        let mut casts = 0;
        while next != Next::Failed {
            assert!(casts <= MAX_TURNS, "unbounded");
            assert_eq!(next, Next::Put("prepare 407".into()));
            assert_eq!(
                knows().ask(&mut gate),
                Next::Put("cast rolaren gate".into())
            );
            casts += 1;
            next = knows().answered(&[STUBBORN[0]]).ask(&mut gate);
        }
        assert_eq!(casts, MAX_TURNS);
    }

    #[test]
    fn a_walker_with_no_way_is_told_so() {
        let mut gate = RolarenGate::default();
        opened(&mut gate);
        assert_eq!(
            Scene::at(1, 2).answered(LOCKED).ask(&mut gate),
            Next::Stop(NO_WAY.into())
        );
    }
}

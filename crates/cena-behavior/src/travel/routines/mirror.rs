//! `Routine::Mirror`: the oak-framed mirror of room 16165.
//!
//! Upstream (`upstream_scripts/mirror.rb`): `turn mirror right` until it
//! "won't turn any farther to the right". Then `tilt mirror` until the light
//! falls "from the base of the statue onto the center of the statue". A tilt
//! that "flips around" has gone past: `turn mirror left` once -- which may
//! itself find the centre -- and, if that was the left stop, `turn mirror
//! right` four times before tilting again. Then `move 'go shadow'`.
//!
//! Upstream's loops wait for ever on a mirror that never answers; here every
//! command sent counts toward [`MAX_TURNS`]. `shadow.rb` is another exit's
//! script and no part of this.

use super::{MAX_TURNS, Next, Seen, Solver};

const RIGHT_STOP: &str = "The mirror won't turn any farther to the right";
const LEFT_STOP: &str = "The mirror won't turn any farther to the left";
const CENTRE: &str = "from the base of the statue onto the center of the statue";
const FLIPPED: &str = "flips around";
/// How far back from the left stop upstream turns.
const BACK_RIGHT: u32 = 4;

#[derive(Default)]
pub(super) struct Mirror {
    turns: u32,
    at: At,
}

#[derive(Default)]
enum At {
    #[default]
    Start,
    /// `turn mirror right` was sent, toward the stop.
    Righting,
    /// `tilt mirror` was sent.
    Tilting,
    /// `turn mirror left` was sent, after a flip.
    Lefting,
    /// Coming back from the left stop: this many right turns still to send.
    Backing(u32),
    Gone,
}

impl Mirror {
    fn put(&mut self, at: At, command: &str) -> Next {
        if self.turns >= MAX_TURNS {
            return Next::Failed;
        }
        self.turns += 1;
        self.at = at;
        Next::Put(command.to_owned())
    }

    fn go(&mut self) -> Next {
        self.at = At::Gone;
        Next::Go("go shadow".to_owned())
    }
}

impl Solver for Mirror {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Righting if seen.answered(RIGHT_STOP) => self.put(At::Tilting, "tilt mirror"),
            At::Start | At::Righting => self.put(At::Righting, "turn mirror right"),
            At::Tilting | At::Lefting if seen.answered(CENTRE) => self.go(),
            At::Tilting if seen.answered(FLIPPED) => self.put(At::Lefting, "turn mirror left"),
            At::Lefting if seen.answered(LEFT_STOP) => {
                self.put(At::Backing(BACK_RIGHT - 1), "turn mirror right")
            }
            At::Backing(left) if left > 0 => self.put(At::Backing(left - 1), "turn mirror right"),
            At::Tilting | At::Lefting | At::Backing(_) => self.put(At::Tilting, "tilt mirror"),
            At::Gone => Next::Done,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Scene;
    use super::*;

    const RIGHT: &str = "You turn the oak-framed mirror slightly to the right.";
    const LEFT: &str = "You turn the oak-framed mirror slightly to the left.";
    const UP: &str = "You tilt the oak-framed mirror up slightly.";
    const FLIP: &str = "You tilt the oak-framed mirror up until it flips around.";
    const LIT: &str = "The light moves from the base of the statue onto the center of the statue.";

    fn put(command: &str) -> Next {
        Next::Put(command.to_owned())
    }

    fn hears(mirror: &mut Mirror, line: &str) -> Next {
        Scene::at(1, 2).answered(&[line]).ask(mirror)
    }

    #[test]
    fn right_to_the_stop_then_tilted_to_the_centre_and_through() {
        let mut mirror = Mirror::default();
        assert_eq!(Scene::at(1, 2).ask(&mut mirror), put("turn mirror right"));
        assert_eq!(hears(&mut mirror, RIGHT), put("turn mirror right"));
        assert_eq!(hears(&mut mirror, RIGHT_STOP), put("tilt mirror"));
        assert_eq!(hears(&mut mirror, UP), put("tilt mirror"));
        assert_eq!(hears(&mut mirror, LIT), Next::Go("go shadow".into()));
        assert_eq!(Scene::at(2, 2).ask(&mut mirror), Next::Done);
    }

    fn flipped() -> Mirror {
        let mut mirror = Mirror::default();
        Scene::at(1, 2).ask(&mut mirror);
        hears(&mut mirror, RIGHT_STOP);
        assert_eq!(hears(&mut mirror, FLIP), put("turn mirror left"));
        mirror
    }

    #[test]
    fn a_flip_is_one_turn_left_and_then_tilting_again() {
        let mut mirror = flipped();
        assert_eq!(hears(&mut mirror, LEFT), put("tilt mirror"));
        assert_eq!(hears(&mut mirror, LIT), Next::Go("go shadow".into()));
    }

    #[test]
    fn the_turn_left_may_itself_find_the_centre() {
        let mut mirror = flipped();
        assert_eq!(hears(&mut mirror, LIT), Next::Go("go shadow".into()));
    }

    #[test]
    fn the_left_stop_is_four_turns_right_whatever_they_say() {
        let mut mirror = flipped();
        for _ in 0..4 {
            // Not the right stop's answer to the first loop: it is not read.
            assert_eq!(hears(&mut mirror, LEFT_STOP), put("turn mirror right"));
        }
        assert_eq!(hears(&mut mirror, RIGHT), put("tilt mirror"));
    }

    #[test]
    fn a_mirror_that_never_answers_is_given_up() {
        let mut mirror = Mirror::default();
        for _ in 0..MAX_TURNS {
            assert_eq!(Scene::at(1, 2).ask(&mut mirror), put("turn mirror right"));
        }
        assert_eq!(Scene::at(1, 2).ask(&mut mirror), Next::Failed);
    }
}

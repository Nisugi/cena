//! `Puzzle::VaalornDoor`: the door that wants a gem (room 14060).
//!
//! Upstream (`upstream_scripts/vaalorn_door.rb`): `open door`. If "That is
//! already open." go through. If "There doesn't seem to be any way to do
//! that." the door wants a gem: when `GameObj.right_hand.type` is `gem`, `put`
//! it `in vaalorn door`; when it is not, upstream pauses and tells the player
//! to hold a cheap gem. Then `go door`.
//!
//! # Where this differs, and why
//!
//! - **The pause is [`Next::Stop`]**: the trip ends, says what to hold, and is
//!   started again, which comes back here with the gem in hand.
//! - **The model has no item types**, so "is a gem" is asked of the noun
//!   against [`GEM_NOUNS`], a short list of the common ones. A gem by another
//!   noun stops the trip rather than being sacrificed; anything not on the
//!   list is never put in the door. Upstream, once unpaused, puts in whatever
//!   the right hand then holds; this does not.
//! - Upstream reads lines for ever until one of the two comes. Here an answer
//!   with neither goes on to `go door`, and the trip looks at where it landed.

use super::{Next, Seen, Solver};

/// Nouns taken for a gem. A guess at the common ones, erring small.
const GEM_NOUNS: [&str; 20] = [
    "gem",
    "quartz",
    "agate",
    "turquoise",
    "garnet",
    "amethyst",
    "topaz",
    "pearl",
    "coral",
    "jade",
    "opal",
    "diamond",
    "emerald",
    "ruby",
    "sapphire",
    "peridot",
    "zircon",
    "spinel",
    "tourmaline",
    "moonstone",
];

const NEEDS_A_GEM: &str = "The Vaalorn door opens only for a gem, and you are not holding one. \
    Hold a cheap gem in your right hand and start the trip again.";

#[derive(Default)]
pub(super) struct VaalornDoor {
    at: At,
}

#[derive(Default)]
enum At {
    #[default]
    Start,
    Opened,
    Paid,
    Gone,
}

impl Solver for VaalornDoor {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Start => {
                self.at = At::Opened;
                Next::Put("open door".to_owned())
            }
            At::Opened if seen.answered("There doesn't seem to be any way to do that.") => {
                let hand = &seen.state.right_hand;
                let gem = hand
                    .noun()
                    .filter(|noun| GEM_NOUNS.contains(noun))
                    .and(hand.id());
                let Some(gem) = gem else {
                    return Next::Stop(NEEDS_A_GEM.to_owned());
                };
                self.at = At::Paid;
                Next::Put(format!("put #{gem} in vaalorn door"))
            }
            At::Opened | At::Paid => {
                self.at = At::Gone;
                Next::Go("go door".to_owned())
            }
            At::Gone => Next::Done,
        }
    }
}

#[cfg(test)]
mod tests {
    use cena_session::hands::Hand;

    use super::super::testing::Scene;
    use super::*;

    const SHUT: &str = "There doesn't seem to be any way to do that.";

    fn holding(noun: &str) -> Hand {
        Hand::Holding {
            id: Some("77".into()),
            noun: Some(noun.into()),
            name: format!("a {noun}"),
        }
    }

    #[test]
    fn an_open_door_is_gone_through() {
        let mut door = VaalornDoor::default();
        assert_eq!(
            Scene::at(1, 2).ask(&mut door),
            Next::Put("open door".into())
        );
        let open = Scene::at(1, 2).answered(&["That is already open."]);
        assert_eq!(open.ask(&mut door), Next::Go("go door".into()));
        assert_eq!(Scene::at(2, 2).ask(&mut door), Next::Done);
    }

    #[test]
    fn a_gem_in_the_right_hand_is_paid() {
        let mut door = VaalornDoor::default();
        Scene::at(1, 2).ask(&mut door);
        let mut shut = Scene::at(1, 2).answered(&[SHUT]);
        shut.state.right_hand = holding("quartz");
        assert_eq!(
            shut.ask(&mut door),
            Next::Put("put #77 in vaalorn door".into())
        );
        assert_eq!(Scene::at(1, 2).ask(&mut door), Next::Go("go door".into()));
        assert_eq!(Scene::at(2, 2).ask(&mut door), Next::Done);
    }

    #[test]
    fn no_gem_stops_the_trip_and_says_so() {
        for hand in [Hand::Empty, Hand::Unknown, holding("sword")] {
            let mut door = VaalornDoor::default();
            Scene::at(1, 2).ask(&mut door);
            let mut shut = Scene::at(1, 2).answered(&[SHUT]);
            shut.state.right_hand = hand;
            assert_eq!(shut.ask(&mut door), Next::Stop(NEEDS_A_GEM.into()));
        }
    }

    #[test]
    fn an_answer_with_neither_line_still_tries_the_door() {
        let mut door = VaalornDoor::default();
        Scene::at(1, 2).ask(&mut door);
        assert_eq!(Scene::at(1, 2).ask(&mut door), Next::Go("go door".into()));
    }
}

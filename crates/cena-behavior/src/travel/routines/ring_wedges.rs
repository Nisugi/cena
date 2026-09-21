//! `Routine::RingWedges`: the stone ring of room 8373.
//!
//! Upstream (`upstream_scripts/ring_wedges.rb`): for each wedge in turn --
//! northern, eastern, western, southern -- `look <which> wedge`, and while it
//! is not "currently pointed at a sigil of" the one that is its own, `turn
//! ring` and look again; then `push <which> wedge`. Nothing more: the trip
//! looks at where the last push left the walker.
//!
//! Upstream's `dothis` sends the look again until the game answers it; here
//! an answer that names no sigil is likewise looked at again rather than
//! turned on. Upstream turns for ever on a ring that never comes round; here
//! every command sent counts toward [`MAX_TURNS`].

use cena_session::ChunkLine;

use super::{MAX_TURNS, Next, Seen, Solver};

/// Each wedge and the sigil it must point at, in upstream's order.
const WEDGES: &[(&str, &str)] = &[
    (
        "northern",
        "a large thick torus surrounded by nine tiny circles",
    ),
    (
        "eastern",
        "two crossed upside-down hammers circumscribed by a rounded arc",
    ),
    (
        "western",
        "a jagged triangular arch bisected by a vertical line",
    ),
    (
        "southern",
        "three pairs of obliquely intersecting parallel lines",
    ),
];

#[derive(Default)]
pub(super) struct RingWedges {
    /// The position in [`WEDGES`] of the wedge being set.
    wedge: usize,
    /// Whether the last thing sent was the look, so its answer is the wedge's.
    looked: bool,
    turns: u32,
}

impl RingWedges {
    fn put(&mut self, looked: bool, command: String) -> Next {
        if self.turns >= MAX_TURNS {
            return Next::Failed;
        }
        self.turns += 1;
        self.looked = looked;
        Next::Put(command)
    }
}

impl Solver for RingWedges {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        let Some((which, sigil)) = WEDGES.get(self.wedge) else {
            return Next::Done;
        };
        let points = format!("The stone ring's {which} wedge is currently pointed at a sigil of ");
        let pointed = seen
            .answer
            .iter()
            .map(ChunkLine::text)
            .find(|text| text.contains(&points));
        match pointed {
            Some(text) if self.looked && text.contains(sigil) => {
                self.wedge += 1;
                self.put(false, format!("push {which} wedge"))
            }
            Some(_) if self.looked => self.put(false, "turn ring".to_owned()),
            _ => self.put(true, format!("look {which} wedge")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Scene;
    use super::*;

    fn put(command: &str) -> Next {
        Next::Put(command.to_owned())
    }

    fn pointed(which: &str, sigil: &str) -> String {
        format!("The stone ring's {which} wedge is currently pointed at a sigil of {sigil}.")
    }

    fn hears(ring: &mut RingWedges, line: &str) -> Next {
        Scene::at(1, 2).answered(&[line]).ask(ring)
    }

    #[test]
    fn the_ring_is_turned_until_the_wedge_points_home_and_then_pushed() {
        let mut ring = RingWedges::default();
        assert_eq!(Scene::at(1, 2).ask(&mut ring), put("look northern wedge"));
        let wrong = pointed("northern", WEDGES[3].1);
        assert_eq!(hears(&mut ring, &wrong), put("turn ring"));
        // The answer to the turn is not the wedge's, whatever it holds.
        let right = pointed("northern", WEDGES[0].1);
        assert_eq!(hears(&mut ring, &right), put("look northern wedge"));
        assert_eq!(hears(&mut ring, &right), put("push northern wedge"));
        assert_eq!(hears(&mut ring, "Click."), put("look eastern wedge"));
    }

    #[test]
    fn every_wedge_is_set_in_upstreams_order_and_that_is_all() {
        let mut ring = RingWedges::default();
        for (which, sigil) in WEDGES {
            assert_eq!(
                Scene::at(1, 2).ask(&mut ring),
                put(&format!("look {which} wedge"))
            );
            assert_eq!(
                hears(&mut ring, &pointed(which, sigil)),
                put(&format!("push {which} wedge"))
            );
        }
        assert_eq!(Scene::at(1, 2).ask(&mut ring), Next::Done);
    }

    #[test]
    fn the_table_is_upstreams() {
        let upstream = [
            "'northern' => 'a large thick torus surrounded by nine tiny circles'",
            "'eastern' => 'two crossed upside-down hammers circumscribed by a rounded arc'",
            "'western' => 'a jagged triangular arch bisected by a vertical line'",
            "'southern' => 'three pairs of obliquely intersecting parallel lines'",
        ];
        let ours: Vec<String> = WEDGES
            .iter()
            .map(|(which, sigil)| format!("'{which}' => '{sigil}'"))
            .collect();
        assert_eq!(ours, upstream);
    }

    #[test]
    fn another_wedges_sigil_is_never_home() {
        for (at, (which, _)) in WEDGES.iter().enumerate() {
            for (other, (_, sigil)) in WEDGES.iter().enumerate() {
                if at == other {
                    continue;
                }
                let mut ring = RingWedges {
                    wedge: at,
                    looked: true,
                    turns: 0,
                };
                assert_eq!(hears(&mut ring, &pointed(which, sigil)), put("turn ring"));
            }
        }
    }

    #[test]
    fn a_look_the_game_did_not_answer_is_looked_again() {
        let mut ring = RingWedges::default();
        Scene::at(1, 2).ask(&mut ring);
        assert_eq!(
            hears(&mut ring, "...wait 1 seconds."),
            put("look northern wedge")
        );
        // Another wedge's line is not this one's answer either.
        let eastern = pointed("eastern", WEDGES[0].1);
        assert_eq!(hears(&mut ring, &eastern), put("look northern wedge"));
    }

    #[test]
    fn a_ring_that_never_comes_round_is_given_up() {
        let mut ring = RingWedges::default();
        let wrong = pointed("northern", WEDGES[1].1);
        for _ in 0..MAX_TURNS {
            assert!(matches!(hears(&mut ring, &wrong), Next::Put(_)));
        }
        assert_eq!(hears(&mut ring, &wrong), Next::Failed);
    }
}

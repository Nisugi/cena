//! `Routine::ColourBarrier`: the barrier of room 30850.
//!
//! Upstream (`upstream_scripts/colour_barrier.rb`): the room's description
//! shows a blue, black, yellow or red barrier, and the colour says which walk
//! leads to its grotto. Walk it, `go grotto`, `touch crystal`, `out`, walk
//! back -- the same moves reversed, each turned about -- and `go barrier`.
//!
//! The colour is read from what the description links (`Walker::sees`,
//! upstream's `GameObj.room_desc`) and, failing that, from the description's
//! own words. Upstream tries the four colours in the order its pattern lists
//! them, and so does this.
//!
//! Deviations: with no barrier of a known colour upstream raises an error;
//! here that is [`Next::Failed`]. Upstream walks on whatever a move does; a
//! walk that fails here ends the routine, and the trip plans again from
//! wherever it stopped.

use cena_map::{Action, Step};

use super::{Next, Seen, Solver};

/// The colours in the order upstream's pattern tries them, and each one's
/// walk from the barrier to the room its grotto opens off.
const WALKS: &[(&str, &[&str])] = &[
    ("blue", &["n", "sw", "sw"]),
    ("black", &["n", "sw", "sw", "sw", "s", "se"]),
    ("yellow", &["n", "se", "se", "se", "s", "sw"]),
    ("red", &["n", "se", "se"]),
];

/// Upstream's `reverse_direction`, for the directions the walks use.
fn turned_about(dir: &str) -> Option<&'static str> {
    Some(match dir {
        "n" => "s",
        "s" => "n",
        "sw" => "ne",
        "se" => "nw",
        _ => return None,
    })
}

/// The walk for the barrier this text names.
fn walk_in(text: &str) -> Option<&'static [&'static str]> {
    WALKS
        .iter()
        .find(|(colour, _)| text.contains(&format!("{colour} barrier")))
        .map(|(_, walk)| *walk)
}

fn moves(commands: Vec<String>) -> Next {
    Next::Steps(vec![Step {
        action: Action::Moves(commands),
        when: None,
    }])
}

#[derive(Default)]
pub(super) struct ColourBarrier {
    walk: &'static [&'static str],
    at: At,
}

#[derive(Default)]
enum At {
    #[default]
    Start,
    InGrotto,
    Touched,
    Back,
    Gone,
}

impl ColourBarrier {
    fn read(seen: &Seen<'_>) -> Option<&'static [&'static str]> {
        let linked = seen
            .walker
            .sees
            .iter()
            .flatten()
            .find_map(|name| walk_in(name));
        linked.or_else(|| {
            let description = seen.state.room.description.as_ref()?;
            walk_in(&description.plain())
        })
    }

    fn back(&self) -> Option<Vec<String>> {
        let mut commands = vec!["out".to_owned()];
        for dir in self.walk.iter().rev() {
            commands.push(turned_about(dir)?.to_owned());
        }
        Some(commands)
    }
}

impl Solver for ColourBarrier {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Start => {
                let Some(walk) = Self::read(seen) else {
                    return Next::Failed;
                };
                self.walk = walk;
                self.at = At::InGrotto;
                let mut commands: Vec<String> = walk.iter().map(|dir| (*dir).to_owned()).collect();
                commands.push("go grotto".to_owned());
                moves(commands)
            }
            At::InGrotto | At::Back if !seen.ok => Next::Done,
            At::InGrotto => {
                self.at = At::Touched;
                Next::Put("touch crystal".to_owned())
            }
            At::Touched => {
                let Some(commands) = self.back() else {
                    return Next::Failed;
                };
                self.at = At::Back;
                moves(commands)
            }
            At::Back => {
                self.at = At::Gone;
                Next::Go("go barrier".to_owned())
            }
            At::Gone => Next::Done,
        }
    }
}

#[cfg(test)]
mod tests {
    use cena_session::ChunkLine;

    use super::super::testing::Scene;
    use super::*;

    fn walk(commands: &[&str]) -> Next {
        moves(commands.iter().map(|dir| (*dir).to_owned()).collect())
    }

    fn linked(name: &str) -> Scene {
        let mut scene = Scene::at(1, 2);
        scene.walker.sees = Some(vec!["a crystal".into(), name.into()]);
        scene
    }

    /// Every row, written out in full rather than derived: what upstream's
    /// `split`, `reverse` and `reverse_direction` come to.
    const ROUND_TRIPS: &[(&str, &[&str], &[&str])] = &[
        (
            "blue",
            &["n", "sw", "sw", "go grotto"],
            &["out", "ne", "ne", "s"],
        ),
        (
            "black",
            &["n", "sw", "sw", "sw", "s", "se", "go grotto"],
            &["out", "nw", "n", "ne", "ne", "ne", "s"],
        ),
        (
            "yellow",
            &["n", "se", "se", "se", "s", "sw", "go grotto"],
            &["out", "ne", "n", "nw", "nw", "nw", "s"],
        ),
        (
            "red",
            &["n", "se", "se", "go grotto"],
            &["out", "nw", "nw", "s"],
        ),
    ];

    #[test]
    fn each_colour_walks_out_touches_and_walks_back_the_same_way() {
        assert_eq!(ROUND_TRIPS.len(), WALKS.len());
        for (colour, out, back) in ROUND_TRIPS {
            let mut barrier = ColourBarrier::default();
            let name = format!("a shimmering {colour} barrier");
            assert_eq!(linked(&name).ask(&mut barrier), walk(out));
            assert_eq!(
                Scene::at(5, 2).ask(&mut barrier),
                Next::Put("touch crystal".into())
            );
            assert_eq!(Scene::at(5, 2).ask(&mut barrier), walk(back));
            assert_eq!(
                Scene::at(1, 2).ask(&mut barrier),
                Next::Go("go barrier".into())
            );
            assert_eq!(Scene::at(2, 2).ask(&mut barrier), Next::Done);
        }
    }

    #[test]
    fn the_descriptions_own_words_will_do() {
        let mut scene = Scene::at(1, 2);
        let line = ChunkLine::plain("A red barrier of light fills the arch.");
        scene.state.room.description = Some(line.runs);
        let mut barrier = ColourBarrier::default();
        assert_eq!(scene.ask(&mut barrier), walk(ROUND_TRIPS[3].1));
    }

    #[test]
    fn no_barrier_of_a_known_colour_cannot_be_crossed() {
        let mut barrier = ColourBarrier::default();
        assert_eq!(linked("a green barrier").ask(&mut barrier), Next::Failed);
    }

    #[test]
    fn a_walk_that_fails_ends_it_either_way() {
        let mut barrier = ColourBarrier::default();
        linked("a blue barrier").ask(&mut barrier);
        let mut lost = Scene::at(3, 2);
        lost.failed = true;
        assert_eq!(lost.ask(&mut barrier), Next::Done);

        let mut barrier = ColourBarrier::default();
        linked("a blue barrier").ask(&mut barrier);
        Scene::at(5, 2).ask(&mut barrier);
        Scene::at(5, 2).ask(&mut barrier);
        let mut lost = Scene::at(4, 2);
        lost.failed = true;
        assert_eq!(lost.ask(&mut barrier), Next::Done);
    }
}

//! `Puzzle::LabyrinthEntry`: the mural's leaves (room 9767).
//!
//! Upstream (`upstream_scripts/labyrinth_entry.rb`): `touch green leaf` until
//! "You touch the". When the game says "You shouldn't mess with that while
//! `<someone>` is working on it." wait up to fifteen seconds for the leaves to
//! fade or for them to be taken in, and touch again. Any other answer is
//! "unknown result", and upstream **goes on regardless**; so does this. Then
//! touch the gold, red and blighted leaves and the outstretched hand, recite
//! the verse, and wait up to twenty seconds to be somewhere else.
//!
//! # Where this differs, and why
//!
//! - Upstream's wait reads one line and gives up unless it is the fading
//!   (`until !retry.match(get) or …`), which is plainly not what it means.
//!   This waits for either line, for the fifteen seconds.
//! - Upstream waits on somebody else without end. [`MAX_WAITS`] bounds it.
//! - "Somewhere else" is the room the routine began in, not the literal 9767.

use cena_map::RoomId;

use super::{Next, Seen, Solver};

const TOUCH: [&str; 4] = [
    "gold leaf",
    "red leaf",
    "blighted leaf",
    "outstretched hand",
];
const VERSE: &str = "recite Listen to the wind in the forest,;That which whispers the words \
    of the lady of the tower.;She of old who is called Maaghara.";
const BUSY: &str = "You shouldn't mess with that while";
const FREED: [&str; 2] = [
    "The shimmering leaves on the wall begin to fade, blending once again into the mural.",
    "form is quickly lost in a bright shroud of light as",
];
/// How many times somebody else is waited for: five minutes of them.
const MAX_WAITS: u32 = 20;
/// Seconds to be carried off in, as upstream.
const MAX_CARRY: u32 = 20;

#[derive(Default)]
pub(super) struct LabyrinthEntry {
    began: Option<RoomId>,
    waits: u32,
    at: At,
}

#[derive(Default)]
enum At {
    #[default]
    Start,
    Green,
    Touching(usize),
    Carried(u32),
}

impl Solver for LabyrinthEntry {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Start => {
                self.began = self.began.or(seen.here);
                self.at = At::Green;
                Next::Put("touch green leaf".to_owned())
            }
            At::Green if !seen.answered("You touch the") && seen.answered(BUSY) => {
                if self.waits >= MAX_WAITS {
                    return Next::Failed;
                }
                self.waits += 1;
                // Whether or not the line comes, the leaf is touched again.
                self.at = At::Start;
                Next::Await(FREED.map(str::to_owned).to_vec(), 15_000)
            }
            At::Green => {
                self.at = At::Touching(0);
                self.next(seen)
            }
            At::Touching(done) => {
                self.at = At::Touching(done + 1);
                if let Some(thing) = TOUCH.get(done) {
                    return Next::Put(format!("touch {thing}"));
                }
                self.at = At::Carried(0);
                Next::Put(VERSE.to_owned())
            }
            At::Carried(waited) => {
                if seen.here != self.began || waited >= MAX_CARRY {
                    return Next::Done;
                }
                self.at = At::Carried(waited + 1);
                Next::Pause(1000)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Scene;
    use super::*;

    const BUSY_LINE: &str = "You shouldn't mess with that while Xanith is working on it.";

    fn rest() -> Vec<Next> {
        let mut all: Vec<Next> = TOUCH
            .iter()
            .map(|thing| Next::Put(format!("touch {thing}")))
            .collect();
        all.push(Next::Put(VERSE.into()));
        all
    }

    #[test]
    fn the_leaves_are_touched_in_order_and_the_verse_recited() {
        let mut mural = LabyrinthEntry::default();
        let here = Scene::at(9767, 2);
        assert_eq!(here.ask(&mut mural), Next::Put("touch green leaf".into()));
        let touched = Scene::at(9767, 2).answered(&["You touch the green leaf."]);
        let mut sent = vec![touched.ask(&mut mural)];
        sent.extend((0..4).map(|_| here.ask(&mut mural)));
        assert_eq!(sent, rest());
        assert_eq!(here.ask(&mut mural), Next::Pause(1000));
        assert_eq!(Scene::at(2, 2).ask(&mut mural), Next::Done);
    }

    #[test]
    fn somebody_at_the_leaves_is_waited_for_and_the_leaf_touched_again() {
        let mut mural = LabyrinthEntry::default();
        Scene::at(9767, 2).ask(&mut mural);
        let busy = Scene::at(9767, 2).answered(&[BUSY_LINE]);
        assert_eq!(
            busy.ask(&mut mural),
            Next::Await(FREED.map(str::to_owned).to_vec(), 15_000)
        );
        // Whether or not the line came, as upstream.
        let mut late = Scene::at(9767, 2);
        late.failed = true;
        assert_eq!(late.ask(&mut mural), Next::Put("touch green leaf".into()));
    }

    #[test]
    fn an_unknown_answer_goes_on_as_upstream_does() {
        let mut mural = LabyrinthEntry::default();
        Scene::at(9767, 2).ask(&mut mural);
        assert_eq!(
            Scene::at(9767, 2).ask(&mut mural),
            Next::Put("touch gold leaf".into())
        );
    }

    #[test]
    fn waiting_on_somebody_is_bounded() {
        let mut mural = LabyrinthEntry::default();
        let busy = Scene::at(9767, 2).answered(&[BUSY_LINE]);
        let mut asked = 0;
        let last = loop {
            asked += 1;
            let next = busy.ask(&mut mural);
            if next == Next::Failed || asked > 1000 {
                break next;
            }
        };
        assert_eq!(last, Next::Failed);
        // A touch and a wait for each, and the touch that was one too many.
        assert_eq!(asked, MAX_WAITS * 2 + 2);
    }

    #[test]
    fn a_walker_never_carried_off_is_done_after_twenty_seconds() {
        let mut mural = LabyrinthEntry::default();
        let here = Scene::at(9767, 2);
        for _ in 0..6 {
            here.ask(&mut mural);
        }
        let pauses = (0..MAX_CARRY)
            .filter(|_| here.ask(&mut mural) == Next::Pause(1000))
            .count();
        assert_eq!(pauses, MAX_CARRY as usize);
        assert_eq!(here.ask(&mut mural), Next::Done);
    }
}

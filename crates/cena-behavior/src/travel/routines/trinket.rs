//! `Routine::Trinket`: the Mist Harbor trinket (31 exits).
//!
//! Upstream (`upstream_scripts/fwi_trinket.rb`): the trinket is the thing the
//! profile names as `fwi_trinket`. If `GameObj` cannot already see it -- worn,
//! or in a hand -- then free a hand when both are full, `get my <trinket>`,
//! and note the container the game says it came out of; no trinket is the end
//! of it. Write down the room being left as `fwi_return_room` -- or clear it,
//! when leaving Mist Harbor -- then `turn #<id>`. A trinket that was taken out
//! goes back where it came from, the hands are filled again, and
//! `$go2_restart`: arrival in Mist Harbor is a random room, so the trip looks
//! at where it landed.
//!
//! Taking out, putting back and the hands are the trip's own steps
//! (`Action::TakeOut`, `Action::PutBack`, `Action::EmptyHands`,
//! `Action::FillHands`), so what is owed back is known to the trip that owes
//! it, and is paid even if this routine is stopped half-way.
//!
//! # Where this leaves upstream, and why
//!
//! - **Which way the trinket is going is told by the goal**, not by where the
//!   walker stands: a solver is not shown the map, so it cannot ask the
//!   location of its own room. A goal in Mist Harbor is a town being left,
//!   and any other goal is Mist Harbor being left. The two agree wherever no
//!   trinket exit runs from the island to the island.
//! - **Both hands are emptied** where upstream empties one: the trip's step
//!   for it has no one-handed form.
//! - **`turn my <trinket>`** when the model holds no id for it; upstream
//!   would raise on `nil.id`.

use cena_map::{Action, Step};

use super::{Next, Seen, Solver};

/// The profile setting that names the trinket.
const SETTING: &str = "fwi_trinket";
/// Where the way back from Mist Harbor leads.
const MEMORY: &str = "fwi_return_room";

pub(super) struct Trinket {
    /// The goal is in Mist Harbor: a town is being left, and is remembered.
    to_harbor: bool,
    /// The trinket was taken out of a container, and goes back.
    taken: bool,
    /// Both hands were full, and are filled again.
    emptied: bool,
    at: At,
}

enum At {
    Start,
    HandsFreed,
    TakenOut,
    Noted,
    Turned,
    Finished,
}

impl Trinket {
    pub fn new(to_harbor: bool) -> Self {
        Trinket {
            to_harbor,
            taken: false,
            emptied: false,
            at: At::Start,
        }
    }

    fn start(&mut self, seen: &Seen<'_>) -> Next {
        let Some(name) = seen.walker.settings.get(SETTING) else {
            return Next::Failed;
        };
        if at_hand(seen, name).is_some() || wearing(seen, name) {
            return self.note(seen);
        }
        let hands = &seen.state;
        if hands.left_hand.is_holding() && hands.right_hand.is_holding() {
            self.emptied = true;
            self.at = At::HandsFreed;
            return steps(Action::EmptyHands);
        }
        self.take_out(name)
    }

    fn take_out(&mut self, name: &str) -> Next {
        self.taken = true;
        self.at = At::TakenOut;
        steps(Action::TakeOut(name.to_owned()))
    }

    /// `UserVars.mapdb_fwi_return_room = …`, before the turn as upstream has
    /// it.
    fn note(&mut self, seen: &Seen<'_>) -> Next {
        self.at = At::Noted;
        match seen.here {
            Some(here) if self.to_harbor => {
                steps(Action::Remember(MEMORY.to_owned(), here.0.to_string()))
            }
            _ => steps(Action::Forget(MEMORY.to_owned())),
        }
    }

    fn turn(&mut self, seen: &Seen<'_>) -> Next {
        self.at = At::Turned;
        let name = seen.walker.settings.get(SETTING);
        let id = name.and_then(|name| at_hand(seen, name).or_else(|| worn_id(seen, name)));
        Next::Put(match (id, name) {
            (Some(id), _) => format!("turn #{id}"),
            (None, Some(name)) => format!("turn my {name}"),
            (None, None) => return Next::Failed,
        })
    }

    fn tidy(&mut self) -> Next {
        self.at = At::Finished;
        let mut owed = Vec::new();
        if self.taken {
            owed.push(step(Action::PutBack));
        }
        if self.emptied {
            owed.push(step(Action::FillHands));
        }
        if owed.is_empty() {
            Next::Done
        } else {
            Next::Steps(owed)
        }
    }
}

fn step(action: Action) -> Step {
    Step { action, when: None }
}

fn steps(action: Action) -> Next {
    Next::Steps(vec![step(action)])
}

/// Whether a thing called this is what the profile names: the whole name, its
/// end (`a gold ring` for `gold ring`), or the noun alone.
fn named(full: &str, noun: &str, wanted: &str) -> bool {
    full == wanted || noun == wanted || full.ends_with(&format!(" {wanted}"))
}

/// The id of the trinket, if a hand holds it.
fn at_hand(seen: &Seen<'_>, wanted: &str) -> Option<String> {
    [&seen.state.right_hand, &seen.state.left_hand]
        .into_iter()
        .find(|hand| named(hand.name().unwrap_or(""), hand.noun().unwrap_or(""), wanted))
        .and_then(|hand| hand.id().map(str::to_owned))
}

fn wearing(seen: &Seen<'_>, wanted: &str) -> bool {
    let walker = seen.walker;
    walker.worn.as_ref().is_some_and(|w| w.contains(wanted))
        || walker
            .worn_nouns
            .as_ref()
            .is_some_and(|w| w.contains(wanted))
}

/// The id of the trinket, if it is worn.
fn worn_id(seen: &Seen<'_>, wanted: &str) -> Option<String> {
    seen.state
        .inventory_snapshot
        .on_person()
        .find(|item| {
            let full = format!("{} {}", item.adjective, item.noun);
            named(full.trim(), &item.noun, wanted)
        })
        .map(|item| item.id.clone())
}

impl Solver for Trinket {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Start => self.start(seen),
            At::HandsFreed => match seen.walker.settings.get(SETTING) {
                Some(name) => self.take_out(&name.clone()),
                None => Next::Failed,
            },
            // `fail "could not find your fwi trinket"`. The hands are the
            // trip's to give back.
            At::TakenOut if !seen.ok => Next::Failed,
            At::TakenOut => self.note(seen),
            At::Noted => self.turn(seen),
            At::Turned => self.tidy(),
            At::Finished => Next::Done,
        }
    }
}

#[cfg(test)]
mod tests {
    use cena_session::InventoryItem;
    use cena_session::hands::Hand;

    use super::super::testing::Scene;
    use super::*;

    const NAME: &str = "silver locket";

    fn scene(here: u32) -> Scene {
        let mut scene = Scene::at(here, 3668);
        scene.walker.settings.insert(SETTING.into(), NAME.into());
        scene
    }

    fn holding(id: &str, name: &str, noun: &str) -> Hand {
        Hand::Holding {
            id: Some(id.into()),
            noun: Some(noun.into()),
            name: name.into(),
        }
    }

    /// The trinket worn, as both the walker's facts and the model have it.
    fn wearing_it(mut scene: Scene) -> Scene {
        scene.walker.worn = Some([NAME.to_owned()].into());
        scene.walker.worn_nouns = Some(["locket".to_owned()].into());
        let items = [
            InventoryItem {
                id: "11".into(),
                relation: "worn".into(),
                parent: "player".into(),
                adjective: "dark".into(),
                noun: "cloak".into(),
                ..InventoryItem::default()
            },
            InventoryItem {
                id: "77".into(),
                relation: "worn".into(),
                parent: "player".into(),
                adjective: "silver".into(),
                noun: "locket".into(),
                ..InventoryItem::default()
            },
        ];
        scene
            .state
            .inventory_snapshot
            .apply_snapshot("7", &items, &[], None);
        scene
    }

    fn remembers(room: &str) -> Next {
        steps(Action::Remember(MEMORY.into(), room.into()))
    }

    #[test]
    fn a_worn_trinket_is_turned_where_it_is_and_the_town_remembered() {
        let mut trinket = Trinket::new(true);
        assert_eq!(wearing_it(scene(228)).ask(&mut trinket), remembers("228"));
        assert_eq!(
            wearing_it(scene(228)).ask(&mut trinket),
            Next::Put("turn #77".into())
        );
        assert_eq!(wearing_it(scene(3668)).ask(&mut trinket), Next::Done);
    }

    #[test]
    fn leaving_mist_harbor_forgets_the_way_back() {
        let mut trinket = Trinket::new(false);
        assert_eq!(
            wearing_it(scene(3668)).ask(&mut trinket),
            steps(Action::Forget(MEMORY.into()))
        );
    }

    #[test]
    fn a_walker_the_map_cannot_place_remembers_nothing() {
        let mut trinket = Trinket::new(true);
        let mut lost = wearing_it(scene(228));
        lost.here = None;
        assert_eq!(lost.ask(&mut trinket), steps(Action::Forget(MEMORY.into())));
    }

    #[test]
    fn a_trinket_put_away_is_taken_out_turned_and_put_back() {
        let mut trinket = Trinket::new(true);
        assert_eq!(
            scene(228).ask(&mut trinket),
            steps(Action::TakeOut(NAME.into()))
        );
        let mut out = scene(228);
        out.state.right_hand = holding("77", "silver locket", "locket");
        assert_eq!(out.ask(&mut trinket), remembers("228"));
        assert_eq!(out.ask(&mut trinket), Next::Put("turn #77".into()));
        assert_eq!(
            scene(3668).ask(&mut trinket),
            Next::Steps(vec![step(Action::PutBack)])
        );
        assert_eq!(scene(3668).ask(&mut trinket), Next::Done);
    }

    #[test]
    fn full_hands_are_emptied_first_and_filled_last() {
        let mut trinket = Trinket::new(true);
        let mut full = scene(228);
        full.state.right_hand = holding("1", "steel broadsword", "broadsword");
        full.state.left_hand = holding("2", "wooden shield", "shield");
        assert_eq!(full.ask(&mut trinket), steps(Action::EmptyHands));
        assert_eq!(
            scene(228).ask(&mut trinket),
            steps(Action::TakeOut(NAME.into()))
        );
        let mut out = scene(228);
        out.state.left_hand = holding("77", "a silver locket", "locket");
        assert_eq!(out.ask(&mut trinket), remembers("228"));
        assert_eq!(out.ask(&mut trinket), Next::Put("turn #77".into()));
        assert_eq!(
            scene(3668).ask(&mut trinket),
            Next::Steps(vec![step(Action::PutBack), step(Action::FillHands)])
        );
    }

    #[test]
    fn one_full_hand_is_left_alone() {
        let mut trinket = Trinket::new(true);
        let mut armed = scene(228);
        armed.state.right_hand = holding("1", "steel broadsword", "broadsword");
        armed.state.left_hand = Hand::Empty;
        assert_eq!(armed.ask(&mut trinket), steps(Action::TakeOut(NAME.into())));
    }

    #[test]
    fn a_trinket_already_in_hand_is_turned_and_not_put_anywhere() {
        let mut trinket = Trinket::new(true);
        let mut out = scene(228);
        out.state.right_hand = holding("77", "silver locket", "locket");
        assert_eq!(out.ask(&mut trinket), remembers("228"));
        assert_eq!(out.ask(&mut trinket), Next::Put("turn #77".into()));
        assert_eq!(out.ask(&mut trinket), Next::Done);
    }

    #[test]
    fn no_trinket_to_take_out_gives_the_exit_up() {
        let mut trinket = Trinket::new(true);
        let _ = scene(228).ask(&mut trinket);
        let mut none = scene(228);
        none.failed = true;
        assert_eq!(none.ask(&mut trinket), Next::Failed);
    }

    #[test]
    fn a_profile_that_names_no_trinket_cannot_cross() {
        let mut trinket = Trinket::new(true);
        assert_eq!(Scene::at(228, 3668).ask(&mut trinket), Next::Failed);
    }

    #[test]
    fn a_trinket_with_no_id_is_turned_by_name() {
        let mut trinket = Trinket::new(true);
        let mut worn = scene(228);
        worn.walker.worn_nouns = Some([NAME.to_owned()].into());
        let _ = worn.ask(&mut trinket);
        assert_eq!(
            worn.ask(&mut trinket),
            Next::Put("turn my silver locket".into())
        );
    }
}

//! `Routine::Seeking`: the Order of Voln's symbol of seeking.
//!
//! Upstream (`upstream_scripts/seeking_engine.rb`): up to twenty times, `put
//! "symbol of seeking"` and read the room name the vision shows, less a
//! trailing ` (12345)`. The name that is one of the destination's titles ends
//! the asking; the first name coming round again means the destination is not
//! on offer, and upstream says so and kills the trip. Then `symbol of seeking
//! confirm`, and wait for `Your surroundings blur into a white fog`.
//!
//! The vision's room name is told from the rest of the answer by its markup,
//! `<style id="roomName" />`, as upstream's pattern tells it -- here read from
//! the parsed runs, not scanned for. It may come after the prompt that ends
//! the command's answer (upstream waits six seconds for it), so an answer
//! without one is followed by one wait for a line holding `[`.
//!
//! # Where this leaves upstream, and why
//!
//! - **Nothing is confirmed that was not seen to be the destination.**
//!   Upstream falls out of its twenty tries and confirms whatever was offered
//!   last -- or nothing it ever read. This gives the exit up instead.
//! - **The memory is written after arriving**, not before confirming
//!   (`cena_map::Routine::Seeking`): a confirm that fails leaves no false
//!   memory. It is written only when the walker is seen at the goal.
//! - **`set roomname on`/`off` is not sent.** Upstream finds out that room
//!   names are off by scanning the server buffer, which a solver is not shown.
//!   A walker with room names off never sees a name, and the exit is given up
//!   after twenty tries.

use cena_map::{Action, Step};
use cena_session::ChunkLine;

use super::{Next, Seen, Solver};

/// Upstream's `20.times`.
const MAX_OFFERS: u32 = 20;
/// Upstream's `matchtimeout(6, …)` and `dothistimeout(…, 6, …)`.
const WAIT_MS: u64 = 6000;
const FOG: &str = "Your surroundings blur into a white fog";

pub(super) struct Seeking {
    /// The goal's titles, as the map has them: brackets included.
    titles: Vec<String>,
    remember: Option<(String, String)>,
    /// The first name offered: seeing it again is having seen them all.
    first: Option<String>,
    asked: u32,
    at: At,
}

enum At {
    Asking,
    /// The symbol was sent; `true` once the vision has been waited for too.
    Offered(bool),
    Confirmed,
    Landed,
    Finished,
}

impl Seeking {
    pub fn new(titles: Vec<String>, remember: Option<(String, String)>) -> Self {
        Seeking {
            titles,
            remember,
            first: None,
            asked: 0,
            at: At::Asking,
        }
    }

    fn ask(&mut self) -> Next {
        if self.asked >= MAX_OFFERS || self.titles.is_empty() {
            return Next::Failed;
        }
        self.asked += 1;
        self.at = At::Offered(false);
        Next::Put("symbol of seeking".to_owned())
    }

    fn offered(&mut self, seen: &Seen<'_>, waited: bool) -> Next {
        let Some(name) = seen.answer.iter().find_map(room_name) else {
            if waited {
                return self.ask();
            }
            self.at = At::Offered(true);
            return Next::Await(vec!["[".to_owned()], WAIT_MS);
        };
        if self.first.as_deref() == Some(name.as_str()) {
            return Next::Stop(format!(
                "The symbol of seeking does not offer {}. It has shown every \
                 place it will take you, and that is not one of them.",
                self.titles.first().map_or("that place", String::as_str)
            ));
        }
        self.first.get_or_insert_with(|| name.clone());
        if !self.titles.contains(&name) {
            return self.ask();
        }
        self.at = At::Confirmed;
        Next::Put("symbol of seeking confirm".to_owned())
    }

    fn landed(&mut self, seen: &Seen<'_>) -> Next {
        self.at = At::Finished;
        match self.remember.take() {
            Some((name, value)) if seen.here == Some(seen.goal) => Next::Steps(vec![Step {
                action: Action::Remember(name, value),
                when: None,
            }]),
            _ => Next::Done,
        }
    }
}

/// The room name a line shows, if it shows one: the text marked `roomName`,
/// less the room number some walkers have the game add.
fn room_name(line: &ChunkLine) -> Option<String> {
    let marked: String = line
        .runs
        .runs
        .iter()
        .filter(|run| run.style.preset.as_deref() == Some("roomName"))
        .map(|run| run.text.as_str())
        .collect();
    let name = marked.trim();
    let name = match name.rsplit_once(" (") {
        Some((title, number))
            if number
                .strip_suffix(')')
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit())) =>
        {
            title
        }
        _ => name,
    };
    (!name.is_empty()).then(|| name.to_owned())
}

impl Solver for Seeking {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Asking => self.ask(),
            At::Offered(waited) => self.offered(seen, waited),
            At::Confirmed => {
                self.at = At::Landed;
                if seen.answered(FOG) {
                    return self.landed(seen);
                }
                Next::Await(vec![FOG.to_owned()], WAIT_MS)
            }
            At::Landed => self.landed(seen),
            At::Finished => Next::Done,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Scene;
    use super::*;

    const GOAL: &str = "[Red Forest, Path]";

    fn seeking(remember: bool) -> Seeking {
        Seeking::new(
            vec!["[Red Forest, Trail]".into(), GOAL.into()],
            remember.then(|| ("redforest_location".into(), "WL".into())),
        )
    }

    /// A vision: a line of prose, and the room's name marked as the game
    /// marks it.
    fn vision(name: &str) -> Scene {
        let mut scene = Scene::at(3600, 24715).answered(&["Your vision is pulled away...", name]);
        for run in scene
            .answer
            .iter_mut()
            .skip(1)
            .flat_map(|line| &mut line.runs.runs)
        {
            run.style.preset = Some("roomName".into());
        }
        scene
    }

    fn symbol() -> Next {
        Next::Put("symbol of seeking".into())
    }

    fn remembers() -> Next {
        Next::Steps(vec![Step {
            action: Action::Remember("redforest_location".into(), "WL".into()),
            when: None,
        }])
    }

    #[test]
    fn it_asks_until_the_goal_is_offered_confirms_and_remembers_on_arrival() {
        let mut voln = seeking(true);
        assert_eq!(Scene::at(3600, 24715).ask(&mut voln), symbol());
        assert_eq!(vision("[Graveyard, Gate]").ask(&mut voln), symbol());
        // A second title of the goal, with the room number some flags add.
        assert_eq!(
            vision("[Red Forest, Path] (24715)").ask(&mut voln),
            Next::Put("symbol of seeking confirm".into())
        );
        assert_eq!(
            Scene::at(3600, 24715).ask(&mut voln),
            Next::Await(vec![FOG.into()], 6000)
        );
        assert_eq!(Scene::at(24715, 24715).ask(&mut voln), remembers());
        assert_eq!(Scene::at(24715, 24715).ask(&mut voln), Next::Done);
    }

    #[test]
    fn fog_in_the_confirms_answer_is_not_waited_for_again() {
        let mut voln = seeking(true);
        let _ = Scene::at(3600, 24715).ask(&mut voln);
        let _ = vision(GOAL).ask(&mut voln);
        let fog = Scene::at(24715, 24715).answered(&[FOG]);
        assert_eq!(fog.ask(&mut voln), remembers());
    }

    #[test]
    fn nothing_is_remembered_by_a_walker_that_did_not_arrive_or_has_nothing_to() {
        let mut voln = seeking(true);
        let _ = Scene::at(3600, 24715).ask(&mut voln);
        let _ = vision(GOAL).ask(&mut voln);
        let _ = Scene::at(3600, 24715).ask(&mut voln);
        assert_eq!(Scene::at(3600, 24715).ask(&mut voln), Next::Done);

        let mut plain = seeking(false);
        let _ = Scene::at(3600, 24715).ask(&mut plain);
        let _ = vision(GOAL).ask(&mut plain);
        let _ = Scene::at(3600, 24715).ask(&mut plain);
        assert_eq!(Scene::at(24715, 24715).ask(&mut plain), Next::Done);
    }

    #[test]
    fn the_first_name_coming_round_again_stops_the_trip() {
        let mut voln = seeking(false);
        let _ = Scene::at(3600, 24715).ask(&mut voln);
        assert_eq!(vision("[Graveyard, Gate]").ask(&mut voln), symbol());
        assert_eq!(vision("[Icemule, South Gate]").ask(&mut voln), symbol());
        assert!(matches!(
            vision("[Graveyard, Gate]").ask(&mut voln),
            Next::Stop(why) if why.contains("[Red Forest, Trail]")
        ));
    }

    #[test]
    fn a_vision_that_comes_late_is_waited_for_once_and_then_asked_for_again() {
        let mut voln = seeking(false);
        let _ = Scene::at(3600, 24715).ask(&mut voln);
        // Prose in brackets is not a room name: only the markup says so.
        let unmarked = || Scene::at(3600, 24715).answered(&["[Red Forest, Path]"]);
        assert_eq!(
            unmarked().ask(&mut voln),
            Next::Await(vec!["[".into()], 6000)
        );
        assert_eq!(unmarked().ask(&mut voln), symbol());
        assert_eq!(
            Scene::at(3600, 24715).ask(&mut voln),
            Next::Await(vec!["[".into()], 6000)
        );
        assert_eq!(
            vision(GOAL).ask(&mut voln),
            Next::Put("symbol of seeking confirm".into())
        );
    }

    #[test]
    fn twenty_offers_is_the_end_and_nothing_is_confirmed() {
        let mut voln = seeking(false);
        let mut sent = 0;
        let mut scene = Scene::at(3600, 24715);
        loop {
            match scene.ask(&mut voln) {
                Next::Put(command) => {
                    assert_eq!(command, "symbol of seeking");
                    sent += 1;
                    scene = vision(&format!("[Somewhere, {sent}]"));
                }
                last => {
                    assert_eq!(last, Next::Failed);
                    break;
                }
            }
        }
        assert_eq!(sent, 20);
    }

    #[test]
    fn a_goal_the_map_has_no_title_for_cannot_be_sought() {
        let mut voln = Seeking::new(Vec::new(), None);
        assert_eq!(Scene::at(3600, 24715).ask(&mut voln), Next::Failed);
    }
}

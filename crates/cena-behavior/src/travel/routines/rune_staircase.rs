//! `Puzzle::RuneStaircase`: the dwarven runes of room 18178.
//!
//! Upstream (`upstream_scripts/rune_staircase.rb`): `look staircase`; if "you
//! could easily scramble up them to the ledge above", it is already formed.
//! Otherwise, until it is: empty the hands if both are full; `look runes` to
//! learn which rune the fifth "is directly under"; and `press` the rune that
//! answers it -- under O press R, under R press S, under Z press Z, under S
//! press O, under E press E. The press says what the staircase did and where
//! the fifth rune went ("It is now lined up under the ..."), which is the
//! next press; "Nothing happens" means press E next; a bare "You touch the E
//! rune." lowered the lot, and the runes are looked at again. A "complete
//! staircase" ends it: hands back, `go staircase`.
//!
//! Deviations. When `look runes` cannot be read upstream sets the rune to a
//! lower-case `e`, which is in no table, and sends `press  rune`; here the
//! runes are looked at again instead. A rune that is in no table is where
//! upstream pauses with "something went wrong", and is [`Next::Stop`] here.
//! Upstream's loop is unbounded; every command sent here counts toward
//! [`MAX_TURNS`]. "complete staircase" is looked for in all the press says
//! before "Curiously", where upstream looks only in its middle sentence.

use cena_map::{Action, Step};
use cena_session::ChunkLine;

use super::{MAX_TURNS, Next, Seen, Solver};

const FORMED: &str = "you could easily scramble up them to the ledge above";
const LABELED: &str = "The last one, labeled ";
const UNDER: &str = "is directly under the ";
const SHIFTED: &str = "rune seems to have shifted position.";
const NOW_UNDER: &str = "It is now lined up under the ";
const NOTHING: &str = "Nothing happens";
const LOWERED: &str = "You touch the E rune.";
const COMPLETE: &str = "complete staircase";

/// Under this rune, press that one. Upstream's `runes`, in its order.
const PRESS: &[(char, char)] = &[('O', 'R'), ('R', 'S'), ('Z', 'Z'), ('S', 'O'), ('E', 'E')];

fn press_under(under: char) -> Option<char> {
    PRESS
        .iter()
        .find(|(rune, _)| *rune == under)
        .map(|(_, press)| *press)
}

/// The letter that follows `marker`.
fn letter_after(text: &str, marker: &str) -> Option<char> {
    let at = text.find(marker)? + marker.len();
    text[at..].chars().next().filter(|c| c.is_alphanumeric())
}

fn step(action: Action) -> Step {
    Step { action, when: None }
}

#[derive(Default)]
pub(super) struct RuneStaircase {
    /// The rune the fifth is under. `None` until the runes are read.
    under: Option<char>,
    hands_emptied: bool,
    turns: u32,
    at: At,
}

#[derive(Default)]
enum At {
    #[default]
    Start,
    LookedAtStairs,
    /// Between commands: hands, then a look or a press.
    Working,
    LookedAtRunes,
    Pressed,
    /// Formed: hands back, and up.
    Leaving,
    Gone,
}

impl RuneStaircase {
    fn put(&mut self, at: At, command: String) -> Next {
        if self.turns >= MAX_TURNS {
            return Next::Failed;
        }
        self.turns += 1;
        self.at = at;
        Next::Put(command)
    }

    fn work(&mut self, seen: &Seen<'_>) -> Next {
        let state = seen.state;
        if state.right_hand.id().is_some() && state.left_hand.id().is_some() {
            self.hands_emptied = true;
            self.at = At::Working;
            return Next::Steps(vec![step(Action::EmptyHands)]);
        }
        let Some(under) = self.under else {
            return self.put(At::LookedAtRunes, "look runes".to_owned());
        };
        match press_under(under) {
            Some(press) => self.put(At::Pressed, format!("press {press} rune")),
            None => Next::Stop(format!(
                "Something went wrong working the staircase's runes: the last rune is under \
                 {under}, which is none of R, S, Z, O and E. Set the staircase by hand, or go \
                 another way, and start the trip again."
            )),
        }
    }

    /// What the press did. `true`: the staircase is complete.
    fn pressed(&mut self, seen: &Seen<'_>) -> bool {
        let lines: Vec<String> = seen.answer.iter().map(ChunkLine::text).collect();
        if let Some(text) = lines.iter().find(|text| text.contains(SHIFTED)) {
            self.under = letter_after(text, NOW_UNDER);
            let said = text
                .find("Curiously")
                .map_or(text.as_str(), |at| &text[..at]);
            return said.contains(COMPLETE);
        }
        if lines.iter().any(|text| text.contains(NOTHING)) {
            self.under = Some('E');
        } else if lines.iter().any(|text| text.contains(LOWERED)) {
            self.under = None;
        }
        false
    }
}

impl Solver for RuneStaircase {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Start => self.put(At::LookedAtStairs, "look staircase".to_owned()),
            At::LookedAtStairs if seen.answered(FORMED) => {
                self.at = At::Leaving;
                self.next(seen)
            }
            At::LookedAtStairs | At::Working => self.work(seen),
            At::LookedAtRunes => {
                self.under = seen
                    .answer
                    .iter()
                    .map(ChunkLine::text)
                    .find(|text| text.contains(LABELED))
                    .and_then(|text| letter_after(&text, UNDER));
                self.work(seen)
            }
            At::Pressed => {
                if self.pressed(seen) {
                    self.at = At::Leaving;
                    return self.next(seen);
                }
                self.work(seen)
            }
            At::Leaving if std::mem::take(&mut self.hands_emptied) => {
                Next::Steps(vec![step(Action::FillHands)])
            }
            At::Leaving => {
                self.at = At::Gone;
                Next::Go("go staircase".to_owned())
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

    fn put(command: &str) -> Next {
        Next::Put(command.to_owned())
    }

    fn hears(stairs: &mut RuneStaircase, line: &str) -> Next {
        Scene::at(1, 2).answered(&[line]).ask(stairs)
    }

    fn runes(under: char) -> String {
        format!(
            "There are five protrusions with rune markings.  They look worn.  Four of the \
             runes are lined up in a row, labeled: R, S, Z and O, in dwarven.  The last one, \
             labeled E, is directly under the {under} rune."
        )
    }

    fn shifted(stairs: &str, under: char) -> String {
        format!(
            "You touch the R rune.  With a loud grinding sound, some steps slide out.  \
             {stairs}  Curiously the E rune seems to have shifted position.  It is now lined \
             up under the {under} rune."
        )
    }

    fn holding(id: &str) -> Hand {
        Hand::Holding {
            id: Some(id.into()),
            noun: None,
            name: "thing".into(),
        }
    }

    #[test]
    fn a_staircase_already_formed_is_only_climbed() {
        let mut stairs = RuneStaircase::default();
        assert_eq!(Scene::at(1, 2).ask(&mut stairs), put("look staircase"));
        let formed = "The steps are whole, and you could easily scramble up them to the \
                      ledge above.";
        assert_eq!(hears(&mut stairs, formed), Next::Go("go staircase".into()));
        assert_eq!(Scene::at(2, 2).ask(&mut stairs), Next::Done);
    }

    #[test]
    fn each_press_says_where_the_rune_went_until_the_staircase_is_complete() {
        let mut stairs = RuneStaircase::default();
        Scene::at(1, 2).ask(&mut stairs);
        let senseless = "It really doesn't make much sense.";
        assert_eq!(hears(&mut stairs, senseless), put("look runes"));
        assert_eq!(hears(&mut stairs, &runes('O')), put("press R rune"));
        let partial = shifted("The stairs are a partial staircase.", 'S');
        assert_eq!(hears(&mut stairs, &partial), put("press O rune"));
        let complete = shifted("The stairs are now a complete staircase.", 'Z');
        assert_eq!(
            hears(&mut stairs, &complete),
            Next::Go("go staircase".into())
        );
    }

    #[test]
    fn every_rune_has_its_press() {
        let table = [('O', 'R'), ('R', 'S'), ('Z', 'Z'), ('S', 'O'), ('E', 'E')];
        assert_eq!(PRESS, table);
        for (under, press) in table {
            let mut stairs = RuneStaircase {
                at: At::LookedAtRunes,
                ..RuneStaircase::default()
            };
            assert_eq!(
                hears(&mut stairs, &runes(under)),
                put(&format!("press {press} rune"))
            );
        }
    }

    #[test]
    fn nothing_happening_means_e_and_a_bare_e_means_look_again() {
        let mut stairs = RuneStaircase {
            under: Some('O'),
            at: At::Pressed,
            ..RuneStaircase::default()
        };
        let nothing = "You touch the R rune.  Nothing happens.";
        assert_eq!(hears(&mut stairs, nothing), put("press E rune"));
        let lowered = "You touch the E rune.  With a loud grinding sound, the entire \
                       staircase lowers into the ledge.";
        assert_eq!(hears(&mut stairs, lowered), put("look runes"));
    }

    #[test]
    fn runes_that_cannot_be_read_are_looked_at_again() {
        let mut stairs = RuneStaircase {
            at: At::LookedAtRunes,
            ..RuneStaircase::default()
        };
        assert_eq!(hears(&mut stairs, "...wait 2 seconds."), put("look runes"));
    }

    #[test]
    fn a_rune_in_no_table_stops_the_trip() {
        let mut stairs = RuneStaircase {
            at: At::LookedAtRunes,
            ..RuneStaircase::default()
        };
        assert!(matches!(hears(&mut stairs, &runes('Q')), Next::Stop(_)));
    }

    #[test]
    fn two_full_hands_are_emptied_first_and_given_back_before_the_climb() {
        let mut stairs = RuneStaircase::default();
        Scene::at(1, 2).ask(&mut stairs);
        let mut full = Scene::at(1, 2);
        full.state.right_hand = holding("10");
        full.state.left_hand = holding("11");
        assert_eq!(
            full.ask(&mut stairs),
            Next::Steps(vec![step(Action::EmptyHands)])
        );
        // One full hand is left alone.
        let mut one = Scene::at(1, 2);
        one.state.right_hand = holding("10");
        one.state.left_hand = Hand::Empty;
        assert_eq!(one.ask(&mut stairs), put("look runes"));
        hears(&mut stairs, &runes('O'));
        let complete = shifted("A complete staircase.", 'Z');
        assert_eq!(
            hears(&mut stairs, &complete),
            Next::Steps(vec![step(Action::FillHands)])
        );
        assert_eq!(
            Scene::at(1, 2).ask(&mut stairs),
            Next::Go("go staircase".into())
        );
    }

    #[test]
    fn runes_that_never_form_a_staircase_are_given_up() {
        let mut stairs = RuneStaircase::default();
        for _ in 0..MAX_TURNS {
            assert!(matches!(Scene::at(1, 2).ask(&mut stairs), Next::Put(_)));
        }
        assert_eq!(Scene::at(1, 2).ask(&mut stairs), Next::Failed);
    }
}

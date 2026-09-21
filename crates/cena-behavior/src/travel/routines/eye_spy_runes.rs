//! `Puzzle::EyeSpyRunes`: the two rune panels an eye reads, room 18748.
//!
//! Upstream (`upstream_scripts/eye_spy_runes.rb`): wait for the mana and cast
//! Eye Spy (707); `tell eye to go` each of twelve ways; `tell eye to read
//! basalt`, and `draw <the rune that glows> rune on basalt`; the same for
//! `chalcedony`; `tell eye to return`. The exit's own steps do the leaving.
//!
//! # Where this is not upstream
//!
//! - A panel whose glowing rune is never read kills upstream's script (`nil`
//!   has no `captures`). Here that panel is not drawn on and the rest goes
//!   on: the eye is still called home, and the trip sees it did not cross.
//! - The rune line is looked for in the answer to `tell`, and only then
//!   waited for, upstream's ten seconds.
//! - The wait for mana is the shared, bounded one (`casting`).

use cena_map::{Action, Step};

use super::casting::{self, MANA_WAIT_MS, MANA_WAITS};
use super::{Next, Seen, Solver};

const EYE_SPY: u16 = 707;
const WALK: [&str; 12] = [
    "out",
    "south",
    "door",
    "northwest",
    "northeast",
    "west",
    "water",
    "grate",
    "southeast",
    "southeast",
    "open",
    "stair",
];
const PANELS: [&str; 2] = ["basalt", "chalcedony"];
const RUNES: [&str; 11] = [
    "wy'zio",
    "ag'loenar",
    "odeir'cos",
    "beiron'fyn",
    "ikar'fyn",
    "quiss'fyn",
    "erikar'fyn",
    "lorae'tyr",
    "shien'tyr",
    "vakra",
    "grk'tyr",
];
const READ_MS: u64 = 10_000;

#[derive(Default)]
enum At {
    #[default]
    Mana,
    /// The eye has gone this many of its ways.
    Walking(usize),
    /// `read` was sent for this panel; `true` once it was waited on too.
    Reading(usize, bool),
    /// `draw` was sent, or passed over, for this panel.
    Drawn(usize),
    Returned,
}

#[derive(Default)]
pub(super) struct EyeSpyRunes {
    at: At,
    waits: u32,
}

fn glows_on(panel: &str) -> String {
    format!("rune glows with a pallid light on the surface of the panel of {panel}")
}

/// The rune the answer says glows on this panel.
fn rune(seen: &Seen<'_>, panel: &str) -> Option<&'static str> {
    let glows = glows_on(panel);
    seen.answer.iter().find_map(|line| {
        let text = line.text();
        let before = &text[..text.find(&glows)?];
        RUNES
            .into_iter()
            .find(|rune| before.ends_with(&format!("The {rune} ")))
    })
}

fn read(panel: usize) -> Next {
    match PANELS.get(panel) {
        Some(name) => Next::Put(format!("tell eye to read {name}")),
        None => Next::Put("tell eye to return".into()),
    }
}

impl Solver for EyeSpyRunes {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Mana => {
                let Some(name) = casting::name_of(EYE_SPY) else {
                    return Next::Failed;
                };
                if !casting::affords(seen, EYE_SPY) {
                    self.waits += 1;
                    if self.waits > MANA_WAITS {
                        return Next::Stop(
                            "Waited ten minutes for the mana to cast Eye Spy.".to_owned(),
                        );
                    }
                    return Next::Pause(MANA_WAIT_MS);
                }
                self.at = At::Walking(0);
                Next::Steps(vec![Step {
                    action: Action::Cast(name.to_owned()),
                    when: None,
                }])
            }
            At::Walking(gone) => {
                if let Some(way) = WALK.get(gone) {
                    self.at = At::Walking(gone + 1);
                    return Next::Put(format!("tell eye to go {way}"));
                }
                self.at = At::Reading(0, false);
                read(0)
            }
            At::Reading(panel, waited) => {
                let Some(name) = PANELS.get(panel) else {
                    return Next::Done;
                };
                if let Some(rune) = rune(seen, name) {
                    self.at = At::Drawn(panel);
                    return Next::Put(format!("draw {rune} rune on {name}"));
                }
                if waited {
                    self.at = At::Drawn(panel);
                    return self.next(seen);
                }
                self.at = At::Reading(panel, true);
                Next::Await(vec![glows_on(name)], READ_MS)
            }
            At::Drawn(panel) => {
                self.at = if panel + 1 < PANELS.len() {
                    At::Reading(panel + 1, false)
                } else {
                    At::Returned
                };
                read(panel + 1)
            }
            At::Returned => Next::Done,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::casting::tests::affording;
    use super::super::testing::Scene;
    use super::*;

    fn glowing(rune: &str, panel: &str) -> Scene {
        let line = format!("The {rune} {}.", glows_on(panel));
        Scene::at(1, 2).answered(&[line.as_str()])
    }

    /// Cast, and walk the eye to the panels.
    fn at_the_panels() -> EyeSpyRunes {
        let mut eye = EyeSpyRunes::default();
        let spell = casting::name_of(707).unwrap().to_owned();
        assert_eq!(
            Scene::at(1, 2).ask(&mut eye),
            Next::Steps(vec![Step {
                action: Action::Cast(spell),
                when: None
            }])
        );
        for way in WALK {
            assert_eq!(
                Scene::at(1, 2).ask(&mut eye),
                Next::Put(format!("tell eye to go {way}"))
            );
        }
        assert_eq!(
            Scene::at(1, 2).ask(&mut eye),
            Next::Put("tell eye to read basalt".into())
        );
        eye
    }

    #[test]
    fn the_eye_walks_reads_both_panels_and_comes_home() {
        let mut eye = at_the_panels();
        assert_eq!(
            glowing("vakra", "basalt").ask(&mut eye),
            Next::Put("draw vakra rune on basalt".into())
        );
        assert_eq!(
            Scene::at(1, 2).ask(&mut eye),
            Next::Put("tell eye to read chalcedony".into())
        );
        assert_eq!(
            glowing("grk'tyr", "chalcedony").ask(&mut eye),
            Next::Put("draw grk'tyr rune on chalcedony".into())
        );
        assert_eq!(
            Scene::at(1, 2).ask(&mut eye),
            Next::Put("tell eye to return".into())
        );
        assert_eq!(Scene::at(1, 2).ask(&mut eye), Next::Done);
    }

    #[test]
    fn ikar_is_not_mistaken_for_erikar() {
        let mut eye = at_the_panels();
        assert_eq!(
            glowing("erikar'fyn", "basalt").ask(&mut eye),
            Next::Put("draw erikar'fyn rune on basalt".into())
        );
    }

    #[test]
    fn a_rune_that_comes_late_is_waited_for() {
        let mut eye = at_the_panels();
        assert_eq!(
            Scene::at(1, 2).ask(&mut eye),
            Next::Await(vec![glows_on("basalt")], READ_MS)
        );
        assert_eq!(
            glowing("vakra", "basalt").ask(&mut eye),
            Next::Put("draw vakra rune on basalt".into())
        );
    }

    #[test]
    fn the_other_panels_rune_is_not_drawn_here() {
        let mut eye = at_the_panels();
        assert_eq!(
            glowing("vakra", "chalcedony").ask(&mut eye),
            Next::Await(vec![glows_on("basalt")], READ_MS)
        );
    }

    #[test]
    fn a_panel_never_read_is_passed_over_and_the_eye_still_comes_home() {
        let mut eye = at_the_panels();
        Scene::at(1, 2).ask(&mut eye);
        assert_eq!(
            Scene::at(1, 2).ask(&mut eye),
            Next::Put("tell eye to read chalcedony".into())
        );
        Scene::at(1, 2).ask(&mut eye);
        assert_eq!(
            Scene::at(1, 2).ask(&mut eye),
            Next::Put("tell eye to return".into())
        );
        assert_eq!(Scene::at(1, 2).ask(&mut eye), Next::Done);
    }

    #[test]
    fn mana_is_waited_for_and_not_for_ever() {
        let mut eye = EyeSpyRunes::default();
        let poor = || affording(Scene::at(1, 2), &[]);
        for _ in 0..MANA_WAITS {
            assert_eq!(poor().ask(&mut eye), Next::Pause(MANA_WAIT_MS));
        }
        assert!(matches!(poor().ask(&mut eye), Next::Stop(_)));
    }
}

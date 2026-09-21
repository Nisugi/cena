//! `Puzzle::ThreePillars`: the sorcerers' three pillars, room 10781.
//!
//! Upstream (`upstream_scripts/three_pillars.rb`): for the `deep`, `black`
//! and `dark` pillar in turn, `look <it> pillar` and cast at it the spell its
//! symbol names -- waiting for **three times** the spell's mana first, and
//! casting again until the game says `Cast Roundtime`. A pillar showing no
//! symbol upstream knows is passed over. Then it ends; the exit's own steps
//! do the leaving.
//!
//! Upstream tests the symbols in the order of [`SYMBOLS`], which matters only
//! if a description ever holds two.
//!
//! # Where this is not upstream
//!
//! Both of upstream's loops are unbounded. The wait for mana is the shared
//! one (`casting`), and a pillar is cast at [`MAX_CASTS`] times before the
//! exit fails.

use super::casting::{Casting, Pay, Turn};
use super::{Next, Seen, Solver};

const PILLARS: [&str; 3] = ["deep pillar", "black pillar", "dark pillar"];
/// What the pillar shows, the spell that answers it, and its mana.
const SYMBOLS: [(&str, u16, i32); 6] = [
    ("dark eye", 717, 17),
    ("hazy black", 704, 4),
    ("slate grey", 705, 5),
    ("blood-drop", 701, 1),
    ("radiating circle", 702, 2),
    ("dark cloud", 703, 3),
];
const MAX_CASTS: u32 = 10;

#[derive(Default)]
pub(super) struct ThreePillars {
    /// Which pillar is being worked.
    pillar: usize,
    looked: bool,
    cast: Option<Casting>,
    casts: u32,
}

impl ThreePillars {
    fn on(&mut self) {
        self.pillar += 1;
        self.looked = false;
        self.cast = None;
        self.casts = 0;
    }
}

impl Solver for ThreePillars {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        loop {
            let Some(pillar) = PILLARS.get(self.pillar) else {
                return Next::Done;
            };
            if !std::mem::replace(&mut self.looked, true) {
                return Next::Put(format!("look {pillar}"));
            }
            if self.cast.is_none() {
                let shown = SYMBOLS.iter().find(|(symbol, ..)| seen.answered(symbol));
                let Some((_, number, mana)) = shown else {
                    self.on();
                    continue;
                };
                self.cast = Some(Casting::new(*number, pillar, Pay::ManaOver(mana * 3)));
            }
            let Some(cast) = self.cast.as_mut() else {
                return Next::Failed;
            };
            match cast.turn(seen) {
                Turn::Ask(next) => return next,
                Turn::Landed if seen.answered("Cast Roundtime") => self.on(),
                Turn::Landed | Turn::Hindered => {
                    self.casts += 1;
                    if self.casts >= MAX_CASTS {
                        return Next::Failed;
                    }
                }
                Turn::NoMana => return cast.no_mana(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::casting::MANA_WAIT_MS;
    use super::super::casting::tests::set_mana;
    use super::super::testing::Scene;
    use super::*;

    const LANDED: &[&str] = &["Cast Roundtime 3 Seconds."];

    fn scene() -> Scene {
        Scene::at(1, 2)
    }

    #[test]
    fn each_pillar_is_looked_at_and_answered_with_its_spell() {
        let mut pillars = ThreePillars::default();
        let shows = [
            ("deep pillar", "a dark eye", 717),
            ("black pillar", "a slate grey crescent", 705),
            ("dark pillar", "a dark cloud", 703),
        ];
        let mut before = scene();
        for (pillar, symbol, number) in shows {
            assert_eq!(
                before.ask(&mut pillars),
                Next::Put(format!("look {pillar}"))
            );
            assert_eq!(
                scene().answered(&[symbol]).ask(&mut pillars),
                Next::Put(format!("prepare {number}"))
            );
            assert_eq!(
                scene().ask(&mut pillars),
                Next::Put(format!("cast {pillar}"))
            );
            before = scene().answered(LANDED);
        }
        assert_eq!(before.ask(&mut pillars), Next::Done);
    }

    #[test]
    fn every_symbol_names_its_own_spell() {
        for (symbol, number, _) in SYMBOLS {
            let mut pillars = ThreePillars::default();
            scene().ask(&mut pillars);
            assert_eq!(
                scene().answered(&[symbol]).ask(&mut pillars),
                Next::Put(format!("prepare {number}")),
                "{symbol}"
            );
        }
    }

    #[test]
    fn a_pillar_with_no_known_symbol_is_passed_over() {
        let mut pillars = ThreePillars::default();
        scene().ask(&mut pillars);
        assert_eq!(
            scene()
                .answered(&["The symbols consist of nothing much."])
                .ask(&mut pillars),
            Next::Put("look black pillar".into())
        );
    }

    #[test]
    fn three_times_the_mana_is_waited_for() {
        let mut pillars = ThreePillars::default();
        scene().ask(&mut pillars);
        let mut poor = scene().answered(&["a hazy black orb"]);
        set_mana(&mut poor, 12);
        assert_eq!(poor.ask(&mut pillars), Next::Pause(MANA_WAIT_MS));
        let mut rich = scene();
        set_mana(&mut rich, 13);
        assert_eq!(rich.ask(&mut pillars), Next::Put("prepare 704".into()));
    }

    #[test]
    fn a_cast_that_does_not_land_is_made_again_and_not_for_ever() {
        let mut pillars = ThreePillars::default();
        scene().ask(&mut pillars);
        let mut next = scene().answered(&["a blood-drop"]).ask(&mut pillars);
        let mut casts = 0;
        while next != Next::Failed {
            assert!(casts <= MAX_CASTS, "unbounded");
            assert_eq!(next, Next::Put("prepare 701".into()));
            assert_eq!(
                scene().ask(&mut pillars),
                Next::Put("cast deep pillar".into())
            );
            casts += 1;
            next = scene()
                .answered(&["You don't have a spell prepared!"])
                .ask(&mut pillars);
        }
        assert_eq!(casts, MAX_CASTS);
    }
}

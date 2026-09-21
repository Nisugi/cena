//! `Puzzle::CrownDoor`: the stone crown and its door (room 2677).
//!
//! Upstream (`upstream_scripts/crown_door.rb`): `go door`, and if that works
//! there is nothing to solve. Otherwise `push tine` until the ring stops "with
//! the tine set with the" one of six named stones, or the tine "won't even
//! budge" -- which means the door is already worked, and only `go door` is
//! left. With a stone set and a hundred mana in hand, a hundred mana is cast
//! at the crown: the dearest known spell that does not overshoot, else the
//! cheapest that does; or, under Rapid Fire (515), any known five-mana spell
//! again and again. A cast that is hindered is cast again. Then `release` what
//! Rapid Fire left prepared, `touch crown`, `say Aenatumgana`, half a second,
//! and `go door`.
//!
//! # Where this differs, and why
//!
//! - **The cast is `incant <number> crown`.** Upstream's `Spell#cast('crown')`
//!   is Lich choosing between `prepare`/`cast` and `incant`; [`Action::Cast`]
//!   has no target, and `CastAt` expects to be carried off. UNVERIFIED live.
//! - A spell's cost is the spell table's `mana`, else its number's last two
//!   digits -- which is what upstream sorts by.
//! - Upstream's sort leaves spells of equal rank in no stated order; here the
//!   higher number goes first, so a replay casts the same.
//! - The model does not say what is prepared, so `release` is sent whenever
//!   Rapid Fire is up, where upstream also asks `checkprep`.
//! - Upstream pushes, and recasts a hindered spell, without end.
//!   [`MAX_TURNS`] bounds the pushes and [`MAX_CASTS`] the casts.
//!
//! [`Action::Cast`]: cena_map::Action::Cast

use cena_session::VitalsExt;
use cena_session::spells::{self, Spell};

use super::{MAX_TURNS, Next, Seen, Solver};

const SET: &str = "finally stopping with the tine set with the";
const STONES: [&str; 6] = [
    "reflective glass stone aligned with the word 'Honor'",
    "clear crystal stone aligned with the word 'Truth'",
    "milky white stone aligned with the word 'Piety'",
    "dull grey stone aligned with the word 'Humility'",
    "flawless silver stone aligned with the word 'Faith'",
    "veil iron stone aligned with the word 'Courage'",
];
const STUCK: &str = "You grasp the top tine and try to turn it, but it won't even budge.";
const HINDERED: &str = "[Spell Hindrance for";
const CASTABLES: [u16; 25] = [
    110, 120, 140, 210, 216, 230, 420, 425, 430, 520, 540, 620, 625, 650, 1040, 1110, 1115, 1601,
    1602, 1603, 1604, 1607, 1613, 1614, 1615,
];
const MANA_5: [u16; 12] = [
    105, 205, 305, 405, 505, 605, 705, 905, 1005, 1105, 1205, 1605,
];
const RAPID_FIRE: u16 = 515;
const WANTED: i32 = 100;
/// Casts, hindered ones among them: a hundred mana five at a time, thrice.
const MAX_CASTS: u32 = 60;

#[derive(Default)]
pub(super) struct CrownDoor {
    at: At,
    pushes: u32,
    casts: u32,
    /// Mana still owed to the crown, and the cost of the cast just made.
    needed: i32,
    last: i32,
}

#[derive(Default)]
enum At {
    #[default]
    Start,
    Tried,
    Pushing,
    Casting,
    Finish(usize),
    Gone,
}

fn cost(spell: &Spell) -> i32 {
    spell.mana.map_or(i32::from(spell.number % 100), i32::from)
}

fn known(seen: &Seen<'_>, numbers: &[u16]) -> Vec<&'static Spell> {
    let mut all: Vec<&Spell> = numbers
        .iter()
        .filter_map(|number| spells::spell(*number))
        .filter(|spell| {
            seen.walker
                .known_spells
                .as_ref()
                .is_some_and(|known| known.contains(&spell.name))
        })
        .collect();
    all.sort_by_key(|spell| std::cmp::Reverse((spell.number % 100, spell.number)));
    all
}

fn rapid_fire(seen: &Seen<'_>) -> bool {
    spells::spell(RAPID_FIRE).is_some_and(|spell| {
        seen.walker
            .active_spells
            .as_ref()
            .is_some_and(|active| active.contains(&spell.name))
    })
}

impl CrownDoor {
    fn push(&mut self, seen: &Seen<'_>) -> Next {
        if seen.answered(STUCK) {
            return self.go();
        }
        if seen.answered(SET) && STONES.iter().any(|stone| seen.answered(stone)) {
            let mana = seen.state.vitals.mana().and_then(|mana| mana.current);
            self.needed = if mana.is_some_and(|mana| mana >= WANTED) {
                WANTED
            } else {
                0
            };
            self.at = At::Casting;
            return self.cast(seen);
        }
        if self.pushes >= MAX_TURNS {
            return Next::Failed;
        }
        self.pushes += 1;
        Next::Put("push tine".to_owned())
    }

    fn cast(&mut self, seen: &Seen<'_>) -> Next {
        if !seen.answered(HINDERED) {
            self.needed -= std::mem::take(&mut self.last);
        }
        let spell = if self.needed <= 0 || self.casts >= MAX_CASTS {
            None
        } else if let (true, Some(small)) = (rapid_fire(seen), known(seen, &MANA_5).last()) {
            // Upstream takes the first listed, which is the lowest numbered.
            Some(*small)
        } else {
            let castables = known(seen, &CASTABLES);
            let under = castables.iter().find(|spell| cost(spell) <= self.needed);
            let over = castables
                .iter()
                .rev()
                .find(|spell| cost(spell) >= self.needed);
            under.or(over).copied()
        };
        let Some(spell) = spell else {
            self.at = At::Finish(0);
            return self.finish(seen);
        };
        self.casts += 1;
        self.last = cost(spell);
        Next::Put(format!("incant {} crown", spell.number))
    }

    fn finish(&mut self, seen: &Seen<'_>) -> Next {
        let At::Finish(done) = self.at else {
            return Next::Failed;
        };
        self.at = At::Finish(done + 1);
        match done {
            0 if rapid_fire(seen) => Next::Put("release".to_owned()),
            0 => self.finish(seen),
            1 => Next::Put("touch crown".to_owned()),
            2 => Next::Put("say Aenatumgana".to_owned()),
            3 => Next::Pause(500),
            _ => self.go(),
        }
    }

    fn go(&mut self) -> Next {
        self.at = At::Gone;
        Next::Go("go door".to_owned())
    }
}

impl Solver for CrownDoor {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match self.at {
            At::Start => {
                self.at = At::Tried;
                Next::Go("go door".to_owned())
            }
            At::Tried if seen.ok => Next::Done,
            At::Tried => {
                self.at = At::Pushing;
                self.push(seen)
            }
            At::Pushing => self.push(seen),
            At::Casting => self.cast(seen),
            At::Finish(_) => self.finish(seen),
            At::Gone => Next::Done,
        }
    }
}

#[cfg(test)]
mod tests {
    use cena_session::Vital;

    use super::super::testing::Scene;
    use super::*;

    fn name(number: u16) -> String {
        spells::spell(number).unwrap().name.clone()
    }

    fn aligned() -> String {
        format!(
            "Slowly, the stone ring surrounding the crown rotates, {SET} {}.",
            STONES[2]
        )
    }

    fn scene(mana: i32, knows: &[u16], line: &str) -> Scene {
        let mut scene = Scene::at(1, 2).answered(&[line]);
        scene.state.vitals.insert(
            "mana".into(),
            Vital {
                percent: 0,
                current: Some(mana),
                max: Some(200),
            },
        );
        scene.walker.known_spells = Some(knows.iter().map(|number| name(*number)).collect());
        scene
    }

    /// A door that would not open, and a tine pushed once.
    fn pushed() -> CrownDoor {
        let mut door = CrownDoor::default();
        assert_eq!(Scene::at(1, 2).ask(&mut door), Next::Go("go door".into()));
        let mut shut = Scene::at(1, 2);
        shut.failed = true;
        assert_eq!(shut.ask(&mut door), Next::Put("push tine".into()));
        door
    }

    fn ending() -> Vec<Next> {
        vec![
            Next::Put("touch crown".into()),
            Next::Put("say Aenatumgana".into()),
            Next::Pause(500),
            Next::Go("go door".into()),
            Next::Done,
        ]
    }

    #[test]
    fn a_door_that_opens_needs_nothing() {
        let mut door = CrownDoor::default();
        Scene::at(1, 2).ask(&mut door);
        assert_eq!(Scene::at(2, 2).ask(&mut door), Next::Done);
    }

    #[test]
    fn the_tine_is_pushed_until_a_named_stone_is_set() {
        let mut door = pushed();
        let between = format!("Slowly, the stone ring rotates, {SET} plain stone.");
        assert_eq!(
            scene(0, &[], &between).ask(&mut door),
            Next::Put("push tine".into())
        );
        // No mana: nothing is cast, and the crown is touched all the same.
        let sent: Vec<Next> = (0..5)
            .map(|_| scene(0, &[], &aligned()).ask(&mut door))
            .collect();
        assert_eq!(sent, ending());
    }

    #[test]
    fn a_tine_that_will_not_budge_leaves_only_the_door() {
        let mut door = pushed();
        assert_eq!(
            scene(200, &[140], STUCK).ask(&mut door),
            Next::Go("go door".into())
        );
        assert_eq!(Scene::at(2, 2).ask(&mut door), Next::Done);
    }

    #[test]
    fn a_hundred_mana_is_spent_dearest_first_without_overshooting() {
        let mut door = pushed();
        // 140 (40) twice is 80; 40 would overshoot 20, so 120 (20) ends it.
        let knows = [110, 120, 140];
        let mut sent = vec![scene(150, &knows, &aligned()).ask(&mut door)];
        sent.extend((0..3).map(|_| scene(150, &knows, "You gesture.").ask(&mut door)));
        assert_eq!(
            sent,
            [
                Next::Put("incant 140 crown".into()),
                Next::Put("incant 140 crown".into()),
                Next::Put("incant 120 crown".into()),
                Next::Put("touch crown".into()),
            ]
        );
    }

    #[test]
    fn the_cheapest_that_overshoots_is_cast_when_nothing_fits() {
        let mut door = pushed();
        let knows = [140, 650];
        let mut sent = vec![scene(150, &knows, &aligned()).ask(&mut door)];
        sent.extend((0..3).map(|_| scene(150, &knows, "You gesture.").ask(&mut door)));
        // 50, 50 is the hundred; with 50 and 40 known nothing is left over.
        assert_eq!(
            sent,
            [
                Next::Put("incant 650 crown".into()),
                Next::Put("incant 650 crown".into()),
                Next::Put("touch crown".into()),
                Next::Put("say Aenatumgana".into()),
            ]
        );
        let mut door = pushed();
        let mut sent = vec![scene(150, &[140], &aligned()).ask(&mut door)];
        sent.extend((0..3).map(|_| scene(150, &[140], "You gesture.").ask(&mut door)));
        // 40, 40, and then 20 are owed: only 40 is known, and it overshoots.
        assert_eq!(sent[2], Next::Put("incant 140 crown".into()));
        assert_eq!(sent[3], Next::Put("touch crown".into()));
    }

    #[test]
    fn a_hindered_cast_is_cast_again_and_not_counted() {
        let mut door = pushed();
        let cast = Next::Put("incant 650 crown".into());
        assert_eq!(scene(150, &[650], &aligned()).ask(&mut door), cast);
        let hindered = "[Spell Hindrance for a hauberk is 12% with current Armor Use skill]";
        assert_eq!(scene(150, &[650], hindered).ask(&mut door), cast);
        assert_eq!(scene(150, &[650], "You gesture.").ask(&mut door), cast);
        assert_eq!(
            scene(150, &[650], "You gesture.").ask(&mut door),
            Next::Put("touch crown".into())
        );
    }

    #[test]
    fn under_rapid_fire_five_mana_spells_are_cast_and_released() {
        let mut door = pushed();
        let rapid = |line: &str| {
            let mut scene = scene(150, &[140, 505, 1605], line);
            scene.walker.active_spells = Some([name(RAPID_FIRE)].into());
            scene
        };
        let cast = Next::Put("incant 505 crown".into());
        assert_eq!(rapid(&aligned()).ask(&mut door), cast);
        let casts = (0..19)
            .filter(|_| rapid("You gesture.").ask(&mut door) == cast)
            .count();
        assert_eq!(casts, 19);
        assert_eq!(
            rapid("You gesture.").ask(&mut door),
            Next::Put("release".into())
        );
        assert_eq!(rapid("").ask(&mut door), Next::Put("touch crown".into()));
    }

    #[test]
    fn the_pushes_and_the_casts_are_bounded() {
        let mut door = pushed();
        let pushes = (0..MAX_TURNS + 5)
            .filter(|_| Scene::at(1, 2).ask(&mut door) == Next::Put("push tine".into()))
            .count();
        assert_eq!(pushes, MAX_TURNS as usize - 1);
        assert_eq!(Scene::at(1, 2).ask(&mut door), Next::Failed);

        let mut door = pushed();
        let hindered = format!("{} {HINDERED} a hauberk", aligned());
        let casts = (0..MAX_CASTS + 5)
            .filter(|_| {
                scene(150, &[650], &hindered).ask(&mut door) == Next::Put("incant 650 crown".into())
            })
            .count();
        assert_eq!(casts, MAX_CASTS as usize);
    }
}

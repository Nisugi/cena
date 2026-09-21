//! Casting a spell at a thing **and staying put**, which six routines do.
//!
//! Upstream is `Spell[n].cast(target)` behind `wait_until { affordable? }`,
//! and recast on `[Spell Hindrance`. `Action::CastAt` cannot say it: as a step
//! it is followed by the wait to be carried elsewhere (it was built for
//! Phase), and a pillar carries nobody. So this asks for what the driver's
//! own cast sends (`hands::cast_commands`): `prepare <n>` then `cast
//! <target>`, as two [`Next::Put`]s, and the caller reads the second's answer.
//!
//! Upstream waits for mana for ever. This waits [`MANA_WAITS`] times
//! [`MANA_WAIT_MS`] and then gives the caller [`Turn::NoMana`].
//!
//! A gauge or a spell list the game has not stated yet (`None`) counts as
//! *not known* and as *affordable*: the first keeps a walker from preparing a
//! spell it may not have, the second costs one failed `prepare` at worst.

use cena_session::{VitalsExt, spells};

use super::{Next, Seen};

/// How long one wait for mana is, and how many there are: ten minutes.
pub(super) const MANA_WAIT_MS: u64 = 5000;
pub(super) const MANA_WAITS: u32 = 120;
/// How often a hindered cast is made again.
pub(super) const MAX_HINDERED: u32 = 20;

/// The spell's name in the table, which is how a [`Seen`] lists spells.
pub(super) fn name_of(number: u16) -> Option<&'static str> {
    spells::spell(number).map(|spell| spell.name.as_str())
}

fn listed(list: Option<&std::collections::HashSet<String>>, number: u16) -> Option<bool> {
    Some(list?.contains(name_of(number)?))
}

/// `Spell[n].known?`
pub(super) fn knows(seen: &Seen<'_>, number: u16) -> bool {
    listed(seen.walker.known_spells.as_ref(), number).unwrap_or(false)
}

/// `Spell[n].affordable?`
pub(super) fn affords(seen: &Seen<'_>, number: u16) -> bool {
    listed(seen.walker.affordable_spells.as_ref(), number).unwrap_or(true)
}

/// `Spell[n].active?`
pub(super) fn active(seen: &Seen<'_>, number: u16) -> bool {
    listed(seen.walker.active_spells.as_ref(), number).unwrap_or(false)
}

/// What must be true before the spell is prepared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pay {
    /// `Spell[n].affordable?`
    Affordable,
    /// `XMLData.mana > n`
    ManaOver(i32),
}

/// What one turn of a cast comes to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Turn {
    /// Ask the driver for this, and come back.
    Ask(Next),
    /// It was cast, and `seen.answer` is what the game said. Asking again
    /// casts again.
    Landed,
    /// The mana never came.
    NoMana,
    /// Armour hindered it [`MAX_HINDERED`] times.
    Hindered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    Ready,
    Prepared,
    Cast,
}

/// One spell at one thing, part-way through.
#[derive(Debug)]
pub(super) struct Casting {
    number: u16,
    target: String,
    pay: Pay,
    stage: Stage,
    waits: u32,
    hindered: u32,
}

impl Casting {
    pub fn new(number: u16, target: &str, pay: Pay) -> Self {
        Casting {
            number,
            target: target.to_owned(),
            pay,
            stage: Stage::Ready,
            waits: 0,
            hindered: 0,
        }
    }

    fn can_pay(&self, seen: &Seen<'_>) -> bool {
        match self.pay {
            Pay::Affordable => affords(seen, self.number),
            Pay::ManaOver(least) => seen
                .state
                .vitals
                .mana()
                .and_then(|mana| mana.current)
                .is_none_or(|mana| mana > least),
        }
    }

    /// What a routine tells the player when the mana never came.
    pub fn no_mana(&self) -> Next {
        Next::Stop(format!(
            "Waited ten minutes for the mana to cast {} and it never came.",
            self.number
        ))
    }

    pub fn turn(&mut self, seen: &Seen<'_>) -> Turn {
        if self.stage == Stage::Cast {
            self.stage = Stage::Ready;
            if !seen.answered("[Spell Hindrance") {
                return Turn::Landed;
            }
            self.hindered += 1;
            if self.hindered >= MAX_HINDERED {
                return Turn::Hindered;
            }
        }
        if self.stage == Stage::Prepared {
            self.stage = Stage::Cast;
            return Turn::Ask(Next::Put(format!("cast {}", self.target)));
        }
        if !self.can_pay(seen) {
            self.waits += 1;
            if self.waits > MANA_WAITS {
                return Turn::NoMana;
            }
            return Turn::Ask(Next::Pause(MANA_WAIT_MS));
        }
        self.stage = Stage::Prepared;
        Turn::Ask(Next::Put(format!("prepare {}", self.number)))
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::testing::Scene;
    use super::*;

    /// A scene whose walker knows these spells, by number.
    pub fn knowing(mut scene: Scene, numbers: &[u16]) -> Scene {
        let names = numbers
            .iter()
            .map(|number| name_of(*number).unwrap().to_owned())
            .collect();
        scene.walker.known_spells = Some(names);
        scene
    }

    /// A scene whose walker can pay for only these.
    pub fn affording(mut scene: Scene, numbers: &[u16]) -> Scene {
        let names = numbers
            .iter()
            .map(|number| name_of(*number).unwrap().to_owned())
            .collect();
        scene.walker.affordable_spells = Some(names);
        scene
    }

    fn ask(cast: &mut Casting, scene: &Scene) -> Turn {
        struct Probe<'a>(&'a mut Casting, Option<Turn>);
        impl super::super::Solver for Probe<'_> {
            fn next(&mut self, seen: &Seen<'_>) -> Next {
                self.1 = Some(self.0.turn(seen));
                Next::Done
            }
        }
        let mut probe = Probe(cast, None);
        scene.ask(&mut probe);
        probe.1.unwrap()
    }

    #[test]
    fn it_prepares_casts_and_lands() {
        let mut cast = Casting::new(407, "gate", Pay::Affordable);
        let scene = Scene::at(1, 2);
        assert_eq!(
            ask(&mut cast, &scene),
            Turn::Ask(Next::Put("prepare 407".into()))
        );
        assert_eq!(
            ask(&mut cast, &scene),
            Turn::Ask(Next::Put("cast gate".into()))
        );
        assert_eq!(ask(&mut cast, &scene), Turn::Landed);
    }

    #[test]
    fn a_hindered_cast_is_made_again_and_not_for_ever() {
        let mut cast = Casting::new(407, "gate", Pay::Affordable);
        let plain = Scene::at(1, 2);
        let hindered = Scene::at(1, 2).answered(&["[Spell Hindrance for a jerkin is 4%]"]);
        ask(&mut cast, &plain);
        for _ in 1..MAX_HINDERED {
            ask(&mut cast, &plain);
            assert_eq!(
                ask(&mut cast, &hindered),
                Turn::Ask(Next::Put("prepare 407".into()))
            );
        }
        ask(&mut cast, &plain);
        assert_eq!(ask(&mut cast, &hindered), Turn::Hindered);
    }

    #[test]
    fn mana_is_waited_for_and_not_for_ever() {
        let mut cast = Casting::new(407, "gate", Pay::Affordable);
        let poor = affording(Scene::at(1, 2), &[]);
        for _ in 0..MANA_WAITS {
            assert_eq!(ask(&mut cast, &poor), Turn::Ask(Next::Pause(MANA_WAIT_MS)));
        }
        assert_eq!(ask(&mut cast, &poor), Turn::NoMana);
    }

    #[test]
    fn a_mana_floor_is_strictly_over() {
        let mut cast = Casting::new(717, "deep pillar", Pay::ManaOver(51));
        let mut scene = Scene::at(1, 2);
        set_mana(&mut scene, 51);
        assert_eq!(ask(&mut cast, &scene), Turn::Ask(Next::Pause(MANA_WAIT_MS)));
        set_mana(&mut scene, 52);
        assert_eq!(
            ask(&mut cast, &scene),
            Turn::Ask(Next::Put("prepare 717".into()))
        );
    }

    pub fn set_mana(scene: &mut Scene, mana: i32) {
        scene.state.vitals.insert(
            "mana".into(),
            cena_session::Vital {
                percent: 0,
                current: Some(mana),
                max: Some(100),
            },
        );
    }
}

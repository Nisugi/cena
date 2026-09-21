//! Cost arms that price an exit by the walker's skill against its load, and
//! the one wall with several prices (`Cost::Ladder`).

use cena_map::{Cond, Cost, Rung};

use super::costs::gated;
use super::holes;

pub(super) fn cost(script: &str) -> Option<Cost> {
    graveyard_wall(script).or_else(|| climb_under_load(script))
}

/// `Skills.climbing >= [XMLData.encumbrance_value/1.25,12].max`: at least
/// twelve ranks, and enough for the load.
const CLIMBS: &str = "Skills.climbing >= [XMLData.encumbrance_value/1.25,12].max";

fn climbs() -> Cond {
    Cond::All(vec![
        Cond::Not(Box::new(Cond::SkillUnder("climbing".to_owned(), 12))),
        Cond::SkillCarriesLoad("climbing".to_owned(), 4, 5),
    ])
}

fn seconds(text: &str) -> Option<f64> {
    text.parse()
        .ok()
        .filter(|s: &f64| s.is_finite() && *s >= 0.0)
}

/// The Graveyard wall, 4140 and 4141. Whoever knows a spell that opens the
/// gate pays least; a Warrior of 15 can batter it; anyone can wait for it.
/// Going in, a good climber goes over instead -- and upstream asks that
/// *before* it asks about the Warrior, dearer though it is. Ported as written.
fn graveyard_wall(script: &str) -> Option<Cost> {
    let found = holes(
        script,
        &[
            ";e if [407, 1604, 304, 1207].any? { |num| Spell[num].known? }; ",
            "; elsif ",
            "(Stats.prof == 'Warrior') and (Stats.level >= 15); ",
            "; else ",
            "; end",
            "",
        ],
    )?;
    let [opened, climb, battered, waited, "" | ";"] = found[..] else {
        return None;
    };
    let opens = Cond::Any(
        ["Unlock", "Consecrate", "Bless Item", "Force Projection"]
            .map(|spell| Cond::SpellKnown(spell.to_owned()))
            .to_vec(),
    );
    let mut ladder = vec![Rung {
        when: opens,
        then: seconds(opened)?,
    }];
    if !climb.is_empty() {
        let [over] = holes(climb, &[&format!("{CLIMBS}; "), "; elsif "])?[..] else {
            return None;
        };
        ladder.push(Rung {
            when: climbs(),
            then: seconds(over)?,
        });
    }
    ladder.push(Rung {
        when: Cond::All(vec![
            Cond::Profession("Warrior".to_owned()),
            Cond::LevelAtLeast(15),
        ]),
        then: seconds(battered)?,
    });
    Some(Cost::Ladder {
        ladder,
        otherwise: Some(seconds(waited)?),
    })
}

/// A climb only a good enough climber may plan on. 1 exit.
fn climb_under_load(script: &str) -> Option<Cost> {
    let [then] = holes(script, &[&format!(";e if {CLIMBS}; "), "; else; nil; end"])?[..] else {
        return None;
    };
    gated(climbs(), then)
}

#[cfg(test)]
mod tests {
    use cena_map::Walker;

    use super::*;

    const IN: &str = ";e if [407, 1604, 304, 1207].any? { |num| Spell[num].known? }; 0.6; elsif \
                      Skills.climbing >= [XMLData.encumbrance_value/1.25,12].max; 120.0; elsif \
                      (Stats.prof == 'Warrior') and (Stats.level >= 15); 5.2; else 30.0; end;";

    #[test]
    fn nobody_is_refused_and_what_is_known_earns_its_price() {
        let wall = cost(IN).unwrap();
        assert_eq!(wall.price(&Walker::default()), Some(30.0));
        let warrior = Walker {
            profession: Some("Warrior".into()),
            level: Some(20),
            ..Walker::default()
        };
        assert_eq!(wall.price(&warrior), Some(5.2));
        // Upstream asks about the climb first, dearer though it is.
        let climber = Walker {
            skills: Some([("climbing".to_owned(), 50)].into()),
            encumbrance: Some(40),
            ..warrior.clone()
        };
        assert_eq!(wall.price(&climber), Some(120.0));
        let laden = Walker {
            encumbrance: Some(80),
            ..climber
        };
        assert_eq!(wall.price(&laden), Some(5.2), "50 ranks carry 62 percent");
        let cleric = Walker {
            known_spells: Some(["Bless Item".to_owned()].into()),
            ..laden
        };
        assert_eq!(wall.price(&cleric), Some(0.6));
    }

    #[test]
    fn the_way_out_has_no_climb() {
        let out = IN
            .replace(
                "Skills.climbing >= [XMLData.encumbrance_value/1.25,12].max; 120.0; elsif ",
                "",
            )
            .replace("end;", "end");
        let Some(Cost::Ladder { ladder, .. }) = cost(&out) else {
            panic!("a ladder");
        };
        assert_eq!(ladder.len(), 2);
    }

    #[test]
    fn a_climb_alone_is_a_gate() {
        let script = ";e if Skills.climbing >= [XMLData.encumbrance_value/1.25,12].max; 3.0; \
                      else; nil; end";
        let gate = cost(script).unwrap();
        assert_eq!(gate.price(&Walker::default()), None);
        let climber = Walker {
            skills: Some([("climbing".to_owned(), 12)].into()),
            encumbrance: Some(15),
            ..Walker::default()
        };
        assert_eq!(gate.price(&climber), Some(3.0));
    }
}

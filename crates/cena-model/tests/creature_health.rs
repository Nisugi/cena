//! `<crtrStatus health= maxhealth=>`: the game's own answer to a question
//! Cena was inferring.
//!
//! Both attributes fell into `CreatureStatus::unknown` -- the map that means
//! "the game added a flag since this table was cut". Meanwhile the registry
//! estimated the same number from a bestiary template and a damage tally,
//! which is what a combat tracker had to do before the game reported hit
//! points at all.
//!
//! MEASURED over the author's September logs: health appears in 2 files of
//! 127, both 2026-09-19 or later, and in **zero** older ones -- including a
//! session carrying 11,414 `<crtrStatus>` tags and none. The same creature
//! appears 236 times without it and 7 times with, so it is a protocol
//! feature that turned on, not a property of particular creatures.

// `Classification` is deliberately not re-exported from the crate root: it
// collides with `gameobj::Classification` (`lib.rs:49-50`).
use cena_model::CreatureStatus;
use cena_model::state::creature::status::{Classification, Status};

/// Real rows, verbatim from the author's 2026-09-19 and 2026-09-20 logs.
mod wire {
    /// A hostile creature at full health.
    pub const HEALTHY: &[(&str, &str)] = &[
        ("health", "360"),
        ("maxhealth", "360"),
        ("hostile", "1"),
        ("ascended", "1"),
    ];
    /// The same shape, nearly dead: `health="30" maxhealth="600"`.
    pub const WOUNDED: &[(&str, &str)] = &[
        ("health", "30"),
        ("maxhealth", "600"),
        ("hostile", "1"),
        ("ascended", "1"),
    ];
    /// A DEAD creature: health goes negative. Verbatim from the log.
    pub const DEAD: &[(&str, &str)] = &[
        ("health", "-10"),
        ("maxhealth", "360"),
        ("hostile", "1"),
        ("dead", "1"),
        ("prone", "1"),
    ];
    /// A non-hostile NPC with no HP model at all.
    pub const NO_MODEL: &[(&str, &str)] = &[("health", "0"), ("maxhealth", "0"), ("flying", "1")];
    /// The jeweler Etaenia: non-hostile, and yet she HAS hit points.
    pub const SHOPKEEPER: &[(&str, &str)] =
        &[("health", "240"), ("maxhealth", "240"), ("inferior", "1")];
}

fn status(attrs: &[(&str, &str)]) -> CreatureStatus {
    CreatureStatus::from_attrs("-285052", attrs.iter().copied())
}

#[test]
fn hit_points_are_typed_rather_than_unknown() {
    // The defect. `health` and `maxhealth` landed in the bag meaning "we do
    // not recognise this flag", which is where a real-time health feed is
    // least likely to be noticed.
    let s = status(wire::WOUNDED);
    assert_eq!(s.health, Some(30));
    assert_eq!(s.max_health, Some(600));
    assert!(
        s.unknown.is_empty(),
        "nothing left in the unrecognised bag: {:?}",
        s.unknown
    );
}

#[test]
fn hit_points_are_absolute_not_a_percentage() {
    let s = status(wire::WOUNDED);
    assert_eq!(s.health_percent(), Some(5), "30 of 600 is 5%");
    assert_eq!(status(wire::HEALTHY).health_percent(), Some(100));
}

#[test]
fn a_health_of_zero_is_a_value_not_an_absent_flag() {
    // The ordering hazard: `<crtrStatus>` treats `x="0"` as "flag off", and
    // applying that rule to `health="0"` would read a dead creature as
    // "the health flag is not set" and drop the number.
    let s = status(wire::NO_MODEL);
    assert_eq!(s.health, Some(0), "stated, and zero");
    assert_eq!(s.max_health, Some(0));
    assert!(s.is(Status::Flying), "flags still parse");
}

#[test]
fn a_dead_creature_has_negative_health() {
    // **The field must be SIGNED.** An unsigned version failed to parse this
    // and silently reported `None`, so a creature flagged `dead` carried no
    // health information at all -- the opposite of what the row states.
    //
    // Found by driving a real log: the unsigned code reported 327 of 331
    // hostile rows carrying health where raw `grep` counted 331. The four it
    // lost were the dead ones.
    let s = status(wire::DEAD);
    assert_eq!(s.health, Some(-10));
    assert_eq!(s.max_health, Some(360));
    assert!(s.unknown.is_empty(), "not dropped: {:?}", s.unknown);
    assert!(s.is_classified(Classification::Dead));
}

#[test]
fn negative_health_reports_zero_percent_rather_than_underflowing() {
    assert_eq!(status(wire::DEAD).health_percent(), Some(0));
}

#[test]
fn a_creature_with_no_hp_model_has_no_percentage() {
    // 42 such rows in the author's logs, every one non-hostile. Reporting
    // 0% for a shopkeeper would read as "nearly dead" to a target-picker,
    // and dividing by zero would be worse.
    assert_eq!(status(wire::NO_MODEL).health_percent(), None);
}

#[test]
fn a_maxhealth_of_zero_does_not_mean_not_hostile() {
    // The author's reading holds one way -- all 42 zero-max rows are
    // non-hostile -- but it does NOT invert. The jeweler Etaenia is not
    // hostile and carries a real 240/240, so "no HP model" and "not a
    // combatant" are different facts and only the first is what 0 states.
    let jeweler = status(wire::SHOPKEEPER);
    assert!(!jeweler.is_classified(Classification::Hostile));
    assert_eq!(jeweler.health_percent(), Some(100));
}

#[test]
fn unstated_hit_points_stay_none() {
    // §5.2: most of the corpus predates this feature entirely, so "the
    // server did not say" has to survive as its own answer.
    let s = status(&[("hostile", "1"), ("stunned", "1")]);
    assert_eq!((s.health, s.max_health), (None, None));
    assert_eq!(s.health_percent(), None);
}

/// The registry: the server's number outranks the estimate, and the tally we
/// keep becomes a check on the combat parser instead.
mod registry {
    use super::*;
    use cena_model::CreatureInstance;

    fn creature() -> CreatureInstance {
        CreatureInstance::new(-285_052, None, "an ashen orc", None)
    }

    #[test]
    fn stated_hit_points_outrank_the_bestiary_estimate() {
        let mut c = creature();
        let inferred_max = c.max_hp();
        c.sync_crtr_status(&status(wire::WOUNDED), None);
        assert_eq!(c.max_hp(), 600, "the server's number, not the template's");
        assert_eq!(c.current_hp(), 30);
        assert!(c.hp_is_stated());
        assert_ne!(
            inferred_max, 600,
            "guard: the template disagreed, so this proves the override"
        );
    }

    #[test]
    fn an_unreported_creature_still_uses_the_tally() {
        // The inference is not removed -- it is right for every creature the
        // server says nothing about, which is most of them before 2026-09-19.
        let mut c = creature();
        c.add_damage(50);
        assert!(!c.hp_is_stated());
        assert_eq!(c.current_hp(), c.max_hp() - 50);
    }

    #[test]
    fn a_creature_with_no_hp_model_falls_back_to_the_template() {
        // `maxhealth="0"` is not an answer, so it must not become the max --
        // dividing by it, or reporting a shopkeeper at 0 HP, are both wrong.
        let mut c = creature();
        c.sync_crtr_status(&status(wire::NO_MODEL), None);
        assert!(c.max_hp() > 0, "0 is not a usable maximum");
        assert!(!c.hp_is_stated());
    }

    #[test]
    fn hit_points_persist_across_a_frame_that_omits_them() {
        // Unlike the flags, which the tag re-sends in full every time, an
        // omitted `health=` says nothing rather than "zero".
        let mut c = creature();
        c.sync_crtr_status(&status(wire::WOUNDED), None);
        c.sync_crtr_status(&status(&[("hostile", "1")]), None);
        assert_eq!(c.stated_health(), Some(30), "the last statement stands");
        assert_eq!(c.current_hp(), 30);
    }

    #[test]
    fn agreement_between_our_tally_and_the_server_is_zero() {
        // The author's design: the tally stops being the HP source and
        // becomes a check on the combat parser.
        let mut c = creature();
        c.sync_crtr_status(&status(wire::WOUNDED), None);
        c.add_damage(570); // 600 - 30
        assert_eq!(c.damage_discrepancy(), Some(0));
    }

    #[test]
    fn missed_damage_shows_up_as_a_negative_discrepancy() {
        // The case worth surfacing: the creature lost health we never saw,
        // so the combat parser missed a line -- or another hunter landed one.
        let mut c = creature();
        c.sync_crtr_status(&status(wire::WOUNDED), None);
        c.add_damage(100);
        assert_eq!(
            c.damage_discrepancy(),
            Some(-470),
            "it lost 570 and we counted 100"
        );
    }

    #[test]
    fn a_dead_creature_reconciles_past_zero() {
        // It took 370 to bring a 360-HP creature to -10, and the check has
        // to agree: an unsigned subtraction would saturate at 360 and report
        // a phantom 10 points of overcounting on every kill.
        let mut c = creature();
        c.sync_crtr_status(&status(wire::DEAD), None);
        c.add_damage(370);
        assert_eq!(c.damage_discrepancy(), Some(0));
        assert_eq!(c.stated_health(), Some(-10), "the sign survives");
        assert_eq!(c.current_hp(), 0, "but it has no hit points left");
    }

    #[test]
    fn overcounted_damage_shows_up_as_a_positive_discrepancy() {
        let mut c = creature();
        c.sync_crtr_status(&status(wire::HEALTHY), None);
        c.add_damage(25);
        assert_eq!(
            c.damage_discrepancy(),
            Some(25),
            "we counted 25 against a creature at full health"
        );
    }

    #[test]
    fn there_is_nothing_to_reconcile_without_a_stated_hp() {
        let mut c = creature();
        c.add_damage(100);
        assert_eq!(c.damage_discrepancy(), None, "no server number to check");

        c.sync_crtr_status(&status(wire::NO_MODEL), None);
        assert_eq!(c.damage_discrepancy(), None, "and 0 max is not a number");
    }
}

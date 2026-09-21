//! `cena_map::cond`: three-valued, so "unknown" never turns into "yes".

use cena_map::{Cond, Walker};

fn hasted() -> Cond {
    Cond::SpellActive("Haste".into())
}

#[test]
fn a_question_about_a_missing_fact_has_no_answer() {
    let nobody = Walker::default();
    assert_eq!(hasted().ask(&nobody), None);
    assert_eq!(Cond::EncumbranceOver(50).ask(&nobody), None);
    assert_eq!(Cond::SkillUnder("survival".into(), 50).ask(&nobody), None);
    assert_eq!(
        Cond::Setting("ice_mode".into(), "run".into()).ask(&nobody),
        None
    );
}

/// The reason `ask` is not a `bool`: with two values, `not unknown` is `true`,
/// and a guard written as "unless hasted" would fire for a walker nobody has
/// looked at.
#[test]
fn not_unknown_is_unknown_and_neither_holds() {
    let nobody = Walker::default();
    let not_hasted = Cond::Not(Box::new(hasted()));
    assert_eq!(not_hasted.ask(&nobody), None);
    assert!(!hasted().holds(&nobody));
    assert!(!not_hasted.holds(&nobody));
}

#[test]
fn one_decisive_part_settles_the_whole_whatever_else_is_unknown() {
    let heavy = Walker {
        encumbrance: Some(80),
        ..Walker::default()
    };
    let over = Cond::EncumbranceOver(50);
    let under = Cond::Not(Box::new(Cond::EncumbranceOver(50)));
    assert_eq!(
        Cond::Any(vec![hasted(), over.clone()]).ask(&heavy),
        Some(true)
    );
    assert_eq!(
        Cond::All(vec![hasted(), under.clone()]).ask(&heavy),
        Some(false)
    );
    // Nothing decisive, something unknown: unknown.
    assert_eq!(Cond::Any(vec![hasted(), under]).ask(&heavy), None);
    assert_eq!(Cond::All(vec![hasted(), over]).ask(&heavy), None);
    // Empty: `all` of nothing holds, `any` of nothing does not.
    assert_eq!(Cond::All(vec![]).ask(&heavy), Some(true));
    assert_eq!(Cond::Any(vec![]).ask(&heavy), Some(false));
}

/// Skills are known as a table: a skill missing from a known table is zero
/// ranks, which is a different thing from the table being unknown.
#[test]
fn an_untrained_skill_is_zero_not_unknown() {
    let trained_nothing = Walker {
        skills: Some([].into()),
        ..Walker::default()
    };
    assert_eq!(
        Cond::SkillUnder("survival".into(), 50).ask(&trained_nothing),
        Some(true)
    );
}

#[test]
fn a_condition_is_plain_json() {
    let cond: Cond = serde_json::from_str(
        r#"{"all":[{"not":{"setting":["ice_mode","run"]}},{"skill_under":["survival",50]}]}"#,
    )
    .unwrap();
    assert_eq!(
        cond,
        Cond::All(vec![
            Cond::Not(Box::new(Cond::Setting("ice_mode".into(), "run".into()))),
            Cond::SkillUnder("survival".into(), 50),
        ])
    );
}

/// Leaving an event ground: only the way you came in is open, and a character
/// with no memory of coming in has no way priced at all.
#[test]
fn a_memory_opens_only_the_way_back() {
    let to_illistim = Cond::Remembered("duskruin_origin".into(), "7".into());
    let to_landing = Cond::Remembered("duskruin_origin".into(), "284".into());
    let came_from_illistim = Walker {
        memories: [("duskruin_origin".to_owned(), "7".to_owned())].into(),
        ..Walker::default()
    };
    assert!(to_illistim.holds(&came_from_illistim));
    assert!(!to_landing.holds(&came_from_illistim));
    assert_eq!(to_illistim.ask(&Walker::default()), None);
    assert!(!to_illistim.holds(&Walker::default()));
}

/// A gated cost has three outcomes, and the third is the one a `bool` loses:
/// a walker nobody has looked at is refused, not charged the `else` price.
#[test]
fn a_gated_cost_is_impassable_when_it_cannot_be_answered() {
    let cost: cena_map::Cost =
        serde_json::from_str(r#"{"when":{"profession":"Bard"},"then":0.2,"else":30}"#).unwrap();
    let walker = |profession: Option<&str>| Walker {
        profession: profession.map(str::to_owned),
        ..Walker::default()
    };
    assert_eq!(cost.price(&walker(Some("Bard"))), Some(0.2));
    assert_eq!(cost.price(&walker(Some("Rogue"))), Some(30.0));
    assert_eq!(cost.price(&walker(None)), None);

    let members_only: cena_map::Cost =
        serde_json::from_str(r#"{"when":{"profession":"Bard"},"then":0.2}"#).unwrap();
    assert_eq!(members_only.price(&walker(Some("Rogue"))), None);
    assert_eq!(cena_map::Cost::Fixed(1.5).price(&walker(None)), Some(1.5));
}

/// Two ways across, and a walker nobody has looked at: `not` leaves it with
/// neither, `otherwise` gives it the second.
#[test]
fn otherwise_is_never_unknown() {
    let walking = || Cond::SpellActive("Water Walking".into());
    let nobody = Walker::default();
    assert!(!walking().holds(&nobody));
    assert!(!Cond::Not(Box::new(walking())).holds(&nobody));
    assert!(Cond::Otherwise(Box::new(walking())).holds(&nobody));

    let up = Walker {
        active_spells: Some(["Water Walking".to_owned()].into()),
        ..Walker::default()
    };
    assert!(walking().holds(&up));
    assert!(!Cond::Otherwise(Box::new(walking())).holds(&up));
}

//! Spellcaster (`plan/37` Stage 6): the typed spell, cast as set up.

use cena_behavior::spellcaster::{CasterProfile, edit, lines, typed};
use cena_session::GameState;

#[test]
fn a_number_an_alias_a_target_and_a_count() {
    let state = GameState::default();
    let mut profile = CasterProfile::default();
    assert_eq!(
        lines(&profile, &state, &["401"]),
        Ok(vec!["incant 401".to_owned()])
    );
    assert_eq!(
        lines(&profile, &state, &["401", "3"]),
        Ok(vec!["incant 401 3".to_owned()])
    );
    assert_eq!(
        lines(&profile, &state, &["401", "bob"]),
        Ok(vec!["prepare 401".to_owned(), "cast bob".to_owned()])
    );
    assert!(edit(&mut profile, &["alias", "401", "ed"]).is_ok());
    assert_eq!(
        lines(&profile, &state, &["ed"]),
        Ok(vec!["incant 401".to_owned()])
    );
    assert!(lines(&profile, &state, &["nosuch"]).is_err());
}

#[test]
fn a_verb_and_a_stance_set_for_a_spell() {
    let state = GameState::default();
    let mut profile = CasterProfile::default();
    assert!(edit(&mut profile, &["verb", "901", "evoke"]).is_ok());
    assert!(edit(&mut profile, &["stance", "901", "offensive"]).is_ok());
    assert_eq!(
        lines(&profile, &state, &["901"]),
        Ok(vec![
            "stance offensive".to_owned(),
            "incant 901 evoke".to_owned(),
            "stance guarded".to_owned()
        ])
    );
    assert!(edit(&mut profile, &["stance", "901", "sideways"]).is_err());
    assert!(edit(&mut profile, &["verb", "901", "shout"]).is_err());
}

#[test]
fn safety_refuses_an_attack_spell_with_nothing_hostile_here() {
    let state = GameState::default();
    let mut profile = CasterProfile::default();
    assert!(edit(&mut profile, &["set", "safety", "on"]).is_ok());
    assert!(
        lines(&profile, &state, &["901"]).is_err(),
        "Minor Shock is an attack"
    );
    assert!(
        lines(&profile, &state, &["401"]).is_ok(),
        "a defense is not"
    );
}

#[test]
fn a_typed_number_or_alias_is_a_spell_until_switched_off() {
    let mut profile = CasterProfile::default();
    assert!(profile.typed, "on unless the player turns it off");
    assert!(edit(&mut profile, &["alias", "401", "ed"]).is_ok());
    assert_eq!(
        typed(&profile, "401 bob 3"),
        Some(vec!["401".to_owned(), "bob".to_owned(), "3".to_owned()])
    );
    assert!(typed(&profile, "Ed").is_some(), "an alias");
    assert_eq!(typed(&profile, "north"), None);
    assert_eq!(typed(&profile, "99"), None, "not three or four digits");
    assert_eq!(typed(&profile, "9999"), None, "not a spell");
    assert!(edit(&mut profile, &["set", "typed", "off"]).is_ok());
    assert_eq!(typed(&profile, "401"), None);
    assert!(
        CasterProfile::parse("").is_ok_and(|p| p.typed),
        "a file that does not say keeps it on"
    );
}

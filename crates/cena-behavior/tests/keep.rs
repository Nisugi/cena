//! Keep (`plan/37` Stage 4): spellactive's choice of what to cast next.

use std::collections::BTreeMap;

use cena_behavior::keep::{KeepProfile, edit, next};
use cena_session::{Effect, Frame, GameState};

fn at(time: u32) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: time.to_string(),
        text: ">".into(),
    });
    state
}

fn up(state: &mut GameState, id: &str, category: &str, text: &str, ends_at: u32) {
    state.effects.insert(
        id.to_owned(),
        Effect {
            category: category.to_owned(),
            text: text.to_owned(),
            ends_at: Some(ends_at),
            percent: 50,
        },
    );
}

fn keeping(spells: &[u16]) -> KeepProfile {
    KeepProfile {
        spells: spells.to_vec(),
        ..KeepProfile::default()
    }
}

#[test]
fn a_spell_down_is_cast_one_up_is_not_and_not_again_at_once() {
    let mut state = at(1000);
    up(
        &mut state,
        "401",
        "Active Spells",
        "Elemental Defense I",
        2000,
    );
    let mut tried = BTreeMap::new();
    let profile = keeping(&[401, 406]);
    assert_eq!(
        next(&profile, &state, None, &mut tried),
        Some(vec!["incant 406".to_owned()])
    );
    assert_eq!(
        next(&profile, &state, None, &mut tried),
        None,
        "sent a moment ago: not again at every prompt"
    );
}

#[test]
fn nothing_in_a_nocast_room() {
    let state = at(1000);
    let profile = KeepProfile {
        nocast: vec![228],
        ..keeping(&[406])
    };
    assert_eq!(
        next(&profile, &state, Some(228), &mut BTreeMap::new()),
        None
    );
    assert!(next(&profile, &state, Some(229), &mut BTreeMap::new()).is_some());
}

#[test]
fn spellactives_substitutions_and_waits() {
    let mut state = at(1000);
    assert_eq!(
        next(&keeping(&[606]), &state, None, &mut BTreeMap::new()),
        Some(vec!["incant 625".to_owned()]),
        "606 by way of 625"
    );
    assert_eq!(
        next(&keeping(&[1699]), &state, None, &mut BTreeMap::new()),
        Some(vec!["incant 1608".to_owned()]),
        "Beacon of Courage by 1608"
    );
    assert_eq!(
        next(&keeping(&[506]), &state, None, &mut BTreeMap::new()),
        None,
        "nothing hostile here"
    );
    up(&mut state, "Barkskin", "Cooldowns", "Barkskin", 2000);
    assert_eq!(
        next(&keeping(&[605]), &state, None, &mut BTreeMap::new()),
        None,
        "Barkskin waits out its cooldown"
    );
}

#[test]
fn the_profile_edits_spellactive_takes() {
    let mut profile = KeepProfile::default();
    assert!(edit(&mut profile, &["add", "401"]).is_ok());
    assert!(edit(&mut profile, &["add", "elemental", "defense", "ii"]).is_ok());
    assert_eq!(profile.spells, [401, 406]);
    assert!(edit(&mut profile, &["del", "401"]).is_ok());
    assert!(edit(&mut profile, &["nocast", "add", "228"]).is_ok());
    assert!(edit(&mut profile, &["power"]).is_ok());
    assert_eq!(profile.spells, [406]);
    assert_eq!(profile.nocast, [228]);
    assert!(!profile.power);
    assert!(edit(&mut profile, &["add", "no such spell"]).is_err());
}

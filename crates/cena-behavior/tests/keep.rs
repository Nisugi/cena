//! Keep (`plan/37` Stage 4): spellactive's choice of what to cast next.

use std::collections::BTreeMap;

use cena_behavior::keep::{KeepProfile, edit, next};
use cena_session::{Effect, Frame, GameState, Link, LinkKind, Run, Runs};

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

/// The settings menu's table is every field, in order: a field added to the
/// profile and not there would never be shown (`plan/50` §7 step 1).
#[test]
fn the_menus_table_is_every_setting() {
    let table = toml::Table::try_from(cena_behavior::keep::KeepProfile::default())
        .expect("a profile is a table");
    let keys: Vec<&str> = table.keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        cena_behavior::settings::names(cena_behavior::keep::TABLE)
    );
}

/// A vital, as the wire states it.
fn vital(state: &mut GameState, id: &str, current: i32, max: i32) {
    let percent = u32::try_from(current * 100 / max).unwrap_or(0);
    state.apply(&Frame::ProgressBar(cena_session::ProgressBar {
        id: id.to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent,
        text: format!("{id} {current}/{max}"),
        amount: Some(cena_session::Amount { current, max }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
}

/// The game's spell list, stating these and no others.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only the link matters"
)]
fn knows(state: &mut GameState, spells: &[(&str, &str)]) {
    state.known_spells.begin();
    for (number, name) in spells {
        state.known_spells.read_line(&Runs {
            runs: vec![Run {
                text: (*name).to_owned(),
                style: Default::default(),
                link: Some(Link {
                    kind: LinkKind::Exist {
                        id: "x".to_owned(),
                        noun: (*number).to_owned(),
                    },
                    text: (*name).to_owned(),
                    coord: None,
                }),
                inner_link: None,
            }],
        });
    }
}

/// spellactive's `power` (`:246-250`): Sigil of Power when the mana is 25
/// short, if the sigil is known, once in the retry window.
#[test]
fn sigil_of_power_when_25_short_and_known() {
    let mut state = at(1000);
    vital(&mut state, "mana", 60, 100);
    let profile = KeepProfile {
        power: true,
        ..keeping(&[])
    };
    let mut tried = BTreeMap::new();
    assert_eq!(
        next(&profile, &state, None, &mut tried),
        None,
        "the game has not listed the sigil"
    );
    knows(&mut state, &[("9718", "Sigil of Power")]);
    assert_eq!(
        next(&profile, &state, None, &mut tried),
        Some(vec!["sigil of power".to_owned()])
    );
    assert_eq!(
        next(&profile, &state, None, &mut tried),
        None,
        "not again within the retry window"
    );
    let later = {
        let mut later = state.clone();
        later.apply(&Frame::Prompt {
            time: "1031".into(),
            text: ">".into(),
        });
        later
    };
    assert_eq!(
        next(&profile, &later, None, &mut tried),
        Some(vec!["sigil of power".to_owned()]),
        "the window passed"
    );
    vital(&mut state, "mana", 76, 100);
    assert_eq!(
        next(&profile, &state, None, &mut BTreeMap::new()),
        None,
        "24 short is not 25"
    );
    let off = KeepProfile {
        power: false,
        ..profile
    };
    vital(&mut state, "mana", 0, 100);
    assert_eq!(next(&off, &state, None, &mut BTreeMap::new()), None);
}

/// A spell that costs spirit is cast only above 75% spirit (`:216`).
#[test]
fn a_spirit_spell_waits_for_spirit_above_three_quarters() {
    let mut state = at(1000);
    vital(&mut state, "mana", 100, 100);
    vital(&mut state, "spirit", 7, 10);
    let profile = keeping(&[340]);
    assert_eq!(
        state.spell_cost(340, "spirit"),
        Some(2.0),
        "Symbol of the Proselyte costs spirit"
    );
    assert_eq!(next(&profile, &state, None, &mut BTreeMap::new()), None);
    vital(&mut state, "spirit", 8, 10);
    assert_eq!(
        next(&profile, &state, None, &mut BTreeMap::new()),
        Some(vec!["incant 340".to_owned()])
    );
}

/// spellactive's cooldown-bound short spells (`:204`) wait out a cooldown
/// under their own name, and a war spell is cast once something can be
/// attacked here.
#[test]
fn a_cooldown_under_the_spells_name_waits_and_a_war_spell_needs_a_target() {
    let mut state = at(1000);
    up(&mut state, "Bravery", "Cooldowns", "Bravery", 2000);
    assert_eq!(
        next(&keeping(&[211]), &state, None, &mut BTreeMap::new()),
        None,
        "Bravery cooling"
    );
    assert_eq!(
        next(&keeping(&[215]), &state, None, &mut BTreeMap::new()),
        Some(vec!["incant 215".to_owned()]),
        "Heroism is not"
    );
    let mut state = at(1000);
    assert!(state.targeting.read("#1234", Some("giant warg")));
    assert_eq!(
        next(&keeping(&[506]), &state, None, &mut BTreeMap::new()),
        Some(vec!["incant 506".to_owned()]),
        "a target: Celerity is cast"
    );
}

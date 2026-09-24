//! Setting the stance: what is sent, when nothing is, and what confirms it
//! (`plan/30` §3, Lich's `stance.rb`).

use cena_behavior::stance::{Want, command, landed, safest};
use cena_session::{GameState, PsmCategory, PsmLine, PsmRanks, Stance};

/// A character standing where the bar says.
fn at(text: &str, percent: u32) -> GameState {
    let mut state = GameState::default();
    state.character.stance = Some(text.to_owned());
    state.character.stance_percent = Some(percent);
    state
}

fn with_perfection(mut state: GameState) -> GameState {
    state.character.psms.replace_category(
        PsmCategory::CombatManeuver,
        &[PsmLine {
            mnemonic: "stance".to_owned(),
            display_name: "Stance Perfection".to_owned(),
            ranks: PsmRanks {
                ranks: 1,
                max: 1,
                known_by_bold: false,
            },
            kind: "Passive".to_owned(),
            category: String::new(),
        }],
    );
    state
}

#[test]
fn a_name_or_a_multiple_of_ten_is_asked_for_and_nothing_else() {
    assert_eq!(Want::parse("gua"), Ok(Want::Named(Stance::Guarded)));
    assert_eq!(Want::parse("80"), Ok(Want::Percent(80)));
    assert!(
        Want::parse("85").is_err(),
        "Lich refuses a percent off the tens"
    );
    assert!(Want::parse("110").is_err());
    assert!(Want::parse("of").is_err(), "two letters name nothing");
}

#[test]
fn nothing_is_sent_when_already_there() {
    let state = at("guarded (80%)", 80);
    assert_eq!(command(Want::Named(Stance::Guarded), &state), None);
    assert_eq!(command(Want::Percent(80), &state), None);
    assert!(landed(Want::Named(Stance::Guarded), &state));
}

#[test]
fn a_percent_is_sent_exactly_only_with_stance_perfection() {
    let state = at("defensive (100%)", 100);
    assert_eq!(
        command(Want::Percent(80), &state).as_deref(),
        Some("stance guarded"),
        "without it, the band's name"
    );
    let state = with_perfection(state);
    assert_eq!(
        command(Want::Percent(80), &state).as_deref(),
        Some("cman stance 80")
    );
    assert_eq!(
        command(Want::Named(Stance::Offensive), &state).as_deref(),
        Some("stance offensive")
    );
}

#[test]
fn a_change_the_game_refused_has_not_landed() {
    // "Cast Roundtime in effect": the bar did not move.
    let still = at("defensive (100%)", 100);
    assert!(!landed(Want::Named(Stance::Offensive), &still));
    // A game event knocked the percent off a round number: the name holds,
    // the exact percent does not.
    let knocked = at("defensive (99%)", 99);
    assert!(landed(Want::Named(Stance::Defensive), &knocked));
    assert!(!landed(Want::Percent(100), &knocked));
}

#[test]
fn the_safest_stance_is_guarded_unless_no_cast_roundtime_can_run() {
    let unknown = GameState::default();
    assert_eq!(
        safest(&unknown),
        Stance::Guarded,
        "an unknown clock proves nothing"
    );
}

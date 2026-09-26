//! The casting step (`plan/37` Stage 3): the lines, the answers, readiness.

use cena_behavior::cast::{Answer, Casting, NotReady, Verb, classify, ready};
use cena_session::{Amount, Frame, GameState, ProgressBar};

fn mana(state: &mut GameState, current: i32, max: i32) {
    state.apply(&Frame::ProgressBar(ProgressBar {
        id: "mana".to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent: 50,
        text: format!("mana {current}/{max}"),
        amount: Some(Amount { current, max }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
}

#[test]
fn incant_alone_prepare_and_cast_at_a_target_release_another_first() {
    let mut state = GameState::default();
    let alone = Casting {
        spell: 401,
        count: Some(3),
        ..Casting::default()
    };
    assert_eq!(alone.lines(&state), ["incant 401 3"]);
    let channeled = Casting {
        spell: 901,
        verb: Verb::Channel,
        ..Casting::default()
    };
    assert_eq!(channeled.lines(&state), ["incant 901 channel"]);
    let at = Casting {
        spell: 401,
        target: Some("bob".to_owned()),
        ..Casting::default()
    };
    assert_eq!(at.lines(&state), ["prepare 401", "cast bob"]);
    state.apply(&Frame::Spell {
        text: "Minor Sanctuary".to_owned(),
    });
    assert_eq!(alone.lines(&state), ["release", "incant 401 3"]);
    state.apply(&Frame::Spell {
        text: "None".to_owned(),
    });
    assert_eq!(alone.lines(&state), ["incant 401 3"]);
}

#[test]
fn the_answers_lich_waits_on() {
    assert_eq!(classify("Cast Roundtime 3 Seconds."), Some(Answer::Cast));
    assert_eq!(
        classify("Your magic fizzles ineffectually."),
        Some(Answer::Fizzled)
    );
    assert_eq!(
        classify(
            "[Spell Hindrance for Full Plate is 45% with current Armor Use skill, d100 roll: 12]"
        ),
        Some(Answer::Hindered)
    );
    assert_eq!(classify("Cast at what?"), Some(Answer::NoTarget));
    assert_eq!(
        classify("Be at peace my child, there is no need for spells of war in here."),
        Some(Answer::NotHere)
    );
    assert_eq!(
        classify("You are unable to do that right now."),
        Some(Answer::Unable)
    );
    assert_eq!(classify("You swing a sword."), None);
}

#[test]
fn not_enough_mana_for_the_count_and_the_reserve() {
    let mut state = GameState::default();
    // 401 costs 1 mana.
    mana(&mut state, 5, 100);
    assert_eq!(ready(&state, 401, 3, 0), Ok(()));
    assert!(matches!(ready(&state, 401, 3, 3), Err(NotReady::Mana(..))));
    assert_eq!(ready(&state, 401, 1, 4), Ok(()));
}

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
        link: None,
    });
    assert_eq!(alone.lines(&state), ["release", "incant 401 3"]);
    state.apply(&Frame::Spell {
        text: "None".to_owned(),
        link: None,
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

/// A pool's bar, as the wire states it.
fn bar(state: &mut GameState, id: &str, current: i32, max: i32) {
    state.apply(&Frame::ProgressBar(ProgressBar {
        id: id.to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent: 50,
        text: format!("{id} {current}/{max}"),
        amount: Some(Amount { current, max }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
}

/// The first spell the table prices in `pool`, and what it costs this
/// character.
fn priced_in(state: &GameState, pool: &str) -> Option<(u16, i32)> {
    (100_u16..10_000).find_map(|number| {
        let cost = state.spell_cost(number, pool).filter(|cost| *cost > 0.0)?;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a spell's cost is a small whole number"
        )]
        Some((number, cost.ceil() as i32))
    })
}

#[test]
fn the_rest_of_what_lich_waits_on() {
    assert_eq!(classify("Sing Roundtime 3 Seconds."), Some(Answer::Cast));
    assert_eq!(
        classify("But you don't have any mana!"),
        Some(Answer::NoMana)
    );
    assert_eq!(
        classify("You do not currently have a target."),
        Some(Answer::NoTarget)
    );
    assert_eq!(
        classify("You can only evoke certain spells."),
        Some(Answer::NoSuchVerb)
    );
    assert_eq!(
        classify("You do not know that spell!"),
        Some(Answer::Unknown)
    );
    assert_eq!(classify("Your spell is ready."), Some(Answer::Ready));
    assert_eq!(
        classify("You already have a spell readied!"),
        Some(Answer::AlreadyReady)
    );
    assert_eq!(
        classify("Cast Roundtime 3 Seconds"),
        None,
        "the game ends the line with a full stop"
    );
}

#[test]
fn cast_roundtime_left_is_waited_out_to_the_second() {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.apply(&Frame::CastTime { value: 1_003 });
    assert_eq!(ready(&state, 401, 1, 0), Err(NotReady::CastRoundtime(3)));
    state.apply(&Frame::Prompt {
        time: "1003".into(),
        text: ">".into(),
    });
    assert_eq!(ready(&state, 401, 1, 0), Ok(()), "over at its second");
}

#[test]
fn spirit_keeps_one_back_and_stamina_does_not() {
    let mut state = GameState::default();
    let (spell, cost) = priced_in(&state, "spirit").expect("a spell that costs spirit");
    bar(&mut state, "spirit", cost, 10);
    assert_eq!(
        ready(&state, spell, 1, 0),
        Err(NotReady::Spirit),
        "its cost exactly would leave none: one is kept back"
    );
    bar(&mut state, "spirit", cost + 1, 10);
    assert_eq!(ready(&state, spell, 1, 0), Ok(()));

    let mut state = GameState::default();
    let (spell, cost) = priced_in(&state, "stamina").expect("a spell that costs stamina");
    bar(&mut state, "stamina", cost - 1, 100);
    assert_eq!(ready(&state, spell, 1, 0), Err(NotReady::Stamina));
    bar(&mut state, "stamina", cost, 100);
    assert_eq!(
        ready(&state, spell, 1, 0),
        Ok(()),
        "its cost exactly is enough"
    );
}

//! Waggle (`plan/37` Stage 5): ewaggle's passes, and `spell active` read.

use cena_behavior::cast::Answer;
use cena_behavior::waggle::{Step, WaggleProfile, Waggled, Waggler, read_spell_active};
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs, SkillLine};

/// A character who knows Elemental Defense I (401) with 30 ranks of Minor
/// Elemental: 150 minutes a cast on self, 50 on another.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only the link matters"
)]
fn caster() -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.character.name = Some("Nisugi".to_owned());
    if let Some(line) = SkillLine::classify("  Minor Elemental..................|           30") {
        state.character.skills.apply(&line, false);
    }
    state.known_spells.begin();
    state.known_spells.read_line(&Runs {
        runs: vec![Run {
            text: "Elemental Defense I".to_owned(),
            style: Default::default(),
            link: Some(Link {
                kind: LinkKind::Exist {
                    id: "x".to_owned(),
                    noun: "401".to_owned(),
                },
                text: "Elemental Defense I".to_owned(),
                coord: None,
            }),
            inner_link: None,
        }],
    });
    state
}

fn profile() -> WaggleProfile {
    WaggleProfile {
        cast_list: vec![401],
        ..WaggleProfile::default()
    }
}

#[test]
fn a_stackable_spell_is_cast_on_yourself_until_stop_at() {
    let state = caster();
    assert_eq!(
        state.spell_minutes(401, cena_session::spells::CastType::SelfCast),
        Some(150.0)
    );
    let mut run = Waggler::new(profile(), &[]);
    let Step::Cast(first) = run.next(&state) else {
        panic!("a cast");
    };
    assert_eq!((first.spell, first.target.clone()), (401, None));
    run.outcome(&[], &[Answer::Cast], &state);
    assert!(
        matches!(run.next(&state), Step::Cast(_)),
        "150 of 180: once more"
    );
    run.outcome(&[], &[Answer::Cast], &state);
    assert_eq!(
        run.next(&state),
        Step::Done(Waggled::Done(2)),
        "300: enough"
    );
}

#[test]
fn another_is_asked_first_and_topped_up_only_under_start_at() {
    let state = caster();
    let asked = |time: &str| {
        let mut run = Waggler::new(profile(), &["Bob".to_owned()]);
        assert_eq!(run.next(&state), Step::Ask("Bob".to_owned()));
        let lines = vec![
            "Bob currently has the following active effects:".to_owned(),
            format!("  Elemental Defense I ......... {time}"),
        ];
        run.outcome(&lines, &[], &state);
        run.next(&state)
    };
    assert_eq!(
        asked("3:05:00"),
        Step::Done(Waggled::Done(0)),
        "185 minutes is over start_at"
    );
    let Step::Cast(casting) = asked("2:55:00") else {
        panic!("175 minutes is under start_at: a cast");
    };
    assert_eq!(casting.target.as_deref(), Some("Bob"));
}

#[test]
fn spell_active_is_read_for_sharing_and_minutes() {
    let (sharing, have) = read_spell_active(&[
        "Bob currently has the following active effects:".to_owned(),
        "  Elemental Defense I ......... 1:30:30".to_owned(),
        "  Mage Armor - Fire .......... Indefinite".to_owned(),
    ]);
    assert!(sharing);
    assert_eq!(have.get(&401), Some(&90.5));
    assert_eq!(have.get(&520), Some(&599.0));
    let (sharing, _) = read_spell_active(&["Bob has spell sharing disabled.".to_owned()]);
    assert!(!sharing);
}

/// Mana, as the wire states it.
fn mana(state: &mut GameState, current: i32) {
    state.apply(&Frame::ProgressBar(cena_session::ProgressBar {
        id: "mana".to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent: 50,
        text: format!("mana {current}/100"),
        amount: Some(cena_session::Amount { current, max: 100 }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
}

/// A run on yourself that has just been told to cast 401.
fn casting() -> (GameState, Waggler) {
    let state = caster();
    let mut run = Waggler::new(profile(), &[]);
    assert!(matches!(run.next(&state), Step::Cast(_)), "a cast");
    (state, run)
}

#[test]
fn a_spell_the_room_or_the_game_refuses_is_not_tried_again() {
    for refusal in [Answer::NotHere, Answer::Unknown] {
        let (state, mut run) = casting();
        run.outcome(&[], &[refusal], &state);
        assert_eq!(
            run.next(&state),
            Step::Done(Waggled::Done(0)),
            "{refusal:?}: the one spell on the list is left"
        );
    }
}

#[test]
fn armor_in_the_way_is_cast_again() {
    let (state, mut run) = casting();
    run.outcome(&[], &[Answer::Hindered], &state);
    let Step::Cast(again) = run.next(&state) else {
        panic!("the same cast again");
    };
    assert_eq!(again.spell, 401);
    run.outcome(&[], &[Answer::Cast], &state);
    run.next(&state);
    run.outcome(&[], &[Answer::Cast], &state);
    assert_eq!(
        run.next(&state),
        Step::Done(Waggled::Done(2)),
        "the hindered one is not counted as cast"
    );
}

#[test]
fn an_answer_nobody_read_moves_on_rather_than_casting_for_ever() {
    let (state, mut run) = casting();
    run.outcome(&[], &[], &state);
    assert_eq!(run.next(&state), Step::Done(Waggled::Done(0)));
}

#[test]
fn someone_who_cannot_be_cast_at_is_left_and_the_next_one_taken() {
    let state = caster();
    let mut run = Waggler::new(profile(), &["Bob".to_owned(), "self".to_owned()]);
    assert_eq!(run.next(&state), Step::Ask("Bob".to_owned()));
    run.outcome(
        &["Bob currently has the following active effects:".to_owned()],
        &[],
        &state,
    );
    let Step::Cast(at_bob) = run.next(&state) else {
        panic!("a cast at Bob");
    };
    assert_eq!(at_bob.target.as_deref(), Some("Bob"));
    run.outcome(&[], &[Answer::NoTarget], &state);
    let Step::Cast(own) = run.next(&state) else {
        panic!("Bob is left, and yourself is next");
    };
    assert_eq!(own.target, None);
}

#[test]
fn someone_not_sharing_gets_one_cast_or_none_by_the_profile() {
    let state = caster();
    let hidden = vec!["Bob has spell sharing disabled.".to_owned()];
    let mut run = Waggler::new(profile(), &["Bob".to_owned()]);
    assert_eq!(run.next(&state), Step::Ask("Bob".to_owned()));
    run.outcome(&hidden, &[], &state);
    assert!(
        matches!(run.next(&state), Step::Cast(_)),
        "what Bob has cannot be known: one cast"
    );
    run.outcome(&[], &[Answer::Cast], &state);
    assert_eq!(
        run.next(&state),
        Step::Done(Waggled::Done(1)),
        "and one only"
    );

    let skipping = WaggleProfile {
        skip_not_sharing: true,
        ..profile()
    };
    let mut run = Waggler::new(skipping, &["Bob".to_owned()]);
    assert_eq!(run.next(&state), Step::Ask("Bob".to_owned()));
    run.outcome(&hidden, &[], &state);
    assert_eq!(run.next(&state), Step::Done(Waggled::Done(0)));
}

#[test]
fn short_of_mana_waits_or_ends_the_run_by_the_profile() {
    let mut state = caster();
    mana(&mut state, 0);
    let mut run = Waggler::new(profile(), &[]);
    assert_eq!(run.next(&state), Step::Wait(5), "waited for");
    let bailing = WaggleProfile {
        bail: true,
        ..profile()
    };
    let mut run = Waggler::new(bailing, &[]);
    assert_eq!(run.next(&state), Step::Done(Waggled::OutOfMana));
    // The reserve is mana too.
    mana(&mut state, 5);
    let keeping = WaggleProfile {
        reserve_mana: 10,
        ..profile()
    };
    let mut run = Waggler::new(keeping, &[]);
    assert_eq!(run.next(&state), Step::Wait(5));
}

#[test]
fn cast_roundtime_is_waited_out_for_as_long_as_it_has_left() {
    let mut state = caster();
    state.apply(&Frame::CastTime { value: 1_003 });
    let mut run = Waggler::new(profile(), &[]);
    assert_eq!(run.next(&state), Step::Wait(3));
}

//! The game's replies to what the hunt sent (`inventory/12` §2), read and
//! acted on at the next tick.

use cena_behavior::hunt::replies::{Reply, read};
use cena_behavior::hunt::{Ending, Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs};

const PROFILE: &str = r#"
targets = [{ any = true, routine = "a" }]

[rooms]
hunting = 10
resting = 20

[wander]
wait = 0

[routines]
a = ["attack"]
"#;

/// A state at game second `second` in the game's room `room`, with one
/// hostile creature, `#42`, already targeted.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn fighting(second: u32, room: &str) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: second.to_string(),
        text: ">".into(),
    });
    state.room.id = Some(room.to_owned());
    state.status.set("standing", true);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    let mut run = Run {
        text: "kobold".to_owned(),
        style: Default::default(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: "42".to_owned(),
                noun: "kobold".to_owned(),
            },
            text: "kobold".to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs: vec![run] },
    });
    state.apply(&Frame::CreatureStatus {
        id: "42".to_owned(),
        attrs: vec![
            ("exist".to_owned(), "42".to_owned()),
            ("hostile".to_owned(), "1".to_owned()),
        ],
    });
    state.targeting.read("#42", None);
    state
}

const EXITS: &[RoomId] = &[RoomId(11)];

fn here(room: u32) -> Here<'static> {
    Here {
        room: Some(RoomId(room)),
        exits: EXITS,
    }
}

fn attack() -> Said {
    Said::Send {
        line: "attack".to_owned(),
        target: Some(42),
    }
}

#[test]
fn each_reply_is_read_and_other_lines_are_not() {
    for (line, reply) in [
        (
            "You currently have no valid target.  You will need to specify one.",
            Reply::NoTarget,
        ),
        (
            "It looks like somebody already did the job for you.",
            Reply::NoTarget,
        ),
        (
            "You spin about but don't see anything to hit!",
            Reply::NoTarget,
        ),
        ("You can't make that dextrous of a move!", Reply::Injured),
        (
            "You can't think clearly enough to prepare a spell!",
            Reply::Injured,
        ),
        (
            "Your arrow strikes the kobold, but it has no effect!",
            Reply::NoEffect,
        ),
        (
            "Be at peace my child, there is no need for spells of war in here.",
            Reply::Sanctuary,
        ),
        (
            "You are unable to muster the will to attack anything.",
            Reply::Unwilling,
        ),
        (
            "You don't seem to be able to move to do that.",
            Reply::Rooted,
        ),
        ("But you don't have any mana!", Reply::NoMana),
    ] {
        assert_eq!(read(line), Some(reply), "{line}");
    }
    assert_eq!(read("You swing a broadsword at a kobold!"), None);
}

#[test]
fn a_kill_that_landed_in_flight_drops_the_target() {
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let state = fighting(1_000, "10");
    assert_eq!(hunt.tick(&state, here(10), Some(1_000)), attack());
    assert_eq!(hunt.target(), Some(42));
    hunt.replied(
        ["It looks like somebody already did the job for you."],
        Some(1_000),
    );
    assert_eq!(hunt.target(), None);
}

#[test]
fn a_sanctuary_is_left_and_not_fought_in() {
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let state = fighting(1_000, "10");
    assert_eq!(hunt.tick(&state, here(10), Some(1_000)), attack());
    hunt.replied(
        ["Be at peace my child, there is no need for spells of war in here."],
        Some(1_000),
    );
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_001)),
        Said::Walk(RoomId(11)),
        "a creature is here and the hunt still walks on"
    );
}

#[test]
fn calmed_or_rooted_waits_a_moment_then_attacks_again() {
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let state = fighting(1_000, "10");
    assert_eq!(hunt.tick(&state, here(10), Some(1_000)), attack());
    hunt.replied(
        ["You don't seem to be able to move to do that."],
        Some(1_000),
    );
    assert_eq!(hunt.tick(&state, here(10), Some(1_001)), Said::Wait(1));
    assert_eq!(hunt.tick(&state, here(10), Some(1_003)), attack());
}

#[test]
fn an_attack_with_no_effect_ends_the_hunt() {
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let state = fighting(1_000, "10");
    assert_eq!(hunt.tick(&state, here(10), Some(1_000)), attack());
    hunt.replied(
        ["Your arrow strikes the kobold, but it has no effect!"],
        Some(1_000),
    );
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_001)),
        Said::Done(Ending::NoEffect)
    );
}

#[test]
fn an_injury_rests_once_and_ends_the_hunt_if_the_rest_did_not_mend_it() {
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let state = fighting(1_000, "10");
    assert_eq!(hunt.tick(&state, here(10), Some(1_000)), attack());
    hunt.replied(["You can't make that dextrous of a move!"], Some(1_000));
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_001)),
        Said::Walk(RoomId(20)),
        "to the resting room"
    );
    // Rested, and back: nothing in the profile keeps the rest going.
    let resting = fighting(1_002, "20");
    let mut said = hunt.tick(&resting, here(20), Some(1_002));
    for second in 1_003..1_010 {
        if said == Said::Walk(RoomId(10)) {
            break;
        }
        said = hunt.tick(&resting, here(20), Some(second));
    }
    assert_eq!(said, Said::Walk(RoomId(10)), "walking back");
    let back = fighting(1_020, "10");
    let mut said = hunt.tick(&back, here(10), Some(1_020));
    for second in 1_021..1_030 {
        if said == attack() {
            break;
        }
        said = hunt.tick(&back, here(10), Some(second));
    }
    assert_eq!(said, attack(), "hunting again");
    hunt.replied(["You can't make that dextrous of a move!"], Some(1_030));
    assert_eq!(
        hunt.tick(&back, here(10), Some(1_031)),
        Said::Done(Ending::Injured)
    );
}

/// Another player walking in after the room was claimed does not stall the
/// hunt; a room someone else was in first is walked on from.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors"
)]
#[test]
fn the_room_is_claimed_on_entry_not_every_tick() {
    let stranger = |state: &mut GameState| {
        state.apply(&Frame::Component {
            id: "room players".into(),
            body: Runs {
                runs: vec![Run {
                    text: "Bob".to_owned(),
                    style: Default::default(),
                    link: Some(Link {
                        kind: LinkKind::Exist {
                            id: "-9".to_owned(),
                            noun: "Bob".to_owned(),
                        },
                        text: "Bob".to_owned(),
                        coord: None,
                    }),
                    inner_link: None,
                }],
            },
        });
    };
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let mut state = fighting(1_000, "10");
    assert_eq!(hunt.tick(&state, here(10), Some(1_000)), attack());
    stranger(&mut state);
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_001)),
        attack(),
        "claimed on entry: Bob arriving changes nothing"
    );

    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let mut theirs = fighting(1_000, "10");
    stranger(&mut theirs);
    assert_eq!(
        hunt.tick(&theirs, here(10), Some(1_000)),
        Said::Walk(RoomId(11)),
        "Bob was here first"
    );
}

#[test]
fn a_dread_over_its_threshold_sends_the_hunt_to_rest_and_stop_after_ends_it() {
    let profile = format!("{PROFILE}\n[rest]\nstop_after = 1\n[rest.when]\ncreeping_dread = 5\n");
    let mut hunt = Hunt::new(Profile::parse(&profile).unwrap(), 1);
    let mut state = fighting(1_000, "10");
    state.effects.insert(
        "cd".to_owned(),
        cena_session::Effect {
            category: "Debuffs".to_owned(),
            text: "Creeping Dread (4)".to_owned(),
            ends_at: None,
            percent: 0,
        },
    );
    assert_eq!(hunt.tick(&state, here(10), Some(1_000)), attack(), "4 of 5");
    state.effects.insert(
        "cd".to_owned(),
        cena_session::Effect {
            category: "Debuffs".to_owned(),
            text: "Creeping Dread (5)".to_owned(),
            ends_at: None,
            percent: 0,
        },
    );
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_001)),
        Said::Walk(RoomId(20))
    );
    state.effects.clear();
    let resting = {
        let mut s = fighting(1_002, "20");
        s.effects.clear();
        s
    };
    let mut said = hunt.tick(&resting, here(20), Some(1_002));
    for second in 1_003..1_010 {
        if matches!(said, Said::Done(_)) {
            break;
        }
        said = hunt.tick(&resting, here(20), Some(second));
    }
    assert_eq!(said, Said::Done(Ending::Rested(1)));
}

/// A cloud in the room, beside the kobold.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors"
)]
fn clouded(state: &mut GameState) {
    let run = |id: &str, noun: &str, bold: u16| {
        let mut run = Run {
            text: noun.to_owned(),
            style: Default::default(),
            link: Some(Link {
                kind: LinkKind::Exist {
                    id: id.to_owned(),
                    noun: noun.to_owned(),
                },
                text: noun.to_owned(),
                coord: None,
            }),
            inner_link: None,
        };
        run.style.bold_depth = bold;
        run
    };
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs {
            runs: vec![run("42", "kobold", 1), run("88", "cloud", 0)],
        },
    });
}

#[test]
fn a_hazard_or_a_message_the_profile_flees_sends_the_hunt_out() {
    let mut state = fighting(1_000, "10");
    clouded(&mut state);
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_000)),
        attack(),
        "clouds not fled"
    );
    let fleeing =
        format!("{PROFILE}\n[flee]\nclouds = true\nmessages = [\"the ground trembles\"]\n");
    let mut hunt = Hunt::new(Profile::parse(&fleeing).unwrap(), 1);
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_000)),
        Said::Walk(RoomId(11))
    );
    let calm = fighting(1_000, "10");
    let mut hunt = Hunt::new(Profile::parse(&fleeing).unwrap(), 1);
    assert_eq!(hunt.tick(&calm, here(10), Some(1_000)), attack());
    hunt.heard("The Ground Trembles beneath you!");
    assert_eq!(
        hunt.tick(&calm, here(10), Some(1_001)),
        Said::Walk(RoomId(11))
    );
}

#[test]
fn low_spirit_rests_when_the_profile_says() {
    let profile = format!("{PROFILE}\n[rest.when]\nspirit_at_most = 50\n");
    let mut hunt = Hunt::new(Profile::parse(&profile).unwrap(), 1);
    let mut state = fighting(1_000, "10");
    let spirit = |percent: u32| {
        Frame::ProgressBar(cena_session::ProgressBar {
            id: "spirit".to_owned(),
            dialog: None,
            percent,
            text: format!("spirit {}/10", percent / 10),
            amount: None,
            attrs: Vec::new(),
            time_remaining_secs: None,
        })
    };
    state.apply(&spirit(80));
    assert_eq!(hunt.tick(&state, here(10), Some(1_000)), attack());
    state.apply(&spirit(50));
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_001)),
        Said::Walk(RoomId(20))
    );
}

#[test]
fn lone_targets_only_leaves_a_room_entered_with_two() {
    let profile = format!("{PROFILE}\n[flee]\nlone_only = true\n");
    let mut hunt = Hunt::new(Profile::parse(&profile).unwrap(), 1);
    let mut state = fighting(1_000, "10");
    clouded(&mut state);
    // The cloud is not a creature: one kobold, stay.
    assert_eq!(hunt.tick(&state, here(10), Some(1_000)), attack());
}

#[test]
fn a_rest_fogs_then_walks_the_waypoints_then_the_resting_room() {
    let profile = format!(
        "{PROFILE}\n[rest]\nfog = [\"incant 130\"]\nwaypoints = [15]\n[rest.when]\nspirit_at_most = 50\n"
    );
    let mut hunt = Hunt::new(Profile::parse(&profile).unwrap(), 1);
    let mut state = fighting(1_000, "10");
    state.apply(&Frame::ProgressBar(cena_session::ProgressBar {
        id: "spirit".to_owned(),
        dialog: None,
        percent: 40,
        text: "spirit 4/10".to_owned(),
        amount: None,
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_000)),
        Said::Send {
            line: "incant 130".to_owned(),
            target: None
        }
    );
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_001)),
        Said::Walk(RoomId(15))
    );
    assert_eq!(
        hunt.tick(&state, here(15), Some(1_002)),
        Said::Walk(RoomId(20))
    );
}

#[test]
fn the_walk_back_goes_through_the_rally_points() {
    let profile = PROFILE.replace("resting = 20", "resting = 20\nrally = [30]");
    let mut hunt = Hunt::new(Profile::parse(&profile).unwrap(), 1);
    hunt.replied(["But you don't have any mana!"], Some(1_000));
    let state = fighting(1_000, "10");
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_000)),
        Said::Walk(RoomId(20))
    );
    let resting = fighting(1_001, "20");
    let mut said = hunt.tick(&resting, here(20), Some(1_001));
    for second in 1_002..1_010 {
        if matches!(said, Said::Walk(_)) {
            break;
        }
        said = hunt.tick(&resting, here(20), Some(second));
    }
    assert_eq!(said, Said::Walk(RoomId(30)), "the rally point first");
    let rally = fighting(1_020, "30");
    assert_eq!(
        hunt.tick(&rally, here(30), Some(1_020)),
        Said::Walk(RoomId(10))
    );
}

#[test]
fn a_voln_master_uses_symbol_of_mana_before_resting_for_mana() {
    let profile = format!("{PROFILE}\n[rest]\nwracking = true\n");
    let mut hunt = Hunt::new(Profile::parse(&profile).unwrap(), 1);
    let mut state = fighting(1_000, "10");
    state.character.standing.society = Some(Some(cena_session::Society::OrderOfVoln));
    state.character.standing.society_rank = Some(26);
    hunt.replied(["But you don't have any mana!"], Some(1_000));
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_000)),
        Said::Send {
            line: "symbol of mana".to_owned(),
            target: None
        }
    );
    assert_eq!(
        hunt.tick(&state, here(10), Some(1_001)),
        attack(),
        "the symbol answered the need; no rest"
    );
}

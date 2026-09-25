//! The hunt's answers to incidents (`hunt/react.rs`).

use cena_behavior::hunt::{Ending, Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::containers::ItemRef;
use cena_session::incident::{Disarm, Incident};
use cena_session::{Frame, GameState, Runs};

const PROFILE: &str = r#"
targets = [{ any = true, routine = "a" }]

[routines]
a = ["attack"]
"#;

fn standing(second: u32) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: second.to_string(),
        text: ">".into(),
    });
    state.room.id = Some("10".to_owned());
    state.status.set("standing", true);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    state
}

fn here() -> Here<'static> {
    Here {
        room: Some(RoomId(10)),
        exits: &[],
    }
}

fn send(line: &str) -> Said {
    Said::Send {
        line: line.to_owned(),
        target: None,
    }
}

fn knocked() -> Incident {
    Incident::Disarmed {
        how: Disarm::Knocked,
        weapon: Some(ItemRef {
            id: "5".to_owned(),
            noun: "broadsword".to_owned(),
            text: "steel broadsword".to_owned(),
        }),
    }
}

#[test]
fn a_knocked_weapon_is_knelt_for_and_recovered_and_the_kneel_is_kept() {
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let mut state = standing(1_000);
    hunt.incidents(&[knocked()]);
    assert_eq!(hunt.tick(&state, here(), Some(1_000)), send("kneel"));
    state.status.set("standing", false);
    state.status.set("kneeling", true);
    assert_eq!(
        hunt.tick(&state, here(), Some(1_001)),
        send("recover item"),
        "kneeling for a recovery is not stood up from"
    );
    hunt.replied(["You spy a steel broadsword and recover it!"], Some(1_002));
    assert_eq!(
        hunt.tick(&state, here(), Some(1_003)),
        send("stand"),
        "recovered: stand again"
    );
}

#[test]
fn ten_failed_searches_end_the_hunt() {
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let mut state = standing(1_000);
    state.status.set("standing", false);
    state.status.set("kneeling", true);
    hunt.incidents(&[knocked()]);
    for second in 0..10 {
        assert_eq!(
            hunt.tick(&state, here(), Some(1_000 + second)),
            send("recover item")
        );
    }
    assert_eq!(
        hunt.tick(&state, here(), Some(1_020)),
        Said::Done(Ending::Disarmed)
    );
}

#[test]
fn a_weapon_reaction_is_taken_unless_switched_off() {
    let state = standing(1_000);
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    hunt.incidents(&[Incident::WeaponReaction("TACKLE #77".to_owned())]);
    assert_eq!(
        hunt.tick(&state, here(), Some(1_000)),
        send("weapon TACKLE #77")
    );

    let off = format!("{PROFILE}\n[react]\nweapon_reaction = false\n");
    let mut hunt = Hunt::new(Profile::parse(&off).unwrap(), 1);
    hunt.incidents(&[Incident::WeaponReaction("TACKLE #77".to_owned())]);
    assert_ne!(
        hunt.tick(&state, here(), Some(1_000)),
        send("weapon TACKLE #77")
    );
}

#[test]
fn swallowed_the_hunt_fights_its_way_out() {
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let mut state = standing(1_000);
    state.room.title = Some("The Belly of the Beast".to_owned());
    assert_eq!(hunt.tick(&state, here(), Some(1_000)), send("attack wall"));
    assert_eq!(hunt.tick(&state, here(), Some(1_001)), send("attack wall"));
    state.room.title = Some("Ooze, Innards".to_owned());
    assert_eq!(hunt.tick(&state, here(), Some(1_002)), send("kill organ"));
    state.room.title = Some("Snowy Ridge".to_owned());
    assert_ne!(hunt.tick(&state, here(), Some(1_003)), send("kill organ"));
}

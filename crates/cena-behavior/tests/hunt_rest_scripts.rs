//! bigshot's `hunting_scripts` and `resting_scripts` (`bigshot.lic:7278`,
//! `:7539`): Hydra runs no Lich script, so the importer names each, and the
//! one a rest runs that Hydra has built in -- the waggle -- runs at the rest.

use cena_behavior::hunt::{Here, Hunt, Profile, Said, import};
use cena_behavior::waggle::WaggleProfile;
use cena_map::RoomId;
use cena_session::{Frame, GameState, Runs};

/// Standing in `room` at second 1000, half encumbered, nothing here.
fn standing(room: &str) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.room.id = Some(room.to_owned());
    state.status.set("standing", true);
    state.character.encumbrance_percent = Some(50);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs: Vec::new() },
    });
    state
}

fn tick(hunt: &mut Hunt, room: u32) -> Said {
    let state = standing(&room.to_string());
    let here = Here {
        room: Some(RoomId(room)),
        exits: &[],
        tags: &[],
    };
    hunt.tick(&state, here, state.game_time_now())
}

const PROFILE: &str = "targets = [{ any = true, routine = \"a\" }]\n[rooms]\nhunting = 10\nresting = 20\n[rest]\nencumbered = 40\ncommands = [\"sit\"]\n[routines]\na = [\"attack\"]\n";

#[test]
fn a_rest_with_the_waggle_on_waggles_first_and_once() {
    let on = PROFILE.replace(
        "commands = [\"sit\"]\n",
        "commands = [\"sit\"]\nwaggle = true\n",
    );
    let mut h = Hunt::new(Profile::parse(&on).unwrap(), 1).with_waggle(WaggleProfile::default());
    assert_eq!(
        tick(&mut h, 10),
        Said::Walk(RoomId(20)),
        "encumbered: to rest"
    );
    assert_eq!(
        tick(&mut h, 20),
        Said::Waggle(Vec::new()),
        "the waggle first"
    );
    let after = tick(&mut h, 20);
    assert_eq!(
        after,
        Said::Send {
            line: "sit".to_owned(),
            target: None
        },
        "then the rest commands, and no second waggle"
    );

    let mut off =
        Hunt::new(Profile::parse(PROFILE).unwrap(), 1).with_waggle(WaggleProfile::default());
    tick(&mut off, 10);
    assert_ne!(
        tick(&mut off, 20),
        Said::Waggle(Vec::new()),
        "not asked for"
    );
}

#[test]
fn each_script_is_named_and_a_resting_ewaggle_turns_the_rest_waggle_on() {
    let brought = import(
        "t",
        "resting_scripts: ewaggle, eherbs, mystery\nhunting_scripts: spellactive, reaction\n",
    )
    .unwrap();
    assert!(brought.profile.rest.waggle);
    let notes = brought.notes.join("\n");
    for expected in [
        "resting_scripts: `ewaggle` is built in",
        "resting_scripts: `eherbs` is built in",
        "resting_scripts: `mystery` is a Lich script",
        "hunting_scripts: `spellactive` is built in",
        "hunting_scripts: `reaction` is a Lich script",
    ] {
        assert!(notes.contains(expected), "{expected}\n{notes}");
    }
    let hunting = import("t", "hunting_scripts: ewaggle\n").unwrap();
    assert!(
        !hunting.profile.rest.waggle,
        "a hunting waggle is not the rest's"
    );
}

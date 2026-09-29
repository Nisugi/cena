//! A rest taken for the weight (`rest.encumbered`), for every profile.
//!
//! bigshot rests when `Char.percent_encumbrance >= @ENCUMBERED`
//! (`ready_to_rest?`) and will not leave while it still is
//! (`ready_to_hunt?`). The rest held for it only on a profile with
//! `rooms.allowed`, so any other rested, left at once, and rested again, each
//! tick (the crate review of 2026-09-28).

use cena_behavior::hunt::engine::{Phase, Why};
use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::{Frame, GameState, Runs};

/// No `rooms.allowed`: a profile as bigshot's importer writes one.
const PROFILE: &str = "targets = [{ any = true, routine = \"a\" }]\n[rooms]\nhunting = 10\nresting = 20\n[rest]\nencumbered = 40\ncommands = [\"sit\"]\n[routines]\na = [\"attack\"]\n";

/// Standing in `room` at second 1000 carrying `weight` percent, nothing here.
fn standing(room: u32, weight: u32) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.room.id = Some(room.to_string());
    state.status.set("standing", true);
    state.character.encumbrance_percent = Some(weight);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    state
}

fn tick(hunt: &mut Hunt, room: u32, weight: u32) -> Said {
    let state = standing(room, weight);
    let here = Here {
        room: Some(RoomId(room)),
        exits: &[],
        tags: &[],
    };
    hunt.tick(&state, here, state.game_time_now())
}

#[test]
fn a_rest_for_the_weight_holds_until_it_is_off() {
    let mut hunt = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    assert_eq!(tick(&mut hunt, 10, 50), Said::Walk(RoomId(20)));
    assert_eq!(hunt.phase(), Phase::ToRest(Why::Encumbered));
    assert_eq!(
        tick(&mut hunt, 20, 50),
        Said::Send {
            line: "sit".to_owned(),
            target: None
        }
    );
    assert!(
        matches!(tick(&mut hunt, 20, 50), Said::Wait(_)),
        "still carrying half: it stays"
    );
    assert!(matches!(hunt.phase(), Phase::Resting(_)));
    assert_eq!(
        tick(&mut hunt, 20, 10),
        Said::Walk(RoomId(10)),
        "the weight is off: back to the hunt"
    );
}

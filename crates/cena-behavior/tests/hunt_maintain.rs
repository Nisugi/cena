//! Maintain (`hunt/maintain.rs`): Barkskin's lockout after it absorbs,
//! which no dialog lists (the author's `cab.lic:81-88`).

use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::{Frame, GameState, Runs};

/// Game second `second`, standing alone in room 10, with the Active Spells
/// dialog stated and Barkskin not in it.
fn quiet(second: u32) -> GameState {
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
    state.effects.clear_category("Active Spells");
    state
}

fn hunt() -> Result<Hunt, String> {
    let profile = Profile::parse(
        r#"
signs = ["605"]
[rooms]
hunting = 10
"#,
    )?;
    Ok(Hunt::new(profile, 1))
}

/// What the hunt sends at game second `second`, if a line.
fn sends(hunt: &mut Hunt, second: u32) -> Option<String> {
    let state = quiet(second);
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    match hunt.tick(&state, here, state.game_time_now()) {
        Said::Send { line, .. } => Some(line),
        _ => None,
    }
}

#[test]
fn barkskin_waits_out_its_lockout_after_it_absorbs() {
    let mut hunt = hunt().unwrap();
    assert_eq!(sends(&mut hunt, 1_000).as_deref(), Some("incant 605"));
    hunt.heard(
        "The layer of bark on you hardens and absorbs the attack!",
        Some(1_010),
    );
    // Past the ordinary retry, and still inside the 301 seconds.
    assert_eq!(sends(&mut hunt, 1_100), None, "locked out");
    assert_eq!(sends(&mut hunt, 1_310), None, "one second short");
    assert_eq!(sends(&mut hunt, 1_311).as_deref(), Some("incant 605"));
}

#[test]
fn without_the_absorb_it_is_the_ordinary_retry() {
    let mut hunt = hunt().unwrap();
    assert_eq!(sends(&mut hunt, 1_000).as_deref(), Some("incant 605"));
    assert_eq!(sends(&mut hunt, 1_059), None, "inside the retry");
    assert_eq!(sends(&mut hunt, 1_061).as_deref(), Some("incant 605"));
    // Magical energy starts the lockout as an attack does.
    hunt.heard(
        "The layer of bark on you hardens and absorbs the magical energy!",
        Some(1_061),
    );
    assert_eq!(sends(&mut hunt, 1_200), None);
}

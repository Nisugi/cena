//! Maintain (`hunt/maintain.rs`): Barkskin's lockout after it absorbs, which
//! no dialog lists (the author's `cab.lic:81-88`); a sign whose own cooldown is
//! listed; and `check_favor` for the Voln symbols.

use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::societies::voln;
use cena_session::{Effect, Frame, GameState, Runs};

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

/// A hunt keeping these signs, with `check_favor` as given.
fn keeping(signs: &str, check_favor: bool) -> Result<Hunt, String> {
    let profile = Profile::parse(&format!(
        "signs = [{signs}]\ncheck_favor = {check_favor}\n[rooms]\nhunting = 10\n"
    ))?;
    Ok(Hunt::new(profile, 1))
}

/// What the hunt sends against `state`, if a line.
fn sends_against(hunt: &mut Hunt, state: &GameState) -> Option<String> {
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    match hunt.tick(state, here, state.game_time_now()) {
        Said::Send { line, .. } => Some(line),
        _ => None,
    }
}

#[test]
fn a_sign_whose_cooldown_is_listed_waits() {
    let mut state = quiet(1_000);
    state.effects.clear_category("Cooldowns");
    state.effects.insert(
        "c".to_owned(),
        Effect {
            category: "Cooldowns".to_owned(),
            text: "Wall of Force".to_owned(),
            ends_at: Some(1_100),
            percent: 50,
        },
    );
    let mut hunt = keeping("\"140\"", false).unwrap();
    assert_eq!(sends_against(&mut hunt, &state), None, "140 is cooling");
    let mut free = keeping("\"140\"", false).unwrap();
    assert_eq!(
        sends_against(&mut free, &quiet(1_000)).as_deref(),
        Some("incant 140")
    );
}

#[test]
fn check_favor_casts_a_symbol_only_with_the_favor_for_it() {
    let courage = voln::symbol("courage").unwrap();
    let cost = voln::favor_cost(&courage.cost, 40).unwrap();
    let with_favor = |favor: Option<i64>| {
        let mut state = quiet(1_000);
        state.character.experience.level = Some("40".to_owned());
        state.character.currency.voln_favor = favor;
        state
    };
    let mut short = keeping("\"9805\"", true).unwrap();
    let have = i64::from(cost) - 1;
    assert_eq!(sends_against(&mut short, &with_favor(Some(have))), None);
    let mut enough = keeping("\"9805\"", true).unwrap();
    assert_eq!(
        sends_against(&mut enough, &with_favor(Some(i64::from(cost)))).as_deref(),
        Some("incant 9805")
    );
    let mut unknown = keeping("\"9805\"", true).unwrap();
    assert_eq!(
        sends_against(&mut unknown, &with_favor(None)),
        None,
        "favor unknown"
    );
    let mut unchecked = keeping("\"9805\"", false).unwrap();
    assert_eq!(
        sends_against(&mut unchecked, &with_favor(None)).as_deref(),
        Some("incant 9805")
    );
}

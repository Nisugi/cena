//! `censer_between_actions` (`plan/33` §2h): Ethereal Censer before a
//! routine step whenever it is off cooldown and affordable.

use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::{Amount, Effect, Frame, GameState, Link, LinkKind, ProgressBar, Run, Runs};

/// A link to `id` with this `noun`, named `text`.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn linked(text: &str, id: &str, noun: &str, bold: bool) -> Run {
    let mut run = Run {
        text: text.to_owned(),
        style: Default::default(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: id.to_owned(),
                noun: noun.to_owned(),
            },
            text: text.to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    if bold {
        run.style.bold_depth = 1;
    }
    run
}

/// Game second `second`, standing in room 10 with a mastodon targeted,
/// `mana` points of 200, and Ethereal Censer known when `knows`.
fn fighting(second: u32, mana: i32, knows: bool) -> GameState {
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
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs {
            runs: vec![linked("mastodon", "42", "mastodon", true)],
        },
    });
    state.apply(&Frame::CreatureStatus {
        id: "42".into(),
        attrs: vec![
            ("exist".to_owned(), "42".to_owned()),
            ("hostile".to_owned(), "1".to_owned()),
        ],
    });
    state.targeting.read("#42", None);
    state.apply(&Frame::ProgressBar(ProgressBar {
        id: "mana".to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent: 50,
        text: format!("mana {mana}/200"),
        amount: Some(Amount {
            current: mana,
            max: 200,
        }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
    state.known_spells.begin();
    if knows {
        state.known_spells.read_line(&Runs {
            runs: vec![linked("Ethereal Censer", "x", "320", false)],
        });
    }
    state
}

fn hunt() -> Result<Hunt, String> {
    with_step("fire")
}

/// A hunt with the censer on, whose routine is this one step.
fn with_step(step: &str) -> Result<Hunt, String> {
    let profile = Profile::parse(&format!(
        r#"
censer_between_actions = true
targets = [{{ any = true, routine = "a" }}]
[rooms]
hunting = 10
[routines]
a = ["{step}"]
"#
    ))?;
    Ok(Hunt::new(profile, 1))
}

/// What the hunt sends on this tick, if a line.
fn sends(hunt: &mut Hunt, state: &GameState) -> Option<String> {
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
fn the_censer_goes_first_then_the_step_it_waited_for() {
    let mut hunt = hunt().unwrap();
    let state = fighting(1_000, 150, true);
    assert_eq!(sends(&mut hunt, &state).as_deref(), Some("incant 320"));
    assert_eq!(
        sends(&mut hunt, &state).as_deref(),
        Some("fire #42"),
        "not again until the retry is out"
    );
    // Eleven seconds on, and the game has not listed the cooldown: again.
    let later = fighting(1_011, 150, true);
    assert_eq!(sends(&mut hunt, &later).as_deref(), Some("incant 320"));
}

#[test]
fn not_while_its_cooldown_is_up() {
    let mut hunt = hunt().unwrap();
    let mut state = fighting(1_000, 150, true);
    state.effects.clear_category("Cooldowns");
    state.effects.insert(
        "c".to_owned(),
        Effect {
            category: "Cooldowns".to_owned(),
            text: "Ethereal Censer".to_owned(),
            ends_at: Some(1_100),
            percent: 50,
        },
    );
    assert_eq!(sends(&mut hunt, &state).as_deref(), Some("fire #42"));
}

#[test]
fn not_unless_it_is_known_and_affordable() {
    let mut unknown = hunt().unwrap();
    assert_eq!(
        sends(&mut unknown, &fighting(1_000, 150, false)).as_deref(),
        Some("fire #42")
    );
    // 320 costs nothing (Lich's `spell_extras.tsv`, `mana0`, and the wiki,
    // `reference/wiki_clean/Ethereal Censer _320_.txt:14`), so the mana
    // must cover only the step's own spell: 608 is 8.
    let mut poor = with_step("incant 608").unwrap();
    assert_eq!(
        sends(&mut poor, &fighting(1_000, 5, true)).as_deref(),
        None,
        "5 mana covers neither: no censer, and no 608, which bigshot's cmd_spell does not cast unaffordable"
    );
    let mut enough = with_step("incant 608").unwrap();
    assert_eq!(
        sends(&mut enough, &fighting(1_000, 10, true)).as_deref(),
        Some("incant 320")
    );
}

#[test]
fn off_unless_the_profile_asks() {
    let profile = Profile::parse(
        r#"
targets = [{ any = true, routine = "a" }]
[rooms]
hunting = 10
[routines]
a = ["fire"]
"#,
    )
    .unwrap();
    let mut hunt = Hunt::new(profile, 1);
    assert_eq!(
        sends(&mut hunt, &fighting(1_000, 150, true)).as_deref(),
        Some("fire #42")
    );
}

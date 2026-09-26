//! The room itself: see the module above for what it is for. Here, not in
//! `mod.rs`, because a `mod.rs` stays a facade (`cena-arch-tests`,
//! `facade_files_stay_facades`).

use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::{Amount, Effect, Frame, GameState, Link, LinkKind, ProgressBar, Run, Runs};

/// A bold creature link.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
pub fn linked(noun: &str) -> Run {
    let mut run = Run {
        text: noun.to_owned(),
        style: Default::default(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: "42".to_owned(),
                noun: noun.to_owned(),
            },
            text: noun.to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    run
}

/// Room 10 at `second`, creature #42 (`noun`) targeted, with `attrs` on
/// its status.
pub fn fighting(second: u32, noun: &str, attrs: &[(&str, &str)]) -> GameState {
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
            runs: vec![linked(noun)],
        },
    });
    let mut all = vec![
        ("exist".to_owned(), "42".to_owned()),
        ("hostile".to_owned(), "1".to_owned()),
    ];
    all.extend(
        attrs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
    );
    state.apply(&Frame::CreatureStatus {
        id: "42".into(),
        attrs: all,
    });
    state.targeting.read("#42", None);
    state
}

pub fn kobold(second: u32) -> GameState {
    fighting(second, "kobold", &[])
}

/// `state` with the stamina bar at `points` of 100.
pub fn stamina(mut state: GameState, points: i32) -> GameState {
    state.apply(&Frame::ProgressBar(ProgressBar {
        id: "stamina".to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent: 100,
        text: format!("stamina {points}/100"),
        amount: Some(Amount {
            current: points,
            max: 100,
        }),
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
    state
}

/// `state` with an effect `text` (id `id`) up in `dialog` until `ends`.
pub fn with(mut state: GameState, dialog: &str, id: &str, text: &str, ends: u32) -> GameState {
    state.effects.clear_category(dialog);
    state.effects.insert(
        id.to_owned(),
        Effect {
            category: dialog.to_owned(),
            text: text.to_owned(),
            ends_at: Some(ends),
            percent: 50,
        },
    );
    state
}

pub fn hunt(routine: &[&str]) -> Result<Hunt, String> {
    let steps: Vec<String> = routine.iter().map(|s| format!("{s:?}")).collect();
    Ok(Hunt::new(
        Profile::parse(&format!(
            "targets = [{{ any = true, routine = \"a\" }}]\n[rooms]\nhunting = 10\n[routines]\na = [{}]\n",
            steps.join(", ")
        ))?,
        1,
    ))
}

pub fn tick(hunt: &mut Hunt, state: &GameState) -> String {
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    match hunt.tick(state, here, state.game_time_now()) {
        Said::Send { line, .. } => line,
        Said::Wait(n) => format!("wait {n}"),
        _ => "nothing".to_owned(),
    }
}

pub fn ticks(hunt: &mut Hunt, state: &GameState, n: usize) -> Vec<String> {
    (0..n).map(|_| tick(hunt, state)).collect()
}

/// The first line a routine of `step` sends in `state`.
pub fn first(step: &str, state: &GameState) -> String {
    hunt(&[step]).map_or_else(|e| e, |mut h| tick(&mut h, state))
}

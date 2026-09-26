//! Steps that run more than once or carry a buff before them
//! (`hunt/repeat.rs`, `hunt/verbs.rs`): `eachtarget`, `force`, `resonance`,
//! and `celerity`, `slayer` and `tonis` before a step.

use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::{Effect, Frame, GameState, Link, LinkKind, Run, Runs};

/// A link to `id`, bold for a creature.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn linked(text: &str, id: &str) -> Run {
    let mut run = Run {
        text: text.to_owned(),
        style: Default::default(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: id.to_owned(),
                noun: text.to_owned(),
            },
            text: text.to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    run
}

/// Standing in room 10 at `second`, mastodon #42 targeted and kobold #43
/// beside it, #43 immobile when `frozen`.
fn fighting(second: u32, frozen: bool) -> GameState {
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
            runs: vec![linked("mastodon", "42"), linked("kobold", "43")],
        },
    });
    for (id, still) in [("42", false), ("43", frozen)] {
        let mut attrs = vec![
            ("exist".to_owned(), id.to_owned()),
            ("hostile".to_owned(), "1".to_owned()),
        ];
        if still {
            attrs.push(("immobile".to_owned(), "1".to_owned()));
        }
        state.apply(&Frame::CreatureStatus {
            id: id.into(),
            attrs,
        });
    }
    state.targeting.read("#42", None);
    state
}

fn hunt(routine: &[&str]) -> Result<Hunt, String> {
    let steps: Vec<String> = routine.iter().map(|s| format!("{s:?}")).collect();
    let profile = Profile::parse(&format!(
        "targets = [{{ any = true, routine = \"a\" }}]\n[rooms]\nhunting = 10\n[routines]\na = [{}]\n",
        steps.join(", ")
    ))?;
    Ok(Hunt::new(profile, 7))
}

/// What one tick does: a line, `wait N`, or `nothing`.
fn tick(hunt: &mut Hunt, state: &GameState) -> String {
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

fn ticks(hunt: &mut Hunt, state: &GameState, n: usize) -> Vec<String> {
    (0..n).map(|_| tick(hunt, state)).collect()
}

#[test]
fn eachtarget_targets_each_creature_and_runs_the_step_against_it() {
    let mut h = hunt(&["eachtarget fire", "kick"]).unwrap();
    assert_eq!(
        ticks(&mut h, &fighting(1_000, false), 4),
        ["fire #42", "target #43", "fire #43", "kick"],
        "the game's own target first, then the other; then the routine goes on"
    );
}

#[test]
fn a_swept_steps_guards_are_read_against_each_creature() {
    let mut h = hunt(&["eachtarget fire (!immobilized)", "kick"]).unwrap();
    assert_eq!(
        ticks(&mut h, &fighting(1_000, true), 3),
        ["fire #42", "target #43", "kick"],
        "the kobold is targeted, as bigshot targets first, and not fired at"
    );
}

#[test]
fn a_swept_spell_sends_all_its_lines_before_the_next_creature() {
    let mut h = hunt(&["eachtarget incant 1106"]).unwrap();
    assert_eq!(
        ticks(&mut h, &fighting(1_000, false), 5),
        [
            "prepare 1106",
            "cast #42",
            "target #43",
            "prepare 1106",
            "cast #43"
        ]
    );
}

#[test]
fn force_repeats_the_step_until_its_result_reaches_the_goal() {
    let state = fighting(1_000, false);
    let mut h = hunt(&["force kick till 100", "punch"]).unwrap();
    assert_eq!(tick(&mut h, &state), "kick");
    h.replied(
        [
            "You kick at a mastodon!",
            "[SMR result: 90 (Open d100: 40)]",
        ],
        Some(1_000),
    );
    assert_eq!(tick(&mut h, &state), "kick", "90 is short of 100");
    // Somebody else's roll is not the character's.
    h.replied(
        [
            "A kobold kicks at you!",
            "[SMR result: 150 (Open d100: 90)]",
        ],
        Some(1_001),
    );
    assert_eq!(tick(&mut h, &state), "kick");
    h.replied(
        [
            "You kick at a mastodon!",
            "[SMR result: 120 (Open d100: 70)]",
        ],
        Some(1_002),
    );
    assert_eq!(tick(&mut h, &state), "punch", "120 reached it");
}

#[test]
fn a_forced_spell_reads_its_warding_result_and_a_failure_ends_a_force() {
    let state = fighting(1_000, false);
    let mut h = hunt(&["force 1106 until 150", "punch"]).unwrap();
    assert_eq!(ticks(&mut h, &state, 2), ["prepare 1106", "cast #42"]);
    h.replied(
        [
            "You gesture at a mastodon.",
            "  CS: +567 - TD: +471 + CvA: +25 + d100: +72 == +193",
        ],
        Some(1_000),
    );
    assert_eq!(tick(&mut h, &state), "punch");

    let mut tired = hunt(&["force kick till 300", "punch"]).unwrap();
    assert_eq!(tick(&mut tired, &state), "kick");
    tired.replied(
        ["You do not have enough stamina to attempt this maneuver."],
        Some(1_000),
    );
    assert_eq!(tick(&mut tired, &state), "punch");
}

#[test]
fn a_force_gives_up_after_thirty_seconds() {
    let mut h = hunt(&["force kick till 300", "punch"]).unwrap();
    assert_eq!(tick(&mut h, &fighting(1_000, false)), "kick");
    assert_eq!(tick(&mut h, &fighting(1_030, false)), "kick");
    assert_eq!(tick(&mut h, &fighting(1_031, false)), "punch");
}

#[test]
fn resonance_never_casts_the_same_spell_twice_running() {
    let mut h = hunt(&["resonance 1030 1040 1050"]).unwrap();
    let lines = ticks(&mut h, &fighting(1_000, false), 12);
    for pair in lines.windows(2) {
        assert_ne!(pair[0], pair[1], "{lines:?}");
    }
    for line in &lines {
        assert!(
            ["incant 1030", "incant 1040", "incant 1050"].contains(&line.as_str()),
            "{lines:?}"
        );
    }
}

/// `fighting`, with the spell list seen and `(spell, seconds left)` up.
fn with_spells(up: &[(u16, u32)]) -> GameState {
    let mut state = fighting(1_000, false);
    state.effects.clear_category("Active Spells");
    for (spell, left) in up {
        state.effects.insert(
            spell.to_string(),
            Effect {
                category: "Active Spells".to_owned(),
                text: String::new(),
                ends_at: Some(1_000 + left),
                percent: 50,
            },
        );
    }
    state
}

#[test]
fn a_buff_named_before_a_step_goes_up_first_when_it_is_down_or_lapsing() {
    let first_two = |step: &str, state: &GameState| -> Vec<String> {
        let mut h = hunt(&[step]).unwrap();
        ticks(&mut h, state, 2)
    };
    assert_eq!(
        first_two("celerity fire", &with_spells(&[])),
        ["incant 506", "fire #42"]
    );
    assert_eq!(
        first_two("haste fire", &with_spells(&[(506, 2)])),
        ["fire #42", "fire #42"],
        "Celerity is not cast while it is up at all"
    );
    assert_eq!(
        first_two("slayer fire", &with_spells(&[(240, 2)])),
        ["incant 240", "fire #42"],
        "Spirit Slayer with three seconds or less is cast again"
    );
    assert_eq!(
        first_two("1035 fire", &with_spells(&[(1035, 60)])),
        ["fire #42", "fire #42"]
    );
    assert_eq!(
        first_two("tonis fire", &fighting(1_000, false)),
        ["fire #42", "fire #42"],
        "no spell list seen: nothing is known down"
    );
}

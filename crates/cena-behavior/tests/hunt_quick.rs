//! Quick hunting (`hunt/quick.rs`, bigshot's `;bigshot quick`): this room,
//! until it is clear.

use cena_behavior::hunt::command::{Command, parse};
use cena_behavior::hunt::{Ending, Here, Hunt, Profile, Said, import};
use cena_map::RoomId;
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs};

const PROFILE: &str = r#"
targets = [{ name = "troll", routine = "b" }]
[rooms]
hunting = 10
resting = 20
[rest]
encumbered = 1
[routines]
a = ["attack"]
b = ["kick"]
quick = ["fire"]
"#;

/// A link to `id`, bold for a creature.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn linked(text: &str, id: &str, bold: bool) -> Run {
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
    if bold {
        run.style.bold_depth = 1;
    }
    run
}

/// Room 10 at second 1000, heavily encumbered, with a stranger present
/// and, when `kobold`, a hostile kobold #42 targeted.
fn room(kobold: bool) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.room.id = Some("10".to_owned());
    state.status.set("standing", true);
    state.character.encumbrance_percent = Some(50);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs {
            runs: vec![linked("Stranger", "-5", false)],
        },
    });
    if kobold {
        state.apply(&Frame::Component {
            id: "room objs".into(),
            body: Runs {
                runs: vec![linked("kobold", "42", true)],
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
    }
    state
}

fn tick(hunt: &mut Hunt, state: &GameState) -> Said {
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[RoomId(11)],
        tags: &[],
    };
    hunt.tick(state, here, state.game_time_now())
}

#[test]
fn quick_fights_any_creature_here_on_its_routine_whoever_else_is_here() {
    let mut quick = Hunt::new(Profile::parse(PROFILE).unwrap(), 1).quick();
    assert_eq!(
        tick(&mut quick, &room(true)),
        Said::Send {
            line: "fire #42".to_owned(),
            target: Some(42)
        },
        "the quick routine, a stranger here and encumbrance past the rest threshold"
    );
    // The same profile hunting normally: the stranger's room is not
    // fought in, and the encumbrance sends it to rest.
    let mut plain = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    assert!(
        !matches!(tick(&mut plain, &room(true)), Said::Send { ref line, .. } if line == "fire #42"),
        "a normal hunt does not fire here"
    );
}

#[test]
fn quick_ends_when_the_room_is_clear() {
    let mut quick = Hunt::new(Profile::parse(PROFILE).unwrap(), 1).quick();
    assert_eq!(tick(&mut quick, &room(false)), Said::Done(Ending::Cleared));
}

#[test]
fn without_quick_commands_the_quick_routine_is_a() {
    let profile = PROFILE.replace("quick = [\"fire\"]\n", "");
    let mut quick = Hunt::new(Profile::parse(&profile).unwrap(), 1).quick();
    assert_eq!(
        tick(&mut quick, &room(true)),
        Said::Send {
            line: "attack".to_owned(),
            target: Some(42)
        }
    );
}

#[test]
fn hunt_name_quick_is_asked_for_and_bigshots_keys_come_across() {
    assert_eq!(
        parse("hunt ojandhaart quick"),
        Some(Ok(Command::Quick("ojandhaart".to_owned())))
    );
    let brought = import(
        "t",
        "quickhunt_targets: troll(b), kobold\nquick_commands: fire, hide\n",
    )
    .unwrap();
    let p = &brought.profile;
    let targets: Vec<(Option<&str>, &str)> = p
        .quick_targets
        .iter()
        .map(|t| (t.name.as_deref(), t.routine.as_str()))
        .collect();
    assert_eq!(targets, [(Some("troll"), "b"), (Some("kobold"), "quick")]);
    let quick: Vec<String> = p.routines["quick"]
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(quick, ["fire", "hide"]);
}

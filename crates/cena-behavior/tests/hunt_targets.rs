//! Which creatures the hunt fights (`hunt/targets.rs`), reading the model's
//! creature facts (`inventory/12` §2): nobody's companion, no hazard under
//! an `any` target, and a kill that left no corpse still counted.
//!
//! The kill line is synthetic, from Implosion's own pattern as the model's
//! tests have it (`crates/cena-model/tests/creature_facts.rs`); no committed
//! fixture carries one.

use cena_behavior::hunt::engine::{Phase, Why};
use cena_behavior::hunt::{Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs, TextFrame};

fn link(id: &str, noun: &str, text: &str) -> Link {
    Link {
        kind: LinkKind::Exist {
            id: id.to_owned(),
            noun: noun.to_owned(),
        },
        text: text.to_owned(),
        coord: None,
    }
}

/// A bold creature link in the room list.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn creature(id: &str, noun: &str, name: &str) -> Run {
    let mut run = Run {
        text: name.to_owned(),
        style: Default::default(),
        link: Some(link(id, noun, name)),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    run
}

/// Room 10 at second 1000 with these creatures, `hostile` ones flagged so.
fn room(creatures: &[(&str, &str, &str, bool)]) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
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
            runs: creatures
                .iter()
                .map(|(id, noun, name, _)| creature(id, noun, name))
                .collect(),
        },
    });
    for (id, _, _, hostile) in creatures {
        if *hostile {
            state.apply(&Frame::CreatureStatus {
                id: (*id).into(),
                attrs: vec![
                    ("exist".to_owned(), (*id).to_owned()),
                    ("hostile".to_owned(), "1".to_owned()),
                ],
            });
        }
    }
    state
}

fn hunt(targets: &str, extra: &str) -> Result<Hunt, String> {
    Ok(Hunt::new(
        Profile::parse(&format!(
            "targets = [{targets}]\n[rooms]\nhunting = 10\nresting = 20\n[routines]\na = [\"attack\"]\n{extra}"
        ))?,
        1,
    ))
}

fn tick(hunt: &mut Hunt, state: &GameState) -> Said {
    let here = Here {
        room: Some(RoomId(10)),
        exits: &[],
        tags: &[],
    };
    hunt.tick(state, here, state.game_time_now())
}

fn sent(said: &Said) -> Option<&str> {
    match said {
        Said::Send { line, .. } => Some(line),
        _ => None,
    }
}

#[test]
fn a_companion_is_never_fought_even_by_an_any_target() {
    let any = "{ any = true, routine = \"a\" }";
    let mut h = hunt(any, "").unwrap();
    // A snowcat in the room that no status has described, known as a
    // creature from a line that named it: the game has not said it is not
    // hostile, and its name says whose it is.
    let mut pet = room(&[("7", "snowcat", "peak snowcat", false)]);
    say(
        &mut pet,
        "You blinded a ",
        ("7", "snowcat", "peak snowcat"),
        "!",
    );
    let known: Vec<i64> = pet.creatures().in_room().map(|c| c.id).collect();
    assert_eq!(known, [7], "the input reaches the check");
    assert!(sent(&tick(&mut h, &pet)).is_none());
    let mut h = hunt(any, "").unwrap();
    let both = room(&[
        ("7", "snowcat", "peak snowcat", false),
        ("42", "kobold", "kobold", true),
    ]);
    assert_eq!(sent(&tick(&mut h, &both)), Some("target #42"));
}

#[test]
fn a_wasp_nest_is_no_any_targets_but_a_target_that_names_it_takes_it() {
    let nest = room(&[("9", "nest", "wasp nest", true)]);
    let mut any = hunt("{ any = true, routine = \"a\" }", "").unwrap();
    assert!(sent(&tick(&mut any, &nest)).is_none());
    let mut named = hunt(
        "{ name = \"nest\", routine = \"a\" }, { any = true, routine = \"a\" }",
        "",
    )
    .unwrap();
    assert_eq!(sent(&tick(&mut named, &nest)), Some("target #9"));
}

/// A main-window line naming creature `id` in bold, as the parser leaves
/// it, then a prompt.
#[expect(
    clippy::default_trait_access,
    reason = "the text's style type is not re-exported for behaviors"
)]
fn say(state: &mut GameState, before: &str, (id, noun, name): (&str, &str, &str), after: &str) {
    let parts = [
        (before, None),
        (name, Some(link(id, noun, name))),
        (after, None),
    ];
    let last = parts.len() - 1;
    for (at, (text, link)) in parts.into_iter().enumerate() {
        let mut frame = TextFrame {
            content: text.to_owned(),
            stream: String::new(),
            style: Default::default(),
            link,
            inner_link: None,
            ends_line: at == last,
        };
        if frame.link.is_some() {
            frame.style.bold_depth = 1;
        }
        state.apply(&Frame::Text(frame));
    }
    state.apply(&Frame::Prompt {
        time: "1001".into(),
        text: ">".into(),
    });
}

#[test]
fn a_kill_that_left_no_corpse_counts_toward_the_overkill_rest() {
    let rest = "[rest]\nfried = 90\noverkill = 1\n";
    let any = "{ any = true, routine = \"a\" }";
    let mut state = room(&[("42", "servant", "horned kobold servant", true)]);
    state.character.experience.mind_percent = Some(100);
    // Fried, but no kill yet: the hunt fights on.
    let mut h = hunt(any, rest).unwrap();
    assert_eq!(sent(&tick(&mut h, &state)), Some("target #42"));
    // Implosion's kill line (`killcounter.lic:223`, as the model reads it).
    say(
        &mut state,
        "Rather abrupt decompression causes a ",
        ("42", "servant", "horned kobold servant"),
        " to explode!",
    );
    // Counted where kills are counted, after the rest arm has spoken for
    // this tick: the rest follows at the next.
    tick(&mut h, &state);
    let after = tick(&mut h, &state);
    assert_eq!(h.phase(), Phase::ToRest(Why::Fried), "{after:?}");
}

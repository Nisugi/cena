//! Bounty mode (`hunt/bounty.rs`, bigshot's `;bigshot bounty`): hunt until
//! the bounty is done or a new one is ready, rest, and end. The bounty lines
//! are the model's own patterns (`crates/cena-model/src/state/bounty.rs`).

use cena_behavior::hunt::command::{Command, parse};
use cena_behavior::hunt::engine::{Phase, Why};
use cena_behavior::hunt::{Ending, Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::{Effect, Frame, GameState, Link, LinkKind, Run, Runs};

const PROFILE: &str = "targets = [{ any = true, routine = \"a\" }]\n[rooms]\nhunting = 10\nresting = 20\n[routines]\na = [\"attack\"]\n";

const DONE: &str = "You have succeeded in your task and can return to the Adventurer's Guild to receive your reward.";
const NONE: &str = "You are not currently assigned a task.";
const BANDITS: &str = "You have been tasked to suppress bandit activity on the Old Mine Road.  You need to kill 5 more of them to complete your task.";

/// Room `room` at second 1000, the mind at `mind` percent, the bounty as
/// `bounty` says, with a hostile `(id, noun, name)` when given.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn state(room: &str, mind: u32, bounty: &str, creature: Option<(&str, &str, &str)>) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
    state.room.id = Some(room.to_owned());
    state.status.set("standing", true);
    state.character.experience.mind_percent = Some(mind);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    let runs = creature
        .iter()
        .map(|(id, noun, name)| {
            let mut run = Run {
                text: (*name).to_owned(),
                style: Default::default(),
                link: Some(Link {
                    kind: LinkKind::Exist {
                        id: (*id).to_owned(),
                        noun: (*noun).to_owned(),
                    },
                    text: (*name).to_owned(),
                    coord: None,
                }),
                inner_link: None,
            };
            run.style.bold_depth = 1;
            run
        })
        .collect();
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs },
    });
    if let Some((id, _, _)) = creature {
        state.apply(&Frame::CreatureStatus {
            id: id.into(),
            attrs: vec![
                ("exist".to_owned(), id.to_owned()),
                ("hostile".to_owned(), "1".to_owned()),
            ],
        });
    }
    state.effects.clear_category("Cooldowns");
    assert!(state.bounty.read_line(bounty), "the model reads {bounty:?}");
    state
}

fn tick(hunt: &mut Hunt, state: &GameState, room: u32) -> Said {
    let here = Here {
        room: Some(RoomId(room)),
        exits: &[],
        tags: &[],
    };
    hunt.tick(state, here, state.game_time_now())
}

fn bounty_hunt() -> Hunt {
    Hunt::new(Profile::parse(PROFILE).unwrap_or_default(), 1).bounty()
}

#[test]
fn a_done_bounty_sends_the_hunt_to_rest_and_the_rest_ends_it() {
    let mut h = bounty_hunt();
    assert_eq!(
        tick(&mut h, &state("10", 50, DONE, None), 10),
        Said::Walk(RoomId(20))
    );
    assert_eq!(h.phase(), Phase::ToRest(Why::Bounty));
    let mut arrived = Said::Nothing;
    for _ in 0..5 {
        arrived = tick(&mut h, &state("20", 50, DONE, None), 20);
        if matches!(arrived, Said::Done(_)) {
            break;
        }
    }
    assert_eq!(arrived, Said::Done(Ending::Bounty));

    let mut plain = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    tick(&mut plain, &state("10", 50, DONE, None), 10);
    assert_eq!(plain.phase(), Phase::Hunting, "only in bounty mode");
}

#[test]
fn a_saturated_mind_keeps_hunting_and_a_new_bounty_waits_for_its_cooldown() {
    let mut full = bounty_hunt();
    tick(&mut full, &state("10", 100, DONE, None), 10);
    assert_eq!(full.phase(), Phase::Hunting, "saturated: learn first");

    let mut idle = bounty_hunt();
    tick(&mut idle, &state("10", 50, NONE, None), 10);
    assert_eq!(
        idle.phase(),
        Phase::ToRest(Why::Bounty),
        "a new one is ready"
    );

    let mut cooling = state("10", 50, NONE, None);
    cooling.effects.insert(
        "c".to_owned(),
        Effect {
            category: "Cooldowns".to_owned(),
            text: "Next Bounty".to_owned(),
            ends_at: Some(1_300),
            percent: 50,
        },
    );
    let mut waiting = bounty_hunt();
    tick(&mut waiting, &cooling, 10);
    assert_eq!(waiting.phase(), Phase::Hunting, "not yet");
}

#[test]
fn a_bandit_bounty_is_not_done_while_bandits_are_here_and_ignores_the_mind() {
    let bandit = Some(("50", "bandit", "human bandit"));
    let mut h = bounty_hunt();
    tick(&mut h, &state("10", 50, BANDITS, None), 10);
    assert_eq!(h.phase(), Phase::Hunting, "the task is still to do");
    tick(&mut h, &state("10", 100, DONE, bandit), 10);
    assert_eq!(h.phase(), Phase::Hunting, "a bandit is here");
    tick(&mut h, &state("10", 100, DONE, None), 10);
    assert_eq!(
        h.phase(),
        Phase::ToRest(Why::Bounty),
        "no bandit, and the mind is no matter on a bandit bounty"
    );
}

#[test]
fn hunt_name_bounty_is_asked_for() {
    assert_eq!(
        parse("hunt ojandhaart bounty"),
        Some(Ok(Command::Bounty("ojandhaart".to_owned())))
    );
}

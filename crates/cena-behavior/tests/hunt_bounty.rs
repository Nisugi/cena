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

const GEMS: &str = "The gem dealer in Wehnimer's Landing, Kylia, has received orders from multiple customers requesting a blue sapphire.  You have been tasked to retrieve 3 more of them.  You can SELL them to the gem dealer as you find them.";
const SKINS: &str = "You have been tasked to retrieve 4 wolf pelts of at least fair quality for Tamzin in Wehnimer's Landing.  You can SKIN them off the corpse of a wolf or purchase them from another adventurer.  You can SELL the skins to the furrier as you collect them.";

/// `state` with container #9 (`My Sack`) showing these `(id, noun, name)`.
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors"
)]
fn sack(state: &mut GameState, items: &[(&str, &str, &str)]) {
    state.apply(&Frame::Container {
        id: "9".to_owned(),
        title: Some("My Sack".to_owned()),
        target: Some("#9".to_owned()),
    });
    state.apply(&Frame::ClearContainer { id: "9".to_owned() });
    for (id, noun, name) in items {
        state.apply(&Frame::ContainerItem {
            container_id: "9".to_owned(),
            content: Runs {
                runs: vec![Run {
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
                }],
            },
        });
    }
}

fn sapphires(n: usize) -> Vec<(String, String, String)> {
    (0..n)
        .map(|i| {
            (
                format!("{}", 100 + i),
                "sapphire".to_owned(),
                "blue sapphire".to_owned(),
            )
        })
        .collect()
}

fn with_items(bounty: &str, items: &[(String, String, String)]) -> GameState {
    let mut s = state("10", 50, bounty, None);
    let refs: Vec<(&str, &str, &str)> = items
        .iter()
        .map(|(a, b, c)| (a.as_str(), b.as_str(), c.as_str()))
        .collect();
    sack(&mut s, &refs);
    s
}

#[test]
fn a_gem_bounty_is_done_when_the_containers_hold_enough() {
    let mut short = bounty_hunt();
    tick(&mut short, &with_items(GEMS, &sapphires(2)), 10);
    assert_eq!(short.phase(), Phase::Hunting, "two of three");
    let mut enough = bounty_hunt();
    tick(&mut enough, &with_items(GEMS, &sapphires(3)), 10);
    assert_eq!(enough.phase(), Phase::ToRest(Why::Bounty));
}

#[test]
fn a_skin_bounty_counts_loose_skins_or_a_measured_bundle() {
    let pelts: Vec<(String, String, String)> = (0..4)
        .map(|i| {
            (
                format!("{}", 200 + i),
                "pelt".to_owned(),
                "wolf pelt".to_owned(),
            )
        })
        .collect();
    let mut loose = bounty_hunt();
    tick(&mut loose, &with_items(SKINS, &pelts), 10);
    assert_eq!(
        loose.phase(),
        Phase::ToRest(Why::Bounty),
        "four loose pelts"
    );

    let bundle = vec![(
        "70".to_owned(),
        "bundle".to_owned(),
        "bundle of wolf pelts".to_owned(),
    )];
    let held = with_items(SKINS, &bundle);
    let mut h = bounty_hunt();
    assert_eq!(
        tick(&mut h, &held, 10),
        Said::Send {
            line: "measure #70".to_owned(),
            target: None
        }
    );
    h.replied(
        ["You glance through the bundle and count a total of 5 wolf pelts."],
        Some(1_000),
    );
    tick(&mut h, &held, 10);
    assert_eq!(h.phase(), Phase::ToRest(Why::Bounty), "five bundled");

    let mut thin = bounty_hunt();
    tick(&mut thin, &held, 10);
    thin.replied(
        ["You glance through the bundle and count a total of 2 wolf pelts."],
        Some(1_000),
    );
    assert_ne!(
        tick(&mut thin, &held, 10),
        Said::Send {
            line: "measure #70".to_owned(),
            target: None
        },
        "measured once"
    );
    assert_eq!(thin.phase(), Phase::Hunting, "two of four");

    let mut hidden = held.clone();
    hidden.status.set("hidden", true);
    let mut quiet = bounty_hunt();
    assert_ne!(
        tick(&mut quiet, &hidden, 10),
        Said::Send {
            line: "measure #70".to_owned(),
            target: None
        },
        "not measured while hidden"
    );
}

/// The worn list read, naming only `(id, noun, name)`.
#[expect(
    clippy::default_trait_access,
    reason = "the text's style type is not re-exported for behaviors"
)]
fn wearing(state: &mut GameState, (id, noun, name): (&str, &str, &str)) {
    use cena_session::TextFrame;
    state.apply(&Frame::StreamPush { id: "inv".into() });
    let line = |state: &mut GameState, parts: Vec<(String, Option<Link>)>| {
        let last = parts.len() - 1;
        for (at, (content, link)) in parts.into_iter().enumerate() {
            state.apply(&Frame::Text(TextFrame {
                content,
                stream: "inv".to_owned(),
                style: Default::default(),
                link,
                inner_link: None,
                ends_line: at == last,
            }));
        }
    };
    line(state, vec![("Your worn items are:".to_owned(), None)]);
    let item = Link {
        kind: LinkKind::Exist {
            id: id.to_owned(),
            noun: noun.to_owned(),
        },
        text: name.to_owned(),
        coord: None,
    };
    line(
        state,
        vec![("  ".to_owned(), None), (name.to_owned(), Some(item))],
    );
    state.apply(&Frame::Prompt {
        time: "1000".into(),
        text: ">".into(),
    });
}

#[test]
fn only_worn_containers_count_once_the_worn_list_is_read() {
    let mut worn = with_items(GEMS, &sapphires(3));
    wearing(&mut worn, ("9", "sack", "a leather sack"));
    let mut h = bounty_hunt();
    tick(&mut h, &worn, 10);
    assert_eq!(h.phase(), Phase::ToRest(Why::Bounty), "the sack is worn");

    let mut elsewhere = with_items(GEMS, &sapphires(3));
    wearing(&mut elsewhere, ("8", "cloak", "a wool cloak"));
    let mut h = bounty_hunt();
    tick(&mut h, &elsewhere, 10);
    assert_eq!(h.phase(), Phase::Hunting, "the sack is not worn");
}

#[test]
fn loose_skins_and_a_bundle_are_counted_apart_as_bigshot_counts_them() {
    let mixed = vec![
        ("200".to_owned(), "pelt".to_owned(), "wolf pelt".to_owned()),
        ("201".to_owned(), "pelt".to_owned(), "wolf pelt".to_owned()),
        (
            "70".to_owned(),
            "bundle".to_owned(),
            "bundle of wolf pelts".to_owned(),
        ),
    ];
    let held = with_items(SKINS, &mixed);
    let mut h = bounty_hunt();
    assert_eq!(
        tick(&mut h, &held, 10),
        Said::Send {
            line: "measure #70".to_owned(),
            target: None
        }
    );
    h.replied(
        ["You glance through the bundle and count a total of 2 wolf pelts."],
        Some(1_000),
    );
    tick(&mut h, &held, 10);
    assert_eq!(
        h.phase(),
        Phase::Hunting,
        "two loose and two bundled: neither count reaches four"
    );
}

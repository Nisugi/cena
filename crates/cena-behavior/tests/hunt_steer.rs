//! A hunt as an agent steers and reads it (`plan/35` §4, M7 steps 3b and
//! 3c): the author's hold, "defend, start nothing", and retreat, "the rest
//! room, then end" (`crates/cena-behavior/src/hunt/steer.rs`); and how it is
//! getting on, stalls included (`hunt/progress.rs`). Driven tick by tick with
//! no game; the fixtures are `hunt_engine.rs`'s.

use cena_behavior::hunt::engine::{Phase, Why};
use cena_behavior::hunt::{Ending, Here, Hunt, Profile, Said};
use cena_behavior::operation::Steering;
use cena_map::RoomId;
use cena_session::{Frame, GameState, Link, LinkKind, ProgressBar, Run, Runs};
use tokio_util::sync::CancellationToken;

const PROFILE: &str = r#"
prepare = ["ready weapon", "incant 515"]
signs = ["515"]
targets = [
  { name = "mastodon", routine = "b" },
  { any = true, routine = "a" },
]

[rooms]
hunting = 10
boundaries = [30]
resting = 20

[stance]
hunting = "offensive"
wander = "defensive"

[rest]
fried = 100
encumbered = 20
until = { experience = 90, mana = 50 }
when = { bleeding = true, health_at_most = 60 }
commands = ["store all"]

[flee]
count = 2

[loot]
delay = true

[wander]
wait = 3

[routines]
a = ["attack"]
b = ["fire (hidden)", "kweed (!expiring \"Tangleweed Vigor\" 5)", "volley", "coupdegrace (thp 20)", "fire"]

[sequences]
volley = ["store weapon", "weapon volley"]
"#;

/// The profile above. An error is a broken fixture, which every test unwraps
/// into a failure.
fn profile() -> Result<Profile, String> {
    Profile::parse(PROFILE)
}

/// A state at game second `second`, standing, in the game's room `room`.
fn state(second: u32, room: &str) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: second.to_string(),
        text: ">".into(),
    });
    state.room.id = Some(room.to_owned());
    state.status.set("standing", true);
    // The game states who is here with every room; a room whose players
    // were never stated cannot be claimed (`claim_room`).
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    state
}

/// A creature in the room, with these `<crtrStatus>` attributes, hostile
/// unless the attributes say otherwise (a status without `hostile="1"` is
/// a companion's, `hunt_replay.rs`).
#[expect(
    clippy::default_trait_access,
    reason = "the run's style type is not re-exported for behaviors; only its bold depth matters"
)]
fn creature(state: &mut GameState, id: i64, noun: &str, attrs: &[(&str, &str)]) {
    let mut run = Run {
        text: noun.to_owned(),
        style: Default::default(),
        link: Some(Link {
            kind: LinkKind::Exist {
                id: id.to_string(),
                noun: noun.to_owned(),
            },
            text: noun.to_owned(),
            coord: None,
        }),
        inner_link: None,
    };
    run.style.bold_depth = 1;
    // Every creature already here stays in the list the room re-states.
    let mut runs: Vec<Run> = state
        .creatures()
        .in_room()
        .map(|kept| {
            let mut kept_run = run.clone();
            kept_run.text.clone_from(&kept.name);
            kept_run.link = Some(Link {
                kind: LinkKind::Exist {
                    id: kept.id.to_string(),
                    noun: kept.noun.clone().unwrap_or_default(),
                },
                text: kept.name.clone(),
                coord: None,
            });
            kept_run
        })
        .collect();
    runs.push(run);
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs { runs },
    });
    let mut attrs: Vec<(String, String)> = attrs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    attrs.insert(0, ("exist".to_owned(), id.to_string()));
    if !attrs.iter().any(|(k, _)| k == "hostile") {
        attrs.push(("hostile".to_owned(), "1".to_owned()));
    }
    state.apply(&Frame::CreatureStatus {
        id: id.to_string(),
        attrs,
    });
}

fn here(room: u32, exits: &[RoomId]) -> Here<'_> {
    Here {
        room: Some(RoomId(room)),
        exits,
        tags: &[],
    }
}

fn send(line: &str, target: Option<i64>) -> Said {
    Said::Send {
        line: line.to_owned(),
        target,
    }
}

const NO_EXITS: &[RoomId] = &[];

/// A hunt fighting creature 42, targeted and in its stance.
fn fighting() -> Option<(Hunt, GameState)> {
    let mut hunt = Hunt::new(profile().ok()?, 1);
    let mut state = state(1_000, "10");
    creature(
        &mut state,
        42,
        "mastodon",
        &[("health", "50"), ("maxhealth", "100")],
    );
    let at = here(10, NO_EXITS);
    (hunt.tick(&state, at, Some(1_000)) == send("target #42", Some(42))).then_some(())?;
    state.targeting.read("#42", None);
    state.character.stance = Some("offensive".to_owned());
    state.character.stance_percent = Some(0);
    Some((hunt, state))
}

/// Held with nothing being fought: no target is chosen, and nothing else is
/// begun -- the same room that engages a creature unheld.
#[test]
fn a_held_hunt_begins_nothing() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = state(1_000, "10");
    creature(&mut state, 42, "mastodon", &[]);
    hunt.hold(true);
    assert_eq!(
        hunt.tick(&state, here(10, NO_EXITS), Some(1_000)),
        Said::Wait(1)
    );
    assert_eq!(hunt.target(), None, "no new target");
    hunt.hold(false);
    assert_eq!(
        hunt.tick(&state, here(10, NO_EXITS), Some(1_000)),
        send("target #42", Some(42)),
        "resumed, it engages"
    );

    // An empty room, past wander.wait: unheld it would walk on.
    let mut hunt = Hunt::new(profile().unwrap(), 7);
    let empty = state_in("10");
    let exits = [RoomId(11)];
    hunt.hold(true);
    for second in [1_000, 1_010] {
        assert_eq!(
            hunt.tick(&empty, here(10, &exits), Some(second)),
            Said::Wait(1),
            "no wander"
        );
    }
}

/// Held mid-fight: the creature already being fought is fought on; when it
/// dies, the next is not taken up.
#[test]
fn a_held_hunt_fights_on_and_takes_up_no_other() {
    let (mut hunt, mut state) = fighting().unwrap();
    hunt.hold(true);
    let at = here(10, NO_EXITS);
    assert_eq!(
        hunt.tick(&state, at, Some(1_000)),
        send("store weapon", Some(42)),
        "defended against"
    );
    creature(&mut state, 43, "mastodon", &[]);
    // 42 is killed: its hit points, as the game says them.
    state.apply(&Frame::CreatureStatus {
        id: "42".to_owned(),
        attrs: [
            ("exist", "42"),
            ("health", "0"),
            ("maxhealth", "100"),
            ("hostile", "1"),
        ]
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect(),
    });
    assert_eq!(
        hunt.tick(&state, at, Some(1_001)),
        Said::Wait(1),
        "{:?}",
        hunt.target()
    );
    assert_eq!(hunt.target(), None);
    assert_eq!(
        hunt.tick(&state, at, Some(1_002)),
        Said::Wait(1),
        "and no loot either"
    );
}

/// Held, it still keeps the character alive: it rests when hurt and flees a
/// crowd, as it always would.
#[test]
fn a_held_hunt_still_rests_and_flees() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut bleeding = state(1_000, "10");
    bleeding.status.set("bleeding", true);
    hunt.hold(true);
    assert_eq!(
        hunt.tick(&bleeding, here(10, NO_EXITS), Some(1_000)),
        Said::Walk(RoomId(20))
    );

    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut crowded = state(1_000, "10");
    for id in 1..=3 {
        creature(&mut crowded, id, "mastodon", &[]);
    }
    hunt.hold(true);
    let exits = [RoomId(11)];
    assert_eq!(
        hunt.tick(&crowded, here(10, &exits), Some(1_000)),
        Said::Walk(RoomId(11))
    );
}

/// A rest that finishes while held stays in the resting room; resumed, it
/// walks back to hunt. Driven through the run's own controls, read at each
/// tick by `heed`, as the driver does.
#[test]
fn a_rest_finished_while_held_waits_for_resume() {
    let steering = Steering::new(CancellationToken::new());
    let mut hunt = Hunt::new(profile().unwrap(), 1).steered_by(steering.clone());
    let mut state = state(1_000, "10");
    state.character.experience.mind_percent = Some(100);
    // Stated, as the game states it: an unstated weight holds a rest.
    state.character.encumbrance_percent = Some(0);
    hunt.heed();
    assert_eq!(
        hunt.tick(&state, here(10, NO_EXITS), Some(1_000)),
        Said::Walk(RoomId(20))
    );
    state.room.id = Some("20".to_owned());
    let at_rest = here(20, NO_EXITS);
    assert_eq!(
        hunt.tick(&state, at_rest, Some(1_100)),
        send("store all", None)
    );
    state.character.experience.mind_percent = Some(10);
    mana(&mut state);
    steering.hold();
    hunt.heed();
    assert_eq!(
        hunt.tick(&state, at_rest, Some(1_300)),
        Said::Wait(5),
        "rested, held"
    );
    assert!(matches!(hunt.phase(), Phase::Resting(_)));
    steering.resume();
    hunt.heed();
    assert_eq!(
        hunt.tick(&state, at_rest, Some(1_305)),
        Said::Walk(RoomId(10))
    );
    assert_eq!(hunt.phase(), Phase::Returning);
}

/// Retreat: the target dropped, the walk to the resting room, and the end
/// there, over a hold; without a resting room, the end at once.
#[test]
fn a_retreat_walks_to_the_resting_room_and_ends_there() {
    let (mut hunt, state) = fighting().unwrap();
    hunt.hold(true);
    hunt.retreat();
    assert_eq!(
        hunt.tick(&state, here(10, NO_EXITS), Some(1_000)),
        Said::Walk(RoomId(20))
    );
    assert_eq!(hunt.phase(), Phase::ToRest(Why::Retreat));
    assert_eq!(hunt.target(), None);
    assert!(
        hunt.take_notes()
            .iter()
            .any(|note| note.contains("retreating"))
    );
    let arrived = state_in("20");
    assert_eq!(
        hunt.tick(&arrived, here(20, NO_EXITS), Some(1_050)),
        Said::Done(Ending::Retreated),
        "no rest, no selling: it ends"
    );

    let mut profile = profile().unwrap();
    profile.rooms.resting = None;
    let mut hunt = Hunt::new(profile, 1);
    hunt.retreat();
    assert_eq!(
        hunt.tick(&state_in("10"), here(10, NO_EXITS), Some(1_000)),
        Said::Done(Ending::NoRestingRoom)
    );
}

/// A standing state in `room` at game second 1000, nobody else there.
fn state_in(room: &str) -> GameState {
    state(1_000, room)
}

/// The mana bar at 60%.
fn mana(state: &mut GameState) {
    state.apply(&Frame::ProgressBar(ProgressBar {
        id: "mana".to_owned(),
        dialog: None,
        percent: 60,
        text: "mana 60/100".to_owned(),
        amount: None,
        attrs: Vec::new(),
        time_remaining_secs: None,
    }));
}

/// Issue #19, point 6: a hunt that goes round empty rooms says so once
/// `STALL_AFTER` game seconds pass with nothing engaged, naming how many
/// rooms it searched; held it is not stalled, and an engagement ends it.
#[test]
fn a_hunt_says_when_it_gets_nowhere() {
    let mut hunt = Hunt::new(profile().unwrap(), 7);
    let exits = [RoomId(11)];
    hunt.tick(&state(1_000, "10"), here(10, &exits), Some(1_000));
    assert_eq!(hunt.progress(Some(1_000)).stalled, None);
    hunt.tick(&state(1_100, "11"), here(11, &exits), Some(1_100));
    let later = hunt.progress(Some(1_300));
    assert_eq!(
        later.stalled.as_deref(),
        Some("nothing engaged for 5 minutes, across 2 rooms")
    );
    assert_eq!(later.counts["rooms_searched"], 2);
    assert_eq!(later.doing, "hunting");
    hunt.hold(true);
    let held = hunt.progress(Some(1_300));
    assert_eq!((held.doing.as_str(), held.stalled), ("held: hunting", None));
    hunt.hold(false);
    let mut fight = state(1_310, "11");
    creature(&mut fight, 42, "mastodon", &[]);
    assert_eq!(
        hunt.tick(&fight, here(11, &exits), Some(1_310)),
        send("target #42", Some(42))
    );
    let engaged = hunt.progress(Some(1_700));
    assert_eq!(engaged.counts["engaged"], 1);
    assert_eq!(
        engaged.stalled.as_deref(),
        Some("nothing engaged for 6 minutes, across 0 rooms"),
        "counted from the engagement"
    );
    assert_eq!(hunt.progress(Some(1_320)).stalled, None);
}

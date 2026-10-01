//! The profile, the room and the tick's arguments (`super`).

use cena_behavior::hunt::{Here, Profile, Said};
use cena_map::RoomId;
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs};

pub const PROFILE: &str = r#"
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
pub fn profile() -> Result<Profile, String> {
    Profile::parse(PROFILE)
}

/// A state at game second `second`, standing, in the game's room `room`.
pub fn state(second: u32, room: &str) -> GameState {
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
pub fn creature(state: &mut GameState, id: i64, noun: &str, attrs: &[(&str, &str)]) {
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

pub fn here(room: u32, exits: &[RoomId]) -> Here<'_> {
    Here {
        room: Some(RoomId(room)),
        exits,
        tags: &[],
    }
}

pub fn send(line: &str, target: Option<i64>) -> Said {
    Said::Send {
        line: line.to_owned(),
        target,
    }
}

pub const NO_EXITS: &[RoomId] = &[];

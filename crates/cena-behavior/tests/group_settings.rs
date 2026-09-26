//! The `[group]` keys (`plan/39` Stage 5): what each does on the engine,
//! and bigshot's keys read by the importer. One test per key, each cited.

use cena_behavior::group::{Leading, Party, Report, Role, Rooms};
use cena_behavior::hunt::engine::Phase;
use cena_behavior::hunt::said::Why;
use cena_behavior::hunt::{Here, Hunt, Profile, Said, import};
use cena_map::RoomId;
use cena_session::group::{GroupEvent, Member};
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs, State};

const PROFILE: &str = r#"
targets = [{ any = true, routine = "a" }]

[rooms]
hunting = 10
resting = 20
rally = [12]

[rest]
fried = 100
until = { experience = 50 }
commands = ["store all"]
waypoints = [15]

[wander]
wait = 0

[routines]
a = ["attack"]
"#;

/// The profile above, with these lines added to its `[group]`.
fn with_group(lines: &str) -> Result<Profile, String> {
    Profile::parse(&format!("{PROFILE}\n[group]\n{lines}\n"))
}

fn state(second: u32) -> GameState {
    let mut state = GameState::default();
    state.apply(&Frame::Prompt {
        time: second.to_string(),
        text: ">".into(),
    });
    state.room.id = Some("1000".to_owned());
    state.status.set("standing", true);
    state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs { runs: Vec::new() },
    });
    state.character.experience.mind_percent = Some(0);
    state
}

fn member(noun: &str) -> Member {
    Member {
        id: format!("-10{}", noun.len()),
        noun: noun.to_owned(),
        text: noun.to_owned(),
    }
}

/// `state`, leading `follower`.
fn leading(mut state: GameState, follower: &str) -> GameState {
    state.group.apply(
        &GroupEvent::Listed {
            leading: true,
            members: vec![member(follower)],
        },
        None,
    );
    state
}

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

/// A warg in the room, hostile and alive.
fn warg(state: &mut GameState) {
    state.apply(&Frame::Component {
        id: "room objs".into(),
        body: Runs {
            runs: vec![linked("warg", "41", "warg", true)],
        },
    });
    state.apply(&Frame::CreatureStatus {
        id: "41".into(),
        attrs: vec![
            ("exist".to_owned(), "41".to_owned()),
            ("hostile".to_owned(), "1".to_owned()),
        ],
    });
}

fn here(room: u32) -> Here<'static> {
    Here {
        room: Some(RoomId(room)),
        exits: &[RoomId(11)],
        tags: &[],
    }
}

fn send(line: &str) -> Said {
    Said::Send {
        line: line.to_owned(),
        target: None,
    }
}

fn report(name: &str) -> Report {
    Report {
        name: name.to_owned(),
        link: State::Ready,
        room: Some(RoomId(10)),
        rest: None,
        unready: None,
        hindrance: None,
        grouped: true,
        health: Some(100),
        headroom: None,
        prepared: None,
        looted: Vec::new(),
        dropped: None,
    }
}

fn led(followers: Vec<Report>) -> Party {
    Party {
        name: "Ashryn".to_owned(),
        role: Role::Lead,
        leader: "Ashryn".to_owned(),
        leading: None,
        followers,
        musters: Vec::new(),
        dropped: false,
        awaiting: Vec::new(),
        leader_lost: None,
        recoverer: None,
    }
}

fn followed(leading: Leading) -> Party {
    Party {
        name: "Kiyna".to_owned(),
        role: Role::Follow,
        leader: "Ashryn".to_owned(),
        leading: Some(leading),
        followers: Vec::new(),
        musters: Vec::new(),
        dropped: false,
        awaiting: Vec::new(),
        leader_lost: None,
        recoverer: None,
    }
}

// --- the importer ----------------------------------------------------------------

/// bigshot's "MA Grouping" keys, `quiet_followers`, `troubadours_rally` and
/// `disable_commands` (`bigshot.lic:3470-3551`) land in `[group]`; a
/// `group_deader` is said to be replaced (`plan/39` §8, question 10).
#[test]
fn bigshots_group_keys_import() {
    let yaml = "---
hunting_room_id: '10'
ma_looter: Kiyna
never_loot: Dicate, Zeta
random_loot: true
final_loot: true
independent_travel: true
independent_return: true
group_deader: true
quiet_followers: false
troubadours_rally: true
disable_commands: hide, ambush
";
    let brought = import("group", yaml).unwrap();
    let table = &brought.profile.group;
    assert_eq!(table.ma_looter.as_deref(), Some("Kiyna"));
    assert_eq!(table.never_loot, ["Dicate", "Zeta"]);
    assert!(table.random_loot && table.final_loot);
    assert!(table.independent_travel && table.independent_return);
    assert!(!table.quiet_followers);
    assert!(table.troubadours_rally);
    let disabled: Vec<String> = table
        .disable_commands
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(disabled, ["hide", "ambush"]);
    assert!(
        brought
            .notes
            .iter()
            .any(|note| note.starts_with("group_deader:")),
        "{:?}",
        brought.notes
    );
    assert!(
        !brought
            .notes
            .iter()
            .any(|note| note.starts_with("not imported")),
        "{:?}",
        brought.notes
    );
}

/// `quiet_followers` is on unless the profile says otherwise
/// (`bigshot.lic:3551`).
#[test]
fn quiet_followers_is_on_by_default() {
    let brought = import("group", "---\nhunting_room_id: '10'\n").unwrap();
    assert!(brought.profile.group.quiet_followers);
}

// --- the keys on the engine ------------------------------------------------------

/// `disable_commands` (`find_routine`, `bigshot.lic:7166-7168`): fried and
/// in a group, it replaces the target's routine; alone, it does not.
#[test]
fn disable_commands_replace_the_routine_when_fried_in_a_group() {
    let mut fried = leading(state(100), "Kiyna");
    fried.character.experience.mind_percent = Some(100);
    fried.targeting.read("#41", None);
    warg(&mut fried);
    let mut hunt = Hunt::new(with_group(r#"disable_commands = ["hide"]"#).unwrap(), 1);
    hunt.see(led(vec![report("Kiyna")]));
    let said = hunt.tick(&fried, here(10), Some(100));
    assert_eq!(
        said,
        Said::Send {
            line: "hide".to_owned(),
            target: Some(41)
        }
    );
    let mut alone = Hunt::new(with_group(r#"disable_commands = ["hide"]"#).unwrap(), 1);
    alone.see(None);
    let mut solo = state(100);
    solo.character.experience.mind_percent = Some(100);
    solo.targeting.read("#41", None);
    warg(&mut solo);
    assert!(matches!(
        alone.tick(&solo, here(10), Some(100)),
        Said::Walk(_) | Said::Send { .. }
    ));
    assert_ne!(
        alone.tick(&solo, here(10), Some(101)),
        Said::Send {
            line: "hide".to_owned(),
            target: Some(41)
        }
    );
}

/// `troubadours_rally` (`group_status_ailments`, `bigshot.lic:6698-6712`):
/// with 1040 known, a member of the group here stunned is rallied first.
#[test]
fn troubadours_rally_frees_a_stunned_member() {
    let mut fighting = leading(state(100), "Kiyna");
    fighting.targeting.read("#41", None);
    warg(&mut fighting);
    fighting.known_spells.begin();
    fighting.known_spells.read_line(&Runs {
        runs: vec![linked("Troubadour's Rally", "x", "1040", false)],
    });
    let mut player = linked("Kiyna", "-105", "Kiyna", false);
    player.text = "Kiyna".to_owned();
    fighting.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs {
            runs: vec![
                Run {
                    link: None,
                    ..linked("Also here: ", "", "", false)
                },
                player,
                Run {
                    link: None,
                    ..linked(" who is stunned.", "", "", false)
                },
            ],
        },
    });
    let mut hunt = Hunt::new(with_group("troubadours_rally = true").unwrap(), 1);
    hunt.see(led(vec![report("Kiyna")]));
    assert_eq!(
        hunt.tick(&fighting, here(10), Some(100)),
        send("incant 1040")
    );
    let mut off = Hunt::new(with_group("troubadours_rally = false").unwrap(), 1);
    off.see(led(vec![report("Kiyna")]));
    assert_ne!(
        off.tick(&fighting, here(10), Some(100)),
        send("incant 1040")
    );
}

/// `independent_return` (`bigshot.lic:7478-7496`): the leader disbands
/// before the walk home; a follower walks the leader's waypoints to its
/// resting room on its own, not after the leader.
#[test]
fn independent_return_disbands_and_each_walks_home_alone() {
    let mut hunt = Hunt::new(with_group("independent_return = true").unwrap(), 1);
    let grouped = leading(state(100), "Kiyna");
    let heavy = Report {
        rest: Some(Why::Encumbered),
        ..report("Kiyna")
    };
    hunt.see(led(vec![heavy.clone()]));
    assert_eq!(
        hunt.tick(&grouped, here(10), Some(100)),
        send("disband group")
    );
    hunt.see(led(vec![heavy]));
    assert_eq!(
        hunt.tick(&grouped, here(10), Some(101)),
        Said::Walk(RoomId(15))
    );
    assert!(hunt.leading(Some(RoomId(10))).independent_return);

    let mut follower = Hunt::new(Profile::parse(PROFILE).unwrap(), 1);
    let apart = Leading {
        rest: 1,
        phase: Some(Phase::ToRest(Why::Encumbered)),
        room: Some(RoomId(17)),
        rooms: Rooms {
            resting: Some(RoomId(20)),
            waypoints: vec![RoomId(15)],
            ..Rooms::default()
        },
        independent_return: true,
        ..Leading::default()
    };
    follower.see(followed(apart.clone()));
    let alone = state(100);
    assert_eq!(
        follower.tick(&alone, here(10), Some(100)),
        Said::Walk(RoomId(15))
    );
    follower.see(followed(apart));
    assert_eq!(
        follower.tick(&alone, here(15), Some(101)),
        Said::Walk(RoomId(20)),
        "the leader's resting room, not the leader's room 17"
    );
}

/// `independent_travel` (`bigshot.lic:7246-7263`): gathered at the rest,
/// the leader disbands before the walk out.
#[test]
fn independent_travel_disbands_before_the_walk_out() {
    let mut hunt = Hunt::new(with_group("independent_travel = true").unwrap(), 1);
    let state = leading(state(100), "Kiyna");
    let heavy = Report {
        rest: Some(Why::Encumbered),
        ..report("Kiyna")
    };
    hunt.see(led(vec![heavy.clone()]));
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        Said::Walk(RoomId(15))
    );
    hunt.see(led(vec![heavy]));
    assert_eq!(
        hunt.tick(&state, here(15), Some(100)),
        Said::Walk(RoomId(20))
    );
    let ready = Report {
        room: Some(RoomId(20)),
        prepared: Some(1),
        ..report("Kiyna")
    };
    hunt.see(led(vec![ready.clone()]));
    assert_eq!(hunt.tick(&state, here(20), Some(101)), send("store all"));
    hunt.see(led(vec![ready.clone()]));
    assert_eq!(
        hunt.tick(&state, here(20), Some(102)),
        send("disband group")
    );
    hunt.see(led(vec![ready]));
    assert_eq!(
        hunt.tick(&state, here(20), Some(103)),
        Said::Walk(RoomId(12))
    );
}

/// `pre_hunt` (`bigshot.lic:7266-7275`): at the hunting room the leader
/// gathers its group before it hunts.
#[test]
fn the_leader_gathers_before_it_hunts() {
    let mut hunt = Hunt::new(with_group("").unwrap(), 1);
    let state = leading(state(100), "Kiyna");
    let heavy = Report {
        rest: Some(Why::Encumbered),
        ..report("Kiyna")
    };
    hunt.see(led(vec![heavy.clone()]));
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        Said::Walk(RoomId(15))
    );
    hunt.see(led(vec![heavy]));
    assert_eq!(
        hunt.tick(&state, here(15), Some(100)),
        Said::Walk(RoomId(20))
    );
    let at_rest = Report {
        room: Some(RoomId(20)),
        prepared: Some(1),
        ..report("Kiyna")
    };
    hunt.see(led(vec![at_rest.clone()]));
    assert_eq!(hunt.tick(&state, here(20), Some(101)), send("store all"));
    hunt.see(led(vec![at_rest.clone()]));
    assert_eq!(
        hunt.tick(&state, here(20), Some(102)),
        Said::Walk(RoomId(12))
    );
    hunt.see(led(vec![at_rest.clone()]));
    assert_eq!(
        hunt.tick(&state, here(12), Some(103)),
        Said::Walk(RoomId(10))
    );
    // Arrived at the hunting room, the follower still in 20.
    hunt.see(led(vec![at_rest]));
    let said = hunt.tick(&state, here(10), Some(110));
    assert!(
        matches!(said, Said::Wait(1)) || said == send("group open"),
        "{said:?}"
    );
    assert_eq!(hunt.phase(), Phase::Preparing);
    hunt.see(led(vec![report("Kiyna")]));
    let _ = hunt.tick(&state, here(10), Some(111));
    assert_eq!(hunt.phase(), Phase::Hunting);
}

/// `final_loot` (`bs_wander`, `bigshot.lic:9435-9438`): with a loot
/// profile, the leader loots the room once before it wanders on.
#[test]
fn final_loot_loots_the_room_once_before_leaving() {
    let loot = cena_behavior::loot::LootProfile::parse("").unwrap();
    let mut hunt = Hunt::new(with_group("final_loot = true").unwrap(), 1).with_loot(loot);
    let state = leading(state(100), "Kiyna");
    hunt.see(led(vec![report("Kiyna")]));
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        Said::Loot(Vec::new())
    );
    hunt.see(led(vec![report("Kiyna")]));
    assert_eq!(
        hunt.tick(&state, here(10), Some(101)),
        Said::Walk(RoomId(11))
    );
}

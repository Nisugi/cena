//! The hunt engine in a group (`plan/39` Stage 3, `hunt/party.rs`): each
//! arm the role changes, by one tick against a state and a [`Party`], as
//! `hunt_engine.rs` drives the solo arms. One test per rule, each cited.

use cena_behavior::group::{Hindrance, Leading, Muster, Party, PrepOrder, Report, Role};
use cena_behavior::hunt::engine::Phase;
use cena_behavior::hunt::said::Why;
use cena_behavior::hunt::{Ending, Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::group::{GroupEvent, Member};
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs, State};

const PROFILE: &str = r#"
targets = [
  { name = "warg", routine = "a" },
  { any = true, routine = "b" },
]
prepare = ["ready weapon"]

[rooms]
hunting = 10
resting = 20

[rest]
fried = 100
until = { experience = 50 }
commands = ["store all"]

[wander]
wait = 0

[routines]
a = ["attack"]
b = ["fire"]

[group]
quiet_followers = true
"#;

fn profile() -> Result<Profile, String> {
    Profile::parse(PROFILE)
}

/// A state at game second `second`, standing, in the game's room 1000, with
/// the room's players stated (nobody else here).
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

/// A member link as the game sends one.
fn member(noun: &str) -> Member {
    Member {
        id: format!("-1{}", noun.len()),
        noun: noun.to_owned(),
        text: noun.to_owned(),
    }
}

/// `state`, following `leader`.
fn following(mut state: GameState, leader: &str) -> GameState {
    state
        .group
        .apply(&GroupEvent::JoinedGroup(member(leader)), None);
    state
}

/// `state`, leading `follower`.
fn leading(mut state: GameState, follower: &str) -> GameState {
    state
        .group
        .apply(&GroupEvent::Joined(member(follower)), None);
    state
}

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
    attrs.push(("hostile".to_owned(), "1".to_owned()));
    state.apply(&Frame::CreatureStatus {
        id: id.to_string(),
        attrs,
    });
}

fn here(room: u32) -> Here<'static> {
    Here {
        room: Some(RoomId(room)),
        exits: &[],
        tags: &[],
    }
}

fn send(line: &str, target: Option<i64>) -> Said {
    Said::Send {
        line: line.to_owned(),
        target,
    }
}

/// A follower's report: in room 10, connected, grouped, nothing to say.
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

/// The follower Kiyna's party, its leader Ashryn publishing `leading`.
fn follower_party(leading: Leading) -> Party {
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

/// The leader Ashryn's party, with these followers and musters.
fn leader_party(followers: Vec<Report>, musters: Vec<(String, Muster)>) -> Party {
    Party {
        name: "Ashryn".to_owned(),
        role: Role::Lead,
        leader: "Ashryn".to_owned(),
        leading: None,
        followers,
        musters,
        dropped: false,
        awaiting: Vec::new(),
        leader_lost: None,
        recoverer: None,
    }
}

/// The leader hunting in room 10.
fn hunting_in_10() -> Leading {
    Leading {
        phase: Some(Phase::Hunting),
        room: Some(RoomId(10)),
        ..Leading::default()
    }
}

// --- follow ----------------------------------------------------------------------

/// `plan/39` §5, Assist: the leader's target while it stands
/// (`bigshot.lic:10145-10150`), over the follower's own first choice.
#[test]
fn a_follower_takes_the_leaders_target() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = following(state(100), "Ashryn");
    creature(&mut state, 41, "warg", &[]);
    creature(&mut state, 42, "troll", &[]);
    hunt.see(follower_party(Leading {
        target: Some(42),
        ..hunting_in_10()
    }));
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        send("target #42", Some(42))
    );
}

/// Without a target from the leader, its own choice by its own ranks.
#[test]
fn a_follower_without_the_leaders_target_chooses_its_own() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = following(state(100), "Ashryn");
    creature(&mut state, 41, "warg", &[]);
    hunt.see(follower_party(hunting_in_10()));
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        send("target #41", Some(41))
    );
}

/// `bigshot.lic:10195-10199`: a follower loots only when the leader names it.
#[test]
fn a_follower_loots_only_when_named() {
    let mut state = following(state(100), "Ashryn");
    creature(&mut state, 43, "warg", &[("dead", "1")]);
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    hunt.see(follower_party(Leading {
        looter: Some("Ashryn".to_owned()),
        ..hunting_in_10()
    }));
    assert_ne!(
        hunt.tick(&state, here(10), Some(100)),
        send("loot #43", None)
    );
    let mut named = Hunt::new(profile().unwrap(), 1);
    named.see(follower_party(Leading {
        looter: Some("Kiyna".to_owned()),
        ..hunting_in_10()
    }));
    assert_eq!(
        named.tick(&state, here(10), Some(100)),
        send("loot #43", None)
    );
}

/// `plan/39` §5, Follow: apart and out of the group, it walks to the
/// leader's room (`group_all_followers`, `bigshot.lic:9335-9340`).
#[test]
fn a_follower_apart_and_ungrouped_walks_to_the_leader() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    hunt.see(follower_party(Leading {
        room: Some(RoomId(12)),
        ..hunting_in_10()
    }));
    assert_eq!(
        hunt.tick(&state(100), here(10), Some(100)),
        Said::Walk(RoomId(12))
    );
}

/// Grouped, the game carries it: it waits a moment before walking itself.
#[test]
fn a_grouped_follower_apart_waits_for_the_game_then_walks() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let party = follower_party(Leading {
        room: Some(RoomId(12)),
        ..hunting_in_10()
    });
    hunt.see(party.clone());
    let grouped = following(state(100), "Ashryn");
    assert_eq!(hunt.tick(&grouped, here(10), Some(100)), Said::Wait(1));
    hunt.see(party);
    let later = following(state(103), "Ashryn");
    assert_eq!(
        hunt.tick(&later, here(10), Some(103)),
        Said::Walk(RoomId(12))
    );
}

/// `bigshot.lic:9342-9346`: with the leader and not grouped, `join` it.
#[test]
fn a_follower_with_the_leader_but_ungrouped_joins() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    hunt.see(follower_party(hunting_in_10()));
    assert_eq!(
        hunt.tick(&state(100), here(10), Some(100)),
        send("join Ashryn", None)
    );
}

/// A follower does not rest on its own reasons: it reports them, and the
/// leader decides (`plan/39` §5).
#[test]
fn a_fried_follower_hunts_on_until_the_leader_rests() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut fried = following(state(100), "Ashryn");
    fried.character.experience.mind_percent = Some(100);
    hunt.see(follower_party(hunting_in_10()));
    let said = hunt.tick(&fried, here(10), Some(100));
    assert!(!matches!(said, Said::Walk(_)), "{said:?}");
    assert_eq!(hunt.phase(), Phase::Hunting);
    let mine = hunt.report(&fried, "Kiyna", Some(RoomId(10)), State::Ready);
    assert_eq!(mine.rest, Some(Why::Fried), "the reason is reported");
}

/// `quiet_followers` (`bigshot.lic:7525-7549`): at the leader's rest, a
/// follower waits for the leader's prep **for this rest**, then its own
/// resting commands, then reports itself prepared for it.
#[test]
fn a_follower_preps_after_the_leader_has_for_this_rest() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let state = following(state(100), "Ashryn");
    let at_rest = |prepared| Leading {
        rest: 2,
        prepared,
        phase: Some(Phase::Resting(Why::Fried)),
        room: Some(RoomId(20)),
        order: Some(PrepOrder::LeaderFirst),
        ..Leading::default()
    };
    hunt.see(follower_party(at_rest(Some(1))));
    assert_eq!(
        hunt.tick(&state, here(20), Some(100)),
        Said::Wait(1),
        "the last rest's prep"
    );
    hunt.see(follower_party(at_rest(Some(2))));
    assert_eq!(
        hunt.tick(&state, here(20), Some(101)),
        send("store all", None)
    );
    hunt.see(follower_party(at_rest(Some(2))));
    assert_eq!(hunt.tick(&state, here(20), Some(102)), Said::Wait(1));
    let mine = hunt.report(&state, "Kiyna", Some(RoomId(20)), State::Ready);
    assert_eq!(mine.prepared, Some(2));
}

// --- lead ------------------------------------------------------------------------

/// `ma_looter` (`bigshot.lic:7109-7112`): a follower named loots, and the
/// leader waits for it before it walks on (`:7466`, `:9431`).
#[test]
fn a_leader_names_the_looter_and_waits_for_it() {
    let text = PROFILE.replace(
        "quiet_followers = true",
        "quiet_followers = true\nma_looter = \"Kiyna\"",
    );
    let mut hunt = Hunt::new(Profile::parse(&text).unwrap(), 1);
    let mut state = leading(state(100), "Kiyna");
    creature(&mut state, 43, "warg", &[("dead", "1")]);
    hunt.see(leader_party(vec![report("Kiyna")], Vec::new()));
    let said = hunt.tick(&state, here(10), Some(100));
    assert_ne!(said, send("loot #43", None), "not the leader's to loot");
    assert_eq!(said, Said::Wait(1), "and it waits for the looter");
    assert_eq!(
        hunt.leading(Some(RoomId(10))).looter.as_deref(),
        Some("Kiyna")
    );
    // Looted: the leader goes on.
    let looted = Report {
        looted: vec![43],
        ..report("Kiyna")
    };
    hunt.see(leader_party(vec![looted], Vec::new()));
    assert_ne!(hunt.tick(&state, here(10), Some(101)), Said::Wait(1));
}

/// `should_rest?` (`bigshot.lic:9060`): fried rests the group only when
/// every member is fried.
#[test]
fn a_fried_leader_hunts_on_until_every_member_is_fried() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut fried = leading(state(100), "Kiyna");
    fried.character.experience.mind_percent = Some(100);
    hunt.see(leader_party(vec![report("Kiyna")], Vec::new()));
    let said = hunt.tick(&fried, here(10), Some(100));
    assert_ne!(said, Said::Walk(RoomId(20)), "Kiyna is not fried");
    let both = Report {
        rest: Some(Why::Fried),
        ..report("Kiyna")
    };
    hunt.see(leader_party(vec![both], Vec::new()));
    assert_eq!(
        hunt.tick(&fried, here(10), Some(101)),
        Said::Walk(RoomId(20))
    );
    assert_eq!(hunt.phase(), Phase::ToRest(Why::Fried));
}

/// A follower's reason rests the group, and names it when the leader has
/// none (`scripts/eohunter/rest.rb:753-766`).
#[test]
fn a_followers_reason_rests_the_group() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let heavy = Report {
        rest: Some(Why::Encumbered),
        ..report("Kiyna")
    };
    hunt.see(leader_party(vec![heavy], Vec::new()));
    let state = leading(state(100), "Kiyna");
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        Said::Walk(RoomId(20))
    );
    assert_eq!(hunt.phase(), Phase::ToRest(Why::Encumbered));
    assert_eq!(hunt.leading(Some(RoomId(10))).rest, 1, "the first rest");
}

/// Resting, the leader waits for each follower's prep **for this rest**,
/// and says why (`bigshot.lic:7577-7593`; the barrier by number, `plan/39`
/// §0e), then walks back.
#[test]
fn a_leader_walks_back_only_when_every_follower_is_ready_for_this_rest() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let heavy = Report {
        rest: Some(Why::Encumbered),
        ..report("Kiyna")
    };
    let state = leading(state(100), "Kiyna");
    hunt.see(leader_party(vec![heavy], Vec::new()));
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        Said::Walk(RoomId(20))
    );
    // At the rest room: the leader's own commands.
    let at_rest = |prepared, unready| Report {
        room: Some(RoomId(20)),
        prepared,
        unready,
        ..report("Kiyna")
    };
    hunt.see(leader_party(vec![at_rest(None, None)], Vec::new()));
    assert_eq!(
        hunt.tick(&state, here(20), Some(101)),
        send("store all", None)
    );
    hunt.see(leader_party(vec![at_rest(None, None)], Vec::new()));
    assert_eq!(
        hunt.tick(&state, here(20), Some(102)),
        Said::Wait(5),
        "not prepared"
    );
    let notes = hunt.take_notes();
    assert!(
        notes
            .iter()
            .any(|n| n == "Kiyna isn't hunting because: preparing for the rest"),
        "{notes:?}"
    );
    hunt.see(leader_party(
        vec![at_rest(Some(1), Some("mind still above threshold"))],
        Vec::new(),
    ));
    assert_eq!(
        hunt.tick(&state, here(20), Some(103)),
        Said::Wait(5),
        "still resting"
    );
    hunt.see(leader_party(vec![at_rest(Some(1), None)], Vec::new()));
    assert_eq!(
        hunt.tick(&state, here(20), Some(104)),
        Said::Walk(RoomId(10))
    );
}

/// The gather (`bigshot.lic:7556-7562`): ready, but not here, the leader
/// opens the group and waits before walking back.
#[test]
fn a_leader_gathers_before_walking_back() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let state = leading(state(100), "Kiyna");
    let heavy = Report {
        rest: Some(Why::Encumbered),
        ..report("Kiyna")
    };
    hunt.see(leader_party(vec![heavy], Vec::new()));
    let _ = hunt.tick(&state, here(10), Some(100));
    let away = Report {
        room: Some(RoomId(21)),
        prepared: Some(1),
        ..report("Kiyna")
    };
    hunt.see(leader_party(vec![away.clone()], Vec::new()));
    assert_eq!(
        hunt.tick(&state, here(20), Some(101)),
        send("store all", None)
    );
    hunt.see(leader_party(vec![away.clone()], Vec::new()));
    assert_eq!(
        hunt.tick(&state, here(20), Some(102)),
        send("group open", None)
    );
    hunt.see(leader_party(vec![away], Vec::new()));
    assert_eq!(hunt.tick(&state, here(20), Some(103)), Said::Wait(1));
    let back = Report {
        room: Some(RoomId(20)),
        prepared: Some(1),
        ..report("Kiyna")
    };
    hunt.see(leader_party(vec![back], Vec::new()));
    assert_eq!(
        hunt.tick(&state, here(20), Some(104)),
        Said::Walk(RoomId(10))
    );
}

/// `plan/39` §8a: a member Hydra gave up, standing here grouped and well,
/// is taken home: the group rests now, for it.
#[test]
fn a_linkdead_member_here_is_taken_home() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let state = leading(state(100), "Kiyna");
    let gone = Report {
        link: State::Closed,
        ..report("Kiyna")
    };
    hunt.see(leader_party(
        vec![gone],
        vec![("Kiyna".to_owned(), Muster::TakeHome)],
    ));
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        Said::Walk(RoomId(20))
    );
    assert_eq!(hunt.phase(), Phase::ToRest(Why::Linkdead));
}

/// §8a: *"drag em if something happened to them"*: dragged, then home.
#[test]
fn a_linkdead_member_hurt_is_dragged_home() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let state = leading(state(100), "Kiyna");
    let party = leader_party(
        vec![report("Kiyna")],
        vec![("Kiyna".to_owned(), Muster::Drag)],
    );
    hunt.see(party.clone());
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        send("drag Kiyna", None)
    );
    hunt.see(party);
    assert_eq!(
        hunt.tick(&state, here(10), Some(101)),
        Said::Walk(RoomId(20))
    );
}

/// `plan/39` §8, question 7, row 3: the group goes to a member that cannot
/// move.
#[test]
fn a_leader_fetches_a_member_that_cannot_move() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let state = leading(state(100), "Kiyna");
    hunt.see(leader_party(
        vec![report("Kiyna")],
        vec![("Kiyna".to_owned(), Muster::Fetch(RoomId(14)))],
    ));
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        Said::Walk(RoomId(14))
    );
}

/// Question 7, row 1, and question 5: held for, the leader does not wander
/// off, and does not walk to rest.
#[test]
fn a_leader_holding_for_a_member_neither_wanders_nor_rests() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut fried = leading(state(100), "Kiyna");
    fried.character.experience.mind_percent = Some(100);
    let lost = Report {
        rest: Some(Why::Fried),
        link: State::Reconnecting,
        ..report("Kiyna")
    };
    let until = std::time::Instant::now();
    hunt.see(leader_party(
        vec![lost],
        vec![("Kiyna".to_owned(), Muster::Lost { until })],
    ));
    assert_eq!(hunt.tick(&fried, here(10), Some(100)), Said::Wait(1));
    assert_eq!(hunt.phase(), Phase::Hunting);
}

/// Question 10: a member dead, every hunt ends, and the leader, able,
/// carries it out first: its hand taken (hands already empty here), then,
/// without Spirit Guide, dragged to the resting room.
#[test]
fn a_dead_member_is_carried_out_by_the_leader_then_every_hunt_ends() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let state = leading(state(100), "Kiyna");
    let dead = Report {
        hindrance: Some(Hindrance::Dead),
        ..report("Kiyna")
    };
    let party = leader_party(vec![dead], vec![("Kiyna".to_owned(), Muster::Dead)]);
    hunt.see(party.clone());
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        send("hold Kiyna", None)
    );
    hunt.see(party.clone());
    assert_eq!(
        hunt.tick(&state, here(10), Some(101)),
        Said::Wait(1),
        "already held: no wait for the hand"
    );
    hunt.see(party.clone());
    assert_eq!(
        hunt.tick(&state, here(10), Some(102)),
        send("drag Kiyna", None)
    );
    hunt.see(party.clone());
    assert_eq!(
        hunt.tick(&state, here(10), Some(103)),
        Said::Walk(RoomId(20))
    );
    hunt.see(party);
    assert_eq!(
        hunt.tick(&state, here(20), Some(120)),
        Said::Done(Ending::MemberDied)
    );
}

/// Solo, none of it: the group's arms are silent without a party.
#[test]
fn alone_the_hunt_is_the_solo_hunt() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = state(100);
    creature(&mut state, 43, "warg", &[("dead", "1")]);
    hunt.see(None);
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        send("loot #43", None)
    );
}

/// bigshot's `head` waits for its followers to register before it hunts
/// (`bigshot.lic:9927-9999`): a member of the game's group whose hunt has
/// not reported yet is waited for, and said so.
#[test]
fn a_leader_waits_for_its_members_to_start() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut state = leading(state(100), "Kiyna");
    creature(&mut state, 43, "warg", &[("dead", "1")]);
    let mut party = leader_party(Vec::new(), Vec::new());
    party.awaiting = vec!["Kiyna".to_owned()];
    hunt.see(party.clone());
    assert_eq!(
        hunt.tick(&state, here(10), Some(100)),
        send("group open", None),
        "`head` opens the group (`bigshot.lic:9908`)"
    );
    hunt.see(party);
    assert_eq!(hunt.tick(&state, here(10), Some(101)), Said::Wait(1));
    assert_eq!(
        hunt.take_notes(),
        vec!["waiting for Kiyna to start hunting.".to_owned()]
    );
    hunt.see(leader_party(vec![report("Kiyna")], Vec::new()));
    assert_eq!(
        hunt.tick(&state, here(10), Some(102)),
        send("loot #43", None)
    );
}

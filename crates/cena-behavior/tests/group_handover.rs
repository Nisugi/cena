//! A lost leader and a dead member (`plan/39` Stages 6 and 7): the
//! follower's side of each, and the leader's, by one tick at a time as
//! `group_engine.rs` drives the rest of the group's arms.

use std::time::Instant;

use cena_behavior::group::{Hindrance, Leading, Muster, Party, Report, Role};
use cena_behavior::hunt::engine::Phase;
use cena_behavior::hunt::{Ending, Here, Hunt, Profile, Said};
use cena_map::RoomId;
use cena_session::group::{GroupEvent, Member};
use cena_session::{Frame, GameState, Link, LinkKind, Run, Runs, State};

const PROFILE: &str = r#"
targets = [{ any = true, routine = "a" }]
[rooms]
hunting = 10
resting = 20
[rest]
fried = 100
[wander]
wait = 0
[routines]
a = ["attack"]
"#;

fn profile() -> Result<Profile, String> {
    Profile::parse(PROFILE)
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

/// A warg in the room.
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
        exits: &[],
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

/// Kiyna, following Ashryn, who publishes `leading`.
fn following(leading: Leading, lost: Option<Muster>, recoverer: Option<&str>) -> Party {
    Party {
        name: "Kiyna".to_owned(),
        role: Role::Follow,
        leader: "Ashryn".to_owned(),
        leading: Some(leading),
        followers: Vec::new(),
        musters: Vec::new(),
        dropped: false,
        awaiting: Vec::new(),
        leader_lost: lost,
        recoverer: recoverer.map(str::to_owned),
    }
}

/// Ashryn, leading these.
fn leading(followers: Vec<Report>, musters: Vec<(String, Muster)>) -> Party {
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

fn hunting_in(room: u32) -> Leading {
    Leading {
        phase: Some(Phase::Hunting),
        room: Some(RoomId(room)),
        ..Leading::default()
    }
}

// --- Stage 6: the leader lost ---------------------------------------------------

/// `plan/39` §8, question 5: the leader's connection lost, the follower
/// keeps the room clear and does not go looking for it.
#[test]
fn a_follower_holds_the_room_for_a_lost_leader() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut grouped = state(100);
    grouped
        .group
        .apply(&GroupEvent::JoinedGroup(member("Ashryn")), None);
    warg(&mut grouped);
    let until = Instant::now();
    hunt.see(following(
        hunting_in(12),
        Some(Muster::Lost { until }),
        None,
    ));
    assert_eq!(
        hunt.tick(&grouped, here(10), Some(100)),
        Said::Send {
            line: "target #41".to_owned(),
            target: Some(41)
        },
        "fights here, rather than walking to the leader's last room"
    );
}

/// While the driver places it anew after a handover, it waits.
#[test]
fn a_follower_waits_while_the_lead_is_handed_over() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    hunt.see(following(hunting_in(10), Some(Muster::Gone), None));
    assert_eq!(hunt.tick(&state(100), here(10), Some(100)), Said::Wait(1));
}

/// §8a with the handover's own step (`plan/39` §1): a member given up,
/// standing here, well, but not in the group, is added, then taken home.
#[test]
fn a_leader_adds_a_linkdead_member_it_does_not_hold() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut here_state = state(100);
    here_state.apply(&Frame::Component {
        id: "room players".into(),
        body: Runs {
            runs: vec![linked("Kiyna", "-10966", "Kiyna", false)],
        },
    });
    let gone = Report {
        link: State::Closed,
        grouped: false,
        ..report("Kiyna")
    };
    let party = leading(vec![gone], vec![("Kiyna".to_owned(), Muster::Add)]);
    hunt.see(party.clone());
    assert_eq!(
        hunt.tick(&here_state, here(10), Some(100)),
        send("group #-10966")
    );
    hunt.see(party);
    assert_ne!(
        hunt.tick(&here_state, here(10), Some(101)),
        send("group #-10966"),
        "added once"
    );
}

// --- Stage 7: a dead member ------------------------------------------------------

/// Question 10: the leader dead, a follower not named to carry it out ends
/// its hunt.
#[test]
fn a_follower_not_carrying_a_dead_leader_ends() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    hunt.see(following(
        hunting_in(10),
        Some(Muster::Dead),
        Some("Dicate"),
    ));
    assert_eq!(
        hunt.tick(&state(100), here(10), Some(100)),
        Said::Done(Ending::MemberDied)
    );
}

/// Question 10: the follower named carries the dead leader out: its hand,
/// then Spirit Guide (130) when known, and the hunt is over.
#[test]
fn a_follower_carries_a_dead_leader_out_by_spirit_guide() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut knows = state(100);
    knows.known_spells.begin();
    knows.known_spells.read_line(&Runs {
        runs: vec![linked("Spirit Guide", "x", "130", false)],
    });
    let party = following(hunting_in(10), Some(Muster::Dead), Some("Kiyna"));
    hunt.see(party.clone());
    assert_eq!(hunt.tick(&knows, here(10), Some(100)), send("hold Ashryn"));
    hunt.see(party.clone());
    assert_eq!(hunt.tick(&knows, here(10), Some(101)), Said::Wait(1));
    hunt.see(party.clone());
    assert_eq!(
        hunt.tick(&knows, here(10), Some(106)),
        Said::Wait(1),
        "the hand not yet taken: waited for, then on"
    );
    hunt.see(party.clone());
    assert_eq!(hunt.tick(&knows, here(10), Some(107)), send("incant 130"));
    hunt.see(party);
    assert_eq!(
        hunt.tick(&knows, here(10), Some(108)),
        Said::Done(Ending::MemberDied)
    );
}

/// Question 10: *"the leader if able, else any member who can"*: the
/// leader stuck names Kiyna, publishes it, and waits; Kiyna carries the
/// dead out; any other member's hunt ends.
#[test]
fn a_stuck_leader_names_the_follower_that_carries_the_dead_out() {
    let mut leader = Hunt::new(profile().unwrap(), 1);
    let mut stuck = state(100);
    stuck.status.set("stunned", true);
    let dead = Report {
        hindrance: Some(Hindrance::Dead),
        ..report("Zeta")
    };
    leader.see(leading(
        vec![report("Kiyna"), dead],
        vec![("Zeta".to_owned(), Muster::Dead)],
    ));
    assert_eq!(leader.tick(&stuck, here(10), Some(100)), Said::Wait(1));
    let published = leader.leading(Some(RoomId(10)));
    assert_eq!(
        published.recover,
        Some(("Zeta".to_owned(), "Kiyna".to_owned()))
    );

    let mut kiyna = Hunt::new(profile().unwrap(), 1);
    kiyna.see(following(published.clone(), None, None));
    assert_eq!(
        kiyna.tick(&state(100), here(10), Some(100)),
        send("hold Zeta")
    );

    let mut dicate = Hunt::new(profile().unwrap(), 1);
    let mut party = following(published, None, None);
    party.name = "Dicate".to_owned();
    dicate.see(party);
    assert_eq!(
        dicate.tick(&state(100), here(10), Some(100)),
        Said::Done(Ending::MemberDied)
    );
}

/// Question 10: *"if you can't drag them probably try to alert the
/// human"*: nobody able, the player is told, and the hunt ends.
#[test]
fn nobody_able_to_carry_the_dead_alerts_the_player() {
    let mut hunt = Hunt::new(profile().unwrap(), 1);
    let mut stuck = state(100);
    stuck.status.set("stunned", true);
    let dead = Report {
        hindrance: Some(Hindrance::Dead),
        ..report("Zeta")
    };
    hunt.see(leading(vec![dead], vec![("Zeta".to_owned(), Muster::Dead)]));
    assert_eq!(
        hunt.tick(&stuck, here(10), Some(100)),
        Said::Done(Ending::MemberDied)
    );
    assert!(
        hunt.take_alerts()
            .iter()
            .any(|alert| alert.contains("they need you")),
        "the player is told"
    );
}

//! The group's rules for a member apart from the leader (`plan/39` Stage 2,
//! `crates/cena-behavior/src/group/muster.rs` and `group/successor.rs`):
//! question 7's table, each wait's deadline, the recovery of a dead member,
//! and who leads when the leader is lost. One test per answer, each cited.

use std::time::{Duration, Instant};

use cena_behavior::group::{
    Hindrance, LOST_WAIT, Muster, Report, Settings, muster, recoverer, successor,
};
use cena_map::RoomId;
use cena_session::State;

/// The leader's room.
const HERE: Option<RoomId> = Some(RoomId(1));

/// A member in the leader's room, connected and grouped, nothing wrong.
fn member(name: &str) -> Report {
    Report {
        name: name.to_owned(),
        link: State::Ready,
        room: HERE,
        rest: None,
        unready: None,
        hindrance: None,
        grouped: true,
        health: None,
        headroom: None,
    }
}

/// `member`, kept by `hindrance`.
fn hindered(name: &str, hindrance: Hindrance) -> Report {
    Report {
        hindrance: Some(hindrance),
        ..member(name)
    }
}

/// `member`, in room `room` (`None`: the map cannot place it).
fn away(name: &str, room: Option<u32>) -> Report {
    Report {
        room: room.map(RoomId),
        ..member(name)
    }
}

/// `member`, with its connection in `link`.
fn linked(name: &str, link: State) -> Report {
    Report {
        link,
        ..member(name)
    }
}

/// `member`, at `health` percent.
fn healthy(name: &str, health: u32) -> Report {
    Report {
        health: Some(health),
        ..member(name)
    }
}

/// What muster answers `seconds` after the member was first seen apart.
fn after(report: &Report, seconds: u64) -> (Option<Muster>, Instant) {
    let since = Instant::now();
    let now = since + Duration::from_secs(seconds);
    let answer = muster(report, HERE, since, now, &Settings::default());
    (answer, since + LOST_WAIT)
}

// --- question 7's table ---------------------------------------------------------

/// With the leader and nothing keeping it: nothing to muster.
#[test]
fn a_member_here_and_free_needs_nothing() {
    assert_eq!(after(&member("Kiyna"), 0).0, None);
}

/// `plan/39` §8, question 7, row 1: here, in roundtime, stunned or down,
/// the group holds and keeps the room clear, until `lost_wait`.
#[test]
fn a_hindered_member_here_is_held_for() {
    for hindrance in [Hindrance::Roundtime, Hindrance::Down, Hindrance::Stuck] {
        let (answer, until) = after(&hindered("Kiyna", hindrance), 10);
        assert_eq!(
            answer,
            Some(Muster::Hold { hindrance, until }),
            "{hindrance:?}"
        );
    }
}

/// `group/muster.rs`: the hold ends at `lost_wait`; still unable, the group
/// rests when it can (`plan/39` §2: *"go rest and wait rather than hunt
/// on"*).
#[test]
fn a_hold_past_its_deadline_is_overdue() {
    let (answer, _) = after(&hindered("Kiyna", Hindrance::Stuck), 90);
    assert_eq!(answer, Some(Muster::Overdue));
}

/// `plan/39` §8, question 7, row 2: elsewhere and able to move, it walks
/// over, and the group waits until `lost_wait`.
#[test]
fn a_member_elsewhere_that_can_move_is_awaited() {
    let (answer, until) = after(&away("Kiyna", Some(2)), 30);
    assert_eq!(answer, Some(Muster::Await { until }));
    let (down, until) = after(
        &Report {
            hindrance: Some(Hindrance::Down),
            ..away("Kiyna", Some(2))
        },
        30,
    );
    assert_eq!(
        down,
        Some(Muster::Await { until }),
        "down can stand and walk"
    );
}

/// Question 7: *"go to them if they can't come to you immediately"*: not
/// arrived by the deadline, the group goes to it.
#[test]
fn a_member_not_arrived_by_the_deadline_is_fetched() {
    let (answer, _) = after(&away("Kiyna", Some(2)), 90);
    assert_eq!(answer, Some(Muster::Fetch(RoomId(2))));
}

/// `plan/39` §8, question 7, row 3: elsewhere and unable to move, the
/// leader walks the group to it at once.
#[test]
fn a_member_elsewhere_that_cannot_move_is_fetched_at_once() {
    let stuck = Report {
        hindrance: Some(Hindrance::Stuck),
        ..away("Kiyna", Some(7))
    };
    assert_eq!(after(&stuck, 0).0, Some(Muster::Fetch(RoomId(7))));
}

/// `group/muster.rs`: apart where the map cannot place it, there is no
/// room to walk to; it is waited for, then the group rests.
#[test]
fn a_member_the_map_cannot_place_is_awaited_then_overdue() {
    let lost = Report {
        hindrance: Some(Hindrance::Stuck),
        ..away("Kiyna", None)
    };
    let (answer, until) = after(&lost, 0);
    assert_eq!(answer, Some(Muster::Await { until }));
    assert_eq!(after(&lost, 90).0, Some(Muster::Overdue));
}

/// `group/muster.rs`: two rooms the map cannot place are not one room, so
/// a member is not with a leader when neither is placed.
#[test]
fn neither_room_known_is_not_together() {
    let since = Instant::now();
    let answer = muster(
        &away("Kiyna", None),
        None,
        since,
        since,
        &Settings::default(),
    );
    assert_eq!(
        answer,
        Some(Muster::Await {
            until: since + LOST_WAIT
        })
    );
}

/// `plan/39` §8, question 5 and §1: a dropped connection is held for,
/// fighting only, for `lost_wait` (90 s); then the handover.
#[test]
fn a_lost_connection_is_waited_for_then_handed_over() {
    let lost = linked("Kiyna", State::Reconnecting);
    let (answer, until) = after(&lost, 89);
    assert_eq!(answer, Some(Muster::Lost { until }));
    assert_eq!(after(&lost, 90).0, Some(Muster::Gone));
    let syncing = linked("Kiyna", State::Syncing);
    assert!(
        matches!(after(&syncing, 0).0, Some(Muster::Lost { .. })),
        "back, but not Ready yet"
    );
}

/// `plan/39` §8a and question 5's proposal: `Closed`, Hydra gave up, so
/// there is nothing to wait for.
#[test]
fn a_connection_given_up_is_handed_over_at_once() {
    assert_eq!(
        after(&linked("Kiyna", State::Closed), 0).0,
        Some(Muster::Gone)
    );
}

/// `plan/12` §5.2: a member not connected is read for nothing else; its
/// report is stale.
#[test]
fn a_lost_connection_is_read_before_anything_else() {
    let lost = Report {
        link: State::Reconnecting,
        hindrance: Some(Hindrance::Dead),
        grouped: false,
        ..member("Kiyna")
    };
    assert!(matches!(after(&lost, 0).0, Some(Muster::Lost { .. })));
}

/// `plan/39` §8, question 10: dead, every hunt ends -- read before it left
/// the group, which death also does (question 7: *"if they leave the group
/// because they died well ...."*).
#[test]
fn a_dead_member_ends_the_hunt_before_it_counts_as_left() {
    let dead = Report {
        grouped: false,
        ..hindered("Kiyna", Hindrance::Dead)
    };
    assert_eq!(after(&dead, 0).0, Some(Muster::Dead));
}

/// `plan/39` §8, question 7, last row: it left the game's group, the group
/// hunts on without it.
#[test]
fn a_member_that_left_the_group_is_left() {
    let left = Report {
        grouped: false,
        ..away("Kiyna", Some(4))
    };
    assert_eq!(after(&left, 0).0, Some(Muster::Left));
}

// --- the recovery ----------------------------------------------------------------

/// `plan/39` §8, question 10: *"the leader if able"*.
#[test]
fn the_leader_recovers_a_dead_member_when_able() {
    let followers = [hindered("Kiyna", Hindrance::Dead), member("Dicate")];
    assert_eq!(recoverer(&member("Leader"), &followers), Some("Leader"));
}

/// Question 10: *"else any member who can"*: the leader stuck, or the one
/// dead, the first follower able.
#[test]
fn else_the_first_member_able_recovers() {
    let followers = [
        hindered("Kiyna", Hindrance::Dead),
        linked("Zeta", State::Reconnecting),
        hindered("Iona", Hindrance::Stuck),
        member("Dicate"),
    ];
    assert_eq!(
        recoverer(&hindered("Leader", Hindrance::Stuck), &followers),
        Some("Dicate")
    );
    assert_eq!(
        recoverer(&hindered("Leader", Hindrance::Dead), &followers),
        Some("Dicate")
    );
}

/// Question 10: nobody able, nobody recovers, and the player is told.
#[test]
fn nobody_able_recovers_nobody() {
    let followers = [hindered("Kiyna", Hindrance::Dead)];
    assert_eq!(
        recoverer(&hindered("Leader", Hindrance::Stuck), &followers),
        None
    );
}

// --- the successor ----------------------------------------------------------------

/// `plan/39` §8, question 4: *"a priority list"*, first present first.
#[test]
fn the_first_present_successor_leads() {
    let settings = Settings {
        successors: vec!["Zeta".to_owned(), "Kiyna".to_owned(), "Dicate".to_owned()],
        ..Settings::default()
    };
    let members = [
        healthy("Dicate", 100),
        healthy("Kiyna", 10),
        linked("Zeta", State::Reconnecting),
    ];
    assert_eq!(successor(&members, &settings, 0), Some("Kiyna"));
}

/// Question 4: *"random if one not set up (the healthiest?)"*: the most
/// health, a dead member never.
#[test]
fn without_a_list_the_healthiest_leads() {
    let members = [
        healthy("Kiyna", 60),
        Report {
            hindrance: Some(Hindrance::Dead),
            ..healthy("Zeta", 100)
        },
        healthy("Dicate", 80),
        member("Iona"),
    ];
    assert_eq!(successor(&members, &Settings::default(), 0), Some("Dicate"));
}

/// Question 4: a tie at random, among the tied only.
#[test]
fn a_tie_for_healthiest_is_settled_by_the_roll() {
    let members = [
        healthy("Kiyna", 90),
        healthy("Dicate", 50),
        healthy("Zeta", 90),
    ];
    let settings = Settings::default();
    assert_eq!(successor(&members, &settings, 0), Some("Kiyna"));
    assert_eq!(successor(&members, &settings, 1), Some("Zeta"));
}

/// Nobody present, nobody leads.
#[test]
fn nobody_present_leads_nobody() {
    let members = [linked("Kiyna", State::Closed)];
    assert_eq!(successor(&members, &Settings::default(), 0), None);
}

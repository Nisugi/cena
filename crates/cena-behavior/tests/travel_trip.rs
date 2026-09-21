//! The trip: `cena_behavior::travel`, stage 1 (`plan/24` §3).
//!
//! Every test drives the machine the way the driver will: tick, do what it
//! says, tell it where that landed. No game, no clock.

use cena_behavior::travel::{MAX_REPLANS, Now, Said, Trip, Why};
use cena_map::{Map, Room, RoomId, Walker};

/// ```text
///   1 --east-- 2 --east-- 3 --east-- 4      plain, 1s each
///   1 --go shortcut, bards only, 0.5s-- 4   and the same two rooms, 10s, for anyone
///   2 --unported-- 4                        nothing has ported it: gone round
///   3 --(pass)-- 50 --go alley-- 5          an urchin hub, which is nowhere
///   4 --north-- 6 --north-- 7               6 is slippery: see the tests
///   9                                       an island
/// ```
const ROOMS: &str = r#"[
  {"id":1,"exits":[{"to":2,"kind":"cardinal","cmd":"east","cost":1},
                   {"to":4,"kind":"go","cmd":"go shortcut","cost":{"when":{"profession":"Bard"},"then":0.5}},
                   {"to":4,"kind":"go","cmd":"go long way","cost":10}]},
  {"id":2,"exits":[{"to":3,"kind":"cardinal","cmd":"east","cost":1},
                   {"to":1,"kind":"cardinal","cmd":"west","cost":1},
                   {"to":4,"kind":"scripted","unported":"00000000deadbeef","cost":0.1}]},
  {"id":3,"exits":[{"to":4,"kind":"cardinal","cmd":"east","cost":1},
                   {"to":50,"kind":"scripted","pass":null,"cost":0}]},
  {"id":50,"exits":[{"to":5,"kind":"go","cmd":"go alley","cost":1}]},
  {"id":4,"exits":[{"to":6,"kind":"cardinal","cmd":"north","cost":1},
                   {"to":3,"kind":"cardinal","cmd":"west","cost":1}]},
  {"id":5},
  {"id":6,"exits":[{"to":7,"kind":"cardinal","cmd":"north","cost":1},
                   {"to":4,"kind":"cardinal","cmd":"south","cost":1}]},
  {"id":7},
  {"id":9}
]"#;

/// In this room, at time zero: for the tests where time does not matter.
fn at(room: u32) -> Now {
    Now {
        here: Some(RoomId(room)),
        ms: 0,
    }
}

fn map() -> Option<Map> {
    let rooms: Vec<Room> = serde_json::from_str(ROOMS).ok()?;
    Map::from_rooms(rooms).ok()
}

/// Walk a trip, landing each move where `lands` says (by command), and give
/// back every command sent and how it ended. `None` when the walk went wrong
/// in a way the test did not script: an unexpected hold, or no end.
fn walk(
    trip: &mut Trip,
    walker: &Walker,
    from: u32,
    lands: &[(&str, u32)],
) -> Option<(Vec<String>, Said)> {
    let map = map()?;
    let mut here = RoomId(from);
    let mut sent = Vec::new();
    let mut landings = lands.iter();
    for _ in 0..200 {
        match trip.tick(&map, walker, at(here.0)) {
            Said::Send(command) => {
                let (expected, to) = landings.next()?;
                assert_eq!(&command, expected, "after {sent:?}");
                here = RoomId(*to);
                sent.push(command);
            }
            ended @ (Said::Arrived | Said::Failed(_)) => return Some((sent, ended)),
            Said::Hold | Said::Do(_) | Said::Routine(_) | Said::Aside(_) => return None,
        }
    }
    None
}

#[test]
fn a_plain_path_is_walked_one_exit_at_a_time() {
    let lands = [("east", 2), ("east", 3), ("east", 4)];
    let (sent, ended) = walk(&mut Trip::to(RoomId(4)), &Walker::default(), 1, &lands).unwrap();
    assert_eq!((sent.len(), ended), (3, Said::Arrived));
}

/// The unported exit 2 -> 4 costs a tenth of the plain way, and the pathfinder
/// would take it; the trip prices it shut.
#[test]
fn what_this_stage_cannot_cross_is_gone_round_not_failed_at() {
    let lands = [("east", 3), ("east", 4)];
    let (sent, ended) = walk(&mut Trip::to(RoomId(4)), &Walker::default(), 2, &lands).unwrap();
    assert_eq!(
        (sent, ended),
        (vec!["east".into(), "east".into()], Said::Arrived)
    );
}

/// Two exits join 1 and 4. The command sent must be the one the walker can
/// pay for, not the first the map lists.
#[test]
fn of_two_exits_between_the_same_rooms_the_walkers_own_is_sent() {
    let bard = Walker {
        profession: Some("Bard".into()),
        ..Walker::default()
    };
    let (sent, _) = walk(&mut Trip::to(RoomId(4)), &bard, 1, &[("go shortcut", 4)]).unwrap();
    assert_eq!(sent, ["go shortcut"]);
    let warrior = Walker {
        profession: Some("Warrior".into()),
        ..Walker::default()
    };
    let lands = [("east", 2), ("east", 3), ("east", 4)];
    let (sent, _) = walk(&mut Trip::to(RoomId(4)), &warrior, 1, &lands).unwrap();
    assert_eq!(sent.len(), 3, "three seconds beats ten");
}

/// 3 -> 50 sends nothing: 50 is a room only the map has. The hub's command
/// is sent from 3, and the walker lands in 5 without ever being in 50.
#[test]
fn a_pass_through_room_is_crossed_without_being_in_it() {
    let (sent, ended) = walk(
        &mut Trip::to(RoomId(5)),
        &Walker::default(),
        3,
        &[("go alley", 5)],
    )
    .unwrap();
    assert_eq!((sent, ended), (vec!["go alley".into()], Said::Arrived));
}

/// Landing somewhere unexpected is a fact to plan from, not a failure.
#[test]
fn being_carried_elsewhere_replans_from_there() {
    let mut trip = Trip::to(RoomId(7));
    // `north` from 4 should land in 6, and slides the walker back to 3.
    let lands = [("north", 3), ("east", 4), ("north", 6), ("north", 7)];
    let (sent, ended) = walk(&mut trip, &Walker::default(), 4, &lands).unwrap();
    assert_eq!((sent.len(), ended), (4, Said::Arrived));
    assert_eq!(trip.replans(), 1);
}

#[test]
fn a_move_under_way_is_not_sent_twice() {
    let map = map().unwrap();
    let walker = Walker::default();
    let mut trip = Trip::to(RoomId(3));
    assert_eq!(trip.tick(&map, &walker, at(1)), Said::Send("east".into()));
    // Still in 1, and ticked again -- by another frame, by the clock.
    assert_eq!(trip.tick(&map, &walker, at(1)), Said::Hold);
    assert_eq!(
        trip.tick(&map, &walker, Now { here: None, ms: 0 }),
        Said::Hold,
        "the room is not known"
    );
    assert_eq!(trip.tick(&map, &walker, at(2)), Said::Send("east".into()));
}

#[test]
fn a_banned_exit_is_never_offered_again() {
    // Without the direct way, the long way round is walked.
    let mut trip = Trip::to(RoomId(4));
    trip.ban(RoomId(1), RoomId(4));
    let lands = [("east", 2), ("east", 3), ("east", 4)];
    let (sent, ended) = walk(&mut trip, &Walker::default(), 1, &lands).unwrap();
    assert_eq!((sent.len(), ended), (3, Said::Arrived));

    // With both ways in banned there is no route, and the trip says so
    // **before it sets out** rather than walking to the dead end to find out.
    let mut trip = Trip::to(RoomId(4));
    trip.ban(RoomId(1), RoomId(4));
    trip.ban(RoomId(3), RoomId(4));
    let (sent, ended) = walk(&mut trip, &Walker::default(), 1, &[]).unwrap();
    assert_eq!((sent.len(), ended), (0, Said::Failed(Why::NoRoute)));
}

#[test]
fn a_trip_that_cannot_be_made_says_why_and_goes_on_saying_it() {
    let map = map().unwrap();
    let walker = Walker::default();
    let mut trip = Trip::to(RoomId(9));
    assert_eq!(trip.tick(&map, &walker, at(1)), Said::Failed(Why::NoRoute));
    assert_eq!(trip.tick(&map, &walker, at(2)), Said::Failed(Why::NoRoute));
    let mut lost = Trip::to(RoomId(4));
    assert_eq!(
        lost.tick(&map, &walker, at(1234)),
        Said::Failed(Why::OffTheMap)
    );
}

/// A walker thrown back every time is not walked in circles for ever.
#[test]
fn a_trip_gives_up_after_planning_too_many_times() {
    let map = map().unwrap();
    let walker = Walker::default();
    let mut trip = Trip::to(RoomId(7));
    let mut here = RoomId(4);
    let mut sends = 0;
    let ended = loop {
        match trip.tick(&map, &walker, at(here.0)) {
            // `north` always lands back in 3; `east` works.
            Said::Send(command) => {
                sends += 1;
                here = if command == "north" {
                    RoomId(3)
                } else {
                    RoomId(4)
                };
            }
            Said::Hold => panic!("held"),
            ended => break ended,
        }
        assert!(sends < 500, "never gave up");
    };
    assert_eq!(ended, Said::Failed(Why::TooManyReplans));
    assert_eq!(trip.replans(), MAX_REPLANS);
}

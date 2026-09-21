//! Move recovery: `cena_behavior::travel`, stage 2 (`plan/24` §3).
//!
//! The lines are the game's, named by `cena_session::movement::classify` --
//! which is Lich's ladder -- so these tests start from text, as the driver
//! will, and not from a hand-picked enum.

use cena_behavior::travel::{MAX_RESENDS, Now, ORPHAN_MS, STEP_TIMEOUT_MS, Said, Trip, Why};
use cena_map::{Map, Room, RoomId, Walker};
use cena_session::movement::classify;

/// ```text
///   1 --go gate-- 2 --north-- 3      the short way, 1s each
///   1 --east-- 4 --north-- 3         the long way, 5s each
/// ```
const ROOMS: &str = r#"[
  {"id":1,"exits":[{"to":2,"kind":"go","cmd":"go gate","cost":1},
                   {"to":4,"kind":"cardinal","cmd":"east","cost":5}]},
  {"id":2,"exits":[{"to":3,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":4,"exits":[{"to":3,"kind":"cardinal","cmd":"north","cost":5}]},
  {"id":3}
]"#;

fn map() -> Option<Map> {
    let rooms: Vec<Room> = serde_json::from_str(ROOMS).ok()?;
    Map::from_rooms(rooms).ok()
}

fn at(room: u32, ms: u64) -> Now {
    Now {
        here: Some(RoomId(room)),
        ms,
    }
}

fn send(command: &str) -> Said {
    Said::Send(command.to_owned())
}

/// Tell the trip what the game said, as the driver will.
fn hear(trip: &mut Trip, line: &str) {
    let named = classify(line);
    assert!(named.is_some(), "Lich's ladder does not name: {line}");
    if let Some(feedback) = named {
        trip.heard(feedback);
    }
}

#[test]
fn a_closed_gate_is_opened_and_the_move_sent_again() {
    let (map, walker) = (map().unwrap(), Walker::default());
    let mut trip = Trip::to(RoomId(3));
    assert_eq!(trip.tick(&map, &walker, at(1, 0)), send("go gate"));
    hear(&mut trip, "The gate appears to be closed.");
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), send("open gate"));
    assert_eq!(trip.tick(&map, &walker, at(1, 200)), send("go gate"));
    assert_eq!(trip.tick(&map, &walker, at(2, 300)), send("north"));
}

#[test]
fn a_gate_still_shut_after_opening_is_locked_and_gone_round() {
    let (map, walker) = (map().unwrap(), Walker::default());
    let mut trip = Trip::to(RoomId(3));
    trip.tick(&map, &walker, at(1, 0));
    hear(&mut trip, "The gate appears to be closed.");
    trip.tick(&map, &walker, at(1, 100));
    trip.tick(&map, &walker, at(1, 200));
    hear(&mut trip, "The gate appears to be closed.");
    assert_eq!(trip.tick(&map, &walker, at(1, 300)), send("east"));
    assert_eq!(trip.wrong_for_the_map(), [(RoomId(1), RoomId(2))]);
}

/// The game names the exit as one that does not exist. The trip goes round
/// it at once, and keeps the fact for whoever maintains the map.
#[test]
fn no_such_way_is_gone_round_and_remembered() {
    let (map, walker) = (map().unwrap(), Walker::default());
    let mut trip = Trip::to(RoomId(3));
    trip.tick(&map, &walker, at(1, 0));
    hear(&mut trip, "You can't go there.");
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), send("east"));
    assert_eq!(trip.wrong_for_the_map().len(), 1);
}

/// A guard said no. The exit is real, so nothing is recorded against the map.
#[test]
fn a_refusal_is_gone_round_and_not_blamed_on_the_map() {
    let (map, walker) = (map().unwrap(), Walker::default());
    let mut trip = Trip::to(RoomId(3));
    trip.tick(&map, &walker, at(1, 0));
    hear(&mut trip, "An unseen force prevents you.");
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), send("east"));
    assert_eq!(trip.wrong_for_the_map(), []);
}

#[test]
fn roundtime_is_waited_out_before_the_move_is_sent_again() {
    let (map, walker) = (map().unwrap(), Walker::default());
    let mut trip = Trip::to(RoomId(3));
    trip.tick(&map, &walker, at(1, 0));
    hear(&mut trip, "...wait 3 seconds.");
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), Said::Hold);
    assert_eq!(trip.tick(&map, &walker, at(1, 2800)), Said::Hold);
    // Lich sleeps N - 0.2 seconds from when it read the line.
    assert_eq!(trip.tick(&map, &walker, at(1, 2900)), send("go gate"));
}

/// Vellum's recorded bug: a failure line that raced an arrival. The room
/// changed, so the move worked, and the line is about nothing.
#[test]
fn arriving_beats_a_failure_line_that_raced_it() {
    let (map, walker) = (map().unwrap(), Walker::default());
    let mut trip = Trip::to(RoomId(3));
    trip.tick(&map, &walker, at(1, 0));
    hear(&mut trip, "You can't go there.");
    assert_eq!(trip.tick(&map, &walker, at(2, 100)), send("north"));
    assert_eq!(trip.wrong_for_the_map(), []);
    assert_eq!(trip.replans(), 0);
}

/// After an exit is given up, lines keep arriving about it. They must not be
/// read as the verdict on the *next* move.
#[test]
fn lines_about_an_abandoned_move_are_not_blamed_on_the_next() {
    let (map, walker) = (map().unwrap(), Walker::default());
    let mut trip = Trip::to(RoomId(3));
    trip.tick(&map, &walker, at(1, 0));
    hear(&mut trip, "You can't go there.");
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), send("east"));
    // Still about `go gate`, arriving late.
    hear(&mut trip, "You can't go there.");
    assert_eq!(trip.tick(&map, &walker, at(1, 200)), Said::Hold);
    assert_eq!(trip.wrong_for_the_map().len(), 1, "`east` was not blamed");
    // Once the window has passed, a line is about the move under way again.
    hear(&mut trip, "You can't go there.");
    assert_eq!(
        trip.tick(&map, &walker, at(1, 100 + ORPHAN_MS)),
        Said::Failed(Why::NoRoute)
    );
}

/// Silence is not failure. The move is sent again; and only a trip that has
/// never left its first room may conclude the exit is at fault.
#[test]
fn silence_resends_and_bans_only_from_the_first_room() {
    let (map, walker) = (map().unwrap(), Walker::default());
    let mut trip = Trip::to(RoomId(3));
    trip.tick(&map, &walker, at(1, 0));
    let mut ms = 0;
    for _ in 0..MAX_RESENDS {
        assert_eq!(
            trip.tick(&map, &walker, at(1, ms + STEP_TIMEOUT_MS - 1)),
            Said::Hold
        );
        ms += STEP_TIMEOUT_MS;
        assert_eq!(trip.tick(&map, &walker, at(1, ms)), send("go gate"));
    }
    ms += STEP_TIMEOUT_MS;
    // Never left room 1: the gate is banned, and the long way is taken.
    assert_eq!(trip.tick(&map, &walker, at(1, ms)), send("east"));
    assert_eq!(
        trip.wrong_for_the_map(),
        [],
        "silence says nothing about the map"
    );
}

#[test]
fn lag_further_along_never_costs_an_exit() {
    let (map, walker) = (map().unwrap(), Walker::default());
    let mut trip = Trip::to(RoomId(3));
    trip.tick(&map, &walker, at(1, 0));
    assert_eq!(trip.tick(&map, &walker, at(2, 10)), send("north"));
    let mut ms = 10;
    for _ in 0..MAX_RESENDS {
        ms += STEP_TIMEOUT_MS;
        assert_eq!(trip.tick(&map, &walker, at(2, ms)), send("north"));
    }
    ms += STEP_TIMEOUT_MS;
    // Room 2 has one way on. Had it been banned there would be no route;
    // it was not, so the trip plans again and tries it again.
    assert_eq!(trip.tick(&map, &walker, at(2, ms)), send("north"));
    assert_eq!(trip.replans(), 1);
}

#[test]
fn a_walker_on_the_ground_stands_before_it_moves() {
    let map = map().unwrap();
    let mut walker = Walker {
        posture: Some("prone".into()),
        ..Walker::default()
    };
    let mut trip = Trip::to(RoomId(3));
    assert_eq!(trip.tick(&map, &walker, at(1, 0)), send("stand"));
    walker.posture = Some("standing".into());
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), send("go gate"));
}

#[test]
fn a_stunned_walker_waits_and_then_sends_what_it_sent() {
    let map = map().unwrap();
    let mut walker = Walker::default();
    let mut trip = Trip::to(RoomId(3));
    trip.tick(&map, &walker, at(1, 0));
    walker.flags.insert("stunned".into(), true);
    hear(&mut trip, "You are still stunned.");
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), Said::Hold);
    assert_eq!(trip.tick(&map, &walker, at(1, 5000)), Said::Hold);
    walker.flags.insert("stunned".into(), false);
    assert_eq!(trip.tick(&map, &walker, at(1, 6000)), send("go gate"));
}

/// Lich: pitch dark is *success*. No room change will say so, so the trip
/// takes the game's word and walks on from where it should now be.
#[test]
fn pitch_dark_is_taken_as_having_arrived() {
    let (map, walker) = (map().unwrap(), Walker::default());
    let mut trip = Trip::to(RoomId(3));
    trip.tick(&map, &walker, at(1, 0));
    hear(&mut trip, "It's pitch dark and you can't see a thing!");
    // Still "in" room 1 as far as anyone can see; the next move is room 2's.
    assert_eq!(trip.tick(&map, &walker, at(1, 100)), send("north"));
    assert_eq!(trip.tick(&map, &walker, at(3, 200)), Said::Arrived);
}

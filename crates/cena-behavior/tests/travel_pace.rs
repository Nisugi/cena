//! Batched travel (`travel/pace.rs`; the author's rule, `plan/16` §5.2g):
//! plain moves sent ahead of the walker, 1 + the typeahead at a time, the
//! typeahead learned from the game's refusal.

mod drive_support;
mod ready;

use std::time::Duration;

use cena_behavior::travel::{Ended, GAP_MS, Now, SETTLE_MS, Said, Trip};
use cena_map::{Map, Room, RoomId, Walker};
use drive_support::{arrival, set_out_as};
use tokio_util::sync::CancellationToken;

/// ```text
///   1 -e- 2 -e- 3 -e- 4 -n- 5 -n- 6 -n- 7 -go door- 8
/// ```
/// `go door` is a plain move too; 7 -> 8 by a scripted crossing ends a
/// batch.
const ROOMS: &str = r#"[
  {"id":1,"exits":[{"to":2,"kind":"cardinal","cmd":"east","cost":1}]},
  {"id":2,"exits":[{"to":3,"kind":"cardinal","cmd":"east","cost":1}]},
  {"id":3,"exits":[{"to":4,"kind":"cardinal","cmd":"east","cost":1}]},
  {"id":4,"exits":[{"to":5,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":5,"exits":[{"to":6,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":6,"exits":[{"to":7,"kind":"go","cmd":"go door","cost":1}]},
  {"id":7,"exits":[{"to":8,"kind":"scripted","steps":[{"put":"push button"},{"move":"go gate"}],"cost":1}]},
  {"id":8}
]"#;

fn map() -> Option<Map> {
    let rooms: Vec<Room> = serde_json::from_str(ROOMS).ok()?;
    Map::from_rooms(rooms).ok()
}

fn standing(typeahead: Option<&str>) -> Walker {
    let mut walker = Walker {
        posture: Some("standing".to_owned()),
        ..Walker::default()
    };
    if let Some(typeahead) = typeahead {
        walker
            .settings
            .insert("typeahead".to_owned(), typeahead.to_owned());
    }
    walker
}

fn at(room: u32, ms: u64) -> Now {
    Now {
        here: Some(RoomId(room)),
        ms,
    }
}

fn all(lines: &[&str]) -> Said {
    Said::SendAll(
        lines.iter().map(|line| (*line).to_owned()).collect(),
        GAP_MS,
    )
}

/// The first move goes alone (go2 1.2); then three at a time on the
/// author's typeahead of 2, topped up as they land, and a batch ends where a
/// scripted crossing begins.
#[test]
fn plain_moves_go_ahead_three_at_a_time_after_the_first() {
    let map = map().unwrap();
    let walker = standing(None);
    let mut trip = Trip::to(RoomId(8));
    assert_eq!(
        trip.tick(&map, &walker, at(1, 0)),
        Said::Send("east".into())
    );
    assert_eq!(
        trip.tick(&map, &walker, at(2, 100)),
        all(&["east", "east", "north"])
    );
    // One landed: one more goes, to keep three unanswered.
    assert_eq!(trip.tick(&map, &walker, at(3, 250)), all(&["north"]));
    // Two more landed: the plain way runs out at 7, so `go door` and no more.
    assert_eq!(trip.tick(&map, &walker, at(5, 400)), all(&["go door"]));
    assert_eq!(trip.tick(&map, &walker, at(6, 550)), Said::Hold);
    // All landed: the scripted crossing goes one step at a time.
    assert_eq!(
        trip.tick(&map, &walker, at(7, 700)),
        Said::Send("push button".into())
    );
}

/// *Sorry, you may only type ahead 1 command.*: the walker is let settle,
/// the trip plans again from where it is, the batch is two, and the
/// typeahead is handed out to keep.
#[test]
fn a_typeahead_refusal_sets_the_typeahead_and_plans_again() {
    let map = map().unwrap();
    let walker = standing(None);
    let mut trip = Trip::to(RoomId(8));
    let _ = trip.tick(&map, &walker, at(1, 0));
    let _ = trip.tick(&map, &walker, at(2, 100));
    trip.heard("Sorry, you may only type ahead 1 command.");
    assert_eq!(trip.tick(&map, &walker, at(3, 200)), Said::Hold);
    assert_eq!(trip.typeahead_learned(), Some(1));
    // Still moving: the settling starts again.
    assert_eq!(trip.tick(&map, &walker, at(4, 900)), Said::Hold);
    assert_eq!(
        trip.tick(&map, &walker, at(4, 900 + SETTLE_MS)),
        all(&["north", "north"]),
        "from where it stopped, two at a time"
    );
}

/// A room off the way stops the batch; the trip plans again from it once
/// the walker is still.
#[test]
fn a_room_off_the_way_stops_sending_and_plans_again() {
    let map = map().unwrap();
    let walker = standing(None);
    let mut trip = Trip::to(RoomId(8));
    let _ = trip.tick(&map, &walker, at(1, 0));
    let _ = trip.tick(&map, &walker, at(2, 100));
    // Back in 1, which the batch sent from 2 never leads to.
    assert_eq!(trip.tick(&map, &walker, at(1, 200)), Said::Hold);
    assert_eq!(
        trip.tick(&map, &walker, at(1, 200 + SETTLE_MS)),
        all(&["east", "east", "east"])
    );
}

/// go2's own `typeahead` setting is the batch's size; `0`, and go2's
/// `delay`, send one move at a time.
#[test]
fn the_walkers_typeahead_sets_the_size_and_zero_or_a_delay_turns_it_off() {
    let map = map().unwrap();
    let one = standing(Some("1"));
    let mut trip = Trip::to(RoomId(8));
    let _ = trip.tick(&map, &one, at(1, 0));
    assert_eq!(trip.tick(&map, &one, at(2, 100)), all(&["east", "east"]));

    let none = standing(Some("0"));
    let mut trip = Trip::to(RoomId(8));
    let _ = trip.tick(&map, &none, at(1, 0));
    assert_eq!(
        trip.tick(&map, &none, at(2, 100)),
        Said::Send("east".into())
    );

    let mut delayed = standing(None);
    delayed.settings.insert("delay".into(), "1".into());
    let mut trip = Trip::to(RoomId(8));
    let _ = trip.tick(&map, &delayed, at(1, 0));
    assert_eq!(
        trip.tick(&map, &delayed, at(2, 100)),
        Said::Send("east".into())
    );
}

/// 1 -> 2 -> 4 -> 3, all north: the harness walks to 3.
const NORTH: &str = r#"[
  {"id":1,"uid":[1001],"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":2,"uid":[1002],"exits":[{"to":4,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":4,"uid":[1004],"exits":[{"to":3,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":3,"uid":[1003]}
]"#;

/// Over a real session: with the game's replies held, the first move goes
/// alone, and once it lands the other two go together, before either lands.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_driver_sends_a_batch_before_its_moves_land() {
    let stop = CancellationToken::new();
    let (walk, transcript, session, _, _) = set_out_as(&stop, NORTH, None, |state| {
        state.status.set("standing", true);
    });
    transcript.hold_replies();
    for uid in [1002, 1004, 1003] {
        transcript.answer("north", &arrival(uid));
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(transcript.lines(), ["north"], "the first move goes alone");
    transcript.release_one();
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        transcript.lines(),
        ["north", "north", "north"],
        "the other two before either landed"
    );
    transcript.release_replies();
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    session.cancel();
}

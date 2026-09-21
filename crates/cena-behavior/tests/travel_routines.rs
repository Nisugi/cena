//! Named routines, through the real driver and a scripted game (`plan/24`
//! stage 6). What each solver decides is tested by table beside it; this is
//! the loop that runs them: a routine's moves going back through the trip,
//! its answers reaching it, and the trip looking at where it landed.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cena_behavior::travel::{Ended, TravelNotes, Travelled, travel};
use cena_map::{Map, Room, RoomId};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{AuthorityToken, CommandId, Session};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";

fn arrival(uid: u32) -> Vec<u8> {
    format!("<nav rm='{uid}'/>\n<prompt time=\"2\">&gt;</prompt>\n").into_bytes()
}

/// An arrival in a room that shows a thing.
fn arrival_showing(uid: u32, noun: &str) -> Vec<u8> {
    format!(
        "<nav rm='{uid}'/>\n<component id='room objs'>You also see a <a exist=\"5\" \
         noun=\"{noun}\">{noun}</a>.</component>\n<prompt time=\"2\">&gt;</prompt>\n"
    )
    .into_bytes()
}

/// A character in room 1 (the game's 1001), walking to `goal`.
fn set_out(
    rooms: &'static str,
    goal: u32,
    mut notes: TravelNotes,
) -> (
    JoinHandle<Option<Travelled>>,
    TranscriptHandle,
    CancellationToken,
) {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let session_cancel = session.cancel_token();
    let (mut snapshot, events) = session.subscribe();
    snapshot.state.room.id = Some("1001".into());
    tokio::spawn(session.into_actor().run());
    let walk = tokio::spawn(async move {
        let rooms: Vec<Room> = serde_json::from_str(rooms).ok()?;
        let map = Map::from_rooms(rooms).ok()?;
        let next = Arc::new(AtomicU64::new(0));
        let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
        let stop = CancellationToken::new();
        let travelled = travel(
            &handle,
            &stop,
            ids,
            AuthorityToken(1),
            (snapshot, events),
            &map,
            RoomId(goal),
            &mut notes,
            |_| {},
        )
        .await;
        Some(travelled)
    });
    (walk, transcript, session_cancel)
}

/// The guild door 1 -> 2, and a long way round by 3.
const DOOR: &str = r#"[
  {"id":1,"uid":[1001],"exits":[
     {"to":2,"kind":"scripted","routine":{"name":"guild_password"},"cost":1},
     {"to":3,"kind":"cardinal","cmd":"east","cost":50}]},
  {"id":3,"uid":[1003],"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":50}]},
  {"id":2,"uid":[1002]}
]"#;

fn with_password() -> TravelNotes {
    let mut notes = TravelNotes::default();
    notes
        .settings
        .insert("rogue_password".into(), "kick, slap".into());
    notes
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_routine_is_run_and_the_trip_walks_on_from_where_it_landed() {
    let (walk, transcript, session) = set_out(DOOR, 2, with_password());
    transcript.answer("go door", &arrival(1002));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(
        transcript.lines(),
        ["lean door", "kick door", "slap door", "go door"]
    );
    session.cancel();
}

/// The door does not open: the routine ran and the walker never moved, so
/// the exit is given up for the trip -- not tried twenty times.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_routine_that_moved_nobody_is_not_tried_again() {
    let (walk, transcript, session) = set_out(DOOR, 2, with_password());
    transcript.answer("east", &arrival(1003));
    transcript.answer("north", &arrival(1002));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    let lines = transcript.lines();
    assert_eq!(
        lines.iter().filter(|line| *line == "lean door").count(),
        1,
        "{lines:?}"
    );
    assert_eq!(lines[lines.len() - 2..], ["east", "north"]);
    session.cancel();
}

/// A circuit 1 -> 4 -> 1, and a thread that shows in 4 and leads to 2 --
/// though the map's exit says 9, because nobody knows where a thread lands.
const RIFT: &str = r#"[
  {"id":1,"uid":[1001],"exits":[
     {"to":9,"kind":"scripted","cost":1,"routine":{"name":"patrol",
        "starts":[1,4],"dirs":["north","south"],
        "landmarks":[{"noun":"thread","enter":"climb thread"}],"after":["stand"]}}]},
  {"id":4,"uid":[1004]},
  {"id":2,"uid":[1002],"exits":[{"to":9,"kind":"cardinal","cmd":"out","cost":1}]},
  {"id":9,"uid":[1009]}
]"#;

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_patrol_goes_round_until_it_sees_the_way_out_and_then_plans_again() {
    let (walk, transcript, session) = set_out(RIFT, 9, TravelNotes::default());
    transcript.answer("north", &arrival_showing(1004, "thread"));
    transcript.answer("climb thread", &arrival(1002));
    transcript.answer("out", &arrival(1009));
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(
        transcript.lines(),
        ["north", "climb thread", "stand", "out"]
    );
    session.cancel();
}

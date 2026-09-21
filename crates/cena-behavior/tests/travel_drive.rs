//! The driver: `cena_behavior::travel::travel` (`plan/24` stage 4c), over a
//! real session and a scripted game.
//!
//! Virtual time throughout, for the reason `stop_and_interleave.rs` gives:
//! what is measured is how many awaits a stop had to cross, not the machine.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cena_behavior::BehaviorError;
use cena_behavior::travel::{Ended, TravelNotes, Travelled, travel};
use cena_map::{Map, Room, RoomId};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::hands::Hand;
use cena_session::{AuthorityToken, CommandId, Session};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// `plan/12` §4.3.
const PREEMPT_GRACE: Duration = Duration::from_millis(250);

/// What the game says to a command nothing was scripted for: nothing, and a
/// prompt. The character does not move.
const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";

/// ```text
///   1 --north-- 2 --(hands emptied) climb rope (hands filled)-- 3
/// ```
/// The game's numbers are the ids plus a thousand, so a test that confused
/// the two would not find its room.
const ROOMS: &str = r#"[
  {"id":1,"uid":[1001],"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":2,"uid":[1002],"exits":[{"to":3,"kind":"scripted","cost":1,
     "steps":[{"empty_hands":null},{"move":"climb rope"},{"fill_hands":null}]}]},
  {"id":3,"uid":[1003]}
]"#;

fn arrival(uid: u32) -> Vec<u8> {
    format!("<nav rm='{uid}'/>\n<prompt time=\"2\">&gt;</prompt>\n").into_bytes()
}

const SWORD_GONE: &[u8] = b"<right>Empty</right>\n<prompt time=\"2\">&gt;</prompt>\n";
const SWORD_BACK: &[u8] =
    b"<right exist=\"11\" noun=\"sword\">broadsword</right>\n<prompt time=\"3\">&gt;</prompt>\n";

/// A character in room 1 with a broadsword, walking to room 3.
fn set_out(
    stop: &CancellationToken,
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
    snapshot.state.right_hand = Hand::Holding {
        id: Some("11".into()),
        noun: Some("sword".into()),
        name: "broadsword".into(),
    };
    snapshot.state.left_hand = Hand::Empty;
    tokio::spawn(session.into_actor().run());

    let stop = stop.clone();
    let walk = tokio::spawn(async move {
        // `None` is a broken fixture, which every test unwraps into a failure.
        let rooms: Vec<Room> = serde_json::from_str(ROOMS).ok()?;
        let map = Map::from_rooms(rooms).ok()?;
        let next = Arc::new(AtomicU64::new(0));
        let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
        let mut notes = TravelNotes::default();
        let travelled = travel(
            &handle,
            &stop,
            ids,
            AuthorityToken(1),
            (snapshot, events),
            &map,
            RoomId(3),
            &mut notes,
            |_| {},
        )
        .await;
        Some(travelled)
    });
    (walk, transcript, session_cancel)
}

/// Let virtual time run until `line` has been written. `false` if it never is.
async fn until_written(transcript: &TranscriptHandle, line: &str) -> bool {
    for _ in 0..200 {
        if transcript.lines().iter().any(|written| written == line) {
            return true;
        }
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    false
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn it_walks_there_storing_and_taking_back_on_the_way() {
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop);
    transcript.answer("north", &arrival(1002));
    transcript.answer("store right", SWORD_GONE);
    transcript.answer("climb rope", &arrival(1003));
    transcript.answer("get #11", SWORD_BACK);

    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert_eq!(travelled.ended, Ended::Arrived);
    assert_eq!(
        transcript.lines(),
        ["north", "store right", "climb rope", "get #11"]
    );
    assert!(travelled.still_stored.is_empty(), "the sword came back");
    session.cancel();
}

/// The author's ruling, 2026-09-21: a stop sends one command per stored item
/// and then stops. Here the rope is never climbed, so the stop lands with the
/// sword stored and a move on the wire.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_stop_takes_back_what_is_stored_once_and_stops() {
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop);
    transcript.answer("north", &arrival(1002));
    transcript.answer("store right", SWORD_GONE);
    let climbing = until_written(&transcript, "climb rope").await;
    assert!(climbing, "never reached the rope: {:?}", transcript.lines());

    let at_stop = Instant::now();
    stop.cancel();
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    let elapsed = at_stop.elapsed();

    assert_eq!(travelled.ended, Ended::Stopped(BehaviorError::Cancelled));
    assert!(elapsed <= PREEMPT_GRACE, "stop took {elapsed:?}");
    // Sharper than the budget, because the budget cannot see the defect: a
    // hold that is not raced against the stop still ends at its next beat,
    // and a beat IS the grace. In virtual time a raced stop crosses no timer.
    assert_eq!(elapsed, Duration::ZERO, "the stop waited out a timer");
    let lines = transcript.lines();
    assert_eq!(lines.last().map(String::as_str), Some("get #11"));
    assert_eq!(
        lines.iter().filter(|line| *line == "get #11").count(),
        1,
        "one get, and then it stops: {lines:?}"
    );
    // Nothing saw it come back, so it is still reported.
    assert_eq!(travelled.still_stored.len(), 1);
    assert_eq!(travelled.still_stored[0].id, "11");

    // ...and it really has stopped: time passing sends nothing more.
    tokio::time::advance(Duration::from_mins(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(transcript.lines().len(), lines.len());
    session.cancel();
}

/// With nothing stored, a stop sends nothing at all (`plan/12` §4.3).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_stop_with_nothing_stored_sends_nothing() {
    let stop = CancellationToken::new();
    let (walk, transcript, session) = set_out(&stop);
    // `north` is answered with a bare prompt: the walker is still in room 1.
    let walking = until_written(&transcript, "north").await;
    assert!(walking, "never set out: {:?}", transcript.lines());
    let sent = transcript.lines().len();

    let at_stop = Instant::now();
    stop.cancel();
    let travelled = walk.await.expect("the walk must not panic").unwrap();
    assert!(at_stop.elapsed() <= PREEMPT_GRACE);
    assert_eq!(travelled.ended, Ended::Stopped(BehaviorError::Cancelled));
    assert!(travelled.still_stored.is_empty());
    assert_eq!(transcript.lines().len(), sent, "a stop is not a send");
    session.cancel();
}

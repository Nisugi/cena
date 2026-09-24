//! The harness itself: see the module above for what it is for. Here, not
//! in `mod.rs`, because a `mod.rs` stays a facade (`cena-arch-tests`,
//! `facade_files_stay_facades`).

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use cena_behavior::travel::{TravelNotes, Travelled, travel, travel_holding};
use cena_map::{Map, Room, RoomId};
use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::group::{GroupEvent, Member};
use cena_session::hands::Hand;
use cena_session::{
    AuthorityToken, CommandId, Event, GameState, GenerationCell, NoticeKind, Session, SessionHandle,
};
use tokio::sync::broadcast::Receiver;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// `plan/12` §4.3.
pub const PREEMPT_GRACE: Duration = Duration::from_millis(250);

/// What the game says to a command nothing was scripted for: nothing, and a
/// prompt. The character does not move.
pub const PROMPT: &[u8] = b"<prompt time=\"1\">&gt;</prompt>\n";

/// ```text
///   1 --north-- 2 --(hands emptied) climb rope (hands filled)-- 3
/// ```
/// The game's numbers are the ids plus a thousand, so a test that confused
/// the two would not find its room.
pub const ROOMS: &str = r#"[
  {"id":1,"uid":[1001],"exits":[{"to":2,"kind":"cardinal","cmd":"north","cost":1}]},
  {"id":2,"uid":[1002],"exits":[{"to":3,"kind":"scripted","cost":1,
     "steps":[{"empty_hands":null},{"move":"climb rope"},{"fill_hands":null}]}]},
  {"id":3,"uid":[1003]}
]"#;

pub fn arrival(uid: u32) -> Vec<u8> {
    format!("<nav rm='{uid}'/>\n<prompt time=\"2\">&gt;</prompt>\n").into_bytes()
}

pub const REFUSED: &[u8] = b"You can't go there.\n<prompt time=\"2\">&gt;</prompt>\n";
pub const SWORD_GONE: &[u8] = b"<right>Empty</right>\n<prompt time=\"2\">&gt;</prompt>\n";
pub const SWORD_BACK: &[u8] =
    b"<right exist=\"11\" noun=\"sword\">broadsword</right>\n<prompt time=\"3\">&gt;</prompt>\n";

/// A character in room 1 with a broadsword, walking to room 3.
pub fn set_out(
    stop: &CancellationToken,
    rooms: &'static str,
) -> (
    JoinHandle<Option<Travelled>>,
    TranscriptHandle,
    CancellationToken,
) {
    let (walk, transcript, session, _) = set_out_with(stop, rooms, &[]);
    (walk, transcript, session)
}

/// [`set_out`], grouped with `company`, and with the handle a player would
/// type into: the only way a test can make the game say something the walker
/// did not ask for.
pub fn set_out_with(
    stop: &CancellationToken,
    rooms: &'static str,
    company: &[Member],
) -> (
    JoinHandle<Option<Travelled>>,
    TranscriptHandle,
    CancellationToken,
    SessionHandle,
) {
    let (walk, transcript, session, typed, _) = set_out_as(stop, rooms, None, |state| {
        for member in company {
            state.group.apply(&GroupEvent::Joined(member.clone()));
        }
    });
    (walk, transcript, session, typed)
}

/// [`set_out`], with whatever else the character knows as it sets out.
pub fn set_out_as(
    stop: &CancellationToken,
    rooms: &'static str,
    last_room: Option<u32>,
    knows: impl FnOnce(&mut GameState),
) -> (
    JoinHandle<Option<Travelled>>,
    TranscriptHandle,
    CancellationToken,
    SessionHandle,
    Receiver<Event>,
) {
    let (walk, transcript, session, typed, told, _) =
        set_out_on_a_connection(stop, rooms, last_room, knows);
    (walk, transcript, session, typed, told)
}

/// [`set_out_as`], and the session's generation cell: what a supervisor
/// advances between connections, so a test can move the connection under
/// the walk.
#[allow(clippy::type_complexity)] // a test harness's tuple, named at each use
pub fn set_out_on_a_connection(
    stop: &CancellationToken,
    rooms: &'static str,
    last_room: Option<u32>,
    knows: impl FnOnce(&mut GameState),
) -> (
    JoinHandle<Option<Travelled>>,
    TranscriptHandle,
    CancellationToken,
    SessionHandle,
    Receiver<Event>,
    GenerationCell,
) {
    set_out_claiming(stop, rooms, last_room, knows, false)
}

/// [`set_out`], by a caller that claims the authority itself and walks with
/// `travel_holding`, as Hunt does: the walk must leave the claim in place.
pub fn set_out_holding(
    stop: &CancellationToken,
    rooms: &'static str,
) -> (
    JoinHandle<Option<Travelled>>,
    TranscriptHandle,
    CancellationToken,
    SessionHandle,
) {
    let (walk, transcript, session, typed, _, _) =
        set_out_claiming(stop, rooms, None, |_| {}, true);
    (walk, transcript, session, typed)
}

#[allow(clippy::type_complexity)] // as above
fn set_out_claiming(
    stop: &CancellationToken,
    rooms: &'static str,
    last_room: Option<u32>,
    knows: impl FnOnce(&mut GameState),
    holding: bool,
) -> (
    JoinHandle<Option<Travelled>>,
    TranscriptHandle,
    CancellationToken,
    SessionHandle,
    Receiver<Event>,
    GenerationCell,
) {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let cell = session.generation_cell();
    let session_cancel = session.cancel_token();
    let (mut snapshot, events) = session.subscribe();
    // A second listener, as a frontend would be: what the player is told.
    let (_, told) = session.subscribe();
    let (_, ready) = session.subscribe();
    snapshot.state.room.id = Some("1001".into());
    snapshot.state.right_hand = Hand::Holding {
        id: Some("11".into()),
        noun: Some("sword".into()),
        name: "broadsword".into(),
    };
    snapshot.state.left_hand = Hand::Empty;
    knows(&mut snapshot.state);
    let typed = handle.clone();
    tokio::spawn(session.into_actor().run());

    let stop = stop.clone();
    let walk = tokio::spawn(async move {
        // Inside the task: this function is not async, and the walk is what
        // must not start before the session is `Ready`.
        crate::ready::until_ready(ready).await.ok()?;
        // `None` is a broken fixture, which every test unwraps into a failure.
        let rooms: Vec<Room> = serde_json::from_str(rooms).ok()?;
        let map = Map::from_rooms(rooms).ok()?;
        let next = Arc::new(AtomicU64::new(0));
        let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
        let mut notes = TravelNotes {
            last_room,
            ..TravelNotes::default()
        };
        let joined = (snapshot, events);
        // Boxed: the driver's future is large now that routines recurse.
        let travelled = if holding {
            handle.claim(AuthorityToken(1)).await.ok()?;
            let walk = travel_holding(
                &handle,
                &stop,
                ids,
                AuthorityToken(1),
                joined,
                &map,
                RoomId(3),
                &mut notes,
                |_| {},
            );
            Box::pin(walk).await
        } else {
            let walk = travel(
                &handle,
                &stop,
                ids,
                AuthorityToken(1),
                joined,
                &map,
                RoomId(3),
                &mut notes,
                |_| {},
            );
            Box::pin(walk).await
        };
        Some(travelled)
    });
    (walk, transcript, session_cancel, typed, told, cell)
}

/// Let virtual time run until `line` has been written. `false` if it never is.
pub async fn until_written(transcript: &TranscriptHandle, line: &str) -> bool {
    for _ in 0..200 {
        if transcript.lines().iter().any(|written| written == line) {
            return true;
        }
        tokio::time::advance(Duration::from_millis(50)).await;
        tokio::task::yield_now().await;
    }
    false
}

/// Every notice published so far, as `(kind, text)`.
pub fn told_so_far(told: &mut Receiver<Event>) -> Vec<(NoticeKind, String)> {
    let mut said = Vec::new();
    while let Ok(event) = told.try_recv() {
        if let Event::Notice(notice) = event {
            said.push((notice.kind, notice.lines().join(" ")));
        }
    }
    said
}

/// `heavy_key.rb`, 1 -> 3 by the spiked gate; and the long way by 2.
pub const GATE: &str = r#"[
  {"id":1,"uid":[1001],"exits":[
     {"to":3,"kind":"scripted","cost":1,"steps":[{"take_out":"heavy key"},
        {"put":"unlock spiked gate with my heavy key"},{"put_back":null},
        {"move":"go spiked gate"}]},
     {"to":2,"kind":"cardinal","cmd":"east","cost":50}]},
  {"id":2,"uid":[1002],"exits":[{"to":3,"kind":"cardinal","cmd":"north","cost":50}]},
  {"id":3,"uid":[1003]}
]"#;

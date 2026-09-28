//! A behavior that fell behind the game's events (the crate review of
//! 2026-09-28, R1): a lag is recorded, and the state is taken afresh from
//! the session before anything more is decided; a stream that cannot be
//! taken afresh stops the behavior before it sends anything from a state
//! with holes in it.

mod drive_support;
mod ready;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use cena_behavior::BehaviorError;
use cena_behavior::travel::{Ended, Heard, TravelNotes, travel};
use cena_map::{Map, Room, RoomId};
use cena_platform::AnsweringSource;
use cena_session::hands::Hand;
use cena_session::{
    AuthorityToken, CommandId, Event, Generation, ObservedEvent, Session, SessionId, State,
};
use drive_support::{PROMPT, ROOMS};
use tokio::sync::broadcast;
use tokio::sync::broadcast::error::TryRecvError;
use tokio_util::sync::CancellationToken;

/// Long enough, in the tests' own time, for any behavior here to stop: one
/// that runs on past it has decided from a state with holes in it.
const LIMIT: std::time::Duration = std::time::Duration::from_mins(1);

/// A stream that has lost events: one slot, three sent. The sender is kept,
/// or the stream would close, and a closed stream ends a behavior another
/// way.
fn fallen_behind() -> (
    broadcast::Sender<ObservedEvent>,
    broadcast::Receiver<ObservedEvent>,
) {
    let (sender, events) = broadcast::channel(1);
    for cursor in 1..=3 {
        let _ = sender.send(ObservedEvent {
            session: SessionId::FIRST,
            generation: Generation::FIRST,
            cursor,
            event: Event::StateChanged(State::Ready),
        });
    }
    (sender, events)
}

/// A stream that lagged says so; joined with the session's observer, it is
/// taken afresh -- the session's state, and its stream from just after it --
/// and is no longer behind; joined without, there is nothing to take it
/// from.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_lagged_stream_is_taken_afresh_from_the_session() {
    let (source, _transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let observer = session.observer();
    let (_, ready) = session.subscribe();
    tokio::spawn(session.into_actor().run());
    ready::until_ready(ready).await.expect("ready");

    let (_kept, events) = fallen_behind();
    let mut heard = Heard::rejoinable(observer, events);
    assert!(!heard.behind());
    assert!(matches!(heard.try_recv(), Err(TryRecvError::Lagged(_))));
    assert!(heard.behind(), "the loss is recorded");
    let snapshot = heard.again().await.expect("taken afresh");
    assert_eq!(snapshot.lifecycle, State::Ready, "the session's own state");
    assert!(!heard.behind(), "and the loss forgotten");
    assert!(
        matches!(heard.try_recv(), Err(TryRecvError::Empty)),
        "the session's stream now, from just after its state"
    );

    let (_kept, events) = fallen_behind();
    let mut plain = Heard::from(events);
    assert!(
        plain.recv().await.is_err(),
        "a lag, waited for as well as polled"
    );
    assert!(plain.behind());
    assert!(plain.again().await.is_none(), "no session to ask");
}

/// A walk whose stream has fallen behind, and cannot be taken afresh,
/// stops before it sends anything: it would have walked from a state with
/// holes in it.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_walk_that_fell_behind_stops_before_it_sends() {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let (mut snapshot, _) = session.subscribe();
    let (_, ready) = session.subscribe();
    snapshot.state.room.id = Some("1001".into());
    snapshot.state.right_hand = Hand::Empty;
    snapshot.state.left_hand = Hand::Empty;
    tokio::spawn(session.into_actor().run());
    ready::until_ready(ready).await.expect("ready");
    let written = transcript.written_count();

    let rooms: Vec<Room> = serde_json::from_str(ROOMS).expect("the rooms");
    let map = Map::from_rooms(rooms).expect("the map");
    let next = Arc::new(AtomicU64::new(0));
    let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
    let (_kept, events) = fallen_behind();
    let stop = CancellationToken::new();
    let mut notes = TravelNotes::default();
    let walk = travel(
        &handle,
        &stop,
        ids,
        AuthorityToken(1),
        (snapshot, Heard::from(events)),
        &map,
        RoomId(3),
        &mut notes,
        |_| {},
    );
    let travelled = tokio::time::timeout(LIMIT, Box::pin(walk))
        .await
        .expect("the walk stops");
    assert_eq!(travelled.ended, Ended::Stopped(BehaviorError::FellBehind));
    assert_eq!(transcript.written_count(), written, "nothing sent");
}

/// A logged-in session over a scripted game: its handle, a snapshot, and
/// what the client has written so far.
async fn logged_in() -> Result<
    (
        cena_session::SessionHandle,
        cena_session::Snapshot,
        cena_platform::TranscriptHandle,
    ),
    String,
> {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let (mut snapshot, _) = session.subscribe();
    let (_, ready) = session.subscribe();
    snapshot.state.room.id = Some("1001".into());
    tokio::spawn(session.into_actor().run());
    ready::until_ready(ready).await?;
    Ok((handle, snapshot, transcript))
}

/// A hunt whose stream has fallen behind, and cannot be taken afresh,
/// stops at its first turn, before it decides or sends anything.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_hunt_that_fell_behind_stops_before_it_sends() {
    use cena_behavior::hunt::{Hunt, HuntEnd, Profile, hunt};
    let (handle, snapshot, transcript) = logged_in().await.expect("logged in");
    let written = transcript.written_count();
    let rooms: Vec<Room> = serde_json::from_str(ROOMS).expect("the rooms");
    let map = Map::from_rooms(rooms).expect("the map");
    let profile =
        Profile::parse("[rooms]\nallowed=[1]\nhunting=1\nresting=1\n").expect("a profile");
    let next = Arc::new(AtomicU64::new(0));
    let ids = move || CommandId(next.fetch_add(1, Ordering::Relaxed));
    handle.claim(AuthorityToken(1)).await.expect("claimed");
    let (_kept, events) = fallen_behind();
    let stop = CancellationToken::new();
    let heartbeat = cena_behavior::watchdog::Heartbeat::default();
    let hunting = Box::pin(hunt(
        &handle,
        &stop,
        ids,
        AuthorityToken(1),
        (snapshot, Heard::from(events)),
        &map,
        Hunt::new(profile, 1),
        &heartbeat,
        TravelNotes::default(),
        |_| {},
        |_| {},
    ));
    let ended = tokio::time::timeout(LIMIT, hunting)
        .await
        .expect("the hunt stops");
    assert!(
        matches!(ended, HuntEnd::Stopped(BehaviorError::FellBehind)),
        "{ended:?}"
    );
    assert_eq!(transcript.written_count(), written, "nothing sent");
}

/// A batch whose stream has fallen behind, and cannot be taken afresh,
/// stops before its first line.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_batch_that_fell_behind_stops_before_it_sends() {
    use cena_behavior::batch::{Desk, Halt, Hydra, Job, Line, Ran, multi};
    let (handle, snapshot, transcript) = logged_in().await.expect("logged in");
    let written = transcript.written_count();
    let desk = Desk::new("Multi", AuthorityToken(4));
    let hydra: Hydra = Arc::new(|_: &str| Box::pin(async { Ran::Unknown }));
    let job = Job::Multi(multi::Multi {
        times: 1,
        lines: vec![Line::Send("look".to_owned())],
    });
    let (_kept, events) = fallen_behind();
    let run = desk
        .run(&handle, (snapshot, Heard::from(events)), job, hydra)
        .expect("it runs");
    let halted = tokio::time::timeout(LIMIT, run)
        .await
        .expect("the batch stops")
        .expect("the task");
    assert!(
        matches!(halted, Err(Halt::Stopped(BehaviorError::FellBehind))),
        "{halted:?}"
    );
    assert_eq!(transcript.written_count(), written, "nothing sent");
}

//! `plan/29` step 2, and `plan/12` §5.5's invariant: **any one session can
//! fail, wedge or panic without affecting the others.**
//!
//! Each test runs two supervised sessions in one process, does something to
//! session A, and asserts session B carries on: its commands still complete,
//! and its generation, events and state are its own.
//!
//! # What these guard, honestly
//!
//! Today nothing is shared between two sessions -- each owns its channels,
//! parser, state, queue and task -- so these pass because of the structure,
//! not because of a check that could be removed. What they catch is the
//! change that ADDS sharing: a pooled event ring, a shared connection
//! manager, a lock two sessions contend on. Where a test must prove its input
//! reached the code (the lag test), it asserts A's side of the effect too, so
//! it cannot pass by flooding nothing.
//!
//! VERIFIED 2026-09-23 by introducing the sharing on purpose:
//!
//! | Mutation | Goes red |
//! |---|---|
//! | every session's observed events through one shared ring (`observation.rs`) | the lag test |
//! | every session's generation from one shared counter (`lifecycle.rs`) | the reconnect test |
//!
//! Not here: a wedged *behavior* (`BEHAVIOR_WATCHDOG`, `12` §5.5) is not built
//! and belongs with behaviors in M6; the session-level wedge is a game that
//! stops answering, which is tested. A failing character-store write needs a
//! filesystem fault to inject and is left to the session table (step 3).

use std::collections::VecDeque;
use std::time::Duration;

use cena_platform::{AnsweringSource, TranscriptHandle};
use cena_session::{
    ConnectError, Connector, Event, Generation, ObservedEvent, Outcome, SessionHandle, SessionId,
    SessionObserver, State, SupervisedEnd, SupervisedSession,
};
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::error::TryRecvError;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

const REPLY: &[u8] = b"You see.\n<prompt time='1'>&gt;</prompt>\n";
const DEADLINE: Duration = Duration::from_secs(5);

/// Hands out its sources in order, then refuses.
struct Connections<S>(VecDeque<S>);

impl<S: cena_platform::ByteSource + 'static> Connector for Connections<S> {
    type Source = S;

    async fn connect(&mut self, _: Generation) -> Result<S, ConnectError> {
        self.0
            .pop_front()
            .ok_or_else(|| ConnectError::fatal("fixture", "exhausted"))
    }
}

/// One running supervised session and what a test needs to reach it.
struct Running {
    handle: SessionHandle,
    observer: SessionObserver,
    cancel: CancellationToken,
    task: JoinHandle<SupervisedEnd>,
}

fn start<S: cena_platform::ByteSource + 'static>(id: u32, sources: Vec<S>) -> Running {
    let (session, handle) = SupervisedSession::numbered(SessionId(id), Connections(sources.into()));
    let observer = session.observer();
    let cancel = session.cancel_token();
    Running {
        handle,
        observer,
        cancel,
        task: tokio::spawn(session.run()),
    }
}

/// Wait until `observer`'s session is `Ready` on connection `generation`.
async fn ready(observer: &SessionObserver, generation: Generation) -> Result<(), String> {
    let (snapshot, mut events) = observer.subscribe().await.map_err(|e| format!("{e:?}"))?;
    if snapshot.lifecycle == State::Ready && snapshot.generation == generation {
        return Ok(());
    }
    loop {
        let next = events.recv().await.map_err(|e| e.to_string())?;
        if next.event == Event::StateChanged(State::Ready) && next.generation == generation {
            return Ok(());
        }
    }
}

/// Session B's health: a command completes, on the connection it started on.
async fn b_still_answers(b: &Running) {
    let outcome = b
        .handle
        .send_manual_at(Generation::FIRST, "look", DEADLINE)
        .await;
    assert!(
        matches!(outcome, Outcome::Confirmed(_)),
        "session B must be untouched by what happened to A: {outcome:?}"
    );
}

/// Two sessions, both `Ready`. A `Result`: not a `#[test]` function, so the
/// workspace's `expect_used` denial reaches it.
async fn two_ready() -> Result<(Running, TranscriptHandle, Running, TranscriptHandle), String> {
    let (a_source, a_transcript) = AnsweringSource::logged_in(REPLY);
    let (b_source, b_transcript) = AnsweringSource::logged_in(REPLY);
    let a = start(0, vec![a_source]);
    let b = start(1, vec![b_source]);
    ready(&a.observer, Generation::FIRST).await?;
    ready(&b.observer, Generation::FIRST).await?;
    Ok((a, a_transcript, b, b_transcript))
}

#[cfg(test)]
mod fixtures {
    /// A game connection whose reader panics: a bug in session A's actor,
    /// as far as the process can tell.
    pub struct Panicking;

    impl cena_platform::ByteSource for Panicking {
        async fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("session A's actor panics, on purpose");
        }
        async fn write_all(&mut self, _: &[u8]) -> std::io::Result<()> {
            Ok(())
        }
        async fn shutdown(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
}

#[tokio::test(start_paused = true)]
async fn a_panicking_session_leaves_the_other_running() {
    let (b_source, _) = AnsweringSource::logged_in(REPLY);
    let b = start(1, vec![b_source]);
    ready(&b.observer, Generation::FIRST)
        .await
        .expect("B logs in");

    let a = start(0, vec![fixtures::Panicking]);
    let ended = tokio::time::timeout(DEADLINE, a.task)
        .await
        .expect("A's task ends");
    assert!(
        ended.as_ref().is_err_and(tokio::task::JoinError::is_panic),
        "A's panic is contained in A's own task: {:?}",
        ended.map(|end| end.stopped_because)
    );

    // A is gone and says so; B is untouched.
    assert_eq!(
        a.handle
            .send_manual_at(Generation::FIRST, "look", DEADLINE)
            .await,
        Outcome::Dead
    );
    b_still_answers(&b).await;
    b.cancel.cancel();
}

#[tokio::test(start_paused = true)]
async fn a_game_that_stops_answering_does_not_slow_the_other() {
    let (a, a_transcript, b, _) = two_ready().await.expect("both log in");
    // A's command goes out and its answer never comes: the window stays open.
    a_transcript.hold_replies();
    let a_handle = a.handle.clone();
    let stuck = tokio::spawn(async move {
        a_handle
            .send_manual_at(Generation::FIRST, "look", DEADLINE)
            .await
    });
    tokio::task::yield_now().await;

    b_still_answers(&b).await;
    assert!(!stuck.is_finished(), "A is still waiting on its game");
    assert_eq!(stuck.await.expect("A's waiter"), Outcome::Timeout);
    a.cancel.cancel();
    b.cancel.cancel();
}

#[tokio::test(start_paused = true)]
async fn a_lagging_observer_of_one_does_not_lag_the_other() {
    let (a, a_transcript, b, _) = two_ready().await.expect("both log in");
    let (_, mut a_events) = a.observer.subscribe().await.expect("A answers");
    let (_, mut b_events) = b.observer.subscribe().await.expect("B answers");

    // Far more frames than any event ring holds, to A only, read by nobody.
    let flood: Vec<u8> = "a line of text\n"
        .repeat(20_000)
        .into_bytes()
        .into_iter()
        .chain(REPLY.iter().copied())
        .collect();
    a_transcript.answer("flood", &flood);
    let flooded = a
        .handle
        .send_manual_at(Generation::FIRST, "flood", DEADLINE)
        .await;
    assert!(matches!(flooded, Outcome::Confirmed(_)), "{flooded:?}");

    // The input reached the code: A's observer really did fall behind.
    assert!(
        drain(&mut a_events).lagged,
        "A's observer never lagged, so this test flooded nothing"
    );

    b_still_answers(&b).await;
    let b_seen = drain(&mut b_events);
    assert!(!b_seen.lagged, "B's observer lagged because of A's traffic");
    assert!(b_seen.events > 0, "B's observer saw its own command");

    a.cancel.cancel();
    b.cancel.cancel();
}

/// What a receiver held: how many events, and whether it had lagged.
struct Drained {
    events: usize,
    lagged: bool,
}

fn drain(events: &mut Receiver<ObservedEvent>) -> Drained {
    let mut drained = Drained {
        events: 0,
        lagged: false,
    };
    loop {
        match events.try_recv() {
            Ok(_) => drained.events += 1,
            Err(TryRecvError::Lagged(_)) => drained.lagged = true,
            Err(TryRecvError::Empty | TryRecvError::Closed) => return drained,
        }
    }
}

#[tokio::test(start_paused = true)]
async fn a_reconnect_leaves_the_other_connection_alone() {
    let (a_first, a_transcript) = AnsweringSource::logged_in(REPLY);
    let (a_second, _) = AnsweringSource::logged_in(REPLY);
    let (b_source, _) = AnsweringSource::logged_in(REPLY);
    let a = start(0, vec![a_first, a_second]);
    let b = start(1, vec![b_source]);
    ready(&a.observer, Generation::FIRST)
        .await
        .expect("A logs in");
    ready(&b.observer, Generation::FIRST)
        .await
        .expect("B logs in");

    a_transcript.hang_up();
    ready(&a.observer, Generation::FIRST.next())
        .await
        .expect("A reconnects");

    let (b_now, _) = b.observer.subscribe().await.expect("B answers");
    assert_eq!(b_now.generation, Generation::FIRST, "B never reconnected");
    assert_eq!(b_now.lifecycle, State::Ready);
    b_still_answers(&b).await;

    a.cancel.cancel();
    b.cancel.cancel();
}

#[tokio::test(start_paused = true)]
async fn quitting_or_removing_one_leaves_the_other_running() {
    let (a, _, b, _) = two_ready().await.expect("both log in");
    // Quit: the orderly way, as a player typing it or the table removing A.
    let _ = a.handle.quit(DEADLINE).await;
    a.cancel.cancel();
    let _ = tokio::time::timeout(DEADLINE, a.task)
        .await
        .expect("A ends");
    b_still_answers(&b).await;

    // And the abrupt way: a cancelled session, with no quit sent first.
    let (c_source, _) = AnsweringSource::logged_in(REPLY);
    let c = start(2, vec![c_source]);
    ready(&c.observer, Generation::FIRST)
        .await
        .expect("C logs in");
    c.cancel.cancel();
    let _ = tokio::time::timeout(DEADLINE, c.task)
        .await
        .expect("C ends");
    b_still_answers(&b).await;
    b.cancel.cancel();
}

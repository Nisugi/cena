//! Offline tests of the snapshot/event fence after the owner has been consumed.
//! `AnsweringSource` explicitly doubles a transport; the room bytes are the
//! shared, scrubbed protocol fixture. Small literals isolate lifecycle cases.

mod support;

use cena_platform::{AnsweringSource, ReplaySource};
use cena_session::{
    ConnectError, Connector, Event, Generation, ObserveError, Outcome, Session, State,
    SupervisedSession,
};
use std::collections::VecDeque;
use std::time::Duration;
use tokio::sync::broadcast::error::TryRecvError;

const DEADLINE: Duration = Duration::from_secs(2);

/// A subscription taken once the connection is `Ready`.
///
/// `run` holds `Syncing` until the first prompt after `<endSetup/>`, and the
/// observation arm comes before the read arm in the actor's `biased` select,
/// so a subscription made the moment an actor starts can see `Syncing`. These
/// tests are about a session somebody is already playing, so they wait.
///
/// A `Result` because this is not a `#[test]` function, so the workspace's
/// `expect_used` denial reaches it.
async fn ready(
    observer: &cena_session::SessionObserver,
) -> Result<
    (
        cena_session::Snapshot,
        tokio::sync::broadcast::Receiver<cena_session::ObservedEvent>,
    ),
    String,
> {
    let (snapshot, mut events) = observer.subscribe().await.map_err(|e| format!("{e:?}"))?;
    if snapshot.lifecycle == State::Ready {
        return Ok((snapshot, events));
    }
    loop {
        let next = events.recv().await.map_err(|e| e.to_string())?;
        if next.event == Event::StateChanged(State::Ready) {
            return observer.subscribe().await.map_err(|e| format!("{e:?}"));
        }
    }
}

struct Connections(VecDeque<AnsweringSource>);

impl Connector for Connections {
    type Source = AnsweringSource;

    async fn connect(&mut self, _: Generation) -> Result<Self::Source, ConnectError> {
        self.0
            .pop_front()
            .ok_or_else(|| ConnectError::fatal("fixture", "exhausted"))
    }
}

#[tokio::test(start_paused = true)]
async fn mid_session_snapshot_is_ready_and_stream_starts_after_its_state() {
    let (source, _) = AnsweringSource::logged_in(&support::room_fixture().expect("room fixture"));
    let (session, handle) = SupervisedSession::new(Connections(vec![source].into()));
    let observer = session.observer();
    let cancel = session.cancel_token();
    let running = tokio::spawn(session.run());

    let (initial, _) = ready(&observer).await.expect("the session becomes Ready");
    assert_eq!(initial.lifecycle, State::Ready);
    assert!(matches!(
        handle
            .send_manual_at(initial.generation, "look", DEADLINE)
            .await,
        Outcome::Confirmed(_)
    ));
    let (snapshot, mut events) = observer.subscribe().await.expect("fresh state");
    assert_eq!(snapshot.lifecycle, State::Ready);
    assert!(snapshot.state.room.description.is_some());
    assert!(snapshot.cursor > initial.cursor);
    assert!(matches!(events.try_recv(), Err(TryRecvError::Empty)));

    assert!(matches!(
        handle
            .send_manual_at(snapshot.generation, "look", DEADLINE)
            .await,
        Outcome::Confirmed(_)
    ));
    let first = events.try_recv().expect("first event after snapshot");
    assert_eq!(first.cursor, snapshot.cursor + 1);
    assert_eq!(first.session, snapshot.session);
    assert_eq!(first.generation, snapshot.generation);
    assert!(matches!(first.event, Event::Sent { .. }));

    cancel.cancel();
    running.await.expect("owner completes");
    let (closed, mut events) = observer.subscribe().await.expect("terminal snapshot");
    assert_eq!(closed.lifecycle, State::Closed);
    assert_eq!(closed.session, snapshot.session);
    assert!(matches!(events.try_recv(), Err(TryRecvError::Closed)));
}

#[tokio::test(start_paused = true)]
async fn lag_resubscription_replaces_the_old_fence_with_fresh_authoritative_state() {
    let mut reply = "<progressBar id='health' value='68'/>"
        .repeat(3000)
        .into_bytes();
    reply.extend_from_slice(
        b"<progressBar id='health' value='67'/><prompt time='1'>&gt;</prompt>\n",
    );
    let (source, _) = AnsweringSource::new(&reply);
    let session = Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    let cancel = session.cancel_token();
    let running = tokio::spawn(session.into_actor().run());
    let (old, mut events) = observer.subscribe().await.expect("first snapshot");
    assert!(matches!(
        handle
            .send_manual_at(old.generation, "look", DEADLINE)
            .await,
        Outcome::Confirmed(_)
    ));
    assert!(matches!(events.try_recv(), Err(TryRecvError::Lagged(_))));

    let (fresh, mut fresh_events) = observer.subscribe().await.expect("resnapshot");
    assert!(fresh.cursor > old.cursor + 2048);
    assert_eq!(
        fresh.state.vitals.get("health").map(|vital| vital.percent),
        Some(67)
    );
    assert!(matches!(fresh_events.try_recv(), Err(TryRecvError::Empty)));
    cancel.cancel();
    running.await.expect("owner completes");
    let closed = fresh_events.try_recv().expect("shutdown after resnapshot");
    assert_eq!(closed.cursor, fresh.cursor + 1);
    assert_eq!(closed.event, Event::StateChanged(State::Closed));
}

/// **A lost connection is not the end of the session, so it does not say
/// `Closed`.**
///
/// The actor published `Closed` on its way out of every connection, and the
/// supervisor published `Reconnecting` one event later -- so a lost link read
/// `[Ready, Closed, Reconnecting, ...]` to every observer, and `Closed` is
/// documented as "the task has ended". `cena-behavior`'s travel maps it to
/// `Dead` (review finding 2). `Closed` belongs to the session's end, once.
#[tokio::test(start_paused = true)]
async fn a_lost_connection_goes_to_reconnecting_without_closing() {
    let reply = b"<prompt time='1'>&gt;</prompt>\n";
    let (first, transcript) = AnsweringSource::new(reply);
    let (second, _) = AnsweringSource::new(reply);
    let (session, _handle) = SupervisedSession::new(Connections(vec![first, second].into()));
    let (_, mut events) = session.subscribe();
    let observer = session.observer();
    let cancel = session.cancel_token();
    let running = tokio::spawn(session.run());
    let _ = observer.subscribe().await.expect("first connection ready");

    transcript.hang_up();
    let mut lifecycle = Vec::new();
    while lifecycle.last() != Some(&State::Reconnecting) {
        if let Event::StateChanged(state) = events.recv().await.expect("an event") {
            lifecycle.push(state);
        }
    }
    assert!(
        !lifecycle.contains(&State::Closed),
        "a dropped connection announced the session closed: {lifecycle:?}"
    );

    cancel.cancel();
    let _ = running.await;
    while let Ok(event) = events.try_recv() {
        if let Event::StateChanged(state) = event {
            lifecycle.push(state);
        }
    }
    assert_eq!(
        lifecycle.iter().filter(|s| **s == State::Closed).count(),
        1,
        "the session's real end says `Closed` exactly once: {lifecycle:?}"
    );
    assert_eq!(lifecycle.last(), Some(&State::Closed), "{lifecycle:?}");
}

/// **A notice reaches a frontend attached through the observer.**
///
/// `SessionHandle::say` published to the legacy `Event` channel only, so the
/// fenced stream `SessionObserver::subscribe` returns -- the one `cena-web`
/// reads -- never carried one (review finding 7). "Travel: no route" reached
/// nobody watching the web frontend.
#[tokio::test(start_paused = true)]
async fn a_notice_reaches_the_fenced_observer_stream() {
    let (source, _) = AnsweringSource::new(b"<prompt time='1'>&gt;</prompt>\n");
    let session = Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    let cancel = session.cancel_token();
    let driver = tokio::spawn(session.into_actor().run());
    let (snapshot, mut fenced) = observer.subscribe().await.expect("running owner");

    handle.say(cena_session::Notice::line(
        cena_session::NoticeKind::Error,
        "Travel: no route",
    ));
    let event = fenced
        .try_recv()
        .expect("the notice must be on the fenced stream");
    assert!(matches!(event.event, Event::Notice(_)), "{:?}", event.event);
    assert!(
        event.cursor > snapshot.cursor,
        "numbered after the snapshot it followed, or a frontend discards it \
         as already seen: {} vs {}",
        event.cursor,
        snapshot.cursor
    );

    cancel.cancel();
    let _ = driver.await;
}

#[tokio::test(start_paused = true)]
async fn reconnect_snapshot_and_transition_share_the_new_generation() {
    let reply = b"<progressBar id='health' value='97'/><prompt time='1'>&gt;</prompt>\n";
    let (first, transcript) = AnsweringSource::logged_in(reply);
    let (second, second_transcript) = AnsweringSource::logged_in(reply);
    let (session, handle) = SupervisedSession::new(Connections(vec![first, second].into()));
    let observer = session.observer();
    let cancel = session.cancel_token();
    let running = tokio::spawn(session.run());
    let (initial, mut events) = ready(&observer).await.expect("the session becomes Ready");
    assert!(matches!(
        handle
            .send_manual_at(initial.generation, "look", DEADLINE)
            .await,
        Outcome::Confirmed(_)
    ));
    transcript.hang_up();
    let transition = loop {
        let event = events.recv().await.expect("reconnect event");
        if event.event == Event::StateChanged(State::Reconnecting) {
            break event;
        }
    };
    assert_eq!(transition.generation, initial.generation.next());
    let (retry, _) = observer.subscribe().await.expect("observe during backoff");
    assert_eq!(retry.lifecycle, State::Reconnecting);
    assert_eq!(retry.generation, transition.generation);
    let ladder = retry
        .retry
        .as_ref()
        .expect("late attachment knows the retry decision");
    assert_eq!(ladder.attempt, 1);
    assert!(ladder.delay > Duration::ZERO);
    assert!(!ladder.detail.is_empty());
    assert_eq!(
        retry.state.in_roundtime(),
        None,
        "reconnect invalidated the server clock"
    );
    assert!(retry.cursor >= transition.cursor);

    tokio::time::advance(Duration::from_secs(2)).await;
    let (again, _) = ready(&observer).await.expect("the session becomes Ready");
    assert_eq!(again.lifecycle, State::Ready);
    assert_eq!(again.generation, retry.generation);
    assert!(
        again.retry.is_none(),
        "a ready connection has no pending retry"
    );
    // The special quit path must reject the browser's previous generation.
    assert_eq!(
        handle
            .send_manual_at(initial.generation, "quit", DEADLINE)
            .await,
        Outcome::Disconnected
    );
    assert_eq!(second_transcript.written_count(), 0);
    assert!(matches!(
        handle
            .send_manual_at(again.generation, "look", DEADLINE)
            .await,
        Outcome::Confirmed(_)
    ));
    cancel.cancel();
    running.await.expect("owner completes");
}

struct PendingConnect;

impl Connector for PendingConnect {
    type Source = ReplaySource;

    async fn connect(&mut self, _: Generation) -> Result<Self::Source, ConnectError> {
        std::future::pending().await
    }
}

struct TransientThenPending(bool);

impl Connector for TransientThenPending {
    type Source = ReplaySource;

    async fn connect(&mut self, _: Generation) -> Result<Self::Source, ConnectError> {
        if std::mem::replace(&mut self.0, false) {
            Err(ConnectError::transient("fixture", "offline"))
        } else {
            std::future::pending().await
        }
    }
}

#[tokio::test(start_paused = true)]
async fn failed_initial_login_is_reconnecting_without_inventing_a_new_generation() {
    let (session, _) = SupervisedSession::new(TransientThenPending(true));
    let observer = session.observer();
    let cancel = session.cancel_token();
    let running = tokio::spawn(session.run());
    let (backoff, _) = observer.subscribe().await.expect("retry snapshot");
    assert_eq!(backoff.lifecycle, State::Reconnecting);
    assert_eq!(backoff.generation, Generation::FIRST);
    assert_eq!(backoff.retry.as_ref().expect("retry metadata").attempt, 1);
    tokio::time::advance(Duration::from_secs(2)).await;
    let (connecting, _) = observer
        .subscribe()
        .await
        .expect("pending reconnect is observable");
    assert_eq!(connecting.lifecycle, State::Reconnecting);
    assert_eq!(connecting.retry, backoff.retry);
    cancel.cancel();
    running.await.expect("owner completes");
}

#[tokio::test(start_paused = true)]
async fn connecting_remains_observable_and_cancel_records_actual_closed_state() {
    let (session, _) = SupervisedSession::new(PendingConnect);
    let observer = session.observer();
    let cancel = session.cancel_token();
    let running = tokio::spawn(session.run());
    let (snapshot, mut events) = observer
        .subscribe()
        .await
        .expect("connecting owner answers");
    assert_eq!(snapshot.lifecycle, State::Connecting);
    cancel.cancel();
    running.await.expect("cancelled connect");
    let terminal = events.try_recv().expect("terminal event");
    assert_eq!(terminal.event, Event::StateChanged(State::Closed));
    let (closed, _) = observer.subscribe().await.expect("closed snapshot");
    assert_eq!(closed.lifecycle, State::Closed);
    assert_eq!(closed.cursor, terminal.cursor);
}

#[tokio::test(start_paused = true)]
async fn fatal_initial_connect_has_a_closed_snapshot_even_without_an_actor() {
    let (session, _) = SupervisedSession::new(Connections(VecDeque::new()));
    let observer = session.observer();
    Box::pin(session.run()).await;
    let (closed, _) = observer.subscribe().await.expect("fatal shutdown recorded");
    assert_eq!(closed.lifecycle, State::Closed);
}

#[tokio::test(start_paused = true)]
async fn unresponsive_and_dropped_owners_never_fabricate_a_terminal_snapshot() {
    let (source, _) = AnsweringSource::new(b"");
    let session = Session::new(source);
    let observer = session.observer();
    let started = tokio::time::Instant::now();
    assert!(matches!(
        observer.subscribe().await,
        Err(ObserveError::Timeout)
    ));
    assert_eq!(started.elapsed(), Duration::from_secs(5));
    drop(session);
    assert!(matches!(
        observer.subscribe().await,
        Err(ObserveError::Closed)
    ));
}

#[test]
fn actor_future_storage_justifies_heap_pinning() {
    let (source, _) = AnsweringSource::new(b"");
    let actor = Session::new(source).into_actor();
    let future = actor.run();
    let inline_bytes = std::mem::size_of_val(&future);
    let pinned = Box::pin(future);
    let pinned_bytes = std::mem::size_of_val(&pinned);
    eprintln!("actor future: inline={inline_bytes} bytes; boxed={pinned_bytes} bytes");
    // Measure this target/layout, not a universal ABI size. The supervisor
    // retains this handle across await instead of embedding the whole future.
    assert!(inline_bytes > pinned_bytes);
}

#[tokio::test(start_paused = true)]
async fn an_undrained_observation_inbox_refuses_excess_requests() {
    let (source, _) = AnsweringSource::new(b"");
    let session = Session::new(source);
    let mut requests = Vec::new();
    for _ in 0..33 {
        let observer = session.observer();
        requests.push(tokio::spawn(async move { observer.subscribe().await }));
    }
    let mut busy = 0;
    let mut timed_out = 0;
    for request in requests {
        match request.await.expect("request task") {
            Err(ObserveError::Busy) => busy += 1,
            Err(ObserveError::Timeout) => timed_out += 1,
            other => panic!("undriven owner cannot answer: {other:?}"),
        }
    }
    assert_eq!(busy, 1);
    assert_eq!(timed_out, 32);
}

#[tokio::test(start_paused = true)]
async fn expired_manual_input_including_quit_cannot_fire_when_the_actor_resumes() {
    let (source, transcript) =
        AnsweringSource::logged_in(b"You look around.\n<prompt time='1'>&gt;</prompt>\n");
    let session = Session::new(source);
    let handle = session.handle();
    let observer = session.observer();
    let cancel = session.cancel_token();
    for line in ["look", "quit"] {
        assert_eq!(
            handle
                .send_manual_at(Generation::FIRST, line, Duration::from_millis(1))
                .await,
            Outcome::Timeout
        );
    }
    let running = tokio::spawn(session.into_actor().run());
    let (snapshot, _) = ready(&observer).await.expect("the session becomes Ready");
    assert_eq!(snapshot.lifecycle, State::Ready);
    assert_eq!(
        transcript.written_count(),
        0,
        "expired intentions never reached the transport"
    );
    assert!(matches!(
        handle
            .send_manual_at(snapshot.generation, "look", DEADLINE)
            .await,
        Outcome::Confirmed(_)
    ));
    assert_eq!(transcript.lines(), vec!["look"]);
    cancel.cancel();
    running.await.expect("owner completes");
}

#[tokio::test(start_paused = true)]
async fn a_subscription_racing_ready_ingress_splits_the_exact_observed_event_sequence() {
    let (source, transcript) = AnsweringSource::new(
        b"<progressBar id='health' value='97'/><prompt time='1'>&gt;</prompt>\n",
    );
    transcript.hold_replies();
    let session = Session::new(source);
    let observer = session.observer();
    let handle = session.handle();
    let cancel = session.cancel_token();
    let running = tokio::spawn(session.into_actor().run());
    let (initial, mut witness) = observer.subscribe().await.expect("initial fence");
    let command = tokio::spawn(async move {
        handle
            .send_manual_at(Generation::FIRST, "look", DEADLINE)
            .await
    });
    let sent = tokio::time::timeout(DEADLINE, witness.recv())
        .await
        .expect("command was sent promptly")
        .expect("sent event");
    assert!(matches!(sent.event, Event::Sent { .. }));
    assert!(
        !command.is_finished(),
        "its reply is still held by the source"
    );

    // Both the read and the observation request become ready before the
    // current-thread runtime polls the owner again. Do not await the command
    // first: that would only test a subscription to an already quiet actor.
    let ((), subscription) = tokio::join!(
        biased;
        async {
            transcript.release_one();
            tokio::task::yield_now().await;
        },
        observer.subscribe(),
    );
    let (snapshot, mut events) = subscription.expect("concurrent attachment");
    assert!(matches!(
        tokio::time::timeout(DEADLINE, command)
            .await
            .expect("bounded command wait")
            .expect("command task"),
        Outcome::Confirmed(_)
    ));
    let (complete, _) = observer.subscribe().await.expect("ingress completed");
    assert_eq!(
        complete.state.vitals.get("health").map(|v| v.percent),
        Some(97)
    );
    assert!(
        snapshot.cursor >= sent.cursor && snapshot.cursor < complete.cursor,
        "the concurrent attachment must leave ingress after its fence"
    );

    let count = complete.cursor - initial.cursor;
    assert!(count < 20, "the fixture is a small, bounded event sequence");
    let mut journal = vec![sent];
    for _ in 1..count {
        journal.push(
            tokio::time::timeout(DEADLINE, witness.recv())
                .await
                .expect("bounded witness wait")
                .expect("unbroken witness stream"),
        );
    }
    let mut expected_state = initial.state;
    for (expected_cursor, event) in (initial.cursor + 1..).zip(&journal) {
        assert_eq!(event.cursor, expected_cursor);
        assert_eq!(
            (event.session, event.generation),
            (snapshot.session, snapshot.generation)
        );
        if event.cursor <= snapshot.cursor {
            if let Event::Frame(frame) = &event.event {
                expected_state.apply(frame);
            }
        } else {
            assert_eq!(events.try_recv().expect("exact suffix event"), *event);
        }
    }
    assert_eq!(
        snapshot.state, expected_state,
        "state is exactly the witnessed prefix"
    );
    assert!(
        matches!(events.try_recv(), Err(TryRecvError::Empty)),
        "no duplicate suffix"
    );
    cancel.cancel();
    tokio::time::timeout(DEADLINE, running)
        .await
        .expect("bounded shutdown")
        .expect("owner completes");
}

#[tokio::test(start_paused = true)]
async fn aborting_an_active_actor_does_not_fabricate_a_closed_snapshot_or_event() {
    let (source, _) = AnsweringSource::logged_in(b"");
    let session = Session::new(source);
    let observer = session.observer();
    let running = tokio::spawn(session.into_actor().run());
    let (active, mut events) = ready(&observer).await.expect("the session becomes Ready");
    assert_eq!(active.lifecycle, State::Ready);

    running.abort();
    let ended = tokio::time::timeout(DEADLINE, running)
        .await
        .expect("bounded abort completion")
        .expect_err("abort ends the active task");
    assert!(ended.is_cancelled());
    assert!(
        matches!(events.try_recv(), Err(TryRecvError::Closed)),
        "an aborted owner did not perform a Closed transition"
    );
    assert!(
        matches!(
            tokio::time::timeout(DEADLINE, observer.subscribe())
                .await
                .expect("bounded observation"),
            Err(ObserveError::Closed)
        ),
        "an aborted owner has no confirmed terminal snapshot to return"
    );
}

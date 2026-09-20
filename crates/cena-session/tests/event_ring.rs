//! **M2 step 6: the event ring, and the question `plan/18` §6 asked.**
//!
//! The author's first live session printed `!! 99 events dropped from the ring`
//! during the login burst. §6 declined to just raise the constant:
//!
//! > *"Raising it is one constant; the real question is whether a subscriber that
//! > needs EVERY frame should be a broadcast subscriber at all, or whether the
//! > model is the only thing that must not miss frames."*
//!
//! # The answer, and it was already in the code
//!
//! **The model is already fed synchronously and never goes through the ring.**
//! `SessionActor::ingest` (`actor/io.rs`) calls `self.state.apply(&frame)` and
//! *then* `self.events.send(...)`, in that order, on the same thread. So
//! `GameState` cannot lag no matter how small the ring is, and every test in
//! `cena-model` folding frames directly is testing the real path rather than a
//! convenient one.
//!
//! That settles the design question: the ring is for **observers** -- a renderer,
//! a log tail, a behavior watching for an event -- and for an observer `Lagged`
//! is honest. `plan/12` §6.3 requires exactly that: *"On overflow the subscriber
//! receives an explicit `Lagged { missed }`, never a silent gap."*
//!
//! So what is left is sizing, and that is a measurement rather than a taste.
//!
//! # The measurement
//!
//! Frames before the first `<prompt>` -- the login burst -- replayed from two of
//! the author's own captures:
//!
//! | capture | burst frames | total |
//! |---|---|---|
//! | `GSIV-Nisugi/2025-04-18` | **1,151** | 4,968 |
//! | `GSIV-Monstr/2025-09-04` | **794** | 6,313 |
//!
//! Against a 256-slot ring. So the burst is **3-4.5x** the ring, and the 99
//! dropped events the author saw were the tail of a much larger overflow that a
//! faster subscriber had already absorbed.
//!
//! 2,048 covers the larger burst with ~78% headroom. It is not a guess at the
//! worst case: a slow subscriber can still lag, and `Lagged` still says so.

use cena_platform::ReplaySource;
use cena_session::{Event, Session};
use std::time::Duration;
use tokio::sync::broadcast::error::RecvError;

/// A login-burst-shaped chunk: `n` frames before any prompt.
fn burst(lines: usize) -> Vec<u8> {
    let mut wire = Vec::new();
    for i in 0..lines {
        wire.extend_from_slice(format!("line {i} of the login burst\n").as_bytes());
    }
    wire.extend_from_slice(b"<prompt time=\"1789775900\">&gt;</prompt>\n");
    wire
}

#[tokio::test(flavor = "current_thread")]
async fn the_model_never_lags_however_far_behind_a_subscriber_falls() {
    // THE DESIGN ANSWER, asserted rather than asserted-in-prose. A subscriber
    // that never reads at all must not cost the model a single frame, because the
    // model is fed synchronously before the broadcast.
    let source = ReplaySource::new(vec![burst(5_000)]);
    let session = Session::new(source);
    // Subscribe and then deliberately never receive.
    let (_snapshot, _asleep) = session.subscribe();

    let end = tokio::time::timeout(Duration::from_secs(10), session.into_actor().run())
        .await
        .expect("the session must finish");

    // **`lines_seen`, not `stream("").len()`.** This measured the retained
    // buffer, which conflates two claims: "the model was fed every line" and
    // "the model keeps every line forever". Only the first is this test's
    // subject, and the second stopped being true when `streams.rs` gained a
    // scrollback cap (review finding 6). Counting what was fed asserts
    // exactly what the comment above promises.
    assert_eq!(
        end.state.lines_seen(),
        5_000,
        "the model lost lines while a subscriber was asleep; it is supposed to be \
         fed synchronously, before the broadcast"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn a_measured_login_burst_does_not_lag_an_attentive_subscriber() {
    // 1,151 frames is the larger of the two measured bursts. A subscriber that
    // keeps up must see the whole thing.
    let source = ReplaySource::new(vec![burst(1_151)]);
    let session = Session::new(source);
    let (_snapshot, mut events) = session.subscribe();

    let task = tokio::spawn(session.into_actor().run());

    let mut seen = 0usize;
    let mut lagged = 0u64;
    loop {
        match tokio::time::timeout(Duration::from_secs(5), events.recv()).await {
            Ok(Ok(Event::Frame(_))) => seen += 1,
            Ok(Ok(_)) => {}
            Ok(Err(RecvError::Lagged(missed))) => lagged += missed,
            Ok(Err(RecvError::Closed)) | Err(_) => break,
        }
    }
    let _ = task.await;

    assert_eq!(
        lagged, 0,
        "an attentive subscriber lagged {lagged} events on a MEASURED login \
         burst; the ring is smaller than the traffic it exists to carry"
    );
    assert!(
        seen >= 1_151,
        "the subscriber saw only {seen} of 1,151 burst frames"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn overflow_is_reported_rather_than_silent() {
    // `plan/12` §6.3: "On overflow the subscriber receives an explicit
    // `Lagged { missed }`, never a silent gap." The bound is a size, not a
    // promise -- a subscriber slow enough will still lag, and must be TOLD.
    //
    // 20,000 frames is far beyond any measured burst, which is the point: this
    // asserts the failure mode is loud, not that it cannot happen.
    let source = ReplaySource::new(vec![burst(20_000)]);
    let session = Session::new(source);
    let (_snapshot, mut events) = session.subscribe();

    let task = tokio::spawn(session.into_actor().run());
    // Let the actor run to completion without reading a single event.
    let end = tokio::time::timeout(Duration::from_secs(20), task)
        .await
        .expect("the session must finish")
        .expect("the actor must not panic");

    let first = events.recv().await;
    assert!(
        matches!(first, Err(RecvError::Lagged(_))),
        "a subscriber that missed thousands of events was not told: {first:?}"
    );
    // Same correction: every line REACHED the model, whatever the scrollback
    // chose to retain.
    assert_eq!(
        end.state.lines_seen(),
        20_000,
        "and the model was still fed every line"
    );
    assert_eq!(
        end.state.stream("").len(),
        cena_model::MAX_STREAM_LINES,
        "...while the scrollback holds its cap, which is a different claim"
    );
}

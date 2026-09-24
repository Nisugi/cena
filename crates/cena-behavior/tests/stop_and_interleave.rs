//! Criteria 4 and 5: `stop` stops the behavior within `PREEMPT_GRACE`, and a
//! manual command mid-behavior does **not**.
//!
//! - **4** (`plan/12:540`): "The behavior runs, and `stop` stops it within
//!   `PREEMPT_GRACE`, verified."
//! - **5** (`plan/12:541`): "Manual input is **interleaved, not preemptive**:
//!   a command typed mid-behavior jumps the queue, runs its round-trip, and
//!   the behavior **continues**. Verified by a test that types a command
//!   mid-sequence and asserts the behavior is still running afterward."
//!
//! # How 250ms is VERIFIED rather than asserted
//!
//! The dishonest version of a latency test measures the *test machine's*
//! scheduler and passes or fails with the load on it. These run under
//! `#[tokio::test(start_paused = true)]`, so `tokio::time::Instant` and every
//! `sleep`/`timeout` in the tree are virtual: the elapsed time measured is
//! **how many awaits the cancel had to traverse**, which is the property the
//! code controls, not how busy the machine was.
//!
//! Virtual time also removes the flake in the other direction. Wall-clock
//! `elapsed <= 250ms` on a loaded CI box fails for reasons that have nothing
//! to do with the code.
//!
//! # The vacuity these tests are written against
//!
//! A cancellation test is the easiest one in the world to write vacuously: if
//! the token is already cancelled when the behavior starts, `look` returns on
//! its first line and the test passes no matter what the loop does. So
//! `stop_stops_the_behavior_within_preempt_grace` cancels **only after the
//! behavior has completed at least one round trip and entered its sleep**,
//! which is the state the 250ms is actually about, and asserts that it did so.

mod looping;
mod ready;

use cena_behavior::BehaviorError;
use cena_platform::AnsweringSource;
use cena_session::{CommandId, Origin, Outcome, Session};
use looping::{LOOK_INTERVAL, look};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// `plan/12` §4.3: "Waits up to **`PREEMPT_GRACE` (default 250ms)** for it to
/// yield."
const PREEMPT_GRACE: Duration = Duration::from_millis(250);

/// The terminator that closes a round-trip window (`plan/12` §4.4).
const PROMPT: &[u8] = b"You see nothing unusual.\n<prompt time=\"1\">&gt;</prompt>\n";

/// A monotonic `CommandId` source. Seeded at 0, never from a clock or a
/// random, so a replay produces the same ids every run.
fn ids() -> impl FnMut() -> CommandId {
    let next = Arc::new(AtomicU64::new(0));
    move || CommandId(next.fetch_add(1, Ordering::Relaxed))
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn stop_stops_the_behavior_within_preempt_grace() {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let (_, ready) = session.subscribe();
    let handle = session.handle();
    let session_cancel = session.cancel_token();
    let actor = tokio::spawn(session.into_actor().run());
    ready::until_ready(ready)
        .await
        .expect("the session becomes Ready");

    let stop = CancellationToken::new();
    let behavior_stop = stop.clone();
    let behavior = tokio::spawn(async move {
        look(
            &handle,
            &behavior_stop,
            ids(),
            cena_session::AuthorityToken(1),
        )
        .await
    });

    // Let the behavior get PAST its first round trip and INTO the sleep. This
    // is what stops the test being vacuous: cancelling before the first await
    // would pass on any implementation, including one that ignores the token
    // entirely after the first check.
    tokio::time::advance(LOOK_INTERVAL / 2).await;
    let sent = transcript.written_count();
    assert!(
        sent >= 1,
        "the behavior must have completed at least one round trip and be \
         parked in its sleep before `stop` is measured, or this test passes on \
         a behavior that never checks the token again. Writes seen: {sent}"
    );

    let at_stop = Instant::now();
    stop.cancel();
    let result = behavior.await.expect("the behavior task must not panic");
    let elapsed = at_stop.elapsed();

    assert_eq!(
        result,
        Err(BehaviorError::Cancelled),
        "a stopped behavior must report why it stopped"
    );
    assert!(
        elapsed <= PREEMPT_GRACE,
        "stop took {elapsed:?}, which exceeds PREEMPT_GRACE {PREEMPT_GRACE:?} \
         (plan/12 §4.3). The behavior was parked in its LOOK_INTERVAL sleep; \
         if that sleep is not raced against the cancellation token, the \
         worst-case stop latency is LOOK_INTERVAL ({LOOK_INTERVAL:?}), not \
         PREEMPT_GRACE."
    );

    session_cancel.cancel();
    actor.await.expect("the actor task must not panic");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_manual_command_midflight_does_not_stop_the_behavior() {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let (_, ready) = session.subscribe();
    let behavior_handle = session.handle();
    // THE SAME handle type, cloned from the same session -- criterion 3's
    // "the same queue as the behavior's" is structural here, not asserted.
    let manual_handle = session.handle();
    let session_cancel = session.cancel_token();
    let actor = tokio::spawn(session.into_actor().run());
    ready::until_ready(ready)
        .await
        .expect("the session becomes Ready");

    let stop = CancellationToken::new();
    let behavior_stop = stop.clone();
    let behavior = tokio::spawn(async move {
        look(
            &behavior_handle,
            &behavior_stop,
            ids(),
            cena_session::AuthorityToken(1),
        )
        .await
    });

    // Let the behavior establish itself and complete one round trip.
    tokio::time::advance(LOOK_INTERVAL * 2).await;
    tokio::task::yield_now().await;

    // NOW HOLD THE NEXT WINDOW OPEN. This is what makes "mid-behavior" mean
    // what criterion 5 says. Without it the manual command arrives while the
    // behavior is merely parked between iterations, and the test passes on an
    // implementation that preempts a genuinely in-flight command -- VERIFIED,
    // by writing that break and watching this test stay green.
    transcript.hold_replies();
    let held_at = transcript.written_count();
    for _ in 0..4 {
        tokio::time::advance(LOOK_INTERVAL).await;
        tokio::task::yield_now().await;
    }
    assert!(
        transcript.written_count() > held_at,
        "the behavior must have a command ON THE WIRE with its terminator \
         withheld before the manual command is typed, or 'mid-behavior' is \
         only 'between iterations'. Writes: {held_at} -> {}",
        transcript.written_count()
    );

    // Type a command mid-behavior, into an open window. This is `say hi`
    // mid-hunt, at the moment it is most dangerous.
    let manual = tokio::spawn(async move {
        manual_handle
            .send_and_await(
                CommandId(9000),
                "say hi",
                Origin::Manual,
                Duration::from_secs(30),
                cena_session::queue::any_frame,
            )
            .await
    });
    tokio::task::yield_now().await;

    // THE BEHAVIOR MUST STILL BE RUNNING, with its round trip still open.
    assert!(
        !behavior.is_finished(),
        "the behavior must CONTINUE when a manual command arrives during its \
         open round trip (plan/12 §4.1, CORRECTED 2026-09-18). The superseded \
         draft made manual input preemptive, which meant typing `say hi` \
         mid-hunt aborted Hunt. Only an explicit stop or pause preempts."
    );

    // Let the game answer again. Both round trips now close.
    transcript.release_replies();
    for _ in 0..6 {
        tokio::time::advance(LOOK_INTERVAL).await;
        tokio::task::yield_now().await;
    }

    let outcome = manual.await.expect("the manual waiter must not panic");
    assert!(
        matches!(outcome, Outcome::Confirmed(_)),
        "the manual command must run its own round-trip and return a typed \
         Outcome (criterion 5: it 'runs its round-trip'). Got: {outcome:?}"
    );

    // And STILL running after its own command completed.
    assert!(
        !behavior.is_finished(),
        "the behavior must still be running after the interleaved manual \
         command completed its round trip"
    );

    // And it is still WORKING, not merely un-finished: it sends again.
    let before = transcript.written_count();
    for _ in 0..30 {
        tokio::time::advance(LOOK_INTERVAL).await;
        tokio::task::yield_now().await;
    }
    let after = transcript.written_count();
    assert!(
        after > before,
        "the behavior must still be issuing commands after the interleaved \
         manual one, not merely still alive as a parked task. Writes went \
         {before} -> {after}."
    );

    // The manual command really did go through the SAME queue: both lines are
    // in one transcript, in the order the wire saw them.
    {
        let lines = transcript.lines();
        assert!(
            lines.contains(&"say hi".to_owned()) && lines.contains(&"look".to_owned()),
            "the manual command and the behavior's commands must reach the \
             wire through one queue (criterion 3). Saw: {lines:?}"
        );
    }

    stop.cancel();
    let _ = behavior.await;
    session_cancel.cancel();
    actor.await.expect("the actor task must not panic");
}

/// Criterion 4, in the state a behavior actually spends its life in: a command
/// on the wire with its answer withheld.
///
/// Found by adversarial review with the whole suite green.
/// `stop_stops_the_behavior_within_preempt_grace` parks the behavior in its
/// **sleep**, which was the one await that was already cancel-aware. The round
/// trip was not: `send_and_await` bounds itself with `ROUND_TRIP_DEADLINE`
/// (10s) and never sees the token, so `stop` could not return until the game
/// answered or that deadline fired -- MEASURED at 7s against a 250ms budget.
///
/// A hunting behavior waits on the game far more than it sleeps, so this is
/// the state criterion 4 is actually about.
///
/// Goes RED on reverting `look`'s round-trip `select!` to a bare
/// `.await` (verified): elapsed becomes `ROUND_TRIP_DEADLINE`, not <= 250ms.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn stop_stops_the_behavior_while_a_round_trip_is_in_flight() {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let (_, ready) = session.subscribe();
    let handle = session.handle();
    let session_cancel = session.cancel_token();
    let actor = tokio::spawn(session.into_actor().run());
    ready::until_ready(ready)
        .await
        .expect("the session becomes Ready");

    let stop = CancellationToken::new();
    let behavior_stop = stop.clone();
    let behavior = tokio::spawn(async move {
        look(
            &handle,
            &behavior_stop,
            ids(),
            cena_session::AuthorityToken(1),
        )
        .await
    });

    // Withhold the terminator, so the NEXT command stays in flight rather than
    // resolving. This is the difference between this test and its sibling.
    transcript.hold_replies();
    let held_at = transcript.written_count();
    for _ in 0..4 {
        tokio::time::advance(LOOK_INTERVAL).await;
        tokio::task::yield_now().await;
    }
    assert!(
        transcript.written_count() > held_at,
        "the behavior must have a command ON THE WIRE with its answer withheld \
         before `stop` is measured, or this test is the sleep test again. \
         Writes: {held_at} -> {}",
        transcript.written_count()
    );

    let at_stop = Instant::now();
    stop.cancel();
    let result = behavior.await.expect("the behavior task must not panic");
    let elapsed = at_stop.elapsed();

    assert!(
        elapsed <= PREEMPT_GRACE,
        "`stop` must take effect within PREEMPT_GRACE ({PREEMPT_GRACE:?}) even \
         with a command in flight (plan/12 §4.3). Took {elapsed:?}. A 10s \
         round-trip deadline is not cancellation: awaiting it bare makes the \
         worst case the deadline, not the grace."
    );
    assert!(
        matches!(result, Err(BehaviorError::Cancelled)),
        "a stopped behavior reports Cancelled, not success: {result:?}"
    );

    session_cancel.cancel();
    let _ = actor.await;
}

//! **A command whose caller gave up must not reach the wire.**
//!
//! # Why this is the worst bug class this client can have
//!
//! The queue holds an [`Envelope`](cena_session::Envelope) and the caller holds
//! the other end of its `oneshot`. When `send_and_await` times out it drops
//! that receiver and returns [`Outcome::Timeout`] -- but the envelope stays
//! queued, and the actor sends it whenever the window ahead of it closes.
//!
//! In a game that is **an attack firing after you gave up on it**: the player
//! or behavior moved on, the roundtime is spent on something nobody wanted, and
//! the character acts on an intention that was withdrawn.
//!
//! # What this does NOT change: §4.4 still holds
//!
//! `plan/12` §4.4 is explicit that a timeout is **not** "the command did not
//! happen". That stays true, and the distinction is what makes this fixable
//! rather than a choice between two lies:
//!
//! | State when the caller gives up | What is true | What we do |
//! |---|---|---|
//! | **queued, never written** | it has not happened | **drop it** -- and `Timeout` was accurate |
//! | **written, unanswered** | it may well have happened | nothing to drop; §4.4's warning is about this case |
//!
//! Dropping an unsent command does not make the timeout a lie. It makes it
//! true.

use cena_platform::AnsweringSource;
use cena_session::{CommandId, Gate, Origin, Outcome, Session};
use std::time::Duration;

const PROMPT: &[u8] = b"<prompt time=\"1789775900\">&gt;</prompt>\n";

/// The reproduction, from the review that found it.
///
/// Hold a `look` response so its window stays open, queue an `attack` behind
/// it, let the attack time out, then release. The attack must not be on the
/// wire.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_timed_out_command_never_reaches_the_wire() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    // `AnsweringSource` never hangs up, so the actor must be CANCELLED or the
    // test's final await never returns.
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.into_actor().run());

    // `look` goes out and its window stays OPEN -- nothing answers it.
    transcript.hold_replies();
    let first_handle = handle.clone();
    let first = tokio::spawn(async move {
        first_handle
            .send_and_await(
                CommandId(1),
                "look",
                Origin::Manual,
                Duration::from_mins(5),
                cena_session::queue::any_frame,
            )
            .await
    });
    while transcript.written_count() == 0 {
        tokio::task::yield_now().await;
    }

    // `attack` queues behind it and the caller gives up.
    let outcome = handle
        .send_and_await(
            CommandId(2),
            "attack",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert_eq!(
        outcome,
        Outcome::Timeout,
        "precondition: the second command must actually time out, or this test \
         is not exercising the path it was written for"
    );

    // Releasing closes the first window, so the queue drains. THIS is where the
    // abandoned command used to go out.
    transcript.release_replies();
    tokio::time::sleep(Duration::from_secs(2)).await;

    let lines = transcript.lines();
    assert!(
        !lines.contains(&"attack".to_owned()),
        "a command whose caller gave up must NOT reach the wire -- in a game \
         this is an attack firing after it was abandoned. Wire saw: {lines:?}"
    );

    cancel.cancel();
    let _ = first.await;
    let _ = task.await;
}

/// **A live command is still sent.** The falsifying pair.
///
/// Without this, dropping *every* queued command would pass the test above.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_command_whose_caller_is_still_waiting_is_sent() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    // `AnsweringSource` never hangs up, so the actor must be CANCELLED or the
    // test's final await never returns.
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.into_actor().run());

    transcript.hold_replies();
    let first_handle = handle.clone();
    let first = tokio::spawn(async move {
        first_handle
            .send_and_await(
                CommandId(1),
                "look",
                Origin::Manual,
                Duration::from_mins(5),
                cena_session::queue::any_frame,
            )
            .await
    });
    while transcript.written_count() == 0 {
        tokio::task::yield_now().await;
    }

    // Queued behind, with a caller that does NOT give up.
    let second_handle = handle.clone();
    let second = tokio::spawn(async move {
        second_handle
            .send_and_await(
                CommandId(2),
                "attack",
                Origin::Manual,
                Duration::from_mins(5),
                cena_session::queue::any_frame,
            )
            .await
    });
    tokio::time::sleep(Duration::from_secs(1)).await;

    transcript.release_replies();
    tokio::time::sleep(Duration::from_secs(2)).await;

    assert!(
        transcript.lines().contains(&"attack".to_owned()),
        "a command whose caller is STILL WAITING must be sent. Dropping every \
         queued command would pass the abandoned-command test and break the \
         queue entirely; this is what stops that."
    );

    let _ = second.await;
    cancel.cancel();
    let _ = first.await;
    let _ = task.await;
}

/// `send_now` is unaffected: it opens no window and is written in the turn it
/// arrives, so there is no queue for it to be abandoned in.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn send_now_is_not_affected() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    // `AnsweringSource` never hangs up, so the actor must be CANCELLED or the
    // test's final await never returns.
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.into_actor().run());

    let _ = handle.send_now("look", Origin::Manual, Gate::None).await;
    tokio::time::sleep(Duration::from_secs(1)).await;

    assert!(
        transcript.lines().contains(&"look".to_owned()),
        "an instant action still goes out"
    );

    cancel.cancel();
    let _ = task.await;
}

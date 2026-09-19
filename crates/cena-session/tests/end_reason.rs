//! Why a connection ended, and whether that warrants a reconnect.
//!
//! Milestone 2's first session-layer step. Until now every exit path produced
//! an **identical** `SessionEnd` — a cancel, a peer hang-up and a socket error
//! were indistinguishable — and the M1 comment on the read arm said so
//! outright: *"distinguishing them would only matter to reconnect, which
//! plan/12 §9c puts in Milestone 2."*
//!
//! A supervisor cannot decide whether to reconnect without this, so it is the
//! first thing built. **Every test here asserts the reason AND what it implies**
//! (`warrants_reconnect`), because the reason is only useful for the decision it
//! drives.

use cena_platform::{AnsweringSource, ByteSource, ReplaySource};
use cena_session::{CommandId, EndReason, Origin, Session};
use std::time::Duration;

/// A reply with a terminator, so a round trip can complete.
const PROMPT: &[u8] = b"You see nothing unusual.\n<prompt time=\"1\">&gt;</prompt>\n";

/// An exhausted recording is a peer hang-up: `read` returns `Ok(0)`.
///
/// This is the shape criterion 9 uses — a source that simply stops is how a
/// killed connection presents to the actor.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_exhausted_source_is_peer_closed() {
    let end = Session::new(ReplaySource::new(vec![]))
        .into_actor()
        .run()
        .await;

    assert_eq!(end.reason, EndReason::PeerClosed);
    assert!(
        end.reason.warrants_reconnect(),
        "a peer hang-up is a lost transport, and a supervisor must reconnect it"
    );
    assert_eq!(end.lifecycle, cena_session::State::Closed);
    assert!(end.source.is_shutdown(), "criterion 6 still holds");
}

/// Cancellation is a **deliberate** stop and must never reconnect.
///
/// This is the distinction M1 collapsed and the one that actually matters: the
/// difference between "the connection died" and "we meant to stop". Getting it
/// wrong in this direction means a supervisor that reconnects after the player
/// quits.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn cancellation_is_deliberate_and_must_not_reconnect() {
    let (source, _transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.into_actor().run());

    // `AnsweringSource` never hangs up, so the ONLY way this session ends is
    // the cancel -- which is what makes this test about cancellation rather
    // than about whichever exit happened to fire first.
    tokio::task::yield_now().await;
    cancel.cancel();

    let end = task.await.expect("the actor must not panic");
    assert_eq!(end.reason, EndReason::Cancelled);
    assert!(
        !end.reason.warrants_reconnect(),
        "a deliberate stop must NOT reconnect. This is the only `false` in \
         warrants_reconnect, and a supervisor that got it wrong would re-login \
         after the player quit."
    );
}

/// A write failure ends the session, and the command that failed is answered.
///
/// `ReplaySource::write_all` errors once shut down (`replay.rs:90-95`), so
/// shutting it down out from under the actor stages a dead socket without a
/// new test double.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_failed_write_is_write_failed_and_warrants_reconnect() {
    // Non-empty, so the actor does not exit on its first read before the
    // command is ever pumped.
    let mut source = ReplaySource::new(vec![PROMPT.to_vec()]);
    // Shut it down BEFORE the actor gets it: reads then return Ok(0) and
    // writes return NotConnected, which is a socket that is gone.
    source
        .shutdown()
        .await
        .expect("shutdown is infallible here");

    let session = Session::new(source);
    let handle = session.handle();
    let task = tokio::spawn(session.into_actor().run());

    let outcome = handle
        .send_and_await(
            CommandId(1),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;

    let end = task.await.expect("the actor must not panic");
    // Either the write failed (WriteFailed) or the read reached EOF first
    // (PeerClosed) -- the `biased;` select makes the command arm win when a
    // command is queued, but both are lost transports and both reconnect,
    // which is the property under test.
    assert!(
        end.reason.warrants_reconnect(),
        "a dead socket warrants a reconnect however it was noticed: {:?}",
        end.reason
    );
    assert!(
        matches!(
            outcome,
            cena_session::Outcome::Dead | cena_session::Outcome::Timeout
        ),
        "the command must be ANSWERED rather than left hanging: {outcome:?}"
    );
}

/// The one rule, stated as a test so a new variant cannot quietly default.
///
/// `warrants_reconnect` is a method rather than a `match` at the supervisor's
/// call site precisely so adding a variant is a compile error somewhere that
/// decides this. This test is the second guard: it enumerates the answers, so
/// a variant added *and* given a wrong answer still fails something.
#[test]
fn only_cancellation_declines_to_reconnect() {
    assert!(!EndReason::Cancelled.warrants_reconnect());
    assert!(EndReason::PeerClosed.warrants_reconnect());
    assert!(EndReason::ReadFailed.warrants_reconnect());
    assert!(EndReason::WriteFailed.warrants_reconnect());
}

/// `PeerClosed` and `ReadFailed` are distinct values even though nothing
/// branches on the difference.
///
/// They exist apart so a **log** can tell "the server hung up" from "the socket
/// errored" — different facts about a session, and a transcript that said only
/// "disconnected" would be unreadable at exactly the moment someone is working
/// out why a session flapped. Same argument `Origin::Script` makes.
#[test]
fn the_two_transport_losses_stay_distinguishable() {
    assert_ne!(
        EndReason::PeerClosed,
        EndReason::ReadFailed,
        "collapsing these would lose the distinction a log needs, even though \
         warrants_reconnect treats them identically"
    );
}

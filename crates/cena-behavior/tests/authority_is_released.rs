//! `look` releases the command authority on **every** exit path.
//!
//! `look.rs` says so -- *"Released on EVERY exit, including cancellation and a
//! dead session"* -- and the code is shaped for it: the loop is split into
//! `look_holding_authority` so no `?` can skip the release. That is good
//! structure and it was **completely untested**. Deleting the `release` line
//! left `cargo test -p cena-behavior` green (review BI-3, verified by
//! deletion).
//!
//! What it costs if it regresses is not subtle. The authority is single-holder
//! by design, so a behavior that exits without releasing locks every later
//! behavior out of the session permanently -- the same permanent lockout
//! `14f14b6` fixed on the dropped-release path, arriving by a different route.

use cena_behavior::{BehaviorError, LOOK_INTERVAL, look};
use cena_platform::AnsweringSource;
use cena_session::{AuthorityToken, CommandId, Session};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio_util::sync::CancellationToken;

/// The terminator that closes a round-trip window (`plan/12` §4.4).
const PROMPT: &[u8] = b"You see nothing unusual.\n<prompt time=\"1\">&gt;</prompt>\n";

/// A monotonic `CommandId` source, seeded at 0 so a replay is deterministic.
fn ids() -> impl FnMut() -> CommandId {
    let next = Arc::new(AtomicU64::new(0));
    move || CommandId(next.fetch_add(1, Ordering::Relaxed))
}

/// **A cancelled `look` leaves the authority claimable.**
///
/// The assertion is a SECOND claim, by a different token, after the first
/// behavior has ended. That is the property that matters to the next behavior,
/// and it is only true if the release actually ran -- there is no way to
/// satisfy it by accident.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_cancelled_look_releases_the_authority() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let session_cancel = session.cancel_token();
    let actor = tokio::spawn(session.into_actor().run());

    let stop = CancellationToken::new();
    let behavior_stop = stop.clone();
    let behavior_handle = handle.clone();
    let behavior = tokio::spawn(async move {
        look(&behavior_handle, &behavior_stop, ids(), AuthorityToken(1)).await
    });

    // Let it claim and get past a round trip, so the authority is genuinely
    // held when it is cancelled. Cancelling before the claim would make the
    // release trivially unnecessary and the test vacuous.
    tokio::time::advance(LOOK_INTERVAL / 2).await;
    assert!(
        transcript.written_count() >= 1,
        "the behavior must have claimed and sent before being stopped, or it \
         never held the authority this test is about"
    );

    stop.cancel();
    let result = behavior.await.expect("the behavior task must not panic");
    assert_eq!(
        result,
        Err(BehaviorError::Cancelled),
        "the first behavior must report that it was cancelled"
    );

    // **THE ASSERTION.** A different token claims what the first one held.
    let second = handle.claim(AuthorityToken(2)).await;
    assert!(
        second.is_ok(),
        "the authority was not released when `look` was cancelled, so no \
         later behavior can ever claim it. The authority is single-holder by \
         design, which makes this a PERMANENT lockout of the session -- the \
         same failure `14f14b6` fixed on the dropped-release path. Got: \
         {second:?}"
    );

    session_cancel.cancel();
    actor.await.expect("the actor task must not panic");
}

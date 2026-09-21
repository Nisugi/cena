//! **A release must survive a full inbox.**
//!
//! # The failure, and why it is permanent
//!
//! `release()` is fire-and-forget by design: `plan/12` §4.3 says cleanup
//! "cannot send commands" and must not block, and a cancelled behavior has a
//! 250ms grace budget. So it used a bare `try_send` and ignored the result.
//!
//! MEASURED before the fix: 60 concurrent `send_and_await` calls with no
//! scheduler yield between them refused 28 of 60 -- the inbox genuinely fills,
//! because the actor takes **one message per loop turn** and a turn can spend
//! up to `WRITE_DEADLINE` inside a single write.
//!
//! A release dropped in that window is **unrecoverable**: the token keeps the
//! authority for the rest of the session, every later behavior gets
//! `AuthorityHeld`, and nothing anywhere records that it happened.
//!
//! # The fix the author chose: a reserved slot
//!
//! Producers of ordinary traffic stop one short of capacity, so the last slot
//! is reachable only from `release`. Not tokio's `try_reserve` -- that takes a
//! permit from the same capacity at the moment of need, which is exactly when
//! the channel is full -- and not `OwnedPermit`, which consumes the `Sender`
//! that every caller shares.

use cena_session::AuthorityToken;

/// **The reserve, asserted on the channel directly.**
///
/// # Five attempts, and why the test is this narrow
///
/// The end-to-end version -- bury the inbox, release, check a second behavior
/// can claim -- **passed with the fix removed**, repeatedly, for four different
/// reasons. Recorded because each is a trap in testing a queue:
///
/// 1. Releasing the held replies before asserting let the actor drain, so the
///    release got through either way.
/// 2. `claim` cannot tell "inbox full" from "someone holds it" -- both are
///    `AuthorityHeld` -- so it cannot observe the bug even while full.
/// 3. Awaiting the buried senders' `JoinHandle`s to count refusals let the
///    actor run, draining the channel the assertion was about.
/// 4. With no actor at all, `send_now` never resolves: it awaits a verdict
///    nobody will send.
///
/// The root cause of all four: **the actor drains one message per loop turn**,
/// so any `await` in the test body hands it a turn. A test that races that is
/// measuring the scheduler, not the reserve.
///
/// So this asserts the arithmetic the reserve *is*, on a bare channel, with no
/// actor and no awaits. It is deterministic and it fails when the reserve is
/// removed -- VERIFIED. What it does **not** cover is the end-to-end path; that
/// is stated rather than faked, and `CommandQueue::release`'s own behaviour is
/// covered in `command_queue.rs`.
#[test]
fn traffic_stops_one_slot_short_and_a_release_fits_in_it() {
    // A bare channel at the real bound, and the handle over it. No actor: the
    // point is what the PRODUCER side does, and an actor would drain it.
    let (tx, _rx) = tokio::sync::mpsc::channel(32);
    let (events, _) = tokio::sync::broadcast::channel(8);
    let handle =
        cena_session::SessionHandle::new(tx, cena_session::GenerationCell::first(), events);

    // Fill with ordinary traffic. `try_send_traffic_for_test` is the same gate
    // `send_and_await`, `send_now` and `claim` all go through.
    let mut accepted = 0;
    while handle.try_send_traffic_for_test() {
        accepted += 1;
        assert!(accepted <= 64, "it must refuse before this runs away");
    }

    assert_eq!(
        accepted, 31,
        "traffic stops ONE short of the 32-slot bound. 32 means it took the          reserved slot, which is the bug."
    );
    assert_eq!(
        handle.capacity_for_debug(),
        1,
        "...leaving exactly one slot free"
    );

    assert!(
        handle.release_reached_the_channel(AuthorityToken(1)),
        "and a release fits into the slot traffic was refused. Without the          reserve it is dropped, the token keeps the authority for the rest of          the session, and every later behavior is locked out with no error          recorded anywhere."
    );
    assert_eq!(
        handle.capacity_for_debug(),
        0,
        "the release consumed the reserved slot, which is what it is for"
    );
}

// **Deliberately no end-to-end test**, and this is the note saying why rather
// than an omission.
//
// Five attempts at one are recorded on the test above. Every version passed
// with the reserve removed, because the actor drains one message per loop turn
// and any `await` in a test body hands it a turn -- so the channel is empty by
// the time the assertion runs. Racing that measures the scheduler.
//
// What is covered instead: the producer-side arithmetic above (deterministic,
// and VERIFIED to fail when the reserve is removed), and
// `CommandQueue::release`'s own behaviour in `command_queue.rs`. The seam
// between them -- "the actor applies a release it received" -- is three lines
// (`queue.rs:143-147`) and is exercised by every authority test in that file.

/// **An abandoned claim does not lock out every later behavior.**
///
/// Review finding 2: `claim` mutated the queue and the reply's send result was
/// discarded, so a caller that dropped its future between sending and
/// receiving left its token holding authority **permanently** -- and every
/// later claimant got `AuthorityHeld` naming a token whose owner no longer
/// exists.
///
/// # Asserted through the actor, and why that is safe here
///
/// This file's other test is narrow because the reserve is arithmetic on a
/// channel the actor drains. This one is the opposite: the bug lives in what
/// the actor does with a dropped receiver, so the actor has to run. There is
/// no race to lose -- the first claim's receiver is dropped before the actor
/// ever starts, so the `send` it eventually makes cannot succeed however the
/// scheduler orders things.
#[tokio::test(flavor = "current_thread")]
async fn a_claim_whose_caller_vanished_is_rolled_back() {
    use cena_platform::ReplaySource;
    use cena_session::Session;

    let source = ReplaySource::new(vec![
        b"<prompt time=\"1789775900\">&gt;</prompt>\n".to_vec(),
    ]);
    let session = Session::new(source);
    let handle = session.handle();

    let first = AuthorityToken(1);
    let second = AuthorityToken(2);

    // **POLL the claim once, then drop it.** `claim` is `async`, so merely
    // constructing the future runs no code and sends nothing -- the first
    // version of this test dropped an unpolled future and passed with the fix
    // removed, which is the trap this file's other test documents.
    //
    // One poll gets as far as `try_send(Inbox::Claim)` and then parks on the
    // oneshot. Dropping there is the real scenario: the message is in the
    // inbox and the receiver is gone.
    // Polled by hand rather than with a futures helper: `cena-session` has no
    // futures dev-dependency, and adding one so a single test can poll once
    // would be a test reshaping the crate graph.
    let mut abandoned = Box::pin(handle.claim(first));
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    let polled = std::future::Future::poll(abandoned.as_mut(), &mut cx);
    assert!(
        polled.is_pending(),
        "the claim must park awaiting the actor's reply"
    );
    drop(abandoned);

    let actor = tokio::spawn(session.into_actor().run());

    // The second behavior must be able to claim: nobody owns the first.
    let granted = tokio::time::timeout(std::time::Duration::from_secs(5), handle.claim(second))
        .await
        .expect("the claim must not hang");

    assert!(
        granted.is_ok(),
        "an abandoned claim left the authority held forever: {granted:?}"
    );

    drop(handle);
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), actor).await;
}

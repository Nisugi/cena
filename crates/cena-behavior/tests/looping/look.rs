//! # Why `look`, and why on a loop
//!
//! `look` is idempotent, has no prerequisites, works from any room, and
//! "renders a room" is already criterion 2 (`plan/12:537`). Nothing else in
//! the game is that safe to send repeatedly.
//!
//! But **a single `look` would not exercise criteria 4 and 5.** A one-shot
//! command has no midpoint to interleave a manual command into, and nothing to
//! stop: a `stop` that arrives after the only command has been sent has
//! nothing to cancel, so the test would pass without the behavior ever being
//! cancellable. That is the vacuous-test failure the house rule exists to
//! catch. So the behavior loops, and the loop is what criteria 4 and 5 are
//! actually about.
//!
//! # The cancellation shape, and what makes `PREEMPT_GRACE` achievable
//!
//! `plan/12` §4.3 gives preemption **250ms** to take effect, and §5.5 requires
//! a `CancellationToken` "checked at every await". The two awaits **in the
//! loop** are cancel-aware:
//!
//! - **the round trip**, through a `select!` against the token;
//! - **the sleep**, through a `select!` against the token.
//!
//! This said "both awaits", of which there are three (review BI-3). The third
//! is the **`claim`**, and it is awaited OUTSIDE the cancel select -- so a
//! stop issued while the authority is held by someone else waits for the
//! claim to resolve rather than returning immediately.
//!
//! That is bounded rather than unbounded: `claim` carries its own timeout, so
//! the wait ends either way. It is recorded here instead of being quietly
//! folded into "both", because the stop-latency budget is 250ms and this is
//! the one await that can exceed it.
//!
//! > **CORRECTED 2026-09-18.** This said the round trip was cancel-aware
//! > "through its own deadline plus the check after it". It was not. A 10s
//! > deadline is not cancellation, and the check after it is unreachable until
//! > that deadline fires -- MEASURED at 7s to stop, against a 250ms budget.
//! > The test only ever cancelled during the sleep, which is the one await
//! > where the claim was trivially true.
//!
//! The sleep is the one that matters. A bare `sleep(LOOK_INTERVAL).await`
//! would make the worst-case stop latency `LOOK_INTERVAL`, not
//! `PREEMPT_GRACE` -- and that is precisely what the falsification test for
//! criterion 4 substitutes to prove the test is not vacuous.

use cena_behavior::BehaviorError;
use cena_session::{AuthorityToken, CommandId, Frame, Origin, SessionHandle};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// How long one `look` round trip may take before the waiter gives up.
///
/// **A starting value, not a settled one.** `plan/12` §10 defers this class of
/// number to "measurement under real load", and the live run has not happened.
/// Lich's `fput` uses 60s (`reference/lich-5/lib/global_defs.rb:1503`), which
/// is a ceiling for a human-paced script rather than for a behavior.
pub const ROUND_TRIP_DEADLINE: Duration = Duration::from_secs(10);

/// How long to wait between looks. Same status: a starting value to be
/// measured, not a settled one.
pub const LOOK_INTERVAL: Duration = Duration::from_secs(1);

/// The frame that answers a `look`: the styled room description.
///
/// `plan/12` §4.4 makes attribution temporal and leaves WHICH frame answered
/// to "the waiter's matcher". `look` asks for a room, so a room is what
/// `Outcome::Confirmed` should carry -- without a matcher the window kept
/// whatever arrived last, and a real `look` resolved with `Confirmed(Compass)`.
///
/// > **CORRECTED 2026-09-18 against live traffic (author-supplied).** This
/// > matched `Frame::Component { id: "room desc" }`, **which a `look` never
/// > produces.** The game emits the room TWICE, in two different shapes, for
/// > two different consumers:
/// >
/// >   * **`look`** writes to the main/story stream as inline text bracketed by
/// >     `<style id="roomName"/>` and `<style id="roomDesc"/>`. No `compDef`,
/// >     no `component` -- VERIFIED by parsing a live `look`: zero `Component`
/// >     frames, and the description arrives as `Text` carrying
/// >     `style.preset == Some("roomDesc")`.
/// >   * **movement** additionally emits `<compDef id='room desc'>` inside
/// >     `<pushStream id='room'>`, which is what feeds the ROOM WINDOW.
/// >
/// > So the old matcher was keyed to the movement shape while the behavior
/// > sends `look`, and would have returned `Timeout` forever against the live
/// > game. The fixtures did not catch it because they were cut from corpus
/// > files that happened to contain the window feed.
///
/// Both shapes are accepted **here**, because either one means "a room
/// description arrived, so the `look` was answered" -- which is all this
/// matcher claims.
///
/// **They are NOT interchangeable to `GameState`, and this matcher must not be
/// read as saying they are** (author, 2026-09-18). `compDef` is the ROOM
/// WINDOW and means *where the character is*; the inline `roomDesc` text is the
/// STORY WINDOW and means *what the character saw*. Abilities that look into
/// another room write the story form **without** the window form -- which is
/// precisely how a client knows the character did not move. Folding the inline
/// shape into `GameState.room` would make a scried room look like a relocation.
/// See `cena_model::state`.
#[must_use]
pub fn is_room_description(frame: &Frame) -> bool {
    match frame {
        // The room-window feed, from movement.
        Frame::Component { id, .. } => id == "room desc",
        // The main-stream feed, from `look`.
        Frame::Text(text) => text.style.preset.as_deref() == Some("roomDesc"),
        _ => false,
    }
}

/// Look, repeatedly, until cancelled.
///
/// Every command goes through `handle`, which is the **same** handle the
/// manual surface holds -- criterion 3's "the same queue as the behavior's" is
/// structural rather than asserted.
///
/// # Errors
///
/// [`BehaviorError::Cancelled`] on `stop`, [`BehaviorError::Dead`] if the
/// session ended. There is no success arm: the loop runs until something stops
/// it, which is what a behavior is.
pub async fn look(
    handle: &SessionHandle,
    cancel: &CancellationToken,
    mut next_id: impl FnMut() -> CommandId,
    token: AuthorityToken,
) -> Result<(), BehaviorError> {
    // §4.2: a behavior runs a SEQUENCE, so it holds the authority for the
    // whole run and releases it on every exit path. Claiming does not queue --
    // if another behavior holds it, this one finds out now.
    handle
        .claim(token)
        .await
        .map_err(|_held| BehaviorError::AuthorityHeld)?;
    let result = look_holding_authority(handle, cancel, &mut next_id, token).await;
    // Released on EVERY exit, including cancellation and a dead session.
    // §4.3: cleanup releases resources; it must not send commands. A release
    // is not a command.
    handle.release(token);
    result
}

/// The loop itself, with the authority already held.
///
/// Split out so `look` releases on every exit path without a `defer`-shaped
/// guard or a `?` that could skip it.
async fn look_holding_authority(
    handle: &SessionHandle,
    cancel: &CancellationToken,
    next_id: &mut impl FnMut() -> CommandId,
    token: AuthorityToken,
) -> Result<(), BehaviorError> {
    loop {
        // Checked before the send, so a behavior started with an
        // already-cancelled token sends nothing. A cleanup path "cannot send
        // commands" (`plan/12` §4.3) -- otherwise "stop" becomes "send more".
        if cancel.is_cancelled() {
            return Err(BehaviorError::Cancelled);
        }

        // THE OTHER LOAD-BEARING AWAIT. `send_and_await` bounds itself with
        // ROUND_TRIP_DEADLINE (10s) and never sees the token, so awaiting it
        // bare made `stop` wait for the game to answer: MEASURED at 7s against
        // a PREEMPT_GRACE of 250ms, a 28x miss. A behavior spends nearly all
        // its life in this await, not in the sleep below, so this is the one
        // that decides whether criterion 4 actually holds.
        //
        // Cancelling does NOT un-send: the command may already have reached
        // the game (`plan/12` §4.4). `Cancelled` is the honest answer.
        let outcome = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(BehaviorError::Cancelled),
            outcome = handle.send_and_await(
                next_id(),
                "look",
                Origin::Behavior(token),
                ROUND_TRIP_DEADLINE,
                is_room_description,
            ) => outcome,
        };
        // A window that closed with nothing matched is not a failure: §4.4
        // says `Timeout` means "no match within the window", never "the
        // command did not happen". Looping is the correct response. A session
        // gone or going stops this run and says why -- NOT a retry: §5.1 says
        // no automation runs while a session has no transport, so looping
        // would spin against a socket that is gone.
        if let Some(gone) = BehaviorError::from_outcome(&outcome) {
            return Err(gone);
        }

        // THE LOAD-BEARING AWAIT. A bare `sleep(LOOK_INTERVAL).await` here
        // makes the worst-case stop latency LOOK_INTERVAL instead of
        // PREEMPT_GRACE, and criterion 4 goes red. See the module docs.
        tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(BehaviorError::Cancelled),
            () = tokio::time::sleep(LOOK_INTERVAL) => {}
        }
    }
}

//! `plan/16` §1: instant actions, and the gate that is *not* the queue's.
//!
//! # The defect this path exists to remove
//!
//! > **AUTHOR, 2026-09-18:** *"those sigils using fput suck because they wait
//! > for a response instead of being instant."*
//!
//! Cena had the same defect structurally. `CommandQueue::take_next` yields
//! nothing while a window is open and `send_and_await` was the only send path,
//! so `sigil of escape` -- the ability for leaving a fight you are losing --
//! queued behind whatever the running behavior last sent.
//!
//! **The first test in this file is that sentence, executable.** It opens a
//! window, leaves it open, and asserts the sigil still reached the wire. Run
//! it against the old code and it fails.

use cena_platform::AnsweringSource;
use cena_session::{
    AuthorityToken, CommandId, Gate, Origin, Outcome, Refusal, Sent, Session, State,
};
use std::time::Duration;

/// A reply with no prompt, so a window opened against it stays open.
const SILENT: &[u8] = b"You see nothing unusual.\n";
/// A line and then a prompt at server second 100, which calibrates the clock.
///
/// **The line matters.** `ingest` does not offer `Frame::Prompt` to a waiter's
/// matcher -- the prompt is the terminator, not the answer -- so a reply that
/// is *only* a prompt closes the window with no match and resolves
/// `Outcome::Timeout`. A calibrating round trip therefore needs something for
/// `any_frame` to accept. (Learned the direct way: written without the line,
/// two tests here failed on `Timeout` before `send_now` was ever reached.)
const PROMPT_AT_100: &[u8] = b"You see nothing unusual.\n<prompt time=\"100\">&gt;</prompt>\n";

/// **The headline.** An instant action goes out while another command's window
/// is open.
///
/// This is `plan/16` §1.1's "sent at the same time as the following command,
/// sometimes a few in a row" and §1.3's complaint in one assertion. The
/// `send_and_await` never resolves here -- deliberately, because the sigil must
/// not have to wait for it.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_instant_action_goes_out_while_a_window_is_open() {
    // Answers with a prompt so the clock calibrates, then the window for the
    // SECOND command stays open because nothing further arrives.
    let (source, transcript) = AnsweringSource::new(PROMPT_AT_100);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let actor = session.into_actor();
    let driver = tokio::spawn(actor.run());

    // Calibrate the clock: one full round trip, so a prompt has been seen.
    let first = handle
        .send_and_await(
            CommandId(1),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(
        matches!(first, Outcome::Confirmed(_)),
        "the calibrating round trip must complete, or the clock is unknown \
         and the gate refuses for the wrong reason: {first:?}"
    );

    // Now the instant action, with no roundtime in effect.
    let sent = handle
        .send_now("sigil of escape", Origin::Manual, Gate::Roundtime)
        .await;
    assert_eq!(
        sent,
        Sent::Ok { at: Some(100) },
        "the sigil must reach the wire, and report the server second its gate \
         was decided on"
    );

    let lines = transcript.lines();
    assert!(
        lines.iter().any(|l| l == "sigil of escape"),
        "the sigil's bytes must be on the wire: {lines:?}"
    );

    cancel.cancel();
    let _ = driver.await;
}

/// The roundtime gate refuses, and says so with the typed reason.
///
/// Three seconds of roundtime, from the real capture's numbers: a
/// `<roundTime value='103'/>` against a prompt at 100.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_instant_action_is_refused_during_roundtime() {
    let (source, transcript) =
        AnsweringSource::new(b"<roundTime value='103'/><prompt time=\"100\">R&gt;</prompt>\n");
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let actor = session.into_actor();
    let driver = tokio::spawn(actor.run());

    let first = handle
        .send_and_await(
            CommandId(1),
            "attack",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(matches!(first, Outcome::Confirmed(_)), "{first:?}");

    let sent = handle
        .send_now("sigil of escape", Origin::Manual, Gate::Roundtime)
        .await;
    assert_eq!(
        sent,
        Sent::Refused(Refusal::Roundtime),
        "in roundtime, a gated action must be refused WITH THE REASON -- not \
         silently dropped, and not `Transient`, which would mean the clock is \
         unknown rather than that a roundtime is running"
    );

    let lines = transcript.lines();
    assert!(
        !lines.iter().any(|l| l == "sigil of escape"),
        "a refused action must not have reached the wire: {lines:?}"
    );

    cancel.cancel();
    let _ = driver.await;
}

/// `Gate::None` sends during roundtime, because the caller said the action is
/// not roundtime-gated.
///
/// The variant exists for the author's parenthesis -- *"shouldn't be subject to
/// typeahead or waiting **(depending on the action)**"*. Which actions are
/// exempt is UNVERIFIED (`plan/16` §8 q1); what is settled is that the type
/// lets a caller say so at the call site.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_ungated_action_sends_during_roundtime() {
    let (source, transcript) =
        AnsweringSource::new(b"<roundTime value='103'/><prompt time=\"100\">R&gt;</prompt>\n");
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let actor = session.into_actor();
    let driver = tokio::spawn(actor.run());

    let first = handle
        .send_and_await(
            CommandId(1),
            "attack",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(matches!(first, Outcome::Confirmed(_)), "{first:?}");

    let sent = handle
        .send_now("stance defensive", Origin::Manual, Gate::None)
        .await;
    assert_eq!(
        sent,
        Sent::Ok { at: None },
        "`at` is None because no clock was consulted -- the evidence field \
         reports what the gate READ, and an ungated send reads nothing"
    );
    assert!(
        transcript.lines().iter().any(|l| l == "stance defensive"),
        "an ungated action sends even in roundtime"
    );

    cancel.cancel();
    let _ = driver.await;
}

/// **Unknown is not permission.** With no prompt seen, a gated send refuses.
///
/// `plan/12` §5.2 makes `Unknown` first-class so it cannot be silently read as
/// `false`. A client that sent here would be asserting "not in roundtime" on
/// the strength of having been told nothing -- and `Transient` is the honest
/// answer, because a prompt will arrive and then the question is answerable.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_gated_action_refuses_while_the_clock_is_unknown() {
    let (source, transcript) = AnsweringSource::new(SILENT);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let actor = session.into_actor();
    let driver = tokio::spawn(actor.run());

    let sent = handle
        .send_now("sigil of escape", Origin::Manual, Gate::Roundtime)
        .await;
    assert_eq!(
        sent,
        Sent::Refused(Refusal::Transient),
        "no prompt has arrived, so `in_roundtime()` is None. Treating that as \
         'go ahead' is exactly the stale-belief bug plan/12 §5.2 forbids"
    );
    assert!(
        !transcript.lines().iter().any(|l| l == "sigil of escape"),
        "nothing may go out on an unknown clock"
    );

    cancel.cancel();
    let _ = driver.await;
}

/// A behavior's instant action is gated on readiness, exactly as its ordinary
/// commands are (`plan/12` §5.3).
///
/// Manual and script origins are not -- the player is never locked out of
/// their character -- and that asymmetry is asserted in the next test.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_behaviors_instant_action_is_refused_before_ready() {
    let (source, _transcript) = AnsweringSource::new(PROMPT_AT_100);
    let session = Session::new(source);
    let handle = session.handle();
    let mut actor = session.into_actor();
    assert_eq!(
        actor.lifecycle(),
        State::Connecting,
        "a freshly built session must not already be Ready, or this asserts \
         nothing"
    );

    let waiter = tokio::spawn(async move {
        handle
            .send_now(
                "sigil of escape",
                Origin::Behavior(AuthorityToken(1)),
                Gate::Roundtime,
            )
            .await
    });
    tokio::task::yield_now().await;
    actor.drain_commands_once().await;

    assert_eq!(
        waiter.await.expect("the waiter must not panic"),
        Sent::Refused(Refusal::Transient),
        "§5.3 gates BEHAVIORS, and an instant action is still a behavior \
         acting before the session has finished learning its state"
    );
}

/// Batching: several instant actions then the command they modify, in order.
///
/// > **AUTHOR:** *"generally I would send them at the same time as the
/// > following command, sometimes a few in a row, since they activate instantly
/// > and the command triggers."*
///
/// The ordering matters as much as the sending: a sigil that lands *after* the
/// attack it was meant to modify is the same failure as not sending it. That
/// holds because `SendNow` rides the **same channel** as `Command`.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn several_instant_actions_batch_ahead_of_their_trigger() {
    let (source, transcript) = AnsweringSource::new(PROMPT_AT_100);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let actor = session.into_actor();
    let driver = tokio::spawn(actor.run());

    let first = handle
        .send_and_await(
            CommandId(1),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(matches!(first, Outcome::Confirmed(_)), "{first:?}");

    for sigil in ["sigil of power", "sigil of defense", "sigil of focus"] {
        assert_eq!(
            handle
                .send_now(sigil, Origin::Manual, Gate::Roundtime)
                .await,
            Sent::Ok { at: Some(100) },
            "{sigil} must go out"
        );
    }
    let trigger = handle
        .send_and_await(
            CommandId(2),
            "attack",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(matches!(trigger, Outcome::Confirmed(_)), "{trigger:?}");

    let lines = transcript.lines();
    let at = |needle: &str| {
        lines
            .iter()
            .position(|l| l == needle)
            .unwrap_or_else(|| panic!("{needle} never reached the wire: {lines:?}"))
    };
    assert!(
        at("sigil of power") < at("sigil of defense")
            && at("sigil of defense") < at("sigil of focus")
            && at("sigil of focus") < at("attack"),
        "the sigils must precede the command they modify, in the order sent. \
         This holds because SendNow rides the SAME channel as Command: two \
         channels give no ordering guarantee between them. {lines:?}"
    );

    cancel.cancel();
    let _ = driver.await;
}

/// A script's instant action is treated like a manual one -- **not** gated on
/// readiness, and not a claimant.
///
/// `plan/16` §5a.1: a cross-character command runs "as if they just sent it".
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_scripts_instant_action_is_ungated_like_a_manual_one() {
    let (source, _transcript) = AnsweringSource::new(PROMPT_AT_100);
    let session = Session::new(source);
    let handle = session.handle();
    let mut actor = session.into_actor();
    assert_eq!(actor.lifecycle(), State::Connecting);

    let waiter = tokio::spawn(async move {
        handle
            .send_now("sigil of escape", Origin::Script, Gate::Roundtime)
            .await
    });
    tokio::task::yield_now().await;
    actor.drain_commands_once().await;

    assert_eq!(
        waiter.await.expect("the waiter must not panic"),
        Sent::Refused(Refusal::Transient),
        "refused for the CLOCK being unknown -- no prompt has arrived -- and \
         NOT for readiness. Both read `Transient`, which is why this test \
         exists beside the behavior one: the two paths must diverge at the \
         readiness check even where the refusal happens to agree"
    );
}

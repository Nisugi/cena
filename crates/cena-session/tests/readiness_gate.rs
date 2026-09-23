//! `plan/12` §5.3: **behaviors may not start until `Ready`**, and manual
//! input is not gated.
//!
//! This is the rule that makes criterion 4's "the behavior runs" mean
//! something: a behavior that could issue commands during `Syncing` would be
//! acting on state the session has not finished learning.
//!
//! # `Syncing` is inhabited
//!
//! It was transited, not inhabited: `run` stepped to `Ready` before its first
//! read, and the first two tests here drive the gate by hand because that was
//! the only way to reach its false branch. Since 2026-09-23 `run` holds
//! `Syncing` until the first prompt after `<endSetup/>` (`actor/readiness.rs`),
//! and the tests at the bottom of this file drive the rule through `run`.

use cena_platform::AnsweringSource;
use cena_session::{CommandId, Origin, Outcome, Session, State};
use std::time::Duration;

const PROMPT: &[u8] = b"You see nothing unusual.\n<prompt time=\"1\">&gt;</prompt>\n";

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_behavior_command_before_ready_is_refused_rather_than_queued() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();

    // The actor exists but has NOT been driven, so it is still in the state
    // `Session::new` left it in.
    let mut actor = session.into_actor();
    assert_eq!(
        actor.lifecycle(),
        State::Connecting,
        "a freshly built session must not already be Ready, or this test \
         asserts nothing"
    );

    // A behavior's command, with the session pre-Ready.
    let waiter = tokio::spawn(async move {
        handle
            .send_and_await(
                CommandId(1),
                "look",
                Origin::Behavior(cena_session::AuthorityToken(1)),
                Duration::from_secs(5),
                cena_session::queue::any_frame,
            )
            .await
    });
    tokio::task::yield_now().await;

    // Drive one turn of the actor's command intake by hand, on an actor that
    // was never run and so has not even reached `Syncing`.
    actor.drain_commands_once().await;

    let outcome = waiter.await.expect("the waiter must not panic");
    assert_eq!(
        outcome,
        Outcome::Refused(cena_session::Refusal::Transient),
        "plan/12 §5.3: behaviors may not start until Ready. A behavior's \
         command arriving during Connecting/Authenticating/Syncing must be \
         REFUSED, not silently queued -- §4.2's reason applies here too: \
         silent queueing is how you get an attack that fires four seconds \
         after the fight ended."
    );
    assert_eq!(
        transcript.written_count(),
        0,
        "nothing may reach the wire from a gated behavior"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_manual_command_before_ready_is_not_gated() {
    // §5.3 gates BEHAVIORS. §4.1 says the player is never locked out of their
    // character. Refusing a typed command during Syncing would be exactly that
    // lockout, so this asserts the gate is narrow rather than broad.
    let (source, _transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let mut actor = session.into_actor();
    assert_eq!(actor.lifecycle(), State::Connecting);

    let waiter = tokio::spawn(async move {
        handle
            .send_and_await(
                CommandId(1),
                "look",
                Origin::Manual,
                Duration::from_secs(5),
                cena_session::queue::any_frame,
            )
            .await
    });
    tokio::task::yield_now().await;
    actor.drain_commands_once().await;

    // An admitted command has no answer yet: its window has not even opened,
    // so its OWN deadline is what eventually resolves it. A refused one
    // resolves instantly. `Timeout` is therefore the evidence of admission,
    // and `Refused` is what a broadened gate would produce.
    //
    // Asserting `!waiter.is_finished()` here instead was VERIFIED worthless:
    // the test passed with the gate broadened to manual input, because a
    // spawned task that has resolved still reports unfinished until the
    // runtime polls it again.
    let outcome = waiter.await.expect("the waiter must not panic");
    assert_eq!(
        outcome,
        Outcome::Timeout,
        "a manual command must be ADMITTED before Ready, not refused: it \
         waits out its own deadline because no game is answering, which is \
         what Timeout means (plan/12 §4.4 -- 'no match within the window', \
         never 'the command did not happen'). A Refused here would mean the \
         player is locked out of their own character during Syncing, which \
         plan/12 §4.1 forbids."
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_full_command_channel_refuses_rather_than_blocking() {
    // `plan/12` §5.5: "bounded channels everywhere; no unbounded await". A
    // blocking send from the UI is how a slow session wedges the frontend, so
    // `SessionHandle::send_and_await` uses `try_send`, never `send().await`.
    //
    // The actor is never driven here, so nothing drains the channel and it
    // fills. That is the condition, not a contrivance: a session stuck in a
    // long read is exactly a session that is not draining.
    let (source, _transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let _actor = session.into_actor();

    // Manual origin, so the readiness gate above is not what refuses these.
    // Each send is its own task: an ADMITTED one parks on its reply until its
    // deadline, and would otherwise stop the loop dead.
    let mut waiters = Vec::new();
    for id in 0..200 {
        let sender = handle.clone();
        waiters.push(tokio::spawn(async move {
            sender
                .send_and_await(
                    CommandId(id),
                    "look",
                    Origin::Manual,
                    Duration::from_mins(1),
                    cena_session::queue::any_frame,
                )
                .await
        }));
        tokio::task::yield_now().await;
    }

    // Everything that was going to refuse has refused by now: `try_send`
    // returns without awaiting, so a refusal resolves on the first poll. The
    // rest are parked on their 60-second deadlines, which nothing will
    // advance, so `is_finished` separates the two without blocking on either.
    tokio::task::yield_now().await;
    let mut refusals = 0;
    let mut still_waiting = 0;
    for waiter in &waiters {
        if waiter.is_finished() {
            refusals += 1;
        } else {
            still_waiting += 1;
        }
    }

    assert!(
        refusals > 0,
        "a bounded channel that never refuses is not bounded. 200 commands \
         were offered to a session that drains nothing; {still_waiting} are \
         still parked and {refusals} came back immediately. If refusals is 0, \
         the send is blocking or the channel is unbounded -- either way a slow \
         session can wedge the caller (plan/12 §5.5)."
    );

    // And a refusal is TYPED, not a dropped command: the caller can act on it.
    for waiter in waiters {
        if waiter.is_finished() {
            let outcome = waiter.await.expect("a refused waiter must not panic");
            assert_eq!(
                outcome,
                Outcome::Refused(cena_session::Refusal::Transient),
                "a full queue must refuse with a reason the caller can retry \
                 on, not with Dead or a silent drop"
            );
            return;
        }
        waiter.abort();
    }
}

// ---------------------------------------------------------------------------
// When `run` becomes Ready (`actor/readiness.rs`)
//
// Driven through `run`, not by hand: `Syncing` is now inhabited, so the gate's
// false branch is reachable the way a real login reaches it. Every byte comes
// from a MANUAL command's scripted reply, because manual input is not gated
// (above) and `AnsweringSource` says nothing unprompted.
// ---------------------------------------------------------------------------

/// A line and its prompt: the prompt is the terminator, not a match, so a
/// reply of nothing but a prompt answers `Timeout`.
const BARE_PROMPT: &[u8] = b"Ok.\n<prompt time=\"1\">&gt;</prompt>\n";

/// Type `line` and wait for its reply to be read.
async fn type_line(handle: &cena_session::SessionHandle, id: u64, line: &str) {
    let outcome = handle
        .send_and_await(
            CommandId(id),
            line,
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(
        matches!(outcome, Outcome::Confirmed(_)),
        "{line}: {outcome:?}"
    );
}

/// Every lifecycle state published so far.
fn states(events: &mut tokio::sync::broadcast::Receiver<cena_session::Event>) -> Vec<State> {
    let mut seen = Vec::new();
    while let Ok(event) = events.try_recv() {
        if let cena_session::Event::StateChanged(s) = event {
            seen.push(s);
        }
    }
    seen
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn ready_is_the_first_prompt_after_end_setup_and_neither_alone() {
    let (source, transcript) = AnsweringSource::new(BARE_PROMPT);
    // The marker with no prompt behind it, as the wire sends it: indicators
    // and vitals follow `<endSetup/>`, and the burst ends at the prompt.
    transcript.answer(
        "marker",
        b"<endSetup/>\n<indicator id=\"IconSTANDING\" visible=\"y\"/>\n",
    );
    let session = Session::new(source);
    let handle = session.handle();
    let (_, mut events) = session.subscribe();
    let running = tokio::spawn(session.into_actor().run());

    // A prompt BEFORE the marker does not open the gate.
    type_line(&handle, 1, "early").await;
    let seen = states(&mut events);
    assert!(seen.contains(&State::Syncing), "{seen:?}");
    assert!(
        !seen.contains(&State::Ready),
        "a prompt before <endSetup/> made the session Ready: {seen:?}"
    );

    // The marker alone does not either. Its reply has no prompt, so the
    // command times out -- which is the point: nothing terminated the burst.
    let marker = handle
        .send_and_await(
            CommandId(2),
            "marker",
            Origin::Manual,
            Duration::from_secs(1),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(
        matches!(marker, Outcome::Confirmed(_) | Outcome::Timeout),
        "{marker:?}"
    );
    let seen = states(&mut events);
    assert!(
        !seen.contains(&State::Ready),
        "<endSetup/> alone made the session Ready, before the indicators and \
         vitals behind it: {seen:?}"
    );

    // The first prompt after it does.
    type_line(&handle, 3, "after").await;
    assert_eq!(states(&mut events), vec![State::Ready]);

    running.abort();
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_behavior_is_refused_while_syncing_and_runs_once_ready() {
    let (source, transcript) = AnsweringSource::new(BARE_PROMPT);
    transcript.answer("marker", b"<endSetup/>\n<prompt time=\"1\">&gt;</prompt>\n");
    let session = Session::new(source);
    let handle = session.handle();
    let running = tokio::spawn(session.into_actor().run());

    let behavior = |id| {
        let handle = handle.clone();
        async move {
            handle
                .send_and_await(
                    CommandId(id),
                    "look",
                    Origin::Behavior(cena_session::AuthorityToken(1)),
                    Duration::from_secs(5),
                    cena_session::queue::any_frame,
                )
                .await
        }
    };

    // Claimed up front, so a refusal below is the gate's and not authority's.
    handle
        .claim(cena_session::AuthorityToken(1))
        .await
        .expect("nobody else holds the authority");
    type_line(&handle, 1, "early").await;
    assert_eq!(
        behavior(2).await,
        Outcome::Refused(cena_session::Refusal::Transient),
        "run() is past its first read and still Syncing: the gate must hold"
    );
    let written = transcript.written_count();

    type_line(&handle, 3, "marker").await;
    let ran = behavior(4).await;
    assert!(
        matches!(ran, Outcome::Confirmed(_)),
        "a Ready session must run the behavior: {ran:?}"
    );
    assert_eq!(transcript.written_count(), written + 2);

    running.abort();
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn without_end_setup_the_first_prompt_after_the_deadline_is_ready() {
    let (source, _transcript) = AnsweringSource::new(BARE_PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let (_, mut events) = session.subscribe();
    let running = tokio::spawn(session.into_actor().run());

    type_line(&handle, 1, "first").await;
    tokio::time::advance(cena_session::SETUP_DEADLINE.saturating_sub(Duration::from_secs(1))).await;
    type_line(&handle, 2, "just before").await;
    assert!(
        !states(&mut events).contains(&State::Ready),
        "the fallback fired before SETUP_DEADLINE"
    );

    tokio::time::advance(Duration::from_secs(2)).await;
    type_line(&handle, 3, "after").await;
    assert_eq!(
        states(&mut events),
        vec![State::Ready],
        "a server that never sends <endSetup/> must not hold the session \
         un-Ready for the life of the connection"
    );

    running.abort();
}

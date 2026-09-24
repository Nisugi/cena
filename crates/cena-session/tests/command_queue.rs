//! Criteria 3, 5 and 6: the shared queue, interleaving, and a clean
//! disconnect.
//!
//! - **3** (`plan/12:538`): "A manually typed command goes through the same
//!   queue as the behavior's, returning a typed `Outcome`."
//! - **5** (`plan/12:541`): "Manual input is **interleaved, not preemptive**:
//!   a command typed mid-behavior jumps the queue, runs its round-trip, and
//!   the behavior **continues**." CORRECTED 2026-09-18 -- an earlier draft
//!   said manual input preempts, which would make `say hi` mid-hunt abort the
//!   hunt.
//! - **6** (`plan/12:550`): "Disconnect is clean: task ends, no leaked
//!   sockets, no panic."

use cena_platform::{AnsweringSource, ReplaySource};
use cena_session::{CommandId, Origin, Outcome, Session};
use std::time::Duration;

/// The terminator. `plan/12` §4.4: the next `Frame::Prompt` closes a window.
const PROMPT: &[u8] = b"You see nothing unusual.\n<prompt time=\"1\">&gt;</prompt>\n";

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_manual_command_returns_a_typed_outcome() {
    let (source, transcript) = AnsweringSource::new(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
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

    // Typed, and specifically Confirmed -- not a bool, not a String, and not
    // Timeout. `plan/12` §4.5 specifies this enum, and the point of criterion
    // 3 is that a command's answer has a TYPE.
    assert!(
        matches!(outcome, Outcome::Confirmed(_)),
        "a manual command must resolve to a typed Outcome::Confirmed once its \
         prompt closes the window (plan/12 §4.4). Got: {outcome:?}"
    );

    // And it went out as ONE write, body and newline together --
    // cena_platform::bytes::ByteSource::write_all's contract, ported from the
    // spike (src/main.rs:110-122): two writes can emit two TLS records and the
    // server drops the command.
    assert_eq!(
        transcript.lines(),
        vec!["look".to_owned()],
        "the command must reach the wire as ONE write of the finished message \
         -- body and newline together. Two write_all calls can emit two TLS \
         records and this server does not tolerate a command split across \
         them (spike/eaccess-spike/src/main.rs:110-122)."
    );

    cancel.cancel();
    task.await.expect("the actor task must not panic");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_manual_command_jumps_the_queue_ahead_of_the_behaviors() {
    use cena_session::CommandQueue;
    use tokio::sync::oneshot;

    // Driven at the queue directly rather than through a session, because the
    // ORDER is the whole assertion and a session would also impose its own
    // one-window-at-a-time pacing on top of it -- which would make the test
    // pass even if `next()` drained `held` first.
    let mut queue = CommandQueue::new();
    let mut keep = Vec::new();
    for (id, line, origin) in [
        (
            1,
            "attack",
            Origin::Behavior(cena_session::AuthorityToken(1)),
        ),
        (
            2,
            "attack",
            Origin::Behavior(cena_session::AuthorityToken(1)),
        ),
        (3, "say hi", Origin::Manual),
    ] {
        let (reply, rx) = oneshot::channel();
        keep.push(rx);
        queue.admit(cena_session::Envelope {
            id: CommandId(id),
            line: line.to_owned(),
            origin,
            reply,
            generation: cena_session::Generation::FIRST,
            matcher: cena_session::queue::any_frame,
            quiet: false,
        });
    }

    let order: Vec<String> = std::iter::from_fn(|| queue.take_next())
        .map(|envelope| envelope.line)
        .collect();

    assert_eq!(
        order,
        vec![
            "say hi".to_owned(),
            "attack".to_owned(),
            "attack".to_owned()
        ],
        "manual input is never QUEUED BEHIND automation (plan/12 §4.1). It was \
         admitted third and must come out first."
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_manual_command_does_not_revoke_the_authority() {
    use cena_session::{AuthorityToken, CommandQueue};
    use tokio::sync::oneshot;

    // §4.1's correction in one assertion. The superseded draft made manual
    // input priority 1 and preemptive; building that would mean this token is
    // gone after the manual command, and `say hi` mid-hunt would abort Hunt.
    let mut queue = CommandQueue::new();
    let behavior = AuthorityToken(7);
    queue.claim(behavior).expect("the authority must be free");

    let (reply, _rx) = oneshot::channel();
    queue.admit(cena_session::Envelope {
        id: CommandId(1),
        line: "say hi".to_owned(),
        origin: Origin::Manual,
        reply,
        generation: cena_session::Generation::FIRST,
        matcher: cena_session::queue::any_frame,
        quiet: false,
    });
    let _ = queue.take_next();

    assert_eq!(
        queue.authority(),
        Some(behavior),
        "manual input is NOT a claimant (plan/12 §4.1, CORRECTED 2026-09-18). \
         Admitting and sending one must leave the authority exactly where it \
         was, or typing `say hi` mid-hunt aborts the hunt."
    );

    // And a second claimant is still refused, which is what "the authority is \
    // still held" actually means to a behavior (§4.2).
    assert!(
        queue.claim(AuthorityToken(8)).is_err(),
        "a second behavior must get AuthorityHeld while the first holds it \
         (plan/12 §4.2): it does not queue behind it"
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn disconnect_is_clean_and_answers_everyone_waiting() {
    // An empty recording: the source is at end of stream on the first read, so
    // the session ends the way a peer hang-up ends it.
    let end = Session::new(ReplaySource::new(vec![]))
        .into_actor()
        .run()
        .await;

    // "Task ends": `run` returned at all. "No panic": this test would have
    // failed rather than reached here.
    assert_eq!(
        end.lifecycle,
        cena_session::State::Closed,
        "a session that ended must be in Closed, not left in Ready"
    );

    // "No leaked sockets": the source was SHUT DOWN, asked of the source
    // itself rather than inferred from a drop that nothing observes.
    assert!(
        end.source.is_shutdown(),
        "criterion 6 is 'no leaked sockets'. The actor owns the source and \
         must close it on every exit path -- cancel, end of stream, and read \
         error are not mutually exclusive, which is why shutdown() is \
         idempotent."
    );
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_waiter_gets_dead_rather_than_hanging_when_the_session_ends() {
    let (source, _transcript) = AnsweringSource::new(b"");
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.into_actor().run());

    // Cancel while the command is in flight and nothing will ever answer it.
    let waiter = tokio::spawn(async move {
        handle
            .send_and_await(
                CommandId(1),
                "look",
                Origin::Manual,
                Duration::from_mins(10),
                cena_session::queue::any_frame,
            )
            .await
    });
    tokio::task::yield_now().await;
    cancel.cancel();

    let outcome = waiter.await.expect("the waiter must not panic");
    assert_eq!(
        outcome,
        Outcome::Dead,
        "criterion 6 is not only 'the task ends'. A caller blocked in \
         send_and_await must be told the session died, not left to sit out its \
         own 600-second deadline."
    );
    task.await.expect("the actor task must not panic");
}

/// `plan/12` §4.4: "frames are offered to **the waiter's matcher**", and the
/// window "resolves with what it **matched**".
///
/// Found by adversarial review after the whole suite was green. `offer`
/// assigned `matched` unconditionally, so it held the LAST frame before the
/// prompt rather than the answer -- a real `look` resolved with
/// `Confirmed(Compass)`, the exits, instead of the room.
///
/// Goes RED on restoring the unconditional assignment in `CommandQueue::offer`
/// (verified): the first assert then sees `Compass`, because the compass
/// arrives after the room description and overwrites it.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_window_resolves_with_what_the_matcher_accepted() {
    // Room description, then a compass, then the prompt. The matcher wants the
    // room, so the compass arriving later must not displace it.
    const ROOM_THEN_COMPASS: &[u8] = b"<component id='room desc'>A dusty road.</component>\n\
<compass><dir value=\"e\"/></compass>\n\
<prompt time=\"1\">&gt;</prompt>\n";

    fn is_room(frame: &cena_session::Frame) -> bool {
        matches!(frame, cena_session::Frame::Component { id, .. } if id == "room desc")
    }

    fn matches_nothing(_frame: &cena_session::Frame) -> bool {
        false
    }

    let (source, _transcript) = AnsweringSource::new(ROOM_THEN_COMPASS);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let actor = tokio::spawn(session.into_actor().run());

    let outcome = handle
        .send_and_await(
            CommandId(1),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            is_room,
        )
        .await;

    match outcome {
        Outcome::Confirmed(frame) => assert!(
            is_room(&frame),
            "the matcher asked for the room description; the window resolved \
             with {frame:?} instead. Last-frame-wins returns whatever happened \
             to arrive nearest the terminator."
        ),
        other => panic!("expected Confirmed(room description), got {other:?}"),
    }

    // And a matcher that accepts nothing must resolve Timeout -- NOT the last
    // frame, and not a false Confirmed. §4.4: Timeout means "no match within
    // the window", never "the command did not happen".
    let (source2, _t2) = AnsweringSource::new(ROOM_THEN_COMPASS);
    let session = Session::new(source2);
    let handle2 = session.handle();
    let cancel2 = session.cancel_token();
    let actor2 = tokio::spawn(session.into_actor().run());

    let outcome = handle2
        .send_and_await(
            CommandId(2),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            matches_nothing,
        )
        .await;
    assert_eq!(
        outcome,
        Outcome::Timeout,
        "a matcher that accepts nothing must time out, not confirm a frame it \
         rejected"
    );

    cancel.cancel();
    cancel2.cancel();
    let _ = actor.await;
    let _ = actor2.await;
}

/// `plan/12` §4.2: "A behavior that wants the authority while another holds it
/// gets `Err(AuthorityHeld)`. It does **not** queue behind it — silent
/// queueing is how you get an attack that fires four seconds after the fight
/// ended."
///
/// This drives a real `Session`, not a bare `CommandQueue`. That distinction
/// is the whole point: `CommandQueue` had `claim`/`release`/`authority` and
/// unit tests for them from the start, and **nothing in the session ever
/// called them**, so the rule was a wish (`plan/05` §0). A test that drives
/// the queue directly proves the struct has a field; only this one proves the
/// session consults it.
///
/// Goes RED on removing the authority check from `SessionActor::admit`
/// (verified): the second behavior's command reaches the wire, and the
/// transcript reads `["attack", "retreat"]`.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_behavior_without_the_authority_is_refused_not_queued() {
    let (source, transcript) = AnsweringSource::logged_in(PROMPT);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let (_, mut events) = session.subscribe();
    let actor = tokio::spawn(session.into_actor().run());
    // Behaviors wait for `Ready` (`readiness_gate.rs`); this is about authority.
    while events.recv().await.expect("the session is running")
        != cena_session::Event::StateChanged(cena_session::State::Ready)
    {}

    let first = cena_session::AuthorityToken(1);
    let second = cena_session::AuthorityToken(2);

    handle.claim(first).await.expect("the authority is free");

    // A second claimant is told NOW, not made to wait.
    let held = handle.claim(second).await;
    assert!(
        held.is_err(),
        "a second claimant must get Err(AuthorityHeld) while the first holds \
         the authority, and must get it immediately: {held:?}"
    );

    // The holder's command goes out.
    let ok = handle
        .send_and_await(
            CommandId(1),
            "attack",
            Origin::Behavior(first),
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(
        matches!(ok, Outcome::Confirmed(_)),
        "the authority holder's command must run: {ok:?}"
    );

    // The non-holder's does NOT -- and is refused rather than queued.
    let refused = handle
        .send_and_await(
            CommandId(2),
            "retreat",
            Origin::Behavior(second),
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert_eq!(
        refused,
        Outcome::Refused(cena_session::Refusal::Permanent),
        "a command from a behavior that does not hold the authority must be \
         REFUSED. Timeout would mean it was queued and never answered; \
         Confirmed would mean it ran."
    );

    // The wire is the ground truth: `retreat` must never have been sent.
    assert_eq!(
        transcript.lines(),
        vec!["attack".to_owned()],
        "the refused command must not reach the wire at all -- that is the \
         'attack that fires four seconds after the fight ended' §4.2 names"
    );

    // And after release, the second behavior can take over.
    handle.release(first);
    tokio::task::yield_now().await;
    assert!(
        handle.claim(second).await.is_ok(),
        "the authority must be claimable again once its holder releases it"
    );

    cancel.cancel();
    let _ = actor.await;
}

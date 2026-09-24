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

use cena_platform::{AnsweringSource, ByteSource};
use cena_session::{
    AuthorityToken, CommandId, Gate, Origin, Outcome, Refusal, Sent, Session, State,
};
use std::sync::{Arc, Mutex};
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
// # WAS "INTERMITTENT AND NOT YET EXPLAINED". It is explained now, and the
// # guess recorded here was WRONG -- corrected 2026-09-21.
//
// The earlier note blamed `start_paused = true` plus `tokio::spawn` racing the
// paused clock under full-crate parallelism, and honestly labelled that a
// guess. It was, and it was wrong. The real cause is not about parallelism at
// all:
//
// `GameState::game_time_now` is `base + game_time_received.elapsed().as_secs()`
// (`cena-model/src/state/clock.rs:55`), and `game_time_received` is a
// **`std::time::Instant`** (`state/equality.rs:11`). Tokio's `start_paused`
// controls `tokio::time`, NOT `std::time` -- so that elapsed reading is real
// wall-clock time however the test drives the runtime.
//
// Whether `as_secs()` truncates to 0 or 1 therefore depends on where the run
// falls relative to a one-second boundary. MEASURED 2026-09-21: it fails
// single-threaded, alone, in a clean worktree at `dbadaef` -- deterministically
// on this machine. "One run in three, only under load" was a symptom on a
// faster machine, not the mechanism, and chasing it as a concurrency bug is why
// it stayed open.
//
// **The assertion was the bug, not the code under test.** Pinning an exact
// server second to a value extrapolated from real elapsed time cannot hold.
// This is very likely also the M4 Windows CI failure in
// `status_and_clock::a_real_roundtime_is_in_effect_and_then_is_not`, reported
// there as "server clock advanced by one second"
// (`plan/m4-despana-status.md`).
//
// Run with `--no-fail-fast` regardless: cargo's default makes one failure look
// like dozens of missing tests.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_instant_action_goes_out_while_a_window_is_open() {
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

    // **A SECOND command, left UNANSWERED, so its window is genuinely open.**
    //
    // This test's name is its whole claim, and it used to assert nothing of
    // the kind: the calibrating `look` above was awaited to `Confirmed`, which
    // CLOSES its window, and there was no second command. Every assertion
    // below passed with no window open anywhere, so a `send_now` routed
    // through `CommandQueue` -- the exact thing this exists to forbid -- would
    // have passed it too (review SE-8).
    //
    // `hold_replies` is what makes the window stay open: the source stops
    // answering, so no prompt arrives to close it. The waiter is spawned
    // because `send_and_await` does not return until it is answered, and the
    // point is that it is not.
    //
    // Each gets ITS OWN reply, so the end of this test can see whose text
    // resolved the attack (review finding 1).
    transcript.answer("attack", ATTACK_REPLY);
    transcript.answer("sigil of escape", SIGIL_REPLY);
    transcript.hold_replies();
    let blocked_handle = handle.clone();
    let blocked = tokio::spawn(async move {
        blocked_handle
            .send_and_await(
                CommandId(2),
                "attack",
                Origin::Manual,
                Duration::from_secs(30),
                cena_session::queue::any_frame,
            )
            .await
    });

    // Let the attack reach the wire and open its window.
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        transcript.lines().iter().any(|l| l == "attack"),
        "the blocking command must be on the wire with its window open, or \
         the sigil below is not racing anything: {:?}",
        transcript.lines()
    );
    assert!(
        !blocked.is_finished(),
        "the blocking command must still be WAITING -- if it resolved, its \
         window is closed and this test is back to proving nothing"
    );

    // Now the instant action, with a window open and no roundtime in effect.
    let sent = handle
        .send_now("sigil of escape", Origin::Manual, Gate::Roundtime)
        .await;
    // **At 100 or 101, not exactly 100.** The gate reports `game_time_now`,
    // which extrapolates from a real `Instant` (see this test's note): crossing
    // a second boundary makes 101 the honest answer. What must hold is that the
    // sigil reached the wire having decided its gate on a KNOWN clock --
    // `Some`, not `None`, which is the `plan/12` §5.2 distinction this gate
    // exists to respect, and the thing a looser `matches!` would drop.
    let Sent::Ok { at: Some(at) } = sent else {
        panic!("the sigil must reach the wire with its gate decided on a known clock: {sent:?}");
    };
    assert!(
        (100..=101).contains(&at),
        "the gate was decided at server second {at}, which is neither the \
         calibrating prompt's second nor the one after it"
    );

    let lines = transcript.lines();
    assert!(
        lines.iter().any(|l| l == "sigil of escape"),
        "the sigil's bytes must be on the wire: {lines:?}"
    );

    // **The sigil came AFTER the attack, and did not wait for it.** That is
    // the property: `send_now` bypasses the queue rather than being ordered
    // behind the command whose window is still open.
    let attack_at = lines.iter().position(|l| l == "attack");
    let sigil_at = lines.iter().position(|l| l == "sigil of escape");
    assert!(
        matches!((attack_at, sigil_at), (Some(a), Some(s)) if a < s),
        "the sigil must reach the wire after the attack was sent and while \
         its window is still open. Order seen: {lines:?}"
    );
    assert!(
        !blocked.is_finished(),
        "the attack's window must STILL be open after the sigil went out -- \
         if the sigil closed it, `send_now` is consuming a window it has no \
         attribution in, which is review finding SE-5"
    );

    // **Now the replies, in the order the server sends them: the ATTACK's
    // first**, because it was on the wire first. This half used to be absent
    // -- the test above named the case and never released a reply -- and the
    // case was broken: the attack resolved `Confirmed("You feel a surge.")`.
    // The owed-prompt count assumed every instant action's prompt arrives
    // before the waiting command's, which is true only of one sent BEFORE the
    // command. So the attack's text was suppressed, its prompt was spent as
    // the sigil's, and the sigil's text was credited to the attack.
    transcript.release_one();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(
        blocked.is_finished(),
        "the attack's own prompt arrived and must close its window. Still \
         waiting means that prompt was spent as the sigil's"
    );
    let resolved = blocked.await.expect("the attack's task must not panic");
    assert_eq!(
        text_of(&resolved).as_deref(),
        Some("You swing at the kobold."),
        "the attack must resolve on ITS OWN reply, not the sigil's: {resolved:?}"
    );

    // And the sigil's prompt, still owed, must not close the NEXT window.
    let looked = the_next_window_survives_one_owed_prompt(&handle, &transcript).await;
    assert_eq!(
        looked.as_ref().and_then(text_of).as_deref(),
        Some("You see nothing unusual."),
        "the look must resolve on its own prompt: {looked:?}"
    );

    cancel.cancel();
    let _ = driver.await;
}

/// Send a `look` while one instant action's prompt is still owed, release
/// that prompt, and check the look is still waiting; then release its own and
/// return what the look resolved to (`None` if it never did).
///
/// Returns rather than asserting on the result, so the helper needs no
/// `expect` -- the workspace denies it outside `#[test]` fns.
async fn the_next_window_survives_one_owed_prompt(
    handle: &cena_session::SessionHandle,
    transcript: &cena_platform::TranscriptHandle,
) -> Option<Outcome> {
    let next = tokio::spawn({
        let handle = handle.clone();
        async move {
            handle
                .send_and_await(
                    CommandId(3),
                    "look",
                    Origin::Manual,
                    Duration::from_secs(30),
                    cena_session::queue::any_frame,
                )
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(20)).await;
    transcript.release_one();
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(
        !next.is_finished(),
        "the sigil's prompt closed the next command's window: {:?}",
        transcript.lines()
    );
    transcript.release_one();
    tokio::time::timeout(Duration::from_secs(1), next)
        .await
        .ok()?
        .ok()
}

/// A socket whose writes each take three seconds, recording what landed.
struct SlowWrite(Arc<Mutex<Vec<String>>>);

impl ByteSource for SlowWrite {
    async fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        std::future::pending().await
    }

    async fn write_all(&mut self, message: &[u8]) -> std::io::Result<()> {
        tokio::time::sleep(Duration::from_secs(3)).await;
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(String::from_utf8_lossy(message).trim().to_owned());
        Ok(())
    }

    async fn shutdown(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// **An instant action whose caller was told `Dead` is never written.**
///
/// `SessionHandle::send_now` stops waiting after five seconds and answers
/// `Dead` -- "nothing sent it". Three sends into a socket that takes three
/// seconds per write put the third past that deadline while it is still in
/// the inbox, and the actor used to write it anyway: VERIFIED before the fix,
/// all three sends on the wire with the third's caller told `Dead` (review
/// finding 4).
/// A caller that believed `Dead` and re-sent `sigil of escape` another way
/// would fire it twice.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_instant_action_whose_caller_gave_up_is_not_written() {
    let wire = Arc::new(Mutex::new(Vec::new()));
    let session = Session::new(SlowWrite(Arc::clone(&wire)));
    let handle = session.handle();
    let cancel = session.cancel_token();
    let driver = tokio::spawn(session.into_actor().run());

    let lines = ["sigil of power", "sigil of defense", "sigil of escape"];
    let mut sends = Vec::new();
    for line in lines {
        let handle = handle.clone();
        sends.push(tokio::spawn(async move {
            handle.send_now(line, Origin::Manual, Gate::None).await
        }));
        tokio::task::yield_now().await;
    }
    let mut verdicts = Vec::new();
    for send in sends {
        verdicts.push(send.await.expect("a send must not panic"));
    }
    // Long enough for every write the actor would ever make to finish.
    tokio::time::sleep(Duration::from_secs(10)).await;
    let written = wire
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();

    assert_eq!(
        verdicts[2],
        Sent::Dead,
        "guard: the third send must have outlived its caller's wait, or this \
         test is not exercising the case. Verdicts: {verdicts:?}"
    );
    assert!(
        !written.iter().any(|w| w == "sigil of escape"),
        "the caller was told `Dead` before the actor reached this send, and \
         it was written anyway: {written:?}"
    );
    // NOT asserted: the SECOND send. Its caller also gives up at five
    // seconds, but by then the actor is already inside that write -- which
    // lands at six. A write in progress cannot be recalled, so `Dead` there is
    // a verdict on the wait, not on the wire. The fix closes the case where
    // the actor had not started; see `SessionHandle::send_now`.

    cancel.cancel();
    let _ = driver.await;
}

/// What the attack says, distinct from anything else in this file.
const ATTACK_REPLY: &[u8] = b"You swing at the kobold.
<prompt time=\"101\">&gt;</prompt>
";
/// What the sigil says.
const SIGIL_REPLY: &[u8] = b"You feel a surge.
<prompt time=\"101\">&gt;</prompt>
";

/// The text an outcome was confirmed on, if it was confirmed on text.
fn text_of(outcome: &Outcome) -> Option<String> {
    match outcome {
        Outcome::Confirmed(frame) => match &**frame {
            cena_protocol::Frame::Text(text) => Some(text.content.trim().to_owned()),
            _ => None,
        },
        _ => None,
    }
}

/// **An instant action whose prompt never comes stops being owed.**
///
/// Review finding 9: the count only ever went down on a prompt, so one
/// `send_now` that drew none -- swallowed, or merged into another reply --
/// left every later window closing one prompt late for the rest of the
/// connection. INFERRED risk; this pins the safety bound (`owed.rs`).
///
/// The sigil here answers with text and NO prompt. Past the deadline, the next
/// command's prompt must be its own.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_owed_prompt_that_never_arrives_does_not_hold_the_next_window() {
    let (source, transcript) = AnsweringSource::new(PROMPT_AT_100);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let driver = tokio::spawn(session.into_actor().run());

    transcript.answer(
        "sigil of power",
        b"You feel a surge.
",
    );
    assert!(matches!(
        handle
            .send_now("sigil of power", Origin::Manual, Gate::None)
            .await,
        Sent::Ok { .. }
    ));
    // Longer than any instant action takes to be answered.
    tokio::time::sleep(Duration::from_mins(1)).await;

    let looked = handle
        .send_and_await(
            CommandId(1),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert_eq!(
        text_of(&looked).as_deref(),
        Some("You see nothing unusual."),
        "the look's own prompt was spent on a sigil that was never answered \
         with one: {looked:?}"
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
        // 100 or 101, for the reason the first test's note gives: the gate's
        // second comes from a real `Instant`, so a second boundary crossed
        // mid-run is a legitimate 101 rather than a defect.
        let sent = handle
            .send_now(sigil, Origin::Manual, Gate::Roundtime)
            .await;
        let Sent::Ok { at: Some(at) } = sent else {
            panic!("{sigil} must go out with its gate decided on a known clock: {sent:?}");
        };
        assert!(
            (100..=101).contains(&at),
            "{sigil} decided its gate at server second {at}"
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

/// **A sigil's prompt does not close the attack's window.**
///
/// `send_now` bypasses the queue, so its response is attributed to nothing —
/// but it still draws a prompt, and any prompt closed the single round-trip
/// window. Three sigils followed by `send_and_await("attack")` therefore
/// resolved the attack on the FIRST sigil's response.
///
/// VERIFIED on the wire before the fix: the attack came back
/// `Confirmed("You feel a surge.")` — a sigil's text — under `any_frame`.
/// Under a strict matcher it would have timed out instead, with its real
/// answer arriving a window late. Every time, for the batching shape the
/// author documented as normal usage (review SE-5).
///
/// `tests/send_now.rs`'s existing batching test passes either way, because
/// `AnsweringSource` consumes each reply between sends. Holding the replies
/// is what reproduces a real round trip, where the sigils' prompts are still
/// in flight when the attack goes out.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_instant_actions_prompt_does_not_resolve_the_next_command() {
    let (source, transcript) = AnsweringSource::new(PROMPT_AT_100);
    let session = Session::new(source);
    let handle = session.handle();
    let cancel = session.cancel_token();
    let driver = tokio::spawn(session.into_actor().run());

    let calibrate = handle
        .send_and_await(
            CommandId(1),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(matches!(calibrate, Outcome::Confirmed(_)), "{calibrate:?}");

    // Held, so the sigils' prompts are still owed when the attack is sent.
    transcript.hold_replies();
    for sigil in ["sigil of power", "sigil of defense", "sigil of focus"] {
        assert!(
            matches!(
                handle.send_now(sigil, Origin::Manual, Gate::None).await,
                Sent::Ok { .. }
            ),
            "{sigil} must go out"
        );
    }

    let attack = tokio::spawn({
        let handle = handle.clone();
        async move {
            handle
                .send_and_await(
                    CommandId(2),
                    "attack",
                    Origin::Manual,
                    Duration::from_secs(30),
                    cena_session::queue::any_frame,
                )
                .await
        }
    });
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        !attack.is_finished(),
        "the attack must still be waiting before any reply is released, or \
         this test is not exercising the race"
    );

    // **One sigil prompt at a time, checking in between.**
    //
    // This is what separates a correct implementation from the defect. With
    // every reply released at once, "the attack resolved on a sigil's prompt"
    // and "the attack resolved on its own" look identical -- which is why the
    // first cut of this test passed against the unfixed code.
    for (i, sigil) in ["power", "defense", "focus"].iter().enumerate() {
        transcript.release_one();
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(
            !attack.is_finished(),
            "the attack resolved on the prompt owed to `sigil of {sigil}`              (release {}/3). An instant action bypasses the queue, so its              response is attributed to nothing -- but it still draws a              prompt, and any prompt closed the in-flight command's window.              The attack's real answer would arrive a window late, or under a              strict matcher it would time out with the answer already past.",
            i + 1
        );
    }

    // Now the attack's own prompt.
    transcript.release_one();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let resolved = tokio::time::timeout(Duration::from_secs(1), attack)
        .await
        .expect("the attack must resolve once ITS prompt arrives, not hang")
        .expect("and its task must not panic");
    let Outcome::Confirmed(frame) = resolved else {
        panic!("the attack must resolve on a real response: {resolved:?}")
    };
    assert!(
        matches!(&*frame, cena_protocol::Frame::Text(_)),
        "and on a text frame rather than a terminator: {frame:?}"
    );

    // The wire order is the property underneath: the sigils preceded the
    // attack, so it was the LAST prompt that belonged to it.
    let lines = transcript.lines();
    let at = |needle: &str| lines.iter().position(|l| l == needle);
    assert!(
        at("sigil of focus") < at("attack"),
        "the sigils must precede their trigger, which is what makes their \
         prompts arrive first: {lines:?}"
    );

    cancel.cancel();
    let _ = driver.await;
}

//! **M2 step 6**: the two things that stop an automatic reconnect.
//!
//! The ladder's *timing* is tested where it is a pure function
//! (`supervisor/retry.rs`'s unit tests); this file tests what the supervisor
//! does with it, which is the part a unit test cannot reach:
//!
//! | Stop | Asks |
//! |---|---|
//! | [`Retryability::Fatal`] | *can* this ever work? |
//! | `MAX_UNATTENDED_LOSSES` | *should* it keep trying? |
//!
//! # These tests run on a paused clock
//!
//! `start_paused = true` makes tokio auto-advance time when every task is
//! parked, so a 30-second backoff costs no wall-clock. Without it a test that
//! climbs three rungs would take eight real seconds -- and the **usual failure
//! is the opposite one**: a ladder bug that never sleeps passes a paused-clock
//! test just as fast as a correct one. So the test that cares about the delay
//! asserts on elapsed time explicitly rather than on completion.

use cena_platform::{AnsweringSource, ReplaySource};
use cena_session::{
    CommandId, ConnectError, Connector, Event, Gate, Generation, Origin, Outcome, Sent,
    StoppedBecause, SupervisedSession,
};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// A connection that says one thing and hangs up. `ReplaySource` returns
/// `Ok(0)` when its chunks run out (`replay.rs:78-80`), which is a peer
/// closing -- [`EndReason::PeerClosed`], which warrants a reconnect.
fn a_connection_that_drops() -> Vec<Vec<u8>> {
    vec![b"<prompt time=\"1789775900\">&gt;</prompt>\n".to_vec()]
}

/// Serves prepared connections, then fails however it was told to.
struct LadderConnector {
    sources: VecDeque<Vec<Vec<u8>>>,
    /// What to say once `sources` is empty.
    exhausted: ConnectError,
    /// How many times `connect` was called. The **observable** the ladder tests
    /// assert on: a supervisor that stopped is a supervisor that stopped asking.
    attempts: Arc<AtomicU32>,
}

impl LadderConnector {
    fn new(sources: Vec<Vec<Vec<u8>>>, exhausted: ConnectError) -> (Self, Arc<AtomicU32>) {
        let attempts = Arc::new(AtomicU32::new(0));
        (
            Self {
                sources: sources.into(),
                exhausted,
                attempts: Arc::clone(&attempts),
            },
            attempts,
        )
    }
}

impl Connector for LadderConnector {
    type Source = ReplaySource;

    async fn connect(&mut self, _generation: Generation) -> Result<ReplaySource, ConnectError> {
        self.attempts.fetch_add(1, Ordering::Relaxed);
        self.sources
            .pop_front()
            .map(ReplaySource::new)
            .ok_or_else(|| self.exhausted.clone())
    }
}

/// **A fatal connect error stops the session on the spot.**
///
/// This is the account-lockout guard, and the assertion that matters is the
/// attempt count: `VellumFE`'s comment is that *"hammering the auth server with
/// a wrong password ... could lock the account"*, so "stopped eventually" is
/// not good enough. It must not try twice.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_fatal_connect_error_is_not_retried_even_once() {
    let (connector, attempts) =
        LadderConnector::new(vec![], ConnectError::fatal("auth", "bad password"));
    let (session, _handle) = SupervisedSession::new(connector);

    // **Bounded on purpose.** This was written as a bare `session.run().await`,
    // and disabling the fatal check to falsify it did not fail the test -- it
    // HUNG, because a transient failure retries forever by design and a paused
    // clock makes "forever" arrive instantly. A test whose failure mode is an
    // infinite loop reports nothing to whoever is reading CI.
    //
    // The timeout is enormous relative to the one attempt this should take, so
    // it can only fire when the behaviour is actually wrong.
    let end = tokio::time::timeout(Duration::from_hours(1), Box::pin(session.run()))
        .await
        .expect(
            "a fatal error must STOP the session. Timing out here means it was \
             retried -- which against a live auth server is the account-lockout \
             case this classification exists to prevent.",
        );

    assert_eq!(
        attempts.load(Ordering::Relaxed),
        1,
        "exactly ONE attempt. A second would be a second chance to lock the \
         account, which is the whole reason this classification exists."
    );
    assert_eq!(
        end.stopped_because,
        StoppedBecause::Fatal(ConnectError::fatal("auth", "bad password")),
        "and it carries the error, so a caller can say WHY the player is not \
         logged in rather than just that they are not"
    );
}

/// **A transient connect error is retried.**
///
/// The falsifying pair to the test above: if `Retryability` were ignored and
/// everything stopped, that test would still pass. This one fails unless the
/// two classifications genuinely diverge.
///
/// It terminates by cancelling from outside rather than by exhausting anything
/// -- a transient failure retries forever *by design*, because a client should
/// survive an outage of any length as long as someone is there.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_transient_connect_error_is_retried_until_cancelled() {
    let (connector, attempts) =
        LadderConnector::new(vec![], ConnectError::transient("tcp", "connection refused"));
    let (session, _handle) = SupervisedSession::new(connector);
    let cancel = session.cancel_token();

    let task = tokio::spawn(session.run());
    // Long enough to climb past the first rungs (1s, 2s, 5s) several times
    // over. On a paused clock this is instant.
    tokio::time::sleep(Duration::from_mins(2)).await;
    let climbed = attempts.load(Ordering::Relaxed);
    cancel.cancel();
    let end = task.await.expect("the supervisor task must not panic");

    assert!(
        climbed > 1,
        "a transient failure must be RETRIED -- got {climbed} attempt(s). \
         Without this the fatal test above would pass on a supervisor that \
         simply stopped on every error."
    );
    // **The bound is the LADDER's arithmetic, not a round number.**
    //
    // This used to be `climbed < 120`, which a constant ~1.1s retry passes --
    // the exact login storm the sentence claims to forbid (review SE-8). A
    // bound loose enough to admit the defect is not a bound.
    //
    // `BACKOFF_SECONDS` is `[1, 2, 5, 10, 30]`, capped at the last rung, so
    // the unjittered attempt times over 120 virtual seconds are
    //
    //     t = 0, 1, 3, 8, 18, 48, 78, 108   -> 8 attempts
    //
    // and the ±20% jitter moves the later ones by at most a few seconds,
    // which can add or drop one at the boundary. 6..=10 is that window with a
    // rung's worth of slack either side; anything outside it means the
    // schedule changed, and the ladder's shape is the thing under test.
    assert!(
        (6..=10).contains(&climbed),
        "{climbed} attempts in 120 virtual seconds. The ladder \
         [1, 2, 5, 10, 30] capped at 30s gives 8 (t = 0, 1, 3, 8, 18, 48, 78, \
         108), and jitter moves that by at most one. Far more means it is \
         retrying at a near-constant interval -- the login storm the backoff \
         exists to prevent. Far fewer means it is sleeping longer than the \
         ladder says, and a session comes back later than it should."
    );
    assert_eq!(
        end.stopped_because,
        StoppedBecause::Cancelled,
        "cancelling during a backoff sleep must be noticed -- plan/12 §5.5 \
         gives stopping a 250ms budget, and a 30s sleep that is not raced \
         would blow it by two orders of magnitude"
    );
}

/// **The cap counts connections with no command, and then stops.**
///
/// `MAX_UNATTENDED_LOSSES` is 2, so: connection 1 drops (unattended 1),
/// reconnect, connection 2 drops (unattended 2) -> stop. The connector is
/// handed *three* sources and must only ever be asked for two.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_unattended_session_stops_at_the_cap() {
    let (connector, attempts) = LadderConnector::new(
        vec![
            a_connection_that_drops(),
            a_connection_that_drops(),
            a_connection_that_drops(),
        ],
        ConnectError::transient("tcp", "unreachable"),
    );
    let (session, _handle) = SupervisedSession::new(connector);

    // Bounded for the same reason the fatal test is: a supervisor that ignored
    // the cap would exhaust the three sources and then retry the transient
    // exhaustion error forever, hanging instead of failing.
    let end = tokio::time::timeout(Duration::from_hours(1), Box::pin(session.run()))
        .await
        .expect(
            "the unattended cap must stop the session. Timing out means it kept \
             reconnecting, which is the re-login-all-night case the cap exists \
             to prevent.",
        );

    assert_eq!(
        attempts.load(Ordering::Relaxed),
        2,
        "TWO connections, not three. A third source was prepared precisely so \
         that a supervisor which ignored the cap would consume it and fail \
         this assertion rather than passing by running out of input."
    );
    assert_eq!(
        end.stopped_because,
        StoppedBecause::Unattended,
        "and it says so: the session is re-openable and the player is simply \
         not there, which is not the same as a failure"
    );
}

/// **A command resets the count**, so an attended session survives drops
/// indefinitely.
///
/// This is the other half of the cap, and the half that makes it safe: the
/// guard is against an *abandoned* client, and a session someone is using must
/// never be stopped by it however often the network flaps.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_command_resets_the_unattended_count() {
    // Five connections -- well past the cap of 2. An unattended session would
    // stop after two.
    let sources: Vec<_> = (0..5).map(|_| a_connection_that_drops()).collect();
    let (connector, attempts) =
        LadderConnector::new(sources, ConnectError::transient("tcp", "unreachable"));
    let (session, handle) = SupervisedSession::new(connector);
    let task = tokio::spawn(session.run());

    // Send on every connection. `send_now` does not wait for a verdict, which
    // is what makes this safe against a transport that is about to die --
    // whether the command is CONFIRMED is irrelevant here; whether it was
    // WRITTEN is the whole measurement.
    for _ in 0..5 {
        let _ = handle.send_now("look", Origin::Manual, Gate::None).await;
        tokio::time::sleep(Duration::from_mins(1)).await;
    }

    let end = task.await.expect("the supervisor task must not panic");

    assert!(
        attempts.load(Ordering::Relaxed) > 2,
        "an ATTENDED session must outlive the cap. It only reached {} \
         connection(s), which means a sent command did not reset the count.",
        attempts.load(Ordering::Relaxed)
    );
    assert!(
        end.generations > Generation::FIRST.next(),
        "and it genuinely reconnected more than once"
    );
}

/// **A session that never connects still leaves a log.**
///
/// MEASURED, from a live run with a mistyped character name: both files came
/// out at **zero bytes**. The supervisor had written "connect failed" into a
/// `BufWriter`, and nothing flushed it -- the actor flushes on shutdown, and a
/// login refused at the first attempt never gets an actor.
///
/// The run that most needed a log was the only one that had none, which is the
/// shape of the bug worth a test: the failure path is the one nobody exercises
/// until something has already gone wrong.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_session_that_never_connects_still_writes_its_log() {
    let dir = std::env::temp_dir().join(format!("cena-supervisor-log-{}", std::process::id()));
    let sink = cena_platform::SessionSink::create(
        &dir,
        "testchar",
        "stamp",
        cena_platform::Redactions::new(),
    )
    .expect("the test needs a writable temp dir");
    let events_path = sink.events_path().to_path_buf();

    let (connector, _attempts) =
        LadderConnector::new(vec![], ConnectError::fatal("auth", "no such character"));
    let (session, _handle) = SupervisedSession::new(connector);
    let end = Box::pin(session.with_sink(sink).run()).await;

    assert_eq!(
        end.stopped_because,
        StoppedBecause::Fatal(ConnectError::fatal("auth", "no such character"))
    );

    let written = std::fs::read_to_string(&events_path).expect("the log file must exist");
    assert!(
        written.contains("connect failed"),
        "the reason the session never started must be IN the log, not \
         buffered and dropped. Got:\n{written}"
    );
    assert!(
        written.contains("no such character"),
        "...including the detail, which is the part that says what to fix"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// **A quit issued while the supervisor is retrying must not hang.**
///
/// The failure this guards, from the review: `quit`'s timeout was passed to the
/// actor and started only when an actor received the message. During a retry
/// ladder there is **no actor** -- the supervisor is sleeping between connect
/// attempts and is not reading the inbox -- so both the send and the wait were
/// unbounded. `main` awaits the quit before cancelling, so an ordinary shutdown
/// during a network outage wedged the whole client.
///
/// Bounded here at many times the quit timeout, so it can only fire if the
/// operation is genuinely unbounded.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn quitting_during_a_reconnect_does_not_hang() {
    let (connector, _attempts) =
        LadderConnector::new(vec![], ConnectError::transient("tcp", "connection refused"));
    let (session, handle) = SupervisedSession::new(connector);
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.run());

    // Let it get well into the ladder, where no actor exists.
    tokio::time::sleep(Duration::from_secs(30)).await;

    let farewell = tokio::time::timeout(
        Duration::from_hours(1),
        handle.quit(Duration::from_secs(10)),
    )
    .await
    .expect(
        "quit must return within its own timeout even with no actor to receive \
         it. Hanging here is the client that will not shut down while the \
         network is down.",
    );

    assert_eq!(
        farewell,
        cena_session::Farewell::Unsent,
        "there was no connection to say goodbye on, and `Unsent` is the honest \
         answer: the game was genuinely not told"
    );

    cancel.cancel();
    let _ = task.await;
}

/// **Receiving the login burst is not the same as working, either.**
///
/// The fix below stopped the ladder resetting on an OUTBOUND byte and made it
/// reset on an inbound one instead -- which every connection that gets as far
/// as its login burst has. Two clients fighting over one character each get
/// the burst before the other knocks them off, so the ladder still reset on
/// every drop: VERIFIED before the fix, **20 connects in 20 virtual seconds**
/// (review finding 3).
///
/// Same shape as the test below, but every connection RECEIVES a prompt
/// before it drops.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_connection_that_receives_and_drops_at_once_does_not_reset_the_ladder() {
    let sources: Vec<Vec<Vec<u8>>> = (0..60).map(|_| a_connection_that_drops()).collect();
    let (connector, attempts) =
        LadderConnector::new(sources, ConnectError::transient("tcp", "unreachable"));
    let (session, handle) = SupervisedSession::new(connector);
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.run());
    let sender = tokio::spawn(async move {
        for _ in 0..40 {
            let _ = handle.send_now("look", Origin::Manual, Gate::None).await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    tokio::time::sleep(Duration::from_secs(20)).await;
    let made = attempts.load(Ordering::Relaxed);
    cancel.cancel();
    sender.abort();
    let _ = task.await;

    assert!(
        made < 10,
        "{made} connects in 20 s: a connection that lived for one login burst \
         reset the ladder to its one-second rung"
    );
}

/// Fails `failures` times, then serves one held-open connection, then fails
/// forever.
struct ClimbThenServe {
    failures: u32,
    calls: u32,
    source: Option<AnsweringSource>,
}

impl Connector for ClimbThenServe {
    type Source = AnsweringSource;

    async fn connect(&mut self, _: Generation) -> Result<AnsweringSource, ConnectError> {
        self.calls += 1;
        if self.calls <= self.failures {
            return Err(ConnectError::transient("tcp", "unreachable"));
        }
        self.source
            .take()
            .ok_or_else(|| ConnectError::transient("tcp", "unreachable"))
    }
}

/// Climb the ladder three rungs, hold one working connection open for
/// `lived`, drop it, and report the attempt number of the next retry.
async fn next_attempt_after_a_connection_that_lived(lived: Duration) -> u32 {
    let (source, transcript) =
        AnsweringSource::logged_in(b"You see.\n<prompt time='1'>&gt;</prompt>\n");
    let (session, handle) = SupervisedSession::new(ClimbThenServe {
        failures: 3,
        calls: 0,
        source: Some(source),
    });
    let (_, mut events) = session.subscribe();
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.run());

    while !matches!(
        events.recv().await,
        Ok(Event::StateChanged(cena_session::State::Ready))
    ) {}
    // Something received, and something sent: worked, and attended.
    let looked = handle
        .send_and_await(
            CommandId(1),
            "look",
            Origin::Manual,
            Duration::from_secs(5),
            cena_session::queue::any_frame,
        )
        .await;
    assert!(matches!(looked, Outcome::Confirmed(_)), "{looked:?}");
    tokio::time::sleep(lived).await;
    transcript.hang_up();

    let attempt = loop {
        if let Ok(Event::ConnectFailed { attempt, .. }) = events.recv().await {
            break attempt;
        }
    };
    cancel.cancel();
    let _ = task.await;
    attempt
}

/// **The threshold, from both sides.** A connection that outlived the
/// ladder's top rung resets it; one that did not, does not. Pins
/// `STABLE_CONNECTION` as the boundary rather than only asserting that a
/// short connection does not reset -- which a supervisor that NEVER reset
/// would also pass.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn only_a_connection_that_stayed_up_resets_the_ladder() {
    let stable = cena_session::STABLE_CONNECTION;
    assert_eq!(
        next_attempt_after_a_connection_that_lived(stable).await,
        1,
        "a connection that stayed up {stable:?} must reset the ladder"
    );
    assert_eq!(
        next_attempt_after_a_connection_that_lived(Duration::from_secs(1)).await,
        4,
        "a connection that lasted one second must not: the three failed \
         connects before it are still the ladder's position"
    );
}

/// **A command sent during an outage is answered at once.**
///
/// `plan/12` §5.1: commands during `Reconnecting` *"fail immediately with
/// `Disconnected`"*. They did not: the inbox was swept only after a connect
/// SUCCEEDED, so with the network down a `send_and_await` waited out its own
/// 30 s deadline and came back `Timeout` -- which tells a behavior the command
/// may have run -- and `send_now` waited out its 5 s backstop. VERIFIED before
/// the fix by the review's probe (review finding 6).
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_command_during_an_outage_is_answered_at_once() {
    let (connector, _attempts) = LadderConnector::new(
        vec![a_connection_that_drops()],
        ConnectError::transient("tcp", "unreachable"),
    );
    let (session, handle) = SupervisedSession::new(connector);
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.run());
    // The one connection has come and gone; the supervisor is on its ladder.
    tokio::time::sleep(Duration::from_millis(50)).await;

    let start = tokio::time::Instant::now();
    let outcome = handle
        .send_and_await(
            CommandId(9),
            "look",
            Origin::Manual,
            Duration::from_secs(30),
            cena_session::queue::any_frame,
        )
        .await;
    assert_eq!(outcome, Outcome::Disconnected);
    assert!(
        start.elapsed() < Duration::from_millis(100),
        "answered after {:?}: it waited for the outage, not for nothing",
        start.elapsed()
    );

    let start = tokio::time::Instant::now();
    let sent = handle.send_now("sigil", Origin::Manual, Gate::None).await;
    assert_eq!(sent, Sent::Dead);
    assert!(
        start.elapsed() < Duration::from_millis(100),
        "send_now answered after {:?}",
        start.elapsed()
    );

    cancel.cancel();
    let _ = task.await;
}

/// **Writing to a connection is not the same as it working.**
///
/// Its successor, receiving, was not enough either: see
/// `a_connection_that_receives_and_drops_at_once_does_not_reset_the_ladder`.
///
/// The defect, found by review: the ladder reset on any outbound byte, so with a
/// behavior sending, "attended" was always true and **neither bound bound
/// anything**. Two clients fighting over one character would re-login at the
/// one-second rung forever.
///
/// Here every connection is served a source that yields NOTHING and ends. A
/// command goes out on each, so the session stays attended and never hits the
/// unattended cap -- but nothing is ever received, so the ladder must keep
/// climbing rather than resetting to one second each time.
///
/// Asserted as elapsed time, because the ladder's observable IS the delay: four
/// attempts at the bottom rung cost about 3 seconds, while four that climb cost
/// 1+2+5+10 minus the first.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_connection_that_receives_nothing_does_not_reset_the_ladder() {
    // MANY sources that yield nothing and end immediately: connected, then
    // gone. The count has to exceed what a one-second ladder would consume in
    // the window, or the connector runs out first and both behaviours look
    // identical -- which is exactly what the first version of this test did
    // (6 sources, 5 attempts either way).
    let sources: Vec<Vec<Vec<u8>>> = (0..60).map(|_| Vec::new()).collect();
    let (connector, attempts) =
        LadderConnector::new(sources, ConnectError::transient("tcp", "unreachable"));
    let (session, handle) = SupervisedSession::new(connector);
    let cancel = session.cancel_token();
    let task = tokio::spawn(session.run());

    // Keep it ATTENDED so the unattended cap never fires: something is sent on
    // every connection, which is exactly the case that used to reset the ladder.
    let sender = tokio::spawn(async move {
        for _ in 0..40 {
            let _ = handle
                .send_now("look", Origin::Manual, cena_session::Gate::None)
                .await;
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });

    let start = tokio::time::Instant::now();
    tokio::time::sleep(Duration::from_secs(20)).await;
    let climbed = attempts.load(Ordering::Relaxed);
    let elapsed = start.elapsed();

    cancel.cancel();
    sender.abort();
    let _ = task.await;

    // MEASURED: 5 attempts with the fix, 15 without -- a 3x separation, so 10
    // sits clear of both. Verified by falsification, which the first version of
    // this test could not do: it served only 6 sources, so the connector ran out
    // before the ladder difference could show and both behaviours read as 5.
    assert!(
        climbed < 10,
        "in {elapsed:?} the supervisor made {climbed} attempts. A ladder that \
         reset on every connection would retry at the one-second rung forever; \
         one that climbs makes only a handful in 20 seconds."
    );
}

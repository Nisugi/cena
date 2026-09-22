//! **The server says it is about to idle-kick, and now the supervisor listens.**
//!
//! `MAX_UNATTENDED_LOSSES = 2` guards the abandoned-client case, and its own
//! comment says why it is two rather than one:
//!
//! > *"One would stop the first time a session was quiet across a single drop,
//! > which is an ordinary network blip on an idle character rather than evidence
//! > of an abandoned client."*
//!
//! **That ambiguity is what the warning removes.** A quiet session that dropped
//! might be a blip; a quiet session that the server WARNED about and then dropped
//! is an idle kick, stated by the only party that knows. So an unattended loss
//! carrying a warning stops on the first one, and an unattended loss without one
//! still takes two.
//!
//! The cost of not doing this is not abstract: an idle kick is the disconnect
//! where the connection **worked perfectly**, so `worked` resets the backoff
//! ladder and the supervisor reconnects at the one-second rung. It then plays
//! for ~30 minutes of nothing and is kicked again. Two full cycles before the cap
//! fires is roughly an hour of pointless re-login and auth churn.
//!
//! Wire evidence, and why nothing inbound clears the warning, are in
//! `crates/cena-model/tests/idle_warning.rs`.

use cena_platform::ReplaySource;
use cena_session::{
    ConnectError, Connector, Gate, Generation, Origin, StoppedBecause, SupervisedSession,
};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// The idle warning as the wire sends it, bells and all.
const IDLE_WARNING: &str = "\x07YOU HAVE BEEN IDLE TOO LONG. PLEASE RESPOND.\x07\n";

/// A connection that warns about idling and then hangs up: the idle kick.
///
/// `ReplaySource` returns `Ok(0)` when its chunks run out, which is the peer
/// closing -- so "warn, prompt, end of chunks" is exactly the shape the corpus
/// shows, with the drop the corpus sessions did not happen to take.
fn a_connection_that_warns_then_drops() -> Vec<Vec<u8>> {
    vec![
        b"<prompt time=\"1789775900\">&gt;</prompt>\n".to_vec(),
        IDLE_WARNING.as_bytes().to_vec(),
        b"<prompt time=\"1789775907\">&gt;</prompt>\n".to_vec(),
    ]
}

/// The same connection without the warning: an ordinary quiet drop.
fn a_quiet_connection_that_drops() -> Vec<Vec<u8>> {
    vec![
        b"<prompt time=\"1789775900\">&gt;</prompt>\n".to_vec(),
        b"<prompt time=\"1789775907\">&gt;</prompt>\n".to_vec(),
    ]
}

/// Serves prepared connections, then reports a transient failure forever.
///
/// Transient on exhaustion deliberately: a supervisor that keeps asking will
/// keep getting an answer, so the attempt count measures what the supervisor
/// CHOSE rather than what the connector ran out of. The ladder test learned this
/// the hard way -- a connector with 6 sources exhausted before the behaviour
/// under test could show itself.
struct KickConnector {
    sources: VecDeque<Vec<Vec<u8>>>,
    attempts: Arc<AtomicU32>,
}

impl KickConnector {
    fn new(sources: Vec<Vec<Vec<u8>>>) -> (Self, Arc<AtomicU32>) {
        let attempts = Arc::new(AtomicU32::new(0));
        (
            Self {
                sources: sources.into(),
                attempts: Arc::clone(&attempts),
            },
            attempts,
        )
    }
}

impl Connector for KickConnector {
    type Source = ReplaySource;

    async fn connect(&mut self, _generation: Generation) -> Result<ReplaySource, ConnectError> {
        self.attempts.fetch_add(1, Ordering::Relaxed);
        self.sources
            .pop_front()
            .map(ReplaySource::new)
            .ok_or_else(|| ConnectError::transient("tcp", "connection refused"))
    }
}

/// **A warned, unattended drop stops on the FIRST one.**
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_idle_warning_before_an_unattended_drop_stops_immediately() {
    // Sixty connections available, so exhaustion cannot be what stops it.
    let sources = std::iter::repeat_with(a_connection_that_warns_then_drops)
        .take(60)
        .collect();
    let (connector, attempts) = KickConnector::new(sources);
    let (session, _handle) = SupervisedSession::new(connector);

    let end = tokio::time::timeout(Duration::from_hours(1), Box::pin(session.run()))
        .await
        .expect("a warned idle kick must STOP the session, not retry forever");

    assert_eq!(
        end.stopped_because,
        StoppedBecause::Unattended,
        "the server warned this session was idle and then dropped it; \
         reconnecting is re-entering a loop the server just told us about"
    );
    assert_eq!(
        attempts.load(Ordering::Relaxed),
        1,
        "exactly ONE attempt. The warning is the evidence MAX_UNATTENDED_LOSSES \
         is a proxy for, so it does not need two drops to be sure."
    );
}

/// **An unwarned quiet drop still takes two**, which is what keeps the test
/// above from passing on a supervisor that simply stops on any quiet drop.
///
/// This is the falsifying pair. Without it, "stop on the first unattended loss"
/// would satisfy the test above while breaking the blip case
/// `MAX_UNATTENDED_LOSSES`'s own comment is about.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn an_unwarned_quiet_drop_still_takes_two() {
    let sources = std::iter::repeat_with(a_quiet_connection_that_drops)
        .take(60)
        .collect();
    let (connector, attempts) = KickConnector::new(sources);
    let (session, _handle) = SupervisedSession::new(connector);

    let end = tokio::time::timeout(Duration::from_hours(1), Box::pin(session.run()))
        .await
        .expect("an unattended session must still stop, just not on the first drop");

    assert_eq!(end.stopped_because, StoppedBecause::Unattended);
    assert_eq!(
        attempts.load(Ordering::Relaxed),
        2,
        "TWO attempts without a warning. One would stop on an ordinary network \
         blip against an idle character, which is what MAX_UNATTENDED_LOSSES \
         being 2 rather than 1 exists to allow."
    );
}

/// **A warning does not stop an ATTENDED session.**
///
/// The dangerous direction, and the one this whole change could get wrong. A
/// player who is present, gets warned, answers, and is then dropped by the
/// network must be reconnected -- the warning says the server CONSIDERED them
/// idle, not that they left. Stopping here would strand a live player on a blip
/// that happened to follow a warning.
///
/// # Why the command is queued before `run`
///
/// A `ReplaySource` drains its chunks as fast as the actor reads them, so a
/// connection built from three chunks is gone in three loop turns. A sender task
/// racing that loses: the actor takes ONE inbox message per turn, and the reads
/// win. Queueing first means the command is waiting in the inbox when the actor
/// starts, so it goes out on connection 1 -- which is the connection whose
/// accounting decides.
///
/// Written down because the racing version failed with "stopped after 1 attempt"
/// and looked exactly like a supervisor bug.
#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_warned_but_attended_session_still_reconnects() {
    let sources = std::iter::repeat_with(a_connection_that_warns_then_drops)
        .take(60)
        .collect();
    let (connector, attempts) = KickConnector::new(sources);
    let (session, handle) = SupervisedSession::new(connector);
    let cancel = session.cancel_token();

    // Someone is here, and their command is already waiting. Spawned rather than
    // awaited because the reply resolves only when the game answers or the
    // connection dies, and this test is about the supervisor's decision.
    let sender = tokio::spawn(async move {
        loop {
            let _ = handle.send_now("look", Origin::Manual, Gate::None).await;
            // Paced. An unbounded tight loop STARVES THE RUNTIME on a paused
            // clock -- `send_now` resolves instantly once a connection is gone,
            // so the loop spins without yielding long enough for auto-advance and
            // the test hangs rather than fails. Learned by hanging.
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    tokio::task::yield_now().await;

    let task = tokio::spawn(session.run());
    tokio::time::sleep(Duration::from_mins(2)).await;
    let climbed = attempts.load(Ordering::Relaxed);
    cancel.cancel();
    sender.abort();
    let end = task.await.expect("the supervisor task must not panic");

    assert!(
        climbed > 1,
        "an ATTENDED session was stopped by an idle warning after          {climbed} attempt(s). The warning means the server thought nobody was          there; an outbound command is proof someone is."
    );
    assert_eq!(
        end.stopped_because,
        StoppedBecause::Cancelled,
        "and it ended because it was cancelled, not because it gave up"
    );
}

//! [`SupervisedSession`]: a session that outlives its connections.
//!
//! A [`Session`](crate::Session) is **one connection**. This is the session: it
//! opens connections, runs one actor over each, and decides whether to open
//! another.
//!
//! # Why the names are not swapped
//!
//! `Session` keeps its name and its exact signature. There are 31 call sites,
//! and a session that never reconnects is still the right thing for every one
//! of them -- a replay, a fixture, a behavior test. This is **additive**: the
//! supervisor is a second entry point, not a replacement.
//!
//! # One actor per connection
//!
//! [`SessionActor::run`] consumes the actor, which is what makes "one actor,
//! one connection" structural rather than a convention. **That is preserved**,
//! not worked around: the supervisor builds a *new* actor per generation and
//! carries the durable parts across in [`SessionCore`]. `VellumFE` reconnects the
//! same way -- abort the connection task, build a new one, keep the event
//! stream (`reference/VellumFE/src/frontend/tui/runtime.rs:546-600`).
//!
//! # What it does between connections, in order
//!
//! 1. Publish [`State::Reconnecting`] -- §5.1's "No automation runs" begins
//!    here, enforced by the readiness gate that already existed.
//! 2. [`GameState::invalidate_for_reconnect`] -- what the login burst will not
//!    re-send becomes `Unknown`.
//! 3. Advance the generation -- every handle now stamps the new one, and
//!    anything already in flight is correctly stale.
//! 4. Wait one rung of the backoff ladder ([`backoff`]), racing the cancel
//!    token so a stop is never delayed by a sleep.
//! 5. Ask the connector for a transport.
//!
//! **The order matters.** Invalidating *before* the new connection means no
//! window exists in which a caller could read a stale roundtime against a
//! session that is live again.
//!
//! # What stops it
//!
//! Reconnect is **automatic and bounded** (the author's decision 2), and the
//! bounds are in [`retry`]: a [`Retryability::Fatal`] connect error stops it
//! immediately, and [`MAX_UNATTENDED_LOSSES`] drops with no command sent stop
//! it as an idle session. A transient failure on an *attended* session retries
//! forever, on a ladder that caps at 30 seconds -- see [`Self::run`] for why
//! that is deliberate rather than a missing third bound.

mod connect;
mod core;
mod retry;

pub use connect::{ConnectError, Connector};
pub use core::SessionCore;
pub use retry::{MAX_UNATTENDED_LOSSES, Retryability, backoff};

use crate::actor::{EndReason, Event, SessionActor, Snapshot};
use crate::command::SessionHandle;
use crate::lifecycle::{Generation, GenerationCell, State};
use cena_model::GameState;
use cena_platform::{Recorder, SessionSink};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

/// Inbound command channel bound. Matches [`crate::actor`]'s, for the same
/// reason: bounded everywhere (`plan/12` §5.5).
const COMMAND_CHANNEL_BOUND: usize = 32;

/// Event broadcast ring size. Matches [`crate::actor`]'s.
const EVENT_CHANNEL_BOUND: usize = 256;

/// Why a supervised session stopped reconnecting.
///
/// [`SupervisedEnd::reason`] says why the last *connection* ended; this says why
/// there was not another one. They are genuinely different questions, and the
/// answers cross: a session can end on [`EndReason::PeerClosed`] -- a reason
/// that warrants a reconnect -- and still stop here, because the ladder was
/// stopped by something the connection itself knows nothing about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoppedBecause {
    /// The session was cancelled: the player quit, or a test ended.
    Cancelled,
    /// The connector reported a failure no retry can fix
    /// ([`Retryability::Fatal`]) -- rejected credentials, most likely.
    ///
    /// Carries the error so a caller can show the player *why* they are not
    /// logged in. A bare "stopped" here is the difference between "fix your
    /// password" and a session that mysteriously never came back.
    Fatal(ConnectError),
    /// [`MAX_UNATTENDED_LOSSES`] consecutive drops with no command in between.
    ///
    /// **Not a failure.** The session is re-openable and the player is simply
    /// not there; `VellumFE` surfaces this as *"Session looked idle - not
    /// reconnecting."* rather than as an error.
    Unattended,
}

/// What a supervised session left behind.
#[derive(Debug)]
pub struct SupervisedEnd {
    /// Why there was no further connection.
    pub stopped_because: StoppedBecause,
    /// Everything that crossed the wire, **across every generation**.
    ///
    /// Criterion 9 is "verified in the replay", which needs the reconnect
    /// itself to be in the recording -- so this spans connections rather than
    /// reporting the last one.
    pub recorder: Recorder,
    /// What the session knew when it stopped.
    pub state: GameState,
    /// Why the **last** connection ended: the reason that was not reconnected.
    pub reason: EndReason,
    /// How many connections this session had.
    ///
    /// [`Generation::FIRST`] means one. The counter names connections, so a
    /// session that never reconnected ends where it started.
    pub generations: Generation,
}

/// A session that reconnects.
#[derive(Debug)]
pub struct SupervisedSession<C: Connector> {
    core: SessionCore,
    connector: C,
}

impl<C: Connector> SupervisedSession<C> {
    /// Build a supervised session, and the handle that reaches it.
    ///
    /// **Opens nothing** -- like [`Session::new`](crate::Session::new),
    /// construction touches no network.
    ///
    /// The handle comes back here rather than from an accessor because it must
    /// be obtainable *before* [`Self::run`] consumes the session, and because
    /// one handle is enough: it is `Clone`, and every clone reads the same
    /// generation cell.
    #[must_use]
    pub fn new(connector: C) -> (Self, SessionHandle) {
        let (tx, rx) = mpsc::channel(COMMAND_CHANNEL_BOUND);
        let (events, _) = broadcast::channel(EVENT_CHANNEL_BOUND);
        let generation = GenerationCell::first();
        let handle = SessionHandle::new(tx, generation.clone());
        let session = Self {
            core: SessionCore {
                commands: rx,
                events,
                state: GameState::default(),
                recorder: Recorder::new(),
                sink: None,
                generation,
                cancel: CancellationToken::new(),
            },
            connector,
        };
        (session, handle)
    }

    /// Attach a log sink. It spans every generation, so one file records the
    /// whole session including its reconnects.
    #[must_use]
    pub fn with_sink(mut self, sink: SessionSink) -> Self {
        self.core.sink = Some(sink);
        self
    }

    /// An event stream that survives every reconnect.
    ///
    /// The snapshot comes from the **durable** state, so a subscriber joining
    /// mid-session sees what the session knows rather than what one connection
    /// has learned.
    #[must_use]
    pub fn subscribe(&self) -> (Snapshot, broadcast::Receiver<Event>) {
        (
            Snapshot {
                state: self.core.state.clone(),
                lifecycle: State::Connecting,
                generation: self.core.generation.get(),
            },
            self.core.events.subscribe(),
        )
    }

    /// Stop the **session**, including any reconnect it would have attempted.
    ///
    /// Distinct from an actor's own cancel, which ends one connection. This is
    /// the one a caller wants: it makes the next end [`EndReason::Cancelled`],
    /// the only reason that does not reconnect.
    #[must_use]
    pub fn cancel_token(&self) -> CancellationToken {
        self.core.cancel.clone()
    }

    /// The shared generation counter, for tests and for a caller that wants to
    /// know which connection it is on.
    #[must_use]
    pub fn generation_cell(&self) -> GenerationCell {
        self.core.generation.clone()
    }

    /// Run until nothing warrants another connection.
    ///
    /// # The three ways this returns
    ///
    /// 1. **Cancelled** -- the deliberate stop, and the only one that is not a
    ///    failure of any kind.
    /// 2. **Fatal** -- the connector said no retry can work.
    /// 3. **Unattended** -- [`MAX_UNATTENDED_LOSSES`] drops with no command in
    ///    between.
    ///
    /// There is deliberately **no fourth**: a transient failure retries
    /// forever, on a ladder that caps at 30 seconds. That is the right
    /// behaviour for a network that is down -- a client that gave up after N
    /// attempts would need the player to notice and act, and the whole point of
    /// bounding by *attendance* rather than by attempt count is that an
    /// attended session should survive an outage of any length.
    pub async fn run(mut self) -> SupervisedEnd {
        // Cancelled is the honest default: a session that never connects at
        // all did not lose a transport.
        let mut reason = EndReason::Cancelled;
        let stopped_because;
        // Which rung of the ladder. Reset by a connection that actually
        // carried traffic -- see `earned_a_reset` for why "connected" is not
        // the same thing.
        let mut attempt = 0u32;
        // Consecutive losses with no command sent. `MAX_UNATTENDED_LOSSES` of
        // these stops the session.
        let mut unattended = 0u32;
        loop {
            let generation = self.core.generation.get();
            // **Raced against the cancel token.** Only the backoff sleep was,
            // so `stop` during a reconnect ran the whole login to completion:
            // the character was logged IN, the new actor then saw the cancel on
            // its first turn and dropped the socket without a `quit`, and the
            // character was left link-dead in-world. Found by review.
            //
            // A cancel here abandons the attempt rather than the result: if the
            // login has already completed, the source is closed on the way out
            // rather than leaked.
            let connected = tokio::select! {
                () = self.core.cancel.cancelled() => {
                    self.log("cancelled while connecting");
                    stopped_because = StoppedBecause::Cancelled;
                    reason = EndReason::Cancelled;
                    break;
                }
                connected = self.connector.connect(generation) => connected,
            };
            // BEFORE anything is logged about this connection, and before a
            // single byte of it is written: a secret minted by the connect --
            // this generation's launch key -- would otherwise reach the
            // `.bytes` file in the clear, because the sink's redaction set was
            // built when the log was opened and that key did not exist yet.
            //
            // Taken on the failure path too. A refused login still names the
            // stage it failed at, and the detail it carries is about to go
            // through `self.log`.
            for secret in self.connector.take_secrets() {
                if let Some(sink) = self.core.sink.as_mut() {
                    sink.redact_key(&secret);
                }
            }
            let source = match connected {
                Ok(source) => source,
                Err(error) => {
                    self.log(&format!("connect failed: {error}"));
                    if !error.retryability.may_retry() {
                        // The connector says no retry can work. Stop NOW --
                        // not after the ladder, not after one more try. The
                        // failure this prevents is Vellum's: "hammering the
                        // auth server with a wrong password ... could lock the
                        // account."
                        stopped_because = StoppedBecause::Fatal(error);
                        break;
                    }
                    // A failed *connect* climbs the ladder too. Otherwise a
                    // server refusing connections would be retried at one
                    // second forever, which is the storm the ladder exists to
                    // prevent.
                    if !self
                        .wait_before_retry(&mut attempt, &error.to_string())
                        .await
                    {
                        stopped_because = StoppedBecause::Cancelled;
                        break;
                    }
                    continue;
                }
            };

            // Everything already recorded, so what THIS connection sends can
            // be told from what earlier ones did. The recorder is durable
            // across generations, which is what makes this a subtraction
            // rather than a flag the actor has to carry.
            let sent_before = outbound_count(&self.core.recorder);

            let actor = SessionActor::supervised(
                source,
                std::mem::take(&mut self.core.state),
                self.core.commands,
                self.core.events.clone(),
                std::mem::take(&mut self.core.recorder),
                self.core.sink.take(),
                // A CHILD token: cancelling the session cancels the running
                // connection, but a connection ending does not cancel the
                // session.
                self.core.cancel.child_token(),
                generation,
            );
            let end = actor.run().await;

            // Take the durable parts back BEFORE deciding anything. They must
            // return even on the path that stops, or `SupervisedEnd` would be
            // missing the recording criterion 9 replays.
            self.core.commands = end.commands;
            self.core.recorder = end.recorder;
            self.core.sink = end.sink;
            self.core.state = end.state;
            reason = end.reason;

            if !reason.warrants_reconnect() {
                stopped_because = StoppedBecause::Cancelled;
                break;
            }
            // A cancelled SESSION does not reconnect, even when the connection
            // ended some other way. Cancelling the parent cancels the child, so
            // the actor usually reports `Cancelled` itself -- but a transport
            // that died in the same turn could report a loss instead, and
            // reconnecting after the player quit is the failure this prevents.
            if self.core.cancel.is_cancelled() {
                reason = EndReason::Cancelled;
                stopped_because = StoppedBecause::Cancelled;
                break;
            }

            // Did anyone use this connection? A command sent is a player (or a
            // behavior) present; none across MAX_UNATTENDED_LOSSES connections
            // is an abandoned client being idle-kicked in a loop.
            let attended = outbound_count(&self.core.recorder) > sent_before;
            if attended {
                unattended = 0;
                // An attended connection resets the LADDER too, so a session
                // that ran for an hour and then dropped retries at one second
                // rather than at whatever rung it climbed to last week.
                attempt = 0;
            } else {
                unattended += 1;
                if unattended >= MAX_UNATTENDED_LOSSES {
                    self.log(&format!(
                        "no command sent across {unattended} connections; not reconnecting"
                    ));
                    stopped_because = StoppedBecause::Unattended;
                    break;
                }
            }

            self.reconnect();
            // The detail is the END REASON here, not a connect error: nothing
            // failed to connect, a live connection was lost.
            let lost = format!("{reason:?}");
            if !self.wait_before_retry(&mut attempt, &lost).await {
                stopped_because = StoppedBecause::Cancelled;
                reason = EndReason::Cancelled;
                break;
            }
        }

        // **Flush, on every exit path.** The actor flushes when it shuts down,
        // but a session that never got an actor -- a login refused at the first
        // attempt -- would otherwise drop its buffered lines unwritten and
        // leave a ZERO-BYTE log behind.
        //
        // MEASURED: a run with a wrong character name produced
        // `nisugi-...-000.bytes` and `nisugi-....log` at 0 bytes each, with the
        // "connect failed" line sitting in a `BufWriter` that was dropped. The
        // one run that most needed a log was the one that had none.
        self.log(&format!("session stopped: {stopped_because:?}"));
        if let Some(sink) = self.core.sink.as_mut() {
            let _ = sink.flush();
        }

        SupervisedEnd {
            stopped_because,
            recorder: self.core.recorder,
            state: self.core.state,
            reason,
            generations: self.core.generation.get(),
        }
    }

    /// Sleep one rung of the ladder, advancing `attempt`.
    ///
    /// Returns `false` if the session was cancelled while waiting, which is the
    /// half of this that matters: a 30-second backoff would otherwise make
    /// `cancel()` take up to 30 seconds to be noticed, and `plan/12` §5.5 gives
    /// stopping a 250ms budget. The wait races the cancel token rather than
    /// checking it afterwards.
    async fn wait_before_retry(&mut self, attempt: &mut u32, detail: &str) -> bool {
        let delay = backoff(*attempt, jitter());
        *attempt = attempt.saturating_add(1);
        self.log(&format!(
            "reconnecting in {}ms (attempt {})",
            delay.as_millis(),
            attempt
        ));
        // ...and to anyone watching, not only to the log file.
        let _ = self.core.events.send(Event::ConnectFailed {
            attempt: *attempt,
            delay,
            detail: detail.to_owned(),
        });
        tokio::select! {
            () = self.core.cancel.cancelled() => false,
            () = tokio::time::sleep(delay) => true,
        }
    }

    /// Between one connection and the next. The order is the contract; see this
    /// module's header.
    fn reconnect(&mut self) {
        // 1. Announce it. From here until the next `Ready`, §5.1's "No
        //    automation runs" holds -- through the readiness gate that already
        //    existed, not a new check.
        let _ = self
            .core
            .events
            .send(Event::StateChanged(State::Reconnecting));
        self.log("lifecycle Reconnecting");

        // 2. Forget what the next login will not re-send, BEFORE the new
        //    connection exists.
        self.core.state.invalidate_for_reconnect();

        // 3. Advance. Every handle now stamps the new generation; anything
        //    already in flight was stamped before this and stays stale.
        self.core.generation.advance();
    }

    /// Write one line to the session log, if there is one.
    ///
    /// Swallows the error for the same reason the actor's does: a full disk or
    /// a revoked permission must not end a session.
    fn log(&mut self, line: &str) {
        if let Some(sink) = self.core.sink.as_mut() {
            let _ = sink.event(line);
        }
    }
}

/// How many commands this session has sent, across every generation.
///
/// **Attendance is measured, not flagged.** The alternative was a
/// `saw_input_since_connect` boolean on the actor, as `VellumFE` carries
/// (`runtime.rs:490`) -- but Cena already records every outbound write, and the
/// recorder is durable across generations precisely so facts like this survive
/// a reconnect. A second source of truth for "did anyone type anything" could
/// disagree with the recording; a subtraction cannot.
fn outbound_count(recorder: &Recorder) -> usize {
    recorder
        .events()
        .iter()
        .filter(|event| matches!(event, cena_platform::RecordedEvent::Outbound { .. }))
        .count()
}

/// A jitter fraction in `0.0..=1.0`.
///
/// # Why this is not `rand`
///
/// It needs one byte of spread per reconnect, and `plan/05` Rule -1 does not
/// support a dependency for that. `VellumFE` reaches for `getrandom` because it
/// already depends on it (`runtime.rs:76`); `cena-session` does not, and adding
/// a crate to a session actor to decorrelate a backoff is the wrong trade.
///
/// # Why this does not break replay determinism
///
/// Criterion 7 requires a replay to produce the same result every run, and this
/// reads a clock. It is safe because **a replay never reaches it**: a
/// [`ReplaySource`](cena_platform::ReplaySource) is handed over by a connector
/// that has already decided what to serve, and the ladder only runs between
/// connections that a test controls. The one test that *does* exercise the
/// ladder asserts on [`backoff`] directly, which is a pure function taking the
/// jitter as a parameter -- that split is why the randomness can live here
/// without being untestable.
fn jitter() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Nanoseconds since the epoch, low bits only. Not cryptographic and not
    // trying to be: the requirement is that five characters dropped by one
    // network blip do not re-login in the same millisecond, and their
    // supervisors reach this line at genuinely different nanoseconds.
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    f64::from(nanos % 1000) / 999.0
}

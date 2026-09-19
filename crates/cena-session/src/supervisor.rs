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
//! 4. Ask the connector for a transport.
//!
//! **The order matters.** Invalidating *before* the new connection means no
//! window exists in which a caller could read a stale roundtime against a
//! session that is live again.

mod connect;
mod core;

pub use connect::{ConnectError, Connector};
pub use core::SessionCore;

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

/// What a supervised session left behind.
#[derive(Debug)]
pub struct SupervisedEnd {
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

    /// Run until a reason says not to reconnect, or until cancelled.
    pub async fn run(mut self) -> SupervisedEnd {
        // Cancelled is the honest default: a session that never connects at
        // all did not lose a transport.
        let mut reason = EndReason::Cancelled;
        loop {
            let generation = self.core.generation.get();
            let source = match self.connector.connect(generation).await {
                Ok(source) => source,
                Err(error) => {
                    // A connector that cannot produce a transport ends the
                    // session. The retry ladder is a LATER step; until it
                    // exists, stopping loudly beats looping silently.
                    self.log(&format!("connect failed: {error}"));
                    break;
                }
            };

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
                break;
            }
            // A cancelled SESSION does not reconnect, even when the connection
            // ended some other way. Cancelling the parent cancels the child, so
            // the actor usually reports `Cancelled` itself -- but a transport
            // that died in the same turn could report a loss instead, and
            // reconnecting after the player quit is the failure this prevents.
            if self.core.cancel.is_cancelled() {
                reason = EndReason::Cancelled;
                break;
            }

            self.reconnect();
        }

        SupervisedEnd {
            recorder: self.core.recorder,
            state: self.core.state,
            reason,
            generations: self.core.generation.get(),
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

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
//! 1. [`GameState::invalidate_for_reconnect`] -- what the login burst will not
//!    re-send becomes `Unknown`.
//! 2. Advance the generation -- every handle now stamps the new one, and
//!    anything already in flight is correctly stale.
//! 3. Publish [`State::Reconnecting`] with that generation. These three steps
//!    happen in one synchronous turn, before observation requests are served.
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
//! bounds are in `retry`: a [`Retryability::Fatal`] connect error stops it
//! immediately, and [`MAX_UNATTENDED_LOSSES`] drops with no command sent stop
//! it as an idle session. A transient failure on an *attended* session retries
//! forever, on a ladder that caps at 30 seconds -- see `Supervisor::run` for why
//! that is deliberate rather than a missing third bound.

mod attach;
mod connect;
mod construct;
mod core;
mod observation;
mod retry;

pub use connect::{ConnectError, Connector};
pub use core::SessionCore;
pub use retry::{MAX_UNATTENDED_LOSSES, Retryability, STABLE_CONNECTION, backoff};
// Not re-exported: the jitter source is an implementation detail of the
// ladder, and `backoff` takes the fraction as a parameter precisely so
// callers never need it.
use retry::jitter;

use crate::actor::{EndReason, Event, SessionActor, Snapshot};
use crate::lifecycle::{Generation, GenerationCell, State};
use cena_model::GameState;
use cena_platform::Recorder;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// What one connection did, for [`SupervisedSession::after_connection`].
///
/// A struct because the function took five positional counters, and two
/// adjacent `u64`s are where the wrong one gets passed.
#[derive(Clone, Copy)]
struct Connection {
    /// The recorder's outbound count when the connection opened.
    sent_before: u64,
    /// Its inbound count at the same moment.
    received_before: u64,
    /// How long the connection's actor ran.
    lived: std::time::Duration,
}

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
    /// Which session ended.
    ///
    /// Carried here because `run` consumes the session, so this is the only
    /// way to correlate an ending with the session that produced it — which a
    /// frontend holding several must do. It is also what lets a test assert
    /// the id survived the reconnects the same value counts.
    pub session: crate::lifecycle::SessionId,
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
    /// Read-only access obtained before `run` consumes the owner, supporting
    /// fresh subscriptions during the actor, connect attempts and backoff.
    #[must_use]
    pub fn observer(&self) -> crate::SessionObserver {
        self.core.observations.observer()
    }

    /// An event stream taken before running. For mid-session attachment use
    /// `observer().subscribe()` on a previously obtained detached observer.
    #[must_use]
    pub fn subscribe(&self) -> (Snapshot, broadcast::Receiver<Event>) {
        (
            self.core
                .events
                .snapshot(&self.core.state, self.core.lifecycle),
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
        // Which rung of the ladder. Reset only by a connection that RECEIVED
        // something -- see `worked` below for why "we wrote to it" is not the
        // same as "it functioned".
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
            let Some(connected) = self.connect_observed(generation).await else {
                self.log("cancelled while connecting");
                stopped_because = StoppedBecause::Cancelled;
                reason = EndReason::Cancelled;
                break;
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
            let sent_before = self.core.recorder.outbound_count();
            let received_before = self.core.recorder.inbound_count();

            // **Before the new actor sees the channel.** See `sweep_inbox`.
            self.sweep_inbox();

            // The name arrives with the `<app>` that triggers the load, and the
            // state is carried across connections: a known name means an
            // earlier connection already read the store.
            let stored_facts_read = self.core.state.character.name.is_some();
            let mut actor = SessionActor::supervised(
                source,
                std::mem::take(&mut self.core.state),
                self.core.commands,
                self.core.events.clone(),
                self.core.observations,
                std::mem::take(&mut self.core.recorder),
                self.core.sink.take(),
                self.core.combat.clone(),
                // A CHILD token: cancelling the session cancels the running
                // connection, but a connection ending does not cancel the
                // session.
                self.core.cancel.child_token(),
                generation,
            );
            actor.menu_dir.clone_from(&self.core.menu_dir);
            actor.persistence.dir.clone_from(&self.core.character_dir);
            // **Read the stored facts once per SESSION, not once per
            // connection.** `<app>` is re-sent on every reconnect, and
            // `load_character` refuses a second load because it would put the
            // disk's older values over what the session has learned since --
            // but it remembers that on the actor, and this is a new actor.
            actor.persistence.loaded = stored_facts_read;
            actor.player_log = self
                .core
                .player_log
                .get()
                .map(|tap| crate::player_log::Feed::new(tap.clone(), generation));
            // One allocation per connection keeps the supervisor future from
            // embedding the actor's large parser/model/select-loop storage.
            // Basis: observation::actor_future_storage_justifies_heap_pinning
            // measures the future against its boxed handle (9,520 vs 8 bytes
            // on x86_64 Linux, Rust 1.96.1, test transport, 2026-09-21).
            // This is storage evidence, not a runtime performance guarantee.
            // This remains the same task and ownership handoff, not a spawn.
            let connected_at = tokio::time::Instant::now();
            let end = Box::pin(actor.run()).await;

            // Take the durable parts back BEFORE deciding anything. They must
            // return even on the path that stops, or `SupervisedEnd` would be
            // missing the recording criterion 9 replays.
            self.core.commands = end.commands;
            self.core.recorder = end.recorder;
            self.core.sink = end.sink;
            self.core.state = end.state;
            self.core.lifecycle = end.lifecycle;
            self.core.observations = end.observations;
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
            if let Some(stop) = self.after_connection(
                Connection {
                    sent_before,
                    received_before,
                    lived: connected_at.elapsed(),
                },
                &mut attempt,
                &mut unattended,
            ) {
                stopped_because = stop;
                break;
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

        self.finish(reason, stopped_because)
    }

    /// Publish the terminal snapshot and flush on every supervisor exit path.
    fn finish(mut self, reason: EndReason, stopped_because: StoppedBecause) -> SupervisedEnd {
        // **Flush, on every exit path.** The actor flushes when it shuts down,
        // but a session that never got an actor -- a login refused at the first
        // attempt -- would otherwise drop its buffered lines unwritten and
        // leave a ZERO-BYTE log behind.
        //
        // MEASURED: a run with a wrong character name produced
        // `nisugi-...-000.bytes` and `nisugi-....log` at 0 bytes each, with the
        // "connect failed" line sitting in a `BufWriter` that was dropped. The
        // one run that most needed a log was the one that had none.
        if self.core.lifecycle != State::Closed {
            self.core.lifecycle = State::Closed;
            let _ = self.core.events.send(Event::StateChanged(State::Closed));
        }
        self.core.observations.finish(
            self.core
                .events
                .snapshot(&self.core.state, self.core.lifecycle),
        );
        self.log(&format!("session stopped: {stopped_because:?}"));
        if let Some(sink) = self.core.sink.as_mut() {
            let _ = sink.flush();
        }

        SupervisedEnd {
            session: self.core.id,
            stopped_because,
            recorder: self.core.recorder,
            state: self.core.state,
            reason,
            generations: self.core.generation.get(),
        }
    }

    /// Discard everything parked since the last connection ended.
    ///
    /// Anything swept counts as attendance -- a swept command never reaches
    /// the recorder, but somebody typed it -- and
    /// [`SessionCore::refuse_while_disconnected`] records that for
    /// [`Self::after_connection`]. [`SessionCore::discard_stale_inbox`] has
    /// the full reasoning and the transcript of the defect (review finding
    /// SE-1).
    ///
    /// Since review finding 6 the connect and backoff waits answer the inbox
    /// as messages arrive, so this catches only what landed in the instant
    /// between the connector returning and the actor starting.
    fn sweep_inbox(&mut self) {
        let swept = self.core.discard_stale_inbox();
        if swept > 0 {
            self.log(&format!(
                "discarded {swept} message(s) queued while disconnected"
            ));
        }
    }

    /// Update the ladder and the unattended cap from what this connection did.
    ///
    /// Returns `Some` if the session should stop. Split from [`Self::run`] under
    /// Rule 4.1 -- move code down -- when the attended/worked split pushed that
    /// function past clippy's 100-line limit. The seam is real: `run` owns the
    /// loop, this owns one connection's accounting.
    fn after_connection(
        &mut self,
        connection: Connection,
        attempt: &mut u32,
        unattended: &mut u32,
    ) -> Option<StoppedBecause> {
        let Connection {
            sent_before,
            received_before,
            lived,
        } = connection;
        let attended_while_disconnected =
            std::mem::take(&mut self.core.attended_while_disconnected);
        // **Two different questions, and they were conflated.**
        //
        // ATTENDED asks "is anyone using this session" and bounds the
        // unattended cap. A command sent is a person or a behavior present.
        //
        // WORKED asks "did this connection function" and is the only thing
        // that may reset the ladder. It used to be the same test, which made
        // the ladder reset on ANY outbound byte -- so with a behavior
        // sending, it was always true and **neither bound bound anything**.
        // Two clients fighting over one character would re-login at the
        // one-second rung forever. Found by review, which also noticed the
        // comment cited an `earned_a_reset` that does not exist.
        //
        // A connection that received nothing did not work, however much we
        // wrote at it: an accepted socket that dies before the login burst
        // is exactly the flapping case the ladder is for.
        //
        // **And receiving is not enough either** (review finding 3). That
        // fix stopped the ladder resetting on an outbound byte, and then
        // reset it on an INBOUND one -- which a login burst always is. Two
        // clients fighting over one character each get the burst before the
        // other knocks them off, so the ladder still reset every time: the
        // review's probe made 20 connects in 20 virtual seconds.
        //
        // So a connection has worked when it received something AND stayed
        // up for `STABLE_CONNECTION`, the ladder's top rung. That bound is not
        // tuned, it is the one that makes the reset safe: a connection that
        // lives at least as long as the longest wait cannot, by resetting,
        // produce reconnects faster than the capped ladder already allows.
        // `VellumFE` resets on the character's identity being confirmed
        // (`reference/VellumFE/src/frontend/headless/runtime.rs:948`), which
        // the burst also carries, so it has the same hole.
        //
        // `|| attended_while_disconnected`: a command the sweep discarded
        // never reached the recorder, but somebody typed it.
        let attended =
            self.core.recorder.outbound_count() > sent_before || attended_while_disconnected;
        let worked =
            self.core.recorder.inbound_count() > received_before && lived >= STABLE_CONNECTION;
        if worked {
            *attempt = 0;
        }
        if attended {
            *unattended = 0;
            return None;
        }
        *unattended += 1;

        // **The server told us, so one is enough.**
        //
        // `MAX_UNATTENDED_LOSSES` is 2 rather than 1 for a stated reason: *"one
        // would stop the first time a session was quiet across a single drop,
        // which is an ordinary network blip on an idle character rather than
        // evidence of an abandoned client."* The idle warning is that evidence,
        // from the only party that has it -- so the ambiguity the cap pads
        // against is gone and the padding is not needed.
        //
        // Why it matters that this is fast: an idle kick is the disconnect where
        // the connection WORKED, so `worked` above has already reset the ladder.
        // Without this the supervisor reconnects at the one-second rung, idles
        // ~30 minutes, and is kicked again -- roughly an hour of auth churn
        // before the cap fires.
        //
        // `attended` is checked FIRST, and that order is the safety property: a
        // player who was warned, answered, and then lost their network sent an
        // outbound byte, so they never reach here. The warning says the server
        // thought nobody was there; a command is proof someone was.
        let allowed = if self.core.state.idle_warned() {
            self.log("the server warned this session was idle before it dropped");
            1
        } else {
            MAX_UNATTENDED_LOSSES
        };
        if *unattended >= allowed {
            self.log(&format!(
                "no command sent across {unattended} connection(s); not reconnecting"
            ));
            return Some(StoppedBecause::Unattended);
        }
        None
    }

    /// Sleep one rung of the ladder, advancing `attempt`.
    ///
    /// Returns `false` if the session was cancelled while waiting, which is the
    /// half of this that matters: a 30-second backoff would otherwise make
    /// `cancel()` take up to 30 seconds to be noticed, and `plan/12` §5.5 gives
    /// stopping a 250ms budget. The wait races the cancel token rather than
    /// checking it afterwards.
    async fn wait_before_retry(&mut self, attempt: &mut u32, detail: &str) -> bool {
        // An initial failed login also enters the retry ladder, although it
        // has not established a connection whose generation must advance.
        if self.core.lifecycle != State::Reconnecting {
            self.core.lifecycle = State::Reconnecting;
            let _ = self
                .core
                .events
                .send(Event::StateChanged(State::Reconnecting));
        }
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
        self.wait_observed(delay).await
    }

    /// Between one connection and the next. The order is the contract; see this
    /// module's header.
    fn reconnect(&mut self) {
        // No await in this transition: invalidate and advance before publishing
        // so the reconnect event and every snapshot name the new generation.
        self.core.state.invalidate_for_reconnect();
        self.core.generation.advance();
        self.core.lifecycle = State::Reconnecting;
        let _ = self
            .core
            .events
            .send(Event::StateChanged(State::Reconnecting));
        self.log("lifecycle Reconnecting");
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

// `outbound_count` moved onto `Recorder` itself. It used to live here as a
// `filter().count()` over the whole log -- O(session length), run twice per
// connection -- and a bounded recorder made it outright WRONG, because the log
// now forgets early writes and the count would go DOWN. A lifetime counter on
// the recorder is both correct and O(1).
//
// Attendance is still MEASURED rather than flagged, which was the point: the
// recorder is the single source of truth for "did anyone send anything", and a
// separate boolean could disagree with the recording.

//! The session actor: one task, one select loop, one socket.
//!
//! `plan/12` §5.5 requires "one supervised task per session", "bounded
//! channels everywhere", "every wait has a deadline" and "a panic kills one
//! session, not the process".
//!
//! This module is the first three. **The fourth is the caller's**, and this
//! header used to claim it outright (review SE-10).
//!
//! # Panic isolation, and where it actually comes from
//!
//! There is no `catch_unwind` here (`grep -rn catch_unwind crates/`: 0 hits).
//! What isolates a panic is `tokio::spawn`: a panicking task is caught by the
//! runtime and surfaces as a `JoinError`, and the process survives.
//!
//! So isolation holds exactly when the caller spawns. `crates/cena/src/main.rs`
//! does. **`SupervisedSession::run` does not** -- it awaits `actor.run()`
//! inline, so an actor panic unwinds *through* the supervisor: no reconnect,
//! no `Closed` event, and the supervisor's own task dies with it.
//!
//! Spawning the actor there is not a one-line fix. The actor owns the command
//! receiver, the recorder and the sink, and hands them back through
//! `SessionEnd` so they survive a generation; a panicking task drops them, so
//! a reconnect would lose the recording criterion 9 replays. Spawning also
//! requires `S: Send + 'static`, which constrains every `ByteSource`.
//!
//! Recorded rather than half-built: no actor panic has been observed, and the
//! outer `tokio::spawn` in `main` still keeps one session's panic from taking
//! the process. What is NOT true today is that such a panic is survivable by
//! the session -- it ends the supervisor too.
//!
//! # The session owns the socket, and one actor is one connection
//!
//! The actor holds its [`ByteSource`] by value and closes it itself, which is
//! how criterion 6 ("no leaked sockets") is met without a drop guard nobody can
//! test. [`SessionActor::run`] takes `self` **by value**, so "one actor, one
//! connection" is structural rather than a convention.
//!
//! **That is what Milestone 2 builds on rather than changes.** An earlier
//! version of this header said there was no connection-manager layer because
//! "reconnect is Milestone 2, so a manager today would be a trait with one
//! implementor". Reconnect is now being built, and the resolution keeps `run`
//! consuming: a supervisor runs **a new actor per connection**, and what
//! survives between them ([`SessionEnd`]) is handed back rather than reused in
//! place.
//!
//! # `biased;` is not a style choice
//!
//! `tokio::select!` without it picks among ready arms **pseudo-randomly**,
//! which would be the single largest source of flake in criterion 7's
//! deterministic replay: on a run where both a queued command and a pending
//! read chunk were ready, the order would differ. `biased;` makes the order a
//! property of the source. It also gives cancellation its priority, which is
//! what criterion 4's `PREEMPT_GRACE` is measured against.
//!
//! # The split this file declared in advance, and then took
//!
//! Frame-to-state folding is in `cena_model::state` and the queue is in
//! [`crate::queue`]. Following the precedent in
//! `crates/cena-arch-tests/tests/ratchet.rs:4-8`, this header named its next
//! split before it was needed: "if this file grows, `run_command` and `ingest`
//! move to `actor/io.rs` and the loop stays."
//!
//! It grew -- to 424 lines against the 400 default, when `plan/12` §5.3's
//! readiness gate landed -- and **the split was taken as written** rather than
//! the cap being raised (`plan/05:352-353`). `pump` and `ingest` are now in
//! `io`; what is here is the loop, the session's shape, and the gate.
//!
//! That split has since been taken too: [`Session`] and [`Snapshot`] are in
//! `handle`.
//!
//! And a third: [`EndReason`] is in `ending`, moved there when Milestone 2's
//! end-reason took this file to 402 against the cap. That seam had been named
//! one commit earlier, which is the practice
//! `crates/cena-arch-tests/tests/ratchet.rs:4-8` set -- **the fourth time it
//! has paid.**
//!
//! **That split has been taken too**, one step later: `shutdown` and
//! `transition` joined [`EndReason`] in `ending` when the supervisor took
//! this file to 437. Named in advance, taken as written -- the fifth time.
//!
//! The logging helpers went with them and **came back**. They were moved to
//! shed the last five lines over the cap, which is arithmetic rather than a
//! seam: `ingest` and `pump` use them on every turn, so they belong with the
//! loop. When the author raised the default cap from 400 to 800 they returned.
//!
//! **The next split, named in advance and not yet needed:** [`SessionEnd`] and
//! `SessionActor::supervised` to `actor/parts.rs` -- what a connection is
//! handed and what it hands back -- leaving `run` and the select loop alone.

use crate::command::Envelope;
use crate::lifecycle::{Generation, State};
use crate::queue::CommandQueue;
use cena_model::GameState;
use cena_platform::{ByteSource, Recorder, SessionSink};
use cena_protocol::{Frame, Parser};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

mod ending;
mod handle;
mod io;

pub use ending::EndReason;
pub use handle::{Session, Snapshot};

/// Inbound command channel bound.
///
/// `plan/12` §5.5: bounded channels everywhere. 32 is a starting value, not a
/// measured one -- `plan/12` §10 defers buffer sizes to "measurement under
/// real load". What matters structurally is that it is *bounded*: a full
/// queue refuses (`Outcome::Refused(Transient)`) rather than blocking the
/// caller, which is what stops a slow session wedging a frontend.
const COMMAND_CHANNEL_BOUND: usize = 32;

/// Event broadcast ring size.
///
/// `plan/12` §6.3: "Bounded ring buffer per subscriber
/// (`tokio::sync::broadcast` semantics). On overflow the subscriber receives
/// an explicit `Lagged { missed }`, never a silent gap." That is what
/// `broadcast` does, so §6.3 costs one constant rather than a mechanism.
///
/// # Why 2,048, and why raising it is not the whole answer
///
/// `plan/18` §6 declined to simply raise this, asking instead *"whether a
/// subscriber that needs EVERY frame should be a broadcast subscriber at all, or
/// whether the model is the only thing that must not miss frames."*
///
/// **The model already is.** `SessionActor::ingest` calls `state.apply(&frame)`
/// and *then* `events.send(...)`, on the same thread, so `GameState` cannot lag
/// however small this is. The ring is for OBSERVERS, and for an observer
/// `Lagged` is the honest answer rather than a failure.
///
/// So what remained was sizing. MEASURED, frames before the first `<prompt>` in
/// two of the author's captures: **1,151** (`GSIV-Nisugi/2025-04-18`) and **794**
/// (`GSIV-Monstr/2025-09-04`). Against 256 -- so the burst ran 3-4.5x the ring,
/// and the `!! 99 events dropped` the author saw was the tail of a much larger
/// overflow. 2,048 covers the larger burst with ~78% headroom.
///
/// It is a size, not a promise: a slow enough subscriber still lags, and
/// `crates/cena-session/tests/event_ring.rs` asserts that it is still told.
const EVENT_CHANNEL_BOUND: usize = 2048;

/// How long one read may block before the loop takes a turn anyway.
///
/// `plan/12` §5.5: "every wait has a deadline; no unbounded `await`". The
/// deadline is not a failure -- a quiet game is normal.
///
/// # What it does NOT buy
///
/// This used to say the deadline "is what lets the loop notice a queued
/// command while nothing is arriving". That is false: `self.commands.recv()`
/// is already an arm of the same `select!`, so a queued command wakes the loop
/// the moment it is sent, deadline or no. Believing otherwise would justify
/// *lowering* this constant to improve command latency, which would cost
/// wakeups and buy nothing (review SE-11).
///
/// What it actually bounds is the wait on a SILENT socket: how long a cancel
/// or an inbox drain can sit behind a read that may never return. At 500 ms
/// that is two idle wakeups per second per session -- affordable at 25
/// characters, and the reason it is not shorter.
const READ_DEADLINE: Duration = Duration::from_millis(500);

/// How long one write may block before the connection is considered gone.
///
/// # This exists because a stalled write froze everything
///
/// `plan/12` §5.5: *"every wait has a deadline; no unbounded `await`"*. The
/// three `write_all` calls did not have one, and they sit **outside** the
/// select loop -- so a socket whose send buffer was full (a peer that stopped
/// reading, a half-open connection) blocked the actor entirely: no reads, no
/// cancellation, no quit. The session was alive and deaf.
///
/// Five seconds is generous for a one-line command on a working connection --
/// the whole point is that it only fires when something is genuinely wrong.
/// **A write that times out ENDS THE CONNECTION** rather than being retried:
/// `plan/10` §10.3a's single-write rule exists because the server drops a
/// command split across two writes, so a partially written command has already
/// corrupted the stream and the only safe move is a new one.
const WRITE_DEADLINE: Duration = Duration::from_secs(5);

/// Read buffer size. One chunk per `read`, and the chunk boundary is recorded
/// as-is so a replay reproduces the same split (`cena_platform::record`).
const READ_BUF: usize = 8 * 1024;

/// Something the session saw or did, published to observers.
///
/// `plan/12` §3 and §4.4: observation never competes with attribution. Every
/// frame is published here **and** offered to an open window; the two are not
/// alternatives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// A frame arrived from the game.
    Frame(Box<Frame>),
    /// A command's bytes went out. Carries the origin, so a behavior can
    /// "notice the player moved the character and re-orient" (`plan/12` §4.1)
    /// without being cancelled by it.
    Sent {
        /// The line, without its newline.
        line: String,
        /// Manual or behavior.
        origin: crate::command::Origin,
    },
    /// The session changed lifecycle state.
    StateChanged(State),
    /// A connection attempt failed, and another is coming after `delay`.
    ///
    /// **Published rather than only logged.** The supervisor writes its retry
    /// decisions to the session log, which is the right place for them -- but a
    /// log file is not where someone watching a client find its way back looks.
    /// The author's live reconnect showed four `logging in` lines with nothing
    /// between them and read as "the ladder did not wait", when the waits were
    /// in a file on disk.
    ///
    /// `cena-session` does not print (a library must not own a terminal), so
    /// the way to put this in front of a person is an event a frontend can
    /// render.
    ConnectFailed {
        /// Which attempt this was, counting from 1.
        attempt: u32,
        /// How long until the next one.
        delay: Duration,
        /// Why it failed, already redacted -- this is displayed.
        detail: String,
    },
}

/// What a finished session leaves behind.
///
/// One struct rather than a tuple because three of its four fields are things
/// a different criterion asserts over, and a `(Recorder, GameState, State, S)`
/// at four call sites is where the wrong element gets read.
///
/// The **source comes back**, which is criterion 6's evidence: "no leaked
/// sockets" is checked by asking the source whether it was shut down, rather
/// than by trusting that a drop ran.
#[derive(Debug)]
pub struct SessionEnd<S: ByteSource> {
    /// Everything that crossed the wire. Criterion 7 replays this.
    pub recorder: Recorder,
    /// What the session knew when it ended. Criteria 2 and 8 read this.
    pub state: GameState,
    /// Always [`State::Closed`] -- the session shut down cleanly.
    pub lifecycle: State,
    /// The source, closed. Criterion 6 reads this.
    pub source: S,
    /// Why the loop broke.
    ///
    /// Milestone 2's supervisor reads this and nothing else to decide whether
    /// to reconnect ([`EndReason::warrants_reconnect`]).
    pub reason: EndReason,
    /// The command receiver, handed back for the next generation.
    ///
    /// **It has to come back.** It is the other end of every
    /// [`SessionHandle`](crate::SessionHandle) a caller holds, so dropping it
    /// with the actor would close the channel under them and a supervised
    /// session would lose its callers at the first reconnect.
    ///
    /// `run` consumes the actor, which is what makes "one actor, one
    /// connection" structural -- so anything that must outlive the connection
    /// travels out through here rather than being reachable on the actor
    /// afterwards.
    pub commands: mpsc::Receiver<crate::command::Inbox>,
    /// This session's log sink, handed back so the next generation writes to
    /// the same file rather than starting a new one.
    pub sink: Option<SessionSink>,
}

/// The task. One per session.
#[derive(Debug)]
pub struct SessionActor<S: ByteSource> {
    source: S,
    parser: Parser,
    state: GameState,
    lifecycle: State,
    queue: CommandQueue,
    /// Prompts owed to `send_now` commands whose responses have not arrived.
    ///
    /// An instant action bypasses the queue, so its response is not attributed
    /// to anything -- but it still draws a prompt, and a prompt is what closes
    /// the in-flight command's round-trip window (`plan/12` §4.4). Without
    /// this counter a sigil's prompt closed the window belonging to the
    /// command it was sent to modify (review SE-5).
    send_now_prompts_owed: usize,
    commands: mpsc::Receiver<crate::command::Inbox>,
    events: broadcast::Sender<Event>,
    recorder: Recorder,
    /// Where this session's wire traffic is written, if anywhere.
    ///
    /// **`Option`, and that is the point.** A session with no sink behaves
    /// exactly as it did before logging existed, which is what leaves every
    /// existing test -- and criterion 7's replay in particular -- untouched by
    /// this field. The binary attaches one; tests do not.
    sink: Option<SessionSink>,
    cancel: CancellationToken,
    generation: Generation,
    /// What a lost transport means to whoever owns this actor.
    ///
    /// **Not a "supervised" flag.** It is a fact the owner knows and the actor
    /// cannot: whether anything will open another connection. A plain
    /// [`Session`] answers [`Outcome::Dead`](crate::command::Outcome::Dead) because
    /// nothing will; a supervisor sets
    /// [`Outcome::Disconnected`](crate::Outcome::Disconnected) because it
    /// will.
    ///
    /// Getting this wrong is a lie in one direction or the other -- `Dead` when
    /// a reconnect is coming tells a behavior to give up on a session that is
    /// about to work, and `Disconnected` when nothing is coming leaves it
    /// waiting forever. Neither is recoverable by the caller, which is why the
    /// distinction is carried rather than guessed.
    ///
    /// Only ever `Dead` or `Disconnected`; it is an [`Outcome`](crate::Outcome)
    /// rather than a `bool` so the value reads as what it is at the point it is
    /// sent, instead of being re-derived from a flag.
    on_disconnect: crate::command::Outcome,
    /// Set once an exit command has gone out: the loop is now **waiting for the
    /// server to close the stream** (`plan/16` §5b.3).
    ///
    /// # Why this is a field and not a loop inside the quit handler
    ///
    /// Awaiting the EOF inline would mean reading the socket from somewhere
    /// other than the one read arm -- two readers of one source, with the
    /// parser and the recorder fed from both. Setting a deadline the existing
    /// arm already observes keeps **one reader**, so the frames the server
    /// sends on its way out are ingested, recorded and published exactly like
    /// any others. A logout message is still a message.
    ///
    /// It also makes the EOF distinction free: `Ok(0)` while this is set is
    /// [`Farewell::Acknowledged`], and the deadline expiring is
    /// [`Farewell::TimedOut`] -- Lich's `remote_eof?`-versus-reader-stopped
    /// check (`orderly_shutdown.rb:189`), which is the same distinction a
    /// reconnect needs.
    quitting: Option<Quitting>,
    /// Set when a write failed or timed out, so [`Self::handle_inbox`] can end
    /// the connection.
    ///
    /// A flag rather than a richer return from `send_now`, which has five early
    /// returns that have nothing to do with writing. It names ONE thing --
    /// "the bytes did not go out" -- and is taken (cleared) when read.
    write_broke_the_stream: bool,
}

/// An exit command has been sent; this is what the loop owes the caller.
#[derive(Debug)]
struct Quitting {
    /// When to stop waiting for the server's EOF.
    deadline: tokio::time::Instant,
    /// Where the verdict goes. Taken by whichever path resolves first.
    reply: Option<tokio::sync::oneshot::Sender<crate::command::Farewell>>,
}

impl<S: ByteSource> SessionActor<S> {
    /// Build one generation's actor over `source`, from a supervisor's durable
    /// parts.
    ///
    /// The counterpart of [`Session::new`], which builds a session that will
    /// only ever have one connection. Everything per-connection is created
    /// fresh here -- notably [`Parser`], whose `pending` buffer must not carry
    /// a half-read tag across a reconnect
    /// (`reference/lich-5/lib/games.rb:432`; see
    /// [`SessionCore`](crate::supervisor::SessionCore)).
    ///
    /// `on_disconnect` is [`Outcome::Disconnected`](crate::Outcome::Disconnected)
    /// because a supervised session **is** getting another connection -- the
    /// one thing a plain `Session` cannot promise.
    #[allow(
        clippy::too_many_arguments,
        reason = "every part is durable state the supervisor owns; bundling them \n                  into a struct would be the same list behind one more name"
    )]
    pub(crate) fn supervised(
        source: S,
        state: GameState,
        commands: mpsc::Receiver<crate::command::Inbox>,
        events: broadcast::Sender<Event>,
        recorder: Recorder,
        sink: Option<SessionSink>,
        cancel: CancellationToken,
        generation: Generation,
    ) -> Self {
        Self {
            source,
            parser: Parser::new(),
            state,
            lifecycle: State::Connecting,
            queue: CommandQueue::new(),
            send_now_prompts_owed: 0,
            commands,
            events,
            recorder,
            sink,
            cancel,
            generation,
            on_disconnect: crate::command::Outcome::Disconnected,
            quitting: None,
            write_broke_the_stream: false,
        }
    }

    /// Run until cancelled, until the stream ends, or until a read fails.
    ///
    /// All three are **clean** ends (criterion 6): the source is shut down,
    /// every waiter is answered [`Outcome::Dead`](crate::command::Outcome::Dead), and the function returns.
    /// None of them panics, and none of them leaves a socket open.
    pub async fn run(mut self) -> SessionEnd<S> {
        // The three states Step 2 transits before behaviors may run. A replay
        // has nothing to do in Connecting or Authenticating and no Infomon
        // sync to run in Syncing (`plan/12` §7.1 puts that in the Out column),
        // so they are transited rather than worked -- but they are transited,
        // which is what makes §5.3's readiness gate have a false branch.
        self.transition(State::Authenticating);
        self.transition(State::Syncing);
        self.transition(State::Ready);

        let mut buf = vec![0u8; READ_BUF];
        // See the command arm below for why this exists.
        let mut senders_gone = false;
        // Set by whichever arm breaks. Not an `Option` unwrapped at the end:
        // every `break` below assigns it first, and the compiler checks that
        // because the loop cannot fall through.
        let reason;
        loop {
            // Drain the queue before waiting. A command admitted on the last
            // turn must go out before the loop parks in a read, or a manual
            // command typed into a quiet session would wait READ_DEADLINE.
            if let Some(failed) = self.pump().await {
                reason = failed;
                break;
            }
            tokio::select! {
                // `biased` -- see the module docs. Order is a property of this
                // source, not of the scheduler's coin flip, and cancellation
                // comes first so PREEMPT_GRACE is not spent waiting a turn.
                biased;

                () = self.cancel.cancelled() => {
                    // A cancel during a pending quit answers it rather than
                    // dropping the caller: they asked for a clean exit and are
                    // still waiting on the reply.
                    self.finish_quit(crate::command::Farewell::TimedOut);
                    reason = EndReason::Cancelled;
                    break;
                }

                // ONLY ARMED WHILE A QUIT IS PENDING, and the guard matters for
                // the same reason the command arm's does: an unconditional
                // `sleep_until` on a far-future instant is fine, but there is
                // no instant to name when nothing is quitting. `quit_deadline`
                // returns a far-future one so the arm is always well-formed,
                // and the guard stops it firing for a session that never quit.
                () = tokio::time::sleep_until(self.quit_deadline()), if self.quitting.is_some() => {
                    // The server was asked and did not close. Lich raises
                    // `ServerExitTimeout` here (`orderly_shutdown.rb:188`).
                    // The session ends ANYWAY -- criterion 6's "no leaked
                    // sockets" may not become conditional on the server
                    // cooperating (`plan/16` §5b, ordering note).
                    self.log("quit: server did not close within the timeout");
                    self.finish_quit(crate::command::Farewell::TimedOut);
                    reason = EndReason::Cancelled;
                    break;
                }

                // DISABLED ONCE THE CHANNEL CLOSES, and the guard is not
                // cosmetic. A closed `mpsc::Receiver` returns `None`
                // *immediately and forever*, so without `if !senders_gone`
                // this arm is permanently ready and `biased;` -- which picks
                // the first ready arm -- never reaches the read below. VERIFIED
                // by writing it without the guard: the test hung, burning CPU,
                // and with `start_paused` tokio never auto-advances because
                // the task is never idle. Every handle dropped means nobody
                // can send again; frames already in flight still matter, so
                // the session ends when the STREAM does, not here.
                received = self.commands.recv(), if !senders_gone => match received {
                    Some(message) => if let Some(failed) = self.handle_inbox(message).await {
                        reason = failed;
                        break;
                    },
                    None => senders_gone = true,
                },

                read = tokio::time::timeout(READ_DEADLINE, self.source.read(&mut buf)) => {
                    match read {
                        // SPLIT IN MILESTONE 2, as this arm's M1 comment said
                        // it would be. Criterion 6 treats them identically --
                        // both shut the source down and answer every waiter --
                        // and so does `warrants_reconnect`. They are two
                        // variants so a LOG can tell them apart; see
                        // `EndReason`.
                        Ok(Ok(0)) => {
                            // **The EOF a quit was waiting for.** This is
                            // Lich's `remote_eof?` (`orderly_shutdown.rb:189`):
                            // the server closed because we asked, which is a
                            // deliberate stop and must NOT reconnect.
                            //
                            // Without this the supervisor would see
                            // `PeerClosed`, call it a lost transport, and log
                            // the character straight back in -- the exact
                            // failure §5b names when it says "a clean `quit`
                            // must not trigger a reconnect; a drop must".
                            reason = if self.finish_quit(crate::command::Farewell::Acknowledged) {
                                EndReason::Cancelled
                            } else {
                                EndReason::PeerClosed
                            };
                            break;
                        }
                        Ok(Err(_)) => {
                            reason = EndReason::ReadFailed;
                            break;
                        }
                        Ok(Ok(n)) => self.ingest(&buf[..n]),
                        // A quiet game is normal, not a failure. The deadline
                        // exists so the loop takes a turn (`plan/12` §5.5:
                        // no unbounded await), not to end the session.
                        Err(_elapsed) => {}
                    }
                }
            }
        }
        self.shutdown(reason).await;
        SessionEnd {
            recorder: self.recorder,
            state: self.state,
            lifecycle: self.lifecycle,
            source: self.source,
            reason,
            commands: self.commands,
            sink: self.sink,
        }
    }

    /// Take everything already queued on the command channel, applying the
    /// readiness gate to each.
    ///
    /// Public so that `plan/12` §5.3's gate can be exercised in the state it
    /// governs. `run` transitions to `Ready` before its first turn -- there is
    /// no Infomon sync to wait for in Step 2 (§7.1) -- so a test that only
    /// used `run` could never observe the gate's false branch, and a gate
    /// whose false branch is unreachable is not a gate (`plan/05` §0).
    ///
    /// Non-blocking: it drains what is there and returns. It is the same
    /// `admit` the loop calls, not a second path that could drift from it.
    pub async fn drain_commands_once(&mut self) {
        while let Ok(message) = self.commands.try_recv() {
            // A write failure here is DISCARDED, deliberately: this method is a
            // test helper for the readiness gate and has no loop to end. The
            // flag it may set is cleared by the next `handle_inbox`, so it
            // cannot leak into a later turn as a spurious end.
            let _ = self.handle_inbox(message).await;
        }
    }

    /// Write one line to the session log, if there is one.
    ///
    /// **Swallows the error deliberately.** A full disk, a revoked permission
    /// or a deleted directory must not end a session: `plan/12` §5.5's
    /// containment table is about a session surviving its own faults, and a
    /// log is an observer of the session rather than part of it. The write is
    /// attempted every time rather than disabled after one failure, because a
    /// transient failure should not silently stop all later logging.
    pub(super) fn log(&mut self, line: &str) {
        if let Some(sink) = self.sink.as_mut() {
            let _ = sink.event(line);
        }
    }

    /// Write raw wire bytes to the session log, if there is one.
    pub(super) fn log_wire(&mut self, inbound: bool, bytes: &[u8]) {
        if let Some(sink) = self.sink.as_mut() {
            let _ = sink.wire(inbound, bytes);
        }
    }

    /// The state as the actor currently sees it. For tests that drive the
    /// actor by hand rather than spawning it.
    #[must_use]
    pub fn state(&self) -> &GameState {
        &self.state
    }

    /// The lifecycle state. `plan/12` §5.3's gate reads this.
    #[must_use]
    pub fn lifecycle(&self) -> State {
        self.lifecycle
    }
}

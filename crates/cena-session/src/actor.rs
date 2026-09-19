//! The session actor: one task, one select loop, one socket.
//!
//! `plan/12` §5.5 requires "one supervised task per session", "bounded
//! channels everywhere", "every wait has a deadline" and "a panic kills one
//! session, not the process". This module is all four.
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
//! Frame-to-state folding is in [`crate::state`] and the queue is in
//! [`crate::queue`]. Following the precedent in
//! `crates/cena-arch-tests/tests/ratchet.rs:4-8`, this header named its next
//! split before it was needed: "if this file grows, `run_command` and `ingest`
//! move to `actor/io.rs` and the loop stays."
//!
//! It grew -- to 424 lines against the 400 default, when `plan/12` §5.3's
//! readiness gate landed -- and **the split was taken as written** rather than
//! the cap being raised (`plan/05:352-353`). `pump` and `ingest` are now in
//! [`io`]; what is here is the loop, the session's shape, and the gate.
//!
//! That split has since been taken too: [`Session`] and [`Snapshot`] are in
//! [`handle`].
//!
//! And a third: [`EndReason`] is in [`ending`], moved there when Milestone 2's
//! end-reason took this file to 402 against the cap. That seam had been named
//! one commit earlier, which is the practice
//! `crates/cena-arch-tests/tests/ratchet.rs:4-8` set -- **the fourth time it
//! has paid.**
//!
//! **That split has been taken too**, one step later: `shutdown`, `transition`
//! and the logging helpers joined [`EndReason`] in [`ending`] when the
//! supervisor took this file to 437. Named in advance, taken as written --
//! the fifth time.
//!
//! **The next split, named in advance and not yet needed:** [`SessionEnd`] and
//! [`SessionActor::supervised`] to `actor/parts.rs` -- what a connection is
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
const EVENT_CHANNEL_BOUND: usize = 256;

/// How long one read may block before the loop takes a turn anyway.
///
/// `plan/12` §5.5: "every wait has a deadline; no unbounded `await`". The
/// deadline is not a failure -- a quiet game is normal -- it is what lets the
/// loop notice a queued command while nothing is arriving.
const READ_DEADLINE: Duration = Duration::from_millis(500);

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
    /// [`Session`] answers [`Outcome::Dead`](crate::Outcome::Dead) because
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
        reason = "every part is durable state         the supervisor owns; bundling them into a struct would be the same         list behind one more name"
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
            commands,
            events,
            recorder,
            sink,
            cancel,
            generation,
            on_disconnect: crate::command::Outcome::Disconnected,
        }
    }

    /// Run until cancelled, until the stream ends, or until a read fails.
    ///
    /// All three are **clean** ends (criterion 6): the source is shut down,
    /// every waiter is answered [`Outcome::Dead`], and the function returns.
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
                    Some(message) => self.handle_inbox(message).await,
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
                            reason = EndReason::PeerClosed;
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
            self.handle_inbox(message).await;
        }
    }

    /// Accept a command, or refuse it because the session is not `Ready`.
    ///
    /// **This is `plan/12` §5.3's readiness gate**: "Behaviors may not start
    /// until `Ready`." A gate that is only a method nobody calls is the wish
    /// `plan/05` §0 warns about, so it is enforced at the one place a
    /// behavior's command can enter the session.
    ///
    /// **Manual input is NOT gated.** §5.3 gates *behaviors*; §4.1 says the
    /// player is never locked out of their character. Refusing a typed command
    /// during `Syncing` would be exactly that lockout, and it is not what
    /// either section asks for.
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

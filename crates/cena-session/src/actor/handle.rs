//! [`Session`] and [`Snapshot`]: what a caller holds, as against what the
//! actor runs.
//!
//! **This split was named in advance.** [`super`]'s header said: "The next
//! split, if this grows again: `Session` and `Snapshot` to `actor/handle.rs`,
//! leaving `SessionActor` and its loop alone." It grew -- to 415 lines against
//! the 400 cap, when the session gained its log sink -- and the split was
//! taken as written rather than the cap being raised (`plan/05` Rule 4.1).
//!
//! Naming the next split before needing it is the practice
//! `crates/cena-arch-tests/tests/ratchet.rs:4-8` set, and this is the third
//! time it has paid: a file at its cap has an obvious seam instead of an
//! argument about one.

use super::{COMMAND_CHANNEL_BOUND, EVENT_CHANNEL_BOUND, Event, SessionActor};
use crate::command::SessionHandle;
use crate::lifecycle::{Generation, State};
use crate::queue::CommandQueue;
use cena_model::GameState;
use cena_platform::{ByteSource, Recorder, SessionSink};
use cena_protocol::Parser;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

/// A consistent view of the session, taken at a point in the event stream.
///
/// `plan/12` §6.2: the join is a single operation, so no event between the
/// snapshot and the subscription is lost. Here the atomicity is structural
/// rather than lock-based -- both halves are produced by the actor's own task,
/// which is the only writer, so there is no window to be atomic *across*.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// What the session knew.
    pub state: GameState,
    /// Where it was in its life.
    pub lifecycle: State,
    /// Which connection this belongs to (`plan/12` §5.2).
    pub generation: Generation,
}

/// Everything a caller needs to drive and observe one session.
#[derive(Debug)]
pub struct Session<S: ByteSource> {
    actor: SessionActor<S>,
    handle: SessionHandle,
    events: broadcast::Sender<Event>,
    cancel: CancellationToken,
}

impl<S: ByteSource> Session<S> {
    /// Build a session over this byte source.
    ///
    /// Nothing runs until [`Session::into_actor`]'s actor is driven, so
    /// constructing a session touches no network even with a
    /// [`LiveSource`](cena_platform::LiveSource).
    #[must_use]
    pub fn new(source: S) -> Self {
        let (tx, rx) = mpsc::channel(COMMAND_CHANNEL_BOUND);
        let (events, _) = broadcast::channel(EVENT_CHANNEL_BOUND);
        let cancel = CancellationToken::new();
        // The cell a handle reads. A plain `Session` never advances it -- one
        // connection, one generation -- but the handle reads it the same way,
        // so a supervised session needs no different handle type.
        let generation = crate::lifecycle::GenerationCell::first();
        Self {
            actor: SessionActor {
                source,
                parser: Parser::new(),
                state: GameState::default(),
                lifecycle: State::Connecting,
                queue: CommandQueue::new(),
                send_now_prompts_owed: 0,
                commands: rx,
                events: events.clone(),
                recorder: Recorder::new(),
                sink: None,
                combat: None,
                combat_refusals_logged: 0,
                cancel: cancel.clone(),
                generation: generation.get(),
                // A plain `Session` has nothing above it to reconnect, so a
                // lost transport IS the end. A supervisor overrides this;
                // see `SessionActor::on_disconnect`.
                on_disconnect: crate::command::Outcome::Dead,
                quitting: None,
                write_broke_the_stream: false,
            },
            handle: SessionHandle::new(tx, generation),
            events,
            cancel,
        }
    }

    /// Write this session's wire traffic to `sink`.
    ///
    /// Separate from [`Session::new`] rather than a parameter on it: every
    /// test in three crates constructs a session, none of them wants a log,
    /// and a required argument would have them all passing `None`. The binary
    /// is the only caller.
    ///
    /// A session without a sink logs nothing and is otherwise identical --
    /// which is what keeps criterion 7's replay unaffected by this feature.
    #[must_use]
    pub fn with_sink(mut self, sink: SessionSink) -> Self {
        self.actor.sink = Some(sink);
        self
    }

    /// Give the combat tracker its crit tables.
    ///
    /// Shared, not loaded here: compiling 2,394 patterns is ~94ms and the
    /// tables are immutable, so the binary loads them once and every session
    /// holds the same `Arc`. Without them every hit records `crit: None`.
    #[must_use]
    pub fn with_crit_tables(
        mut self,
        tables: std::sync::Arc<cena_model::crit::CritTables>,
    ) -> Self {
        self.actor.state.combat_mut().set_crit_tables(tables);
        self
    }

    /// Offer every closed chunk's combat facts to this recorder.
    #[must_use]
    pub fn with_combat_recorder(
        mut self,
        recorder: crate::combat_recorder::worker::RecorderHandle,
    ) -> Self {
        self.actor.combat = Some(recorder);
        self
    }

    /// A handle for sending commands. Cloneable: the manual surface and a
    /// behavior hold the same one, which is what makes criterion 3's "the
    /// **same** queue" structural.
    #[must_use]
    pub fn handle(&self) -> SessionHandle {
        self.handle.clone()
    }

    /// The shared generation counter this session's handles read.
    ///
    /// A plain `Session` never advances it -- one connection, one generation.
    /// It is exposed because a **supervisor** must: advancing it between
    /// connections is what lets a [`SessionHandle`] cloned in an earlier
    /// generation keep working, while a command already stamped stays
    /// correctly stale (`plan/12` §4.4, and see
    /// [`GenerationCell`](crate::GenerationCell)).
    #[must_use]
    pub fn generation_cell(&self) -> crate::lifecycle::GenerationCell {
        self.handle.generation_cell()
    }

    /// The token that stops the session. `plan/12` §4.3: only an explicit
    /// `stop` or `pause` preempts -- never a typed command (§4.1).
    #[must_use]
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    /// Subscribe to events.
    ///
    /// `plan/12` §6.2 wants `(snapshot, events)` as one operation. Before the
    /// actor runs the snapshot is trivially the initial state; after it is
    /// running, a subscriber calls this and then reads, and the actor is the
    /// only writer, so nothing is interleaved.
    #[must_use]
    pub fn subscribe(&self) -> (Snapshot, broadcast::Receiver<Event>) {
        let receiver = self.events.subscribe();
        (
            Snapshot {
                state: self.actor.state.clone(),
                lifecycle: self.actor.lifecycle,
                generation: self.actor.generation,
            },
            receiver,
        )
    }

    /// Consume the session, yielding the actor to drive.
    pub fn into_actor(self) -> SessionActor<S> {
        self.actor
    }
}

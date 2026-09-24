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

use super::{COMMAND_CHANNEL_BOUND, EVENT_CHANNEL_BOUND, Event, SessionActor, owed};
use crate::command::SessionHandle;
use crate::lifecycle::{Generation, State};
use crate::observation::{EventPublisher, ObservationRequests};
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
    /// Which session this is.
    ///
    /// Constant across every reconnect, where [`Self::generation`] advances.
    /// A frontend serving many sessions over one listener (`plan/23` §D1a)
    /// routes on this.
    pub session: crate::lifecycle::SessionId,
    /// What the session knew.
    pub state: GameState,
    /// Where it was in its life.
    pub lifecycle: State,
    /// Which connection this belongs to (`plan/12` §5.2).
    pub generation: Generation,
    /// Last published event included in this snapshot's fence.
    pub cursor: u64,
    /// Most recent retry decision while reconnecting, cleared on transition.
    pub retry: Option<crate::RetryStatus>,
}

/// Everything a caller needs to drive and observe one session.
#[derive(Debug)]
pub struct Session<S: ByteSource> {
    actor: SessionActor<S>,
    handle: SessionHandle,
    events: EventPublisher,
    cancel: CancellationToken,
}

impl<S: ByteSource> Session<S> {
    /// Build a session over this byte source.
    ///
    /// Nothing runs until [`Session::into_actor`]'s actor is driven, so
    /// constructing a session touches no network even with a
    /// [`LiveSource`](cena_platform::LiveSource).
    ///
    /// It is [`SessionId::FIRST`](crate::SessionId::FIRST): the one session
    /// of a process that has one. See [`Self::numbered`] for the others.
    #[must_use]
    pub fn new(source: S) -> Self {
        Self::numbered(crate::lifecycle::SessionId::FIRST, source)
    }

    /// Build session `id` over this byte source: [`Self::new`] for a process
    /// running several (`plan/29`). Every event and snapshot names `id`, so a
    /// frontend serving N sessions can tell them apart.
    ///
    /// The caller mints the ids. Nothing here counts them: a process-wide
    /// counter would be global state, which `plan/12` §7.1 keeps out.
    #[must_use]
    pub fn numbered(id: crate::lifecycle::SessionId, source: S) -> Self {
        let (tx, rx) = mpsc::channel(COMMAND_CHANNEL_BOUND);
        let cancel = CancellationToken::new();
        // The cell a handle reads. A plain `Session` never advances it -- one
        // connection, one generation -- but the handle reads it the same way,
        // so a supervised session needs no different handle type.
        let generation = crate::lifecycle::GenerationCell::first();
        let events = EventPublisher::new(EVENT_CHANNEL_BOUND, generation.clone(), id);
        let handle = SessionHandle::publishing_to(tx, generation.clone(), events.clone());
        let mut queue = CommandQueue::new();
        queue.share_authority(handle.authority_cell());
        Self {
            actor: SessionActor {
                source,
                parser: Parser::new(),
                state: GameState::default(),
                lifecycle: State::Connecting,
                queue,
                owed: owed::OwedPrompts::default(),
                quiet_window: false,
                readiness: super::readiness::Readiness::default(),
                commands: rx,
                events: events.clone(),
                observations: ObservationRequests::new(),
                recorder: Recorder::new(),
                sink: None,
                combat: None,
                menu_dir: None,
                player_log: None,
                persistence: Box::default(),
                combat_refusals_logged: 0,
                cancel: cancel.clone(),
                generation: generation.get(),
                // A plain `Session` has nothing above it to reconnect, so a
                // lost transport IS the end. A supervisor overrides this;
                // see `SessionActor::on_disconnect`.
                on_disconnect: crate::command::Outcome::Dead,
                quitting: None,
                write_ended: None,
            },
            handle,
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

    /// Persist learned menu-command rows under `dir`.
    ///
    /// Separate from [`Session::new`] for the reason [`Session::with_sink`]
    /// is: no test wants a file, and a required argument would have them all
    /// passing `None`.
    ///
    /// The file is written **the moment a `<cmdlist>` push arrives**, which is
    /// what Lich does for the same kind of fact (`infomon.rb:211-217` queues
    /// the write inside `set`, per value, rather than saving at logout). Lich
    /// drains those through a background thread because a sync writes
    /// thousands of values; a push was MEASURED at once in a month of logs, so
    /// the write is direct. If pushes ever turn out to be frequent, that queue
    /// is the known fix.
    ///
    /// `dir` is a directory rather than a path because the dictionary is
    /// **global**: every character receives the same push, so all sessions in
    /// a process share one file. See [`crate::menu_store`].
    #[must_use]
    pub fn with_menu_store(mut self, dir: std::path::PathBuf) -> Self {
        self.actor.menu_dir = Some(dir);
        self
    }

    /// Persist this character's facts under `dir`.
    ///
    /// Written **five minutes after they stop changing**, and unconditionally
    /// on a clean shutdown. See [`crate::dirty_groups`].
    ///
    /// Separate from [`Session::new`] for the reason [`Session::with_sink`]
    /// is: no test wants a file, and a required argument would have them all
    /// passing `None`.
    #[must_use]
    pub fn with_character_store(mut self, dir: std::path::PathBuf) -> Self {
        self.actor.persistence.dir = Some(dir);
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

    /// Record what the player saw and sent in this log (`plan/25`).
    #[must_use]
    pub fn with_player_log(
        mut self,
        log: crate::PlayerLog,
        capture: crate::player_log::Capture,
        settings_dir: Option<std::path::PathBuf>,
    ) -> Self {
        let tap = crate::player_log::Tap::new(log, self.events.session(), capture, settings_dir);
        // First attachment wins; the actor and the handle must agree.
        let tap = self.handle.log_slot().get_or_init(|| tap).clone();
        self.actor.player_log = Some(crate::player_log::Feed::new(tap, self.actor.generation));
        self
    }

    /// A handle for sending commands. Cloneable: the manual surface and a
    /// behavior hold the same one, which is what makes criterion 3's "the
    /// **same** queue" structural.
    #[must_use]
    pub fn handle(&self) -> SessionHandle {
        self.handle.clone()
    }

    /// Read-only access that remains usable after `into_actor` consumes this owner.
    #[must_use]
    pub fn observer(&self) -> crate::SessionObserver {
        self.actor.observations.observer()
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
    /// This owner can only be borrowed before the actor is running. Use
    /// `observer()` for fresh subscriptions while it runs.
    #[must_use]
    pub fn subscribe(&self) -> (Snapshot, broadcast::Receiver<Event>) {
        let receiver = self.events.subscribe();
        (
            self.events
                .snapshot(&self.actor.state, self.actor.lifecycle),
            receiver,
        )
    }

    /// Consume the session, yielding the actor to drive.
    pub fn into_actor(self) -> SessionActor<S> {
        self.actor
    }
}

impl<S: ByteSource> SessionActor<S> {
    /// Hold the command authority in the session's cell rather than this
    /// connection's own, so a holder keeps it across a reconnect
    /// (`command/authority.rs`, SE-4).
    pub(crate) fn share_authority(&mut self, cell: crate::command::authority::Authority) {
        self.queue.share_authority(cell);
    }
}

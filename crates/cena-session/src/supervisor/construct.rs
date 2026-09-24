//! Building a supervised session: [`SupervisedSession::new`] and
//! [`SupervisedSession::numbered`].
//!
//! Moved down out of `supervisor.rs` under Rule 4.4 when `numbered`
//! (`plan/29` step 1) took it past its cap (660 of 650).

use super::{Connector, SessionCore, SupervisedSession};
// The channel bounds are the actor's, imported rather than copied: a
// supervised session and a plain one must size their channels alike, and two
// copies commented "matches" is how they would stop matching.
use crate::actor::{COMMAND_CHANNEL_BOUND, EVENT_CHANNEL_BOUND};
use crate::command::SessionHandle;
use crate::lifecycle::{GenerationCell, State};
use crate::observation::{EventPublisher, ObservationRequests};
use cena_model::GameState;
use cena_platform::Recorder;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

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
    ///
    /// It is [`SessionId::FIRST`](crate::SessionId::FIRST): the one session
    /// of a process that has one. See [`Self::numbered`] for the others.
    #[must_use]
    pub fn new(connector: C) -> (Self, SessionHandle) {
        Self::numbered(crate::lifecycle::SessionId::FIRST, connector)
    }

    /// Build supervised session `id`: [`Self::new`] for a process running
    /// several (`plan/29`). Every event, snapshot and player-log line names
    /// `id`, across every reconnect.
    ///
    /// The caller mints the ids. Nothing here counts them: a process-wide
    /// counter would be global state, which `plan/12` §7.1 keeps out.
    #[must_use]
    pub fn numbered(id: crate::lifecycle::SessionId, connector: C) -> (Self, SessionHandle) {
        let (tx, rx) = mpsc::channel(COMMAND_CHANNEL_BOUND);
        let generation = GenerationCell::first();
        let events = EventPublisher::new(EVENT_CHANNEL_BOUND, generation.clone(), id);
        let handle = SessionHandle::publishing_to(tx, generation.clone(), events.clone());
        let session = Self {
            core: SessionCore {
                id,
                commands: rx,
                events,
                observations: ObservationRequests::new(),
                lifecycle: State::Connecting,
                state: GameState::default(),
                recorder: Recorder::new(),
                sink: None,
                combat: None,
                player_log: handle.log_slot(),
                character_dir: None,
                menu_dir: None,
                generation,
                attendance: handle.attendance(),
                attendance_seen: 0,
                authority: handle.authority_cell(),
                cancel: CancellationToken::new(),
            },
            connector,
        };
        (session, handle)
    }
}

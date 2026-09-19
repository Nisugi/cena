//! [`SessionCore`]: everything that survives a reconnect.
//!
//! **The boundary of this struct IS `plan/12` §5.2's contract, expressed as
//! ownership.** What lives here spans generations; what the actor owns does
//! not. That makes the classification structural rather than a rule someone has
//! to remember — a field put in the wrong place is a bug you can see.
//!
//! # What is deliberately NOT here
//!
//! **`Parser`.** Its `pending` buffer holds an unterminated trailing fragment,
//! and Lich clears the equivalent on reconnect so that *"a fragment left open
//! before a reconnect/session reset does not bleed into the next session"*
//! (`reference/lich-5/lib/games.rb:432`). A connection dropped mid-tag would
//! otherwise splice its fragment onto the next generation's first read — one
//! corrupt frame per reconnect, visible only on a real mid-tag drop.
//!
//! Cena needs no `Parser::reset()` for this: **a new connection gets a new
//! actor, and a new actor gets `Parser::new()`**, which resets `pending` along
//! with every other per-connection field at once. The absence of a field is the
//! design decision here, which is why it is written down — an absent field with
//! no comment reads as an oversight.
//!
//! **`CommandQueue`.** Same reasoning: a queue holds waiters for one
//! connection, and they are answered by `shutdown` before the actor returns.
//! Carrying them forward would deliver a previous connection's answers.

use crate::actor::Event;
use crate::command::Inbox;
use crate::lifecycle::GenerationCell;
use cena_model::GameState;
use cena_platform::{Recorder, SessionSink};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

/// The parts of a session that outlive any one connection.
#[derive(Debug)]
pub struct SessionCore {
    /// The one command channel.
    ///
    /// Durable so that a [`SessionHandle`](crate::SessionHandle) cloned in
    /// generation 0 still reaches the actor in generation 3. A fresh channel
    /// per connection would invalidate every handle a caller holds.
    pub(super) commands: mpsc::Receiver<Inbox>,
    /// The event stream.
    ///
    /// Durable so a `broadcast::Receiver` taken in generation 0 keeps
    /// receiving. `VellumFE` does exactly this on reconnect — it builds a new
    /// command channel and **keeps the existing `server_rx`**
    /// (`reference/VellumFE/src/frontend/tui/runtime.rs:546-600`). Cena keeps
    /// both, because Cena's handle is durable where Vellum's is not.
    pub(super) events: broadcast::Sender<Event>,
    /// What the session knows.
    ///
    /// Carried across and then **invalidated** between generations
    /// ([`GameState::invalidate_for_reconnect`]), never dropped: §5.2's
    /// Retained class is most of it, and the login burst re-sends the rest.
    pub(super) state: GameState,
    /// Everything that crossed the wire, across every generation.
    ///
    /// Durable because criterion 9 is "verified in the replay", which requires
    /// the reconnect itself to be *in* the recording. A per-connection recorder
    /// could not express one.
    pub(super) recorder: Recorder,
    /// Where this session's wire traffic is written, if anywhere. Durable so a
    /// log spans the reconnect rather than restarting at it.
    pub(super) sink: Option<SessionSink>,
    /// The shared generation every handle reads.
    pub(super) generation: GenerationCell,
    /// Stops the **session**, not one connection.
    ///
    /// This is the distinction that makes a supervisor possible: the actor's
    /// own cancel ends its connection, and cancelling this ends the whole
    /// supervised session including any reconnect it was about to attempt.
    pub(super) cancel: CancellationToken,
}

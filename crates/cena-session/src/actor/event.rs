//! [`Event`]: what a session publishes to whoever is watching.
//!
//! Moved down out of `actor.rs` when `Event::Notice` took that file past its
//! cap (`plan/05` Rule 4.4: move code down, do not raise the cap). The
//! vocabulary of the event stream is a thing of its own in any case: every
//! frontend and every behavior reads it, and none of them reads the actor.

use std::time::Duration;

use cena_protocol::Frame;

use crate::lifecycle::State;

/// Something the session saw or did, published to observers.
///
/// `plan/12` §3 and §4.4: observation never competes with attribution. Every
/// frame is published here **and** offered to an open window; the two are not
/// alternatives.
// Not `Eq`: a roll's UCS total is fractional, so combat facts are
// `PartialEq` only.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A frame arrived from the game.
    Frame(Box<Frame>),
    /// A prompt closed a chunk that held combat: every attack event and fact
    /// it yielded, whole and in order.
    ///
    /// **One event per chunk, never one per fact** -- see
    /// [`ChunkFacts`](cena_model::state::combat::ChunkFacts) for why.
    ///
    /// Published AFTER the prompt's own [`Event::Frame`], and after the
    /// model applied it: a subscriber that reads state on this event sees
    /// the creatures as the chunk left them. `Arc` because every subscriber
    /// and the recorder share one allocation.
    Combat(std::sync::Arc<cena_model::state::combat::ChunkFacts>),
    /// The store was read, and these groups are stale.
    ///
    /// Empty means nothing is stale, and is still published: "checked, nothing
    /// to do" and "never checked" are different facts. See `load_character`
    /// for when this fires and why it reports rather than syncs.
    SyncNeeded(Vec<cena_model::state::character::snapshot::Group>),
    /// A command's bytes went out. Carries the origin, so a behavior can
    /// "notice the player moved the character and re-orient" (`plan/12` §4.1)
    /// without being cancelled by it.
    Sent {
        /// The line, without its newline.
        line: String,
        /// Manual or behavior.
        origin: crate::command::Origin,
    },
    /// Hydra said something to the player: a route table, why a trip
    /// stopped, what is still stored (`crate::notice`). **Not from the
    /// game**, which is why it is its own event and not a frame.
    Notice(crate::notice::Notice),
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

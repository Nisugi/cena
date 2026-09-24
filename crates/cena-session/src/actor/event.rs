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
    ///
    /// **It arrives before `Ready`.** It fires on `<app>`, inside the login
    /// burst, while the session is still `Syncing` -- and a behavior's command
    /// then is refused. Whoever runs the sync waits for
    /// `StateChanged(Ready)` first.
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
    /// A **quiet** command's window opened (`true`) or ended (`false`).
    ///
    /// `true` follows that command's [`Event::Sent`]; `false` follows the
    /// prompt that closed its window, or comes when the window ended any
    /// other way. Between the two, the game's main-stream text is that
    /// command's report, which a frontend leaves out of the story -- Lich's
    /// `issue_command(..., quiet: true)`, which infomon's sync runs through
    /// (`reference/lich-5/lib/gemstone/infomon/cli.rb:37`). The frames are
    /// still published, folded and logged: quiet is a presentation fact,
    /// never a reason to lose one. See `SessionHandle::send_quietly`.
    Quiet(bool),
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

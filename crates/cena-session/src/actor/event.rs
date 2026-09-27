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
    /// A frame finished a line of game text: the model's line, the one the
    /// classifiers and the player log read (`actor/line.rs`).
    ///
    /// Published right AFTER the [`Event::Frame`] that finished it. A viewer
    /// draws this rather than assembling lines from text frames, so every
    /// viewer agrees on where a line ends (`plan/45` §4a). With `;sorter` on,
    /// a container look is published as the lines it sorts into, one event
    /// each ([`SessionHandle::sort_containers`](crate::SessionHandle::sort_containers)).
    /// The character's triggers answer each before it is published
    /// ([`SessionHandle::set_triggers`](crate::SessionHandle::set_triggers)):
    /// it may arrive substituted, painted, on another stream, twice (a
    /// redirected copy), or not at all (a squelch).
    /// `Arc` because every subscriber shares one allocation.
    Line(std::sync::Arc<cena_model::line::Line>),
    /// A frame finished a line of game text, **as the game sent it**: the
    /// model's line before `;sorter` and the character's triggers answer it,
    /// published just before the [`Event::Line`]s it becomes (`plan/46`
    /// §4.1).
    ///
    /// What a script reads. Lich's scripts see each line before its hooks
    /// change what is shown (`inventory/13` §1.6), and a trigger is Hydra's
    /// hook: a squelch hides a line from the player, never from a script
    /// waiting for it. Published only while a script runner listens to the
    /// character ([`script::Door::listen`](crate::script::Door::listen)), so
    /// a character nobody scripts publishes each line once.
    Heard(std::sync::Arc<cena_model::line::Line>),
    /// A trigger set or cleared a flag (`cena_model::state::flags`): the
    /// session has made the change to its state, and whoever folds these
    /// events into a state of its own makes it too, so a hunt's guard reads
    /// what the session's triggers set.
    ///
    /// Published after the [`Event::Line`]s of the line that set it, or at
    /// the prompt a condition fired on; only when it changed something.
    Flag(cena_model::state::flags::FlagChange),
    /// A trigger called for attention: a sound, an OS notification, a
    /// banner (`cena_model::trigger::Attention`). The session decides it
    /// and does none of it: the binary's desk plays and notifies, once for
    /// every character that saw the same thing, and a viewer shows the
    /// banner. On a squelched line too.
    ///
    /// Published after the [`Event::Line`]s of the line that called for it,
    /// or at the prompt a condition fired on. `Arc` for [`Event::Line`]'s
    /// reason.
    Attention(std::sync::Arc<cena_model::trigger::Attention>),
    /// A trigger sends a line as if the player typed it (`plan/45` Stage 5,
    /// `cena_model::trigger::Act`): through the `;` command table first,
    /// otherwise to the game as [`Origin::Trigger`](crate::Origin::Trigger).
    /// The session decides and paces it; the binary sends it, holding the
    /// handle a command table is on.
    Act(std::sync::Arc<cena_model::trigger::Act>),
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
    /// The agent's side of the session changed: the player set its level or
    /// answered a request, or an operation it started moved on
    /// (`crate::agent`). Published so an agent learns it in order with
    /// everything else; the player was told in a notice.
    Agent(crate::agent::Change),
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

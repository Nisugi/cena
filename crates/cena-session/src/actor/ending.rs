//! [`EndReason`]: why one connection ended, and whether to reconnect.
//!
//! Split out of `actor.rs` under Rule 4.1 (`plan/05:352-353`) -- **move code
//! down, do not raise the cap.** That file reached 402 lines against the 400
//! default when Milestone 2's end-reason landed, and its header had already
//! named this split in advance. Taken as written rather than argued about.
//!
//! The loop and the session's shape stay in [`super`]; what is here is the
//! vocabulary for *how a connection stopped*, which a supervisor reads and the
//! actor merely reports -- and, since Milestone 2's supervisor took `actor.rs`
//! to 437 lines, the **stopping itself**: [`SessionActor::shutdown`] and
//! [`SessionActor::transition`] came here in the second half of the split this
//! file's own header named.
//!
//! The logging helpers came with them: `shutdown` already flushes the sink, so
//! the sink's writers belong beside the code that closes it rather than beside
//! the loop.

use super::{Event, SessionActor, State};
use cena_platform::ByteSource;

/// Why one connection ended.
///
/// # What Milestone 1 deferred, arriving
///
/// The loop's read arm collapsed `Ok(0)` and `Err(_)` into one `break`, with a
/// comment saying distinguishing them "would only matter to reconnect, which
/// `plan/12` §9c puts in Milestone 2". This is Milestone 2 -- **but that
/// comment framed the question slightly wrong, and it is worth saying how.**
///
/// A supervisor does *not* care whether the peer hung up or the socket errored:
/// both are a lost transport and both reconnect. What it cares about is telling
/// either of them from a **cancellation**, which the M1 loop also collapsed and
/// which that comment never mentions. The distinction that matters is
/// deliberate-stop versus lost-transport, and it cuts across the one M1 named.
///
/// # Why `PeerClosed` and `ReadFailed` are still separate
///
/// [`Self::warrants_reconnect`] treats them identically, so they could be one
/// variant. They are two because **a log has to be able to tell them apart**:
/// "the server hung up" and "the socket errored" are different facts about a
/// session, and a transcript that says only "disconnected" is unreadable at
/// exactly the moment someone is working out why a session flapped.
///
/// That is the same argument [`Origin::Script`](crate::Origin::Script) makes --
/// a separate variant "not because it queues differently but because a log has
/// to be able to tell them apart."
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// The cancel token fired: a deliberate stop.
    ///
    /// The player quit, a test ended, or `plan/16` §5b's orderly shutdown ran.
    /// **Never reconnects** -- this is the only variant that does not.
    Cancelled,
    /// `read` returned `Ok(0)`: the peer hung up, or a recording ran out.
    PeerClosed,
    /// A read failed. The socket is gone.
    ReadFailed,
    /// A write failed mid-[`pump`](Self::pump). The command it failed on has
    /// already been answered [`Outcome::Dead`](crate::Outcome::Dead).
    WriteFailed,
}

impl EndReason {
    /// Whether a supervisor should open a new connection.
    ///
    /// **[`Self::Cancelled`] is the only `false`, and that is the whole rule.**
    /// A clean quit must not reconnect; everything else is a lost transport.
    ///
    /// A method rather than a `match` at the call site, because adding a
    /// variant must be a compile error *somewhere that decides this*. A
    /// wildcard arm in a supervisor would silently default a new reason to
    /// "reconnect", which is the wrong direction to be wrong in.
    #[must_use]
    pub const fn warrants_reconnect(self) -> bool {
        match self {
            Self::Cancelled => false,
            Self::PeerClosed | Self::ReadFailed | Self::WriteFailed => true,
        }
    }
}

impl<S: ByteSource> SessionActor<S> {
    /// Close the source and answer everyone still waiting.
    ///
    /// Everyone is answered [`Self::on_disconnect`] -- `Dead` for a plain
    /// session, `Disconnected` for a supervised one -- **except after a
    /// cancellation**, which is always `Dead`: a deliberate stop is not
    /// followed by a reconnect whoever owns the actor
    /// ([`EndReason::warrants_reconnect`]).
    pub(super) async fn shutdown(&mut self, reason: EndReason) {
        // Idempotent by the trait's contract, which is why this is safe on
        // every one of the three exit paths.
        let _ = self.source.shutdown().await;
        let outcome = if reason.warrants_reconnect() {
            self.on_disconnect.clone()
        } else {
            crate::command::Outcome::Dead
        };
        self.queue.answer_all_waiters(&outcome);
        self.transition(State::Closed);
        // Flush LAST, after the Closed transition has been logged, so the file
        // records its own end. Buffered writers otherwise lose the final lines
        // -- which are the ones that say why a session stopped.
        if let Some(sink) = self.sink.as_mut() {
            let _ = sink.flush();
        }
    }

    pub(super) fn transition(&mut self, next: State) {
        self.lifecycle = next;
        self.log(&format!("lifecycle {next:?}"));
        let _ = self.events.send(Event::StateChanged(next));
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
}

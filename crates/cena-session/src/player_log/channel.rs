//! The record and the bounded channel: **`plan/25` step 1**.
//!
//! Split out of `mod.rs` when [`writer`](super::writer) arrived and made this
//! module a facade -- Rule 4.4: a `mod.rs` re-exports and wires, it does not
//! implement. The module doc beside it records WHY the channel is bounded and
//! why a drop is counted; this is the code that does it.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::lifecycle::{Generation, SessionId};

/// How many lines may wait for the writer before one is dropped.
///
/// **Sized against the login burst, which is the worst case measured.** The
/// first live session (2026-09-18) dropped 99 events from the broadcast ring
/// during login, and that burst is the densest text this client sees: room,
/// exits, worn inventory, spell lists and the services table arriving at once.
///
/// 4096 is that with two orders of magnitude of headroom, which is the right
/// shape for a buffer whose consumer does one `write` per batch. A number
/// chosen to make the drop path unreachable in normal play, not measured to be
/// exactly sufficient -- and the counter exists precisely because "unreachable"
/// is a claim rather than a guarantee.
pub const CAPACITY: usize = 4096;

/// One line on its way to disk.
///
/// Deliberately flat: a timestamp, where it came from, and what it said. No
/// styling, no runs, no frame. See the module doc on why this is not
/// `cena-ui`'s `StoryLine`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogLine {
    /// Wall clock at the moment the line was handed over, `HH:MM:SS.mmm`.
    ///
    /// **One stamp, and the date is not in it** (author, 2026-09-21: *"you
    /// talking about two timestamps on every line? that's a bit much"*). The
    /// wire log settled the same question first, and `sink/config.rs:175` gives
    /// the reason: the date *"is already here"* -- in the filename -- *"and
    /// repeating it on 30,000 lines is waste"*.
    ///
    /// Same format as the wire log's, so two logs of one session line up by
    /// eye. That is the argument for keeping both.
    ///
    /// **Not** the game's clock. `GameState::game_time_now` is the right thing
    /// for a behavior to reason with; a person reading their own history wants
    /// to know when they were at the keyboard.
    pub at: String,
    /// The window the line belongs to: `main`, `thoughts`, `inv`, `death`.
    ///
    /// Kept verbatim rather than typed. The set is the game's and it grows;
    /// `streams.rs` already treats stream ids as open for that reason, and a
    /// log that refused an unfamiliar stream would lose exactly the traffic
    /// worth keeping.
    pub stream: String,
    /// What the line said, assembled and entity-decoded, with no markup.
    pub text: String,
    /// Which session, for a multi-session process.
    pub session: SessionId,
    /// Which connection within that session.
    ///
    /// A reconnect is a full re-login, so a reader seeing this change knows the
    /// character was gone and came back. `LineAssembler` resets on the same
    /// boundary, so no line ever spans two generations.
    pub generation: Generation,
}

/// The session's end of the player log.
///
/// Cheap to clone; every clone shares one channel and one counter.
#[derive(Clone, Debug)]
pub struct PlayerLog {
    tx: tokio::sync::mpsc::Sender<LogLine>,
    dropped: Arc<AtomicU64>,
}

/// The writer's end: where lines arrive.
///
/// Held by step 2's writer task, which is the only thing that should own one.
#[derive(Debug)]
pub struct LogSink {
    rx: tokio::sync::mpsc::Receiver<LogLine>,
    dropped: Arc<AtomicU64>,
}

impl PlayerLog {
    /// A log and its sink, joined by a bounded channel of [`CAPACITY`].
    #[must_use]
    pub fn new() -> (Self, LogSink) {
        Self::with_capacity(CAPACITY)
    }

    /// [`Self::new`], with the capacity given rather than taken from the
    /// constant. For tests that need to reach the drop path without queueing
    /// four thousand lines.
    ///
    /// # Panics
    ///
    /// If `capacity` is zero: `tokio`'s bounded channel requires at least one
    /// slot, and a zero-capacity log would drop every line while claiming to
    /// record them.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> (Self, LogSink) {
        assert!(capacity > 0, "a log with no room records nothing");
        let (tx, rx) = tokio::sync::mpsc::channel(capacity);
        let dropped = Arc::new(AtomicU64::new(0));
        (
            Self {
                tx,
                dropped: Arc::clone(&dropped),
            },
            LogSink { rx, dropped },
        )
    }

    /// Hand one line to the writer, or count it dropped.
    ///
    /// **Never blocks, never awaits, never fails.** This is called from the
    /// actor, which owns the game socket; anything that can wait here can stall
    /// the character it is logging -- and, in a multi-session process, every
    /// other character too.
    ///
    /// Returns whether the line was accepted. Callers may ignore it: the drop
    /// is already counted, and the counter rather than the return value is what
    /// a frontend reads.
    // NOT `#[must_use]`, though clippy::pedantic asks for it. The drop is
    // already counted by the time this returns, so a caller ignoring the bool
    // loses nothing -- and the actor, which is the main caller, has nothing
    // useful to do with it. The counter is the reporting channel.
    #[allow(clippy::must_use_candidate)]
    pub fn record(&self, line: LogLine) -> bool {
        if self.tx.try_send(line).is_ok() {
            return true;
        }
        // Both failure modes are a loss the reader must be able to see: a full
        // channel means the writer is behind, and a closed one means it is
        // gone. Neither is worth distinguishing HERE -- what a reader needs is
        // "your history has a hole in it", and the count says that either way.
        self.dropped.fetch_add(1, Ordering::Relaxed);
        false
    }

    /// How many lines this log has failed to record.
    ///
    /// **Non-zero means the archive has a hole**, and a frontend should say so.
    /// The alternative -- a log that quietly skips lines under load -- produces
    /// a file that reads as complete and is not, which is the one outcome
    /// `plan/25` §4 exists to prevent.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl LogSink {
    /// The next line, or `None` once every [`PlayerLog`] is gone.
    pub async fn recv(&mut self) -> Option<LogLine> {
        self.rx.recv().await
    }

    /// The next line if one is already waiting, without awaiting.
    ///
    /// For tests that assert a line arrived, and for a writer draining what is
    /// queued before it flushes. Returns `None` both when the channel is empty
    /// and when it has ended -- a caller that needs to tell those apart wants
    /// [`Self::recv`].
    pub fn try_recv(&mut self) -> Option<LogLine> {
        self.rx.try_recv().ok()
    }

    /// The shared drop count, so the writer can report it alongside its own
    /// failures. Step 2 adds write errors to this same counter: from a
    /// reader's side, a line lost to a full channel and one lost to a full
    /// disk are the same hole.
    #[must_use]
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Count a line lost after it left the channel.
    ///
    /// For step 2's writer, whose failures are the other half of what
    /// [`PlayerLog::dropped`] reports.
    pub fn note_dropped(&self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }
}

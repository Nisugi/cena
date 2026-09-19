//! [`AnsweringSource`]: a byte source that answers what is written to it.
//!
//! # Why this is not [`ReplaySource`](crate::replay::ReplaySource) with a flag
//!
//! The two answer different questions and have opposite end-of-stream
//! behaviour, which is the whole reason they are separate types rather than
//! one type with a mode field (Rule -1's "no config option with one value"
//! cuts both ways -- a field with two values that select two unrelated
//! behaviours is two types wearing a trench coat).
//!
//! - `ReplaySource` is a **transcript**. It hands back exactly the chunks a
//!   real session read, in order, and then **ends the stream**. Criterion 7
//!   ("the whole session is recorded and replays deterministically") is about
//!   reproducing a past session, and a transcript that never ended would never
//!   finish replaying.
//! - `AnsweringSource` is a **stand-in for the game**. It emits its canned
//!   reply when a command is written and otherwise **stays pending forever**,
//!   because a real game does not hang up when it has nothing to say. That is
//!   what criteria 3, 4 and 5 need: a session that is still alive in the
//!   middle of a behavior, so there is a midpoint to interleave into and
//!   something to stop.
//!
//! An `AnsweringSource` that returned `Ok(0)` when idle would end the session
//! before a behavior's second iteration, and every cancellation test built on
//! it would pass vacuously -- the behavior would already be over.
//!
//! # Why the reply is keyed to a write
//!
//! `plan/12` §4.4's attribution rule is temporal: "a round trip owns the frame
//! stream from the moment its bytes are written until its terminator". A
//! source that emitted its prompt on a timer instead would close windows that
//! no command opened, which is the mis-attribution that section exists to
//! prevent -- and a test built on it would be testing the timer.

use crate::bytes::ByteSource;
use std::io;
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::Notify;

/// What an [`AnsweringSource`] saw and said. Shared, so a test can read it
/// while the actor owns the source.
///
/// This is **not** a process global (Rule 5.2, `plan/05:400-408`): it is an
/// owned `Arc` handed to one source, with no `static` behind it, so N of these
/// coexist in one process without seeing each other.
#[derive(Debug, Default)]
pub struct Transcript {
    /// Every message written, in order, newline included.
    pub written: Vec<Vec<u8>>,
    /// Whether the source was shut down. Criterion 6 reads this.
    pub shutdown: bool,
    /// While true, a write is recorded but **no reply is queued**.
    ///
    /// This is what lets a test hold a round-trip window OPEN. Criterion 5 is
    /// about a command typed *mid-behavior*, and "mid" means the behavior's
    /// own command is on the wire with its terminator still to come -- not
    /// that the behavior happens to be parked between iterations. A test
    /// without this switch can only reach the second state, and would pass on
    /// an implementation that preempted a genuinely in-flight command.
    ///
    /// VERIFIED necessary: the interleave test was written without it, and
    /// the falsification (manual admission interrupts the in-flight command,
    /// which is the superseded plan/12 4.1 design) left it GREEN.
    hold_replies: bool,
    /// Replies withheld while held, owed to the reader.
    owed: Vec<Vec<u8>>,
    /// How many owed replies may be delivered despite the hold.
    ///
    /// `release_replies` lifts the hold entirely, which delivers everything at
    /// once and makes "two responses arriving in order" indistinguishable from
    /// "two arriving together". That distinction is the subject of review
    /// SE-5, so a test needs to let exactly one through and check what
    /// happened before the next.
    release_budget: usize,
    /// While true, the next `read` that finds nothing pending returns `Ok(0)`
    /// instead of parking: the peer has hung up.
    ///
    /// Added for `plan/16` §5b's orderly shutdown, which needs a source that
    /// closes **because it was asked to**. A quit sent to a server that never
    /// EOFs is the timeout case; a quit to one that does is the acknowledged
    /// case, and without this switch only the first is reachable.
    ///
    /// **Not a fourth `ByteSource`.** Rule -1 forbids a trait implementor that
    /// exists to vary one behaviour, and hanging up is something this double
    /// already almost does -- it owns the read side and decides what a read
    /// sees. Note the ordering: pending bytes drain FIRST, so a server's
    /// goodbye text is still delivered before the close.
    hung_up: bool,
}

/// A handle to one [`AnsweringSource`]'s transcript and its hold switch.
///
/// Holds the `Notify` as well as the data, because a parked `read` has to be
/// **woken** when replies are released: without it the reader sits in
/// `future::pending` forever and the released bytes never arrive. That was a
/// live bug in the first version of this type and it presented as a hang, not
/// a wrong answer.
#[derive(Clone, Debug)]
pub struct TranscriptHandle {
    inner: Arc<Mutex<Transcript>>,
    wake: Arc<Notify>,
}

impl TranscriptHandle {
    /// Read the transcript under its lock.
    ///
    /// A poisoned lock means a thread panicked while holding it, and this
    /// crate denies `expect_used` (`Cargo.toml`, `plan/06` §1.5), so the
    /// poison is recovered from rather than re-panicked on. Every caller here
    /// only reads or sets a flag; there is no invariant a panicking writer
    /// could have left half-built.
    fn with<R>(&self, f: impl FnOnce(&mut Transcript) -> R) -> R {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        f(&mut guard)
    }

    /// Everything written so far, as lines with their newline trimmed.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        self.with(|t| {
            t.written
                .iter()
                .map(|m| String::from_utf8_lossy(m).trim_end().to_owned())
                .collect()
        })
    }

    /// How many messages have been written.
    #[must_use]
    pub fn written_count(&self) -> usize {
        self.with(|t| t.written.len())
    }

    /// Whether the source was shut down. Criterion 6 reads this.
    #[must_use]
    pub fn is_shutdown(&self) -> bool {
        self.with(|t| t.shutdown)
    }

    /// Stop answering: a command written from now on leaves its window OPEN.
    pub fn hold_replies(&self) {
        self.with(|t| t.hold_replies = true);
    }

    /// Answer again, delivering everything withheld, and wake the reader.
    pub fn release_replies(&self) {
        self.with(|t| t.hold_replies = false);
        self.wake.notify_waiters();
    }

    /// Deliver **one** withheld reply, keeping the hold on the rest.
    ///
    /// `release_replies` delivers everything at once, which makes two
    /// responses arriving in order indistinguishable from two arriving
    /// together. That distinction is the whole subject of review SE-5: a
    /// `send_now` prompt must not close the window belonging to a later
    /// command, and with every reply released simultaneously a test cannot
    /// tell a correct implementation from the defect.
    ///
    /// `owed` is already a queue, so one reply is the front of it.
    pub fn release_one(&self) {
        self.with(|t| t.release_budget = t.release_budget.saturating_add(1));
        self.wake.notify_waiters();
    }

    /// Hang up: the next read that runs out of bytes returns `Ok(0)`.
    ///
    /// Wakes a parked reader, because a `read` already waiting must learn the
    /// peer is gone rather than sitting in `notified()` forever. That is the
    /// same bug `TranscriptHandle`'s own docs record for `release_replies`.
    pub fn hang_up(&self) {
        self.with(|t| t.hung_up = true);
        self.wake.notify_waiters();
    }
}

/// A byte source that replies to each command and never hangs up on its own.
#[derive(Debug)]
pub struct AnsweringSource {
    /// Emitted once per write.
    reply: Vec<u8>,
    /// Bytes owed to the reader, from replies not yet drained.
    pending: Vec<u8>,
    transcript: TranscriptHandle,
}

impl AnsweringSource {
    /// Answer every written command with these bytes.
    ///
    /// The canonical `reply` is a prompt, because `plan/12` §4.4 makes
    /// `Frame::Prompt` the terminator that closes a round-trip window.
    #[must_use]
    pub fn new(reply: &[u8]) -> (Self, TranscriptHandle) {
        let transcript = TranscriptHandle {
            inner: Arc::new(Mutex::new(Transcript::default())),
            wake: Arc::new(Notify::new()),
        };
        (
            Self {
                reply: reply.to_vec(),
                pending: Vec::new(),
                transcript: transcript.clone(),
            },
            transcript,
        )
    }
}

impl ByteSource for AnsweringSource {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            // Arm BEFORE checking, so a release between the check and the wait
            // is not missed. `Notify::notified()` registers on creation.
            let woken = self.transcript.wake.notified();
            let released = self.transcript.with(|t| {
                if !t.hold_replies {
                    return t.owed.drain(..).collect::<Vec<_>>();
                }
                // Held -- but `release_one` may have granted a budget, which
                // delivers exactly that many from the front of the queue.
                let take = t.release_budget.min(t.owed.len());
                t.release_budget -= take;
                t.owed.drain(..take).collect::<Vec<_>>()
            });
            for reply in released {
                self.pending.extend_from_slice(&reply);
            }
            if !self.pending.is_empty() {
                break;
            }
            // A peer that has hung up reports it, and does so only once the
            // pending bytes above are drained -- a goodbye message arrives
            // before the close, as it does on a real socket.
            //
            // **A shut-down source is at end of stream too**, for the same
            // reason `ReplaySource::read` says so (`replay.rs:72-76`): without
            // it, a session that closed its own source parks here forever
            // instead of ending. Review finding PL-6 -- this source enforced
            // neither half of `shutdown`, so a bug that kept reading or writing
            // after close passed under it while failing against the other two
            // sources.
            if self.transcript.with(|t| t.hung_up || t.shutdown) {
                return Ok(0);
            }
            // Otherwise waits FOREVER until something is written or released,
            // never `Ok(0)`. A game with nothing to say does not hang up, and
            // a source that did would end the session in the middle of the
            // behavior every cancellation test is about. The caller's
            // `tokio::time::timeout` is what bounds this wait (`plan/12`
            // §5.5: every wait has a deadline).
            woken.await;
        }
        let n = self.pending.len().min(buf.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        Ok(n)
    }

    async fn write_all(&mut self, message: &[u8]) -> io::Result<()> {
        // Writing to a closed socket is an error on a real one and on
        // `ReplaySource` (`replay.rs:89-94`). Silently succeeding here let a
        // write-after-shutdown bug in the session pass every test that used
        // this source -- PL-6.
        if self.transcript.with(|t| t.shutdown) {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "write to a shut-down answering source",
            ));
        }
        let reply = self.reply.clone();
        let held = self.transcript.with(|t| {
            t.written.push(message.to_vec());
            if t.hold_replies {
                // The window stays open: the command is on the wire and its
                // terminator has not arrived.
                t.owed.push(reply);
                true
            } else {
                false
            }
        });
        if held {
            return Ok(());
        }
        self.pending.extend_from_slice(&self.reply);
        self.transcript.wake.notify_waiters();
        Ok(())
    }

    async fn shutdown(&mut self) -> io::Result<()> {
        self.transcript.with(|t| t.shutdown = true);
        Ok(())
    }
}

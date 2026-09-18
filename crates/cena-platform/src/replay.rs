//! [`ReplaySource`]: the second [`ByteSource`] implementor, and the one
//! criterion 7 is about.
//!
//! `plan/12:465`: "The whole session is recorded and replays deterministically
//! in a test, **with no network**." This type touches no socket, no DNS and no
//! TLS, which is the literal content of "with no network" -- it cannot reach
//! one, rather than being asked not to.
//!
//! # Chunk boundaries are the point
//!
//! `chunks` is a list of the byte slices each live `read` returned, not one
//! concatenated buffer. Replaying the *same* split exercises
//! `Parser::push_bytes`'s pending-fragment path with a boundary that really
//! occurred. Re-chunking would quietly test a different thing than what ran.
//!
//! # What it does with writes
//!
//! It keeps them. A replay has no peer to answer, so a write cannot change
//! what comes back -- the recording already fixed that. Keeping them is what
//! lets a test assert the session sent what the behavior asked it to, which is
//! how criterion 3's "goes through the same queue" is observed without a game.

use crate::bytes::ByteSource;
use std::io;

/// A [`ByteSource`] that reads from a recording instead of a socket.
#[derive(Debug, Default)]
pub struct ReplaySource {
    chunks: std::collections::VecDeque<Vec<u8>>,
    /// The tail of a chunk that did not fit the caller's buffer.
    partial: Vec<u8>,
    written: Vec<Vec<u8>>,
    shutdown: bool,
}

impl ReplaySource {
    /// Replay these chunks, in order, one per `read` (subject to the caller's
    /// buffer size).
    #[must_use]
    pub fn new(chunks: Vec<Vec<u8>>) -> Self {
        Self {
            chunks: chunks.into(),
            partial: Vec::new(),
            written: Vec::new(),
            shutdown: false,
        }
    }

    /// Replay one whole byte string as a single chunk. The convenience the
    /// fixtures want: a `.xml` file is one `read` as far as a test cares.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self::new(vec![bytes.to_vec()])
    }

    /// Everything the session wrote, in order.
    #[must_use]
    pub fn written(&self) -> &[Vec<u8>] {
        &self.written
    }

    /// Whether [`ByteSource::shutdown`] has been called. Criterion 6 asserts
    /// on this: "no leaked sockets" is, for a replay, "the source was closed".
    #[must_use]
    pub fn is_shutdown(&self) -> bool {
        self.shutdown
    }
}

impl ByteSource for ReplaySource {
    async fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // A shut-down source is at end of stream. Without this, a cancelled
        // session that closed its source could still be fed bytes.
        if self.shutdown {
            return Ok(0);
        }
        if self.partial.is_empty() {
            let Some(next) = self.chunks.pop_front() else {
                return Ok(0);
            };
            self.partial = next;
        }
        let n = self.partial.len().min(buf.len());
        buf[..n].copy_from_slice(&self.partial[..n]);
        self.partial.drain(..n);
        Ok(n)
    }

    async fn write_all(&mut self, message: &[u8]) -> io::Result<()> {
        if self.shutdown {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "write to a shut-down replay source",
            ));
        }
        self.written.push(message.to_vec());
        Ok(())
    }

    async fn shutdown(&mut self) -> io::Result<()> {
        // Idempotent by construction: setting a flag twice is setting it once.
        self.shutdown = true;
        Ok(())
    }
}

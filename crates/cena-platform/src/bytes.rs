//! The byte source: where a session's wire bytes come from.
//!
//! # Why this is a trait, and why it is not one implementor
//!
//! `plan/05` Rule -1 forbids a trait with one implementor. This one has two,
//! both in this crate, both required by Milestone 1: [`LiveSource`](crate::live::LiveSource)
//! for criterion 1 and [`ReplaySource`](crate::replay::ReplaySource) for
//! criterion 7 ("the whole session is recorded and replays deterministically
//! in a test, **with no network**", `plan/12:465`). The second implementor is
//! not speculative future-proofing; it is in the same milestone as the first,
//! and criterion 7 cannot be met without it.
//!
//! # Why it yields BYTES, not lines and not frames
//!
//! **Lines would duplicate work that already exists.**
//! [`Parser::push_bytes`](cena_protocol::Parser::push_bytes) is the read
//! boundary: it buffers until a newline, so a tag split across two TCP reads
//! is rejoined before parsing. Its own doc says so, and the split-at-every-
//! byte-offset test lives beside it. A line-yielding transport would
//! reimplement that reassembly one layer down, in a place no test drives at
//! every offset.
//!
//! **Frames would invert the layers.** `cena-protocol` depends on this crate
//! (`crates/cena-protocol/Cargo.toml:8`). A transport that yielded `Frame`
//! would need `Parser` *below* the crate `Parser` lives in.
//!
//! So: bytes. Chunk boundaries are whatever the source gives, which is exactly
//! what makes replaying a recording exercise the partial-line path for real.
//!
//! # Why not `AsyncRead + AsyncWrite`
//!
//! Considered and rejected. A replay source would have to impersonate a
//! socket, and `AsyncWrite::shutdown` conflates "the recording ended" with
//! "the peer hung up" -- which is the distinction criterion 6 tests.
//!
//! # Deadlines are the caller's
//!
//! `plan/12` §5.5 requires every wait to have a deadline. That deadline is
//! applied by the session with `tokio::time::timeout`, not taken as a
//! parameter here: a timeout belongs to the *policy* that waits, not to the
//! pipe. Passing one in would be a config option with one caller.

use std::future::Future;
use std::io;

/// A source of raw wire bytes for one session.
///
/// `Send` because `plan/12` §5.5 runs each session as one supervised task, and
/// a task's future must cross threads on a multi-thread runtime.
pub trait ByteSource: Send {
    /// Read the next available chunk into `buf`.
    ///
    /// `Ok(0)` means end of stream: the peer hung up, or the recording ran
    /// out. It is not an error -- criterion 6 ("disconnect is clean: task
    /// ends, no leaked sockets, no panic") is the *normal* end of a session,
    /// so it must not arrive as an `Err`.
    fn read(&mut self, buf: &mut [u8]) -> impl Future<Output = io::Result<usize>> + Send;

    /// Write one whole message, as **one** write.
    ///
    /// The single-write rule is ported verbatim from the login spike
    /// (`spike/eaccess-spike/src/main.rs:110-122`) and from Vellum before it
    /// (`reference/VellumFE/src/network.rs:920-931`): "Two `write_all` calls
    /// can emit two TLS records, and this server does not tolerate a command
    /// split across records." Ruby's `IO#puts` is inherently one write, so
    /// Lich never had to think about it -- a genuine Rust-vs-Ruby porting
    /// hazard (`plan/10` §10.3a), and the reason this method takes the
    /// finished message rather than offering a `write` plus a `write_newline`.
    fn write_all(&mut self, message: &[u8]) -> impl Future<Output = io::Result<()>> + Send;

    /// Close the source. **Idempotent**, because criterion 6 is "no leaked
    /// sockets" and the session shuts down on several paths -- cancellation,
    /// end of stream, and read error -- which must not be mutually exclusive.
    fn shutdown(&mut self) -> impl Future<Output = io::Result<()>> + Send;
}

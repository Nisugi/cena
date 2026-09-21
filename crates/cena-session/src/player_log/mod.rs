//! The player log's record and its channel: **`plan/25` step 1**.
//!
//! The second of the two logs `sink/mod.rs:5-22` named in 2026-09-18 -- *"a
//! user log of the processed text"* -- carrying display text as a player read
//! it, where the wire log carries bytes.
//!
//! # This file writes nothing
//!
//! It is the record, the bounded channel and the drop counter. The writer is
//! step 2. The split is deliberate: `plan/25` §4's three properties are about
//! what happens **between** the actor and the disk, and every one of them is
//! testable with no file handle in sight.
//!
//! # Why bounded, and why a drop is counted
//!
//! Lichborne ships this feature and its read path is scrupulously async, with
//! the reason in its own source: *"main owns every session's socket, so a
//! synchronous multi-file scan freezes EVERY connected character at once."*
//! Then its write path is `fs.appendFileSync` on that same thread with the
//! error swallowed to `console.error` -- so a slow disk stalls every session
//! and the dropped records are reported to nobody.
//!
//! Multi-session is this project's headline feature, so the cost of that
//! mistake is higher here than there. Hence:
//!
//! 1. **The actor never blocks on the disk.** [`PlayerLog::record`] is
//!    non-blocking. A full channel drops; it never waits.
//! 2. **A drop is counted**, and the count is readable
//!    ([`PlayerLog::dropped`]). §5.2: a gap nobody is told about is
//!    indistinguishable from no gap.
//! 3. **Dropping is the only lossy path**, and it is visible. A write error is
//!    step 2's problem and reaches the same counter.
//!
//! # The record does not carry a view type
//!
//! `cena-ui` holds `StoryLine`, which is the assembled line a frontend draws.
//! This crate cannot see it: `cena-ui` sits **beside** `cena-session` in the
//! graph, not below it, and `layering.rs:25-29` records that the reverse edge
//! is a violation someone nearly introduced.
//!
//! That constraint produces the better design anyway. A log that outlives the
//! client by years should not be shaped by a type that exists to serve one
//! frontend's rendering. [`LogLine`] carries the text and the stream, which is
//! what a reader in 2030 needs, and no styling at all.

pub mod channel;
pub mod feed;
pub mod tap;
pub mod writer;

pub use channel::{CAPACITY, LogLine, LogSink, PlayerLog};
pub use feed::{Capture, Feed, LogSettings};
pub use tap::Tap;

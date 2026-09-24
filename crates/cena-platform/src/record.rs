//! The recorder: what a session did, in enough detail to run it again.
//!
//! Criterion 7 (`plan/12:551`) is "the whole session is **recorded and replays
//! deterministically** in a test, with no network".
//!
//! # What is recorded: BYTES, not frames
//!
//! Recording frames would make the replay test tautological. It would feed the
//! parser's own output back through the parser, so a parser regression would
//! change the recording and the replay in lockstep and the test would stay
//! green. Recording the bytes means the replay re-parses, and a parser change
//! that alters the frame stream is visible.
//!
//! # `seq`, not a timestamp
//!
//! A wall clock is the single easiest way to make a replay non-deterministic:
//! two runs of the same recording produce two different files. `seq` is a
//! monotonic counter seeded at 0, so a replay of a replay is byte-identical.
//!
//! # Chunk boundaries are preserved deliberately
//!
//! An `Inbound` entry holds exactly the bytes one `read` returned, not a
//! re-chunked stream. Replaying the same split that happened live is what
//! drives `Parser::push_bytes`'s partial-line path with a real boundary rather
//! than an invented one -- the failure `push_bytes` exists to prevent.
//!
//! # On `bytes: Vec<u8>` and Rule 2.1
//!
//! Rule 2.1 (`plan/05:270-274`) is "nothing **above** `cena-protocol` ever
//! sees a raw byte". Its architecture test
//! (`crates/cena-arch-tests/tests/file_rules.rs:358-364`) scans only
//! `crates/cena-protocol/src/`, so this field is outside it. That is the
//! correct scope and not a loophole: `cena-platform` is *below*
//! `cena-protocol` (`plan/12:75-77`), and a transport that could not name a
//! byte could not be a transport. Nothing above protocol ever receives one of
//! these -- `cena-session` sees `Frame`s, and the recorder it owns is fed by
//! the same code that feeds the parser.

/// One thing that crossed the session boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordedEvent {
    /// Bytes read from the wire, exactly as one `read` returned them.
    Inbound {
        /// Position in the recorder's counter, shared by both directions and
        /// seeded at 0; not a timestamp.
        seq: u64,
        /// The chunk, unsplit and unjoined.
        bytes: Vec<u8>,
    },
    /// Bytes written to the wire, exactly as one `write_all` sent them.
    Outbound {
        /// Position in the recorder's counter, shared by both directions and
        /// seeded at 0; not a timestamp.
        seq: u64,
        /// The message as written in one call, its trailing newline included.
        bytes: Vec<u8>,
    },
}

impl RecordedEvent {
    /// The bytes, whichever direction this event went.
    ///
    /// For the size accounting the byte bound needs; a caller that cares about
    /// direction should match instead.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::Inbound { bytes, .. } | Self::Outbound { bytes, .. } => bytes,
        }
    }
}

/// How many bytes of wire traffic a recorder keeps before dropping the oldest.
///
/// # Why this is bounded at all
///
/// It was not, and that is a leak on the production path: an append-only `Vec`
/// that **copies every chunk**, carried across every generation, in a client
/// designed for 3-25 simultaneous characters. A long session held every byte it
/// had ever read, in memory, for the life of the process.
///
/// The `.bytes` sink has already written all of it to disk, so the in-memory
/// copy buys nothing a file cannot answer -- which is what makes bounding it
/// safe rather than a trade.
///
/// # Why 8 MiB, and what it costs
///
/// MEASURED: the author's live sessions log 64-80 KB of inbound traffic per few
/// minutes, so 8 MiB is hours of play per session and 200 MiB across a full
/// 25-character load. That is a ceiling rather than a target: sessions that stay
/// under it behave exactly as before, and criterion 7's replays are fixtures
/// measured in kilobytes.
///
/// What it costs is that a session **longer than the bound cannot be replayed
/// from its start** -- [`Recorder::events`] no longer begins at `seq` 0. That is
/// detectable rather than silent: `seq` is independent of the `Vec`, so a
/// consumer can see the first surviving sequence number and know what it is
/// missing, and [`Recorder::dropped`] says how many.
pub const MAX_RECORDED_BYTES: usize = 8 * 1024 * 1024;

/// Bounded log of one session's wire traffic, oldest dropped first.
///
/// Held by value inside the session actor -- Rule 5.2 (`plan/05:400-408`), no
/// process globals -- so 25 sessions record 25 independent transcripts.
#[derive(Debug, Default)]
pub struct Recorder {
    events: std::collections::VecDeque<RecordedEvent>,
    next_seq: u64,
    /// Bytes currently held, so trimming costs no rescan.
    held_bytes: usize,
    /// How many events were dropped to stay under [`MAX_RECORDED_BYTES`].
    dropped: u64,
    /// Lifetime count of outbound writes, unaffected by trimming.
    outbound_count: u64,
    /// Lifetime count of inbound reads, unaffected by trimming.
    inbound_count: u64,
}

impl Recorder {
    /// A recorder with an empty log and `seq` at 0.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record bytes that arrived from the wire.
    pub fn inbound(&mut self, bytes: &[u8]) {
        let seq = self.take_seq();
        self.held_bytes += bytes.len();
        self.inbound_count += 1;
        self.events.push_back(RecordedEvent::Inbound {
            seq,
            bytes: bytes.to_vec(),
        });
        self.trim();
    }

    /// Record bytes that were sent to the wire.
    pub fn outbound(&mut self, bytes: &[u8]) {
        let seq = self.take_seq();
        self.held_bytes += bytes.len();
        self.outbound_count += 1;
        self.events.push_back(RecordedEvent::Outbound {
            seq,
            bytes: bytes.to_vec(),
        });
        self.trim();
    }

    /// Drop the oldest events until the log fits [`MAX_RECORDED_BYTES`].
    ///
    /// **Never drops the last event**, however large: a recorder holding one
    /// 10 MiB chunk keeps it, because "the most recent thing that happened" is
    /// the one a caller is most likely to be about to read.
    fn trim(&mut self) {
        while self.held_bytes > MAX_RECORDED_BYTES && self.events.len() > 1 {
            let Some(oldest) = self.events.pop_front() else {
                break;
            };
            self.held_bytes = self.held_bytes.saturating_sub(oldest.bytes().len());
            self.dropped += 1;
        }
    }

    /// Everything still recorded, in the order it happened.
    ///
    /// **May not start at `seq` 0** on a session longer than
    /// [`MAX_RECORDED_BYTES`] -- see [`Self::dropped`]. The `.bytes` sink is the
    /// complete record; this is the recent window.
    /// Returns a **slice**, which is why this takes `&mut self`:
    /// `VecDeque::make_contiguous` is what lets a bounded ring keep the flat
    /// `&[RecordedEvent]` its callers already use. Returning an iterator instead
    /// churned every call site for no gain -- `to_vec`, `iter`, `is_empty` all
    /// stopped compiling -- and a recorder is read a handful of times per
    /// session, so one rotation costs nothing.
    pub fn events(&mut self) -> &[RecordedEvent] {
        self.events.make_contiguous()
    }

    /// How many events are still held. Does **not** need `&mut`, unlike
    /// [`Self::events`], so a caller that only wants the count (a log line, a
    /// report) is not forced to take a mutable borrow.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Whether anything is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// How many inbound reads this recorder has seen, **including any it has
    /// since dropped**.
    ///
    /// Counterpart to [`Self::outbound_count`], and for the same reason: the
    /// supervisor needs "did this connection RECEIVE anything" to tell a login
    /// that worked from one that opened a socket and died, and a bounded log
    /// cannot answer that by scanning.
    #[must_use]
    pub const fn inbound_count(&self) -> u64 {
        self.inbound_count
    }

    /// How many outbound writes this recorder has seen, **including any it has
    /// since dropped**.
    ///
    /// A counter rather than a scan. The supervisor uses this to decide whether a
    /// connection was "attended", and it used to `filter().count()` the whole log
    /// twice per connection -- O(session length) on a hot path, and now simply
    /// wrong, because a bounded log forgets the early writes it was counting.
    #[must_use]
    pub const fn outbound_count(&self) -> u64 {
        self.outbound_count
    }

    /// How many events were dropped to stay under [`MAX_RECORDED_BYTES`].
    ///
    /// Nonzero means [`Self::events`] is a window rather than the whole session,
    /// which a replay built from it needs to know. Zero for every session short
    /// enough to fit, which is every test fixture.
    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Bytes currently held. For a diagnostic, and for the bound's own test.
    #[must_use]
    pub const fn held_bytes(&self) -> usize {
        self.held_bytes
    }

    /// The inbound chunks alone, in order, ready to build a
    /// [`ReplaySource`](crate::replay::ReplaySource) that reproduces this
    /// session's reads **with the same chunk boundaries**.
    #[must_use]
    pub fn inbound_chunks(&self) -> Vec<Vec<u8>> {
        self.events
            .iter()
            .filter_map(|e| match e {
                RecordedEvent::Inbound { bytes, .. } => Some(bytes.clone()),
                RecordedEvent::Outbound { .. } => None,
            })
            .collect()
    }

    fn take_seq(&mut self) -> u64 {
        let seq = self.next_seq;
        self.next_seq += 1;
        seq
    }
}

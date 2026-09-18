//! The recorder: what a session did, in enough detail to run it again.
//!
//! Criterion 7 (`plan/12:465`) is "the whole session is **recorded and replays
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
//! `cena-protocol` (`plan/12:74-76`), and a transport that could not name a
//! byte could not be a transport. Nothing above protocol ever receives one of
//! these -- `cena-session` sees `Frame`s, and the recorder it owns is fed by
//! the same code that feeds the parser.

/// One thing that crossed the session boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecordedEvent {
    /// Bytes read from the wire, exactly as one `read` returned them.
    Inbound { seq: u64, bytes: Vec<u8> },
    /// Bytes written to the wire, exactly as one `write_all` sent them.
    Outbound { seq: u64, bytes: Vec<u8> },
}

/// Append-only log of one session's wire traffic.
///
/// Held by value inside the session actor -- Rule 5.2 (`plan/05:400-408`), no
/// process globals -- so 25 sessions record 25 independent transcripts.
#[derive(Debug, Default)]
pub struct Recorder {
    events: Vec<RecordedEvent>,
    next_seq: u64,
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
        self.events.push(RecordedEvent::Inbound {
            seq,
            bytes: bytes.to_vec(),
        });
    }

    /// Record bytes that were sent to the wire.
    pub fn outbound(&mut self, bytes: &[u8]) {
        let seq = self.take_seq();
        self.events.push(RecordedEvent::Outbound {
            seq,
            bytes: bytes.to_vec(),
        });
    }

    /// Everything recorded, in the order it happened.
    #[must_use]
    pub fn events(&self) -> &[RecordedEvent] {
        &self.events
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

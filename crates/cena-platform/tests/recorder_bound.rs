//! The recorder is **bounded**, and the counters survive the bound.
//!
//! # Why it needed bounding
//!
//! It was an append-only `Vec` that copies every chunk, carried across every
//! generation, in a client designed for 3-25 simultaneous characters. A long
//! session held every byte it had ever read, in memory, for the life of the
//! process -- while the `.bytes` sink had already written all of it to disk.
//!
//! Found by review as "unbounded on the production path", which is the right
//! framing: nothing in the test suite runs long enough to notice.

use cena_platform::{MAX_RECORDED_BYTES, RecordedEvent, Recorder};

/// One chunk big enough that a handful of them cross the bound.
fn chunk(byte: u8) -> Vec<u8> {
    vec![byte; 1024 * 1024]
}

#[test]
fn the_log_stops_growing_at_the_bound() {
    let mut recorder = Recorder::new();
    // Twice the bound, in 1 MiB chunks.
    for i in 0..(MAX_RECORDED_BYTES / (1024 * 1024) * 2) {
        recorder.inbound(&chunk(u8::try_from(i % 256).unwrap_or(0)));
    }

    assert!(
        recorder.held_bytes() <= MAX_RECORDED_BYTES,
        "the recorder held {} bytes against a bound of {MAX_RECORDED_BYTES}",
        recorder.held_bytes()
    );
    assert!(
        recorder.dropped() > 0,
        "and it must SAY that it dropped things -- a silent window would make a \
         replay built from it wrong in a way nothing could detect"
    );
}

/// **The oldest go first**, so what survives is the recent window.
#[test]
fn the_window_keeps_the_most_recent_events() {
    let mut recorder = Recorder::new();
    for i in 0..(MAX_RECORDED_BYTES / (1024 * 1024) + 4) {
        recorder.inbound(&chunk(u8::try_from(i).unwrap_or(0)));
    }

    let first_kept = recorder
        .events()
        .first()
        .map(|event| match event {
            RecordedEvent::Inbound { seq, .. } | RecordedEvent::Outbound { seq, .. } => *seq,
        })
        .expect("something must survive");
    assert!(
        first_kept > 0,
        "the first event should have been dropped, so the window no longer \
         starts at seq 0 -- which is the fact `dropped()` exists to report"
    );
}

/// **A single oversized event is kept**, because "the most recent thing that
/// happened" is what a caller is most likely to be about to read.
#[test]
fn one_event_larger_than_the_bound_is_not_dropped() {
    let mut recorder = Recorder::new();
    recorder.inbound(&vec![7u8; MAX_RECORDED_BYTES * 2]);

    assert_eq!(
        recorder.len(),
        1,
        "trimming must never empty the log completely"
    );
    assert_eq!(recorder.dropped(), 0);
}

/// **The counters are lifetime totals**, not derived from the surviving window.
///
/// This is the load-bearing one. The supervisor decides "was this connection
/// attended" and "did it work" by comparing these before and after a connection,
/// and a count derived from a bounded log would go DOWN as old events were
/// dropped -- making an attended session look unattended and stopping it.
#[test]
fn the_counters_are_unaffected_by_trimming() {
    let mut recorder = Recorder::new();
    let writes = 12u64;
    for i in 0..writes {
        recorder.outbound(&chunk(u8::try_from(i).unwrap_or(0)));
    }

    assert!(recorder.dropped() > 0, "precondition: trimming happened");
    assert_eq!(
        recorder.outbound_count(),
        writes,
        "the outbound count must be a LIFETIME total. Derived from the window it \
         would shrink as events were dropped, and the supervisor compares it \
         across a connection to decide whether anyone was there."
    );
    assert!(
        recorder.len() < usize::try_from(writes).unwrap_or(usize::MAX),
        "...while the window itself is smaller than the total, or this test is \
         not exercising the distinction"
    );
}

/// Inbound and outbound are counted separately, because the supervisor asks
/// different questions of each: outbound is "was anyone here", inbound is "did
/// this connection function".
#[test]
fn inbound_and_outbound_are_counted_apart() {
    let mut recorder = Recorder::new();
    recorder.inbound(b"from the game");
    recorder.inbound(b"more from the game");
    recorder.outbound(b"look\n");

    assert_eq!(recorder.inbound_count(), 2);
    assert_eq!(recorder.outbound_count(), 1);
}

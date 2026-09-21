//! `plan/25` step 1: the record, the bounded channel, and the drop counter.
//!
//! These test §4's three properties, all of which are about what happens
//! **between** the actor and the disk. No file is written here.
//!
//! The reason they matter is a working implementation that got it wrong.
//! Lichborne's `sessionLog.ts` is careful to keep its READ path off the thread
//! that owns every session's socket -- and then writes with `appendFileSync`
//! on that thread, swallowing the error to `console.error`. A slow disk stalls
//! every character and the lost lines are reported to nobody.

use cena_session::lifecycle::{Generation, SessionId};
use cena_session::{LogLine, PlayerLog};

fn line(text: &str) -> LogLine {
    LogLine {
        at: "12:34:56.789".to_owned(),
        stream: "main".to_owned(),
        text: text.to_owned(),
        session: SessionId::FIRST,
        generation: Generation::FIRST,
    }
}

#[test]
fn a_recorded_line_reaches_the_sink_unchanged() {
    // The ordinary path. Without this the drop tests below would pass over a
    // log that records nothing at all.
    let (log, mut sink) = PlayerLog::new();
    assert!(log.record(line("You see a rock.")));

    let got = sink.try_recv().expect("the line should be waiting");
    assert_eq!(got.text, "You see a rock.");
    assert_eq!(got.stream, "main");
    assert_eq!(got.at, "12:34:56.789");
    assert_eq!(log.dropped(), 0, "nothing was lost");
}

#[test]
fn a_full_channel_drops_rather_than_blocking() {
    // **PROPERTY 1.** The actor owns the game socket. A `record` that could
    // wait would let a slow disk stall the character being logged -- and every
    // other character in the process, which is the failure this project can
    // least afford.
    //
    // Capacity 2 so the drop path is reached in three lines rather than four
    // thousand. `with_capacity` exists for this.
    let (log, _sink) = PlayerLog::with_capacity(2);

    assert!(log.record(line("first")), "guard: the channel starts empty");
    assert!(log.record(line("second")), "guard: two fit");

    // The third has nowhere to go. It must return, not wait.
    //
    // VERIFIED by mutation: swapping `try_send` for `blocking_send` fails this
    // suite rather than hanging it, because a blocking send inside a runtime
    // panics. So property 1 is enforced here rather than merely asserted --
    // which is not what I expected when writing this, and is worth recording:
    // the first draft of this comment said a regression would hang the test.
    assert!(!log.record(line("third")), "the third should not fit");
}

#[test]
fn a_dropped_line_is_counted() {
    // **PROPERTY 2.** §5.2: a gap nobody is told about is indistinguishable
    // from no gap. A log that silently skips lines under load produces a file
    // that READS as complete and is not.
    let (log, _sink) = PlayerLog::with_capacity(1);

    assert!(log.record(line("kept")));
    assert_eq!(log.dropped(), 0, "guard: the first line was not dropped");

    assert!(!log.record(line("lost")));
    assert!(!log.record(line("also lost")));

    assert_eq!(log.dropped(), 2, "both losses are visible to a reader");
}

#[test]
fn a_closed_sink_counts_drops_rather_than_failing_silently() {
    // A writer that has gone away is a hole in the archive exactly as a full
    // channel is. From a reader's side the two are the same fact -- "your
    // history is missing lines" -- so they share one counter.
    let (log, sink) = PlayerLog::new();
    drop(sink);

    assert!(!log.record(line("nobody is listening")));
    assert_eq!(log.dropped(), 1);
}

#[test]
fn clones_share_one_counter() {
    // The actor and the supervisor may both hold one. A per-clone counter
    // would report a fraction of the loss, which is worse than reporting none:
    // a small number reads as "a little was lost" rather than "this count is
    // wrong".
    let (log, _sink) = PlayerLog::with_capacity(1);
    let other = log.clone();

    assert!(log.record(line("fills it")));
    assert!(!other.record(line("dropped by the clone")));

    assert_eq!(log.dropped(), 1, "the original sees the clone's drop");
    assert_eq!(other.dropped(), 1, "and the clone sees the same count");
}

#[test]
fn the_writers_own_failures_reach_the_same_counter() {
    // Step 2's writer calls `note_dropped` when a line is lost AFTER leaving
    // the channel -- a failed write, a full disk. One counter, because a
    // reader asking "is my history complete?" does not care which side lost it.
    let (log, sink) = PlayerLog::new();

    assert!(log.record(line("this one is fine")));
    sink.note_dropped();

    assert_eq!(sink.dropped(), 1);
    assert_eq!(
        log.dropped(),
        1,
        "a write failure must be visible from the session's side too"
    );
}

#[tokio::test]
async fn the_sink_ends_when_every_log_is_gone() {
    // The writer task's shutdown signal. Without this it would park on `recv`
    // forever and the process would not exit.
    let (log, mut sink) = PlayerLog::new();
    let clone = log.clone();

    log.record(line("last words"));
    drop(log);
    drop(clone);

    assert!(
        sink.recv().await.is_some(),
        "guard: the queued line arrives"
    );
    assert!(
        sink.recv().await.is_none(),
        "the sink should end once no sender remains"
    );
}

#[test]
#[should_panic(expected = "a log with no room records nothing")]
fn a_zero_capacity_log_is_refused() {
    // It would drop every line while reporting itself as a working log --
    // the exact "quietly claims complete history" outcome §4 forbids.
    let _ = PlayerLog::with_capacity(0);
}

//! Phase 2: the author's sequence, exactly.
//!
//! > **AUTHOR, 2026-09-18:** *"send 4 looks at once, wait 0.5 seconds, send 4
//! > looks, wait 0.3 seconds, send 4 looks, sleep 0.2 seconds, send 4 looks,
//! > sleep 0.1 seconds, send 4 looks"*
//!
//! One pass. Five groups of four, with the pause **shrinking** between them:
//!
//! ```text
//! 4 looks | 0.5s | 4 looks | 0.3s | 4 looks | 0.2s | 4 looks | 0.1s | 4 looks
//! ```
//!
//! Descending is what makes it one experiment rather than five: pressure rises
//! as it goes, so the group where refusals start is where the server stopped
//! keeping up. Nothing is repeated and nothing resets in between -- whatever
//! backlog builds is carried forward, which is the thing being measured.
//!
//! # Two earlier versions of this file measured nothing
//!
//! Kept written down because both *looked* like measurements.
//!
//! **Version 1 could not fail.** Two commands per rung against a buffer of
//! two: under the limit by construction. Every rung came back clean at every
//! gap, including 15ms, which reads as "15ms is safe" and meant "this cannot
//! trip".
//!
//! **Version 2 could not accumulate.** Group size fixed, but each trial was
//! followed by a 4-second drain and a 3-second settle -- seven seconds of
//! quiet between every pair of groups. It asked "can the server take this
//! once, from rest?" over and over, and never asked the real question.
//!
//! > **AUTHOR:** *"why are you waiting 1+ seconds between sends?"*
//!
//! The pauses below are the **only** pauses. Frames are collected in the same
//! `select!` that drives the sending, so observing cannot throttle it.
//!
//! Split out of [`super`] under Rule 4.1 (`plan/05:352-353`).

use cena_session::{Event, Frame, Gate, Origin, SessionHandle};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;

/// The author's sequence: how long to pause **after** each group.
///
/// Five groups, four pauses -- the fifth group has nothing after it.
const PAUSES_MS: [u64; 4] = [500, 300, 200, 100];

/// Commands per group. The author's number.
///
/// Four against a buffer of two (MEASURED: the wire said `type ahead 2
/// commands`) overflows by two, so refusals are expected from the first group
/// on. The question is how they change as the pauses shrink.
const GROUP: usize = 4;

/// How long to wait after the final group for its replies to land.
///
/// After all sending, so it cannot pad the cadence.
const TAIL: Duration = Duration::from_secs(4);

/// What one group saw.
#[derive(Debug, Default, Clone, Copy)]
struct GroupResult {
    /// Typeahead refusals attributed to this group.
    refusals: usize,
    /// Room replies attributed to this group.
    rooms: usize,
}

/// Run the author's sequence.
pub(super) async fn phase_2_ladder(
    handle: &SessionHandle,
    events: &mut broadcast::Receiver<Event>,
) {
    eprintln!("\n[phase 2] {GROUP} looks, then a SHRINKING pause, five times over:");
    eprintln!(
        "          {GROUP} | {}ms | {GROUP} | {}ms | {GROUP} | {}ms | {GROUP} | {}ms | {GROUP}",
        PAUSES_MS[0], PAUSES_MS[1], PAUSES_MS[2], PAUSES_MS[3]
    );
    eprintln!("          One pass, nothing repeated, no reset between groups --");
    eprintln!("          so backlog carries forward and pressure rises as it goes.\n");

    let results = run_sequence(handle, events).await;

    eprintln!("\n  group  pause-after   refusals   room replies (of {GROUP})");
    for (i, result) in results.iter().enumerate() {
        let after = PAUSES_MS
            .get(i)
            .map_or_else(|| "-".to_owned(), |ms| format!("{ms}ms"));
        eprintln!(
            "  {:>5}  {after:>11}   {:>8}   {:>12}",
            i + 1,
            result.refusals,
            result.rooms
        );
    }

    let total: usize = results.iter().map(|r| r.refusals).sum();
    match results.iter().position(|r| r.refusals > 0) {
        None => eprintln!("\n  No refusals at any pause -- the whole sequence was absorbed."),
        Some(at) => eprintln!(
            "\n  First refusal in group {}, {total} refusals overall.",
            at + 1
        ),
    }
}

/// Send the sequence, collecting frames as it goes.
///
/// Returns one [`GroupResult`] per group, attributed by which group was in
/// flight when the frame arrived. Attribution is **approximate at the fast
/// end** -- a reply can land after the next group has started -- which is why
/// the printout shows the per-group split *and* the total rather than resting
/// on the split alone.
async fn run_sequence(
    handle: &SessionHandle,
    events: &mut broadcast::Receiver<Event>,
) -> Vec<GroupResult> {
    let groups = PAUSES_MS.len() + 1;
    let current = AtomicUsize::new(0);
    let mut results = vec![GroupResult::default(); groups];

    let send = async {
        for group in 0..groups {
            current.store(group, Ordering::Relaxed);
            for _ in 0..GROUP {
                // `send_now`, so the CLIENT does not serialise them: the
                // question is what the server does with four at once.
                let _ = handle.send_now("look", Origin::Manual, Gate::None).await;
            }
            // THE ONLY PAUSE. The last group has none -- TAIL follows, outside
            // the loop, after all sending is done.
            if let Some(ms) = PAUSES_MS.get(group) {
                tokio::time::sleep(Duration::from_millis(*ms)).await;
            }
        }
        tokio::time::sleep(TAIL).await;
    };
    tokio::pin!(send);

    // Collect WHILE sending. An earlier version returned `Rung::default()`
    // from the sender's arm of a `select!`, silently discarding every count
    // the collector had made.
    loop {
        tokio::select! {
            () = &mut send => break,
            received = events.recv() => match received {
                Ok(Event::Frame(frame)) => {
                    let at = current.load(Ordering::Relaxed).min(groups - 1);
                    inspect(&frame, &mut results[at]);
                }
                Ok(_) => {}
                // Not terminal (`plan/12` 6.3). A lag means the ring overflowed
                // while sending hard, which is expected here and must not end
                // the collection.
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    eprintln!("    !! {missed} events dropped from the ring (still collecting)");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
        }
    }
    results
}

/// Classify one frame into the group that was in flight.
fn inspect(frame: &Frame, result: &mut GroupResult) {
    let text = match frame {
        Frame::Text(text) => text.content.clone(),
        Frame::Component { body, .. } => body.plain(),
        _ => return,
    };
    if text.contains(super::TYPEAHEAD_REFUSAL) {
        result.refusals += 1;
        // Printed verbatim as well as counted: the number in the message is an
        // account entitlement (MEASURED: `2 commands` here), so a bare count
        // would hide a change in it.
        eprintln!("    >> {}", text.trim());
    }
    if text.contains("Obvious exits:") || text.contains("Obvious paths:") {
        result.rooms += 1;
    }
}

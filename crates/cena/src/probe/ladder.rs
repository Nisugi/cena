//! Phase 2: the author's sequence, exactly.
//!
//! > **AUTHOR, 2026-09-18:** *"send 3 looks at once, sleep 0.05, send 3 looks,
//! > sleep 0.05 and do that 10 times."* -- then, on the result: *"is the model
//! > dead? up the 0.05 to 0.1"*
//!
//! ```text
//! (3 looks | 100ms) x 10
//! ```
//!
//! # Is the model dead? No -- it lost a clause
//!
//! The 50ms run refused 17 of 30, which falsified **one** claim: that the
//! pause does not matter. It did not touch the other: **3 accepted at a time**
//! has now held in both runs, and the 50ms run's first three rounds were clean
//! before anything backed up.
//!
//! So the depth is not in question. What is in question is the *rate* on top of
//! it, and 100ms is the sharpest place to ask: it is the **one pause that
//! appears in both runs**. Run A used it between groups of 4 and was clean; run
//! B never tried it. If 100ms is clean here it is the drain rate, and the two
//! runs agree rather than contradicting each other.
//!
//! # Why 3 and 100ms, after the previous two runs
//!
//! The previous sequence (`4 | 500ms | 4 | 300ms | 4 | 200ms | 4 | 100ms | 4`)
//! found that **every group of 4 accepted exactly 3 and refused 1, at every
//! pause** -- 25 sent, 7 refused, 18 accepted, with the groups at 300/200/100ms
//! all landing inside one server second. The pause made no difference at all
//! (`plan/16` §5.2d).
//!
//! That fixed the buffer at **`1 executing + 2 buffered`**. The 50ms run then
//! showed the depth is not the whole story: 30 sent, 17 refused, rounds 1-3
//! clean and refusals from round 4 on, with all 30 commands inside 1.57s
//! (~19/s) and two server seconds.
//!
//! **This run asks whether 100ms is enough to keep up.** Clean means the drain
//! rate sits between 10/s and 19/s and a client pacing at 100ms is safe today.
//! Refusals that start late again mean 10/s is still too fast and the boundary
//! is lower. Refusals from round 1 would mean something else is going on and
//! the pause is not the variable at all.
//!
//! # Two earlier versions of this file measured nothing
//!
//! Kept written down because both *looked* like measurements.
//!
//! **Version 1 could not fail.** Two commands per rung against a buffer of
//! two: under the limit by construction. Clean at every gap including 15ms,
//! which reads as "15ms is safe" and meant "this cannot trip".
//!
//! **Version 2 could not accumulate.** A 4-second drain and a 3-second settle
//! after every trial -- seven seconds of quiet between each pair of groups. It
//! asked "can the server take this once, from rest?" repeatedly and never
//! asked the sustained question.
//!
//! > **AUTHOR:** *"why are you waiting 1+ seconds between sends?"*
//!
//! The pause below is the **only** pause. Frames are collected in the same
//! `select!` that drives the sending, so observing cannot throttle it.
//!
//! Split out of [`super`] under Rule 4.1 (`plan/05:352-353`).

use cena_session::{Event, Frame, Gate, Origin, SessionHandle};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::broadcast;

/// Commands per group. The author's number.
///
/// **Exactly what the previous run accepted per group** (MEASURED: 3 of every
/// 4, in all six groups, `plan/16` §5.2d). So the model's own prediction is
/// the input here, which is the sharpest test available: the expected result
/// is zero refusals, and any refusal at all is information.
const GROUP: usize = 3;

/// The pause after each group.
///
/// **100ms, doubled from the 50ms that failed** (AUTHOR: *"up the 0.05 to
/// 0.1"*). It is the one pause that appears in **both** previous runs: run A
/// used it between groups of 4 and was clean, run B never tried it.
///
/// MEASURED why this is the place to ask: at 50ms the rounds went out every
/// ~61ms -- the sleep plus ~11ms of send overhead -- so 30 commands crossed the
/// wire in 1.57s, about **19 per second**. Run A managed **3.6 per second** and
/// never backed up. 100ms here puts the cadence near **10 per second**, between
/// the two, which is exactly where the boundary should be if one exists.
const PAUSE: Duration = Duration::from_millis(100);

/// How many times the group-and-pause repeats.
///
/// Ten rounds with no reset between them. One round cannot show accumulation;
/// ten can, and **where** the first refusal falls is the signal -- round 1
/// means 3 was never safe, round 7 means backlog built up over time.
const ROUNDS: usize = 10;

/// How long to wait after the final group for its replies to land.
///
/// After all sending, so it cannot pad the cadence.
const TAIL: Duration = Duration::from_secs(4);

/// What one round saw.
#[derive(Debug, Default, Clone, Copy)]
struct RoundResult {
    /// Typeahead refusals attributed to this round.
    refusals: usize,
    /// Room replies attributed to this round.
    rooms: usize,
}

/// Run the author's sequence.
pub(super) async fn phase_2_ladder(
    handle: &SessionHandle,
    events: &mut broadcast::Receiver<Event>,
) {
    eprintln!(
        "\n[phase 2] ({GROUP} looks | {}ms) x {ROUNDS}, one pass, no reset",
        PAUSE.as_millis()
    );
    eprintln!("          {GROUP} is EXACTLY what the last run accepted per group, so the");
    eprintln!("          model predicts zero refusals. Any refusal falsifies it, and");
    eprintln!("          WHERE it falls says how: round 1 = 3 was never safe,");
    eprintln!("          later = the buffer refills more slowly than it drains.\n");

    let results = run_sequence(handle, events).await;

    eprintln!("\n  round   refusals   room replies (of {GROUP})");
    for (i, result) in results.iter().enumerate() {
        eprintln!(
            "  {:>5}   {:>8}   {:>12}",
            i + 1,
            result.refusals,
            result.rooms
        );
    }

    let total: usize = results.iter().map(|r| r.refusals).sum();
    let rooms: usize = results.iter().map(|r| r.rooms).sum();
    eprintln!(
        "\n  {} sent, {total} refusals, {rooms} replies",
        GROUP * ROUNDS
    );
    match results.iter().position(|r| r.refusals > 0) {
        None => eprintln!(
            "  CLEAN at every round -- {GROUP} per group is sustainable at {}ms.",
            PAUSE.as_millis()
        ),
        Some(0) => eprintln!("  Refused from round 1: {GROUP} was never under the limit."),
        Some(at) => eprintln!(
            "  First refusal in round {} -- backlog accumulated rather than the \
             group being too big.",
            at + 1
        ),
    }
    eprintln!("  (Counts here are approximate -- read the .bytes log, see plan/16 §5.2e.)");
}

/// Send the sequence, collecting frames as it goes.
///
/// Returns one [`RoundResult`] per round, attributed by which round was in
/// flight when the frame arrived. **Attribution is approximate**: at 50ms a
/// reply routinely lands after the next round has started, and the previous
/// run's console undercounted 7 wire refusals as 5 for exactly this reason
/// (`plan/16` §5.2e). The totals inherit the same error, so the `.bytes` log is
/// the record and this is commentary.
async fn run_sequence(
    handle: &SessionHandle,
    events: &mut broadcast::Receiver<Event>,
) -> Vec<RoundResult> {
    let current = AtomicUsize::new(0);
    let mut results = vec![RoundResult::default(); ROUNDS];

    let send = async {
        for round in 0..ROUNDS {
            current.store(round, Ordering::Relaxed);
            for _ in 0..GROUP {
                // `send_now`, so the CLIENT does not serialise them: the
                // question is what the server does with three at once.
                let _ = handle.send_now("look", Origin::Manual, Gate::None).await;
            }
            // THE ONLY PAUSE.
            tokio::time::sleep(PAUSE).await;
        }
        tokio::time::sleep(TAIL).await;
    };
    tokio::pin!(send);

    // Collect WHILE sending. An earlier version returned a default from the
    // sender's arm of a `select!`, silently discarding every collected count.
    loop {
        tokio::select! {
            () = &mut send => break,
            received = events.recv() => match received {
                Ok(Event::Frame(frame)) => {
                    let at = current.load(Ordering::Relaxed).min(ROUNDS - 1);
                    inspect(&frame, &mut results[at]);
                }
                Ok(_) => {}
                // Not terminal (`plan/12` §6.3). A lag means the ring
                // overflowed while sending hard, which is expected here and
                // must not end the collection.
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    eprintln!("    !! {missed} events dropped from the ring (still collecting)");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            },
        }
    }
    results
}

/// Classify one frame into the round that was in flight.
fn inspect(frame: &Frame, result: &mut RoundResult) {
    let text = match frame {
        Frame::Text(text) => text.content.clone(),
        Frame::Component { body, .. } => body.plain(),
        _ => return,
    };
    if text.contains(super::TYPEAHEAD_REFUSAL) {
        result.refusals += 1;
        // Printed verbatim as well as counted: the number is an account
        // entitlement (MEASURED: `2 commands` here), so a bare count would hide
        // a change in it.
        eprintln!("    >> {}", text.trim());
    }
    if text.contains("Obvious exits:") || text.contains("Obvious paths:") {
        result.rooms += 1;
    }
}

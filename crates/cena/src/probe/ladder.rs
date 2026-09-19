//! Phase 2: how long a client must wait between groups of commands.
//!
//! > **AUTHOR, 2026-09-18:** *"is it 3, wait 250ms, 3, wait for 250ms, etc or
//! > wait for 200ms or wait for 100ms, etc and then also is it consistent?"*
//!
//! Split out of [`super`] under Rule 4.1 (`plan/05:352-353`) -- move code down,
//! do not raise the cap -- when repeating each rung for consistency took that
//! file to 415 lines against the 400 default.
//!
//! It is the right seam: the other phases each ask one question once, and this
//! one is a repeated experiment with its own sampling design.

use super::drain_for;
use cena_session::{Event, Gate, Origin, SessionHandle};
use std::time::Duration;
use tokio::sync::broadcast;

/// The delays tried between one full group and the next, in milliseconds.
///
/// **Rebuilt 2026-09-18 after the first run measured nothing.** The original
/// ladder sent *two* commands per rung against a buffer of two, so it was under
/// the limit by construction and every rung came back clean at every gap --
/// including 15ms. It could not have failed, which makes it not a measurement.
///
/// > **AUTHOR:** *"how long do we need to wait between the first 3 before the
/// > second 3 will send without tripping?"*
///
/// That is the real question and it needs **a group that fills the buffer,
/// then a gap, then another**. The gap being measured is the server's DRAIN
/// time: how long it takes to process what is outstanding.
///
/// `0` establishes that back-to-back groups do trip, so a clean rung higher up
/// means something. The rest bracket the two values the references chose --
/// Kelfour's `.5s` macro delay and Lich's `Script.execution_sleep 1`.
const DELAY_LADDER_MS: [u64; 7] = [0, 100, 200, 300, 500, 750, 1000];

/// How many commands each group sends.
///
/// **Must exceed the buffer**, or the rung cannot trip at any gap -- the defect
/// in the first version of this ladder. Three is `BASE + PREMIUM + 1` on the
/// author's account (MEASURED: the wire said `type ahead 2 commands`), so a
/// group of 3 fills the buffer and leaves exactly one command needing drain.
///
/// Deliberately the smallest number that can trip: a larger group would refuse
/// more without locating the boundary any more precisely.
const GROUP: usize = 3;

/// How many times each rung is repeated.
///
/// > **AUTHOR:** *"and then also is it consistent?"*
///
/// **One trial per gap cannot answer that.** A single clean rung is equally
/// consistent with "this gap is safe" and with "this gap is marginal and that
/// round got lucky", and the two call for different client policies: a safe
/// gap can be used, a marginal one must not be.
///
/// So each gap is run [`TRIALS`] times and the printout reports **how many of
/// the trials were clean**, not whether the last one was. A gap that is 5/5 is
/// a different fact from one that is 3/5, and the second is the more useful
/// finding -- it says the boundary is *here*, which a run of clean rungs never
/// does.
///
/// Five, because it is the smallest count where a single outlier is visibly an
/// outlier rather than 50% of the evidence, and because each trial costs two
/// groups plus a settle -- so this is already the dominant cost of the probe.
const TRIALS: usize = 5;

/// A rung's result: how many trials were clean, and what the refusals were.
struct Rung {
    /// Trials with no refusal at all.
    clean: usize,
    /// Total refusals across every trial, for a sense of severity.
    refusals: usize,
    /// Room replies seen, against `GROUP * 2 * TRIALS` sent.
    rooms: usize,
}

/// **Q4: how long must a client wait between GROUPS?**
///
/// > **AUTHOR:** *"how long do we need to wait between the first 3 before the
/// > second 3 will send without tripping?"*
///
/// Two groups of [`GROUP`] with a growing gap between them. The **first clean
/// rung is the drain time**: how long the server needs to work through what is
/// outstanding before it will accept a full buffer again.
///
/// # Why the first version of this measured nothing
///
/// It sent two commands per rung against a buffer of two, so no rung could
/// trip at any gap -- and every rung came back clean, including 15ms, which
/// read as "15ms is safe" when it actually meant "this test cannot fail".
/// A test that passes at every input is not evidence about any of them.
///
/// # Why this cannot be read off the server's clock
///
/// MEASURED in the first run: the whole burst -- three executions and two
/// refusals -- landed inside **one** `<prompt time=>` second (`1789780013`).
/// The server's own clock has one-second granularity, so it cannot resolve a
/// drain that completes inside a second. The gap has to be **imposed by the
/// client and varied**, which is what this ladder does.
///
/// # What a clean rung means, and does not
///
/// Per Kelfour the limit counts **unprocessed commands**, so the drain time is
/// a property of **server load right now**, not a constant. A clean rung is
/// today's answer on this account; it is not a number to hard-code. What
/// transfers is the *shape*: bound the outstanding count, do not pace by
/// delay.
pub(super) async fn phase_2_ladder(
    handle: &SessionHandle,
    events: &mut broadcast::Receiver<Event>,
) {
    eprintln!(
        "
[phase 2] TWO GROUPS of {GROUP} `look`s, with a growing gap between them"
    );
    eprintln!("          The first CLEAN rung is the drain time -- how long the server");
    eprintln!("          needs before it will take a full buffer again.");
    eprintln!("          (Kelfour: the limit counts UNPROCESSED commands, so this moves");
    eprintln!("           with load. Today's answer, not a constant.)");

    for ms in DELAY_LADDER_MS {
        let rung = run_rung(handle, events, ms).await;
        let verdict = if rung.clean == TRIALS {
            "CLEAN".to_owned()
        } else if rung.clean == 0 {
            "TRIPPED".to_owned()
        } else {
            // The interesting case, and the reason trials are repeated: a gap
            // that works sometimes is a gap a client must not use.
            "MARGINAL".to_owned()
        };
        eprintln!(
            "  gap {ms:>4}ms: {verdict:<9} {}/{} trials clean, {} refusals,              {} of {} replies",
            rung.clean,
            TRIALS,
            rung.refusals,
            rung.rooms,
            GROUP * 2 * TRIALS
        );
    }
}

/// One rung: [`TRIALS`] repetitions of "a group, the gap, another group".
async fn run_rung(
    handle: &SessionHandle,
    events: &mut broadcast::Receiver<Event>,
    ms: u64,
) -> Rung {
    let mut rung = Rung {
        clean: 0,
        refusals: 0,
        rooms: 0,
    };
    for _ in 0..TRIALS {
        for _ in 0..GROUP {
            let _ = handle.send_now("look", Origin::Manual, Gate::None).await;
        }
        tokio::time::sleep(Duration::from_millis(ms)).await;
        for _ in 0..GROUP {
            let _ = handle.send_now("look", Origin::Manual, Gate::None).await;
        }

        let seen = drain_for(events, Duration::from_secs(4)).await;
        if seen.refusals == 0 {
            rung.clean += 1;
        }
        rung.refusals += seen.refusals;
        rung.rooms += seen.rooms;

        // Settle hard between trials, so each one starts from an empty buffer
        // rather than inheriting the last one's backlog. Longer than the
        // longest gap, or a late drain would be attributed to the wrong trial.
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    rung
}

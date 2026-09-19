//! The typeahead probe: four questions the wire can answer and Lich cannot.
//!
//! # Read this before running it
//!
//! **The limit is already known.** Three sources agree it is one command
//! beyond the one executing (`plan/16` §5.2a), and Lich compares the refusal
//! with `==` rather than parsing a number out of it -- so this is **not** a
//! sweep to find N. Building one would have been measuring a constant.
//!
//! What is genuinely open is narrower, and `plan/16` §5.3 lists it:
//!
//! 1. Do **instant actions** count against the buffer at all? The author's
//!    *"shouldn't be subject to typeahead or waiting (depending on the
//!    action)"* says some may not, and Lich has no instant-action concept, so
//!    it cannot answer. **This is the only one that changes the design.**
//! 2. Does exceeding the limit **drop** the command or refuse it?
//! 3. What does the server do with a command sent **during roundtime**
//!    (`plan/16` §1.5)?
//! 4. How far apart must commands be to stay under the limit -- and, per the
//!    Kelfour newsletter, is that even a fixed number? It says the limit is on
//!    commands **not yet processed**, so it is a buffer depth and load
//!    dependent, not a rate.
//!
//! # Why the commands here are the ones they are
//!
//! Every command in this file is **free, idle and reversible**. `look` and
//! `search` cost nothing and risk nothing; `search` is the author's own
//! suggestion for making a roundtime on purpose. Nothing here attacks, moves,
//! drops, sells, or spends mana. A probe that has to be run against a live
//! character on a live server should not be able to cost that character
//! anything, and the way to guarantee that is to send only commands that
//! cannot.
//!
//! # What it measures with, and why not a stopwatch here
//!
//! **The `.bytes` log is the measurement.** This file prints a running
//! commentary so the author can see it working, but every number that matters
//! is recoverable from the wire log afterwards, with the server's own
//! `<prompt time=>` as the clock. Printing a local elapsed time would be
//! measuring this machine's scheduler as much as the server.
//!
//! # It is off by default
//!
//! Guarded behind `CENA_PROBE=typeahead`, because it deliberately provokes
//! server refusals and should never run as part of an ordinary session.

use crate::run::send_manual;
use cena_session::{Event, Frame, Gate, Origin, Refusal, Sent, SessionHandle};
use std::time::Duration;
use tokio::sync::broadcast;

/// The refusal, verbatim.
///
/// Matched as a **substring**, not with `==`, even though Lich uses equality.
/// Lich is matching a line it has already split; this is searching text that
/// may have arrived with markup or mid-stream. The number is deliberately
/// **not** included in the pattern: `plan/16` §5.3 q4 asks whether the corpus
/// ever carries a value other than `1`, and a matcher that hard-coded `1`
/// could not notice a `2`.
const TYPEAHEAD_REFUSAL: &str = "you may only type ahead";

/// How many commands the burst sends with no gap at all.
///
/// Five, not fifty. The limit is 1, so a burst of five is already four over --
/// enough to see the refusal and to see how many of the five actually took
/// effect. A larger burst would provoke more refusals without answering
/// anything the fifth does not.
const BURST: usize = 5;

/// The delays tried, in milliseconds.
///
/// `0` is the burst itself. `500` is the Kelfour newsletter's macro delay and
/// `1000` is Lich's `Script.execution_sleep 1` -- so this ladder is the two
/// values the references actually chose, with a midpoint and a wider one
/// around them, rather than an arbitrary sweep.
const DELAY_LADDER_MS: [u64; 5] = [0, 250, 500, 750, 1000];

/// Run the probe. Returns what was observed, for the caller to print.
///
/// Takes a subscription rather than making one, so the caller's join is still
/// the single operation `plan/12` §6.2 requires -- nothing between the
/// snapshot and the subscribe.
pub(crate) async fn run(handle: &SessionHandle, events: &mut broadcast::Receiver<Event>) {
    eprintln!("\n{}", "=".repeat(70));
    eprintln!("TYPEAHEAD PROBE  (plan/16 §5.3)");
    eprintln!("The limit is KNOWN to be 1 (§5.2a). This measures the four");
    eprintln!("things that reading Lich could not settle.");
    eprintln!("{}\n", "=".repeat(70));

    // **Drain first.** This receiver was created at login so that it would not
    // miss anything, which means by now it holds -- or has already overflowed
    // with -- the whole login burst and the behavior's traffic. Every phase
    // below counts frames in a window, and starting with a backlog would
    // attribute the login's rooms to phase 1 and open each drain with a
    // `Lagged`. A zero-length drain discards what is queued and leaves the
    // receiver live.
    let stale = drain_for(events, Duration::from_millis(50)).await;
    eprintln!(
        "[probe] discarded {} stale frames before starting\n",
        stale.frames
    );

    phase_1_burst(handle, events).await;
    phase_2_ladder(handle, events).await;
    phase_3_instant_actions();
    phase_4_during_roundtime(handle, events).await;

    eprintln!("\n{}", "=".repeat(70));
    eprintln!("PROBE COMPLETE. The .bytes log is the record; this was commentary.");
    eprintln!("{}\n", "=".repeat(70));
}

/// **Q2: does exceeding the limit drop the command or refuse it?**
///
/// Five `look`s with no gap. `look` is chosen because its *reply is
/// distinctive and idempotent*: looking five times is harmless, and counting
/// the room descriptions that come back says how many of the five the server
/// actually ran. A command whose reply could not be counted would answer
/// "was there a refusal" but not "what happened to the rest".
async fn phase_1_burst(handle: &SessionHandle, events: &mut broadcast::Receiver<Event>) {
    eprintln!("[phase 1] {BURST} `look`s with NO gap -- expect a refusal after the 1st or 2nd");

    for i in 0..BURST {
        // `send_now` with `Gate::None`: this phase is about the SERVER's
        // buffer, so the client must not be the thing that throttles. Using
        // `send_and_await` would serialise them and measure nothing.
        let sent = handle.send_now("look", Origin::Manual, Gate::None).await;
        eprintln!("  look {i}: {sent:?}");
    }

    let seen = drain_for(events, Duration::from_secs(5)).await;
    report(&seen, "phase 1");
}

/// **Q4: how far apart do commands have to be?**
///
/// Two commands at each delay, which is the smallest number that can trip a
/// limit of one. The ladder runs low to high so the first delay that comes
/// back clean is the answer -- and per the newsletter that answer is a
/// property of **server load at this moment**, not a constant, which is why
/// the printout says so rather than presenting it as a tuned value.
async fn phase_2_ladder(handle: &SessionHandle, events: &mut broadcast::Receiver<Event>) {
    eprintln!(
        "\n[phase 2] two `look`s at increasing gaps -- the first clean gap is TODAY's answer"
    );
    eprintln!("          (Kelfour: the limit is on UNPROCESSED commands, so this moves with load)");

    for ms in DELAY_LADDER_MS {
        let _ = handle.send_now("look", Origin::Manual, Gate::None).await;
        tokio::time::sleep(Duration::from_millis(ms)).await;
        let _ = handle.send_now("look", Origin::Manual, Gate::None).await;

        let seen = drain_for(events, Duration::from_secs(3)).await;
        eprintln!(
            "  gap {ms:>4}ms: {}",
            if seen.refusals > 0 {
                format!("REFUSED ({} times)", seen.refusals)
            } else {
                "clean".to_owned()
            }
        );
        // Settle, so the next rung starts from an empty buffer rather than
        // inheriting this one's backlog.
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// **Q1: do instant actions count against the buffer?** The one that matters.
///
/// > **AUTHOR:** *"shouldn't be subject to typeahead or waiting (depending on
/// > the action)"*
///
/// If instant actions are exempt, `send_now` can batch freely and `plan/16`
/// §1.1's "a few in a row, then the trigger" needs no rate policy at all. If
/// they are not, `send_now` needs one, and §5.2's claim that the two features
/// are one problem is right.
///
/// **This phase needs the author, which is why it takes no arguments and
/// awaits nothing.** The seed list is `515` Rapid Fire, `140` Wall of Force and
/// the Sunfist sigils (`plan/16` §1.2) -- all of which cost mana, need the
/// right society, or both. This file will not spend a character's resources on
/// its own initiative, so it prints what to run instead of running it. The
/// signature says so: a phase that sent anything would need the handle.
fn phase_3_instant_actions() {
    eprintln!("\n[phase 3] instant actions vs the buffer -- NOT RUN AUTOMATICALLY");
    eprintln!("          This is the question that changes the design (plan/16 §5.3 q1),");
    eprintln!("          but every candidate costs mana or needs a society:");
    eprintln!("            * `sigil of escape`  -- Guardians of Sunfist, stamina");
    eprintln!("            * `incant 515`       -- Rapid Fire, mana");
    eprintln!("            * `incant 140`       -- Wall of Force, mana");
    eprintln!("          Run three of whichever you can afford back to back, then");
    eprintln!("          grep the .bytes log for `you may only type ahead`.");
    eprintln!("          No hits => instant actions are exempt => send_now needs no rate policy.");
}

/// **Q3: what happens to a command sent DURING roundtime?** (`plan/16` §1.5)
///
/// `search` makes a real roundtime with nothing at stake -- the author's own
/// suggestion. Then a `look` goes out inside it with `Gate::None`, which is
/// the client deliberately declining to gate so the *server's* behaviour is
/// what gets measured.
///
/// This is also the first live exercise of `Gate::None` for its intended
/// purpose, rather than as a test fixture.
async fn phase_4_during_roundtime(handle: &SessionHandle, events: &mut broadcast::Receiver<Event>) {
    eprintln!("\n[phase 4] a command sent INSIDE a roundtime (plan/16 §1.5)");

    let outcome = send_manual(handle, "search").await;
    eprintln!("  search: {outcome:?}");

    // Immediately, while the search's roundtime is still running.
    let sent = handle.send_now("look", Origin::Manual, Gate::None).await;
    eprintln!("  look during roundtime (Gate::None, client gate DECLINED): {sent:?}");

    // And the same thing gated, which must refuse locally without sending.
    let gated = handle
        .send_now("look", Origin::Manual, Gate::Roundtime)
        .await;
    eprintln!("  look during roundtime (Gate::Roundtime): {gated:?}");
    if gated == Sent::Refused(Refusal::Roundtime) {
        eprintln!("  ^ the client gate held: refused locally, nothing sent");
    }

    let seen = drain_for(events, Duration::from_secs(6)).await;
    report(&seen, "phase 4");
}

/// What a drain saw.
#[derive(Debug, Default)]
struct Seen {
    /// Lines carrying the typeahead refusal.
    refusals: usize,
    /// Room descriptions, i.e. `look` replies that actually ran.
    rooms: usize,
    /// `...wait N seconds.` refusals, which are the roundtime's own.
    waits: usize,
    /// Every frame, for a sense of scale.
    frames: usize,
}

/// Collect frames for a fixed window.
///
/// A window rather than a count: the question is what the server *did*, and a
/// dropped command produces no frame at all -- so waiting for N replies would
/// hang on exactly the outcome being measured.
async fn drain_for(events: &mut broadcast::Receiver<Event>, window: Duration) -> Seen {
    let mut seen = Seen::default();
    let deadline = tokio::time::Instant::now() + window;
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            return seen;
        }
        match tokio::time::timeout(remaining, events.recv()).await {
            Ok(Ok(Event::Frame(frame))) => {
                seen.frames += 1;
                inspect(&frame, &mut seen);
            }
            Ok(Ok(_)) => {}
            // Not terminal -- `plan/12` §6.3. The same mistake that once cost
            // a room in `wait_for_room`.
            Ok(Err(broadcast::error::RecvError::Lagged(missed))) => {
                eprintln!("    !! {missed} events dropped from the ring (still connected)");
            }
            Ok(Err(broadcast::error::RecvError::Closed)) | Err(_) => return seen,
        }
    }
}

/// Classify one frame.
fn inspect(frame: &Frame, seen: &mut Seen) {
    let text = match frame {
        Frame::Text(text) => text.content.clone(),
        Frame::Component { body, .. } => body.plain(),
        _ => return,
    };
    if text.contains(TYPEAHEAD_REFUSAL) {
        seen.refusals += 1;
        // Printed verbatim: §5.3 q4 asks whether the number is ever anything
        // but 1, and the only way this run can contribute to that is by
        // showing the line rather than a count of it.
        eprintln!("    >> REFUSAL: {}", text.trim());
    }
    if text.contains("...wait") || text.contains("Wait ") {
        seen.waits += 1;
    }
    if text.contains("Obvious exits:") || text.contains("Obvious paths:") {
        seen.rooms += 1;
    }
}

/// Print what a phase saw.
fn report(seen: &Seen, phase: &str) {
    eprintln!(
        "  [{phase}] frames={} refusals={} room-replies={} wait-refusals={}",
        seen.frames, seen.refusals, seen.rooms, seen.waits
    );
}

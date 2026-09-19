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
//! Selected by `cargo run -p cena -- --typeahead`, because it deliberately
//! provokes server refusals and must never run as part of an ordinary session.

use crate::run::send_manual;
use cena_session::{Event, Frame, Gate, Origin, Refusal, Sent, SessionHandle};
use std::time::Duration;
use tokio::sync::broadcast;

mod ladder;

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
    ladder::phase_2_ladder(handle, events).await;
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

/// **Q1: do instant actions count against the buffer? ANSWERED: yes.**
///
/// > **AUTHOR:** *"shouldn't be subject to typeahead or waiting (depending on
/// > the action)"*
///
/// # Answered by the tests themselves, not by a phase
///
/// > **AUTHOR, 2026-09-18:** *"we were using the instant action look for
/// > testing, so I think that says yes instant actions count against the
/// > buffer (typeahead)."*
///
/// **Every command in every run of this probe was `look`** -- and `look` is an
/// instant action by the author's own definition, *"anything that doesn't cause
/// roundtime"* (`plan/16` §1.1a). MEASURED directly: a `look` sent inside a
/// 7-second roundtime executed normally (§1.5).
///
/// So the evidence was already in hand three runs over: `look` refused at 54
/// commands/second, accepted exactly 3 at a time, and produced `Sorry, you may
/// only type ahead 2 commands.` like anything else. **Instant actions are
/// subject to typeahead.**
///
/// # Why this phase existed anyway, and what that cost
///
/// It was scoped around **abilities** -- sigils, `515`, `140` -- because the
/// first draft of `plan/16` treated "instant action" as a curated list to
/// enumerate rather than as a property of a command. Under a list, `look` is
/// not a member and the question stays open; under the property it is a member
/// and the question was answered by the first burst.
///
/// The cost was real: this phase asked the author to spend mana on a question
/// three runs had already settled. **A definition that is a list invites
/// measuring things the property would have told you.**
///
/// # What is still genuinely open
///
/// Whether a **roundtime-causing** action behaves differently -- an attack or a
/// cast sent while over the buffer. Every command measured so far is free, so
/// the refusal path for something that costs is UNVERIFIED. It is a narrower
/// question than this phase was asking and is recorded in `plan/16` §5.3.
fn phase_3_instant_actions() {
    eprintln!(
        "
[phase 3] instant actions vs the buffer -- ALREADY ANSWERED: yes"
    );
    eprintln!("          Every command in this probe is `look`, which is an instant");
    eprintln!("          action by the definition 'anything that doesn't cause");
    eprintln!("          roundtime' -- MEASURED running inside a 7s roundtime.");
    eprintln!("          It refuses at 54/s and accepts 3 at a time like anything");
    eprintln!("          else, so instant actions ARE subject to typeahead.");
    eprintln!("          No mana needs spending to find this out.");
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

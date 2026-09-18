//! The demonstration the binary runs once it is connected: show the room, run
//! a behavior, interleave a manual command, and make some roundtimes on
//! purpose.
//!
//! Split from `main.rs` under `plan/05` Rule 4.1 -- move code down, do not
//! raise the cap -- when the capture pushed that file to 428 lines. The seam
//! is real: everything here happens *after* a session exists, and none of it
//! knows how one is built.

use crate::{CAPTURE_GAP, CAPTURE_SEARCHES, ROOM_DEADLINE};
use cena_behavior::is_room_description;
use cena_session::{CommandId, Event, Frame, Origin, SessionHandle};
use std::time::Duration;
use tokio::sync::broadcast;

/// Send a few `search` commands, to make roundtimes happen on purpose.
/// `search` is the cheapest command that produces a real roundtime: no
/// combat, nothing to find, no risk to the character. The author named it
/// for exactly that.
///
/// Three things are being measured, all of them from the `.bytes` log
/// rather than from anything printed here:
///
///  1. CLOCK PHASE. `<prompt time=>` is whole seconds, so a tick from N to
///     N+1 marks the server's second boundary. Against our arrival times
///     that pins where the boundary falls -- which turns a `<roundTime>`
///     from a one-second window into a known instant.
///  2. SKEW AND LAG. The same data, accumulated: whether the two clocks
///     agree, and how far behind the wire we are.
///  3. WHAT `roundTime value=N` MEANS. Does it end at the START of second
///     N, or somewhere inside it? Everything about gating instant actions
///     rests on this, and it cannot be read off a fixture -- it needs a
///     real roundtime and the moment the game next accepts a command.
///
/// Sent as MANUAL commands rather than by changing the behavior: `look`'s
/// matcher is `is_room_description`, which a search does not produce, and
/// the behavior is covered by tests that should not be disturbed for a
/// measurement.
pub(crate) async fn run_capture(handle: &SessionHandle) {
    for i in 0..CAPTURE_SEARCHES {
        let outcome = send_manual(handle, "search").await;
        eprintln!("[capture] search {i}: {outcome:?}");
        // Long enough for the roundtime to expire and several prompts to
        // arrive between attempts. Prompts are the calibration signal, so the
        // gaps matter as much as the searches.
        tokio::time::sleep(CAPTURE_GAP).await;
    }
}

/// Wait for the first room description frame, or time out.
///
/// Criterion 2 is "renders a room description from **typed frames**, never raw
/// text" -- so this matches on a `Frame`, not on the byte stream. That is what
/// "parse first" means (`CLAUDE.md`, settled decisions).
///
/// Returns `false` on timeout or a closed stream; the caller prints the
/// failure, because only it knows what to blame.
pub(crate) async fn wait_for_room(events: &mut broadcast::Receiver<Event>) -> bool {
    tokio::time::timeout(ROOM_DEADLINE, async {
        loop {
            match events.recv().await {
                Ok(Event::Frame(frame)) if is_room_description(&frame) => {
                    print_room(&frame);
                    return true;
                }
                Ok(_) => {}
                // `Lagged` is NOT the end of the stream. tokio's own docs:
                // "Returning `RecvError::Lagged` does **not** close or
                // disconnect the channel" -- the next `recv()` succeeds.
                //
                // Treating it as terminal is the bug `plan/12` §6.3 exists to
                // prevent: the session goes to the trouble of handing up "an
                // explicit `Lagged { missed }`, **never a silent gap**"
                // (`cena-session/src/actor.rs:62-66`), and a consumer that
                // collapses it into `Closed` turns it right back into a silent
                // gap -- then blames the WRAYTH banner for a room that was
                // merely dropped from the ring. Found by adversarial review.
                Err(broadcast::error::RecvError::Lagged(missed)) => {
                    eprintln!("  !! {missed} events dropped from the ring (still connected)");
                }
                Err(broadcast::error::RecvError::Closed) => return false,
            }
        }
    })
    .await
    .unwrap_or(false)
}

/// Send one manual command and await its round trip.
///
/// Wrapped in a function so the `Origin::Manual` is stated once: the whole
/// point of §4 is that a typed command and a behavior's command differ in
/// their *origin*, not in their path.
pub(crate) async fn send_manual(handle: &SessionHandle, line: &str) -> cena_session::Outcome {
    handle
        .send_and_await(
            CommandId(9000),
            line,
            Origin::Manual,
            Duration::from_secs(30),
            cena_session::queue::any_frame,
        )
        .await
}

/// Print a room description frame as text.
///
/// Takes the `Frame`, not a string: criterion 2 is "renders a room description
/// **from typed frames, never raw text**", and a function that took a `&str`
/// could not tell the difference.
pub(crate) fn print_room(frame: &Frame) {
    println!("\n{}", "=".repeat(70));
    match frame {
        Frame::Component { id, body } => {
            println!("[room: component {id}]");
            println!("{}", body.plain());
        }
        Frame::Text(text) => {
            // The story-window shape. `plan/15` §2.6: an inline styled
            // `roomDesc` is WHAT YOU SAW, which is not the same claim as
            // `component room desc`'s WHERE YOU ARE -- scrying abilities emit
            // the first without the second. Printed the same way here, and
            // deliberately labelled differently.
            println!("[room: styled roomDesc in the story stream]");
            println!("{}", text.content);
        }
        other => println!("[not a room frame: {other:?}]"),
    }
    println!("{}", "=".repeat(70));
}

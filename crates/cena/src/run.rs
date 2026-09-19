//! The demonstration the binary runs once it is connected: show the room, run
//! a behavior, interleave a manual command -- and then, **only if this run
//! asked for one**, an experiment.
//!
//! The experiment used to be unconditional, which made every run cost the
//! character about fifty seconds of `search` it had not asked for. See
//! [`run_or_probe`] for what that looked like from the author's side.
//!
//! Split from `main.rs` under `plan/05` Rule 4.1 -- move code down, do not
//! raise the cap -- when the capture pushed that file to 428 lines. The seam
//! is real: everything here happens *after* a session exists, and none of it
//! knows how one is built.

use crate::{CAPTURE_GAP, CAPTURE_SEARCHES, ROOM_DEADLINE, probe};
use cena_behavior::is_room_description;
use cena_session::{CommandId, Event, Frame, Origin, SessionHandle};
use std::time::Duration;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// Whichever experiment this run asked for, and **by default none**.
///
/// # Doing nothing is the default, because the run is not the experiment any more
///
/// This used to fall through to [`run_capture`] whenever `CENA_PROBE` was
/// unset, which meant **every** run drove the character through six `search`
/// commands at six-second gaps -- about 50 seconds of the character being
/// unusable, on a run whose point might be nothing more than "log in and see
/// the room".
///
/// The author, mid-session, with the binary holding their character:
///
/// > *"it's busy running your probe so I can't do anything."*
///
/// That was the capture rather than the probe, which is the tell: a default
/// nobody chose had become invisible enough that even its name was wrong. The
/// capture earned its keep while the clock and roundtime work needed data;
/// that work is done (`plan/16` §5.2c), and what is left is a cost paid on
/// every unrelated run.
///
/// So: `CENA_SCRIPT` names the experiment, and an unset one means the session
/// stays quiet and the character stays yours.
///
/// | Value | What it does |
/// |---|---|
/// | unset | **nothing** -- the session idles until `RUN_FOR` |
/// | `capture` | six `search` commands, for clock and roundtime samples |
/// | `typeahead` | the probe, which deliberately provokes refusals |
///
///  ```powershell
///  cargo run -p cena                              # quiet
///  $env:CENA_SCRIPT = "capture";   cargo run -p cena
///  $env:CENA_SCRIPT = "typeahead"; cargo run -p cena
///  ```
///
/// `CENA_PROBE=typeahead` still works, because it is in this repo's notes and
/// in the author's shell history.
pub(crate) async fn run_or_probe(
    handle: &SessionHandle,
    probe_events: &mut broadcast::Receiver<Event>,
    behavior_stop: &CancellationToken,
) {
    let script = std::env::var("CENA_SCRIPT").unwrap_or_default();
    // The older spelling, kept working rather than broken out from under a
    // shell history that has it.
    let probe_requested =
        script == "typeahead" || std::env::var("CENA_PROBE").as_deref() == Ok("typeahead");

    if !probe_requested && script != "capture" {
        eprintln!(
            "
[script] none -- the session is idle and the character is yours.
[script] set CENA_SCRIPT=capture or CENA_SCRIPT=typeahead to run one."
        );
        return;
    }

    if probe_requested {
        // **Stop the behavior first.** It sends a `look` every second, and the
        // probe measures how long the SERVER takes to drain a buffer -- so a
        // concurrent sender is uncontrolled traffic sitting inside every
        // measurement window. The first run left it going and its phase 2
        // results are correspondingly weaker evidence (`plan/16` §5.2c).
        //
        // Criterion 5's interleaving has already been demonstrated by the time
        // this runs, so nothing is lost by quiescing here.
        eprintln!(
            "
[probe] stopping the `look` behavior first -- it would be noise"
        );
        behavior_stop.cancel();
        // Let its in-flight round trip finish and drain, so the probe does not
        // inherit the behavior's last reply.
        tokio::time::sleep(Duration::from_secs(2)).await;
        probe::run(handle, probe_events).await;
    } else {
        run_capture(handle).await;
    }
}

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
async fn run_capture(handle: &SessionHandle) {
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

/// Print what crosses the wire, both directions, in order.
///
/// **This is what makes criterion 5 visible rather than merely true.** The
/// interleaving of a manual command with a running behavior is a fact about
/// ordering, and ordering is only observable as a sequence -- so the evidence
/// is this transcript, not a boolean.
///
/// Moved out of `main` when that function passed clippy's 100-line limit. It
/// is a whole task with one job, which makes it the obvious seam: `main` wires
/// the run together, and this watches it.
pub(crate) async fn watch_events(mut events: broadcast::Receiver<Event>) {
    loop {
        match events.recv().await {
            Ok(Event::Sent { line, origin }) => {
                let tag = match origin {
                    Origin::Manual => "manual",
                    Origin::Behavior(_) => "behavior",
                    // Distinguished HERE, which is the whole reason it is
                    // a separate variant: it queues like manual input, but
                    // a reader of this transcript has to be able to tell
                    // "the player typed this" from "another character's
                    // script sent this".
                    Origin::Script => "script",
                };
                eprintln!("  -> [{tag}] {line}");
            }
            Ok(Event::StateChanged(state)) => eprintln!("  .. lifecycle: {state:?}"),
            Ok(Event::Frame(_)) => {}
            // Keep watching. A `while let Ok(..)` here ended the watcher on
            // the first lag, which would silence the `-> [manual]` and
            // `-> [behavior]` lines for the rest of the run -- and those
            // lines ARE criterion 5's evidence, so their absence would look
            // exactly like the interleaving failing.
            Err(broadcast::error::RecvError::Lagged(missed)) => {
                eprintln!("  !! {missed} events dropped from the ring (still watching)");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    /// The defaults that decide whether a run costs the author their character.
    ///
    /// `run_or_probe` itself needs a live session, so what is tested is the
    /// **selection**, lifted to a pure function. That is the part that
    /// regressed: the old code chose `capture` for an empty environment, and
    /// nothing said so out loud.
    fn selected(script: &str, legacy_probe: Option<&str>) -> &'static str {
        let probe = script == "typeahead" || legacy_probe == Some("typeahead");
        if probe {
            "typeahead"
        } else if script == "capture" {
            "capture"
        } else {
            "none"
        }
    }

    #[test]
    fn an_unset_environment_runs_nothing() {
        assert_eq!(
            selected("", None),
            "none",
            "a plain `cargo run -p cena` must not drive the character. It              used to run the capture -- ~50s of `search` -- on every run,              including the ones that only wanted to see a room."
        );
    }

    #[test]
    fn each_script_is_opt_in_by_name() {
        assert_eq!(selected("capture", None), "capture");
        assert_eq!(selected("typeahead", None), "typeahead");
    }

    /// The old spelling still works: it is in this repo's notes and in the
    /// author's shell history, and breaking it would cost more than keeping it.
    #[test]
    fn the_legacy_probe_variable_still_selects_the_probe() {
        assert_eq!(selected("", Some("typeahead")), "typeahead");
    }

    /// An unrecognised name runs NOTHING rather than falling through to a
    /// script nobody asked for. A typo should cost a quiet session, not the
    /// character.
    #[test]
    fn an_unrecognised_script_runs_nothing() {
        assert_eq!(selected("capturr", None), "none");
        assert_eq!(selected("probe", None), "none");
    }
}

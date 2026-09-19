//! The demonstration the binary runs once it is connected: show the room, run
//! a behavior, interleave a manual command -- and then, **only if this run
//! asked for one**, an experiment.
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

/// Whether this run was asked to demonstrate criteria 3-5 (`-- --demo`).
///
/// It runs a `look` behavior at one-second intervals and interleaves a manual
/// command, so it **sends** -- which is why it is opt-in alongside the scripts
/// rather than always on.
#[must_use]
pub(crate) fn demo_requested() -> bool {
    std::env::args().skip(1).any(|arg| arg == "--demo")
}

/// Which experiment this run asked for, **named on the command line**.
///
/// | Argument | What it does |
/// |---|---|
/// | *(none)* | **nothing** -- the session idles and the character is yours |
/// | `--capture` | six `search` commands, for clock and roundtime samples |
/// | `--typeahead` | the probe, which deliberately provokes refusals |
///
/// [`demo_requested`] gates the `look` behavior separately, for the same
/// reason: it sends too.
///
///  ```powershell
///  cargo run -p cena                  # sends NOTHING
///  cargo run -p cena -- --demo
///  cargo run -p cena -- --typeahead
///  ```
///
/// An argument, not an env var: an env var sticks around for the whole shell,
/// so one probe run would drive the character on every subsequent login.
pub(crate) async fn run_or_probe(
    handle: &SessionHandle,
    probe_events: &mut broadcast::Receiver<Event>,
    behavior_stop: &CancellationToken,
) {
    let selected = Script::from_args(std::env::args().skip(1));

    match selected {
        Script::None => {
            eprintln!(
                "
[script] none -- the session is idle and the character is yours.
[script] pass `-- --capture` or `-- --typeahead` to run one."
            );
            return;
        }
        Script::Capture => {
            run_capture(handle).await;
            return;
        }
        Script::Typeahead => {}
    }

    {
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
    }
}

/// What a run was asked to do to the character.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Script {
    /// Drive nothing. The default, and the only one that costs the author
    /// nothing.
    None,
    /// Six `search` commands, for clock and roundtime samples.
    Capture,
    /// The type-ahead probe, which deliberately provokes server refusals.
    Typeahead,
}

impl Script {
    /// Pick a script from the command line.
    ///
    /// **Unknown arguments select [`Self::None`]**, rather than being reported
    /// as an error that a caller might be tempted to ignore. The failure this
    /// guards is a typo running an experiment nobody asked for; refusing to do
    /// anything is the cheap direction to be wrong in.
    pub(crate) fn from_args<I, S>(args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        for arg in args {
            match arg.as_ref() {
                "--capture" => return Self::Capture,
                "--typeahead" => return Self::Typeahead,
                _ => {}
            }
        }
        Self::None
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
            // **The game's own output.** This arm used to be `{}` -- every
            // frame discarded -- because the binary was a criteria
            // demonstration and printed only its own commands:
            //
            //   AUTHOR: "I've not been able to see anything but what you allow
            //            me to see so far, which has been you sending look
            //            non-stop"
            //
            // Exactly so, and it is backwards now that the session is the
            // point rather than the demo. A client that connects and shows the
            // player nothing the game said is not a client.
            Ok(Event::Frame(frame)) => print_frame(&frame),
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

/// Show a frame the way a player would want to read it.
///
/// **Text and prompts only.** The wire carries far more -- progress bars,
/// indicators, stream routing, dialog data -- and printing all of it would
/// bury the game's prose in markup. Those frames are still parsed, still fold
/// into `GameState`, and are still in the `.bytes` log; this is a reading
/// view, not a dump.
///
/// A real frontend renders from the same frames with styling and windows
/// (`cena-ui`). This is the terminal stand-in until one exists.
fn print_frame(frame: &Frame) {
    match frame {
        Frame::Text(text) => {
            // The main window is `""`. A named stream -- thoughts, deaths,
            // familiar -- is tagged so it is not mistaken for room text.
            let body = text.content.trim_end_matches('\n');
            if body.trim().is_empty() {
                return;
            }
            if text.stream.is_empty() {
                eprintln!("{body}");
            } else {
                eprintln!("[{}] {body}", text.stream);
            }
        }
        // The prompt is the game's "your turn", and seeing it is how a player
        // knows a command finished.
        Frame::Prompt { text, .. } => eprintln!("{text}"),
        // Room description, exits, and the other named components: the login
        // burst is mostly these, so a session that dropped them would show
        // nothing at all on connect.
        Frame::Component { id, body } => {
            let plain = body.plain();
            if !plain.trim().is_empty() {
                eprintln!("[{id}] {plain}");
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::Script;

    /// **A run with no arguments drives nothing.** The whole point.
    #[test]
    fn no_arguments_runs_nothing() {
        assert_eq!(
            Script::from_args(Vec::<String>::new()),
            Script::None,
            "a plain `cargo run -p cena` must not drive the character"
        );
    }

    #[test]
    fn each_script_is_named_explicitly() {
        assert_eq!(Script::from_args(["--capture"]), Script::Capture);
        assert_eq!(Script::from_args(["--typeahead"]), Script::Typeahead);
    }

    /// The regression this exists to prevent: selection used to read
    /// `CENA_SCRIPT`/`CENA_PROBE`, and an env var set once in a shell drove the
    /// character on every later run. `from_args` takes what it considers as a
    /// parameter, so there is no environment in scope for it to reach.
    #[test]
    fn selection_comes_only_from_its_argument() {
        assert_eq!(Script::from_args(Vec::<String>::new()), Script::None);
    }

    /// A typo runs nothing rather than falling through to a script.
    #[test]
    fn an_unrecognised_argument_runs_nothing() {
        assert_eq!(Script::from_args(["--capturr"]), Script::None);
        assert_eq!(Script::from_args(["capture"]), Script::None);
        assert_eq!(Script::from_args(["--probe"]), Script::None);
    }

    /// Cargo's own arguments do not select anything by accident.
    #[test]
    fn unrelated_arguments_are_ignored() {
        assert_eq!(Script::from_args(["--release", "-p", "cena"]), Script::None);
        assert_eq!(
            Script::from_args(["--release", "--typeahead"]),
            Script::Typeahead,
            "...but a real one is still found past them"
        );
    }
}

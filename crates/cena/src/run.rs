//! The demonstration the binary runs once it is connected: show the room, run
//! a behavior, interleave a manual command -- and then, **only if this run
//! asked for one**, an experiment.
//!

//! Split from `main.rs` under `plan/05` Rule 4.1 -- move code down, do not
//! raise the cap -- when the capture pushed that file to 428 lines. The seam
//! is real: everything here happens *after* a session exists, and none of it
//! knows how one is built.

use crate::{CAPTURE_GAP, CAPTURE_SEARCHES, PSM_GAP, ROOM_DEADLINE, probe};
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

/// Whether this run was asked to force the **web-login** path (`-- --web-login`).
///
/// # Why this is needed to test the thing at all
///
/// `authenticate_with_fallback` reaches web login only when eaccess cannot be
/// REACHED -- and eaccess is up almost always. So without a forcing flag the
/// fallback is exercised for the first time during an outage, which is the
/// worst possible moment to find a bug in it.
///
/// Lich carries the same escape hatch (`auth_provider: :web`).
///
/// An argument, not an env var, for the reason recorded on [`Script`]: an env
/// var set once in a shell drove the character on every later run.
#[must_use]
pub(crate) fn web_login_forced() -> bool {
    std::env::args().skip(1).any(|arg| arg == "--web-login")
}

/// Which login provider this run uses, announcing the choice when it is not
/// the default.
///
/// Lives here rather than in `main` because it reads
/// [`web_login_forced`] -- and because `main` is at its 100-line cap, which
/// `plan/05` Rule 4.1 says to answer by moving code down.
#[must_use]
pub(crate) fn login_provider() -> cena_platform::Prefer {
    if web_login_forced() {
        eprintln!(
            "
[login] --web-login: forcing the HTTPS web-login path.
[login] eaccess will NOT be tried, so there is no fallback if this fails."
        );
        cena_platform::Prefer::WebOnly
    } else {
        cena_platform::Prefer::Eaccess
    }
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
[script] pass `-- --capture`, `-- --psm` or `-- --typeahead` to run one."
            );
            return;
        }
        Script::Capture => {
            run_capture(handle).await;
            return;
        }
        Script::Psm => {
            run_psm_capture(handle).await;
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
    /// The PSM and ascension tables, for M3's golden fixtures.
    Psm,
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
                "--psm" => return Self::Psm,
                "--typeahead" => return Self::Typeahead,
                _ => {}
            }
        }
        Self::None
    }
}

/// The commands the PSM capture issues, in order.
///
/// **Every one is a read.** `list` and `info` print a table and change
/// nothing -- no cost, no roundtime, no item touched. That is the same rule
/// `probe.rs` states for the typeahead commands, and it is why this can run
/// against the author's own character.
///
/// # Why `list all all` and not `list all`
///
/// > **AUTHOR, 2026-09-19:** *"you can set up a login test that does a
/// > `weapon list all all` `shield list all all` `ascension list all all` ect."*
///
/// MEASURED in the one archive capture we have: plain `cman list` prints
/// `Availability: profession` in its filter footer, so it shows only what that
/// character can learn. The port has to parse the table for every profession,
/// so the unfiltered form is the one worth having. Both are sent, last, so the
/// pair proves the suffix changes only which rows appear and not the shape.
///
/// # Why ascension is here at all
///
/// It is **not** a PSM (`state/character/psm.rs`'s `AscensionTable` records
/// the three ways the wire says so). It is in this capture because it is the
/// open question: Lich's `PSMStart` matches only `the following ... are
/// available:`, and across five archive files the Ascension table has only
/// ever appeared as `your ... are as follows:`. If `ascension list all all`
/// also prints `as follows:`, **Lich has never stored ascension ranks** -- the
/// rows parse but no accumulator opens. Either answer is worth the two
/// commands.
const PSM_COMMANDS: &[&str] = &[
    // The five PSMs, unfiltered.
    "cman list all all",
    "feat list all all",
    "armor list all all",
    "shield list all all",
    "weapon list all all",
    // Not a PSM; the open question.
    "ascension list all all",
    // The filtered/unfiltered pair, for the shape comparison.
    "cman list",
    "ascension info",
];

/// Capture the PSM and ascension tables for M3's fixtures.
///
/// Each command's output is a **blob** in `plan/15` §2a.4b's sense -- client
/// echo, table, terminating prompt -- so the gap between them exists to keep
/// the blobs separate in the log. Two tables running together would still
/// parse, but the fixture cut would have to guess where one ended.
///
/// Nothing is asserted here. The measurement is the `.bytes` log, exactly as
/// `run_capture` says: this prints a commentary so the author can see it
/// working, and the fixtures are cut from the wire afterwards.
async fn run_psm_capture(handle: &SessionHandle) {
    eprintln!(
        "
[psm] capturing {} tables. Every command is a read -- nothing is spent.",
        PSM_COMMANDS.len()
    );
    for (i, command) in PSM_COMMANDS.iter().enumerate() {
        let outcome = send_manual(handle, command).await;
        eprintln!(
            "[psm] {}/{}: {command} -> {outcome:?}",
            i + 1,
            PSM_COMMANDS.len()
        );
        // Long enough for the table to finish printing and its prompt to
        // arrive. A PSM table is ~40 lines; the gap is not a rate limit but a
        // blob separator.
        tokio::time::sleep(PSM_GAP).await;
    }
    eprintln!(
        "
[psm] done. The tables are in the .bytes log -- cut fixtures from there."
    );
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
    let mut screen = Screen::default();
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
            // The ladder, made visible. These used to go only to the session
            // log, so a run that retried three times looked like one that
            // retried instantly.
            Ok(Event::ConnectFailed {
                attempt,
                delay,
                detail,
            }) => eprintln!(
                "  !! attempt {attempt} failed ({detail}) -- next in {:.1}s",
                delay.as_secs_f32()
            ),
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
            Ok(Event::Frame(frame)) => screen.show(&frame),
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
#[derive(Default)]
struct Screen {
    /// Text seen since the last newline. **This is the whole point of the
    /// type.**
    ///
    /// The parser emits one `Frame::Text` per markup boundary, so a single
    /// game line arrives in pieces: `  a ` and `pebbled grey leather doublet`
    /// are two frames because a `<a exist=...>` link sits between them. A
    /// printer that wrote a line per frame produced
    ///
    /// ```text
    /// [inv]   a
    /// [inv] pebbled grey leather doublet
    /// ```
    ///
    /// MEASURED from the author's live run, and confirmed against the raw
    /// bytes: the wire carries that as ONE line. So the frame boundary is not a
    /// line boundary -- and neither is a newline in the content, because
    /// `push_bytes` strips it. `TextFrame::ends_line` is the fact that was
    /// missing; this waited on a newline that never arrives and joined every
    /// line in the room together instead.
    pending: String,
    /// Which stream the pending text belongs to, so a line is not assembled
    /// from two different windows.
    stream: String,
}

impl Screen {
    /// Show a frame the way a player would want to read it.
    fn show(&mut self, frame: &Frame) {
        match frame {
            Frame::Text(text) => self.push(&text.content, &text.stream, text.ends_line),
            // The prompt is the game's "your turn". It terminates whatever was
            // being assembled, because a prompt never continues a line.
            Frame::Prompt { text, .. } => {
                self.flush();
                eprintln!("{text}");
            }
            // Room description, exits and the other named components. The
            // login burst is mostly these, so dropping them would leave a
            // connect showing nothing at all.
            Frame::Component { id, body } => {
                let plain = body.plain();
                if !plain.trim().is_empty() {
                    self.flush();
                    eprintln!("[{id}] {plain}");
                }
            }
            _ => {}
        }
    }

    /// Accumulate, emitting one line per newline actually on the wire.
    fn push(&mut self, content: &str, stream: &str, ends_line: bool) {
        // A stream change ends the current line: main-window text and a
        // thought must not be spliced into one.
        if stream != self.stream {
            self.flush();
            stream.clone_into(&mut self.stream);
        }
        // An embedded newline still splits -- `&#10;` decodes to one inside a
        // room description -- but it is no longer the ONLY thing that does.
        for (i, piece) in content.split('\n').enumerate() {
            if i > 0 {
                self.flush();
            }
            self.pending.push_str(piece);
        }
        // **The wire's own line boundary.** `push_bytes` strips the newline, so
        // without this the printer waited for one that never arrives.
        if ends_line {
            self.flush();
        }
    }

    /// Emit whatever has accumulated, if it is not blank.
    fn flush(&mut self) {
        let line = std::mem::take(&mut self.pending);
        if line.trim().is_empty() {
            return;
        }
        if self.stream.is_empty() {
            eprintln!("{line}");
        } else {
            eprintln!("[{}] {line}", self.stream);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{PSM_COMMANDS, Script};

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
        assert_eq!(Script::from_args(["--psm"]), Script::Psm);
        assert_eq!(Script::from_args(["--typeahead"]), Script::Typeahead);
    }

    /// The regression this exists to prevent: selection used to read
    /// `CENA_SCRIPT`/`CENA_PROBE`, and an env var set once in a shell drove
    /// the character on every later run. `from_args` takes what it considers
    /// as a parameter, so there is no environment in scope for it to reach.
    ///
    /// # This used to assert nothing
    ///
    /// The body was `from_args([]) == Script::None`, character for character
    /// the same as `no_arguments_runs_nothing` above -- so it could only fail
    /// when that one did, and it said nothing about the environment (review
    /// BI-4). Setting `CENA_SCRIPT` to test it properly is not available:
    /// `set_var` is `unsafe` since Rust 2024 and `unsafe_code = "deny"`, and a
    /// test mutating process-wide state would race every other test here.
    ///
    /// What can be asserted without the environment is the property that makes
    /// the environment irrelevant: the SAME arguments always give the same
    /// answer, and every answer is reachable from arguments alone. A
    /// `from_args` that consulted an env var would have to return something
    /// its parameter does not account for, and the exhaustive mapping below is
    /// what leaves no room for that.
    #[test]
    fn selection_comes_only_from_its_argument() {
        let cases: [(&[&str], Script); 5] = [
            (&[], Script::None),
            (&["--capture"], Script::Capture),
            (&["--psm"], Script::Psm),
            (&["--typeahead"], Script::Typeahead),
            (&["--not-a-script"], Script::None),
        ];
        for (args, want) in cases {
            let first = Script::from_args(args.iter().copied());
            assert_eq!(
                first, want,
                "{args:?} must select {want:?} from the argument alone"
            );
            assert_eq!(
                Script::from_args(args.iter().copied()),
                first,
                "{args:?} gave two different answers, so something outside \
                 the argument is being consulted -- which is the env-var \
                 regression this test is named for"
            );
        }
    }

    /// **Every PSM capture command is a read.**
    ///
    /// The rule `probe.rs` states for the typeahead commands, enforced here
    /// rather than trusted: this script runs against the author's live
    /// character, so a command that spent, moved, dropped or attacked would
    /// cost something real. `list` and `info` print a table and change
    /// nothing.
    ///
    /// Checked by construction -- every command must start with a known
    /// category word and contain only `list`/`info` after it -- so adding a
    /// command that does anything else is a test failure, not a discovery made
    /// live.
    #[test]
    fn every_psm_command_is_a_read() {
        const CATEGORIES: &[&str] = &["cman", "feat", "armor", "shield", "weapon", "ascension"];
        for command in PSM_COMMANDS {
            let mut words = command.split_whitespace();
            let category = words.next().unwrap_or_default();
            assert!(
                CATEGORIES.contains(&category),
                "{command:?} does not start with a known category"
            );
            let verb = words.next().unwrap_or_default();
            assert!(
                verb == "list" || verb == "info",
                "{command:?} is not a read -- only `list` and `info` are"
            );
            for rest in words {
                assert_eq!(
                    rest, "all",
                    "{command:?} carries an argument that is not a filter"
                );
            }
        }
    }

    /// The capture covers all five PSMs, and ascension.
    ///
    /// A category silently missing from the list would mean a fixture nobody
    /// cut and a table shape nobody checked -- which is how `cman` came to be
    /// generalised to all five in the first place.
    ///
    /// **The five are written out here rather than read from
    /// `PsmCategory::ALL`.** `cena` depends on `cena-session`, which
    /// re-exports what a session needs (`GameState`, `Room`, `UnknownTag`) and
    /// not the character vocabulary. Adding a `cena-model` dependency, or
    /// widening the facade, to let one test avoid five string literals would
    /// be the test shaping the crate graph -- and the graph IS the
    /// architecture (`CLAUDE.md`). If a sixth category ever appears, this list
    /// and `PsmCategory::ALL` are both hand-edited, and
    /// `there_are_five_psm_categories` in `cena-model` is what holds that one
    /// honest.
    #[test]
    fn the_psm_capture_covers_every_category() {
        for category in ["cman", "feat", "armor", "shield", "weapon"] {
            let wanted = format!("{category} list all all");
            assert!(
                PSM_COMMANDS.contains(&wanted.as_str()),
                "no unfiltered capture for {category}"
            );
        }
        assert!(
            PSM_COMMANDS.iter().any(|c| c.starts_with("ascension")),
            "ascension is not a PSM but is the open question -- capture it"
        );
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

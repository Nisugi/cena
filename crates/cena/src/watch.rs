//! The terminal's view of one session: Hydra's own lines, each tagged with
//! its character, and the one manual send the binary makes itself.
//!
//! It was `run.rs`, which also held M1's walkthroughs and measurement probes
//! (`--demo`, `--capture`, `--psm`, `--typeahead`). Those were retired at M6
//! (author, 2026-09-24: *"get rid of any test behaviors"*, and of the probes,
//! *"delete them"*); git keeps them.

use cena_session::{CommandId, Event, Origin, SessionHandle};
use std::time::Duration;
use tokio::sync::broadcast;

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

/// Print what crosses the wire, both directions, in order.
///
/// **Hydra's own lines, never the game's text.** The browser is the screen:
/// game text echoed here buried the pairing URL (author, 2026-09-23), and two
/// characters' story in one window is what `plan/29` §5a R2 rules out. What
/// prints is lifecycle, notices, sends and retries.
///
/// The interleaving of a manual command with a running behavior is a fact
/// about ordering, and ordering is only observable as a sequence -- so the
/// `->` lines are that evidence, not decoration.
///
/// `who` goes at the front of every line: empty for one session, and
/// `[Nisugi]` when several share the terminal (`plan/29` Q7), so each line
/// says whose it is.
pub(crate) async fn watch_events(mut events: broadcast::Receiver<Event>, who: String) {
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
                eprintln!("{who}  -> [{tag}] {line}");
            }
            Ok(Event::StateChanged(state)) => eprintln!("{who}  .. lifecycle: {state:?}"),
            // Hydra's own voice (`cena_session::notice`). A terminal is
            // already fixed-width, so a table and prose print the same way;
            // the mark says which kind, since there is no colour to.
            Ok(Event::Notice(notice)) => {
                let mark = match notice.kind {
                    cena_session::NoticeKind::Error => "!!",
                    cena_session::NoticeKind::Warn => " !",
                    cena_session::NoticeKind::Info => "::",
                    cena_session::NoticeKind::Debug => "..",
                };
                for line in notice.lines() {
                    eprintln!("{who}  {mark} {line}");
                }
            }
            // The ladder, made visible. These used to go only to the session
            // log, so a run that retried three times looked like one that
            // retried instantly.
            Ok(Event::ConnectFailed {
                attempt,
                delay,
                detail,
            }) => eprintln!(
                "{who}  !! attempt {attempt} failed ({detail}) -- next in {:.1}s",
                delay.as_secs_f32()
            ),
            // REPORTED, NOT ACTED ON. Running the sync means sending up to
            // fifteen commands (`a_full_sync_is_fifteen_commands`), which needs the authority token
            // (`plan/12` §4.2) that this watcher does not hold -- it renders,
            // it does not claim. `cena_behavior::sync` is what runs it, and
            // the character's owner decides whether to spend that traffic.
            Ok(Event::SyncNeeded(groups)) if groups.is_empty() => {
                eprintln!("{who}  .. character store: up to date");
            }
            Ok(Event::SyncNeeded(groups)) => eprintln!(
                "{who}  .. character store: {} group(s) stale -- {}",
                groups.len(),
                groups
                    .iter()
                    .map(|g| format!("{g:?}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            // Not shown here: the game's text is the browser's to show, and a
            // combat view is a frontend's to build.
            Ok(Event::Frame(_) | Event::Combat(_)) => {}
            // Keep watching. A `while let Ok(..)` here ended the watcher on
            // the first lag, which would silence the `-> [manual]` and
            // `-> [behavior]` lines for the rest of the run -- and those
            // lines ARE criterion 5's evidence, so their absence would look
            // exactly like the interleaving failing.
            Err(broadcast::error::RecvError::Lagged(missed)) => {
                eprintln!("{who}  !! {missed} events dropped from the ring (still watching)");
            }
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}

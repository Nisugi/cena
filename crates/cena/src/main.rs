//! The `cena` binary: Milestone 1 Step 2's end-to-end slice, run for real.
//!
//! One character logs in, the session shows the room it is standing in, a
//! behavior runs against the live game, a typed command interleaves with it,
//! and `stop` ends it. That is `plan/12` §7.2 criteria 1-6 with the network
//! attached, rather than against `AnsweringSource`.
//!
//! # This is the ONLY place criterion 1 is exercised
//!
//! `CLAUDE.md`: "do not log into a live game service without the author
//! present." No test in this workspace reaches the network, and
//! `cena-platform`'s `LiveSource` and `eaccess` both carry a BUILT, NOT RUN
//! header
//! saying so. Criterion 1 is met by **the author running this**, watching it,
//! and nothing else.
//!
//! So the output is written to be *read by a person who is deciding whether it
//! passed*. Every stage announces itself; the room is printed from typed
//! frames; the behavior's commands and the manual one are both visible in the
//! order the wire saw them.
//!
//! # Credentials are prompted, never stored -- but RETAINED in memory
//!
//! `plan/12` §7.1 puts "saved credentials, GUI login, web-login fallback"
//! explicitly **Out** for Milestone 1. This prompts on stdin and writes no
//! CREDENTIAL to disk. It is never logged, and the types that hold it redact
//! themselves (`cena_platform::Credentials`'s `Debug`, and `LiveConnector`'s).
//!
//! **The WIRE is written to disk**, and this paragraph used to imply otherwise
//! by saying "writes nothing to disk". The session sink writes a `.bytes` and a
//! `.log` per run and prints both paths; `Redactions` is what keeps the
//! credential out of them. The on-screen banner made the same mistake and is
//! corrected in `main`.
//!
//! **CHANGED in Milestone 2 step 8.** This paragraph used to say the password
//! was kept "only as long as the handshake needs it", and that is no longer
//! true: it lives in [`LiveConnector`] for the session's whole life, because
//! every reconnect is a full re-login and a session that dropped it could
//! never open a second connection. The author's decision, and the reasoning is
//! recorded on `LiveConnector` itself.
//!
//! What has *not* changed is that dropping it never zeroed it anyway -- the
//! old comment admitted as much. The lifetime is longer and now honest;
//! `zeroize` is still the step `plan/12` §7.1 puts Out of scope.
//!
//! **The password comes from the credential ladder** (`secrets.rs`): the OS
//! keyring, the account's environment variable, or a prompt that does not
//! echo. The prompt used to print it as it was typed (`plan/29` Q2).
//!
//! # Why this names `cena-platform` directly
//!
//! **AMENDED 2026-09-18 (author's call), Milestone 1 Step 2.** The binary
//! gains a direct `cena-platform` edge, four layers below it, which
//! `crates/cena-arch-tests/tests/layering.rs` calls out as exactly the
//! shortcut the dependency graph exists to prevent. It is taken deliberately:
//! the login produces a `ByteSource`, the session consumes one, and the only
//! thing that can hold both is the binary that connects them. The alternative
//! -- a `Session::connect(credentials)` constructor -- was considered and
//! declined, because it puts a protocol the session does not otherwise speak
//! inside the session.

mod ask;
mod commands;
mod connector;
mod frontend;
mod interrupt;
mod play;
mod probe;
mod roster;
mod run;
mod secrets;
mod setup;
mod travel;

use ask::ask;
use cena_behavior::look;
use cena_session::{AuthorityToken, CommandId};
use connector::LiveConnector;
use interrupt::unless_interrupted;
use run::{run_or_probe, send_manual, wait_for_room, watch_events};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

/// How long to let the behavior run before the manual command interleaves.
const BEHAVIOR_WARMUP: Duration = Duration::from_secs(3);

/// Cancel the behavior and wait for it, **whatever it does**.
///
/// Split out of `main` under `plan/05` Rule 4.1 -- move code down, do not
/// raise the cap -- when handling the panic case pushed `main` to 103 lines
/// against clippy's 100.
///
/// # Why this does not use `?`
///
/// It used to be `behavior.await?`, so a behavior PANIC returned from `main`
/// and skipped the `quit`, the sink flush and the shutdown report -- the exact
/// reason the supervisor's await refuses `?`, stated there and not honoured
/// here (review BI-2).
///
/// Criterion 6's "no leaked sockets" must not be conditional on a behavior
/// having behaved. A panicking behavior is reported, and the caller goes on to
/// the orderly shutdown.
async fn stop_the_behavior(
    behavior: Option<tokio::task::JoinHandle<Result<(), cena_behavior::BehaviorError>>>,
    stop: &CancellationToken,
) {
    // --- Criterion 4: stop, within PREEMPT_GRACE ---------------------------
    // The latency is MEASURED in `cena-behavior`'s tests under virtual time,
    // where it is a property of the code rather than of this machine's load.
    // Here it is only demonstrated.
    eprintln!("\n[stop] cancelling the behavior");
    let at_stop = std::time::Instant::now();
    stop.cancel();
    let Some(behavior) = behavior else {
        return;
    };
    match behavior.await {
        Ok(result) => eprintln!(
            "[stop] behavior ended in {:?}: {result:?}",
            at_stop.elapsed()
        ),
        Err(e) => eprintln!(
            "[stop] behavior task FAILED after {:?}: {e}. Continuing to the \
             orderly shutdown -- a behavior that panics must not leave the \
             character logged in.",
            at_stop.elapsed()
        ),
    }
}

/// How long to hold the session open before `stop`, from `--hold <seconds>`.
///
/// With no script selected this is the only thing between login and logout, so
/// it is worth being able to say "keep it up while I look at something". An
/// argument rather than an env var for the same reason scripts are
/// ([`run::Script`]): one mechanism, and nothing left set in a shell.
fn hold_for(web_active: bool) -> Option<Duration> {
    const DEFAULT: Duration = Duration::from_secs(10);
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        // `--hold=60` is the other form people type, and it silently became
        // the default because the match was for the bare flag only.
        if let Some(inline) = arg.strip_prefix("--hold=") {
            return Some(parse_hold(inline).unwrap_or(DEFAULT));
        }
        if arg == "--hold" {
            return Some(
                args.next()
                    .and_then(|raw| parse_hold(&raw))
                    .unwrap_or(DEFAULT),
            );
        }
    }
    (!web_active).then_some(DEFAULT)
}

/// Parse a `--hold` value, **saying so when it cannot**.
///
/// `--hold abc` used to become ten seconds in silence: the `and_then(ok)`
/// discarded the parse failure and the loop carried on, so a typo looked
/// exactly like a working flag and the difference showed up only as a session
/// that ended sooner than asked (review BI-4).
///
/// It warns rather than exiting because this flag is a convenience on a
/// demonstration binary -- refusing to start over a mistyped hold would be a
/// worse trade than starting with the default and saying so.
fn parse_hold(raw: &str) -> Option<Duration> {
    match raw.trim().parse::<u64>() {
        Ok(seconds) => Some(Duration::from_secs(seconds)),
        Err(e) => {
            eprintln!(
                "[args] --hold {raw:?} is not a number of seconds ({e}); \
                 using the default. Write it as `--hold 60` or `--hold=60`."
            );
            None
        }
    }
}

/// How long to wait for the server to close after `quit` (`plan/16` §5b.3).
///
/// Lich's `SERVER_EXIT_TIMEOUT_SECONDS`, taken verbatim
/// (`reference/lich-5/lib/common/orderly_shutdown.rb:16`). It is a bound on
/// politeness, not on correctness: the socket closes either way.
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

/// How many `search` commands the capture sends.
///
/// `search` produces a real roundtime with no combat and nothing at stake --
/// the author's suggestion, and the cheapest way to make a roundtime happen on
/// purpose. Six gives several independent roundtimes to fit against, which is
/// what separates a measurement from an anecdote.
pub(crate) const CAPTURE_SEARCHES: usize = 6;

/// The gap between capture commands.
///
/// Longer than a search's roundtime, so each one is measured from a standing
/// start rather than overlapping the last. The idle time is not wasted: every
/// `<prompt>` that arrives in it is a clock sample.
pub(crate) const CAPTURE_GAP: Duration = Duration::from_secs(6);

/// The gap between PSM capture commands.
///
/// Not a rate limit: it is a **blob separator**. Each command's output is
/// bounded by its terminating prompt (`plan/15` §2a.4b), and leaving room
/// between them keeps two tables from running together in the log, which would
/// make the fixture cut guess where one ended.
///
/// Shorter than `CAPTURE_GAP` because no roundtime has to expire -- a `list`
/// costs nothing and produces none.
pub(crate) const PSM_GAP: Duration = Duration::from_secs(3);

/// How long to wait for the first room description after login.
///
/// The login burst carries the room unprompted (MEASURED 2026-09-18: 2,347
/// bytes including room, exits and inventory), so this is generous rather than
/// tight -- a slow link should not look like a protocol failure.
pub(crate) const ROOM_DEADLINE: Duration = Duration::from_secs(20);

/// A monotonic `CommandId` source, seeded at 0 -- never from a clock or a
/// random, so a recording replays to the same ids (criterion 7).
fn ids() -> impl FnMut() -> CommandId {
    let next = Arc::new(AtomicU64::new(0));
    move || CommandId(next.fetch_add(1, Ordering::Relaxed))
}

/// What the operator is told before anything happens.
///
/// A function rather than eight lines in `main` because it has its own reason to
/// change -- what is true about credentials and logging -- and because it has
/// been WRONG TWICE, each time claiming less was kept than actually was:
///
/// 1. "Not stored" became false at M2 step 8: reconnecting is a full re-login,
///    so the password stays for the session.
/// 2. "Nothing is written to disk" was false from the moment the session sink
///    landed. This banner said it while the same `main` printed two file paths
///    eight lines later. A reassurance the next screenful contradicts is worse
///    than no reassurance: it is the kind of claim someone acts on.
///
/// **Hydra**, not `cena`: `CLAUDE.md` says anything user-facing carries the real
/// name, and the working name is the repository, the crate prefixes and the env
/// vars.
fn banner() {
    eprintln!("Hydra -- one supervised session against the live game.");
    eprintln!("{BANNER_NOTE}");
}

/// The banner's second paragraph, a constant so a test can read it.
///
/// Two sentences on two lines, then a blank one. It had collapsed to "for the
/// (ten spaces) session," -- a `\` continuation whose backslash and newline
/// were lost and whose next-line indent was kept, so the operator read a gap
/// mid-sentence (review finding 14).
const BANNER_NOTE: &str = "The password is never logged, but it IS kept in memory for the \
     session,\nbecause reconnecting re-logs in. The wire IS written to disk -- \
     see the\nlog paths below.\n";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    banner();

    // `--character`, once or more: several characters on the session table
    // (`play.rs`). Without it, the one-character path below, prompted.
    let characters = play::characters();
    if !characters.is_empty() {
        return play::play(characters).await;
    }

    let typed = ask()?;
    eprintln!();
    // From here on Ctrl-C means "shut down in order", not "die". Installed
    // after the prompts: before a session exists there is nothing to order.
    let interrupt = interrupt::on_ctrl_c();

    // --- Criterion 1, now SUPERVISED ---------------------------------------
    //
    // The credentials move into the connector and stay there for the session's
    // lifetime. `main` used to drop them the moment the handshake finished and
    // explain why; that is no longer possible, and the reason is the author's:
    //
    //   AUTHOR: "I mean I don't understand the question. When would it get a
    //            new socket that doesn't require a re-login?"
    //
    // It never would -- so a session that can reconnect is a session that keeps
    // its password. `LiveConnector` carries the full cost, including what the
    // old comment already admitted: dropping a `String` never zeroed it.
    //
    // NOTHING CONNECTS HERE. `SupervisedSession::new` touches no network; the
    // login happens inside `run()`, once per generation.
    //
    // The eaccess certificate pin lives in the data directory, beside the
    // character and menu stores, under Lich's and VellumFE's name.
    let pin = cena_session::character_store::data_dir().join(cena_platform::PIN_FILENAME);
    // Remembered in the roster once the login proves it, so `--character`
    // can start this character next time without asking.
    // The roster entry, and a typed password's keyring offer, once proven.
    let proven = play::Proven::of(&typed);
    let connector = LiveConnector::new(typed, run::login_provider(), pin);
    // The handle comes back WITH the session, because
    // `SupervisedSession::new` mints it: it must be obtainable before `run`
    // consumes the session, and there is no `handle()` accessor to call
    // afterwards.
    let (session, handle, combat_flush, player_flush) = setup::open_session(connector);
    // Hydra's command line, before anything connects (`commands.rs`).
    let commands = commands::Commands::install(&handle);
    let observer = session.observer();
    proven.on_ready(&observer, &std::sync::Arc::default());
    let session_cancel = session.cancel_token();
    let (_snapshot, mut events) = session.subscribe();
    // A SECOND receiver, for the probe. `events` is moved into the watcher
    // task below, and a `broadcast` receiver cannot be shared -- each one gets
    // its own copy of the stream. Taken here rather than later because
    // `session` is consumed by `run()`, and because a receiver only
    // sees what arrives after it is created: subscribing at the probe's own
    // call site would silently drop everything the login sent.
    let (_probe_snapshot, mut probe_events) = session.subscribe();
    // The supervisor's `run` IS the login: it connects, runs one actor over
    // the connection, and opens another if the reason warrants it. Everything
    // below happens against whichever generation is current.
    let supervisor = tokio::spawn(session.run());
    let frontend = frontend::Frontend::start(observer.clone(), handle.clone()).await;

    // --- Criterion 2: the room, from TYPED FRAMES --------------------------
    eprintln!("[waiting] for the first room description frame...");
    let room_shown =
        unless_interrupted(&interrupt, wait_for_room(&mut events, frontend.is_none())).await;

    if room_shown == Some(false) {
        eprintln!(
            "[FAIL criterion 2] no room description frame within {ROOM_DEADLINE:?}. \
             The session is connected -- 'connected' is not 'working' (plan/10 \
             §10.3a). Check the WRAYTH banner reached the server."
        );
    }

    // --- Criteria 3-5: the behavior, and a command interleaved with it -----
    //
    // **Behind `--demo`, because it SENDS.** A `look` every second for the
    // whole run, plus a manual one at three seconds -- about thirteen commands
    // on a default run. Scripts were made opt-in and this was left
    // unconditional, so "quiet, always" was not true:
    //
    //   AUTHOR: "So whyt am I sending look every second still?"
    //
    // The behavior and the manual surface hold the SAME handle type, cloned
    // from the same session. Criterion 3's "the same queue as the behavior's"
    // is structural here, not asserted.
    let stop = CancellationToken::new();
    let demo = run::demo_requested();

    // The event watcher runs either way: it only READS, and a quiet session is
    // still worth watching.
    let watcher = tokio::spawn(watch_events(events, frontend.is_none(), String::new()));

    // **Every phase from here to the hold is raced against Ctrl-C**, and an
    // interrupted one falls through to the SAME orderly shutdown below. This
    // comment used to make that claim for the hold alone, which was the only
    // phase that had a handler: Ctrl-C during the room wait, `--demo`, a
    // capture or `--first` killed the process -- no `quit` (plan/16 5b), no
    // sink flush, the combat thread never joined (review finding 8).
    // `interrupt.rs` has the mechanism.
    let behavior = run_demo(&handle, demo, &stop, &interrupt).await;

    unless_interrupted(&interrupt, run_or_probe(&handle, &mut probe_events, &stop)).await;

    let desk = travel::after_login(&handle, observer.clone(), &commands);
    unless_interrupted(&interrupt, desk).await;

    frontend::wait_for_stop(hold_for(frontend.is_some()), &supervisor, &interrupt).await;

    // --- Criterion 4: stop, within PREEMPT_GRACE ---------------------------
    // The latency is MEASURED in `cena-behavior`'s tests under virtual time,
    // where it is a property of the code rather than of this machine's load.
    // Here it is only demonstrated.
    stop_the_behavior(behavior, &stop).await;
    if let Some(frontend) = frontend {
        frontend.shutdown().await;
    }

    // --- Criterion 6: clean disconnect -------------------------------------
    //
    // ORDERLY SHUTDOWN (`plan/16` §5b), in Lich's order:
    //
    //   drain behaviors -> save -> request server exit -> close
    //
    // The behaviors were drained above (criterion 4). There is nothing to save
    // yet -- §5b.5 records that step as having no content in Cena today, and
    // the ordering is built now precisely so that the save has somewhere to go
    // when there IS something: an exit that skips it is a corruption path.
    //
    // Then `quit`. The author's reason for wanting it is not politeness to the
    // game: "it sends the signal to lich to shutdown so it has time to save
    // everything without corruption". Until tonight every Cena exit was
    // indistinguishable from a crash from the game's side (§5b.4).
    eprintln!("\n[disconnect] sending quit and awaiting the server's close");
    let farewell = handle.quit(EXIT_TIMEOUT).await;
    eprintln!("[disconnect] farewell={farewell:?}");

    // The cancel runs REGARDLESS of how the quit went. §5b's ordering note is
    // explicit: criterion 6's "no leaked sockets" must not become conditional
    // on the server cooperating. A quit that timed out still ends the session.
    session_cancel.cancel();
    // NOT `actor.await?`. `plan/12` §5.5 says "a panic kills one session, not
    // the process", and criterion 6's evidence is the shutdown report below --
    // so a panicking actor is exactly the case where that report matters most,
    // and `?` here would skip it and hide the panic inside a `JoinError`.
    let joined = supervisor.await;
    watcher.abort();
    setup::flush_combat(combat_flush);
    setup::flush_player_log(player_flush).await;

    match joined {
        Ok(end) => {
            // **No `source` to ask, and that is the change a supervisor makes.**
            // It owns one source per generation and shuts each down as that
            // connection ends (`SessionActor::shutdown`), so there is no single
            // socket left for `main` to interrogate. Criterion 6 moved INTO the
            // loop rather than out of the program -- which is why the evidence
            // here is the reason it stopped, not a socket's state.
            eprintln!(
                "[disconnect] stopped_because={:?}, last connection ended {:?}, connections={}, {} events recorded",
                end.stopped_because,
                end.reason,
                // The COUNT of successful connections, not the generation
                // number. `generations.0 + 1` printed "1" for a session that
                // never connected at all, because the counter starts at zero
                // and only advances on a reconnect.
                connections_made(&end),
                end.recorder.len()
            );

            // **A failed login must not exit 0 saying "Done".** Headless mode
            // has nothing but the exit status to go on, and a mistyped
            // character name printed the success banner and returned Ok.
            match &end.stopped_because {
                cena_session::StoppedBecause::Fatal(error) => {
                    eprintln!(
                        "
[FAIL] the session never started: {error}. Nothing was exercised."
                    );
                    return Err(format!("login refused: {error}").into());
                }
                cena_session::StoppedBecause::Unattended => {
                    // NOT an error: the session was re-openable and nobody was
                    // there. Vellum surfaces this as "Session looked idle".
                    eprintln!(
                        "
[idle] no command was sent across {} connection(s), so the session stopped rather than re-logging in all night.",
                        connections_made(&end)
                    );
                }
                cena_session::StoppedBecause::Cancelled => {
                    // Criteria 1-6 and `plan/16` 5b's orderly shutdown. NOT
                    // criterion 9 unless the connection actually dropped: a
                    // clean run never reconnects. The connection count above is
                    // the evidence either way.
                    eprintln!(
                        "
Done. Criteria 1-6 and the orderly shutdown exercised against the live server."
                    );
                }
            }
            Ok(())
        }
        Err(join) => {
            // The socket goes with the process, but say so plainly rather than
            // letting a JoinError stand in for the panic message.
            eprintln!(
                "[FAIL criterion 6] the session task did not end cleanly: {join}. \
                 The socket is closed by process exit, which is NOT the same as \
                 the clean shutdown criterion 6 asks for."
            );
            Err(join.into())
        }
    }
}

/// The criteria 3-5 walkthrough: a `look` behavior, and a manual command
/// interleaved with it.
///
/// **Returns `None` when `--demo` was not passed**, and sends nothing in that
/// case. Split from `main` under `plan/05` Rule 4.1 when gating it pushed that
/// function past clippy's 100-line limit.
async fn run_demo(
    handle: &cena_session::SessionHandle,
    demo: bool,
    stop: &CancellationToken,
    interrupt: &CancellationToken,
) -> Option<tokio::task::JoinHandle<Result<(), cena_behavior::BehaviorError>>> {
    if !demo {
        eprintln!(
            "
[behavior] none -- nothing is being sent. Pass `-- --demo` for the walkthrough.
"
        );
        return None;
    }

    let behavior_handle = handle.clone();
    let behavior_stop = stop.clone();
    eprintln!(
        "
[behavior] starting `look`, interval 1s. Watch for interleaving.
"
    );
    let behavior = tokio::spawn(async move {
        look(&behavior_handle, &behavior_stop, ids(), AuthorityToken(1)).await
    });

    // The warm-up and the manual command are raced against Ctrl-C; the SPAWN
    // above is not, so the behavior's handle always comes back to `main`,
    // which cancels and awaits it whether or not this finished.
    unless_interrupted(interrupt, interleave_manual(handle, &behavior)).await;
    Some(behavior)
}

/// Criterion 5: a manual command MID-BEHAVIOR, after the behavior has warmed
/// up. Split from [`run_demo`] so the wait can be interrupted without losing
/// the behavior's handle.
async fn interleave_manual(
    handle: &cena_session::SessionHandle,
    behavior: &tokio::task::JoinHandle<Result<(), cena_behavior::BehaviorError>>,
) {
    tokio::time::sleep(BEHAVIOR_WARMUP).await;

    // Criterion 5: a manual command MID-BEHAVIOR. It jumps the queue, runs its
    // round trip, and the behavior CONTINUES -- only an explicit stop preempts
    // (`plan/12` §4.1, corrected 2026-09-18).
    eprintln!(
        "
[manual] typing `look` mid-behavior -- the behavior must continue
"
    );
    let outcome = send_manual(handle, "look").await;
    eprintln!(
        "
[manual] outcome: {outcome:?}"
    );
    eprintln!(
        "[criterion 5] behavior still running: {}
",
        !behavior.is_finished()
    );
}

/// How many connections actually carried traffic.
///
/// `Generation` counts from zero and advances only on a **reconnect**, so
/// `generations.0 + 1` reads "1 connection" for a session that never connected
/// at all -- which is exactly what a refused login is.
fn connections_made(end: &cena_session::SupervisedEnd) -> u32 {
    if matches!(end.stopped_because, cena_session::StoppedBecause::Fatal(_)) {
        0
    } else {
        end.generations.0 + 1
    }
}

#[cfg(test)]
mod tests {
    use super::{BANNER_NOTE, parse_hold};
    use std::time::Duration;

    /// The banner reads as two sentences, not as a sentence with a hole in it.
    ///
    /// Review finding 14: a lost `\` continuation left its next-line indent in
    /// the string, and the operator saw "for the          session,".
    #[test]
    fn the_banner_has_no_collapsed_gap() {
        assert!(!BANNER_NOTE.contains("  "), "{BANNER_NOTE:?}");
        assert!(BANNER_NOTE.contains("for the session,\nbecause"));
    }

    /// A malformed `--hold` is reported, not silently defaulted.
    ///
    /// `--hold abc` used to become ten seconds without a word, so a typo was
    /// indistinguishable from a working flag until the session ended early
    /// (review BI-4). `None` is what makes the caller fall back *and say so*.
    #[test]
    fn a_hold_that_is_not_a_number_is_refused() {
        assert_eq!(parse_hold("61"), Some(Duration::from_secs(61)));
        assert_eq!(parse_hold(" 61 "), Some(Duration::from_secs(61)));
        assert_eq!(parse_hold("0"), Some(Duration::from_secs(0)));

        for bad in ["abc", "", "60s", "-5", "6.0", "1e3"] {
            assert_eq!(
                parse_hold(bad),
                None,
                "{bad:?} is not a number of seconds and must be refused, so \
                 the caller falls back loudly rather than silently"
            );
        }
    }
}
